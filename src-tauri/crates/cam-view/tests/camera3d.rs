//! Behaviour tests for the 3D camera (M5 A9): what it projects, how it orbits and how it zooms.
//!
//! The projection is pure math (`cam-view`, S2: "Testeable headless, determinista"), so every pin
//! here is a coordinate comparison against a viewport whose centre is known.

use cam_app::Viewport;
use cam_move::Point3;
use cam_view::{Camera3d, DEFAULT_PITCH, MAX_DISTANCE, MIN_DISTANCE, PITCH_LIMIT};

/// The viewport every test projects into: centre at (400, 300).
const VIEWPORT: Viewport = Viewport::new(800.0, 600.0);

fn camera(target: Point3) -> Camera3d {
  Camera3d::new(target, VIEWPORT)
}

fn target() -> Point3 {
  Point3::new(123.0, -45.0, -9.0)
}

/// The eye's offset from the target, as the camera's own spherical definition puts it.
///
/// Recomputed here from the public fields rather than read from the camera, so a test that depends
/// on where the eye is does not depend on the code under test to say so.
fn eye_offset(camera: &Camera3d) -> Point3 {
  let (sin_yaw, cos_yaw) = camera.yaw.sin_cos();
  let (sin_pitch, cos_pitch) = camera.pitch.sin_cos();
  let distance = camera.distance.mm();
  Point3::new(
    distance * cos_yaw * cos_pitch,
    distance * sin_yaw * cos_pitch,
    distance * sin_pitch,
  )
}

/// `point` rotated about the vertical axis through `target` by `angle` radians.
fn rotated_about_z(point: Point3, target: Point3, angle: f64) -> Point3 {
  let (sin, cos) = angle.sin_cos();
  let dx = point.x.mm() - target.x.mm();
  let dy = point.y.mm() - target.y.mm();
  Point3::new(
    target.x.mm() + dx * cos - dy * sin,
    target.y.mm() + dx * sin + dy * cos,
    point.z.mm(),
  )
}

/// A point offset from `origin` by `scale` times `offset`.
fn scaled_offset(origin: Point3, offset: Point3, scale: f64) -> Point3 {
  Point3::new(
    origin.x.mm() + offset.x.mm() * scale,
    origin.y.mm() + offset.y.mm() * scale,
    origin.z.mm() + offset.z.mm() * scale,
  )
}

#[test]
fn the_target_projects_to_the_centre_of_the_viewport() {
  for pitch in [0.0, DEFAULT_PITCH, PITCH_LIMIT, -PITCH_LIMIT] {
    for yaw in [0.0, 0.7, -2.5] {
      let mut camera = camera(target());
      camera.pitch = pitch;
      camera.yaw = yaw;
      let screen = camera.project(target()).expect("the target is in front");

      assert!(
        (screen.x - 400.0).abs() < 1.0e-6,
        "yaw {yaw}, pitch {pitch}: x was {}",
        screen.x
      );
      assert!(
        (screen.y - 300.0).abs() < 1.0e-6,
        "yaw {yaw}, pitch {pitch}: y was {}",
        screen.y
      );
      // In front of the camera by exactly the orbit distance: the target is the plane the whole
      // projection is measured from.
      assert!((screen.depth - camera.distance.mm()).abs() < 1.0e-9);
    }
  }
}

#[test]
fn yaw_rotates_a_point_about_the_target_by_that_angle() {
  let centre = target();
  let point = Point3::new(
    centre.x.mm() + 25.0,
    centre.y.mm() - 10.0,
    centre.z.mm() + 4.0,
  );

  for pitch in [0.0, DEFAULT_PITCH, -0.9] {
    for delta in [0.25, -0.25, 1.0, -2.0] {
      let mut before = camera(centre);
      before.pitch = pitch;
      before.yaw = 0.4;

      let mut after = before;
      after.orbit(delta, 0.0);

      let expected = before.project(point).expect("point in front");
      // Adding `delta` to the yaw must be the same view as rotating the scene about the vertical
      // axis through the target by `delta`: the projection has no other freedom.
      let rotated = after
        .project(rotated_about_z(point, centre, delta))
        .expect("rotated point in front");

      assert!(
        (expected.x - rotated.x).abs() < 1.0e-9,
        "pitch {pitch}, delta {delta}: x {} vs {}",
        expected.x,
        rotated.x
      );
      assert!(
        (expected.y - rotated.y).abs() < 1.0e-9,
        "pitch {pitch}, delta {delta}: y {} vs {}",
        expected.y,
        rotated.y
      );
      assert!(
        (expected.depth - rotated.depth).abs() < 1.0e-9,
        "pitch {pitch}, delta {delta}: depth {} vs {}",
        expected.depth,
        rotated.depth
      );
    }
  }
}

#[test]
fn yaw_carries_a_point_across_the_centre_a_quarter_turn_apart() {
  // At pitch 0 the camera sits on +X looking towards -X, so screen-right is world +Y: a point
  // straight along +Y from the target starts right of the centre, crosses it at a quarter turn and
  // is left of it beyond. That is the sign convention, stated without borrowing the focal length.
  let centre = target();
  let point = Point3::new(centre.x.mm(), centre.y.mm() + 30.0, centre.z.mm());
  let mut camera = camera(centre);
  camera.pitch = 0.0;
  camera.yaw = 0.0;

  let offset_at = |camera: &Camera3d| camera.project(point).expect("in front").x - 400.0;
  assert!(offset_at(&camera) > 0.0, "the point starts right of centre");

  camera.yaw = 1.0;
  assert!(offset_at(&camera) > 0.0);
  camera.yaw = std::f64::consts::FRAC_PI_2;
  assert!(
    offset_at(&camera).abs() < 1.0e-6,
    "a quarter turn puts the point on the axis"
  );
  camera.yaw = 2.0;
  assert!(
    offset_at(&camera) < 0.0,
    "past the axis it is left of centre"
  );
}

#[test]
fn points_behind_the_camera_or_not_finite_are_not_projected() {
  let centre = target();
  let camera = camera(centre);
  let eye = eye_offset(&camera);

  // Just past the eye, and far beyond it on the same ray: both are behind the viewer, and a
  // nonsense screen coordinate would put them in front of the user's eyes.
  for scale in [1.01, 2.0, 10.0] {
    let behind = scaled_offset(centre, eye, scale);
    assert!(
      camera.project(behind).is_none(),
      "scale {scale} is behind the camera"
    );
  }

  // In front, as the control: half way to the eye, at the target itself, and past the target on the
  // far side are all visible.
  for scale in [0.5, 0.0, -1.5] {
    let front = scaled_offset(centre, eye, scale);
    assert!(
      camera.project(front).is_some(),
      "scale {scale} is in front of the camera"
    );
  }

  assert!(camera.project(Point3::new(f64::NAN, 0.0, 0.0)).is_none());
  assert!(camera
    .project(Point3::new(0.0, f64::INFINITY, 0.0))
    .is_none());
}

#[test]
fn pitch_is_clamped_just_short_of_vertical_at_both_ends() {
  let mut camera = camera(target());
  camera.pitch = 0.0;
  camera.yaw = 0.0;

  camera.orbit(0.0, 10.0);
  assert_eq!(camera.pitch, PITCH_LIMIT);
  assert!(camera.pitch < std::f64::consts::FRAC_PI_2);
  assert_eq!(camera.yaw, 0.0, "orbit must not move an untouched axis");
  assert!(camera.project(target()).is_some());

  camera.orbit(0.0, -20.0);
  assert_eq!(camera.pitch, -PITCH_LIMIT);

  // A field poked past the limit from outside is treated as the limit rather than producing a
  // degenerate basis: the projection has no up vector to invent at the pole.
  let mut poked = camera;
  poked.pitch = std::f64::consts::FRAC_PI_2;
  let screen = poked.project(target()).expect("the pole still projects");
  assert!((screen.x - 400.0).abs() < 1.0e-6);
}

#[test]
fn zoom_clamps_at_a_near_and_a_far_end() {
  let mut camera = camera(target());
  let start = camera.distance.mm();
  assert_eq!(start, cam_view::DEFAULT_DISTANCE.mm());

  // A factor above one moves the camera closer, so the stock grows on screen.
  camera.zoom(2.0);
  assert_eq!(camera.distance.mm(), start / 2.0);

  for _ in 0..400 {
    camera.zoom(2.0);
  }
  assert_eq!(camera.distance, MIN_DISTANCE);

  for _ in 0..400 {
    camera.zoom(0.5);
  }
  assert_eq!(camera.distance, MAX_DISTANCE);

  // A nonsense factor is ignored instead of poisoning the distance; `Length` carries no NaN guard.
  for factor in [0.0, -1.0, f64::NAN, f64::INFINITY] {
    let before = camera.distance;
    camera.zoom(factor);
    assert_eq!(camera.distance, before, "factor {factor} must be ignored");
  }
}
