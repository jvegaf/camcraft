//! Behaviour tests for identifying a hole's centre (A7, D-M4-5).
//!
//! A drill must plunge where the user drew the circle, so this is the rule that decides whether a
//! selected contour is a hole at all. Anything else is refused by name rather than guessed.

use std::f64::consts::{FRAC_PI_2, TAU};

use cam_geom::{primitive, Contour, Point2, Segment, Vertex};
use cam_ops::circle_centre;

fn point_on_circle(centre: Point2, radius: f64, quarter: usize) -> Point2 {
  let angle = quarter as f64 * FRAC_PI_2;
  Point2::new(
    centre.x + radius * angle.cos(),
    centre.y + radius * angle.sin(),
  )
}

#[test]
fn a_circle_built_from_two_half_turns_reports_its_centre() {
  let contour = primitive::circle(Point2::new(12.5, -4.0), 3.0).expect("circle");
  assert_eq!(
    circle_centre(&contour),
    Some(Point2::new(12.5, -4.0)),
    "a circle drills at its centre"
  );
}

#[test]
fn a_circle_built_from_four_quarter_arcs_reports_the_same_centre() {
  let centre = Point2::new(7.0, -2.0);
  let radius = 5.0;
  // A quarter turn's bulge is `tan(sweep / 4)`, and the sweep is a quarter turn.
  let bulge = (FRAC_PI_2 / 4.0).tan();
  let vertices = (0..4)
    .map(|quarter| Vertex::new(point_on_circle(centre, radius, quarter), bulge))
    .collect();
  let contour = Contour::new(vertices, true).expect("four quarter arcs");
  assert_eq!(contour.segments().len(), 4);
  assert_eq!(circle_centre(&contour), Some(centre));
}

#[test]
fn a_clockwise_circle_reports_the_same_centre() {
  let centre = Point2::new(-3.0, 4.0);
  let radius = 2.0;
  let bulge = (FRAC_PI_2 / 4.0).tan();
  let vertices = (0..4)
    .rev()
    .map(|quarter| Vertex::new(point_on_circle(centre, radius, quarter), -bulge))
    .collect();
  let contour = Contour::new(vertices, true).expect("four clockwise quarter arcs");
  assert_eq!(circle_centre(&contour), Some(centre));
}

#[test]
fn a_rounded_rectangle_is_not_a_hole() {
  let contour = Contour::new(
    vec![
      Vertex::new(Point2::new(0.0, 0.0), 0.0),
      Vertex::new(Point2::new(20.0, 0.0), 0.5),
      Vertex::new(Point2::new(20.0, 20.0), 0.0),
      Vertex::new(Point2::new(0.0, 20.0), 0.5),
    ],
    true,
  )
  .expect("rounded rectangle");
  assert!(contour
    .segments()
    .iter()
    .any(|segment| matches!(segment, Segment::Arc(_))));
  assert_eq!(
    circle_centre(&contour),
    None,
    "a shape made of lines and arcs is not a circle"
  );
}

#[test]
fn a_square_is_not_a_hole() {
  let contour = primitive::rectangle(Point2::new(0.0, 0.0), 20.0, 20.0).expect("square");
  assert_eq!(circle_centre(&contour), None);
}

#[test]
fn an_open_pair_of_half_turns_is_not_a_hole() {
  let contour = Contour::new(
    vec![
      Vertex::new(Point2::new(5.0, 0.0), 1.0),
      Vertex::new(Point2::new(-5.0, 0.0), 1.0),
    ],
    false,
  )
  .expect("open pair of half turns");
  assert_eq!(circle_centre(&contour), None);
}

#[test]
fn arcs_with_different_centres_are_not_a_hole() {
  // All segments are arcs, but they do not share a centre and do not add up to a turn, so a drilled
  // point would be a guess: refused rather than approximated.
  let contour = Contour::new(
    vec![
      Vertex::new(Point2::new(0.0, 0.0), 1.0),
      Vertex::new(Point2::new(10.0, 0.0), 0.2),
      Vertex::new(Point2::new(10.0, 10.0), 1.0),
      Vertex::new(Point2::new(0.0, 10.0), 0.2),
    ],
    true,
  )
  .expect("four bulged segments");
  assert!(contour
    .segments()
    .iter()
    .all(|segment| matches!(segment, Segment::Arc(_))));
  let centres: Vec<Point2> = contour
    .segments()
    .iter()
    .map(|segment| match segment {
      Segment::Arc(arc) => arc.center(),
      Segment::Line(_) => unreachable!(),
    })
    .collect();
  assert!(
    centres.windows(2).any(|pair| pair[0] != pair[1]),
    "the arcs differ in centre, so no single centre is a hole: {centres:?}"
  );
  assert_eq!(circle_centre(&contour), None);
}

#[test]
fn the_centre_is_exact_for_a_tiny_circle() {
  let contour = primitive::circle(Point2::new(-1.0, 2.0), 0.05).expect("small circle");
  assert_eq!(circle_centre(&contour), Some(Point2::new(-1.0, 2.0)));
  assert!((contour.perimeter() - TAU * 0.05).abs() < 1.0e-9);
}
