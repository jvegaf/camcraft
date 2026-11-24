//! Behaviour tests for the region model, its normalization and its subtraction (A3, A4, A5, A8).
//!
//! A region is a boundary list under the even-odd rule, so these tests assert geometry, never
//! internals: areas, vertex counts, perimeters and `contains` answers. Where the even-odd rule and the
//! shape a user might have expected disagree — two overlapping squares are *not* their union under
//! parity, they are their symmetric difference — the test says so and pins the parity answer, because
//! `contains` is the rule every later consumer (islands, offsetting, raster spans) trusts.

use proptest::prelude::*;

use cam_geom::primitive;
use cam_geom::region::{Normalized, Region};
use cam_geom::winding::{classify, relation, Membership, Relation};
use cam_geom::{Arc, Contour, GeomError, Point2, Vertex};

/// One nanometre: the tolerance every test here states explicitly.
const TOLERANCE: cam_geom::TolerancePolicy = cam_geom::TolerancePolicy::uniform(1.0e-9);

fn square(at: Point2, width: f64, height: f64) -> Contour {
  primitive::rectangle(at, width, height).expect("square")
}

fn circle(centre: Point2, radius: f64) -> Contour {
  primitive::circle(centre, radius).expect("circle")
}

fn region(boundaries: Vec<Contour>) -> Region {
  Region::new(boundaries).expect("closed boundaries")
}

fn normalize(region: &Region) -> Normalized {
  region.normalize(TOLERANCE).expect("normalization")
}

/// The area the parity rule assigns when the normalized contours bound disjoint material.
///
/// Normalization leaves each material component as its own contour, so a component's area is the
/// absolute signed area of its boundary. A nested boss would need the depths; none of the crossing
/// cases here nest.
fn component_area(normalized: &Normalized) -> f64 {
  normalized
    .region
    .boundaries
    .iter()
    .map(|boundary| boundary.signed_area().abs())
    .sum()
}

fn vertex_count(normalized: &Normalized, index: usize) -> usize {
  normalized.region.boundaries[index].len()
}

fn perimeter(normalized: &Normalized) -> f64 {
  normalized
    .region
    .boundaries
    .iter()
    .map(|boundary| boundary.perimeter())
    .sum()
}

/// True when two boundaries of the region still cross, i.e. when normalization failed its own
/// contract (A4).
fn still_crosses(region: &Region) -> bool {
  let boundaries = &region.boundaries;
  for first in 0..boundaries.len() {
    for second in (first + 1)..boundaries.len() {
      if relation(&boundaries[first], &boundaries[second], TOLERANCE) == Relation::Crossing {
        return true;
      }
    }
  }
  false
}

/// Every arc of the region, as the kernel sees it.
fn arcs(region: &Region) -> Vec<Arc> {
  let mut arcs = Vec::new();
  for boundary in &region.boundaries {
    for segment in boundary.segments() {
      if let Some(arc) = segment.as_arc() {
        arcs.push(arc);
      }
    }
  }
  arcs
}

/// True when `point` is clear of every boundary of the region by a margin, so the two membership
/// answers are not both "on the boundary".
fn away_from_boundaries(region: &Region, point: Point2) -> bool {
  region.boundaries.iter().all(|boundary| {
    classify(
      boundary,
      point,
      cam_geom::TolerancePolicy::uniform(TOLERANCE.epsilon * 1000.0),
    ) != Membership::OnBoundary
  })
}

#[test]
fn region_new_refuses_an_open_boundary_by_name() {
  let open = primitive::line(Point2::new(0.0, 0.0), Point2::new(10.0, 0.0)).expect("line");
  let refused = Region::new(vec![open]).expect_err("an open boundary encloses no material");
  assert!(matches!(refused, GeomError::EditRefused { .. }));
  assert!(
    refused.to_string().contains("closed contour"),
    "the refusal must name what was wrong, got {refused}"
  );
}

#[test]
fn region_new_accepts_closed_boundaries() {
  let accepted = Region::new(vec![square(Point2::new(0.0, 0.0), 10.0, 10.0)]).expect("square");
  assert_eq!(accepted.boundaries.len(), 1);
  assert!(Region::new(Vec::new()).is_ok(), "an empty region is legal");
}

#[test]
fn contains_is_the_parity_of_the_boundaries() {
  // A square with a circular hole: parity 1 between them, parity 2 inside the hole.
  let region = region(vec![
    square(Point2::new(0.0, 0.0), 20.0, 20.0),
    circle(Point2::new(10.0, 10.0), 3.0),
  ]);
  assert!(region.contains(Point2::new(2.0, 2.0), TOLERANCE));
  assert!(!region.contains(Point2::new(10.0, 10.0), TOLERANCE));
  assert!(!region.contains(Point2::new(-1.0, -1.0), TOLERANCE));
  assert!(!region.contains(Point2::new(25.0, 10.0), TOLERANCE));
}

#[test]
fn contains_reports_a_point_on_a_boundary_as_not_material() {
  // A tool must never be told to cut along the edge two regions share, so "on the boundary" is not
  // "inside" even though the winding number there is whatever rounding produced.
  let region = region(vec![square(Point2::new(0.0, 0.0), 10.0, 10.0)]);
  assert!(
    !region.contains(Point2::new(0.0, 5.0), TOLERANCE),
    "on an edge"
  );
  assert!(
    !region.contains(Point2::new(0.0, 0.0), TOLERANCE),
    "on a vertex"
  );
  assert!(region.contains(
    Point2::new(0.0, 5.0) + cam_geom::Vec2::new(1.0e-6, 0.0),
    TOLERANCE
  ));
}

#[test]
fn contains_keeps_its_answer_when_it_shares_the_walk_with_the_distance() {
  // PRESERVE (M7 T1): `contains` now answers through `distance_to_boundary`, so the case worth
  // pinning is the one the old code short-circuited — a point on a boundary whose parity is *odd* is
  // still not material, and the empty region, which has no boundary to be on, is not material either.
  let region = region(vec![
    square(Point2::new(0.0, 0.0), 20.0, 20.0),
    square(Point2::new(8.0, 8.0), 4.0, 4.0),
  ]);
  assert!(
    !region.contains(Point2::new(8.0, 10.0), TOLERANCE),
    "on the hole's wall, with parity 1 from the outer square alone"
  );
  assert!(region.contains(Point2::new(6.0, 10.0), TOLERANCE));
  assert!(!region.contains(Point2::new(10.0, 10.0), TOLERANCE));
  assert!(!Region::new(Vec::new())
    .expect("an empty region is legal")
    .contains(Point2::new(3.0, 4.0), TOLERANCE));
}

#[test]
fn subtract_appends_the_hole_and_normalizes() {
  let pocket = region(vec![square(Point2::new(0.0, 0.0), 20.0, 20.0)]);
  let with_hole = pocket
    .subtract(&circle(Point2::new(10.0, 10.0), 3.0), TOLERANCE)
    .expect("subtraction");
  assert_eq!(with_hole.boundaries.len(), 2, "outer plus the hole");
  assert!(!with_hole.contains(Point2::new(10.0, 10.0), TOLERANCE));
  assert!(with_hole.contains(Point2::new(10.0, 16.0), TOLERANCE));
  assert!(!with_hole.contains(Point2::new(10.0, 22.0), TOLERANCE));

  // Nothing crossed, so normalization is the identity and the arcs are untouched (A4).
  let again = with_hole.normalize(TOLERANCE).expect("idempotent");
  assert_eq!(again.region, with_hole);
  assert!(!again.flattened);
}

#[test]
fn subtracting_the_other_way_round_follows_the_parity_rule() {
  // Subtracting the square from the circle appends the square and lets parity decide. Parity has no
  // notion of "outer": inside both boundaries is parity 2, so the square becomes the hole and the
  // ring between them is material. This is the documented behaviour of the even-odd model (D-RB-2),
  // not an accident of argument order.
  let disc = region(vec![circle(Point2::new(0.0, 0.0), 10.0)]);
  let ring = disc
    .subtract(&square(Point2::new(-3.0, -3.0), 6.0, 6.0), TOLERANCE)
    .expect("subtraction");
  assert_eq!(ring.boundaries.len(), 2);
  assert!(
    !ring.contains(Point2::new(0.0, 0.0), TOLERANCE),
    "inside the square"
  );
  assert!(
    ring.contains(Point2::new(6.0, 0.0), TOLERANCE),
    "between them"
  );
  assert!(
    !ring.contains(Point2::new(12.0, 0.0), TOLERANCE),
    "outside the disc"
  );
}

#[test]
fn a_contour_minus_itself_is_refused_by_name() {
  let boundary = square(Point2::new(0.0, 0.0), 10.0, 10.0);
  let region = region(vec![boundary.clone()]);
  let refused = region
    .subtract(&boundary, TOLERANCE)
    .expect_err("parity 2 everywhere");
  assert!(matches!(refused, GeomError::EditRefused { .. }));
  assert!(
    refused.to_string().contains("no material"),
    "the refusal must say that nothing is left, got {refused}"
  );
}

#[test]
fn two_overlapping_squares_normalize_to_the_parity_contours() {
  // The even-odd rule makes two overlapping squares their symmetric difference, not their union: the
  // overlap is inside two boundaries and therefore not material. Normalization must therefore produce
  // two L-shaped contours of 75 mm2 each (6 vertices each), and the result must have no crossings.
  //
  // The union of these two squares would be one octagon of 175 mm2; that is the shape a user drawing
  // two overlapping rectangles might have in mind, and it is *not* what parity answers, so `contains`
  // and `normalize` agree on 150 mm2 instead. A future union would have to be its own operation.
  let overlapping = region(vec![
    square(Point2::new(0.0, 0.0), 10.0, 10.0),
    square(Point2::new(5.0, 5.0), 10.0, 10.0),
  ]);
  let normalized = normalize(&overlapping);
  assert_eq!(normalized.region.boundaries.len(), 2, "two L-shaped pieces");
  assert!((component_area(&normalized) - 150.0).abs() < 1.0e-9);
  assert_eq!(vertex_count(&normalized, 0), 6);
  assert_eq!(vertex_count(&normalized, 1), 6);
  for boundary in &normalized.region.boundaries {
    assert!(
      (boundary.signed_area().abs() - 75.0).abs() < 1.0e-9,
      "each piece is 75 mm2, got {}",
      boundary.signed_area()
    );
    assert!(!boundary.has_arcs(), "no arc went in, none came out");
  }
  assert!(!still_crosses(&normalized.region));

  // And the membership agrees with the input, overlap included.
  let inside_overlap = Point2::new(7.0, 7.0);
  assert!(!overlapping.contains(inside_overlap, TOLERANCE));
  assert!(!normalized.region.contains(inside_overlap, TOLERANCE));
  assert!(normalized.region.contains(Point2::new(2.0, 2.0), TOLERANCE));
  assert!(normalized
    .region
    .contains(Point2::new(12.0, 12.0), TOLERANCE));
}

#[test]
fn two_squares_touching_along_an_edge_do_not_double_it() {
  // The shared edge is interior to the material, so it must not appear in the result at all: the
  // perimeters of the two inputs sum to 80 mm, the perimeter of the union is 60 mm, and only the sum
  // including the shared edge twice would be 80. Parities either side of the shared edge are both
  // material, which is what tells the arrangement to drop both copies of it.
  let stacked = region(vec![
    square(Point2::new(0.0, 0.0), 10.0, 10.0),
    square(Point2::new(0.0, 10.0), 10.0, 10.0),
  ]);
  let normalized = normalize(&stacked);
  assert_eq!(normalized.region.boundaries.len(), 1, "one outline");
  assert!((normalized.region.boundaries[0].signed_area().abs() - 200.0).abs() < 1.0e-9);
  assert_eq!(vertex_count(&normalized, 0), 6);
  assert!(
    (perimeter(&normalized) - 60.0).abs() < 1.0e-9,
    "the shared edge must not be counted twice, got {}",
    perimeter(&normalized)
  );
  assert!(!normalized.flattened);
  assert!(normalized
    .region
    .contains(Point2::new(5.0, 15.0), TOLERANCE));
  assert!(normalized.region.contains(Point2::new(5.0, 5.0), TOLERANCE));
  assert!(!normalized
    .region
    .contains(Point2::new(5.0, 25.0), TOLERANCE));
}

#[test]
fn a_crossing_configuration_keeps_the_parity_area_and_the_arcs() {
  // A circle of radius 4 centred on a square's right edge: the half inside the square is parity 2 and
  // the half outside is parity 1, so the parity area is the square's own 100 mm2 — the union would be
  // 100 + 8*pi. The two contours are the square minus the inner half-disc (74.87 mm2) and the outer
  // half-disc (25.13 mm2).
  let crossing = region(vec![
    square(Point2::new(0.0, 0.0), 10.0, 10.0),
    circle(Point2::new(10.0, 5.0), 4.0),
  ]);
  let normalized = normalize(&crossing);
  assert_eq!(normalized.region.boundaries.len(), 2);
  assert!(
    (component_area(&normalized) - 100.0).abs() < 1.0e-6,
    "parity area, got {}",
    component_area(&normalized)
  );
  let outer = normalized.region.boundaries[0].signed_area().abs();
  assert!(
    (outer - (100.0 - 8.0 * std::f64::consts::PI)).abs() < 1.0e-6,
    "the square keeps everything but the inner half disc, got {outer}"
  );
  let outer_disc = normalized.region.boundaries[1].signed_area().abs();
  assert!(
    (outer_disc - 8.0 * std::f64::consts::PI).abs() < 1.0e-6,
    "the half outside the square is 8*pi mm2, got {outer_disc}"
  );
  assert!(!still_crosses(&normalized.region));
  assert!(!normalized.flattened, "splitting an arc keeps it an arc");

  // Every surviving arc is the original circle's arc: same centre, same radius (A8).
  let surviving = arcs(&normalized.region);
  assert!(
    !surviving.is_empty(),
    "the circle must still be made of arcs"
  );
  for arc in &surviving {
    assert!(
      (arc.radius() - 4.0).abs() < 1.0e-9,
      "radius {}",
      arc.radius()
    );
    assert!(arc.center().is_near(
      Point2::new(10.0, 5.0),
      cam_geom::TolerancePolicy::uniform(1.0e-9)
    ));
  }
  // The whole circle is a material boundary under parity -- inside it and inside the square cancels,
  // inside it and outside the square does not -- so the surviving arcs cover it exactly once.
  let arc_length: f64 = surviving.iter().map(|arc| arc.length()).sum();
  assert!(
    (arc_length - std::f64::consts::TAU * 4.0).abs() < 1.0e-6,
    "the surviving arcs cover the circle once, got {arc_length}"
  );

  assert!(
    !normalized.region.contains(Point2::new(8.0, 5.0), TOLERANCE),
    "inside both"
  );
  assert!(
    normalized
      .region
      .contains(Point2::new(13.0, 5.0), TOLERANCE),
    "circle only"
  );
  assert!(
    normalized.region.contains(Point2::new(2.0, 5.0), TOLERANCE),
    "square only"
  );
  assert!(!normalized
    .region
    .contains(Point2::new(2.0, 12.0), TOLERANCE));
}

#[test]
fn a_plus_sign_normalizes_correctly() {
  // Two bars crossing in four places: the corners of the crossing square, not just two. Every arm is
  // material, the crossing square is inside both boundaries and therefore not, so the parity area is
  // 500 - 100 = 400 mm2.
  let bars = region(vec![
    square(Point2::new(0.0, 10.0), 30.0, 10.0),
    square(Point2::new(10.0, 0.0), 10.0, 30.0),
  ]);
  let normalized = normalize(&bars);
  assert!(
    (component_area(&normalized) - 400.0).abs() < 1.0e-9,
    "four arms of 75 mm2 plus the four corners, got {}",
    component_area(&normalized)
  );
  assert!(!still_crosses(&normalized.region));
  assert!(!normalized.flattened);
  assert!(
    normalized
      .region
      .contains(Point2::new(15.0, 25.0), TOLERANCE),
    "top arm"
  );
  assert!(
    normalized
      .region
      .contains(Point2::new(15.0, 5.0), TOLERANCE),
    "bottom arm"
  );
  assert!(
    normalized
      .region
      .contains(Point2::new(5.0, 15.0), TOLERANCE),
    "left arm"
  );
  assert!(
    normalized
      .region
      .contains(Point2::new(25.0, 15.0), TOLERANCE),
    "right arm"
  );
  assert!(
    !normalized
      .region
      .contains(Point2::new(15.0, 15.0), TOLERANCE),
    "the crossing"
  );
  assert!(
    !normalized
      .region
      .contains(Point2::new(25.0, 25.0), TOLERANCE),
    "outside"
  );
}

#[test]
fn normalization_returns_the_input_unchanged_when_nothing_crosses() {
  let nested = region(vec![
    square(Point2::new(0.0, 0.0), 20.0, 20.0),
    circle(Point2::new(10.0, 10.0), 3.0),
  ]);
  let normalized = normalize(&nested);
  assert_eq!(normalized.region, nested, "vertex for vertex, arc-exact");
  assert!(!normalized.flattened);
}

#[test]
fn normalization_is_deterministic() {
  // Same input, same contours, vertex for vertex — S5 applies to the kernel as much as to the G-code.
  let build = || {
    region(vec![
      square(Point2::new(0.0, 0.0), 10.0, 10.0),
      circle(Point2::new(10.0, 5.0), 4.0),
      square(Point2::new(5.0, 5.0), 10.0, 10.0),
    ])
  };
  let first = normalize(&build());
  let second = normalize(&build());
  assert_eq!(first, second);
  assert_eq!(
    first.region.boundaries.len(),
    second.region.boundaries.len()
  );
  for (index, boundary) in first.region.boundaries.iter().enumerate() {
    let other = &second.region.boundaries[index];
    assert_eq!(boundary.len(), other.len());
    for (vertex, twin) in boundary.vertices().iter().zip(other.vertices()) {
      assert_eq!(vertex.point, twin.point, "vertex positions must not drift");
      assert_eq!(vertex.bulge, twin.bulge);
    }
  }
  assert!(!first.flattened);
}

proptest! {
  /// Normalization must not change what the region means: every point away from the boundaries gets
  /// the same answer before and after (A4). This is the test that would catch a piece classified on
  /// the wrong side of the parity.
  #[test]
  fn normalized_contains_agrees_with_the_original(
    radius in 1.0f64..4.5,
    x in -4.0f64..16.0,
    y in -4.0f64..14.0,
  ) {
    let crossing = region(vec![
      square(Point2::new(0.0, 0.0), 10.0, 10.0),
      circle(Point2::new(10.0, 5.0), radius),
    ]);
    let normalized = normalize(&crossing);
    let point = Point2::new(x, y);
    prop_assume!(away_from_boundaries(&crossing, point));
    prop_assume!(away_from_boundaries(&normalized.region, point));
    prop_assert_eq!(
      normalized.region.contains(point, TOLERANCE),
      crossing.contains(point, TOLERANCE),
      "point {:?} disagrees after normalization",
      point
    );
  }
}
#[test]
fn a_piece_the_bulge_cannot_hold_is_rebuilt_and_reported() {
  // A contour may legally hold an arc that wraps almost a full turn, and such an arc's ends sit a
  // fraction of a millimetre apart. A contour stores an arc as a bulge and rebuilds it from those two
  // ends, so that round trip loses the radius. The arrangement keeps every piece exact whenever it can
  // (A8) and rebuilds it as straight segments when it cannot — and says so (D-RB-5) instead of handing
  // back geometry that quietly changed shape.
  let sweep = std::f64::consts::TAU - 1.0e-8;
  let centre = Point2::new(45.0, 0.0);
  let radius = 10.0;
  let near_full_turn = Contour::new(
    vec![
      Vertex::new(
        Point2::new(centre.x + radius, centre.y),
        (sweep / 4.0).tan(),
      ),
      Vertex::corner(Point2::new(
        centre.x + radius * sweep.cos(),
        centre.y + radius * sweep.sin(),
      )),
    ],
    true,
  )
  .expect("a near full turn boundary is legal");

  // Something has to cross for normalization to run at all: a tab straddling the plate's left edge.
  let plate = square(Point2::new(0.0, 0.0), 30.0, 30.0);
  let tab = square(Point2::new(-5.0, 10.0), 10.0, 10.0);
  let crossing = region(vec![plate, near_full_turn, tab]);
  let normalized = crossing
    .normalize(cam_geom::TolerancePolicy::uniform(1.0e-8))
    .expect("normalization");

  assert!(
    normalized.flattened,
    "the near full turn arc had to be rebuilt"
  );
  assert!(
    normalized
      .region
      .boundaries
      .iter()
      .all(|boundary| !boundary.has_arcs()),
    "the only arc in this input is the one that was rebuilt"
  );
  // Membership is unchanged: the tab's part over the plate is parity 2, the disc is parity 1.
  assert!(normalized.region.contains(
    Point2::new(45.0, 0.0),
    cam_geom::TolerancePolicy::uniform(1.0e-6)
  ));
  assert!(normalized.region.contains(
    Point2::new(20.0, 20.0),
    cam_geom::TolerancePolicy::uniform(1.0e-6)
  ));
  assert!(normalized.region.contains(
    Point2::new(-2.0, 15.0),
    cam_geom::TolerancePolicy::uniform(1.0e-6)
  ));
  assert!(!normalized.region.contains(
    Point2::new(2.0, 15.0),
    cam_geom::TolerancePolicy::uniform(1.0e-6)
  ));
  assert!(!normalized.region.contains(
    Point2::new(32.0, 32.0),
    cam_geom::TolerancePolicy::uniform(1.0e-6)
  ));
  // The rebuilt disc is still a disc to within the tolerance it was rebuilt at.
  let rebuilt_area: f64 = normalized
    .region
    .boundaries
    .iter()
    .map(|boundary| boundary.signed_area().abs())
    .sum();
  assert!(
    (rebuilt_area - (900.0 + std::f64::consts::PI * 100.0)).abs() < 1.0e-3,
    "parity area, got {rebuilt_area}"
  );
}
