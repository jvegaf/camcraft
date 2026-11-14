//! Behaviour tests for picking, box selection and snapping (A5, A6).

use cam_app::{AppState, Camera, ScreenPoint, Viewport};
use cam_geom::{Contour, Point2};
use cam_model::{Document, Job, Layer};
use cam_view::{box_select, choose_snap, pick, snap_candidates, SnapKind, PICK_TOLERANCE_PX};

fn rectangle(x: f64, y: f64, width: f64, height: f64) -> Contour {
  Contour::from_points(
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

fn circle(center: Point2, radius: f64) -> Contour {
  Contour::new(
    vec![
      cam_geom::Vertex::new(Point2::new(center.x + radius, center.y), 1.0),
      cam_geom::Vertex::new(Point2::new(center.x - radius, center.y), 1.0),
    ],
    true,
  )
  .expect("circle")
}

fn document(entities: Vec<(&str, Contour)>) -> Document {
  let mut document = Document::new(Job::default_sheet());
  document.layers.push(Layer {
    id: "L1".to_owned(),
    role: cam_model::LayerRole::Outline,
    side: cam_model::Side::Top,
    color: "#000000".to_owned(),
    visible: true,
    source: cam_model::Provenance::default(),
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

/// A camera at 2 px/mm looking at the world origin, 1000x700 pixels.
fn camera() -> Camera {
  Camera::new(Point2::ORIGIN, 2.0, Viewport::new(1000.0, 700.0))
}

fn scene_of(app: &AppState) -> cam_view::Scene {
  cam_view::build_scene(app.document(), app.selection())
}

#[test]
fn a_click_within_tolerance_selects_the_edge_and_beyond_it_does_not() {
  let app = AppState::new(document(vec![("E1", rectangle(0.0, 0.0, 100.0, 50.0))]));
  let scene = scene_of(&app);
  let camera = camera();

  // Three pixels above the bottom edge, which runs along y = 0 from x = 0 to 100.
  let near = ScreenPoint::new(50.0 * 2.0 + 500.0, 350.0 + 3.0);
  let hit = pick(&scene, &camera, near, PICK_TOLERANCE_PX).expect("should hit");
  assert_eq!(hit.entity, "E1");
  assert!(hit.distance_px <= 3.0 + 1.0e-9, "{}", hit.distance_px);

  let far = ScreenPoint::new(50.0 * 2.0 + 500.0, 350.0 + 20.0);
  assert!(pick(&scene, &camera, far, PICK_TOLERANCE_PX).is_none());
}

#[test]
fn the_closer_of_two_entities_wins() {
  let app = AppState::new(document(vec![
    ("E1", rectangle(0.0, 0.0, 100.0, 10.0)),
    ("E2", rectangle(0.0, 40.0, 100.0, 10.0)),
  ]));
  let scene = scene_of(&app);
  let camera = camera();
  // Just above the top edge of E1 (y = 10), far from E2 (y >= 40).
  let screen = camera.world_to_screen(Point2::new(50.0, 12.0));
  assert_eq!(
    pick(&scene, &camera, screen, 30.0).map(|hit| hit.entity),
    Some("E1".to_owned())
  );
}

#[test]
fn coincident_edges_resolve_to_the_lowest_id() {
  let app = AppState::new(document(vec![
    ("E2", rectangle(0.0, 0.0, 100.0, 50.0)),
    ("E1", rectangle(0.0, 0.0, 100.0, 50.0)),
  ]));
  let scene = scene_of(&app);
  let camera = camera();
  let screen = camera.world_to_screen(Point2::new(50.0, 0.0));
  assert_eq!(
    pick(&scene, &camera, screen, PICK_TOLERANCE_PX).map(|hit| hit.entity),
    Some("E1".to_owned())
  );
}

#[test]
fn picking_works_on_circle_arcs() {
  let app = AppState::new(document(vec![("C1", circle(Point2::new(0.0, 0.0), 25.0))]));
  let scene = scene_of(&app);
  let camera = camera();
  let on_circle = camera.world_to_screen(Point2::new(25.0, 0.0));
  assert_eq!(
    pick(&scene, &camera, on_circle, PICK_TOLERANCE_PX).map(|hit| hit.entity),
    Some("C1".to_owned())
  );
  let at_centre = camera.world_to_screen(Point2::ORIGIN);
  assert!(
    pick(&scene, &camera, at_centre, PICK_TOLERANCE_PX).is_none(),
    "the middle of a circle is not on it"
  );
}

#[test]
fn tolerance_is_measured_in_pixels_so_zoom_does_not_change_aim() {
  let app = AppState::new(document(vec![("E1", rectangle(0.0, 0.0, 100.0, 50.0))]));
  let scene = scene_of(&app);
  for pixels_per_mm in [0.5, 2.0, 12.0] {
    let camera = Camera::new(Point2::ORIGIN, pixels_per_mm, Viewport::new(1000.0, 700.0));
    let target = camera.world_to_screen(Point2::new(50.0, 0.0));
    let offset = ScreenPoint::new(target.x, target.y + 4.0);
    assert!(
      pick(&scene, &camera, offset, PICK_TOLERANCE_PX).is_some(),
      "4 px above the edge must hit at {pixels_per_mm} px/mm"
    );
    let offset = ScreenPoint::new(target.x, target.y + 15.0);
    assert!(
      pick(&scene, &camera, offset, PICK_TOLERANCE_PX).is_none(),
      "15 px away must miss at {pixels_per_mm} px/mm"
    );
  }
}

#[test]
fn box_select_catches_whatever_the_box_touches() {
  let app = AppState::new(document(vec![
    ("INSIDE", rectangle(10.0, 10.0, 10.0, 10.0)),
    ("CROSSING", rectangle(40.0, 0.0, 40.0, 40.0)),
    ("FAR", rectangle(200.0, 200.0, 10.0, 10.0)),
  ]));
  let scene = scene_of(&app);
  let camera = camera();
  let from = camera.world_to_screen(Point2::new(5.0, 5.0));
  let to = camera.world_to_screen(Point2::new(60.0, 30.0));
  let hits = box_select(&scene, &camera, from, to);
  assert_eq!(hits, vec!["CROSSING".to_owned(), "INSIDE".to_owned()]);
}

#[test]
fn an_empty_box_selects_nothing() {
  let app = AppState::new(document(vec![("E1", rectangle(0.0, 0.0, 10.0, 10.0))]));
  let scene = scene_of(&app);
  let camera = camera();
  let from = camera.world_to_screen(Point2::new(500.0, 500.0));
  let to = camera.world_to_screen(Point2::new(505.0, 505.0));
  assert!(box_select(&scene, &camera, from, to).is_empty());
}

#[test]
fn snap_candidates_offer_endpoints_midpoints_and_centres() {
  let app = AppState::new(document(vec![
    ("E1", rectangle(0.0, 0.0, 40.0, 20.0)),
    ("C1", circle(Point2::new(100.0, 0.0), 10.0)),
  ]));
  let scene = scene_of(&app);
  let camera = camera();
  let kinds = [
    SnapKind::Endpoint,
    SnapKind::Midpoint,
    SnapKind::Center,
    SnapKind::Grid,
  ];

  // Near the rectangle's lower-left corner.
  let near_corner = camera.world_to_screen(Point2::new(0.4, 0.3));
  let snap = choose_snap(&scene, &camera, near_corner, 10.0, &kinds, 0.0).expect("a snap");
  assert_eq!(snap.kind, SnapKind::Endpoint);
  assert!(snap.point.is_near(Point2::ORIGIN, 1.0e-9));

  // Near the middle of its bottom edge.
  let near_middle = camera.world_to_screen(Point2::new(20.3, 0.2));
  let snap = choose_snap(&scene, &camera, near_middle, 10.0, &kinds, 0.0).expect("a snap");
  assert_eq!(snap.kind, SnapKind::Midpoint);
  assert!(snap.point.is_near(Point2::new(20.0, 0.0), 1.0e-9));

  // Near the centre of the circle.
  let near_centre = camera.world_to_screen(Point2::new(100.2, 0.2));
  let snap = choose_snap(&scene, &camera, near_centre, 10.0, &kinds, 0.0).expect("a snap");
  assert_eq!(snap.kind, SnapKind::Center);
  assert!(snap.point.is_near(Point2::new(100.0, 0.0), 1.0e-9));
}

#[test]
fn disabled_snap_kinds_are_not_offered() {
  let app = AppState::new(document(vec![("E1", rectangle(0.0, 0.0, 40.0, 20.0))]));
  let scene = scene_of(&app);
  let camera = camera();
  let near_corner = camera.world_to_screen(Point2::new(0.3, 0.3));
  assert!(
    choose_snap(
      &scene,
      &camera,
      near_corner,
      10.0,
      &[SnapKind::Midpoint],
      0.0
    )
    .is_none(),
    "endpoints are disabled, so nothing should snap here"
  );
  assert!(choose_snap(
    &scene,
    &camera,
    near_corner,
    10.0,
    &[SnapKind::Endpoint],
    0.0
  )
  .is_some());
}

#[test]
fn the_grid_offers_the_nearest_intersection() {
  let app = AppState::new(document(vec![("E1", rectangle(0.0, 0.0, 40.0, 20.0))]));
  let scene = scene_of(&app);
  let camera = camera();
  let near_grid_point = camera.world_to_screen(Point2::new(10.4, 10.2));
  let snap = choose_snap(
    &scene,
    &camera,
    near_grid_point,
    10.0,
    &[SnapKind::Grid],
    5.0,
  )
  .expect("a grid snap");
  assert_eq!(snap.kind, SnapKind::Grid);
  assert!(snap.point.is_near(Point2::new(10.0, 10.0), 1.0e-9));
}

#[test]
fn geometry_is_preferred_over_the_grid_at_equal_distance() {
  let app = AppState::new(document(vec![("E1", rectangle(0.0, 0.0, 40.0, 20.0))]));
  let scene = scene_of(&app);
  let camera = camera();
  // Cursor exactly between the rectangle's corner (0,0) and a grid point at (2,0): both are one
  // millimetre away, so the tie-break decides, and geometry must win.
  let between = camera.world_to_screen(Point2::new(1.0, 0.0));
  let candidates = snap_candidates(
    &scene,
    &camera,
    between,
    10.0,
    &[SnapKind::Grid, SnapKind::Endpoint],
    2.0,
  );
  let kinds: Vec<SnapKind> = candidates.iter().map(|snap| snap.kind).collect();
  assert!(
    kinds.contains(&SnapKind::Endpoint) && kinds.contains(&SnapKind::Grid),
    "both kinds should be candidates: {candidates:?}"
  );
  assert_eq!(kinds[0], SnapKind::Endpoint, "geometry must win the tie");
}

#[test]
fn duplicate_snap_points_are_offered_once() {
  let app = AppState::new(document(vec![("E1", rectangle(0.0, 0.0, 40.0, 20.0))]));
  let scene = scene_of(&app);
  let camera = camera();
  // The corner coincides with a grid intersection, so the user must be offered one point, not two.
  let at_corner = camera.world_to_screen(Point2::ORIGIN);
  let candidates = snap_candidates(
    &scene,
    &camera,
    at_corner,
    10.0,
    &[SnapKind::Grid, SnapKind::Endpoint],
    5.0,
  );
  assert_eq!(candidates.len(), 1, "{candidates:?}");
  assert_eq!(candidates[0].kind, SnapKind::Endpoint);
}

#[test]
fn snapping_is_measured_in_pixels() {
  let app = AppState::new(document(vec![("E1", rectangle(0.0, 0.0, 40.0, 20.0))]));
  let scene = scene_of(&app);
  for pixels_per_mm in [0.5, 3.0, 20.0] {
    let camera = Camera::new(Point2::ORIGIN, pixels_per_mm, Viewport::new(1000.0, 700.0));
    let corner = camera.world_to_screen(Point2::ORIGIN);
    let near = ScreenPoint::new(corner.x + 5.0, corner.y - 5.0);
    let snap = choose_snap(&scene, &camera, near, 10.0, &[SnapKind::Endpoint], 0.0);
    assert!(
      snap.is_some(),
      "5 px away must snap at {pixels_per_mm} px/mm"
    );
  }
}
