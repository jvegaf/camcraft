//! Headless pipeline: `validate`, `inspect`, `postp validate`, `tools` and `emit`.
//!
//! This binary is the verification surface of the whole application: every behaviour that matters
//! must be reachable here, because a behaviour that only exists in a window cannot be tested.
//!
//! Contract: machine output on stdout, human diagnostics on stderr, and these exit codes:
//! `0` success, `2` invalid input, `3` unsupported schema, `4` post-processor error.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use cam_geom::Point2;
use cam_model::{Document, Operation, Toolpath};
use cam_move::{MoveStream, Segment};
use cam_post::{emit, parse_post, ProgramMeta};
use cam_store::document::DocumentFile;
use cam_store::tools::{CuttingData, ToolGeometry, ToolLibrary};
use cam_toolpath::{profile, ProfileRequest, ToolSpec};

const HELP: &str = "\
cam-cli — headless CAM pipeline

USAGE:
    cam-cli <command> [options]

COMMANDS:
    validate <document>       Check the document's schema and referential invariants
    inspect <document>        Print layer, entity and toolpath counts
    postp validate <file>     Parse a post-processor and print a summary
    tools list <library>      List the tools in a SQLite tool library
    tools add <library>       Add or replace a tool: --id, --diameter, --number, [--name-format]
    emit <document>           Compute toolpaths and write G-code
        --post <file>         Post-processor to use (required)
        --out <path>          Write the program here instead of stdout
        --library <path>      Tool library; defaults to the document's tools_ref

OPTIONS:
    -h, --help                Show this help

EXIT CODES:
    0   success
    2   invalid input: unusable path, malformed document, missing tool data
    3   unsupported document schema
    4   post-processor error, including a program the post cannot express
";

fn main() -> ExitCode {
  let arguments: Vec<String> = std::env::args().skip(1).collect();
  match run(&arguments) {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      eprintln!("error: {error}");
      ExitCode::from(error.code() as u8)
    }
  }
}

/// A failure with the exit code the contract assigns to it.
#[derive(Debug)]
struct Failure {
  code: i32,
  message: String,
}

impl std::fmt::Display for Failure {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Failure {
  fn code(&self) -> i32 {
    self.code
  }

  fn input(message: impl Into<String>) -> Self {
    Self {
      code: 2,
      message: message.into(),
    }
  }

  fn schema(message: impl Into<String>) -> Self {
    Self {
      code: 3,
      message: message.into(),
    }
  }

  fn post(message: impl Into<String>) -> Self {
    Self {
      code: 4,
      message: message.into(),
    }
  }
}

fn run(arguments: &[String]) -> Result<(), Failure> {
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
    "tools" => tools(rest),
    "emit" => emit_command(rest),
    other => Err(Failure::input(format!(
      "unknown command {other:?}; see --help"
    ))),
  }
}

/// The single positional argument a command expects.
fn only_path(rest: &[String], what: &str) -> Result<PathBuf, Failure> {
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
fn open(path: &Path) -> Result<Document, Failure> {
  let file = DocumentFile::load(path).map_err(|error| match error {
    cam_store::StoreError::Invalid { .. } => classify_invalid(path, &error),
    other => Failure::input(other.to_string()),
  })?;
  Ok(file.document)
}

/// An invalid document is either a schema problem or an invariant problem; the messages differ in
/// meaning and the exit code has to as well.
fn classify_invalid(path: &Path, error: &cam_store::StoreError) -> Failure {
  let text = error.to_string();
  if text.contains("unsupported document schema") {
    Failure::schema(format!("{}: {text}", path.display()))
  } else {
    Failure::input(format!("{}: {text}", path.display()))
  }
}

fn validate(rest: &[String]) -> Result<(), Failure> {
  let path = only_path(rest, "validate")?;
  let document = open(&path)?;
  println!(
    "valid: {} layer(s), {} entity(ies), {} toolpath(s)",
    document.layers.len(),
    document.entities().count(),
    document.toolpaths.len()
  );
  Ok(())
}

fn inspect(rest: &[String]) -> Result<(), Failure> {
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
  for toolpath in &document.toolpaths {
    let kind = match toolpath.operation {
      Operation::Profile(_) => "profile",
    };
    println!(
      "  {} {:?}: {kind}, tool T{}, {} entity(ies)",
      toolpath.id,
      toolpath.name,
      toolpath.tool_number,
      toolpath.entity_ids.len()
    );
  }
  for entity in document.entities() {
    if let Some(contour) = entity.kind.as_contour() {
      total_length += contour.perimeter();
    }
  }
  println!("geometry length: {total_length:.3} mm");
  Ok(())
}

fn postp(rest: &[String]) -> Result<(), Failure> {
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

fn tools(rest: &[String]) -> Result<(), Failure> {
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

fn emit_command(rest: &[String]) -> Result<(), Failure> {
  let mut document_path: Option<PathBuf> = None;
  let mut post_path: Option<PathBuf> = None;
  let mut out_path: Option<PathBuf> = None;
  let mut library_path: Option<PathBuf> = None;

  let mut index = 0;
  while index < rest.len() {
    match rest[index].as_str() {
      "--post" => {
        post_path = rest.get(index + 1).map(PathBuf::from);
        index += 2;
      }
      "--out" => {
        out_path = rest.get(index + 1).map(PathBuf::from);
        index += 2;
      }
      "--library" => {
        library_path = rest.get(index + 1).map(PathBuf::from);
        index += 2;
      }
      other if other.starts_with('-') => {
        return Err(Failure::input(format!("unknown emit option {other:?}")))
      }
      other => {
        if document_path.is_some() {
          return Err(Failure::input("emit takes exactly one document"));
        }
        document_path = Some(PathBuf::from(other));
        index += 1;
      }
    }
  }

  let document_path =
    document_path.ok_or_else(|| Failure::input("emit requires a document path"))?;
  let post_path = post_path.ok_or_else(|| Failure::input("emit requires --post"))?;

  let document = open(&document_path)?;

  let library_path = match library_path.or_else(|| {
    document.tools_ref.as_ref().map(|reference| {
      document_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(reference)
    })
  }) {
    Some(path) => path,
    None => {
      return Err(Failure::input(
        "a profile cut needs a tool diameter: pass --library or set tools_ref in the document",
      ))
    }
  };
  let library =
    ToolLibrary::open(&library_path).map_err(|error| Failure::input(error.to_string()))?;

  let post_text = std::fs::read_to_string(&post_path)
    .map_err(|error| Failure::input(format!("{}: {error}", post_path.display())))?;
  let post = parse_post(&post_text)
    .map_err(|error| Failure::post(format!("{}: {error}", post_path.display())))?;

  let mut stream = MoveStream::new(document.job.safe_z, Point2::new(0.0, 0.0));
  for toolpath in &document.toolpaths {
    stream
      .segments
      .push(build_segment(&document, toolpath, &library)?);
  }

  let meta = ProgramMeta {
    file_name: document_path
      .file_name()
      .map(|name| name.to_string_lossy().into_owned())
      .unwrap_or_default(),
    file_path: document_path.display().to_string(),
  };
  let emitted = emit(&post, &stream, &meta).map_err(|error| Failure::post(error.to_string()))?;

  match out_path {
    Some(path) => {
      std::fs::write(&path, &emitted.text)
        .map_err(|error| Failure::input(format!("{}: {error}", path.display())))?;
      eprintln!("wrote {} line(s) to {}", emitted.lines, path.display());
    }
    None => print!("{}", emitted.text),
  }
  Ok(())
}

/// Turn one document toolpath into a segment of machine moves.
fn build_segment(
  document: &Document,
  toolpath: &Toolpath,
  library: &ToolLibrary,
) -> Result<Segment, Failure> {
  let number = toolpath.tool_number;
  let (geometry, cutting) = library
    .tool_for_number(number)
    .map_err(|error| Failure::input(error.to_string()))?
    .ok_or_else(|| {
      Failure::input(format!(
        "tool T{number} used by toolpath {:?} is not in the library",
        toolpath.id
      ))
    })?;

  let diameter = geometry.diameter.ok_or_else(|| {
    Failure::input(format!(
      "tool T{number} has no diameter, so a profile offset cannot be computed"
    ))
  })?;

  let spec = ToolSpec {
    number,
    diameter,
    feed_rate: cutting.feed_rate.unwrap_or(0.0),
    plunge_rate: cutting.plunge_rate.unwrap_or(0.0),
    spindle_speed: cutting.spindle_speed.unwrap_or(0),
  };

  let mut moves = Vec::new();
  match &toolpath.operation {
    Operation::Profile(params) => {
      for entity_id in &toolpath.entity_ids {
        let entity = document.entity(entity_id).ok_or_else(|| {
          Failure::input(format!(
            "toolpath {:?} cites unknown entity {entity_id:?}",
            toolpath.id
          ))
        })?;
        let contour = entity
          .kind
          .as_contour()
          .ok_or_else(|| Failure::input(format!("entity {entity_id:?} is not a contour")))?;
        moves.extend(
          profile(&ProfileRequest {
            contour,
            params,
            tool: &spec,
            job: &document.job,
          })
          .map_err(|error| Failure::input(format!("toolpath {:?}: {error}", toolpath.id)))?,
        );
      }
    }
  }

  Ok(Segment {
    name: toolpath.name.clone(),
    tool_number: number,
    feed_rate: spec.feed_rate,
    plunge_rate: spec.plunge_rate,
    spindle_speed: spec.spindle_speed,
    moves,
  })
}
