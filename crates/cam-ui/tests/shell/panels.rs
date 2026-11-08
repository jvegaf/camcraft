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
    joined(&texts).contains("L1 — Part"),
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
fn opening_a_category_puts_its_task_editor_over_the_panel() {
  // The idle panel says what is selected; opening `TOOLPATHS` replaces that with the authoring body, which is
  // what "the task editor replaces the panel's content" means (S6).
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let ctx = egui::Context::default();
  let (first, texts) = frame_with(&ctx, &mut shell, &mut app);
  let all = joined(&texts);
  assert!(
    all.contains("Properties"),
    "the idle panel shows the selection's numbers: {all}"
  );
  let at = first
    .drawn
    .category_hits
    .iter()
    .find(|(word, _)| word == "TOOLPATHS")
    .map(|(_, point)| egui::pos2(point.0, point.1))
    .expect("TOOLPATHS is on the ribbon");

  let output = click_point(&ctx, &mut shell, &mut app, at);
  assert_eq!(shell.task(), Some(cam_ui::Category::Toolpaths));
  assert_eq!(
    output.drawn.task_editor, "New toolpath · Profile 1",
    "the toolpath task is the strategy editor, and S10 makes its header say which mode it is in and whose \
     strategy it is describing"
  );
  let (_, texts) = frame_with(&ctx, &mut shell, &mut app);
  let mut all_texts = texts;
  // The strategy editor S10 puts at the top of the task is taller than the 55 % body an 800 px window gives
  // it, so the authoring body below it starts under the fold. The claim this test makes is S6's — the task
  // editor, not the selection's numbers, is the panel's content — so the body is read the way an operator
  // reaches it: the wheel, then the words that were there all along.
  let inspector = egui::pos2(
    (output.canvas.x + output.canvas.width) as f32 + 100.0,
    (output.canvas.y + 260.0) as f32,
  );
  all_texts.extend(texts_scrolled_to_the_end(
    &ctx, &mut shell, &mut app, inspector,
  ));
  let all = joined(&all_texts);
  assert!(
    all.contains("Add text") && all.contains("Authoring"),
    "the toolpath task carries the authoring body: {all}"
  );
  assert!(
    !all.contains("Properties"),
    "the task editor replaces the panel's content, it does not stack on it: {all}"
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

#[test]
fn the_layers_block_folds_on_its_own_and_leaves_the_task_where_it_was() {
  // S6: `Camadas` is its own collapsible block below the task editor. It folds without closing the task, and
  // the task does not come back when the block does.
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.open_task(cam_ui::Category::Toolpaths);
  let ctx = egui::Context::default();

  let (first, _) = frame_with(&ctx, &mut shell, &mut app);
  assert!(first.drawn.layers_open, "the block starts expanded");
  assert_eq!(
    first.drawn.layer_rows,
    vec![("L1".to_owned(), 1, true)],
    "an expanded block draws its rows"
  );
  let at = first
    .drawn
    .layers_heading
    .map(|(x, y)| egui::pos2(x, y))
    .expect("the block reports its heading");

  let output = click_point(&ctx, &mut shell, &mut app, at);
  assert!(!output.drawn.layers_open, "the heading folds the block");
  assert!(
    output.drawn.layer_rows.is_empty(),
    "a folded block draws no row: {:?}",
    output.drawn.layer_rows
  );
  assert_eq!(
    shell.task(),
    Some(cam_ui::Category::Toolpaths),
    "folding the layers block must not close the task"
  );
  assert_eq!(
    output.drawn.task_editor, "New toolpath · Profile 1",
    "the task editor stays where it was"
  );
}

// ---------------------------------------------------------------------------------------------------
// The rulers (camui-prancheta S5)
// ---------------------------------------------------------------------------------------------------
