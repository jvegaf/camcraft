//! Differential and regression tests for the indexed boolean acceleration (task 061 T2).

use std::f64::consts::PI;

use cam_geom::boolean::{combine, BooleanOp, Combined, RegionIndex};
use cam_geom::region::Region;
use cam_geom::{Arc, Contour, Point2, Segment, TolerancePolicy};

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

fn circle(center: Point2, radius: f64) -> Contour {
  // Two semicircles forming a closed CCW contour.
  let top = Segment::Arc(Arc::new(center, radius, 0.0, PI).expect("top semicircle"));
  let bottom = Segment::Arc(Arc::new(center, radius, PI, PI).expect("bottom semicircle"));
  Contour::from_segments(&[top, bottom], true).expect("circle contour")
}

/// Simple deterministic RNG (xorshift64) so the sample is reproducible without extra deps.
#[derive(Clone)]
struct Rng(u64);

impl Rng {
  fn new(seed: u64) -> Self {
    Self(seed)
  }

  fn next_u64(&mut self) -> u64 {
    let mut x = self.0;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    self.0 = x;
    x
  }

  fn f64(&mut self) -> f64 {
    // in [0, 1)
    (self.next_u64() as f64) / (u64::MAX as f64 + 1.0)
  }

  fn range(&mut self, lo: f64, hi: f64) -> f64 {
    lo + (hi - lo) * self.f64()
  }
}

fn assert_indexed_equals_contains(
  region: &Region,
  point: Point2,
  tolerance: TolerancePolicy,
  ctx: &str,
) {
  let expected = region.contains(point, tolerance);
  let index = RegionIndex::build(region, tolerance);
  let got = index.contains(point, tolerance);
  assert_eq!(
    got, expected,
    "indexed membership differs from Region::contains at ({}, {}) [{}]",
    point.x, point.y, ctx
  );
}

#[test]
fn indexed_membership_equals_region_contains_for_many_points_and_regions() {
  let tolerance = TolerancePolicy::uniform(1.0e-6);
  let mut rng = Rng::new(0x061E14_C0A5EED);
  let mut comparisons: usize = 0;

  // Region 1: single rectangle.
  let r1 = Region::new(vec![rect(0.0, 0.0, 10.0, 6.0)]).expect("r1");
  // Region 2: rectangle with a rectangular hole.
  let r2 = Region::new(vec![rect(-5.0, -5.0, 15.0, 15.0), rect(2.0, 2.0, 8.0, 8.0)])
    .expect("r2 with hole");
  // Region 3: circular boundary (arcs).
  let r3 = Region::new(vec![circle(Point2::new(20.0, 20.0), 5.0)]).expect("r3 circle");
  // Region 4: rectangle with a circular hole (arc hole boundary).
  let outer = rect(30.0, 0.0, 50.0, 20.0);
  let hole = circle(Point2::new(40.0, 10.0), 3.0);
  let r4 = Region::new(vec![outer, hole]).expect("r4 rect with circular hole");
  // Region 5: several disjoint boundaries.
  let r5 = Region::new(vec![
    rect(60.0, 0.0, 62.0, 2.0),
    rect(65.0, 5.0, 70.0, 9.0),
    circle(Point2::new(63.0, 12.0), 1.5),
  ])
  .expect("r5 disjoint");

  type NamedRegion = (&'static str, Region, (f64, f64, f64, f64));
  let regions: Vec<NamedRegion> = vec![
    ("rect", r1, (-2.0, -2.0, 12.0, 8.0)),
    ("rect_hole", r2.clone(), (-7.0, -7.0, 17.0, 17.0)),
    ("circle", r3, (13.0, 13.0, 27.0, 27.0)),
    ("rect_circular_hole", r4.clone(), (28.0, -2.0, 52.0, 22.0)),
    ("disjoint", r5, (58.0, -2.0, 72.0, 15.0)),
  ];

  for (name, region, (min_x, min_y, max_x, max_y)) in &regions {
    let index = RegionIndex::build(region, tolerance);

    // Random points over and around the region.
    for _ in 0..2000 {
      let p = Point2::new(rng.range(*min_x, *max_x), rng.range(*min_y, *max_y));
      let expected = region.contains(p, tolerance);
      let got = index.contains(p, tolerance);
      assert_eq!(got, expected, "{name}: random ({}, {})", p.x, p.y);
      comparisons += 1;
    }

    // Points on and near boundaries (sample boundary segments).
    for boundary in &region.boundaries {
      for segment in boundary.segments() {
        for &t in &[0.0, 0.25, 0.5, 0.75, 1.0] {
          let on = match segment {
            Segment::Line(line) => line.point_at(t),
            Segment::Arc(arc) => arc.point_at_fraction(t),
          };
          // Exactly on boundary (within representable precision of parameterisation).
          let expected_on = region.contains(on, tolerance);
          let got_on = index.contains(on, tolerance);
          assert_eq!(
            got_on, expected_on,
            "{name}: on-boundary at t={t} ({}, {})",
            on.x, on.y
          );
          comparisons += 1;

          // Near boundary: offset by small distances along an approximate normal.
          let (tan_x, tan_y) = match segment {
            Segment::Line(line) => {
              let d = line.end - line.start;
              let len = d.length().max(1.0e-12);
              (d.x / len, d.y / len)
            }
            Segment::Arc(arc) => {
              let p = arc.point_at_fraction(t);
              let r = (p - arc.center())
                .normalized()
                .unwrap_or(cam_geom::Vec2::X_AXIS);
              // tangent is perpendicular to radius; direction depends on sweep sign
              if arc.is_ccw() {
                (-r.y, r.x)
              } else {
                (r.y, -r.x)
              }
            }
          };
          let nx = -tan_y;
          let ny = tan_x;
          for &d in &[
            tolerance.epsilon * 0.25,
            tolerance.epsilon * 0.9,
            tolerance.epsilon * 2.0,
            0.01,
          ] {
            for &sign in &[-1.0, 1.0] {
              let p = Point2::new(on.x + nx * d * sign, on.y + ny * d * sign);
              let expected = region.contains(p, tolerance);
              let got = index.contains(p, tolerance);
              assert_eq!(
                got, expected,
                "{name}: near-boundary d={d} sign={sign} at ({}, {})",
                p.x, p.y
              );
              comparisons += 1;
            }
          }
        }
      }
    }
  }

  // Explicit cases: inside holes must be outside material.
  // r2 has a hole at [2,8]x[2,8]
  let in_hole = Point2::new(5.0, 5.0);
  assert!(!r2.contains(in_hole, tolerance));
  assert_indexed_equals_contains(&r2, in_hole, tolerance, "inside rectangular hole");
  comparisons += 1;
  // r4 has a circular hole centered (40,10) r=3
  let in_circular_hole = Point2::new(40.0, 10.0);
  assert!(!r4.contains(in_circular_hole, tolerance));
  assert_indexed_equals_contains(&r4, in_circular_hole, tolerance, "inside circular hole");
  comparisons += 1;

  // Sanity: ensure we actually did "many" comparisons.
  assert!(
    comparisons >= 10_000,
    "expected at least 10000 comparisons, got {comparisons}"
  );
  eprintln!("indexed_membership_equals_region_contains: {comparisons} point comparisons");
}

#[test]
fn combine_regions_with_far_apart_bounding_boxes() {
  // Regression for the cut phase: a spatial filter must not skip pairs just because regions are
  // far apart overall; disjoint distant regions must still combine correctly.
  let tolerance = TolerancePolicy::uniform(1.0e-6);
  let a = Region::new(vec![rect(0.0, 0.0, 10.0, 10.0)]).expect("a");
  let b = Region::new(vec![rect(1000.0, 1000.0, 1010.0, 1010.0)]).expect("b");

  let result = combine(&a, &b, BooleanOp::Union, tolerance).expect("union far apart");
  let region = match result {
    Combined::Region { region, .. } => region,
    Combined::Empty => panic!("union of two disjoint non-empty regions must not be empty"),
  };
  assert_eq!(
    region.boundaries.len(),
    2,
    "two far-apart squares should produce two boundaries"
  );

  // Points inside each original region must be inside the union.
  assert!(region.contains(Point2::new(5.0, 5.0), tolerance));
  assert!(region.contains(Point2::new(1005.0, 1005.0), tolerance));
  // A point between them must be outside.
  assert!(!region.contains(Point2::new(500.0, 500.0), tolerance));

  // Intersection of far-apart regions is empty.
  let inter = combine(&a, &b, BooleanOp::Intersection, tolerance).expect("intersection far apart");
  assert!(
    matches!(inter, Combined::Empty),
    "far-apart regions have empty intersection"
  );

  // Difference A \ B equals A.
  let diff = combine(&a, &b, BooleanOp::Difference, tolerance).expect("difference far apart");
  let diff_region = match diff {
    Combined::Region { region, .. } => region,
    Combined::Empty => panic!("A minus far-away B must equal A"),
  };
  assert!(diff_region.contains(Point2::new(5.0, 5.0), tolerance));
  assert!(!diff_region.contains(Point2::new(1005.0, 1005.0), tolerance));
}
