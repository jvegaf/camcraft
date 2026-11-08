use super::support::*;
use cam_app::AppState;
use cam_geom::{Contour, Point2};
use cam_model::{Document, Job, Layer};
use cam_ui::theme::Role;
use cam_ui::{Shell, ShellOutput, Tool, UiAction};

/// The sheet with nothing drawn on it: the state S5's reference frame shows.
fn empty_document() -> Document {
  Document::new(Job::default_sheet())
}

/// Move the pointer to one window point and report the frame it was read in.
///
/// Two passes, for the reason [`click_at`] runs two: egui decides what is under the pointer from the state
/// the previous pass ended with.
fn move_to(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
  at: egui::Pos2,
) -> ShellOutput {
  let mut last = None;
  for _ in 0..2 {
    last = Some(frame_with_events(ctx, shell, app, vec![egui::Event::PointerMoved(at)]).0);
  }
  last.expect("two passes ran")
}

/// Press at `from`, move to `to`, release: one complete drag gesture, and the frame that saw it end.
fn drag_between(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
  from: egui::Pos2,
  to: egui::Pos2,
) -> ShellOutput {
  let modifiers = egui::Modifiers::default();
  let mut last = None;
  for events in [
    vec![
      egui::Event::PointerMoved(from),
      egui::Event::PointerButton {
        pos: from,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers,
      },
    ],
    vec![egui::Event::PointerMoved(to)],
    vec![egui::Event::PointerButton {
      pos: to,
      button: egui::PointerButton::Primary,
      pressed: false,
      modifiers,
    }],
  ] {
    last = Some(frame_with_events(ctx, shell, app, events).0);
  }
  last.expect("a drag is three frames")
}

/// The number a tick's own text parses back to, so a test reads the label and not only the report.
fn tick_number(tick: &cam_ui::RulerTick) -> f64 {
  tick
    .text
    .parse()
    .unwrap_or_else(|_| panic!("{:?} is not a number in millimetres", tick.text))
}

#[test]
fn the_rulers_lie_along_the_top_and_the_left_of_the_drawing_area() {
  // S5: two rulers, one along the top edge and one along the left edge, drawn in the shell's own tokens.
  let mut app = AppState::new(empty_document());
  let mut shell = Shell::new();
  let (output, _) = frame(&mut shell, &mut app);
  let rulers = &output.drawn.rulers;
  let palette = cam_ui::theme::Theme::Dark.palette();

  let (top_x, top_y, top_width, top_height) = rulers.top_band;
  let (left_x, left_y, left_width, left_height) = rulers.left_band;
  assert!(
    top_height > 0.0 && left_width > 0.0,
    "both bands have to exist: top {rulers:?}"
  );
  assert_eq!(
    f64::from(top_y + top_height),
    output.canvas.y,
    "the top band must end exactly where the drawing area begins"
  );
  assert_eq!(
    f64::from(left_x + left_width),
    output.canvas.x,
    "the left band must end exactly where the drawing area begins"
  );
  assert_eq!(
    f64::from(top_x),
    f64::from(left_x),
    "both bands start at the frame's left edge"
  );
  assert_eq!(
    f64::from(left_y),
    output.canvas.y,
    "the left band starts below the top one"
  );
  assert_eq!(
    f64::from(left_height),
    output.canvas.height,
    "the left band runs the whole height of the drawing area"
  );
  assert!(
    f64::from(top_width) >= output.canvas.width,
    "the top band spans at least the drawing area's width, got {top_width} for {}",
    output.canvas.width
  );

  // Tokens, not literals: the band is the panel step of the ladder and its ink is the dim text every
  // secondary number in the frame is written in.
  assert_eq!(rulers.band, palette.get(Role::Panel));
  assert_eq!(rulers.ink, palette.get(Role::TextDim));

  assert!(
    rulers.top.len() > 3 && rulers.left.len() > 3,
    "a ruler with no numbers on it is a border: {:?}",
    rulers
  );
  for tick in &rulers.top {
    assert!(
      tick.at.1 >= top_y && tick.at.1 <= top_y + top_height,
      "{} is drawn outside the top band",
      tick.text
    );
  }
  for tick in &rulers.left {
    assert!(
      tick.at.0 >= left_x && tick.at.0 <= left_x + left_width,
      "{} is drawn outside the left band",
      tick.text
    );
  }
}

#[test]
fn the_ruler_counts_in_steps_of_the_grids_nice_spacing() {
  // S5 does not invent a second round-number rule: the step is the grid's own `nice_spacing` at the same
  // target, so a ruler tick lands on a grid line.
  let mut app = AppState::new(empty_document());
  let mut shell = Shell::new();
  let (output, _) = frame(&mut shell, &mut app);
  let rulers = &output.drawn.rulers;
  let pixels_per_mm = app.camera().pixels_per_mm();
  let expected = cam_view::nice_spacing(pixels_per_mm, cam_view::TARGET_GRID_PIXELS);

  assert_eq!(
    rulers.spacing, expected,
    "the ruler must step by the grid's nice spacing"
  );
  assert!(
    rulers.top.windows(2).count() > 2,
    "a step that wide leaves too few numbers to read: {:?}",
    rulers.top
  );
  for pair in rulers.top.windows(2) {
    assert!(
      (pair[1].value - pair[0].value - expected).abs() < 1.0e-9,
      "{} and {} are not one step apart",
      pair[0].text,
      pair[1].text
    );
    let pixels = f64::from(pair[1].at.0 - pair[0].at.0);
    assert!(
      (pixels - expected * pixels_per_mm).abs() < 1.0,
      "one step has to be one step on screen: {pixels} px for {expected} mm at {pixels_per_mm} px/mm"
    );
  }
  // The same step down the left band, where the pixels run the other way.
  for pair in rulers.left.windows(2) {
    assert!(
      (pair[0].value - pair[1].value - expected).abs() < 1.0e-9,
      "{} and {} are not one step apart down the left ruler",
      pair[0].text,
      pair[1].text
    );
  }
}

#[test]
fn the_ruler_numbers_are_the_world_coordinates_under_them() {
  // S5: numbers in millimetres read off the world coordinates — not a sequence that starts at zero. This is
  // the assertion that makes the difference: every tick is asked for the coordinate the camera puts under
  // its own pixel, on both bands.
  let mut app = AppState::new(empty_document());
  let mut shell = Shell::new();
  let (output, _) = frame(&mut shell, &mut app);
  let rulers = &output.drawn.rulers;
  let canvas = output.canvas;
  let camera = *app.camera();
  let job = app.document().job.clone();
  let tolerance = 1.0 / camera.pixels_per_mm();

  // The empty document frames the stock, so the middle of the drawing area is the middle of the stock.
  let middle = camera.screen_to_world(canvas.to_screen(egui::pos2(
    (canvas.x + canvas.width / 2.0) as f32,
    (canvas.y + canvas.height / 2.0) as f32,
  )));
  assert!(
    (middle.x - job.width.mm() / 2.0).abs() < 0.5 && (middle.y - job.height.mm() / 2.0).abs() < 0.5,
    "the empty sheet must centre on the stock, got {middle:?} for a {} x {} sheet",
    job.width.mm(),
    job.height.mm()
  );

  for tick in &rulers.top {
    let under = camera.screen_to_world(canvas.to_screen(egui::pos2(tick.at.0, canvas.y as f32)));
    assert!(
      (under.x - tick.value).abs() <= tolerance,
      "the top ruler says {} over world x {:.4}",
      tick.text,
      under.x
    );
    assert_eq!(
      tick_number(tick),
      tick.value,
      "{} is not its value",
      tick.text
    );
  }
  for tick in &rulers.left {
    let under = camera.screen_to_world(canvas.to_screen(egui::pos2(canvas.x as f32, tick.at.1)));
    assert!(
      (under.y - tick.value).abs() <= tolerance,
      "the left ruler says {} over world y {:.4}",
      tick.text,
      under.y
    );
    assert_eq!(
      tick_number(tick),
      tick.value,
      "{} is not its value",
      tick.text
    );
  }

  // Left of the origin the top ruler already reads negative here, because the stock is narrower than the
  // frame: the sheet's left half is world x < 0.
  let origin = camera.world_to_screen(Point2::new(0.0, 0.0));
  let origin_x = (canvas.x + origin.x) as f32;
  let negative: Vec<&cam_ui::RulerTick> =
    rulers.top.iter().filter(|tick| tick.value < 0.0).collect();
  assert!(
    !negative.is_empty(),
    "the frame shows world x < 0 to the left of the origin, so the ruler must say so: {:?}",
    rulers.top
  );
  for tick in negative {
    assert!(
      tick.at.0 < origin_x,
      "{} has to be left of the origin at {origin_x}",
      tick.text
    );
  }

  // Below the origin reads negative too. This stock is taller than the frame is at this zoom, so the visible
  // band under the origin is shorter than one step and no *tick* lands in it — the reading does. The
  // invariant is the gap, not which number happens to be last: the space between the lowest printed number and
  // the bottom edge is less than one step, which a renderer that dropped a tick that fits could not satisfy.
  let bottom = camera.screen_to_world(canvas.to_screen(egui::pos2(
    (canvas.x + 10.0) as f32,
    (canvas.y + canvas.height) as f32,
  )));
  assert!(
    bottom.y < 0.0,
    "the bottom of the drawing area is below the origin, got {bottom:?}"
  );
  let last = rulers
    .left
    .last()
    .expect("the left band prints at least one number");
  assert!(
    last.value - bottom.y <= rulers.spacing + 1e-9,
    "the band below the last printed number ({}) is shorter than one {:.3} step down to {:.3}: {:?}",
    last.value,
    rulers.spacing,
    bottom.y,
    rulers.left
  );
}

/// A drawing centred on the origin: the left ruler has to cross zero and put the negative numbers below it.
#[test]
fn a_drawing_centred_on_the_origin_reads_negative_below_it() {
  let mut document = Document::new(Job::default_sheet());
  document.layers.push(Layer {
    id: "L1".to_owned(),
    name: "Centred".to_owned(),
    visible: true,
    entities: vec![cam_model::Entity {
      id: "E1".to_owned(),
      kind: cam_model::EntityKind::Contour {
        contour: Contour::from_points(
          &[
            Point2::new(-100.0, -100.0),
            Point2::new(100.0, -100.0),
            Point2::new(100.0, 100.0),
            Point2::new(-100.0, 100.0),
          ],
          true,
        )
        .expect("rectangle"),
      },
    }],
  });
  let mut app = AppState::new(document);
  let mut shell = Shell::new();
  let (output, _) = frame(&mut shell, &mut app);
  let rulers = &output.drawn.rulers;
  let canvas = output.canvas;
  let camera = *app.camera();

  let origin = camera.world_to_screen(Point2::new(0.0, 0.0));
  let origin_y = (canvas.y + origin.y) as f32;
  assert!(
    rulers
      .left
      .iter()
      .any(|tick| tick.value < 0.0 && tick.at.1 > origin_y),
    "below the origin the left ruler must read negative: {:?}",
    rulers.left
  );
  assert!(
    rulers
      .left
      .iter()
      .any(|tick| tick.value > 0.0 && tick.at.1 < origin_y),
    "above the origin the left ruler must read positive: {:?}",
    rulers.left
  );
  assert!(
    rulers.top.iter().any(|tick| tick.value < 0.0)
      && rulers.top.iter().any(|tick| tick.value > 0.0),
    "the top ruler crosses zero too: {:?}",
    rulers.top
  );
}

#[test]
fn the_top_ruler_names_the_unit_it_counts_in() {
  // S5: the top one names the current unit.
  let mut app = AppState::new(empty_document());
  let mut shell = Shell::new();
  let (output, texts) = frame(&mut shell, &mut app);
  let rulers = &output.drawn.rulers;

  assert_eq!(rulers.unit, "mm");
  assert_eq!(rulers.unit, cam_ui::ruler::UNIT);
  let (_, top_y, _, top_height) = rulers.top_band;
  assert!(
    rulers.unit_at.1 >= top_y && rulers.unit_at.1 <= top_y + top_height,
    "the unit word belongs on the top band, at {:?} in {:?}",
    rulers.unit_at,
    rulers.top_band
  );
  assert!(
    texts.iter().filter(|text| text.as_str() == "mm").count() >= 2,
    "the ruler paints its own \u{201c}mm\u{201d} beside the status bar's: {texts:?}"
  );
}

#[test]
fn the_rulers_mark_the_cursor_on_both_edges() {
  // S5: an amber marker on both edges tracking the cursor.
  let mut app = AppState::new(empty_document());
  let mut shell = Shell::new();
  // Snapping off, so the marker is the pointer's own position and not a snap point that happens to lie
  // nearby: what this test is about is that the marker follows the cursor at all.
  shell.snapping = false;
  let ctx = egui::Context::default();
  let (first, _) = frame_with(&ctx, &mut shell, &mut app);
  let canvas = first.canvas;
  let at = egui::pos2(
    (canvas.x + canvas.width * 0.42) as f32,
    (canvas.y + canvas.height * 0.58) as f32,
  );
  let output = move_to(&ctx, &mut shell, &mut app, at);
  let rulers = &output.drawn.rulers;
  let marker = rulers
    .marker
    .clone()
    .unwrap_or_else(|| panic!("the pointer is over the drawing area, so both bands mark it"));
  let world = shell
    .cursor_world
    .expect("the pointer is over the drawing area");

  assert_eq!(
    marker.world,
    (world.x, world.y),
    "the marker tracks the cursor's world position, not a pixel"
  );
  let palette = cam_ui::theme::Theme::Dark.palette();
  assert_eq!(
    marker.colour,
    palette.get(Role::Cut),
    "the marker is the cut signal, the amber role that means where the tool bites"
  );
  assert_ne!(
    marker.colour,
    palette.get(Role::Sel),
    "the marker is not the pointer-owns-it cyan"
  );

  // Both legs are in their own band, and each sits at the cursor's own coordinate.
  let (_, top_y, _, top_height) = rulers.top_band;
  let (left_x, _, left_width, _) = rulers.left_band;
  let ((ax, ay), (bx, by)) = marker.top;
  assert!(
    ay >= top_y && ay <= top_y + top_height && by >= top_y && by <= top_y + top_height,
    "the top leg has to be inside the top band: {:?}",
    marker.top
  );
  assert!(
    (ax - bx).abs() < 0.01,
    "the top leg is vertical: {ax} vs {bx}"
  );
  let ((cx, cy), (dx, dy)) = marker.left;
  assert!(
    cx >= left_x && cx <= left_x + left_width && dx >= left_x && dx <= left_x + left_width,
    "the left leg has to be inside the left band: {:?}",
    marker.left
  );
  assert!((cy - dy).abs() < 0.01, "the left leg is horizontal");
  let screen = app.camera().world_to_screen(world);
  assert!(
    (f64::from(ax) - (canvas.x + screen.x)).abs() < 1.0,
    "the top leg is {} px from where the cursor's world x projects",
    f64::from(ax) - (canvas.x + screen.x)
  );
  assert!(
    (f64::from(cy) - (canvas.y + screen.y)).abs() < 1.0,
    "the left leg is {} px from where the cursor's world y projects",
    f64::from(cy) - (canvas.y + screen.y)
  );
}

#[test]
fn the_marker_is_absent_while_the_pointer_is_on_a_ruler() {
  // S5: the marker is drawn only while the pointer is over the drawing area. A band is not one.
  let mut app = AppState::new(empty_document());
  let mut shell = Shell::new();
  shell.snapping = false;
  let ctx = egui::Context::default();
  let (first, _) = frame_with(&ctx, &mut shell, &mut app);
  let canvas = first.canvas;
  let band = f64::from(first.drawn.rulers.top_band.3);

  let on_ruler = move_to(
    &ctx,
    &mut shell,
    &mut app,
    egui::pos2((canvas.x + 120.0) as f32, (canvas.y - band / 2.0) as f32),
  );
  assert!(
    on_ruler.drawn.rulers.marker.is_none(),
    "the pointer is on the ruler, not on the sheet: {:?}",
    on_ruler.drawn.rulers.marker
  );
  assert!(
    shell.cursor_world.is_none(),
    "a point on the ruler is not a point on the sheet either"
  );

  // One pixel below the band it is over the sheet again, and the marker is back.
  let on_sheet = move_to(
    &ctx,
    &mut shell,
    &mut app,
    egui::pos2((canvas.x + 120.0) as f32, (canvas.y + 1.0) as f32),
  );
  assert!(
    on_sheet.drawn.rulers.marker.is_some(),
    "the drawing area begins at the band's edge"
  );
}

#[test]
fn the_wheel_zooms_over_the_sheet_but_not_over_a_ruler() {
  // S5's inset makes the bands chrome, and that has one consequence worth pinning instead of leaving it
  // unnamed (L20): the wheel is gated on the drawing area, so scrolling with the pointer over a ruler does not
  // zoom the sheet — the same as scrolling over the rail or the dock. Pinned here so the day it changes, it
  // changes on purpose.
  let ctx = egui::Context::default();
  let mut app = AppState::new(empty_document());
  let mut shell = Shell::new();
  let (first, _) = frame_with(&ctx, &mut shell, &mut app);
  let canvas = first.canvas;
  let band = f64::from(first.drawn.rulers.top_band.3);
  assert!(
    band > 0.0,
    "the frame has to have drawn a top band: {:?}",
    first.drawn.rulers.top_band
  );

  let on_band = egui::pos2((canvas.x + 120.0) as f32, (canvas.y - band / 2.0) as f32);
  let _ = move_to(&ctx, &mut shell, &mut app, on_band);
  let before = app.camera().pixels_per_mm();
  let _ = frame_with_events(
    &ctx,
    &mut shell,
    &mut app,
    vec![
      egui::Event::PointerMoved(on_band),
      egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, 40.0),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
        modifiers: egui::Modifiers::default(),
      },
    ],
  );
  assert_eq!(
    app.camera().pixels_per_mm(),
    before,
    "the wheel over the top ruler must not zoom the sheet"
  );

  let on_sheet = egui::pos2((canvas.x + 120.0) as f32, (canvas.y + 40.0) as f32);
  let _ = move_to(&ctx, &mut shell, &mut app, on_sheet);
  let before = app.camera().pixels_per_mm();
  let _ = frame_with_events(
    &ctx,
    &mut shell,
    &mut app,
    vec![egui::Event::MouseWheel {
      unit: egui::MouseWheelUnit::Point,
      delta: egui::vec2(0.0, 40.0),
      phase: egui::TouchPhase::Move,
      source: egui::MouseWheelSource::Wheel,
      modifiers: egui::Modifiers::default(),
    }],
  );
  assert!(
    app.camera().pixels_per_mm() > before,
    "the same wheel on the sheet still zooms ({before} -> {})",
    app.camera().pixels_per_mm()
  );
}

#[test]
fn a_gesture_on_a_ruler_is_not_a_gesture_on_the_sheet() {
  // S5: the drawing area is inset by the ruler bands, so a click on a ruler is not a click on the sheet.
  // The same drag is run twice, once in the band and once just inside the sheet: the first must reach
  // nothing and the second must still draw.
  let mut app = AppState::new(empty_document());
  let mut shell = Shell::new();
  shell.active_tool = Tool::Rectangle;
  let ctx = egui::Context::default();
  let (first, _) = frame_with(&ctx, &mut shell, &mut app);
  let canvas = first.canvas;
  let (top_x, top_y, top_width, top_height) = first.drawn.rulers.top_band;
  let (left_x, left_y, left_width, left_height) = first.drawn.rulers.left_band;
  // There has to be a band to press on, and it has to be outside the drawing area: without both of these
  // the two assertions below would hold for a canvas with no ruler at all.
  assert!(
    top_height > 0.0 && left_width > 0.0,
    "there is no band to press on: {:?}",
    first.drawn.rulers
  );
  assert_eq!(f64::from(top_y + top_height), canvas.y);
  assert_eq!(f64::from(left_x + left_width), canvas.x);
  let on_top_band_at = egui::pos2(top_x + top_width / 2.0, top_y + top_height / 2.0);
  let on_left_band_at = egui::pos2(left_x + left_width / 2.0, left_y + left_height / 2.0);
  assert!(f64::from(on_top_band_at.y) < canvas.y);
  assert!(f64::from(on_left_band_at.x) < canvas.x);

  let on_top_band = drag_between(
    &ctx,
    &mut shell,
    &mut app,
    on_top_band_at,
    egui::pos2(on_top_band_at.x + 100.0, on_top_band_at.y),
  );
  assert!(
    on_top_band.actions.is_empty(),
    "a drag along the top ruler must not draw: {:?}",
    on_top_band.actions
  );
  let on_left_band = drag_between(
    &ctx,
    &mut shell,
    &mut app,
    on_left_band_at,
    egui::pos2(on_left_band_at.x, on_left_band_at.y + 100.0),
  );
  assert!(
    on_left_band.actions.is_empty(),
    "a drag along the left ruler must not draw: {:?}",
    on_left_band.actions
  );
  assert!(
    app
      .document()
      .layers
      .iter()
      .all(|layer| layer.entities.is_empty()),
    "nothing may have reached the document"
  );

  let on_sheet = drag_between(
    &ctx,
    &mut shell,
    &mut app,
    egui::pos2((canvas.x + 120.0) as f32, (canvas.y + 11.0) as f32),
    egui::pos2((canvas.x + 220.0) as f32, (canvas.y + 111.0) as f32),
  );
  assert!(
    matches!(on_sheet.actions.as_slice(), [UiAction::Draw(_)]),
    "the same drag on the sheet still draws: {:?}",
    on_sheet.actions
  );
}

#[test]
fn the_drawing_is_framed_inside_the_inset_area_the_bands_leave() {
  // The rulers take room from the canvas and fitting the document has to follow them. This is the assertion
  // that can actually fail: the frame is drawn with the fit's own padding, so a fit measured against the
  // *un-inset* central area would leave `padding - band` on the two band sides — while a plainer "the corners
  // are inside the drawing rect" check would still pass, because that padding is wider than the band.
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let (output, _) = frame(&mut shell, &mut app);
  let canvas = output.canvas;
  let band = output.drawn.rulers.top_band;
  assert!(
    band.3 > 0.0,
    "the frame has to have drawn a top band for this to mean anything: {band:?}"
  );
  assert!(
    (canvas.y as f32 - (band.1 + band.3)).abs() < 0.5,
    "the drawing area has to start exactly where the top band ends: canvas.y {} against band {band:?}",
    canvas.y
  );
  let camera = *app.camera();
  let corners: Vec<egui::Pos2> = [(0.0, 0.0), (100.0, 0.0), (100.0, 60.0), (0.0, 60.0)]
    .iter()
    .map(|(x, y)| canvas.to_window(camera.world_to_screen(Point2::new(*x, *y))))
    .collect();
  let min_x = corners
    .iter()
    .map(|corner| corner.x)
    .fold(f32::MAX, f32::min);
  let max_x = corners
    .iter()
    .map(|corner| corner.x)
    .fold(f32::MIN, f32::max);
  let min_y = corners
    .iter()
    .map(|corner| corner.y)
    .fold(f32::MAX, f32::min);
  let max_y = corners
    .iter()
    .map(|corner| corner.y)
    .fold(f32::MIN, f32::max);
  let right = (canvas.x + canvas.width) as f32;
  let bottom = (canvas.y + canvas.height) as f32;
  for (edge, gap) in [
    ("left", min_x - canvas.x as f32),
    ("top", min_y - canvas.y as f32),
    ("right", right - max_x),
    ("bottom", bottom - max_y),
  ] {
    assert!(
      gap >= 20.0,
      "the {edge} padding is {gap:.1} px: the fit was measured against something wider than the drawing area"
    );
  }
}
