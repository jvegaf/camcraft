//! What the verified axis is actually worth, measured rather than assumed (M7 T2, A2, D-M7-5).
//!
//! The milestone's `uncut` is going to be made of these two numbers, so they are measured here and
//! printed, not guessed: the fraction of the region's area that the axis's inscribed balls cover, and
//! the deepest point they miss. Two tip sets are reported for every fixture, because they answer two
//! different questions:
//!
//! * **vertices** — the tips the program actually feeds to (T4 emits one `Move::Feed` per vertex), so
//!   this is the coverage a chorded walk buys, and the gap between consecutive vertices is the
//!   chording error D-M7-5's tolerance names;
//! * **axis** — the same polyline sampled at the measurement step, which approximates the true axis.
//!   The difference between the two columns is therefore exactly what the chording costs, and the
//!   residual in the `axis` column is what the lattice's own resolution costs.

use std::collections::BTreeMap;

use cam_geom::medial::{medial_axis, MedialAxis};
use cam_geom::primitive;
use cam_geom::region::Region;
use cam_geom::{Contour, Point2};

/// The measurement resolution, in millimetres. Independent of the axis tolerance: it decides how
/// honestly the region is sampled, and nothing else.
const STEP: f64 = 0.1;

/// The two tolerances the milestone is measured at (D-M7-5).
const TOLERANCES: [f64; 2] = [0.5, 0.1];

fn point(x: f64, y: f64) -> Point2 {
  Point2::new(x, y)
}

fn region(boundaries: Vec<Contour>) -> Region {
  Region::new(boundaries).expect("closed boundaries")
}

fn circle_contour(radius: f64) -> Contour {
  primitive::circle(Point2::ORIGIN, radius).expect("circle")
}

fn fixtures() -> Vec<(&'static str, Region)> {
  let square = region(vec![
    primitive::rectangle(point(0.0, 0.0), 10.0, 10.0).expect("square")
  ]);
  let disc = region(vec![circle_contour(10.0)]);
  let rectangle = region(vec![
    primitive::rectangle(point(0.0, 0.0), 20.0, 6.0).expect("rectangle")
  ]);
  let l_shape = region(vec![primitive::polyline(
    &[
      point(0.0, 0.0),
      point(10.0, 0.0),
      point(10.0, 4.0),
      point(4.0, 4.0),
      point(4.0, 10.0),
      point(0.0, 10.0),
    ],
    true,
  )
  .expect("L")]);
  let annulus = region(vec![circle_contour(10.0), circle_contour(4.0)]);
  let ellipse = region(vec![
    primitive::ellipse(Point2::ORIGIN, 5.0, 2.0, 0.01)
      .expect("ellipse")
      .contour,
  ]);
  vec![
    ("square 10x10", square),
    ("disc r10", disc),
    ("rectangle 20x6", rectangle),
    ("L 10x10", l_shape),
    ("annulus 4/10", annulus),
    ("ellipse 5x2", ellipse),
  ]
}

/// One fixture at one tolerance.
struct Measurement {
  /// How many branches the axis is made of: one retract to safe Z each (D-M4-6).
  branches: usize,
  vertices: usize,
  /// Area-weighted fraction covered by the balls at the branch vertices.
  vertex_coverage: f64,
  /// Area-weighted fraction covered by the balls along the whole polyline.
  axis_coverage: f64,
  /// The largest clearance among samples the vertex tips miss.
  vertex_missing_clearance: f64,
  /// The largest clearance among samples the dense tips miss.
  axis_missing_clearance: f64,
  missing_area: f64,
  total_area: f64,
}

/// A tip and its clearance: the centre of an inscribed ball and its radius.
type Tip = (Point2, f64);

/// A set of tips bucketed by a grid of side `cell`, so a query only looks near itself.
struct Tips {
  cell: f64,
  buckets: BTreeMap<(i64, i64), Vec<Tip>>,
}

impl Tips {
  fn new(tips: Vec<Tip>) -> Self {
    let widest = tips
      .iter()
      .map(|(_, clearance)| *clearance)
      .fold(0.0_f64, f64::max)
      .max(1.0e-6);
    let mut buckets: BTreeMap<(i64, i64), Vec<Tip>> = BTreeMap::new();
    for (p, clearance) in tips {
      let key = ((p.x / widest).floor() as i64, (p.y / widest).floor() as i64);
      buckets.entry(key).or_default().push((p, clearance));
    }
    Self {
      cell: widest,
      buckets,
    }
  }

  /// Whether some tip's inscribed ball holds `query`.
  fn covers(&self, query: Point2) -> bool {
    let key = (
      (query.x / self.cell).floor() as i64,
      (query.y / self.cell).floor() as i64,
    );
    for i in -1..=1 {
      for j in -1..=1 {
        let Some(bucket) = self.buckets.get(&(key.0 + i, key.1 + j)) else {
          continue;
        };
        for (p, clearance) in bucket {
          if query.distance_to(*p) <= *clearance {
            return true;
          }
        }
      }
    }
    false
  }
}

/// The tips at the branch vertices, and the same polyline sampled at `STEP`.
fn tips(region: &Region, axis: &MedialAxis) -> (Vec<Tip>, Vec<Tip>) {
  let mut vertices = Vec::new();
  let mut dense = Vec::new();
  for branch in &axis.branches {
    for vertex in &branch.vertices {
      vertices.push((vertex.point, vertex.clearance));
    }
    let count = branch.vertices.len();
    let spans = if branch.closed || count == 1 {
      count
    } else {
      count.saturating_sub(1)
    };
    for index in 0..spans {
      let from = branch.vertices[index].point;
      let to = branch.vertices[(index + 1) % count].point;
      let pieces = (from.distance_to(to) / STEP).ceil().max(1.0) as usize;
      for piece in 0..=pieces {
        let p = from.lerp(to, piece as f64 / pieces as f64);
        let clearance = region
          .distance_to_boundary(p, 0.0)
          .expect("the fixture has boundaries")
          .distance;
        dense.push((p, clearance));
      }
    }
  }
  (vertices, dense)
}

fn measure(region: &Region, tolerance: f64) -> Measurement {
  let axis = medial_axis(region, tolerance).expect("axis");
  let (vertex_tips, dense_tips) = tips(region, &axis);
  let vertices = vertex_tips.len();
  let vertex_tips = Tips::new(vertex_tips);
  let dense_tips = Tips::new(dense_tips);

  let bounds = region
    .boundaries
    .iter()
    .fold(cam_geom::Bounds::empty(), |bounds, boundary| {
      boundary
        .segments()
        .iter()
        .fold(bounds, |bounds, segment| bounds.union(segment.bounds()))
    });

  let mut total_area = 0.0;
  let mut vertex_area = 0.0;
  let mut axis_area = 0.0;
  let mut missing_area = 0.0;
  let mut vertex_missing_clearance: f64 = 0.0;
  let mut axis_missing_clearance: f64 = 0.0;
  let cell = STEP * STEP;

  let cols = (bounds.width() / STEP).ceil() as usize + 1;
  let rows = (bounds.height() / STEP).ceil() as usize + 1;
  for row in 0..rows {
    for col in 0..cols {
      let query = point(
        bounds.min.x + col as f64 * STEP,
        bounds.min.y + row as f64 * STEP,
      );
      let Some(answer) = region.distance_to_boundary(query, 0.0) else {
        continue;
      };
      if !answer.inside {
        continue;
      }
      total_area += cell;
      if vertex_tips.covers(query) {
        vertex_area += cell;
      } else {
        vertex_missing_clearance = vertex_missing_clearance.max(answer.distance);
      }
      if dense_tips.covers(query) {
        axis_area += cell;
      } else {
        axis_missing_clearance = axis_missing_clearance.max(answer.distance);
        missing_area += cell;
      }
    }
  }

  Measurement {
    branches: axis.branches.len(),
    vertices,
    vertex_coverage: vertex_area / total_area,
    axis_coverage: axis_area / total_area,
    vertex_missing_clearance,
    axis_missing_clearance,
    missing_area,
    total_area,
  }
}

#[test]
fn the_axis_balls_cover_the_region_at_both_tolerances() {
  println!(
    "{:15} {:>5} {:>8} {:>7} {:>9} {:>9} {:>10} {:>10} {:>9}",
    "fixture", "tol", "branches", "verts", "vertices", "axis", "miss(v)", "miss(a)", "area"
  );
  for (name, region) in fixtures() {
    for tolerance in TOLERANCES {
      let m = measure(&region, tolerance);
      println!(
        "{:15} {:5.2} {:8} {:7} {:9.5} {:9.5} {:10.5} {:10.5} {:9.3}",
        name,
        tolerance,
        m.branches,
        m.vertices,
        m.vertex_coverage,
        m.axis_coverage,
        m.vertex_missing_clearance,
        m.axis_missing_clearance,
        m.total_area
      );
      // The inscribed balls of the axis cover the region: this is the claim the V-carve rests on,
      // and a coarse lattice may cost a little of it near the tips, never most of it.
      assert!(
        m.axis_coverage >= 0.98,
        "{name} at {tolerance}: the axis's balls cover {:.4} of the area, missing {:.4} mm2",
        m.axis_coverage,
        m.missing_area
      );
      assert!(
        m.vertex_coverage >= 0.95,
        "{name} at {tolerance}: the vertex tips cover {:.4} of the area",
        m.vertex_coverage
      );
      // What is missed is shallow: a tip's ball is a whole ball, so a hole in the cover is at most as
      // deep as the chording step allows, and it is the depth the simulation will report as uncut.
      assert!(
        m.axis_missing_clearance <= tolerance,
        "{name} at {tolerance}: a missed sample is {:.4} mm deep, deeper than the tolerance",
        m.axis_missing_clearance
      );
    }
  }
}

/// True when a finer walk cannot help because the axis is a single point (a disc).
fn axis_coverage_saturated(measurement: &Measurement) -> bool {
  measurement.vertices == 1 && measurement.axis_coverage > 0.9999
}

#[test]
fn a_finer_tolerance_never_covers_less() {
  for (name, region) in fixtures() {
    let coarse = measure(&region, TOLERANCES[0]);
    let fine = measure(&region, TOLERANCES[1]);
    assert!(
      fine.axis_coverage >= coarse.axis_coverage - 1.0e-9,
      "{name}: 0.1 mm covers {:.5} where 0.5 mm covered {:.5}",
      fine.axis_coverage,
      coarse.axis_coverage
    );
    assert!(
      fine.vertex_coverage >= coarse.vertex_coverage - 1.0e-9,
      "{name}: 0.1 mm's vertices cover {:.5} where 0.5 mm's covered {:.5}",
      fine.vertex_coverage,
      coarse.vertex_coverage
    );
    // A finer tolerance walks the same axis in finer steps — except on a disc, whose axis is a single
    // point at every tolerance, which is exactly the answer it should give.
    assert!(
      fine.vertices >= coarse.vertices,
      "{name}: a finer tolerance is a finer walk, {} vs {} vertices",
      coarse.vertices,
      fine.vertices
    );
    assert!(
      fine.vertices > coarse.vertices || axis_coverage_saturated(&fine),
      "{name}: a coarser tolerance is not a finer walk, {} vs {} vertices",
      coarse.vertices,
      fine.vertices
    );
  }
}

/// The branch counts this implementation gives, at `TOLERANCES[0]` and at `TOLERANCES[1]`.
///
/// Pinned, because the count is a *result* and not an implementation detail: each branch is a walk
/// with its own retract to safe Z between them (D-M4-6), so the shape of this table is the shape of
/// the program. Pinned by name so a renamed fixture cannot quietly change what is being asserted.
fn expected_branches(name: &str) -> (usize, usize) {
  match name {
    // Two diagonals meeting at the centre: four half-diagonals.
    "square 10x10" => (4, 4),
    // One point, whatever the tolerance: the centre.
    "disc r10" => (1, 1),
    // One midline plus four corner branches.
    "rectangle 20x6" => (5, 5),
    // The diagonal, the two parabolas off the reflex corner, and four corner branches — with the
    // junction's arms joined into one walk where the junction vertex itself was refused.
    "L 10x10" => (9, 9),
    // One closed loop at the mid radius.
    "annulus 4/10" => (1, 1),
    // One curve, one branch. This is the fixture that used to return 13 fragments at 0.1 mm.
    "ellipse 5x2" => (1, 1),
    other => panic!("no pinned branch count for the fixture {other}"),
  }
}

#[test]
fn a_finer_tolerance_never_returns_more_branches_than_a_coarser_one() {
  for (name, region) in fixtures() {
    let coarse = measure(&region, TOLERANCES[0]);
    let fine = measure(&region, TOLERANCES[1]);
    let (expected_coarse, expected_fine) = expected_branches(name);
    assert_eq!(
      coarse.branches, expected_coarse,
      "{name}: {} branches at {} mm",
      coarse.branches, TOLERANCES[0]
    );
    assert_eq!(
      fine.branches, expected_fine,
      "{name}: {} branches at {} mm",
      fine.branches, TOLERANCES[1]
    );
    // The property itself, stated as a property: accuracy is bought in the *steps*, never in a new
    // axis. A finer tolerance that answers with more branches has answered with more retracts, which
    // is a worse program produced by a better measurement.
    assert!(
      fine.branches <= coarse.branches,
      "{name}: {} mm returns {} branches where {} mm returned {}",
      TOLERANCES[1],
      fine.branches,
      TOLERANCES[0],
      coarse.branches
    );
    // And the finer walk is not the coarser one thinned out: it still covers at least as much.
    assert!(
      fine.vertices >= coarse.vertices,
      "{name}: {} mm walks {} vertices where {} mm walked {}",
      TOLERANCES[1],
      fine.vertices,
      TOLERANCES[0],
      coarse.vertices
    );
  }
}
