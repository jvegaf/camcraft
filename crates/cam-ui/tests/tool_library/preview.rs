use cam_app::AppState;
use cam_ui::Shell;

use super::support::*;

// ---------------------------------------------------------------------------------------------
// S2's third column: the preview — the card, one shared scale, and two dimensions
// ---------------------------------------------------------------------------------------------

#[test]
fn the_preview_card_is_the_preview_column_drawn_to_one_shared_scale() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  let opened = open_library(&ctx, &mut shell, &mut app);
  let columns = opened.drawn.tool_columns.clone();

  // Ø6 first, then Ø12: a picture that drew every tool to the frame's width would give the same number twice,
  // which is the bug this pins.
  let (small, _) = click_word(
    &ctx,
    &mut shell,
    &mut app,
    &opened,
    "T1 - Flat end mill (6mm)",
  );
  let (small_card, small_width) = {
    let card = small
      .drawn
      .tool_drawing
      .expect("the flat end mill was not drawn");
    (card, small.drawn.tool_width_px)
  };
  assert!(
    small_width > 0.0,
    "the drawing reported no width at all, so `to scale` is unanswerable"
  );
  // The card is the preview column: as tall and as wide as the column it was given.
  assert_eq!(
    (small_card.2, small_card.3),
    (columns[2], 330.0),
    "the preview card is not the preview column's own rectangle"
  );

  let (large, _) = click_word(
    &ctx,
    &mut shell,
    &mut app,
    &small,
    "T3 - Corner radius (12mm)",
  );
  let (large_card, large_width) = {
    let card = large
      .drawn
      .tool_drawing
      .expect("the corner radius was not drawn");
    (card, large.drawn.tool_width_px)
  };
  assert_eq!(
    (large_card.0, large_card.1, large_card.2, large_card.3),
    small_card,
    "the two tools were drawn in different boxes, so their widths are not comparable"
  );
  let ratio = large_width / small_width;
  assert!(
    (ratio - 2.0).abs() < 0.02,
    "the drawing is not to one shared scale: Ø12 came out {ratio:.3} times Ø6, not 2"
  );
}

#[test]
fn the_callouts_name_the_overall_length_and_the_diameter_and_nothing_the_library_lacks() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(1));
  let mut shell = Shell::new();
  // A record that holds both dimensions, and then one that holds neither.
  shell.set_tools(vec![measured("G1", Some(1))]);
  shell.open_tool_library(&app);
  let _ = frame_with(&ctx, &mut shell, &mut app);
  let (drawn, texts) = frame_with(&ctx, &mut shell, &mut app);

  assert!(
    drawn
      .drawn
      .tool_callouts
      .iter()
      .any(|callout| callout == "Overall length 38 mm"),
    "the length was not called out: {:?}",
    drawn.drawn.tool_callouts
  );
  assert!(
    drawn
      .drawn
      .tool_callouts
      .iter()
      .any(|callout| callout == "Diameter Ø6 mm"),
    "the diameter was not called out: {:?}",
    drawn.drawn.tool_callouts
  );
  // A callout is a sentence about a dimension line, and what the card paints is the dimension itself: the two
  // numbers the report's words are built from are on the card.
  let painted = joined(&texts);
  for number in ["38", "Ø6"] {
    assert!(
      painted.contains(number),
      "the callout naming {number} was reported and the card painted no such number: {painted}"
    );
  }

  // A tool the library never measured has no length to call out: a dimension the record does not hold is a
  // dimension the shell invented.
  let mut thin = Shell::new();
  thin.set_tools(vec![flat_mill("G1", Some(1), 6.0)]);
  thin.open_tool_library(&app);
  let _ = frame_with(&ctx, &mut thin, &mut app);
  let (bare, _) = frame_with(&ctx, &mut thin, &mut app);
  assert!(
    !bare
      .drawn
      .tool_callouts
      .iter()
      .any(|callout| callout.starts_with("Overall length")),
    "an unmeasured tool was given a length: {:?}",
    bare.drawn.tool_callouts
  );
  assert!(bare
    .drawn
    .tool_callouts
    .iter()
    .any(|c| c.starts_with("Diameter")));
}

#[test]
fn the_kinds_own_name_is_the_caption_under_the_card() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  let opened = open_library(&ctx, &mut shell, &mut app);

  // One row of each of the six kinds: the caption says what the tool is called, so it is the row's own family.
  let mut frame = opened;
  for (family, title, _) in frame.drawn.tool_rows.clone() {
    let (chosen, texts) = click_word(&ctx, &mut shell, &mut app, &frame, &title);
    assert!(
      chosen.drawn.tool_selected.is_some(),
      "the row {title:?} selected nothing"
    );
    assert_eq!(
      chosen.drawn.tool_preview_caption.as_deref(),
      Some(family.as_str()),
      "the caption under the {title:?} card is not the kind's own name"
    );
    assert!(
      joined(&texts).contains(&family),
      "the caption was reported and never painted: {}",
      joined(&texts)
    );
    frame = chosen;
  }
}
