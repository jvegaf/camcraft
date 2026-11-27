//! Behaviour tests for the viewport camera (A4).

use cam_app::{Camera, ScreenPoint, Viewport, MAX_PIXELS_PER_MM, MIN_PIXELS_PER_MM};
use cam_geom::{Bounds, Point2};
use proptest::prelude::*;

fn camera() -> Camera {
  Camera::new(Point2::new(100.0, 50.0), 4.0, Viewport::new(1000.0, 700.0))
}

fn job_bounds() -> Bounds {
  Bounds::from_points(&[Point2::new(0.0, 0.0), Point2::new(1219.0, 2438.0)])
}

#[test]
fn world_and_screen_round_trip() {
  let camera = camera();
  for world in [
    Point2::new(0.0, 0.0),
    Point2::new(100.0, 50.0),
    Point2::new(-37.5, 812.25),
    Point2::new(1219.0, 2438.0),
  ] {
    let back = camera.screen_to_world(camera.world_to_screen(world));
    assert!(
      back.is_near(world, cam_geom::TolerancePolicy::uniform(1.0e-9)),
      "{:?} came back as {:?}",
      world,
      back
    );
  }
}

#[test]
fn the_centre_of_the_viewport_is_the_camera_centre() {
  let camera = camera();
  let centre = camera.viewport().center();
  assert!(camera
    .screen_to_world(centre)
    .is_near(camera.center(), cam_geom::TolerancePolicy::uniform(1.0e-9)));
}

#[test]
fn world_y_grows_upwards_on_screen() {
  let camera = camera();
  let low = camera.world_to_screen(Point2::new(100.0, 0.0));
  let high = camera.world_to_screen(Point2::new(100.0, 100.0));
  assert!(
    high.y < low.y,
    "higher world Y must map to a smaller screen Y"
  );
}

#[test]
fn panning_moves_the_content_with_the_cursor() {
  let mut camera = camera();
  let before = camera.world_to_screen(Point2::new(100.0, 50.0));
  camera.pan_by_pixels(40.0, -25.0);
  let after = camera.world_to_screen(Point2::new(100.0, 50.0));
  assert!((after.x - (before.x + 40.0)).abs() < 1.0e-9);
  assert!((after.y - (before.y - 25.0)).abs() < 1.0e-9);
}

#[test]
fn panning_by_pixels_is_independent_of_zoom() {
  for zoom in [0.5, 1.0, 7.5, 60.0] {
    let mut camera = Camera::new(Point2::ORIGIN, zoom, Viewport::new(800.0, 600.0));
    camera.pan_by_pixels(100.0, 0.0);
    // 100 pixels of drag is 100 / zoom millimetres of world movement, in the opposite direction.
    assert!(
      (camera.center().x + 100.0 / zoom).abs() < 1.0e-9,
      "zoom {zoom}: centre {}",
      camera.center().x
    );
  }
}

#[test]
fn zooming_keeps_the_world_point_under_the_cursor() {
  let mut camera = camera();
  let anchor = ScreenPoint::new(740.0, 130.0);
  let before = camera.screen_to_world(anchor);
  camera.zoom_at(anchor, 1.7);
  let after = camera.screen_to_world(anchor);
  assert!(
    after.is_near(before, cam_geom::TolerancePolicy::uniform(1.0e-6)),
    "the anchor drifted from {before:?} to {after:?}"
  );
  assert!(camera.pixels_per_mm() > 4.0, "zoom must actually increase");
}

#[test]
fn zoom_is_clamped_at_both_limits() {
  let mut camera = camera();
  for _ in 0..200 {
    camera.zoom_at(ScreenPoint::new(10.0, 10.0), 2.0);
  }
  assert_eq!(camera.pixels_per_mm(), MAX_PIXELS_PER_MM);
  for _ in 0..400 {
    camera.zoom_at(ScreenPoint::new(10.0, 10.0), 0.5);
  }
  assert_eq!(camera.pixels_per_mm(), MIN_PIXELS_PER_MM);
}

#[test]
fn a_nonsense_zoom_is_replaced_instead_of_poisoning_the_camera() {
  let camera = Camera::new(Point2::ORIGIN, f64::NAN, Viewport::new(800.0, 600.0));
  assert_eq!(camera.pixels_per_mm(), 1.0);
  let mut camera = camera;
  camera.zoom_at(ScreenPoint::new(1.0, 1.0), f64::NAN);
  assert_eq!(camera.pixels_per_mm(), 1.0);
  camera.zoom_at(ScreenPoint::new(1.0, 1.0), -2.0);
  assert_eq!(camera.pixels_per_mm(), 1.0);
}

#[test]
fn zoom_to_fit_frames_the_job_inside_the_viewport_with_the_margin() {
  let mut camera = Camera::default();
  camera.set_viewport(Viewport::new(1000.0, 700.0));
  camera.zoom_to_fit(job_bounds(), 20.0);

  let bounds = job_bounds();
  let bottom_left = camera.world_to_screen(bounds.min);
  let top_right = camera.world_to_screen(bounds.max);
  let viewport = camera.viewport();

  assert!(
    bottom_left.x >= 20.0 - 1.0e-6,
    "clipped on the left: {}",
    bottom_left.x
  );
  assert!(
    bottom_left.y <= viewport.height - 20.0 + 1.0e-6,
    "clipped at the bottom"
  );
  assert!(
    top_right.x <= viewport.width - 20.0 + 1.0e-6,
    "clipped on the right"
  );
  assert!(top_right.y >= 20.0 - 1.0e-6, "clipped at the top");
  assert!(
    bounds
      .center()
      .is_near(camera.center(), cam_geom::TolerancePolicy::uniform(1.0e-9)),
    "fit must centre the job"
  );
}

#[test]
fn fit_on_an_empty_bounds_leaves_the_camera_alone() {
  let mut camera = camera();
  let before = camera;
  camera.zoom_to_fit(Bounds::empty(), 10.0);
  assert_eq!(camera, before);
}

proptest! {
  /// The screen/world round trip holds at any legal zoom and centre.
  #[test]
  fn round_trip_holds_everywhere(
    centre_x in -5000.0f64..5000.0,
    centre_y in -5000.0f64..5000.0,
    zoom in 0.02f64..150.0,
    screen_x in 0.0f64..1000.0,
    screen_y in 0.0f64..700.0,
  ) {
    let camera = Camera::new(
      Point2::new(centre_x, centre_y),
      zoom,
      Viewport::new(1000.0, 700.0),
    );
    let screen = ScreenPoint::new(screen_x, screen_y);
    let world = camera.screen_to_world(screen);
    let back = camera.world_to_screen(world);
    prop_assert!((back.x - screen.x).abs() < 1.0e-6, "{} vs {}", back.x, screen.x);
    prop_assert!((back.y - screen.y).abs() < 1.0e-6, "{} vs {}", back.y, screen.y);
  }
}
