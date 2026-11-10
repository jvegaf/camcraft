use cam_app::AppState;
use cam_ui::tool_library::tool_title;
use cam_ui::Shell;

use super::support::*;

// ---------------------------------------------------------------------------------------------
// S1: the header band — the dialog's title, the chosen tool's title, and the close cross
// ---------------------------------------------------------------------------------------------

#[test]
fn the_header_band_names_the_dialog_and_then_the_tool_the_tree_chose() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  // A library the toolpath's own number is not in: the tree selects nothing, so the band carries the
  // dialog's title and nothing else.
  shell.set_tools(vec![flat_mill("G1", Some(1), 6.0)]);
  let opened = open_library(&ctx, &mut shell, &mut app);

  assert_eq!(opened.drawn.tool_selected, None);
  assert_eq!(
    opened.drawn.tool_header.as_deref(),
    Some("Choose tool"),
    "the band must carry the dialog's own title while nothing is selected"
  );
  let (_, texts) = frame_with(&ctx, &mut shell, &mut app);
  assert!(
    joined(&texts).contains("Choose tool"),
    "the title was reported and never painted: {}",
    joined(&texts)
  );

  // Once a row is chosen the title is the tool's own: a title one frame behind the form is a reporting
  // defect, so the click's own frame carries the new sentence.
  let (chosen, texts) = click_word(
    &ctx,
    &mut shell,
    &mut app,
    &opened,
    "T1 - Flat end mill (6mm)",
  );
  assert_eq!(chosen.drawn.tool_selected.as_deref(), Some("G1"));
  let title = chosen
    .drawn
    .tool_header
    .clone()
    .expect("an open modal always carries a title");
  assert_eq!(
    title,
    format!(
      "Choose tool — {}",
      tool_title(&flat_mill("G1", Some(1), 6.0))
    ),
    "the band does not name the tool the tree just chose"
  );
  assert!(
    joined(&texts).contains(&title),
    "the chosen tool's title was reported and never painted: {}",
    joined(&texts)
  );
}

#[test]
fn the_close_cross_sits_at_the_far_right_and_leaves_without_writing() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  let opened = open_library(&ctx, &mut shell, &mut app);
  let before = app.document().clone();

  // The cross is the rightmost thing the modal drew: the band carries it at its far end, past every control
  // in the body and the commit pair in the footer.
  let close = opened
    .drawn
    .tool_library_hits
    .iter()
    .find(|(word, _)| word == "Close")
    .map(|(_, point)| *point)
    .unwrap_or_else(|| {
      panic!(
        "the header drew no close cross: {:?}",
        opened.drawn.tool_library_hits
      )
    });
  for (word, point) in &opened.drawn.tool_library_hits {
    assert!(
      close.0 >= point.0,
      "the close cross is not at the far right: {word:?} is drawn right of it"
    );
  }

  let (left, _) = click_word(&ctx, &mut shell, &mut app, &opened, "Close");
  assert!(
    !shell.show_tool_library,
    "the close cross left the modal open"
  );
  assert!(
    left.drawn.tool_saved.is_empty(),
    "closing the modal wrote the library: {:?}",
    left.drawn.tool_saved
  );
  assert!(
    left.actions.is_empty(),
    "closing the modal wrote the document: {:?}",
    left.actions
  );
  assert_eq!(
    app.document(),
    &before,
    "a close changed the document it was opened over"
  );
  let (after, _) = frame_with(&ctx, &mut shell, &mut app);
  assert!(
    after.drawn.tool_library.is_empty() && after.drawn.tool_header.is_none(),
    "a closed modal still reported itself: {:?}",
    after.drawn.tool_header
  );
}
