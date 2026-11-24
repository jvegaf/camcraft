//! Behaviour tests for point membership in a closed contour (A1).
//!
//! These are the tests every later boolean depends on: a pocket's islands are decided by asking
//! "is this point inside that contour", and an arc-only bug here would silently cut the wrong part.
//! The property test compares the analytic answer against a fine tessellation, which is the only way
//! to see an arc-specific mistake.

use proptest::prelude::*;

use cam_geom::primitive;
use cam_geom::winding::{classify, winding_number, Membership};
use cam_geom::{Contour, Point2, Vertex};

fn square(size: f64) -> Contour {
  primitive::rectangle(Point2::new(0.0, 0.0), size, size).expect("square")
}

fn circle(centre: Point2, radius: f64) -> Contour {
  primitive::circle(centre, radius).expect("circle")
}

/// A four-arc shape with bulges: arcs of different sweep about different centres.
fn bulged_shape() -> Contour {
  Contour::new(
    vec![
      Vertex::new(Point2::new(0.0, 0.0), 0.6),
      Vertex::new(Point2::new(20.0, 0.0), -0.4),
      Vertex::new(Point2::new(20.0, 14.0), 0.9),
      Vertex::new(Point2::new(0.0, 14.0), -0.25),
    ],
    true,
  )
  .expect("bulged shape")
}

/// A C: the notch on the right is outside the contour even though it is inside the bounds.
fn c_shape() -> Contour {
  Contour::from_points(
    &[
      Point2::new(0.0, 0.0),
      Point2::new(30.0, 0.0),
      Point2::new(30.0, 6.0),
      Point2::new(10.0, 6.0),
      Point2::new(10.0, 14.0),
      Point2::new(30.0, 14.0),
      Point2::new(30.0, 20.0),
      Point2::new(0.0, 20.0),
    ],
    true,
  )
  .expect("c shape")
}

/// Winding of a polygon given as points, by counting ray crossings — the reference the analytic
/// answer is checked against.
fn tessellated_winding(points: &[Point2], point: Point2) -> i32 {
  let mut winding = 0;
  for index in 0..points.len() {
    let a = points[index];
    let b = points[(index + 1) % points.len()];
    let side = (b.x - a.x) * (point.y - a.y) - (point.x - a.x) * (b.y - a.y);
    if a.y <= point.y {
      if b.y > point.y && side > 0.0 {
        winding += 1;
      }
    } else if b.y <= point.y && side < 0.0 {
      winding -= 1;
    }
  }
  winding
}

fn nearest_boundary_distance(contour: &Contour, point: Point2) -> f64 {
  contour
    .segments()
    .iter()
    .map(|segment| segment.distance_to_point(point))
    .fold(f64::INFINITY, f64::min)
}

#[test]
fn a_square_reports_inside_and_outside() {
  let contour = square(10.0);
  assert_eq!(
    classify(
      &contour,
      Point2::new(5.0, 5.0),
      cam_geom::TolerancePolicy::uniform(0.0)
    ),
    Membership::Inside
  );
  assert_eq!(
    classify(
      &contour,
      Point2::new(-1.0, 5.0),
      cam_geom::TolerancePolicy::uniform(0.0)
    ),
    Membership::Outside
  );
  assert_eq!(
    classify(
      &contour,
      Point2::new(11.0, 11.0),
      cam_geom::TolerancePolicy::uniform(0.0)
    ),
    Membership::Outside
  );
  assert_eq!(winding_number(&contour, Point2::new(5.0, 5.0)), 1);
  assert_eq!(winding_number(&contour, Point2::new(-1.0, 5.0)), 0);
}

#[test]
fn an_arc_only_contour_reports_inside_and_outside() {
  let contour = circle(Point2::new(3.0, -2.0), 4.0);
  assert!(contour
    .segments()
    .iter()
    .all(|segment| matches!(segment, cam_geom::Segment::Arc(_))));
  for (point, expected) in [
    (Point2::new(3.0, -2.0), Membership::Inside),
    (Point2::new(6.5, -2.0), Membership::Inside),
    (Point2::new(6.9, -2.0), Membership::Inside),
    (Point2::new(7.1, -2.0), Membership::Outside),
    (Point2::new(-2.0, -2.0), Membership::Outside),
    (Point2::new(3.0, 2.5), Membership::Outside),
  ] {
    assert_eq!(
      classify(&contour, point, cam_geom::TolerancePolicy::uniform(0.0)),
      expected,
      "{point:?} against the circle"
    );
  }
}

#[test]
fn a_bulged_shape_reports_inside_and_outside() {
  let contour = bulged_shape();
  assert!(contour.has_arcs());
  assert_eq!(
    classify(
      &contour,
      Point2::new(10.0, 7.0),
      cam_geom::TolerancePolicy::uniform(0.0)
    ),
    Membership::Inside
  );
  assert_eq!(
    classify(
      &contour,
      Point2::new(10.0, 16.0),
      cam_geom::TolerancePolicy::uniform(0.0)
    ),
    Membership::Inside,
    "the top bulge rises to about y = 23, so 16 mm is still material"
  );
  assert_eq!(
    classify(
      &contour,
      Point2::new(10.0, 30.0),
      cam_geom::TolerancePolicy::uniform(0.0)
    ),
    Membership::Outside,
    "above the top bulge"
  );
  // The bottom bulge dips below its own chord, so a point level with the chord's two ends is inside:
  // that case is where a branch bug in the arc's angle shows up as "outside".
  assert_eq!(
    classify(
      &contour,
      Point2::new(4.621_961_548_201_008, 0.0),
      cam_geom::TolerancePolicy::uniform(0.0)
    ),
    Membership::Inside,
    "the bottom bulge dips to y = -6 between the chord ends"
  );
}

#[test]
fn a_point_on_an_edge_or_a_vertex_is_on_the_boundary() {
  let contour = square(10.0);
  assert_eq!(
    classify(
      &contour,
      Point2::new(5.0, 0.0),
      cam_geom::TolerancePolicy::uniform(1.0e-9)
    ),
    Membership::OnBoundary
  );
  assert_eq!(
    classify(
      &contour,
      Point2::new(0.0, 0.0),
      cam_geom::TolerancePolicy::uniform(1.0e-9)
    ),
    Membership::OnBoundary,
    "a vertex is on the boundary too"
  );
  let round = circle(Point2::new(0.0, 0.0), 5.0);
  assert_eq!(
    classify(
      &round,
      Point2::new(0.0, 5.0),
      cam_geom::TolerancePolicy::uniform(1.0e-9)
    ),
    Membership::OnBoundary
  );
}

#[test]
fn tolerance_decides_a_point_near_the_boundary() {
  let contour = square(10.0);
  // 1 nm outside the edge: on the boundary at 1 um tolerance, outside at 1 pm.
  let just_outside = Point2::new(5.0, -1.0e-9);
  assert_eq!(
    classify(
      &contour,
      just_outside,
      cam_geom::TolerancePolicy::uniform(1.0e-6)
    ),
    Membership::OnBoundary
  );
  assert_eq!(
    classify(
      &contour,
      just_outside,
      cam_geom::TolerancePolicy::uniform(1.0e-12)
    ),
    Membership::Outside
  );
}

#[test]
fn the_winding_sign_follows_the_contour_direction() {
  let counter_clockwise = square(10.0);
  let clockwise = counter_clockwise.reverse();
  let inside = Point2::new(5.0, 5.0);
  assert_eq!(winding_number(&counter_clockwise, inside), 1);
  assert_eq!(winding_number(&clockwise, inside), -1);
  assert_eq!(
    classify(&clockwise, inside, cam_geom::TolerancePolicy::uniform(0.0)),
    Membership::Inside
  );
  assert_eq!(
    classify(
      &clockwise,
      Point2::new(-1.0, -1.0),
      cam_geom::TolerancePolicy::uniform(0.0)
    ),
    Membership::Outside
  );
  // The reversed circle keeps the same membership with the opposite winding.
  let round = circle(Point2::new(0.0, 0.0), 5.0);
  assert_eq!(winding_number(&round, Point2::new(0.0, 0.0)), 1);
  assert_eq!(winding_number(&round.reverse(), Point2::new(0.0, 0.0)), -1);
}

#[test]
fn the_notch_of_a_c_shape_is_outside() {
  let contour = c_shape();
  assert_eq!(
    classify(
      &contour,
      Point2::new(20.0, 10.0),
      cam_geom::TolerancePolicy::uniform(0.0)
    ),
    Membership::Outside,
    "the notch is inside the bounds but outside the material"
  );
  assert_eq!(
    classify(
      &contour,
      Point2::new(5.0, 10.0),
      cam_geom::TolerancePolicy::uniform(0.0)
    ),
    Membership::Inside
  );
  assert_eq!(
    classify(
      &contour,
      Point2::new(20.0, 3.0),
      cam_geom::TolerancePolicy::uniform(0.0)
    ),
    Membership::Inside,
    "the lower arm"
  );
}

#[test]
fn an_open_contour_encloses_nothing() {
  let open = Contour::from_points(
    &[
      Point2::new(0.0, 0.0),
      Point2::new(10.0, 0.0),
      Point2::new(10.0, 10.0),
    ],
    false,
  )
  .expect("open polyline");
  assert_eq!(winding_number(&open, Point2::new(9.0, 1.0)), 0);
  assert_eq!(
    classify(
      &open,
      Point2::new(9.0, 1.0),
      cam_geom::TolerancePolicy::uniform(0.0)
    ),
    Membership::Outside
  );
}

proptest! {
  /// The analytic winding must agree with a fine tessellation away from the boundary, and the two
  /// disagreement cases that matter (`OnBoundary` and points a tessellation cannot resolve) are
  /// excluded by distance rather than by hope.
  #[test]
  fn the_analytic_winding_agrees_with_a_fine_tessellation(
    x in -10.0f64..30.0,
    y in -10.0f64..25.0,
    which in 0u8..3,
  ) {
    let contour = match which {
      0 => circle(Point2::new(10.0, 7.0), 6.0),
      1 => bulged_shape(),
      _ => {
        Contour::new(
          vec![
            Vertex::new(Point2::new(2.0, 2.0), 1.0),
            Vertex::new(Point2::new(18.0, 2.0), 0.3),
            Vertex::new(Point2::new(10.0, 16.0), -0.7),
          ],
          true,
        )
        .expect("three arc shape")
      }
    };
    // The sampled reference casts a horizontal ray, and a ray exactly on a vertex is its own
    // degenerate case rather than a disagreement about the contour, so the ray is nudged by an amount
    // no authored coordinate lands on and both answers are asked about the same shifted point.
    let point = Point2::new(x, y + 1.234_567_89e-7);
    prop_assume!(nearest_boundary_distance(&contour, point) > 1.0e-3);

    let tessellated = contour.tessellate(cam_geom::TolerancePolicy::standard().with_chord(1.0e-4));
    let reference = tessellated_winding(&tessellated, point);
    prop_assert_eq!(
      winding_number(&contour, point),
      reference,
      "analytic and sampled winding disagree at {:?}",
      point
    );
    prop_assert_eq!(
      classify(&contour, point, cam_geom::TolerancePolicy::uniform(1.0e-9)),
      if reference == 0 { Membership::Outside } else { Membership::Inside }
    );
  }
}
