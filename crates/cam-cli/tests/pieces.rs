//! End-to-end tests for the CSV part list **in** and the cut list **out** (M10 T2b, S3: "Import y
//! export").
//!
//! These run the real binary, so they assert the observable contract: the exit code, the exact stdout
//! and stderr, the file left on disk and the bytes the script form writes.

use std::path::{Path, PathBuf};
use std::process::Command;

use cam_geom::{Contour, Point2};
use cam_model::{
  Document, Entity, EntityKind, Job, Layer, Nesting, NestingPart, NestingPlacement, NestingSheet,
};
use cam_store::document::save_document;
use cam_units::{Angle, Length, UnitSystem};

fn binary() -> &'static str {
  env!("CARGO_BIN_EXE_cam-cli")
}

/// A scratch directory under `/var/tmp`: the brief forbids `/tmp`, which is a full tmpfs here.
fn scratch(name: &str) -> PathBuf {
  let directory =
    PathBuf::from("/var/tmp").join(format!("campilot-pieces-{}-{name}", std::process::id()));
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
    width: Length::from_mm(400.0),
    height: Length::from_mm(300.0),
    thickness: Length::from_mm(18.0),
    units: UnitSystem::Mm,
    origin: cam_model::OriginCorner::BottomLeft,
    safe_z: Length::from_mm(5.0),
    material: None,
  }
}

/// A document holding one rectangle entity per `(id, width, height)` and, when given, that nesting
/// section. Nothing else: every test starts from a document whose state it can state.
fn write_document(
  directory: &Path,
  name: &str,
  entities: &[(&str, f64, f64)],
  nesting: Option<Nesting>,
) -> PathBuf {
  let mut document = Document::new(job());
  if !entities.is_empty() {
    document.layers.push(Layer {
      id: "L1".to_owned(),
      name: "Parts".to_owned(),
      visible: true,
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
  }
  document.nesting = nesting;

  let path = directory.join(name);
  save_document(&path, &document).expect("document saved");
  path
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

fn placement(part_id: &str, sheet_id: &str, x: f64, y: f64, degrees: f64) -> NestingPlacement {
  NestingPlacement {
    part_id: part_id.to_owned(),
    sheet_id: sheet_id.to_owned(),
    position: Point2::new(x, y),
    rotation: Angle::from_degrees(degrees),
  }
}

fn nesting(
  sheets: Vec<NestingSheet>,
  parts: Vec<NestingPart>,
  placements: Vec<NestingPlacement>,
) -> Nesting {
  Nesting {
    sheets,
    parts,
    placements,
    cost: None,
  }
}

fn write_csv(directory: &Path, name: &str, text: &str) -> PathBuf {
  let path = directory.join(name);
  std::fs::write(&path, text).expect("csv written");
  path
}

fn write_script(directory: &Path, name: &str, text: &str) -> PathBuf {
  let path = directory.join(name);
  std::fs::write(&path, text).expect("script written");
  path
}

fn load(path: &Path) -> Document {
  cam_store::document::load_document(path).expect("document loads")
}

struct Outcome {
  code: i32,
  stdout: String,
  stderr: String,
}

impl Outcome {
  /// The bytes on stdout, for the invariant-1 comparisons: a script and its subcommand must print the
  /// same file, not only the same text.
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

/// The report a three-row part list prints, which is `cam_import::csv`'s own report.
const PART_LIST_REPORT: &str = "\
format: csv
header: line 1
parts: 3
comments: 0
blank lines: 0
";

/// The three-row part list every shape test starts from.
const THREE_ROWS: &str = "\
name,width,height,quantity
side,120,80,2
shelf,90,60,1
back,120,80,4
";

#[test]
fn a_part_list_authors_a_rectangle_and_a_part_per_row_then_nests_them() {
  let directory = scratch("authoring");
  let document = write_document(&directory, "bare.c2d", &[], None);
  let csv = write_csv(&directory, "parts.csv", THREE_ROWS);
  let out = directory.join("authored.c2d");

  let imported = run(&[
    "parts",
    document.to_str().expect("utf8"),
    "--csv",
    csv.to_str().expect("utf8"),
    "--layer",
    "Cabinet",
    "--out",
    out.to_str().expect("utf8"),
  ]);
  assert_eq!(imported.code, 0, "{}", imported.stderr);
  assert_eq!(imported.stdout, PART_LIST_REPORT);
  assert_eq!(
    imported.stderr,
    format!("wrote 1 layer(s), 3 entity(ies) to {}\n", out.display())
  );

  // One new layer, every rectangle in it, in file order, each a closed rectangle at the origin with
  // the row's own exact size.
  let authored = load(&out);
  assert_eq!(authored.layers.len(), 1);
  assert_eq!(authored.layers[0].id, "L1");
  assert_eq!(authored.layers[0].name, "Cabinet");
  assert!(authored.layers[0].visible);
  let entities: Vec<(&str, [f64; 4])> = authored.layers[0]
    .entities
    .iter()
    .map(|entity| {
      let contour = entity
        .kind
        .as_contour()
        .expect("an authored part is a contour");
      assert!(contour.is_closed(), "{} is closed", entity.id);
      assert_eq!(contour.len(), 4, "{} is four vertices", entity.id);
      let bounds = contour.bounds();
      (
        entity.id.as_str(),
        [bounds.min.x, bounds.min.y, bounds.max.x, bounds.max.y],
      )
    })
    .collect();
  assert_eq!(
    entities,
    vec![
      ("side", [0.0, 0.0, 120.0, 80.0]),
      ("shelf", [0.0, 0.0, 90.0, 60.0]),
      ("back", [0.0, 0.0, 120.0, 80.0]),
    ]
  );

  // One nesting part per row, named by `fresh_part_id`'s scan, selectiing the row's own entity and
  // carrying the row's quantity as its instance count.
  let nesting = authored.nesting.as_ref().expect("nesting section");
  assert!(nesting.sheets.is_empty());
  assert!(nesting.placements.is_empty());
  let parts: Vec<(&str, Vec<&str>, usize)> = nesting
    .parts
    .iter()
    .map(|part| {
      (
        part.id.as_str(),
        part.entity_ids.iter().map(String::as_str).collect(),
        part.instances,
      )
    })
    .collect();
  assert_eq!(
    parts,
    vec![
      ("P1", vec!["side"], 2),
      ("P2", vec!["shelf"], 1),
      ("P3", vec!["back"], 4),
    ]
  );

  // The authored parts nest exactly like any other document's: the nester places every instance on
  // the one board, at the origins it chose.
  let nested = run(&[
    "nest",
    out.to_str().expect("utf8"),
    "--sheets",
    "400x300",
    "--gap",
    "0",
  ]);
  assert_eq!(nested.code, 0, "{}", nested.stderr);

  let loaded = load(&out);
  let placements: Vec<(String, String, f64, f64)> = loaded
    .nesting
    .expect("nested section")
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
    .collect();
  assert_eq!(placements.len(), 7, "every authored instance was placed");
  // The nester's own deck order and origins, asserted exactly: its equal-area parts are placed
  // before the smaller one, each copy on a free corner of the board it was handed. Every footprint is
  // inside the 400 x 300 board and none of them overlap; the counts are the CSV's quantities.
  assert_eq!(
    placements
      .iter()
      .map(|(part, _, x, y)| (part.as_str(), *x, *y))
      .collect::<Vec<_>>(),
    vec![
      ("P1", 0.0, 0.0),
      ("P1", 0.0, 80.0),
      ("P3", 0.0, 160.0),
      ("P3", 120.0, 0.0),
      ("P3", 240.0, 0.0),
      ("P3", 120.0, 80.0),
      ("P2", 0.0, 240.0),
    ]
  );

  std::fs::remove_dir_all(&directory).ok();
}

/// The cut list a hand-authored nesting must produce, byte for byte: one header row, one row per
/// stored placement, millimetres and degrees to three decimals, one final newline.
const CUT_LIST: &str = "\
part,sheet,x,y,rotation,width,height
P1,S1,0.000,0.000,0.000,120.000,80.000
P1,S1,120.000,0.000,0.000,120.000,80.000
P2,S1,0.000,80.000,90.000,90.000,60.000
";

/// A document whose nesting holds three placements, one of them turned a quarter turn, so the cut
/// list's formatting is pinned without depending on a nester's packing.
fn nested_document(directory: &Path) -> PathBuf {
  write_document(
    directory,
    "nested.c2d",
    &[("side", 120.0, 80.0), ("shelf", 90.0, 60.0)],
    Some(nesting(
      vec![sheet("S1", 400.0, 300.0)],
      vec![part("P1", &["side"], 2), part("P2", &["shelf"], 1)],
      vec![
        placement("P1", "S1", 0.0, 0.0, 0.0),
        placement("P1", "S1", 120.0, 0.0, 0.0),
        placement("P2", "S1", 0.0, 80.0, 90.0),
      ],
    )),
  )
}

#[test]
fn the_cut_list_is_the_bytes_a_shop_reads() {
  let directory = scratch("cut-list");
  let document = nested_document(&directory);

  let first = run(&["cutlist", document.to_str().expect("utf8")]);
  assert_eq!(first.code, 0, "{}", first.stderr);
  assert_eq!(first.stdout, CUT_LIST);
  assert!(first.stderr.is_empty(), "{}", first.stderr);

  // `--out` writes the same bytes elsewhere and prints nothing, like `emit`'s program.
  let out = directory.join("cut-list.csv");
  let written = run(&[
    "cutlist",
    document.to_str().expect("utf8"),
    "--out",
    out.to_str().expect("utf8"),
  ]);
  assert_eq!(written.code, 0, "{}", written.stderr);
  assert!(written.stdout.is_empty(), "{}", written.stdout);
  assert_eq!(
    std::fs::read_to_string(&out).expect("cut list written"),
    CUT_LIST
  );

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_cut_list_without_placements_is_refused_by_the_state_it_is_in() {
  let directory = scratch("no-placements");

  // A nesting section that authors parts but holds no placement: a request that has not been nested.
  let unplaced = write_document(
    &directory,
    "unplaced.c2d",
    &[("side", 120.0, 80.0)],
    Some(nesting(
      vec![sheet("S1", 400.0, 300.0)],
      vec![part("P1", &["side"], 1)],
      Vec::new(),
    )),
  );
  let out = directory.join("never.csv");
  let refused = run(&[
    "cutlist",
    unplaced.to_str().expect("utf8"),
    "--out",
    out.to_str().expect("utf8"),
  ]);
  assert_eq!(refused.code, 2, "{}", refused.stderr);
  assert!(refused.stdout.is_empty(), "{}", refused.stdout);
  assert_eq!(
    refused.stderr,
    "error: cutlist: the document's nesting section has no placements (1 part(s), 0 placement(s)), \
     so nothing has been nested yet\n"
  );
  assert!(!out.exists(), "a refused cut list writes no file");

  // A document with no nesting section at all is a different state, said by name.
  let bare = write_document(&directory, "bare.c2d", &[("side", 120.0, 80.0)], None);
  let refused = run(&["cutlist", bare.to_str().expect("utf8")]);
  assert_eq!(refused.code, 2, "{}", refused.stderr);
  assert_eq!(
    refused.stderr,
    "error: cutlist: the document has no nesting section, so there are no placements to list: nest \
     a document that authors sheets and parts first\n"
  );

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_duplicate_part_name_in_the_list_is_refused_with_its_source_line() {
  let directory = scratch("duplicate-row");
  let document = write_document(&directory, "bare.c2d", &[], None);
  let before = std::fs::read(&document).expect("document bytes");
  let csv = write_csv(
    &directory,
    "duplicate.csv",
    "\
name,width,height,quantity
side,120,80,2
side,90,60,1
",
  );

  let refused = run(&[
    "parts",
    document.to_str().expect("utf8"),
    "--csv",
    csv.to_str().expect("utf8"),
  ]);
  assert_eq!(refused.code, 2, "{}", refused.stderr);
  assert!(refused.stdout.is_empty(), "{}", refused.stdout);
  assert_eq!(
    refused.stderr,
    format!(
      "error: {}: csv at line 3, field 1 (\"name\"): duplicate part name \"side\": already named on \
       line 2\n",
      csv.display()
    )
  );
  assert_eq!(
    std::fs::read(&document).expect("document bytes"),
    before,
    "a refused import writes nothing"
  );

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_part_name_the_document_already_holds_is_refused_with_its_source_line() {
  let directory = scratch("collision");
  let document = write_document(&directory, "bare.c2d", &[], None);
  let first = write_csv(
    &directory,
    "first.csv",
    "\
name,width,height,quantity
side,120,80,2
",
  );
  let second = write_csv(
    &directory,
    "second.csv",
    "\
name,width,height,quantity
shelf,90,60,1
side,120,80,1
",
  );

  let imported = run(&[
    "parts",
    document.to_str().expect("utf8"),
    "--csv",
    first.to_str().expect("utf8"),
  ]);
  assert_eq!(imported.code, 0, "{}", imported.stderr);
  let before = std::fs::read(&document).expect("document bytes");

  let refused = run(&[
    "parts",
    document.to_str().expect("utf8"),
    "--csv",
    second.to_str().expect("utf8"),
  ]);
  assert_eq!(refused.code, 2, "{}", refused.stderr);
  assert!(refused.stdout.is_empty(), "{}", refused.stdout);
  assert_eq!(
    refused.stderr,
    format!(
      "error: {}: csv at line 3, field 1 (\"name\"): the document already holds a part named \
       \"side\"\n",
      second.display()
    )
  );
  assert_eq!(
    std::fs::read(&document).expect("document bytes"),
    before,
    "a refused import authors nothing, not even the rows before the collision"
  );

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn an_import_that_would_not_validate_is_refused_with_the_model_message_and_the_csv_line() {
  let directory = scratch("invalid-document");
  // A document that has been nested: from its first placement on, the placements and every part's
  // instance count are the two halves of one answer, so a new part with no placement is a document
  // that would not load.
  let document = write_document(
    &directory,
    "nested.c2d",
    &[("side", 120.0, 80.0)],
    Some(nesting(
      vec![sheet("S1", 400.0, 300.0)],
      vec![part("P1", &["side"], 1)],
      vec![placement("P1", "S1", 0.0, 0.0, 0.0)],
    )),
  );
  let before = std::fs::read(&document).expect("document bytes");
  let csv = write_csv(
    &directory,
    "extra.csv",
    "\
name,width,height,quantity
extra,50,40,3
",
  );
  let out = directory.join("never.c2d");

  let refused = run(&[
    "parts",
    document.to_str().expect("utf8"),
    "--csv",
    csv.to_str().expect("utf8"),
    "--out",
    out.to_str().expect("utf8"),
  ]);
  assert_eq!(refused.code, 2, "{}", refused.stderr);
  assert!(refused.stdout.is_empty(), "{}", refused.stdout);
  assert_eq!(
    refused.stderr,
    format!(
      "error: {}: csv at line 2: nesting part \"P2\" asks for 3 instances but 0 placements name it\n",
      csv.display()
    )
  );
  assert!(!out.exists(), "a refused import writes no document");
  assert_eq!(
    std::fs::read(&document).expect("document bytes"),
    before,
    "the document the import was handed is unchanged"
  );

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn the_script_reaches_both_commands_and_both_write_the_subcommands_bytes() {
  let directory = scratch("parity");

  // Parts in: the command line is handed a bare document and writes it where `--out` says; the script
  // opens the same kind of document, runs the same command and writes it with `save`.
  let csv = write_csv(&directory, "parts.csv", THREE_ROWS);
  let bare = write_document(&directory, "bare.c2d", &[], None);
  let from_cli = directory.join("cli.c2d");
  let from_script = directory.join("script.c2d");

  let cli = run(&[
    "parts",
    bare.to_str().expect("utf8"),
    "--csv",
    csv.to_str().expect("utf8"),
    "--layer",
    "Cabinet",
    "--out",
    from_cli.to_str().expect("utf8"),
  ]);
  assert_eq!(cli.code, 0, "{}", cli.stderr);

  let script = write_script(
    &directory,
    "parts.cam",
    &format!(
      "\
open {bare}
parts --csv {csv} --layer Cabinet
save {from_script}
",
      bare = bare.display(),
      csv = csv.display(),
      from_script = from_script.display()
    ),
  );
  let scripted = run(&["script", script.to_str().expect("utf8")]);
  assert_eq!(scripted.code, 0, "{}", scripted.stderr);
  assert_eq!(
    scripted.stdout_bytes(),
    cli.stdout_bytes(),
    "a script's parts must print the bytes the parts subcommand prints"
  );
  let cli_document = std::fs::read(&from_cli).expect("cli document");
  let script_document = std::fs::read(&from_script).expect("script document");
  assert_eq!(
    script_document.len(),
    cli_document.len(),
    "a script's parts must write the bytes the parts subcommand writes"
  );
  assert_eq!(
    script_document, cli_document,
    "a script's parts must write the bytes the parts subcommand writes"
  );

  // Cut list out: the command line reads the document itself; the script reads it with `open`.
  let nested = nested_document(&directory);
  let from_cli = run(&["cutlist", nested.to_str().expect("utf8")]);
  assert_eq!(from_cli.code, 0, "{}", from_cli.stderr);
  let script = write_script(
    &directory,
    "cutlist.cam",
    &format!("open {}\ncutlist\n", nested.display()),
  );
  let scripted = run(&["script", script.to_str().expect("utf8")]);
  assert_eq!(scripted.code, 0, "{}", scripted.stderr);
  assert_eq!(
    scripted.stdout_bytes(),
    from_cli.stdout_bytes(),
    "a script's cutlist must print the bytes the cutlist subcommand prints"
  );

  // A script writes its document with `save`, so the scripted `parts` refuses `--out` by name.
  let refused = write_script(
    &directory,
    "parts-out.cam",
    &format!(
      "open {}\nparts --csv parts.csv --out out.c2d\n",
      bare.display()
    ),
  );
  let outcome = run(&["script", refused.to_str().expect("utf8")]);
  assert_eq!(outcome.code, 2, "{}", outcome.stderr);
  assert_eq!(
    outcome.stderr,
    "error: line 2:23: parts takes no --out: a script writes its document with `save <document>`\n"
  );

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn two_runs_of_the_same_import_and_of_the_same_cut_list_are_byte_identical() {
  let directory = scratch("determinism");
  let csv = write_csv(&directory, "parts.csv", THREE_ROWS);
  let bare = write_document(&directory, "bare.c2d", &[], None);

  let mut first = None;
  for name in ["first.c2d", "second.c2d"] {
    let out = directory.join(name);
    let run = run(&[
      "parts",
      bare.to_str().expect("utf8"),
      "--csv",
      csv.to_str().expect("utf8"),
      "--out",
      out.to_str().expect("utf8"),
    ]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let bytes = std::fs::read(&out).expect("document bytes");
    match &first {
      None => first = Some((run.stdout, bytes)),
      Some((stdout, bytes_first)) => {
        assert_eq!(
          run.stdout.as_bytes(),
          stdout.as_bytes(),
          "two runs print one report"
        );
        assert_eq!(&bytes, bytes_first, "two runs write one document");
      }
    }
  }

  let nested = nested_document(&directory);
  let one = run(&["cutlist", nested.to_str().expect("utf8")]);
  let two = run(&["cutlist", nested.to_str().expect("utf8")]);
  assert_eq!(one.code, 0, "{}", one.stderr);
  assert_eq!(one.stdout_bytes(), two.stdout_bytes());

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn the_commands_need_their_arguments() {
  let directory = scratch("arguments");

  let outcome = run(&["parts"]);
  assert_eq!(outcome.code, 2, "{}", outcome.stderr);
  assert_eq!(outcome.stderr, "error: parts requires a document path\n");

  let document = write_document(&directory, "bare.c2d", &[], None);
  let outcome = run(&["parts", document.to_str().expect("utf8")]);
  assert_eq!(outcome.code, 2, "{}", outcome.stderr);
  assert_eq!(outcome.stderr, "error: parts requires --csv <file>\n");

  let outcome = run(&["cutlist"]);
  assert_eq!(outcome.code, 2, "{}", outcome.stderr);
  assert_eq!(outcome.stderr, "error: cutlist requires a document path\n");

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn the_help_lists_both_commands_their_flags_and_the_cut_lists_units() {
  let outcome = run(&["--help"]);
  assert_eq!(outcome.code, 0);
  for needle in [
    "parts <document>",
    "--csv <file>",
    "cutlist <document>",
    "--out <path>",
  ] {
    assert!(
      outcome.stdout.contains(needle),
      "help is missing {needle:?}"
    );
  }
}
