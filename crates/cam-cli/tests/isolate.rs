//! End-to-end tests for `cam-cli isolate` (E3.6).
//!
//! These run the real binary and verify G-code generation and cam-sim acceptance.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use cam_geom::{Contour, Point2};
use cam_model::{
  Document, Entity, EntityKind, IsolationKind, IsolationParams, Job, Layer, LayerRole,
  MillingDirection, Operation, Toolpath,
};
use cam_move::{MoveStream, Segment, ToolShape};
use cam_post::{emit, parse_post, ProgramMeta};
use cam_store::document::save_document;
use cam_units::Length;

const POST: &str = include_str!("../../cam-post/tests/fixtures/post_mm.pp");

fn binary() -> &'static str {
  env!("CARGO_BIN_EXE_cam-cli")
}

fn scratch(name: &str) -> PathBuf {
  let directory = std::env::temp_dir().join(format!("camiso-{}-{name}", std::process::id()));
  let _ = std::fs::remove_dir_all(&directory);
  std::fs::create_dir_all(&directory).expect("scratch directory");
  directory
}

fn rectangle(x0: f64, y0: f64, w: f64, h: f64) -> Contour {
  Contour::from_points(
    &[
      Point2::new(x0, y0),
      Point2::new(x0 + w, y0),
      Point2::new(x0 + w, y0 + h),
      Point2::new(x0, y0 + h),
    ],
    true,
  )
  .expect("rectangle")
}

fn isolation_document(contours: Vec<Contour>, params: IsolationParams) -> Document {
  let mut document = Document::new(Job {
    width: Length::from_mm(100.0),
    height: Length::from_mm(80.0),
    thickness: Length::from_mm(1.6),
    origin: cam_model::OriginCorner::BottomLeft,
    side_up: cam_model::BoardSideUp::Top,
  });
  let entities: Vec<Entity> = contours
    .into_iter()
    .enumerate()
    .map(|(i, contour)| Entity {
      id: format!("E{i}"),
      kind: EntityKind::Contour { contour },
      source: cam_model::Provenance::default(),
    })
    .collect();
  let entity_ids: Vec<String> = entities.iter().map(|e| e.id.clone()).collect();
  document.layers.push(Layer {
    id: "L1".to_owned(),
    role: LayerRole::CopperTop,
    side: cam_model::Side::Top,
    color: "#cc0000".to_owned(),
    visible: true,
    source: cam_model::Provenance::default(),
    entities,
  });
  document.toolpaths.push(Toolpath {
    id: "TP1".to_owned(),
    name: "Top isolation".to_owned(),
    entity_ids,
    tool_id: None,
    tool_number: 1,
    operation: Operation::Isolation(params),
  });
  document
}

fn write_document(directory: &Path, document: &Document) -> PathBuf {
  let path = directory.join("board.cmc");
  save_document(&path, document).expect("document saves");
  path
}

fn run(arguments: &[&str]) -> Output {
  Command::new(binary())
    .args(arguments)
    .output()
    .expect("cam-cli runs")
}

fn stdout(output: &Output) -> String {
  String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
  String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn isolate_reports_without_post() {
  let directory = scratch("report");
  let doc = isolation_document(
    vec![rectangle(30.0, 30.0, 20.0, 20.0)],
    IsolationParams {
      passes: 1,
      overlap: 0.0,
      direction: MillingDirection::Climb,
      kind: IsolationKind::Full,
      cut_depth: Length::from_mm(0.1),
      keep_pours: false,
    },
  );
  let path = write_document(&directory, &doc);
  let output = run(&["isolate", path.to_str().unwrap(), "--diameter", "1.0"]);
  assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
  let out = stdout(&output);
  assert!(out.contains("isolate Top isolation:"), "{out}");
  assert!(out.contains("1 toolpath(s)"), "{out}");
  assert!(out.contains("move(s)"), "{out}");
}

#[test]
fn isolate_emits_gcode_accepted_by_camsim() {
  let directory = scratch("emit");
  let doc = isolation_document(
    vec![rectangle(30.0, 30.0, 20.0, 20.0)],
    IsolationParams {
      passes: 1,
      overlap: 0.0,
      direction: MillingDirection::Climb,
      kind: IsolationKind::Full,
      cut_depth: Length::from_mm(0.1),
      keep_pours: false,
    },
  );
  let document_path = write_document(&directory, &doc);
  let post_path = directory.join("post.pp");
  std::fs::write(&post_path, POST).expect("post writes");
  let out_path = directory.join("isolate.nc");

  let output = run(&[
    "isolate",
    document_path.to_str().unwrap(),
    "--diameter",
    "1.0",
    "--post",
    post_path.to_str().unwrap(),
    "--out",
    out_path.to_str().unwrap(),
  ]);
  assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
  let program = std::fs::read_to_string(&out_path).expect("gcode written");

  // Validate emitted output: CLI output must equal emission of the isolation move stream for this
  // document/params through the same post-processor and metadata.
  let copper = cam_ops::unite_copper(
    &[rectangle(30.0, 30.0, 20.0, 20.0)],
    cam_geom::TolerancePolicy::uniform(1.0e-6),
  )
  .expect("unite");
  let moves = cam_ops::isolation_moves(
    &copper,
    1.0,
    1,
    0.0,
    MillingDirection::Climb,
    IsolationKind::Full,
    false,
    0.1,
    cam_geom::TolerancePolicy::uniform(1.0e-6),
    &doc.board,
    &copper,
  )
  .expect("moves");
  let mut stream = MoveStream::new(
    Length::from_mm(cam_model::DEFAULT_SAFE_Z_MM),
    Point2::new(0.0, 0.0),
  );
  stream.segments.push(Segment {
    name: "Top isolation".to_owned(),
    tool_number: 1,
    tool: ToolShape::cylinder(Length::from_mm(1.0)),
    feed_rate: 100.0,
    plunge_rate: 30.0,
    spindle_speed: 18_000,
    moves,
  });
  let post = parse_post(POST).expect("post parses");
  let meta = ProgramMeta {
    file_name: document_path
      .file_name()
      .map(|name| name.to_string_lossy().into_owned())
      .unwrap_or_else(|| document_path.display().to_string()),
    file_path: document_path.display().to_string(),
  };
  let emitted = emit(&post, &stream, &meta).expect("emit");
  assert_eq!(program, emitted.text, "CLI must emit the expected G-code");

  // Acceptance by cam-sim (E3.6): the emitted isolation move stream simulates without collision/crash.
  let mut field = cam_sim::Heightfield::new(&doc.board, Length::from_mm(0.5)).expect("field");
  let removal = cam_sim::sweep::apply(&stream, &mut field);
  assert!(
    removal.crashes.is_empty(),
    "isolation programme must simulate without collision: {:?}",
    removal.crashes
  );
}

#[test]
fn isolate_wires_vbit_depth_derivation() {
  // V-bit 30°, tip 0.2 mm, tool width 1.0 mm => depth ~1.4928 mm (E3.3/S8 wiring via CLI flags).
  let directory = scratch("vbit");
  let doc = isolation_document(
    vec![rectangle(30.0, 30.0, 20.0, 20.0)],
    IsolationParams {
      passes: 1,
      overlap: 0.5,
      direction: MillingDirection::Climb,
      kind: IsolationKind::Full,
      // cut_depth in params would be different, but CLI vbit flags must derive and use vbit depth.
      cut_depth: Length::from_mm(0.1),
      keep_pours: false,
    },
  );
  let path = write_document(&directory, &doc);
  let post = directory.join("post.pp");
  std::fs::write(&post, POST).expect("post writes");
  let out_path = directory.join("vbit.nc");

  let output = run(&[
    "isolate",
    path.to_str().unwrap(),
    "--diameter",
    "1.0",
    "--post",
    post.to_str().unwrap(),
    "--out",
    out_path.to_str().unwrap(),
    "--vbit-angle",
    "30",
    "--vbit-tip",
    "0.2",
  ]);
  assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
  let program = std::fs::read_to_string(&out_path).expect("gcode written");
  // Derived depth is tan(75°)*(1.0-0.2)/2 ≈ 1.4928 mm, so cutting moves reach about Z-1.4928.
  assert!(
    program.contains("Z-1.4928") || program.contains("Z-1.492"),
    "V-bit derived depth must be used in emitted G-code: {program}"
  );
}

#[test]
fn isolate_requires_diameter() {
  let directory = scratch("nodiam");
  let doc = isolation_document(
    vec![rectangle(30.0, 30.0, 20.0, 20.0)],
    IsolationParams {
      passes: 1,
      overlap: 0.0,
      direction: MillingDirection::Climb,
      kind: IsolationKind::Full,
      cut_depth: Length::from_mm(0.1),
      keep_pours: false,
    },
  );
  let path = write_document(&directory, &doc);
  let output = run(&["isolate", path.to_str().unwrap()]);
  assert_eq!(output.status.code(), Some(2));
  assert!(
    stderr(&output).contains("--diameter"),
    "{}",
    stderr(&output)
  );
}
