//! Behaviour tests for `cam_geom::medial` — the verified medial axis (M7 T2, A2, D-M7-2, D-M7-4).
//!
//! Two kinds of assertion, and both are needed. The **shape** assertions write the arithmetic out for
//! fixtures whose axis is known in closed form, so a plausible-looking axis cannot pass by accident.
//! The **certificate** assertion is the one the contract is actually made of: for every retained
//! vertex this file recomputes the exact nearest point of every boundary segment *independently of the
//! kernel*, and demands at least two distinct nearest points at the vertex's own clearance. A wrong
//! axis that happens to have the right shape still fails it.
//!
//! The certificate is deliberately re-derived here rather than borrowed from the implementation: a
//! checker that asks the same code the same question proves nothing.

use std::f64::consts::TAU;

use cam_geom::medial::{medial_axis, MedialAxis};
use cam_geom::primitive;
use cam_geom::region::Region;
use cam_geom::{Contour, Point2, Segment};

/// The coarse tolerance the structural fixtures use, in millimetres.
const COARSE: f64 = 0.5;

/// The fine tolerance, an order of magnitude below the coarse one.
const FINE: f64 = 0.1;

/// Position slack for a structural assertion. It is the bisection tolerance of the coarse pass
/// (a ten-thousandth of it), with room for the lattice's own rounding.
const NEAR: f64 = 1.0e-3;

fn point(x: f64, y: f64) -> Point2 {
  Point2::new(x, y)
}

fn region(boundaries: Vec<Contour>) -> Region {
  Region::new(boundaries).expect("closed boundaries")
}

fn circle_contour(radius: f64) -> Contour {
  primitive::circle(Point2::ORIGIN, radius).expect("circle")
}

/// A 10 x 10 square at the origin: the axis is its two diagonals.
fn square() -> Region {
  region(vec![
    primitive::rectangle(point(0.0, 0.0), 10.0, 10.0).expect("square")
  ])
}

/// A 20 x 6 rectangle at the origin: the axis is one long midline plus four corner branches.
fn rectangle() -> Region {
  region(vec![
    primitive::rectangle(point(0.0, 0.0), 20.0, 6.0).expect("rectangle")
  ])
}

/// The 10 x 10 square with its 6 x 6 top-right quadrant removed. The reflex corner is `(4, 4)`.
fn l_shape() -> Region {
  let points = [
    point(0.0, 0.0),
    point(10.0, 0.0),
    point(10.0, 4.0),
    point(4.0, 4.0),
    point(4.0, 10.0),
    point(0.0, 10.0),
  ];
  region(vec![primitive::polyline(&points, true).expect("L")])
}

/// A disc of radius 10 with a concentric hole of radius 4: the axis is the circle of radius 7.
fn annulus() -> Region {
  region(vec![circle_contour(10.0), circle_contour(4.0)])
}

/// A 20 x 0.2 sliver: 0.1 mm of material either side of its midline.
fn sliver() -> Region {
  region(vec![
    primitive::rectangle(point(0.0, 0.0), 20.0, 0.2).expect("sliver")
  ])
}

/// The kernel's 16-arc approximation of a 5 x 2 ellipse: the fixture whose axis is one curve.
fn ellipse_5x2() -> Region {
  region(vec![
    primitive::ellipse(Point2::ORIGIN, 5.0, 2.0, 0.01)
      .expect("ellipse")
      .contour,
  ])
}

/// The analytic clearance inside the 10 x 10 square: the nearest of its four walls.
fn square_clearance(p: Point2) -> f64 {
  p.x.min(10.0 - p.x).min(p.y).min(10.0 - p.y)
}

// -- The independent certificate ------------------------------------------------------------------

/// The nearest point of one segment, and its exact distance, computed here from first principles.
///
/// `interval` is true for the one degenerate case the contract names: an arc asked about its own
/// centre, where every point of the sweep is equally near.
fn nearest_on(segment: Segment, query: Point2) -> (Point2, f64, bool) {
  match segment {
    Segment::Line(line) => {
      let span = line.end - line.start;
      let length_squared = span.length_squared();
      let t = if length_squared <= 0.0 {
        0.0
      } else {
        ((query - line.start).dot(span) / length_squared).clamp(0.0, 1.0)
      };
      let foot = line.start + span * t;
      (foot, query.distance_to(foot), false)
    }
    Segment::Arc(arc) => {
      let offset = query - arc.center();
      let length = offset.length();
      if length <= f64::MIN_POSITIVE {
        return (arc.start_point(), arc.radius(), true);
      }
      let mut turned = if arc.sweep() > 0.0 {
        offset.angle() - arc.start_angle()
      } else {
        arc.start_angle() - offset.angle()
      };
      turned = turned.rem_euclid(TAU);
      if turned <= arc.sweep().abs() {
        let foot = arc.center() + offset * (arc.radius() / length);
        return (foot, (length - arc.radius()).abs(), false);
      }
      let (start, end) = (arc.start_point(), arc.end_point());
      let (to_start, to_end) = (query.distance_to(start), query.distance_to(end));
      if to_start <= to_end {
        (start, to_start, false)
      } else {
        (end, to_end, false)
      }
    }
  }
}

/// The empty-ball certificate at `query`: the exact clearance, and the distinct nearest boundary points.
struct Certificate {
  distance: f64,
  points: Vec<Point2>,
  interval: bool,
}

fn certificate(region: &Region, query: Point2) -> Certificate {
  let mut best = f64::INFINITY;
  let mut all = Vec::new();
  for boundary in &region.boundaries {
    for segment in boundary.segments() {
      let (foot, distance, interval) = nearest_on(segment, query);
      best = best.min(distance);
      all.push((foot, distance, interval));
    }
  }
  let slack = 1.0e-9 * (1.0 + best.abs());
  let mut points: Vec<Point2> = Vec::new();
  let mut interval = false;
  for (foot, distance, whole) in all {
    if distance > best + slack {
      continue;
    }
    interval |= whole;
    if !points.iter().any(|other| other.is_near(foot, slack)) {
      points.push(foot);
    }
  }
  Certificate {
    distance: best,
    points,
    interval,
  }
}

/// Every retained vertex is a medial point: material, exactly as far as its clearance says, and
/// touching the boundary in at least two distinct points (or along an interval, for an arc's centre).
fn assert_every_vertex_certified(region: &Region, axis: &MedialAxis) {
  for branch in &axis.branches {
    assert!(
      !branch.vertices.is_empty(),
      "a branch is never empty: it would be a point that is not on the axis"
    );
    for vertex in &branch.vertices {
      let cert = certificate(region, vertex.point);
      assert!(
        vertex.clearance > 0.0,
        "clearance {:?} is not positive, so the tip is on or outside the boundary",
        vertex
      );
      assert!(
        (cert.distance - vertex.clearance).abs() <= 1.0e-9 * (1.0 + cert.distance),
        "vertex {:?} reports clearance {} where the exact distance is {}",
        vertex.point,
        vertex.clearance,
        cert.distance
      );
      assert!(
        cert.points.len() >= 2 || cert.interval,
        "vertex {:?} is not a medial point: {} distinct nearest point(s) at {}",
        vertex.point,
        cert.points.len(),
        cert.distance
      );
      let answer = region
        .distance_to_boundary(vertex.point, 0.0)
        .expect("the fixture has boundaries");
      assert!(
        answer.inside,
        "vertex {:?} is not material, so it is not on the region's axis",
        vertex.point
      );
    }
  }
}

/// Every fixture, at both tolerances, must satisfy the certificate — the axis is only as good as this.
#[test]
fn every_retained_vertex_of_every_fixture_is_a_certified_medial_point() {
  let fixtures: [(&str, Region); 6] = [
    ("square", square()),
    ("rectangle", rectangle()),
    ("L", l_shape()),
    ("annulus", annulus()),
    ("disc", region(vec![circle_contour(10.0)])),
    ("ellipse", ellipse_5x2()),
  ];
  for (name, fixture) in &fixtures {
    for tolerance in [COARSE, FINE] {
      let axis = medial_axis(fixture, tolerance).expect("the fixtures have positive tolerances");
      assert!(
        !axis.branches.is_empty(),
        "{name} at {tolerance} has no branches at all"
      );
      let vertices: usize = axis
        .branches
        .iter()
        .map(|branch| branch.vertices.len())
        .sum();
      println!(
        "{name:12} at {tolerance}: {} branch(es), {vertices} vertex/vertices, {} candidate(s) dropped",
        axis.branches.len(),
        axis.candidates_dropped
      );
      assert_every_vertex_certified(fixture, &axis);
    }
  }
}

// -- A square ----------------------------------------------------------------------------------

#[test]
fn a_square_has_four_branches_that_are_its_diagonals_meeting_at_the_centre() {
  let region = square();
  let axis = medial_axis(&region, COARSE).expect("axis");
  assert_every_vertex_certified(&region, &axis);

  // The axis of a square is its two diagonals, which meet at the centre: four half-diagonals.
  assert_eq!(
    axis.branches.len(),
    4,
    "a square has four half-diagonals, got {:?}",
    axis
      .branches
      .iter()
      .map(|branch| branch.vertices.len())
      .collect::<Vec<_>>()
  );

  let centre = point(5.0, 5.0);
  for branch in &axis.branches {
    let first = branch.vertices.first().expect("non-empty");
    let last = branch.vertices.last().expect("non-empty");
    assert!(
      first.point.is_near(centre, NEAR) || last.point.is_near(centre, NEAR),
      "a diagonal branch leaves the centre, got {:?} to {:?}",
      first.point,
      last.point
    );
    for vertex in &branch.vertices {
      // On one of the two diagonals: x == y (the one through the origin corner) or x + y == 10.
      let on_diagonal = (vertex.point.x - vertex.point.y).abs() <= NEAR
        || (vertex.point.x + vertex.point.y - 10.0).abs() <= NEAR;
      assert!(
        on_diagonal,
        "vertex {:?} is off both diagonals",
        vertex.point
      );
      // The clearance is the analytic distance to the nearer pair of walls.
      assert!(
        (vertex.clearance - square_clearance(vertex.point)).abs() <= NEAR,
        "vertex {:?} reports {} where the walls give {}",
        vertex.point,
        vertex.clearance,
        square_clearance(vertex.point)
      );
    }
  }

  // The centre is the deepest point: the inscribed circle has radius 5.
  let deepest = axis
    .branches
    .iter()
    .flat_map(|branch| branch.vertices.iter())
    .map(|vertex| vertex.clearance)
    .fold(0.0_f64, f64::max);
  assert!(
    (deepest - 5.0).abs() <= NEAR,
    "the deepest tip is the centre, at 5 mm; got {deepest}"
  );
}

// -- A circle, and the star a tessellated disc produces -------------------------------------------

#[test]
fn a_discs_axis_is_its_centre_alone_and_a_tessellated_disc_is_not() {
  let disc = region(vec![circle_contour(10.0)]);
  let axis = medial_axis(&disc, COARSE).expect("axis");
  assert_every_vertex_certified(&disc, &axis);

  // A disc's medial axis is exactly one point, the centre, and its clearance is the radius.
  assert_eq!(axis.branches.len(), 1, "a disc has one point of axis");
  let vertices: Vec<_> = axis.branches[0].vertices.clone();
  assert_eq!(vertices.len(), 1, "the centre is the whole axis");
  assert!(vertices[0].point.is_near(Point2::ORIGIN, NEAR));
  assert!((vertices[0].clearance - 10.0).abs() <= NEAR);

  // The same disc, tessellated by the *caller* into chords, has a star of spikes instead: every
  // vertex of the polygon is the tip of one. The chord count is what explains the difference.
  // A 10 mm radius at a 0.5 mm chord error is only 10 chords (5 per half turn), and every one of
  // those ten vertices is a spike.
  let chords = circle_contour(10.0).tessellate(COARSE).len();
  assert!(
    chords >= 8,
    "a 10 mm circle at 0.5 mm chords should be a coarse polygon, got {chords} chords"
  );
  let polygon = region(vec![
    primitive::polygon(Point2::ORIGIN, 10.0, 64, 0.0).expect("64-gon")
  ]);
  let star = medial_axis(&polygon, COARSE).expect("axis");
  assert_every_vertex_certified(&polygon, &star);
  assert!(
    star.branches.len() > 8,
    "a 64-gon's axis is a star of spikes towards its vertices, not one point; got {} branches",
    star.branches.len()
  );
  println!(
    "disc: {} branch(es) of {} vertex/vertices; the caller's tessellation at {COARSE} mm is {chords} chords and the 64-gon's axis is {} branches, {} vertices, {} dropped",
    axis.branches.len(),
    axis.branches[0].vertices.len(),
    star.branches.len(),
    star.branches.iter().map(|branch| branch.vertices.len()).sum::<usize>(),
    star.candidates_dropped
  );
}

// -- The same shapes turned off the lattice ------------------------------------------------------

#[test]
fn a_square_turned_thirty_degrees_still_has_its_two_diagonals() {
  // Nothing about the axis depends on the lattice: the same square rotated about its own centre has
  // the same four half-diagonals, and every vertex must still certify.
  let square = primitive::rectangle(point(0.0, 0.0), 10.0, 10.0).expect("square");
  let turned =
    cam_geom::transform::rotate(&square, 30.0_f64.to_radians(), point(5.0, 5.0)).expect("rotate");
  let region = region(vec![turned]);
  let axis = medial_axis(&region, COARSE).expect("axis");
  assert_every_vertex_certified(&region, &axis);
  assert_eq!(axis.branches.len(), 4);
  // The diagonals of a square turned by 30 degrees run at 30 + 45 = 75 and 30 - 45 = -15 degrees.
  let centre = point(5.0, 5.0);
  let directions = [
    point(75.0_f64.to_radians().cos(), 75.0_f64.to_radians().sin()),
    point(
      (-15.0_f64).to_radians().cos(),
      (-15.0_f64).to_radians().sin(),
    ),
  ];
  for branch in &axis.branches {
    for vertex in &branch.vertices {
      let offset = vertex.point - centre;
      let on_diagonal = directions.iter().any(|direction| {
        // The cross product of the offset with a unit direction is the distance off that line.
        (offset.x * direction.y - offset.y * direction.x).abs() <= NEAR
      });
      assert!(
        on_diagonal,
        "vertex {:?} is off both of the rotated square's diagonals",
        vertex.point
      );
    }
  }
}

#[test]
fn an_ellipse_of_semi_axes_five_and_two_has_its_analytic_axis_on_y_zero() {
  // The fixture's boundary is the kernel's 16-arc ellipse, whose radial error is 6.1e-3 mm, so every
  // clearance compared against the *analytic* ellipse is allowed 0.01 mm — one order above the
  // boundary's own error — and no more.
  const BOUNDARY_ERROR: f64 = 0.01;
  // Semi-axes, and the answer they imply: the medial axis of an ellipse is the segment between the two
  // centres of curvature at the ends of the major axis, `(+/-(a - b^2/a), 0) = (+/-4.2, 0)`, and the
  // clearance at those two endpoints is the radius of curvature there, `b^2/a = 0.8`.
  const A: f64 = 5.0;
  const B: f64 = 2.0;
  let analytic_end = A - B * B / A;
  assert!(
    (analytic_end - 4.2).abs() <= 1.0e-12,
    "the analytic endpoints are a - b^2/a = 4.2 mm, got {analytic_end}"
  );
  assert!(
    (B * B / A - 0.8).abs() <= 1.0e-12,
    "the clearance at them is the radius of curvature b^2/a = 0.8 mm"
  );
  // The exact distance from `(x, 0)` to the ellipse, for `|x| <= a - b^2/a`: the nearest boundary
  // point is `(a cos t, b sin t)` with `cos t = a x / (a^2 - b^2)`, which is 1 exactly at `x = 4.2`
  // and gives the radius of curvature there.
  let clearance_at = |x: f64| -> f64 {
    let cos = (A * x / (A * A - B * B)).clamp(-1.0, 1.0);
    let sin = (1.0 - cos * cos).sqrt();
    point(x, 0.0).distance_to(point(A * cos, B * sin))
  };
  assert!((clearance_at(analytic_end) - 0.8).abs() <= 1.0e-12);
  assert!((clearance_at(0.0) - B).abs() <= 1.0e-12);

  let ellipse = primitive::ellipse(Point2::ORIGIN, A, B, 0.01).expect("ellipse");
  assert!(
    ellipse.arc_count() >= 4,
    "the ellipse really is built from arcs"
  );
  let region = region(vec![ellipse.contour]);
  let axis = medial_axis(&region, COARSE).expect("axis");
  assert_every_vertex_certified(&region, &axis);
  // One branch, straight along `y = 0`, with the clearance falling from `b` at the centre to the
  // radius of curvature `b^2/a` at the analytic ends.
  assert_eq!(
    axis.branches.len(),
    1,
    "the axis of an ellipse is one branch, got {}",
    axis.branches.len()
  );
  let branch = &axis.branches[0];
  assert!(!branch.closed, "an ellipse's axis is a path, not a loop");
  for vertex in &branch.vertices {
    assert!(
      vertex.point.y.abs() <= NEAR,
      "the axis lies on y = 0, got {:?}",
      vertex.point
    );
    let expected = clearance_at(vertex.point.x);
    assert!(
      (vertex.clearance - expected).abs() <= BOUNDARY_ERROR,
      "at x = {} the clearance is {} where the ellipse gives {}",
      vertex.point.x,
      vertex.clearance,
      expected
    );
  }
  let deepest = branch
    .vertices
    .iter()
    .map(|vertex| vertex.clearance)
    .fold(0.0_f64, f64::max);
  assert!(
    (deepest - B).abs() <= BOUNDARY_ERROR,
    "the deepest tip is the centre at b = {B} mm, got {deepest}"
  );
  assert!(
    branch
      .vertices
      .iter()
      .any(|vertex| vertex.point.x.abs() <= NEAR && vertex.clearance >= B - BOUNDARY_ERROR),
    "the centre is a vertex: it is where the axis is deepest"
  );

  // What the axis actually reaches, and why it is not the analytic endpoint. This is measured, not
  // fitted: at a 0.5 mm tolerance the branch ends at `x = +-3.5`, 0.7 mm short of `+-4.2`. Two measured
  // facts explain the shortfall and neither of them is the shape being wrong:
  //
  // * every tip beyond `x = 4.5` has a clearance at or below the 0.5 mm tolerance — the exact distance
  //   from `(4.5, 0)` to the ellipse is 0.4986 — so it is not material by the caller's own declaration
  //   (D-M7-5) and the certificate drops it;
  // * between `x = 3.5` and `4.5` the coarse lattice offers no candidate that pairs the two symmetric
  //   contacts. The samples at `y = -0.5` and `y = +0.5` are nearest to two different arcs (14 and 1,
  //   a fixture-level detail of the 16-arc chain), while the row exactly on `y = 0` sits on the tie
  //   between the two arcs that meet at the axis end (0 and 15), so its label is decided by rounding
  //   noise in the two distances rather than by a distance difference; the crossings on those columns
  //   then equate features that are not the nearest ones and the certificate drops them too.
  //
  // The contacts at the last retained vertex are therefore two distinct boundary points 2.17 mm apart:
  // the same-point dedup did **not** truncate this branch, the tolerance did. Coverage is what it
  // costs, never depth (the claim's point 1), and the module documentation records both costs.
  let reach = branch
    .vertices
    .iter()
    .map(|vertex| vertex.point.x.abs())
    .fold(0.0_f64, f64::max);
  assert!(
    (reach - 3.5).abs() <= NEAR,
    "the axis stops at x = +-3.5 mm at a {COARSE} mm tolerance, 0.7 mm short of the analytic 4.2; got {reach}"
  );
  let shortfall = analytic_end - reach;
  assert!(
    (shortfall - 0.7).abs() <= NEAR,
    "the branch is 0.7 mm short of the analytic endpoint, got {shortfall}"
  );
  // The last tip is still a genuine medial point with two well-separated contacts, so the shortfall is
  // not the dedup collapsing a pair: it is where the tolerance stopped offering tips.
  let last = branch
    .vertices
    .iter()
    .max_by(|a, b| a.point.x.abs().total_cmp(&b.point.x.abs()))
    .expect("non-empty");
  let contacts = certificate(&region, last.point).points;
  let spread = if contacts.len() >= 2 {
    contacts[0].distance_to(contacts[1])
  } else {
    0.0
  };
  assert!(
    contacts.len() >= 2 && spread > COARSE,
    "the last tip's two contacts are distinct and {spread} mm apart, so nothing was deduped away: {:?}",
    contacts
  );

  // The shortfall is the tolerance's, not the method's: at a tenth of the step the same fixture walks
  // past the analytic endpoint. Nothing about the construction caps it at a coarse lattice column.
  let fine = medial_axis(&region, FINE).expect("axis");
  assert_every_vertex_certified(&region, &fine);
  let fine_reach = fine
    .branches
    .iter()
    .flat_map(|branch| &branch.vertices)
    .map(|vertex| vertex.point.x.abs())
    .fold(0.0_f64, f64::max);
  assert!(
    fine_reach > analytic_end,
    "at a {FINE} mm tolerance the walk passes the analytic endpoint {analytic_end}, got {fine_reach}"
  );

  // A finer tolerance buys accuracy in the *steps*, never a different axis: one branch, on the same
  // line, with the same deepest point. This is the property the V-carve needs and it is not a detail:
  // before the seams were repaired this fixture reported **thirteen** fragments at 0.1 mm against one
  // branch at 0.5 mm, and every fragment is a retract to safe Z (D-M4-6) — a worse program bought with
  // a better measurement.
  assert_eq!(
    fine.branches.len(),
    1,
    "a finer tolerance returns the same one branch, got {}",
    fine.branches.len()
  );
  let fine_branch = &fine.branches[0];
  assert!(
    !fine_branch.closed,
    "an ellipse's axis is a path, not a loop"
  );
  for vertex in &fine_branch.vertices {
    assert!(
      vertex.point.y.abs() <= NEAR,
      "the axis lies on y = 0 at every tolerance, got {:?}",
      vertex.point
    );
    // The analytic clearance holds up to the curvature maximum. Past it the exact ellipse's nearest
    // point is the single vertex `(5, 0)`, while this fixture's boundary is the 16-arc approximation,
    // so the formula is not the fixture's answer there and asserting it would be asserting the wrong
    // shape (the certificate still checks every one of those vertices against the exact distance).
    if vertex.point.x.abs() <= analytic_end {
      assert!(
        (vertex.clearance - clearance_at(vertex.point.x)).abs() <= BOUNDARY_ERROR,
        "at x = {} the clearance is {} where the ellipse gives {}",
        vertex.point.x,
        vertex.clearance,
        clearance_at(vertex.point.x)
      );
    }
  }
  let fine_deepest = fine_branch
    .vertices
    .iter()
    .map(|vertex| vertex.clearance)
    .fold(0.0_f64, f64::max);
  assert!(
    (fine_deepest - B).abs() <= BOUNDARY_ERROR,
    "the deepest tip is the centre at b = {B} mm at both tolerances, got {fine_deepest}"
  );
  assert!(
    fine_branch
      .vertices
      .iter()
      .any(|vertex| vertex.point.x.abs() <= NEAR && vertex.clearance >= B - BOUNDARY_ERROR),
    "the centre is a vertex at both tolerances: it is where the axis is deepest"
  );

  // How far the refined axis reaches, measured: `x = +-4.8`, past the analytic endpoint rather than to
  // it, and the reason is the boundary and not the method. The axis of the *exact* ellipse stops at
  // `+-4.2`, where its two symmetric contacts merge into the nearest single vertex `(5, 0)`; the
  // moment they merge the point is not medial, and past it the nearest point of the ellipse is `(5, 0)`
  // alone. This fixture's boundary is the 16-arc approximation, whose rightmost point is a **joint**
  // between two mirrored arcs — a curvature jump, not a smooth maximum — so on it the two contacts
  // stay distinct all the way to the tip and the axis is medial there too. What stops the walk is the
  // caller's own material band, one lattice step short of the tip: the last column whose clearance
  // exceeds a 0.1 mm tolerance is `x = 4.8` (clearance 0.1996), and `x = 4.9` is 0.0998 away from the
  // wall — inside the tolerance, so not material (D-M7-5).
  assert!(
    (fine_reach - 4.8).abs() <= NEAR,
    "at a {FINE} mm tolerance the axis reaches x = +-4.8 mm, past the analytic +-{analytic_end}; got {fine_reach}"
  );
  println!(
    "ellipse a={A} b={B}: {} branch(es) at {COARSE} reaching x = +-{reach}, deepest {deepest}; at {FINE} {} branch(es) reaching x = +-{fine_reach}; {} and {} candidate(s) dropped",
    axis.branches.len(),
    fine.branches.len(),
    axis.candidates_dropped,
    fine.candidates_dropped
  );
}

// -- A rectangle ----------------------------------------------------------------------------------

#[test]
fn a_rectangle_has_one_long_midline_that_ends_where_the_corners_take_over() {
  let region = rectangle();
  let axis = medial_axis(&region, COARSE).expect("axis");
  assert_every_vertex_certified(&region, &axis);

  // The axis of a 20 x 6 rectangle: the midline at y = 3 from x = 3 to x = 17, where the four corner
  // bisectors take over — five branches, and exactly one of them is the midline.
  assert_eq!(
    axis.branches.len(),
    5,
    "one midline plus four corner branches"
  );
  let midlines: Vec<_> = axis
    .branches
    .iter()
    .filter(|branch| {
      branch
        .vertices
        .iter()
        .all(|v| (v.point.y - 3.0).abs() <= NEAR)
    })
    .collect();
  assert_eq!(midlines.len(), 1, "exactly one branch runs along y = 3");
  let midline = midlines[0];
  let lowest = midline
    .vertices
    .iter()
    .map(|v| v.point.x)
    .fold(f64::INFINITY, f64::min);
  let highest = midline
    .vertices
    .iter()
    .map(|v| v.point.x)
    .fold(f64::NEG_INFINITY, f64::max);
  // The corners take over at x = 3 and x = 17: there the distance to the end wall (3) equals the
  // distance to a side wall (3), which is where the midline stops being nearest to the two walls.
  assert!(
    (lowest - 3.0).abs() <= COARSE && (highest - 17.0).abs() <= COARSE,
    "the midline spans x = 3 to x = 17, got {lowest} to {highest}"
  );
  for vertex in &midline.vertices {
    assert!(
      (vertex.clearance - 3.0).abs() <= NEAR,
      "every midline tip is 3 mm from the two long walls, got {}",
      vertex.clearance
    );
  }
}

// -- An L shape and its reflex corner -------------------------------------------------------------

#[test]
fn an_l_shape_carries_a_branch_whose_nearest_points_include_the_reflex_corner() {
  let region = l_shape();
  let axis = medial_axis(&region, COARSE).expect("axis");
  assert_every_vertex_certified(&region, &axis);

  // The reflex corner at (4, 4) is a boundary *point* the axis is equidistant from: the bisector of
  // that point and the bottom wall is the parabola y = ((x - 4)^2 + 16) / 8, whose vertex is (4, 2)
  // — the point 2 mm above the bottom wall and 2 mm from the reflex corner. The two contacts are
  // (4, 0) and (4, 4), which is exactly what makes it medial rather than the reflex cone's interior,
  // where the corner is the *only* nearest point and nothing certifies.
  let reflex = point(4.0, 4.0);
  let on_parabola = axis
    .branches
    .iter()
    .flat_map(|branch| branch.vertices.iter())
    .find(|vertex| {
      let cert = certificate(&region, vertex.point);
      vertex.point.is_near(point(4.0, 2.0), COARSE)
        && cert.points.iter().any(|p| p.is_near(reflex, 1.0e-9))
    });
  let vertex = on_parabola.expect("the reflex corner is one of some vertex's two nearest points");
  assert!(
    (vertex.clearance - 2.0).abs() <= COARSE,
    "the parabola's vertex is 2 mm from both the corner and the wall, got {}",
    vertex.clearance
  );

  // The four convex corners behave as a square's do, so the axis is a connected tree with junctions.
  assert!(
    axis.branches.len() >= 4,
    "an L has at least one branch per corner plus the reflex branch, got {}",
    axis.branches.len()
  );
}

// -- An annulus -----------------------------------------------------------------------------------

#[test]
fn an_annulus_has_one_closed_loop_at_the_mid_radius() {
  let region = annulus();
  let axis = medial_axis(&region, COARSE).expect("axis");
  assert_every_vertex_certified(&region, &axis);

  // Two concentric boundaries with radii 4 and 10: the axis is the circle where the two are equally
  // far, i.e. radius (4 + 10) / 2 = 7, with clearance 7 - 4 = 3 = 10 - 7.
  assert_eq!(
    axis.branches.len(),
    1,
    "an annulus has one loop, not {} branches",
    axis.branches.len()
  );
  let branch = &axis.branches[0];
  assert!(
    branch.closed,
    "the mid-line between two boundaries is closed"
  );
  let mut length = 0.0;
  let count = branch.vertices.len();
  for index in 0..count {
    let vertex = branch.vertices[index];
    let radius = vertex.point.distance_to(Point2::ORIGIN);
    assert!(
      (radius - 7.0).abs() <= NEAR,
      "a loop vertex is at radius 7, got {radius}"
    );
    assert!((vertex.clearance - 3.0).abs() <= NEAR);
    let next = branch.vertices[(index + 1) % count];
    length += vertex.point.distance_to(next.point);
  }
  let expected = TAU * 7.0;
  assert!(
    (length - expected).abs() <= expected * 0.02,
    "the loop's length is 2*pi*7 = {expected}, got {length}"
  );
}

// -- A region with no axis at all -----------------------------------------------------------------

#[test]
fn a_sliver_thinner_than_the_tolerance_has_an_empty_axis_rather_than_a_panic() {
  let region = sliver();
  // 0.1 mm of material either side of the midline, so every tip is within 0.1 mm of a wall: at a
  // 0.5 mm tolerance no point is further from the boundary than the tolerance and none is material.
  let coarse = medial_axis(&region, COARSE).expect("axis");
  assert!(
    coarse.branches.is_empty(),
    "a sliver thinner than the tolerance has no axis, got {:?}",
    coarse.branches
  );
  // The same region at a tolerance below its thickness does have one, and it is the midline.
  let fine = medial_axis(&region, 0.05).expect("axis");
  assert!(
    !fine.branches.is_empty(),
    "0.05 mm resolves a 0.2 mm sliver"
  );
  assert_every_vertex_certified(&region, &fine);
  for vertex in fine
    .branches
    .iter()
    .flat_map(|branch| branch.vertices.iter())
  {
    assert!(
      vertex.clearance <= 0.1 + NEAR,
      "every tip of a 0.2 mm sliver is at most 0.1 mm deep, got {}",
      vertex.clearance
    );
  }
}

#[test]
fn an_empty_region_has_an_empty_axis() {
  let empty = Region::default();
  let axis = medial_axis(&empty, COARSE).expect("an empty region is not an error");
  assert!(axis.branches.is_empty());
}

#[test]
fn a_tolerance_that_would_need_an_unbounded_lattice_is_refused_by_name() {
  // The lattice is `(width / tolerance + 1) * (height / tolerance + 1)` samples. A 10 x 10 square at
  // 4 micrometres wants 2501^2 = 6,255,001 of them, past the 4,000,000 cap, and the refusal names the
  // lattice instead of silently coarsening the caller's tolerance or hanging.
  let error = medial_axis(&square(), 0.004).expect_err("the lattice is too large to build");
  let message = error.to_string();
  assert!(
    message.contains("lattice"),
    "the refusal names it: {message}"
  );
}

#[test]
fn a_tolerance_that_is_not_positive_and_finite_is_refused() {
  let region = square();
  for tolerance in [0.0, -0.5, f64::NAN, f64::INFINITY] {
    assert!(
      medial_axis(&region, tolerance).is_err(),
      "tolerance {tolerance} is not a length and must be refused"
    );
  }
}

// -- Determinism (S10) ----------------------------------------------------------------------------

#[test]
fn two_runs_return_the_same_branches_in_the_same_order_vertex_for_vertex() {
  // The ellipse is in the list because it is the fixture the seams are joined on: even the repair has
  // to be a function of the region and nothing else.
  for fixture in [square(), rectangle(), l_shape(), annulus(), ellipse_5x2()] {
    for tolerance in [COARSE, FINE] {
      let first = medial_axis(&fixture, tolerance).expect("axis");
      let second = medial_axis(&fixture, tolerance).expect("axis");
      assert_eq!(
        first, second,
        "two runs on the same region and tolerance must agree exactly"
      );
      // A branch is never emitted twice, and the order is by first vertex.
      let mut starts: Vec<(f64, f64)> = first
        .branches
        .iter()
        .map(|branch| (branch.vertices[0].point.y, branch.vertices[0].point.x))
        .collect();
      let sorted = {
        let mut copy = starts.clone();
        copy.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
        copy
      };
      assert_eq!(starts, sorted, "branches are ordered by their first vertex");
      starts.dedup();
    }
  }
}

// -- The shape of the answer ----------------------------------------------------------------------

#[test]
fn a_finer_tolerance_walks_the_same_axis_in_finer_steps() {
  let region = square();
  let coarse = medial_axis(&region, COARSE).expect("axis");
  let fine = medial_axis(&region, FINE).expect("axis");
  assert_eq!(coarse.branches.len(), fine.branches.len());
  let coarse_vertices: usize = coarse.branches.iter().map(|b| b.vertices.len()).sum();
  let fine_vertices: usize = fine.branches.iter().map(|b| b.vertices.len()).sum();
  assert!(
    fine_vertices > coarse_vertices,
    "the finer tolerance refines the same diagonals: {coarse_vertices} vs {fine_vertices} vertices"
  );
  // Both are the same two diagonals, so the total branch length is the same to within the chording.
  let path_length = |axis: &MedialAxis| -> f64 {
    axis
      .branches
      .iter()
      .map(|branch| {
        branch
          .vertices
          .windows(2)
          .map(|pair| pair[0].point.distance_to(pair[1].point))
          .sum::<f64>()
      })
      .sum()
  };
  // The diagonals of a 10 x 10 square are 2 * 10 * sqrt(2) long corner to corner. The walk stops
  // short of each corner by one lattice step: a tip whose clearance is not above the tolerance is not
  // material, and the next vertex along a diagonal is one step (tolerance * sqrt(2) of clearance)
  // further in, so each of the four tips loses at most (tolerance + tolerance * sqrt(2)) * sqrt(2).
  let expected = 2.0 * 10.0 * 2.0_f64.sqrt();
  let cut = 4.0 * (FINE + FINE * 2.0_f64.sqrt()) * 2.0_f64.sqrt();
  assert!(
    (path_length(&fine) - expected).abs() <= cut,
    "the two diagonals are {expected} mm of axis less at most {cut} at the four tips, got {}",
    path_length(&fine)
  );
}
