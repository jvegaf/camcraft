//! Behaviour tests for the contour transforms (M2 A4, A5).
//!
//! The point of these tests is which properties survive: arcs stay arcs when the transform is exact,
//! winding comes back counter-clockwise, and the cases that cannot stay exact say so instead of
//! silently changing shape.

use cam_geom::transform::{self, ScaleOutcome};
use cam_geom::{primitive, Contour, GeomError, Point2, Vec2, Vertex};
use proptest::prelude::*;
use std::f64::consts::PI;

fn assert_close(actual: f64, expected: f64, tolerance: f64, what: &str) {
  assert!(
    (actual - expected).abs() <= tolerance,
    "{what}: expected {expected}, got {actual}"
  );
}

#[test]
fn translate_moves_every_vertex_and_keeps_the_arcs() {
  let circle = primitive::circle(Point2::new(3.0, 4.0), 5.0).expect("circle");
  let delta = Vec2::new(10.0, -2.0);
  let moved = transform::translate(&circle, delta).expect("valid translation");
  assert_eq!(moved.len(), 2, "a translated circle is still two arcs");
  assert!(moved.has_arcs(), "translation never tessellates");
  let expected: Vec<Vertex> = circle
    .vertices()
    .iter()
    .map(|vertex| Vertex::new(vertex.point.translated(delta), vertex.bulge))
    .collect();
  assert_eq!(moved, Contour::new(expected, true).expect("valid"));
  assert_close(moved.bounds().center().x, 13.0, 1.0e-9, "centre x");
  assert_close(moved.bounds().center().y, 2.0, 1.0e-9, "centre y");
}

#[test]
fn rotate_about_the_origin_maps_points() {
  let line = primitive::line(Point2::new(1.0, 0.0), Point2::new(2.0, 0.0)).expect("line");
  let turned = transform::rotate(&line, PI / 2.0, Point2::ORIGIN).expect("valid rotation");
  assert_close(turned.vertices()[0].point.x, 0.0, 1.0e-12, "rotated x");
  assert_close(turned.vertices()[0].point.y, 1.0, 1.0e-12, "rotated y");
  assert_close(turned.vertices()[1].point.y, 2.0, 1.0e-12, "second y");
}

#[test]
fn rotate_keeps_a_circle_a_circle() {
  let centre = Point2::new(4.0, -6.0);
  let circle = primitive::circle(centre, 7.0).expect("circle");
  let pivot = Point2::new(1.0, 1.0);
  let turned = transform::rotate(&circle, 0.7, pivot).expect("valid rotation");
  assert_eq!(turned.len(), 2, "rotation preserves both arcs");
  assert!(turned.has_arcs(), "rotation never tessellates");
  assert_close(
    turned.perimeter(),
    circle.perimeter(),
    1.0e-9,
    "circumference",
  );
  let expected_centre = centre.rotated_about(pivot, 0.7);
  assert_close(
    turned.bounds().center().x,
    expected_centre.x,
    1.0e-9,
    "centre x",
  );
  assert_close(
    turned.bounds().center().y,
    expected_centre.y,
    1.0e-9,
    "centre y",
  );
  let arc = turned.segments()[0].as_arc().expect("arc");
  assert_close(arc.radius(), 7.0, 1.0e-9, "radius");
}

#[test]
fn mirror_of_a_circle_lands_on_the_reflected_centre() {
  let circle = primitive::circle(Point2::new(3.0, 4.0), 2.0).expect("circle");
  // Mirror about the vertical line x = 10.
  let mirrored =
    transform::mirror(&circle, Point2::new(10.0, 0.0), Vec2::Y_AXIS).expect("valid axis");
  assert_eq!(mirrored.len(), 2, "a mirrored circle is still two arcs");
  assert!(mirrored.has_arcs(), "mirror never tessellates");
  assert_close(
    mirrored.bounds().center().x,
    17.0,
    1.0e-9,
    "mirrored centre x",
  );
  assert_close(
    mirrored.bounds().center().y,
    4.0,
    1.0e-9,
    "mirrored centre y",
  );
  assert_close(
    mirrored.perimeter(),
    circle.perimeter(),
    1.0e-9,
    "circumference",
  );
}

#[test]
fn mirrored_closed_contour_stays_counter_clockwise() {
  let rectangle = primitive::rectangle(Point2::new(0.0, 0.0), 10.0, 4.0).expect("rectangle");
  let mirrored =
    transform::mirror(&rectangle, Point2::new(0.0, 0.0), Vec2::X_AXIS).expect("valid axis");
  assert!(mirrored.is_closed(), "mirror keeps the contour closed");
  assert!(
    mirrored.is_ccw(),
    "winding is normalised back to counter-clockwise"
  );
  assert_close(
    mirrored.signed_area(),
    rectangle.signed_area(),
    1.0e-9,
    "area",
  );
  // A rectangle sitting above the X axis lands below it.
  assert!(mirrored.bounds().max.y <= 0.0, "mirrored below the axis");
}

#[test]
fn mirror_with_a_zero_direction_is_refused() {
  let circle = primitive::circle(Point2::ORIGIN, 1.0).expect("circle");
  let error =
    transform::mirror(&circle, Point2::ORIGIN, Vec2::ZERO).expect_err("a line needs a direction");
  assert!(
    matches!(error, GeomError::InvalidPrimitive { .. }),
    "{error:?}"
  );
}

#[test]
fn uniform_scale_doubles_area_and_keeps_arcs() {
  let circle = primitive::circle(Point2::new(1.0, 1.0), 2.0).expect("circle");
  let ScaleOutcome {
    contour,
    lost_arcs,
    approximation_error,
  } = transform::scale(
    &circle,
    2.0,
    2.0,
    Point2::ORIGIN,
    cam_geom::TolerancePolicy::uniform(0.01),
  )
  .expect("valid scale");
  assert_eq!(lost_arcs, 0, "a uniform scale keeps every arc");
  assert_eq!(approximation_error, 0.0, "a uniform scale is exact");
  assert_eq!(contour.len(), 2, "still two arcs");
  assert_close(
    contour.signed_area(),
    4.0 * circle.signed_area(),
    1.0e-9,
    "area",
  );
  assert_close(contour.bounds().center().x, 2.0, 1.0e-9, "scaled centre x");
}

#[test]
fn negative_uniform_scale_is_a_rotation_and_stays_exact() {
  let circle = primitive::circle(Point2::ORIGIN, 3.0).expect("circle");
  let outcome = transform::scale(
    &circle,
    -1.0,
    -1.0,
    Point2::ORIGIN,
    cam_geom::TolerancePolicy::uniform(0.01),
  )
  .expect("valid scale");
  assert_eq!(outcome.lost_arcs, 0, "a half turn is not an approximation");
  assert!(outcome.contour.has_arcs());
  assert_close(
    outcome.contour.perimeter(),
    circle.perimeter(),
    1.0e-9,
    "circumference",
  );
}

#[test]
fn non_uniform_scale_of_a_circle_reports_lost_arcs() {
  let tolerance = cam_geom::TolerancePolicy::uniform(0.001);
  let circle = primitive::circle(Point2::ORIGIN, 5.0).expect("circle");
  let ScaleOutcome {
    contour,
    lost_arcs,
    approximation_error,
  } = transform::scale(&circle, 2.0, 1.0, Point2::ORIGIN, tolerance).expect("valid scale");
  assert!(
    lost_arcs >= 1,
    "an arc cannot stay circular under a non-uniform scale"
  );
  assert!(
    !contour.has_arcs(),
    "the affected arcs become straight segments"
  );
  assert!(contour.is_closed(), "closedness survives");
  assert!(contour.is_ccw(), "winding survives");
  assert!(
    approximation_error <= tolerance.chord,
    "error {approximation_error} within {tolerance:?}"
  );
  assert_close(
    contour.bounds().width(),
    20.0,
    2.0 * tolerance.chord,
    "width",
  );
  assert_close(
    contour.bounds().height(),
    10.0,
    2.0 * tolerance.chord,
    "height",
  );
  assert_close(
    contour.bounds().center().x,
    0.0,
    tolerance.epsilon,
    "centre x",
  );
}

#[test]
fn non_uniform_scale_of_a_polygon_is_exact() {
  let rectangle = primitive::rectangle(Point2::ORIGIN, 4.0, 2.0).expect("rectangle");
  let outcome = transform::scale(
    &rectangle,
    2.0,
    1.0,
    Point2::ORIGIN,
    cam_geom::TolerancePolicy::uniform(0.001),
  )
  .expect("valid scale");
  assert_eq!(outcome.lost_arcs, 0, "straight segments scale exactly");
  assert_eq!(outcome.approximation_error, 0.0);
  assert_eq!(outcome.contour.len(), 4, "four corners stay four corners");
  assert_close(outcome.contour.signed_area(), 16.0, 1.0e-12, "area");
}

#[test]
fn scale_about_an_anchor_leaves_the_anchor_fixed() {
  let anchor = Point2::new(7.0, -3.0);
  let rectangle = primitive::rectangle(Point2::new(7.0, -3.0), 2.0, 2.0).expect("rectangle");
  let outcome = transform::scale(
    &rectangle,
    3.0,
    3.0,
    anchor,
    cam_geom::TolerancePolicy::uniform(0.001),
  )
  .expect("valid scale");
  assert_close(
    outcome.contour.bounds().min.x,
    anchor.x,
    1.0e-12,
    "anchor x",
  );
  assert_close(
    outcome.contour.bounds().min.y,
    anchor.y,
    1.0e-12,
    "anchor y",
  );
  assert_close(outcome.contour.bounds().width(), 6.0, 1.0e-12, "width");
}

#[test]
fn scale_with_a_zero_factor_is_refused() {
  let rectangle = primitive::rectangle(Point2::ORIGIN, 2.0, 2.0).expect("rectangle");
  for (sx, sy) in [(0.0, 1.0), (1.0, 0.0)] {
    let error = transform::scale(
      &rectangle,
      sx,
      sy,
      Point2::ORIGIN,
      cam_geom::TolerancePolicy::uniform(0.001),
    )
    .expect_err("a zero factor collapses the shape");
    assert!(
      matches!(error, GeomError::InvalidPrimitive { .. }),
      "{sx}/{sy}: {error:?}"
    );
  }
}

#[test]
fn scale_refuses_a_non_positive_tolerance() {
  let rectangle = primitive::rectangle(Point2::ORIGIN, 2.0, 2.0).expect("rectangle");
  let error = transform::scale(
    &rectangle,
    2.0,
    1.0,
    Point2::ORIGIN,
    cam_geom::TolerancePolicy::uniform(0.0),
  )
  .expect_err("a tolerance must be positive");
  assert!(
    matches!(error, GeomError::InvalidPrimitive { .. }),
    "{error:?}"
  );
}

proptest! {
  /// Rotating by an angle and back returns the original geometry (A4).
  #[test]
  fn rotation_round_trip(
    angle in -PI..PI,
    radius in 0.5f64..200.0,
    cx in -100.0f64..100.0,
    cy in -100.0f64..100.0,
  ) {
    let centre = Point2::new(cx, cy);
    let circle = primitive::circle(centre, radius).unwrap();
    let pivot = Point2::new(3.0, -4.0);
    let there = transform::rotate(&circle, angle, pivot).unwrap();
    let back = transform::rotate(&there, -angle, pivot).unwrap();
    let scale = radius.max(1.0);
    prop_assert!((back.bounds().center().x - centre.x).abs() / scale < 1.0e-9);
    prop_assert!((back.bounds().center().y - centre.y).abs() / scale < 1.0e-9);
    prop_assert!((back.perimeter() - circle.perimeter()).abs() / scale < 1.0e-9);
  }

  /// Mirroring twice about the same line is the identity, and winding is preserved each time.
  #[test]
  fn mirror_is_an_involution(
    width in 0.5f64..100.0,
    height in 0.5f64..100.0,
    dx in -50.0f64..50.0,
    dy in -50.0f64..50.0,
    angle in -PI..PI,
  ) {
    let rectangle = primitive::rectangle(Point2::new(1.0, 2.0), width, height).unwrap();
    let direction = Vec2::from_angle(angle, 1.0);
    let there = transform::mirror(&rectangle, Point2::new(dx, dy), direction).unwrap();
    prop_assert!(there.is_ccw(), "a mirror normalises winding");
    prop_assert!(there.is_closed());
    let back = transform::mirror(&there, Point2::new(dx, dy), direction).unwrap();
    prop_assert!(back.is_ccw());
    let scale = width.max(height);
    prop_assert!((back.bounds().center().x - rectangle.bounds().center().x).abs() / scale < 1.0e-9);
    prop_assert!((back.bounds().center().y - rectangle.bounds().center().y).abs() / scale < 1.0e-9);
    prop_assert!((back.signed_area() - rectangle.signed_area()).abs() / (width * height) < 1.0e-9);
  }
}
