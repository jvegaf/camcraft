//! Behaviour tests for the backend-agnostic scene and the grid (A10).

use cam_app::{AppState, Command, ScreenPoint, Viewport};
use cam_geom::{Bounds, Point2};
use cam_model::{Document, Job, Layer};
use cam_view::{
  build_grid, build_scene, nice_spacing, SceneItem, Style, MAX_GRID_LINES, TARGET_GRID_PIXELS,
};
use proptest::prelude::*;

fn document_with(entities: Vec<(&str, cam_geom::Contour)>, visible: bool) -> Document {
  let mut document = Document::new(Job::default_sheet());
  document.layers.push(Layer {
    id: "L1".to_owned(),
    name: "Part".to_owned(),
    visible,
    role: cam_model::LayerRole::Geometry,
    entities: entities
      .into_iter()
      .map(|(id, contour)| cam_model::Entity {
        id: id.to_owned(),
        kind: cam_model::EntityKind::Contour { contour },
      })
      .collect(),
  });
  document
}

fn rectangle(x: f64, y: f64, width: f64, height: f64) -> cam_geom::Contour {
  cam_geom::Contour::from_points(
    &[
      Point2::new(x, y),
      Point2::new(x + width, y),
      Point2::new(x + width, y + height),
      Point2::new(x, y + height),
    ],
    true,
  )
  .expect("rectangle")
}

fn circle(center: Point2, radius: f64) -> cam_geom::Contour {
  cam_geom::Contour::new(
    vec![
      cam_geom::Vertex::new(Point2::new(center.x + radius, center.y), 1.0),
      cam_geom::Vertex::new(Point2::new(center.x - radius, center.y), 1.0),
    ],
    true,
  )
  .expect("circle")
}

#[test]
fn a_rectangle_becomes_one_item_per_edge() {
  let document = document_with(vec![("E1", rectangle(0.0, 0.0, 40.0, 20.0))], true);
  let scene = build_scene(&document, &cam_app::Selection::default());
  assert_eq!(scene.len(), 4);
  for item in &scene.items {
    assert!(matches!(item, SceneItem::Polyline { .. }));
    assert_eq!(item.entity(), Some("E1"));
    assert_eq!(item.style(), Style::Normal);
  }
}

#[test]
fn a_circle_stays_two_arcs() {
  let document = document_with(vec![("C1", circle(Point2::new(10.0, 10.0), 5.0))], true);
  let scene = build_scene(&document, &cam_app::Selection::default());
  assert_eq!(scene.len(), 2);
  for item in &scene.items {
    match item {
      SceneItem::Arc { radius, .. } => assert!((radius - 5.0).abs() < 1.0e-12),
      other => panic!("a circle must stay arcs, got {other:?}"),
    }
  }
}

#[test]
fn selected_entities_are_styled_and_the_rest_are_not() {
  let document = document_with(
    vec![
      ("E1", rectangle(0.0, 0.0, 10.0, 10.0)),
      ("E2", rectangle(50.0, 0.0, 10.0, 10.0)),
    ],
    true,
  );
  let mut app = AppState::new(document);
  app.select(vec!["E2".to_owned()]);
  let scene = build_scene(app.document(), app.selection());

  let selected = scene
    .items
    .iter()
    .filter(|item| item.style() == Style::Selected)
    .count();
  assert_eq!(selected, 4, "the four edges of E2 are selected");
  let normal = scene
    .items
    .iter()
    .filter(|item| item.style() == Style::Normal)
    .count();
  assert_eq!(normal, 4);
}

#[test]
fn a_hidden_layer_is_absent_from_the_scene_and_from_picking() {
  let document = document_with(vec![("E1", rectangle(0.0, 0.0, 40.0, 20.0))], false);
  let scene = build_scene(&document, &cam_app::Selection::default());
  assert!(scene.is_empty(), "a hidden layer must not be drawn");
}

#[test]
fn scene_bounds_match_the_geometry() {
  let document = document_with(
    vec![
      ("E1", rectangle(0.0, 0.0, 40.0, 20.0)),
      ("C1", circle(Point2::new(100.0, 100.0), 10.0)),
    ],
    true,
  );
  let scene = build_scene(&document, &cam_app::Selection::default());
  let bounds = scene.bounds();
  assert!((bounds.min.x).abs() < 1.0e-9);
  assert!((bounds.min.y).abs() < 1.0e-9);
  assert!((bounds.max.x - 110.0).abs() < 1.0e-9, "{}", bounds.max.x);
  assert!((bounds.max.y - 110.0).abs() < 1.0e-9, "{}", bounds.max.y);
}

#[test]
fn drawn_geometry_through_a_command_appears_in_the_scene() {
  let mut document = Document::new(Job::default_sheet());
  document.layers.push(Layer {
    id: "L1".to_owned(),
    name: "Part".to_owned(),
    visible: true,
    role: cam_model::LayerRole::Geometry,
    entities: Vec::new(),
  });
  let mut app = AppState::new(document);
  let id = app.next_entity_id();
  app
    .execute(Command::AddRectangle {
      layer_id: "L1".to_owned(),
      entity_id: id.clone(),
      at: Point2::new(5.0, 5.0),
      width: 30.0,
      height: 15.0,
    })
    .expect("added");
  let scene = build_scene(app.document(), app.selection());
  assert_eq!(scene.len(), 4);
  assert!(scene
    .items
    .iter()
    .all(|item| item.entity() == Some(id.as_str())));
}

#[test]
fn grid_spacing_is_a_round_number_and_lands_near_the_target_size() {
  for pixels_per_mm in [0.05, 0.3, 1.0, 2.5, 7.7, 40.0, 150.0] {
    let spacing = nice_spacing(pixels_per_mm, TARGET_GRID_PIXELS);
    let on_screen = spacing * pixels_per_mm;
    assert!(
      (TARGET_GRID_PIXELS - 1.0e-9..TARGET_GRID_PIXELS * 10.0).contains(&on_screen),
      "zoom {pixels_per_mm}: spacing {spacing} mm is {on_screen} px"
    );
    let normalised = spacing / 10f64.powf(spacing.log10().floor());
    assert!(
      [1.0, 2.0, 5.0, 10.0]
        .iter()
        .any(|value| (normalised - value).abs() < 1.0e-9),
      "spacing {spacing} is not a 1/2/5 step"
    );
  }
}

#[test]
fn the_grid_covers_the_visible_area() {
  let mut app = AppState::new(Document::new(Job::default_sheet()));
  app.camera_mut().set_viewport(Viewport::new(1000.0, 700.0));
  app.camera_mut().zoom_to_fit(
    Bounds::from_points(&[Point2::new(0.0, 0.0), Point2::new(200.0, 100.0)]),
    20.0,
  );

  let camera = *app.camera();
  let grid = build_grid(&camera, TARGET_GRID_PIXELS);
  assert!(!grid.is_empty());
  assert!(grid.len() <= MAX_GRID_LINES * 2);

  let visible_top_left = camera.screen_to_world(ScreenPoint::new(0.0, 0.0));
  let visible_bottom_right = camera.screen_to_world(ScreenPoint::new(1000.0, 700.0));
  let min_x = visible_top_left.x.min(visible_bottom_right.x);
  let max_x = visible_top_left.x.max(visible_bottom_right.x);

  let vertical_lines = grid
    .items
    .iter()
    .filter(|item| match item {
      SceneItem::Polyline { points, .. } => (points[0].x - points[1].x).abs() < 1.0e-9,
      _ => false,
    })
    .count();
  assert!(vertical_lines > 0, "the grid must have vertical lines");
  for item in &grid.items {
    let bounds = item.bounds();
    assert!(
      bounds.min.x >= min_x - 1.0e-6,
      "{:?} left of the viewport",
      bounds
    );
    assert!(
      bounds.max.x <= max_x + 1.0e-6,
      "{:?} right of the viewport",
      bounds
    );
  }
}

#[test]
fn an_empty_viewport_produces_no_grid() {
  // set_viewport deliberately ignores a zero-size window so a minimised app keeps its view; build
  // the degenerate camera directly to exercise the grid's own guard.
  let camera = cam_app::Camera::new(Point2::ORIGIN, 1.0, Viewport::new(0.0, 0.0));
  assert!(build_grid(&camera, TARGET_GRID_PIXELS).is_empty());
}

proptest! {
  /// Grid spacing never grows when the view zooms in.
  #[test]
  fn zooming_in_never_coarsens_the_grid(a in 0.05f64..50.0, factor in 1.01f64..8.0) {
    let fine = nice_spacing(a, TARGET_GRID_PIXELS);
    let coarser = nice_spacing(a * factor, TARGET_GRID_PIXELS);
    prop_assert!(coarser <= fine + 1.0e-12, "{} -> {}", fine, coarser);
  }
}
