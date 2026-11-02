//! Behaviour tests for the segment intersection queries (M2 A7).
//!
//! Trim depends on these, so the cases that a naive implementation silently drops — tangency,
//! coincidence, and a single root that falls inside the segment — are the ones tested hardest.

use cam_geom::distance::segment_to_segment;
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

fn quarter(center: Point2, radius: f64, start: f64) -> Segment {
  Segment::Arc(Arc::new(center, radius, start, PI / 2.0).expect("quarter arc"))
}

fn arc(center: Point2, radius: f64, start: f64, sweep: f64) -> Segment {
  Segment::Arc(Arc::new(center, radius, start, sweep).expect("arc"))
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
fn two_quarters_of_one_circle_that_share_no_angle_do_not_meet() {
  // The case the distance proptest kept finding: same centre, same radius, sweeps that share no
  // angle. The supporting circles coincide; the arcs do not, and an arc is the thing `intersect`
  // is asked about, so `Coincident` here would contradict its own contract.
  let east = quarter(Point2::ORIGIN, 5.0, 0.0);
  let west = quarter(Point2::ORIGIN, 5.0, PI);
  let apart = segment_to_segment(east, west, TOL);
  assert!(
    (apart - 50.0_f64.sqrt()).abs() <= TOL,
    "two disjoint quarters of one radius-5 circle are 5*sqrt(2) apart, measured {apart}"
  );
  assert_eq!(
    intersect::intersect(east, west, TOL),
    Intersections::None,
    "the distance says they are {} apart, so they do not meet",
    apart
  );
}

#[test]
fn two_quarters_of_one_circle_that_overlap_share_a_length() {
  let east = quarter(Point2::ORIGIN, 5.0, 0.0);
  let north_east = quarter(Point2::ORIGIN, 5.0, PI / 4.0);
  assert_eq!(
    intersect::intersect(east, north_east, TOL),
    Intersections::Coincident
  );
}

#[test]
fn two_quarters_of_one_circle_that_only_touch_meet_at_one_point() {
  let east = quarter(Point2::ORIGIN, 5.0, 0.0);
  let north = quarter(Point2::ORIGIN, 5.0, PI / 2.0);
  match intersect::intersect(east, north, TOL) {
    Intersections::Point(point) => assert!(
      point.distance_to(Point2::new(0.0, 5.0)) <= TOL,
      "the quarters share only the angle {}",
      PI / 2.0
    ),
    other => panic!("a shared endpoint is one point, not {other:?}"),
  }
  // The same touch across the seam, where one sweep ends at a full turn and the other starts at 0.
  let below = quarter(Point2::ORIGIN, 5.0, 3.0 * PI / 2.0);
  match intersect::intersect(below, east, TOL) {
    Intersections::Point(point) => assert!(
      point.distance_to(Point2::new(5.0, 0.0)) <= TOL,
      "the seam is where the sweeps meet"
    ),
    other => panic!("a shared endpoint across the seam is one point, not {other:?}"),
  }
}

#[test]
fn a_clockwise_quarter_covers_the_angles_it_runs_back_through() {
  let east = quarter(Point2::ORIGIN, 5.0, 0.0);
  // The same quarter, drawn backwards: a negative sweep covers the angles that *end* at its start.
  let backwards = arc(Point2::ORIGIN, 5.0, PI / 2.0, -PI / 2.0);
  assert_eq!(
    intersect::intersect(east, backwards, TOL),
    Intersections::Coincident
  );
  // And a quarter that runs backwards from angle 0 covers the fourth quadrant, not the first: it
  // shares exactly one angle with `east`, so reading a negative sweep as positive would make these
  // two coincide when they share a single point.
  let below = arc(Point2::ORIGIN, 5.0, 0.0, -PI / 2.0);
  match intersect::intersect(below, east, TOL) {
    Intersections::Point(point) => assert!(
      point.distance_to(Point2::new(5.0, 0.0)) <= TOL,
      "a clockwise and a counter-clockwise quarter from angle 0 touch only at angle 0"
    ),
    other => panic!("two quarters meeting at one angle are one point, not {other:?}"),
  }
}

#[test]
fn a_full_turn_arc_covers_every_quarter_of_its_circle() {
  let all = Segment::Arc(Arc::new(Point2::ORIGIN, 5.0, 0.0, TAU).expect("one full turn"));
  let east = quarter(Point2::ORIGIN, 5.0, 0.0);
  assert_eq!(
    intersect::intersect(all, east, TOL),
    Intersections::Coincident
  );
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

// ---- an oracle for the sweeps, computed without `intersect`'s own `coverage` -------------

/// The kernel's angular tolerance for a circle of this radius: the linear tolerance read as an
/// angle, with the floor it uses at radius zero. The oracle states the same rule on purpose — it
/// has to arbitrate *the* rule, not a constant of its own, or a disagreement it reports may be
/// about two unrelated thresholds rather than about the sweeps.
fn angular_tolerance(radius: f64) -> f64 {
  TOL / radius + 1.0e-12
}

/// Whether `theta` lies on `arc`'s sweep, within `slack` radians of either end.
fn angle_covered(arc: Arc, theta: f64, slack: f64) -> bool {
  if arc.sweep() >= 0.0 {
    let f = (theta - arc.start_angle()).rem_euclid(TAU);
    f <= arc.sweep() + slack || (TAU - f) <= slack
  } else {
    let b = (arc.start_angle() - theta).rem_euclid(TAU);
    b <= -arc.sweep() + slack || (TAU - b) <= slack
  }
}

/// The measure of the two sweeps' intersection, taken from boundary midpoints rather than from the
/// interval list the kernel builds, so the two agree only if both are right.
fn overlap_measure(a: Arc, b: Arc) -> f64 {
  let mut bounds = vec![0.0, TAU];
  for angle in [
    a.start_angle(),
    a.start_angle() + a.sweep(),
    b.start_angle(),
    b.start_angle() + b.sweep(),
  ] {
    let m = angle.rem_euclid(TAU);
    if m > 0.0 && m < TAU {
      bounds.push(m);
    }
  }
  bounds.sort_by(|x, y| x.partial_cmp(y).unwrap());
  bounds.dedup();
  bounds
    .windows(2)
    .filter(|w| {
      let mid = 0.5 * (w[0] + w[1]);
      // A window never contains one of the four boundaries, so a midpoint is strictly interior and
      // needs no slack: any slack here would only hide a boundary that is not where it claims to be.
      angle_covered(a, mid, 0.0) && angle_covered(b, mid, 0.0)
    })
    .map(|w| w[1] - w[0])
    .sum()
}

fn arc_of(segment: Segment) -> Arc {
  match segment {
    Segment::Arc(arc) => arc,
    Segment::Line(_) => panic!("expected an arc"),
  }
}

/// The pair the distance proptest found at master with a million cases: one centre, one radius,
/// sweeps that share no angle. `intersect` answered `Coincident` while the distance was
/// 0.7653668647301798, so the two functions disagreed about the same two arcs.
#[test]
fn the_proptest_case_of_equal_circles_with_disjoint_sweeps_does_not_meet() {
  let first = arc(Point2::new(-5.0, 11.0), 1.0, PI, PI / 2.0);
  let second = arc(Point2::new(-5.0, 11.0), 1.0, PI / 4.0, -PI / 2.0);
  assert!((segment_to_segment(first, second, TOL) - 0.7653668647301798).abs() <= 1e-12);
  assert_eq!(
    intersect::intersect(first, second, TOL),
    Intersections::None
  );
  assert_eq!(
    intersect::intersect(second, first, TOL),
    Intersections::None
  );
}

/// A full turn overlaps every sweep, including one far too small to matter, in either direction.
#[test]
fn a_full_turn_overlaps_a_tiny_sweep() {
  let all = arc(Point2::ORIGIN, 5.0, 0.0, TAU);
  let tiny = arc(Point2::ORIGIN, 5.0, PI, 1.0e-3);
  assert_eq!(
    intersect::intersect(all, tiny, TOL),
    Intersections::Coincident
  );
  assert_eq!(
    intersect::intersect(tiny, all, TOL),
    Intersections::Coincident
  );
  let backwards = arc(Point2::ORIGIN, 5.0, 0.0, -TAU);
  assert_eq!(
    intersect::intersect(backwards, quarter(Point2::ORIGIN, 5.0, 0.0), TOL),
    Intersections::Coincident
  );
}

/// Two arcs of one circle that genuinely overlap by less than the tolerance are not `None`: the
/// overlap is still a meeting, collapsed to the point it is indistinguishable from. `None` here
/// would contradict `segment_to_segment`, which answers a distance of zero for the same pair — the
/// disagreement this whole area exists to prevent, in the one window the grid cannot reach.
#[test]
fn an_overlap_below_the_tolerance_is_a_point_and_never_nothing() {
  let all = arc(Point2::ORIGIN, 5.0, 0.0, TAU);
  // 1e-11 rad is two orders of magnitude below `TOL / radius`, and lies wholly inside the turn.
  let sliver = arc(Point2::ORIGIN, 5.0, PI, 1.0e-11);
  for (first, second) in [(all, sliver), (sliver, all)] {
    let answer = intersect::intersect(first, second, TOL);
    assert!(!answer.is_empty(), "a real overlap is never None");
    assert!(
      !answer.is_coincident(),
      "below the tolerance the overlap is a point, not a length"
    );
    assert_eq!(
      segment_to_segment(first, second, TOL),
      0.0,
      "distance and intersect must agree that the pair meets"
    );
    let points = answer.points();
    assert_eq!(points.len(), 1, "got {points:?}");
    assert!(
      points[0].distance_to(Point2::new(-5.0, 0.0)) <= TOL,
      "the point must be where the sliver is, got {:?}",
      points[0]
    );
  }
}

/// A tiny overlap on the seam is reported where the seam is, not halfway around the circle from it.
/// `coverage` splits a sweep that crosses `0`/`TAU` into two intervals, so the two endpoints the
/// fallback averages are always clipped from the same side of the seam — the raw `max(start)` and
/// `min(end)` it reads never come from opposite branches.
///
/// Both ways of reaching the answer are pinned, because only the second one is the fallback this
/// test exists for: a sliver at `0` is caught by the endpoint comparison, and a sliver at `5` sits
/// strictly inside the wrapped arc's second interval, so only the shared-interval midpoint can
/// answer. Reading `350` and `0` as one interval would put that midpoint at `175`.
#[test]
fn a_tiny_overlap_on_the_seam_is_a_point_on_the_seam() {
  let through_350 = arc(Point2::ORIGIN, 5.0, 350.0 * PI / 180.0, 20.0 * PI / 180.0);
  for degrees in [0.0, 5.0] {
    let start = degrees * PI / 180.0;
    let sliver = arc(Point2::ORIGIN, 5.0, start, 1.0e-11);
    let expected = Point2::new(5.0 * start.cos(), 5.0 * start.sin());
    for (first, second) in [(through_350, sliver), (sliver, through_350)] {
      let answer = intersect::intersect(first, second, TOL);
      assert!(!answer.is_empty(), "the two arcs meet at {degrees} degrees");
      assert!(!answer.is_coincident(), "below the tolerance it is a point");
      let points = answer.points();
      assert_eq!(points.len(), 1, "got {points:?}");
      assert!(
        points[0].distance_to(expected) <= TOL,
        "the point must be on the seam at {degrees} degrees, not halfway round: got {:?}",
        points[0]
      );
    }
  }
}

/// Every same-circle pair answers for the sweeps and not for the circles, checked against an oracle
/// that measures angle coverage a different way, and against `segment_to_segment` in the same pass.
///
/// The oracle shares the tolerance *formula* with the kernel and nothing else: the measurement is
/// independent, the rule is the contract's. An oracle with a threshold of its own (this one used
/// `1.0e-6` for coincidence and `1e-12` for touch, which are not `TOL / radius + 1e-12`) agrees on
/// this grid only because every non-zero overlap in it is a multiple of `PI / 8`. The window below
/// the tolerance is pinned separately, by `an_overlap_below_the_tolerance_is_a_point_and_never_nothing`.
#[test]
fn same_circle_pairs_are_decided_by_their_sweeps_not_their_circles() {
  let starts: Vec<f64> = (0..16)
    .map(|k| f64::from(k) * PI / 8.0)
    .chain([-PI, -TAU, TAU + PI / 8.0])
    .collect();
  let sweeps = [
    PI / 8.0,
    PI / 2.0,
    PI,
    3.0 * PI / 2.0,
    TAU,
    -PI / 8.0,
    -PI / 2.0,
    -PI,
    -3.0 * PI / 2.0,
    -TAU,
  ];
  let arcs: Vec<Segment> = starts
    .iter()
    .flat_map(|start| {
      sweeps
        .iter()
        .map(move |sweep| arc(Point2::ORIGIN, 5.0, *start, *sweep))
    })
    .collect();
  for first in &arcs {
    for second in &arcs {
      let (a, b) = (arc_of(*first), arc_of(*second));
      let measure = overlap_measure(a, b);
      let angular = angular_tolerance(a.radius());
      let shares_angle = [a.start_angle(), a.start_angle() + a.sweep()]
        .iter()
        .any(|theta| angle_covered(b, *theta, angular))
        || [b.start_angle(), b.start_angle() + b.sweep()]
          .iter()
          .any(|theta| angle_covered(a, *theta, angular));
      let expected = if measure > angular {
        "coincident"
      } else if shares_angle {
        "one point"
      } else {
        "none"
      };
      let answer = intersect::intersect(*first, *second, TOL);
      let got = if answer.is_coincident() {
        "coincident"
      } else if answer.is_empty() {
        "none"
      } else {
        "one point"
      };
      assert_eq!(
        got,
        expected,
        "({}, {}) vs ({}, {})",
        a.start_angle(),
        a.sweep(),
        b.start_angle(),
        b.sweep()
      );
      assert_eq!(
        segment_to_segment(*first, *second, TOL) == 0.0,
        !answer.is_empty(),
        "distance and intersect disagree on ({}, {}) vs ({}, {})",
        a.start_angle(),
        a.sweep(),
        b.start_angle(),
        b.sweep()
      );
    }
  }
}
