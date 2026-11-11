//! End-to-end tests for the command language (M10 T1).
//!
//! These run the real binary on a real script file, so they assert the observable contract: the exit
//! code, the exact stdout and stderr, and the file left on disk.

use std::path::{Path, PathBuf};
use std::process::Command;

use cam_geom::{Contour, Point2};
use cam_model::{
  Document, Entity, EntityKind, IsolationKind, IsolationParams, Job, Layer, LayerRole,
  MillingDirection, Operation, Toolpath,
};
use cam_store::document::save_document;
use cam_units::{Length, UnitSystem};

fn binary() -> &'static str {
  env!("CARGO_BIN_EXE_cam-cli")
}

/// A scratch directory under `/var/tmp`: the brief forbids `/tmp`, which is a full tmpfs here.
fn scratch(name: &str) -> PathBuf {
  let directory =
    PathBuf::from("/var/tmp").join(format!("campilot-script-{}-{name}", std::process::id()));
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

/// A document with two rectangular entities and one profile toolpath, so `nest`, `emit` and
/// `simulate` all have something to work on.
///
/// `parts` are the nesting parts the document already authors, as `(id, entity ids, instances)`: the
/// CLI's `nest` needs them, and a test of the script's own `part` command starts from none.
fn write_document(directory: &Path, parts: &[(&str, &[&str], usize)]) -> PathBuf {
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
    entities: vec![
      Entity {
        id: "E1".to_owned(),
        kind: EntityKind::Contour {
          contour: rectangle_contour(40.0, 30.0),
        },
      },
      Entity {
        id: "E2".to_owned(),
        kind: EntityKind::Contour {
          contour: rectangle_contour(30.0, 20.0),
        },
      },
    ],
  });
  document.toolpaths.push(Toolpath {
    id: "TP1".to_owned(),
    name: "Outside profile".to_owned(),
    entity_ids: vec!["E1".to_owned()],
    island_ids: Vec::new(),
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

  if !parts.is_empty() {
    document.nesting = Some(cam_model::Nesting {
      sheets: Vec::new(),
      parts: parts
        .iter()
        .map(|(id, entity_ids, instances)| cam_model::NestingPart {
          id: (*id).to_owned(),
          entity_ids: entity_ids.iter().map(|id| (*id).to_owned()).collect(),
          instances: *instances,
        })
        .collect(),
      placements: Vec::new(),
      cost: None,
    });
  }

  let path = directory.join("parts.c2d");
  save_document(&path, &document).expect("document saved");
  path
}

/// A tool library with a 6 mm cutter as T1, so `emit` and `simulate` can name a tool.
fn write_script(directory: &Path, name: &str, text: &str) -> PathBuf {
  let path = directory.join(name);
  std::fs::write(&path, text).expect("script written");
  path
}

struct Outcome {
  code: i32,
  stdout: String,
  stderr: String,
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

/// Run the binary with `directory` as its working directory, so a test can prove a command reaches no
/// ambient file.
fn run_in(directory: &Path, arguments: &[&str]) -> Outcome {
  outcome(
    Command::new(binary())
      .current_dir(directory)
      .env("CAM_CLI_DOCUMENT", "/var/tmp/decoy.c2d")
      .env("CAM_CLI_LIBRARY", "/var/tmp/decoy.sqlite")
      .env("CAM_CLI_POST", "/var/tmp/decoy.pp")
      .args(arguments)
      .output()
      .expect("the binary runs"),
  )
}

/// The nesting placements a document on disk holds, as `(part, sheet, x, y)`.
fn placements(document: &Path) -> Vec<(String, String, f64, f64)> {
  let loaded = cam_store::document::load_document(document).expect("document loads");
  loaded
    .nesting
    .expect("nesting section")
    .placements
    .iter()
    .map(|placement| {
      (
        placement.part_id.clone(),
        placement.sheet_id.clone(),
        placement.position.x,
        placement.position.y,
      )
    })
    .collect()
}

fn script_outcome(script: &Path) -> Outcome {
  run(&["script", script.to_str().expect("utf8")])
}

/// The nest report a run of `sheet`/`part`/`nest` prints for two parts on one 100 x 100 board, then the
/// counts `validate` prints for the document that was nested.
const NESTED_REPORT: &str = "\
nest: 1 sheet(s) of 100.000 x 100.000 mm, 2 part(s), 3 instance(s)
options: gap 0.000 mm, rotation off
sheet S1: used
part: P1 rectangle, 2 instance(s)
placement: P1 on S1 at (0.000, 0.000), rotation none, 40.000 x 30.000 mm
placement: P1 on S1 at (40.000, 0.000), rotation none, 40.000 x 30.000 mm
part: P2 rectangle, 1 instance(s)
placement: P2 on S1 at (0.000, 30.000), rotation none, 30.000 x 20.000 mm
algorithm: rectangle, maxrects
sheets used: 1
utilisation: 0.300
waste: 0.700
unplaced: none
valid: 1 layer(s), 2 entity(ies), 1 toolpath(s)
";

#[test]
fn a_script_authors_sheets_and_parts_nests_them_and_saves_exactly() {
  let directory = scratch("authoring");
  let document = write_document(&directory, &[]);
  let saved = directory.join("nested.c2d");
  let script = write_script(
    &directory,
    "author.cam",
    &format!(
      "\
open {document}
sheet 100 100
part 2 E1
part 1 E2
nest --gap 0
validate
save {saved}
",
      document = document.display(),
      saved = saved.display()
    ),
  );

  let outcome = script_outcome(&script);
  assert_eq!(outcome.code, 0, "{}", outcome.stderr);
  assert_eq!(outcome.stdout, NESTED_REPORT);
  assert_eq!(
    outcome.stderr,
    format!("wrote 1 layer(s), 2 entity(ies) to {}\n", saved.display())
  );

  // The placements are the ones the nesters produce, asserted exactly: two copies of the first part
  // then one of the second, on the one board the script authored.
  assert_eq!(
    placements(&saved),
    vec![
      ("P1".to_owned(), "S1".to_owned(), 0.0, 0.0),
      ("P1".to_owned(), "S1".to_owned(), 40.0, 0.0),
      ("P2".to_owned(), "S1".to_owned(), 0.0, 30.0),
    ]
  );

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_script_that_nests_writes_the_document_the_nest_subcommand_writes() {
  let directory = scratch("nest-parity");
  let cli_directory = directory.join("cli");
  let script_directory = directory.join("script");
  std::fs::create_dir_all(&cli_directory).expect("cli directory");
  std::fs::create_dir_all(&script_directory).expect("script directory");
  // The command line is handed a document that already authors its parts; the script authors the same
  // parts itself, so the two runs differ in how the sections were authored and in nothing else.
  let authored = write_document(&cli_directory, &[("P1", &["E1"], 2), ("P2", &["E2"], 1)]);
  let bare = write_document(&script_directory, &[]);
  let from_cli = directory.join("cli.c2d");
  let from_script = directory.join("script.c2d");

  let cli = run(&[
    "nest",
    authored.to_str().expect("utf8"),
    "--sheets",
    "100x100",
    "--gap",
    "0",
    "--out",
    from_cli.to_str().expect("utf8"),
  ]);
  assert_eq!(cli.code, 0, "{}", cli.stderr);

  let script = write_script(
    &directory,
    "nest.cam",
    &format!(
      "\
open {bare}
sheet 100 100
part 2 E1
part 1 E2
nest --gap 0
save {from_script}
",
      bare = bare.display(),
      from_script = from_script.display()
    ),
  );
  let outcome = script_outcome(&script);
  assert_eq!(outcome.code, 0, "{}", outcome.stderr);

  assert_eq!(
    std::fs::read(&from_script).expect("script document"),
    std::fs::read(&from_cli).expect("cli document"),
    "a script's nest must write the bytes the nest subcommand writes"
  );

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_bad_argument_count_reports_the_line_and_the_column_past_the_last_token() {
  let directory = scratch("bad-count");
  let document = write_document(&directory, &[]);
  let script = write_script(
    &directory,
    "bad-count.cam",
    &format!(
      "open {document}\nsheet 100\n",
      document = document.display()
    ),
  );

  let outcome = script_outcome(&script);
  assert_eq!(outcome.code, 2, "{}", outcome.stderr);
  assert_eq!(
    outcome.stderr,
    "error: line 2:10: sheet expects <width> <height> in millimetres, got 1 argument(s)\n"
  );

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_bad_argument_count_after_a_quoted_token_reports_the_column_past_its_closing_quote() {
  let directory = scratch("bad-count-quoted");
  let document = write_document(&directory, &[]);
  let script = write_script(
    &directory,
    "bad-count-quoted.cam",
    &format!(
      "open {document}\nsheet \"100 200\"\n",
      document = document.display()
    ),
  );

  let outcome = script_outcome(&script);
  assert_eq!(outcome.code, 2, "{}", outcome.stderr);
  // `sheet "100 200"` is fifteen characters and the closing quote is the fifteenth, so the argument
  // the line does not have would have gone in column sixteen. Adding the token's text length to the
  // opening quote's column stops at fourteen: inside the token, two short of one past it.
  assert_eq!(
    outcome.stderr,
    "error: line 2:16: sheet expects <width> <height> in millimetres, got 1 argument(s)\n"
  );

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_bad_value_reports_the_line_and_column_of_the_offending_token() {
  let directory = scratch("bad-value");
  let document = write_document(&directory, &[]);
  let script = write_script(
    &directory,
    "bad-value.cam",
    &format!(
      "open {document}\nsheet 100 wide\n",
      document = document.display()
    ),
  );

  let outcome = script_outcome(&script);
  assert_eq!(outcome.code, 2, "{}", outcome.stderr);
  assert_eq!(
    outcome.stderr,
    "error: line 2:11: sheet expects a height in millimetres, got \"wide\"\n"
  );

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_model_refusal_names_the_script_line_and_the_model_message() {
  let directory = scratch("model-refusal");
  let document = write_document(&directory, &[]);
  let saved = directory.join("never.c2d");
  let script = write_script(
    &directory,
    "model.cam",
    &format!(
      "\
open {document}
sheet 0 100
save {saved}
",
      document = document.display(),
      saved = saved.display()
    ),
  );

  let outcome = script_outcome(&script);
  assert_eq!(outcome.code, 2, "{}", outcome.stderr);
  assert_eq!(
    outcome.stderr,
    "error: line 2:1: nesting sheet \"S1\" width must be positive, got 0 mm\n"
  );
  assert!(
    !saved.exists(),
    "a refused script writes nothing, and this one never reached its save"
  );

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_mutation_that_would_not_validate_is_refused_and_leaves_the_file_alone() {
  let directory = scratch("refused-mutation");
  let document = write_document(&directory, &[]);
  let before = std::fs::read(&document).expect("document bytes");
  let script = write_script(
    &directory,
    "refused.cam",
    &format!(
      "\
open {document}
part 1 E9
save {document}
",
      document = document.display()
    ),
  );

  let outcome = script_outcome(&script);
  assert_eq!(outcome.code, 2, "{}", outcome.stderr);
  assert_eq!(
    outcome.stderr,
    "error: line 2:1: nesting part \"P1\" references unknown entity \"E9\"\n"
  );
  assert_eq!(
    std::fs::read(&document).expect("document bytes"),
    before,
    "the document on disk is unchanged"
  );

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_comment_and_a_blank_line_change_nothing() {
  let directory = scratch("comments");
  let document = write_document(&directory, &[]);
  let plain = write_script(
    &directory,
    "plain.cam",
    &format!(
      "\
open {document}
sheet 100 100
part 2 E1
part 1 E2
nest --gap 0
save {plain}
",
      document = document.display(),
      plain = directory.join("plain.c2d").display()
    ),
  );
  let annotated = write_script(
    &directory,
    "annotated.cam",
    &format!(
      "\
# The board, then the parts of the cabinet.
open {document}   # the document to mutate

   # authoring
sheet 100 100
part 2 E1

part 1 E2
nest --gap 0
save {annotated}
# and that is the whole script
",
      document = document.display(),
      annotated = directory.join("annotated.c2d").display()
    ),
  );

  let from_plain = script_outcome(&plain);
  assert_eq!(from_plain.code, 0, "{}", from_plain.stderr);
  let from_annotated = script_outcome(&annotated);
  assert_eq!(from_annotated.code, 0, "{}", from_annotated.stderr);

  assert_eq!(
    from_annotated.stdout, from_plain.stdout,
    "a comment and a blank line change no output"
  );
  assert_eq!(
    std::fs::read(directory.join("annotated.c2d")).expect("annotated document"),
    std::fs::read(directory.join("plain.c2d")).expect("plain document"),
    "a comment and a blank line change no bytes"
  );

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_path_comes_from_the_script_or_the_command_line_and_nowhere_else() {
  let directory = scratch("paths");
  let elsewhere = directory.join("elsewhere");
  let document = write_document(&elsewhere, &[]);
  let saved = directory.join("saved").join("nested.c2d");

  // A working directory holding a decoy of the document's own file name, and three decoy paths in the
  // environment: none of them is named by the script, so none of them may be read or written.
  let work = directory.join("work");
  std::fs::create_dir_all(&work).expect("work directory");
  let decoy = work.join("parts.c2d");
  std::fs::write(&decoy, b"not a document").expect("decoy written");
  let decoy_before = std::fs::read(&decoy).expect("decoy bytes");

  let script = write_script(
    &work,
    "paths.cam",
    &format!(
      "\
open {document}
part 1 E2
save {saved}
",
      document = document.display(),
      saved = saved.display()
    ),
  );

  let outcome = run_in(&work, &["script", script.to_str().expect("utf8")]);
  assert_eq!(outcome.code, 0, "{}", outcome.stderr);
  assert_eq!(
    std::fs::read(&decoy).expect("decoy bytes"),
    decoy_before,
    "the decoy beside the script is untouched"
  );

  let mut left: Vec<String> = std::fs::read_dir(&work)
    .expect("work directory")
    .map(|entry| {
      entry
        .expect("entry")
        .file_name()
        .to_string_lossy()
        .into_owned()
    })
    .collect();
  left.sort();
  assert_eq!(
    left,
    vec!["parts.c2d".to_owned(), "paths.cam".to_owned()],
    "the script wrote nothing into its working directory"
  );

  // What it saved came from the document the script named, not from the decoy.
  let loaded = cam_store::document::load_document(&saved).expect("saved document loads");
  assert_eq!(loaded.entities().count(), 2);
  assert_eq!(
    loaded
      .nesting
      .expect("nesting section")
      .parts
      .iter()
      .map(|part| part.entity_ids.clone())
      .collect::<Vec<_>>(),
    vec![vec!["E2".to_owned()]]
  );

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_script_needs_a_path_and_an_open_document() {
  let outcome = run(&["script"]);
  assert_eq!(outcome.code, 2, "{}", outcome.stderr);
  assert_eq!(outcome.stderr, "error: script requires a path\n");

  let directory = scratch("no-document");
  let script = write_script(&directory, "bare.cam", "validate\n");
  let outcome = script_outcome(&script);
  assert_eq!(outcome.code, 2, "{}", outcome.stderr);
  assert_eq!(
    outcome.stderr,
    "error: line 1:1: no document is open: a script reads one with `open <document>`\n"
  );

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_script_refuses_a_positional_document_for_nest() {
  // R1-001: `nest` used to accept a positional document into its own parser and then drop it, passing
  // `None` to the refusal — so `open a` plus `nest b` nested **a**, and a following `save b` could
  // overwrite **b** with a's document. The refusal is the fix; the second half proves that the file the
  // script named is never written, which is the part a silent success would have hidden.
  let directory = scratch("nest-positional");
  let document = write_document(&directory, &[("P1", &["E1"], 1)]);
  let other = directory.join("other.c2d");
  std::fs::copy(&document, &other).expect("copy");
  let before = std::fs::read(&other).expect("read");

  let script = write_script(
    &directory,
    "nest-path.cam",
    &format!(
      "open {document}\nnest --sheets 400x300 {other}\nsave {other}\n",
      document = document.display(),
      other = other.display()
    ),
  );
  let outcome = script_outcome(&script);
  assert_eq!(outcome.code, 2, "{}", outcome.stderr);
  assert_eq!(
    outcome.stderr,
    "error: line 2:23: nest takes no document path: a script mutates the document `open` read\n"
  );
  assert_eq!(
    std::fs::read(&other).expect("read"),
    before,
    "a script that cannot nest writes nothing to the document it named"
  );
}
