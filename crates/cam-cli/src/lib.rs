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

pub mod pieces;
pub mod printsheet;
pub mod script;

use std::io::Write;
use std::path::{Path, PathBuf};

use cam_model::{Document, NestingPart, NestingSheet};
use cam_post::parse_post;
use cam_store::document::DocumentFile;
use cam_store::tools::{CuttingData, ToolGeometry, ToolLibrary};
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

/// Write a report to the sink its caller chose: the one way a report leaves this crate.
///
/// `print!` panics when its own write fails, which a caller holding a buffer should never inherit, so a
/// sink that refuses the bytes becomes a refusal instead.
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
    "job: {:.3} x {:.3} x {:.3} mm, origin {:?}, default safe Z {:.3} mm",
    document.board.width.mm(),
    document.board.height.mm(),
    document.board.thickness.mm(),
    document.board.origin,
    cam_model::DEFAULT_SAFE_Z_MM
  );
  println!("layers: {}", document.layers.len());
  for layer in &document.layers {
    println!(
      "  {}: {} entity(ies), visible {}",
      layer.id,
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
      let mut kind = cam_store::tools::ToolKind::FlatEndMill;
      let mut flutes: u32 = 2;
      let mut included_angle = None;
      let mut flat_diameter = None;
      let mut tip_radius = None;
      let mut flute_length = None;
      let mut shank_diameter = None;
      let mut overall_length = None;
      let mut feed: Option<f64> = Some(100.0);
      let mut plunge: Option<f64> = Some(30.0);
      let mut spindle: Option<u32> = Some(18_000);
      let mut stepdown = Some(cam_units::Length::from_mm(6.0));
      let mut stepover = None;
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
          "--kind" => {
            let Some(value) = rest.get(index + 2) else {
              return Err(Failure::input("--kind requires a value"));
            };
            kind = cam_store::tools::ToolKind::from_slug(value)
              .ok_or_else(|| Failure::input(format!("unknown tool kind {value:?}")))?;
            index += 2;
          }
          "--flutes" => {
            flutes = rest
              .get(index + 2)
              .and_then(|value| value.parse::<u32>().ok())
              .ok_or_else(|| Failure::input("--flutes requires an integer value"))?;
            index += 2;
          }
          "--included-angle" => {
            included_angle = rest
              .get(index + 2)
              .and_then(|value| value.parse::<f64>().ok())
              .map(cam_units::Angle::from_degrees);
            if included_angle.is_none() {
              return Err(Failure::input("--included-angle requires a number"));
            }
            index += 2;
          }
          "--flat-diameter" => {
            flat_diameter = rest
              .get(index + 2)
              .and_then(|value| value.parse::<f64>().ok())
              .map(cam_units::Length::from_mm);
            if flat_diameter.is_none() {
              return Err(Failure::input("--flat-diameter requires a number"));
            }
            index += 2;
          }
          "--tip-radius" => {
            tip_radius = rest
              .get(index + 2)
              .and_then(|value| value.parse::<f64>().ok())
              .map(cam_units::Length::from_mm);
            if tip_radius.is_none() {
              return Err(Failure::input("--tip-radius requires a number"));
            }
            index += 2;
          }
          "--flute-length" => {
            flute_length = rest
              .get(index + 2)
              .and_then(|value| value.parse::<f64>().ok())
              .map(cam_units::Length::from_mm);
            if flute_length.is_none() {
              return Err(Failure::input("--flute-length requires a number"));
            }
            index += 2;
          }
          "--shank-diameter" => {
            shank_diameter = rest
              .get(index + 2)
              .and_then(|value| value.parse::<f64>().ok())
              .map(cam_units::Length::from_mm);
            if shank_diameter.is_none() {
              return Err(Failure::input("--shank-diameter requires a number"));
            }
            index += 2;
          }
          "--overall-length" => {
            overall_length = rest
              .get(index + 2)
              .and_then(|value| value.parse::<f64>().ok())
              .map(cam_units::Length::from_mm);
            if overall_length.is_none() {
              return Err(Failure::input("--overall-length requires a number"));
            }
            index += 2;
          }
          "--feed" => {
            feed = rest
              .get(index + 2)
              .and_then(|value| value.parse::<f64>().ok())
              .map(Some)
              .ok_or_else(|| Failure::input("--feed requires a number"))?;
            index += 2;
          }
          "--plunge" => {
            plunge = rest
              .get(index + 2)
              .and_then(|value| value.parse::<f64>().ok())
              .map(Some)
              .ok_or_else(|| Failure::input("--plunge requires a number"))?;
            index += 2;
          }
          "--spindle" => {
            spindle = rest
              .get(index + 2)
              .and_then(|value| value.parse::<u32>().ok())
              .map(Some)
              .ok_or_else(|| Failure::input("--spindle requires an integer value"))?;
            index += 2;
          }
          "--stepdown" => {
            stepdown = rest
              .get(index + 2)
              .and_then(|value| value.parse::<f64>().ok())
              .map(|value| Some(cam_units::Length::from_mm(value)))
              .ok_or_else(|| Failure::input("--stepdown requires a number"))?;
            index += 2;
          }
          "--stepover" => {
            stepover = rest
              .get(index + 2)
              .and_then(|value| value.parse::<f64>().ok())
              .map(|value| Some(cam_units::Length::from_mm(value)))
              .ok_or_else(|| Failure::input("--stepover requires a number"))?;
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
      let tool = cam_store::tools::StoredTool {
        geometry: ToolGeometry {
          id: id.clone(),
          name_format,
          tool_type: kind.raw(),
          units: cam_units::UnitSystem::Mm,
          diameter: Some(cam_units::Length::from_mm(diameter)),
          included_angle,
          flat_diameter,
          num_flutes: Some(flutes),
          flute_length,
          tip_radius,
          shank_diameter,
          overall_length,
          custom_attributes: None,
        },
        cutting: CuttingData {
          id: format!("{id}-cut"),
          rate_units: cam_units::UnitSystem::Mm,
          feed_rate: feed,
          plunge_rate: plunge,
          spindle_speed: spindle,
          stepdown,
          stepover,
          clear_stepover: None,
          tool_number: Some(number),
          notes: None,
        },
      };
      library
        .save_tool(&tool)
        .map_err(|error| Failure::input(error.to_string()))?;
      println!("added {id} as T{number} with a {diameter} mm diameter");
      Ok(())
    }
    other => Err(Failure::input(format!(
      "unknown tools subcommand {other:?}; expected list or add"
    ))),
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
