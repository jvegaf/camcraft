use super::support::*;
use cam_app::AppState;
use cam_model::{Operation, ProfileParams};
use cam_ui::strategy::{self, Draft};
use cam_ui::{Category, Shell, ShellOutput};
use cam_units::Length;

// ---------------------------------------------------------------------------------------------
// Preserved: the editor is the toolpath task's, and no other panel's
// ---------------------------------------------------------------------------------------------

#[test]
fn the_editor_is_the_toolpath_task_and_the_other_tasks_are_untouched() {
  // The editor replaces the panel's body only while `TOOLPATHS` is open. The other two tasks keep their own
  // header and draw none of S10's surface, which is what keeps the inspector's tabs working.
  let ctx = egui::Context::default();
  let mut app = AppState::new(profile_document());
  let mut shell = Shell::new();
  shell.set_tools(vec![flat_mill()]);

  // Idle: no task, no editor.
  let (idle, _) = {
    let _ = frame(&ctx, &mut shell, &mut app);
    frame(&ctx, &mut shell, &mut app)
  };
  assert!(idle.drawn.task_editor.is_empty(), "idle draws no editor");
  assert!(
    idle.drawn.strategy_note.is_empty() && idle.drawn.strategy_hits.is_empty(),
    "idle draws none of S10's surface: {:?}",
    idle.drawn.strategy_hits
  );

  for (category, label) in [
    (Category::Relief3d, "RELIEF AND 3D PART"),
    (Category::MachineOutput, "MACHINE AND OUTPUT"),
  ] {
    shell.open_task(category);
    let _ = frame(&ctx, &mut shell, &mut app);
    let (output, _) = frame(&ctx, &mut shell, &mut app);
    assert_eq!(
      output.drawn.task_editor, label,
      "{label} keeps its own header"
    );
    assert!(
      output.drawn.strategy_note.is_empty(),
      "{label} draws no strategy note"
    );
  }
}

#[test]
fn the_draft_seeds_from_the_document_and_cancel_puts_it_back() {
  // The draft is a draft: it seeds from the toolpath it edits, it can be typed into, and `Cancel` throws the
  // typed numbers away so the next opening reads the document again.
  let ctx = egui::Context::default();
  let mut app = AppState::new(profile_document());
  let mut shell = Shell::new();
  shell.set_tools(vec![flat_mill()]);

  let (_drawn, _) = editor_frame(&ctx, &mut shell, &mut app);
  assert_eq!(
    shell.strategy.name, "Perfil 01",
    "the draft is seeded with the document's name"
  );
  assert_eq!(
    shell.strategy.operation,
    Operation::Profile(ProfileParams::default()),
    "and with the document's operation"
  );

  shell.strategy.operation = Operation::Profile(ProfileParams {
    cut_depth: Length::from_mm(3.0),
    ..ProfileParams::default()
  });
  let (drawn, _) = editor_frame(&ctx, &mut shell, &mut app);
  let cancelled = click_word(&ctx, &mut shell, &mut app, &drawn, "Cancel");
  assert!(cancelled.actions.is_empty());
  assert_eq!(shell.task(), None);

  let (reopened, _) = editor_frame(&ctx, &mut shell, &mut app);
  assert_eq!(
    reopened
      .drawn
      .strategy_fields
      .iter()
      .find(|(label, _)| label == "Cut depth")
      .map(|(_, value)| value.clone()),
    Some("18.00 mm".to_owned()),
    "Cancel discards the typed number: the section reads the document again"
  );
  assert_eq!(
    shell.strategy.operation,
    Operation::Profile(ProfileParams::default())
  );
}

#[test]
fn the_draft_is_a_public_parameter_so_a_host_or_a_test_can_set_it() {
  // The same rule `Shell::material_size` follows: a draft that could not be set from outside a frame is a
  // draft only a mouse could test. This also pins the defaults the editor opens with.
  let draft = Draft::default();
  assert_eq!(draft.name, "Profile 1");
  assert_eq!(draft.tool_number, None);
  assert!(draft.quick_open);
  assert_eq!(
    draft.operation,
    Operation::Profile(ProfileParams::default())
  );
  assert_eq!(strategy::NOTE, NOTE);
}

#[test]
fn a_control_the_scroll_area_clips_is_not_reported_as_pressable() {
  // L23, applied before a reviewer has to find it: a clipped control is still a shape, so a reported point is
  // only a promise while the frame really showed it. At 800 px the inspector's body is shorter than the editor,
  // and the footer sits below its clip — an operator cannot press what is not on screen.
  let ctx = egui::Context::default();
  let mut app = AppState::new(profile_document());
  let mut shell = Shell::new();
  shell.set_tools(vec![flat_mill()]);
  shell.open_task(Category::Toolpaths);

  let short = || egui::RawInput {
    screen_rect: Some(egui::Rect::from_min_size(
      egui::Pos2::ZERO,
      egui::vec2(1200.0, 800.0),
    )),
    ..Default::default()
  };
  let mut last = None;
  for _ in 0..2 {
    let mut captured: Option<ShellOutput> = None;
    let full = ctx.run_pass(short(), |ctx| {
      captured = Some(shell.show(&mut app, ctx));
    });
    full.drop_without_applying_deltas();
    last = captured;
  }
  let output = last.expect("the shell ran");

  // The frame still draws the editor — this is not "nothing was drawn" — and it reports the parts it showed.
  assert_eq!(output.drawn.strategy_note, NOTE);
  assert!(
    output
      .drawn
      .strategy_hits
      .iter()
      .any(|(word, _)| word == "Quick parameter edit"),
    "the parts the frame showed are pressable: {:?}",
    output.drawn.strategy_hits
  );
  for word in ["Generate toolpath", "Cancel"] {
    assert!(
      !output
        .drawn
        .strategy_hits
        .iter()
        .any(|(name, _)| name == word),
      "{word} is below the scroll area's clip at 800 px and must not be reported as pressable: {:?}",
      output.drawn.strategy_hits
    );
  }
}

#[test]
fn changing_the_kind_while_editing_leaves_the_name_field_on_the_documents_name() {
  // R2-001 in this task's review: while editing, the draft's name is the document's, so choosing a kind must
  // not invent one — the header read `Edit · Pocket 1` over a document still called `Perfil 01`. Renaming is
  // possible now, but a kind is not a rename: the operator types the new name and `Generate` writes it.
  let ctx = egui::Context::default();
  // The document the finding describes: its toolpath still carries the *previous kind's default* name, which is
  // the only case in which the draft would have renamed it. A document called `Perfil 01` would have hidden the
  // defect — the rename branch never runs for a name somebody chose — so this test asks for `Profile 1`.
  let mut app = AppState::new(document_with(
    Operation::Profile(ProfileParams::default()),
    "Profile 1",
  ));
  let mut shell = Shell::new();
  shell.set_tools(vec![flat_mill()]);
  let (editing, _) = editor_frame(&ctx, &mut shell, &mut app);
  assert_eq!(
    editing.drawn.task_editor, "Edit · Profile 1",
    "the editing header names the document's toolpath"
  );

  let after = click_word(&ctx, &mut shell, &mut app, &editing, "Pocket");
  assert_eq!(
    after.drawn.task_editor, "Edit · Profile 1",
    "choosing a kind does not rewrite the name field"
  );
  assert_eq!(
    shell.strategy.name, "Profile 1",
    "and the draft did not take a new one"
  );
  assert!(
    after
      .drawn
      .strategy_cards
      .iter()
      .any(|(word, chosen, _)| word == "Pocket" && *chosen),
    "the kind itself did change, or this test would prove nothing: {:?}",
    after.drawn.strategy_cards
  );
}

#[test]
fn pressing_the_kind_a_strategy_already_is_keeps_the_numbers_that_were_typed() {
  // R3-001: `choose_kind` re-defaulted the operation on every press, so clicking the card that was already
  // chosen silently threw away the numbers typed into `CUT` and the quick block.
  let ctx = egui::Context::default();
  let mut app = AppState::new(rectangle_document());
  app.select(vec!["E1".to_owned()]);
  let mut shell = Shell::new();
  shell.set_tools(vec![flat_mill()]);
  let _ = editor_frame(&ctx, &mut shell, &mut app);
  shell.strategy.tool_number = Some(2);
  match &mut shell.strategy.operation {
    Operation::Profile(params) => params.cut_depth = cam_units::Length::from_mm(7.5),
    other => panic!("the draft opens as a profile, not {other:?}"),
  }
  let (chosen, _) = editor_frame(&ctx, &mut shell, &mut app);
  assert!(
    chosen
      .drawn
      .strategy_cards
      .iter()
      .any(|(word, on, _)| word == "Profile" && *on),
    "the profile card is the chosen one: {:?}",
    chosen.drawn.strategy_cards
  );

  let _ = click_word(&ctx, &mut shell, &mut app, &chosen, "Profile");
  match &shell.strategy.operation {
    Operation::Profile(params) => assert!(
      (params.cut_depth.mm() - 7.5).abs() < 1e-9,
      "pressing the chosen kind must not re-default the numbers, got {}",
      params.cut_depth.mm()
    ),
    other => panic!("the draft changed kind by itself: {other:?}"),
  }
}
