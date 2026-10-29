//! Behaviour tests for joining contours (M2 A8).
//!
//! A join that guesses is worse than one that refuses: the wrong chain produces a cut path nobody
//! asked for, and the mistake only appears in the machine.

use cam_geom::join;
use cam_geom::{primitive, Contour, GeomError, Point2, Vertex};

const TOL: f64 = 1.0e-9;

fn line(ax: f64, ay: f64, bx: f64, by: f64) -> Contour {
  primitive::line(Point2::new(ax, ay), Point2::new(bx, by)).expect("valid line")
}

#[test]
fn two_lines_sharing_an_endpoint_become_one_contour() {
  let joined = join::join(&[line(0.0, 0.0, 5.0, 0.0), line(5.0, 0.0, 10.0, 0.0)], TOL)
    .expect("the lines meet end to end");
  assert_eq!(joined.len(), 3);
  assert!(!joined.is_closed());
  assert!((joined.perimeter() - 10.0).abs() < 1.0e-12);
  assert_eq!(joined.vertices()[2].point, Point2::new(10.0, 0.0));
}

#[test]
fn a_contour_drawn_end_to_start_is_reversed_to_fit() {
  // The second line is drawn backwards, which costs nothing to the user and must cost nothing here.
  let joined = join::join(&[line(0.0, 0.0, 5.0, 0.0), line(10.0, 0.0, 5.0, 0.0)], TOL)
    .expect("the endpoints still match");
  assert_eq!(joined.len(), 3);
  assert_eq!(joined.vertices()[0].point, Point2::new(0.0, 0.0));
  assert_eq!(joined.vertices()[2].point, Point2::new(10.0, 0.0));
  assert!((joined.perimeter() - 10.0).abs() < 1.0e-12);
}

#[test]
fn joining_a_ring_of_lines_closes_the_contour() {
  let square = join::join(
    &[
      line(0.0, 0.0, 10.0, 0.0),
      line(10.0, 0.0, 10.0, 10.0),
      line(10.0, 10.0, 0.0, 10.0),
      line(0.0, 10.0, 0.0, 0.0),
    ],
    TOL,
  )
  .expect("the four sides form a ring");
  assert!(
    square.is_closed(),
    "a ring closes instead of repeating its first point"
  );
  assert_eq!(square.len(), 4, "the duplicated corner is dropped");
  assert!((square.perimeter() - 40.0).abs() < 1.0e-12);
  assert!((square.signed_area() - 100.0).abs() < 1.0e-12);
  assert!(square.is_ccw());
}

#[test]
fn arcs_keep_their_bulge_when_joined() {
  let arc = Contour::new(
    vec![
      Vertex::new(Point2::new(0.0, 0.0), 1.0),
      Vertex::corner(Point2::new(10.0, 0.0)),
    ],
    false,
  )
  .expect("valid arc");
  let expected_perimeter = arc.perimeter() + 10.0;
  let joined = join::join(&[arc, line(10.0, 0.0, 20.0, 0.0)], TOL).expect("they meet");
  assert_eq!(joined.len(), 3);
  assert!(joined.has_arcs(), "the arc survives the join");
  assert_eq!(joined.vertices()[0].bulge, 1.0);
  assert!((joined.perimeter() - expected_perimeter).abs() < 1.0e-12);
}

#[test]
fn three_ends_at_one_point_are_ambiguous() {
  let error = join::join(
    &[
      line(0.0, 0.0, 10.0, 0.0),
      line(0.0, 0.0, 0.0, 10.0),
      line(0.0, 0.0, -10.0, 0.0),
    ],
    TOL,
  )
  .expect_err("three ends cannot be chained deterministically");
  let message = error.to_string();
  assert!(message.contains("ambiguous"), "{message}");
}

#[test]
fn a_gap_wider_than_the_tolerance_is_not_closed() {
  let error = join::join(&[line(0.0, 0.0, 5.0, 0.0), line(5.1, 0.0, 10.0, 0.0)], TOL)
    .expect_err("the ends do not meet");
  assert!(matches!(error, GeomError::EditRefused { .. }), "{error:?}");
}

#[test]
fn a_gap_within_the_tolerance_is_closed() {
  let joined = join::join(
    &[line(0.0, 0.0, 5.0, 0.0), line(5.001, 0.0, 10.0, 0.0)],
    0.01,
  )
  .expect("a hundredth of a millimetre is within tolerance");
  assert_eq!(joined.len(), 3, "the two contours become one");
}

#[test]
fn a_group_that_forms_two_chains_is_refused() {
  let error = join::join(
    &[
      line(0.0, 0.0, 1.0, 0.0),
      line(1.0, 0.0, 2.0, 0.0),
      line(100.0, 0.0, 101.0, 0.0),
    ],
    TOL,
  )
  .expect_err("the third line belongs to no chain");
  assert!(matches!(error, GeomError::EditRefused { .. }), "{error:?}");
}

#[test]
fn a_closed_contour_cannot_be_joined() {
  let rectangle = primitive::rectangle(Point2::ORIGIN, 10.0, 4.0).expect("rectangle");
  let error = join::join(&[rectangle, line(0.0, 0.0, 5.0, 0.0)], TOL)
    .expect_err("a closed contour has no free ends");
  assert!(matches!(error, GeomError::EditRefused { .. }), "{error:?}");
}

#[test]
fn joining_a_single_contour_returns_it_unchanged() {
  let original = line(0.0, 0.0, 5.0, 0.0);
  let joined = join::join(std::slice::from_ref(&original), TOL).expect("nothing to do");
  assert_eq!(joined, original);
}

#[test]
fn joining_nothing_is_refused() {
  let error = join::join(&[], TOL).expect_err("there is nothing to join");
  assert!(matches!(error, GeomError::EditRefused { .. }), "{error:?}");
}

#[test]
fn joining_a_mismatched_group_is_refused_without_changing_it() {
  let parts = [line(0.0, 0.0, 5.0, 0.0), line(50.0, 0.0, 60.0, 0.0)];
  let error = join::join(&parts, TOL).expect_err("they do not meet");
  assert!(matches!(error, GeomError::EditRefused { .. }), "{error:?}");
  // The inputs are untouched: join takes them by reference and never edits in place.
  assert_eq!(parts[0], line(0.0, 0.0, 5.0, 0.0));
  assert_eq!(parts[1], line(50.0, 0.0, 60.0, 0.0));
}
