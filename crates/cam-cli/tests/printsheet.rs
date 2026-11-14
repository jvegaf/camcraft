//! End-to-end tests for the HTML print sheet (M10 T3, S5: "HTML print sheets").
//!
//! These run the real binary, so they assert the observable contract: the exit code, the exact bytes
//! on stdout or in `--out`, the sheet a document renders to, and the refusal a document that does not
//! validate produces.

use std::path::{Path, PathBuf};
use std::process::Command;

use cam_geom::{Contour, Point2};
use cam_model::{
  Document, Entity, EntityKind, IsolationKind, IsolationParams, Job, Layer, LayerRole,
  MillingDirection, Nesting, NestingPart, NestingPlacement, NestingSheet, Operation, Toolpath,
};
use cam_store::document::save_document;
use cam_units::{Angle, Length};

fn binary() -> &'static str {
  env!("CARGO_BIN_EXE_cam-cli")
}

/// A scratch directory under `/var/tmp`: the brief forbids `/tmp`, which is a full tmpfs here.
fn scratch(name: &str) -> PathBuf {
  let directory =
    PathBuf::from("/var/tmp").join(format!("campilot-printsheet-{}-{name}", std::process::id()));
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

fn job() -> Job {
  Job {
    width: Length::from_mm(200.0),
    height: Length::from_mm(100.0),
    thickness: Length::from_mm(18.0),
    origin: cam_model::OriginCorner::BottomLeft,
    side_up: cam_model::BoardSideUp::Top,
  }
}

/// A document holding one layer of rectangle entities, the toolpaths it is given and, when given,
/// that nesting section. Nothing else: every test starts from a document whose state it can state.
fn write_document(
  directory: &Path,
  name: &str,
  layer_id: &str,
  entities: &[(&str, f64, f64)],
  toolpaths: Vec<Toolpath>,
  nesting: Option<Nesting>,
) -> PathBuf {
  let mut document = Document::new(job());
  document.layers.push(Layer {
    id: layer_id.to_owned(),
    role: LayerRole::Outline,
    side: cam_model::Side::Both,
    color: "#000000".to_owned(),
    visible: true,
    source: cam_model::Provenance::default(),
    entities: entities
      .iter()
      .map(|(id, width, height)| Entity {
        id: (*id).to_owned(),
        kind: EntityKind::Contour {
          contour: rectangle_contour(*width, *height),
        },
      })
      .collect(),
  });
  document.toolpaths = toolpaths;
  document.nesting = nesting;

  let path = directory.join(name);
  save_document(&path, &document).expect("document saved");
  path
}

/// One profile toolpath outside `entity_ids` with tool `tool`, and the parameters a sheet prints.
fn profile_toolpath(id: &str, name: &str, entity_ids: &[&str], tool: u32) -> Toolpath {
  Toolpath {
    id: id.to_owned(),
    name: name.to_owned(),
    entity_ids: entity_ids.iter().map(|id| (*id).to_owned()).collect(),
    tool_number: tool,
    operation: Operation::Isolation(IsolationParams {
      passes: 1,
      overlap: 0.5,
      direction: MillingDirection::Climb,
      kind: IsolationKind::Full,
      cut_depth: Length::from_mm(0.1),
      keep_pours: false,
    }),
  }
}

fn sheet(id: &str, width: f64, height: f64) -> NestingSheet {
  NestingSheet {
    id: id.to_owned(),
    width: Length::from_mm(width),
    height: Length::from_mm(height),
  }
}

fn part(id: &str, entity_ids: &[&str], instances: usize) -> NestingPart {
  NestingPart {
    id: id.to_owned(),
    entity_ids: entity_ids.iter().map(|id| (*id).to_owned()).collect(),
    instances,
  }
}

fn placement(part_id: &str, sheet_id: &str, x: f64, y: f64) -> NestingPlacement {
  NestingPlacement {
    part_id: part_id.to_owned(),
    sheet_id: sheet_id.to_owned(),
    position: Point2::new(x, y),
    rotation: Angle::from_degrees(0.0),
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

/// Point the document's first toolpath at an entity the document does not hold, which is a
/// referential invariant [`cam_model::Document::validate`] refuses by name.
fn point_the_toolpath_at_an_unknown_entity(document: &Path) {
  let payload = document_payload(document);
  let at = payload.find("\"entity_ids\"").expect("entity_ids");
  let entity = payload[at..]
    .find("\"E1\"")
    .map(|offset| at + offset)
    .expect("the toolpath names E1");
  let broken = format!(
    "{}\"E9\"{}",
    &payload[..entity],
    &payload[entity + "\"E1\"".len()..]
  );
  assert!(broken.contains("\"E9\""), "the toolpath now names E9");
  rewrite_payload(document, &broken);
}

struct Outcome {
  code: i32,
  stdout: String,
  stderr: String,
}

impl Outcome {
  fn stdout_bytes(&self) -> &[u8] {
    self.stdout.as_bytes()
  }
}

fn outcome(output: std::process::Output) -> Outcome {
  Outcome {
    code: output.status.code().unwrap_or(-1),
    stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
    stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
  }
}

fn run(arguments: &[&str]) -> Outcome {
  outcome(
    Command::new(binary())
      .args(arguments)
      .output()
      .expect("the binary runs"),
  )
}

/// The whole sheet the golden document renders to, byte for byte: one self-contained HTML file, one
/// section per thing the document is, and the utilisation `Document::nesting_utilisation` derives.
const GOLDEN_SHEET: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Print sheet</title>
<style>
body { margin: 2rem; color: #111; font-family: Georgia, serif; }
h1 { margin: 0 0 0.25rem; font-size: 1.6rem; }
h2 { margin: 2rem 0 0.5rem; font-size: 1.2rem; border-bottom: 1px solid #999; }
h3 { margin: 1rem 0 0.25rem; font-size: 1rem; }
p { margin: 0.25rem 0; }
table { margin: 0.5rem 0; border-collapse: collapse; }
th, td { padding: 0.2rem 0.6rem; border: 1px solid #bbb; text-align: left; }
th { background: #eee; }
.empty { font-style: italic; }
</style>
</head>
<body>
<h1>Print sheet</h1>
<p>schema camcraft/document@1; figures in millimetres</p>
<h2>Job</h2>
<table>
<tr><th>width</th><td>200.000 mm</td></tr>
<tr><th>height</th><td>100.000 mm</td></tr>
<tr><th>thickness</th><td>18.000 mm</td></tr>
<tr><th>origin</th><td>bottom left</td></tr>
<tr><th>safe Z</th><td>5.000 mm</td></tr>
</table>
<h2>Layers</h2>
<table>
<tr><th>layer</th><th>role/side</th><th>visible</th><th>entities</th></tr>
<tr><td>L1</td><td>Outline/Both</td><td>yes</td><td>2</td></tr>
</table>
<h2>Toolpaths</h2>
<h3>TP1 — Outside profile</h3>
<table>
<tr><th>operation</th><td>isolation</td></tr>
<tr><th>tool</th><td>T1</td></tr>
<tr><th>entities</th><td>E1</td></tr>
<tr><th>passes</th><td>1</td></tr>
<tr><th>overlap</th><td>0.500</td></tr>
<tr><th>direction</th><td>climb</td></tr>
<tr><th>kind</th><td>full</td></tr>
<tr><th>cut depth</th><td>0.100 mm</td></tr>
<tr><th>keep pours</th><td>false</td></tr>
</table>
<h2>Tools</h2>
<table>
<tr><th>tool</th><th>toolpaths</th></tr>
<tr><td>T1</td><td>1</td></tr>
</table>
<p>diameter, feeds and speeds: omitted, because the document names a tool number and the tool library is not part of the document.</p>
<h2>Nesting</h2>
<table>
<tr><th>sheet</th><th>width</th><th>height</th><th>placements</th></tr>
<tr><td>S1</td><td>200.000 mm</td><td>100.000 mm</td><td>1</td></tr>
</table>
<table>
<tr><th>part</th><th>entities</th><th>instances</th></tr>
<tr><td>P1</td><td>E1</td><td>1</td></tr>
</table>
<table>
<tr><th>part</th><th>sheet</th><th>x</th><th>y</th><th>rotation</th></tr>
<tr><td>P1</td><td>S1</td><td>0.000 mm</td><td>0.000 mm</td><td>0.000 deg</td></tr>
</table>
<p>utilisation: 0.060 of the used sheets' area; waste: 0.940</p>
<h2>Omitted</h2>
<p>the program: omitted, because a sheet reads the document and runs no post-processor.</p>
<p>the simulation: omitted, because a sheet reports the document and simulates nothing.</p>
<p>the file name: omitted, because the document does not record one, so two copies of one document print one sheet.</p>
</body>
</html>
"#;

/// The document the golden sheet is rendered from: a job, one layer with two entities, one profile
/// toolpath and a nesting holding one sheet, one part and one placement.
fn golden_document(directory: &Path) -> PathBuf {
  write_document(
    directory,
    "golden.cmc",
    "L1",
    &[("E1", 40.0, 30.0), ("E2", 30.0, 20.0)],
    vec![profile_toolpath("TP1", "Outside profile", &["E1"], 1)],
    Some(Nesting {
      sheets: vec![sheet("S1", 200.0, 100.0)],
      parts: vec![part("P1", &["E1"], 1)],
      placements: vec![placement("P1", "S1", 0.0, 0.0)],
      cost: None,
    }),
  )
}

#[test]
fn the_print_sheet_is_the_golden_html() {
  let directory = scratch("golden");
  let document = golden_document(&directory);

  let printed = run(&["report", document.to_str().expect("utf8")]);
  assert_eq!(printed.code, 0, "{}", printed.stderr);
  assert!(printed.stderr.is_empty(), "{}", printed.stderr);
  assert_eq!(printed.stdout, GOLDEN_SHEET);

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn two_runs_of_the_print_sheet_are_byte_identical() {
  let directory = scratch("determinism");
  let document = golden_document(&directory);

  let first = run(&["report", document.to_str().expect("utf8")]);
  let second = run(&["report", document.to_str().expect("utf8")]);
  assert_eq!(first.code, 0, "{}", first.stderr);
  assert_eq!(second.code, 0, "{}", second.stderr);
  assert_eq!(first.stdout_bytes(), second.stdout_bytes());
  assert_eq!(first.stdout, GOLDEN_SHEET);

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn out_and_stdout_carry_the_same_sheet() {
  let directory = scratch("out");
  let document = golden_document(&directory);

  let printed = run(&["report", document.to_str().expect("utf8")]);
  assert_eq!(printed.code, 0, "{}", printed.stderr);

  let out = directory.join("sheet.html");
  let written = run(&[
    "report",
    document.to_str().expect("utf8"),
    "--out",
    out.to_str().expect("utf8"),
  ]);
  assert_eq!(written.code, 0, "{}", written.stderr);
  assert!(written.stdout.is_empty(), "{}", written.stdout);
  assert_eq!(
    std::fs::read(&out).expect("sheet written"),
    printed.stdout_bytes(),
    "--out writes the same bytes stdout prints"
  );

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_hostile_layer_name_and_entity_id_cannot_inject_markup() {
  let directory = scratch("escaping");
  let document = write_document(
    &directory,
    "hostile.cmc",
    "R&D <b>\"x\"</b> 'q'",
    &[("<script>alert(1)</script>", 40.0, 30.0)],
    vec![profile_toolpath(
      "TP1",
      "Outside profile",
      &["<script>alert(1)</script>"],
      1,
    )],
    None,
  );

  let printed = run(&["report", document.to_str().expect("utf8")]);
  assert_eq!(printed.code, 0, "{}", printed.stderr);

  // Every character that can open a tag or end an attribute is escaped, in the layer id and in an entity id.
  assert!(
    printed
      .stdout
      .contains("<td>R&amp;D &lt;b&gt;&quot;x&quot;&lt;/b&gt; &#39;q&#39;</td>"),
    "the layer id is escaped:\n{}",
    printed.stdout
  );
  assert!(
    printed
      .stdout
      .contains("<td>&lt;script&gt;alert(1)&lt;/script&gt;</td>"),
    "the entity id is escaped:\n{}",
    printed.stdout
  );
  for injected in ["<script>", "<b>", "</script>"] {
    assert!(
      !printed.stdout.contains(injected),
      "{injected:?} must not survive as markup"
    );
  }

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_document_with_no_toolpath_still_prints_a_sheet_that_says_so() {
  let directory = scratch("no-toolpaths");
  let document = write_document(
    &directory,
    "bare.cmc",
    "L1",
    &[("E1", 40.0, 30.0)],
    Vec::new(),
    None,
  );

  let printed = run(&["report", document.to_str().expect("utf8")]);
  assert_eq!(printed.code, 0, "{}", printed.stderr);
  assert!(printed.stdout.starts_with("<!DOCTYPE html>\n"));
  assert!(printed.stdout.ends_with("</html>\n"));
  assert!(
    printed.stdout.contains(
      "<p class=\"empty\">no toolpath: the document holds none, so there is nothing to cut.</p>"
    ),
    "{}",
    printed.stdout
  );
  assert!(
    printed.stdout.contains(
      "<p class=\"empty\">no tool: the document holds no toolpath, so none names a tool.</p>"
    ),
    "{}",
    printed.stdout
  );
  assert!(
    printed
      .stdout
      .contains("<p class=\"empty\">no nesting: the document holds no nesting section"),
    "{}",
    printed.stdout
  );

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn the_nesting_section_reports_the_utilisation_the_document_derives() {
  let directory = scratch("utilisation");
  let document = write_document(
    &directory,
    "nested.cmc",
    "L1",
    &[("E1", 40.0, 30.0)],
    Vec::new(),
    Some(Nesting {
      sheets: vec![sheet("S1", 200.0, 100.0), sheet("S2", 400.0, 200.0)],
      parts: vec![part("P1", &["E1"], 2)],
      placements: vec![
        placement("P1", "S1", 0.0, 0.0),
        placement("P1", "S2", 0.0, 0.0),
      ],
      cost: None,
    }),
  );

  let printed = run(&["report", document.to_str().expect("utf8")]);
  assert_eq!(printed.code, 0, "{}", printed.stderr);

  // The sheet prints the fraction the document derives: 2 x (40 x 30) mm2 over 200 x 100 plus
  // 400 x 200 mm2 of used sheets, and waste its complement.
  let loaded = cam_store::document::load_document(&document).expect("loads");
  let derived = loaded
    .nesting_utilisation()
    .expect("two placements name two sheets");
  assert_eq!(cam_units::Format::fixed(3).apply(derived), "0.024");
  assert!(
    printed
      .stdout
      .contains("<p>utilisation: 0.024 of the used sheets' area; waste: 0.976</p>"),
    "{}",
    printed.stdout
  );
  // The sheets, parts and placements the document authors are the ones on the sheet.
  for row in [
    "<tr><td>S1</td><td>200.000 mm</td><td>100.000 mm</td><td>1</td></tr>",
    "<tr><td>S2</td><td>400.000 mm</td><td>200.000 mm</td><td>1</td></tr>",
    "<tr><td>P1</td><td>E1</td><td>2</td></tr>",
  ] {
    assert!(printed.stdout.contains(row), "missing {row:?}");
  }

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_document_that_does_not_validate_is_refused_by_name() {
  let directory = scratch("refusal");
  let document = write_document(
    &directory,
    "broken.cmc",
    "L1",
    &[("E1", 40.0, 30.0)],
    vec![profile_toolpath("TP1", "Outside profile", &["E1"], 1)],
    None,
  );
  point_the_toolpath_at_an_unknown_entity(&document);

  let out = directory.join("never.html");
  let refused = run(&[
    "report",
    document.to_str().expect("utf8"),
    "--out",
    out.to_str().expect("utf8"),
  ]);
  assert_eq!(refused.code, 2, "{}", refused.stderr);
  assert!(refused.stdout.is_empty(), "{}", refused.stdout);
  assert!(
    refused.stderr.contains("is not a valid document"),
    "{}",
    refused.stderr
  );
  assert!(
    refused
      .stderr
      .contains("toolpath \"TP1\" references unknown entity \"E9\""),
    "{}",
    refused.stderr
  );
  assert!(!out.exists(), "a refused sheet writes no file");

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn help_lists_the_report_command_and_its_flags() {
  let printed = run(&["--help"]);
  assert_eq!(printed.code, 0, "{}", printed.stderr);
  for needle in ["report <document>", "HTML print sheet", "--out <path>"] {
    assert!(
      printed.stdout.contains(needle),
      "help is missing {needle:?}:\n{}",
      printed.stdout
    );
  }
}
