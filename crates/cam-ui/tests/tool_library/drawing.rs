use cam_app::AppState;
use cam_ui::Shell;

use super::support::*;

// ---------------------------------------------------------------------------------------------
// The drawing: to scale, with its own dimensions called out
// ---------------------------------------------------------------------------------------------

#[test]
fn the_drawing_is_one_shared_scale_so_two_tools_are_comparable() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  let opened = open_library(&ctx, &mut shell, &mut app);

  // Ø6.35 and Ø12.7 in the same box: a picture that drew every tool to the frame's width would give the same
  // number twice, which is the bug this pins.
  let (small, small_texts) = click_word(&ctx, &mut shell, &mut app, &opened, "T1 - Tool (6mm)");
  let small_width = small.drawn.tool_width_px;
  let small_box = small
    .drawn
    .tool_drawing
    .expect("the flat mill was not drawn");
  assert!(
    small_width > 0.0,
    "the drawing reported no width at all, so `to scale` is unanswerable"
  );

  let (large, large_texts) = click_word(&ctx, &mut shell, &mut app, &small, "T3 - Tool (12.7mm)");
  let large_width = large.drawn.tool_width_px;
  assert_eq!(
    large.drawn.tool_drawing.map(|rect| rect.2),
    Some(small_box.2),
    "the two tools were drawn in boxes of different widths, so their widths are not comparable"
  );
  let ratio = large_width / small_width;
  assert!(
    (ratio - 12.7 / 6.0).abs() < 0.02,
    "the drawing is not to one shared scale: Ø12.7 came out {ratio:.3} times Ø6, not {:.3}",
    12.7 / 6.0
  );

  // The callouts are the tool's own numbers, painted and reported, and a tool has only the ones it holds.
  assert!(
    large
      .drawn
      .tool_callouts
      .iter()
      .any(|callout| callout == "Ø 12.70 mm"),
    "the width was not called out: {:?}",
    large.drawn.tool_callouts
  );
  assert!(
    large
      .drawn
      .tool_callouts
      .iter()
      .any(|callout| callout.contains("Included angle 90.0°")),
    "the V-bit's angle was not called out: {:?}",
    large.drawn.tool_callouts
  );
  assert!(
    !small
      .drawn
      .tool_callouts
      .iter()
      .any(|callout| callout.contains("Included angle")),
    "a flat end mill was given a V's dimension: {:?}",
    small.drawn.tool_callouts
  );
  for callout in &large.drawn.tool_callouts {
    assert!(
      joined(&large_texts).contains(callout),
      "the callout {callout:?} was reported and never painted: {}",
      joined(&large_texts)
    );
  }
  assert!(joined(&small_texts).contains("Ø 6.00 mm"));
}
