use super::support::*;
use cam_app::AppState;
use cam_ui::Shell;

// ---------------------------------------------------------------------------------------------
// One surface for both modes
// ---------------------------------------------------------------------------------------------

#[test]
fn creating_and_editing_are_one_surface_and_only_the_header_prefix_changes() {
  // S10's first sentence. The two modes run the same code path; the subject the document puts in front of it
  // decides the prefix, and nothing else about the surface.
  let ctx = egui::Context::default();

  // Creating: the document holds no toolpath, so the draft is the editor's subject.
  let mut app = AppState::new(rectangle_document());
  app.select(vec!["E1".to_owned()]);
  let mut shell = Shell::new();
  shell.set_tools(vec![flat_mill()]);
  // The editor seeds its draft from the document, so the tests that set a draft open the editor first and
  // then set it — the order a host follows too.
  let _ = editor_frame(&ctx, &mut shell, &mut app);
  shell.strategy.tool_number = Some(2);
  let (creating, creating_text) = editor_frame(&ctx, &mut shell, &mut app);
  assert_eq!(
    creating.drawn.task_editor, "New toolpath · Profile 1",
    "the creating mode's header names the draft"
  );

  // Editing: the document holds one toolpath, and the editor edits it.
  let mut app = AppState::new(profile_document());
  let mut shell = Shell::new();
  shell.set_tools(vec![flat_mill()]);
  let (editing, editing_text) = editor_frame(&ctx, &mut shell, &mut app);
  assert_eq!(
    editing.drawn.task_editor, "Edit · Perfil 01",
    "the editing mode's header names the document's toolpath"
  );

  // The same surface: every control the one mode drew, the other drew too, in the same order. The one
  // difference the spec names is the header prefix, above — and the name is a **field in both modes** now that
  // `cam-app` can rename a toolpath, so what is asserted here is that both draw it and both draw the same rest.
  assert_eq!(
    controls(&creating),
    controls(&editing),
    "creating and editing must offer the same editor"
  );
  assert!(
    controls(&creating).iter().any(|word| word == "Name"),
    "creating a toolpath offers its name field: {:?}",
    creating.drawn.strategy_hits
  );
  assert!(
    controls(&editing).iter().any(|word| word == "Name"),
    "editing offers the same name field, because `Generate` can rename: {:?}",
    editing.drawn.strategy_hits
  );
  // The sentence that explained the old limitation is gone with the limitation: a sentence about a name that
  // could not be changed would now be false, and a false sentence is worse than none.
  for (mode, painted) in [("creating", &creating_text), ("editing", &editing_text)] {
    assert!(
      !painted
        .iter()
        .any(|text| text.starts_with("The name is fixed while editing")),
      "{mode} must not explain a limitation that no longer exists: {painted:?}"
    );
  }
  assert_eq!(
    creating.drawn.strategy_note, editing.drawn.strategy_note,
    "the note is the same sentence in both modes"
  );
  assert_eq!(
    creating.drawn.strategy_cards, editing.drawn.strategy_cards,
    "the strategy's picture cards are the same choice in both modes"
  );
}

#[test]
fn the_editor_holds_its_parts_in_the_specs_order() {
  // S10's order: name, current-tool card with `Choose another tool…`, the collapsible `Quick parameter edit`,
  // the sections, the note, then the footer pair. Read from the painted text, which is drawn in that order,
  // so a part that moved is a part whose index moved.
  let ctx = egui::Context::default();
  let mut app = AppState::new(profile_document());
  let mut shell = Shell::new();
  shell.set_tools(vec![flat_mill()]);

  let (output, painted) = editor_frame(&ctx, &mut shell, &mut app);
  let index = |word: &str| {
    painted
      .iter()
      .position(|text| text == word)
      .unwrap_or_else(|| panic!("the editor did not draw {word:?}: {painted:?}"))
  };
  let order = [
    "Name",
    "TOOL — Perfil 01",
    "Choose another tool…",
    "Quick parameter edit",
    "CUT",
    NOTE,
    "Generate toolpath",
    "Cancel",
  ];
  let mut last = 0;
  for word in order {
    let at = index(word);
    assert!(
      at >= last,
      "S10 puts {word:?} after the part before it, and the frame drew it before: {painted:?}"
    );
    last = at;
  }
  // The card's own opener is a point the frame really showed, not a word painted in passing.
  assert!(
    output
      .drawn
      .tool_card_hits
      .iter()
      .any(|(word, _)| word == "Choose another tool…"),
    "the card's opener must be pressable: {:?}",
    output.drawn.tool_card_hits
  );
}

#[test]
fn the_note_is_the_specs_sentence_verbatim() {
  // S10 quotes the sentence, and the reference frame prints it under the sections. One string, drawn and
  // reported, so the frame and this test cannot read two spellings of it.
  let ctx = egui::Context::default();
  let mut app = AppState::new(profile_document());
  let mut shell = Shell::new();
  shell.set_tools(vec![flat_mill()]);

  let (output, painted) = editor_frame(&ctx, &mut shell, &mut app);

  assert_eq!(output.drawn.strategy_note, NOTE);
  assert!(
    painted.iter().any(|text| text == NOTE),
    "the note must be on the frame, not only in the report: {painted:?}"
  );
}
