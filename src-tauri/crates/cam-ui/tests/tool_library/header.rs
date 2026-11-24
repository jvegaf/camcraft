use cam_app::AppState;
use cam_ui::Shell;

use super::support::*;

// ---------------------------------------------------------------------------------------------
// S1: the header band — the dialog's title, the chosen tool's title, and the close cross
// ---------------------------------------------------------------------------------------------

#[test]
fn the_header_band_is_always_choose_tool() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  // A library the toolpath's own number is not in: the tree selects nothing.
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

  // Once a row is chosen the title stays `Choose tool`: the chosen tool is already in the name field.
  let (fresh, _) = frame_with(&ctx, &mut shell, &mut app);
  let at = fresh
    .drawn
    .tool_library_hits
    .iter()
    .find(|(word, _)| word == "T1 - Flat end mill (6mm)")
    .map(|(_, point)| egui::pos2(point.0, point.1))
    .expect("row hit");
  click_at(&ctx, &mut shell, &mut app, at);
  let (chosen, texts) = frame_with(&ctx, &mut shell, &mut app);
  assert_eq!(
    chosen.drawn.tool_selected.as_deref(),
    Some("G1"),
    "row click did not select: hit={at:?} hits={:?}",
    fresh.drawn.tool_library_hits
  );
  assert_eq!(
    chosen.drawn.tool_header.as_deref(),
    Some("Choose tool"),
    "the band must be exactly `Choose tool` even with a tool selected"
  );
  assert!(
    joined(&texts).contains("Choose tool"),
    "the title was reported and never painted: {}",
    joined(&texts)
  );
  assert!(
    !joined(&texts).contains("Choose tool —"),
    "the header must not repeat the chosen tool's name: {}",
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
