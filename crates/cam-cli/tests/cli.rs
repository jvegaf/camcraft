//! End-to-end tests for the headless pipeline (A8, A9).
//!
//! These run the real binary, so they assert the observable contract: exit code, exact stdout and
//! stderr, and the files left on disk.

use std::path::{Path, PathBuf};
use std::process::Command;

use cam_geom::{Contour, Point2};
use cam_model::{
  Document, Entity, EntityKind, Job, Layer, Operation, ProfileParams, ProfileSide, Toolpath,
};
use cam_store::document::save_document;
use cam_store::tools::{CuttingData, ToolGeometry, ToolLibrary};
use cam_units::{Length, UnitSystem};

const POST: &str = include_str!("../../cam-post/tests/fixtures/post_mm.pp");

fn binary() -> &'static str {
  env!("CARGO_BIN_EXE_cam-cli")
}

fn scratch(name: &str) -> PathBuf {
  let directory = std::env::temp_dir().join(format!("campilot-cli-{}-{name}", std::process::id()));
  let _ = std::fs::remove_dir_all(&directory);
  std::fs::create_dir_all(&directory).expect("scratch directory");
  directory
}

fn rectangle_contour(width: f64, height: f64) -> Contour {
  Contour::from_points(
    &[
      Point2::new(0.0, 0.0),
      Point2::new(width, 0.0),
      Point2::new(width, height),
      Point2::new(0.0, height),
    ],
    true,
  )
  .expect("rectangle")
}

fn write_document(directory: &Path, contour: Contour, tools_ref: Option<&str>) -> PathBuf {
  let mut document = Document::new(Job {
    width: Length::from_mm(200.0),
    height: Length::from_mm(100.0),
    thickness: Length::from_mm(18.0),
    units: UnitSystem::Mm,
    origin: cam_model::OriginCorner::BottomLeft,
    safe_z: Length::from_mm(5.0),
  });
  document.layers.push(Layer {
    id: "L1".to_owned(),
    name: "Part".to_owned(),
    visible: true,
    entities: vec![Entity {
      id: "E1".to_owned(),
      kind: EntityKind::Contour { contour },
    }],
  });
  document.toolpaths.push(Toolpath {
    id: "TP1".to_owned(),
    name: "Outside profile".to_owned(),
    entity_ids: vec!["E1".to_owned()],
    tool_number: 1,
    operation: Operation::Profile(ProfileParams {
      side: ProfileSide::Outside,
      allowance: Length::ZERO,
      pass_depth: Length::from_mm(6.0),
      start_depth: Length::ZERO,
      cut_depth: Length::from_mm(6.0),
      lead_in: Length::ZERO,
      lead_out: Length::ZERO,
      tabs: Vec::new(),
    }),
  });
  document.tools_ref = tools_ref.map(str::to_owned);

  let path = directory.join("part.c2d");
  save_document(&path, &document).expect("document saved");
  path
}

fn write_library(directory: &Path) -> PathBuf {
  let path = directory.join("tools.sqlite");
  let library = ToolLibrary::open(&path).expect("library");
  library
    .put_geometry(&ToolGeometry {
      id: "G1".to_owned(),
      name_format: "{Tool Type} ({Diameter|F}{Units Short})".to_owned(),
      tool_type: 0,
      units: UnitSystem::Mm,
      diameter: Some(Length::from_mm(6.0)),
      included_angle: None,
      flat_diameter: None,
      num_flutes: Some(2),
      flute_length: None,
      tip_radius: None,
      custom_attributes: None,
    })
    .expect("geometry");
  library
    .put_cutting_data(&CuttingData {
      id: "C1".to_owned(),
      rate_units: UnitSystem::Mm,
      feed_rate: Some(100.0),
      plunge_rate: Some(30.0),
      spindle_speed: Some(18_000),
      stepdown: Some(Length::from_mm(6.0)),
      stepover: None,
      clear_stepover: None,
      tool_number: Some(1),
      notes: None,
    })
    .expect("cutting data");
  library.link("E1", "G1", "C1", None, None).expect("link");
  path
}

fn write_post(directory: &Path) -> PathBuf {
  let path = directory.join("post.pp");
  std::fs::write(&path, POST).expect("post written");
  path
}

struct Outcome {
  code: i32,
  stdout: String,
  stderr: String,
}

fn run(arguments: &[&str]) -> Outcome {
  let output = Command::new(binary())
    .args(arguments)
    .output()
    .expect("the binary runs");
  Outcome {
    code: output.status.code().unwrap_or(-1),
    stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
    stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
  }
}

#[test]
fn validate_reports_counts_and_succeeds() {
  let directory = scratch("validate");
  let document = write_document(&directory, rectangle_contour(50.0, 25.0), None);
  let outcome = run(&["validate", document.to_str().expect("utf8")]);
  assert_eq!(outcome.code, 0, "{}", outcome.stderr);
  assert_eq!(
    outcome.stdout,
    "valid: 1 layer(s), 1 entity(ies), 1 toolpath(s)\n"
  );
  assert!(outcome.stderr.is_empty(), "{}", outcome.stderr);
}

#[test]
fn an_unsupported_schema_exits_three_and_names_both_versions() {
  let directory = scratch("schema");
  let document = write_document(&directory, rectangle_contour(50.0, 25.0), None);
  let mut loaded = cam_store::document::load_document(&document).expect("loads");
  loaded.schema = "c2d/document@42".to_owned();
  // Save bypasses validation only through the raw file writer, so rewrite the JSON directly.
  let bytes = std::fs::read(&document).expect("read");
  let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("zip");
  let mut payload = String::new();
  std::io::Read::read_to_string(
    &mut archive.by_name("document.json").expect("entry"),
    &mut payload,
  )
  .expect("utf8");
  let broken = payload.replace("c2d/document@1", "c2d/document@42");
  let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
  writer
    .start_file("document.json", zip::write::SimpleFileOptions::default())
    .expect("start");
  std::io::Write::write_all(&mut writer, broken.as_bytes()).expect("write");
  let raw = writer.finish().expect("finish").into_inner();
  std::fs::write(&document, raw).expect("write document");

  let outcome = run(&["validate", document.to_str().expect("utf8")]);
  assert_eq!(outcome.code, 3, "{}", outcome.stderr);
  assert!(
    outcome.stderr.contains("c2d/document@42"),
    "{}",
    outcome.stderr
  );
  assert!(
    outcome.stderr.contains("c2d/document@1"),
    "{}",
    outcome.stderr
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_missing_document_exits_two() {
  let outcome = run(&["validate", "/nonexistent/part.c2d"]);
  assert_eq!(outcome.code, 2);
  assert!(outcome.stderr.contains("error:"), "{}", outcome.stderr);
}

#[test]
fn inspect_prints_a_stable_summary() {
  let directory = scratch("inspect");
  let document = write_document(&directory, rectangle_contour(50.0, 25.0), None);
  let outcome = run(&["inspect", document.to_str().expect("utf8")]);
  assert_eq!(outcome.code, 0, "{}", outcome.stderr);
  let expected = "\
schema: c2d/document@1
job: 200.000 x 100.000 x 18.000 mm, origin BottomLeft, safe Z 5.000 mm
layers: 1
  L1 \"Part\": 1 entity(ies), visible true
toolpaths: 1
  TP1 \"Outside profile\": profile, tool T1, 1 entity(ies)
geometry length: 150.000 mm
";
  assert_eq!(outcome.stdout, expected);
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn postp_validate_summarises_and_rejects_with_line_and_column() {
  let directory = scratch("postp");
  let good = write_post(&directory);
  let outcome = run(&["postp", "validate", good.to_str().expect("utf8")]);
  assert_eq!(outcome.code, 0, "{}", outcome.stderr);
  assert!(outcome.stdout.contains("blocks: 16"), "{}", outcome.stdout);

  let bad = directory.join("bad.pp");
  std::fs::write(&bad, "POST_NAME = \"x\"\nNOT_A_DIRECTIVE = \"y\"\n").expect("write");
  let outcome = run(&["postp", "validate", bad.to_str().expect("utf8")]);
  assert_eq!(outcome.code, 4, "{}", outcome.stderr);
  assert!(outcome.stderr.contains("line 2:1"), "{}", outcome.stderr);
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn emit_produces_the_whole_program_end_to_end() {
  let directory = scratch("emit");
  let document = write_document(&directory, rectangle_contour(50.0, 25.0), None);
  let library = write_library(&directory);
  let post = write_post(&directory);
  let out = directory.join("part.nc");

  let outcome = run(&[
    "emit",
    document.to_str().expect("utf8"),
    "--post",
    post.to_str().expect("utf8"),
    "--library",
    library.to_str().expect("utf8"),
    "--out",
    out.to_str().expect("utf8"),
  ]);
  assert_eq!(outcome.code, 0, "{}", outcome.stderr);
  assert!(outcome.stdout.is_empty(), "{}", outcome.stdout);
  assert!(outcome.stderr.contains("wrote"), "{}", outcome.stderr);

  let program = std::fs::read_to_string(&out).expect("program written");
  let lines: Vec<&str> = program
    .split("\r\n")
    .filter(|line| !line.is_empty())
    .collect();
  // A 6 mm tool cutting a 50x25 rectangle outside: the tool centre runs a 56x31 path, 6 mm deep,
  // in one pass, with the tool radius offset on every side. The path starts at the offset corner
  // nearest the authored start vertex, so the lead-in and the tab positions keep their meaning.
  assert_eq!(
    lines,
    vec![
      "( Outside profile )",
      "( CamPilot fixture Mach3 Arcs (mm) (*.nc) )",
      "N0G00G17G90G21",
      "N10T1M06",
      "N20S18000M03",
      "N30G00X-3.0000Y-3.0000",
      "N40G01Z-6.0000F100.0",
      "N50G01X53.0000",
      "N60G01Y28.0000",
      "N70G01X-3.0000",
      "N80G01Y-3.0000",
      "N90G00Z5.0000",
      "N100G00Z5.0000",
      "N110G00X0.0000Y0.0000",
      "N120M05",
      "N130M30",
    ]
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn emit_is_byte_identical_across_runs() {
  let directory = scratch("determinism");
  let document = write_document(&directory, rectangle_contour(50.0, 25.0), None);
  let library = write_library(&directory);
  let post = write_post(&directory);
  let arguments = [
    "emit",
    document.to_str().expect("utf8"),
    "--post",
    post.to_str().expect("utf8"),
    "--library",
    library.to_str().expect("utf8"),
  ];
  let first = run(&arguments);
  let second = run(&arguments);
  assert_eq!(first.code, 0, "{}", first.stderr);
  assert_eq!(first.stdout, second.stdout);
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn emit_refuses_a_document_it_cannot_cut_and_writes_nothing() {
  let directory = scratch("refuse");
  let document = write_document(&directory, rectangle_contour(50.0, 25.0), None);

  // A tool too big for the part: 60 mm diameter inside a 50x25 rectangle.
  let library = ToolLibrary::open(directory.join("tools.sqlite")).expect("library");
  library
    .put_geometry(&ToolGeometry {
      id: "BIG".to_owned(),
      name_format: "{Tool Name}".to_owned(),
      tool_type: 0,
      units: UnitSystem::Mm,
      diameter: Some(Length::from_mm(60.0)),
      included_angle: None,
      flat_diameter: None,
      num_flutes: Some(2),
      flute_length: None,
      tip_radius: None,
      custom_attributes: None,
    })
    .expect("geometry");
  library
    .put_cutting_data(&CuttingData {
      id: "BIGCUT".to_owned(),
      rate_units: UnitSystem::Mm,
      feed_rate: Some(100.0),
      plunge_rate: Some(30.0),
      spindle_speed: Some(18_000),
      stepdown: Some(Length::from_mm(6.0)),
      stepover: None,
      clear_stepover: None,
      tool_number: Some(9),
      notes: None,
    })
    .expect("data");
  library
    .link("BIGLINK", "BIG", "BIGCUT", None, None)
    .expect("link");

  let mut loaded = cam_store::document::load_document(&document).expect("loads");
  loaded.toolpaths[0].tool_number = 9;
  loaded.toolpaths[0].operation = Operation::Profile(ProfileParams {
    side: ProfileSide::Inside,
    ..ProfileParams::default()
  });
  save_document(&document, &loaded).expect("saved");

  let post = write_post(&directory);
  let out = directory.join("part.nc");
  let outcome = run(&[
    "emit",
    document.to_str().expect("utf8"),
    "--post",
    post.to_str().expect("utf8"),
    "--library",
    directory.join("tools.sqlite").to_str().expect("utf8"),
    "--out",
    out.to_str().expect("utf8"),
  ]);
  assert_eq!(outcome.code, 2, "{}", outcome.stderr);
  assert!(
    outcome.stderr.contains("does not fit"),
    "{}",
    outcome.stderr
  );
  assert!(
    !out.exists(),
    "no program may be written when the cut is refused"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn emit_requires_tool_data_and_says_so() {
  let directory = scratch("no-tool");
  let document = write_document(&directory, rectangle_contour(50.0, 25.0), None);
  let post = write_post(&directory);
  let outcome = run(&[
    "emit",
    document.to_str().expect("utf8"),
    "--post",
    post.to_str().expect("utf8"),
  ]);
  assert_eq!(outcome.code, 2);
  assert!(
    outcome.stderr.contains("tool diameter"),
    "{}",
    outcome.stderr
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn help_lists_every_command_option_and_exit_code() {
  let outcome = run(&["--help"]);
  assert_eq!(outcome.code, 0);
  for needle in [
    "validate",
    "inspect",
    "postp validate",
    "tools list",
    "tools add",
    "emit",
    "--post",
    "--out",
    "--library",
    "EXIT CODES",
    "unsupported document schema",
    "post-processor error",
  ] {
    assert!(
      outcome.stdout.contains(needle),
      "help is missing {needle:?}"
    );
  }
}

#[test]
fn tools_add_then_list_round_trips() {
  let directory = scratch("tools");
  let library = directory.join("tools.sqlite");
  let added = run(&[
    "tools",
    "add",
    library.to_str().expect("utf8"),
    "--id",
    "G1",
    "--diameter",
    "6.35",
    "--number",
    "3",
  ]);
  assert_eq!(added.code, 0, "{}", added.stderr);

  let listed = run(&["tools", "list", library.to_str().expect("utf8")]);
  assert_eq!(listed.code, 0, "{}", listed.stderr);
  assert!(listed.stdout.contains("T3"), "{}", listed.stdout);
  assert!(listed.stdout.contains("6.350 mm"), "{}", listed.stdout);
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn an_unknown_command_is_invalid_input() {
  let outcome = run(&["frobnicate"]);
  assert_eq!(outcome.code, 2);
  assert!(
    outcome.stderr.contains("unknown command"),
    "{}",
    outcome.stderr
  );
}
