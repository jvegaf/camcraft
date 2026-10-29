//! Behaviour tests for the geometry kernel's primitives, arcs and chaining (A2).

use cam_geom::{
  chain, Arc, Bounds, Contour, LineSeg, Point2, Segment, Vec2, DEFAULT_TOLERANCE as TOL,
};
use proptest::prelude::*;
use std::f64::consts::{PI, TAU};

fn rect(w: f64, h: f64) -> Contour {
  Contour::from_points(
    &[
      Point2::new(0.0, 0.0),
      Point2::new(w, 0.0),
      Point2::new(w, h),
      Point2::new(0.0, h),
    ],
    true,
  )
  .expect("rectangle is a valid contour")
}

fn circle(center: Point2, radius: f64) -> Contour {
  // A full circle is two half-turn arcs; an arc may never be a full turn.
  Contour::new(
    vec![
      cam_geom::Vertex::new(Point2::new(center.x + radius, center.y), 1.0),
      cam_geom::Vertex::new(Point2::new(center.x - radius, center.y), 1.0),
    ],
    true,
  )
  .expect("circle is valid")
}

#[test]
fn point_and_vector_arithmetic() {
  let a = Point2::new(1.0, 2.0);
  let b = Point2::new(4.0, 6.0);
  assert_eq!(a.distance_to(b), 5.0);
  assert_eq!(b - a, Vec2::new(3.0, 4.0));
  assert_eq!(a + Vec2::new(1.0, 1.0), Point2::new(2.0, 3.0));
  assert!((Vec2::new(3.0, 4.0).length() - 5.0).abs() < 1.0e-12);
  assert_eq!(Vec2::new(1.0, 0.0).perp_ccw(), Vec2::new(0.0, 1.0));
  assert!((Vec2::from_angle(PI / 2.0, 2.0).y - 2.0).abs() < 1.0e-12);
  assert_eq!(Vec2::ZERO.normalized(), None);
}

#[test]
fn bounds_expand_and_contains() {
  let bounds = Bounds::from_points(&[Point2::new(0.0, 0.0), Point2::new(10.0, 5.0)]);
  assert!(!bounds.is_empty());
  assert_eq!(bounds.width(), 10.0);
  assert_eq!(bounds.height(), 5.0);
  assert_eq!(bounds.center(), Point2::new(5.0, 2.5));
  assert!(bounds.contains(Point2::new(10.0, 5.0), 0.0));
  assert!(!bounds.contains(Point2::new(10.5, 5.0), 0.0));
  assert!(bounds.contains(Point2::new(10.5, 5.0), 1.0));
  assert!(Bounds::empty().is_empty());
}

#[test]
fn arc_from_three_points_recovers_the_circle() {
  let arc = Arc::from_three_points(
    Point2::new(1.0, 0.0),
    Point2::new(0.0, 1.0),
    Point2::new(-1.0, 0.0),
  )
  .expect("three points on the unit circle");
  assert!((arc.radius() - 1.0).abs() < 1.0e-12);
  assert!(arc.center().distance_to(Point2::new(0.0, 0.0)) < 1.0e-12);
  assert!(arc.is_ccw());
  assert!((arc.sweep() - PI).abs() < 1.0e-12);
  assert!(Arc::from_three_points(
    Point2::new(0.0, 0.0),
    Point2::new(1.0, 0.0),
    Point2::new(2.0, 0.0)
  )
  .is_err());
}

proptest! {
    /// A bulge round-trips through endpoints, including major arcs where bulge > 1.
    #[test]
    fn bulge_round_trip(
        sweep in 0.05f64..(TAU - 0.35),
        start_angle in -PI..PI,
        radius in 0.5f64..500.0,
    ) {
        let arc = Arc::new(Point2::new(3.0, -2.0), radius, start_angle, sweep).expect("valid arc");
        let again = Arc::from_endpoints_bulge(arc.start_point(), arc.end_point(), arc.bulge())
            .expect("bulge reconstructs the arc");
        let scale = radius.max(1.0);
        prop_assert!((again.radius() - radius).abs() / scale < 1.0e-6, "radius {} vs {}", again.radius(), radius);
        prop_assert!(again.start_point().distance_to(arc.start_point()) / scale < 1.0e-6);
        prop_assert!(again.end_point().distance_to(arc.end_point()) / scale < 1.0e-6);
        prop_assert!((again.sweep() - sweep).abs() < 1.0e-6, "sweep {} vs {}", again.sweep(), sweep);
    }

    /// Tessellation never deviates from the true arc by more than the requested tolerance.
    #[test]
    fn tessellation_respects_tolerance(
        sweep in 0.05f64..(TAU - 0.05),
        radius in 1.0f64..200.0,
        tolerance in 0.0005f64..0.2,
    ) {
        let arc = Arc::new(Point2::ORIGIN, radius, 0.3, sweep).expect("valid arc");
        let points = arc.tessellate(tolerance);
        prop_assert!(points.len() >= 2);
        let steps = points.len() - 1;
        for index in 0..steps {
            let a = arc.point_at_fraction(index as f64 / steps as f64);
            let b = arc.point_at_fraction((index + 1) as f64 / steps as f64);
            let mid = arc.point_at_fraction((index as f64 + 0.5) / steps as f64);
            let chord_mid = a.lerp(b, 0.5);
            let deviation = mid.distance_to(chord_mid);
            prop_assert!(deviation <= tolerance * 1.0000001, "deviation {}", deviation);
        }
    }

    /// Reversing a closed contour flips the sign of its area and keeps its magnitude.
    #[test]
    fn reverse_flips_area(w in 0.1f64..100.0, h in 0.1f64..100.0) {
        let contour = rect(w, h);
        let reversed = contour.reverse();
        prop_assert!((contour.signed_area() - w * h).abs() < 1.0e-9);
        prop_assert!((reversed.signed_area() + w * h).abs() < 1.0e-9);
        prop_assert!(contour.is_ccw());
        prop_assert!(!reversed.is_ccw());
    }
}

#[test]
fn rectangle_area_winding_and_perimeter() {
  let contour = rect(20.0, 10.0);
  assert_eq!(contour.len(), 4);
  assert!(contour.is_closed());
  assert_eq!(contour.signed_area(), 200.0);
  assert_eq!(contour.perimeter(), 60.0);
  assert!(!contour.has_arcs());
}

#[test]
fn arc_area_is_exact_against_the_analytic_value() {
  // A semicircle closed by its diameter: one arc plus one line.
  let radius = 10.0;
  let contour = Contour::new(
    vec![
      cam_geom::Vertex::new(Point2::new(-radius, 0.0), 1.0),
      cam_geom::Vertex::new(Point2::new(radius, 0.0), 0.0),
    ],
    true,
  )
  .expect("semicircle contour");
  assert!(contour.has_arcs());
  let expected = PI * radius * radius / 2.0;
  assert!(
    (contour.signed_area() - expected).abs() < 1.0e-9,
    "{} vs {}",
    contour.signed_area(),
    expected
  );
  assert!((contour.perimeter() - (PI * radius + 2.0 * radius)).abs() < 1.0e-9);
}

#[test]
fn two_half_arcs_make_a_closed_circle() {
  let radius = 25.0;
  let contour = circle(Point2::new(5.0, 5.0), radius);
  assert!(contour.is_closed());
  assert!(contour.has_arcs());
  let expected = PI * radius * radius;
  assert!((contour.signed_area() - expected).abs() < 1.0e-6);
  let segments = contour.segments();
  assert_eq!(segments.len(), 2);
  assert!(segments[1]
    .end_point()
    .is_near(segments[0].start_point(), 1.0e-9));
}

#[test]
fn chain_reassembles_a_rectangle_from_shuffled_segments() {
  let (a, b, c, d) = (
    Point2::new(0.0, 0.0),
    Point2::new(40.0, 0.0),
    Point2::new(40.0, 20.0),
    Point2::new(0.0, 20.0),
  );
  // Segments out of order, and two of them reversed.
  let segments = vec![
    Segment::Line(LineSeg::new(b, c)),
    Segment::Line(LineSeg::new(a, b)),
    Segment::Line(LineSeg::new(c, d)),
    Segment::Line(LineSeg::new(a, d)),
  ];
  let chained = chain(&segments, TOL);
  assert_eq!(chained.closed.len(), 1, "expected one closed contour");
  assert!(chained.open.is_empty());
  let contour = &chained.closed[0];
  assert_eq!(contour.len(), 4);
  assert!((contour.signed_area().abs() - 800.0).abs() < 1.0e-9);
}

#[test]
fn chain_keeps_two_disjoint_loops_apart_and_reports_open_chains() {
  let loops = vec![
    Segment::Line(LineSeg::new(Point2::new(0.0, 0.0), Point2::new(10.0, 0.0))),
    Segment::Line(LineSeg::new(
      Point2::new(10.0, 0.0),
      Point2::new(10.0, 10.0),
    )),
    Segment::Line(LineSeg::new(
      Point2::new(10.0, 10.0),
      Point2::new(0.0, 10.0),
    )),
    Segment::Line(LineSeg::new(Point2::new(0.0, 10.0), Point2::new(0.0, 0.0))),
    Segment::Line(LineSeg::new(Point2::new(50.0, 0.0), Point2::new(60.0, 0.0))),
    Segment::Line(LineSeg::new(
      Point2::new(60.0, 0.0),
      Point2::new(60.0, 10.0),
    )),
    Segment::Line(LineSeg::new(
      Point2::new(60.0, 10.0),
      Point2::new(50.0, 10.0),
    )),
    Segment::Line(LineSeg::new(
      Point2::new(50.0, 10.0),
      Point2::new(50.0, 0.0),
    )),
  ];
  let chained = chain(&loops, TOL);
  assert_eq!(chained.closed.len(), 2);
  assert!(chained.open.is_empty());

  let open = vec![
    Segment::Line(LineSeg::new(Point2::new(0.0, 0.0), Point2::new(10.0, 0.0))),
    Segment::Line(LineSeg::new(
      Point2::new(10.0, 0.0),
      Point2::new(10.0, 10.0),
    )),
  ];
  let chained = chain(&open, TOL);
  assert_eq!(chained.closed.len(), 0);
  assert_eq!(chained.open.len(), 1);
}

#[test]
fn chaining_preserves_arcs() {
  // A rectangle whose corners are quarter arcs: chaining must keep them as arcs, never
  // tessellate them (A2).
  let radius = 5.0;
  let bulge = (PI / 4.0 / 2.0).tan();
  let contour = Contour::new(
    vec![
      cam_geom::Vertex::new(Point2::new(radius, 0.0), 0.0),
      cam_geom::Vertex::new(Point2::new(30.0, 0.0), bulge),
      cam_geom::Vertex::new(Point2::new(30.0, 20.0), bulge),
      cam_geom::Vertex::new(Point2::new(0.0, 20.0), bulge),
      cam_geom::Vertex::new(Point2::new(0.0, 0.0), bulge),
    ],
    true,
  )
  .expect("rounded rectangle");
  let segments = contour.segments();
  let chained = chain(&segments, TOL);
  assert_eq!(chained.closed.len(), 1);
  let round_tripped = &chained.closed[0];
  assert!(round_tripped.has_arcs(), "chaining must not flatten arcs");
  let arc_count = round_tripped
    .segments()
    .iter()
    .filter(|segment| segment.as_arc().is_some())
    .count();
  assert_eq!(arc_count, 4);
  assert!((round_tripped.signed_area() - contour.signed_area()).abs() < 1.0e-9);
}

proptest! {
    /// Chaining a tessellated circle yields exactly one closed contour (A2 anchor).
    #[test]
    fn chaining_a_tessellated_circle_closes(
        radius in 1.0f64..100.0,
        tolerance in 0.001f64..0.05,
    ) {
        let contour = circle(Point2::ORIGIN, radius);
        let points = contour.tessellate(tolerance);
        let segments: Vec<Segment> = (0..points.len())
            .map(|index| Segment::Line(LineSeg::new(points[index], points[(index + 1) % points.len()])))
            .collect();
        let chained = chain(&segments, TOL);
        prop_assert_eq!(chained.closed.len(), 1, "open chains: {}", chained.open.len());
        prop_assert!(chained.open.is_empty());
        // The walk closes as soon as the tail lands back on the start, so a final segment shorter
        // than the tolerance is left over and reported, not silently merged.
        prop_assert!(chained.dropped <= 1, "dropped {}", chained.dropped);
        let closed = &chained.closed[0];
        prop_assert!(closed.len() + 1 >= segments.len());
        // The chained contour is an inscribed polygon, so its area is short of the true circle
        // by about 8/6 * sagitta/radius. Asserting against the analytic value would be wrong at
        // coarse tolerances; asserting against the discretisation bound is the real contract.
        let expected = PI * radius * radius;
        let relative_error = (closed.signed_area() - expected).abs() / expected;
        prop_assert!(
            relative_error <= 2.0 * tolerance / radius,
            "relative area error {} exceeds the discretisation bound for tolerance {} at radius {}",
            relative_error,
            tolerance,
            radius
        );
    }
}
