use super::support::*;
use cam_app::AppState;
use cam_ui::help::Command as HelpCommand;
use cam_ui::Shell;

#[test]
fn the_script_chip_opens_the_panel_and_escape_closes_it_without_moving_the_canvas() {
  // M10 T4b/S1: the panel is the sixth surface and it is a window over the canvas, so opening it must add
  // a surface and leave the drawing where it was (S2) — a mount that re-laid the frame out would move
  // `output.canvas` and, with it, every gesture that reads the rectangle.
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let ctx = egui::Context::default();

  let (closed, closed_texts) = frame_with(&ctx, &mut shell, &mut app);
  assert!(!shell.show_script, "the script panel must start closed");
  assert!(
    !joined(&closed_texts).contains(cam_ui::script_panel::PATH_LABEL),
    "a closed panel paints nothing: {}",
    joined(&closed_texts)
  );

  let _ = click_palette_chip(&ctx, &mut shell, &mut app, HelpCommand::Script.label());
  assert!(
    shell.show_script,
    "the SHOW AND SELECT Script chip must open the panel"
  );

  // An egui area positions itself on a sizing pass, so the frame it opens in is not the one that
  // settles: the modal is read on the quiet frame after, the way `open_material_dialog` reads its own.
  let _ = frame_with(&ctx, &mut shell, &mut app);
  let (open, texts) = frame_with(&ctx, &mut shell, &mut app);
  assert!(
    joined(&texts).contains(cam_ui::script_panel::PATH_LABEL),
    "the modal must paint the panel's own path label: {}",
    joined(&texts)
  );
  assert_eq!(
    open.canvas, closed.canvas,
    "the panel is a window over the canvas, so the drawing must not move"
  );

  let _ = press(&ctx, &mut shell, &mut app, egui::Key::Escape);
  assert!(!shell.show_script, "Escape must close the script panel");
  let (_, after) = frame_with(&ctx, &mut shell, &mut app);
  assert!(
    !joined(&after).contains(cam_ui::script_panel::PATH_LABEL),
    "a closed panel paints nothing after Escape: {}",
    joined(&after)
  );
}

#[test]
fn a_run_from_the_script_modal_edits_the_shells_document() {
  // M11 T2, the whole of the residual this feature closes: `Run` used to edit a file on disk and leave
  // the shell's document alone. It now hands the shell the document the script produced, through the
  // same edit path every other command uses — so the edit is in the document, in one undo step, and the
  // modified state says so.
  let directory = scratch("modal-edits");
  let script = directory.join("run.cam");
  std::fs::write(&script, "sheet 300 200\n").expect("script written");
  let mut app = AppState::new(document_with_rectangle());
  assert!(!app.is_dirty(), "a fresh document has nothing to save");
  let mut shell = Shell::new();
  let ctx = egui::Context::default();

  let _ = frame_with(&ctx, &mut shell, &mut app);
  let _ = click_palette_chip(&ctx, &mut shell, &mut app, HelpCommand::Script.label());
  // The sizing pass that settles the modal, then the frame that draws `Run` and reports its point.
  let _ = frame_with(&ctx, &mut shell, &mut app);
  shell.script_panel().set_path(script.display().to_string());
  let _ = frame_with(&ctx, &mut shell, &mut app);
  let at = shell
    .script_panel()
    .run_button()
    .expect("the modal drew the Run button");

  let (_, texts) = click_at(&ctx, &mut shell, &mut app, egui::pos2(at.0, at.1));

  assert!(
    app.is_dirty(),
    "a run that edited the document must leave the shell's modified state set"
  );
  let nesting = app
    .document()
    .nesting
    .as_ref()
    .expect("the sheet the script authored is in the shell's own document");
  assert_eq!(nesting.sheets.len(), 1, "exactly the one sheet authored");
  assert_eq!(nesting.sheets[0].width, cam_units::Length::from_mm(300.0));
  assert_eq!(nesting.sheets[0].height, cam_units::Length::from_mm(200.0));
  assert!(
    !joined(&texts).contains("line 1:"),
    "an accepted run paints no diagnostic: {}",
    joined(&texts)
  );

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_refused_run_leaves_the_shells_document_and_its_modified_state_untouched() {
  // S4 of the feature: a run is one transaction, so the lines a script accepted before the refusal are
  // not kept. The shell's document, its history and its modified state stay exactly as they were, and the
  // diagnostic is painted on the panel that tried.
  let directory = scratch("modal-refuses");
  let script = directory.join("broken.cam");
  std::fs::write(&script, "sheet 300 200\nshet 100 100\n").expect("script written");
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let ctx = egui::Context::default();

  let _ = frame_with(&ctx, &mut shell, &mut app);
  let _ = click_palette_chip(&ctx, &mut shell, &mut app, HelpCommand::Script.label());
  let _ = frame_with(&ctx, &mut shell, &mut app);
  shell.script_panel().set_path(script.display().to_string());
  let _ = frame_with(&ctx, &mut shell, &mut app);
  let at = shell
    .script_panel()
    .run_button()
    .expect("the modal drew the Run button");

  let (_, texts) = click_at(&ctx, &mut shell, &mut app, egui::pos2(at.0, at.1));

  assert!(
    !app.is_dirty(),
    "a refused run is not an edit, so nothing became unsaved"
  );
  assert!(
    app.document().nesting.is_none(),
    "the sheet line 1 authored is not kept: the run was refused on line 2"
  );
  assert!(
    joined(&texts).contains("line 2:1: unknown script command \"shet\""),
    "the refusal is painted, located where it happened: {}",
    joined(&texts)
  );

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_run_that_changed_nothing_does_not_mark_the_document_modified() {
  // R4-001 of the review of M11: the commit path replaced the document unconditionally, so a script that
  // succeeded and changed nothing still recorded an undo step and left the document unsaved. A run that
  // produces the document it was given is not an edit.
  let directory = scratch("modal-noop");
  let script = directory.join("noop.cam");
  std::fs::write(&script, "# nothing to do\n").expect("script written");
  let mut app = AppState::new(document_with_rectangle());
  let before = app.document().clone();
  let mut shell = Shell::new();
  let ctx = egui::Context::default();

  let _ = frame_with(&ctx, &mut shell, &mut app);
  let _ = click_palette_chip(&ctx, &mut shell, &mut app, HelpCommand::Script.label());
  let _ = frame_with(&ctx, &mut shell, &mut app);
  shell.script_panel().set_path(script.display().to_string());
  let _ = frame_with(&ctx, &mut shell, &mut app);
  let at = shell
    .script_panel()
    .run_button()
    .expect("the modal drew the Run button");

  let (_, texts) = click_at(&ctx, &mut shell, &mut app, egui::pos2(at.0, at.1));

  assert!(
    !joined(&texts).contains("line 1:"),
    "the script succeeded, so the frame paints no diagnostic: {}",
    joined(&texts)
  );
  assert!(
    !app.is_dirty(),
    "a run that changed nothing must not leave the document unsaved"
  );
  assert_eq!(
    app.document(),
    &before,
    "and the document is the one the run was given"
  );
  assert!(
    !app.undo(),
    "nor does it record an undo step for an edit that never happened"
  );

  std::fs::remove_dir_all(&directory).ok();
}
