//! Behaviour tests for the exact segment-to-segment distance (D-RB-11).
//!
//! The distance is what makes the offset distance proof sound for a **whole** piece of curve: a
//! straight chord between two crossings can cut across a bulge that neither of its ends sees, so the
//! proof cannot ask about ends and hope. These tests pin the closed forms against hand-computed
//! values, then prove the answer from the outside — against a sampled lower bound and against
//! [`intersect`] — and finally exercise it in the feature's own terms: K5's 17 mm chord, and the
//! analytic ring count of the 4 mm island fixture.

use proptest::prelude::*;

use cam_geom::distance::segment_to_segment;
use cam_geom::intersect::intersect;
use cam_geom::offset::{offset_contour, OffsetOptions};
use cam_geom::primitive;
use cam_geom::region::Region;
use cam_geom::{Arc, Contour, LineSeg, Point2, Segment};

use std::f64::consts::{FRAC_PI_2, PI};

/// One nanometre: the tolerance every test here states explicitly.
const TOLERANCE: f64 = 1.0e-9;

fn point(x: f64, y: f64) -> Point2 {
  Point2::new(x, y)
}

fn line(x0: f64, y0: f64, x1: f64, y1: f64) -> Segment {
  Segment::Line(LineSeg::new(point(x0, y0), point(x1, y1)))
}

fn arc(cx: f64, cy: f64, radius: f64, start: f64, sweep: f64) -> Segment {
  Segment::Arc(Arc::new(point(cx, cy), radius, start, sweep).expect("a valid arc"))
}

/// The exact minimum distance, with no tolerance snapping at all.
fn exact(first: Segment, second: Segment) -> f64 {
  segment_to_segment(first, second, 0.0)
}

fn square(size: f64) -> Contour {
  primitive::rectangle(Point2::new(0.0, 0.0), size, size).expect("square")
}

fn circle(centre: Point2, radius: f64) -> Contour {
  primitive::circle(centre, radius).expect("circle")
}

fn region(boundaries: Vec<Contour>) -> Region {
  Region::new(boundaries).expect("closed boundaries")
}

/// The shortest distance from `segment` to any segment of `contour`, exactly.
fn distance_to_contour(segment: Segment, contour: &Contour) -> f64 {
  contour
    .segments()
    .into_iter()
    .map(|other| exact(segment, other))
    .fold(f64::INFINITY, f64::min)
}

/// A point of `segment` at `fraction` of its length: used only to *check* the exact answer.
fn sample(segment: Segment, fraction: f64) -> Point2 {
  match segment {
    Segment::Line(line) => line.point_at(fraction),
    Segment::Arc(arc) => arc.point_at_fraction(fraction),
  }
}

#[test]
fn parallel_lines_keep_the_gap_between_them() {
  let lower = line(0.0, 0.0, 10.0, 0.0);
  let upper = line(0.0, 5.0, 10.0, 5.0);
  assert!((exact(lower, upper) - 5.0).abs() < 1.0e-12);

  // The same gap, but with the spans staggered so that no perpendicular foot is inside both: the
  // answer is the corner to corner distance, 10 mm along and 5 mm across.
  let staggered = line(20.0, 5.0, 30.0, 5.0);
  let expected = (10.0_f64 * 10.0 + 5.0 * 5.0).sqrt();
  assert!(
    (exact(lower, staggered) - expected).abs() < 1.0e-12,
    "expected {expected}, got {}",
    exact(lower, staggered)
  );
}

#[test]
fn perpendicular_lines_are_zero_apart_when_they_cross_and_not_otherwise() {
  let horizontal = line(0.0, 0.0, 10.0, 0.0);
  // A proper crossing: the minimum is attained at an interior point of both, where the distance
  // function is not differentiable.
  assert_eq!(exact(horizontal, line(5.0, -5.0, 5.0, 5.0)), 0.0);
  // A T-junction: the vertical stops on the horizontal.
  assert_eq!(exact(horizontal, line(5.0, 0.0, 5.0, 10.0)), 0.0);
  // The same perpendicular pair, 2 mm short of touching: the minimum is the gap between the end and
  // the horizontal, and it is not a perpendicular foot.
  assert!((exact(horizontal, line(5.0, 2.0, 5.0, 10.0)) - 2.0).abs() < 1.0e-12);
  // Collinear segments that stop 10 mm short are 10 mm apart, not 0.
  assert!((exact(horizontal, line(20.0, 0.0, 30.0, 0.0)) - 10.0).abs() < 1.0e-12);
}

#[test]
fn a_line_through_a_circles_centre_is_the_radius_away() {
  // The spec's figure here is a correction worth recording: for a *finite* segment through the centre
  // the minimum is attained at the segment's own farthest point from the centre, because the distance
  // to the circle is `r - |q|` all the way along it. So a segment reaching 4 mm either side of the
  // centre is `10 - 4` away, and it is the centre itself — a degenerate segment — that is exactly one
  // radius away.
  let full_circle = arc(0.0, 0.0, 10.0, 0.0, std::f64::consts::TAU);
  let through = line(-4.0, 0.0, 4.0, 0.0);
  assert!((exact(through, full_circle) - 6.0).abs() < 1.0e-12);
  // The same segment against a semicircle on one side answers the same, because the radius is the
  // same all the way round.
  let half = arc(0.0, 0.0, 10.0, 0.0, PI);
  assert!((exact(through, half) - 6.0).abs() < 1.0e-12);
  // A point at the centre — the limiting case of a line through it — is exactly `r` away.
  let centre = line(0.0, 0.0, 0.0, 0.0);
  assert!((exact(centre, full_circle) - 10.0).abs() < 1.0e-12);
}

#[test]
fn a_line_tangent_to_a_circle_is_zero_apart() {
  // y = 10 touches the r10 circle at (0, 10), which is interior to the semicircle's sweep.
  let tangent = line(-5.0, 10.0, 5.0, 10.0);
  let half = arc(0.0, 0.0, 10.0, 0.0, PI);
  assert_eq!(exact(tangent, half), 0.0);
  // And the line that merely *nearly* touches is not zero: 0.5 mm outside the radius.
  let outside = line(-5.0, 10.5, 5.0, 10.5);
  assert!((exact(outside, half) - 0.5).abs() < 1.0e-12);
}

#[test]
fn two_concentric_circles_are_the_radius_difference_apart() {
  let outer = arc(0.0, 0.0, 10.0, 0.0, PI);
  let inner = arc(0.0, 0.0, 4.0, 0.0, PI);
  assert!((exact(outer, inner) - 6.0).abs() < 1.0e-12);
  // Concentric arcs whose sweeps do not overlap are decided by their ends, not by the radius: a
  // quarter on the right and a quarter on the left are `√116` apart, from (0, 10) to (-4, 0).
  let right_quarter = arc(0.0, 0.0, 10.0, 0.0, FRAC_PI_2);
  let left_quarter = arc(0.0, 0.0, 4.0, PI, FRAC_PI_2);
  let expected = 116.0_f64.sqrt();
  assert!(
    (exact(right_quarter, left_quarter) - expected).abs() < 1.0e-9,
    "expected {expected}, got {}",
    exact(right_quarter, left_quarter)
  );
}

#[test]
fn two_circles_side_by_side_are_the_gap_between_them_apart() {
  // Distinct centres, facing halves: the nearest points are on the centre line, interior to both
  // arcs, so this exercises the arc/arc centre-line candidates.
  let left = arc(0.0, 0.0, 5.0, -FRAC_PI_2, PI);
  let right = arc(20.0, 0.0, 7.0, FRAC_PI_2, PI);
  assert!((exact(left, right) - 8.0).abs() < 1.0e-12);
  // Pulling them 30 mm apart instead gives 18.
  let far = arc(30.0, 0.0, 7.0, FRAC_PI_2, PI);
  assert!((exact(left, far) - 18.0).abs() < 1.0e-12);
}

#[test]
fn a_nearest_point_that_is_an_endpoint_rather_than_a_foot() {
  // The perpendicular foot of the second segment falls 10 mm past the first one's end, so the answer
  // is the end to end distance: sqrt(10² + 3²).
  let first = line(0.0, 0.0, 10.0, 0.0);
  let second = line(20.0, 3.0, 20.0, 6.0);
  let expected = 109.0_f64.sqrt();
  assert!((exact(first, second) - expected).abs() < 1.0e-12);

  // The same for an arc: this segment sits below a semicircle, and the nearest points of the arc are
  // its two ends at (±10, 0), not any point of its sweep.
  let needle = arc(0.0, 0.0, 10.0, 0.0, PI);
  let below = line(0.0, -5.0, 0.0, -1.0);
  let expected = 101.0_f64.sqrt();
  assert!(
    (exact(below, needle) - expected).abs() < 1.0e-12,
    "expected {expected}, got {}",
    exact(below, needle)
  );
}

#[test]
fn an_arc_whose_nearest_point_is_interior() {
  // The segment is outside the circle and parallel to the arc's tangent at its topmost point, so the
  // minimum is radial and interior to both curves: 2 mm above the r10 semicircle's apex at (0, 10).
  // A segment *inside* the circle would not do: there the answer is `r - |q|` at the segment's own
  // farthest point from the centre, which is an endpoint.
  let above = line(-5.0, 12.0, 5.0, 12.0);
  let half = arc(0.0, 0.0, 10.0, 0.0, PI);
  assert!((exact(above, half) - 2.0).abs() < 1.0e-12);
  // Raising the segment further leaves more room, still with an interior foot on both curves.
  let farther = line(-5.0, 15.0, 5.0, 15.0);
  assert!((exact(farther, half) - 5.0).abs() < 1.0e-12);
}

#[test]
fn the_k5_chord_dips_inside_the_distance_its_ends_clear() {
  // K5's fixture, at its crossing stage. A 100 mm square pocket with a centred r10 island, offset by
  // 23 mm: the wall's inset is the 23..77 square, the island's growth is the r33 disc, and the two
  // cross. Between two crossings parity claims the disc's four lunes as material, and each lune is
  // bounded by an arc of the grown island and a **straight chord** of the shrunken wall.
  let outer = square(100.0);
  let island = circle(Point2::new(50.0, 50.0), 10.0);
  let distance = 23.0;
  let naive = region(vec![
    offset_contour(&outer, -distance, &OffsetOptions::default())
      .expect("the wall's inset")
      .contour,
    offset_contour(&island, distance, &OffsetOptions::default())
      .expect("the island's growth")
      .contour,
  ])
  .normalize(TOLERANCE)
  .expect("the offsets cross cleanly")
  .region;
  let lens = naive
    .boundaries
    .iter()
    .find(|piece| piece.len() == 2)
    .expect("the crossing stage leaves a lens");
  let chord = lens
    .segments()
    .into_iter()
    .find_map(|segment| match segment {
      Segment::Line(line) => Some(line),
      Segment::Arc(_) => None,
    })
    .expect("a lens has a straight chord");
  let chord = Segment::Line(chord);

  // Both of the chord's ends are the crossings, and both of them sit exactly the stage distance from
  // the island — which is all a proof that looks at ends ever sees.
  for end in [chord.start_point(), chord.end_point()] {
    let clearance = island
      .segments()
      .into_iter()
      .map(|segment| segment.distance_to_point(end))
      .fold(f64::INFINITY, f64::min);
    assert!(
      (clearance - distance).abs() < 1.0e-6,
      "a chord end is {clearance} mm from the island, not {distance}"
    );
  }

  // The chord runs along y = 23, while the island's circle reaches down to y = 40: in the middle the
  // chord is 17 mm away, six millimetres inside the distance its ends clear. That midpoint is a point
  // of the chord, so the exact pair distance cannot miss it.
  let middle = sample(chord, 0.5);
  assert!((middle.y - 23.0).abs() < 1.0e-9);
  let from_island = distance_to_contour(chord, &island);
  assert!(
    (from_island - 17.0).abs() < 1.0e-6,
    "the exact distance from the chord to the island is {from_island}, not 17"
  );
  assert!(
    from_island < distance - 0.002,
    "the chord is inside the stage's distance, so it must fail the proof"
  );

  // And the proof that uses it drops both lenses: four corner slivers survive, as the arc-only fix
  // already established, and the chord can no longer be carried by ends that clear.
  let result = region(vec![outer, island])
    .offset(distance, TOLERANCE)
    .expect("the corner slivers are far enough from both originals");
  assert_eq!(
    result.region.boundaries.len(),
    4,
    "four corners and neither lens, got {:?}",
    result
      .region
      .boundaries
      .iter()
      .map(|piece| piece.bounds())
      .collect::<Vec<_>>()
  );
}

#[test]
fn the_region_offset_keeps_the_four_millimetre_fixtures_analytic_stages() {
  // `crates/cam-toolpath/tests/pocket_islands.rs` cuts this fixture at a 4 mm stepover and expects 14
  // rings. A pocket ring comes from one offset piece, so the same arithmetic is visible here without
  // the tool: the five stages below 20 mm are two pieces each (the wall's inset and the island's
  // growth), and the crossing stage at 23 mm is the four corner slivers — 5 × 2 + 4 = 14.
  let material = region(vec![square(100.0), circle(Point2::new(50.0, 50.0), 10.0)]);
  let mut total = 0;
  for stage in [3.0, 7.0, 11.0, 15.0, 19.0, 23.0] {
    let result = material
      .offset(stage, TOLERANCE)
      .unwrap_or_else(|error| panic!("the fixture's {stage} mm stage failed: {error}"));
    let expected = if stage < 20.0 { 2 } else { 4 };
    assert_eq!(
      result.region.boundaries.len(),
      expected,
      "at {stage} mm, got {:?}",
      result
        .region
        .boundaries
        .iter()
        .map(|piece| piece.bounds())
        .collect::<Vec<_>>()
    );
    total += result.region.boundaries.len();
  }
  assert_eq!(total, 14, "the analytic ring count of the 4 mm fixture");
}

/// Segments on a 5 mm grid, so hand-checkable arithmetic and exact meetings are common.
fn any_line() -> impl Strategy<Value = Segment> {
  (-20_i32..=20, -20_i32..=20, -20_i32..=20, -20_i32..=20)
    .prop_filter("a segment needs two distinct ends", |(x0, y0, x1, y1)| {
      (x0, y0) != (x1, y1)
    })
    .prop_map(|(x0, y0, x1, y1)| line(f64::from(x0), f64::from(y0), f64::from(x1), f64::from(y1)))
}

/// Arcs on the same grid, with quarter-turn directions so tangencies and shared ends are reachable.
fn any_arc() -> impl Strategy<Value = Segment> {
  const STARTS: [f64; 8] = [
    0.0,
    FRAC_PI_2,
    PI,
    3.0 * FRAC_PI_2,
    std::f64::consts::FRAC_PI_4,
    3.0 * std::f64::consts::FRAC_PI_4,
    -std::f64::consts::FRAC_PI_4,
    -3.0 * std::f64::consts::FRAC_PI_4,
  ];
  const SWEEPS: [f64; 8] = [
    FRAC_PI_2,
    PI,
    3.0 * FRAC_PI_2,
    std::f64::consts::TAU,
    -FRAC_PI_2,
    -PI,
    -3.0 * FRAC_PI_2,
    -std::f64::consts::TAU,
  ];
  (
    -20_i32..=20,
    -20_i32..=20,
    1_i32..=10,
    0_usize..8,
    0_usize..8,
  )
    .prop_map(|(cx, cy, radius, start, sweep)| {
      arc(
        f64::from(cx),
        f64::from(cy),
        f64::from(radius),
        STARTS[start],
        SWEEPS[sweep],
      )
    })
}

fn any_segment() -> impl Strategy<Value = Segment> {
  prop_oneof![any_line(), any_arc()]
}

proptest! {
  #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

  /// The answer does not depend on which segment is asked first — the closed forms are symmetric, so
  /// the values are identical bit for bit and not merely close.
  #[test]
  fn the_distance_is_symmetric(first in any_segment(), second in any_segment()) {
    prop_assert_eq!(
      segment_to_segment(first, second, TOLERANCE),
      segment_to_segment(second, first, TOLERANCE)
    );
  }

  /// The exact minimum is never above the distance between two points that are actually on the two
  /// curves. Sampling can only ever *witness* an upper bound for the answer (or a witness *below* a
  /// too-large answer), which is exactly the right use for it: the answer itself is the closed form.
  #[test]
  fn the_distance_never_exceeds_any_sampled_pair_of_points(
    first in any_segment(),
    second in any_segment(),
  ) {
    let answered = exact(first, second);
    prop_assert!(answered.is_finite());
    for i in 0..=8 {
      let a = sample(first, f64::from(i) / 8.0);
      for j in 0..=8 {
        let b = sample(second, f64::from(j) / 8.0);
        let sampled = a.distance_to(b);
        prop_assert!(
          answered <= sampled + 1.0e-9,
          "the answer {} is above the sampled pair {:?}/{:?} at {}",
          answered,
          a,
          b,
          sampled
        );
      }
    }
  }

  /// The distance is zero exactly when the kernel's own intersection test says the two segments meet.
  ///
  /// This compares two independent pieces of algebra rather than two copies of one tolerance rule: the
  /// closed forms and exact meeting tests here, against [`intersect`]'s root finding. The tolerance is
  /// the kernel's own nanometre rather than `0.0`, because `intersect` decides membership on the
  /// circle by comparing radii with the tolerance — at `0.0` its own floating-point noise makes it
  /// miss a crossing this algebra finds (a quarter arc and a line that really does pass through it),
  /// which is `intersect`'s limit and not the distance's. The one configuration this case could not
  /// arbitrate — two arcs on the very same circle with disjoint sweeps, where `intersect` used to
  /// report a coincidence of the whole circle — is reachable, because `any_arc` guards no distinctness:
  /// two independently drawn arcs can share a centre and a radius. It is arbitrated now, because
  /// `intersect::arc_arc` decides one circle by the sweeps, and the case this test kept finding is its
  /// first assertion. The generator stays unfiltered on purpose: a guard against equal centres and
  /// radii would delete the evidence instead of answering it.
  #[test]
  fn the_distance_is_zero_exactly_when_the_segments_meet(
    first in any_segment(),
    second in any_segment(),
  ) {
    let answered = segment_to_segment(first, second, TOLERANCE);
    let met = !intersect(first, second, TOLERANCE).is_empty();
    prop_assert_eq!(
      answered == 0.0,
      met,
      "distance {} and intersect {:?} disagree",
      answered,
      intersect(first, second, TOLERANCE)
    );
  }
}

#[test]
fn the_nearest_point_and_the_distance_are_the_same_answer() {
  // `Segment::nearest_point` was added for the medial axis (M7 A2), which needs to know *which*
  // boundary point is nearest and not only how far it is. The two answers are computed by separate
  // code paths for an arc, so they are pinned together here: whatever the query point, the distance
  // to the reported nearest point is the distance the kernel already reports.
  let queries = [
    point(0.0, 0.0),
    point(0.0, 10.0),
    point(0.0, -3.0),
    point(4.0, 4.0),
    point(13.0, 0.0),
    point(-2.0, -2.0),
    point(2.0, 7.0),
  ];
  let segments = [
    line(0.0, 0.0, 10.0, 0.0),
    line(10.0, 0.0, 10.0, 10.0),
    arc(0.0, 0.0, 10.0, 0.0, PI),
    arc(3.0, 1.0, 2.5, 0.5, -2.0),
    // A zero-length segment, which is a point.
    line(4.0, 4.0, 4.0, 4.0),
  ];
  for segment in segments {
    for query in queries {
      let nearest = segment.nearest_point(query);
      assert!(
        (nearest.distance - segment.distance_to_point(query)).abs() <= 1.0e-12,
        "{segment:?} at {query:?}: nearest_point says {} and distance_to_point says {}",
        nearest.distance,
        segment.distance_to_point(query)
      );
      assert!(
        (nearest.point.distance_to(query) - nearest.distance).abs() <= 1.0e-12,
        "the reported point is exactly the reported distance away"
      );
      // The nearest set is a single point except at an arc's own centre.
      let at_centre = matches!(segment, Segment::Arc(a) if a.center().is_near(query, 1.0e-12));
      assert_eq!(nearest.interval, at_centre, "{segment:?} at {query:?}");
    }
  }
}
