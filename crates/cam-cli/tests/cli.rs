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

/// The JSON payload inside a saved document's archive.
fn document_payload(document: &Path) -> String {
  let bytes = std::fs::read(document).expect("read");
  let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("zip");
  let mut payload = String::new();
  std::io::Read::read_to_string(
    &mut archive.by_name("document.json").expect("entry"),
    &mut payload,
  )
  .expect("utf8");
  payload
}

/// Rewrite the schema tag inside a saved document's archive.
///
/// Saving always stamps [`cam_model::DOCUMENT_SCHEMA`], so a test that needs the bytes an earlier
/// build would have written has to reach past the writer into the archive.
fn rewrite_schema_tag(document: &Path, tag: &str) {
  let payload = document_payload(document);
  let broken = payload.replace(cam_model::DOCUMENT_SCHEMA, tag);
  assert_ne!(
    broken, payload,
    "the archive carries the current schema tag"
  );
  let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
  writer
    .start_file("document.json", zip::write::SimpleFileOptions::default())
    .expect("start");
  std::io::Write::write_all(&mut writer, broken.as_bytes()).expect("write");
  let raw = writer.finish().expect("finish").into_inner();
  std::fs::write(document, raw).expect("write document");
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
  // Save always stamps the current schema tag, so rewrite the JSON inside the archive directly.
  rewrite_schema_tag(&document, "c2d/document@42");

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

// --- P5: golden G-code for pockets with islands (A14, A15) ---

/// A document whose `E1` is the pocket boundary and whose `E2..` are its islands.
///
/// One pocket toolpath on `E1` with the 6 mm tool [`write_library`] provides: no allowance, one 6 mm
/// pass, and the stepover the caller picks. The islands reach the strategy the way a user's do —
/// through `island_ids` in the document — so the program a strategy emits depends only on the
/// outline and the islands around it.
fn write_island_pocket(
  directory: &Path,
  name: &str,
  contours: Vec<Contour>,
  strategy: PocketStrategy,
  stepover: f64,
) -> PathBuf {
  let path = write_vectors(directory, name, contours);
  let mut document = cam_store::document::load_document(&path).expect("loads");
  let island_ids: Vec<String> = (2..=document.layers[0].entities.len())
    .map(|index| format!("E{index}"))
    .collect();
  document.toolpaths.push(Toolpath {
    id: "TP1".to_owned(),
    name: "Pocket".to_owned(),
    entity_ids: vec!["E1".to_owned()],
    island_ids,
    tool_number: 1,
    operation: Operation::Pocket(PocketParams {
      strategy,
      allowance: Length::ZERO,
      stepover: Length::from_mm(stepover),
      pass_depth: Length::from_mm(6.0),
      start_depth: Length::ZERO,
      cut_depth: Length::from_mm(6.0),
    }),
  });
  save_document(&path, &document).expect("document saved");
  path
}

/// The 100 mm square and the r10 island at (50, 50) that the offset golden clears.
fn islanded_square() -> Vec<Contour> {
  vec![
    rectangle_contour(100.0, 100.0),
    cam_geom::primitive::circle(Point2::new(50.0, 50.0), 10.0).expect("island"),
  ]
}

/// The whole program a 100 mm square with a centred r10 island emits as an offset pocket, byte for
/// byte: 8 mm stepover over a 6 mm tool, no allowance, one 6 mm pass.
///
/// The grid is `inset + k * stepover` with `inset = 3`, so the stages are `d = 3, 11, 19` and each
/// stage cuts **two** rings — the wall shrinking inward and the island growing outward (A12):
///
/// - `d = 3` — the wall is the square `3…97` cut as **four straight moves** (`X97`, `Y97`, `X3`,
///   `Y3`, N50–N80: no arcs on a wall), and the island is the circle at `r = 10 + 3 = 13` about
///   (50, 50) cut as **two ccw half turns** (N120 `G03 X37 I-13 J0` from (63, 50), N130
///   `G03 X63 I13 J0` back: `I` is the centre offset `50 - 63` and every arc is `G03` because both
///   builders are counter-clockwise).
/// - `d = 11` — the wall is `11…89` (N170–N200: `97 - 8`), the island `r = 21` (N240/N250, `I∓21`).
/// - `d = 19` — the wall is `19…81` (N290–N320), the island `r = 29` (N360/N370, `I∓29`).
/// - `d = 27` — refused by the kernel, so the loop stops: the shrunken wall ring's corner (27, 27)
///   is only `√(23² + 23²) - 10 = 22.5` mm from the island, and the grown island ring passes 13 mm
///   from the wall, so neither piece can prove the 27 mm it claims and the offset leaves no
///   material. That is why there are exactly six rings and nothing that gouges.
///
/// Ordering is the kernel's own (lowest `(bounds.min.y, bounds.min.x)` first), so the wall ring of a
/// stage always precedes its island ring. Every line ends with CRLF because the fixture
/// post-processor declares `LINE_ENDING = "[13][10]"`; the program is 45 lines.
const ISLAND_OFFSET_PROGRAM: &str = "\
( Pocket )\r\n\
( CamPilot fixture Mach3 Arcs (mm) (*.nc) )\r\n\
N0G00G17G90G21\r\n\
N10T1M06\r\n\
N20S18000M03\r\n\
N30G00X3.0000Y3.0000\r\n\
N40G01Z-6.0000F100.0\r\n\
N50G01X97.0000\r\n\
N60G01Y97.0000\r\n\
N70G01X3.0000\r\n\
N80G01Y3.0000\r\n\
N90G00Z5.0000\r\n\
N100G00X63.0000Y50.0000\r\n\
N110G01Z-6.0000\r\n\
N120G03X37.0000I-13.0000J0.0000\r\n\
N130G03X63.0000I13.0000J0.0000\r\n\
N140G00Z5.0000\r\n\
N150G00X11.0000Y11.0000\r\n\
N160G01Z-6.0000\r\n\
N170G01X89.0000\r\n\
N180G01Y89.0000\r\n\
N190G01X11.0000\r\n\
N200G01Y11.0000\r\n\
N210G00Z5.0000\r\n\
N220G00X71.0000Y50.0000\r\n\
N230G01Z-6.0000\r\n\
N240G03X29.0000I-21.0000J0.0000\r\n\
N250G03X71.0000I21.0000J0.0000\r\n\
N260G00Z5.0000\r\n\
N270G00X19.0000Y19.0000\r\n\
N280G01Z-6.0000\r\n\
N290G01X81.0000\r\n\
N300G01Y81.0000\r\n\
N310G01X19.0000\r\n\
N320G01Y19.0000\r\n\
N330G00Z5.0000\r\n\
N340G00X79.0000Y50.0000\r\n\
N350G01Z-6.0000\r\n\
N360G03X21.0000I-29.0000J0.0000\r\n\
N370G03X79.0000I29.0000J0.0000\r\n\
N380G00Z5.0000\r\n\
N390G00Z5.0000\r\n\
N400G00X0.0000Y0.0000\r\n\
N410M05\r\n\
N420M30\r\n";

#[test]
fn a_pocket_with_a_circular_island_emits_the_golden_program() {
  let directory = scratch("golden-island-offset");
  let document = write_island_pocket(
    &directory,
    "island.c2d",
    islanded_square(),
    PocketStrategy::Offset,
    8.0,
  );
  let library = write_library(&directory);
  let post = write_post(&directory);

  let program = emit_program(&directory, &document, &library, &post);
  assert_eq!(program, ISLAND_OFFSET_PROGRAM);
  // A14: the same input emits the same bytes on the next run too.
  assert_eq!(
    emit_program(&directory, &document, &library, &post),
    program
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn the_island_golden_program_survives_a_document_save_and_load_round_trip() {
  let directory = scratch("golden-island-round-trip");
  let document = write_island_pocket(
    &directory,
    "island.c2d",
    islanded_square(),
    PocketStrategy::Offset,
    8.0,
  );
  let library = write_library(&directory);
  let post = write_post(&directory);

  // Saving an island-carrying toolpath again is the round trip A9 asks for: `island_ids` survives
  // the archive and the strategy cuts the bytes it cut before.
  let reloaded = directory.join("reloaded.c2d");
  let loaded = cam_store::document::load_document(&document).expect("loads");
  assert_eq!(loaded.toolpaths[0].island_ids, vec!["E2".to_owned()]);
  save_document(&reloaded, &loaded).expect("saved");

  assert_eq!(
    emit_program(&directory, &reloaded, &library, &post),
    ISLAND_OFFSET_PROGRAM
  );
  std::fs::remove_dir_all(&directory).ok();
}

/// The whole program a 60 mm square with a centred r10 island emits as a raster pocket, byte for
/// byte: 9 mm stepover over a 6 mm tool, no allowance, one 6 mm pass.
///
/// The inset region is `3…57` in both axes and the island grows to `r = 13` about (30, 30), so the
/// row grid is `y = 3, 12, 21, 30, 39, 48` (the next grid height, 57, lies on the region's own top
/// edge, comes back ambiguous and then misses the material, so it is dropped). The rows alternate
/// direction, which is why the odd ones start at x = 57 and cut right to left:
///
/// - `y = 3` — retried from `3.00001` because it lies along the region's bottom edge; one span
///   `3…57` (N50).
/// - `y = 12` — misses the island (`|12 - 30| = 18 > 13`), one span `57…3` (N90) cut right to left.
/// - `y = 21` — **crosses the island**: `|21 - 30| = 9`, so the material's x ends are the analytic
///   `30 ± √(13² - 9²) = 30 ± 9.3808`, i.e. two spans `3…20.6192` and `39.3808…57` (N130, N170).
/// - `y = 30` — crosses the island through its centre: the gap is exactly `30 ± 13`, two spans
///   `57…43` and `17…3` (N210, N250).
/// - `y = 39` — the mirror of `y = 21`: two spans `3…20.6192` and `39.3808…57` (N290, N330).
/// - `y = 48` — misses the island, one span `57…3` (N370).
///
/// That is nine spans, and a row crossing the island is **never** one span (A13). The gap is the
/// circle's true intersection, not a tessellation of it. 45 lines, CRLF as above.
const ISLAND_RASTER_PROGRAM: &str = "\
( Pocket )\r\n\
( CamPilot fixture Mach3 Arcs (mm) (*.nc) )\r\n\
N0G00G17G90G21\r\n\
N10T1M06\r\n\
N20S18000M03\r\n\
N30G00X3.0000Y3.0000\r\n\
N40G01Z-6.0000F100.0\r\n\
N50G01X57.0000\r\n\
N60G00Z5.0000\r\n\
N70G00Y12.0000\r\n\
N80G01Z-6.0000\r\n\
N90G01X3.0000\r\n\
N100G00Z5.0000\r\n\
N110G00Y21.0000\r\n\
N120G01Z-6.0000\r\n\
N130G01X20.6192\r\n\
N140G00Z5.0000\r\n\
N150G00X39.3808\r\n\
N160G01Z-6.0000\r\n\
N170G01X57.0000\r\n\
N180G00Z5.0000\r\n\
N190G00Y30.0000\r\n\
N200G01Z-6.0000\r\n\
N210G01X43.0000\r\n\
N220G00Z5.0000\r\n\
N230G00X17.0000\r\n\
N240G01Z-6.0000\r\n\
N250G01X3.0000\r\n\
N260G00Z5.0000\r\n\
N270G00Y39.0000\r\n\
N280G01Z-6.0000\r\n\
N290G01X20.6192\r\n\
N300G00Z5.0000\r\n\
N310G00X39.3808\r\n\
N320G01Z-6.0000\r\n\
N330G01X57.0000\r\n\
N340G00Z5.0000\r\n\
N350G00Y48.0000\r\n\
N360G01Z-6.0000\r\n\
N370G01X3.0000\r\n\
N380G00Z5.0000\r\n\
N390G00Z5.0000\r\n\
N400G00X0.0000Y0.0000\r\n\
N410M05\r\n\
N420M30\r\n";

#[test]
fn a_raster_pocket_with_an_island_emits_the_golden_program() {
  let directory = scratch("golden-island-raster");
  let document = write_island_pocket(
    &directory,
    "island.c2d",
    vec![
      rectangle_contour(60.0, 60.0),
      cam_geom::primitive::circle(Point2::new(30.0, 30.0), 10.0).expect("island"),
    ],
    PocketStrategy::Raster,
    9.0,
  );
  let library = write_library(&directory);
  let post = write_post(&directory);

  assert_eq!(
    emit_program(&directory, &document, &library, &post),
    ISLAND_RASTER_PROGRAM
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_c2d_document_at_schema_one_emits_the_same_program_as_its_schema_two_original() {
  // A15's migration guarantee, observed the only way it can be: the same document written under the
  // older tag hands the post-processor the same bytes.
  let directory = scratch("golden-schema-one");
  let document = write_island_pocket(
    &directory,
    "island.c2d",
    islanded_square(),
    PocketStrategy::Offset,
    8.0,
  );
  let library = write_library(&directory);
  let post = write_post(&directory);

  let current = emit_program(&directory, &document, &library, &post);
  assert_eq!(current, ISLAND_OFFSET_PROGRAM, "the @2 original");

  rewrite_schema_tag(&document, "c2d/document@1");
  let payload = document_payload(&document);
  assert!(
    payload.contains("c2d/document@1"),
    "the archive really carries the older tag: {payload}"
  );
  assert!(
    !payload.contains(cam_model::DOCUMENT_SCHEMA),
    "and no longer the current one: {payload}"
  );

  assert_eq!(
    emit_program(&directory, &document, &library, &post),
    current,
    "a @1 document cuts exactly as it did before the schema moved"
  );
  std::fs::remove_dir_all(&directory).ok();
}

/// The whole program a 100 mm square with an r24 island and an r12 island nested inside it emits as
/// an offset pocket, byte for byte: 7 mm stepover over a 6 mm tool, no allowance, one 6 mm pass.
///
/// The nested island is a **boss**, and the parity rule is what makes it material again (A3, D-RB-2):
/// a point at the centre is inside three boundaries — the pocket's square, the r24 hole and the r12
/// boss — and three is odd, so the boss disk is material and the pocket **clears** it, while the
/// annulus between the boss and the hole is inside two boundaries and is left standing. The kernel
/// therefore offsets each boundary the way that grows the material: the square inward, the hole
/// outward (`r = 24 + d`) and the boss inward (`r = 12 - d`).
///
/// The grid is `inset + k * stepover` with `inset = 3`, so the stages are `d = 3, 10` and each cuts
/// **three** rings — wall, hole, boss, in the kernel's order (lowest `(bounds.min.y, bounds.min.x)`):
///
/// - `d = 3` — the wall is the square `3…97` as four straight moves (N50–N80); the hole ring is
///   `r = 27` about (50, 50), cut as two ccw half turns from (77, 50) (N120/N130, `I∓27`); the boss
///   ring is `r = 9` from (59, 50) (N170/N180, `I∓9`) — *inside* the boss's own boundary, because
///   that disk is what the tool is clearing, not what it must avoid.
/// - `d = 10` — the wall is `10…90` (N220–N250), the hole `r = 34` from (84, 50) (N290/N300), the
///   boss `r = 2` from (52, 50) (N340/N350).
/// - `d = 17` — refused by the kernel, so the loop stops: the boss ring would have to shrink to
///   `12 - 17 = -5` mm, a radius that does not exist, and the whole stage is refused rather than
///   returned half-built. That is why there are three rings per stage and none that gouges the
///   annulus the pocket must leave.
///
/// All eight arc blocks are `G03`: parity decides an offset's **sign**, never its orientation, and
/// both islands are counter-clockwise circles built by `cam_geom::primitive::circle`. 43 lines, CRLF
/// as above.
const BOSS_OFFSET_PROGRAM: &str = "\
( Pocket )\r\n\
( CamPilot fixture Mach3 Arcs (mm) (*.nc) )\r\n\
N0G00G17G90G21\r\n\
N10T1M06\r\n\
N20S18000M03\r\n\
N30G00X3.0000Y3.0000\r\n\
N40G01Z-6.0000F100.0\r\n\
N50G01X97.0000\r\n\
N60G01Y97.0000\r\n\
N70G01X3.0000\r\n\
N80G01Y3.0000\r\n\
N90G00Z5.0000\r\n\
N100G00X77.0000Y50.0000\r\n\
N110G01Z-6.0000\r\n\
N120G03X23.0000I-27.0000J0.0000\r\n\
N130G03X77.0000I27.0000J0.0000\r\n\
N140G00Z5.0000\r\n\
N150G00X59.0000\r\n\
N160G01Z-6.0000\r\n\
N170G03X41.0000I-9.0000J0.0000\r\n\
N180G03X59.0000I9.0000J0.0000\r\n\
N190G00Z5.0000\r\n\
N200G00X10.0000Y10.0000\r\n\
N210G01Z-6.0000\r\n\
N220G01X90.0000\r\n\
N230G01Y90.0000\r\n\
N240G01X10.0000\r\n\
N250G01Y10.0000\r\n\
N260G00Z5.0000\r\n\
N270G00X84.0000Y50.0000\r\n\
N280G01Z-6.0000\r\n\
N290G03X16.0000I-34.0000J0.0000\r\n\
N300G03X84.0000I34.0000J0.0000\r\n\
N310G00Z5.0000\r\n\
N320G00X52.0000\r\n\
N330G01Z-6.0000\r\n\
N340G03X48.0000I-2.0000J0.0000\r\n\
N350G03X52.0000I2.0000J0.0000\r\n\
N360G00Z5.0000\r\n\
N370G00Z5.0000\r\n\
N380G00X0.0000Y0.0000\r\n\
N390M05\r\n\
N400M30\r\n";

#[test]
fn a_pocket_with_a_boss_inside_an_island_emits_the_golden_program() {
  // The model allows a nested island pair (a boss inside a hole) and the strategy clears the boss,
  // so the whole path from the document to the post is observable here: two stages of three rings,
  // with the boss ring shrinking from r9 to r2 instead of the annulus between the islands.
  let directory = scratch("golden-boss");
  let document = write_island_pocket(
    &directory,
    "boss.c2d",
    vec![
      rectangle_contour(100.0, 100.0),
      cam_geom::primitive::circle(Point2::new(50.0, 50.0), 24.0).expect("hole"),
      cam_geom::primitive::circle(Point2::new(50.0, 50.0), 12.0).expect("boss"),
    ],
    PocketStrategy::Offset,
    7.0,
  );
  let library = write_library(&directory);
  let post = write_post(&directory);

  assert_eq!(
    emit_program(&directory, &document, &library, &post),
    BOSS_OFFSET_PROGRAM
  );
  std::fs::remove_dir_all(&directory).ok();
}

// --- M5 S4: the `simulate` command, its report and its exit code (A7, D-M5-6) ---

/// A tool library holding one tool at T1 with the given diameter, authored by `tools add`.
///
/// The command is the real one, so the feed rates and the stepdown the strategies take their
/// defaults from are the ones a user's library carries.
fn write_sim_library(directory: &Path, diameter: f64) -> PathBuf {
  let path = directory.join("sim-tools.sqlite");
  let diameter = diameter.to_string();
  let outcome = run(&[
    "tools",
    "add",
    path.to_str().expect("utf8"),
    "--id",
    "SIM",
    "--diameter",
    &diameter,
    "--number",
    "1",
  ]);
  assert_eq!(outcome.code, 0, "{}", outcome.stderr);
  path
}

/// Rewrite a saved document's job: the stock the simulation places `z = 0` and the floor at.
fn set_job(document: &Path, edit: impl Fn(&mut Job)) {
  let mut loaded = cam_store::document::load_document(document).expect("loads");
  edit(&mut loaded.job);
  save_document(document, &loaded).expect("saved");
}

/// Author a pocket on `E1` through the real `toolpath` command, as a user's document carries one.
fn author_pocket(document: &Path, library: &Path, name: &str, stepover: f64, cut_depth: f64) {
  author_pocket_with(document, library, name, "offset", stepover, cut_depth);
}

/// The same, with the clearing strategy named: `offset` or `raster`.
fn author_pocket_with(
  document: &Path,
  library: &Path,
  name: &str,
  strategy: &str,
  stepover: f64,
  cut_depth: f64,
) {
  let stepover = stepover.to_string();
  let cut_depth = cut_depth.to_string();
  let outcome = run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "pocket",
    "--entities",
    "E1",
    "--tool",
    "1",
    "--name",
    name,
    "--strategy",
    strategy,
    "--stepover",
    &stepover,
    "--pass-depth",
    "6",
    "--cut-depth",
    &cut_depth,
    "--library",
    library.to_str().expect("utf8"),
  ]);
  assert_eq!(outcome.code, 0, "{}", outcome.stderr);
}

/// Author a drilling toolpath on the circle `E1` through the real `toolpath` command.
fn author_drill(document: &Path, library: &Path, name: &str, cut_depth: f64) {
  let cut_depth = cut_depth.to_string();
  let outcome = run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "drilling",
    "--entities",
    "E1",
    "--tool",
    "1",
    "--name",
    name,
    "--cut-depth",
    &cut_depth,
    "--library",
    library.to_str().expect("utf8"),
  ]);
  assert_eq!(outcome.code, 0, "{}", outcome.stderr);
}

/// Run `simulate` on a document and its library.
fn simulate(document: &Path, library: &Path, extra: &[&str]) -> Outcome {
  let mut arguments = vec![
    "simulate",
    document.to_str().expect("utf8"),
    "--library",
    library.to_str().expect("utf8"),
  ];
  arguments.extend_from_slice(extra);
  run(&arguments)
}

/// The first millimetre figure on the report line that starts with `prefix`.
fn report_volume(stdout: &str, prefix: &str) -> f64 {
  let line = stdout
    .lines()
    .find(|line| line.starts_with(prefix))
    .unwrap_or_else(|| panic!("no {prefix:?} line in:\n{stdout}"));
  line
    .split_whitespace()
    .find_map(|word| word.parse::<f64>().ok())
    .unwrap_or_else(|| panic!("no number on {line:?}"))
}

/// The whole report a 100 mm square pocket prints, byte for byte.
///
/// The fixture is a pocket the tool really clears: a 2 mm tool in 1 mm cells reaches every cell the
/// document authorises — a cell centre 0.707 mm from the square's corner is inside the 1 mm disc the
/// tool sweeps there — and no cell outside it. So `gouge` and `uncut` are zero and the report is
/// clean, which is what makes everything else in it pinned rather than counted.
///
/// The program is `emit`'s own stream: 25 rings at insets 1, 3, … 49 mm (a 6 mm pass, 2 mm
/// stepover, no allowance), each cut as a rapid, a plunge, four sides and a retract — 25 x 7 = 175
/// moves. The cutting length is the sum of the four sides of every ring: `sum(4 * (100 - 2k))` for
/// k = 1, 3, … 49 = 5000 mm. The removed volume is the region's area times the depth,
/// 100 x 100 x 6 = 60000 mm3, on the 200 x 100 x 18 mm stock the fixture declares.
const CLEAN_POCKET_REPORT: &str = "\
simulate: 200.000 x 100.000 x 18.000 mm, 1 x 1 mm cells (200 x 100), 1 toolpath(s)
program: 175 move(s), 5000.000 mm cutting length
removed: 60000.000 mm3 of 360000.000 mm3, 0 through cell(s)
gouge: 0.000 mm3 over 0 cell(s)
uncut: 0.000 mm3 over 0 cell(s)
crash: none
";

#[test]
fn simulate_pins_the_whole_report_of_a_pocket_that_cuts_what_it_claimed() {
  let directory = scratch("simulate-report");
  let document = write_vectors(
    &directory,
    "pocket.c2d",
    vec![rectangle_contour(100.0, 100.0)],
  );
  let library = write_sim_library(&directory, 2.0);
  author_pocket(&document, &library, "Pocket", 2.0, 6.0);

  let before = std::fs::read(&document).expect("read");
  let outcome = simulate(&document, &library, &[]);
  assert_eq!(outcome.code, 0, "{}{}", outcome.stdout, outcome.stderr);
  assert!(outcome.stderr.is_empty(), "{}", outcome.stderr);
  assert_eq!(outcome.stdout, CLEAN_POCKET_REPORT);

  // A8's oracle: the reported volume is the analytic area times depth, within the coverage bound
  // of a boundary at most half a cell wide — `perimeter x cell / 2`, in volume.
  let removed = report_volume(&outcome.stdout, "removed:");
  let analytic = 100.0 * 100.0 * 6.0;
  let bound = 4.0 * 100.0 * 1.0 / 2.0 * 6.0;
  assert!(
    (removed - analytic).abs() <= bound,
    "removed {removed} mm3 against the analytic {analytic} mm3, bound {bound} mm3"
  );
  assert_eq!(
    std::fs::read(&document).expect("read"),
    before,
    "a simulation writes nothing to the document"
  );
  std::fs::remove_dir_all(&directory).ok();
}

/// The whole report a 100 mm square raster pocket prints, byte for byte.
///
/// The same region as the offset pocket above, cleared the other way: a 2 mm tool in 1 mm cells,
/// inset 1 mm from the square's edges, so the rows span x = 1 .. 99 (98 mm) and start at y = 1 one
/// stepover apart. The stepover is 1.58 mm, which puts 63 rows at y = 1, 2.58, ... 98.96, and that
/// last row is what the fixture turns on. A row ends 1 mm inside the square's edge, so a cell centre
/// half a cell further out is `sqrt(1^2 - 0.5^2)` = 0.866 mm away across the row: the last row has to
/// be within that of the top row of centres at 99.5 mm, and short of the inset region's far edge at
/// 99 mm, where the kernel reports a row lying on a boundary as ambiguous and the strategy drops it —
/// the row-lattice finding recorded in the task's return. Between rows the discs overlap by 0.79 mm at
/// the row ends, inside that 0.866 mm, so the raster clears the whole 100 x 100 square: 10000 cells at
/// 6 mm deep is 60000 mm3. Four moves a row (rapid, plunge, feed, retract) is 63 x 4 = 252 moves, and
/// the cutting length is 63 rows x 98 mm = 6174 mm.
const CLEAN_RASTER_REPORT: &str = "\
simulate: 200.000 x 100.000 x 18.000 mm, 1 x 1 mm cells (200 x 100), 1 toolpath(s)
program: 252 move(s), 6174.000 mm cutting length
removed: 60000.000 mm3 of 360000.000 mm3, 0 through cell(s)
gouge: 0.000 mm3 over 0 cell(s)
uncut: 0.000 mm3 over 0 cell(s)
crash: none
";

#[test]
fn simulate_pins_the_whole_report_of_a_raster_pocket_that_cuts_what_it_claimed() {
  let directory = scratch("simulate-raster");
  let document = write_vectors(
    &directory,
    "raster.c2d",
    vec![rectangle_contour(100.0, 100.0)],
  );
  let library = write_sim_library(&directory, 2.0);
  author_pocket_with(&document, &library, "Pocket", "raster", 1.58, 6.0);

  let outcome = simulate(&document, &library, &[]);
  assert_eq!(outcome.code, 0, "{}{}", outcome.stdout, outcome.stderr);
  assert!(outcome.stderr.is_empty(), "{}", outcome.stderr);
  assert_eq!(outcome.stdout, CLEAN_RASTER_REPORT);

  // A8's oracle for the second strategy: the same region, the same analytic volume, the same bound —
  // the region's perimeter times half a cell times the depth.
  let removed = report_volume(&outcome.stdout, "removed:");
  let analytic = 100.0 * 100.0 * 6.0;
  let bound = 4.0 * 100.0 * 1.0 / 2.0 * 6.0;
  assert!(
    (removed - analytic).abs() <= bound,
    "removed {removed} mm3 against the analytic {analytic} mm3, bound {bound} mm3"
  );
  std::fs::remove_dir_all(&directory).ok();
}

/// The whole report a drilling toolpath prints, byte for byte.
///
/// One r3 hole drilled 6 mm deep with a 6 mm tool: the tool is the hole, so the document's target and
/// the program's sweep are the same disc. 32 cells of 1 mm2 have their centre inside it, which is
/// 32 x 1 x 6 = 192 mm3 — against the analytic `pi r^2 depth` = 169.646 mm3, inside the hole's own
/// bound of `2 pi r x cell / 2 x depth` = 56.549 mm3. The program is a rapid, one plunge (the peck
/// depth is zero, so the hole is one plunge) and a retract: 3 moves, no cutting length.
const CLEAN_DRILL_REPORT: &str = "\
simulate: 200.000 x 100.000 x 18.000 mm, 1 x 1 mm cells (200 x 100), 1 toolpath(s)
program: 3 move(s), 0.000 mm cutting length
removed: 192.000 mm3 of 360000.000 mm3, 0 through cell(s)
gouge: 0.000 mm3 over 0 cell(s)
uncut: 0.000 mm3 over 0 cell(s)
crash: none
";

#[test]
fn simulate_pins_the_whole_report_of_a_hole_the_drill_matches() {
  let directory = scratch("simulate-drill");
  let document = write_vectors(&directory, "hole.c2d", vec![circle_contour(3.0)]);
  let library = write_sim_library(&directory, 6.0);
  author_drill(&document, &library, "Drill", 6.0);

  let outcome = simulate(&document, &library, &[]);
  assert_eq!(outcome.code, 0, "{}{}", outcome.stdout, outcome.stderr);
  assert!(outcome.stderr.is_empty(), "{}", outcome.stderr);
  assert_eq!(outcome.stdout, CLEAN_DRILL_REPORT);

  // A8's oracle for drilling: the removed volume is `pi r^2 depth` per hole, inside the hole's
  // perimeter times half a cell times the depth.
  let removed = report_volume(&outcome.stdout, "removed:");
  let analytic = PI * 3.0 * 3.0 * 6.0;
  let bound = 2.0 * PI * 3.0 * 1.0 / 2.0 * 6.0;
  assert!(
    (removed - analytic).abs() <= bound,
    "removed {removed} mm3 against the analytic {analytic} mm3, bound {bound} mm3"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_finer_resolution_reduces_the_coverage_error() {
  // A6: refinement is monotone on a fixed fixture. The region is 120.4 x 90.4 mm, so its edges do
  // not land on the cell lattice and the reported volume is the cell count's: the volume error
  // against `area x depth` is exactly the coverage error the bound is written for. The tool is 1 mm
  // wide and the rings step over 0.6 mm, which keeps the sweep covering the whole region at both
  // resolutions — the innermost ring's leftover is 0.3 mm, inside the 0.5 mm the tool reaches — so
  // the two runs differ in the reported volume and in nothing else.
  let directory = scratch("simulate-refine");
  let document = write_vectors(
    &directory,
    "refine.c2d",
    vec![rectangle_contour(120.4, 90.4)],
  );
  let library = write_sim_library(&directory, 1.0);
  author_pocket(&document, &library, "Pocket", 0.6, 6.0);

  let coarse = simulate(&document, &library, &[]);
  let fine = simulate(&document, &library, &["--resolution", "0.5"]);
  assert_eq!(coarse.code, 0, "{}{}", coarse.stdout, coarse.stderr);
  assert_eq!(fine.code, 0, "{}{}", fine.stdout, fine.stderr);
  assert!(
    coarse
      .stdout
      .starts_with("simulate: 200.000 x 100.000 x 18.000 mm, 1 x 1 mm cells"),
    "{}",
    coarse.stdout
  );
  assert!(
    fine
      .stdout
      .starts_with("simulate: 200.000 x 100.000 x 18.000 mm, 0.5 x 0.5 mm cells"),
    "{}",
    fine.stdout
  );

  let analytic = 120.4 * 90.4 * 6.0;
  let coarse_error = (report_volume(&coarse.stdout, "removed:") - analytic).abs();
  let fine_error = (report_volume(&fine.stdout, "removed:") - analytic).abs();
  let bound = |cell: f64| 2.0 * (120.4 + 90.4) * cell / 2.0 * 6.0;
  assert!(
    coarse_error <= bound(1.0),
    "1 mm error {coarse_error} mm3 over the {} mm3 bound:\n{}",
    bound(1.0),
    coarse.stdout
  );
  assert!(
    fine_error <= bound(0.5),
    "0.5 mm error {fine_error} mm3 over the {} mm3 bound:\n{}",
    bound(0.5),
    fine.stdout
  );
  assert!(
    fine_error < coarse_error,
    "0.5 mm cells must be closer than 1 mm cells: {fine_error} vs {coarse_error}"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_pocket_deeper_than_the_stock_reports_uncut_and_exits_six() {
  // S6's too-shallow case, stated so the document is the defect: it asks for 6 mm of material out of
  // a 5 mm stock, so the deepest millimetre of the claim is material the program never removes.
  let directory = scratch("simulate-uncut");
  let document = write_vectors(
    &directory,
    "shallow.c2d",
    vec![rectangle_contour(100.0, 100.0)],
  );
  set_job(&document, |job| job.thickness = Length::from_mm(5.0));
  let library = write_sim_library(&directory, 2.0);
  author_pocket(&document, &library, "Pocket", 2.0, 6.0);

  let outcome = simulate(&document, &library, &[]);
  assert_eq!(outcome.code, 6, "{}{}", outcome.stdout, outcome.stderr);
  let expected = "\
simulate: 200.000 x 100.000 x 5.000 mm, 1 x 1 mm cells (200 x 100), 1 toolpath(s)
program: 175 move(s), 5000.000 mm cutting length
removed: 50000.000 mm3 of 100000.000 mm3, 10000 through cell(s)
gouge: 0.000 mm3 over 0 cell(s)
uncut: 10000.000 mm3 over 10000 cell(s)
crash: none
";
  assert_eq!(outcome.stdout, expected);
  assert!(outcome.stderr.contains("uncut"), "{}", outcome.stderr);
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_drill_wider_than_its_hole_reports_gouge_and_exits_six() {
  // S6's too-deep direction. A pocket cannot be too deep through a document that validates — the
  // target is derived from the same `cut_depth` the pass sequence lands on — so the gouge is stated
  // the way a document can express it: a 4 mm hole drilled with a 6 mm tool removes a disc a
  // millimetre wider on every side than the hole the document authorised.
  //
  // The arithmetic: 32 cells of 1 mm2 have their centre inside the 3 mm disc the tool sweeps, and 12
  // of them are inside the 2 mm circle the document authorised, so the 20 cells outside the hole are
  // 6 mm of material nobody asked to lose: 20 x 1 x 6 = 120 mm3 of gouge, over 20 cells. Every cell
  // the document did claim was reached, so uncut is zero — the same volume reported with the sign the
  // defect has, and not the other one.
  let directory = scratch("simulate-gouge");
  let document = write_vectors(
    &directory,
    "gouge.c2d",
    vec![cam_geom::primitive::circle(Point2::new(50.0, 50.0), 2.0).expect("hole")],
  );
  let library = write_sim_library(&directory, 6.0);
  author_drill(&document, &library, "Drill", 6.0);

  let outcome = simulate(&document, &library, &[]);
  assert_eq!(outcome.code, 6, "{}{}", outcome.stdout, outcome.stderr);
  let expected = "\
simulate: 200.000 x 100.000 x 18.000 mm, 1 x 1 mm cells (200 x 100), 1 toolpath(s)
program: 3 move(s), 0.000 mm cutting length
removed: 192.000 mm3 of 360000.000 mm3, 0 through cell(s)
gouge: 120.000 mm3 over 20 cell(s)
uncut: 0.000 mm3 over 0 cell(s)
crash: none
";
  assert_eq!(outcome.stdout, expected);
  assert!(outcome.stderr.contains("gouge"), "{}", outcome.stderr);
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_rapid_at_depth_exits_six_and_names_the_toolpath_and_move() {
  // D-M5-4: a clearance height below the stock top is a program that rams the tool. The crash rule
  // reports it instead of cutting with it, and the report names where.
  let directory = scratch("simulate-crash");
  let document = write_vectors(
    &directory,
    "crash.c2d",
    vec![rectangle_contour(100.0, 100.0)],
  );
  set_job(&document, |job| job.safe_z = Length::from_mm(-3.0));
  let library = write_sim_library(&directory, 2.0);
  author_pocket(&document, &library, "Pocket", 2.0, 6.0);

  let outcome = simulate(&document, &library, &[]);
  assert_eq!(outcome.code, 6, "{}{}", outcome.stdout, outcome.stderr);
  let crashes: Vec<&str> = outcome
    .stdout
    .lines()
    .filter(|line| line.starts_with("crash: ") && *line != "crash: none")
    .collect();
  assert_eq!(
    crashes.first().copied(),
    Some("crash: Pocket move 0 at (1.000, 1.000, -3.000)"),
    "the first crash is the rapid onto the first ring:\n{}",
    outcome.stdout
  );
  assert!(
    crashes.len() > 1,
    "every rapid below the surface is reported, not only the first:\n{}",
    outcome.stdout
  );
  let mut previous = None;
  for line in &crashes {
    let index: usize = line
      .split_once(" move ")
      .and_then(|(_, rest)| rest.split_once(' '))
      .and_then(|(index, _)| index.parse().ok())
      .unwrap_or_else(|| panic!("no move index on {line:?}"));
    assert!(
      line.starts_with("crash: Pocket move "),
      "each crash names its toolpath:\n{line}"
    );
    if let Some(previous) = previous {
      assert!(
        previous < index,
        "crash lines are in program order:\n{}",
        outcome.stdout
      );
    }
    previous = Some(index);
  }
  assert!(outcome.stderr.contains("crash"), "{}", outcome.stderr);
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn two_simulations_of_one_document_print_identical_bytes() {
  // A7 and S9: the report is the contract, so the same document prints the same text every time.
  let directory = scratch("simulate-determinism");
  let document = write_vectors(
    &directory,
    "twice.c2d",
    vec![rectangle_contour(100.0, 100.0)],
  );
  let library = write_sim_library(&directory, 2.0);
  author_pocket(&document, &library, "Pocket", 2.0, 6.0);

  let first = simulate(&document, &library, &[]);
  let second = simulate(&document, &library, &[]);
  assert_eq!(first.code, 0, "{}{}", first.stdout, first.stderr);
  assert_eq!(first.code, second.code);
  assert_eq!(first.stdout, second.stdout);
  assert_eq!(first.stderr, second.stderr);
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_resolution_over_the_cell_cap_exits_two_naming_the_cell_the_grid_and_the_cap() {
  // A6: the request is refused by name, never allocated.
  let directory = scratch("simulate-cap");
  let document = write_vectors(&directory, "cap.c2d", vec![rectangle_contour(100.0, 100.0)]);
  let library = write_sim_library(&directory, 2.0);
  author_pocket(&document, &library, "Pocket", 2.0, 6.0);

  let outcome = simulate(&document, &library, &["--resolution", "0.01"]);
  assert_eq!(outcome.code, 2, "{}{}", outcome.stdout, outcome.stderr);
  assert!(outcome.stdout.is_empty(), "no report when nothing ran");
  for needle in ["0.01", "20000", "10000", "4000000"] {
    assert!(
      outcome.stderr.contains(needle),
      "the refusal names {needle:?}: {}",
      outcome.stderr
    );
  }
  assert!(
    outcome.stderr.contains("--resolution"),
    "the refusal says what to change: {}",
    outcome.stderr
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn simulate_without_a_tool_library_exits_two_and_says_so() {
  let directory = scratch("simulate-no-library");
  let document = write_vectors(
    &directory,
    "bare.c2d",
    vec![rectangle_contour(100.0, 100.0)],
  );
  let outcome = run(&["simulate", document.to_str().expect("utf8")]);
  assert_eq!(outcome.code, 2, "{}{}", outcome.stdout, outcome.stderr);
  assert!(
    outcome.stderr.contains("--library"),
    "the refusal names the flag: {}",
    outcome.stderr
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn the_help_lists_the_simulate_command_its_flags_and_its_exit_code() {
  let outcome = run(&["--help"]);
  assert_eq!(outcome.code, 0);
  for needle in [
    "simulate <document>",
    "--library <path>",
    "--resolution <mm>",
    "the simulation found",
  ] {
    assert!(
      outcome.stdout.contains(needle),
      "help is missing {needle:?}:\n{}",
      outcome.stdout
    );
  }
}
