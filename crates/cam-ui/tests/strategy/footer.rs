use super::support::*;
use cam_app::AppState;
use cam_model::{Document, DrillParams, Operation, ProfileParams};
use cam_ui::{Shell, UiAction};
use cam_units::Length;

// ---------------------------------------------------------------------------------------------
// The footer: Generate toolpath and Cancel
// ---------------------------------------------------------------------------------------------

#[test]
fn generating_a_new_strategy_emits_the_action_and_cancel_leaves_the_document_as_it_was() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(rectangle_document());
  app.select(vec!["E1".to_owned()]);
  let mut shell = Shell::new();
  shell.set_tools(vec![flat_mill()]);
  let _ = editor_frame(&ctx, &mut shell, &mut app);
  shell.strategy.tool_number = Some(2);
  shell.strategy.name = "Profile 1".to_owned();
  shell.strategy.operation = Operation::Profile(ProfileParams {
    pass_depth: Length::from_mm(1.5),
    ..ProfileParams::default()
  });

  let (drawn, _) = editor_frame(&ctx, &mut shell, &mut app);
  assert!(
    drawn.drawn.strategy_refusal.is_empty(),
    "a complete draft must not be refused: {:?}",
    drawn.drawn.strategy_refusal
  );
  let pressed = click_word(&ctx, &mut shell, &mut app, &drawn, "Generate toolpath");

  // The button emits an authoring action, and emitting it is not applying it: the document is the host's.
  assert_eq!(pressed.actions.len(), 1, "{:?}", pressed.actions);
  let action = pressed.actions[0].clone();
  match &action {
    UiAction::AddToolpath {
      name,
      entity_ids,
      tool_number,
      operation,
      ..
    } => {
      assert_eq!(
        name, "Profile 1",
        "the name the editor holds is the name written"
      );
      assert_eq!(entity_ids, &vec!["E1".to_owned()]);
      assert_eq!(*tool_number, 2, "the chosen tool is the tool written");
      assert_eq!(*operation, shell.strategy.operation);
    }
    other => panic!("`Generate toolpath` must add a toolpath, got {other:?}"),
  }
  assert!(
    app.document().toolpaths.is_empty(),
    "the frame may not write the document behind the host's back"
  );

  // Applied the way a host applies it, the strategy exists, and the editor now edits it.
  shell.apply_action(&mut app, &action);
  assert_eq!(app.document().toolpaths.len(), 1);
  assert_eq!(app.document().toolpaths[0].name, "Profile 1");
  let (after, _) = editor_frame(&ctx, &mut shell, &mut app);
  assert_eq!(
    after.drawn.task_editor, "Edit · Profile 1",
    "the editor that created a strategy edits it on the next frame"
  );

  // Cancel: the document, the tool and the name stay exactly as they were, and the task leaves with the draft.
  let before = app.document().clone();
  let (drawn, _) = editor_frame(&ctx, &mut shell, &mut app);
  let cancelled = click_word(&ctx, &mut shell, &mut app, &drawn, "Cancel");
  assert!(
    cancelled.actions.is_empty(),
    "Cancel must not emit anything: {:?}",
    cancelled.actions
  );
  assert_eq!(
    app.document(),
    &before,
    "Cancel leaves the document as it was"
  );
  assert_eq!(shell.task(), None, "Cancel leaves the task");
  let (reopened, _) = editor_frame(&ctx, &mut shell, &mut app);
  assert_eq!(
    reopened.drawn.task_editor, "Edit · Profile 1",
    "and re-opening reads the document's own name, not a discarded draft"
  );
}

#[test]
fn generating_an_existing_strategy_rewrites_its_operation_and_leaves_the_rest_alone() {
  // Editing is not re-creating: `Generate toolpath` writes the operation and leaves the geometry and the tool
  // alone, and a name nobody typed over stays the document's — the rename is only emitted when it changed.
  let ctx = egui::Context::default();
  let mut app = AppState::new(profile_document());
  let mut shell = Shell::new();
  shell.set_tools(vec![flat_mill()]);
  let (drawn, _) = editor_frame(&ctx, &mut shell, &mut app);
  assert!(
    drawn
      .drawn
      .strategy_fields
      .iter()
      .any(|(label, value)| label == "Pass depth" && value == "6.00 mm"),
    "the section must read the document's own number: {:?}",
    drawn.drawn.strategy_fields
  );

  // The number the editor holds is the one it writes.
  shell.strategy.operation = Operation::Profile(ProfileParams {
    pass_depth: Length::from_mm(2.25),
    ..ProfileParams::default()
  });
  let (drawn, _) = editor_frame(&ctx, &mut shell, &mut app);
  let pressed = click_word(&ctx, &mut shell, &mut app, &drawn, "Generate toolpath");
  let action = pressed.actions[0].clone();
  match &action {
    UiAction::SetToolpathOperation {
      toolpath_id,
      operation,
    } => {
      assert_eq!(toolpath_id, "TP9");
      match operation {
        Operation::Profile(params) => assert_eq!(params.pass_depth, Length::from_mm(2.25)),
        other => panic!("the draft was a profile, got {other:?}"),
      }
    }
    other => panic!("editing a strategy must rewrite its operation, got {other:?}"),
  }
  shell.apply_action(&mut app, &action);
  assert_eq!(
    app.document().toolpaths[0].name,
    "Perfil 01",
    "an untouched name is not rewritten"
  );
  assert_eq!(
    app.document().toolpaths[0].tool_number,
    2,
    "a rewrite keeps the tool"
  );
  assert_eq!(
    app.document().toolpaths[0].entity_ids,
    vec!["E1".to_owned()]
  );
}

#[test]
fn the_editors_name_is_a_field_an_operator_can_type_into_while_editing() {
  // The old surface drew the name as a reading while editing because `cam-app` had no rename. `Generate`
  // renames now, so the field is real in both modes: pressing it and typing reaches the draft, and the header
  // follows the field rather than the document.
  let ctx = egui::Context::default();
  let mut app = AppState::new(profile_document());
  let mut shell = Shell::new();
  shell.set_tools(vec![flat_mill()]);
  let (drawn, _) = editor_frame(&ctx, &mut shell, &mut app);
  assert!(
    controls(&drawn).iter().any(|word| word == "Name"),
    "editing draws the name field: {:?}",
    drawn.drawn.strategy_hits
  );
  assert_eq!(
    shell.strategy.name, "Perfil 01",
    "the field opens on the document's name"
  );

  let _ = type_into_name(&ctx, &mut shell, &mut app, &drawn, "Roughing pass");
  assert_eq!(
    shell.strategy.name, "Roughing pass",
    "the keystrokes reach the draft"
  );
  // The header is painted above the field, so the frame that types reads the name it held when it started; the
  // next frame is the one the operator sees after the keystrokes.
  let (settled, _) = editor_frame(&ctx, &mut shell, &mut app);
  assert_eq!(
    settled.drawn.task_editor, "Edit · Roughing pass",
    "the header follows the field, not the document"
  );
  assert_eq!(
    app.document().toolpaths[0].name,
    "Perfil 01",
    "typing writes nothing: the document keeps its name until Generate"
  );
}

#[test]
fn generate_renames_an_existing_strategy_and_emits_no_rename_when_the_name_is_untouched() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(profile_document());
  let mut shell = Shell::new();
  shell.set_tools(vec![flat_mill()]);
  let (drawn, _) = editor_frame(&ctx, &mut shell, &mut app);

  // An untouched name: Generate is the operation and nothing else. A rename nobody asked for would add a
  // second undo step that undid nothing, which is a lie about what the last action was.
  let depth = app.history_depth();
  let pressed = click_word(&ctx, &mut shell, &mut app, &drawn, "Generate toolpath");
  assert_eq!(
    pressed.actions.len(),
    1,
    "an untouched name emits no rename: {:?}",
    pressed.actions
  );
  assert!(
    matches!(pressed.actions[0], UiAction::SetToolpathOperation { .. }),
    "the one action is the operation: {:?}",
    pressed.actions
  );
  for action in &pressed.actions {
    shell.apply_action(&mut app, action);
  }
  assert_eq!(
    app.history_depth(),
    depth + 1,
    "and the history gains one step, not two"
  );
  assert_eq!(app.document().toolpaths[0].name, "Perfil 01");

  // A typed name: the same press emits the operation and the rename, in the order that lets a refused
  // operation leave the name alone.
  let (drawn, _) = editor_frame(&ctx, &mut shell, &mut app);
  let typed = type_into_name(&ctx, &mut shell, &mut app, &drawn, "Roughing pass");
  let depth = app.history_depth();
  let pressed = click_word(&ctx, &mut shell, &mut app, &typed, "Generate toolpath");
  assert_eq!(
    pressed.actions.len(),
    2,
    "a typed name emits the rename and the operation: {:?}",
    pressed.actions
  );
  assert!(
    matches!(pressed.actions[0], UiAction::SetToolpathOperation { .. }),
    "the operation goes first, so a refusal cannot leave a renamed toolpath behind: {:?}",
    pressed.actions
  );
  match &pressed.actions[1] {
    UiAction::SetToolpathName { toolpath_id, name } => {
      assert_eq!(toolpath_id, "TP9");
      assert_eq!(name, "Roughing pass");
    }
    other => panic!("and then rename what was typed, got {other:?}"),
  }
  for action in &pressed.actions {
    shell.apply_action(&mut app, action);
  }

  let stored = &app.document().toolpaths[0];
  assert_eq!(stored.name, "Roughing pass", "Generate writes the name");
  assert_eq!(stored.tool_number, 2, "a rename keeps the tool");
  assert_eq!(stored.entity_ids, vec!["E1".to_owned()]);
  assert_eq!(
    app.history_depth(),
    depth + 2,
    "the rename is a step of its own, so it is one undo of its own"
  );
}

#[test]
fn cancel_puts_a_typed_name_back_and_leaves_the_document_as_it_was() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(profile_document());
  let mut shell = Shell::new();
  shell.set_tools(vec![flat_mill()]);
  let (drawn, _) = editor_frame(&ctx, &mut shell, &mut app);
  let typed = type_into_name(&ctx, &mut shell, &mut app, &drawn, "Roughing pass");
  assert_eq!(shell.strategy.name, "Roughing pass");

  let before = app.document().clone();
  let cancelled = click_word(&ctx, &mut shell, &mut app, &typed, "Cancel");
  assert!(
    cancelled.actions.is_empty(),
    "Cancel must not emit anything: {:?}",
    cancelled.actions
  );
  assert_eq!(
    app.document(),
    &before,
    "Cancel leaves the name, and everything else, as it was"
  );
  assert_eq!(shell.task(), None, "Cancel leaves the task");

  let (reopened, painted) = editor_frame(&ctx, &mut shell, &mut app);
  assert_eq!(
    shell.strategy.name, "Perfil 01",
    "the reopened draft reads the document's name, not the typed one"
  );
  assert_eq!(reopened.drawn.task_editor, "Edit · Perfil 01");
  assert!(
    painted.iter().any(|text| text == "Perfil 01"),
    "and the frame draws the document's name again: {painted:?}"
  );
}

#[test]
fn generate_is_drawn_only_when_it_can_act_and_the_footer_says_why_when_it_cannot() {
  // L6/L14: a button that emits nothing is the dead control this feature keeps correcting. Each state below is
  // one the sentence names, and the sentence is what replaces the button.
  let ctx = egui::Context::default();
  let cases: Vec<(&str, Document, Option<u32>, bool, &str)> = vec![
    (
      "no tool chosen",
      rectangle_document(),
      None,
      true,
      "No tool yet",
    ),
    (
      "nothing selected",
      rectangle_document(),
      Some(2),
      false,
      "Nothing is selected",
    ),
  ];
  for (label, document, tool, select, expected) in cases {
    let mut app = AppState::new(document);
    if select {
      app.select(vec!["E1".to_owned()]);
    }
    let mut shell = Shell::new();
    shell.set_tools(vec![flat_mill()]);
    let _ = editor_frame(&ctx, &mut shell, &mut app);
    shell.strategy.tool_number = tool;
    let (drawn, _) = editor_frame(&ctx, &mut shell, &mut app);
    assert!(
      !controls(&drawn)
        .iter()
        .any(|word| word == "Generate toolpath"),
      "{label}: Generate must not be drawn while it cannot act: {:?}",
      drawn.drawn.strategy_hits
    );
    assert!(
      drawn.drawn.strategy_refusal.contains(expected),
      "{label}: the footer must say why, got {:?}",
      drawn.drawn.strategy_refusal
    );
    assert!(
      controls(&drawn).iter().any(|word| word == "Cancel"),
      "{label}: Cancel can always be honoured, so it is always drawn"
    );
  }

  // A strategy whose operation this shell does not draft: no form, no button, and the sentence names both the
  // operation and the route that does author it.
  let mut app = AppState::new(document_with(
    Operation::Drilling(DrillParams::default()),
    "Drill 1",
  ));
  let mut shell = Shell::new();
  shell.set_tools(vec![flat_mill()]);
  let (drawn, _) = editor_frame(&ctx, &mut shell, &mut app);
  assert!(
    !controls(&drawn)
      .iter()
      .any(|word| word == "Generate toolpath"),
    "an operation with no editor here must not offer Generate: {:?}",
    drawn.drawn.strategy_hits
  );
  assert!(
    drawn.drawn.strategy_refusal.contains("Drilling")
      && drawn.drawn.strategy_refusal.contains("cam-cli"),
    "the refusal must name the operation and the route: {:?}",
    drawn.drawn.strategy_refusal
  );
  // Choosing a kind this shell does draft replaces it, and the footer comes back: the refusal is a state, not
  // a dead end.
  let (drawn, _) = editor_frame(&ctx, &mut shell, &mut app);
  let after = click_word(&ctx, &mut shell, &mut app, &drawn, "Pocket");
  assert!(
    controls(&after)
      .iter()
      .any(|word| word == "Generate toolpath"),
    "choosing a drafted kind must restore the footer: {:?}",
    after.drawn.strategy_hits
  );
}
