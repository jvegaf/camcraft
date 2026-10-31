//! Headless pipeline: `validate`, `inspect`, `import`, `check`, `postp validate`, `tools`, `toolpath`,
//! `emit` and `simulate`.
//!
//! This binary is the verification surface of the whole application: every behaviour that matters
//! must be reachable here, because a behaviour that only exists in a window cannot be tested.
//!
//! Contract: machine output on stdout, human diagnostics on stderr, and these exit codes:
//! `0` success, `2` invalid input, `3` unsupported schema, `4` post-processor error, `5` vector
//! issues, `6` a simulation that found gouge, uncut material or a crash (D-M5-6).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use cam_geom::Point2;
use cam_model::{Document, Operation, Toolpath};
use cam_move::{MoveStream, Segment, ToolShape};
use cam_post::{emit, parse_post, ProgramMeta};
use cam_sim::{deviations, sweep, target_for, Heightfield, ToolShapes};
use cam_store::document::DocumentFile;
use cam_store::tools::{CuttingData, ToolGeometry, ToolLibrary};
use cam_toolpath::{
  chamfer, circle_centre, drill, engrave, female, fluting, inlay, male, pocket, profile, texture,
  thread, ChamferRequest, DrillHole, DrillRequest, EngraveRequest, FlutingRequest, InlayRequest,
  PocketRequest, ProfileRequest, TextureRequest, ThreadRequest, ToolSpec, ToolpathError,
};
use cam_units::{Format, Length};

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
    check <document>          Report open and self-intersecting vectors
    toolpath <document>       Append a toolpath: --kind, --entities, --tool, [--id], [--name], [--out]
        --kind <kind>         profile, pocket, drilling, chamfer, engrave, fluting, texture, inlay or
                              thread (required); a texture cuts grooves only, so the material between
                              them stands and the region is never cleared; an inlay appends **two**
                              toolpaths, a female cavity and the male plug that fits it; a thread
                              mills one helical descent around each circle, one pitch per revolution
        --entities <ids>      Comma-separated entity ids (required)
        --islands <ids>       pocket: contours inside the pocket to leave standing
        --tool <number>       Tool number in the library (required)
        --side <side>         profile: inside, outside or on (default outside)
        --allowance <mm>      profile, pocket: material left on the vector
        --strategy <mode>     pocket: offset or raster (default offset)
        --stepover <mm>       pocket, inlay: ring or row spacing; defaults to the tool's stepover
        --spacing <mm>        texture: distance between grooves; defaults to the tool's stepover.
                              The material between grooves stands: a texture does not clear
        --angle <degrees>     texture: direction of the grooves, from the X axis (default 0)
        --inlay-allowance <mm>  inlay: clearance between the male plug and the female cavity, which is
                              how far the female's region grows (default 0)
        --clear               pocket, inlay: take the default from the tool's clear stepover
        --peck-depth <mm>     drilling: depth per peck, 0 for one plunge
        --retract <mode>      drilling: full (default) or amount
        --retract-amount <mm> drilling: lift above the peck when --retract amount
        --bank <count:pitch>  drilling: a gang of drills spaced along X
        --chamfer-width <mm>  chamfer: width of the bevel; the depth is derived from the tool's angle
        --pass-depth <mm>     depth per pass; profile, pocket and inlay, defaults to the tool's stepdown
        --start-depth <mm>    depth at which cutting starts
        --cut-depth <mm>      total cut depth (default 18); chamfer derives its depth instead, engrave
                              and texture cut their one pass at it, fluting ramps to it, and a thread's
                              helix descends to it
        --ramp <mm>           fluting: length of the ramped entry and of the ramped exit, 0 (default)
                              to plunge in place; longer than half the vector is an error
        --pitch <mm>          thread: how far the helix descends each revolution (required)
        --thread-direction <d>  thread: internal (default) inside a hole, or external around a boss
        --lead-in <mm>        profile: straight lead-in
        --lead-out <mm>       profile: straight lead-out
        --tab <pos:width:height>  profile: repeatable holding tab
    postp validate <file>     Parse a post-processor and print a summary
    tools list <library>      List the tools in a SQLite tool library
    tools add <library>       Add or replace a tool: --id, --diameter, --number, [--name-format]
    emit <document>           Compute toolpaths and write G-code
        --post <file>         Post-processor to use (required)
        --out <path>          Write the program here instead of stdout
        --library <path>      Tool library; defaults to the document's tools_ref
    simulate <document>       Simulate the program and report removed, gouge, uncut and crashes
        --library <path>      Tool library; defaults to the document's tools_ref
        --resolution <mm>     Cell size of the removal grid (default 1)

OPTIONS:
    -h, --help                Show this help

EXIT CODES:
    0   success
    2   invalid input: unusable path, malformed document, missing tool data
    3   unsupported document schema
    4   post-processor error, including a program the post cannot express
    5   the document has vector issues (check only)
    6   the simulation found gouge, uncut material or a crash (simulate only)
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

  fn vectors(message: impl Into<String>) -> Self {
    Self {
      code: 5,
      message: message.into(),
    }
  }

  /// A simulated program that does not cut what the document authorised (D-M5-6).
  fn simulation(message: impl Into<String>) -> Self {
    Self {
      code: 6,
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
    "import" => import_command(rest),
    "check" => check(rest),
    "toolpath" => toolpath_command(rest),
    "tools" => tools(rest),
    "emit" => emit_command(rest),
    "simulate" => simulate_command(rest),
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
    let kind = operation_label(&toolpath.operation);
    let islands = if toolpath.island_ids.is_empty() {
      String::new()
    } else {
      format!(", {} island(s)", toolpath.island_ids.len())
    };
    println!(
      "  {} {:?}: {kind}, tool T{}, {} entity(ies){islands}",
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

fn check(rest: &[String]) -> Result<(), Failure> {
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

fn import_command(rest: &[String]) -> Result<(), Failure> {
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
fn option_value<'a>(rest: &'a [String], index: usize, name: &str) -> Result<&'a str, Failure> {
  rest
    .get(index + 1)
    .map(String::as_str)
    .ok_or_else(|| Failure::input(format!("{name} requires a value")))
}

/// The number that follows an option.
fn number(rest: &[String], index: usize, name: &str) -> Result<f64, Failure> {
  let value = option_value(rest, index, name)?;
  value
    .parse::<f64>()
    .map_err(|_| Failure::input(format!("{name} expects a number, got {value:?}")))
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

/// Everything `cam-cli toolpath` accepts, before it is turned into one [`Operation`].
#[derive(Default)]
struct ToolpathArgs {
  path: Option<PathBuf>,
  out: Option<PathBuf>,
  library: Option<PathBuf>,
  kind: Option<String>,
  entities: Option<String>,
  islands: Option<String>,
  tool: Option<u32>,
  id: Option<String>,
  name: Option<String>,
  side: Option<String>,
  strategy: Option<String>,
  stepover: Option<f64>,
  clear: bool,
  allowance: Option<f64>,
  pass_depth: Option<f64>,
  start_depth: Option<f64>,
  cut_depth: Option<f64>,
  lead_in: Option<f64>,
  lead_out: Option<f64>,
  tabs: Vec<(f64, f64, f64)>,
  peck_depth: Option<f64>,
  retract: Option<String>,
  retract_amount: Option<f64>,
  bank: Option<(u32, f64)>,
  chamfer_width: Option<f64>,
  ramp: Option<f64>,
  spacing: Option<f64>,
  angle: Option<f64>,
  inlay_allowance: Option<f64>,
  pitch: Option<f64>,
  thread_direction: Option<String>,
}

fn toolpath_command(rest: &[String]) -> Result<(), Failure> {
  let mut args = ToolpathArgs::default();
  let mut index = 0;
  while index < rest.len() {
    let flag = rest[index].as_str();
    // Every option takes exactly one value except `--clear`.
    let take = |what: &str| value_at(rest, index, what);
    match flag {
      "--out" => args.out = Some(PathBuf::from(take("--out")?)),
      "--library" => args.library = Some(PathBuf::from(take("--library")?)),
      "--kind" => args.kind = Some(take("--kind")?),
      "--entities" => args.entities = Some(take("--entities")?),
      "--islands" => args.islands = Some(take("--islands")?),
      "--tool" => args.tool = Some(parse_count(&take("--tool")?, "--tool")?),
      "--id" => args.id = Some(take("--id")?),
      "--name" => args.name = Some(take("--name")?),
      "--side" => args.side = Some(take("--side")?),
      "--strategy" => args.strategy = Some(take("--strategy")?),
      "--retract" => args.retract = Some(take("--retract")?),
      "--stepover" => args.stepover = Some(parse_mm(&take("--stepover")?, "--stepover")?),
      "--allowance" => args.allowance = Some(parse_mm(&take("--allowance")?, "--allowance")?),
      "--pass-depth" => args.pass_depth = Some(parse_mm(&take("--pass-depth")?, "--pass-depth")?),
      "--start-depth" => {
        args.start_depth = Some(parse_mm(&take("--start-depth")?, "--start-depth")?)
      }
      "--cut-depth" => args.cut_depth = Some(parse_mm(&take("--cut-depth")?, "--cut-depth")?),
      "--lead-in" => args.lead_in = Some(parse_mm(&take("--lead-in")?, "--lead-in")?),
      "--lead-out" => args.lead_out = Some(parse_mm(&take("--lead-out")?, "--lead-out")?),
      "--peck-depth" => args.peck_depth = Some(parse_mm(&take("--peck-depth")?, "--peck-depth")?),
      "--retract-amount" => {
        args.retract_amount = Some(parse_mm(&take("--retract-amount")?, "--retract-amount")?)
      }
      "--bank" => args.bank = Some(parse_bank(&take("--bank")?)?),
      "--chamfer-width" => {
        args.chamfer_width = Some(parse_mm(&take("--chamfer-width")?, "--chamfer-width")?)
      }
      "--ramp" => args.ramp = Some(parse_mm(&take("--ramp")?, "--ramp")?),
      "--spacing" => args.spacing = Some(parse_mm(&take("--spacing")?, "--spacing")?),
      "--pitch" => args.pitch = Some(parse_mm(&take("--pitch")?, "--pitch")?),
      "--thread-direction" => args.thread_direction = Some(take("--thread-direction")?),
      "--inlay-allowance" => {
        args.inlay_allowance = Some(parse_mm(&take("--inlay-allowance")?, "--inlay-allowance")?)
      }
      "--angle" => {
        args.angle = Some(parse_degrees(&take("--angle")?, "--angle")?);
      }
      "--tab" => args.tabs.push(parse_tab(&take("--tab")?)?),
      "--clear" => {
        args.clear = true;
        index += 1;
        continue;
      }
      other if other.starts_with('-') => {
        return Err(Failure::input(format!("unknown toolpath option {other:?}")))
      }
      other => {
        if args.path.is_some() {
          return Err(Failure::input(
            "toolpath takes exactly one document, before its options",
          ));
        }
        args.path = Some(PathBuf::from(other));
        index += 1;
        continue;
      }
    }
    index += 2;
  }

  let path = args
    .path
    .clone()
    .ok_or_else(|| Failure::input("toolpath requires a document path"))?;
  let kind = args
    .kind
    .clone()
    .ok_or_else(|| Failure::input("toolpath requires --kind"))?;
  let entity_ids: Vec<String> = args
    .entities
    .as_deref()
    .ok_or_else(|| Failure::input("toolpath requires --entities"))?
    .split(',')
    .map(str::trim)
    .filter(|id| !id.is_empty())
    .map(str::to_owned)
    .collect();
  if entity_ids.is_empty() {
    return Err(Failure::input("--entities names no entity"));
  }
  let island_ids: Vec<String> = args
    .islands
    .as_deref()
    .map(|ids| {
      ids
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .collect()
    })
    .unwrap_or_default();
  if !island_ids.is_empty() && kind != "pocket" {
    return Err(Failure::input(format!(
      "--islands is only valid with --kind pocket, not {kind:?}"
    )));
  }
  let tool_number = args
    .tool
    .ok_or_else(|| Failure::input("toolpath requires --tool"))?;

  let mut document = open(&path)?;
  let library = open_library_hint(&document, &path, args.library.as_deref())?;
  // The bank summary below needs the tool the same way `emit` does, so read the whole tool here.
  let tool_data = library
    .as_ref()
    .and_then(|library| library.tool_for_number(tool_number).ok().flatten());
  let diameter = tool_data
    .as_ref()
    .and_then(|(geometry, _cutting)| geometry.diameter);
  // A chamfer's depth comes from the bit's angle, so it is read here, where the tool is, and written
  // into the document (D-M6-2, D-M6-9). Its absence is refused by name below. The tip's flat is read
  // for the same reason: it disqualifies the bit from cutting a bevel of the width the user names
  // (D-M6-10).
  let included_angle = tool_data
    .as_ref()
    .and_then(|(geometry, _cutting)| geometry.included_angle);
  let flat_diameter = tool_data
    .as_ref()
    .and_then(|(geometry, _cutting)| geometry.flat_diameter);
  let cutting = tool_data.map(|(_geometry, cutting)| cutting);

  let operation = match kind.as_str() {
    "profile" => {
      let side = match args.side.as_deref() {
        None | Some("outside") => cam_model::ProfileSide::Outside,
        Some("inside") => cam_model::ProfileSide::Inside,
        Some("on") => cam_model::ProfileSide::On,
        Some(other) => {
          return Err(Failure::input(format!(
            "unknown --side {other:?}; expected inside, outside or on"
          )))
        }
      };
      Operation::Profile(cam_model::ProfileParams {
        side,
        allowance: mm(args.allowance.unwrap_or(0.0)),
        pass_depth: mm(default_pass_depth(
          args.pass_depth,
          cutting.as_ref().and_then(|cutting| cutting.stepdown),
          "profile",
        )?),
        start_depth: mm(args.start_depth.unwrap_or(0.0)),
        cut_depth: mm(default_cut_depth(args.cut_depth)),
        lead_in: mm(args.lead_in.unwrap_or(0.0)),
        lead_out: mm(args.lead_out.unwrap_or(0.0)),
        tabs: args
          .tabs
          .iter()
          .map(|(position, width, height)| cam_model::Tab {
            position: *position,
            width: mm(*width),
            height: mm(*height),
          })
          .collect(),
      })
    }
    "pocket" => {
      let strategy = match args.strategy.as_deref() {
        None | Some("offset") => cam_model::PocketStrategy::Offset,
        Some("raster") => cam_model::PocketStrategy::Raster,
        Some(other) => {
          return Err(Failure::input(format!(
            "unknown --strategy {other:?}; expected offset or raster"
          )))
        }
      };
      let library_stepover = cutting.as_ref().and_then(|cutting| {
        if args.clear {
          cutting.clear_stepover
        } else {
          cutting.stepover
        }
      });
      let stepover = match (args.stepover, library_stepover) {
        (Some(stepover), _) => stepover,
        (None, Some(stepover)) => stepover.mm(),
        (None, None) => {
          return Err(Failure::input(
            "a pocket needs --stepover: the tool library has no stepover for that tool",
          ))
        }
      };
      Operation::Pocket(cam_model::PocketParams {
        strategy,
        allowance: mm(args.allowance.unwrap_or(0.0)),
        stepover: mm(stepover),
        pass_depth: mm(default_pass_depth(
          args.pass_depth,
          cutting.as_ref().and_then(|cutting| cutting.stepdown),
          "pocket",
        )?),
        start_depth: mm(args.start_depth.unwrap_or(0.0)),
        cut_depth: mm(default_cut_depth(args.cut_depth)),
      })
    }
    "drilling" => {
      let retract = match args.retract.as_deref() {
        None | Some("full") => cam_model::RetractMode::Full,
        Some("amount") => cam_model::RetractMode::Amount {
          amount: mm(
            args
              .retract_amount
              .ok_or_else(|| Failure::input("--retract amount requires --retract-amount"))?,
          ),
        },
        Some(other) => {
          return Err(Failure::input(format!(
            "unknown --retract {other:?}; expected full or amount"
          )))
        }
      };
      Operation::Drilling(cam_model::DrillParams {
        peck_depth: mm(args.peck_depth.unwrap_or(0.0)),
        retract,
        start_depth: mm(args.start_depth.unwrap_or(0.0)),
        cut_depth: mm(default_cut_depth(args.cut_depth)),
        bank: args.bank.map(|(count, pitch)| cam_model::DrillBank {
          count,
          pitch: mm(pitch),
        }),
      })
    }
    "chamfer" => {
      // A chamfer has no side and no authored depth: the bevel is symmetric about the vector and its Z
      // is derived from the bit's angle (D-M6-2 as amended in L7). Both flags would otherwise be
      // accepted and ignored, which is a silently different bevel; they are refused by name instead.
      for (flag, given) in [
        ("--side", args.side.is_some()),
        ("--cut-depth", args.cut_depth.is_some()),
        ("--pass-depth", args.pass_depth.is_some()),
      ] {
        if given {
          return Err(Failure::input(format!(
            "--kind chamfer takes no {flag}: the tip runs on the vector and its depth is derived \
             from --chamfer-width and the tool's included angle"
          )));
        }
      }
      let chamfer_width = args
        .chamfer_width
        .ok_or_else(|| Failure::input("a chamfer needs --chamfer-width, in millimetres"))?;
      // D-M6-9: a tool without an angle has no bevel behind it. Defaulting 90 degrees would cut a
      // different chamfer, so the refusal names the tool and nothing is written.
      let included_angle = included_angle.ok_or_else(|| {
        Failure::input(format!(
          "tool T{tool_number} has no included angle in the library, so a chamfer's depth cannot be \
           derived: a chamfer needs a V-bit"
        ))
      })?;
      // D-M6-10: with a flat of diameter `f` the bevel reaches the surface at `f/2 + d·tan(θ/2)`, so the
      // derived depth cannot honour `--chamfer-width` on such a bit. Refused by name rather than
      // cutting a bevel wider than the number the user typed.
      if let Some(flat) = flat_diameter {
        if flat.mm() > 0.0 {
          return Err(Failure::input(format!(
            "tool T{tool_number} has a {} mm flat tip, so a chamfer's bevel would reach the surface \
             wider than --chamfer-width {chamfer_width} mm: a chamfer needs a sharp V-bit",
            flat.mm()
          )));
        }
      }
      Operation::Chamfer(cam_model::ChamferParams {
        chamfer_width: mm(chamfer_width),
        included_angle,
        start_depth: mm(args.start_depth.unwrap_or(0.0)),
      })
    }
    "engrave" => {
      // An engrave is one pass at one depth with the tool centre on the vector (D-M6-4). A side, a pass
      // law, a tab and a lead each have no meaning here, and accepting them would mean ignoring them:
      // a `--pass-depth 2` nobody applied still looks applied. Refused by name instead.
      for (flag, given) in [
        ("--side", args.side.is_some()),
        ("--pass-depth", args.pass_depth.is_some()),
        ("--tab", !args.tabs.is_empty()),
        ("--lead-in", args.lead_in.is_some()),
        ("--lead-out", args.lead_out.is_some()),
      ] {
        if given {
          return Err(Failure::input(format!(
            "--kind engrave takes no {flag}: an engrave is one pass at a constant depth, with the tool \
             centre on the vector, no stepdown, no tabs and no lead"
          )));
        }
      }
      Operation::Engrave(cam_model::EngraveParams {
        start_depth: mm(args.start_depth.unwrap_or(0.0)),
        cut_depth: mm(default_cut_depth(args.cut_depth)),
      })
    }
    "fluting" => {
      // A flute is one pass along the vector with its centre on it (D-M6-5), so every flag a profile or
      // a pocket would consume has no meaning here: a side, a pass law, a tab, a lead, a stepover and an
      // allowance all widen or chunk a cut that is neither widened nor chunked. Accepting them would
      // mean ignoring them, and a `--stepover 2` nobody applied still looks applied. Refused by name.
      for (flag, given) in [
        ("--side", args.side.is_some()),
        ("--pass-depth", args.pass_depth.is_some()),
        ("--tab", !args.tabs.is_empty()),
        ("--lead-in", args.lead_in.is_some()),
        ("--lead-out", args.lead_out.is_some()),
        ("--stepover", args.stepover.is_some()),
        ("--allowance", args.allowance.is_some()),
      ] {
        if given {
          return Err(Failure::input(format!(
            "--kind fluting takes no {flag}: a flute is one pass along the vector with the tool centre \
             on it, and its only Z law is --ramp"
          )));
        }
      }
      Operation::Fluting(cam_model::FlutingParams {
        start_depth: mm(args.start_depth.unwrap_or(0.0)),
        cut_depth: mm(default_cut_depth(args.cut_depth)),
        ramp: mm(args.ramp.unwrap_or(0.0)),
      })
    }
    "texture" => {
      // A texture cuts grooves only (D-M6-6), so every flag a profile, a pocket or a flute would
      // consume has no meaning here: a side, a pass law, a tab, a lead, an allowance and a ramp all
      // widen, chunk or tilt a groove the pattern alone defines. `--stepover` belongs to a pocket: a
      // texture's equivalent is `--spacing`, and accepting both would leave two numbers naming one
      // distance. Refused by name, so nothing that was never applied still looks applied.
      for (flag, given) in [
        ("--side", args.side.is_some()),
        ("--pass-depth", args.pass_depth.is_some()),
        ("--tab", !args.tabs.is_empty()),
        ("--lead-in", args.lead_in.is_some()),
        ("--lead-out", args.lead_out.is_some()),
        ("--allowance", args.allowance.is_some()),
        ("--stepover", args.stepover.is_some()),
        ("--ramp", args.ramp.is_some()),
      ] {
        if given {
          return Err(Failure::input(format!(
            "--kind texture takes no {flag}: a texture cuts parallel grooves at --spacing apart and \
             leaves the material between them standing, so it takes no side, no pass law, no tab, no \
             lead, no allowance, no stepover and no ramp"
          )));
        }
      }
      // `--spacing` falls back to the library's stepover exactly the way a pocket's `--stepover`
      // does, and its absence is an input error naming the flag rather than a defaulted lattice.
      let spacing = match (
        args.spacing,
        cutting.as_ref().and_then(|cutting| cutting.stepover),
      ) {
        (Some(spacing), _) => spacing,
        (None, Some(stepover)) => stepover.mm(),
        (None, None) => {
          return Err(Failure::input(
            "a texture needs --spacing: the tool library has no stepover for that tool",
          ))
        }
      };
      Operation::Texture(cam_model::TextureParams {
        start_depth: mm(args.start_depth.unwrap_or(0.0)),
        cut_depth: mm(default_cut_depth(args.cut_depth)),
        spacing: mm(spacing),
        angle: cam_units::Angle::from_degrees(args.angle.unwrap_or(0.0)),
      })
    }
    "inlay" => {
      // An inlay is a male/female pair of pockets (D-M6-7), so every flag that belongs to a different
      // cutting law has no meaning here: a bevel's width, a groove's spacing or angle, a flute's ramp, a
      // profile's side, tabs and leads. `--strategy` is refused with them because the pair's parameters
      // carry no clearing pattern at all, so a `--strategy raster` nobody stored would still look
      // applied. Refused by name, so nothing that was never applied looks applied.
      for (flag, given) in [
        ("--side", args.side.is_some()),
        ("--tab", !args.tabs.is_empty()),
        ("--lead-in", args.lead_in.is_some()),
        ("--lead-out", args.lead_out.is_some()),
        ("--ramp", args.ramp.is_some()),
        ("--spacing", args.spacing.is_some()),
        ("--angle", args.angle.is_some()),
        ("--chamfer-width", args.chamfer_width.is_some()),
        ("--strategy", args.strategy.is_some()),
      ] {
        if given {
          return Err(Failure::input(format!(
            "--kind inlay takes no {flag}: an inlay is a male/female pair of pockets, cleared with the \
             pocket's offset pattern at --stepover apart, with one allowance between them"
          )));
        }
      }
      // The pair names itself, and the names are part of the contract: two toolpaths per command, in a
      // fixed order, so one `--id` or `--name` could only ever name one of them.
      if args.id.is_some() || args.name.is_some() {
        return Err(Failure::input(
          "--kind inlay names its two toolpaths itself: it appends \"inlay female\" and \"inlay male\" \
           with consecutive ids, so --id and --name cannot name a pair",
        ));
      }
      // The pocket's library fallbacks, unchanged: the stepover from the library (`--clear` asking for
      // the clear stepover, exactly as a pocket does) and the pass depth from its stepdown.
      let library_stepover = cutting.as_ref().and_then(|cutting| {
        if args.clear {
          cutting.clear_stepover
        } else {
          cutting.stepover
        }
      });
      let stepover = match (args.stepover, library_stepover) {
        (Some(stepover), _) => stepover,
        (None, Some(stepover)) => stepover.mm(),
        (None, None) => {
          return Err(Failure::input(
            "an inlay needs --stepover: the tool library has no stepover for that tool",
          ))
        }
      };
      Operation::Inlay(cam_model::InlayParams {
        allowance: mm(args.inlay_allowance.unwrap_or(0.0)),
        stepover: mm(stepover),
        pass_depth: mm(default_pass_depth(
          args.pass_depth,
          cutting.as_ref().and_then(|cutting| cutting.stepdown),
          "inlay",
        )?),
        start_depth: mm(args.start_depth.unwrap_or(0.0)),
        cut_depth: mm(default_cut_depth(args.cut_depth)),
      })
    }
    "thread" => {
      // A thread is one helical descent around a circle (A7), so every flag that belongs to another
      // cutting law has no meaning here: a side, a pass law, a tab, a lead, a stepover, an allowance, a
      // ramp, a bevel's width and a clearing strategy each describe a cut a thread does not make.
      // Refused by name, so nothing that was never applied looks applied.
      for (flag, given) in [
        ("--side", args.side.is_some()),
        ("--pass-depth", args.pass_depth.is_some()),
        ("--tab", !args.tabs.is_empty()),
        ("--lead-in", args.lead_in.is_some()),
        ("--lead-out", args.lead_out.is_some()),
        ("--stepover", args.stepover.is_some()),
        ("--allowance", args.allowance.is_some()),
        ("--ramp", args.ramp.is_some()),
        ("--chamfer-width", args.chamfer_width.is_some()),
        ("--strategy", args.strategy.is_some()),
        ("--inlay-allowance", args.inlay_allowance.is_some()),
      ] {
        if given {
          return Err(Failure::input(format!(
            "--kind thread takes no {flag}: a thread is one helical descent around each circle, one \
             --pitch per revolution, so it takes no side, no pass law, no tab, no lead, no stepover, no \
             allowance and no other strategy's geometry"
          )));
        }
      }
      let pitch = args
        .pitch
        .ok_or_else(|| Failure::input("a thread needs --pitch, in millimetres"))?;
      let direction = match args.thread_direction.as_deref() {
        None | Some("internal") => cam_model::ThreadDirection::Internal,
        Some("external") => cam_model::ThreadDirection::External,
        Some(other) => {
          return Err(Failure::input(format!(
            "unknown --thread-direction {other:?}; expected internal or external"
          )))
        }
      };
      Operation::Thread(cam_model::ThreadParams {
        pitch: mm(pitch),
        direction,
        start_depth: mm(args.start_depth.unwrap_or(0.0)),
        cut_depth: mm(default_cut_depth(args.cut_depth)),
      })
    }
    other => {
      return Err(Failure::input(format!(
        "unknown toolpath kind {other:?}; expected profile, pocket, drilling, chamfer, engrave, \
         fluting, texture, inlay or thread"
      )))
    }
  };

  // An inlay is two toolpaths from one command, in a fixed order (A6, D-M6-7), so it names them itself
  // and appends both before the document is written: one half would be a document whose only outcome is
  // a male with no cavity, or a cavity with no plug.
  if let Operation::Inlay(params) = &operation {
    return append_inlay(
      &mut document,
      args.out.clone().unwrap_or(path),
      &entity_ids,
      tool_number,
      params,
      diameter,
      cutting.as_ref(),
    );
  }

  let id = match args.id.clone() {
    Some(id) => id,
    None => {
      let mut number = document.toolpaths.len() + 1;
      while document
        .toolpaths
        .iter()
        .any(|toolpath| toolpath.id == format!("TP{number}"))
      {
        number += 1;
      }
      format!("TP{number}")
    }
  };
  let name = match args.name.clone() {
    Some(name) => name,
    None => {
      let base = match operation {
        Operation::Profile(_) => "Profile",
        Operation::Pocket(_) => "Pocket",
        Operation::Drilling(_) => "Drilling",
        Operation::Chamfer(_) => "Chamfer",
        Operation::Engrave(_) => "Engrave",
        Operation::Fluting(_) => "Fluting",
        Operation::Texture(_) => "Texture",
        Operation::Inlay(_) => "Inlay",
        Operation::Thread(_) => "Thread",
      };
      let ordinal = document.toolpaths.len() + 1;
      format!("{base} {ordinal}")
    }
  };

  // A drilling toolpath cuts circles only (D-M4-5), and a pocket needs a closed region. Appending a
  // toolpath that can never be computed would leave a document whose only failure mode is a later
  // `emit`, so the selection is checked here, while the command can still say no and change nothing.
  if matches!(operation, Operation::Drilling(_)) {
    for entity_id in &entity_ids {
      let contour = contour_of(&document, &id, entity_id)?;
      if circle_centre(contour).is_none() {
        return Err(Failure::input(
          ToolpathError::NotACircle {
            entity: entity_id.clone(),
          }
          .to_string(),
        ));
      }
    }
  }

  // A thread runs on a circle and nowhere else (A7): its diameter is what `(D ± t)/2` is measured from, so
  // a selection that is not a circle is refused here, while the command can still say no and change
  // nothing, rather than left for a later `emit` to discover. The strategy is then asked for the whole
  // path, because one of its refusals — a tool that does not fit inside the hole — needs the tool's
  // diameter from the library and cannot be answered by the geometry alone.
  if let Operation::Thread(params) = &operation {
    if let Some(diameter) = diameter {
      let spec = ToolSpec {
        number: tool_number,
        diameter,
        included_angle: None,
        feed_rate: cutting
          .as_ref()
          .and_then(|cutting| cutting.feed_rate)
          .unwrap_or(0.0),
        plunge_rate: cutting
          .as_ref()
          .and_then(|cutting| cutting.plunge_rate)
          .unwrap_or(0.0),
        spindle_speed: cutting
          .as_ref()
          .and_then(|cutting| cutting.spindle_speed)
          .unwrap_or(0),
      };
      for entity_id in &entity_ids {
        let contour = contour_of(&document, &id, entity_id)?;
        thread(&ThreadRequest {
          entity: entity_id,
          contour,
          params,
          tool: &spec,
          job: &document.job,
        })
        .map_err(|error| Failure::input(error.to_string()))?;
      }
    }
  }

  document.toolpaths.push(Toolpath {
    id: id.clone(),
    name,
    entity_ids: entity_ids.clone(),
    island_ids: island_ids.clone(),
    tool_number,
    operation: operation.clone(),
  });
  document
    .validate()
    .map_err(|error| Failure::input(error.to_string()))?;

  let destination = args.out.clone().unwrap_or(path);
  cam_store::document::save_document(&destination, &document)
    .map_err(|error| Failure::input(error.to_string()))?;

  let islands = if island_ids.is_empty() {
    String::new()
  } else {
    format!(", {} island(s)", island_ids.len())
  };
  println!(
    "added {id} {}, tool T{tool_number}, {} entity(ies){islands}",
    operation_label(&operation),
    entity_ids.len()
  );
  // A drill bank is selection only (A9): say which drill cuts each hole so the gang can be loaded.
  // Nothing of it reaches the program, and a selection that cannot be computed says why.
  let bank_report = match &operation {
    Operation::Drilling(params) if params.bank.is_some() => Some(bank_summary(
      &document,
      &entity_ids,
      tool_number,
      params,
      diameter,
      cutting.as_ref(),
    )),
    _ => None,
  };
  if let Some(report) = bank_report {
    println!("{report}");
  }
  Ok(())
}

/// Append the two toolpaths one inlay means, in the fixed order, and write the document back.
///
/// `inlay female` (the region grown by the allowance) is appended first and `inlay male` (the job
/// rectangle with that region standing as an island) second, with consecutive `TPn` ids and those exact
/// names (M6 A6, D-M6-7). Both carry the **same** [`cam_model::InlayParams`]; what tells them apart is
/// what the document already records about what a toolpath does — the female names the region it clears
/// as its entities, and the male names the same region as the islands it leaves standing and takes no
/// boundary entity, because its boundary is the job rectangle and the job is not an entity.
///
/// The pair's own geometry rules are asked here, by the real strategy, before anything is written: a
/// document whose only failure mode is a later `emit` is worse than a command that says no. A tool
/// whose diameter the library does not record is not a reason to refuse here — `emit` names that one —
/// so the check is skipped and the pocket's own refusal takes over.
fn append_inlay(
  document: &mut Document,
  destination: PathBuf,
  entity_ids: &[String],
  tool_number: u32,
  params: &cam_model::InlayParams,
  diameter: Option<Length>,
  cutting: Option<&CuttingData>,
) -> Result<(), Failure> {
  let female_id = next_toolpath_id(document, None);
  let male_id = next_toolpath_id(document, Some(&female_id));

  if let Some(diameter) = diameter {
    let spec = ToolSpec {
      number: tool_number,
      diameter,
      included_angle: None,
      feed_rate: cutting.and_then(|cutting| cutting.feed_rate).unwrap_or(0.0),
      plunge_rate: cutting
        .and_then(|cutting| cutting.plunge_rate)
        .unwrap_or(0.0),
      spindle_speed: cutting
        .and_then(|cutting| cutting.spindle_speed)
        .unwrap_or(0),
    };
    let mut contours = Vec::with_capacity(entity_ids.len());
    for entity_id in entity_ids {
      contours.push(contour_of(document, &female_id, entity_id)?);
    }
    inlay(&InlayRequest {
      contours: &contours,
      params,
      tool: &spec,
      job: &document.job,
    })
    .map_err(|error| Failure::input(error.to_string()))?;
  }

  document.toolpaths.push(Toolpath {
    id: female_id.clone(),
    name: "inlay female".to_owned(),
    entity_ids: entity_ids.to_vec(),
    island_ids: Vec::new(),
    tool_number,
    operation: Operation::Inlay(params.clone()),
  });
  document.toolpaths.push(Toolpath {
    id: male_id.clone(),
    name: "inlay male".to_owned(),
    entity_ids: Vec::new(),
    island_ids: entity_ids.to_vec(),
    tool_number,
    operation: Operation::Inlay(params.clone()),
  });
  document
    .validate()
    .map_err(|error| Failure::input(error.to_string()))?;

  cam_store::document::save_document(&destination, document)
    .map_err(|error| Failure::input(error.to_string()))?;

  println!(
    "added {female_id} inlay female, tool T{tool_number}, {} entity(ies)",
    entity_ids.len()
  );
  println!(
    "added {male_id} inlay male, tool T{tool_number}, {} island(s)",
    entity_ids.len()
  );
  Ok(())
}

/// The next free `TPn` id, skipping `taken` as well when a pair is being named at once.
///
/// The same scan the single-toolpath path performs — from `len + 1` upwards, so a second run of the same
/// command appends the next pair instead of colliding — with one extra exclusion, because the two ids of
/// a pair are chosen before either is pushed.
fn next_toolpath_id(document: &Document, taken: Option<&str>) -> String {
  let mut number = document.toolpaths.len() + 1;
  loop {
    let id = format!("TP{number}");
    let used = document.toolpaths.iter().any(|toolpath| toolpath.id == id);
    if !used && taken != Some(id.as_str()) {
      return id;
    }
    number += 1;
  }
}

/// The bank selection a banked drilling toolpath will produce, for whoever loads the gang.
///
/// The selection is computed by the real strategy, so the report is exactly what `emit` will cut. A
/// bank emits no machine command (S9, A9), so this report is all the operator gets from the document.
/// When there is nothing to select from — no tool diameter in the library, or an entity that is not a
/// circle — the reason is reported instead of a guess.
fn bank_summary(
  document: &Document,
  entity_ids: &[String],
  tool_number: u32,
  params: &cam_model::DrillParams,
  diameter: Option<Length>,
  cutting: Option<&CuttingData>,
) -> String {
  let Some(bank) = params.bank else {
    return String::new();
  };
  let headline = format!(
    "bank of {} drill(s) at {} mm pitch",
    bank.count,
    bank.pitch.mm()
  );
  let Some(diameter) = diameter else {
    return format!(
      "{headline}: no selection to report, the tool library has no diameter for T{tool_number}"
    );
  };
  // Drilling selects a drill, never a cone: the bank report asks the strategy only about the XY its
  // holes and pitches put the head at, so the tool's angle is irrelevant here and stays unset.
  let spec = ToolSpec {
    number: tool_number,
    diameter,
    included_angle: None,
    feed_rate: cutting.and_then(|cutting| cutting.feed_rate).unwrap_or(0.0),
    plunge_rate: cutting
      .and_then(|cutting| cutting.plunge_rate)
      .unwrap_or(0.0),
    spindle_speed: cutting
      .and_then(|cutting| cutting.spindle_speed)
      .unwrap_or(0),
  };

  let mut holes = Vec::with_capacity(entity_ids.len());
  for entity_id in entity_ids {
    let centre = document
      .entity(entity_id)
      .and_then(|entity| entity.kind.as_contour())
      .and_then(circle_centre);
    let Some(centre) = centre else {
      return format!("{headline}: no selection to report, entity {entity_id:?} is not a circle");
    };
    holes.push(DrillHole {
      entity: entity_id.clone(),
      centre,
    });
  }

  match drill(&DrillRequest {
    holes: &holes,
    params,
    tool: &spec,
    job: &document.job,
  }) {
    Ok(path) => {
      let picks: Vec<String> = path
        .order
        .iter()
        .zip(&path.drill_index)
        .map(|(entity, index)| match index {
          Some(index) => format!("D{index} cuts {entity}"),
          None => entity.clone(),
        })
        .collect();
      format!(
        "{headline}: {}; head travel {:.3} mm",
        picks.join(", "),
        path.bank_travel.mm()
      )
    }
    Err(error) => format!("{headline}: no selection to report, {error}"),
  }
}

/// The tool library to read defaults from, if the caller named one or the document references one.
///
/// A `tools_ref` that does not exist is not an error here: the strategy may not need a default, and
/// opening it would create an empty library as a side effect of asking.
fn open_library_hint(
  document: &Document,
  document_path: &Path,
  explicit: Option<&Path>,
) -> Result<Option<ToolLibrary>, Failure> {
  let candidate = explicit.map(Path::to_path_buf).or_else(|| {
    document.tools_ref.as_ref().map(|reference| {
      document_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(reference)
    })
  });
  match candidate {
    Some(path) if path.exists() => ToolLibrary::open(&path)
      .map(Some)
      .map_err(|error| Failure::input(error.to_string())),
    Some(path) if explicit.is_some() => Err(Failure::input(format!(
      "{}: no such tool library",
      path.display()
    ))),
    _ => Ok(None),
  }
}

fn default_pass_depth(
  given: Option<f64>,
  library: Option<Length>,
  kind: &str,
) -> Result<f64, Failure> {
  match (given, library) {
    (Some(pass_depth), _) => Ok(pass_depth),
    (None, Some(stepdown)) => Ok(stepdown.mm()),
    (None, None) => Err(Failure::input(format!(
      "a {kind} needs --pass-depth: the tool library has no stepdown for that tool"
    ))),
  }
}

fn default_cut_depth(given: Option<f64>) -> f64 {
  given.unwrap_or_else(|| cam_model::ProfileParams::default().cut_depth.mm())
}

fn operation_label(operation: &Operation) -> String {
  match operation {
    Operation::Profile(_) => "profile".to_owned(),
    Operation::Pocket(params) => match params.strategy {
      cam_model::PocketStrategy::Raster => "pocket (raster)".to_owned(),
      cam_model::PocketStrategy::Offset => "pocket (offset)".to_owned(),
    },
    Operation::Drilling(params) => {
      if params.bank.is_some() {
        "drilling (bank)".to_owned()
      } else {
        "drilling".to_owned()
      }
    }
    Operation::Chamfer(_) => "chamfer".to_owned(),
    Operation::Engrave(_) => "engrave".to_owned(),
    Operation::Fluting(_) => "fluting".to_owned(),
    Operation::Inlay(_) => "inlay".to_owned(),
    Operation::Thread(params) => format!(
      "thread ({} mm pitch, {})",
      params.pitch.mm(),
      match params.direction {
        cam_model::ThreadDirection::Internal => "internal",
        cam_model::ThreadDirection::External => "external",
      }
    ),
    Operation::Texture(params) => format!(
      "texture ({} mm at {} degrees)",
      params.spacing.mm(),
      params.angle.degrees()
    ),
  }
}

fn mm(value: f64) -> Length {
  Length::from_mm(value)
}

fn value_at(rest: &[String], index: usize, what: &str) -> Result<String, Failure> {
  rest
    .get(index + 1)
    .filter(|value| !value.starts_with("--"))
    .cloned()
    .ok_or_else(|| Failure::input(format!("{what} requires a value")))
}

fn parse_mm(value: &str, option: &str) -> Result<f64, Failure> {
  value.parse::<f64>().map_err(|_| {
    Failure::input(format!(
      "{option} expects a number of millimetres, got {value:?}"
    ))
  })
}

fn parse_degrees(value: &str, option: &str) -> Result<f64, Failure> {
  value.parse::<f64>().map_err(|_| {
    Failure::input(format!(
      "{option} expects a number of degrees, got {value:?}"
    ))
  })
}

fn parse_count(value: &str, option: &str) -> Result<u32, Failure> {
  value
    .parse::<u32>()
    .map_err(|_| Failure::input(format!("{option} expects a whole number, got {value:?}")))
}

fn parse_bank(value: &str) -> Result<(u32, f64), Failure> {
  let (count, pitch) = value
    .split_once(':')
    .ok_or_else(|| Failure::input(format!("--bank expects <count>:<pitch> mm, got {value:?}")))?;
  Ok((parse_count(count, "--bank")?, parse_mm(pitch, "--bank")?))
}

fn parse_tab(value: &str) -> Result<(f64, f64, f64), Failure> {
  let parts: Vec<&str> = value.split(':').collect();
  match parts.as_slice() {
    [position, width, height] => Ok((
      position.parse::<f64>().map_err(|_| {
        Failure::input(format!(
          "--tab position must be a fraction, got {position:?}"
        ))
      })?,
      parse_mm(width, "--tab")?,
      parse_mm(height, "--tab")?,
    )),
    _ => Err(Failure::input(format!(
      "--tab expects <position>:<width>:<height>, got {value:?}"
    ))),
  }
}

/// The contour of an entity, or the refusal that names why it is not one.
fn contour_of<'a>(
  document: &'a Document,
  toolpath_id: &str,
  entity_id: &str,
) -> Result<&'a cam_geom::Contour, Failure> {
  let entity = document.entity(entity_id).ok_or_else(|| {
    Failure::input(format!(
      "toolpath {toolpath_id:?} cites unknown entity {entity_id:?}"
    ))
  })?;
  entity
    .kind
    .as_contour()
    .ok_or_else(|| Failure::input(format!("entity {entity_id:?} is not a contour")))
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
    included_angle: geometry.included_angle,
    feed_rate: cutting.feed_rate.unwrap_or(0.0),
    plunge_rate: cutting.plunge_rate.unwrap_or(0.0),
    spindle_speed: cutting.spindle_speed.unwrap_or(0),
  };

  // The shape the simulation sweeps (A8). A chamfer is the only operation that cuts with a cone, and
  // its cone is the library's own geometry: the bit's included angle and its flat, zero when the
  // library does not record one. A chamfer on a tool with no angle is refused by name rather than
  // defaulted to 90 degrees, because that would cut a different bevel (D-M6-9).
  let tool_shape = match &toolpath.operation {
    Operation::Chamfer(params) => {
      let included_angle = geometry.included_angle.ok_or_else(|| {
        Failure::input(format!(
          "tool T{number} has no included angle, so toolpath {:?} cannot cut a chamfer: a V-bit \
           strategy needs the tool's angle, and defaulting 90 degrees would cut a different bevel",
          toolpath.id
        ))
      })?;
      // The same refusal as at authoring, repeated here because the library can change under a
      // document that was written against a sharp bit: a flat widens the bevel to
      // `chamfer_width + flat/2`, and the document's derived depth knows nothing of it (D-M6-10).
      // A program that would cut a bevel nobody asked for is not written, and the simulation refuses
      // the same shape, so the bit fails at every boundary that can see it.
      let flat_diameter = geometry.flat_diameter.unwrap_or(Length::ZERO);
      if flat_diameter.mm() > 0.0 {
        return Err(Failure::input(format!(
          "tool T{number} has a {} mm flat tip, so toolpath {:?} would cut a bevel wider than its \
           {} mm chamfer width: a chamfer needs a sharp V-bit",
          flat_diameter.mm(),
          toolpath.id,
          params.chamfer_width.mm()
        )));
      }
      ToolShape::v_bit(included_angle, flat_diameter)
    }
    // An engrave, a flute or a texture carries the tool's real shape, whatever it is: a V-bit when the
    // library records an angle and a cylinder otherwise (M6 A8). Nothing about the path depends on it —
    // a groove's depth is authored and a ramp is a length — but the simulation must sweep the groove the
    // bit actually cuts, and a segment that said "cylinder" for a V-bit would report a correct engrave
    // as gouge, and a correct V-groove flute or texture as a flat slot.
    Operation::Engrave(_) | Operation::Fluting(_) | Operation::Texture(_) => {
      match geometry.included_angle {
        Some(included_angle) => ToolShape::v_bit(
          included_angle,
          geometry.flat_diameter.unwrap_or(Length::ZERO),
        ),
        None => ToolShape::cylinder(spec.diameter),
      }
    }
    _ => ToolShape::cylinder(spec.diameter),
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
    Operation::Pocket(params) => {
      // Each selected contour is its own outer boundary, and the toolpath's islands are holes in
      // every one of them (A9, D-RB-1: the document names the islands, nothing is inferred from
      // nesting). A nested selection therefore still pockets both contours, without the inner one
      // silently becoming a hole.
      for entity_id in &toolpath.entity_ids {
        let contour = contour_of(document, &toolpath.id, entity_id)?;
        let mut islands = Vec::with_capacity(toolpath.island_ids.len());
        for island_id in &toolpath.island_ids {
          // `Document::validate` already refuses an island that does not resolve, but a toolpath
          // reaching `emit` without having been validated must fail by name, not panic.
          let island = document.entity(island_id).ok_or_else(|| {
            Failure::input(format!(
              "toolpath {:?} cites unknown island {island_id:?}",
              toolpath.id
            ))
          })?;
          let island = island
            .kind
            .as_contour()
            .ok_or_else(|| Failure::input(format!("island {island_id:?} is not a contour")))?;
          islands.push(island);
        }
        let path = pocket(&PocketRequest {
          contour,
          islands: &islands,
          params,
          tool: &spec,
          job: &document.job,
        })
        .map_err(|error| Failure::input(format!("toolpath {:?}: {error}", toolpath.id)))?;
        moves.extend(path.moves);
      }
    }
    Operation::Drilling(params) => {
      let mut holes = Vec::with_capacity(toolpath.entity_ids.len());
      for entity_id in &toolpath.entity_ids {
        let contour = contour_of(document, &toolpath.id, entity_id)?;
        let centre = circle_centre(contour).ok_or_else(|| {
          Failure::input(format!(
            "toolpath {:?}: {}",
            toolpath.id,
            ToolpathError::NotACircle {
              entity: entity_id.clone()
            }
          ))
        })?;
        holes.push(DrillHole {
          entity: entity_id.clone(),
          centre,
        });
      }
      let path = drill(&DrillRequest {
        holes: &holes,
        params,
        tool: &spec,
        job: &document.job,
      })
      .map_err(|error| Failure::input(format!("toolpath {:?}: {error}", toolpath.id)))?;
      moves.extend(path.moves);
    }
    Operation::Chamfer(params) => {
      for entity_id in &toolpath.entity_ids {
        let contour = contour_of(document, &toolpath.id, entity_id)?;
        moves.extend(
          chamfer(&ChamferRequest {
            contour,
            params,
            tool: &spec,
            job: &document.job,
          })
          .map_err(|error| Failure::input(format!("toolpath {:?}: {error}", toolpath.id)))?,
        );
      }
    }
    Operation::Fluting(params) => {
      // Open and closed alike, like an engrave: a flute is one groove per vector (D-M6-5).
      for entity_id in &toolpath.entity_ids {
        let contour = contour_of(document, &toolpath.id, entity_id)?;
        moves.extend(
          fluting(&FlutingRequest {
            contour,
            params,
            tool: &spec,
            job: &document.job,
          })
          .map_err(|error| Failure::input(format!("toolpath {:?}: {error}", toolpath.id)))?,
        );
      }
    }
    Operation::Engrave(params) => {
      // Open and closed vectors alike: engraving an open one is the strategy's point (D-M6-4).
      for entity_id in &toolpath.entity_ids {
        let contour = contour_of(document, &toolpath.id, entity_id)?;
        moves.extend(
          engrave(&EngraveRequest {
            contour,
            params,
            tool: &spec,
            job: &document.job,
          })
          .map_err(|error| Failure::input(format!("toolpath {:?}: {error}", toolpath.id)))?,
        );
      }
    }
    Operation::Texture(params) => {
      // Every selected contour is a boundary of **one** region, and the region's parity rule is what
      // turns an appended contour into a hole (D-RB-2), so the pattern is clipped against the whole
      // selection at once rather than once per entity (D-M6-6).
      let mut contours = Vec::with_capacity(toolpath.entity_ids.len());
      for entity_id in &toolpath.entity_ids {
        contours.push(contour_of(document, &toolpath.id, entity_id)?);
      }
      moves.extend(
        texture(&TextureRequest {
          contours: &contours,
          params,
          tool: &spec,
          job: &document.job,
        })
        .map_err(|error| Failure::input(format!("toolpath {:?}: {error}", toolpath.id)))?,
      );
    }
    Operation::Thread(params) => {
      // One descent per selected circle, and each is its own thread: a thread's diameter is the entity's
      // own, so two circles are two threads with two orbits and never one shared one (M6 A7).
      for entity_id in &toolpath.entity_ids {
        let contour = contour_of(document, &toolpath.id, entity_id)?;
        moves.extend(
          thread(&ThreadRequest {
            entity: entity_id,
            contour,
            params,
            tool: &spec,
            job: &document.job,
          })
          .map_err(|error| Failure::input(format!("toolpath {:?}: {error}", toolpath.id)))?,
        );
      }
    }
    Operation::Inlay(params) => {
      // One half per toolpath, and the document says which half it is (see
      // `cam_model::InlayParams`): the female names the region it clears, and the male names that same
      // region as the islands it leaves standing inside the job rectangle, whose boundary is not an
      // entity at all.
      let ids = if toolpath.island_ids.is_empty() {
        &toolpath.entity_ids
      } else {
        &toolpath.island_ids
      };
      let mut contours = Vec::with_capacity(ids.len());
      for entity_id in ids {
        contours.push(contour_of(document, &toolpath.id, entity_id)?);
      }
      let request = InlayRequest {
        contours: &contours,
        params,
        tool: &spec,
        job: &document.job,
      };
      let half = if toolpath.island_ids.is_empty() {
        female(&request)
      } else {
        male(&request)
      };
      moves.extend(
        half.map_err(|error| Failure::input(format!("toolpath {:?}: {error}", toolpath.id)))?,
      );
    }
  }

  Ok(Segment {
    name: toolpath.name.clone(),
    tool_number: number,
    tool: tool_shape,
    feed_rate: spec.feed_rate,
    plunge_rate: spec.plunge_rate,
    spindle_speed: spec.spindle_speed,
    moves,
  })
}

/// The cell size, as a fraction of which the deviation tolerance is stated.
///
/// An eighth of a cell is small enough that the test S6 names — a pocket cut a whole millimetre
/// shallow or deep — is still reported at a 1 mm cell, and large enough to stay above the sweep's
/// own sampling noise: the union of discs sampled at half a cell misses a boundary by at most
/// `cell² / (32 * tool radius)`, which is a hundredth of a millimetre for a 1 mm cell and a 3 mm
/// tool.
const TOLERANCE_PER_CELL: f64 = 8.0;

/// `simulate`: run the document's program over a heightfield and report what it did to the stock.
///
/// The report is the contract (A7): the same document prints the same bytes, and a program that
/// gouges, leaves material standing or rams the tool exits `6` rather than `0` (D-M5-6). Nothing is
/// written: a simulation is a question about the document, not a change to it.
fn simulate_command(rest: &[String]) -> Result<(), Failure> {
  let mut document_path: Option<PathBuf> = None;
  let mut library_path: Option<PathBuf> = None;
  let mut resolution = cam_sim::DEFAULT_CELL;

  let mut index = 0;
  while index < rest.len() {
    match rest[index].as_str() {
      "--library" => {
        library_path = Some(PathBuf::from(option_value(rest, index, "--library")?));
        index += 2;
      }
      "--resolution" => {
        resolution = parse_mm(option_value(rest, index, "--resolution")?, "--resolution")?;
        index += 2;
      }
      other if other.starts_with('-') => {
        return Err(Failure::input(format!("unknown simulate option {other:?}")))
      }
      other => {
        if document_path.is_some() {
          return Err(Failure::input("simulate takes exactly one document"));
        }
        document_path = Some(PathBuf::from(other));
        index += 1;
      }
    }
  }

  let document_path =
    document_path.ok_or_else(|| Failure::input("simulate requires a document path"))?;
  let document = open(&document_path)?;

  // The library resolves exactly as `emit`'s does: `--library`, else the document's own `tools_ref`
  // relative to the document.
  let library_path =
    match library_path.or_else(|| {
      document.tools_ref.as_ref().map(|reference| {
        document_path
          .parent()
          .unwrap_or_else(|| Path::new("."))
          .join(reference)
      })
    }) {
      Some(path) => path,
      None => return Err(Failure::input(
        "simulate needs a tool library to know the tool sizes: pass --library or set tools_ref in \
         the document",
      )),
    };
  let library =
    ToolLibrary::open(&library_path).map_err(|error| Failure::input(error.to_string()))?;

  // The program is the one `emit` would run (D-M5-1): the same segments, built by the same
  // function, so the simulation cannot disagree with the machine about what was cut.
  let mut stream = MoveStream::new(document.job.safe_z, Point2::new(0.0, 0.0));
  for toolpath in &document.toolpaths {
    stream
      .segments
      .push(build_segment(&document, toolpath, &library)?);
  }

  let cell = Length::from_mm(resolution);
  let mut field =
    Heightfield::new(&document.job, cell).map_err(|error| Failure::input(error.to_string()))?;
  // Every tool the program uses, at the shape the library gives it: a profile's target band is the
  // material its tool sweeps, and the segment already carries the shape that built it.
  let tools = ToolShapes::new(
    stream
      .segments
      .iter()
      .map(|segment| (segment.tool_number, segment.tool)),
  );
  let target =
    target_for(&document, cell, &tools).map_err(|error| Failure::input(error.to_string()))?;

  let removal = sweep::apply(&stream, &mut field);
  let deviation = deviations(
    &field,
    &target,
    Length::from_mm(cell.mm() / TOLERANCE_PER_CELL),
  )
  .map_err(|error| Failure::input(error.to_string()))?;

  print_report(&document, &stream, &field, &deviation, &removal);

  let mut defects = Vec::new();
  if deviation.gouge > 0.0 {
    defects.push(format!("gouge {} mm3", decimal(deviation.gouge)));
  }
  if deviation.uncut > 0.0 {
    defects.push(format!("uncut {} mm3", decimal(deviation.uncut)));
  }
  if !removal.crashes.is_empty() {
    defects.push(format!("{} crash(es)", removal.crashes.len()));
  }
  if defects.is_empty() {
    Ok(())
  } else {
    Err(Failure::simulation(format!(
      "the simulation found defects: {}",
      defects.join(", ")
    )))
  }
}

/// The six-line report, in one order and one decimal count, so two runs print the same bytes (S9).
fn print_report(
  document: &Document,
  stream: &MoveStream,
  field: &Heightfield,
  deviation: &cam_sim::Deviation,
  removal: &cam_sim::Removal,
) {
  let grid = field.grid();
  let volumes = field.volumes();
  let cell = short_decimal(grid.cell.mm());
  println!(
    "simulate: {} x {} x {} mm, {cell} x {cell} mm cells ({} x {}), {} toolpath(s)",
    decimal(document.job.width.mm()),
    decimal(document.job.height.mm()),
    decimal(document.job.thickness.mm()),
    grid.columns,
    grid.rows,
    document.toolpaths.len()
  );
  println!(
    "program: {} move(s), {} mm cutting length",
    stream.move_count(),
    decimal(stream.cut_length().mm())
  );
  println!(
    "removed: {} mm3 of {} mm3, {} through cell(s)",
    decimal(volumes.removed),
    decimal(grid.volume(document.job.thickness)),
    volumes.through_cells
  );
  println!(
    "gouge: {} mm3 over {} cell(s)",
    decimal(deviation.gouge),
    deviation.gouge_cells
  );
  println!(
    "uncut: {} mm3 over {} cell(s)",
    decimal(deviation.uncut),
    deviation.uncut_cells
  );
  if removal.crashes.is_empty() {
    println!("crash: none");
  }
  for crash in &removal.crashes {
    println!(
      "crash: {} move {} at ({}, {}, {})",
      crash.toolpath,
      crash.move_index,
      decimal(crash.at.x.mm()),
      decimal(crash.at.y.mm()),
      decimal(crash.at.z.mm())
    );
  }
}

/// A millimetre figure with the report's three decimals, through `cam-units` so it is locale-free.
fn decimal(value: f64) -> String {
  Format::fixed(3).apply(value)
}

/// A cell size without trailing zeroes: `1` and `0.5`, not `1.000` and `0.500`.
///
/// A resolution of one millimetre is the default and reads as `1 x 1 mm cells`; a fractional one
/// keeps as many decimals as it needs. The trimming is deterministic text manipulation on a value
/// `Format` already rounded, so the line stays byte-stable.
fn short_decimal(value: f64) -> String {
  let text = decimal(value);
  let trimmed = text.trim_end_matches('0').trim_end_matches('.');
  if trimmed.is_empty() || trimmed == "-" {
    "0".to_owned()
  } else {
    trimmed.to_owned()
  }
}
