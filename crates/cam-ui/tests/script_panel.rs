//! The scripting panel (M10 T4b, spec S4: "CLI más panel mínimo").
//!
//! S4 asked for a panel that runs a script and shows what happened, and these tests read it the way the
//! rest of this crate's UI tests do: real `egui` frames, painted text, and the panel's own state. Four
//! things are pinned here, and each is a promise S4 or a T4b invariant makes:
//!
//! * a successful script paints its report;
//! * a failing script paints the engine's located `line:column` diagnostic;
//! * a failure keeps the previous result visible instead of clearing the area;
//! * a captured run holds the **cut list** and the **`save` line**, which are the two things T4a left
//!   printing past the sink. That last one is also the adapter's test: the panel is driven through
//!   [`cam_ui::script_panel::ScriptPanel::run`], which needs no frame at all, and then painted to prove
//!   the same text reaches the screen.

use std::path::{Path, PathBuf};

use cam_geom::{Contour, Point2};
use cam_model::{Document, Entity, EntityKind, Job, Layer};
use cam_store::document::save_document;
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

/// The document every test opens: one layer with two rectangular parts on it.
fn write_document(directory: &Path) -> PathBuf {
  let mut document = Document::new(Job::default_sheet());
  document.layers.push(Layer {
    id: "L1".to_owned(),
    name: "Parts".to_owned(),
    visible: true,
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
  let path = directory.join("parts.c2d");
  save_document(&path, &document).expect("document saved");
  path
}

fn write_script(directory: &Path, name: &str, body: &str) -> PathBuf {
  let path = directory.join(name);
  std::fs::write(&path, body).expect("script written");
  path
}

/// A script that succeeds: it opens the document and asks the model what it holds.
fn valid_script(directory: &Path, document: &Path) -> PathBuf {
  write_script(
    directory,
    "valid.cam",
    &format!("open {}\nvalidate\n", document.display()),
  )
}

/// A script that fails on its third line, with the word `shet` where `sheet` belongs.
fn broken_script(directory: &Path, document: &Path) -> PathBuf {
  write_script(
    directory,
    "broken.cam",
    &format!("# the board\nopen {}\nshet 100 100\n", document.display()),
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

/// Run one frame and report the text it painted.
fn frame(ctx: &egui::Context, panel: &mut ScriptPanel) -> Vec<String> {
  frame_with(ctx, panel, Vec::new())
}

fn frame_with(
  ctx: &egui::Context,
  panel: &mut ScriptPanel,
  events: Vec<egui::Event>,
) -> Vec<String> {
  let mut input = raw_input();
  input.events = events;
  // egui 0.37 stacks its panels on a root `Ui`, which is where a panel with no parent of its own goes.
  let full = ctx.run_pass(input, |ctx| {
    ctx.root_ui(|ui| panel.ui(ui));
  });
  let texts = painted_text(&full);
  // The texture deltas belong to a renderer a headless test does not have; dropping them silently is
  // exactly what epaint forbids, so say so explicitly.
  full.drop_without_applying_deltas();
  texts
}

/// Press the panel's one button: read the point the frame drew it at, then click there.
///
/// A click is two frames, the way a mouse reports it, and the frame that reports the click is the one
/// the release lands in — which is also the frame the panel runs the script in.
fn press_run(ctx: &egui::Context, panel: &mut ScriptPanel) -> Vec<String> {
  let _ = frame(ctx, panel);
  let at = panel.run_button().expect("the frame drew the Run button");
  let at = egui::pos2(at.0, at.1);
  let mut painted: Vec<String> = Vec::new();
  for pressed in [true, false] {
    painted = frame_with(
      ctx,
      panel,
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
  painted
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
  let document = write_document(&directory);
  let mut script = format!("open {}\n", document.display());
  for _ in 0..80 {
    script.push_str("validate\n");
  }
  let path = directory.join("bounded.cam");
  std::fs::write(&path, &script).expect("script written");
  let ctx = egui::Context::default();
  let mut panel = ScriptPanel::with_path(path.to_str().expect("utf8"));

  let _ = press_run(&ctx, &mut panel);

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
// The four promises
// ---------------------------------------------------------------------------------------------

#[test]
fn a_successful_script_paints_its_report() {
  let directory = scratch("paints-report");
  let document = write_document(&directory);
  let script = valid_script(&directory, &document);
  let ctx = egui::Context::default();
  let mut panel = ScriptPanel::with_path(script.display().to_string());

  let painted = press_run(&ctx, &mut panel);

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
  headless.run();
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
  let document = write_document(&directory);
  let script = broken_script(&directory, &document);
  let ctx = egui::Context::default();
  let mut panel = ScriptPanel::with_path(script.display().to_string());

  let painted = press_run(&ctx, &mut panel);

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

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_failed_run_keeps_the_last_result_visible() {
  let directory = scratch("keeps-last");
  let document = write_document(&directory);
  let good = valid_script(&directory, &document);
  let bad = broken_script(&directory, &document);
  let ctx = egui::Context::default();
  let mut panel = ScriptPanel::with_path(good.display().to_string());

  let first = press_run(&ctx, &mut panel);
  let report = "valid: 1 layer(s), 2 entity(ies), 0 toolpath(s)";
  assert!(contains(&first, report), "{}", joined(&first));
  let last_result = panel.output().to_owned();

  // A later run that fails must answer beside the last result, not instead of it.
  panel.set_path(bad.display().to_string());
  let second = press_run(&ctx, &mut panel);
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
fn a_captured_run_holds_the_cut_list_and_the_save_line() {
  // T4a left two bypasses: a scripted `cutlist` without `--out` printed straight to the process's
  // stdout, and `save` wrote its `wrote …` line with `eprintln!`. Neither went through the sink the
  // engine takes, so a capturing caller — this panel — lost both. This is the end-to-end proof that
  // both now arrive, and it is asserted through the panel and through the painted frame, because a
  // caller with no egui reads the first and an operator reads the second.
  let directory = scratch("capture");
  let document = write_document(&directory);
  let saved = directory.join("nested.c2d");
  let script = write_script(
    &directory,
    "run.cam",
    &format!(
      "\
open {document}
sheet 100 100
part 1 E1
nest --gap 0
cutlist
save {saved}
",
      document = document.display(),
      saved = saved.display()
    ),
  );
  let ctx = egui::Context::default();
  let mut panel = ScriptPanel::with_path(script.display().to_string());

  let painted = press_run(&ctx, &mut panel);

  // The cut list: its one header row and the row the document's placement makes.
  for needle in [
    "part,sheet,x,y,rotation,width,height",
    "P1,S1,0.000,0.000,0.000,40.000,30.000",
  ] {
    assert!(
      contains(&painted, needle),
      "the painted area lost the cut list's {needle:?}: {}",
      joined(&painted)
    );
    assert!(
      panel.output().contains(needle),
      "a capturing caller lost the cut list's {needle:?}: {}",
      panel.output()
    );
  }
  // And the `save` line, which names the document it wrote.
  let save_line = format!("wrote 1 layer(s), 2 entity(ies) to {}", saved.display());
  assert!(
    contains(&painted, &save_line),
    "the painted area lost the save line: {}",
    joined(&painted)
  );
  assert!(
    panel.output().contains(&save_line),
    "a capturing caller lost the save line: {}",
    panel.output()
  );
  assert!(panel.error().is_none(), "{:?}", panel.error());

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_successful_script_that_prints_nothing_clears_the_previous_output() {
  // R3-001, the first path nobody covered: a run that **succeeds** and prints nothing has still
  // answered, so the area must go empty. Keying off "the buffer is empty" left the previous run's text
  // on screen with the error cleared, which reads as "this is what just ran".
  let directory = scratch("empty-success");
  let document = write_document(&directory);
  let good = valid_script(&directory, &document);
  let silent = write_script(&directory, "silent.cam", "# nothing to do\n");
  let ctx = egui::Context::default();
  let mut panel = ScriptPanel::with_path(good.display().to_string());

  let first = press_run(&ctx, &mut panel);
  assert!(contains(&first, "valid: "), "{}", joined(&first));
  assert!(
    !panel.output().is_empty(),
    "the first run has something to show"
  );

  panel.set_path(silent.display().to_string());
  let second = press_run(&ctx, &mut panel);
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
  let document = write_document(&directory);
  let good = valid_script(&directory, &document);
  let partial = write_script(
    &directory,
    "partial.cam",
    // `validate` twice, so the partial capture is *different* text from the previous run: with one
    // `validate` the two would be identical and the assertion could not tell "kept" from "overwrote".
    &format!(
      "open {}\nvalidate\nvalidate\nshet 100 100\n",
      document.display()
    ),
  );
  let ctx = egui::Context::default();
  let mut panel = ScriptPanel::with_path(good.display().to_string());

  let _ = press_run(&ctx, &mut panel);
  let previous = panel.output().to_owned();
  assert!(!previous.is_empty(), "the first run has something to show");

  panel.set_path(partial.display().to_string());
  let second = press_run(&ctx, &mut panel);
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
    contains(&second, "line 4:1: unknown script command"),
    "the failure must be painted: {}",
    joined(&second)
  );
}
