use super::support::*;

// ---------------------------------------------------------------------------------------------
// S8 — `2D | 3D` is a segmented toggle at the top right of the canvas, and 3D is a mode: the canvas
// splits in two, and the 3D half carries its own chrome (camui-prancheta T6).
// ---------------------------------------------------------------------------------------------

#[test]
fn the_toggle_paints_the_view_it_offers_where_a_hand_can_reach_it() {
  // The toggle reports its cells (a test presses them) *and* paints them: a control that only reported its
  // words would be invisible to the operator. Its home moved to the canvas in camui-prancheta T6 (S8), so this
  // no longer looks at the app bar.
  let mut harness = Harness::new();
  harness.frame(Vec::new());

  assert!(
    harness.texts.iter().any(|text| text == "3D"),
    "the canvas offers the 3D toggle, painted texts: {:?}",
    harness.texts
  );
  assert!(
    harness
      .output
      .drawn
      .view_toggle
      .iter()
      .any(|(word, _)| word == "3D"),
    "and reports where it drew it"
  );
}

#[test]
fn the_view_toggle_is_drawn_once_at_the_canvases_top_right() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());

  let words: Vec<&str> = harness
    .output
    .drawn
    .view_toggle
    .iter()
    .map(|(word, _)| word.as_str())
    .collect();
  assert_eq!(words, ["2D", "3D"], "the toggle offers the two views");

  let (x, y, width, height) = harness
    .output
    .drawn
    .view_toggle_rect
    .expect("the toggle reported where it was drawn");
  // The canvas is the whole drawing area: the 2D half when the split is open (which is what `canvas` reports), and
  // the 3D half beside it. The toggle sits at the right end of that area, which in this mode is the 3D half's.
  let (two_d, three_d) = harness.halves();
  let (left, top) = (two_d.min.x, two_d.min.y);
  let right = three_d.max.x;
  assert!(
    x >= left + (right - left) / 2.0,
    "the toggle sits in the canvas' right half: {x} in {left}..{right}"
  );
  assert!(
    x + width <= right,
    "and inside the canvas: {} against {right}",
    x + width
  );
  // The top right of the canvas is the strip the ruler's band occupies (S5), which is the canvas' own top
  // edge and not a corner of the drawing area.
  let thickness = cam_ui::theme::Theme::Dark.metrics().ruler_thickness;
  assert!(
    y + height <= top + 1.0 && y >= top - thickness - 1.0,
    "the toggle sits on the canvas' top edge, not down in the drawing: y {y}..{} against {top}",
    y + height
  );

  // One toggle and no second copy: the bar used to carry this control, and two of one control is the
  // defect this rule exists to prevent.
  let painted = |word: &str| harness.texts.iter().filter(|text| *text == word).count();
  assert_eq!(
    painted("2D"),
    1,
    "exactly one `2D`, painted: {:?}",
    harness.texts
  );
  assert_eq!(
    painted("3D"),
    1,
    "exactly one `3D`, painted: {:?}",
    harness.texts
  );
}

#[test]
fn pressing_3d_splits_the_canvas_and_pressing_2d_puts_it_back() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());
  assert!(
    harness.output.drawn.split.is_some(),
    "a shell holding a preview opens on the split"
  );

  let two_d = hit(&harness.output.drawn.view_toggle, "2D");
  harness.click(two_d);
  assert!(!harness.shell.view_3d(), "`2D` turns the mode off");
  assert!(
    harness.output.drawn.split.is_none(),
    "with the mode off there is one canvas, not two halves"
  );
  assert_eq!(
    harness.output.drawn.painted_triangles, 0,
    "and no 3D surface is painted"
  );

  let three_d = hit(&harness.output.drawn.view_toggle, "3D");
  harness.click(three_d);
  assert!(harness.shell.view_3d(), "`3D` turns the mode back on");
  assert!(
    harness.output.drawn.split.is_some(),
    "and the canvas splits again"
  );
}

#[test]
fn the_split_puts_the_drawing_on_the_left_and_the_three_d_view_on_the_right() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());

  let (two_d, three_d) = harness.halves();
  let canvas = harness.output.canvas;
  assert_eq!(
    rect_of((
      canvas.x as f32,
      canvas.y as f32,
      canvas.width as f32,
      canvas.height as f32
    )),
    two_d,
    "the drawing area is the 2D half, which is the rectangle the 2D camera is told about"
  );
  assert!(
    (three_d.min.x - two_d.max.x).abs() <= 1.0,
    "the 3D half begins where the drawing half ends: {} then {}",
    two_d.max.x,
    three_d.min.x
  );
  assert!(
    (three_d.width() - two_d.width()).abs() <= 1.0
      && (three_d.height() - two_d.height()).abs() <= 1.0,
    "the two halves are the same size: {two_d:?} and {three_d:?}"
  );
  // The 3D half inherits the ruler's inset (L20), so both halves' content starts on one line rather than
  // the 3D view reaching up into the band the 2D half gave away.
  assert!(
    (three_d.min.y - two_d.min.y).abs() <= 1.0,
    "the 3D half starts on the drawing's own top line: {} against {}",
    three_d.min.y,
    two_d.min.y
  );
}

#[test]
fn the_drawing_half_keeps_its_pick_and_its_picture_while_the_split_is_open() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());
  assert!(
    harness.output.drawn.painted_items > 0,
    "the drawing is painted in its own half, not hidden behind the 3D view"
  );

  // A point on the fixture rectangle's bottom edge, in the drawing half's own coordinates.
  let edge = window_point(&harness, 50.0, 10.0);
  harness.click((edge.x, edge.y));
  assert!(
    harness.app.selection().contains("E1"),
    "a click in the drawing half still picks: {:?}",
    harness.app.selection().ids().collect::<Vec<_>>()
  );

  // And a drawing tool still draws there: the split takes half the width, not the gesture.
  let before: usize = harness
    .app
    .document()
    .layers
    .iter()
    .map(|layer| layer.entities.len())
    .sum();
  harness.shell.active_tool = cam_ui::Tool::Line;
  harness.drag(
    window_point(&harness, 20.0, 20.0),
    window_point(&harness, 60.0, 60.0),
  );
  let after: usize = harness
    .app
    .document()
    .layers
    .iter()
    .map(|layer| layer.entities.len())
    .sum();
  assert_eq!(
    after,
    before + 1,
    "a drag in the drawing half with a drawing tool armed is still one command"
  );
}

#[test]
fn each_half_keeps_its_own_gesture_while_the_split_is_open() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());
  let (two_d, three_d) = harness.halves();
  let before = *harness.preview().camera();
  let zoom = harness.app.camera().pixels_per_mm();

  // A drag in the drawing half is a marquee: it moves neither the 3D eye nor the 2D zoom.
  let from = two_d.center();
  harness.drag(from, egui::pos2(from.x + 60.0, from.y + 40.0));
  let after_two_d = *harness.preview().camera();
  assert!(
    (after_two_d.yaw - before.yaw).abs() < 1.0e-9
      && (after_two_d.pitch - before.pitch).abs() < 1.0e-9,
    "the drawing half's drag must not orbit the 3D view: {} -> {}",
    before.yaw,
    after_two_d.yaw
  );

  // A wheel over the drawing half zooms the drawing, exactly as it does without the split.
  harness.wheel(two_d.center(), 120.0);
  assert!(
    harness.app.camera().pixels_per_mm() > zoom,
    "the drawing half still zooms about the pointer"
  );

  // A drag in the 3D half orbits it, and a wheel there zooms the 3D eye instead of the drawing.
  let from = three_d.center();
  let drawing_zoom = harness.app.camera().pixels_per_mm();
  let distance = harness.preview().camera().distance.mm();
  harness.wheel(from, 120.0);
  assert!(
    (harness.app.camera().pixels_per_mm() - drawing_zoom).abs() < 1.0e-9,
    "a wheel over the 3D half is not the drawing's zoom"
  );
  assert!(
    harness.preview().camera().distance.mm() < distance,
    "it is the 3D camera's own zoom: {distance} -> {}",
    harness.preview().camera().distance.mm()
  );
  harness.drag(from, egui::pos2(from.x + 80.0, from.y + 40.0));
  let after_three_d = *harness.preview().camera();
  assert!(
    (after_three_d.yaw - before.yaw).abs() > 1.0e-6,
    "the 3D half's drag orbits the eye: {} -> {}",
    before.yaw,
    after_three_d.yaw
  );
}

#[test]
fn the_home_button_returns_the_picture_to_the_state_it_opened_in() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());
  let framed = *harness.preview().home_camera();

  let (_, three_d) = harness.halves();
  harness.wheel(three_d.center(), 120.0);
  let from = three_d.center();
  harness.drag(from, egui::pos2(from.x + 80.0, from.y + 40.0));
  let away = *harness.preview().camera();
  assert!(
    (away.yaw - framed.yaw).abs() > 1.0e-6,
    "the fixture must have turned the eye before the button is asked to undo it"
  );

  let button = harness
    .output
    .drawn
    .home_button
    .expect("the 3D half must report where its home button was drawn");
  harness.click(button);
  let back = *harness.preview().camera();
  assert_eq!(back.yaw, framed.yaw, "the home button restores the angle");
  assert_eq!(back.pitch, framed.pitch);
  assert_eq!(back.target, framed.target);
  assert_eq!(
    back.distance, framed.distance,
    "and the distance, which is the whole framed state"
  );
}

#[test]
fn the_axis_triad_labels_three_axes_and_turns_with_the_camera() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());

  let words: Vec<&str> = harness
    .output
    .drawn
    .triad
    .iter()
    .map(|(word, _)| word.as_str())
    .collect();
  assert_eq!(words, ["TOP", "FRONT", "RIGHT"], "S8's three labels");

  let before = harness.output.drawn.triad.clone();
  let (_, three_d) = harness.halves();
  let from = three_d.center();
  harness.drag(from, egui::pos2(from.x + 90.0, from.y + 30.0));
  let after = harness.output.drawn.triad.clone();
  assert_ne!(
    before, after,
    "the triad is drawn from the camera, so orbiting must turn it: {before:?} then {after:?}"
  );
}

#[test]
fn the_rulers_frame_the_drawing_half_and_stop_where_the_three_d_half_begins() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());

  let rulers = &harness.output.drawn.rulers;
  let (_, three_d) = harness.halves();
  let band_end = rulers.top_band.0 + rulers.top_band.2;
  assert!(
    (band_end - three_d.min.x).abs() <= 1.0,
    "the top band ends where the 3D half begins: {band_end} against {}",
    three_d.min.x
  );
  assert!(
    rulers.top_band.3 > 0.0 && !rulers.top.is_empty(),
    "the band still numbers the drawing half: {:?}",
    rulers.top_band
  );
}
