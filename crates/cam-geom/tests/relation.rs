//! Behaviour tests for how two closed contours sit relative to each other (A2).
//!
//! This is the only place that decides containment, and the island validation trusts it: "this hole is
//! inside that pocket" must not be a guess made from bounding boxes, because a rectangle's bounds
//! contain a circle's bounds without containing the circle.

use proptest::prelude::*;

use cam_geom::primitive;
use cam_geom::winding::{relation, Relation};
use cam_geom::{Contour, Point2};

fn square(at: Point2, size: f64) -> Contour {
  primitive::rectangle(at, size, size).expect("square")
}

fn circle(centre: Point2, radius: f64) -> Contour {
  primitive::circle(centre, radius).expect("circle")
}

#[test]
fn a_circle_inside_a_square_is_first_inside() {
  let inner = circle(Point2::new(10.0, 10.0), 3.0);
  let outer = square(Point2::new(0.0, 0.0), 20.0);
  assert_eq!(relation(&inner, &outer, 1.0e-9), Relation::FirstInside);
  assert_eq!(relation(&outer, &inner, 1.0e-9), Relation::SecondInside);
}

#[test]
fn the_tolerance_decides_how_close_counts_as_meeting() {
  // Two squares 0.5 nm apart: they meet at a 1 nm tolerance and are apart at a picometre one. The
  // tolerance is threaded into the intersection test, not left to the caller to guess.
  let first = square(Point2::new(0.0, 0.0), 10.0);
  let second = square(Point2::new(10.0 + 5.0e-10, 0.0), 10.0);
  assert_eq!(relation(&first, &second, 1.0e-9), Relation::Touching);
  assert_eq!(relation(&first, &second, 1.0e-12), Relation::Disjoint);
}

#[test]
fn two_overlapping_squares_cross() {
  let first = square(Point2::new(0.0, 0.0), 10.0);
  let second = square(Point2::new(5.0, 5.0), 10.0);
  assert_eq!(relation(&first, &second, 1.0e-9), Relation::Crossing);
  assert_eq!(relation(&second, &first, 1.0e-9), Relation::Crossing);
}

#[test]
fn a_contour_crossing_an_arc_is_a_crossing() {
  let plate = square(Point2::new(-10.0, -10.0), 20.0);
  let hole = circle(Point2::new(10.0, 0.0), 5.0);
  assert_eq!(relation(&plate, &hole, 1.0e-9), Relation::Crossing);
}

#[test]
fn two_separate_squares_are_disjoint() {
  let first = square(Point2::new(0.0, 0.0), 10.0);
  let second = square(Point2::new(20.0, 0.0), 10.0);
  assert_eq!(relation(&first, &second, 1.0e-9), Relation::Disjoint);
  assert_eq!(relation(&second, &first, 1.0e-9), Relation::Disjoint);
}

#[test]
fn squares_sharing_an_edge_touch_rather_than_cross() {
  let first = square(Point2::new(0.0, 0.0), 10.0);
  let second = square(Point2::new(10.0, 0.0), 10.0);
  assert_eq!(relation(&first, &second, 1.0e-9), Relation::Touching);
}

#[test]
fn a_circle_tangent_to_the_inside_of_a_square_touches() {
  let inner = circle(Point2::new(10.0, 10.0), 10.0);
  let outer = square(Point2::new(0.0, 0.0), 20.0);
  assert_eq!(relation(&inner, &outer, 1.0e-9), Relation::Touching);
}

#[test]
fn identical_contours_touch() {
  let first = square(Point2::new(0.0, 0.0), 10.0);
  let second = square(Point2::new(0.0, 0.0), 10.0);
  assert_eq!(relation(&first, &second, 1.0e-9), Relation::Touching);
}

#[test]
fn a_shape_in_the_notch_of_another_is_disjoint_from_it() {
  // The C's bounds contain the small square in the notch, so containment cannot be a bounds question.
  let c = Contour::from_points(
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
  .expect("c shape");
  let in_the_notch = square(Point2::new(18.0, 8.0), 4.0);
  assert_eq!(relation(&in_the_notch, &c, 1.0e-9), Relation::Disjoint);
  assert_eq!(relation(&c, &in_the_notch, 1.0e-9), Relation::Disjoint);
}

#[test]
fn an_arc_inside_another_arc_is_inside_it() {
  let inner = circle(Point2::new(1.0, 1.0), 2.0);
  let outer = circle(Point2::new(0.0, 0.0), 6.0);
  assert_eq!(relation(&inner, &outer, 1.0e-9), Relation::FirstInside);
}

proptest! {
  /// Containment cannot be symmetric: whatever one contour is to another, the other must be the mirror
  /// image to it, and never both "inside" each other.
  #[test]
  fn the_relation_is_antisymmetric(
    ax in -20.0f64..20.0,
    ay in -20.0f64..20.0,
    asize in 1.0f64..20.0,
    bx in -20.0f64..20.0,
    by in -20.0f64..20.0,
    bsize in 1.0f64..20.0,
  ) {
    let first = square(Point2::new(ax, ay), asize);
    let second = square(Point2::new(bx, by), bsize);
    let forward = relation(&first, &second, 1.0e-9);
    let backward = relation(&second, &first, 1.0e-9);
    let mirror = match forward {
      Relation::Disjoint => Relation::Disjoint,
      Relation::FirstInside => Relation::SecondInside,
      Relation::SecondInside => Relation::FirstInside,
      Relation::Crossing => Relation::Crossing,
      Relation::Touching => Relation::Touching,
    };
    prop_assert_eq!(backward, mirror, "first {:?} second {:?}", first.bounds(), second.bounds());
    prop_assert!(
      !(forward == Relation::FirstInside && backward == Relation::FirstInside),
      "two contours cannot each be inside the other"
    );
  }
}
