//! Behaviour tests for the ellipse builder (M2 A3).
//!
//! An ellipse is not representable by circular arcs, so the builder must approximate it and must
//! report the error it achieved. These tests check the bound, not the vertex count: a wrong aspect
//! ratio that happens to produce plausible arcs still fails.

use cam_geom::primitive::{self, EllipseApproximation};
use cam_geom::{Contour, GeomError, Point2, MAX_TESSELLATION_STEPS};
use std::f64::consts::PI;

fn assert_close(actual: f64, expected: f64, tolerance: f64, what: &str) {
  assert!(
    (actual - expected).abs() <= tolerance,
    "{what}: expected {expected}, got {actual}"
  );
}

/// Roughness of an approximated ellipse: the largest gap between the parametric ellipse and the
/// contour, measured independently of the builder's own error report.
fn measured_error(ellipse: &EllipseApproximation, center: Point2, rx: f64, ry: f64) -> f64 {
  let mut worst: f64 = 0.0;
  let samples = 4096;
  for index in 0..samples {
    let angle = std::f64::consts::TAU * f64::from(index) / f64::from(samples);
    let point = Point2::new(center.x + rx * angle.cos(), center.y + ry * angle.sin());
    let distance = distance_to_contour(&ellipse.contour, point);
    worst = worst.max(distance);
  }
  worst
}

fn distance_to_contour(contour: &Contour, point: Point2) -> f64 {
  contour
    .segments()
    .iter()
    .map(|segment| segment.distance_to_point(point))
    .fold(f64::INFINITY, f64::min)
}

#[test]
fn ellipse_meets_the_requested_tolerance() {
  let cases = [
    (Point2::new(10.0, -4.0), 20.0, 10.0, 0.01),
    (Point2::ORIGIN, 50.0, 50.0, 0.01),
    (Point2::ORIGIN, 100.0, 1.0, 0.01),
    (Point2::new(-3.0, 7.0), 8.0, 2.0, 0.001),
  ];
  for (center, rx, ry, tolerance) in cases {
    let ellipse = primitive::ellipse(center, rx, ry, tolerance).expect("valid ellipse");
    assert!(
      ellipse.max_radial_error <= tolerance,
      "{rx}x{ry} tol {tolerance}: reported error {}",
      ellipse.max_radial_error
    );
    let measured = measured_error(&ellipse, center, rx, ry);
    assert!(
      measured <= tolerance,
      "{rx}x{ry} tol {tolerance}: measured error {measured}"
    );
    assert!(ellipse.contour.is_closed(), "{rx}x{ry} is closed");
    assert!(ellipse.contour.is_ccw(), "{rx}x{ry} is counter-clockwise");
    assert!(ellipse.contour.has_arcs(), "{rx}x{ry} keeps arcs");
  }
}

#[test]
fn ellipse_bounds_are_exact_to_the_reported_error() {
  let center = Point2::new(5.0, 5.0);
  let (rx, ry) = (30.0, 12.0);
  let ellipse = primitive::ellipse(center, rx, ry, 0.01).expect("valid ellipse");
  let bounds = ellipse.contour.bounds();
  // The four arcs put a vertex on each axis extreme, but their own extreme can overshoot the
  // ellipse by a fraction of the approximation error. The box must never drift further than the
  // error the builder reports to the caller.
  let slack = ellipse.max_radial_error;
  assert_close(bounds.width(), 2.0 * rx, 2.0 * slack, "ellipse width");
  assert_close(bounds.height(), 2.0 * ry, 2.0 * slack, "ellipse height");
  assert_close(bounds.min.x, center.x - rx, slack, "left extreme");
  assert_close(bounds.max.y, center.y + ry, slack, "top extreme");
  assert_close(bounds.max.x, center.x + rx, slack, "right extreme");
  assert_close(bounds.min.y, center.y - ry, slack, "bottom extreme");
}

#[test]
fn circular_ellipse_bounds_are_exact() {
  let center = Point2::new(-7.0, 2.0);
  let ellipse = primitive::ellipse(center, 4.0, 4.0, 0.01).expect("valid ellipse");
  let bounds = ellipse.contour.bounds();
  assert_close(bounds.width(), 8.0, 1.0e-12, "circle width");
  assert_close(bounds.height(), 8.0, 1.0e-12, "circle height");
}

#[test]
fn ellipse_area_matches_the_analytic_value() {
  let (rx, ry) = (40.0, 15.0);
  let ellipse = primitive::ellipse(Point2::ORIGIN, rx, ry, 0.005).expect("valid ellipse");
  let analytic = PI * rx * ry;
  let relative = (ellipse.contour.signed_area() - analytic).abs() / analytic;
  assert!(
    relative < 1.0e-3,
    "area {} vs analytic {analytic}",
    ellipse.contour.signed_area()
  );
}

#[test]
fn circular_ellipse_is_exactly_a_circle() {
  let ellipse = primitive::ellipse(Point2::new(1.0, 2.0), 5.0, 5.0, 0.01).expect("valid ellipse");
  assert_eq!(
    ellipse.max_radial_error, 0.0,
    "a circle needs no approximation"
  );
  assert_eq!(
    ellipse.contour.len(),
    2,
    "an equal-radius ellipse is the two-arc circle"
  );
  let expected = primitive::circle(Point2::new(1.0, 2.0), 5.0).expect("circle");
  assert_eq!(ellipse.contour, expected);
}

#[test]
fn ellipse_with_a_non_positive_radius_is_refused() {
  for (rx, ry) in [(0.0, 5.0), (5.0, 0.0), (-1.0, 5.0), (5.0, -1.0)] {
    let error =
      primitive::ellipse(Point2::ORIGIN, rx, ry, 0.01).expect_err("both radii must be positive");
    assert!(
      matches!(error, GeomError::InvalidPrimitive { .. }),
      "{rx}x{ry}: {error:?}"
    );
  }
}

#[test]
fn ellipse_refuses_a_non_positive_tolerance() {
  for tolerance in [0.0, -0.01] {
    let error = primitive::ellipse(Point2::ORIGIN, 10.0, 5.0, tolerance)
      .expect_err("a tolerance must be positive");
    assert!(
      matches!(error, GeomError::InvalidPrimitive { .. }),
      "{tolerance}: {error:?}"
    );
  }
}

#[test]
fn ellipse_arc_count_stays_bounded() {
  // A 1000:1 sliver at a micrometre tolerance cannot be met, and the builder must stop rather than
  // allocate without limit.
  let ellipse = primitive::ellipse(Point2::ORIGIN, 1000.0, 1.0, 1.0e-6).expect("valid ellipse");
  assert!(
    ellipse.contour.len() < MAX_TESSELLATION_STEPS,
    "arc count {} exceeds the kernel's bound",
    ellipse.contour.len()
  );
  assert!(
    ellipse.max_radial_error > 1.0e-6,
    "an unmet tolerance must be reported honestly, got {}",
    ellipse.max_radial_error
  );
}

#[test]
fn a_tighter_tolerance_never_needs_fewer_arcs() {
  let loose = primitive::ellipse(Point2::ORIGIN, 20.0, 8.0, 0.5).expect("valid ellipse");
  let tight = primitive::ellipse(Point2::ORIGIN, 20.0, 8.0, 0.001).expect("valid ellipse");
  assert!(
    tight.contour.len() >= loose.contour.len(),
    "tight {} arcs vs loose {} arcs",
    tight.contour.len(),
    loose.contour.len()
  );
  assert!(tight.max_radial_error < loose.max_radial_error);
}

#[test]
fn every_ellipse_vertex_carries_an_arc() {
  let ellipse = primitive::ellipse(Point2::new(-2.0, 3.0), 25.0, 9.0, 0.01).expect("valid ellipse");
  assert!(
    ellipse.contour.len() >= 4,
    "an ellipse is at least four arcs"
  );
  for vertex in ellipse.contour.vertices() {
    assert!(
      vertex.bulge != 0.0,
      "every ellipse segment is an arc, found a straight one at {:?}",
      vertex.point
    );
  }
  let segments = ellipse.contour.segments();
  assert_eq!(segments.len(), ellipse.contour.len());
  for (index, segment) in segments.iter().enumerate() {
    let next = &segments[(index + 1) % segments.len()];
    assert!(
      segment.end_point().is_near(next.start_point(), 1.0e-12),
      "arc {index} does not meet its neighbour"
    );
  }
}
