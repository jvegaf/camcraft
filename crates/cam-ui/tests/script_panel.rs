//! The scripting panel: one script, one `Run`, one output area (M10 T4b), now over the shell's document
//! (M11 T2).
//!
//! Every test here drives the real widget and reads what the frame painted, because the panel's contract
//! is what an operator sees. The document is the one the panel's caller owns: a run mutates a copy and
//! hands it back only when every line was accepted, which is why most tests below read both the painted
//! text and the returned document.

use std::path::{Path, PathBuf};

use cam_geom::{Contour, Point2};
use cam_model::{Document, Entity, EntityKind, Job, Layer, LayerRole};
use cam_ui::script_panel::{ScriptPanel, BODY_HEIGHT, PATH_LABEL, RUN, TITLE};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

/// A scratch directory under `/var/tmp`: the brief forbids `/tmp`, which is a full tmpfs here.
///
/// `name` is one path segment and nothing else: it is joined onto a directory this helper deletes
/// first, so a name carrying a separator or a `..` would aim that deletion at somebody else's tree.
fn scratch(name: &str) -> PathBuf {
  assert!(
    !name.contains('/') && !name.contains('\\') && !name.contains(".."),
    "a scratch name is one path segment, got {name:?}"
  );
  let directory =
    PathBuf::from("/var/tmp").join(format!("campilot-panel-{}-{name}", std::process::id()));
  let _ = std::fs::remove_dir_all(&directory);
  std::fs::create_dir_all(&directory).expect("scratch directory");
  directory
}

fn rectangle(width: f64, height: f64) -> Contour {
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

/// The document every test runs against: one layer with two rectangular parts on it.
///
/// It is a value, not a file: since M11 the panel edits the document its caller holds, so a script that
/// names a path on disk has nothing to do with the document any more.
fn document() -> Document {
  let mut document = Document::new(Job::default_sheet());
  document.layers.push(Layer {
    id: "L1".to_owned(),
    role: LayerRole::Outline, /*was Geometry*/
    side: cam_model::Side::Both,
    color: "#000000".to_owned(),
    visible: true,
    source: cam_model::Provenance::default(),
    entities: vec![
      Entity {
        id: "E1".to_owned(),
        kind: EntityKind::Contour {
          contour: rectangle(40.0, 30.0),
        },
      },
      Entity {
        id: "E2".to_owned(),
        kind: EntityKind::Contour {
          contour: rectangle(30.0, 20.0),
        },
      },
    ],
  });
  document
}

fn write_script(directory: &Path, name: &str, body: &str) -> PathBuf {
  let path = directory.join(name);
  std::fs::write(&path, body).expect("script written");
  path
}

/// A script that succeeds: it asks the model what the document holds.
fn valid_script(directory: &Path) -> PathBuf {
  write_script(directory, "valid.cam", "validate\n")
}

/// A script that fails on its third line, with the word `shet` where `sheet` belongs.
fn broken_script(directory: &Path) -> PathBuf {
  write_script(
    directory,
    "broken.cam",
    "# the board\nvalidate\nshet 100 100\n",
  )
}

// ---------------------------------------------------------------------------------------------
// The frame harness
// ---------------------------------------------------------------------------------------------

fn raw_input() -> egui::RawInput {
  egui::RawInput {
    screen_rect: Some(egui::Rect::from_min_size(
      egui::Pos2::ZERO,
      egui::vec2(900.0, 900.0),
    )),
    ..Default::default()
  }
}

/// Every piece of text the frame actually drew.
fn painted_text(output: &egui::FullOutput) -> Vec<String> {
  output
    .shapes
    .iter()
    .filter_map(|clipped| match &clipped.shape {
      egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
      _ => None,
    })
    .collect()
}

/// Run one frame and report the text it painted, and the document it handed back.
fn frame(
  ctx: &egui::Context,
  panel: &mut ScriptPanel,
  document: &Document,
) -> (Vec<String>, Option<Document>) {
  frame_with(ctx, panel, document, Vec::new())
}

fn frame_with(
  ctx: &egui::Context,
  panel: &mut ScriptPanel,
  document: &Document,
  events: Vec<egui::Event>,
) -> (Vec<String>, Option<Document>) {
  let mut input = raw_input();
  input.events = events;
  let mut committed = None;
  // egui 0.37 stacks its panels on a root `Ui`, which is where a panel with no parent of its own goes.
  let full = ctx.run_pass(input, |ctx| {
    ctx.root_ui(|ui| committed = panel.ui(ui, document, None));
  });
  let texts = painted_text(&full);
  // The texture deltas belong to a renderer a headless test does not have; dropping them silently is
  // exactly what epaint forbids, so say so explicitly.
  full.drop_without_applying_deltas();
  (texts, committed)
}

/// Press the panel's one button: read the point the frame drew it at, then click there.
///
/// A click is two frames, the way a mouse reports it, and the frame that reports the click is the one
/// the release lands in — which is also the frame the panel runs the script in.
fn press_run(
  ctx: &egui::Context,
  panel: &mut ScriptPanel,
  document: &Document,
) -> (Vec<String>, Option<Document>) {
  let _ = frame(ctx, panel, document);
  let at = panel.run_button().expect("the frame drew the Run button");
  let at = egui::pos2(at.0, at.1);
  let mut painted: Vec<String> = Vec::new();
  let mut committed = None;
  for pressed in [true, false] {
    (painted, committed) = frame_with(
      ctx,
      panel,
      document,
      vec![
        egui::Event::PointerMoved(at),
        egui::Event::PointerButton {
          pos: at,
          button: egui::PointerButton::Primary,
          pressed,
          modifiers: egui::Modifiers::default(),
        },
      ],
    );
  }
  (painted, committed)
}

fn contains(painted: &[String], needle: &str) -> bool {
  painted.iter().any(|text| text.contains(needle))
}

fn joined(painted: &[String]) -> String {
  painted.join("\n")
}

#[test]
fn a_long_run_stays_inside_a_bounded_body() {
  // A window can hold this panel only if the output area has a ceiling: without one a script that prints
  // for hundreds of lines grows the body past any screen, and a surface that cannot be sized is not one
  // (mount task T1).
  let directory = scratch("bounded-body");
  let document = document();
  let mut script = String::new();
  for _ in 0..80 {
    script.push_str("validate\n");
  }
  let path = write_script(&directory, "bounded.cam", &script);
  let ctx = egui::Context::default();
  let mut panel = ScriptPanel::with_path(path.to_str().expect("utf8"));

  let _ = press_run(&ctx, &mut panel, &document);

  assert!(
    !panel.output().is_empty(),
    "the run printed something to measure"
  );
  let (_, height) = panel.body_size().expect("the frame reported the body");
  assert!(
    height <= BODY_HEIGHT + 1.0,
    "the body took {height} points, past the {BODY_HEIGHT} a window can hold"
  );
  std::fs::remove_dir_all(&directory).ok();
}

// ---------------------------------------------------------------------------------------------
// The promises the panel had before M11, unchanged
// ---------------------------------------------------------------------------------------------

#[test]
fn a_successful_script_paints_its_report() {
  let directory = scratch("paints-report");
  let document = document();
  let script = valid_script(&directory);
  let ctx = egui::Context::default();
  let mut panel = ScriptPanel::with_path(script.display().to_string());

  let (painted, _) = press_run(&ctx, &mut panel, &document);

  // The panel's own words first: a frame that painted none of them would make the rest meaningless.
  for word in [TITLE, PATH_LABEL, RUN] {
    assert!(
      contains(&painted, word),
      "the panel must paint {word:?}: {}",
      joined(&painted)
    );
  }
  let report = "valid: 1 layer(s), 2 entity(ies), 0 toolpath(s)";
  assert!(
    contains(&painted, report),
    "a successful run must paint the report the script wrote: {}",
    joined(&painted)
  );
  assert!(panel.error().is_none(), "{:?}", panel.error());
  assert!(
    panel.output().contains(report),
    "the same text must be readable without a frame: {}",
    panel.output()
  );

  // The adapter boundary itself: a caller that has no egui runs the same script through the same
  // engine and reads the same report, with no frame and no widget anywhere.
  let mut headless = ScriptPanel::with_path(script.display().to_string());
  let mut headless_document = document.clone();
  assert!(
    headless.run(&mut headless_document, None),
    "the headless run accepts the script"
  );
  assert!(headless.error().is_none(), "{:?}", headless.error());
  assert!(
    headless.output().contains(report),
    "a caller without egui must get the report: {}",
    headless.output()
  );

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_failing_script_paints_the_located_error_with_its_line_and_column() {
  let directory = scratch("paints-error");
  let document = document();
  let script = broken_script(&directory);
  let ctx = egui::Context::default();
  let mut panel = ScriptPanel::with_path(script.display().to_string());

  let (painted, committed) = press_run(&ctx, &mut panel, &document);

  // The word `shet` is the third line's first token, so the diagnostic is `line 3:1` — the whole point
  // of invariant 2: a mistyped command says where it is, and the panel shows it rather than a generic
  // "the script failed".
  let located = "line 3:1: unknown script command \"shet\"";
  assert!(
    contains(&painted, located),
    "a failing run must paint the located diagnostic: {}",
    joined(&painted)
  );
  let error = panel.error().expect("the failure is kept");
  assert!(
    error.starts_with(located),
    "the error the panel keeps must be the engine's own diagnostic: {error}"
  );
  assert!(
    committed.is_none(),
    "a run that failed hands back no document: the shell must keep the one it has"
  );

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_failed_run_keeps_the_last_result_visible() {
  let directory = scratch("keeps-last");
  let document = document();
  let good = valid_script(&directory);
  let bad = broken_script(&directory);
  let ctx = egui::Context::default();
  let mut panel = ScriptPanel::with_path(good.display().to_string());

  let (first, _) = press_run(&ctx, &mut panel, &document);
  let report = "valid: 1 layer(s), 2 entity(ies), 0 toolpath(s)";
  assert!(contains(&first, report), "{}", joined(&first));
  let last_result = panel.output().to_owned();

  // A later run that fails must answer beside the last result, not instead of it.
  panel.set_path(bad.display().to_string());
  let (second, _) = press_run(&ctx, &mut panel, &document);
  assert!(
    contains(&second, "line 3:1: unknown script command"),
    "the failure must be painted: {}",
    joined(&second)
  );
  assert!(
    contains(&second, report),
    "the previous result must stay on screen: {}",
    joined(&second)
  );
  assert_eq!(
    panel.output(),
    last_result,
    "the area keeps the text the last successful run produced"
  );

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_successful_script_that_prints_nothing_clears_the_previous_output() {
  // R3-001, the first path nobody covered: a run that **succeeds** and prints nothing has still
  // answered, so the area must go empty. Keying off "the buffer is empty" left the previous run's text
  // on screen with the error cleared, which reads as "this is what just ran".
  let directory = scratch("empty-success");
  let document = document();
  let good = valid_script(&directory);
  let silent = write_script(&directory, "silent.cam", "# nothing to do\n");
  let ctx = egui::Context::default();
  let mut panel = ScriptPanel::with_path(good.display().to_string());

  let (first, _) = press_run(&ctx, &mut panel, &document);
  assert!(contains(&first, "valid: "), "{}", joined(&first));
  assert!(
    !panel.output().is_empty(),
    "the first run has something to show"
  );

  panel.set_path(silent.display().to_string());
  let (second, _) = press_run(&ctx, &mut panel, &document);
  assert_eq!(
    panel.output(),
    "",
    "a run that succeeded and printed nothing replaces the area"
  );
  assert!(
    !contains(&second, "valid: "),
    "the previous report must not survive a successful silent run: {}",
    joined(&second)
  );
}

#[test]
fn a_failed_run_that_printed_partially_keeps_the_previous_output() {
  // R3-001, the second path: a run that prints something and then fails used to overwrite the previous
  // result with its partial capture, which contradicted the documented "a failure keeps the previous
  // output" rule. The failure answers beside the last complete result.
  let directory = scratch("partial-failure");
  let document = document();
  let good = valid_script(&directory);
  let partial = write_script(
    &directory,
    "partial.cam",
    // `validate` twice, so the partial capture is *different* text from the previous run: with one
    // `validate` the two would be identical and the assertion could not tell "kept" from "overwrote".
    "validate\nvalidate\nshet 100 100\n",
  );
  let ctx = egui::Context::default();
  let mut panel = ScriptPanel::with_path(good.display().to_string());

  let _ = press_run(&ctx, &mut panel, &document);
  let previous = panel.output().to_owned();
  assert!(!previous.is_empty(), "the first run has something to show");

  panel.set_path(partial.display().to_string());
  let (second, _) = press_run(&ctx, &mut panel, &document);
  assert_eq!(
    panel.output(),
    previous,
    "a failed run keeps the previous result, not its own partial capture"
  );
  assert_ne!(
    panel.output(),
    format!("{previous}{previous}"),
    "the partial capture is what a failure must not leave behind"
  );
  assert!(
    contains(&second, "line 3:1: unknown script command"),
    "the failure must be painted: {}",
    joined(&second)
  );

  std::fs::remove_dir_all(&directory).ok();
}

// ---------------------------------------------------------------------------------------------
// What M11 changed: the run edits the caller's document, and it writes no file
// ---------------------------------------------------------------------------------------------

#[test]
fn a_run_hands_back_the_document_it_mutated() {
  // The whole point of the task: a script run from the shell edits the shell's document. The panel
  // hands that document back — one value, so the caller keeps it through its own edit path.
  let directory = scratch("hands-back");
  let document = document();
  assert!(document.nesting.is_none(), "the fixture starts un-nested");
  let script = write_script(
    &directory,
    "author.cam",
    "sheet 100 100\npart 1 E1\nvalidate\n",
  );
  let ctx = egui::Context::default();
  let mut panel = ScriptPanel::with_path(script.display().to_string());

  let (painted, committed) = press_run(&ctx, &mut panel, &document);

  let report = "valid: 1 layer(s), 2 entity(ies), 0 toolpath(s)";
  assert!(
    contains(&painted, report),
    "the painted area lost the validation report: {}",
    joined(&painted)
  );
  assert!(panel.error().is_none(), "{:?}", panel.error());

  let edited = committed.expect("an accepted run hands its document back");
  let nesting = edited
    .nesting
    .as_ref()
    .expect("the sheet and the part the script authored are in the document it hands back");
  assert_eq!(nesting.sheets.len(), 1, "one sheet, the one authored");
  assert_eq!(nesting.parts.len(), 1, "one part, the one authored");
  assert!(
    document.nesting.is_none(),
    "the document the caller passed in is untouched: the panel mutates a copy"
  );

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn save_is_refused_because_the_shell_writes_the_document() {
  // S3 of this feature: a script run from the shell writes no file. The rule is refused on its own
  // line, located, and nothing reaches the disk — the shell's Save is the one writer.
  let directory = scratch("no-save");
  let document = document();
  let target = directory.join("nested.cmc");
  let script = write_script(
    &directory,
    "save.cam",
    &format!("sheet 100 100\nsave {}\n", target.display()),
  );
  let ctx = egui::Context::default();
  let mut panel = ScriptPanel::with_path(script.display().to_string());

  let (painted, committed) = press_run(&ctx, &mut panel, &document);

  let located = "line 2:1: a script run from the shell writes no file";
  assert!(
    contains(&painted, located),
    "the refusal must be painted, located at the line that tried: {}",
    joined(&painted)
  );
  assert!(
    !target.exists(),
    "nothing reached the disk: the shell's Save is the only writer"
  );
  assert!(
    committed.is_none(),
    "and the sheet the first line authored is not kept: a run is one transaction"
  );

  std::fs::remove_dir_all(&directory).ok();
}
