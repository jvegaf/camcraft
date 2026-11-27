use cam_geom::primitive::rectangle;
use cam_geom::region::Region;
use cam_geom::{Point2, TolerancePolicy};

use cam_ops::penetration::{
  cut_model_overlaps_copper, intersection_area, penetration_ratio, region_area,
  PENETRATION_THRESHOLD,
};

fn rect_region(x: f64, y: f64, w: f64, h: f64) -> Region {
  Region::new(vec![
    rectangle(Point2::new(x, y), w, h).expect("valid rectangle")
  ])
  .expect("valid region")
}

#[test]
fn no_overlap_is_zero_ratio() {
  let tol = TolerancePolicy::standard();
  let swept = rect_region(0.0, 0.0, 10.0, 10.0);
  let copper = rect_region(20.0, 0.0, 10.0, 10.0);
  assert_eq!(penetration_ratio(&swept, &copper, tol).unwrap(), 0.0);
  assert_eq!(intersection_area(&swept, &copper, tol).unwrap(), 0.0);
  assert!(!cut_model_overlaps_copper(&swept, &copper, tol, PENETRATION_THRESHOLD).unwrap());
}

#[test]
fn injected_penetration_is_detected() {
  let tol = TolerancePolicy::standard();
  // Swept cut intrudes 1mm into a 10x10 protected copper area.
  let swept = rect_region(9.0, 0.0, 10.0, 10.0);
  let copper = rect_region(0.0, 0.0, 10.0, 10.0);
  let ratio = penetration_ratio(&swept, &copper, tol).unwrap();
  let overlap = intersection_area(&swept, &copper, tol).unwrap();
  assert!((overlap - 10.0).abs() < 1.0e-6, "overlap {overlap}");
  assert!((ratio - 0.1).abs() < 1.0e-6, "ratio {ratio}");
  assert!(cut_model_overlaps_copper(&swept, &copper, tol, PENETRATION_THRESHOLD).unwrap());
}

#[test]
fn identical_regions_are_full_penetration() {
  let tol = TolerancePolicy::standard();
  let swept = rect_region(0.0, 0.0, 10.0, 10.0);
  let copper = rect_region(0.0, 0.0, 10.0, 10.0);
  let ratio = penetration_ratio(&swept, &copper, tol).unwrap();
  assert!((ratio - 1.0).abs() < 1.0e-9, "ratio {ratio}");
  assert!((region_area(&copper) - 100.0).abs() < 1.0e-9);
  assert!(cut_model_overlaps_copper(&swept, &copper, tol, PENETRATION_THRESHOLD).unwrap());
}

#[test]
fn empty_protected_copper_is_not_penetrated() {
  let tol = TolerancePolicy::standard();
  let swept = rect_region(0.0, 0.0, 10.0, 10.0);
  let copper = Region::default();
  let ratio = penetration_ratio(&swept, &copper, tol).unwrap();
  assert_eq!(ratio, 0.0);
  assert!(!cut_model_overlaps_copper(&swept, &copper, tol, PENETRATION_THRESHOLD).unwrap());
}

#[test]
fn tiny_injected_penetration_crosses_threshold() {
  let tol = TolerancePolicy::standard();
  // A small but unambiguous overlap injected artificially into a large protected area.
  let copper = rect_region(0.0, 0.0, 1000.0, 1000.0);
  let swept = rect_region(999.99, 0.0, 1.0, 1.0);
  let ratio = penetration_ratio(&swept, &copper, tol).unwrap();
  assert!(ratio > PENETRATION_THRESHOLD, "ratio {ratio}");
  assert!(cut_model_overlaps_copper(&swept, &copper, tol, PENETRATION_THRESHOLD).unwrap());
}
