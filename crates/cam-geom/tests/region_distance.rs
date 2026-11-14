//! Behaviour tests for the exact distance from a point to a region's boundary (M7 T1, A1, D-M7-2).
//!
//! The V-carve surface is this distance field, so the answer has to be exact in two senses. It is the
//! **minimum over every segment of every boundary** of that segment's own exact point distance, which
//! is why a circle's centre is one *radius* away rather than one apothem; and it is exact in
//! arithmetic, so every expectation here is written out rather than measured — `3.0`, not "about 3".
//!
//! Every test also pins the second half of the answer, membership: a distance alone cannot tell a
//! point 2 mm inside a wall from one 2 mm outside it, and the medial-axis code and the claim both need
//! the two facts at the same point.

use proptest::prelude::*;

use cam_geom::primitive;
use cam_geom::region::Region;
use cam_geom::{Contour, Point2, Segment};

/// One nanometre: the tolerance every test here states explicitly.
const TOLERANCE: cam_geom::TolerancePolicy = cam_geom::TolerancePolicy::uniform(1.0e-9);

fn point(x: f64, y: f64) -> Point2 {
  Point2::new(x, y)
}

fn square(at: Point2, width: f64, height: f64) -> Contour {
  primitive::rectangle(at, width, height).expect("square")
}

fn circle(centre: Point2, radius: f64) -> Contour {
  primitive::circle(centre, radius).expect("circle")
}

fn region(boundaries: Vec<Contour>) -> Region {
  Region::new(boundaries).expect("closed boundaries")
}

/// The exact distance to the boundary and the membership, at the test tolerance.
fn answer(region: &Region, point: Point2) -> (f64, bool) {
  let answer = region
    .distance_to_boundary(point, TOLERANCE)
    .expect("the region has at least one boundary");
  (answer.distance, answer.inside)
}

/// The region's boundary sampled at a fixed arc-length step, with the largest step actually taken.
///
/// This is the *checker*, never the implementation: samples are used only to prove the reported
/// distance is a lower bound that is tight to within one step of a dense set of real boundary points.
fn sample_boundary(region: &Region, step: f64) -> (Vec<Point2>, f64) {
  let mut points = Vec::new();
  let mut largest: f64 = 0.0;
  for boundary in &region.boundaries {
    for segment in boundary.segments() {
      let length = segment.length();
      let pieces = (length / step).ceil().max(1.0) as usize;
      for index in 0..=pieces {
        let fraction = index as f64 / pieces as f64;
        points.push(match segment {
          Segment::Line(line) => line.point_at(fraction),
          Segment::Arc(arc) => arc.point_at_fraction(fraction),
        });
      }
      largest = largest.max(length / pieces as f64);
    }
  }
  (points, largest)
}

#[test]
fn a_point_inside_a_square_is_as_far_from_a_wall_as_the_nearest_wall() {
  // Walls of the 10 x 10 square at the origin: x = 0, x = 10, y = 0, y = 10.
  let region = region(vec![square(point(0.0, 0.0), 10.0, 10.0)]);

  // (3, 4): 3 to x = 0, 7 to x = 10, 4 to y = 0, 6 to y = 10, so the nearest is 3.
  assert_eq!(answer(&region, point(3.0, 4.0)), (3.0, true));

  // (7, 9): 7, 3, 9, 1 — the nearest wall is the top, at 10 - 9 = 1.
  assert_eq!(answer(&region, point(7.0, 9.0)), (1.0, true));
}

#[test]
fn two_walls_that_tie_are_still_one_exact_distance() {
  let region = region(vec![square(point(0.0, 0.0), 10.0, 10.0)]);

  // (3, 3): 3 to x = 0 and 3 to y = 0 — a diagonal tie between two walls, reported once.
  assert_eq!(answer(&region, point(3.0, 3.0)), (3.0, true));

  // (5, 5): all four walls are 5 away, so the tie is a four-way one and the answer is still exact.
  assert_eq!(answer(&region, point(5.0, 5.0)), (5.0, true));
}

#[test]
fn the_centre_of_a_circle_is_exactly_one_radius_away() {
  // The case that proves arcs are never tessellated: a 64-gon inscribed in r = 10 would report its
  // apothem, 10 * cos(pi / 64) = 9.98795456..., from the centre. The kernel reports the radius itself.
  let region = region(vec![circle(point(0.0, 0.0), 10.0)]);
  assert_eq!(answer(&region, point(0.0, 0.0)), (10.0, true));

  // Off centre the distance is still radial: (0, 4) is 10 - 4 = 6 from the nearer arc and 10 + 4 = 14
  // from the far one. It is 6 to within the floating-point geometry the contour *stores*, not to
  // within a chord tolerance: a bulged contour rebuilds each arc's centre from its bulge, and the
  // primitive circle's centres sit at (0, +-6.12e-16), which moves the answer by one ulp in the last
  // place (5.999999999999999). A 64-gon would be out by 1.2e-2, sixteen orders coarser.
  let off_centre = answer(&region, point(0.0, 4.0)).0;
  assert!(
    (off_centre - 6.0).abs() < 1.0e-12,
    "expected the radial 6 mm, got {off_centre}"
  );
}

#[test]
fn an_annulus_reports_the_nearer_of_its_two_radii() {
  // Material between r = 10 and r = 20, both centred on the origin.
  let region = region(vec![
    circle(point(0.0, 0.0), 20.0),
    circle(point(0.0, 0.0), 10.0),
  ]);

  // (12, 0): 12 - 10 = 2 to the inner circle, 20 - 12 = 8 to the outer — the inner one wins.
  assert_eq!(answer(&region, point(12.0, 0.0)), (2.0, true));

  // (18, 0): 18 - 10 = 8 to the inner circle, 20 - 18 = 2 to the outer — the outer one wins.
  assert_eq!(answer(&region, point(18.0, 0.0)), (2.0, true));

  // (5, 0) is inside the hole: 10 - 5 = 5 to the inner circle, 20 - 5 = 15 to the outer. The distance
  // is positive and 5, and the point is *not* material — which is exactly why the call reports both.
  assert_eq!(answer(&region, point(5.0, 0.0)), (5.0, false));
}

#[test]
fn outside_the_region_the_distance_is_still_positive() {
  let region = region(vec![square(point(0.0, 0.0), 10.0, 10.0)]);

  // (-3, 5) is 3 from the wall x = 0 — the same number a point 3 mm *inside* that wall reports.
  assert_eq!(answer(&region, point(-3.0, 5.0)), (3.0, false));

  // (13, 5) is 3 from the wall x = 10, on the other side.
  assert_eq!(answer(&region, point(13.0, 5.0)), (3.0, false));

  // (13, 13) is off a corner: the nearest boundary point is the vertex (10, 10), so the answer is
  // sqrt(3^2 + 3^2) = sqrt(18) = 4.2426..., not 3.
  assert_eq!(answer(&region, point(13.0, 13.0)), (18.0_f64.sqrt(), false));
}

#[test]
fn a_point_on_the_boundary_is_exactly_zero_away_and_not_material() {
  let square_region = region(vec![square(point(0.0, 0.0), 10.0, 10.0)]);
  // (0, 5) lies on the wall x = 0: zero, exactly, and an edge is not material.
  assert_eq!(answer(&square_region, point(0.0, 5.0)), (0.0, false));
  // A vertex is on the boundary too: (10, 10) is 0 from both of its walls.
  assert_eq!(answer(&square_region, point(10.0, 10.0)), (0.0, false));

  // The same rule on an arc: (10, 0) is the circle's own vertex, on the arc, so zero.
  let circle_region = region(vec![circle(point(0.0, 0.0), 10.0)]);
  assert_eq!(answer(&circle_region, point(10.0, 0.0)), (0.0, false));
}

#[test]
fn the_tolerance_decides_membership_but_never_the_distance() {
  // A point a tenth of a nanometre off the wall x = 0 is within the 1 nm tolerance, so it is not
  // material; the distance it reports is still the exact 1e-10, not a snapped zero, because the claim
  // measures material against the true field.
  let region = region(vec![square(point(0.0, 0.0), 10.0, 10.0)]);
  let answer = region
    .distance_to_boundary(point(1.0e-10, 5.0), TOLERANCE)
    .expect("the region has a boundary");
  assert_eq!(answer.distance, 1.0e-10);
  assert!(
    !answer.inside,
    "within tolerance of an edge is not material"
  );
}

#[test]
fn a_hole_can_be_the_nearer_boundary() {
  // A 20 x 20 square with a 4 x 4 square hole at (8, 8), so the hole is [8, 12] x [8, 12].
  let region = region(vec![
    square(point(0.0, 0.0), 20.0, 20.0),
    square(point(8.0, 8.0), 4.0, 4.0),
  ]);

  // (6, 10): 6 to the wall x = 0, 14 to x = 20, 10 to y = 0, 10 to y = 20, and 8 - 6 = 2 to the hole's
  // wall x = 8. The hole is the nearer boundary, and the point is material (parity 1).
  assert_eq!(answer(&region, point(6.0, 10.0)), (2.0, true));

  // (10, 10) is the hole's own centre: 2 to each of its four walls, 10 to each outer wall, so 2 — and
  // it is enclosed twice, once by the square and once by the hole, so it is not material.
  assert_eq!(answer(&region, point(10.0, 10.0)), (2.0, false));
}

#[test]
fn the_combined_answer_is_the_membership_contains_reports() {
  // One call answers both questions, and the membership half must be the region's own rule, not a
  // second opinion: parity across the boundaries, with an edge never counted as material.
  let region = region(vec![
    square(point(0.0, 0.0), 20.0, 20.0),
    square(point(8.0, 8.0), 4.0, 4.0),
  ]);
  for probe in [
    point(6.0, 10.0),
    point(10.0, 10.0),
    point(10.0, 5.0),
    point(-4.0, 10.0),
    point(25.0, 25.0),
    point(0.0, 10.0),
    point(8.0, 8.0),
  ] {
    let answer = region
      .distance_to_boundary(probe, TOLERANCE)
      .expect("the region has a boundary");
    assert_eq!(
      answer.inside,
      region.contains(probe, TOLERANCE),
      "membership disagrees with contains at {probe:?}"
    );
  }
}

#[test]
fn a_region_with_no_boundary_has_no_distance_to_report() {
  // `Region::default()` is a legal region: an area with no boundary encloses nothing. It therefore has
  // no distance to report, and `0.0` (which would claim the point lies *on* a boundary that does not
  // exist) and a panic would both be wrong answers. `None` is the contract.
  let region = Region::default();
  assert_eq!(
    region.distance_to_boundary(point(3.0, 4.0), TOLERANCE),
    None
  );
  assert!(!region.contains(point(3.0, 4.0), TOLERANCE));
}

proptest! {
  /// The report is a lower bound on the distance to every real boundary point, and tight to within
  /// one sampling step — a claim a tessellating implementation could only pass by luck, because its
  /// tentative answer is a *chord* distance and the sampling step is finer than any usable chord.
  #[test]
  fn the_report_is_a_lower_bound_tight_to_one_sampling_step(
    hole_x in 8.0_f64..32.0,
    hole_y in 8.0_f64..22.0,
    hole_radius in 1.0_f64..5.0,
    x in -10.0_f64..50.0,
    y in -10.0_f64..40.0,
  ) {
    // A 40 x 30 square with a circular hole whose centre stays at least 3 mm inside it.
    let region = region(vec![
      square(point(0.0, 0.0), 40.0, 30.0),
      circle(point(hole_x, hole_y), hole_radius),
    ]);
    let probe = point(x, y);
    let answer = region
      .distance_to_boundary(probe, TOLERANCE)
      .expect("the region has boundaries");

    let (samples, step) = sample_boundary(&region, 0.05);
    let slack = 1.0e-9;
    let mut best = f64::INFINITY;
    for sample in &samples {
      let sampled = probe.distance_to(*sample);
      prop_assert!(
        answer.distance <= sampled + slack,
        "reported {} is more than the {} to a boundary point at {:?}",
        answer.distance, sampled, sample
      );
      best = best.min(sampled);
    }
    prop_assert!(
      answer.distance >= best - step - slack,
      "reported {} is more than one step ({}) below the best sampled {}",
      answer.distance, step, best
    );
  }
}
