//! Behaviour tests for trimming a contour against a cutter (M2 A9).
//!
//! Trim is only safe when the cut is unambiguous. These tests pin both halves of that: the surviving
//! pieces are the right length, and every cutter that would make the choice a guess is refused.

use cam_geom::trim::{self, TrimOutcome};
use cam_geom::{primitive, Contour, GeomError, Point2};
use std::f64::consts::PI;

const TOL: cam_geom::TolerancePolicy = cam_geom::TolerancePolicy::uniform(1.0e-9);

fn line(ax: f64, ay: f64, bx: f64, by: f64) -> Contour {
  primitive::line(Point2::new(ax, ay), Point2::new(bx, by)).expect("valid line")
}

fn assert_close(actual: f64, expected: f64, tolerance: f64, what: &str) {
  assert!(
    (actual - expected).abs() <= tolerance,
    "{what}: expected {expected}, got {actual}"
  );
}

#[test]
fn a_line_trimmed_by_a_circle_loses_the_middle() {
  let target = line(0.0, 0.0, 10.0, 0.0);
  let cutter = primitive::circle(Point2::new(5.0, 0.0), 1.0).expect("circle");
  let TrimOutcome { kept, removed } = trim::trim(&target, &cutter, TOL).expect("two clear cuts");
  assert_eq!(kept.len(), 2, "the line keeps both of its ends");
  assert_close(removed.perimeter(), 2.0, 1.0e-9, "the removed span");
  assert_close(
    kept.iter().map(Contour::perimeter).sum::<f64>(),
    8.0,
    1.0e-9,
    "the kept spans",
  );
  assert!(kept.iter().all(|piece| !piece.is_closed()));
}

#[test]
fn a_rectangle_trimmed_by_a_line_becomes_one_open_contour() {
  let target = primitive::rectangle(Point2::ORIGIN, 10.0, 10.0).expect("rectangle");
  let cutter = line(5.0, -5.0, 5.0, 15.0);
  let TrimOutcome { kept, removed } = trim::trim(&target, &cutter, TOL).expect("two clear cuts");
  assert_eq!(
    kept.len(),
    1,
    "a closed contour opened by the cut is one piece"
  );
  let kept = &kept[0];
  assert!(!kept.is_closed(), "the surviving piece is open");
  assert_close(
    kept.perimeter() + removed.perimeter(),
    target.perimeter(),
    1.0e-9,
    "nothing is lost or invented",
  );
  assert_close(removed.perimeter(), 20.0, 1.0e-9, "the removed half");
}

#[test]
fn trimming_a_circle_keeps_the_arcs_exact() {
  let target = primitive::circle(Point2::ORIGIN, 5.0).expect("circle");
  let cutter = line(0.0, -10.0, 0.0, 10.0);
  let TrimOutcome { kept, removed } = trim::trim(&target, &cutter, TOL).expect("two clear cuts");
  assert_eq!(kept.len(), 1);
  let kept = &kept[0];
  assert!(kept.has_arcs(), "a half circle is still arcs");
  assert_close(kept.perimeter(), 5.0 * PI, 1.0e-9, "half the circumference");
  // The kept piece is open, so its terminal vertex carries no segment and therefore has a zero
  // bulge by construction (`Contour::from_segments` appends a corner). What must stay arcs are the
  // segments themselves.
  let segments = kept.segments();
  assert_eq!(segments.len(), 2, "the kept half is two arcs");
  assert!(
    segments.iter().all(|segment| segment.as_arc().is_some()),
    "the halves stay arcs"
  );
  assert_close(removed.perimeter(), 5.0 * PI, 1.0e-9, "the removed half");
}

#[test]
fn a_cutter_that_misses_is_refused() {
  let target = line(0.0, 0.0, 10.0, 0.0);
  let cutter = primitive::circle(Point2::new(5.0, 20.0), 1.0).expect("circle");
  let error = trim::trim(&target, &cutter, TOL).expect_err("no cut points");
  assert!(matches!(error, GeomError::EditRefused { .. }), "{error:?}");
  assert!(error.to_string().contains("cut point"), "{error}");
}

#[test]
fn a_tangent_cutter_is_refused() {
  let target = line(-10.0, 0.0, 10.0, 0.0);
  let cutter = primitive::circle(Point2::new(0.0, 1.0), 1.0).expect("circle");
  // The circle touches the line at exactly one point, which cannot divide it.
  let error = trim::trim(&target, &cutter, TOL).expect_err("a tangent is not a cut");
  assert!(matches!(error, GeomError::EditRefused { .. }), "{error:?}");
}

#[test]
fn a_cutter_that_crosses_four_times_is_refused() {
  let target = primitive::rectangle(Point2::ORIGIN, 10.0, 10.0).expect("rectangle");
  let cutter = primitive::rectangle(Point2::new(-5.0, -5.0), 20.0, 20.0).expect("rectangle");
  let error = trim::trim(&target, &cutter, TOL).expect_err("four cuts leave the choice open");
  assert!(matches!(error, GeomError::EditRefused { .. }), "{error:?}");
}

#[test]
fn trimming_leaves_the_inputs_untouched() {
  let target = line(0.0, 0.0, 10.0, 0.0);
  let cutter = primitive::circle(Point2::new(5.0, 0.0), 1.0).expect("circle");
  let before = (target.clone(), cutter.clone());
  let _ = trim::trim(&target, &cutter, TOL).expect("two clear cuts");
  assert_eq!(target, before.0);
  assert_eq!(cutter, before.1);
}
