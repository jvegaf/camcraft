//! Behaviour tests for the set operations between two regions.

use cam_geom::boolean::{combine, BooleanOp, Combined};
use cam_geom::region::Region;
use cam_geom::{primitive, Contour, Point2, Segment};

const TOLERANCE: f64 = 1.0e-9;

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Region {
  let contour = Contour::from_points(
    &[
      Point2::new(x0, y0),
      Point2::new(x1, y0),
      Point2::new(x1, y1),
      Point2::new(x0, y1),
    ],
    true,
  )
  .expect("rectangle");
  Region::new(vec![contour]).expect("one boundary")
}

fn disc(cx: f64, cy: f64, radius: f64) -> Region {
  Region::new(vec![
    primitive::circle(Point2::new(cx, cy), radius).expect("circle")
  ])
  .expect("one boundary")
}

/// A region's area as its boundaries' signed contributions.
///
/// The walk orients every survivor with material on its left, so the outer boundary counts positive
/// and a hole counts negative. The absolute value of the sum is the material's area.
fn area(region: &Region) -> f64 {
  let signed: f64 = region
    .boundaries
    .iter()
    .flat_map(|boundary| boundary.segments())
    .map(|segment| segment.area_contribution())
    .sum();
  signed.abs()
}

fn result(a: &Region, b: &Region, op: BooleanOp) -> Option<Region> {
  match combine(a, b, op, TOLERANCE).expect("the operation is resolvable") {
    Combined::Empty => None,
    Combined::Region { region, .. } => Some(region),
  }
}

fn area_of(a: &Region, b: &Region, op: BooleanOp) -> f64 {
  result(a, b, op).map_or(0.0, |region| area(&region))
}

fn close(actual: f64, expected: f64) -> bool {
  let scale = actual.abs().max(expected.abs()).max(1.0);
  (actual - expected).abs() <= 1.0e-9 * scale
}

fn expect_close(actual: f64, expected: f64, what: &str) {
  assert!(
    close(actual, expected),
    "{what}: got {actual}, expected {expected}"
  );
}

#[test]
fn two_overlapping_rectangles_split_at_their_crossing() {
  let a = rect(0.0, 0.0, 2.0, 2.0);
  let b = rect(1.0, 1.0, 3.0, 3.0);
  expect_close(area(&a), 4.0, "A");
  expect_close(area(&b), 4.0, "B");
  expect_close(area_of(&a, &b, BooleanOp::Union), 7.0, "A ∪ B");
  expect_close(area_of(&a, &b, BooleanOp::Intersection), 1.0, "A ∩ B");
  expect_close(area_of(&a, &b, BooleanOp::Difference), 3.0, "A ∖ B");
  expect_close(area_of(&b, &a, BooleanOp::Difference), 3.0, "B ∖ A");
}

#[test]
fn disjoint_regions_intersect_in_nothing_and_that_is_an_answer() {
  let a = rect(0.0, 0.0, 1.0, 1.0);
  let b = rect(2.0, 0.0, 3.0, 1.0);
  assert!(
    result(&a, &b, BooleanOp::Intersection).is_none(),
    "two disjoint regions intersect in nothing, reported as Empty rather than an error"
  );
  expect_close(area_of(&a, &b, BooleanOp::Union), 2.0, "A ∪ B");
  expect_close(area_of(&a, &b, BooleanOp::Difference), 1.0, "A ∖ B");
}

#[test]
fn coincident_regions_answer_the_basic_identities() {
  // The most basic identities a set engine owes, on the input that first broke this one: two regions
  // whose boundaries lie exactly on top of each other. Both contribute every edge, so without a
  // doubled-edge rule the union would carry two coincident boundaries -- an area of 8 enclosing no
  // material at all, because parity cancels. The pieces are deduplicated, so the answer is the square.
  let a = rect(0.0, 0.0, 2.0, 2.0);
  let b = rect(0.0, 0.0, 2.0, 2.0);
  expect_close(area_of(&a, &b, BooleanOp::Union), 4.0, "A union A");
  expect_close(
    area_of(&a, &b, BooleanOp::Intersection),
    4.0,
    "A intersect A",
  );
  assert!(
    result(&a, &b, BooleanOp::Difference).is_none(),
    "A minus A is nothing"
  );
  let union = result(&a, &b, BooleanOp::Union).expect("a union");
  assert_eq!(
    union.boundaries.len(),
    1,
    "one square, not two coincident copies"
  );
}

#[test]
fn a_nested_region_subtracts_as_a_hole() {
  let outer = rect(0.0, 0.0, 2.0, 2.0);
  let inner = rect(0.5, 0.5, 1.5, 1.5);
  expect_close(area_of(&outer, &inner, BooleanOp::Union), 4.0, "A ∪ B");
  expect_close(
    area_of(&outer, &inner, BooleanOp::Intersection),
    1.0,
    "A ∩ B",
  );
  expect_close(area_of(&outer, &inner, BooleanOp::Difference), 3.0, "A ∖ B");
  assert!(
    result(&inner, &outer, BooleanOp::Difference).is_none(),
    "B inside A leaves nothing"
  );
}

#[test]
fn two_bars_make_a_plus() {
  let horizontal = rect(0.0, 1.0, 3.0, 2.0);
  let vertical = rect(1.0, 0.0, 2.0, 3.0);
  expect_close(
    area_of(&horizontal, &vertical, BooleanOp::Union),
    5.0,
    "A ∪ B",
  );
  expect_close(
    area_of(&horizontal, &vertical, BooleanOp::Intersection),
    1.0,
    "A ∩ B",
  );
  expect_close(
    area_of(&horizontal, &vertical, BooleanOp::Difference),
    2.0,
    "A ∖ B",
  );
}

#[test]
fn the_area_identity_holds_for_every_pair() {
  // The oracle that catches a misclassified crossing: two different operations have to agree about
  // the same total. A shape that passes its own area check and breaks this is still wrong.
  let pairs = [
    (rect(0.0, 0.0, 2.0, 2.0), rect(0.0, 0.0, 2.0, 2.0)),
    (rect(0.0, 0.0, 2.0, 2.0), rect(1.0, 1.0, 3.0, 3.0)),
    (rect(0.0, 0.0, 1.0, 1.0), rect(2.0, 0.0, 3.0, 1.0)),
    (rect(0.0, 0.0, 2.0, 2.0), rect(0.5, 0.5, 1.5, 1.5)),
    (rect(0.0, 1.0, 3.0, 2.0), rect(1.0, 0.0, 2.0, 3.0)),
    (disc(0.0, 0.0, 1.0), disc(1.0, 0.0, 1.0)),
    (disc(0.0, 0.0, 1.0), rect(0.0, 0.0, 1.0, 1.0)),
  ];
  for (index, (a, b)) in pairs.iter().enumerate() {
    let union = area_of(a, b, BooleanOp::Union);
    let intersection = area_of(a, b, BooleanOp::Intersection);
    let total = area(a) + area(b);
    assert!(
      close(union + intersection, total),
      "pair {index}: |A∪B| + |A∩B| = {} but |A| + |B| = {}",
      union + intersection,
      total
    );
    expect_close(area_of(b, a, BooleanOp::Union), union, "union commutes");
    expect_close(
      area_of(b, a, BooleanOp::Intersection),
      intersection,
      "intersection commutes",
    );
  }
}

#[test]
fn a_union_of_discs_keeps_its_arcs() {
  let a = disc(0.0, 0.0, 1.0);
  let b = disc(1.0, 0.0, 1.0);
  match combine(&a, &b, BooleanOp::Union, TOLERANCE).expect("resolvable") {
    Combined::Region { region, flattened } => {
      assert!(
        !flattened,
        "the arcs fit the model, so nothing had to be flattened"
      );
      let arcs = region
        .boundaries
        .iter()
        .flat_map(|boundary| boundary.segments())
        .filter(|segment| matches!(segment, Segment::Arc(_)))
        .count();
      assert!(arcs >= 2, "the lens's arcs survive as arcs, found {arcs}");
    }
    Combined::Empty => panic!("two overlapping discs have a union"),
  }
}

#[test]
fn a_degenerate_tolerance_is_refused() {
  let a = rect(0.0, 0.0, 1.0, 1.0);
  let b = rect(0.5, 0.5, 1.5, 1.5);
  assert!(
    combine(&a, &b, BooleanOp::Union, 0.0).is_err(),
    "zero tolerance"
  );
  assert!(
    combine(&a, &b, BooleanOp::Union, f64::NAN).is_err(),
    "NaN tolerance"
  );
}
