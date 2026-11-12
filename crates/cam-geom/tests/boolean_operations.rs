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

#[test]
fn a_contained_disc_subtracts_from_a_rectangle() {
  // The pinning case for the seam bug: a circle is two semicircles sharing their endpoints, so
  // endpoint-only deduplication would delete one and leave the boundary uncloseable. A fully
  // contained disc never crosses the rectangle, which is exactly the arrangement that used to fail.
  let plate = rect(0.0, 0.0, 10.0, 10.0);
  let hole = disc(5.0, 5.0, 1.0);
  match combine(&plate, &hole, BooleanOp::Difference, TOLERANCE).expect("resolvable") {
    Combined::Region { region, flattened } => {
      assert!(
        !flattened,
        "rectilinear edges and exact arcs need no flattening"
      );
      assert_eq!(
        region.boundaries.len(),
        2,
        "the plate and one hole boundary"
      );
      expect_close(
        area(&region),
        100.0 - std::f64::consts::PI,
        "plate minus disc",
      );
    }
    Combined::Empty => panic!("subtracting a contained disc leaves the plate"),
  }
}

#[test]
fn union_with_an_empty_region_returns_the_disc() {
  let empty = Region::new(Vec::new()).expect("an empty region");
  let circle = disc(0.0, 0.0, 1.0);
  match combine(&empty, &circle, BooleanOp::Union, TOLERANCE).expect("resolvable") {
    Combined::Region { region, .. } => {
      assert_eq!(region.boundaries.len(), 1, "the disc alone");
      expect_close(area(&region), std::f64::consts::PI, "disc area");
    }
    Combined::Empty => panic!("a disc union nothing is the disc"),
  }
}

#[test]
fn a_disc_tangent_to_the_inside_of_an_edge_resolves() {
  let plate = rect(0.0, 0.0, 10.0, 10.0);
  // Circle centered at (5,1) with radius 1 touches the bottom edge y=0 at (5,0) from inside.
  let hole = disc(5.0, 1.0, 1.0);
  let pi = std::f64::consts::PI;
  match (
    combine(&plate, &hole, BooleanOp::Union, TOLERANCE).expect("union resolvable"),
    combine(&plate, &hole, BooleanOp::Intersection, TOLERANCE).expect("intersection resolvable"),
    combine(&plate, &hole, BooleanOp::Difference, TOLERANCE).expect("difference resolvable"),
  ) {
    (
      Combined::Region { region: u, .. },
      Combined::Region { region: i, .. },
      Combined::Region { region: d, .. },
    ) => {
      expect_close(area(&u), 100.0, "tangent-inside union");
      expect_close(area(&i), pi, "tangent-inside intersection");
      expect_close(area(&d), 100.0 - pi, "tangent-inside difference");
      assert_eq!(
        d.boundaries.len(),
        2,
        "difference leaves the plate and a hole"
      );
      expect_close(
        area(&u) + area(&i),
        100.0 + pi,
        "tangent-inside area identity",
      );
    }
    other => panic!("a tangent-inside disc must resolve, got {other:?}"),
  }
}

/// A contour that crosses itself: two triangles meeting at (1, 1), each of area 1.
fn bowtie() -> Region {
  let contour = Contour::from_points(
    &[
      Point2::new(0.0, 0.0),
      Point2::new(2.0, 2.0),
      Point2::new(2.0, 0.0),
      Point2::new(0.0, 2.0),
    ],
    true,
  )
  .expect("bow-tie");
  Region::new(vec![contour]).expect("a self-crossing contour is a legal region")
}

#[test]
fn a_self_crossing_contour_is_healed_into_its_two_loops() {
  // Before this, `normalize` passed a self-crossing contour through as it arrived: the parity rule
  // still answered for every point, but the boundary was one crossing contour whose signed area is
  // zero while its material is not. The arrangement now cuts it against itself.
  let eight = bowtie();
  let healed = eight
    .normalize(TOLERANCE)
    .expect("the arrangement resolves the crossing");
  assert_eq!(
    healed.region.boundaries.len(),
    2,
    "two loops, not one crossing contour"
  );
  expect_close(area(&healed.region), 2.0, "the eight's material");
}

#[test]
fn a_self_crossing_region_answers_the_basic_identities() {
  let eight = bowtie();
  expect_close(area_of(&eight, &eight, BooleanOp::Union), 2.0, "A ∪ A");
  expect_close(
    area_of(&eight, &eight, BooleanOp::Intersection),
    2.0,
    "A ∩ A",
  );
  assert!(
    result(&eight, &eight, BooleanOp::Difference).is_none(),
    "A ∖ A is nothing"
  );
}

#[test]
fn regions_meeting_at_one_corner_have_no_intersection() {
  let a = rect(0.0, 0.0, 1.0, 1.0);
  let b = rect(1.0, 1.0, 2.0, 2.0);
  expect_close(area_of(&a, &b, BooleanOp::Union), 2.0, "A ∪ B");
  assert!(
    result(&a, &b, BooleanOp::Intersection).is_none(),
    "a corner is no area, so the intersection is empty"
  );
}

#[test]
fn a_shared_edge_is_not_counted_twice() {
  let a = rect(0.0, 0.0, 1.0, 1.0);
  let b = rect(1.0, 0.0, 2.0, 1.0);
  expect_close(area_of(&a, &b, BooleanOp::Union), 2.0, "A ∪ B");
  assert!(
    result(&a, &b, BooleanOp::Intersection).is_none(),
    "an edge is no area, so the intersection is empty"
  );
}

/// Axis-aligned overlap area of two rectangles, each as `(x0, y0, x1, y1)`; zero when they only touch
/// or are apart.
fn rect_overlap_area(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> f64 {
  let (ax0, ay0, ax1, ay1) = a;
  let (bx0, by0, bx1, by1) = b;
  let x0 = ax0.max(bx0);
  let y0 = ay0.max(by0);
  let x1 = ax1.min(bx1);
  let y1 = ay1.min(by1);
  let w = (x1 - x0).max(0.0);
  let h = (y1 - y0).max(0.0);
  w * h
}

#[test]
fn two_hundred_known_area_operations_hold() {
  // Deterministic corpus of rectangle pairs with analytically known areas (C1a/S2).
  // Each pair yields 3 operations (union, intersection, difference).
  let mut a_rects: Vec<(f64, f64, f64, f64)> = Vec::new();
  let mut b_rects: Vec<(f64, f64, f64, f64)> = Vec::new();
  // Base A: 10x6 at origin.
  let a = (0.0, 0.0, 10.0, 6.0);
  // 70 varied B rectangles: offsets/scales covering disjoint, touching, overlap, containment, identical.
  for (dx, dy, w, h) in [
    (12.0, 0.0, 4.0, 3.0),   // disjoint right
    (-6.0, 0.0, 4.0, 3.0),   // disjoint left
    (0.0, 8.0, 4.0, 3.0),    // disjoint above
    (0.0, -5.0, 4.0, 3.0),   // disjoint below
    (10.0, 0.0, 4.0, 6.0),   // touching right edge
    (-4.0, 0.0, 4.0, 6.0),   // touching left edge
    (0.0, 6.0, 10.0, 2.0),   // touching top edge
    (0.0, -2.0, 10.0, 2.0),  // touching bottom edge
    (10.0, 6.0, 2.0, 2.0),   // touching corner
    (2.0, 1.0, 3.0, 2.0),    // contained
    (1.0, 1.0, 8.0, 4.0),    // contained large
    (0.0, 0.0, 10.0, 6.0),   // identical
    (4.0, 2.0, 3.0, 2.0),    // overlap center
    (-2.0, 1.0, 5.0, 3.0),   // overlap left
    (8.0, 1.0, 5.0, 3.0),    // overlap right
    (2.0, -1.0, 4.0, 3.0),   // overlap bottom
    (2.0, 4.0, 4.0, 4.0),    // overlap top
    (-1.0, -1.0, 12.0, 8.0), // B contains A
    (3.0, 1.0, 4.0, 4.0),    // overlap middle
    (5.0, 0.0, 5.0, 6.0),    // half overlap right
    (0.0, 0.0, 5.0, 6.0),    // half overlap left
    (0.0, 3.0, 10.0, 3.0),   // half overlap top
    (0.0, 0.0, 10.0, 3.0),   // half overlap bottom
    (-3.0, 2.0, 6.0, 2.0),   // straddle left
    (7.0, 2.0, 6.0, 2.0),    // straddle right
    (2.0, -2.0, 6.0, 4.0),   // straddle bottom
    (2.0, 4.0, 6.0, 4.0),    // straddle top
    (1.0, 2.0, 1.0, 1.0),    // small inside
    (8.0, 4.0, 1.0, 1.0),    // small inside corner-ish
    (9.5, 5.5, 2.0, 2.0),    // overlap corner outside
    (-1.5, -1.5, 2.0, 2.0),  // overlap bottom-left outside
    (4.0, 2.5, 2.0, 1.0),    // thin horizontal inside
    (4.5, 1.0, 1.0, 4.0),    // thin vertical inside
    (11.0, 1.0, 3.0, 4.0),   // disjoint far right
    (3.0, 7.0, 4.0, 2.0),    // disjoint far above
  ] {
    a_rects.push(a);
    b_rects.push((dx, dy, dx + w, dy + h));
  }
  // Second base pair family: different A and more Bs (35 pairs).
  let a2 = (-5.0, -3.0, 5.0, 3.0);
  for (dx, dy, w, h) in [
    (6.0, 0.0, 2.0, 2.0),
    (-8.0, 0.0, 2.0, 2.0),
    (0.0, 4.0, 2.0, 2.0),
    (0.0, -6.0, 2.0, 2.0),
    (5.0, -3.0, 3.0, 6.0),
    (-8.0, -3.0, 3.0, 6.0),
    (-5.0, 3.0, 10.0, 2.0),
    (-5.0, -5.0, 10.0, 2.0),
    (-4.0, -2.0, 8.0, 4.0),
    (-6.0, -4.0, 12.0, 8.0),
    (-5.0, -3.0, 10.0, 6.0),
    (0.0, 0.0, 3.0, 2.0),
    (-2.0, -1.0, 4.0, 2.0),
    (3.0, 1.0, 4.0, 4.0),
    (-9.0, 1.0, 6.0, 4.0),
    (-2.0, -6.0, 4.0, 5.0),
    (-2.0, 1.0, 4.0, 5.0),
    (4.0, -1.0, 3.0, 2.0),
    (-7.0, -1.0, 3.0, 2.0),
    (1.0, 2.5, 3.0, 2.0),
    (1.0, -4.5, 3.0, 2.0),
    (-3.0, -1.0, 2.0, 2.0),
    (2.0, -2.0, 2.0, 4.0),
    (-4.0, -2.0, 2.0, 4.0),
    (0.0, -3.0, 10.0, 3.0),
    (0.0, 0.0, 10.0, 3.0),
    (-10.0, -3.0, 5.0, 6.0),
    (0.0, -3.0, 5.0, 6.0),
    (5.0, 3.0, 1.0, 1.0),
    (-6.0, -4.0, 1.0, 1.0),
    (2.0, 1.0, 1.0, 1.0),
    (-3.0, -2.0, 1.0, 1.0),
    (7.0, 2.0, 2.0, 2.0),
    (-1.0, 5.0, 2.0, 2.0),
    (10.0, 0.0, 1.0, 1.0),
  ] {
    a_rects.push(a2);
    b_rects.push((dx, dy, dx + w, dy + h));
  }

  assert!(a_rects.len() >= 67, "need >=67 pairs for >=201 operations");
  let mut ops = 0usize;
  for ((ax0, ay0, ax1, ay1), (bx0, by0, bx1, by1)) in a_rects.into_iter().zip(b_rects) {
    let ra = rect(ax0, ay0, ax1, ay1);
    let rb = rect(bx0, by0, bx1, by1);
    let aa = (ax1 - ax0) * (ay1 - ay0);
    let ab = (bx1 - bx0) * (by1 - by0);
    let inter = rect_overlap_area((ax0, ay0, ax1, ay1), (bx0, by0, bx1, by1));
    let expected_union = aa + ab - inter;
    let expected_inter = inter;
    let expected_diff = aa - inter;

    let u = area_of(&ra, &rb, BooleanOp::Union);
    let i_res = result(&ra, &rb, BooleanOp::Intersection);
    let i = i_res.as_ref().map_or(0.0, area);
    let d_res = result(&ra, &rb, BooleanOp::Difference);
    let d = d_res.as_ref().map_or(0.0, area);

    expect_close(u, expected_union, "known-area union");
    expect_close(i, expected_inter, "known-area intersection");
    expect_close(d, expected_diff, "known-area difference");
    ops += 3;
  }
  assert!(ops >= 200, "corpus must run >=200 operations, ran {ops}");
}
