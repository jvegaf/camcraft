//! Headless pipeline: `validate`, `inspect`, `import`, `export`, `check`, `postp validate`, `tools`,
//! `nest`, `parts`, `cutlist`, `report` and `script`.
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

pub mod pieces;
pub mod printsheet;
pub mod script;

use std::io::Write;
use std::path::{Path, PathBuf};

use cam_geom::nest::{NestAlgorithm, NestOptions, NestPlan, NestRotation};
use cam_geom::nest_profile::{ProfileNestOptions, ProfileNestPlan};
use cam_geom::{Bounds, Contour, Point2, Vec2};
use cam_model::{Document, Nesting, NestingCost, NestingPart, NestingPlacement, NestingSheet};
use cam_post::parse_post;
use cam_store::document::DocumentFile;
use cam_store::tools::{CuttingData, ToolGeometry, ToolLibrary};
use cam_units::{Angle, Format, Length};

const HELP: &str = "\
cam-cli — headless CAM pipeline

USAGE:
    cam-cli <command> [options]

COMMANDS:
    validate <document>       Check the document's schema and referential invariants
    inspect <document>        Print layer, entity and toolpath counts
    import <input>            Import DXF, SVG, PDF, EPS or PNG into a new document
        --out <path>          Write the document here; without it the report goes to stdout
        --layer <name>        Put every imported contour in one layer with this name
        --tolerance <mm>      Flattening tolerance for curves (default 0.01)
        --dpi <n>             Raster trace scale (default 96)
        --threshold <0-255>   Raster trace luminance cut-off (default 128)
        --invert              Trace the light regions instead of the dark ones
        --min-area <mm2>      Drop traced regions smaller than this area
        --simplify <mm>       Simplify traced contours within this distance
    export <document>         Write the document's geometry back out as DXF or SVG
        --format <name>       dxf or svg (required)
        --out <path>          Write the file here instead of stdout
    check <document>          Report open and self-intersecting vectors
    nest <document>           Nest its parts onto its sheets and store the placements and the cost; a
                              part left unplaced is reported with its id and exits 7
        --sheets <WxH>        A board to nest on, as width x height in millimetres; repeat it once per
                              authored sheet. It replaces the document's own sheets, because a sheet is
                              an authored record that lives in the document. Required when the document
                              authors none
        --gap <mm>            Minimum separation between parts on a board (default 0)
        --rotation <on|off>   Whether a part may turn a quarter turn to fit (default on)
        --algorithm <mode>    rect-bssf, rect-blsf, rect-baf or rect-guillotine (default rect-bssf);
                              a choice a rectangle honours and a free profile ignores in favour of
                              its own heuristic
        --step <mm>           Resolution of the free-profile placement grid; a non-rectangle honours
                              it and a rectangle ignores it (default 1)
        --out <path>          Write the nested document here instead of in place
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
    tools add <library>       Add a tool (--id, --diameter, --number)
    postp validate <file>     Parse a .pp post-processor and report what it declares
    script <file.cam>         Run a command language script against the document it opens. It takes no
                              --out and writes nothing except the document a `save` line names. It
                              reads the nesting, part and cutlist commands, and the same report every
                              command prints on the command line. A refusal names the line and the
                              column, and the document is left exactly as the file held it

EXIT CODES:
    0   success
    2   invalid input or a document the command cannot read
    3   unsupported document schema
    4   post-processor error
    5   vector issues found by check
    7   a nesting could not place every instance, so nothing was written (nest only)
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

  /// A nesting that could not place every instance the document asks for (M9).
  ///
  /// This is a **finding**, not invalid input: every flag was usable and every record in the document
  /// is sound, and the report is what contradicts the request. It gets a code of its own because `6`
  /// is the simulation's finding and is spoken for, and because a caller scripting a cut has to see
  /// `0` only when every instance landed — while still being able to tell "I could not nest this" from
  /// "you gave me something I cannot read".
  pub fn nesting(message: impl Into<String>) -> Self {
    Self {
      code: 7,
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

/// What a command produced, for the commands that can end in a **finding** rather than a refusal.
///
/// A `nest` that could not place every instance and a `simulation` that found a defect both have a
/// report to print and a failure to exit with, in that order: the report is the finding's body. Both
/// the subcommand and the scripted command read this one shape, so a script cannot print a report its
/// subcommand would have refused to print first (M10 invariant 1).
pub enum Outcome {
  /// The report, and the command succeeded.
  Done(String),
  /// The report, and the finding the command ends with.
  Finding(String, Failure),
}

impl Outcome {
  /// Print the report, then answer with the command's result.
  pub fn print_and_result(self) -> Result<(), Failure> {
    self.write_to(&mut std::io::stdout())
  }

  /// Write the report to `sink`, then answer with the command's result.
  ///
  /// This is the one body behind [`Outcome::print_and_result`]: a subcommand passes stdout, a script
  /// passes the sink its caller chose (`script::run_script`), so neither can report something the other
  /// would not (M10 invariant 1). The report is the finding's body, so it is written before the failure
  /// is returned.
  pub fn write_to(self, sink: &mut dyn Write) -> Result<(), Failure> {
    match self {
      Self::Done(report) => write_report(sink, &report),
      Self::Finding(report, failure) => {
        write_report(sink, &report)?;
        Err(failure)
      }
    }
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
    "validate" => validate(rest),
    "inspect" => inspect(rest),
    "postp" => postp(rest),
    "import" => import_command(rest),
    "export" => export_command(rest),
    "check" => check(rest),
    "nest" => nest_command(rest),
    "parts" => pieces::parts_command(rest),
    "cutlist" => pieces::cutlist_command(rest),
    "report" => printsheet::report_command(rest),
    "tools" => tools(rest),
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

pub fn validate(rest: &[String]) -> Result<(), Failure> {
  let path = only_path(rest, "validate")?;
  let document = open(&path)?;
  print!("{}", validate_report(&document));
  Ok(())
}

/// The report `cam-cli validate` prints for a document that loaded.
///
/// The script's `validate` command prints this same report for the document it holds, so a scripted
/// run and a command-line run describe a document in one sentence, not two (M10 invariant 1).
pub fn validate_report(document: &Document) -> String {
  format!(
    "valid: {} layer(s), {} entity(ies), {} toolpath(s)\n",
    document.layers.len(),
    document.entities().count(),
    document.toolpaths.len()
  )
}

pub fn inspect(rest: &[String]) -> Result<(), Failure> {
  let path = only_path(rest, "inspect")?;
  let document = open(&path)?;
  println!("schema: {}", document.schema);
  println!(
    "job: {:.3} x {:.3} x {:.3} mm, origin {:?}, safe Z {:.3} mm",
    document.job.width.mm(),
    document.job.height.mm(),
    document.job.thickness.mm(),
    document.job.origin,
    document.job.safe_z.mm()
  );
  println!("layers: {}", document.layers.len());
  for layer in &document.layers {
    println!(
      "  {} {:?}: {} entity(ies), visible {}",
      layer.id,
      layer.name,
      layer.entities.len(),
      layer.visible
    );
  }
  println!("toolpaths: {}", document.toolpaths.len());
  let mut total_length = 0.0;
  for entity in document.entities() {
    if let Some(contour) = entity.kind.as_contour() {
      total_length += contour.perimeter();
    }
  }
  println!("geometry length: {total_length:.3} mm");
  Ok(())
}

pub fn check(rest: &[String]) -> Result<(), Failure> {
  let path = only_path(rest, "check")?;
  let document = open(&path)?;

  // Findings are collected with the entity id and the entity's own issue order, then sorted by
  // id, so the report is stable whatever order the layers happen to be in.
  let mut findings: Vec<(String, usize, String)> = Vec::new();
  for entity in document.entities() {
    let Some(contour) = entity.kind.as_contour() else {
      continue;
    };
    let issues = cam_geom::validate::inspect(contour, cam_geom::DEFAULT_TOLERANCE);
    let mut order = 0;
    if let Some(gap) = issues.open {
      findings.push((
        entity.id.clone(),
        order,
        format!("{} open: gap {gap:.4} mm", entity.id),
      ));
      order += 1;
    }
    for intersection in &issues.intersections {
      findings.push((
        entity.id.clone(),
        order,
        format!(
          "{} self-intersection at {:.3}, {:.3} (segments {}, {})",
          entity.id,
          intersection.point.x,
          intersection.point.y,
          intersection.first,
          intersection.second
        ),
      ));
      order += 1;
    }
    for index in &issues.degenerate {
      findings.push((
        entity.id.clone(),
        order,
        format!("{} degenerate segment {index}", entity.id),
      ));
      order += 1;
    }
  }

  if findings.is_empty() {
    println!("all vectors valid");
    return Ok(());
  }

  findings.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
  for (_, _, line) in &findings {
    println!("{line}");
  }
  Err(Failure::vectors(format!(
    "{} vector issue(s) found",
    findings.len()
  )))
}

/// Write the document's geometry back out as DXF or SVG.
///
/// The inverse of `import`, and deliberately as small: it reads one document, asks `cam-export` for the
/// bytes and writes them, or prints them. It takes no layer, no tolerance and no toolpath flag, because
/// what goes out is what the document holds (M12 S2, S5). What it *does* refuse, and the writer with it:
/// a format this build does not write (named, never guessed from an extension), a DXF layer name the
/// reader would silently alter or drop (empty, padded, carrying a control character, duplicated) and an
/// SVG of a document with no geometry to derive a page from. `--out` may not be the document being read,
/// and the write lands whole or not at all.
pub fn export_command(rest: &[String]) -> Result<(), Failure> {
  let mut document_path: Option<PathBuf> = None;
  let mut format: Option<cam_export::Format> = None;
  let mut out: Option<PathBuf> = None;

  let mut index = 0;
  while index < rest.len() {
    match rest[index].as_str() {
      "--format" => {
        let value = option_value(rest, index, "--format")?;
        format = Some(
          cam_export::Format::from_name(value)
            .ok_or_else(|| Failure::input(format!("export writes dxf or svg, not {value:?}")))?,
        );
        index += 2;
      }
      "--out" => {
        out = Some(PathBuf::from(option_value(rest, index, "--out")?));
        index += 2;
      }
      other if other.starts_with('-') => {
        return Err(Failure::input(format!("unknown export option {other:?}")))
      }
      other => {
        if document_path.is_some() {
          return Err(Failure::input("export takes exactly one document"));
        }
        document_path = Some(PathBuf::from(other));
        index += 1;
      }
    }
  }

  let document_path =
    document_path.ok_or_else(|| Failure::input("export requires a document path"))?;
  let format = format.ok_or_else(|| Failure::input("export requires --format dxf or svg"))?;
  let document = open(&document_path)?;
  let bytes = cam_export::export_bytes(&document, format)
    .map_err(|error| Failure::input(format!("{}: {error}", document_path.display())))?;

  match out {
    Some(path) => {
      // Writing over the document we just read would destroy it and report success, so the two paths are
      // compared canonically rather than by spelling (review-a7e624d841d3d0c6's R4-SAME-PATH, R3-001).
      let destination = path.canonicalize().unwrap_or_else(|_| path.clone());
      if destination
        == document_path
          .canonicalize()
          .unwrap_or_else(|_| document_path.clone())
      {
        return Err(Failure::input(format!(
          "export --out must not be the document it reads: {}",
          path.display()
        )));
      }
      // Whole or not at all: a truncated `.dxf` beside a live document is worse than a refusal
      // (R4-ATOMIC-OUT). The temporary sits beside the destination so the rename stays on one filesystem.
      let temporary = path.with_extension(format!(
        "{}.tmp",
        path
          .extension()
          .map(|extension| extension.to_string_lossy().into_owned())
          .unwrap_or_default()
      ));
      std::fs::write(&temporary, &bytes)
        .map_err(|error| Failure::input(format!("{}: {error}", temporary.display())))?;
      std::fs::rename(&temporary, &path).map_err(|error| {
        let _ = std::fs::remove_file(&temporary);
        Failure::input(format!("{}: {error}", path.display()))
      })?;
      eprintln!("wrote {} ({} bytes)", path.display(), bytes.len());
    }
    None => {
      // `print!` panics on a closed pipe, which turns `export | head` into exit 101 and a backtrace
      // instead of a failure a script can read (R4-STDOUT-PANIC).
      use std::io::Write;
      let mut stdout = std::io::stdout().lock();
      stdout
        .write_all(&bytes)
        .and_then(|()| stdout.flush())
        .map_err(|error| Failure::input(format!("cannot write to stdout: {error}")))?;
    }
  }
  Ok(())
}

pub fn import_command(rest: &[String]) -> Result<(), Failure> {
  let mut input: Option<PathBuf> = None;
  let mut out: Option<PathBuf> = None;
  let mut layer: Option<String> = None;
  let mut options = cam_import::ImportOptions::default();

  let mut index = 0;
  while index < rest.len() {
    match rest[index].as_str() {
      "--out" => {
        out = Some(PathBuf::from(option_value(rest, index, "--out")?));
        index += 2;
      }
      "--layer" => {
        layer = Some(option_value(rest, index, "--layer")?.to_string());
        index += 2;
      }
      "--tolerance" => {
        options.tolerance = number(rest, index, "--tolerance")?;
        index += 2;
      }
      "--dpi" => {
        options.trace.dpi = number(rest, index, "--dpi")?;
        index += 2;
      }
      "--threshold" => {
        let value = number(rest, index, "--threshold")?;
        if !(0.0..=255.0).contains(&value) {
          return Err(Failure::input(format!(
            "--threshold must be between 0 and 255, got {value}"
          )));
        }
        options.trace.threshold = Some(value.round() as u8);
        index += 2;
      }
      "--min-area" => {
        options.trace.min_area = number(rest, index, "--min-area")?;
        index += 2;
      }
      "--simplify" => {
        options.trace.simplify = number(rest, index, "--simplify")?;
        index += 2;
      }
      "--invert" => {
        options.trace.invert = true;
        index += 1;
      }
      other if other.starts_with('-') => {
        return Err(Failure::input(format!("unknown import option {other:?}")))
      }
      other => {
        if input.is_some() {
          return Err(Failure::input("import takes exactly one input file"));
        }
        input = Some(PathBuf::from(other));
        index += 1;
      }
    }
  }

  let input = input.ok_or_else(|| Failure::input("import requires an input file"))?;
  if options.tolerance <= 0.0 {
    return Err(Failure::input(
      "--tolerance must be a positive length in mm",
    ));
  }
  if options.trace.dpi <= 0.0 {
    return Err(Failure::input("--dpi must be a positive number"));
  }

  let imported = cam_import::import_path(&input, &options)
    .map_err(|error| Failure::input(format!("{}: {error}", input.display())))?;
  let document = cam_import::into_document(&imported, layer.as_deref())
    .map_err(|error| Failure::input(format!("{}: {error}", input.display())))?;

  if let Some(path) = &out {
    cam_store::document::save_document(path, &document)
      .map_err(|error| Failure::input(format!("{}: {error}", path.display())))?;
    eprintln!(
      "wrote {} layer(s), {} entity(ies) to {}",
      document.layers.len(),
      document.entities().count(),
      path.display()
    );
  }

  print!("{}", imported.report.render());
  Ok(())
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

pub fn postp(rest: &[String]) -> Result<(), Failure> {
  let Some(subcommand) = rest.first() else {
    return Err(Failure::input("postp requires a subcommand: validate"));
  };
  if subcommand != "validate" {
    return Err(Failure::input(format!(
      "unknown postp subcommand {subcommand:?}; expected validate"
    )));
  }
  let path = only_path(&rest[1..], "postp validate")?;
  let text = std::fs::read_to_string(&path)
    .map_err(|error| Failure::input(format!("{}: {error}", path.display())))?;
  let post =
    parse_post(&text).map_err(|error| Failure::post(format!("{}: {error}", path.display())))?;
  println!("post: {}", post.post_name);
  println!("extension: {}", post.file_extension);
  println!("units: {:?}", post.units);
  println!("variables: {}", post.variables.len());
  println!("blocks: {}", post.blocks.len());
  Ok(())
}

pub fn tools(rest: &[String]) -> Result<(), Failure> {
  let Some(subcommand) = rest.first() else {
    return Err(Failure::input("tools requires a subcommand: list or add"));
  };
  match subcommand.as_str() {
    "list" => {
      let path = only_path(&rest[1..], "tools list")?;
      let library = ToolLibrary::open(&path).map_err(|error| Failure::input(error.to_string()))?;
      for tool in library
        .tools()
        .map_err(|error| Failure::input(error.to_string()))?
      {
        println!(
          "T{} {} diameter {:.3} mm",
          tool.cutting.tool_number.unwrap_or(0),
          cam_store::tools::render_name(&tool.geometry.name_format, "Tool", &tool.geometry)
            .unwrap_or_else(|_| tool.geometry.id.clone()),
          tool.geometry.diameter.unwrap_or_default().mm()
        );
      }
      Ok(())
    }
    "add" => {
      let mut path = None;
      let mut id = None;
      let mut diameter = None;
      let mut number = None;
      let mut name_format = String::from("{Tool Type} ({Diameter|F}{Units Short})");
      let mut index = 0;
      while index < rest.len() - 1 {
        let argument = &rest[index + 1];
        match argument.as_str() {
          "--id" => {
            id = rest.get(index + 2).cloned();
            index += 2;
          }
          "--diameter" => {
            diameter = rest
              .get(index + 2)
              .and_then(|value| value.parse::<f64>().ok());
            index += 2;
          }
          "--number" => {
            number = rest
              .get(index + 2)
              .and_then(|value| value.parse::<u32>().ok());
            index += 2;
          }
          "--name-format" => {
            if let Some(value) = rest.get(index + 2) {
              name_format = value.clone();
            }
            index += 2;
          }
          other if !other.starts_with('-') && path.is_none() => {
            path = Some(PathBuf::from(other));
            index += 1;
          }
          other => {
            return Err(Failure::input(format!(
              "unknown tools add option {other:?}"
            )))
          }
        }
      }
      let path = path.ok_or_else(|| Failure::input("tools add requires a library path"))?;
      let id = id.ok_or_else(|| Failure::input("tools add requires --id"))?;
      let diameter = diameter.ok_or_else(|| Failure::input("tools add requires --diameter"))?;
      let number = number.ok_or_else(|| Failure::input("tools add requires --number"))?;

      let library = ToolLibrary::open(&path).map_err(|error| Failure::input(error.to_string()))?;
      library
        .put_geometry(&ToolGeometry {
          id: id.clone(),
          name_format,
          tool_type: 0,
          units: cam_units::UnitSystem::Mm,
          diameter: Some(cam_units::Length::from_mm(diameter)),
          included_angle: None,
          flat_diameter: None,
          num_flutes: Some(2),
          flute_length: None,
          tip_radius: None,
          shank_diameter: None,
          overall_length: None,
          custom_attributes: None,
        })
        .map_err(|error| Failure::input(error.to_string()))?;
      library
        .put_cutting_data(&CuttingData {
          id: format!("{id}-cut"),
          rate_units: cam_units::UnitSystem::Mm,
          feed_rate: Some(100.0),
          plunge_rate: Some(30.0),
          spindle_speed: Some(18_000),
          stepdown: Some(cam_units::Length::from_mm(6.0)),
          stepover: None,
          clear_stepover: None,
          tool_number: Some(number),
          notes: None,
        })
        .map_err(|error| Failure::input(error.to_string()))?;
      library
        .link(&format!("{id}-link"), &id, &format!("{id}-cut"), None, None)
        .map_err(|error| Failure::input(error.to_string()))?;
      println!("added {id} as T{number} with a {diameter} mm diameter");
      Ok(())
    }
    other => Err(Failure::input(format!(
      "unknown tools subcommand {other:?}; expected list or add"
    ))),
  }
}

/// Everything `cam-cli nest` accepts, before it is turned into one nesting run.
#[derive(Default)]
pub struct NestArgs {
  pub path: Option<PathBuf>,
  pub out: Option<PathBuf>,
  pub sheets: Vec<(f64, f64)>,
  pub gap: Option<f64>,
  pub rotation: Option<String>,
  pub algorithm: Option<String>,
  pub step: Option<f64>,
}

/// One part of the document's nesting section, as the nesters need it: one closed contour and the
/// class its own geometry puts it in.
pub struct NestPart {
  pub id: String,
  pub instances: usize,
  pub contour: Contour,
  pub class: NestClass,
}

/// Which of the two nesters a part's geometry sends it to (S6, S7).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NestClass {
  /// A rectangle: the cabinetry panels S7 ships first, packed by maxrects and guillotine.
  Rectangle,
  /// Everything else: a free profile, placed by the no-fit heuristic.
  Profile,
}

impl NestClass {
  /// How the report names the class.
  pub fn label(self) -> &'static str {
    match self {
      Self::Rectangle => "rectangle",
      Self::Profile => "free profile",
    }
  }
}

/// One instance a run placed, before it is written into the document.
///
/// `position` is the **placed** footprint's lower-left corner and `rotation` the turn the nester took
/// (D-M9-1), which is exactly what a [`NestingPlacement`] stores, so nothing is re-derived on the way
/// in.
#[derive(Clone, Copy)]
pub struct NestedInstance {
  pub part: usize,
  pub sheet: usize,
  pub position: Point2,
  pub rotation: Angle,
  pub size: Vec2,
}

/// What a run was asked for and what packed it: the two options every class shares, the algorithm the
/// rectangle plan was produced by, and the heuristic the free profiles were placed with.
#[derive(Default)]
pub struct NestRun {
  pub gap: f64,
  pub allow_rotation: bool,
  pub rectangle: Option<NestAlgorithm>,
  pub profile: Option<(&'static str, f64)>,
}

/// One `nest` run's request: everything the run asks for that is not the document itself.
///
/// Every field is an `Option` exactly where the command distinguishes "the caller said nothing" from
/// "the caller said the default", because a flag that means nothing for a document is refused by name
/// rather than defaulted.
#[derive(Default)]
pub struct NestRequest {
  pub sheets: Vec<(f64, f64)>,
  pub gap: Option<f64>,
  pub rotation: Option<String>,
  pub algorithm: Option<String>,
  pub step: Option<f64>,
}

/// Everything `cam-cli nest`'s command line holds, parsed and nothing more.
///
/// The scripted `nest` command hands its tokens to this same parser (M10 invariant 1), so an option
/// means one thing in both places and no second parser exists to drift from it.
pub fn nest_args(rest: &[String]) -> Result<NestArgs, Failure> {
  let mut args = NestArgs::default();
  let mut index = 0;
  while index < rest.len() {
    let flag = rest[index].as_str();
    // Every option takes exactly one value.
    let take = |what: &str| value_at(rest, index, what);
    match flag {
      "--out" => args.out = Some(PathBuf::from(take("--out")?)),
      "--sheets" => args.sheets.push(parse_sheet(&take("--sheets")?)?),
      "--gap" => {
        let value = take("--gap")?;
        let gap = parse_mm(&value, "--gap")?;
        if !gap.is_finite() || gap < 0.0 {
          return Err(Failure::input(format!(
            "--gap must be a non-negative distance in mm, got {value}"
          )));
        }
        args.gap = Some(gap);
      }
      "--step" => {
        let value = take("--step")?;
        let step = parse_mm(&value, "--step")?;
        if !step.is_finite() || step <= 0.0 {
          return Err(Failure::input(format!(
            "--step must be a positive distance in mm, got {value}"
          )));
        }
        args.step = Some(step);
      }
      "--rotation" => args.rotation = Some(take("--rotation")?),
      "--algorithm" => args.algorithm = Some(take("--algorithm")?),
      other if other.starts_with('-') => {
        return Err(Failure::input(format!("unknown nest option {other:?}")))
      }
      other => {
        if args.path.is_some() {
          return Err(Failure::input(
            "nest takes exactly one document, before its options",
          ));
        }
        args.path = Some(PathBuf::from(other));
        index += 1;
        continue;
      }
    }
    index += 2;
  }

  Ok(args)
}

pub fn nest_command(rest: &[String]) -> Result<(), Failure> {
  let NestArgs {
    path,
    out,
    sheets,
    gap,
    rotation,
    algorithm,
    step,
  } = nest_args(rest)?;
  let path = path.ok_or_else(|| Failure::input("nest requires a document path"))?;
  let mut document = open(&path)?;
  let request = NestRequest {
    sheets,
    gap,
    rotation,
    algorithm,
    step,
  };
  let outcome = nest_document(&mut document, &request)?;
  match outcome {
    Outcome::Done(report) => {
      let destination = out.unwrap_or(path);
      cam_store::document::save_document(&destination, &document)
        .map_err(|error| Failure::input(format!("{}: {error}", destination.display())))?;
      print!("{report}");
      Ok(())
    }
    Outcome::Finding(report, failure) => {
      print!("{report}");
      Err(failure)
    }
  }
}

/// Nest the document's parts onto its sheets, store the placements and the cost, and report the run.
///
/// The shape follows `toolpath`: take the document, add what the command was asked for, validate, and
/// hand the report back — the caller writes it out, `--out` or the document itself. What differs is
/// what a run can end with — a nesting that could not place every instance is an [`Outcome::Finding`]
/// with its own exit code, and it writes nothing. `target` is the caller's document and is replaced
/// only when the run succeeded, because a document whose placements fall short of a part's instance
/// count does not load: every refusal leaves the caller's document exactly as it was found (M10
/// invariant 4).
pub fn nest_document(target: &mut Document, request: &NestRequest) -> Result<Outcome, Failure> {
  let mut document = target.clone();

  let gap = request.gap.unwrap_or(0.0);
  let allow_rotation = match request.rotation.as_deref() {
    None | Some("off") => false,
    Some("on") => true,
    Some(other) => {
      return Err(Failure::input(format!(
        "--rotation expects on or off, got {other:?}"
      )))
    }
  };
  let algorithm = match request.algorithm.as_deref() {
    None | Some("auto") => NestAlgorithm::Auto,
    Some("maxrects") => NestAlgorithm::MaxRects,
    Some("guillotine") => NestAlgorithm::Guillotine,
    Some(other) => {
      return Err(Failure::input(format!(
        "--algorithm expects auto, maxrects or guillotine, got {other:?}"
      )))
    }
  };
  let step = request
    .step
    .unwrap_or(cam_geom::nest_profile::DEFAULT_SEARCH_STEP);

  // The parts are authored in the document (S1) and there is no default part list: a part is a
  // selection of this document's own entities, so there is nothing a flag could name.
  let Some(nesting) = document.nesting.take() else {
    return Err(Failure::input(
      "nest needs parts: the document authors no nesting section, so there is nothing to nest",
    ));
  };
  if nesting.parts.is_empty() {
    return Err(Failure::input(
      "nest needs parts: the document authors no nesting parts, so there is nothing to nest",
    ));
  }

  // The boards this run nests on. `--sheets` authors them on the command line and replaces whatever
  // the document held: the sheets are authored records that live in the document (S4), and a run that
  // kept a board nobody named would report a nesting on a sheet the caller had replaced. Without the
  // flag the document's own boards are it, and there is no default board — a sheet size is authored,
  // not assumed.
  let mut sheets: Vec<NestingSheet> = if request.sheets.is_empty() {
    if nesting.sheets.is_empty() {
      return Err(Failure::input(
        "nest needs --sheets: the document authors no sheets to nest on, and a nesting has no default \
         board",
      ));
    }
    nesting.sheets.clone()
  } else {
    request
      .sheets
      .iter()
      .enumerate()
      .map(|(number, (width, height))| NestingSheet {
        id: format!("S{}", number + 1),
        width: Length::from_mm(*width),
        height: Length::from_mm(*height),
      })
      .collect()
  };

  // Both nesters pack **identical** boards — one `NestPlan` is one sheet, repeated — so boards of two
  // sizes cannot be one run, and are refused by name rather than packed on the first board's size.
  let width = sheets[0].width.mm();
  let height = sheets[0].height.mm();
  for sheet in &sheets[1..] {
    if (sheet.width.mm() - width).abs() > cam_geom::DEFAULT_TOLERANCE
      || (sheet.height.mm() - height).abs() > cam_geom::DEFAULT_TOLERANCE
    {
      return Err(Failure::input(format!(
        "sheet {:?} is {} x {} mm but sheet {:?} is {} x {} mm: a nesting packs identical boards, so \
         one run cannot nest onto boards of two sizes",
        sheets[0].id,
        decimal(width),
        decimal(height),
        sheet.id,
        decimal(sheet.width.mm()),
        decimal(sheet.height.mm()),
      )));
    }
  }

  let mut parts: Vec<NestPart> = Vec::with_capacity(nesting.parts.len());
  for part in &nesting.parts {
    let contour = part_contour(&document, part)?;
    parts.push(NestPart {
      id: part.id.clone(),
      instances: part.instances,
      class: classify(&contour),
      contour,
    });
  }

  // Every flag that means nothing for this document is refused by name, the rule `moulding` follows for
  // its 22: a `--step` nobody searched with, or an `--algorithm` no packer ran, still looks applied.
  let has_rectangle = parts.iter().any(|part| part.class == NestClass::Rectangle);
  let has_profile = parts.iter().any(|part| part.class == NestClass::Profile);
  if request.algorithm.is_some() && !has_rectangle {
    return Err(Failure::input(
      "nest takes no --algorithm: no part in this document is a rectangle, so neither maxrects nor \
       guillotine packs anything, and a free profile is placed by the no-fit heuristic --step drives",
    ));
  }
  if request.step.is_some() && !has_profile {
    return Err(Failure::input(
      "nest takes no --step: every part in this document is a rectangle, so the free-profile \
       heuristic never searches a lattice and no step is applied",
    ));
  }

  let board = Bounds {
    min: Point2::ORIGIN,
    max: Point2::new(width, height),
  };
  let mut run = NestRun {
    gap,
    allow_rotation,
    ..NestRun::default()
  };
  let mut instances: Vec<NestedInstance> = Vec::new();
  let mut boards_used = 0;

  if has_rectangle {
    // A rectangle part goes through `cam_geom::nest` — maxrects plus the guillotine pass S6 names —
    // which packs axis-aligned boxes, so a rectangle selection becomes the box its own bounds are.
    // Every instance is one entry in the parts list, so a plan index maps back to the part it belongs
    // to and an unplaced entry names the part and not a copy number.
    let mut owner: Vec<usize> = Vec::new();
    let mut boxes: Vec<Bounds> = Vec::new();
    for (number, part) in parts.iter().enumerate() {
      if part.class != NestClass::Rectangle {
        continue;
      }
      let bounds = part.contour.bounds();
      for _ in 0..part.instances {
        owner.push(number);
        boxes.push(bounds);
      }
    }
    let options = NestOptions::new(gap, allow_rotation)
      .map_err(|error| Failure::input(error.to_string()))?
      .with_algorithm(algorithm);
    let plan =
      NestPlan::new(board, &boxes, options).map_err(|error| Failure::input(error.to_string()))?;
    run.rectangle = Some(plan.algorithm());
    boards_used = plan.sheets_used();
    for placement in plan.placements() {
      instances.push(NestedInstance {
        part: owner[placement.part()],
        sheet: placement.sheet(),
        position: placement.origin(),
        rotation: nest_angle(placement.rotation()),
        size: placement.footprint_size(),
      });
    }
  }

  if has_profile {
    // A free-profile part goes through `cam_geom::nest_profile`, the sound but incomplete no-fit
    // heuristic: one call packs one board and reports the parts it could not fit, so a round that
    // placed nothing has found the parts that fit no board — the loop is bounded by the part count and
    // never guesses. The two classes get their own boards because neither nester can be handed the
    // other's placements as an occupied board: a mixed document nests each class on sheets of its own,
    // which the report says instead of hiding.
    let options = ProfileNestOptions::new(gap, allow_rotation)
      .map_err(|error| Failure::input(error.to_string()))?
      .with_step(step)
      .map_err(|error| Failure::input(error.to_string()))?;
    let board_outline = Contour::from_points(
      &[
        Point2::ORIGIN,
        Point2::new(width, 0.0),
        Point2::new(width, height),
        Point2::new(0.0, height),
      ],
      true,
    )
    .map_err(|error| Failure::input(error.to_string()))?;

    let mut pool: Vec<(usize, Contour)> = Vec::new();
    for (number, part) in parts.iter().enumerate() {
      if part.class != NestClass::Profile {
        continue;
      }
      for _ in 0..part.instances {
        pool.push((number, part.contour.clone()));
      }
    }

    let mut boards = 0;
    while !pool.is_empty() {
      let contours: Vec<Contour> = pool.iter().map(|(_, contour)| contour.clone()).collect();
      let plan = ProfileNestPlan::new(&board_outline, &contours, options)
        .map_err(|error| Failure::input(error.to_string()))?;
      run.profile = Some((plan.algorithm(), options.step()));
      if plan.placements().is_empty() {
        break;
      }
      for (index, placement) in plan.placements().iter().enumerate() {
        let outline = plan.outline(index).ok_or_else(|| {
          Failure::input("the free-profile nester reported a placement with no outline")
        })?;
        let bounds = outline.bounds();
        instances.push(NestedInstance {
          part: pool[placement.part()].0,
          sheet: boards_used + boards,
          position: placement.origin(),
          rotation: nest_angle(placement.rotation()),
          size: Vec2::new(bounds.width(), bounds.height()),
        });
      }
      boards += 1;
      pool = plan
        .unplaced()
        .iter()
        .map(|index| pool[*index].clone())
        .collect();
    }
    boards_used += boards;
  }

  // Open the boards the run used, so every placement names a sheet the document holds: a nester
  // repeats one board, and a board it needed that the document does not author is authored here at the
  // same size (S4). A board the document authored and the run did not use stays, and the report says
  // it was unused.
  while sheets.len() < boards_used {
    sheets.push(NestingSheet {
      id: fresh_sheet_id(&sheets),
      width: Length::from_mm(width),
      height: Length::from_mm(height),
    });
  }

  let placements: Vec<NestingPlacement> = instances
    .iter()
    .map(|instance| NestingPlacement {
      part_id: parts[instance.part].id.clone(),
      sheet_id: sheets[instance.sheet].id.clone(),
      position: instance.position,
      rotation: instance.rotation,
    })
    .collect();

  document.nesting = Some(Nesting {
    sheets,
    parts: nesting.parts,
    placements,
    cost: None,
  });

  // The cost record is written from **`Document::nesting_utilisation`** and from nothing else. The
  // nester has its own `NestPlan::utilisation`, but that is its view of its own run: a record derived
  // from it would be the second arithmetic T1c closed, and the model's own load-time check would then
  // refuse the document this command had just written. One derivation, then, feeding the record and
  // the report alike — and when the document cannot answer, no record at all, because an invented
  // zero would be a claim nobody made.
  let fractions = document
    .nesting_utilisation()
    .map(|utilisation| (utilisation, 1.0 - utilisation));
  let used_sheets = document
    .nesting
    .as_ref()
    .map(|nesting| {
      nesting
        .placements
        .iter()
        .map(|placement| placement.sheet_id.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len()
    })
    .unwrap_or(0);
  if let Some((utilisation, waste)) = fractions {
    if let Some(nesting) = document.nesting.as_mut() {
      nesting.cost = Some(NestingCost {
        sheets_used: used_sheets,
        utilisation,
        waste,
      });
    }
  }

  let report = nest_report(&document, &parts, &instances, &run, used_sheets, fractions);
  let shortfall = nest_shortfall(&parts, &instances);
  if !shortfall.is_empty() {
    // A nesting that could not place every instance is a finding, and nothing is written: the model
    // refuses a document whose placements fall short of a part's instance count
    // (`NestingInstanceCountMismatch`), so a partial nesting has no honest form to store and the
    // document stays exactly as the run found it. The report is the finding, and exit 7 names it.
    let named: Vec<String> = shortfall
      .iter()
      .map(|(number, placed, wanted)| {
        format!("{} ({placed} of {wanted} instance(s))", parts[*number].id)
      })
      .collect();
    return Ok(Outcome::Finding(
      report,
      Failure::nesting(format!(
        "the nesting left {} of {} instance(s) unplaced: {}; the document was not written",
        shortfall
          .iter()
          .map(|(_, placed, wanted)| wanted - placed)
          .sum::<usize>(),
        parts.iter().map(|part| part.instances).sum::<usize>(),
        named.join(", ")
      )),
    ));
  }

  document
    .validate()
    .map_err(|error| Failure::input(error.to_string()))?;
  *target = document;
  Ok(Outcome::Done(report))
}

/// The `W x H` one `--sheets` flag authors, in millimetres.
pub fn parse_sheet(value: &str) -> Result<(f64, f64), Failure> {
  let malformed = || {
    Failure::input(format!(
      "--sheets expects <width>x<height> in millimetres, got {value:?}"
    ))
  };
  let (width, height) = value.split_once('x').ok_or_else(malformed)?;
  let width = width.trim().parse::<f64>().map_err(|_| malformed())?;
  let height = height.trim().parse::<f64>().map_err(|_| malformed())?;
  if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
    return Err(Failure::input(format!(
      "--sheets must name a positive width and height in millimetres, got {value}"
    )));
  }
  Ok((width, height))
}

/// The one closed contour a part is nested from, or the refusal that names the part.
///
/// Both nesters place a single outline, and a part whose selection is several contours or an open wire
/// has no single outline to place: nesting its bounding box and reporting it as one part would be a
/// placement the cut does not match, so the part is refused by name instead of guessed at.
pub fn part_contour(document: &Document, part: &NestingPart) -> Result<Contour, Failure> {
  let [entity_id] = part.entity_ids.as_slice() else {
    return Err(Failure::input(format!(
      "nesting part {:?} names {} entities, so its outline is not one closed contour and neither \
       nester can place it: a part is nested from exactly one closed contour",
      part.id,
      part.entity_ids.len()
    )));
  };
  let entity = document.entity(entity_id).ok_or_else(|| {
    Failure::input(format!(
      "nesting part {:?} references unknown entity {entity_id:?}",
      part.id
    ))
  })?;
  let contour = entity.kind.as_contour().ok_or_else(|| {
    Failure::input(format!(
      "nesting part {:?} selects entity {entity_id:?}, which is not a contour",
      part.id
    ))
  })?;
  if !contour.is_closed() {
    return Err(Failure::input(format!(
      "nesting part {:?} selects entity {entity_id:?}, which is an open contour: a part's footprint \
       is the area it encloses, so a part is nested from a closed contour",
      part.id
    )));
  }
  Ok(contour.clone())
}

/// Which nester a part's own geometry puts it in (S6, S7).
///
/// A part is a **rectangle** when its selection is one closed, arc-free contour of four vertices whose
/// area is its bounding box's: those are the cabinetry panels S7 ships first, and they go through
/// `cam_geom::nest`, whose maxrects and guillotine passes are the two heuristics S6 names. Every other
/// closed contour is a **free profile** and goes through `cam_geom::nest_profile`, the no-fit
/// heuristic. The class is read from the part and never from a flag, so a part cannot be handed to an
/// engine that cannot express it, and the report names the class of every part.
pub fn classify(contour: &Contour) -> NestClass {
  let bounds = contour.bounds();
  let box_area = bounds.width() * bounds.height();
  let area = contour.signed_area().abs();
  // A four-vertex closed contour whose area is its box's can only be the box itself: every other
  // quadrilateral inscribed in the box has strictly less area, and a self-intersecting one — a bowtie
  // through the same four corners — has less again. The tolerance is the crate's, relative to the box,
  // so a panel authored on a grid is a rectangle and a sliver is not a coincidence.
  let is_rectangle = !contour.has_arcs()
    && contour.len() == 4
    && box_area > 0.0
    && (area - box_area).abs() <= cam_geom::DEFAULT_TOLERANCE * box_area;
  if is_rectangle {
    NestClass::Rectangle
  } else {
    NestClass::Profile
  }
}

/// The turn a nester chose in the document's own terms: a quarter turn counter-clockwise about the
/// part's footprint corner (D-M9-1), or none. Both nesters speak [`NestRotation`], so one mapping
/// serves both and a document has one rotation concept and not two.
pub fn nest_angle(rotation: NestRotation) -> Angle {
  match rotation {
    NestRotation::None => Angle::ZERO,
    NestRotation::QuarterTurn => Angle::from_radians(std::f64::consts::FRAC_PI_2),
  }
}

/// The name a plan's algorithm is reported by.
pub fn algorithm_label(algorithm: NestAlgorithm) -> &'static str {
  match algorithm {
    // A plan never reports this: it names the heuristic that packed it.
    NestAlgorithm::Auto => "auto",
    NestAlgorithm::MaxRects => "maxrects",
    NestAlgorithm::Guillotine => "guillotine",
  }
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

/// How many instances of each part a run placed, beside how many the part asks for.
///
/// The one reading of "did this part get nested": the report and the exit code both read it, so they
/// cannot disagree about what was left out.
pub fn nest_shortfall(
  parts: &[NestPart],
  instances: &[NestedInstance],
) -> Vec<(usize, usize, usize)> {
  parts
    .iter()
    .enumerate()
    .map(|(number, part)| {
      let placed = instances
        .iter()
        .filter(|instance| instance.part == number)
        .count();
      (number, placed, part.instances)
    })
    .filter(|(_, placed, wanted)| placed < wanted)
    .collect()
}

/// The report `cam-cli nest` prints: the boards, every placement, the engine that packed each part
/// class and what the document's own arithmetic says about utilisation and waste.
///
/// One order, one decimal count and no iteration over a map, so two runs print the same bytes
/// (invariant 4). Every number is either a placement the run made or a value
/// [`Document::nesting_utilisation`] derived from the document that was written, so the report and the
/// stored section cannot disagree (invariant 2).
pub fn nest_report(
  document: &Document,
  parts: &[NestPart],
  instances: &[NestedInstance],
  run: &NestRun,
  used_sheets: usize,
  fractions: Option<(f64, f64)>,
) -> String {
  let Some(nesting) = document.nesting.as_ref() else {
    return String::new();
  };
  let width = nesting.sheets.first().map_or(0.0, |sheet| sheet.width.mm());
  let height = nesting
    .sheets
    .first()
    .map_or(0.0, |sheet| sheet.height.mm());

  let mut lines: Vec<String> = Vec::new();
  lines.push(format!(
    "nest: {} sheet(s) of {} x {} mm, {} part(s), {} instance(s)",
    nesting.sheets.len(),
    decimal(width),
    decimal(height),
    parts.len(),
    parts.iter().map(|part| part.instances).sum::<usize>(),
  ));
  lines.push(format!(
    "options: gap {} mm, rotation {}",
    decimal(run.gap),
    if run.allow_rotation { "on" } else { "off" },
  ));
  for (number, sheet) in nesting.sheets.iter().enumerate() {
    let used = instances.iter().any(|instance| instance.sheet == number);
    lines.push(format!(
      "sheet {}: {}",
      sheet.id,
      if used { "used" } else { "unused" }
    ));
  }
  for (number, part) in parts.iter().enumerate() {
    lines.push(format!(
      "part: {} {}, {} instance(s)",
      part.id,
      part.class.label(),
      part.instances,
    ));
    for instance in instances.iter().filter(|instance| instance.part == number) {
      lines.push(format!(
        "placement: {} on {} at ({}, {}), rotation {}, {} x {} mm",
        part.id,
        nesting.sheets[instance.sheet].id,
        decimal(instance.position.x),
        decimal(instance.position.y),
        if instance.rotation == Angle::ZERO {
          "none"
        } else {
          "quarter turn"
        },
        decimal(instance.size.x),
        decimal(instance.size.y),
      ));
    }
  }
  if let Some(algorithm) = run.rectangle {
    lines.push(format!(
      "algorithm: rectangle, {}",
      algorithm_label(algorithm)
    ));
  }
  if let Some((heuristic, step)) = run.profile {
    lines.push(format!(
      "algorithm: free profile, {heuristic} at {} mm",
      decimal(step)
    ));
  }
  lines.push(format!("sheets used: {used_sheets}"));
  match fractions {
    Some((utilisation, waste)) => {
      lines.push(format!("utilisation: {}", decimal(utilisation)));
      lines.push(format!("waste: {}", decimal(waste)));
    }
    None => {
      lines.push("utilisation: none".to_owned());
      lines.push("waste: none".to_owned());
    }
  }
  let shortfall = nest_shortfall(parts, instances);
  if shortfall.is_empty() {
    lines.push("unplaced: none".to_owned());
  }
  for (number, placed, wanted) in shortfall {
    lines.push(format!(
      "unplaced: {}, {} of {} instance(s)",
      parts[number].id,
      wanted - placed,
      wanted
    ));
  }
  let mut text = lines.join("\n");
  text.push('\n');
  text
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

/// Parse a non-negative distance in millimetres.
pub(crate) fn parse_mm(value: &str, what: &str) -> Result<f64, Failure> {
  let number = value
    .parse::<f64>()
    .map_err(|_| Failure::input(format!("{what} must be a distance in mm, got {value:?}")))?;
  if number < 0.0 {
    return Err(Failure::input(format!(
      "{what} must be a non-negative distance in mm, got {value}"
    )));
  }
  Ok(number)
}

/// A short label for an operation kind, for reports.
pub(crate) fn operation_label(operation: &cam_model::Operation) -> &'static str {
  match operation {
    cam_model::Operation::Profile(_) => "profile",
    cam_model::Operation::Pocket(_) => "pocket",
    cam_model::Operation::Drilling(_) => "drilling",
    cam_model::Operation::Chamfer(_) => "chamfer",
    cam_model::Operation::VCarve(_) => "vcarve",
    cam_model::Operation::Engrave(_) => "engrave",
    cam_model::Operation::Fluting(_) => "fluting",
    cam_model::Operation::Texture(_) => "texture",
    cam_model::Operation::Inlay(_) => "inlay",
    cam_model::Operation::Thread(_) => "thread",
    cam_model::Operation::Moulding(_) => "moulding",
  }
}
