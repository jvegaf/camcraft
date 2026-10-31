//! End-to-end tests for the headless pipeline (A8, A9).
//!
//! These run the real binary, so they assert the observable contract: exit code, exact stdout and
//! stderr, and the files left on disk.

use std::f64::consts::{FRAC_PI_2, PI};
use std::path::{Path, PathBuf};
use std::process::Command;

use cam_geom::{Arc, Contour, LineSeg, Point2, Segment};
use cam_model::{
  ChamferParams, Document, DrillParams, Entity, EntityKind, FlutingParams, Job, Layer, Operation,
  PocketParams, PocketStrategy, ProfileParams, ProfileSide, RetractMode, Toolpath,
};
use cam_store::document::save_document;
use cam_store::tools::{CuttingData, ToolGeometry, ToolLibrary};
use cam_units::{Angle, Length, UnitSystem};

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
  rewrite_payload(document, &broken);
}

/// Replace a saved document's `document.json` with `payload`, keeping the archive.
fn rewrite_payload(document: &Path, payload: &str) {
  let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
  writer
    .start_file("document.json", zip::write::SimpleFileOptions::default())
    .expect("start");
  std::io::Write::write_all(&mut writer, payload.as_bytes()).expect("write");
  let raw = writer.finish().expect("finish").into_inner();
  std::fs::write(document, raw).expect("write document");
}

/// Drop the `ramp` field from a saved document's `fluting` operation, as an older or hand-written
/// document that predates it would have been written.
fn remove_fluting_ramp(document: &Path) {
  let payload = document_payload(document);
  let marker = "\"ramp\"";
  let at = payload.find(marker).expect("the archive carries the ramp");
  // The field is its operation's last, so it is preceded by the previous field's comma and ends at the
  // line break before the closing brace. Both are found by position, not by their printed form, so the
  // pretty-printer's layout can change without breaking this.
  let comma = payload[..at].rfind(',').expect("a field separator");
  let end = payload[at..]
    .find('\n')
    .map(|offset| at + offset)
    .expect("a line break after the field");
  let without = format!("{}{}", &payload[..comma], &payload[end..]);
  assert!(
    !without.contains(marker),
    "the ramp field is gone from:\n{without}"
  );
  rewrite_payload(document, &without);
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
  assert!(outcome.stdout.contains("blocks: 20"), "{}", outcome.stdout);

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

/// `source`, re-saved at another origin corner — the one field `Job::origin` that moves the machine
/// frame relative to the document.
fn with_origin(source: &Path, corner: cam_model::OriginCorner, target: &Path) {
  let mut document = cam_store::document::load_document(source).expect("the document loads");
  document.job.origin = corner;
  save_document(target, &document).expect("the document saves");
}

/// The arc words a program states, in order, as `G02` and `G03`.
///
/// Only the word is read, not the block: a helix carries `G02`/`G03` too, and the leading words of the
/// header (`G17`, `G90`, `G21`) contain neither.
fn arc_words(program: &str) -> Vec<&'static str> {
  program
    .lines()
    .filter_map(|line| {
      if line.contains("G02") {
        Some("G02")
      } else if line.contains("G03") {
        Some("G03")
      } else {
        None
      }
    })
    .collect()
}

#[test]
fn a_top_left_origin_flips_every_arc_word_and_a_centred_origin_flips_none() {
  // A7 as widened by L6. `top_left` reflects the job, so a curve the document traverses
  // counter-clockwise is clockwise in the machine's own numbering, and a program that mapped only its
  // points states the complementary arc: a G03 where the machine needs G02. That is a wrong cut, and it
  // survived every milestone until now because every fixture is bottom-left. `center` translates the
  // job, so it keeps the document's handedness and is the control that shows the flip is the mirror's
  // doing rather than the point mapping's.
  let directory = scratch("origin-arc-handedness");
  let document = write_document(
    &directory,
    cam_geom::primitive::circle(Point2::new(100.0, 50.0), 15.0).expect("a circle"),
    None,
  );
  let library = write_library(&directory);
  let post = write_post(&directory);

  let bottom_left = arc_words(&emit_program(&directory, &document, &library, &post));
  assert!(
    !bottom_left.is_empty(),
    "a circle profile is emitted with arc blocks at bottom-left"
  );
  assert!(
    bottom_left.iter().all(|word| *word == "G03"),
    "the document's circle runs counter-clockwise, so bottom-left says G03: {bottom_left:?}"
  );

  let top_left_document = directory.join("top-left.c2d");
  with_origin(
    &document,
    cam_model::OriginCorner::TopLeft,
    &top_left_document,
  );
  let top_left = arc_words(&emit_program(
    &directory,
    &top_left_document,
    &library,
    &post,
  ));
  let complementary: Vec<&str> = bottom_left
    .iter()
    .map(|word| if *word == "G02" { "G03" } else { "G02" })
    .collect();
  assert_eq!(
    top_left, complementary,
    "reflecting the job reverses every arc's sense, so the program states the other word"
  );

  let centred_document = directory.join("centred.c2d");
  with_origin(
    &document,
    cam_model::OriginCorner::Center,
    &centred_document,
  );
  let centred = arc_words(&emit_program(
    &directory,
    &centred_document,
    &library,
    &post,
  ));
  assert_eq!(
    centred, bottom_left,
    "centring the job moves it without mirroring it, so the arc words are the same"
  );
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
    "chamfer",
    "--chamfer-width",
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
/// row grid is `y = 3, 12, 21, 30, 39, 48, 57` — seven rows, the last one lying exactly on the
/// region's own top edge. A row on a boundary is the row the edge sits on, so it is cut a hair inside
/// rather than dropped (S6): at `y = 57` the retry upward is outside the region and clips nothing, and
/// the retry downward at `56.99999` clips the full span `3…57`, which is why the row is cut left to
/// right as the seventh row. Dropping it — what the strategy did before S6 — left the tool's last row
/// at `y = 48`, whose 3 mm radius reaches 51, so the 9 mm strip from 51 to the square's 60 mm edge was
/// never cut. The rows alternate direction, which is why the odd ones start at x = 57 and cut right to
/// left:
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
/// - `y = 57` — the region's far edge: `|57 - 30| = 27 > 13`, so one span `3…57` (N410), cut left to
///   right because the row index is even.
///
/// That is ten spans, and a row crossing the island is **never** one span (A13). The gap is the
/// circle's true intersection, not a tessellation of it. 49 lines, CRLF as above.
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
N390G00Y57.0000\r\n\
N400G01Z-6.0000\r\n\
N410G01X57.0000\r\n\
N420G00Z5.0000\r\n\
N430G00Z5.0000\r\n\
N440G00X0.0000Y0.0000\r\n\
N450M05\r\n\
N460M30\r\n";

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
/// stepover apart. The stepover is **2 mm** — the strategy's own stepover, not a tuned one — which
/// puts 50 rows at y = 1, 3, … 99: the last row lies exactly on the inset region's far edge, and that
/// row is the fixture's point. A row on a boundary is the row the edge sits on, not an empty row, so
/// it is cut a hair inside (S6); before that fix the far-edge row was dropped, the last row sat at
/// y = 97, and a 2 mm tool there reaches y = 98 — leaving the 2 mm strip from 98 to the square's edge
/// standing as 200 cells (two rows of 100) at 6 mm deep, 1200 mm3, and exit 6 (the defect M5's oracle
/// measured).
///
/// With the row cut, every cell centre of the square is within the 1 mm the tool reaches: rows are 2
/// apart and cell centres are half-integers, so a centre is 0.5 mm from a row, and 0.5 mm from a row
/// end at worst — `sqrt(0.5^2 + 0.5^2)` = 0.707 mm, inside the radius. The raster therefore clears the
/// whole 100 x 100 square: 10000 cells at 6 mm deep is 60000 mm3, gouge 0 and uncut 0. Four moves a
/// row (rapid, plunge, feed, retract) is 50 x 4 = 200 moves, and the cutting length is 50 rows x 98 mm
/// = 4900 mm.
///
/// What this pins is the row lattice's far edge, not a claim that holds at every resolution: a 2 mm
/// stepover under a 2 mm tool leaves adjacent rows exactly tangent, so the same program at
/// `--resolution 0.5` reports `uncut: 300.000 mm3 over 200 cell(s)` — the centres between two tangent
/// rows are 0.75 + 0.75 mm from a row end, past the radius. The stepover is the defect's own number
/// (S6), and 1 mm cells are the resolution the oracle's bound is written for.
const CLEAN_RASTER_REPORT: &str = "\
simulate: 200.000 x 100.000 x 18.000 mm, 1 x 1 mm cells (200 x 100), 1 toolpath(s)
program: 200 move(s), 4900.000 mm cutting length
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
  author_pocket_with(&document, &library, "Pocket", "raster", 2.0, 6.0);

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

// --- chamfer (M6 A2, A8, D-M6-2, D-M6-9) ------------------------------------------------------

/// A library holding one 90 degree V-bit as T1: what a chamfer needs, and a real bit's geometry.
///
/// The diameter is the bit's large diameter and moves nothing — a chamfer's tip runs on the vector —
/// and the 90 degree included angle is what makes the derived depth equal the bevel's width.
fn write_chamfer_library(directory: &Path) -> PathBuf {
  write_chamfer_library_at(directory, "chamfer-tools.sqlite", 90.0, 0.0)
}

/// The same library with a chosen included angle and tip flat, so the refusals can be reached.
fn write_chamfer_library_at(directory: &Path, name: &str, degrees: f64, flat: f64) -> PathBuf {
  let path = directory.join(name);
  let library = ToolLibrary::open(&path).expect("library");
  library
    .put_geometry(&ToolGeometry {
      id: "V1".to_owned(),
      name_format: "{Tool Type} ({Diameter|F}{Units Short})".to_owned(),
      tool_type: 0,
      units: UnitSystem::Mm,
      diameter: Some(Length::from_mm(12.7)),
      included_angle: Some(Angle::from_degrees(degrees)),
      flat_diameter: Some(Length::from_mm(flat)),
      num_flutes: Some(2),
      flute_length: None,
      tip_radius: None,
      custom_attributes: None,
    })
    .expect("geometry");
  library
    .put_cutting_data(&CuttingData {
      id: "V1CUT".to_owned(),
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
  library
    .link("V1LINK", "V1", "V1CUT", None, None)
    .expect("link");
  path
}

/// The whole program one chamfer emits, byte for byte.
///
/// A 90 degree V-bit and a 3 mm bevel with no start depth: the tip runs on the 40 x 30 rectangle
/// itself, unoffset, at `-(0 + 3 / tan(45)) = -3` mm, in **one** pass — rapid, one plunge, four
/// sides, retract to safe Z. CRLF, because the fixture post declares `LINE_ENDING = "[13][10]"`.
const GOLDEN_CHAMFER_PROGRAM: &str = "\
( Chamfer 1 )\r\n\
( CamPilot fixture Mach3 Arcs (mm) (*.nc) )\r\n\
N0G00G17G90G21\r\n\
N10T1M06\r\n\
N20S18000M03\r\n\
N30G00X0.0000Y0.0000\r\n\
N40G01Z-3.0000F100.0\r\n\
N50G01X40.0000\r\n\
N60G01Y30.0000\r\n\
N70G01X0.0000\r\n\
N80G01Y0.0000\r\n\
N90G00Z5.0000\r\n\
N100G00Z5.0000\r\n\
N110G00X0.0000Y0.0000\r\n\
N120M05\r\n\
N130M30\r\n";

#[test]
fn a_chamfer_authored_through_the_cli_emits_the_golden_program() {
  let directory = scratch("golden-chamfer");
  let document = write_vectors(
    &directory,
    "chamfer.c2d",
    vec![rectangle_contour(40.0, 30.0)],
  );
  let library = write_chamfer_library(&directory);
  let post = write_post(&directory);

  let added = run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "chamfer",
    "--entities",
    "E1",
    "--tool",
    "1",
    "--chamfer-width",
    "3",
    "--library",
    library.to_str().expect("utf8"),
  ]);
  assert_eq!(added.code, 0, "{}", added.stderr);
  assert_eq!(
    added.stdout, "added TP1 chamfer, tool T1, 1 entity(ies)\n",
    "{}",
    added.stdout
  );

  // What the document stores is the bevel and the bit, never a derived depth (D-M6-2 as amended in
  // L7): the angle is the library's own, in radians, and the depth is a question asked at emission.
  let loaded = cam_store::document::load_document(&document).expect("loads");
  match &loaded.toolpaths[0].operation {
    Operation::Chamfer(params) => {
      assert_eq!(params.chamfer_width, Length::from_mm(3.0));
      assert_eq!(params.included_angle, Angle::from_degrees(90.0));
      assert_eq!(params.start_depth, Length::ZERO);
      // `tan(pi/4)` is not exactly 1 in binary floating point, so the derived depth is 3 mm to
      // within rounding — and 3.0000 mm once the post's four decimals format it.
      assert!(
        (params.depth().mm() - 3.0).abs() < 1.0e-9,
        "a 90 degree bit and a 3 mm bevel derive 3 mm, got {}",
        params.depth().mm()
      );
    }
    other => panic!("expected a chamfer, got {other:?}"),
  }

  let program = emit_program(&directory, &document, &library, &post);
  assert_eq!(program, GOLDEN_CHAMFER_PROGRAM);
  // S6: the same document emits the same bytes on the next run.
  assert_eq!(
    emit_program(&directory, &document, &library, &post),
    program
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_chamfer_on_a_tool_without_an_included_angle_exits_two_naming_the_tool() {
  let directory = scratch("chamfer-no-angle");
  let document = write_vectors(&directory, "plain.c2d", vec![rectangle_contour(40.0, 30.0)]);
  // The ordinary library's T1 is an end mill: a diameter and no angle.
  let library = write_library(&directory);
  let before = std::fs::read(&document).expect("read");

  let outcome = run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "chamfer",
    "--entities",
    "E1",
    "--tool",
    "1",
    "--chamfer-width",
    "3",
    "--library",
    library.to_str().expect("utf8"),
  ]);
  assert_eq!(outcome.code, 2, "{}", outcome.stdout);
  assert!(
    outcome.stderr.contains("T1"),
    "the refusal names the tool: {}",
    outcome.stderr
  );
  assert!(
    outcome.stderr.contains("included angle"),
    "and says what is missing: {}",
    outcome.stderr
  );
  assert_eq!(
    std::fs::read(&document).expect("read"),
    before,
    "a refused toolpath writes nothing"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn emit_refuses_a_chamfer_whose_tool_has_no_included_angle_naming_the_tool() {
  // The other half of D-M6-9: a document authored elsewhere still carries the bevel, and the library
  // the machine is set up with has no angle for that tool. Defaulting 90 degrees would cut a
  // different bevel, so the program is refused by name and nothing is written.
  let directory = scratch("chamfer-emit-no-angle");
  let document = write_vectors(
    &directory,
    "chamfer.c2d",
    vec![rectangle_contour(40.0, 30.0)],
  );
  let library = write_library(&directory);
  let post = write_post(&directory);

  let mut loaded = cam_store::document::load_document(&document).expect("loads");
  loaded.toolpaths.push(Toolpath {
    id: "TP1".to_owned(),
    name: "Chamfer 1".to_owned(),
    entity_ids: vec!["E1".to_owned()],
    island_ids: Vec::new(),
    tool_number: 1,
    operation: Operation::Chamfer(ChamferParams {
      chamfer_width: Length::from_mm(3.0),
      included_angle: Angle::from_degrees(90.0),
      start_depth: Length::ZERO,
    }),
  });
  save_document(&document, &loaded).expect("document saved");

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
  assert_eq!(outcome.code, 2, "{}", outcome.stdout);
  assert!(
    outcome.stderr.contains("T1"),
    "the refusal names the tool: {}",
    outcome.stderr
  );
  assert!(!out.exists(), "a refused program writes nothing");
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_chamfer_takes_no_side_and_no_authored_depth() {
  // A bevel is symmetric about the vector and its depth is derived from the bit, so both flags have
  // no meaning here. Refusing them is the difference between a refusal and a silently different
  // bevel: a `--cut-depth 12` that nobody applied would still look like it had been.
  let directory = scratch("chamfer-flags");
  let library = write_chamfer_library(&directory);
  for flag in ["--side", "--cut-depth", "--pass-depth"] {
    let document = write_vectors(
      &directory,
      "chamfer.c2d",
      vec![rectangle_contour(40.0, 30.0)],
    );
    let before = std::fs::read(&document).expect("read");
    let outcome = run(&[
      "toolpath",
      document.to_str().expect("utf8"),
      "--kind",
      "chamfer",
      "--entities",
      "E1",
      "--tool",
      "1",
      "--chamfer-width",
      "3",
      flag,
      "6",
      "--library",
      library.to_str().expect("utf8"),
    ]);
    assert_eq!(outcome.code, 2, "{flag}: {}", outcome.stdout);
    assert!(
      outcome.stderr.contains(flag),
      "the refusal names the flag: {}",
      outcome.stderr
    );
    assert_eq!(
      std::fs::read(&document).expect("read"),
      before,
      "{flag}: nothing is written"
    );
  }
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_chamfer_without_a_width_exits_two_naming_the_option() {
  let directory = scratch("chamfer-no-width");
  let document = write_vectors(
    &directory,
    "chamfer.c2d",
    vec![rectangle_contour(40.0, 30.0)],
  );
  let library = write_chamfer_library(&directory);
  let outcome = run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "chamfer",
    "--entities",
    "E1",
    "--tool",
    "1",
    "--library",
    library.to_str().expect("utf8"),
  ]);
  assert_eq!(outcome.code, 2, "{}", outcome.stdout);
  assert!(
    outcome.stderr.contains("--chamfer-width"),
    "{}",
    outcome.stderr
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_chamfer_with_no_width_or_an_impossible_angle_exits_two_naming_the_value() {
  // The depth is `width / tan(half angle)`, so a zero width has no bevel and an angle at either end
  // of `(0, 180)` degrees has no finite tangent. Both are input errors that name the value, never a
  // clamp: a clamped bevel is a different bevel on someone's part.
  let directory = scratch("chamfer-invalid");
  let document = write_vectors(
    &directory,
    "chamfer.c2d",
    vec![rectangle_contour(40.0, 30.0)],
  );
  let library = write_chamfer_library(&directory);

  let zero = run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "chamfer",
    "--entities",
    "E1",
    "--tool",
    "1",
    "--chamfer-width",
    "0",
    "--library",
    library.to_str().expect("utf8"),
  ]);
  assert_eq!(zero.code, 2, "{}", zero.stdout);
  assert!(
    zero.stderr.contains("chamfer width") && zero.stderr.contains("got 0 mm"),
    "the refusal names the value: {}",
    zero.stderr
  );

  // The same rule holds for the angle, and it is the document model that holds it: a library bit
  // whose included angle is 0 degrees has no tangent to derive from, so authoring a chamfer with it
  // is refused naming the value. (Such a document cannot even be written — saving validates it — so
  // this is the only path that reaches the rule.)
  let flat_bit = write_chamfer_library_at(&directory, "zero-angle.sqlite", 0.0, 0.0);
  let impossible = run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "chamfer",
    "--entities",
    "E1",
    "--tool",
    "1",
    "--chamfer-width",
    "3",
    "--library",
    flat_bit.to_str().expect("utf8"),
  ]);
  assert_eq!(impossible.code, 2, "{}", impossible.stdout);
  assert!(
    impossible.stderr.contains("included angle") && impossible.stderr.contains("got 0 degrees"),
    "the refusal names the value: {}",
    impossible.stderr
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_chamfer_simulates_with_no_gouge_and_no_uncut() {
  // A8's end-to-end claim: the shape the library describes reaches `cam-sim` through the segment, so
  // the cone the program is simulated with is the cone the document derived its depth from. A
  // cylinder in the segment would not merely disagree — `cam-sim` refuses a chamfer claimed with one
  // — and an angle that reached the simulation differently from the document would report a correct
  // bevel as gouge or uncut. Both directions are visible here, at the binary's own tolerance.
  let directory = scratch("chamfer-simulate");
  let document = write_vectors(
    &directory,
    "chamfer.c2d",
    vec![rectangle_contour(40.0, 30.0)],
  );
  let library = write_chamfer_library(&directory);

  let added = run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "chamfer",
    "--entities",
    "E1",
    "--tool",
    "1",
    "--chamfer-width",
    "3",
    "--start-depth",
    "1",
    "--library",
    library.to_str().expect("utf8"),
  ]);
  assert_eq!(added.code, 0, "{}", added.stderr);

  let outcome = run(&[
    "simulate",
    document.to_str().expect("utf8"),
    "--library",
    library.to_str().expect("utf8"),
  ]);
  assert_eq!(outcome.code, 0, "{}", outcome.stderr);
  assert!(
    outcome.stdout.contains("gouge: 0.000 mm3 over 0 cell(s)"),
    "{}",
    outcome.stdout
  );
  assert!(
    outcome.stdout.contains("uncut: 0.000 mm3 over 0 cell(s)"),
    "{}",
    outcome.stdout
  );
  assert!(outcome.stdout.contains("crash: none"), "{}", outcome.stdout);
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_chamfer_on_a_flat_tipped_bit_exits_two_naming_the_tool() {
  // D-M6-10: a flat of diameter `f` widens the bevel to `w + f/2`, and `ChamferParams` reads only the
  // document, so the depth it derives cannot honour the width the user typed on a flat-tipped bit. The
  // bit is refused by name at authoring time rather than cutting a bevel nobody asked for.
  let directory = scratch("chamfer-flat-tip");
  let document = write_vectors(
    &directory,
    "chamfer.c2d",
    vec![rectangle_contour(40.0, 30.0)],
  );
  let library = write_chamfer_library_at(&directory, "flat.sqlite", 90.0, 2.0);
  let before = std::fs::read(&document).expect("read");

  let outcome = run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "chamfer",
    "--entities",
    "E1",
    "--tool",
    "1",
    "--chamfer-width",
    "3",
    "--library",
    library.to_str().expect("utf8"),
  ]);
  assert_eq!(outcome.code, 2, "{}", outcome.stdout);
  assert!(
    outcome.stderr.contains("T1"),
    "the refusal names the tool: {}",
    outcome.stderr
  );
  assert!(
    outcome.stderr.contains("flat"),
    "and says what disqualifies it: {}",
    outcome.stderr
  );
  assert_eq!(
    std::fs::read(&document).expect("read"),
    before,
    "a refused toolpath writes nothing"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn emit_refuses_a_chamfer_whose_tool_has_a_flat_tip_naming_the_tool() {
  // The third boundary that can see the bit (D-M6-10), and the one that costs the part if it is
  // missed: a document authored against a sharp bit, then emitted against a library whose T1 has
  // picked up a flat. The flat widens the bevel to `w + f/2` while the document's derived depth knows
  // nothing of it, so the program must not be written at all — exit 2 naming the tool, no output file.
  let directory = scratch("chamfer-emit-flat-tip");
  let document = write_vectors(
    &directory,
    "chamfer.c2d",
    vec![rectangle_contour(40.0, 30.0)],
  );
  let library = write_chamfer_library(&directory);
  let post = write_post(&directory);

  // Authored while the bit is still sharp.
  let added = run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "chamfer",
    "--entities",
    "E1",
    "--tool",
    "1",
    "--chamfer-width",
    "3",
    "--library",
    library.to_str().expect("utf8"),
  ]);
  assert_eq!(added.code, 0, "{}", added.stderr);

  // The same tool number, now described with a 2 mm flat at the tip.
  let tools = ToolLibrary::open(&library).expect("library");
  tools
    .put_geometry(&ToolGeometry {
      id: "V1".to_owned(),
      name_format: "{Tool Type} ({Diameter|F}{Units Short})".to_owned(),
      tool_type: 0,
      units: UnitSystem::Mm,
      diameter: Some(Length::from_mm(12.7)),
      included_angle: Some(Angle::from_degrees(90.0)),
      flat_diameter: Some(Length::from_mm(2.0)),
      num_flutes: Some(2),
      flute_length: None,
      tip_radius: None,
      custom_attributes: None,
    })
    .expect("geometry");
  drop(tools);

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
  assert_eq!(outcome.code, 2, "{}", outcome.stdout);
  assert!(
    outcome.stderr.contains("T1"),
    "the refusal names the tool: {}",
    outcome.stderr
  );
  assert!(
    outcome.stderr.contains("flat tip"),
    "and says what disqualifies it: {}",
    outcome.stderr
  );
  assert!(!out.exists(), "a refused program writes nothing");
  std::fs::remove_dir_all(&directory).ok();
}

// --- quick engrave (M6 A3, D-M6-4) -----------------------------------------------------------

/// The whole program one engrave emits, byte for byte.
///
/// A 90 degree V-bit and a 2 mm engrave with no start depth: the tool centre runs on the 40 x 30
/// rectangle itself, unoffset, at `-2` mm, in **one** pass — rapid, one plunge, four sides, retract to
/// safe Z. Every line is the chamfer golden's, with this strategy's own Z and name: an engrave
/// replaces the pass law and the bevel derivation, not the walk. CRLF, because the fixture post
/// declares `LINE_ENDING = "[13][10]"`.
const GOLDEN_ENGRAVE_PROGRAM: &str = "\
( Engrave 1 )\r\n\
( CamPilot fixture Mach3 Arcs (mm) (*.nc) )\r\n\
N0G00G17G90G21\r\n\
N10T1M06\r\n\
N20S18000M03\r\n\
N30G00X0.0000Y0.0000\r\n\
N40G01Z-2.0000F100.0\r\n\
N50G01X40.0000\r\n\
N60G01Y30.0000\r\n\
N70G01X0.0000\r\n\
N80G01Y0.0000\r\n\
N90G00Z5.0000\r\n\
N100G00Z5.0000\r\n\
N110G00X0.0000Y0.0000\r\n\
N120M05\r\n\
N130M30\r\n";

#[test]
fn an_engrave_authored_through_the_cli_emits_the_golden_program() {
  let directory = scratch("golden-engrave");
  let document = write_vectors(
    &directory,
    "engrave.c2d",
    vec![rectangle_contour(40.0, 30.0)],
  );
  let library = write_chamfer_library(&directory);
  let post = write_post(&directory);

  let added = run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "engrave",
    "--entities",
    "E1",
    "--tool",
    "1",
    "--cut-depth",
    "2",
    "--library",
    library.to_str().expect("utf8"),
  ]);
  assert_eq!(added.code, 0, "{}", added.stderr);
  assert_eq!(
    added.stdout, "added TP1 engrave, tool T1, 1 entity(ies)\n",
    "{}",
    added.stdout
  );

  // What the document stores is the authored depth and the start it begins from — nothing derived.
  let loaded = cam_store::document::load_document(&document).expect("loads");
  match &loaded.toolpaths[0].operation {
    Operation::Engrave(params) => {
      assert_eq!(params.start_depth, Length::ZERO);
      assert_eq!(params.cut_depth, Length::from_mm(2.0));
    }
    other => panic!("expected an engrave, got {other:?}"),
  }

  let program = emit_program(&directory, &document, &library, &post);
  assert_eq!(program, GOLDEN_ENGRAVE_PROGRAM);
  // S6: the same document emits the same bytes on the next run.
  assert_eq!(
    emit_program(&directory, &document, &library, &post),
    program
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn an_engrave_takes_no_side_no_pass_depth_no_tabs_and_no_leads() {
  // An engrave is one pass at one depth with the tool centre on the vector (D-M6-4), so a side, a
  // pass law, a tab and a lead each have no meaning here. Refusing them by name is the difference
  // between a refusal and a silently different groove: a `--pass-depth 2` that nobody applied would
  // still look like it had been.
  let directory = scratch("engrave-flags");
  let library = write_chamfer_library(&directory);
  for (flag, value) in [
    ("--side", "on"),
    ("--pass-depth", "2"),
    ("--tab", "0.5:2:1"),
    ("--lead-in", "3"),
    ("--lead-out", "3"),
  ] {
    let document = write_vectors(
      &directory,
      "engrave.c2d",
      vec![rectangle_contour(40.0, 30.0)],
    );
    let before = std::fs::read(&document).expect("read");
    let outcome = run(&[
      "toolpath",
      document.to_str().expect("utf8"),
      "--kind",
      "engrave",
      "--entities",
      "E1",
      "--tool",
      "1",
      "--cut-depth",
      "2",
      flag,
      value,
      "--library",
      library.to_str().expect("utf8"),
    ]);
    assert_eq!(outcome.code, 2, "{flag}: {}", outcome.stdout);
    assert!(
      outcome.stderr.contains(flag),
      "the refusal names the flag: {}",
      outcome.stderr
    );
    assert_eq!(
      std::fs::read(&document).expect("read"),
      before,
      "{flag}: nothing is written"
    );
  }
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn an_engrave_whose_cut_depth_is_not_below_its_start_depth_exits_two_naming_both() {
  // The profile's own rule, inherited through D-M6-4: the deepest pass lands on `cut_depth`, so a
  // document whose cut depth is not below its start depth is refused as an input error that names
  // both numbers rather than emitting a program that cuts nothing.
  let directory = scratch("engrave-depths");
  let document = write_vectors(
    &directory,
    "engrave.c2d",
    vec![rectangle_contour(40.0, 30.0)],
  );
  let library = write_chamfer_library(&directory);
  let post = write_post(&directory);

  let added = run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "engrave",
    "--entities",
    "E1",
    "--tool",
    "1",
    "--cut-depth",
    "4",
    "--start-depth",
    "4",
    "--library",
    library.to_str().expect("utf8"),
  ]);
  assert_eq!(
    added.code, 0,
    "authoring stores what was asked: {}",
    added.stderr
  );

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
  assert_eq!(outcome.code, 2, "{}", outcome.stdout);
  assert!(
    outcome.stderr.contains("cut depth 4 mm") && outcome.stderr.contains("start depth 4 mm"),
    "the refusal names both depths: {}",
    outcome.stderr
  );
  assert!(!out.exists(), "a refused program writes nothing");
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn an_engrave_simulates_with_no_gouge_and_no_uncut() {
  // A8's end-to-end claim for the third strategy: the V-bit the library describes reaches `cam-sim`
  // through the segment, so the groove the program is simulated with is the groove its tool cuts. A
  // segment that carried a cylinder instead would remove a flat slot the target does not authorise,
  // and the report would show it.
  let directory = scratch("engrave-simulate");
  let document = write_vectors(
    &directory,
    "engrave.c2d",
    vec![rectangle_contour(40.0, 30.0)],
  );
  let library = write_chamfer_library(&directory);

  let added = run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "engrave",
    "--entities",
    "E1",
    "--tool",
    "1",
    "--cut-depth",
    "2",
    "--library",
    library.to_str().expect("utf8"),
  ]);
  assert_eq!(added.code, 0, "{}", added.stderr);

  let outcome = run(&[
    "simulate",
    document.to_str().expect("utf8"),
    "--library",
    library.to_str().expect("utf8"),
  ]);
  assert_eq!(outcome.code, 0, "{}", outcome.stderr);
  assert!(
    outcome.stdout.contains("gouge: 0.000 mm3 over 0 cell(s)"),
    "{}",
    outcome.stdout
  );
  assert!(
    outcome.stdout.contains("uncut: 0.000 mm3 over 0 cell(s)"),
    "{}",
    outcome.stdout
  );
  std::fs::remove_dir_all(&directory).ok();
}

// --- fluting (M6 A4, A9, D-M6-5) ---------------------------------------------------------------

/// The whole program one ramped flute emits, byte for byte.
///
/// A 6 mm end mill and a 0.5 mm flute ramped over 10 mm on a 40 x 30 rectangle (140 mm of perimeter).
/// The entry and the exit are real moves — the machine traverses them — and **each is one move**: a
/// single feed of 10 mm of travel whose Z descends from the surface to `-0.5` along the bottom edge, the
/// plateau around the rest of the loop at `-0.5`, and the mirror move from `(0, 10)` back up to
/// `(0, 0)`. The ramp used to be walked in steps of a tenth of a millimetre of Z because the simulation
/// removed a move's *destination* depth over its whole travel; `cam-sim` now interpolates Z along a
/// move (D-M6-12, which amends D-M5-3), so the staircase is gone and the program says what the machine
/// does. CRLF, because the fixture post declares `LINE_ENDING = "[13][10]"`.
const GOLDEN_FLUTING_PROGRAM: &str = "\
( Fluting 1 )\r\n\
( CamPilot fixture Mach3 Arcs (mm) (*.nc) )\r\n\
N0G00G17G90G21\r\n\
N10T1M06\r\n\
N20S18000M03\r\n\
N30G00X0.0000Y0.0000\r\n\
N40G01Z0.0000F100.0\r\n\
N50G01X10.0000Z-0.5000\r\n\
N60G01X40.0000\r\n\
N70G01Y30.0000\r\n\
N80G01X0.0000\r\n\
N90G01Y10.0000\r\n\
N100G01Y0.0000Z0.0000\r\n\
N110G00Z5.0000\r\n\
N120G00Z5.0000\r\n\
N130G00X0.0000Y0.0000\r\n\
N140M05\r\n\
N150M30\r\n";

/// Author a ramped flute on `E1` through the real `toolpath` command.
fn author_fluting(document: &Path, library: &Path, ramp: f64, cut_depth: f64) -> Outcome {
  let ramp = ramp.to_string();
  let cut_depth = cut_depth.to_string();
  run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "fluting",
    "--entities",
    "E1",
    "--tool",
    "1",
    "--ramp",
    &ramp,
    "--cut-depth",
    &cut_depth,
    "--library",
    library.to_str().expect("utf8"),
  ])
}

#[test]
fn a_ramped_flute_authored_through_the_cli_emits_the_golden_program() {
  let directory = scratch("golden-fluting");
  let document = write_vectors(
    &directory,
    "fluting.c2d",
    vec![rectangle_contour(40.0, 30.0)],
  );
  let library = write_library(&directory);
  let post = write_post(&directory);

  let added = author_fluting(&document, &library, 10.0, 0.5);
  assert_eq!(added.code, 0, "{}", added.stderr);
  assert_eq!(
    added.stdout, "added TP1 fluting, tool T1, 1 entity(ies)\n",
    "{}",
    added.stdout
  );

  // What the document stores is the authored depth and the ramp; nothing is derived from the tool.
  let loaded = cam_store::document::load_document(&document).expect("loads");
  match &loaded.toolpaths[0].operation {
    Operation::Fluting(params) => {
      assert_eq!(params.start_depth, Length::ZERO);
      assert_eq!(params.cut_depth, Length::from_mm(0.5));
      assert_eq!(params.ramp, Length::from_mm(10.0));
    }
    other => panic!("expected a fluting toolpath, got {other:?}"),
  }

  let program = emit_program(&directory, &document, &library, &post);
  assert_eq!(program, GOLDEN_FLUTING_PROGRAM);
  // S6: the same document emits the same bytes on the next run.
  assert_eq!(
    emit_program(&directory, &document, &library, &post),
    program
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_flute_takes_no_side_no_stepover_no_pass_depth_no_tabs_no_leads_and_no_allowance() {
  // D-M6-5: a flute is one pass along the vector with its centre on it, so every flag a profile or a
  // pocket would consume has no meaning here. Refusing them by name is the difference between a
  // refusal and a silently different groove: a `--stepover` nobody applied still looks applied.
  let directory = scratch("fluting-flags");
  let library = write_library(&directory);
  for (flag, value) in [
    ("--side", "on"),
    ("--pass-depth", "0.2"),
    ("--tab", "0.5:2:1"),
    ("--lead-in", "3"),
    ("--lead-out", "3"),
    ("--stepover", "2"),
    ("--allowance", "1"),
  ] {
    let document = write_vectors(
      &directory,
      "fluting.c2d",
      vec![rectangle_contour(40.0, 30.0)],
    );
    let before = std::fs::read(&document).expect("read");
    let outcome = run(&[
      "toolpath",
      document.to_str().expect("utf8"),
      "--kind",
      "fluting",
      "--entities",
      "E1",
      "--tool",
      "1",
      "--ramp",
      "10",
      flag,
      value,
      "--library",
      library.to_str().expect("utf8"),
    ]);
    assert_eq!(outcome.code, 2, "{flag}: {}", outcome.stdout);
    assert!(
      outcome.stderr.contains(flag),
      "the refusal names the flag: {}",
      outcome.stderr
    );
    assert_eq!(
      std::fs::read(&document).expect("read"),
      before,
      "{flag}: nothing is written"
    );
  }
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_flute_whose_ramp_is_longer_than_half_the_vector_exits_two_naming_both_numbers() {
  // A4: the two ramps have to fit on the vector. Authoring succeeds — half of a 140 mm perimeter is
  // 70 mm — and emitting refuses the 71 mm one by name, because a silently shortened ramp would cut
  // the ends at a depth nobody authored.
  let directory = scratch("fluting-ramp");
  let document = write_vectors(
    &directory,
    "fluting.c2d",
    vec![rectangle_contour(40.0, 30.0)],
  );
  let library = write_library(&directory);
  let post = write_post(&directory);

  let added = author_fluting(&document, &library, 71.0, 0.5);
  assert_eq!(
    added.code, 0,
    "authoring stores what was asked: {}",
    added.stderr
  );

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
  assert_eq!(outcome.code, 2, "{}", outcome.stdout);
  assert!(
    outcome.stderr.contains("71") && outcome.stderr.contains("140"),
    "the refusal names the ramp and the vector's length: {}",
    outcome.stderr
  );
  assert!(!out.exists(), "a refused program writes nothing");
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_ramped_flute_simulates_with_no_gouge_and_no_uncut() {
  // The end-to-end claim A4 rests on: the target authorises the ramp law *along the path*, and the
  // program cuts that law. A target that claimed the full depth over the whole band would report both
  // ramps as uncut; a program that ramped in one move would be reported as a gouge. Neither number is
  // tolerance-softened: both are zero cells.
  let directory = scratch("fluting-simulate");
  let document = write_vectors(
    &directory,
    "fluting.c2d",
    vec![rectangle_contour(40.0, 30.0)],
  );
  let library = write_library(&directory);

  let added = author_fluting(&document, &library, 10.0, 0.5);
  assert_eq!(added.code, 0, "{}", added.stderr);

  let outcome = simulate(&document, &library, &[]);
  assert_eq!(outcome.code, 0, "{}{}", outcome.stdout, outcome.stderr);
  // The program is the ramped one and not an empty segment: "no deviation" from a program that cut
  // nothing would say nothing at all. Nine moves: the rapid, the plunge to the surface, the ramp in,
  // the four plateau sides, the ramp out and the retract.
  assert!(
    report_volume(&outcome.stdout, "program:") > 8.0,
    "the ramps are real moves: {}",
    outcome.stdout
  );
  assert!(
    report_volume(&outcome.stdout, "removed:") > 0.0,
    "and they remove material: {}",
    outcome.stdout
  );
  assert!(
    outcome.stdout.contains("gouge: 0.000 mm3 over 0 cell(s)"),
    "{}",
    outcome.stdout
  );
  assert!(
    outcome.stdout.contains("uncut: 0.000 mm3 over 0 cell(s)"),
    "{}",
    outcome.stdout
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_fluting_document_written_without_a_ramp_still_loads_and_plays_as_the_plain_groove() {
  // A1's compatibility rule, for the one field this strategy defaults: a document that names a
  // `fluting` toolpath and no `ramp` — hand-written, or written by a build that predates the field —
  // must load rather than fail, and it must describe what its absence means, the in-place plunge. The
  // same rectangle is authored twice, once with `--ramp 10` and its field removed afterwards, and once
  // with `--ramp 0`; the two programs are then the same bytes.
  let directory = scratch("fluting-no-ramp");
  let stripped = write_vectors(
    &directory,
    "stripped.c2d",
    vec![rectangle_contour(40.0, 30.0)],
  );
  let authored = write_vectors(
    &directory,
    "authored.c2d",
    vec![rectangle_contour(40.0, 30.0)],
  );
  let library = write_library(&directory);
  let post = write_post(&directory);

  let added = author_fluting(&stripped, &library, 10.0, 0.5);
  assert_eq!(added.code, 0, "{}", added.stderr);
  remove_fluting_ramp(&stripped);

  let validated = run(&["validate", stripped.to_str().expect("utf8")]);
  assert_eq!(validated.code, 0, "{}", validated.stderr);

  let added = author_fluting(&authored, &library, 0.0, 0.5);
  assert_eq!(added.code, 0, "{}", added.stderr);
  assert_eq!(
    emit_program(&directory, &stripped, &library, &post),
    emit_program(&directory, &authored, &library, &post),
    "a document without a ramp is the plain groove: plunge, cut, retract"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_six_millimetre_flute_simulates_with_no_gouge_and_no_uncut() {
  // The graded case, at the judgement M5 fixed: an eighth of a cell, with nothing added for the ramp.
  // The sweep pays for the ramp where it belongs — in its own sampling, which takes enough steps that
  // the depth residual between two of them is inside that eighth (D-M6-12) — so a deep flute reports its
  // ramps as cut. The thin fixture next door would pass at any sampling; this one is what shows the
  // residual is resolved rather than tolerated.
  let directory = scratch("fluting-simulate-deep");
  let document = write_vectors(
    &directory,
    "fluting.c2d",
    vec![rectangle_contour(40.0, 30.0)],
  );
  let library = write_library(&directory);

  let added = author_fluting(&document, &library, 10.0, 6.0);
  assert_eq!(added.code, 0, "{}", added.stderr);

  let outcome = simulate(&document, &library, &[]);
  assert_eq!(outcome.code, 0, "{}{}", outcome.stdout, outcome.stderr);
  assert!(
    outcome.stdout.contains("gouge: 0.000 mm3 over 0 cell(s)"),
    "{}",
    outcome.stdout
  );
  assert!(
    outcome.stdout.contains("uncut: 0.000 mm3 over 0 cell(s)"),
    "{}",
    outcome.stdout
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_six_millimetre_flute_over_a_one_millimetre_ramp_simulates_with_no_gouge_and_no_uncut() {
  // The steepest ramp a flute can carry at this size: 6 mm of Z over 1 mm of travel, a grade of 6. A
  // document-global tolerance keyed on the grade would have licensed 3.125 mm of deviation anywhere in
  // this document, which would have hidden a millimetre of gouge on the plateau as well as on the ramp.
  // The sampling settles it instead — a move crossing that much Z takes 96 steps at a 1 mm cell — so the
  // judgement stays at an eighth of a cell and this document is still clean.
  let directory = scratch("fluting-simulate-steep");
  let document = write_vectors(
    &directory,
    "fluting.c2d",
    vec![rectangle_contour(40.0, 30.0)],
  );
  let library = write_library(&directory);

  let added = author_fluting(&document, &library, 1.0, 6.0);
  assert_eq!(added.code, 0, "{}", added.stderr);

  let outcome = simulate(&document, &library, &[]);
  assert_eq!(outcome.code, 0, "{}{}", outcome.stdout, outcome.stderr);
  assert!(
    outcome.stdout.contains("gouge: 0.000 mm3 over 0 cell(s)"),
    "{}",
    outcome.stdout
  );
  assert!(
    outcome.stdout.contains("uncut: 0.000 mm3 over 0 cell(s)"),
    "{}",
    outcome.stdout
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_ramped_flute_on_an_arc_is_authored_as_a_helix_and_simulates_clean() {
  // The defect T4 refused and T7 lifts, measured before the refusal and again now: an imported circle of
  // radius 25 fluted 6 mm deep with a 10 mm ramp used to be cut as a chord — `gouge 33.228 mm3 over 22
  // cell(s), uncut 30.093 mm3 over 34 cell(s)`, exit 6 — because a ramped move could not ride on
  // `Move::Arc`. With `Move::Helix` the ramp rides the arc itself, so the program is clean at the
  // judgement M5 fixed: an eighth of a cell.
  let directory = scratch("fluting-arc-ramp");
  let document = write_vectors(&directory, "circle.c2d", vec![circle_contour(25.0)]);
  let library = write_library(&directory);
  let post = write_post(&directory);

  let added = author_fluting(&document, &library, 10.0, 6.0);
  assert_eq!(added.code, 0, "{}{}", added.stdout, added.stderr);

  let program = emit_program(&directory, &document, &library, &post);
  let helix_lines = program
    .lines()
    .filter(|line| (line.contains("G02") || line.contains("G03")) && line.contains('Z'))
    .count();
  assert_eq!(
    helix_lines, 2,
    "the ramped entry and the ramped exit are the program's only helixes:\n{program}"
  );
  assert!(
    program.contains("G02") || program.contains("G03"),
    "and the plateau is still cut along the circle's arcs:\n{program}"
  );

  let outcome = simulate(&document, &library, &[]);
  assert_eq!(outcome.code, 0, "{}{}", outcome.stdout, outcome.stderr);
  assert!(
    outcome.stdout.contains("gouge: 0.000 mm3 over 0 cell(s)"),
    "{}",
    outcome.stdout
  );
  assert!(
    outcome.stdout.contains("uncut: 0.000 mm3 over 0 cell(s)"),
    "{}",
    outcome.stdout
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_document_that_already_carries_a_ramped_flute_on_an_arc_emits_a_helical_program() {
  // A document written before T7's lift (or by hand) is not refused any more: the strategy emits the
  // same helix through `emit` and `simulate` that the command now authors. This is the fixture that
  // used to exit 2 at both doors, and before T4's refusal, exit 6 with tens of cubic millimetres of
  // gouge and uncut material.
  let directory = scratch("fluting-arc-ramp-document");
  let document = write_vectors(&directory, "circle.c2d", vec![circle_contour(25.0)]);
  let library = write_library(&directory);
  let post = write_post(&directory);
  let mut loaded = cam_store::document::load_document(&document).expect("loads");
  loaded.toolpaths.push(Toolpath {
    id: "TP1".to_owned(),
    name: "Fluting 1".to_owned(),
    entity_ids: vec!["E1".to_owned()],
    island_ids: Vec::new(),
    tool_number: 1,
    operation: Operation::Fluting(FlutingParams {
      start_depth: Length::ZERO,
      cut_depth: Length::from_mm(6.0),
      ramp: Length::from_mm(10.0),
    }),
  });
  save_document(&document, &loaded).expect("saved");

  let program = emit_program(&directory, &document, &library, &post);
  assert!(
    program
      .lines()
      .any(|line| (line.contains("G02") || line.contains("G03")) && line.contains('Z')),
    "the ramps reach the program as helical moves:\n{program}"
  );

  let simulated = simulate(&document, &library, &[]);
  assert_eq!(
    simulated.code, 0,
    "{}{}",
    simulated.stdout, simulated.stderr
  );
  assert!(
    simulated.stdout.contains("gouge: 0.000 mm3 over 0 cell(s)")
      && simulated.stdout.contains("uncut: 0.000 mm3 over 0 cell(s)"),
    "{}",
    simulated.stdout
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_flute_without_a_ramp_still_cuts_an_arc_as_an_arc_through_the_cli() {
  // The other half of the lifted refusal: `ramp = 0` on the same arc contour is exactly what `Move::Arc`
  // for, so it is authored, emitted with arc blocks and simulated clean. Refusing it would have been a
  // regression dressed as safety.
  let directory = scratch("fluting-arc-flat");
  let document = write_vectors(&directory, "circle.c2d", vec![circle_contour(15.0)]);
  let library = write_library(&directory);
  let post = write_post(&directory);

  let added = author_fluting(&document, &library, 0.0, 6.0);
  assert_eq!(added.code, 0, "{}", added.stderr);

  let program = emit_program(&directory, &document, &library, &post);
  assert!(
    program.contains("G02") || program.contains("G03"),
    "the circle's arcs reach the program as arcs:\n{program}"
  );

  let outcome = simulate(&document, &library, &[]);
  assert_eq!(outcome.code, 0, "{}{}", outcome.stdout, outcome.stderr);
  assert!(
    outcome.stdout.contains("gouge: 0.000 mm3 over 0 cell(s)")
      && outcome.stdout.contains("uncut: 0.000 mm3 over 0 cell(s)"),
    "{}",
    outcome.stdout
  );
  std::fs::remove_dir_all(&directory).ok();
}

// --- texture (M6 A5, A9, D-M6-6) ---------------------------------------------------------------

/// The whole program one textured square emits, byte for byte.
///
/// A 6 mm end mill and a 0.5 mm texture at a 10 mm spacing and 0 degrees on a 40 x 30 mm square. The
/// pattern is the rows half a spacing inside the region — `y = 5, 15, 25` — and each is **one pass**:
/// a rapid over its first vertex at safe Z, a plunge in place to `-0.5`, one feed across the square, and
/// a retract to safe Z before the next groove starts (D-M4-6, D-M6-6). Twelve moves, and no move that
/// clears the material *between* the grooves: that is what a texture is, and the three bands it leaves
/// are the pattern.
///
/// CRLF, because the fixture post declares `LINE_ENDING = "[13][10]"`.
const GOLDEN_TEXTURE_PROGRAM: &str = "\
( Texture 1 )\r\n\
( CamPilot fixture Mach3 Arcs (mm) (*.nc) )\r\n\
N0G00G17G90G21\r\n\
N10T1M06\r\n\
N20S18000M03\r\n\
N30G00X0.0000Y5.0000\r\n\
N40G01Z-0.5000F100.0\r\n\
N50G01X40.0000\r\n\
N60G00Z5.0000\r\n\
N70G00X0.0000Y15.0000\r\n\
N80G01Z-0.5000\r\n\
N90G01X40.0000\r\n\
N100G00Z5.0000\r\n\
N110G00X0.0000Y25.0000\r\n\
N120G01Z-0.5000\r\n\
N130G01X40.0000\r\n\
N140G00Z5.0000\r\n\
N150G00Z5.0000\r\n\
N160G00X0.0000Y0.0000\r\n\
N170M05\r\n\
N180M30\r\n";

/// Author a texture on `E1` through the real `toolpath` command.
fn author_texture(
  document: &Path,
  library: &Path,
  spacing: &str,
  degrees: &str,
  cut_depth: f64,
) -> Outcome {
  let cut_depth = cut_depth.to_string();
  run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "texture",
    "--entities",
    "E1",
    "--tool",
    "1",
    "--spacing",
    spacing,
    "--angle",
    degrees,
    "--cut-depth",
    &cut_depth,
    "--library",
    library.to_str().expect("utf8"),
  ])
}

#[test]
fn a_texture_authored_through_the_cli_emits_the_golden_program() {
  let directory = scratch("golden-texture");
  let document = write_vectors(
    &directory,
    "texture.c2d",
    vec![rectangle_contour(40.0, 30.0)],
  );
  let library = write_library(&directory);
  let post = write_post(&directory);

  let added = author_texture(&document, &library, "10", "0", 0.5);
  assert_eq!(added.code, 0, "{}", added.stderr);
  assert_eq!(
    added.stdout, "added TP1 texture (10 mm at 0 degrees), tool T1, 1 entity(ies)\n",
    "{}",
    added.stdout
  );

  // What the document stores is the pattern's two parameters and the one depth; nothing is derived
  // from the tool.
  let loaded = cam_store::document::load_document(&document).expect("loads");
  match &loaded.toolpaths[0].operation {
    Operation::Texture(params) => {
      assert_eq!(params.start_depth, Length::ZERO);
      assert_eq!(params.cut_depth, Length::from_mm(0.5));
      assert_eq!(params.spacing, Length::from_mm(10.0));
      assert_eq!(params.angle, Angle::ZERO);
    }
    other => panic!("expected a texture toolpath, got {other:?}"),
  }

  let program = emit_program(&directory, &document, &library, &post);
  assert_eq!(program, GOLDEN_TEXTURE_PROGRAM);
  // S6: the same document emits the same bytes on the next run.
  assert_eq!(
    emit_program(&directory, &document, &library, &post),
    program
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_texture_takes_no_side_no_pass_depth_no_tabs_no_leads_no_allowance_no_stepover_and_no_ramp() {
  // D-M6-6: a texture cuts grooves at a spacing and leaves the material between them standing, so every
  // flag a profile, a pocket or a flute would consume has no meaning here. Refusing them by name is the
  // difference between a refusal and a silently different pattern.
  let directory = scratch("texture-flags");
  let library = write_library(&directory);
  for (flag, value) in [
    ("--side", "on"),
    ("--pass-depth", "0.2"),
    ("--tab", "0.5:2:1"),
    ("--lead-in", "3"),
    ("--lead-out", "3"),
    ("--allowance", "1"),
    ("--stepover", "2"),
    ("--ramp", "2"),
  ] {
    let document = write_vectors(
      &directory,
      "texture.c2d",
      vec![rectangle_contour(40.0, 30.0)],
    );
    let before = std::fs::read(&document).expect("read");
    let outcome = run(&[
      "toolpath",
      document.to_str().expect("utf8"),
      "--kind",
      "texture",
      "--entities",
      "E1",
      "--tool",
      "1",
      "--spacing",
      "10",
      flag,
      value,
      "--library",
      library.to_str().expect("utf8"),
    ]);
    assert_eq!(outcome.code, 2, "{flag}: {}", outcome.stdout);
    assert!(
      outcome.stderr.contains(flag),
      "the refusal names the flag: {}",
      outcome.stderr
    );
    assert!(
      outcome.stderr.contains("between them standing") || outcome.stderr.contains("--spacing"),
      "and says what a texture is instead: {}",
      outcome.stderr
    );
    assert_eq!(
      std::fs::read(&document).expect("read"),
      before,
      "{flag}: nothing is written"
    );
  }
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_texture_without_a_spacing_anywhere_exits_two_naming_the_option() {
  // `--spacing` falls back to the library's stepover the way a pocket's `--stepover` does, and this
  // library has none: the absence is an input error naming the flag, never a defaulted lattice. The
  // fixture's tool is a 6 mm end mill with no stepover in its cutting data.
  let directory = scratch("texture-nospacing");
  let document = write_vectors(
    &directory,
    "texture.c2d",
    vec![rectangle_contour(40.0, 30.0)],
  );
  let library = write_library(&directory);
  let before = std::fs::read(&document).expect("read");

  let outcome = run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "texture",
    "--entities",
    "E1",
    "--tool",
    "1",
    "--library",
    library.to_str().expect("utf8"),
  ]);
  assert_eq!(outcome.code, 2, "{}", outcome.stdout);
  assert!(
    outcome.stderr.contains("--spacing"),
    "the refusal names the missing input: {}",
    outcome.stderr
  );
  assert_eq!(
    std::fs::read(&document).expect("read"),
    before,
    "nothing is written"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_texture_takes_its_spacing_from_the_tool_librarys_stepover() {
  // The pocket precedent, applied to the pattern: `--spacing` absent, the library's `stepover` present,
  // and the document stores the library's number. The library's stepover is 2.4 mm.
  let directory = scratch("texture-library-spacing");
  let library = write_library_with_pocket_rates(&directory);
  let document = write_vectors(
    &directory,
    "texture.c2d",
    vec![rectangle_contour(40.0, 30.0)],
  );
  let outcome = run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "texture",
    "--entities",
    "E1",
    "--tool",
    "2",
    "--library",
    library.to_str().expect("utf8"),
  ]);
  assert_eq!(outcome.code, 0, "{}", outcome.stderr);
  let loaded = cam_store::document::load_document(&document).expect("loads");
  match &loaded.toolpaths[0].operation {
    Operation::Texture(params) => assert_eq!(
      params.spacing,
      Length::from_mm(2.4),
      "the library's stepover is the pattern's spacing"
    ),
    other => panic!("expected a texture toolpath, got {other:?}"),
  }
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_texture_simulates_with_no_gouge_and_no_uncut() {
  // The end-to-end claim A5 rests on: the target authorises the document's own pattern, and the program
  // cuts that pattern. A target that claimed the region instead of the grooves would report the
  // standing material as a gouge; a program that cleared would report it as uncut. Both are zero cells,
  // with nothing tolerance-softened.
  let directory = scratch("texture-simulate");
  let document = write_vectors(
    &directory,
    "texture.c2d",
    vec![rectangle_contour(40.0, 30.0)],
  );
  let library = write_library(&directory);

  let added = author_texture(&document, &library, "10", "0", 0.5);
  assert_eq!(added.code, 0, "{}", added.stderr);

  let outcome = simulate(&document, &library, &[]);
  assert_eq!(outcome.code, 0, "{}{}", outcome.stdout, outcome.stderr);
  // The program is the textured one and not an empty segment: "no deviation" from a program that cut
  // nothing would say nothing at all.
  assert_eq!(
    report_volume(&outcome.stdout, "program:"),
    12.0,
    "three grooves, one pass of four moves each: {}",
    outcome.stdout
  );
  assert!(
    outcome.stdout.contains("120.000 mm cutting length"),
    "and every pass is 40 mm long: {}",
    outcome.stdout
  );
  assert!(
    report_volume(&outcome.stdout, "removed:") > 0.0,
    "and they remove material: {}",
    outcome.stdout
  );
  assert!(
    outcome.stdout.contains("gouge: 0.000 mm3 over 0 cell(s)"),
    "{}",
    outcome.stdout
  );
  assert!(
    outcome.stdout.contains("uncut: 0.000 mm3 over 0 cell(s)"),
    "{}",
    outcome.stdout
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_texture_at_forty_five_degrees_emits_the_patterns_diagonal_grooves() {
  // `--angle` reaches the program. The pattern is clipped in the groove frame, so on a 40 x 30 mm
  // rectangle at 45 degrees the region's own extent is the diagonal, `(40 + 30) / sqrt(2) = 49.497` mm,
  // and a 6 mm spacing gives eight rows. Every one is a diagonal chord of the rectangle, so every
  // cutting move changes both X and Y — and none of them is an axis-aligned row a bounding-box
  // rotation would have produced.
  //
  // This is a program assertion and not a simulation one on purpose: a 45 degree groove's footprint
  // boundary cuts the cell lattice at an angle, so the sweep's half-cell sampling leaves up to a
  // hundredth of a millimetre of the claim's rind uncut along it — M5's published one-sided coverage
  // property (D-M5-3), asserted where it belongs in `cam-sim/tests/target.rs` and `oracle.rs` rather
  // than traded for a loose tolerance here.
  let directory = scratch("texture-45-program");
  let document = write_vectors(
    &directory,
    "texture.c2d",
    vec![rectangle_contour(40.0, 30.0)],
  );
  let library = write_library(&directory);
  let post = write_post(&directory);

  let added = author_texture(&document, &library, "6", "45", 0.5);
  assert_eq!(added.code, 0, "{}", added.stderr);

  let program = emit_program(&directory, &document, &library, &post);
  let cutting: Vec<&str> = program
    .lines()
    .filter(|line| line.contains("G01X") && line.contains("Y"))
    .collect();
  assert_eq!(
    cutting.len(),
    8,
    "eight grooves, one diagonal pass each:\n{program}"
  );
  for line in cutting {
    assert!(
      !line.contains("Y0.0000") || !line.contains("X0.0000"),
      "no groove is axis-aligned: {line}"
    );
  }
  std::fs::remove_dir_all(&directory).ok();
}

// --- inlay (M6 A6, A9, D-M6-7) ----------------------------------------------------------------

/// A document of a given size with the given contours as `E1..En`, and no toolpaths.
fn write_sized_vectors(
  directory: &Path,
  name: &str,
  width: f64,
  height: f64,
  contours: Vec<Contour>,
) -> PathBuf {
  let mut document = Document::new(Job {
    width: Length::from_mm(width),
    height: Length::from_mm(height),
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

/// A counter-clockwise rectangle with the given corners.
fn rectangle_at(min: f64, bottom: f64, max: f64, top: f64) -> Contour {
  Contour::from_points(
    &[
      Point2::new(min, bottom),
      Point2::new(max, bottom),
      Point2::new(max, top),
      Point2::new(min, top),
    ],
    true,
  )
  .expect("rectangle")
}

/// A 2 mm tool with a 1 mm stepover: the fixture the simulation resolves cell for cell, `emit`'s and
/// `simulate`'s own clean pocket precedent.
fn write_small_tool_library(directory: &Path) -> PathBuf {
  let path = directory.join("small.sqlite");
  let library = ToolLibrary::open(&path).expect("library");
  library
    .put_geometry(&ToolGeometry {
      id: "G3".to_owned(),
      name_format: "{Tool Type} ({Diameter|F}{Units Short})".to_owned(),
      tool_type: 0,
      units: UnitSystem::Mm,
      diameter: Some(Length::from_mm(2.0)),
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
      id: "C3".to_owned(),
      rate_units: UnitSystem::Mm,
      feed_rate: Some(100.0),
      plunge_rate: Some(30.0),
      spindle_speed: Some(18_000),
      stepdown: Some(Length::from_mm(2.0)),
      stepover: Some(Length::from_mm(1.0)),
      clear_stepover: None,
      tool_number: Some(1),
      notes: None,
    })
    .expect("cutting data");
  library.link("E3", "G3", "C3", None, None).expect("link");
  path
}

/// Author an inlay on `E1` through the real `toolpath` command.
fn author_inlay(document: &Path, library: &Path, extra: &[&str]) -> Outcome {
  let mut arguments = vec![
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "inlay",
    "--entities",
    "E1",
    "--tool",
    "1",
    "--library",
    library.to_str().expect("utf8"),
  ];
  arguments.extend_from_slice(extra);
  run(&arguments)
}

#[test]
fn an_inlay_command_appends_the_pair_in_the_fixed_order() {
  // A6 and D-M6-7: one command, **two** toolpaths, the cavity first and the plug that fits it second,
  // with consecutive ids and those exact names. The document says which half each one is with what it
  // already records about a toolpath's work: the female names the region it clears, the male names the
  // same region as the islands it leaves standing and takes no boundary entity, because its boundary is
  // the job rectangle and the job is not an entity.
  let directory = scratch("inlay-pair");
  let document = write_vectors(
    &directory,
    "inlay.c2d",
    vec![rectangle_at(20.0, 20.0, 60.0, 60.0)],
  );
  let library = write_library(&directory);
  let before = cam_store::document::load_document(&document).expect("loads");
  assert!(before.toolpaths.is_empty(), "the fixture has no toolpaths");

  let added = author_inlay(
    &document,
    &library,
    &["--inlay-allowance", "1.5", "--stepover", "4"],
  );
  assert_eq!(added.code, 0, "{}", added.stderr);
  assert_eq!(
    added.stdout,
    "added TP1 inlay female, tool T1, 1 entity(ies)\n\
     added TP2 inlay male, tool T1, 1 island(s)\n",
    "{}",
    added.stdout
  );

  let loaded = cam_store::document::load_document(&document).expect("loads");
  assert_eq!(loaded.toolpaths.len(), 2, "one command, two toolpaths");
  assert_eq!(loaded.toolpaths[0].id, "TP1");
  assert_eq!(loaded.toolpaths[0].name, "inlay female");
  assert_eq!(loaded.toolpaths[0].entity_ids, vec!["E1".to_owned()]);
  assert!(loaded.toolpaths[0].island_ids.is_empty());
  assert_eq!(loaded.toolpaths[1].id, "TP2");
  assert_eq!(loaded.toolpaths[1].name, "inlay male");
  assert!(loaded.toolpaths[1].entity_ids.is_empty());
  assert_eq!(loaded.toolpaths[1].island_ids, vec!["E1".to_owned()]);

  // Both halves carry the same parameters — one allowance, one depth, one stepover — and the allowance
  // is the number the flag named.
  match (
    &loaded.toolpaths[0].operation,
    &loaded.toolpaths[1].operation,
  ) {
    (Operation::Inlay(female), Operation::Inlay(male)) => {
      assert_eq!(female, male, "one copy of the pair's numbers");
      assert_eq!(female.allowance, Length::from_mm(1.5));
      assert_eq!(
        female.cut_depth,
        Length::from_mm(18.0),
        "--cut-depth's own default, from the profile's"
      );
      assert_eq!(
        female.pass_depth,
        Length::from_mm(6.0),
        "the library's stepdown"
      );
      assert_eq!(female.start_depth, Length::ZERO);
    }
    other => panic!("expected two inlay halves, got {other:?}"),
  }
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_second_inlay_command_appends_the_next_pair() {
  // The ids are deterministic and never reused: the second run appends `TP3` and `TP4`, in that order,
  // because the pair is named from the document's own next free ids.
  let directory = scratch("inlay-second");
  let document = write_vectors(
    &directory,
    "inlay.c2d",
    vec![rectangle_at(20.0, 20.0, 60.0, 60.0)],
  );
  let library = write_library(&directory);

  let first = author_inlay(
    &document,
    &library,
    &["--inlay-allowance", "1", "--stepover", "4"],
  );
  assert_eq!(first.code, 0, "{}", first.stderr);
  let second = author_inlay(
    &document,
    &library,
    &["--inlay-allowance", "2", "--stepover", "4"],
  );
  assert_eq!(second.code, 0, "{}", second.stderr);
  assert_eq!(
    second.stdout,
    "added TP3 inlay female, tool T1, 1 entity(ies)\n\
     added TP4 inlay male, tool T1, 1 island(s)\n",
    "{}",
    second.stdout
  );

  let loaded = cam_store::document::load_document(&document).expect("loads");
  let ids: Vec<&str> = loaded
    .toolpaths
    .iter()
    .map(|toolpath| toolpath.id.as_str())
    .collect();
  assert_eq!(ids, vec!["TP1", "TP2", "TP3", "TP4"]);
  let names: Vec<&str> = loaded
    .toolpaths
    .iter()
    .map(|toolpath| toolpath.name.as_str())
    .collect();
  assert_eq!(
    names,
    vec!["inlay female", "inlay male", "inlay female", "inlay male"]
  );
  std::fs::remove_dir_all(&directory).ok();
}

/// The whole program one inlay pair emits, byte for byte.
///
/// A 40 x 30 mm sheet, a 16 x 6 mm region at (12, 12), a 6 mm tool, a 1 mm allowance, a 4 mm stepover
/// and a 6 mm depth. The **female** is one ring — the tool centre 2 mm inside the region's own wall,
/// which is `radius - allowance` inside it and therefore `radius` inside the cavity the allowance grows
/// it to — cut first: `N30`..`N90`, seven moves. The **male** is the frame between the stock's edge and
/// the region, cleared in rows 4 mm apart: `N100` is the post's segment change (the tool does not change
/// and the clearance height does not move, so its `G00` carries nothing), then nineteen rows' worth of
/// passes — `N110`..`N500` — where a row that crosses the region is cut as **two** spans and a row that
/// misses it as one, zigzagging so each row starts where the last one ended. The post names the first
/// segment in its header block and no further one, which is why only the female is named.
///
/// CRLF, because the fixture post declares `LINE_ENDING = "[13][10]"`.
const GOLDEN_INLAY_PROGRAM: &str = "\
( inlay female )\r\n\
( CamPilot fixture Mach3 Arcs (mm) (*.nc) )\r\n\
N0G00G17G90G21\r\n\
N10T1M06\r\n\
N20S18000M03\r\n\
N30G00X14.0000Y14.0000\r\n\
N40G01Z-6.0000F100.0\r\n\
N50G01X26.0000\r\n\
N60G01Y16.0000\r\n\
N70G01X14.0000\r\n\
N80G01Y14.0000\r\n\
N90G00Z5.0000\r\n\
N100G00\r\n\
N110G00X3.0000Y3.0000\r\n\
N120G01Z-6.0000\r\n\
N130G01X37.0000\r\n\
N140G00Z5.0000\r\n\
N150G00Y7.0000\r\n\
N160G01Z-6.0000\r\n\
N170G01X3.0000\r\n\
N180G00Z5.0000\r\n\
N190G00Y11.0000\r\n\
N200G01Z-6.0000\r\n\
N210G01X9.0000\r\n\
N220G00Z5.0000\r\n\
N230G00X31.0000\r\n\
N240G01Z-6.0000\r\n\
N250G01X37.0000\r\n\
N260G00Z5.0000\r\n\
N270G00Y15.0000\r\n\
N280G01Z-6.0000\r\n\
N290G01X31.0000\r\n\
N300G00Z5.0000\r\n\
N310G00X9.0000\r\n\
N320G01Z-6.0000\r\n\
N330G01X3.0000\r\n\
N340G00Z5.0000\r\n\
N350G00Y19.0000\r\n\
N360G01Z-6.0000\r\n\
N370G01X9.0000\r\n\
N380G00Z5.0000\r\n\
N390G00X31.0000\r\n\
N400G01Z-6.0000\r\n\
N410G01X37.0000\r\n\
N420G00Z5.0000\r\n\
N430G00Y23.0000\r\n\
N440G01Z-6.0000\r\n\
N450G01X3.0000\r\n\
N460G00Z5.0000\r\n\
N470G00Y27.0000\r\n\
N480G01Z-6.0000\r\n\
N490G01X37.0000\r\n\
N500G00Z5.0000\r\n\
N510G00Z5.0000\r\n\
N520G00X0.0000Y0.0000\r\n\
N530M05\r\n\
N540M30\r\n\
";

#[test]
fn an_inlay_authored_through_the_cli_emits_the_golden_program() {
  // The pair's program, byte for byte: the female's single ring first — the 16 x 6 mm region at
  // (12, 12) cleared with a 6 mm tool and a 1 mm allowance, so the tool centre runs 2 mm inside its
  // wall — and then the male's rows across the frame between the stock's edge and the region, 4 mm
  // apart, each row cut as **two** spans where it crosses the region and as one where it does not.
  let directory = scratch("golden-inlay");
  let document = write_sized_vectors(
    &directory,
    "inlay.c2d",
    40.0,
    30.0,
    vec![rectangle_at(12.0, 12.0, 28.0, 18.0)],
  );
  let library = write_library(&directory);
  let post = write_post(&directory);

  let added = author_inlay(
    &document,
    &library,
    &[
      "--inlay-allowance",
      "1",
      "--stepover",
      "4",
      "--cut-depth",
      "6",
    ],
  );
  assert_eq!(added.code, 0, "{}", added.stderr);

  let program = emit_program(&directory, &document, &library, &post);
  assert_eq!(program, GOLDEN_INLAY_PROGRAM);
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn two_runs_of_an_inlay_print_and_emit_the_same_bytes() {
  // S6: identical input, identical bytes — the command's own report and the program it makes possible.
  let directory = scratch("inlay-determinism");
  let first_document = write_sized_vectors(
    &directory,
    "first.c2d",
    40.0,
    30.0,
    vec![rectangle_at(12.0, 12.0, 28.0, 18.0)],
  );
  let second_document = write_sized_vectors(
    &directory,
    "second.c2d",
    40.0,
    30.0,
    vec![rectangle_at(12.0, 12.0, 28.0, 18.0)],
  );
  let library = write_library(&directory);
  let post = write_post(&directory);

  let arguments = [
    "--inlay-allowance",
    "1",
    "--stepover",
    "4",
    "--cut-depth",
    "6",
  ];
  let first = author_inlay(&first_document, &library, &arguments);
  let second = author_inlay(&second_document, &library, &arguments);
  assert_eq!(first.code, 0, "{}", first.stderr);
  assert_eq!(second.code, 0, "{}", second.stderr);
  assert_eq!(first.stdout, second.stdout);

  let program = emit_program(&directory, &first_document, &library, &post);
  assert_eq!(
    emit_program(&directory, &second_document, &library, &post),
    program
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn an_inlay_takes_no_side_no_tab_no_lead_no_ramp_no_spacing_no_angle_and_no_chamfer_width() {
  // D-M6-7: the pair is two pockets, so every flag that belongs to another cutting law has no meaning
  // here — and `--strategy` with them, because the pair's parameters carry no clearing pattern at all.
  // Refusing them by name is the difference between a refusal and a silently different pair.
  let directory = scratch("inlay-flags");
  let library = write_library(&directory);
  for (flag, value) in [
    ("--side", "on"),
    ("--tab", "0.5:2:1"),
    ("--lead-in", "3"),
    ("--lead-out", "3"),
    ("--ramp", "10"),
    ("--spacing", "4"),
    ("--angle", "45"),
    ("--chamfer-width", "3"),
    ("--strategy", "raster"),
  ] {
    let document = write_vectors(
      &directory,
      "inlay.c2d",
      vec![rectangle_at(20.0, 20.0, 60.0, 60.0)],
    );
    let before = std::fs::read(&document).expect("read");
    let outcome = author_inlay(&document, &library, &["--stepover", "4", flag, value]);
    assert_eq!(outcome.code, 2, "{flag}: {}", outcome.stdout);
    assert!(
      outcome.stderr.contains(flag),
      "the refusal names the flag: {}",
      outcome.stderr
    );
    assert!(
      outcome.stderr.contains("male/female pair of pockets"),
      "and says what an inlay is instead: {}",
      outcome.stderr
    );
    assert_eq!(
      std::fs::read(&document).expect("read"),
      before,
      "{flag}: nothing is written"
    );
  }
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn an_inlay_names_the_pair_itself_so_it_takes_no_id_and_no_name() {
  // The two ids and the two names are part of the command's contract: one `--id` or `--name` could only
  // ever name one of the two toolpaths it appends.
  let directory = scratch("inlay-names");
  let document = write_vectors(
    &directory,
    "inlay.c2d",
    vec![rectangle_at(20.0, 20.0, 60.0, 60.0)],
  );
  let library = write_library(&directory);
  for flag in [["--id", "PAIR"], ["--name", "Inlay"]] {
    let outcome = author_inlay(&document, &library, &["--stepover", "4", flag[0], flag[1]]);
    assert_eq!(outcome.code, 2, "{}", outcome.stdout);
    assert!(
      outcome.stderr.contains(flag[0]),
      "the refusal names the flag: {}",
      outcome.stderr
    );
  }
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn an_inlay_takes_its_stepover_and_pass_depth_from_the_tool_library() {
  // The pocket's own fallbacks, unchanged: `--stepover` absent takes the library's stepover and
  // `--pass-depth` absent takes its stepdown. The fixture tool is 6 mm with a 2.4 mm stepover and a
  // 5 mm stepdown.
  let directory = scratch("inlay-library");
  let library = write_library_with_pocket_rates(&directory);
  let document = write_vectors(
    &directory,
    "inlay.c2d",
    vec![rectangle_at(20.0, 20.0, 60.0, 60.0)],
  );
  let outcome = run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "inlay",
    "--entities",
    "E1",
    "--tool",
    "2",
    "--library",
    library.to_str().expect("utf8"),
  ]);
  assert_eq!(outcome.code, 0, "{}", outcome.stderr);
  let loaded = cam_store::document::load_document(&document).expect("loads");
  match &loaded.toolpaths[0].operation {
    Operation::Inlay(params) => {
      assert_eq!(
        params.stepover,
        Length::from_mm(2.4),
        "the library's stepover"
      );
      assert_eq!(
        params.pass_depth,
        Length::from_mm(5.0),
        "the library's stepdown"
      );
    }
    other => panic!("expected an inlay toolpath, got {other:?}"),
  }

  // A library with no stepover at all is an input error naming the flag, never a defaulted lattice — the
  // same refusal a pocket speaks.
  let plain = write_library(&directory);
  let document = write_vectors(
    &directory,
    "inlay-nostepover.c2d",
    vec![rectangle_at(20.0, 20.0, 60.0, 60.0)],
  );
  let outcome = author_inlay(&document, &plain, &[]);
  assert_eq!(outcome.code, 2, "{}", outcome.stdout);
  assert!(
    outcome.stderr.contains("--stepover"),
    "the refusal names the missing input: {}",
    outcome.stderr
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn an_inlay_pair_simulates_with_no_gouge_and_no_uncut() {
  // A9's end-to-end claim for the pair: the target authorises the female's cavity and the male's frame,
  // and the two programs cut exactly those. The fixture is the one whose geometry the 1 mm grid resolves
  // cleanly — a 2 mm tool at a 1 mm stepover, on a region whose walls sit on the lattice — so neither
  // number is tolerance-softened.
  let directory = scratch("inlay-simulate");
  let document = write_vectors(
    &directory,
    "inlay.c2d",
    vec![rectangle_at(40.0, 20.0, 120.0, 80.0)],
  );
  let library = write_small_tool_library(&directory);

  let added = author_inlay(
    &document,
    &library,
    &[
      "--inlay-allowance",
      "2",
      "--stepover",
      "1",
      "--cut-depth",
      "2",
    ],
  );
  assert_eq!(added.code, 0, "{}", added.stderr);

  let outcome = simulate(&document, &library, &[]);
  assert_eq!(outcome.code, 0, "{}{}", outcome.stdout, outcome.stderr);
  assert!(
    outcome.stdout.contains("gouge: 0.000 mm3 over 0 cell(s)"),
    "{}",
    outcome.stdout
  );
  assert!(
    outcome.stdout.contains("uncut: 0.000 mm3 over 0 cell(s)"),
    "{}",
    outcome.stdout
  );
  // And it is the pair that was simulated, not an empty program: two toolpaths, and both halves remove
  // material.
  assert!(
    outcome.stdout.contains("2 toolpath(s)"),
    "{}",
    outcome.stdout
  );
  let removed = report_volume(&outcome.stdout, "removed:");
  // The cavity is the 80 x 60 region grown by 2 mm — 84 x 64 — and the frame is the rest of the 200 x 100
  // stock, so together they are the whole 200 x 100 mm sheet at 2 mm: 40000 mm3.
  assert!(
    (removed - 40_000.0).abs() < 1.0,
    "the pair clears the whole stock: {removed} mm3 of it, {}",
    outcome.stdout
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn an_inlay_whose_region_is_not_inside_the_stock_exits_two_naming_the_job() {
  // The male clears the stock *around* the region, so a region on or across the stock's edge has nothing
  // beside it to clear. The command asks the strategy, and refuses before anything is written.
  let directory = scratch("inlay-outside");
  let document = write_vectors(
    &directory,
    "inlay.c2d",
    vec![rectangle_at(150.0, 20.0, 220.0, 60.0)],
  );
  let library = write_library(&directory);
  let before = std::fs::read(&document).expect("read");
  let outcome = author_inlay(&document, &library, &["--stepover", "4"]);
  assert_eq!(outcome.code, 2, "{}", outcome.stdout);
  assert!(
    outcome.stderr.contains("200 x 100"),
    "the refusal names the stock: {}",
    outcome.stderr
  );
  assert_eq!(
    std::fs::read(&document).expect("read"),
    before,
    "nothing is written"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn an_inlay_on_an_open_contour_exits_two_naming_the_geometry() {
  // A cavity is a region's and an open vector bounds none: refused by the document's own closed-geometry
  // rule, before anything is written.
  let directory = scratch("inlay-open");
  let open = Contour::from_points(&[Point2::new(20.0, 20.0), Point2::new(60.0, 20.0)], false)
    .expect("an open vector");
  let document = write_vectors(&directory, "inlay.c2d", vec![open]);
  let library = write_library(&directory);
  let outcome = author_inlay(&document, &library, &["--stepover", "4"]);
  assert_eq!(outcome.code, 2, "{}", outcome.stdout);
  assert!(
    outcome.stderr.contains("closed geometry"),
    "the refusal says what is wrong: {}",
    outcome.stderr
  );
  std::fs::remove_dir_all(&directory).ok();
}

// --- thread milling (M6 A7, A9, D-M6-8) -------------------------------------------------------

/// Author a thread on `E1` through the real `toolpath` command.
fn author_thread(
  document: &Path,
  library: &Path,
  pitch: f64,
  direction: &str,
  cut_depth: f64,
) -> Outcome {
  let pitch = pitch.to_string();
  let cut_depth = cut_depth.to_string();
  run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "thread",
    "--entities",
    "E1",
    "--tool",
    "1",
    "--pitch",
    &pitch,
    "--thread-direction",
    direction,
    "--cut-depth",
    &cut_depth,
    "--library",
    library.to_str().expect("utf8"),
  ])
}

/// The whole program one internal thread emits, byte for byte.
///
/// A 16 mm circle milled by a 6 mm tool at a 1.5 mm pitch, 6 mm deep: the tool centre orbits at
/// `(D − t)/2 = 5` mm, so it enters over `(30, 15)` — the circle's centre is `(25, 15)` — and the descent
/// is `6 / 1.5 = 4` whole revolutions, one **helical** block each, `G03` with its incremental centre and
/// the Z it has reached. Four turns of exactly one pitch, and a retract.
///
/// CRLF, because the fixture post declares `LINE_ENDING = "[13][10]"`.
const GOLDEN_THREAD_PROGRAM: &str = "\
( Thread 1 )\r\n\
( CamPilot fixture Mach3 Arcs (mm) (*.nc) )\r\n\
N0G00G17G90G21\r\n\
N10T1M06\r\n\
N20S18000M03\r\n\
N30G00X30.0000Y15.0000\r\n\
N40G01Z0.0000F100.0\r\n\
N50G03Z-1.5000I-5.0000J0.0000\r\n\
N60G03Z-3.0000I-5.0000J0.0000\r\n\
N70G03Z-4.5000I-5.0000J0.0000\r\n\
N80G03Z-6.0000I-5.0000J0.0000\r\n\
N90G00Z5.0000\r\n\
N100G00Z5.0000\r\n\
N110G00X0.0000Y0.0000\r\n\
N120M05\r\n\
N130M30\r\n";

#[test]
fn a_thread_authored_through_the_cli_emits_the_golden_program() {
  let directory = scratch("golden-thread");
  let document = write_vectors(&directory, "thread.c2d", vec![circle_contour(8.0)]);
  let library = write_library(&directory);
  let post = write_post(&directory);

  let added = author_thread(&document, &library, 1.5, "internal", 6.0);
  assert_eq!(added.code, 0, "{}{}", added.stdout, added.stderr);
  assert_eq!(
    added.stdout,
    "added TP1 thread (1.5 mm pitch, internal), tool T1, 1 entity(ies)\n"
  );

  let program = emit_program(&directory, &document, &library, &post);
  assert_eq!(program, GOLDEN_THREAD_PROGRAM);
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn two_thread_runs_emit_identical_bytes() {
  // S6: identical input, identical output. Two fresh documents, the same command twice, the same
  // document bytes and the same program bytes.
  let directory = scratch("thread-determinism");
  let library = write_library(&directory);
  let post = write_post(&directory);
  let first = write_vectors(&directory, "first.c2d", vec![circle_contour(8.0)]);
  let second = write_vectors(&directory, "second.c2d", vec![circle_contour(8.0)]);
  assert_eq!(
    author_thread(&first, &library, 1.5, "internal", 6.0).code,
    0
  );
  assert_eq!(
    author_thread(&second, &library, 1.5, "internal", 6.0).code,
    0
  );
  assert_eq!(
    std::fs::read(&first).expect("read"),
    std::fs::read(&second).expect("read"),
    "the two documents are identical"
  );
  assert_eq!(
    emit_program(&directory, &first, &library, &post),
    emit_program(&directory, &second, &library, &post),
    "and so are the two programs"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn an_external_thread_orbits_outside_the_circle() {
  // The same circle as a boss: the orbit is `(D + t)/2 = 11` mm, so the entry is over `x = 36` and every
  // helical block's incremental centre offset is `I-11.0000` — the tool is outside the wall it cuts.
  let directory = scratch("thread-external");
  let document = write_vectors(&directory, "boss.c2d", vec![circle_contour(8.0)]);
  let library = write_library(&directory);
  let post = write_post(&directory);

  let added = author_thread(&document, &library, 2.0, "external", 8.0);
  assert_eq!(added.code, 0, "{}{}", added.stdout, added.stderr);
  assert_eq!(
    added.stdout,
    "added TP1 thread (2 mm pitch, external), tool T1, 1 entity(ies)\n"
  );

  let program = emit_program(&directory, &document, &library, &post);
  assert!(
    program.contains("N30G00X36.0000Y15.0000"),
    "the orbit is (D + t)/2 outside the circle:\n{program}"
  );
  assert!(
    program.contains("I-11.0000"),
    "and the tool centre is 11 mm from the circle's centre:\n{program}"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_thread_refuses_by_name_the_flags_its_own_law_does_not_have() {
  // A thread is one helical descent at a pitch: a side, a pass law, a tab, a lead, a stepover, an
  // allowance, a ramp, a bevel's width, a clearing strategy and an inlay's allowance all describe a cut
  // it does not make. Each is refused by name, and nothing is written.
  let directory = scratch("thread-flags");
  let document = write_vectors(&directory, "thread.c2d", vec![circle_contour(8.0)]);
  let library = write_library(&directory);
  let before = std::fs::read(&document).expect("read");

  for (flag, value) in [
    ("--side", "inside"),
    ("--pass-depth", "2"),
    ("--stepover", "2"),
    ("--allowance", "1"),
    ("--ramp", "4"),
    ("--chamfer-width", "3"),
    ("--strategy", "raster"),
    ("--inlay-allowance", "0.5"),
  ] {
    let outcome = run(&[
      "toolpath",
      document.to_str().expect("utf8"),
      "--kind",
      "thread",
      "--entities",
      "E1",
      "--tool",
      "1",
      "--pitch",
      "1.5",
      flag,
      value,
      "--library",
      library.to_str().expect("utf8"),
    ]);
    assert_eq!(outcome.code, 2, "{flag}: {}", outcome.stderr);
    assert!(
      outcome.stderr.contains(flag) && outcome.stderr.contains("takes no"),
      "the refusal names {flag}: {}",
      outcome.stderr
    );
  }
  assert_eq!(
    std::fs::read(&document).expect("read"),
    before,
    "nothing is written"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_thread_without_a_pitch_or_with_a_pitch_that_descends_nowhere_is_refused_by_name() {
  let directory = scratch("thread-pitch");
  let document = write_vectors(&directory, "thread.c2d", vec![circle_contour(8.0)]);
  let library = write_library(&directory);
  let before = std::fs::read(&document).expect("read");

  let missing = run(&[
    "toolpath",
    document.to_str().expect("utf8"),
    "--kind",
    "thread",
    "--entities",
    "E1",
    "--tool",
    "1",
    "--library",
    library.to_str().expect("utf8"),
  ]);
  assert_eq!(missing.code, 2, "{}", missing.stderr);
  assert!(
    missing.stderr.contains("--pitch"),
    "the refusal names the flag: {}",
    missing.stderr
  );

  for pitch in ["0", "-1.5"] {
    let outcome = run(&[
      "toolpath",
      document.to_str().expect("utf8"),
      "--kind",
      "thread",
      "--entities",
      "E1",
      "--tool",
      "1",
      "--pitch",
      pitch,
      "--library",
      library.to_str().expect("utf8"),
    ]);
    assert_eq!(outcome.code, 2, "pitch {pitch}: {}", outcome.stderr);
    assert!(
      outcome.stderr.contains("pitch") && outcome.stderr.contains(pitch),
      "the refusal names the pitch and its value: {}",
      outcome.stderr
    );
  }
  assert_eq!(
    std::fs::read(&document).expect("read"),
    before,
    "nothing is written"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_thread_on_geometry_that_is_not_a_circle_is_refused_by_name() {
  let directory = scratch("thread-square");
  let document = write_vectors(
    &directory,
    "square.c2d",
    vec![rectangle_contour(40.0, 30.0)],
  );
  let library = write_library(&directory);
  let before = std::fs::read(&document).expect("read");

  let outcome = author_thread(&document, &library, 1.5, "internal", 6.0);
  assert_eq!(outcome.code, 2, "{}{}", outcome.stdout, outcome.stderr);
  assert!(
    outcome.stderr.contains("E1") && outcome.stderr.contains("not a circle"),
    "the refusal names the entity and what is wrong with it: {}",
    outcome.stderr
  );
  assert_eq!(
    std::fs::read(&document).expect("read"),
    before,
    "nothing is written"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn an_internal_thread_whose_tool_is_wider_than_the_hole_is_refused_by_name() {
  // A 4 mm hole with the library's 6 mm tool: `(D − t)/2` is negative, so there is no orbit to run the
  // tool centre on. Refused while the command can still say no, naming both diameters.
  let directory = scratch("thread-too-wide");
  let document = write_vectors(&directory, "hole.c2d", vec![circle_contour(2.0)]);
  let library = write_library(&directory);
  let before = std::fs::read(&document).expect("read");

  let outcome = author_thread(&document, &library, 1.5, "internal", 6.0);
  assert_eq!(outcome.code, 2, "{}{}", outcome.stdout, outcome.stderr);
  assert!(
    outcome.stderr.contains("6") && outcome.stderr.contains("4"),
    "the refusal names the tool and the hole: {}",
    outcome.stderr
  );
  assert!(
    outcome.stderr.contains("orbit"),
    "and why they cannot be combined: {}",
    outcome.stderr
  );
  assert_eq!(
    std::fs::read(&document).expect("read"),
    before,
    "nothing is written"
  );

  // The same tool outside the same circle is a thread that fits by construction: the orbit only grows.
  let external = author_thread(&document, &library, 1.5, "external", 6.0);
  assert_eq!(external.code, 0, "{}", external.stderr);
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_coarse_thread_simulates_with_no_gouge_and_no_uncut() {
  // **The acceptance criterion of the exact-claim round.** A real thread: 6 mm deep at a 1.5 mm pitch — four
  // revolutions — with a 6 mm tool in a 16 mm hole, on 1 mm cells and the judgement's eighth of a cell. The
  // material a helix leaves between two revolutions is the **thread**, not material standing: the claim is
  // the spiral the descent leaves, per cell, so a correct coarse thread reports nothing on either sign.
  // Before the claim was exact this document exited 6 with tens of cubic millimetres of uncut.
  let directory = scratch("thread-simulate");
  let document = write_vectors(&directory, "thread.c2d", vec![circle_contour(8.0)]);
  let library = write_library(&directory);

  let added = author_thread(&document, &library, 1.5, "internal", 6.0);
  assert_eq!(added.code, 0, "{}{}", added.stdout, added.stderr);

  let outcome = simulate(&document, &library, &[]);
  assert_eq!(outcome.code, 0, "{}{}", outcome.stdout, outcome.stderr);
  assert!(
    outcome.stdout.contains("gouge: 0.000 mm3 over 0 cell(s)"),
    "{}",
    outcome.stdout
  );
  assert!(
    outcome.stdout.contains("uncut: 0.000 mm3 over 0 cell(s)"),
    "{}",
    outcome.stdout
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn the_help_lists_the_thread_kind_and_its_two_options() {
  let outcome = run(&["--help"]);
  assert_eq!(outcome.code, 0);
  for expected in [
    "texture, inlay or\n                              thread",
    "--pitch <mm>",
    "--thread-direction <d>",
    "helical descent",
  ] {
    assert!(
      outcome.stdout.contains(expected),
      "help is missing {expected}:\n{}",
      outcome.stdout
    );
  }
}

#[test]
fn a_post_without_helical_blocks_refuses_a_thread_by_name_and_writes_nothing() {
  // A7's other half: the program expresses the helix as a helical arc, so a post that cannot express one
  // **refuses** it — naming the post and the move kind — instead of emitting a flat circle at the wrong
  // depth. M4's arc precedent, at the CLI's own door: exit 4, and no program file.
  let directory = scratch("thread-post-refusal");
  let document = write_vectors(&directory, "thread.c2d", vec![circle_contour(8.0)]);
  let library = write_library(&directory);
  let added = author_thread(&document, &library, 1.5, "internal", 6.0);
  assert_eq!(added.code, 0, "{}", added.stderr);

  let arcs_only = directory.join("arcs-only.pp");
  std::fs::write(
    &arcs_only,
    "POST_NAME = \"arcs only\"\n\
     VAR X_POSITION = [X|C|X|1.4]\n\
     VAR Z_POSITION = [Z|C|Z|1.4]\n\
     VAR ARC_CENTRE_I_INC_POSITION = [I|A|I|1.4]\n\
     VAR ARC_CENTRE_J_INC_POSITION = [J|A|J|1.4]\n\
     begin FIRST_CW_ARC_MOVE\n\"G02[X][I][J]\"\n\
     begin CW_ARC_MOVE\n\"G02[X][I][J]\"\n",
  )
  .expect("post written");

  let out = directory.join("program.nc");
  let outcome = run(&[
    "emit",
    document.to_str().expect("utf8"),
    "--post",
    arcs_only.to_str().expect("utf8"),
    "--library",
    library.to_str().expect("utf8"),
    "--out",
    out.to_str().expect("utf8"),
  ]);
  assert_eq!(outcome.code, 4, "{}", outcome.stderr);
  assert!(
    outcome.stderr.contains("arcs only")
      && outcome.stderr.contains("Move::Helix")
      && outcome.stderr.contains("refusing to emit an approximation"),
    "the refusal names the post, the move kind and the refusal to approximate: {}",
    outcome.stderr
  );
  assert!(!out.exists(), "a refused program writes nothing");
  std::fs::remove_dir_all(&directory).ok();
}
