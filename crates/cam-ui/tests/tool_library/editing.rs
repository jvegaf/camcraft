use cam_app::AppState;
use cam_ui::tool_library::{FormField, ShapeField};
use cam_ui::{Shell, UiAction};

use super::support::*;

// ---------------------------------------------------------------------------------------------
// S9: every keystroke edits a draft, and only `Use this tool` writes
// ---------------------------------------------------------------------------------------------

#[test]
fn typing_a_diameter_edits_the_draft_and_writes_nothing_until_use_this_tool() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(1));
  let mut shell = Shell::new();
  shell.set_tools(vec![flat_mill("G1", Some(1), 6.0)]);
  shell.open_tool_library(&app);
  let before = app.document().clone();
  let _ = frame_with(&ctx, &mut shell, &mut app);
  let (_, runs) = frame_with_runs(&ctx, &mut shell, &mut app);

  let (typed, texts) = type_into_box(&ctx, &mut shell, &mut app, &runs, "Diameter", "4.5");

  // The keystroke reached the draft, and the form shows what the draft now says: a form one frame behind the
  // boxes would be describing a tool the operator is not editing.
  let diameter = FormField::Shape(ShapeField::Diameter);
  let draft = shell.tool_draft.as_ref().expect("the modal holds a draft");
  assert_eq!(draft.text(diameter), "4.5");
  let committed = draft.commit().expect("4.5 mm is a diameter");
  assert_eq!(
    committed.geometry.diameter.map(|diameter| diameter.mm()),
    Some(4.5)
  );
  assert_eq!(value_of(&typed, "GEOMETRY", "Diameter"), "4.5");
  assert!(
    joined(&texts).contains("4.5"),
    "the typed diameter was never painted: {}",
    joined(&texts)
  );

  // And nothing was written: no library row, no action, and the document still cuts with what it cut with.
  assert!(
    typed.drawn.tool_saved.is_empty(),
    "a keystroke wrote the library: {:?}",
    typed.drawn.tool_saved
  );
  assert!(
    typed.actions.is_empty(),
    "a keystroke changed the document: {:?}",
    typed.actions
  );
  assert!(
    shell.show_tool_library,
    "typing closed the modal before the operator said yes"
  );
  assert_eq!(app.document(), &before, "a keystroke changed the document");

  // `Use this tool` is what writes, and what it writes is the number of the tool the form describes.
  let (wrote, _) = click_word(&ctx, &mut shell, &mut app, &typed, "Use this tool");
  assert_eq!(
    wrote.actions,
    vec![UiAction::SetToolpathToolNumber {
      toolpath_id: "P1".to_owned(),
      tool_number: 1,
    }],
    "`Use this tool` must emit exactly the number the form showed"
  );
  assert!(!shell.show_tool_library, "a commit closes the modal");
}

#[test]
fn a_draft_that_cannot_commit_is_replaced_by_a_sentence_naming_the_first_thing_wrong() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(1));

  // A box whose text is not a number: the sentence names the box and quotes what it holds.
  let mut unreadable = Shell::new();
  unreadable.set_tools(vec![measured("G1", Some(1))]);
  unreadable.open_tool_library(&app);
  let _ = frame_with(&ctx, &mut unreadable, &mut app);
  let (_, runs) = frame_with_runs(&ctx, &mut unreadable, &mut app);
  let (typed, texts) = type_into_box(&ctx, &mut unreadable, &mut app, &runs, "Feed", "abc");
  assert!(
    !hits(&typed).contains(&"Use this tool"),
    "a draft that does not describe a tool still offered to write one: {:?}",
    hits(&typed)
  );
  let sentence = "Feed has to be a number, and \"abc\" is not one.";
  assert!(
    typed.drawn.tool_library.iter().any(|word| word == sentence),
    "the form did not name the box that is wrong: {:?}",
    typed.drawn.tool_library
  );
  assert!(
    joined(&texts).contains(sentence),
    "the sentence was reported and never painted: {}",
    joined(&texts)
  );
  assert!(unreadable.show_tool_library, "a refusal closed the modal");

  // Numbers that parse and together describe no tool: the store's own rule is the sentence, so the button and
  // the library cannot disagree about what a tool is.
  let mut impossible = Shell::new();
  impossible.set_tools(vec![measured("G1", Some(1))]);
  impossible.open_tool_library(&app);
  let _ = frame_with(&ctx, &mut impossible, &mut app);
  let (_, runs) = frame_with_runs(&ctx, &mut impossible, &mut app);
  let (zeroed, texts) = type_into_box(&ctx, &mut impossible, &mut app, &runs, "Diameter", "0");
  assert!(!hits(&zeroed).contains(&"Use this tool"));
  let sentence = "A diameter has to be greater than zero, and this one is 0 mm.";
  assert!(
    zeroed
      .drawn
      .tool_library
      .iter()
      .any(|word| word == sentence),
    "the form did not name the first thing wrong: {:?}",
    zeroed.drawn.tool_library
  );
  assert!(joined(&texts).contains(sentence));
  assert!(impossible.show_tool_library);
  assert!(
    zeroed.actions.is_empty(),
    "a refused draft wrote the document: {:?}",
    zeroed.actions
  );
}

#[test]
fn cancel_discards_the_draft_and_leaves_the_toolpath_the_search_and_the_library_alone() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  let _ = open_library(&ctx, &mut shell, &mut app);
  let before = app.document().clone();

  let (_, runs) = frame_with_runs(&ctx, &mut shell, &mut app);
  let (edited, _) = type_into_box(&ctx, &mut shell, &mut app, &runs, "Diameter", "4.5");
  assert_eq!(value_of(&edited, "GEOMETRY", "Diameter"), "4.5");

  let (left, _) = click_word(&ctx, &mut shell, &mut app, &edited, "Cancel");
  assert!(
    left.actions.is_empty(),
    "`Cancel` wrote something: {:?}",
    left.actions
  );
  assert!(
    left.drawn.tool_saved.is_empty(),
    "`Cancel` wrote the library: {:?}",
    left.drawn.tool_saved
  );
  assert_eq!(
    app.document(),
    &before,
    "the document changed under `Cancel`"
  );
  assert!(!shell.show_tool_library);
  // The drafts go too: a search left behind would filter the next visit for a reason nobody remembers, and a
  // tool draft left behind would be a form describing a tool nobody is editing with numbers nobody confirmed.
  assert_eq!(shell.tool_search, "");
  assert!(shell.tool_library_target().is_none());
  assert!(
    shell.tool_draft.is_none(),
    "`Cancel` left the edited draft behind: {:?}",
    shell.tool_draft
  );

  // And a second visit starts on the tool the document cuts with, not on the row the cancelled visit left.
  let reopened = open_library(&ctx, &mut shell, &mut app);
  assert_eq!(reopened.drawn.tool_selected.as_deref(), Some("G2"));
  assert_eq!(
    value_of(&reopened, "GEOMETRY", "Diameter"),
    "6",
    "a second visit shows the library's diameter, not the one the cancelled visit typed"
  );
}

#[test]
fn leaving_a_row_discards_the_draft_that_belongs_to_it() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  let opened = open_library(&ctx, &mut shell, &mut app);
  let (chosen, _) = click_word(
    &ctx,
    &mut shell,
    &mut app,
    &opened,
    "T1 - Flat end mill (6mm)",
  );
  assert_eq!(chosen.drawn.tool_selected.as_deref(), Some("G1"));
  let (_, runs) = frame_with_runs(&ctx, &mut shell, &mut app);
  let (edited, _) = type_into_box(&ctx, &mut shell, &mut app, &runs, "Diameter", "4.5");
  assert_eq!(value_of(&edited, "GEOMETRY", "Diameter"), "4.5");

  // The draft belongs to the row it was seeded from, so another row is a different draft — not the same boxes
  // describing a second tool.
  let (other, _) = click_word(
    &ctx,
    &mut shell,
    &mut app,
    &edited,
    "T3 - Corner radius (12mm)",
  );
  assert_eq!(other.drawn.tool_selected.as_deref(), Some("G3"));
  assert_eq!(value_of(&other, "GEOMETRY", "Diameter"), "12");
  assert_eq!(
    shell
      .tool_draft
      .as_ref()
      .map(|draft| draft.geometry_id.clone()),
    Some("G3".to_owned())
  );
  // Coming back is a fresh seed from the record, because the edit went with the row it was typed into.
  let (back, _) = click_word(
    &ctx,
    &mut shell,
    &mut app,
    &other,
    "T1 - Flat end mill (6mm)",
  );
  assert_eq!(value_of(&back, "GEOMETRY", "Diameter"), "6");
}
