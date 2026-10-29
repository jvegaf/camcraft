//! Behaviour tests for the segment intersection queries (M2 A7).
//!
//! Trim depends on these, so the cases that a naive implementation silently drops — tangency,
//! coincidence, and a single root that falls inside the segment — are the ones tested hardest.

use cam_geom::intersect::{self, Intersections};
use cam_geom::{Arc, LineSeg, Point2, Segment};
use proptest::prelude::*;
use std::f64::consts::{PI, TAU};

const TOL: f64 = 1.0e-9;

fn line(ax: f64, ay: f64, bx: f64, by: f64) -> Segment {
  Segment::Line(LineSeg::new(Point2::new(ax, ay), Point2::new(bx, by)))
}

/// A circle expressed as one near-full arc, because an arc may never be a full turn.
fn circle(center: Point2, radius: f64) -> Segment {
  Segment::Arc(Arc::new(center, radius, 0.0, TAU - 1.0e-6).expect("near-full arc"))
}

fn half_circle(center: Point2, radius: f64, start: f64) -> Segment {
  Segment::Arc(Arc::new(center, radius, start, PI).expect("half arc"))
}

fn sorted(mut points: Vec<Point2>) -> Vec<(f64, f64)> {
  points.sort_by(|a, b| {
    a.x
      .partial_cmp(&b.x)
      .unwrap()
      .then(a.y.partial_cmp(&b.y).unwrap())
  });
  points.into_iter().map(|p| (p.x, p.y)).collect()
}

#[test]
fn two_straight_segments_cross_at_one_point() {
  let a = line(0.0, 0.0, 10.0, 10.0);
  let b = line(0.0, 10.0, 10.0, 0.0);
  match intersect::intersect(a, b, TOL) {
    Intersections::Point(point) => {
      assert!((point.x - 5.0).abs() < 1.0e-12, "{point:?}");
      assert!((point.y - 5.0).abs() < 1.0e-12, "{point:?}");
    }
    other => panic!("expected one crossing point, got {other:?}"),
  }
}

#[test]
fn a_t_junction_is_one_point() {
  let a = line(0.0, 0.0, 10.0, 0.0);
  let b = line(5.0, 0.0, 5.0, 5.0);
  match intersect::intersect(a, b, TOL) {
    Intersections::Point(point) => assert_eq!(point, Point2::new(5.0, 0.0)),
    other => panic!("expected the touching point, got {other:?}"),
  }
}

#[test]
fn identical_segments_are_coincident() {
  let a = line(0.0, 0.0, 10.0, 0.0);
  match intersect::intersect(a, a, TOL) {
    Intersections::Coincident => {}
    other => panic!("expected coincidence, got {other:?}"),
  }
}

#[test]
fn partially_overlapping_collinear_segments_are_coincident() {
  let a = line(0.0, 0.0, 10.0, 0.0);
  let b = line(5.0, 0.0, 15.0, 0.0);
  match intersect::intersect(a, b, TOL) {
    Intersections::Coincident => {}
    other => panic!("expected coincidence, got {other:?}"),
  }
}

#[test]
fn collinear_segments_that_do_not_overlap_do_not_meet() {
  let a = line(0.0, 0.0, 1.0, 0.0);
  let b = line(5.0, 0.0, 6.0, 0.0);
  assert_eq!(intersect::intersect(a, b, TOL), Intersections::None);
}

#[test]
fn parallel_segments_that_miss_do_not_meet() {
  let a = line(0.0, 0.0, 10.0, 0.0);
  let b = line(0.0, 1.0, 10.0, 1.0);
  assert_eq!(intersect::intersect(a, b, TOL), Intersections::None);
}

#[test]
fn segments_whose_lines_cross_outside_the_segments_do_not_meet() {
  let a = line(0.0, 0.0, 1.0, 0.0);
  let b = line(5.0, -5.0, 5.0, 5.0);
  assert_eq!(intersect::intersect(a, b, TOL), Intersections::None);
}

#[test]
fn a_chord_crossing_a_circle_returns_two_points() {
  let arc = half_circle(Point2::ORIGIN, 5.0, 0.0);
  let chord = line(-10.0, 2.0, 10.0, 2.0);
  match intersect::intersect(chord, arc, TOL) {
    Intersections::Points(first, second) => {
      // Ordered along the chord, so the first point is the left one.
      assert!(first.x < second.x, "{first:?} should precede {second:?}");
      let expected = (25.0f64 - 4.0).sqrt();
      assert!((first.x + expected).abs() < 1.0e-9, "{first:?}");
      assert!((second.x - expected).abs() < 1.0e-9, "{second:?}");
      for point in [first, second] {
        assert!((point.y - 2.0).abs() < 1.0e-9);
        assert!((point.distance_to(Point2::ORIGIN) - 5.0).abs() < 1.0e-9);
      }
    }
    other => panic!("expected two crossing points, got {other:?}"),
  }
}

#[test]
fn a_touching_line_is_tangent_not_a_crossing() {
  let arc = half_circle(Point2::ORIGIN, 5.0, 0.0);
  let tangent = line(-10.0, 5.0, 10.0, 5.0);
  match intersect::intersect(tangent, arc, TOL) {
    Intersections::Tangent(point) => {
      assert!((point.x - 0.0).abs() < 1.0e-6, "{point:?}");
      assert!((point.y - 5.0).abs() < 1.0e-6, "{point:?}");
    }
    other => panic!("expected tangency, got {other:?}"),
  }
}

#[test]
fn a_line_that_misses_the_arc_does_not_meet_it() {
  let arc = half_circle(Point2::ORIGIN, 5.0, 0.0);
  let miss = line(-10.0, 6.0, 10.0, 6.0);
  assert_eq!(intersect::intersect(miss, arc, TOL), Intersections::None);
}

#[test]
fn a_chord_with_only_one_root_inside_the_arc_still_reports_it() {
  // The upper half only: the chord at y = 2 has both roots on the circle, but the arc spans the
  // upper half, so only the right-hand root lies on this arc piece.
  let arc = Segment::Arc(Arc::new(Point2::ORIGIN, 5.0, 0.0, PI / 2.0).expect("quarter arc"));
  let chord = line(-10.0, 2.0, 10.0, 2.0);
  match intersect::intersect(chord, arc, TOL) {
    Intersections::Point(point) => {
      let expected = (25.0f64 - 4.0).sqrt();
      assert!((point.x - expected).abs() < 1.0e-9, "{point:?}");
    }
    other => panic!("expected one point on the quarter arc, got {other:?}"),
  }
}

#[test]
fn two_crossing_circles_return_two_points() {
  let first = circle(Point2::ORIGIN, 5.0);
  let second = circle(Point2::new(3.0, 0.0), 5.0);
  match intersect::intersect(first, second, TOL) {
    Intersections::Points(a, b) => {
      for point in [a, b] {
        assert!(
          (point.distance_to(Point2::ORIGIN) - 5.0).abs() < 1.0e-9,
          "{point:?}"
        );
        assert!(
          (point.distance_to(Point2::new(3.0, 0.0)) - 5.0).abs() < 1.0e-9,
          "{point:?}"
        );
        assert!((point.x - 1.5).abs() < 1.0e-9, "{point:?}");
      }
      assert!(
        (a.y + b.y).abs() < 1.0e-9,
        "the two points are mirrored in Y"
      );
    }
    other => panic!("expected two points, got {other:?}"),
  }
}

#[test]
fn externally_tangent_circles_return_one_tangent_point() {
  let first = circle(Point2::ORIGIN, 5.0);
  let second = circle(Point2::new(10.0, 0.0), 5.0);
  match intersect::intersect(first, second, TOL) {
    Intersections::Tangent(point) => {
      assert!((point.x - 5.0).abs() < 1.0e-6, "{point:?}");
      assert!((point.y).abs() < 1.0e-6, "{point:?}");
    }
    other => panic!("expected tangency, got {other:?}"),
  }
}

#[test]
fn concentric_circles_either_coincide_or_miss() {
  let same = circle(Point2::ORIGIN, 5.0);
  assert_eq!(
    intersect::intersect(same, same, TOL),
    Intersections::Coincident
  );
  let inner = circle(Point2::ORIGIN, 3.0);
  assert_eq!(intersect::intersect(same, inner, TOL), Intersections::None);
}

#[test]
fn circles_that_do_not_reach_each_other_do_not_meet() {
  let first = circle(Point2::ORIGIN, 1.0);
  let second = circle(Point2::new(10.0, 0.0), 1.0);
  assert_eq!(
    intersect::intersect(first, second, TOL),
    Intersections::None
  );
}

#[test]
fn a_single_point_result_is_reported_as_a_point() {
  let result = Intersections::Point(Point2::new(1.0, 2.0));
  assert_eq!(result.points(), vec![Point2::new(1.0, 2.0)]);
  assert!(!result.is_empty());
  assert!(Intersections::None.is_empty());
  assert_eq!(Intersections::Coincident.points(), Vec::<Point2>::new());
  assert_eq!(
    Intersections::Points(Point2::new(1.0, 1.0), Point2::new(2.0, 2.0))
      .points()
      .len(),
    2
  );
}

proptest! {
  /// Intersection is symmetric: swapping the arguments gives the same points and the same kind.
  #[test]
  fn intersection_is_symmetric(
    ax in -20.0f64..20.0,
    ay in -20.0f64..20.0,
    adx in -20.0f64..20.0,
    ady in -20.0f64..20.0,
    bx in -20.0f64..20.0,
    by in -20.0f64..20.0,
    radius in 0.5f64..20.0,
  ) {
    let a = Segment::Line(LineSeg::new(Point2::new(ax, ay), Point2::new(ax + adx, ay + ady)));
    let b = Segment::Arc(Arc::new(Point2::new(bx, by), radius, 0.0, TAU - 1.0e-6).unwrap());
    let forward = intersect::intersect(a, b, TOL);
    let backward = intersect::intersect(b, a, TOL);
    prop_assert_eq!(std::mem::discriminant(&forward), std::mem::discriminant(&backward));
    prop_assert_eq!(sorted(forward.points()), sorted(backward.points()));
  }

  /// Every reported point lies on both segments.
  #[test]
  fn reported_points_lie_on_both_segments(
    ax in -20.0f64..20.0,
    ay in -20.0f64..20.0,
    adx in -20.0f64..20.0,
    ady in -20.0f64..20.0,
    radius in 0.5f64..20.0,
    cx in -10.0f64..10.0,
  ) {
    let a = Segment::Line(LineSeg::new(Point2::new(ax, ay), Point2::new(ax + adx, ay + ady)));
    let b = Segment::Arc(Arc::new(Point2::new(cx, 0.0), radius, 0.0, TAU - 1.0e-6).unwrap());
    for point in intersect::intersect(a, b, TOL).points() {
      prop_assert!(a.distance_to_point(point) < 1.0e-6, "{point:?} not on the line");
      prop_assert!(b.distance_to_point(point) < 1.0e-6, "{point:?} not on the arc");
    }
  }
}
