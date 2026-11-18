//! Regression for boolean acceleration paths (task 061 T4).
//!
//! Exercises the indexed membership candidate grid and the spatial vertex/dedup
//! paths in `combine` on a deterministic arrangement with many disjoint
//! boundaries. The result's geometry (boundary count and total material area)
//! must remain identical to the set-theoretic expectation.

use cam_geom::boolean::{combine, BooleanOp, Combined};
use cam_geom::region::Region;
use cam_geom::{Contour, Point2, TolerancePolicy};

const TOLERANCE: TolerancePolicy = TolerancePolicy::uniform(1.0e-9);

fn rect(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Contour {
  Contour::from_points(
    &[
      Point2::new(min_x, min_y),
      Point2::new(max_x, min_y),
      Point2::new(max_x, max_y),
      Point2::new(min_x, max_y),
    ],
    true,
  )
  .expect("rectangle contour")
}

fn grid_region(cols: i32, rows: i32, size: f64, gap: f64, ox: f64, oy: f64) -> Region {
  let mut boundaries = Vec::new();
  for iy in 0..rows {
    for ix in 0..cols {
      let x = ox + ix as f64 * (size + gap);
      let y = oy + iy as f64 * (size + gap);
      boundaries.push(rect(x, y, x + size, y + size));
    }
  }
  Region::new(boundaries).expect("disjoint grid region")
}

fn material_area(region: &Region) -> f64 {
  let signed: f64 = region
    .boundaries
    .iter()
    .flat_map(|boundary| boundary.segments())
    .map(|segment| segment.area_contribution())
    .sum();
  signed.abs()
}

fn close(actual: f64, expected: f64) -> bool {
  let scale = actual.abs().max(expected.abs()).max(1.0);
  (actual - expected).abs() <= 1.0e-9 * scale
}

#[test]
fn many_disjoint_boundaries_union_preserves_geometry() {
  // Two interleaved 8x8 grids of disjoint squares: stresses boundary-candidate
  // filtering in membership probes and vertex/dedup handling across many
  // independent contours.
  let a = grid_region(8, 8, 0.8, 1.2, 0.0, 0.0);
  let b = grid_region(8, 8, 0.8, 1.2, 0.9, 0.0);
  let expected_area = 64.0 * 0.64 * 2.0;

  let combined = match combine(&a, &b, BooleanOp::Union, TOLERANCE).expect("union") {
    Combined::Region { region, .. } => region,
    Combined::Empty => panic!("union of material must not be empty"),
  };

  assert_eq!(
    combined.boundaries.len(),
    128,
    "each disjoint square remains an independent boundary"
  );
  let got_area = material_area(&combined);
  assert!(
    close(got_area, expected_area),
    "total area must be preserved: got {got_area}, expected {expected_area}"
  );

  // Indexed membership must classify representative points correctly across the
  // spread-out boundary set (candidate grid must not miss any boundary).
  let inside_a = Point2::new(0.4, 0.4);
  let inside_b = Point2::new(1.3, 0.4);
  let outside = Point2::new(0.85, 0.4);
  assert!(
    combined.contains(inside_a, TOLERANCE),
    "inside a square of a"
  );
  assert!(
    combined.contains(inside_b, TOLERANCE),
    "inside a square of b"
  );
  assert!(
    !combined.contains(outside, TOLERANCE),
    "gap between interleaved squares is empty"
  );
}

#[test]
fn subtracting_many_holes_preserves_geometry() {
  let outer = Region::new(vec![rect(-1.0, -1.0, 17.0, 17.0)]).expect("outer");
  let holes = grid_region(8, 8, 0.8, 1.2, 0.0, 0.0);
  let outer_area = 18.0 * 18.0;
  let hole_area = 64.0 * 0.64;

  let combined =
    match combine(&outer, &holes, BooleanOp::Difference, TOLERANCE).expect("difference") {
      Combined::Region { region, .. } => region,
      Combined::Empty => panic!("difference must leave material"),
    };

  // One outer boundary plus one boundary per hole.
  assert_eq!(combined.boundaries.len(), 65);
  let got_area = material_area(&combined);
  assert!(
    close(got_area, outer_area - hole_area),
    "material area must subtract holes: got {got_area}, expected {}",
    outer_area - hole_area
  );
  assert!(
    !combined.contains(Point2::new(0.4, 0.4), TOLERANCE),
    "inside a hole"
  );
  assert!(
    combined.contains(Point2::new(1.5, 1.5), TOLERANCE),
    "material between holes"
  );
}
