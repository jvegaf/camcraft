//! End-to-end tests for the headless pipeline (A8, A9).
//!
//! These run the real binary, so they assert the observable contract: exit code, exact stdout and
//! stderr, and the files left on disk.

use std::f64::consts::FRAC_PI_2;
use std::path::{Path, PathBuf};
use std::process::Command;

use cam_geom::{Contour, Point2};
use cam_model::{
  Document, Entity, EntityKind, IsolationKind, IsolationParams, Job, Layer, LayerRole,
  MillingDirection, Nesting, NestingPart, NestingPlacement, NestingSheet, Operation, Toolpath,
};
use cam_store::document::save_document;
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

fn write_document(directory: &Path, contour: Contour, tools_ref: Option<&str>) -> PathBuf {
  let mut document = Document::new(Job {
    width: Length::from_mm(200.0),
    height: Length::from_mm(100.0),
    thickness: Length::from_mm(18.0),
    units: UnitSystem::Mm,
    origin: cam_model::OriginCorner::BottomLeft,
    safe_z: Length::from_mm(5.0),
    material: None,
  });
  document.layers.push(Layer {
    id: "L1".to_owned(),
    name: "Part".to_owned(),
    role: LayerRole::Geometry,
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
    operation: Operation::Isolation(IsolationParams {
      passes: 1,
      overlap: 0.5,
      direction: MillingDirection::Climb,
      kind: IsolationKind::Full,
      cut_depth: Length::from_mm(0.1),
      keep_pours: false,
    }),
  });
  document.tools_ref = tools_ref.map(str::to_owned);

  let path = directory.join("part.c2d");
  save_document(&path, &document).expect("document saved");
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
    outcome.stderr.contains("camcraft/document@1"),
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

/// One tool with three cutting records: its own (100/30) and one per material (mdf 120/40, hardwood
/// 60/20). That is the shape the store has carried since migration 1 — a tool's rates are per material
/// and per machine — and M11's selector is what finally asks for the one you are cutting.
///
/// The same document `write_document` builds, with the job's material set — M11's document field. Built
/// through the model and not by patching bytes: `.c2d` is not text, which the first version of this
/// helper discovered.
#[test]
fn export_writes_the_document_back_out_in_both_formats() {
  // M12's contract through the command: a document goes out as DXF or SVG, and the layer identity the
  // document holds reaches the DXF file. The round trip itself is `cam-export`'s own test; this one pins
  // the command's surface — exit code, the bytes on disk, the refusal for a format nobody writes.
  let directory = scratch("export");
  let document = write_document(&directory, rectangle_contour(50.0, 25.0), None);

  for (format, extension, expected) in [("dxf", "dxf", "LWPOLYLINE"), ("svg", "svg", "<path")] {
    let out = directory.join(format!("part.{extension}"));
    let outcome = run(&[
      "export",
      document.to_str().expect("utf8"),
      "--format",
      format,
      "--out",
      out.to_str().expect("utf8"),
    ]);
    assert_eq!(outcome.code, 0, "{format}: {}", outcome.stderr);
    assert!(outcome.stdout.is_empty(), "{format}: {}", outcome.stdout);
    assert!(
      outcome.stderr.contains("wrote"),
      "{format}: {}",
      outcome.stderr
    );
    let written = std::fs::read_to_string(&out).expect("the file was written");
    assert!(
      written.contains(expected),
      "{format}: the file carries {expected}: {written}"
    );
  }

  // Without `--out` the bytes go to stdout, the way `emit` writes a program.
  let to_stdout = run(&[
    "export",
    document.to_str().expect("utf8"),
    "--format",
    "dxf",
  ]);
  assert_eq!(to_stdout.code, 0, "{}", to_stdout.stderr);
  assert!(
    to_stdout.stdout.contains("LWPOLYLINE"),
    "{}",
    to_stdout.stdout
  );

  // A format this build does not write is refused by name, not guessed from an extension.
  let refused = run(&[
    "export",
    document.to_str().expect("utf8"),
    "--format",
    "pdf",
  ]);
  assert_eq!(refused.code, 2, "{}", refused.stderr);
  assert!(refused.stdout.is_empty(), "{}", refused.stdout);
  assert!(refused.stderr.contains("pdf"), "{}", refused.stderr);

  // And the format is not optional.
  let missing = run(&["export", document.to_str().expect("utf8")]);
  assert_eq!(missing.code, 2, "{}", missing.stderr);
  assert!(missing.stderr.contains("--format"), "{}", missing.stderr);

  // Writing over the document we just read would destroy it and report success, so it is refused
  // (review-a7e624d841d3d0c6's R4-SAME-PATH and R3-001).
  let before = std::fs::read(&document).expect("the document is there");
  let over_itself = run(&[
    "export",
    document.to_str().expect("utf8"),
    "--format",
    "dxf",
    "--out",
    document.to_str().expect("utf8"),
  ]);
  assert_eq!(over_itself.code, 2, "{}", over_itself.stderr);
  assert!(
    over_itself.stderr.contains("must not be the document"),
    "{}",
    over_itself.stderr
  );
  assert_eq!(
    std::fs::read(&document).expect("the document survives"),
    before,
    "a refused export leaves the document byte for byte as it was"
  );

  // The write lands whole: a second export over the same path replaces it, and no temporary is left
  // behind (R4-ATOMIC-OUT).
  let reused = directory.join("part.dxf");
  let again = run(&[
    "export",
    document.to_str().expect("utf8"),
    "--format",
    "dxf",
    "--out",
    reused.to_str().expect("utf8"),
  ]);
  assert_eq!(again.code, 0, "{}", again.stderr);
  let leftovers: Vec<String> = std::fs::read_dir(&directory)
    .expect("the scratch directory")
    .filter_map(|entry| entry.ok())
    .map(|entry| entry.file_name().to_string_lossy().into_owned())
    .filter(|name| name.contains("tmp"))
    .collect();
  assert!(
    leftovers.is_empty(),
    "no temporary is left behind: {leftovers:?}"
  );
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
    material: None,
  });
  document.layers.push(Layer {
    id: "L1".to_owned(),
    name: "Vectors".to_owned(),
    role: LayerRole::Geometry,
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

const NEST_REPORT: &str = "\
nest: 1 sheet(s) of 1000.000 x 600.000 mm, 1 part(s), 1 instance(s)
options: gap 0.000 mm, rotation off
sheet S1: used
part: P1 rectangle, 1 instance(s)
placement: P1 on S1 at (0.000, 0.000), rotation none, 600.000 x 400.000 mm
algorithm: rectangle, maxrects
sheets used: 1
utilisation: 0.400
waste: 0.600
unplaced: none
";

#[test]
fn a_rectangle_part_nests_onto_one_sheet_and_the_document_records_the_placement_and_the_cost() {
  let directory = scratch("nest-rectangle");
  let document = write_nesting_document(
    &directory,
    "panel.c2d",
    vec![rectangle_contour(600.0, 400.0)],
    &[("P1", &[1], 1)],
    &[("S1", 1000.0, 600.0)],
    &[("P1", "S1", 0.0, 0.0)],
  );

  let outcome = run(&["nest", document.to_str().expect("utf8")]);
  assert_eq!(outcome.code, 0, "{}{}", outcome.stdout, outcome.stderr);
  assert!(outcome.stderr.is_empty(), "{}", outcome.stderr);
  assert_eq!(outcome.stdout, NEST_REPORT);

  // The document is written back with the placements and the cost (S2, S3), and the placement is the
  // one the report printed: the part's own corner on the sheet's own corner.
  let loaded = cam_store::document::load_document(&document).expect("loads");
  let nesting = loaded.nesting.as_ref().expect("the section is written");
  assert_eq!(
    nesting.placements,
    vec![NestingPlacement {
      part_id: "P1".to_owned(),
      sheet_id: "S1".to_owned(),
      position: Point2::new(0.0, 0.0),
      rotation: Angle::ZERO,
    }]
  );

  // The cost record is the document's own arithmetic and nothing else (T1c): written from
  // `Document::nesting_utilisation`, so the field and the placements cannot state two utilisations.
  let cost = nesting.cost.as_ref().expect("the cost record is written");
  assert_eq!(cost.sheets_used, 1);
  assert_eq!(
    Some(cost.utilisation),
    loaded.nesting_utilisation(),
    "the record is exactly the fraction the document derives"
  );
  assert!(
    (cost.utilisation - 0.4).abs() < 1e-12,
    "{}",
    cost.utilisation
  );
  assert!((cost.waste - 0.6).abs() < 1e-12, "{}", cost.waste);
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_second_sheet_is_opened_when_the_parts_do_not_fit_one() {
  // A document nested on one 2000 x 1000 mm board, re-nested onto 1000 x 600 mm boards: two 600 x 400
  // mm instances no longer share a board, so the packer opens a second one — and the document, which
  // authored one, has to hold the board the run actually used, or the placement would name a sheet
  // that does not exist (S4).
  let directory = scratch("nest-second-sheet");
  let document = write_nesting_document(
    &directory,
    "two-panels.c2d",
    vec![rectangle_contour(600.0, 400.0)],
    &[("P1", &[1], 2)],
    &[("S1", 2000.0, 1000.0)],
    &[("P1", "S1", 0.0, 0.0), ("P1", "S1", 600.0, 0.0)],
  );

  let outcome = run(&[
    "nest",
    document.to_str().expect("utf8"),
    "--sheets",
    "1000x600",
  ]);
  assert_eq!(outcome.code, 0, "{}{}", outcome.stdout, outcome.stderr);
  let expected = "\
nest: 2 sheet(s) of 1000.000 x 600.000 mm, 1 part(s), 2 instance(s)
options: gap 0.000 mm, rotation off
sheet S1: used
sheet S2: used
part: P1 rectangle, 2 instance(s)
placement: P1 on S1 at (0.000, 0.000), rotation none, 600.000 x 400.000 mm
placement: P1 on S2 at (0.000, 0.000), rotation none, 600.000 x 400.000 mm
algorithm: rectangle, maxrects
sheets used: 2
utilisation: 0.400
waste: 0.600
unplaced: none
";
  assert_eq!(outcome.stdout, expected);

  let loaded = cam_store::document::load_document(&document).expect("loads");
  let nesting = loaded.nesting.as_ref().expect("the section is written");
  assert_eq!(nesting.sheets.len(), 2, "the opened board is authored too");
  assert_eq!(nesting.sheets[1].id, "S2");
  assert_eq!(nesting.sheets[1].width.mm(), 1000.0);
  assert_eq!(nesting.sheets[1].height.mm(), 600.0);
  let sheets: Vec<&str> = nesting
    .placements
    .iter()
    .map(|placement| placement.sheet_id.as_str())
    .collect();
  assert_eq!(sheets, vec!["S1", "S2"]);
  let cost = nesting.cost.as_ref().expect("the cost record is written");
  assert_eq!(cost.sheets_used, 2);
  assert_eq!(Some(cost.utilisation), loaded.nesting_utilisation());
  assert!(
    (cost.utilisation - 0.4).abs() < 1e-12,
    "{}",
    cost.utilisation
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_part_that_fits_no_sheet_is_reported_with_its_id_and_exits_seven() {
  // A 1200 x 500 mm part nested on a 2000 x 1000 mm board, re-nested onto 1000 x 600 mm boards with
  // rotation off: no board holds it. That is a finding about the document's own request, not invalid
  // input, so it has its own exit code — and the document is left exactly as it was, because
  // `Document::validate` refuses a document whose placements fall short of each part's instance count
  // (`NestingInstanceCountMismatch`), so a partial nesting has no honest form to store.
  let directory = scratch("nest-unplaced");
  let document = write_nesting_document(
    &directory,
    "wide.c2d",
    vec![rectangle_contour(1200.0, 500.0)],
    &[("P1", &[1], 1)],
    &[("S1", 2000.0, 1000.0)],
    &[("P1", "S1", 0.0, 0.0)],
  );
  let before = std::fs::read(&document).expect("read");

  let outcome = run(&[
    "nest",
    document.to_str().expect("utf8"),
    "--sheets",
    "1000x600",
  ]);
  assert_eq!(outcome.code, 7, "{}{}", outcome.stdout, outcome.stderr);
  let expected = "\
nest: 1 sheet(s) of 1000.000 x 600.000 mm, 1 part(s), 1 instance(s)
options: gap 0.000 mm, rotation off
sheet S1: unused
part: P1 rectangle, 1 instance(s)
algorithm: rectangle, maxrects
sheets used: 0
utilisation: none
waste: none
unplaced: P1, 1 of 1 instance(s)
";
  assert_eq!(outcome.stdout, expected);
  assert!(
    outcome.stderr.contains("P1") && outcome.stderr.contains("unplaced"),
    "the failure names the part and the finding: {}",
    outcome.stderr
  );
  assert_eq!(
    std::fs::read(&document).expect("read"),
    before,
    "an incomplete nesting writes nothing"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_free_profile_part_goes_through_the_no_fit_heuristic_and_the_report_names_it() {
  // The L bracket is not a rectangle, so it takes the other nester S6 names — the free-profile no-fit
  // heuristic — and the report says so, per part class, and names the lattice step it searched with.
  //
  // Its utilisation is 0.200 and not 0.100, and that is the document's definition rather than a
  // rounding: a placement's footprint is the part's bounding box (D-M9-2), so 300 x 400 = 120 000 of
  // the board's 600 000 mm2 is what the one arithmetic counts, conservative for a curved or notched
  // outline by design. The report reads that arithmetic, never the nester's view of the run.
  let directory = scratch("nest-profile");
  let document = write_nesting_document(
    &directory,
    "bracket.c2d",
    vec![l_bracket()],
    &[("Bracket", &[1], 1)],
    &[("S1", 1000.0, 600.0)],
    &[("Bracket", "S1", 0.0, 0.0)],
  );

  let outcome = run(&["nest", document.to_str().expect("utf8")]);
  assert_eq!(outcome.code, 0, "{}{}", outcome.stdout, outcome.stderr);
  let expected = "\
nest: 1 sheet(s) of 1000.000 x 600.000 mm, 1 part(s), 1 instance(s)
options: gap 0.000 mm, rotation off
sheet S1: used
part: Bracket free profile, 1 instance(s)
placement: Bracket on S1 at (0.000, 0.000), rotation none, 300.000 x 400.000 mm
algorithm: free profile, free-profile no-fit heuristic at 10.000 mm
sheets used: 1
utilisation: 0.200
waste: 0.800
unplaced: none
";
  assert_eq!(outcome.stdout, expected);

  let nesting = load_nesting(&document);
  assert_eq!(
    nesting.placements,
    vec![NestingPlacement {
      part_id: "Bracket".to_owned(),
      sheet_id: "S1".to_owned(),
      position: Point2::new(0.0, 0.0),
      rotation: Angle::ZERO,
    }]
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_search_step_too_fine_for_the_anchor_cap_is_refused_with_the_lattice_it_asked_for() {
  // The cap is arithmetic, not a taste in millimetres: a 1000 x 600 board at 0.2 mm asks for
  // 5001 x 3001 anchors, and the refusal says so before the search starts.
  let directory = scratch("nest-step-cap");
  let document = write_nesting_document(
    &directory,
    "brackets.c2d",
    vec![l_bracket()],
    &[("Bracket", &[1], 1)],
    &[("S1", 1000.0, 600.0)],
    &[("Bracket", "S1", 0.0, 0.0)],
  );
  let before = std::fs::read(&document).expect("read");

  let outcome = run(&["nest", document.to_str().expect("utf8"), "--step", "0.2"]);
  assert_eq!(outcome.code, 2, "{}", outcome.stderr);
  assert!(
    outcome.stderr.contains(
      "a 0.2 mm search step over this board needs 5001 x 3001 = 15008001 anchors, over the 4000000 \
       anchor cap; use a coarser --step"
    ),
    "the refusal names the lattice and the cap: {}",
    outcome.stderr
  );
  assert_eq!(
    std::fs::read(&document).expect("read"),
    before,
    "a refused step leaves the document alone"
  );

  // The same board at 2 mm is 501 x 301 = 150 801 anchors, so the step is the caller's again: the run
  // reaches the search and its report says which step it searched with. Asserting that is the
  // difference between proving the accepted behaviour and accepting any exit other than `2` (review
  // `review-2b7f8544a4b4dbd6`, finding `R2-001`).
  let outcome = run(&["nest", document.to_str().expect("utf8"), "--step", "2"]);
  assert!(
    matches!(outcome.code, 0 | 7),
    "a nesting run ends in 0, or in 7 when it cannot place everything, got {}: {}",
    outcome.code,
    outcome.stderr
  );
  assert!(
    outcome
      .stdout
      .contains("free-profile no-fit heuristic at 2.000 mm"),
    "the report names the step the search used: {}",
    outcome.stdout
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn nest_refuses_the_flags_a_documents_own_classes_give_no_meaning_to_by_name() {
  // The moulding's rule, at this command's door: a flag that was never applied still looks applied, so
  // a run takes no `--step` when every part is a rectangle (the no-fit heuristic never searches) and
  // no `--algorithm` when no part is one (neither maxrects nor guillotine packs anything).
  let directory = scratch("nest-inapplicable");
  let rectangles = write_nesting_document(
    &directory,
    "panels.c2d",
    vec![rectangle_contour(600.0, 400.0)],
    &[("P1", &[1], 1)],
    &[("S1", 1000.0, 600.0)],
    &[("P1", "S1", 0.0, 0.0)],
  );
  let profiles = write_nesting_document(
    &directory,
    "brackets.c2d",
    vec![l_bracket()],
    &[("Bracket", &[1], 1)],
    &[("S1", 1000.0, 600.0)],
    &[("Bracket", "S1", 0.0, 0.0)],
  );

  for (document, flag, value) in [
    (&rectangles, "--step", "2"),
    (&rectangles, "--step", "10"),
    (&profiles, "--algorithm", "guillotine"),
    (&profiles, "--algorithm", "auto"),
  ] {
    let before = std::fs::read(document).expect("read");
    let outcome = run(&["nest", document.to_str().expect("utf8"), flag, value]);
    assert_eq!(outcome.code, 2, "{flag} {value}: {}", outcome.stderr);
    assert!(
      outcome.stderr.contains(flag) && outcome.stderr.contains("takes no"),
      "the refusal names {flag}: {}",
      outcome.stderr
    );
    assert_eq!(
      std::fs::read(document).expect("read"),
      before,
      "{flag} writes nothing"
    );
  }
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn nest_refuses_a_flag_value_it_cannot_honour_by_name() {
  let directory = scratch("nest-flag-values");
  let rectangles = write_nesting_document(
    &directory,
    "panels.c2d",
    vec![rectangle_contour(600.0, 400.0)],
    &[("P1", &[1], 1)],
    &[("S1", 1000.0, 600.0)],
    &[("P1", "S1", 0.0, 0.0)],
  );
  let profiles = write_nesting_document(
    &directory,
    "brackets.c2d",
    vec![l_bracket()],
    &[("Bracket", &[1], 1)],
    &[("S1", 1000.0, 600.0)],
    &[("Bracket", "S1", 0.0, 0.0)],
  );

  // Each refusal names the flag and the value it refused, and nothing is written.
  for (document, arguments) in [
    (&rectangles, vec!["--rotation", "sideways"]),
    (&rectangles, vec!["--algorithm", "triangle"]),
    (&rectangles, vec!["--gap", "-1"]),
    (&rectangles, vec!["--sheets", "1000"]),
    (&rectangles, vec!["--sheets", "0x600"]),
    (&profiles, vec!["--step", "0"]),
  ] {
    let before = std::fs::read(document).expect("read");
    let mut command = vec!["nest", document.to_str().expect("utf8")];
    command.extend_from_slice(&arguments);
    let outcome = run(&command);
    assert_eq!(outcome.code, 2, "{arguments:?}: {}", outcome.stderr);
    let flag = arguments[0];
    assert!(
      outcome.stderr.contains(flag),
      "the refusal names {flag}: {}",
      outcome.stderr
    );
    assert!(
      outcome.stderr.contains(arguments[1]),
      "the refusal names the value {:?}: {}",
      arguments[1],
      outcome.stderr
    );
    assert_eq!(
      std::fs::read(document).expect("read"),
      before,
      "{arguments:?} writes nothing"
    );
  }

  // A flag from another command is not this command's to ignore: `--cut-depth` here would be a depth
  // nobody applied, so it is refused as an option this command does not have.
  let before = std::fs::read(&rectangles).expect("read");
  let outcome = run(&[
    "nest",
    rectangles.to_str().expect("utf8"),
    "--cut-depth",
    "5",
  ]);
  assert_eq!(outcome.code, 2, "{}{}", outcome.stdout, outcome.stderr);
  assert!(
    outcome.stderr.contains("--cut-depth") && outcome.stderr.contains("unknown nest option"),
    "the refusal names the flag it does not have: {}",
    outcome.stderr
  );
  assert_eq!(std::fs::read(&rectangles).expect("read"), before);

  // Boards of two sizes cannot be one run: both nesters pack identical boards, so a second size is
  // refused by name instead of silently packed at the first board's size.
  let two_sizes = run(&[
    "nest",
    rectangles.to_str().expect("utf8"),
    "--sheets",
    "1000x600",
    "--sheets",
    "800x600",
  ]);
  assert_eq!(
    two_sizes.code, 2,
    "{}{}",
    two_sizes.stdout, two_sizes.stderr
  );
  assert!(
    two_sizes.stderr.contains("identical boards") && two_sizes.stderr.contains("800.000"),
    "the refusal names both boards: {}",
    two_sizes.stderr
  );
  assert_eq!(std::fs::read(&rectangles).expect("read"), before);
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn nest_refuses_a_document_with_no_parts_or_no_boards_by_name() {
  let directory = scratch("nest-nothing-to-nest");

  // Boards are authored, never defaulted (S4), so a document that authors none names --sheets.
  let no_boards = write_nesting_document(
    &directory,
    "no-boards.c2d",
    vec![rectangle_contour(600.0, 400.0)],
    &[("P1", &[1], 0)],
    &[],
    &[],
  );
  let before = std::fs::read(&no_boards).expect("read");
  let outcome = run(&["nest", no_boards.to_str().expect("utf8")]);
  assert_eq!(outcome.code, 2, "{}{}", outcome.stdout, outcome.stderr);
  assert!(
    outcome.stderr.contains("--sheets"),
    "the refusal names the flag that would author them: {}",
    outcome.stderr
  );
  assert_eq!(std::fs::read(&no_boards).expect("read"), before);

  // And a document with boards but no part has nothing to nest: no default part list either.
  let no_parts = write_nesting_document(
    &directory,
    "no-parts.c2d",
    vec![rectangle_contour(600.0, 400.0)],
    &[],
    &[("S1", 1000.0, 600.0)],
    &[],
  );
  let before = std::fs::read(&no_parts).expect("read");
  let outcome = run(&["nest", no_parts.to_str().expect("utf8")]);
  assert_eq!(outcome.code, 2, "{}{}", outcome.stdout, outcome.stderr);
  assert!(
    outcome.stderr.contains("part"),
    "the refusal says what is missing: {}",
    outcome.stderr
  );
  assert_eq!(std::fs::read(&no_parts).expect("read"), before);
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn nest_refuses_a_part_that_is_not_one_closed_contour_by_name() {
  // Neither nester accepts a selection of several contours or an open wire, and nesting one part's
  // bounding box while calling it two parts would be a report nobody could reconcile with the cut. The
  // refusal names the part, so the author knows which selection to fix.
  let directory = scratch("nest-part-shape");
  let two_entities = write_nesting_document(
    &directory,
    "two-entities.c2d",
    vec![
      rectangle_contour(300.0, 200.0),
      rectangle_contour(100.0, 100.0),
    ],
    &[("P1", &[1, 2], 1)],
    &[("S1", 1000.0, 600.0)],
    &[("P1", "S1", 0.0, 0.0)],
  );
  let before = std::fs::read(&two_entities).expect("read");
  let outcome = run(&["nest", two_entities.to_str().expect("utf8")]);
  assert_eq!(outcome.code, 2, "{}{}", outcome.stdout, outcome.stderr);
  assert!(
    outcome.stderr.contains("P1") && outcome.stderr.contains("one closed contour"),
    "the refusal names the part and the rule: {}",
    outcome.stderr
  );
  assert_eq!(std::fs::read(&two_entities).expect("read"), before);

  let open = write_nesting_document(
    &directory,
    "open.c2d",
    vec![
      // The wire has to have extent. A flat one has no positive-area footprint at all, so
      // `Document::validate` refuses the placement before `nest` ever reads the document (the model's
      // `NestingPlacementWithoutFootprint`), and this test's subject — the nester refusing a part that
      // is not one closed contour, by name — would then be asserted on nobody's behalf. The rule this
      // exercises is about the *shape* of the selection, not about its area.
      Contour::from_points(
        &[
          Point2::new(0.0, 0.0),
          Point2::new(100.0, 0.0),
          Point2::new(100.0, 60.0),
        ],
        false,
      )
      .expect("open wire"),
    ],
    &[("Rail", &[1], 1)],
    &[("S1", 1000.0, 600.0)],
    &[("Rail", "S1", 0.0, 0.0)],
  );
  let before = std::fs::read(&open).expect("read");
  let outcome = run(&["nest", open.to_str().expect("utf8")]);
  assert_eq!(outcome.code, 2, "{}{}", outcome.stdout, outcome.stderr);
  assert!(
    outcome.stderr.contains("Rail") && outcome.stderr.contains("open"),
    "the refusal names the part and what is wrong with it: {}",
    outcome.stderr
  );
  assert_eq!(std::fs::read(&open).expect("read"), before);
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn two_nest_runs_print_and_write_identical_bytes() {
  // Determinism is a test and not a hope (invariant 4). The fixture mixes both classes, so both
  // nesters run, and it authors its boards on the command line, so the second run has to reproduce the
  // sheet list the first one wrote as well as its placements and its report.
  let directory = scratch("nest-determinism");
  let document = write_nesting_document(
    &directory,
    "mixed.c2d",
    vec![rectangle_contour(600.0, 400.0), l_bracket()],
    &[("Panel", &[1], 2), ("Bracket", &[2], 1)],
    &[
      ("S1", 1000.0, 600.0),
      ("S2", 1000.0, 600.0),
      ("S3", 1000.0, 600.0),
    ],
    &[
      ("Panel", "S1", 0.0, 0.0),
      ("Panel", "S2", 0.0, 0.0),
      ("Bracket", "S3", 0.0, 0.0),
    ],
  );
  let arguments = [
    "nest",
    document.to_str().expect("utf8"),
    "--sheets",
    "1000x600",
    "--sheets",
    "1000x600",
    "--gap",
    "8",
  ];

  let first = run(&arguments);
  assert_eq!(first.code, 0, "{}{}", first.stdout, first.stderr);
  let first_bytes = std::fs::read(&document).expect("read");

  let second = run(&arguments);
  assert_eq!(second.code, 0, "{}{}", second.stdout, second.stderr);
  let second_bytes = std::fs::read(&document).expect("read");

  assert_eq!(first.stdout, second.stdout, "the report is byte-identical");
  assert_eq!(first_bytes, second_bytes, "the document is byte-identical");

  // The mixed fixture is also the one that shows the two classes never share a board: neither nester
  // can be handed the other's placements as an occupied board, so the free profile is nested on sheets
  // of its own rather than on top of a panel.
  let nesting = load_nesting(&document);
  let panels: Vec<&str> = nesting
    .placements
    .iter()
    .filter(|placement| placement.part_id == "Panel")
    .map(|placement| placement.sheet_id.as_str())
    .collect();
  let brackets: Vec<&str> = nesting
    .placements
    .iter()
    .filter(|placement| placement.part_id == "Bracket")
    .map(|placement| placement.sheet_id.as_str())
    .collect();
  assert_eq!(panels, vec!["S1", "S2"]);
  assert!(
    brackets.iter().all(|sheet| !panels.contains(sheet)),
    "the free profile gets a board of its own: {brackets:?} against {panels:?}"
  );
  assert_eq!(
    nesting.sheets.len(),
    3,
    "every board a placement names is authored in the document"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn the_algorithm_flag_names_the_heuristic_the_report_reports() {
  // Both heuristics S6 names are reachable from the command surface by name, and the report says which
  // one packed instead of leaving the caller to infer it.
  let directory = scratch("nest-algorithm");
  for (requested, reported) in [
    ("auto", "maxrects"),
    ("maxrects", "maxrects"),
    ("guillotine", "guillotine"),
  ] {
    let document = write_nesting_document(
      &directory,
      "panels.c2d",
      vec![rectangle_contour(600.0, 400.0)],
      &[("P1", &[1], 1)],
      &[("S1", 1000.0, 600.0)],
      &[("P1", "S1", 0.0, 0.0)],
    );
    let outcome = run(&[
      "nest",
      document.to_str().expect("utf8"),
      "--algorithm",
      requested,
    ]);
    assert_eq!(outcome.code, 0, "{requested}: {}", outcome.stderr);
    let expected = format!("algorithm: rectangle, {reported}\n");
    assert!(
      outcome.stdout.contains(&expected),
      "--algorithm {requested} reports {reported}: {}",
      outcome.stdout
    );
  }
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn nest_writes_the_out_path_and_leaves_the_input_document_alone() {
  // `--out` is the write-back surface, exactly as `toolpath` has one: without it the document is
  // rewritten in place, with it the nesting lands where the caller asked and the input keeps every
  // byte it had.
  let directory = scratch("nest-out");
  let document = write_nesting_document(
    &directory,
    "panel.c2d",
    vec![rectangle_contour(600.0, 400.0)],
    &[("P1", &[1], 1)],
    &[("S1", 1000.0, 600.0)],
    &[("P1", "S1", 0.0, 0.0)],
  );
  let before = std::fs::read(&document).expect("read");
  let out = directory.join("nested.c2d");

  let outcome = run(&[
    "nest",
    document.to_str().expect("utf8"),
    "--out",
    out.to_str().expect("utf8"),
  ]);
  assert_eq!(outcome.code, 0, "{}{}", outcome.stdout, outcome.stderr);
  assert_eq!(std::fs::read(&document).expect("read"), before);
  assert_eq!(load_nesting(&out).placements.len(), 1);
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn the_help_lists_the_nest_command_its_flags_and_its_exit_code() {
  // Invariant 6: a command, its flags and a new exit code are documented where a caller looks first.
  let outcome = run(&["--help"]);
  assert_eq!(outcome.code, 0);
  for expected in [
    "nest <document>",
    "--sheets <WxH>",
    "--gap <mm>",
    "--rotation <on|off>",
    "--algorithm <mode>",
    "--step <mm>",
    "unplaced",
    "7   a nesting could not place every instance",
  ] {
    assert!(
      outcome.stdout.contains(expected),
      "help is missing {expected:?}:\n{}",
      outcome.stdout
    );
  }
}

#[test]
fn a_quarter_turn_places_a_part_the_board_only_holds_sideways() {
  // A 500 x 900 mm part on a 1000 x 600 mm board fits only turned, so `--rotation on` is the one way it
  // enters — and the turn reaches the document as a quarter turn counter-clockwise about the part's own
  // footprint corner (D-M9-1). The test then asks the document's own footprint derivation where the
  // material sits: `Document::placement_footprint` has to agree with the 900 x 500 mm the report
  // printed, or the placement a nester chose and the placement the model validates would be two
  // different rectangles.
  let directory = scratch("nest-quarter-turn");
  let document = write_nesting_document(
    &directory,
    "rail.c2d",
    vec![rectangle_contour(500.0, 900.0)],
    &[("P1", &[1], 1)],
    // Authored on a board tall enough to hold it upright, then re-nested onto the short board that
    // only holds it turned.
    &[("S1", 1000.0, 1000.0)],
    &[("P1", "S1", 0.0, 0.0)],
  );

  let outcome = run(&[
    "nest",
    document.to_str().expect("utf8"),
    "--sheets",
    "1000x600",
    "--rotation",
    "on",
  ]);
  assert_eq!(outcome.code, 0, "{}{}", outcome.stdout, outcome.stderr);
  let expected = "\
nest: 1 sheet(s) of 1000.000 x 600.000 mm, 1 part(s), 1 instance(s)
options: gap 0.000 mm, rotation on
sheet S1: used
part: P1 rectangle, 1 instance(s)
placement: P1 on S1 at (0.000, 0.000), rotation quarter turn, 900.000 x 500.000 mm
algorithm: rectangle, maxrects
sheets used: 1
utilisation: 0.750
waste: 0.250
unplaced: none
";
  assert_eq!(outcome.stdout, expected);

  let loaded = cam_store::document::load_document(&document).expect("loads");
  let nesting = loaded.nesting.as_ref().expect("the section is written");
  assert_eq!(
    nesting.placements[0].rotation,
    Angle::from_radians(FRAC_PI_2)
  );
  let footprint = loaded
    .placement_footprint(&nesting.placements[0])
    .expect("the placed footprint");
  let bounds = footprint.bounds();
  // The turn is arithmetic on floating-point cosines, so the corners are compared within the crate's
  // tolerance rather than bit for bit; the report's three decimals are what a reader sees.
  assert!(
    bounds.min.x.abs() < 1e-9 && bounds.min.y.abs() < 1e-9,
    "{bounds:?}"
  );
  assert!(
    (bounds.max.x - 900.0).abs() < 1e-9 && (bounds.max.y - 500.0).abs() < 1e-9,
    "{bounds:?}"
  );
  let cost = nesting.cost.as_ref().expect("the cost record is written");
  assert_eq!(Some(cost.utilisation), loaded.nesting_utilisation());
  assert!(
    (cost.utilisation - 0.75).abs() < 1e-12,
    "{}",
    cost.utilisation
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn the_first_nesting_takes_an_authored_document_with_parts_and_no_placements() {
  // The state the user authors (S1): two boards and two parts with their instance counts, no
  // placement and no cost. Until the model's instance rule learnt this state, a section that asked for
  // instances and placed none was refused at load, so `nest` could only re-nest a document a run had
  // already nested — and a fresh document had no CLI route to its first nesting. This is that route.
  //
  // The run has to honour the authored counts (2 panels, 1 shelf) and fill the authored boards in
  // order: the packer's own placements, the part and sheet ids they name, the cost record the document
  // derives from them, and nothing invented.
  let directory = scratch("nest-first");
  let document = write_nesting_document(
    &directory,
    "authored.c2d",
    vec![
      rectangle_contour(600.0, 400.0),
      rectangle_contour(300.0, 200.0),
    ],
    &[("Panel", &[1], 2), ("Shelf", &[2], 1)],
    &[("S1", 1000.0, 600.0), ("S2", 1000.0, 600.0)],
    &[],
  );

  // Before the run the document is exactly the request: the section is there, and it places nothing.
  let authored = load_nesting(&document);
  assert!(authored.placements.is_empty(), "the request places nothing");
  assert!(authored.cost.is_none(), "the request carries no cost");

  let outcome = run(&["nest", document.to_str().expect("utf8")]);
  assert_eq!(outcome.code, 0, "{}{}", outcome.stdout, outcome.stderr);
  assert!(outcome.stderr.is_empty(), "{}", outcome.stderr);
  let expected = "\
nest: 2 sheet(s) of 1000.000 x 600.000 mm, 2 part(s), 3 instance(s)
options: gap 0.000 mm, rotation off
sheet S1: used
sheet S2: used
part: Panel rectangle, 2 instance(s)
placement: Panel on S1 at (0.000, 0.000), rotation none, 600.000 x 400.000 mm
placement: Panel on S2 at (0.000, 0.000), rotation none, 600.000 x 400.000 mm
part: Shelf rectangle, 1 instance(s)
placement: Shelf on S1 at (0.000, 400.000), rotation none, 300.000 x 200.000 mm
algorithm: rectangle, maxrects
sheets used: 2
utilisation: 0.450
waste: 0.550
unplaced: none
";
  assert_eq!(outcome.stdout, expected);

  // The written document holds the placements the report printed, naming the authored parts and the
  // authored boards — the request became a nesting, and no board had to be invented for it.
  let loaded = cam_store::document::load_document(&document).expect("loads");
  let nesting = loaded.nesting.as_ref().expect("the section is written");
  assert_eq!(
    nesting.placements,
    vec![
      NestingPlacement {
        part_id: "Panel".to_owned(),
        sheet_id: "S1".to_owned(),
        position: Point2::new(0.0, 0.0),
        rotation: Angle::ZERO,
      },
      NestingPlacement {
        part_id: "Panel".to_owned(),
        sheet_id: "S2".to_owned(),
        position: Point2::new(0.0, 0.0),
        rotation: Angle::ZERO,
      },
      NestingPlacement {
        part_id: "Shelf".to_owned(),
        sheet_id: "S1".to_owned(),
        position: Point2::new(0.0, 400.0),
        rotation: Angle::ZERO,
      },
    ]
  );
  let sheets: Vec<&str> = nesting
    .sheets
    .iter()
    .map(|sheet| sheet.id.as_str())
    .collect();
  assert_eq!(sheets, vec!["S1", "S2"], "the authored boards, in order");

  // The cost record is the document's own arithmetic and nothing else (T1c): two 600 x 400 panels and
  // one 300 x 200 shelf are 540 000 mm2 of the two 1000 x 600 mm boards' 1 200 000 mm2, so 0.45 — the
  // fraction `Document::nesting_utilisation` derives from the placements the run just wrote.
  let cost = nesting.cost.as_ref().expect("the cost record is written");
  assert_eq!(cost.sheets_used, 2);
  assert_eq!(
    Some(cost.utilisation),
    loaded.nesting_utilisation(),
    "the record is exactly the fraction the document derives"
  );
  assert!(
    (cost.utilisation - 0.45).abs() < 1e-12,
    "{}",
    cost.utilisation
  );
  assert!((cost.waste - 0.55).abs() < 1e-12, "{}", cost.waste);
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_first_nested_document_re_nests_to_identical_bytes() {
  // Determinism (invariant 4) across the one state change this command exists for: the first run
  // answers an authored request, the second re-nests the nesting it wrote. Both states are valid, so
  // both runs are, and the command is idempotent on its own output — the second run prints the same
  // report and leaves the same bytes.
  //
  // The two instances must fit the **one** authored sheet: a fixture that only fits because `nest`
  // opens a second sheet would prove sheet growth and idempotence at once, and a reader could not
  // tell which of the two made it pass. 400 x 400 twice on 1000 x 600 fits upright, with rotation
  // off, in one sheet — so this test is about the bytes and nothing else.
  let directory = scratch("nest-first-determinism");
  let document = write_nesting_document(
    &directory,
    "authored.c2d",
    vec![rectangle_contour(400.0, 400.0)],
    &[("P1", &[1], 2)],
    &[("S1", 1000.0, 600.0)],
    &[],
  );
  let arguments = ["nest", document.to_str().expect("utf8")];

  let first = run(&arguments);
  assert_eq!(first.code, 0, "{}{}", first.stdout, first.stderr);
  let after_first = load_nesting(&document);
  assert_eq!(
    after_first.placements.len(),
    2,
    "the first run placed both authored instances"
  );
  assert_eq!(
    after_first.sheets.len(),
    1,
    "both instances fit the authored sheet, so the run opened no second one"
  );
  let first_bytes = std::fs::read(&document).expect("read");

  let second = run(&arguments);
  assert_eq!(second.code, 0, "{}{}", second.stdout, second.stderr);
  let second_bytes = std::fs::read(&document).expect("read");

  assert_eq!(first.stdout, second.stdout, "the report is byte-identical");
  assert_eq!(first_bytes, second_bytes, "the document is byte-identical");
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_first_nesting_that_cannot_fit_leaves_the_authored_request_untouched() {
  // A request the run cannot satisfy is a finding about the request (exit 7), and the request survives
  // it: a 1200 x 500 mm part on the authored 1000 x 600 mm board fits nowhere upright, so the report
  // names the part and how many of its copies landed, and the document keeps every byte it had. The
  // existing exit-7 fixture re-nests a nested document; this one is the **authored** state, which only
  // just became legal, so a command that rewrote it would destroy the request it failed to answer.
  let directory = scratch("nest-first-unplaced");
  let document = write_nesting_document(
    &directory,
    "wide.c2d",
    vec![rectangle_contour(1200.0, 500.0)],
    &[("P1", &[1], 1)],
    &[("S1", 1000.0, 600.0)],
    &[],
  );
  let before = std::fs::read(&document).expect("read");

  let outcome = run(&["nest", document.to_str().expect("utf8")]);
  assert_eq!(outcome.code, 7, "{}{}", outcome.stdout, outcome.stderr);
  let expected = "\
nest: 1 sheet(s) of 1000.000 x 600.000 mm, 1 part(s), 1 instance(s)
options: gap 0.000 mm, rotation off
sheet S1: unused
part: P1 rectangle, 1 instance(s)
algorithm: rectangle, maxrects
sheets used: 0
utilisation: none
waste: none
unplaced: P1, 1 of 1 instance(s)
";
  assert_eq!(outcome.stdout, expected);
  assert!(
    outcome.stderr.contains("P1") && outcome.stderr.contains("unplaced"),
    "the failure names the part and the finding: {}",
    outcome.stderr
  );
  assert_eq!(
    std::fs::read(&document).expect("read"),
    before,
    "a run that could not satisfy the request writes nothing"
  );
  std::fs::remove_dir_all(&directory).ok();
}

/// the command that crosses from the first to the second.
fn write_nesting_document(
  directory: &Path,
  name: &str,
  contours: Vec<Contour>,
  parts: &[(&str, &[usize], usize)],
  sheets: &[(&str, f64, f64)],
  placements: &[(&str, &str, f64, f64)],
) -> PathBuf {
  let mut document = Document::new(Job {
    width: Length::from_mm(200.0),
    height: Length::from_mm(100.0),
    thickness: Length::from_mm(18.0),
    units: UnitSystem::Mm,
    origin: cam_model::OriginCorner::BottomLeft,
    safe_z: Length::from_mm(5.0),
    material: None,
  });
  document.layers.push(Layer {
    id: "L1".to_owned(),
    name: "Parts".to_owned(),
    role: LayerRole::Geometry,
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
  document.nesting = Some(Nesting {
    sheets: sheets
      .iter()
      .map(|(id, width, height)| NestingSheet {
        id: (*id).to_owned(),
        width: Length::from_mm(*width),
        height: Length::from_mm(*height),
      })
      .collect(),
    parts: parts
      .iter()
      .map(|(id, entities, instances)| NestingPart {
        id: (*id).to_owned(),
        entity_ids: entities.iter().map(|index| format!("E{index}")).collect(),
        instances: *instances,
      })
      .collect(),
    placements: placements
      .iter()
      .map(|(part, sheet, x, y)| NestingPlacement {
        part_id: (*part).to_owned(),
        sheet_id: (*sheet).to_owned(),
        position: Point2::new(*x, *y),
        rotation: Angle::ZERO,
      })
      .collect(),
    cost: None,
  });
  let path = directory.join(name);
  save_document(&path, &document).expect("document saved");
  path
}

/// An L-shaped bracket: 60 000 mm2 of material inside a 300 x 400 mm box, 120 000 mm2, so its
/// footprint is not the rectangle its bounds suggest and only the free-profile heuristic can place it.
fn l_bracket() -> Contour {
  Contour::from_points(
    &[
      Point2::new(0.0, 0.0),
      Point2::new(300.0, 0.0),
      Point2::new(300.0, 100.0),
      Point2::new(100.0, 100.0),
      Point2::new(100.0, 400.0),
      Point2::new(0.0, 400.0),
    ],
    true,
  )
  .expect("L bracket")
}

fn load_nesting(path: &Path) -> Nesting {
  cam_store::document::load_document(path)
    .expect("the nested document loads")
    .nesting
    .expect("the nested document carries a nesting section")
}
