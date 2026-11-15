//! Headless pipeline: `validate`, `inspect`, `import`, `export`, `check`, `postp validate`, `tools`,
//! `parts`, `cutlist`, `report` and `script`.
//!
//! This binary is the verification surface of the whole application: every behaviour that matters
//! must be reachable here, because a behaviour that only exists in a window cannot be tested.
//!
//! Contract: machine output on stdout, human diagnostics on stderr, and these exit codes:
//! `0` success, `2` invalid input, `3` unsupported schema, `4` post-processor error, `5` vector
//! issues, `7` a nesting that could not place every instance (M9).
//!
//! Every command's logic lives in a function that its argument-parsing arm calls, and the scripted
//! command language (`script`) calls the same functions (M10 invariant 1), so no behaviour has a
//! second implementation.
//!
//! Command implementations live in [`commands`], one module per command, so adding a new command
//! (for example `drill` or `cutout` in ola 4) means adding a file under `commands/` and registering
//! it in the dispatcher without editing unrelated command code.

pub mod commands;
pub mod pieces;
pub mod printsheet;
pub mod script;

// Preserve the historical crate-root API used by `script`, `pieces`, `printsheet`, tests, and the
// binary. Command implementations moved to `commands/*`; shared helpers remain here.
pub use commands::validate::report as validate_report;

use std::io::Write;
use std::path::{Path, PathBuf};

use cam_model::{Document, NestingPart, NestingSheet};
use cam_units::Format;

const HELP: &str = "\
cam-cli — headless CAM pipeline

USAGE:
    cam-cli <command> [options]

COMMANDS:
    validate <document>       Check the document's schema and referential invariants
    inspect <document>        Print layer, entity and toolpath counts
    import <input>            Import DXF or SVG into a new document
        --out <path>          Write the document here; without it the report goes to stdout
        --layer <name>        Put every imported contour in one layer with this name
        --tolerance <mm>      Flattening tolerance for curves (default 0.01)
    export <document>         Write the document's geometry back out as DXF or SVG
        --format <name>       dxf or svg (required)
        --out <path>          Write the file here instead of stdout
    check <document>          Report open and self-intersecting vectors
    drill <excellon>          Drill from an Excellon table: group by tool, order deterministically,
                              and emit G-code with hit and tool-change counts
        --post <file>         Post-processor (.pp) to emit with (required)
        --cut-depth <mm>      Total cut depth below the surface (required)
        --peck-depth <mm>     Peck depth; 0 cuts in one plunge (default 0)
        --start-depth <mm>    Depth at which cutting begins (default 0)
        --retract <mode>      full or an amount in mm above each peck (default full)
        --slot-mode <mode>    drill or trajectory for slot features (default drill)
        --job <document>      Document whose board/origin maps coordinates
        --out <path>          Write the G-code to a file instead of stdout
    parts <document>          Author nesting parts: --csv <file> from a four-column list, or
                              --entity <id> --instances <n> [--id <id>] [--name <name>] for one part;
                              --out <path> writes the document elsewhere
    cutlist <document>        Report the nesting's placements as a cut list: one header, one row per
                              placed instance, every dimension in mm, or a refusal if none is stored
        --out <path>          Write the CSV to a file instead of stdout
    report <document>         Write an HTML print sheet of the nesting: a board card per sheet, every
                              placement drawn, the utilisation the document holds, and what a sheet
                              cannot honestly show named in an Omitted section. Every style and every id
                              is inside the file, so it prints on its own. Refuses a document with no
                              nesting stored
        --out <path>          Write the HTML to a file instead of stdout
    tools list <library>      List the tools in a SQLite library
    tools add <library>       Add a tool (--id, --diameter, --number; --kind, geometry and cutting flags)
    postp validate <file>     Parse a .pp post-processor and report what it declares
    script <file.cam>         Run a command language script against the document it opens. It takes no
                              --out and writes nothing except the document a `save` line names. It
                              reads the sheet, part, parts and cutlist commands, and the same report every
                              command prints on the command line. A refusal names the line and the
                              column, and the document is left exactly as the file held it

EXIT CODES:
    0   success
    2   invalid input or a document the command cannot read
    3   unsupported document schema
    4   post-processor error
    5   vector issues found by check
";

/// A failure with the exit code the contract assigns to it.
#[derive(Debug)]
pub struct Failure {
  pub code: i32,
  pub message: String,
}

impl std::fmt::Display for Failure {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Failure {
  pub fn code(&self) -> i32 {
    self.code
  }

  pub fn input(message: impl Into<String>) -> Self {
    Self {
      code: 2,
      message: message.into(),
    }
  }

  pub fn schema(message: impl Into<String>) -> Self {
    Self {
      code: 3,
      message: message.into(),
    }
  }

  pub fn post(message: impl Into<String>) -> Self {
    Self {
      code: 4,
      message: message.into(),
    }
  }

  pub fn vectors(message: impl Into<String>) -> Self {
    Self {
      code: 5,
      message: message.into(),
    }
  }

  /// The same failure, said at a position in a script: the `cam-post` parser's diagnostic shape,
  /// `line {line}:{column}: {message}` (M10 invariant 2).
  ///
  /// It rewrites the message and keeps the code, so a scripted command exits with the code its
  /// subcommand would have: a scripted command keeps the code its subcommand would produce, not a
  /// generic `2` that hides what happened.
  pub fn at(self, line: usize, column: usize) -> Self {
    Self {
      code: self.code,
      message: format!("line {line}:{column}: {}", self.message),
    }
  }
}

/// A short label for an operation kind, for reports.
pub(crate) fn operation_label(operation: &cam_model::Operation) -> &'static str {
  match operation {
    cam_model::Operation::Isolation(_) => "isolation",
    cam_model::Operation::NonCopperClear(_) => "non-copper clear",
    cam_model::Operation::Cutout(_) => "cutout",
    cam_model::Operation::Paint(_) => "paint",
    cam_model::Operation::Panelize(_) => "panelize",
  }
}

/// Write a report to the sink its caller chose: the one way a report leaves this crate.
///
/// `print!` panics when its own write fails, which a caller holding a buffer should never inherit, so a
/// sink that refuses the bytes becomes a refusal instead.
pub(crate) fn write_report(sink: &mut dyn Write, report: &str) -> Result<(), Failure> {
  sink
    .write_all(report.as_bytes())
    .map_err(|error| Failure::input(format!("cannot write the report: {error}")))
}

pub fn run(arguments: &[String]) -> Result<(), Failure> {
  let Some(command) = arguments.first() else {
    print!("{HELP}");
    return Err(Failure::input("no command given; see --help"));
  };

  if command == "--help" || command == "-h" || command == "help" {
    print!("{HELP}");
    return Ok(());
  }

  let rest = &arguments[1..];
  match command.as_str() {
    "validate" => commands::validate::run(rest),
    "inspect" => commands::inspect::run(rest),
    "postp" => commands::postp::run(rest),
    "import" => commands::import::run(rest),
    "export" => commands::export::run(rest),
    "check" => commands::check::run(rest),
    "drill" => commands::drill::run(rest),
    "parts" => pieces::parts_command(rest),
    "cutlist" => pieces::cutlist_command(rest),
    "report" => printsheet::report_command(rest),
    "tools" => commands::tools::run(rest),
    "script" => script::script_command(rest),
    other => Err(Failure::input(format!(
      "unknown command {other:?}; see --help"
    ))),
  }
}

/// The single positional argument a command expects.
pub fn only_path(rest: &[String], what: &str) -> Result<PathBuf, Failure> {
  match rest {
    [path] if !path.starts_with('-') => Ok(PathBuf::from(path)),
    [] => Err(Failure::input(format!("{what} requires a path"))),
    _ => Err(Failure::input(format!(
      "{what} takes exactly one path, got {} argument(s)",
      rest.len()
    ))),
  }
}

/// Load a document, mapping the schema mismatch onto its own exit code.
pub fn open(path: &Path) -> Result<Document, Failure> {
  let file = DocumentFile::load(path).map_err(|error| match error {
    cam_store::StoreError::Invalid { .. } => classify_invalid(path, &error),
    other => Failure::input(other.to_string()),
  })?;
  Ok(file.document)
}

use cam_store::document::DocumentFile;

/// An invalid document is either a schema problem or an invariant problem; the messages differ in
/// meaning and the exit code has to as well.
pub fn classify_invalid(path: &Path, error: &cam_store::StoreError) -> Failure {
  let text = error.to_string();
  if text.contains("unsupported document schema") {
    Failure::schema(format!("{}: {text}", path.display()))
  } else {
    Failure::input(format!("{}: {text}", path.display()))
  }
}

/// The value that follows an option.
pub fn option_value<'a>(rest: &'a [String], index: usize, name: &str) -> Result<&'a str, Failure> {
  rest
    .get(index + 1)
    .map(String::as_str)
    .ok_or_else(|| Failure::input(format!("{name} requires a value")))
}

/// The number that follows an option.
pub fn number(rest: &[String], index: usize, name: &str) -> Result<f64, Failure> {
  let value = option_value(rest, index, name)?;
  value
    .parse::<f64>()
    .map_err(|_| Failure::input(format!("{name} expects a number, got {value:?}")))
}

/// The next free `S{n}` sheet id: the `next_toolpath_id` scan, from `len + 1` upwards, so a document
/// that authors its own ids never collides with one a run opens.
pub fn fresh_sheet_id(sheets: &[NestingSheet]) -> String {
  let mut number = sheets.len() + 1;
  loop {
    let id = format!("S{number}");
    if sheets.iter().all(|sheet| sheet.id != id) {
      return id;
    }
    number += 1;
  }
}

/// The next free `P{n}` part id: the `fresh_sheet_id` scan, so a scripted `part` that authors one
/// part per line names them P1, P2, ... and never collides with a part the document already holds.
pub fn fresh_part_id(parts: &[NestingPart]) -> String {
  let mut number = parts.len() + 1;
  loop {
    let id = format!("P{number}");
    if parts.iter().all(|part| part.id != id) {
      return id;
    }
    number += 1;
  }
}

/// A millimetre figure with the report's three decimals, through `cam-units` so it is locale-free.
pub fn decimal(value: f64) -> String {
  Format::fixed(3).apply(value)
}

/// The value that follows an option at `index`, or the refusal that names the missing value.
pub(crate) fn value_at(rest: &[String], index: usize, what: &str) -> Result<String, Failure> {
  rest
    .get(index + 1)
    .cloned()
    .ok_or_else(|| Failure::input(format!("{what} requires a value")))
}
