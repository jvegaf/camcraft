use super::support::*;
use cam_app::AppState;
use cam_model::{Document, Job};
use cam_ui::{handle_action, Shell, ShellOutput, Tab, UiAction};

#[test]
fn the_layer_dock_reports_every_layer_and_paints_its_name() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let (output, texts) = frame(&mut shell, &mut app);

  assert_eq!(
    output.drawn.layer_rows,
    vec![("L1".to_owned(), 1, true)],
    "the dock must report id, entity count and visibility"
  );
  assert!(
    joined(&texts).contains("L1 — Outline/Both"),
    "the layer row must be painted: {}",
    joined(&texts)
  );
  // `LAYERS` is its own block since camui-prancheta S6, and every section heading in this frame is uppercase.
  assert!(
    joined(&texts).contains("LAYERS"),
    "the layers block must head itself: {}",
    joined(&texts)
  );
}

#[test]
fn the_selection_tab_shows_the_job_and_the_selection() {
  let mut app = AppState::new(document_with_rectangle());
  app.select(vec!["E1".to_owned()]);
  let mut shell = Shell::new();
  shell.set_tab(Tab::Selection);
  let (output, texts) = frame(&mut shell, &mut app);

  let properties = output.drawn.property_lines.join(" | ");
  assert!(properties.contains("Job:"), "{properties}");
  assert!(properties.contains("E1: contour"), "{properties}");
  assert!(joined(&texts).contains("Inspector"));
  assert!(joined(&texts).contains("Delete selection"));
}

#[test]
fn the_job_tab_shows_labelled_measurements_in_two_columns() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.set_tab(Tab::Job);
  let (_, texts) = frame(&mut shell, &mut app);
  let all = joined(&texts);
  for label in ["MATERIAL", "Width X", "Height Y", "Thickness Z", "DOCUMENT"] {
    assert!(all.contains(label), "missing {label:?} in {all}");
  }
}

#[test]
fn the_job_tab_names_the_step_the_document_is_missing() {
  // An empty sheet: the drawing is what is missing, and the sentence names it.
  let mut app = AppState::new(Document::new(Job::default_sheet()));
  let mut shell = Shell::new();
  shell.set_tab(Tab::Job);
  let (_, texts) = frame(&mut shell, &mut app);
  let all = joined(&texts);
  assert!(
    all.contains("Nothing drawn yet"),
    "an empty sheet must say so: {all}"
  );

  // One vector and no toolpath: the missing step is the toolpath now, and the drawing is no longer
  // what is missing.
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.set_tab(Tab::Job);
  let (_, texts) = frame(&mut shell, &mut app);
  let all = joined(&texts);
  assert!(all.contains("The Toolpaths task authors one"), "{all}");
  assert!(!all.contains("Nothing drawn yet"), "{all}");
}

#[test]
fn selecting_through_an_action_updates_the_properties_panel() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.set_tab(Tab::Selection);
  let _ = frame(&mut shell, &mut app);
  handle_action(&mut app, &UiAction::SetSelection(vec!["E1".to_owned()])).expect("select");
  let (output, _) = frame(&mut shell, &mut app);
  assert!(output
    .drawn
    .property_lines
    .join(" ")
    .contains("E1: contour"));
}

/// The stock `Job::default_sheet()` gives a fresh document, in millimetres: what `Cancel` must restore.
fn material_size_of(_app: &AppState) -> (f64, f64, f64) {
  (1219.0, 2438.0, 18.0)
}

/// Click a reported point, one press and one release, as a person does.
fn click_point(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
  at: egui::Pos2,
) -> ShellOutput {
  click_at(ctx, shell, app, at).0
}

#[test]
fn the_right_panel_is_a_ribbon_of_three_categories_in_the_references_order() {
  // S6: `TOOLPATHS`, `RELIEF AND 3D PART`, `MACHINE AND OUTPUT`, glyph only, one row per category. The words
  // are asserted from the panel's own report rather than from painted text: they are the task editor's future
  // header, and a glyph-only ribbon paints none of them.
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let (output, _) = frame(&mut shell, &mut app);
  assert_eq!(
    output.drawn.categories,
    vec![
      "TOOLPATHS".to_owned(),
      "RELIEF AND 3D PART".to_owned(),
      "MACHINE AND OUTPUT".to_owned()
    ],
    "the ribbon is the reference's three categories, in its order"
  );
  // The old tab row is gone: a tab names a shelf and a category opens a task. `Tools` and `Job` were the tab
  // labels, and neither is painted anywhere now.
  assert!(
    !output.drawn.categories.iter().any(|word| word == "Tools"),
    "the panel must not offer the old tab labels: {:?}",
    output.drawn.categories
  );
  assert!(
    output.drawn.task_editor.is_empty(),
    "no task is open on a fresh shell: {:?}",
    output.drawn.task_editor
  );
}

#[test]
fn the_task_header_keeps_the_typed_numbers_on_done_and_discards_them_on_cancel() {
  // The only real difference between `✓` and `✕` in this panel: every number applies as it is typed except the
  // material's drafts, so `Done` has nothing to undo and `Cancel` is the one gesture that throws the draft away.
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.open_task(cam_ui::Category::MachineOutput);
  shell.material_size = (100.0, 50.0, 3.0);
  let ctx = egui::Context::default();

  let (first, _) = frame_with(&ctx, &mut shell, &mut app);
  let done = first
    .drawn
    .task_editor_hits
    .iter()
    .find(|(word, _)| word == "Done")
    .map(|(_, point)| egui::pos2(point.0, point.1))
    .expect("the header carries Done");
  let output = click_point(&ctx, &mut shell, &mut app, done);
  assert_eq!(
    output.drawn.task_editor, "",
    "Done closes the task editor, and the panel falls back to the selection"
  );
  assert_eq!(
    shell.material_size,
    (100.0, 50.0, 3.0),
    "Done keeps the numbers that were typed"
  );

  shell.open_task(cam_ui::Category::MachineOutput);
  shell.material_size = (100.0, 50.0, 3.0);
  let (first, _) = frame_with(&ctx, &mut shell, &mut app);
  let cancel = first
    .drawn
    .task_editor_hits
    .iter()
    .find(|(word, _)| word == "Cancel")
    .map(|(_, point)| egui::pos2(point.0, point.1))
    .expect("the header carries Cancel");
  let output = click_point(&ctx, &mut shell, &mut app, cancel);
  assert_eq!(
    output.drawn.task_editor, "",
    "Cancel closes the task editor"
  );
  assert_eq!(
    shell.material_size,
    material_size_of(&app),
    "Cancel puts the material's draft back to the document's own stock"
  );
}
