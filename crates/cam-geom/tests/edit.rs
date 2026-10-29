//! Behaviour tests for node editing (M2 A6).
//!
//! Inserting a node on an arc is the case where a naive implementation quietly rewrites the geometry:
//! the two halves must be the *same* circle, with sweeps that sum to the original, not a new arc
//! bulged through the click point.

use cam_geom::edit;
use cam_geom::{primitive, Contour, GeomError, Point2, Vertex};
use std::f64::consts::PI;

fn assert_close(actual: f64, expected: f64, tolerance: f64, what: &str) {
  assert!(
    (actual - expected).abs() <= tolerance,
    "{what}: expected {expected}, got {actual}"
  );
}

/// The upper half of the circle of radius 5 centred on the origin, as one arc.
fn half_circle() -> Contour {
  Contour::new(
    vec![
      Vertex::new(Point2::new(5.0, 0.0), 1.0),
      Vertex::corner(Point2::new(-5.0, 0.0)),
    ],
    false,
  )
  .expect("valid half circle")
}

#[test]
fn move_vertex_moves_one_point_and_keeps_the_rest() {
  let rectangle = primitive::rectangle(Point2::ORIGIN, 10.0, 4.0).expect("rectangle");
  let moved = edit::move_vertex(&rectangle, 1, Point2::new(12.0, -1.0)).expect("valid move");
  assert_eq!(moved.len(), 4);
  assert_eq!(moved.vertices()[1].point, Point2::new(12.0, -1.0));
  assert_eq!(moved.vertices()[0], rectangle.vertices()[0]);
  assert!(moved.is_closed(), "the contour stays closed");
}

#[test]
fn moving_a_vertex_on_a_closed_contour_keeps_it_closed() {
  let circle = primitive::circle(Point2::ORIGIN, 5.0).expect("circle");
  let moved = edit::move_vertex(&circle, 0, Point2::new(6.0, 0.0)).expect("valid move");
  assert!(moved.is_closed());
  assert_eq!(moved.len(), 2);
  assert_eq!(moved.vertices()[0].point, Point2::new(6.0, 0.0));
  assert_eq!(moved.vertices()[0].bulge, 1.0, "the bulge is untouched");
}

#[test]
fn move_vertex_out_of_range_is_refused() {
  let rectangle = primitive::rectangle(Point2::ORIGIN, 10.0, 4.0).expect("rectangle");
  let error = edit::move_vertex(&rectangle, 9, Point2::new(1.0, 1.0)).expect_err("no such vertex");
  assert!(matches!(error, GeomError::EditRefused { .. }), "{error:?}");
}

#[test]
fn moving_a_vertex_onto_its_neighbour_is_refused() {
  let line = primitive::line(Point2::ORIGIN, Point2::new(10.0, 0.0)).expect("line");
  let error = edit::move_vertex(&line, 0, Point2::new(10.0, 0.0)).expect_err("zero-length segment");
  assert!(matches!(error, GeomError::EditRefused { .. }), "{error:?}");
}

#[test]
fn inserting_on_a_line_splits_it_into_collinear_vertices() {
  let line = primitive::line(Point2::ORIGIN, Point2::new(10.0, 0.0)).expect("line");
  // The click is off the line; it is projected onto it, which is what a drawing tool does.
  let split = edit::insert_vertex(&line, 0, Point2::new(4.0, 2.0)).expect("valid insert");
  assert_eq!(split.len(), 3);
  assert!(!split.is_closed());
  assert!(!split.has_arcs(), "a straight segment stays straight");
  assert_close(split.vertices()[1].point.x, 4.0, 1.0e-12, "inserted x");
  assert_close(split.vertices()[1].point.y, 0.0, 1.0e-12, "inserted y");
  assert_close(split.perimeter(), 10.0, 1.0e-12, "length is unchanged");
}

#[test]
fn inserting_on_an_arc_preserves_the_arc_exactly() {
  let contour = half_circle();
  let click = Point2::new(0.5, 6.0);
  let split = edit::insert_vertex(&contour, 0, click).expect("valid insert");
  assert_eq!(split.len(), 3);
  assert!(split.has_arcs());
  let segments = split.segments();
  assert_eq!(segments.len(), 2);
  let mut total_sweep = 0.0;
  for segment in &segments {
    let arc = segment.as_arc().expect("both halves are arcs");
    assert_close(arc.radius(), 5.0, 1.0e-12, "both halves keep the radius");
    assert_close(
      arc.center().x,
      0.0,
      1.0e-12,
      "both halves keep the centre x",
    );
    assert_close(
      arc.center().y,
      0.0,
      1.0e-12,
      "both halves keep the centre y",
    );
    total_sweep += arc.sweep();
  }
  assert_close(
    total_sweep,
    PI,
    1.0e-12,
    "the sweeps still add up to half a turn",
  );
  assert_close(
    split.perimeter(),
    contour.perimeter(),
    1.0e-12,
    "length is unchanged",
  );
  // The new vertex is the click projected radially onto the circle, not the click itself.
  let inserted = split.vertices()[1].point;
  assert_close(
    inserted.distance_to(Point2::ORIGIN),
    5.0,
    1.0e-12,
    "projected onto the arc",
  );
  let direction = (click - Point2::ORIGIN)
    .normalized()
    .expect("the click has a direction");
  let expected = Point2::ORIGIN + direction * 5.0;
  assert!(
    inserted.is_near(expected, 1.0e-12),
    "expected the radial projection {expected:?}, got {inserted:?}"
  );
}

#[test]
fn inserting_beyond_the_segment_is_refused() {
  let line = primitive::line(Point2::ORIGIN, Point2::new(10.0, 0.0)).expect("line");
  let error =
    edit::insert_vertex(&line, 0, Point2::new(15.0, 1.0)).expect_err("outside the segment");
  assert!(matches!(error, GeomError::EditRefused { .. }), "{error:?}");
}

#[test]
fn inserting_at_an_endpoint_is_refused() {
  let line = primitive::line(Point2::ORIGIN, Point2::new(10.0, 0.0)).expect("line");
  let error = edit::insert_vertex(&line, 0, Point2::new(0.0, 0.0)).expect_err("on an endpoint");
  assert!(matches!(error, GeomError::EditRefused { .. }), "{error:?}");
}

#[test]
fn inserting_on_a_segment_that_does_not_exist_is_refused() {
  let line = primitive::line(Point2::ORIGIN, Point2::new(10.0, 0.0)).expect("line");
  let error = edit::insert_vertex(&line, 5, Point2::new(4.0, 0.0)).expect_err("no such segment");
  assert!(matches!(error, GeomError::EditRefused { .. }), "{error:?}");
}

#[test]
fn inserting_on_the_closing_segment_of_a_closed_contour_works() {
  let rectangle = primitive::rectangle(Point2::ORIGIN, 10.0, 4.0).expect("rectangle");
  // The closing segment runs from the fourth corner back to the first.
  let split = edit::insert_vertex(&rectangle, 3, Point2::new(-1.0, 2.0)).expect("valid insert");
  assert_eq!(split.len(), 5);
  assert!(split.is_closed());
  assert_close(
    split.vertices()[4].point.y,
    2.0,
    1.0e-12,
    "inserted on the left edge",
  );
  assert_close(split.signed_area(), 40.0, 1.0e-12, "the area is unchanged");
}

#[test]
fn deleting_a_vertex_removes_it_and_keeps_the_contour_valid() {
  let contour = primitive::polyline(
    &[
      Point2::new(0.0, 0.0),
      Point2::new(5.0, 0.0),
      Point2::new(10.0, 0.0),
    ],
    false,
  )
  .expect("polyline");
  let shorter = edit::delete_vertex(&contour, 1).expect("valid delete");
  assert_eq!(shorter.len(), 2);
  assert_close(shorter.perimeter(), 10.0, 1.0e-12, "length is unchanged");
}

#[test]
fn deleting_below_two_vertices_is_refused() {
  let line = primitive::line(Point2::ORIGIN, Point2::new(10.0, 0.0)).expect("line");
  let error = edit::delete_vertex(&line, 0).expect_err("a contour needs two vertices");
  assert!(matches!(error, GeomError::EditRefused { .. }), "{error:?}");
}

#[test]
fn deleting_a_vertex_out_of_range_is_refused() {
  let rectangle = primitive::rectangle(Point2::ORIGIN, 10.0, 4.0).expect("rectangle");
  let error = edit::delete_vertex(&rectangle, 7).expect_err("no such vertex");
  assert!(matches!(error, GeomError::EditRefused { .. }), "{error:?}");
}

#[test]
fn deleting_a_vertex_keeps_the_bulge_of_the_vertex_that_survives() {
  // An arc from the first vertex to the second, then a straight segment to the third.
  let contour = Contour::new(
    vec![
      Vertex::new(Point2::new(0.0, 0.0), 1.0),
      Vertex::corner(Point2::new(10.0, 0.0)),
      Vertex::corner(Point2::new(20.0, 0.0)),
    ],
    false,
  )
  .expect("valid contour");
  let shorter = edit::delete_vertex(&contour, 1).expect("valid delete");
  assert_eq!(shorter.len(), 2);
  assert_eq!(
    shorter.vertices()[0].bulge,
    1.0,
    "the surviving vertex keeps its own bulge"
  );
  assert!(
    shorter.has_arcs(),
    "the arc now spans the two remaining vertices"
  );
}
