//! Behaviour tests for the authoring builders (M2 A1, A2).
//!
//! Every assertion is analytic: area, perimeter, winding and the arc structure of the result. A
//! vertex dump would pass while the geometry is wrong.

use cam_geom::{primitive, Arc, Contour, GeomError, Point2, Segment};
use proptest::prelude::*;
use std::f64::consts::{PI, TAU};

const EPS: cam_geom::TolerancePolicy = cam_geom::TolerancePolicy::uniform(1.0e-9);

fn assert_close(actual: f64, expected: f64, tolerance: cam_geom::TolerancePolicy, what: &str) {
  assert!(
    (actual - expected).abs() <= tolerance.epsilon,
    "{what}: expected {expected}, got {actual}"
  );
}

#[test]
fn line_is_an_open_contour_with_two_vertices() {
  let contour = primitive::line(Point2::new(1.0, 2.0), Point2::new(4.0, 6.0)).expect("valid line");
  assert_eq!(contour.len(), 2);
  assert!(!contour.is_closed());
  assert!(!contour.has_arcs());
  assert_close(contour.perimeter(), 5.0, EPS, "line length");
  assert_eq!(contour.vertices()[0].point, Point2::new(1.0, 2.0));
  assert_eq!(contour.vertices()[1].point, Point2::new(4.0, 6.0));
}

#[test]
fn line_with_coincident_endpoints_is_refused() {
  let error = primitive::line(Point2::ORIGIN, Point2::ORIGIN).expect_err("coincident is refused");
  assert!(
    matches!(error, GeomError::DegenerateArc { .. }),
    "{error:?}"
  );
}

#[test]
fn polyline_keeps_its_closed_flag() {
  let points = [
    Point2::new(0.0, 0.0),
    Point2::new(3.0, 0.0),
    Point2::new(3.0, 4.0),
  ];
  let open = primitive::polyline(&points, false).expect("valid open polyline");
  assert_eq!(open.len(), 3);
  assert!(!open.is_closed());
  assert_close(open.perimeter(), 7.0, EPS, "open polyline length");

  let closed = primitive::polyline(&points, true).expect("valid closed polyline");
  assert!(closed.is_closed());
  assert_close(closed.perimeter(), 12.0, EPS, "closed polyline length");
  assert!(closed.is_ccw());
}

#[test]
fn polyline_with_one_point_is_refused() {
  let error = primitive::polyline(&[Point2::ORIGIN], false).expect_err("too short");
  assert!(
    matches!(error, GeomError::ContourTooShort { vertices: 1 }),
    "{error:?}"
  );
}

#[test]
fn rectangle_is_counter_clockwise_with_analytic_area() {
  let contour = primitive::rectangle(Point2::new(2.0, 3.0), 10.0, 4.0).expect("valid rectangle");
  assert_eq!(contour.len(), 4);
  assert!(contour.is_closed());
  assert!(!contour.has_arcs());
  assert!(contour.is_ccw());
  assert_close(contour.signed_area(), 40.0, EPS, "rectangle area");
  assert_close(contour.perimeter(), 28.0, EPS, "rectangle perimeter");
  assert_eq!(contour.bounds().min, Point2::new(2.0, 3.0));
  assert_eq!(contour.bounds().max, Point2::new(12.0, 7.0));
}

#[test]
fn rectangle_with_a_non_positive_side_is_refused() {
  for (width, height) in [(0.0, 5.0), (5.0, 0.0), (-1.0, 5.0)] {
    let error = primitive::rectangle(Point2::ORIGIN, width, height)
      .expect_err("non-positive side must be refused");
    assert!(
      matches!(error, GeomError::InvalidPrimitive { .. }),
      "{width}x{height}: {error:?}"
    );
  }
}

#[test]
fn circle_is_exactly_two_half_turn_arcs() {
  let contour = primitive::circle(Point2::new(5.0, -1.0), 3.0).expect("valid circle");
  assert_eq!(
    contour.len(),
    2,
    "a circle is two half turns, never one full turn"
  );
  assert!(contour.is_closed());
  assert!(contour.has_arcs());

  let segments = contour.segments();
  assert_eq!(segments.len(), 2);
  for segment in segments {
    let arc = segment.as_arc().expect("every circle segment is an arc");
    assert_close(arc.sweep().abs(), PI, EPS, "half turn sweep");
    assert_close(arc.radius(), 3.0, EPS, "circle radius");
    assert_close(arc.center().x, 5.0, EPS, "circle centre x");
    assert_close(arc.center().y, -1.0, EPS, "circle centre y");
  }
}

#[test]
fn circle_area_and_perimeter_are_analytic() {
  let contour = primitive::circle(Point2::ORIGIN, 7.5).expect("valid circle");
  assert_close(
    contour.signed_area(),
    PI * 7.5 * 7.5,
    cam_geom::TolerancePolicy::uniform(1.0e-6),
    "circle area",
  );
  assert_close(contour.perimeter(), TAU * 7.5, EPS, "circle circumference");
  assert!(contour.is_ccw(), "a drawn circle closes counter-clockwise");
}

#[test]
fn circle_with_a_non_positive_radius_is_refused() {
  for radius in [0.0, -2.0] {
    let error = primitive::circle(Point2::ORIGIN, radius).expect_err("radius must be positive");
    assert!(
      matches!(error, GeomError::InvalidPrimitive { .. }),
      "{radius}: {error:?}"
    );
  }
}

#[test]
fn polygon_has_exactly_n_straight_segments() {
  let sides = 6;
  let radius = 12.0;
  let contour =
    primitive::polygon(Point2::new(1.0, 1.0), radius, sides, 0.0).expect("valid polygon");
  assert_eq!(contour.len(), sides);
  assert!(contour.is_closed());
  assert!(!contour.has_arcs(), "a polygon is straight segments only");
  assert!(contour.is_ccw());
  let expected_area =
    f64::from(sides as u32) / 2.0 * radius * radius * (TAU / f64::from(sides as u32)).sin();
  assert_close(contour.signed_area(), expected_area, EPS, "polygon area");
  let expected_perimeter =
    2.0 * f64::from(sides as u32) * radius * (PI / f64::from(sides as u32)).sin();
  assert_close(
    contour.perimeter(),
    expected_perimeter,
    cam_geom::TolerancePolicy::uniform(1.0e-9),
    "polygon perimeter",
  );
}

#[test]
fn polygon_rotation_places_the_first_vertex() {
  let contour = primitive::polygon(Point2::ORIGIN, 10.0, 4, PI / 4.0).expect("valid polygon");
  let first = contour.vertices()[0].point;
  assert_close(
    first.x,
    10.0 * (PI / 4.0).cos(),
    cam_geom::TolerancePolicy::uniform(1.0e-9),
    "rotated x",
  );
  assert_close(
    first.y,
    10.0 * (PI / 4.0).sin(),
    cam_geom::TolerancePolicy::uniform(1.0e-9),
    "rotated y",
  );
}

#[test]
fn polygon_with_fewer_than_three_sides_is_refused() {
  for sides in [0, 1, 2] {
    let error =
      primitive::polygon(Point2::ORIGIN, 5.0, sides, 0.0).expect_err("a polygon needs 3 sides");
    assert!(
      matches!(error, GeomError::InvalidPrimitive { .. }),
      "{sides}: {error:?}"
    );
  }
}

#[test]
fn star_alternates_outer_and_inner_vertices() {
  let points = 5;
  let (outer, inner) = (10.0, 4.0);
  let contour = primitive::star(Point2::ORIGIN, outer, inner, points, 0.0).expect("valid star");
  assert_eq!(
    contour.len(),
    points * 2,
    "a star has two vertices per point"
  );
  assert!(contour.is_closed());
  assert!(!contour.has_arcs());
  assert!(contour.is_ccw());
  for (index, vertex) in contour.vertices().iter().enumerate() {
    let radius = vertex.point.distance_to(Point2::ORIGIN);
    let expected = if index % 2 == 0 { outer } else { inner };
    assert_close(
      radius,
      expected,
      cam_geom::TolerancePolicy::uniform(1.0e-9),
      "star vertex radius",
    );
  }
}

#[test]
fn star_with_an_inner_radius_outside_the_outer_one_is_refused() {
  for (outer, inner) in [(10.0, 10.0), (10.0, 12.0), (10.0, 0.0)] {
    let error = primitive::star(Point2::ORIGIN, outer, inner, 5, 0.0)
      .expect_err("the inner radius must be strictly inside the outer one");
    assert!(
      matches!(error, GeomError::InvalidPrimitive { .. }),
      "{outer}/{inner}: {error:?}"
    );
  }
}

#[test]
fn arc_through_three_points_passes_through_the_middle_one() {
  let a = Point2::new(0.0, 0.0);
  let b = Point2::new(5.0, 5.0);
  let c = Point2::new(10.0, 0.0);
  let contour = primitive::arc_through(a, b, c).expect("valid arc");
  assert_eq!(contour.len(), 2);
  assert!(!contour.is_closed());
  assert!(contour.has_arcs());
  let arc = contour.segments()[0].as_arc().expect("one arc segment");
  assert_close(
    arc.radius(),
    5.0,
    cam_geom::TolerancePolicy::uniform(1.0e-9),
    "arc radius",
  );
  let middle = arc.point_at_fraction(0.5);
  assert!(
    middle.is_near(b, cam_geom::TolerancePolicy::uniform(1.0e-9)),
    "the arc's midpoint is the middle point: got {middle:?}"
  );
}

#[test]
fn arc_through_collinear_points_is_refused() {
  let error = primitive::arc_through(
    Point2::new(0.0, 0.0),
    Point2::new(1.0, 0.0),
    Point2::new(2.0, 0.0),
  )
  .expect_err("collinear points have no circle");
  assert!(
    matches!(error, GeomError::DegenerateArc { .. }),
    "{error:?}"
  );
}

#[test]
fn every_closed_builder_is_counter_clockwise() {
  let built: Vec<(&str, Contour)> = vec![
    (
      "rectangle",
      primitive::rectangle(Point2::ORIGIN, 4.0, 3.0).unwrap(),
    ),
    ("circle", primitive::circle(Point2::ORIGIN, 4.0).unwrap()),
    (
      "polygon",
      primitive::polygon(Point2::ORIGIN, 4.0, 7, 0.3).unwrap(),
    ),
    (
      "star",
      primitive::star(Point2::ORIGIN, 4.0, 2.0, 6, 0.1).unwrap(),
    ),
  ];
  for (name, contour) in built {
    assert!(contour.is_closed(), "{name} is closed");
    assert!(contour.is_ccw(), "{name} is counter-clockwise");
    assert!(contour.signed_area() > 0.0, "{name} has a positive area");
  }
}

#[test]
fn an_arc_that_would_close_on_itself_is_refused() {
  // Three points on a circle where the first and last coincide cannot become one arc.
  let error = primitive::arc_through(
    Point2::new(1.0, 0.0),
    Point2::new(0.0, 1.0),
    Point2::new(1.0, 0.0),
  )
  .expect_err("a full turn is not an arc");
  assert!(
    matches!(error, GeomError::DegenerateArc { .. }),
    "{error:?}"
  );
}

proptest! {
  /// Every builder produces a contour that survives a JSON round trip unchanged (A3 in M0 terms).
  #[test]
  fn builders_round_trip_through_json(
    width in 0.1f64..500.0,
    height in 0.1f64..500.0,
    radius in 0.1f64..250.0,
    sides in 3usize..12,
    rotation in -PI..PI,
  ) {
    let contours = vec![
      primitive::rectangle(Point2::new(1.0, 2.0), width, height).unwrap(),
      primitive::circle(Point2::new(-3.0, 4.0), radius).unwrap(),
      primitive::polygon(Point2::ORIGIN, radius, sides, rotation).unwrap(),
      primitive::star(Point2::ORIGIN, radius, radius / 2.0, sides, rotation).unwrap(),
      primitive::line(Point2::ORIGIN, Point2::new(width, height)).unwrap(),
    ];
    for contour in contours {
      let json = serde_json::to_string(&contour).unwrap();
      let back: Contour = serde_json::from_str(&json).unwrap();
      prop_assert_eq!(&back, &contour);
    }
  }

  /// A circle of any radius keeps its analytic area and circumference (A2).
  #[test]
  fn circle_is_analytic_at_any_radius(radius in 0.05f64..1000.0) {
    let contour = primitive::circle(Point2::new(3.0, -4.0), radius).unwrap();
    let scale = (radius * radius).max(1.0);
    prop_assert!(((contour.signed_area() - PI * radius * radius) / scale).abs() < 1.0e-12);
    prop_assert!(((contour.perimeter() - TAU * radius) / radius.max(1.0)).abs() < 1.0e-12);
    let arc: Arc = contour.segments()[0].as_arc().unwrap();
    prop_assert!((arc.sweep().abs() - PI).abs() < 1.0e-12);
    prop_assert!(matches!(contour.segments()[1], Segment::Arc(_)));
  }
}
