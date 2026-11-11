//! Behaviour tests for arc-preserving contour offsetting (A2).

use cam_geom::offset::{offset_contour, OffsetOptions, OffsetWarning};
use cam_geom::{Contour, Point2};
use proptest::prelude::*;
use std::f64::consts::PI;

fn rect(w: f64, h: f64) -> Contour {
  Contour::from_points(
    &[
      Point2::new(0.0, 0.0),
      Point2::new(w, 0.0),
      Point2::new(w, h),
      Point2::new(0.0, h),
    ],
    true,
  )
  .expect("rectangle")
}

fn circle(radius: f64) -> Contour {
  Contour::new(
    vec![
      cam_geom::Vertex::new(Point2::new(radius, 0.0), 1.0),
      cam_geom::Vertex::new(Point2::new(-radius, 0.0), 1.0),
    ],
    true,
  )
  .expect("circle from two half arcs")
}

fn options() -> OffsetOptions {
  OffsetOptions::default()
}

/// The U bracket an existing region test already uses to reach the self-intersection branch, with a
/// shallow arc along its base. Shrunk by 8 mm its arm walls cross, which is the cleanup path.
fn bracket() -> Contour {
  Contour::new(
    vec![
      cam_geom::Vertex::new(Point2::new(0.0, 0.0), 0.05),
      cam_geom::Vertex::new(Point2::new(100.0, 0.0), 0.0),
      cam_geom::Vertex::new(Point2::new(100.0, 100.0), 0.0),
      cam_geom::Vertex::new(Point2::new(85.0, 100.0), 0.0),
      cam_geom::Vertex::new(Point2::new(85.0, 40.0), 0.0),
      cam_geom::Vertex::new(Point2::new(15.0, 40.0), 0.0),
      cam_geom::Vertex::new(Point2::new(15.0, 100.0), 0.0),
      cam_geom::Vertex::new(Point2::new(0.0, 100.0), 0.0),
    ],
    true,
  )
  .expect("a U bracket with an arc along its base")
}

#[test]
fn the_offset_starts_at_the_corner_nearest_the_authored_start() {
  // Lead-ins and tab positions are measured from the start of the contour, so the offset has to keep
  // pointing at the same place instead of silently rotating the path.
  let contour = rect(40.0, 20.0);
  let offset = offset_contour(&contour, 3.0, &options()).expect("offset");
  let start = contour.vertices()[0].point;
  let nearest = offset
    .contour
    .vertices()
    .iter()
    .map(|vertex| vertex.point.distance_to(start))
    .fold(f64::INFINITY, f64::min);
  let first = offset.contour.vertices()[0].point.distance_to(start);
  assert!(
    (first - nearest).abs() < 1.0e-9,
    "start drifted: {first} vs {nearest}"
  );
}

#[test]
fn zero_distance_is_the_identity() {
  let contour = rect(40.0, 20.0);
  let offset = offset_contour(&contour, 0.0, &options()).expect("identity offset");
  assert_eq!(offset.contour, contour);
  assert!(!offset.flattened);
  assert!(offset.warnings.is_empty());
}

#[test]
fn rectangle_grows_outward_by_the_distance_on_every_side() {
  let contour = rect(40.0, 20.0);
  let offset = offset_contour(&contour, 3.0, &options()).expect("outward offset");
  assert_eq!(offset.contour.len(), 4, "a rectangle stays four corners");
  assert!(!offset.contour.has_arcs());
  assert!(!offset.flattened);
  let expected = (40.0 + 6.0) * (20.0 + 6.0);
  assert!(
    (offset.contour.signed_area() - expected).abs() < 1.0e-9,
    "{} vs {}",
    offset.contour.signed_area(),
    expected
  );
  let bounds = offset.contour.bounds();
  assert!((bounds.min.x + 3.0).abs() < 1.0e-9);
  assert!((bounds.max.x - 43.0).abs() < 1.0e-9);
  assert!((bounds.min.y + 3.0).abs() < 1.0e-9);
  assert!((bounds.max.y - 23.0).abs() < 1.0e-9);
}

#[test]
fn rectangle_shrinks_inward_and_keeps_its_winding() {
  let contour = rect(40.0, 20.0);
  let offset = offset_contour(&contour, -3.0, &options()).expect("inward offset");
  assert_eq!(offset.contour.len(), 4);
  assert!(offset.contour.is_ccw());
  let expected = (40.0 - 6.0) * (20.0 - 6.0);
  assert!((offset.contour.signed_area() - expected).abs() < 1.0e-9);
}

#[test]
fn circle_offset_preserves_arcs() {
  // The A2 evidence: an offset circle is still two arcs, not a polygon.
  let contour = circle(25.0);
  let offset = offset_contour(&contour, 3.0, &options()).expect("outward circle offset");
  assert!(!offset.flattened, "no flattening for a simple offset");
  assert!(offset.contour.has_arcs(), "arcs must survive the offset");
  let segments = offset.contour.segments();
  assert_eq!(segments.len(), 2);
  for segment in &segments {
    let arc = segment.as_arc().expect("still an arc");
    assert!((arc.radius() - 28.0).abs() < 1.0e-9, "{}", arc.radius());
  }
  let expected = PI * 28.0 * 28.0;
  assert!((offset.contour.signed_area() - expected).abs() / expected < 1.0e-9);
}

#[test]
fn inward_circle_offset_still_preserves_arcs() {
  let contour = circle(25.0);
  let offset = offset_contour(&contour, -10.0, &options()).expect("inward circle offset");
  assert!(offset.contour.has_arcs());
  for segment in offset.contour.segments() {
    let arc = segment.as_arc().expect("still an arc");
    assert!((arc.radius() - 15.0).abs() < 1.0e-9);
  }
}

#[test]
fn clockwise_contour_offsets_towards_its_own_outside() {
  let counter_clockwise = rect(40.0, 20.0);
  let clockwise = counter_clockwise.reverse();
  let offset = offset_contour(&clockwise, 3.0, &options()).expect("outward on cw contour");
  // Positive distance still means "away from the material", so the region grows.
  assert!(
    offset.contour.signed_area().abs() > clockwise.signed_area().abs(),
    "{} vs {}",
    offset.contour.signed_area(),
    clockwise.signed_area()
  );
  assert!(!offset.contour.is_ccw(), "winding must be preserved");
}

#[test]
fn offsetting_beyond_the_feature_size_is_refused_not_silently_wrong() {
  let contour = rect(40.0, 20.0);
  let result = offset_contour(&contour, -12.0, &options());
  assert!(result.is_err(), "expected a collapse, got {result:?}");
}

#[test]
fn a_collapsing_arc_radius_is_refused() {
  let contour = circle(5.0);
  let result = offset_contour(&contour, -6.0, &options());
  assert!(result.is_err(), "expected a radius collapse");
}

#[test]
fn sharp_corners_are_reported_when_the_miter_limit_is_hit() {
  // A needle triangle: the interior angle at the tip is about 0.6 degrees, so the miter join would
  // sit ~200 mm away for a 1 mm offset. The limit must catch it and say so.
  let contour = Contour::from_points(
    &[
      Point2::new(0.0, 0.0),
      Point2::new(100.0, 0.0),
      Point2::new(0.0, 1.0),
    ],
    true,
  )
  .expect("needle triangle");
  let offset = offset_contour(&contour, 1.0, &options()).expect("offset of a sharp triangle");
  assert!(
    offset.warnings.contains(&OffsetWarning::MiterLimited),
    "expected a miter warning, got {:?}",
    offset.warnings
  );
  assert!(offset.contour.is_closed());
}

proptest! {
    /// An outward offset of a convex polygon stays simple, keeps its winding and grows its area.
    #[test]
    fn convex_outward_offsets_stay_simple(
        vertices in 5usize..12,
        radius in 10.0f64..200.0,
        distance in 0.2f64..4.0,
    ) {
        let points: Vec<Point2> = (0..vertices)
            .map(|index| {
                let angle = std::f64::consts::TAU * index as f64 / vertices as f64;
                Point2::new(radius * angle.cos(), radius * angle.sin())
            })
            .collect();
        let contour = Contour::from_points(&points, true).expect("convex polygon");
        let offset = offset_contour(&contour, distance, &options()).expect("outward offset");
        prop_assert!(!offset.flattened, "a convex outward offset must not need flattening");
        prop_assert!(offset.contour.is_ccw());
        prop_assert!(offset.contour.signed_area() > contour.signed_area());
    }

    /// Outward then equally inward returns to the original within a tight tolerance.
    #[test]
    fn offset_round_trip(w in 20.0f64..200.0, h in 20.0f64..200.0, distance in 0.5f64..5.0) {
        let contour = rect(w, h);
        let out = offset_contour(&contour, distance, &options()).expect("outward");
        let back = offset_contour(&out.contour, -distance, &options()).expect("inward");
        let expected = w * h;
        prop_assert!(
            (back.contour.signed_area() - expected).abs() / expected < 1.0e-6,
            "area {} vs {}",
            back.contour.signed_area(),
            expected
        );
    }

    /// A rectangle offset outward keeps exactly four sharp corners.
    #[test]
    fn rectangle_offsets_keep_four_corners(w in 10.0f64..500.0, h in 10.0f64..500.0, distance in 0.1f64..4.0) {
        let contour = rect(w, h);
        let offset = offset_contour(&contour, distance, &options()).expect("offset");
        prop_assert_eq!(offset.contour.len(), 4);
        prop_assert!(!offset.contour.has_arcs());
        prop_assert!(offset.warnings.is_empty() || offset.warnings == vec![OffsetWarning::Flattened]);
    }
}

#[test]
fn offsetting_an_open_contour_is_a_refusal_not_a_collapse() {
  // The input was never offsettable. Reporting it as a collapse told the user their geometry
  // collapsed, which it did not.
  let open = cam_geom::primitive::line(
    cam_geom::Point2::new(0.0, 0.0),
    cam_geom::Point2::new(10.0, 0.0),
  )
  .expect("valid line");
  let error = offset_contour(&open, 1.0, &OffsetOptions::default())
    .expect_err("an open contour has no inside to offset towards");
  assert!(
    matches!(error, cam_geom::GeomError::EditRefused { .. }),
    "the refusal class must say the input was refused, not that it collapsed: {error:?}"
  );
  assert!(
    error.to_string().contains("only closed contours"),
    "the reason must name the precondition: {error}"
  );
}

#[test]
fn a_self_intersecting_offset_keeps_the_arcs_it_started_with() {
  // G3.2: shrinking this U bracket by 8 mm makes the two offset walls of an arm cross, which is the
  // one path in the kernel that used to rebuild the contour out of sampled points. The base is a
  // shallow arc, so if the cleanup keeps the curve the arc is still there afterwards; a flattened
  // rebuild loses it. The bracket's own geometry is the fixture an existing region test already
  // uses to reach this branch, with one arc added.
  let offset = offset_contour(&bracket(), -8.0, &options()).expect("the bracket offsets");
  assert!(
    !offset.flattened,
    "the curve survives the self-intersection cleanup: {:?}",
    offset.warnings
  );
  assert!(
    offset.contour.has_arcs(),
    "the arc along the base is still an arc"
  );
}

#[test]
fn a_self_intersecting_offset_keeps_the_largest_boundary() {
  // R3-test-does-not-prove-survivor: the cleanup keeps the largest boundary by absolute area, and
  // that choice is the one that decides which geometry survives, so it needs its own pin. The
  // bracket at -8 mm offers exactly two candidates: the eroded base of the block, and the sliver
  // left at the notch mouth where the two arm walls cross. Keeping the smaller one returns the
  // sliver and nothing else, so asserting the base's area is what separates the two.
  let offset = offset_contour(&bracket(), -8.0, &options()).expect("the bracket offsets");
  let area = offset.contour.signed_area().abs();
  assert!(
    (area - 2175.88).abs() < 1.0,
    "the surviving boundary is the eroded base of the block (~2175.88 mm²: 70 mm x 24 mm of floor \
     plus the two mouth caps), not the notch sliver: {area}"
  );
}
