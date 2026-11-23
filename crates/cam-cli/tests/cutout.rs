//! End-to-end tests for `cam-cli cutout` and `cam-cli program` (E2.3, E2.5).
//!
//! These run the real binary, so they assert the observable contract: exit code, exact stdout and
//! stderr, the files left on disk, and the program's own text.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use cam_geom::{Contour, Point2};
use cam_model::{
  CutoutParams, Document, Entity, EntityKind, Job, Layer, LayerRole, Operation, Tab, TabKind,
  Toolpath,
};
use cam_store::document::save_document;
use cam_units::Length;

const POST: &str = include_str!("../../cam-post/tests/fixtures/post_mm.pp");

fn binary() -> &'static str {
  env!("CARGO_BIN_EXE_cam-cli")
}

fn scratch(name: &str) -> PathBuf {
  let directory = std::env::temp_dir().join(format!("camcut-{}-{name}", std::process::id()));
  let _ = std::fs::remove_dir_all(&directory);
  std::fs::create_dir_all(&directory).expect("scratch directory");
  directory
}

fn board_contour(width: f64, height: f64) -> Contour {
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

/// A 60 x 40 mm board whose outline layer is cut out with two tabs, `tabs` deep.
fn document(tabs: Vec<Tab>) -> Document {
  let mut document = Document::new(Job {
    width: Length::from_mm(60.0),
    height: Length::from_mm(40.0),
    thickness: Length::from_mm(1.6),
    origin: cam_model::OriginCorner::BottomLeft,
    side_up: cam_model::BoardSideUp::Top,
  });
  document.layers.push(Layer {
    id: "L1".to_owned(),
    role: LayerRole::Outline,
    side: cam_model::Side::Both,
    color: "#000000".to_owned(),
    visible: true,
    source: cam_model::Provenance::default(),
    entities: vec![Entity {
      id: "E1".to_owned(),
      kind: EntityKind::Contour {
        contour: board_contour(60.0, 40.0),
      },
      source: cam_model::Provenance::default(),
    }],
  });
  document.toolpaths.push(Toolpath {
    id: "TP1".to_owned(),
    name: "Board cutout".to_owned(),
    entity_ids: vec!["E1".to_owned()],
    tool_id: None,
    tool_number: 1,
    operation: Operation::Cutout(CutoutParams {
      margin: Length::ZERO,
      tabs,
      tab_kind: TabKind::Straight,
      pass_depth: Length::from_mm(0.8),
      cut_depth: Length::from_mm(1.6),
      cut_interiors: false,
    }),
  });
  document
}

/// A document with the outline layer but no cutout toolpath.
fn document_without_cutout() -> Document {
  let mut document = document(Vec::new());
  document.toolpaths.clear();
  document
}

fn write_document(directory: &Path, document: &Document) -> PathBuf {
  let path = directory.join("board.cmc");
  save_document(&path, document).expect("document saves");
  path
}

/// Two tools and three hits, in the dialect the corpus files use.
fn write_drill(directory: &Path) -> PathBuf {
  let path = directory.join("board.drl");
  let text = "\
M48
FMAT,2
METRIC
T1C0.800
T2C1.000
%
G90
T1
X10.0Y10.0
X20.0Y10.0
T2
X30.0Y20.0
";
  std::fs::write(&path, text).expect("drill file writes");
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
fn cutout_reports_every_cutout_toolpath_it_cuts() {
  let directory = scratch("report");
  let document = write_document(&directory, &document(Vec::new()));
  let path = document.to_string_lossy().into_owned();

  let output = run(&["cutout", &path, "--diameter", "2.0"]);
  assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
  assert_eq!(
    stdout(&output),
    "cutout Board cutout: 1 contour(s), 14 move(s)\ncutout: 1 toolpath(s), 14 move(s)\n",
    "{}",
    stderr(&output)
  );
}

#[test]
fn cutout_emits_a_program_and_out_writes_it_whole() {
  let directory = scratch("program");
  let document = write_document(&directory, &document(Vec::new()));
  let post = directory.join("post.pp");
  std::fs::write(&post, POST).expect("post writes");
  let path = document.to_string_lossy().into_owned();
  let post_path = post.to_string_lossy().into_owned();

  let printed = run(&["cutout", &path, "--diameter", "2.0", "--post", &post_path]);
  assert_eq!(printed.status.code(), Some(0), "{}", stderr(&printed));
  let program = stdout(&printed);
  assert!(
    program.contains("G00X-1.0000Y-1.0000"),
    "the tool starts one radius outside the board: {program}"
  );
  assert!(
    program.contains("G01X61.0000"),
    "and cuts one radius past its far edge: {program}"
  );

  let destination = directory.join("board.nc");
  let written = run(&[
    "cutout",
    &path,
    "--diameter",
    "2.0",
    "--post",
    &post_path,
    "--out",
    destination.to_string_lossy().as_ref(),
  ]);
  assert_eq!(written.status.code(), Some(0), "{}", stderr(&written));
  assert_eq!(stdout(&written), "", "--out leaves stdout empty");
  assert_eq!(
    std::fs::read_to_string(&destination).expect("program file"),
    program,
    "--out writes the same bytes stdout carried"
  );
}

#[test]
fn a_document_without_a_cutout_is_refused() {
  let directory = scratch("no-cutout");
  let document = write_document(&directory, &document_without_cutout());
  let output = run(&[
    "cutout",
    document.to_string_lossy().as_ref(),
    "--diameter",
    "2.0",
  ]);
  assert_eq!(output.status.code(), Some(2));
  assert!(
    stderr(&output).contains("no cutout toolpath"),
    "{}",
    stderr(&output)
  );
}

#[test]
fn a_missing_diameter_is_refused() {
  let directory = scratch("no-diameter");
  let document = write_document(&directory, &document(Vec::new()));
  let output = run(&["cutout", document.to_string_lossy().as_ref()]);
  assert_eq!(output.status.code(), Some(2));
  assert!(
    stderr(&output).contains("--diameter"),
    "{}",
    stderr(&output)
  );
}

#[test]
fn the_same_document_emits_the_same_program_twice() {
  let directory = scratch("determinism");
  let document = write_document(
    &directory,
    &document(vec![Tab {
      position: 0.25,
      width: Length::from_mm(4.0),
      height: Length::from_mm(0.6),
    }]),
  );
  let post = directory.join("post.pp");
  std::fs::write(&post, POST).expect("post writes");
  let path = document.to_string_lossy().into_owned();
  let post_path = post.to_string_lossy().into_owned();

  let first = run(&["cutout", &path, "--diameter", "2.0", "--post", &post_path]);
  let second = run(&["cutout", &path, "--diameter", "2.0", "--post", &post_path]);
  assert_eq!(first.status.code(), Some(0), "{}", stderr(&first));
  assert_eq!(
    stdout(&first),
    stdout(&second),
    "two runs of the same document must agree byte for byte"
  );
}

#[test]
fn program_drills_then_cuts_the_board_out() {
  let directory = scratch("whole");
  let document = write_document(&directory, &document(Vec::new()));
  let drill = write_drill(&directory);
  let post = directory.join("grbl.pp");
  std::fs::write(&post, cam_post::GRBL_POST).expect("post writes");

  let output = run(&[
    "program",
    document.to_string_lossy().as_ref(),
    "--drill",
    drill.to_string_lossy().as_ref(),
    "--post",
    post.to_string_lossy().as_ref(),
    "--cut-depth",
    "1.6",
    "--diameter",
    "2.0",
    "--tool",
    "3",
  ]);
  assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));

  let program = stdout(&output);
  // The GRBL post names no toolpath, so the order is read where the machine reads it: T1's hole,
  // then T2's hole, then the cutout's first rapid outside the board.
  let first_hole = program.find("X10.000Y10.000").expect("T1 drills");
  let second_hole = program.find("X30.000Y20.000").expect("T2 drills");
  let cutout = program.find("X-1.000Y-1.000").expect("the cutout runs");
  assert!(
    first_hole < second_hole && second_hole < cutout,
    "drills first, cutout last: {program}"
  );
  assert!(
    program.contains("(tool change)"),
    "the two tools are a machine stop, not a silent swap: {program}"
  );

  // The condition `cam_sim::sweep::Crash` reports is a rapid that reaches below the surface, so no
  // rapid may carry a negative Z: that move would ram the stock at rapid rate.
  for line in program.lines() {
    let command = line
      .trim_start_matches('N')
      .trim_start_matches(|c: char| c.is_ascii_digit());
    if !(command.starts_with("G0") && !command.starts_with("G01")) {
      continue;
    }
    let Some(z) = command.split('Z').nth(1) else {
      continue;
    };
    let z: f64 = z
      .chars()
      .take_while(|character| character.is_ascii_digit() || *character == '-')
      .collect::<String>()
      .parse()
      .unwrap_or(0.0);
    assert!(z >= 0.0, "a rapid reaches below the surface: {line}");
  }

  // E2.3/E2.5: the emitted programme is also accepted by cam-sim as a collision-free move stream.
  let doc = cam_store::document::load_document(&document).expect("document loads");
  let drill_text = std::fs::read_to_string(&drill).expect("drill loads");
  let excellon = cam_excellon::parse(&drill_text).expect("drill parses");
  struct Resolver;
  impl cam_ops::excellon_drill::ToolResolver for Resolver {
    fn resolve(
      &self,
      tool: &cam_excellon::Tool,
    ) -> Result<cam_ops::excellon_drill::ExcellonToolSetup, cam_ops::ToolpathError> {
      Ok(cam_ops::excellon_drill::ExcellonToolSetup {
        excellon_tool: tool.number,
        tool: cam_ops::ToolSpec {
          number: tool.number,
          diameter: tool.diameter,
          included_angle: None,
          feed_rate: 100.0,
          plunge_rate: 30.0,
          spindle_speed: 18_000,
        },
        params: cam_ops::DrillParams {
          peck_depth: Length::ZERO,
          retract: cam_ops::RetractMode::Full,
          start_depth: Length::ZERO,
          cut_depth: Length::from_mm(1.6),
          bank: None,
        },
      })
    }
  }
  let plan = cam_ops::excellon_drill::plan_excellon_drill(
    &excellon,
    &Resolver,
    &doc.board,
    cam_ops::excellon_drill::SlotMode::Drill,
  )
  .expect("drill plan");
  let mut stream = cam_move::MoveStream::new(
    Length::from_mm(cam_model::DEFAULT_SAFE_Z_MM),
    Point2::new(0.0, 0.0),
  );
  for group in &plan.groups {
    if group.moves.is_empty() {
      continue;
    }
    stream.segments.push(cam_move::Segment {
      name: format!("Drill T{}", group.excellon_tool),
      tool_number: group.excellon_tool,
      tool: cam_move::ToolShape::cylinder(group.diameter.unwrap_or_else(|| Length::from_mm(1.0))),
      feed_rate: 100.0,
      plunge_rate: 30.0,
      spindle_speed: 18_000,
      moves: group.moves.clone(),
    });
  }
  for toolpath in &doc.toolpaths {
    let Operation::Cutout(params) = &toolpath.operation else {
      continue;
    };
    let contours: Vec<_> = toolpath
      .entity_ids
      .iter()
      .map(|id| {
        doc
          .entity(id)
          .and_then(|entity| entity.kind.as_contour())
          .expect("contour")
          .clone()
      })
      .collect();
    let moves = cam_ops::cutout(&cam_ops::CutoutRequest {
      contours: &contours,
      params,
      tool: &cam_ops::ToolSpec {
        number: 3,
        diameter: Length::from_mm(2.0),
        included_angle: None,
        feed_rate: 100.0,
        plunge_rate: 30.0,
        spindle_speed: 18_000,
      },
      job: &doc.board,
    })
    .expect("cutout moves");
    stream.segments.push(cam_move::Segment {
      name: toolpath.name.clone(),
      tool_number: 3,
      tool: cam_move::ToolShape::cylinder(Length::from_mm(2.0)),
      feed_rate: 100.0,
      plunge_rate: 30.0,
      spindle_speed: 18_000,
      moves,
    });
  }
  let mut field = cam_sim::Heightfield::new(&doc.board, Length::from_mm(1.0)).expect("field fits");
  let removal = cam_sim::sweep::apply(&stream, &mut field);
  assert!(
    removal.crashes.is_empty(),
    "the complete board programme must simulate without collision: {:?}",
    removal.crashes
  );

  let report = stderr(&output);
  assert!(report.contains("3 drill hit(s)"), "{report}");
  assert!(report.contains("2 tool(s)"), "{report}");
  assert!(report.contains("1 tool change(s)"), "{report}");
  assert!(report.contains("1 cutout toolpath(s)"), "{report}");
}

#[test]
fn program_requires_a_drill_file() {
  let directory = scratch("no-drill");
  let document = write_document(&directory, &document(Vec::new()));
  let post = directory.join("post.pp");
  std::fs::write(&post, POST).expect("post writes");

  let output = run(&[
    "program",
    document.to_string_lossy().as_ref(),
    "--post",
    post.to_string_lossy().as_ref(),
    "--cut-depth",
    "1.6",
    "--diameter",
    "2.0",
  ]);
  assert_eq!(output.status.code(), Some(2));
  assert!(stderr(&output).contains("--drill"), "{}", stderr(&output));
}

#[test]
fn cutout_refuses_a_drilling_flag_it_does_not_honour() {
  let directory = scratch("wrong-flag");
  let document = write_document(&directory, &document(Vec::new()));
  let output = run(&[
    "cutout",
    document.to_string_lossy().as_ref(),
    "--diameter",
    "2.0",
    "--cut-depth",
    "1.6",
  ]);
  assert_eq!(output.status.code(), Some(2));
  assert!(
    stderr(&output).contains("unknown cutout option"),
    "{}",
    stderr(&output)
  );
}

#[test]
fn out_without_a_post_is_refused_rather_than_silently_writing_nothing() {
  let directory = scratch("out-without-post");
  let document = write_document(&directory, &document(Vec::new()));
  let destination = directory.join("board.nc");

  let output = run(&[
    "cutout",
    document.to_string_lossy().as_ref(),
    "--diameter",
    "2.0",
    "--out",
    destination.to_string_lossy().as_ref(),
  ]);
  assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
  assert!(
    stderr(&output).contains("needs --post"),
    "{}",
    stderr(&output)
  );
  assert!(
    !destination.exists(),
    "a refused run must not leave a program behind"
  );
}

#[test]
fn program_refuses_a_cutout_tool_the_drill_table_already_claims() {
  let directory = scratch("tool-collision");
  let document = write_document(&directory, &document(Vec::new()));
  let drill = write_drill(&directory);
  let post = directory.join("post.pp");
  std::fs::write(&post, POST).expect("post writes");

  let output = run(&[
    "program",
    document.to_string_lossy().as_ref(),
    "--drill",
    drill.to_string_lossy().as_ref(),
    "--post",
    post.to_string_lossy().as_ref(),
    "--cut-depth",
    "1.6",
    "--diameter",
    "2.0",
  ]);
  assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
  let message = stderr(&output);
  assert!(
    message.contains("--tool 1") && message.contains("drill tool"),
    "the refusal names the colliding tool: {message}"
  );
}
