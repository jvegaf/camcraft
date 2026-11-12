//! The degenerate corpus: the inputs a real drawing produces and a naive classifier refuses.
//!
//! C1b asks for two things that a pile of happy-path cases cannot show: that a normalization
//! **accepts** the shapes a drawing actually contains (self-crossing contours, boundaries that lie on
//! top of each other, vertices that touch without crossing) and that the area identity still holds on
//! each of them. Both are asserted pair by pair below, and the corpus is deterministic, so a
//! regression names the case that broke.

use cam_geom::boolean::{combine, normalize, BooleanOp, Combined};
use cam_geom::region::Region;
use cam_geom::{primitive, Contour, Point2, Segment};
use std::f64::consts::PI;

/// Finer than every feature below, so a refusal is a defect and not a tolerance choice.
const TOLERANCE: f64 = 1.0e-9;

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Region {
  polygon(&[
    Point2::new(x0, y0),
    Point2::new(x1, y0),
    Point2::new(x1, y1),
    Point2::new(x0, y1),
  ])
}

fn polygon(points: &[Point2]) -> Region {
  let contour = Contour::from_points(points, true).expect("a closed contour");
  Region::new(vec![contour]).expect("one boundary")
}

fn disc(cx: f64, cy: f64, radius: f64) -> Region {
  Region::new(vec![
    primitive::circle(Point2::new(cx, cy), radius).expect("circle")
  ])
  .expect("one boundary")
}

/// A region's area as its boundaries' signed contributions, absolute.
fn area(region: &Region) -> f64 {
  region
    .boundaries
    .iter()
    .flat_map(|boundary| boundary.segments())
    .map(|segment: Segment| segment.area_contribution())
    .sum::<f64>()
    .abs()
}

/// The area after the engine has healed the contour, which is what the identity is about.
fn healed_area(region: &Region) -> f64 {
  normalize(region, TOLERANCE)
    .map(|normalized| area(&normalized.region))
    .unwrap_or_else(|error| panic!("a degenerate input must normalize, got: {error}"))
}

fn result_area(a: &Region, b: &Region, op: BooleanOp) -> f64 {
  match combine(a, b, op, TOLERANCE) {
    Ok(Combined::Empty) => 0.0,
    Ok(Combined::Region { region, .. }) => area(&region),
    Err(error) => panic!("{op:?} was refused on a degenerate corpus pair: {error}"),
  }
}

fn close(actual: f64, expected: f64) -> bool {
  let scale = actual.abs().max(expected.abs()).max(1.0);
  (actual - expected).abs() <= 1.0e-9 * scale
}

/// Every pair the corpus is made of. Both sides carry material, so a refusal on any of them is a
/// defect rather than an "unresolvable input" outcome.
fn corpus() -> Vec<(&'static str, Region, Region)> {
  vec![
    // A contour that crosses itself: a figure eight whose two lobes meet at (1, 1).
    (
      "figure-eight",
      polygon(&[
        Point2::new(0.0, 0.0),
        Point2::new(2.0, 2.0),
        Point2::new(2.0, 0.0),
        Point2::new(0.0, 2.0),
      ]),
      rect(0.0, 0.0, 2.0, 2.0),
    ),
    // Two boundaries that share their whole left/right edge and meet nowhere else.
    (
      "shared-edge",
      rect(0.0, 0.0, 10.0, 10.0),
      rect(10.0, 0.0, 20.0, 10.0),
    ),
    // Two boundaries that are the same curve traversed the same way.
    (
      "coincident",
      rect(0.0, 0.0, 10.0, 10.0),
      rect(0.0, 0.0, 10.0, 10.0),
    ),
    // The same curve traversed the other way: orientation must not change the answer.
    (
      "reversed",
      rect(0.0, 0.0, 10.0, 10.0),
      polygon(&[
        Point2::new(0.0, 0.0),
        Point2::new(0.0, 10.0),
        Point2::new(10.0, 10.0),
        Point2::new(10.0, 0.0),
      ]),
    ),
    // A single contact vertex: one shared point bounds no area.
    (
      "corner-contact",
      rect(0.0, 0.0, 10.0, 10.0),
      rect(10.0, 10.0, 20.0, 20.0),
    ),
    // A disc tangent to four edges from the inside: it touches without crossing, at four vertices.
    (
      "disc-tangent-inside",
      rect(0.0, 0.0, 10.0, 10.0),
      disc(5.0, 5.0, 5.0),
    ),
    // Two discs osculating at (5, 0): contact between two curved boundaries.
    (
      "osculating-discs",
      disc(0.0, 0.0, 5.0),
      disc(10.0, 0.0, 5.0),
    ),
    // Collinear vertices, which a hand-drawn or exported contour always has.
    (
      "collinear-vertices",
      polygon(&[
        Point2::new(0.0, 0.0),
        Point2::new(5.0, 0.0),
        Point2::new(10.0, 0.0),
        Point2::new(10.0, 10.0),
        Point2::new(0.0, 10.0),
      ]),
      rect(5.0, 5.0, 15.0, 15.0),
    ),
  ]
}

#[test]
fn every_degenerate_pair_combines_and_keeps_the_area_identity() {
  for (name, a, b) in corpus() {
    let union = result_area(&a, &b, BooleanOp::Union);
    let intersection = result_area(&a, &b, BooleanOp::Intersection);
    let difference = result_area(&a, &b, BooleanOp::Difference);
    let area_a = healed_area(&a);
    let area_b = healed_area(&b);

    assert!(
      close(union + intersection, area_a + area_b),
      "{name}: |A∪B| + |A∩B| = {} but |A| + |B| = {area_a} + {area_b}",
      union + intersection
    );
    assert!(
      close(difference + intersection, area_a),
      "{name}: |A∖B| + |A∩B| = {} but |A| = {area_a}",
      difference + intersection
    );
    assert!(
      close(union, area_a + area_b - intersection),
      "{name}: the union is not the pair's material minus what they share"
    );
  }
}

#[test]
fn normalization_accepts_every_degenerate_input() {
  for (name, a, b) in corpus() {
    for (side, region) in [("first", a), ("second", b)] {
      let normalized = normalize(&region, TOLERANCE)
        .unwrap_or_else(|error| panic!("{name}: the {side} input was refused: {error}"));
      assert!(
        !normalized.region.boundaries.is_empty(),
        "{name}: the {side} input carries material, so its normalization cannot be empty"
      );
    }
  }
}

#[test]
fn a_self_crossing_contour_resolves_into_its_lobes() {
  let bowtie = polygon(&[
    Point2::new(0.0, 0.0),
    Point2::new(2.0, 2.0),
    Point2::new(2.0, 0.0),
    Point2::new(0.0, 2.0),
  ]);
  // The signed area of the raw crossing contour is zero while its material is not: the engine heals
  // it into two triangles of one square millimetre each.
  assert!(
    close(area(&bowtie), 0.0),
    "the raw contour lies about its area"
  );
  assert!(close(healed_area(&bowtie), 2.0));
}

#[test]
fn a_contour_without_material_is_refused_before_the_engine_sees_it() {
  // Out and back along the same line: consecutive vertices coincide, and the contour builder refuses
  // them, so a contour that bounds no material cannot reach the boolean engine at all. Pinned here
  // so the corpus cannot silently grow a case that cannot be built.
  let refused = Contour::from_points(
    &[
      Point2::new(0.0, 0.0),
      Point2::new(10.0, 0.0),
      Point2::new(0.0, 0.0),
    ],
    true,
  );
  assert!(
    refused.is_err(),
    "a contour with coincident consecutive vertices is a constructor error, not an arrangement one"
  );
}

#[test]
fn a_tangent_disc_is_shared_material_not_an_error() {
  let square = rect(0.0, 0.0, 10.0, 10.0);
  let tangent = disc(5.0, 5.0, 5.0);
  let disc_area = PI * 25.0;

  assert!(close(healed_area(&tangent), disc_area));
  assert!(close(
    result_area(&square, &tangent, BooleanOp::Intersection),
    disc_area
  ));
  assert!(close(
    result_area(&square, &tangent, BooleanOp::Union),
    100.0
  ));
  assert!(close(
    result_area(&square, &tangent, BooleanOp::Difference),
    100.0 - disc_area
  ));
}

#[test]
fn osculating_discs_meet_in_a_point_and_no_area() {
  let left = disc(0.0, 0.0, 5.0);
  let right = disc(10.0, 0.0, 5.0);
  assert!(close(
    result_area(&left, &right, BooleanOp::Intersection),
    0.0
  ));
  assert!(close(
    result_area(&left, &right, BooleanOp::Union),
    2.0 * PI * 25.0
  ));
}
