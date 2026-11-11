//! Behaviour tests for the region offset and its distance proof (A6), and for raster spans (A7).
//!
//! These tests assert geometry, never internals: where a ring sits, how far it is from the boundary it
//! was offset from, which boundary grew and which shrank, and what a row clipped against the region
//! comes back as. The `RegionOffset` fields are computed from real offsets, because `flattened` and
//! `warnings` are part of what the caller is promised (A8): a caller that is handed a tessellated ring
//! without being told cannot report it to the user, and one that is told about flattening when nothing
//! was flattened learns to ignore the flag.
//!
//! The over-shrink tests are the important ones. `offset_contour` alone answers *well-formed* geometry
//! for a square inset by more than half its own width (D-M4-9), so this suite asserts both sides of
//! that: the naive kernel call returns a ring, and the region offset refuses it by name. A region
//! offset that trusted the kernel would hand the pocket a ring that cuts the wall it was meant to
//! leave standing.
//!
//! The same is true one level finer for an arc (D-RB-10). A lens-shaped piece at a crossing stage has
//! its two crossings as its only vertices, both of them exactly the requested distance away, while the
//! arc between them bulges towards a wall neither crossing is near — so the suite pins both what a
//! vertices-only proof would have kept and what the sound one keeps, and measures the bulge.

use std::f64::consts::PI;

use cam_geom::offset::{offset_contour, OffsetOptions, OffsetWarning};
use cam_geom::primitive;
use cam_geom::region::{Region, RegionOffset};
use cam_geom::{Contour, GeomError, Point2, Segment};
use proptest::prelude::*;

/// One nanometre: the tolerance every test here states explicitly.
const TOLERANCE: f64 = 1.0e-9;

/// The slack the kernel's distance proof allows itself, in millimetres.
///
/// `DISTANCE_SLACK` in `region_offset.rs`: two offset chord tolerances, kept at that value so the
/// region reproduces M4's rings. A distance assertion allows exactly this much and no more.
const SLACK: f64 = 0.002;

fn square(at: Point2, width: f64, height: f64) -> Contour {
  primitive::rectangle(at, width, height).expect("square")
}

fn circle(centre: Point2, radius: f64) -> Contour {
  primitive::circle(centre, radius).expect("circle")
}

fn region(boundaries: Vec<Contour>) -> Region {
  Region::new(boundaries).expect("closed boundaries")
}

fn offset(region: &Region, distance: f64) -> RegionOffset {
  region.offset(distance, TOLERANCE).expect("region offset")
}

/// The shortest distance from `point` to a single boundary.
fn distance_to_contour(contour: &Contour, point: Point2) -> f64 {
  contour
    .segments()
    .iter()
    .map(|segment| segment.distance_to_point(point))
    .fold(f64::INFINITY, f64::min)
}

/// The ring of a result that is `width` millimetres wide.
///
/// Width is how these tests pick a boundary out of a region without depending on the order
/// normalization happened to chain it in: a square, a grown circle and a shrunk circle never share one.
fn ring_of_width(result: &RegionOffset, width: f64) -> &Contour {
  result
    .region
    .boundaries
    .iter()
    .find(|boundary| (boundary.bounds().width() - width).abs() < 1.0e-6)
    .unwrap_or_else(|| {
      panic!(
        "no ring is {width} mm wide, got widths {:?}",
        result
          .region
          .boundaries
          .iter()
          .map(|boundary| boundary.bounds().width())
          .collect::<Vec<_>>()
      )
    })
}

/// True when every vertex of the region is at least `distance - allowed` from `original`.
fn every_vertex_clears(
  result: &RegionOffset,
  original: &Contour,
  distance: f64,
  allowed: f64,
) -> bool {
  result.region.boundaries.iter().all(|boundary| {
    boundary
      .vertices()
      .iter()
      .all(|vertex| distance_to_contour(original, vertex.point) >= distance - allowed)
  })
}

/// The distance from `point` to the nearest of `originals`.
fn nearest_distance(originals: &[&Contour], point: Point2) -> f64 {
  originals
    .iter()
    .map(|contour| distance_to_contour(contour, point))
    .fold(f64::INFINITY, f64::min)
}

/// The points of a piece the distance proof has to cover: every vertex, and the midpoint of every arc.
///
/// An arc's midpoint is the apex of its sagitta, which is exactly where a bulging piece comes closest
/// to something — the one point a vertices-only proof never looks at.
fn proof_points(piece: &Contour) -> Vec<Point2> {
  let mut points: Vec<Point2> = piece.vertices().iter().map(|vertex| vertex.point).collect();
  for segment in piece.segments() {
    if let Segment::Arc(arc) = segment {
      points.push(arc.mid_point());
    }
  }
  points
}

/// Assert that no point the proof has to cover of any surviving piece is closer than
/// `distance - slack` to any of `originals`.
fn assert_no_survivor_is_too_close(
  result: &RegionOffset,
  originals: &[&Contour],
  distance: f64,
  context: &str,
) {
  for boundary in &result.region.boundaries {
    for point in proof_points(boundary) {
      let clearance = nearest_distance(originals, point);
      assert!(
        clearance >= distance - SLACK,
        "{context}: {point:?} is {clearance} mm from the originals, inside {distance}"
      );
    }
  }
}

#[test]
fn a_square_inset_by_the_distance_on_all_four_sides() {
  let original = square(Point2::new(0.0, 0.0), 100.0, 100.0);
  let distance = 10.0;
  let result = region(vec![original.clone()])
    .offset(distance, TOLERANCE)
    .expect("region offset");

  assert_eq!(result.region.boundaries.len(), 1, "one square, one ring");
  assert!(!result.flattened, "nothing here needed rebuilding");
  assert!(result.warnings.is_empty(), "no corner was miter limited");

  let ring = &result.region.boundaries[0];
  assert_eq!(ring.len(), 4, "a square stays four corners");
  let bounds = ring.bounds();
  assert!(
    (bounds.min.x - 10.0).abs() < 1.0e-6,
    "left side at {}, not 10",
    bounds.min.x
  );
  assert!(
    (bounds.min.y - 10.0).abs() < 1.0e-6,
    "bottom at {}, not 10",
    bounds.min.y
  );
  assert!(
    (bounds.max.x - 90.0).abs() < 1.0e-6,
    "right at {}, not 90",
    bounds.max.x
  );
  assert!(
    (bounds.max.y - 90.0).abs() < 1.0e-6,
    "top at {}, not 90",
    bounds.max.y
  );
  for vertex in ring.vertices() {
    let inset = distance_to_contour(&original, vertex.point);
    assert!(
      (inset - distance).abs() < 1.0e-6,
      "vertex {:?} is {inset} mm inside the wall, not {distance}",
      vertex.point
    );
  }
  assert!(
    (ring.signed_area() - 6400.0).abs() < 1.0e-6,
    "an 80 mm square, got {}",
    ring.signed_area()
  );
}

#[test]
fn a_square_with_a_centred_island_follows_both_boundaries() {
  let outer = square(Point2::new(0.0, 0.0), 100.0, 100.0);
  let island = circle(Point2::new(50.0, 50.0), 10.0);
  let distance = 8.0;
  let result = offset(&region(vec![outer.clone(), island.clone()]), distance);

  assert!(!result.flattened, "the offsets keep their arcs and lines");
  assert_eq!(result.region.boundaries.len(), 2, "a ring each");

  // The distance proof, read off the geometry: no vertex of either ring is closer than the distance
  // to *both* original boundaries.
  for boundary in &result.region.boundaries {
    for vertex in boundary.vertices() {
      let from_outer = distance_to_contour(&outer, vertex.point);
      let from_island = distance_to_contour(&island, vertex.point);
      assert!(
        from_outer >= distance - 1.0e-6,
        "vertex {:?} is {from_outer} mm inside the wall, under {distance}",
        vertex.point
      );
      assert!(
        from_island >= distance - 1.0e-6,
        "vertex {:?} is {from_island} mm off the island, under {distance}",
        vertex.point
      );
    }
  }

  // The outer ring is the wall's inset: exactly `distance` on all four sides, 84 mm wide.
  let outer_ring = ring_of_width(&result, 100.0 - 2.0 * distance);
  for vertex in outer_ring.vertices() {
    let inset = distance_to_contour(&outer, vertex.point);
    assert!(
      (inset - distance).abs() < 1.0e-6,
      "outer ring vertex {:?} is {inset} mm inside, not {distance}",
      vertex.point
    );
  }

  // The island's ring is *grown*: radius 18, not 2, and its vertices sit exactly `distance` outside
  // the island they came from.
  let island_ring = ring_of_width(&result, 2.0 * (10.0 + distance));
  for vertex in island_ring.vertices() {
    let from_island = distance_to_contour(&island, vertex.point);
    assert!(
      (from_island - distance).abs() < 1.0e-6,
      "island ring vertex {:?} is {from_island} mm off the island, not {distance}",
      vertex.point
    );
  }
  assert!(
    (island_ring.signed_area() - PI * (10.0 + distance).powi(2)).abs() < 1.0e-6,
    "an 18 mm circle, got {}",
    island_ring.signed_area()
  );

  // And the material answers say the same: the hole really is bigger, and the band between the wall's
  // ring and the island's ring is still material.
  assert!(
    !result.region.contains(Point2::new(62.0, 50.0), TOLERANCE),
    "12 mm from the island centre is now inside the grown hole"
  );
  assert!(
    result.region.contains(Point2::new(80.0, 50.0), TOLERANCE),
    "30 mm from the island centre is still material"
  );
}

#[test]
fn an_odd_depth_boundary_grows_so_the_hole_gets_bigger() {
  let outer = square(Point2::new(0.0, 0.0), 100.0, 100.0);
  let island = circle(Point2::new(50.0, 50.0), 10.0);
  let before = island.signed_area().abs();
  let distance = 5.0;
  let result = offset(&region(vec![outer, island]), distance);

  let hole = ring_of_width(&result, 2.0 * (10.0 + distance));
  let after = hole.signed_area().abs();
  assert!(
    after > before,
    "a hole grows in the offset direction: {before} then {after}"
  );
  assert!(
    (after - PI * 15.0 * 15.0).abs() < 1.0e-6,
    "a 15 mm circle, got {after}"
  );
}

#[test]
fn nested_parity_offsets_a_boss_inside_a_hole_inside_a_square() {
  let outer = square(Point2::new(0.0, 0.0), 100.0, 100.0);
  let hole = circle(Point2::new(50.0, 50.0), 30.0);
  let boss = circle(Point2::new(50.0, 50.0), 10.0);
  let distance = 5.0;
  let result = offset(&region(vec![outer, hole, boss]), distance);

  assert!(!result.flattened);
  assert_eq!(result.region.boundaries.len(), 3, "square, hole and boss");
  // Depth 0 shrinks to 90 mm, depth 1 grows to r = 35, depth 2 shrinks to r = 5: parity decides all
  // three signs, and each one is the opposite of its neighbour.
  assert_eq!(ring_of_width(&result, 100.0 - 2.0 * distance).len(), 4);
  assert!(
    (ring_of_width(&result, 2.0 * (30.0 + distance))
      .signed_area()
      .abs()
      - PI * 35.0 * 35.0)
      .abs()
      < 1.0e-6
  );
  assert!(
    (ring_of_width(&result, 2.0 * (10.0 - distance))
      .signed_area()
      .abs()
      - PI * 5.0 * 5.0)
      .abs()
      < 1.0e-6
  );

  // Parity across the three rings: wall 1, hole 2, boss 3.
  assert!(
    result.region.contains(Point2::new(50.0, 10.0), TOLERANCE),
    "between the wall and the grown hole is material"
  );
  assert!(
    !result.region.contains(Point2::new(50.0, 20.0), TOLERANCE),
    "inside the grown hole is not material"
  );
  assert!(
    result.region.contains(Point2::new(50.0, 53.0), TOLERANCE),
    "the shrunken boss is material again"
  );
}

#[test]
fn an_over_shrink_is_refused_instead_of_gouging_the_wall() {
  let small = square(Point2::new(0.0, 0.0), 3.0, 3.0);
  let distance = 2.0;

  // The kernel alone is happy: a 3 mm square inset by 2 mm comes back as a well-formed 1 mm ring,
  // wound the same way as its parent (D-M4-9). This is the M4 defect, pinned here as evidence.
  let naive = offset_contour(&small, -distance, &OffsetOptions::default())
    .expect("the kernel does not see this gouge");
  assert!(!naive.flattened);
  assert_eq!(naive.contour.len(), 4, "a well-formed square ring");
  assert!(
    (naive.contour.signed_area() - 1.0).abs() < 1.0e-9,
    "a 1 mm square, got {}",
    naive.contour.signed_area()
  );
  for vertex in naive.contour.vertices() {
    let inset = distance_to_contour(&small, vertex.point);
    assert!(
      (inset - 1.0).abs() < 1.0e-9,
      "the naive ring sits 1 mm in, not {distance}: {:?}",
      vertex.point
    );
  }

  // The region offset runs the distance proof and refuses: no material is left at that distance.
  let error = region(vec![small])
    .offset(distance, TOLERANCE)
    .expect_err("the distance proof refuses the escaped ring");
  let GeomError::EditRefused { reason } = error else {
    panic!("refused by name, got {error:?}");
  };
  assert!(
    reason.contains("no material"),
    "the reason names what collapsed: {reason}"
  );
}

#[test]
fn an_offset_wider_than_the_material_is_refused() {
  let outer = square(Point2::new(0.0, 0.0), 100.0, 100.0);
  let island = circle(Point2::new(50.0, 50.0), 10.0);
  let distance = 55.0;

  // Past half the square's width both kernel calls still answer well-formed rings: the wall's offset
  // comes back as a 10 mm square in the middle of the region, each vertex 45 mm from the wall it was
  // supposed to be 55 mm inside.
  let naive = offset_contour(&outer, -distance, &OffsetOptions::default())
    .expect("the kernel does not see this one either");
  assert!(!naive.flattened);
  assert_eq!(naive.contour.len(), 4);
  assert!(
    (naive.contour.signed_area() - 100.0).abs() < 1.0e-9,
    "a 10 mm square, got {}",
    naive.contour.signed_area()
  );
  for vertex in naive.contour.vertices() {
    let inset = distance_to_contour(&outer, vertex.point);
    assert!(
      (inset - 45.0).abs() < 1.0e-9,
      "the escaped ring is 45 mm in, not {distance}: {:?}",
      vertex.point
    );
  }
  assert!(
    offset_contour(&island, distance, &OffsetOptions::default()).is_ok(),
    "the island's own growth is a well-formed ring too"
  );

  // The region proves otherwise: neither piece is `distance` from the boundaries it came from, so
  // nothing is left and the offset is refused rather than returned as a gouging ring.
  let error = region(vec![outer, island])
    .offset(distance, TOLERANCE)
    .expect_err("a 100 mm square cannot lose 110 mm of width");
  let GeomError::EditRefused { reason } = error else {
    panic!("refused by name, got {error:?}");
  };
  assert!(
    reason.contains("no material"),
    "the reason names what collapsed: {reason}"
  );
}

#[test]
fn a_region_whose_boundaries_cross_is_refused() {
  // Two crossing squares describe their parity (symmetric) difference (D-RB-8), so the given
  // boundaries are not the material boundary and the distance proof has nothing honest to measure.
  let crossing = region(vec![
    square(Point2::new(0.0, 0.0), 10.0, 10.0),
    square(Point2::new(5.0, 5.0), 10.0, 10.0),
  ]);
  let error = crossing
    .offset(1.0, TOLERANCE)
    .expect_err("crossing boundaries are refused");
  let GeomError::EditRefused { reason } = error else {
    panic!("refused by name, got {error:?}");
  };
  assert!(
    reason.contains("normalize it first"),
    "the reason names the fix: {reason}"
  );
}

#[test]
fn boundaries_that_share_a_span_with_material_on_the_same_side_are_refused() {
  // Two squares whose bottom and top edges lie on the same lines across a 5 mm span, with their
  // interiors on the same side of it: the material of one lies over the material of the other, so the
  // two boundaries cannot both be the material boundary. This pair used to be read as a touch — a
  // coincident edge was enough — and contributed no nesting depth, so the offset measured a region
  // nobody had described (`odd/tasks/030-winding-collinear-overlap.md` S3).
  let overlapping = region(vec![
    square(Point2::new(0.0, 0.0), 10.0, 10.0),
    square(Point2::new(5.0, 0.0), 10.0, 10.0),
  ]);
  let error = overlapping
    .offset(1.0, TOLERANCE)
    .expect_err("boundaries whose material coincides are refused");
  let GeomError::EditRefused { reason } = error else {
    panic!("refused by name, got {error:?}");
  };
  assert!(
    reason.contains("normalize it first"),
    "the reason names the fix: {reason}"
  );
}

#[test]
fn a_narrow_neck_flattens_the_ring_and_says_so() {
  // A U bracket: a 40 mm base with two 15 mm wide arms rising from its ends. Each arm is a narrow neck
  // of material, and shrinking by 8 mm — more than half the arm's width — makes the two offset walls of
  // an arm cross. That is the one path in the offset kernel that removes a self-intersection by
  // rebuilding the contour as straight segments, and the region reports it (A8, D-RB-5) instead of
  // handing back geometry that quietly changed shape.
  //
  // A neck *between* two lobes cannot be this fixture: the kernel refuses that shape outright while
  // joining the neck's mouth (`DegenerateArc`, consecutive vertices coincide), before the
  // self-intersection removal that flattens anything runs at all.
  let bracket = primitive::polyline(
    &[
      Point2::new(0.0, 0.0),
      Point2::new(100.0, 0.0),
      Point2::new(100.0, 100.0),
      Point2::new(85.0, 100.0),
      Point2::new(85.0, 40.0),
      Point2::new(15.0, 40.0),
      Point2::new(15.0, 100.0),
      Point2::new(0.0, 100.0),
    ],
    true,
  )
  .expect("a U bracket outline");
  let distance = 8.0;
  let result = offset(&region(vec![bracket.clone()]), distance);

  assert!(result.flattened, "the arms forced a rebuild");
  assert!(
    result.warnings.contains(&OffsetWarning::Flattened),
    "and the caller is told: {:?}",
    result.warnings
  );
  assert!(
    every_vertex_clears(&result, &bracket, distance, 0.002 + 1.0e-9),
    "the surviving ring still clears the distance proof"
  );
  assert_eq!(result.region.boundaries.len(), 1, "the base survives alone");
  assert!(
    (result.region.boundaries[0].signed_area() - 2016.0).abs() < 1.0e-6,
    "the base's inset, 84 mm by 24 mm, got {}",
    result.region.boundaries[0].signed_area()
  );
  assert!(
    result.region.contains(Point2::new(50.0, 20.0), TOLERANCE),
    "the middle of the base is still material"
  );
  assert!(
    !result.region.contains(Point2::new(50.0, 36.0), TOLERANCE),
    "4 mm below the notch floor is not reachable by a tool centre 8 mm from it"
  );
  assert!(
    !result.region.contains(Point2::new(7.0, 70.0), TOLERANCE),
    "the arms are gone: they were narrower than twice the distance"
  );
}

#[test]
fn a_row_crossing_the_island_comes_back_as_two_spans() {
  let outer = square(Point2::new(0.0, 0.0), 100.0, 100.0);
  let centre = Point2::new(50.0, 50.0);
  let radius = 10.0;
  let material = region(vec![outer, circle(centre, radius)]);

  // 3 mm above the centre: the island's x's are the analytic circle's, 50 ± sqrt(100 - 9).
  let y = 53.0;
  let half = (radius * radius - (y - centre.y) * (y - centre.y)).sqrt();
  let spans = material
    .raster_spans(y, TOLERANCE)
    .expect("a row that crosses cleanly");

  assert_eq!(
    spans.len(),
    2,
    "the island splits the row in two: {spans:?}"
  );
  assert!(
    (spans[0].0 - 0.0).abs() < 1.0e-6,
    "from the left wall: {:?}",
    spans[0]
  );
  assert!(
    (spans[0].1 - (centre.x - half)).abs() < 1.0e-6,
    "to the island's left x {}: {:?}",
    centre.x - half,
    spans[0]
  );
  assert!(
    (spans[1].0 - (centre.x + half)).abs() < 1.0e-6,
    "from the island's right x {}: {:?}",
    centre.x + half,
    spans[1]
  );
  assert!(
    (spans[1].1 - 100.0).abs() < 1.0e-6,
    "to the right wall: {:?}",
    spans[1]
  );
}

#[test]
fn a_row_tangent_to_the_island_is_ambiguous() {
  let outer = square(Point2::new(0.0, 0.0), 100.0, 100.0);
  let island = circle(Point2::new(50.0, 50.0), 10.0);
  let material = region(vec![outer, island]);

  // y = 60 touches the island at exactly one x, which cannot be paired: the caller retries.
  assert_eq!(material.raster_spans(60.0, TOLERANCE), None);
  // A micrometre lower the row crosses properly and comes back as two spans.
  let just_below = material
    .raster_spans(60.0 - 1.0e-6, TOLERANCE)
    .expect("a row just below the tangent crosses twice");
  assert_eq!(just_below.len(), 2);
}

#[test]
fn a_row_along_a_straight_boundary_is_ambiguous() {
  // The square's own bottom edge: the row lies along it, which is a coincidence, not a crossing.
  let material = region(vec![square(Point2::new(0.0, 0.0), 100.0, 100.0)]);
  assert_eq!(material.raster_spans(0.0, TOLERANCE), None);
}

#[test]
fn a_row_with_an_unusable_tolerance_is_ambiguous() {
  let material = region(vec![square(Point2::new(0.0, 0.0), 100.0, 100.0)]);
  assert_eq!(
    material.raster_spans(37.0, 0.0),
    None,
    "nothing to dedupe with"
  );
  assert_eq!(
    material.raster_spans(37.0, -1.0),
    None,
    "a negative tolerance"
  );
  assert_eq!(
    material.raster_spans(f64::NAN, TOLERANCE),
    None,
    "a NaN row"
  );
  assert!(
    material.raster_spans(37.0, TOLERANCE).is_some(),
    "a usable tolerance still clips the row"
  );
}

#[test]
fn a_region_without_islands_reproduces_the_single_contour_spans() {
  let material = region(vec![square(Point2::new(0.0, 0.0), 100.0, 100.0)]);

  let spans = material
    .raster_spans(37.0, TOLERANCE)
    .expect("a row across the square");
  assert_eq!(spans.len(), 1, "one span, as M4's single contour gave");
  assert!(
    (spans[0].0 - 0.0).abs() < 1.0e-6,
    "left edge: {:?}",
    spans[0]
  );
  assert!(
    (spans[0].1 - 100.0).abs() < 1.0e-6,
    "right edge: {:?}",
    spans[0]
  );

  // A row above the region is not ambiguous, it simply has no spans.
  assert_eq!(material.raster_spans(137.0, TOLERANCE), Some(Vec::new()));
}

#[test]
fn a_zero_distance_offset_changes_nothing() {
  // Distance 0 is the identity, so a ring loop that starts at inset 0 is not a special case.
  let material = region(vec![
    square(Point2::new(0.0, 0.0), 100.0, 100.0),
    circle(Point2::new(50.0, 50.0), 10.0),
  ]);
  let result = offset(&material, 0.0);
  assert!(!result.flattened);
  assert!(result.warnings.is_empty());
  assert_eq!(result.region, material);
  assert_eq!(
    result.region.raster_spans(53.0, TOLERANCE),
    material.raster_spans(53.0, TOLERANCE)
  );
}

#[test]
fn a_bulging_piece_that_only_its_vertices_could_pass_is_dropped() {
  // D-RB-10's fixture. A 100 mm square pocket with a centred r10 island, offset by 23 mm: the wall's
  // inset is the 23..77 square, the island's growth is the r33 disc around (50, 50), and the two
  // cross. Between the crossings the grown island leaves the shrunken wall, so parity claims the
  // disc's four lunes as material — and it is the distance proof, not parity, that has to refuse
  // them.
  let outer = square(Point2::new(0.0, 0.0), 100.0, 100.0);
  let island = circle(Point2::new(50.0, 50.0), 10.0);
  let distance = 23.0;

  // Reproduce what the proof used to see: the two offsets normalized, with no distance proof at all.
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
  let originals: [&Contour; 2] = [&outer, &island];

  // The vertices-only rule keeps six of the eight pieces: the four corner slivers, which really are
  // 23 mm from both originals, and two lenses whose only vertices are their crossings.
  let vertex_clean = naive
    .boundaries
    .iter()
    .filter(|piece| {
      piece
        .vertices()
        .iter()
        .all(|vertex| nearest_distance(&originals, vertex.point) >= distance - SLACK)
    })
    .count();
  assert_eq!(
    vertex_clean, 6,
    "the vertices-only proof keeps the two lenses as well as the four corners"
  );

  // One of them, measured. A lens has two vertices — its crossings — and one arc of the island's
  // growth; the arc dips towards the wall's inset line and comes 6 mm inside the requested distance.
  let lens = naive
    .boundaries
    .iter()
    .find(|piece| piece.len() == 2)
    .expect("the crossing stage leaves a lens");
  let arc = lens
    .segments()
    .into_iter()
    .find_map(|segment| segment.as_arc())
    .expect("a lens has an arc");
  for vertex in lens.vertices() {
    let from_wall = distance_to_contour(&outer, vertex.point);
    let from_island = distance_to_contour(&island, vertex.point);
    assert!(
      (from_wall - distance).abs() < 1.0e-9 && (from_island - distance).abs() < 1.0e-9,
      "the crossings sit exactly {distance} from both originals, got {from_wall} and {from_island}"
    );
  }
  let apex = arc.mid_point();
  let from_wall = distance_to_contour(&outer, apex);
  let from_island = distance_to_contour(&island, apex);
  assert!(
    (apex.y - 17.0).abs() < 1.0e-6,
    "the arc's lowest point is at y = 17, not {}",
    apex.y
  );
  assert!(
    (from_wall - 17.0).abs() < 1.0e-6,
    "the apex is 17 mm from the wall, not {from_wall}"
  );
  assert!(
    (distance - from_wall - 6.0).abs() < 1.0e-6,
    "the bulge is 6 mm inside the requested distance: {} vs {distance}",
    from_wall
  );
  assert!(
    (from_island - distance).abs() < 1.0e-6,
    "and it is still exactly {distance} from the island it grew from, got {from_island}"
  );
  assert!(
    from_wall < distance - SLACK,
    "a vertex-only proof cannot see this bulge"
  );

  // The sound proof refuses that piece and keeps the four corner slivers, which are 23 mm from both
  // originals everywhere — the arc included.
  let result = offset(&region(vec![outer.clone(), island.clone()]), distance);
  assert_eq!(
    result.region.boundaries.len(),
    4,
    "the four corners, and neither lens: got {:?}",
    result
      .region
      .boundaries
      .iter()
      .map(|piece| piece.bounds())
      .collect::<Vec<_>>()
  );
  for piece in &result.region.boundaries {
    assert_eq!(piece.len(), 3, "a corner sliver is three vertices");
    assert_eq!(
      piece
        .segments()
        .iter()
        .filter(|s| s.as_arc().is_some())
        .count(),
      1,
      "and one arc of the island's growth"
    );
  }
  assert_no_survivor_is_too_close(&result, &originals, distance, "at 23 mm");

  // One step further the wall's own inset (23 mm of a 100 mm square is at 23..77, i.e. 23 mm from
  // the wall rather than 27) is inside the requested distance, so even the corners go and the whole
  // offset is refused instead of returning a ring that gouges.
  let error = region(vec![outer, island])
    .offset(27.0, TOLERANCE)
    .expect_err("at 27 mm nothing is far enough from both originals");
  let GeomError::EditRefused { reason } = error else {
    panic!("refused by name, got {error:?}");
  };
  assert!(
    reason.contains("no material"),
    "the reason names what collapsed: {reason}"
  );
}

proptest! {
  #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

  /// Every point a surviving piece's claim covers — its vertices and its arcs' apexes — is at least
  /// `distance - slack` from every original boundary, at every distance the fixture has material for.
  ///
  /// This is the property D-RB-10 exists for. It fails for the vertices-only rule at 23 mm, where the
  /// lens above is kept with an apex 6 mm inside the distance.
  #[test]
  fn no_surviving_piece_comes_closer_than_the_distance_it_claims(distance in 0.5_f64..=23.0) {
    let outer = square(Point2::new(0.0, 0.0), 100.0, 100.0);
    let island = circle(Point2::new(50.0, 50.0), 10.0);
    let originals: [&Contour; 2] = [&outer, &island];
    let result = region(vec![outer.clone(), island.clone()])
      .offset(distance, TOLERANCE);
    // One distance in this range is not a geometry question: at exactly 20 mm the grown island
    // (r30) is tangent to all four wall insets (at 30 from the wall), and the arrangement refuses
    // that degeneracy by name rather than guessing which way round it goes (K3's recorded limit).
    // The property is about the pieces that *survive*, and a refusal has none.
    prop_assume!(result.is_ok());
    let result = result.expect("just checked");
    for boundary in &result.region.boundaries {
      for point in proof_points(boundary) {
        let clearance = nearest_distance(&originals, point);
        prop_assert!(
          clearance >= distance - SLACK,
          "at {} mm: {:?} is {} mm from the originals",
          distance,
          point,
          clearance
        );
      }
    }

  }

  /// The same property on a three-boundary region, where a boss sits inside a hole: the signs flip at
  /// every depth, so the proof runs in both directions at once. The boss is r10 and shrinks, so the
  /// distances stop at 9 mm — beyond that the boss itself is gone and the offset is refused.
  #[test]
  fn no_surviving_piece_of_a_nested_region_comes_closer_than_its_distance(distance in 0.5_f64..=9.0) {
    let outer = square(Point2::new(0.0, 0.0), 100.0, 100.0);
    let hole = circle(Point2::new(50.0, 50.0), 30.0);
    let boss = circle(Point2::new(50.0, 50.0), 10.0);
    let originals: [&Contour; 3] = [&outer, &hole, &boss];
    let result = region(vec![outer.clone(), hole.clone(), boss.clone()])
      .offset(distance, TOLERANCE)
      .expect("square, hole and boss all survive these distances");
    for boundary in &result.region.boundaries {
      for point in proof_points(boundary) {
        let clearance = nearest_distance(&originals, point);
        prop_assert!(
          clearance >= distance - SLACK,
          "at {} mm: {:?} is {} mm from the originals",
          distance,
          point,
          clearance
        );
      }
    }
  }
}
