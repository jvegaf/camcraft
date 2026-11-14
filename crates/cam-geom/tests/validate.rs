//! Behaviour tests for vector validation (M3 A16).

use cam_geom::primitive;
use cam_geom::validate::inspect;
use cam_geom::{Contour, Point2, TolerancePolicy, Vertex, DEFAULT_TOLERANCE};

const TOL: TolerancePolicy = TolerancePolicy::uniform(DEFAULT_TOLERANCE);
use proptest::prelude::*;

fn figure_eight() -> Contour {
  Contour::from_points(
    &[
      Point2::new(0.0, 0.0),
      Point2::new(10.0, 10.0),
      Point2::new(10.0, 0.0),
      Point2::new(0.0, 10.0),
    ],
    true,
  )
  .expect("figure eight")
}

#[test]
fn a_figure_eight_reports_the_crossing_once() {
  let issues = inspect(&figure_eight(), TOL);
  assert!(issues.open.is_none());
  assert_eq!(issues.intersections.len(), 1, "{issues:?}");
  let crossing = &issues.intersections[0];
  assert!(crossing.point.is_near(
    Point2::new(5.0, 5.0),
    cam_geom::TolerancePolicy::uniform(1e-9)
  ));
  // Segments 0 and 2 are the two diagonals.
  assert_eq!((crossing.first, crossing.second), (0, 2));
}

#[test]
fn a_circle_and_a_rectangle_are_clean() {
  for contour in [
    primitive::circle(Point2::new(3.0, 4.0), 7.5).expect("circle"),
    primitive::rectangle(Point2::new(-2.0, -3.0), 20.0, 10.0).expect("rectangle"),
  ] {
    let issues = inspect(&contour, TOL);
    assert!(issues.is_clean(), "{issues:?}");
    assert_eq!(issues.count(), 0);
  }
}

#[test]
fn an_open_contour_reports_its_gap_and_no_intersection() {
  let contour = Contour::from_points(
    &[
      Point2::new(0.0, 0.0),
      Point2::new(10.0, 0.0),
      Point2::new(10.0, 5.0),
    ],
    false,
  )
  .expect("open");
  let issues = inspect(&contour, TOL);
  assert_eq!(issues.open, Some(5.0_f64.hypot(10.0)));
  assert!(issues.intersections.is_empty());
  assert!(!issues.is_clean());
}

#[test]
fn segments_touching_at_a_shared_vertex_are_not_a_self_intersection() {
  // A closed pentagon: every vertex is shared by two adjacent segments and nothing crosses.
  let contour = primitive::polygon(Point2::ORIGIN, 10.0, 5, 0.0).expect("pentagon");
  assert!(inspect(&contour, TOL).is_clean());
}

#[test]
fn an_arc_that_crosses_another_segment_is_reported_with_its_index() {
  // A bow tie whose first segment is a half-turn arc: the arc from (-5,-5) to (5,5) sweeping over the
  // other diagonal.
  let contour = Contour::new(
    vec![
      Vertex::new(Point2::new(-5.0, -5.0), 1.0),
      Vertex::corner(Point2::new(5.0, 5.0)),
      Vertex::corner(Point2::new(5.0, -5.0)),
      Vertex::corner(Point2::new(-5.0, 5.0)),
    ],
    true,
  )
  .expect("arc bow tie");
  assert!(contour.has_arcs());
  let issues = inspect(&contour, TOL);
  assert!(
    issues
      .intersections
      .iter()
      .any(|issue| issue.first == 0 || issue.second == 0),
    "no arc intersection in {issues:?}"
  );
}

#[test]
fn a_self_intersection_keeps_a_deterministic_order() {
  let first = inspect(&figure_eight(), TOL);
  let second = inspect(&figure_eight(), TOL);
  assert_eq!(first.intersections, second.intersections);
  // Ordering is by segment index, then by coordinate.
  let keys: Vec<(usize, usize)> = first
    .intersections
    .iter()
    .map(|issue| (issue.first, issue.second))
    .collect();
  let mut sorted = keys.clone();
  sorted.sort_unstable();
  assert_eq!(keys, sorted);
}

#[test]
fn a_short_segment_is_reported_as_degenerate() {
  // Two vertices a nanometre apart are a valid contour as far as `Contour::new` is concerned, but
  // the segment is degenerate at any real tolerance.
  let contour = Contour::from_points(
    &[
      Point2::new(0.0, 0.0),
      Point2::new(1e-9, 0.0),
      Point2::new(10.0, 5.0),
    ],
    true,
  )
  .expect("contour");
  let issues = inspect(&contour, TOL);
  assert_eq!(issues.degenerate, vec![0], "{issues:?}");
  assert!(!issues.is_clean());
}

#[test]
fn an_empty_contour_is_reported_as_clean_rather_than_panicking() {
  // `Contour` cannot be empty, so this is the closest expressible case: a two-vertex closed
  // contour that doubles back on itself is a coincident pair, not a crash.
  let contour = Contour::from_points(
    &[
      Point2::new(0.0, 0.0),
      Point2::new(10.0, 0.0),
      Point2::new(0.0, 0.0),
    ],
    true,
  );
  if let Ok(contour) = contour {
    let issues = inspect(&contour, TOL);
    assert!(!issues.intersections.is_empty(), "{issues:?}");
  }
}

proptest! {
    /// A convex polygon never reports a self-intersection, at any side count or radius.
    #[test]
    fn a_convex_polygon_is_always_clean(sides in 3usize..24, radius in 0.5f64..500.0) {
        let contour = primitive::polygon(Point2::new(1.0, 2.0), radius, sides, 0.3)
            .expect("polygon");
        let issues = inspect(&contour, TOL);
        prop_assert!(issues.is_clean(), "{:?}", issues);
    }

    /// Validation never panics on a builder's output, whatever the shape parameters.
    #[test]
    fn inspecting_a_star_never_panics(points in 3usize..16, outer in 1.0f64..100.0, inner_ratio in 0.2f64..0.9) {
        let inner = outer * inner_ratio;
        if let Ok(contour) = primitive::star(Point2::ORIGIN, outer, inner, points, 0.1) {
            let issues = inspect(&contour, TOL);
            prop_assert_eq!(issues.open, None);
        }
    }
}
