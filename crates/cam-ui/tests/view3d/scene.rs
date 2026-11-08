use super::support::*;
use cam_ui::theme::{Palette, Role, Theme};
use cam_units::Length;

#[test]
fn an_open_panel_paints_triangles_and_the_stock_box() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());

  assert!(
    harness.output.drawn.painted_triangles > 0,
    "the open panel must paint the simulated surface"
  );
  // Twelve stock-box edges, the document's four vector edges, the program's four projected edges — the first
  // move opens a polyline instead of being joined to a start position the stream does not carry — and the tool
  // marker's axis and diameter. Each layer is counted on its own because the overlay row hides them one at a
  // time (S8), and a single total could not say which one went missing.
  let drawn = &harness.output.drawn;
  assert_eq!(drawn.painted_material_lines, 12, "twelve box edges");
  assert_eq!(drawn.painted_vector_lines, 4, "the fixture's rectangle");
  assert_eq!(drawn.painted_program_lines, 4, "four projected edges");
  assert_eq!(drawn.painted_tool_lines, 2, "the tool's axis and diameter");
  assert_eq!(drawn.painted_3d_lines, 22, "and the total is their sum");
}

#[test]
fn a_closed_panel_paints_no_3d_at_all() {
  let mut harness = Harness::new();
  harness.shell.show_3d = false;
  harness.frame(Vec::new());

  assert_eq!(harness.output.drawn.painted_triangles, 0);
  assert_eq!(harness.output.drawn.painted_3d_lines, 0);
  assert!(
    harness.mesh_vertices.iter().all(|&vertices| vertices < 100),
    "no surface reached the painter, only egui's own decorations: {:?}",
    harness.mesh_vertices
  );
}

#[test]
fn the_surface_reaches_the_painter_as_an_egui_mesh() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());

  assert!(harness.output.drawn.painted_triangles > 0);
  let surface_vertices = 3 * harness.output.drawn.painted_triangles;
  assert_eq!(
    harness
      .mesh_vertices
      .iter()
      .filter(|&&vertices| vertices == surface_vertices)
      .count(),
    1,
    "the surface is one `egui::Mesh` of {surface_vertices} vertices, with no new dependency \
     behind it (A11); the frame's meshes were {:?}",
    harness.mesh_vertices
  );
}

#[test]
fn a_drag_orbits_the_camera() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());
  let before = *harness
    .shell
    .preview()
    .expect("the fixture loaded a preview")
    .camera();

  let from = harness.in_panel();
  let to = egui::pos2(from.x + 120.0, from.y + 60.0);
  let modifiers = egui::Modifiers::default();
  harness.frame(vec![
    egui::Event::PointerMoved(from),
    egui::Event::PointerButton {
      pos: from,
      button: egui::PointerButton::Primary,
      pressed: true,
      modifiers,
    },
  ]);
  harness.frame(vec![egui::Event::PointerMoved(to)]);
  harness.frame(vec![egui::Event::PointerButton {
    pos: to,
    button: egui::PointerButton::Primary,
    pressed: false,
    modifiers,
  }]);

  let after = *harness
    .shell
    .preview()
    .expect("the fixture loaded a preview")
    .camera();
  assert!(
    (after.yaw - before.yaw).abs() > 1.0e-6,
    "a horizontal drag must turn the camera: {} -> {}",
    before.yaw,
    after.yaw
  );
  assert!(
    (after.pitch - before.pitch).abs() > 1.0e-6,
    "a vertical drag must raise or lower the camera: {} -> {}",
    before.pitch,
    after.pitch
  );
  assert_eq!(
    after.target, before.target,
    "orbiting turns the eye, it does not move the target"
  );
}

#[test]
fn the_wheel_zooms_about_the_target() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());
  let before = *harness
    .shell
    .preview()
    .expect("the fixture loaded a preview")
    .camera();

  harness.frame(vec![
    egui::Event::PointerMoved(harness.in_panel()),
    egui::Event::MouseWheel {
      unit: egui::MouseWheelUnit::Point,
      delta: egui::vec2(0.0, 120.0),
      phase: egui::TouchPhase::Move,
      source: egui::MouseWheelSource::Wheel,
      modifiers: egui::Modifiers::default(),
    },
  ]);

  let after = *harness
    .shell
    .preview()
    .expect("the fixture loaded a preview")
    .camera();
  assert!(
    after.distance.mm() < before.distance.mm(),
    "scrolling up must move the eye closer: {} -> {}",
    before.distance.mm(),
    after.distance.mm()
  );
  assert_eq!(
    after.target, before.target,
    "the wheel zooms about the target, it does not shift it"
  );
}

#[test]
fn a_camera_the_projection_rejects_paints_nothing() {
  let mut harness = Harness::new();
  harness
    .shell
    .preview_mut()
    .expect("the fixture loaded a preview")
    .camera_mut()
    .distance = Length::ZERO;
  harness.frame(Vec::new());

  assert_eq!(
    harness.output.drawn.painted_triangles, 0,
    "a camera with no usable projection paints no surface"
  );
  assert_eq!(
    harness.output.drawn.painted_3d_lines, 0,
    "and no lines either"
  );
}

#[test]
fn the_split_paints_both_halves_and_closing_it_paints_one() {
  // T6 replaces the exclusive mode M5 shipped: S8's 3D is a *split*, so the drawing is painted beside the 3D
  // half rather than instead of it. This test is the M5 test rewritten to the rule that replaced it.
  let mut harness = Harness::new();
  harness.frame(Vec::new());

  assert!(
    harness.output.drawn.painted_triangles > 0,
    "the 3D half paints the simulated surface"
  );
  assert!(
    harness.output.drawn.painted_items > 0,
    "and the drawing half paints the document beside it"
  );
  assert!(
    harness.output.drawn.painted_grid_lines > 0,
    "including its grid"
  );

  // Back to the drawing alone: the triangle count drops to zero and the scene keeps its own half's width.
  let (two_d, _) = harness.halves();
  harness.shell.set_view_3d(false);
  harness.frame(Vec::new());
  assert_eq!(
    harness.output.drawn.painted_triangles, 0,
    "a closed split paints no mesh"
  );
  assert_eq!(
    harness.output.drawn.painted_items, 4,
    "the four rectangle edges are still there (A12)"
  );
  assert!(
    harness.output.drawn.painted_grid_lines > 0,
    "and so is the grid"
  );
  assert!(
    harness.output.canvas.width as f32 > two_d.width(),
    "with the split closed the drawing gets the whole canvas back: {} against {}",
    harness.output.canvas.width,
    two_d.width()
  );
}

#[test]
fn a_closed_panel_leaves_the_2d_frame_exactly_as_it_was() {
  // Two shells, one holding a preview and one holding nothing, both past their framing frames: with
  // the panel closed they must paint the same 2D picture, which is what A12 asks of the addition.
  let mut with_preview = Harness::new();
  with_preview.shell.show_3d = false;
  let mut without_preview = Harness::bare();

  with_preview.frame(Vec::new());
  without_preview.frame(Vec::new());

  assert_eq!(
    with_preview.output.drawn.painted_items,
    without_preview.output.drawn.painted_items
  );
  assert_eq!(
    with_preview.output.drawn.painted_grid_lines,
    without_preview.output.drawn.painted_grid_lines
  );
  assert_eq!(with_preview.output.canvas, without_preview.output.canvas);
  assert_eq!(with_preview.output.drawn.painted_triangles, 0);
  assert_eq!(with_preview.output.drawn.painted_3d_lines, 0);
}

// ---------------------------------------------------------------------------------------------
// T9 — the 3D view's colours are the design's, not its own (L7 left this open; L34 authorises it).
// ---------------------------------------------------------------------------------------------

/// Every colour the 3D panel painted in the last frame.
///
/// The panel is found by its surface mesh — the one mesh whose vertex count is three per painted
/// triangle — and every shape clipped to that same rectangle is the panel's own output: the bars,
/// the palette, the dock and the canvas each paint inside their own clip rectangle, so this is the
/// frame's 3D view and nothing else.
fn colours_in_the_3d_panel(harness: &Harness) -> Vec<egui::Color32> {
  let surface_vertices = 3 * harness.output.drawn.painted_triangles;
  assert!(surface_vertices > 0, "the frame painted no surface");
  let panel = harness
    .shapes
    .iter()
    .find(|clipped| {
      matches!(&clipped.shape, egui::Shape::Mesh(mesh) if mesh.vertices.len() == surface_vertices)
    })
    .map(|clipped| clipped.clip_rect)
    .expect("the surface reached the painter as a mesh");
  let mut colours = Vec::new();
  for clipped in harness
    .shapes
    .iter()
    .filter(|clipped| clipped.clip_rect == panel)
  {
    collect_colours(&clipped.shape, &mut colours);
  }
  colours
}

/// The colours one shape paints, appended to `colours`.
fn collect_colours(shape: &egui::Shape, colours: &mut Vec<egui::Color32>) {
  match shape {
    egui::Shape::Vec(shapes) => {
      for shape in shapes {
        collect_colours(shape, colours);
      }
    }
    egui::Shape::Mesh(mesh) => colours.extend(mesh.vertices.iter().map(|vertex| vertex.color)),
    egui::Shape::LineSegment { stroke, .. } => colours.push(stroke.color),
    egui::Shape::Path(path) => {
      if let egui::epaint::ColorMode::Solid(colour) = path.stroke.color {
        colours.push(colour);
      }
      colours.push(path.fill);
    }
    egui::Shape::Rect(rect) => {
      colours.push(rect.fill);
      colours.push(rect.stroke.color);
    }
    egui::Shape::Circle(circle) => {
      colours.push(circle.fill);
      colours.push(circle.stroke.color);
    }
    egui::Shape::Text(text) => colours.push(text.fallback_color),
    _ => {}
  }
}

/// Whether `colour` sits on the closed segment between the ramp's two ends.
///
/// The ramp is a mix, so a painted vertex can be any stop on it, not just a named role. Each channel
/// pins `t` to an interval — a channel's value is the mix rounded, so `t` may be half a step either
/// side — and the colour is on the ramp when all three intervals share a point. This is the honest
/// form of "reachable from the theme": it accepts exactly the colours [`Palette::stock_shade`] can
/// produce and nothing else.
fn on_the_stock_ramp(palette: &Palette, colour: egui::Color32) -> bool {
  let (top, bottom) = (palette.get(Role::StockTop), palette.get(Role::StockBottom));
  let mut low = 0.0_f32;
  let mut high = 1.0_f32;
  for (top, bottom, value) in [
    (top.r(), bottom.r(), colour.r()),
    (top.g(), bottom.g(), colour.g()),
    (top.b(), bottom.b(), colour.b()),
  ] {
    let (top, bottom, value) = (f32::from(top), f32::from(bottom), f32::from(value));
    let delta = bottom - top;
    if delta == 0.0 {
      if (value - top).abs() > 0.5 {
        return false;
      }
      continue;
    }
    let edge = |offset: f32| (value + offset - top) / delta;
    let (first, second) = (edge(-0.5), edge(0.5));
    low = low.max(first.min(second));
    high = high.min(first.max(second));
  }
  low <= high + 1.0e-4
}

/// The colours the audit collected, with the invisible ones dropped and the rest deduplicated so a
/// failure names each offender once instead of once per vertex.
fn distinct_visible(colours: Vec<egui::Color32>) -> Vec<egui::Color32> {
  let mut distinct: Vec<egui::Color32> = Vec::new();
  for colour in colours {
    // An alpha-zero fill paints nothing, so it is not a colour the view chooses.
    if colour.a() == 0 || distinct.contains(&colour) {
      continue;
    }
    distinct.push(colour);
  }
  distinct
}

#[test]
fn the_3d_panel_paints_only_colours_the_theme_owns() {
  // The claim the slice makes: the 3D view no longer owns a single colour. Everything it paints is
  // either a role of the set that is showing or a stop on the stock's ramp between two of them, and
  // the audit reads what actually reached the painter rather than what the code meant to paint.
  for theme in [Theme::Dark, Theme::Light] {
    let mut harness = Harness::new();
    harness.shell.set_theme(theme);
    harness.frame(Vec::new());
    let painted = distinct_visible(colours_in_the_3d_panel(&harness));
    assert!(
      painted.len() >= 4,
      "{theme:?}: the audit saw only {painted:?}, so it did not read the panel"
    );
    let palette = theme.palette();
    let roles: Vec<egui::Color32> = Role::ALL.iter().map(|role| palette.get(*role)).collect();
    let stray: Vec<egui::Color32> = painted
      .iter()
      .copied()
      .filter(|colour| !roles.contains(colour) && !on_the_stock_ramp(&palette, *colour))
      .collect();
    assert!(
      stray.is_empty(),
      "{theme:?}: the 3D panel painted {stray:?}, which no `Role` owns — the view is still \
       shading with literals of its own"
    );
  }
}

#[test]
fn a_cut_stock_is_still_shaded_by_depth_after_the_ramp_became_tokens() {
  // The one thing this slice could quietly destroy: the surface used to interpolate between two
  // literals, and now interpolates between two roles. A field that has been cut must still show more
  // than one shade, every shade must be on the ramp, and the shades must still be ordered by depth —
  // darker where the tool went deeper.
  for theme in [Theme::Dark, Theme::Light] {
    let mut harness = Harness::bare();
    harness
      .shell
      .set_preview(whole_program_field(), program(), &job());
    harness.shell.set_theme(theme);
    harness.frame(Vec::new());
    let palette = theme.palette();
    let painted = distinct_visible(colours_in_the_3d_panel(&harness));
    let shades: Vec<egui::Color32> = painted
      .into_iter()
      .filter(|colour| on_the_stock_ramp(&palette, *colour))
      .collect();
    assert!(
      shades.len() >= 2,
      "{theme:?}: the cut surface is painted in one shade ({shades:?}), so the depth ramp is gone"
    );
    let deepest = shades
      .iter()
      .map(|colour| cam_ui::theme::luminance(*colour))
      .fold(1.0_f32, f32::min);
    assert!(
      deepest < cam_ui::theme::luminance(palette.get(Role::StockTop)),
      "{theme:?}: every shade is the untouched top, so nothing reads as cut"
    );
    assert!(
      deepest > cam_ui::theme::luminance(palette.get(Role::StockBottom)),
      "{theme:?}: the deepest shade is off the bottom of the ramp"
    );
  }
}
