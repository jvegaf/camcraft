//! Headless pipeline: `validate`, `inspect`, `import`, `check`, `postp validate`, `tools`, `toolpath`
//! and `emit`.
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
use cam_toolpath::{
  circle_centre, drill, pocket, profile, DrillHole, DrillRequest, PocketRequest, ProfileRequest,
  ToolSpec, ToolpathError,
};
use cam_units::Length;

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
        --kind <kind>         profile, pocket or drilling (required)
        --entities <ids>      Comma-separated entity ids (required)
        --islands <ids>       pocket: contours inside the pocket to leave standing
        --tool <number>       Tool number in the library (required)
        --side <side>         profile: inside, outside or on (default outside)
        --allowance <mm>      profile, pocket: material left on the vector
        --strategy <mode>     pocket: offset or raster (default offset)
        --stepover <mm>       pocket: ring or row spacing; defaults to the tool's stepover
        --clear               pocket: take the default from the tool's clear stepover
        --peck-depth <mm>     drilling: depth per peck, 0 for one plunge
        --retract <mode>      drilling: full (default) or amount
        --retract-amount <mm> drilling: lift above the peck when --retract amount
        --bank <count:pitch>  drilling: a gang of drills spaced along X
        --pass-depth <mm>     depth per pass; profile and pocket, defaults to the tool's stepdown
        --start-depth <mm>    depth at which cutting starts
        --cut-depth <mm>      total cut depth (default 18)
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

OPTIONS:
    -h, --help                Show this help

EXIT CODES:
    0   success
    2   invalid input: unusable path, malformed document, missing tool data
    3   unsupported document schema
    4   post-processor error, including a program the post cannot express
    5   the document has vector issues (check only)
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
    other => {
      return Err(Failure::input(format!(
        "unknown toolpath kind {other:?}; expected profile, pocket or drilling"
      )))
    }
  };

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
  let spec = ToolSpec {
    number: tool_number,
    diameter,
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
