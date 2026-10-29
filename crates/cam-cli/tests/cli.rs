//! End-to-end tests for the headless pipeline (A8, A9).
//!
//! These run the real binary, so they assert the observable contract: exit code, exact stdout and
//! stderr, and the files left on disk.

use std::f64::consts::{FRAC_PI_2, PI};
use std::path::{Path, PathBuf};
use std::process::Command;

use cam_geom::{Arc, Contour, LineSeg, Point2, Segment};
use cam_model::{
  Document, DrillParams, Entity, EntityKind, Job, Layer, Operation, PocketParams, PocketStrategy,
  ProfileParams, ProfileSide, RetractMode, Toolpath,
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

fn circle_contour(radius: f64) -> Contour {
  cam_geom::primitive::circle(Point2::new(25.0, 15.0), radius).expect("circle")
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
    island_ids: Vec::new(),
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
  let broken = payload.replace("c2d/document@2", "c2d/document@42");
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
    outcome.stderr.contains("c2d/document@2"),
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
schema: c2d/document@2
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

#[test]
fn import_refuses_an_extension_it_cannot_read() {
  let directory = scratch("import-unsupported");
  let input = directory.join("part.dwg");
  std::fs::write(&input, b"not a vector").expect("input written");
  let outcome = run(&["import", input.to_str().expect("path")]);
  assert_eq!(outcome.code, 2);
  assert_eq!(outcome.stdout, "");
  assert!(
    outcome.stderr.contains("unsupported format dwg"),
    "stderr: {}",
    outcome.stderr
  );
  assert!(
    outcome.stderr.contains("dxf, svg, pdf, eps, png"),
    "stderr: {}",
    outcome.stderr
  );
}

#[test]
fn import_without_an_input_exits_2() {
  let outcome = run(&["import"]);
  assert_eq!(outcome.code, 2);
  assert!(
    outcome.stderr.contains("import requires an input file"),
    "stderr: {}",
    outcome.stderr
  );
}

#[test]
fn import_refuses_a_non_positive_tolerance() {
  let directory = scratch("import-tolerance");
  let input = directory.join("part.dwg");
  std::fs::write(&input, b"x").expect("input written");
  let outcome = run(&["import", input.to_str().expect("path"), "--tolerance", "0"]);
  assert_eq!(outcome.code, 2);
  assert!(
    outcome
      .stderr
      .contains("--tolerance must be a positive length in mm"),
    "stderr: {}",
    outcome.stderr
  );
}

#[test]
fn import_refuses_an_unknown_option() {
  let directory = scratch("import-option");
  let input = directory.join("part.dwg");
  std::fs::write(&input, b"x").expect("input written");
  let outcome = run(&["import", input.to_str().expect("path"), "--nope"]);
  assert_eq!(outcome.code, 2);
  assert!(
    outcome.stderr.contains("unknown import option"),
    "stderr: {}",
    outcome.stderr
  );
}

#[test]
fn the_help_lists_the_import_command_and_its_trace_options() {
  let outcome = run(&["--help"]);
  assert_eq!(outcome.code, 0);
  for expected in [
    "import <input>",
    "--tolerance <mm>",
    "--threshold <0-255>",
    "--min-area <mm2>",
    "--simplify <mm>",
  ] {
    assert!(
      outcome.stdout.contains(expected),
      "help is missing {expected}:\n{}",
      outcome.stdout
    );
  }
}

/// A document with the given contours as `E1..En` on one layer, and no toolpaths.
fn write_vectors(directory: &Path, name: &str, contours: Vec<Contour>) -> PathBuf {
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
    name: "Vectors".to_owned(),
    visible: true,
    entities: contours
      .into_iter()
      .enumerate()
      .map(|(index, contour)| Entity {
        id: format!("E{}", index + 1),
        kind: EntityKind::Contour { contour },
      })
      .collect(),
  });
  let path = directory.join(name);
  save_document(&path, &document).expect("document saved");
  path
}

fn figure_eight_contour() -> Contour {
  Contour::from_points(
    &[
      Point2::new(0.0, 0.0),
      Point2::new(10.0, 10.0),
      Point2::new(10.0, 0.0),
      Point2::new(0.0, 10.0),
    ],
    true,
  )
  .expect("figure eight")
}

#[test]
fn check_reports_a_self_intersection_with_its_point_and_segments() {
  let directory = scratch("check-intersection");
  let document = write_vectors(&directory, "eight.c2d", vec![figure_eight_contour()]);
  let outcome = run(&["check", document.to_str().expect("utf8")]);
  assert_eq!(outcome.code, 5, "{}", outcome.stderr);
  assert_eq!(
    outcome.stdout,
    "E1 self-intersection at 5.000, 5.000 (segments 0, 2)\n"
  );
  assert!(
    outcome.stderr.contains("1 vector issue(s) found"),
    "{}",
    outcome.stderr
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn check_reports_an_open_contour_with_its_gap() {
  let directory = scratch("check-open");
  let open = Contour::from_points(
    &[
      Point2::new(0.0, 0.0),
      Point2::new(10.0, 0.0),
      Point2::new(10.0, 4.0),
    ],
    false,
  )
  .expect("open contour");
  let document = write_vectors(&directory, "open.c2d", vec![open]);
  let outcome = run(&["check", document.to_str().expect("utf8")]);
  assert_eq!(outcome.code, 5, "{}", outcome.stderr);
  assert_eq!(
    outcome.stdout,
    format!("E1 open: gap {:.4} mm\n", 10.0_f64.hypot(4.0))
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn check_accepts_a_clean_document_and_says_so() {
  let directory = scratch("check-clean");
  let document = write_vectors(
    &directory,
    "clean.c2d",
    vec![
      rectangle_contour(50.0, 25.0),
      cam_geom::primitive::circle(Point2::new(100.0, 50.0), 8.0).expect("circle"),
    ],
  );
  let outcome = run(&["check", document.to_str().expect("utf8")]);
  assert_eq!(outcome.code, 0, "{}", outcome.stderr);
  assert_eq!(outcome.stdout, "all vectors valid\n");
  assert!(outcome.stderr.is_empty(), "{}", outcome.stderr);
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn check_orders_findings_by_entity_id() {
  let directory = scratch("check-order");
  let open = Contour::from_points(
    &[
      Point2::new(0.0, 0.0),
      Point2::new(10.0, 0.0),
      Point2::new(10.0, 4.0),
    ],
    false,
  )
  .expect("open contour");
  let document = write_vectors(&directory, "order.c2d", vec![open, figure_eight_contour()]);
  let outcome = run(&["check", document.to_str().expect("utf8")]);
  assert_eq!(outcome.code, 5, "{}", outcome.stderr);
  let lines: Vec<&str> = outcome.stdout.lines().collect();
  assert_eq!(lines.len(), 2, "{}", outcome.stdout);
  assert!(lines[0].starts_with("E1 open: gap"), "{}", outcome.stdout);
  assert!(
    lines[1].starts_with("E2 self-intersection"),
    "{}",
    outcome.stdout
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn check_still_exits_two_for_a_missing_document() {
  let outcome = run(&["check", "/nonexistent/part.c2d"]);
  assert_eq!(outcome.code, 2);
  assert!(outcome.stderr.contains("error:"), "{}", outcome.stderr);
}

// --- M4 T1: the `toolpath` command and the new operation kinds (A10, A11) ---

fn write_library_with_pocket_rates(directory: &Path) -> PathBuf {
  let path = directory.join("rates.sqlite");
  let library = ToolLibrary::open(&path).expect("library");
  library
    .put_geometry(&ToolGeometry {
      id: "G2".to_owned(),
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
      id: "C2".to_owned(),
      rate_units: UnitSystem::Mm,
      feed_rate: Some(100.0),
      plunge_rate: Some(30.0),
      spindle_speed: Some(18_000),
      stepdown: Some(Length::from_mm(5.0)),
      stepover: Some(Length::from_mm(2.4)),
      clear_stepover: Some(Length::from_mm(4.8)),
      tool_number: Some(2),
      notes: None,
    })
    .expect("cutting data");
  library.link("E2", "G2", "C2", None, None).expect("link");
  path
}

#[test]
fn toolpath_appends_a_profile_and_the_document_reloads_with_two() {
  let directory = scratch("toolpath-profile");
  let document = write_document(&directory, rectangle_contour(50.0, 25.0), None);
  let outcome = run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "profile",
    "--entities",
    "E1",
    "--tool",
    "1",
    "--side",
    "inside",
    "--cut-depth",
    "9",
    "--pass-depth",
    "3",
  ]);
  assert_eq!(outcome.code, 0, "{}", outcome.stderr);
  assert_eq!(
    outcome.stdout, "added TP2 profile, tool T1, 1 entity(ies)\n",
    "{}",
    outcome.stdout
  );
  let loaded = cam_store::document::load_document(&document).expect("loads");
  assert_eq!(loaded.toolpaths.len(), 2);
  match &loaded.toolpaths[1].operation {
    Operation::Profile(params) => {
      assert_eq!(params.side, ProfileSide::Inside);
      assert_eq!(params.cut_depth, Length::from_mm(9.0));
      assert_eq!(params.pass_depth, Length::from_mm(3.0));
    }
    other => panic!("expected a profile, got {other:?}"),
  }
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_second_toolpath_gets_the_next_free_id() {
  let directory = scratch("toolpath-ids");
  let document = write_document(&directory, rectangle_contour(50.0, 25.0), None);
  let outcome = run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "profile",
    "--entities",
    "E1",
    "--tool",
    "1",
    "--pass-depth",
    "6",
  ]);
  assert_eq!(outcome.code, 0, "{}", outcome.stderr);
  assert!(outcome.stdout.contains("added TP2"), "{}", outcome.stdout);
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn an_unknown_toolpath_kind_exits_two_and_leaves_the_document_untouched() {
  let directory = scratch("toolpath-kind");
  let document = write_document(&directory, rectangle_contour(50.0, 25.0), None);
  let before = std::fs::read(&document).expect("read");
  let outcome = run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "laser_cut",
    "--entities",
    "E1",
    "--tool",
    "1",
  ]);
  assert_eq!(outcome.code, 2);
  assert!(
    outcome.stderr.contains("laser_cut"),
    "the refusal names the kind: {}",
    outcome.stderr
  );
  assert_eq!(std::fs::read(&document).expect("read"), before);
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn toolpath_naming_a_missing_entity_exits_two_and_leaves_the_document_untouched() {
  let directory = scratch("toolpath-entity");
  let document = write_document(&directory, rectangle_contour(50.0, 25.0), None);
  let before = std::fs::read(&document).expect("read");
  let outcome = run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "pocket",
    "--entities",
    "E9",
    "--tool",
    "1",
    "--stepover",
    "3",
    "--pass-depth",
    "6",
  ]);
  assert_eq!(outcome.code, 2);
  assert!(outcome.stderr.contains("E9"), "{}", outcome.stderr);
  assert_eq!(std::fs::read(&document).expect("read"), before);
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_pocket_takes_its_stepover_and_pass_depth_from_the_tool_library() {
  let directory = scratch("toolpath-defaults");
  let library = write_library_with_pocket_rates(&directory);
  let document = write_document(
    &directory,
    rectangle_contour(50.0, 25.0),
    Some(library.to_str().expect("utf8")),
  );
  let outcome = run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "pocket",
    "--entities",
    "E1",
    "--tool",
    "2",
    "--cut-depth",
    "5",
  ]);
  assert_eq!(outcome.code, 0, "{}", outcome.stderr);
  let loaded = cam_store::document::load_document(&document).expect("loads");
  match &loaded.toolpaths[1].operation {
    Operation::Pocket(params) => {
      assert_eq!(
        params.stepover,
        Length::from_mm(2.4),
        "the tool's stepover is the default"
      );
      assert_eq!(
        params.pass_depth,
        Length::from_mm(5.0),
        "the tool's stepdown is the default"
      );
    }
    other => panic!("expected a pocket, got {other:?}"),
  }
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_clearing_pocket_takes_its_stepover_from_clear_stepover() {
  let directory = scratch("toolpath-clear");
  let library = write_library_with_pocket_rates(&directory);
  let document = write_document(
    &directory,
    rectangle_contour(50.0, 25.0),
    Some(library.to_str().expect("utf8")),
  );
  let outcome = run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "pocket",
    "--entities",
    "E1",
    "--tool",
    "2",
    "--clear",
    "--cut-depth",
    "5",
    "--pass-depth",
    "5",
  ]);
  assert_eq!(outcome.code, 0, "{}", outcome.stderr);
  let loaded = cam_store::document::load_document(&document).expect("loads");
  match &loaded.toolpaths[1].operation {
    Operation::Pocket(params) => {
      assert_eq!(
        params.stepover,
        Length::from_mm(4.8),
        "a clearing pocket uses clear_stepover"
      );
    }
    other => panic!("expected a pocket, got {other:?}"),
  }
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_pocket_without_a_stepover_anywhere_exits_two_naming_the_option() {
  let directory = scratch("toolpath-nostepover");
  let document = write_document(&directory, rectangle_contour(50.0, 25.0), None);
  let outcome = run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "pocket",
    "--entities",
    "E1",
    "--tool",
    "1",
  ]);
  assert_eq!(outcome.code, 2);
  assert!(
    outcome.stderr.contains("--stepover"),
    "the refusal names the missing input: {}",
    outcome.stderr
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_drilling_toolpath_accepts_a_bank_and_an_out_path() {
  let directory = scratch("toolpath-bank");
  let document = write_document(&directory, circle_contour(5.0), None);
  let out = directory.join("drilled.c2d");
  let outcome = run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--out",
    out.to_str().expect("utf8"),
    "--kind",
    "drilling",
    "--entities",
    "E1",
    "--tool",
    "3",
    "--peck-depth",
    "2",
    "--retract",
    "amount",
    "--retract-amount",
    "1",
    "--bank",
    "3:32",
  ]);
  assert_eq!(outcome.code, 0, "{}", outcome.stderr);
  let loaded = cam_store::document::load_document(&out).expect("loads");
  assert_eq!(
    loaded.toolpaths.len(),
    2,
    "the source document still has its profile"
  );
  match &loaded.toolpaths[1].operation {
    Operation::Drilling(params) => {
      assert_eq!(params.peck_depth, Length::from_mm(2.0));
      assert_eq!(
        params.retract,
        cam_model::RetractMode::Amount {
          amount: Length::from_mm(1.0)
        }
      );
      assert_eq!(
        params.bank,
        Some(cam_model::DrillBank {
          count: 3,
          pitch: Length::from_mm(32.0)
        })
      );
    }
    other => panic!("expected drilling, got {other:?}"),
  }
  let original = cam_store::document::load_document(&document).expect("loads");
  assert_eq!(original.toolpaths.len(), 1, "--out leaves the input alone");
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn inspect_names_each_toolpath_kind_and_its_strategy() {
  let directory = scratch("inspect-kinds");
  let document = write_document(&directory, rectangle_contour(50.0, 25.0), None);
  let mut loaded = cam_store::document::load_document(&document).expect("loads");
  let offset = cam_model::PocketParams {
    strategy: cam_model::PocketStrategy::Offset,
    ..cam_model::PocketParams::default()
  };
  let raster = cam_model::PocketParams {
    strategy: cam_model::PocketStrategy::Raster,
    ..cam_model::PocketParams::default()
  };
  let banked = cam_model::DrillParams {
    bank: Some(cam_model::DrillBank {
      count: 2,
      pitch: Length::from_mm(32.0),
    }),
    ..cam_model::DrillParams::default()
  };
  loaded.toolpaths.push(Toolpath {
    id: "TP2".to_owned(),
    name: "Pocket offset".to_owned(),
    entity_ids: vec!["E1".to_owned()],
    island_ids: Vec::new(),
    tool_number: 1,
    operation: Operation::Pocket(offset),
  });
  loaded.toolpaths.push(Toolpath {
    id: "TP3".to_owned(),
    name: "Pocket raster".to_owned(),
    entity_ids: vec!["E1".to_owned()],
    island_ids: Vec::new(),
    tool_number: 1,
    operation: Operation::Pocket(raster),
  });
  loaded.toolpaths.push(Toolpath {
    id: "TP4".to_owned(),
    name: "Drill".to_owned(),
    entity_ids: vec!["E1".to_owned()],
    island_ids: Vec::new(),
    tool_number: 1,
    operation: Operation::Drilling(cam_model::DrillParams::default()),
  });
  loaded.toolpaths.push(Toolpath {
    id: "TP5".to_owned(),
    name: "Drill bank".to_owned(),
    entity_ids: vec!["E1".to_owned()],
    island_ids: Vec::new(),
    tool_number: 1,
    operation: Operation::Drilling(banked),
  });
  save_document(&document, &loaded).expect("saved");

  let outcome = run(&["inspect", document.to_str().expect("utf8")]);
  assert_eq!(outcome.code, 0, "{}", outcome.stderr);
  let stdout = &outcome.stdout;
  assert!(stdout.contains("profile, tool T1"), "{stdout}");
  assert!(stdout.contains("pocket (offset), tool T1"), "{stdout}");
  assert!(stdout.contains("pocket (raster), tool T1"), "{stdout}");
  assert!(stdout.contains("drilling, tool T1"), "{stdout}");
  assert!(stdout.contains("drilling (bank), tool T1"), "{stdout}");
  std::fs::remove_dir_all(&directory).ok();
}

// --- M4 T5: golden G-code for the pocket and drilling strategies (A12, A14) ---

/// A counter-clockwise rounded rectangle: four lines joined by four quarter arcs.
///
/// The golden pocket cuts this rather than a plain rectangle so the program carries both straight
/// feed moves and arcs (S8): offsetting a rounded rectangle keeps its arcs as arcs.
fn rounded_rectangle(width: f64, height: f64, radius: f64) -> Contour {
  let (w, h, r) = (width, height, radius);
  let arc = |center: Point2, start: f64| {
    Segment::Arc(Arc::new(center, r, start, FRAC_PI_2).expect("quarter arc"))
  };
  let segments = vec![
    Segment::Line(LineSeg::new(Point2::new(r, 0.0), Point2::new(w - r, 0.0))),
    arc(Point2::new(w - r, r), -FRAC_PI_2),
    Segment::Line(LineSeg::new(Point2::new(w, r), Point2::new(w, h - r))),
    arc(Point2::new(w - r, h - r), 0.0),
    Segment::Line(LineSeg::new(Point2::new(w - r, h), Point2::new(r, h))),
    arc(Point2::new(r, h - r), FRAC_PI_2),
    Segment::Line(LineSeg::new(Point2::new(0.0, h - r), Point2::new(0.0, r))),
    arc(Point2::new(r, r), PI),
  ];
  Contour::from_segments(&segments, true).expect("rounded rectangle")
}

/// A drilled hole: the same two-half-turn circle `cam_geom::primitive::circle` builds.
fn hole(x: f64, y: f64) -> Contour {
  cam_geom::primitive::circle(Point2::new(x, y), 3.0).expect("hole")
}

/// The golden fixture: the pocket shape and the two holes as `E1..E3`, with no toolpaths.
///
/// One definition, shared by the hand-built document and the one `cam-cli toolpath` authors, so the
/// two paths can only differ in how the toolpaths got there.
fn write_golden_vectors(directory: &Path, name: &str) -> PathBuf {
  write_vectors(
    directory,
    name,
    vec![
      rounded_rectangle(40.0, 30.0, 12.0),
      hole(80.0, 70.0),
      hole(120.0, 30.0),
    ],
  )
}

/// The golden document: one offset pocket on `E1` and one drilling toolpath over `E2` and `E3`.
///
/// Pocket: a 6 mm tool, no allowance, 6 mm stepover and one 6 mm pass, so two rings come out
/// (insets 3 and 9). Drilling: 3 mm pecks to 6 mm with a full retract between them.
fn write_golden_document(directory: &Path, name: &str) -> PathBuf {
  let path = write_golden_vectors(directory, name);
  let mut document = cam_store::document::load_document(&path).expect("loads");
  document.toolpaths.push(Toolpath {
    id: "TP1".to_owned(),
    name: "Pocket offset".to_owned(),
    entity_ids: vec!["E1".to_owned()],
    island_ids: Vec::new(),
    tool_number: 1,
    operation: Operation::Pocket(PocketParams {
      strategy: PocketStrategy::Offset,
      allowance: Length::ZERO,
      stepover: Length::from_mm(6.0),
      pass_depth: Length::from_mm(6.0),
      start_depth: Length::ZERO,
      cut_depth: Length::from_mm(6.0),
    }),
  });
  document.toolpaths.push(Toolpath {
    id: "TP2".to_owned(),
    name: "Drill".to_owned(),
    entity_ids: vec!["E2".to_owned(), "E3".to_owned()],
    island_ids: Vec::new(),
    tool_number: 1,
    operation: Operation::Drilling(DrillParams {
      peck_depth: Length::from_mm(3.0),
      retract: RetractMode::Full,
      start_depth: Length::ZERO,
      cut_depth: Length::from_mm(6.0),
      bank: None,
    }),
  });
  save_document(&path, &document).expect("document saved");
  path
}

/// Run `emit` and return the program it wrote.
fn emit_program(directory: &Path, document: &Path, library: &Path, post: &Path) -> String {
  let out = directory.join("program.nc");
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
  std::fs::read_to_string(&out).expect("program written")
}

/// The whole program the golden document emits, byte for byte.
///
/// Every line ends with CRLF because the fixture post-processor declares `LINE_ENDING = "[13][10]"`.
/// The program is 42 lines: a header, the two offset rings of the pocket, and the two drilled holes.
const GOLDEN_PROGRAM: &str = "\
( Pocket offset )\r\n\
( CamPilot fixture Mach3 Arcs (mm) (*.nc) )\r\n\
N0G00G17G90G21\r\n\
N10T1M06\r\n\
N20S18000M03\r\n\
N30G00X12.0000Y3.0000\r\n\
N40G01Z-6.0000F100.0\r\n\
N50G01X28.0000\r\n\
N60G03X37.0000Y12.0000I0.0000J9.0000\r\n\
N70G01Y18.0000\r\n\
N80G03X28.0000Y27.0000I-9.0000J0.0000\r\n\
N90G01X12.0000\r\n\
N100G03X3.0000Y18.0000I0.0000J-9.0000\r\n\
N110G01Y12.0000\r\n\
N120G03X12.0000Y3.0000I9.0000J0.0000\r\n\
N130G00Z5.0000\r\n\
N140G00Y9.0000\r\n\
N150G01Z-6.0000\r\n\
N160G01X28.0000\r\n\
N170G03X31.0000Y12.0000I0.0000J3.0000\r\n\
N180G01Y18.0000\r\n\
N190G03X28.0000Y21.0000I-3.0000J0.0000\r\n\
N200G01X12.0000\r\n\
N210G03X9.0000Y18.0000I0.0000J-3.0000\r\n\
N220G01Y12.0000\r\n\
N230G03X12.0000Y9.0000I3.0000J0.0000\r\n\
N240G00Z5.0000\r\n\
N250G00\r\n\
N260G00X120.0000Y30.0000\r\n\
N270G01Z-3.0000\r\n\
N280G00Z5.0000\r\n\
N290G01Z-6.0000\r\n\
N300G00Z5.0000\r\n\
N310G00X80.0000Y70.0000\r\n\
N320G01Z-3.0000\r\n\
N330G00Z5.0000\r\n\
N340G01Z-6.0000\r\n\
N350G00Z5.0000\r\n\
N360G00Z5.0000\r\n\
N370G00X0.0000Y0.0000\r\n\
N380M05\r\n\
N390M30\r\n";

#[test]
fn pocket_and_drilling_emit_the_golden_program() {
  let directory = scratch("golden-pocket-drilling");
  let document = write_golden_document(&directory, "golden.c2d");
  let library = write_library(&directory);
  let post = write_post(&directory);

  let program = emit_program(&directory, &document, &library, &post);
  assert_eq!(program, GOLDEN_PROGRAM);
  // A12: the same input emits the same bytes on the next run too.
  assert_eq!(
    emit_program(&directory, &document, &library, &post),
    program
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn the_golden_program_survives_a_document_save_and_load_round_trip() {
  let directory = scratch("golden-round-trip");
  let document = write_golden_document(&directory, "golden.c2d");
  let library = write_library(&directory);
  let post = write_post(&directory);

  // Save what was loaded again, so the program comes from a document that made the c2d round trip.
  let reloaded = directory.join("reloaded.c2d");
  let loaded = cam_store::document::load_document(&document).expect("loads");
  save_document(&reloaded, &loaded).expect("saved");

  assert_eq!(
    emit_program(&directory, &reloaded, &library, &post),
    GOLDEN_PROGRAM
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_pocket_and_a_drilling_toolpath_authored_through_the_cli_emit_the_golden_program() {
  let directory = scratch("golden-authored");
  // The same fixture the hand-built document starts from: geometry only, no toolpaths.
  let document = write_golden_vectors(&directory, "authored.c2d");
  let library = write_library(&directory);
  let post = write_post(&directory);

  let pocket = run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "pocket",
    "--entities",
    "E1",
    "--tool",
    "1",
    "--name",
    "Pocket offset",
    "--strategy",
    "offset",
    "--stepover",
    "6",
    "--pass-depth",
    "6",
    "--cut-depth",
    "6",
    "--library",
    library.to_str().expect("utf8"),
  ]);
  assert_eq!(pocket.code, 0, "{}", pocket.stderr);
  assert_eq!(
    pocket.stdout, "added TP1 pocket (offset), tool T1, 1 entity(ies)\n",
    "{}",
    pocket.stdout
  );

  let drilling = run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "drilling",
    "--entities",
    "E2,E3",
    "--tool",
    "1",
    "--name",
    "Drill",
    "--peck-depth",
    "3",
    "--retract",
    "full",
    "--cut-depth",
    "6",
    "--library",
    library.to_str().expect("utf8"),
  ]);
  assert_eq!(drilling.code, 0, "{}", drilling.stderr);
  assert_eq!(
    drilling.stdout, "added TP2 drilling, tool T1, 2 entity(ies)\n",
    "{}",
    drilling.stdout
  );

  assert_eq!(
    emit_program(&directory, &document, &library, &post),
    GOLDEN_PROGRAM
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_drilling_toolpath_over_a_non_circle_exits_two_and_leaves_the_document_untouched() {
  let directory = scratch("toolpath-noncircle");
  let document = write_document(&directory, rectangle_contour(50.0, 25.0), None);
  let before = std::fs::read(&document).expect("read");
  let outcome = run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "drilling",
    "--entities",
    "E1",
    "--tool",
    "1",
    "--cut-depth",
    "6",
  ]);
  assert_eq!(outcome.code, 2, "{}", outcome.stderr);
  assert!(
    outcome.stderr.contains("E1") && outcome.stderr.contains("not a circle"),
    "the refusal names the entity and the reason: {}",
    outcome.stderr
  );
  assert_eq!(
    std::fs::read(&document).expect("read"),
    before,
    "a refused append writes nothing"
  );
  std::fs::remove_dir_all(&directory).ok();
}

// --- P2: islands on the toolpath command (A11) ---

#[test]
fn a_pocket_records_the_islands_it_must_leave_standing() {
  let directory = scratch("toolpath-islands");
  let document = write_document(&directory, rectangle_contour(100.0, 100.0), None);
  let mut loaded = cam_store::document::load_document(&document).expect("loads");
  loaded.layers[0].entities.push(Entity {
    id: "E2".to_owned(),
    kind: EntityKind::Contour {
      contour: cam_geom::primitive::circle(Point2::new(50.0, 50.0), 10.0).expect("circle"),
    },
  });
  save_document(&document, &loaded).expect("saved");

  let outcome = run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "pocket",
    "--entities",
    "E1",
    "--islands",
    "E2",
    "--tool",
    "1",
    "--stepover",
    "3",
    "--pass-depth",
    "6",
    "--cut-depth",
    "6",
  ]);
  assert_eq!(outcome.code, 0, "{}", outcome.stderr);
  let reloaded = cam_store::document::load_document(&document).expect("loads");
  assert_eq!(reloaded.toolpaths[1].island_ids, vec!["E2".to_owned()]);
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn an_island_outside_the_pocket_exits_two_and_leaves_the_document_untouched() {
  let directory = scratch("toolpath-island-outside");
  let document = write_document(&directory, rectangle_contour(100.0, 100.0), None);
  let mut loaded = cam_store::document::load_document(&document).expect("loads");
  loaded.layers[0].entities.push(Entity {
    id: "E2".to_owned(),
    kind: EntityKind::Contour {
      contour: cam_geom::primitive::circle(Point2::new(500.0, 500.0), 10.0).expect("circle"),
    },
  });
  save_document(&document, &loaded).expect("saved");
  let before = std::fs::read(&document).expect("read");

  let outcome = run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "pocket",
    "--entities",
    "E1",
    "--islands",
    "E2",
    "--tool",
    "1",
    "--stepover",
    "3",
    "--pass-depth",
    "6",
    "--cut-depth",
    "6",
  ]);
  assert_eq!(outcome.code, 2, "{}", outcome.stderr);
  assert!(outcome.stderr.contains("E2"), "{}", outcome.stderr);
  assert_eq!(std::fs::read(&document).expect("read"), before);
  std::fs::remove_dir_all(&directory).ok();
}

// --- P3: the offset pocket cuts around its islands (A12) ---

#[test]
fn a_pocket_with_an_island_emits_a_different_program_and_inspect_names_the_islands() {
  // The same pocket authored twice through `cam-cli toolpath`, once without the island and once with
  // it, so the island reaches the strategy the way a user's would: through the document.
  let directory = scratch("pocket-island-emit");
  let library = write_library(&directory);
  let post = write_post(&directory);
  let vectors = || {
    vec![
      rectangle_contour(100.0, 100.0),
      cam_geom::primitive::circle(Point2::new(50.0, 50.0), 10.0).expect("island"),
    ]
  };
  let without = write_vectors(&directory, "without.c2d", vectors());
  let with = write_vectors(&directory, "with.c2d", vectors());

  let author = |document: &Path, islands: Option<&str>| {
    let document = document.to_str().expect("utf8");
    let mut arguments = vec![
      "toolpath",
      document,
      "--kind",
      "pocket",
      "--entities",
      "E1",
      "--tool",
      "1",
      "--stepover",
      "4",
      "--pass-depth",
      "6",
      "--cut-depth",
      "6",
    ];
    if let Some(islands) = islands {
      arguments.push("--islands");
      arguments.push(islands);
    }
    let outcome = run(&arguments);
    assert_eq!(outcome.code, 0, "{}", outcome.stderr);
  };

  author(&without, None);
  let plain = emit_program(&directory, &without, &library, &post);

  author(&with, Some("E2"));
  let islanded = emit_program(&directory, &with, &library, &post);

  assert_ne!(
    plain, islanded,
    "the island adds rings, so the program cannot be the same"
  );
  // Counting lines was the wrong lens: an island ring is two arcs where a wall is four straight
  // moves, so the islanded program is not necessarily longer. What the island really adds is arc
  // moves, and that is what a machine would execute: five island rings of two half turns each, plus
  // the crossing stage's four corner slivers, each bounded by one island arc.
  let arcs = |program: &str| {
    program
      .lines()
      .filter(|line| line.contains("G02") || line.contains("G03"))
      .count()
  };
  assert_eq!(
    arcs(&plain),
    0,
    "a square pocket cuts straight, so no arc blocks: {} lines",
    plain.lines().count()
  );
  assert_eq!(
    arcs(&islanded),
    14,
    "five island rings of two half turns plus four corner slivers: {} lines",
    islanded.lines().count()
  );
  assert!(
    !plain.contains("island"),
    "the island-free program says nothing about islands"
  );

  let outcome = run(&["inspect", with.to_str().expect("utf8")]);
  assert_eq!(outcome.code, 0, "{}", outcome.stderr);
  assert!(
    outcome.stdout.contains(", 1 island(s)"),
    "inspect names the island count: {}",
    outcome.stdout
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn islands_on_a_profile_exit_two_naming_the_flag() {
  let directory = scratch("toolpath-island-kind");
  let document = write_document(&directory, rectangle_contour(100.0, 100.0), None);
  let outcome = run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "profile",
    "--entities",
    "E1",
    "--islands",
    "E1",
    "--tool",
    "1",
    "--pass-depth",
    "6",
  ]);
  assert_eq!(outcome.code, 2);
  assert!(
    outcome.stderr.contains("--islands") && outcome.stderr.contains("pocket"),
    "the refusal names the flag and the operation it belongs to: {}",
    outcome.stderr
  );
  std::fs::remove_dir_all(&directory).ok();
}
