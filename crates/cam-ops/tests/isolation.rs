//! Behaviour tests for copper isolation (E3.1, E3.2).

use cam_geom::region::Region;
use cam_geom::{Contour, Point2, TolerancePolicy};
use cam_model::MillingDirection;
use cam_ops::penetration::PENETRATION_THRESHOLD;
use cam_ops::{
  check_isolation_anti_gouge, isolation_passes, pass_distance, unite_copper,
  vbit_isolation_depth_mm,
};

const TOLERANCE: TolerancePolicy = TolerancePolicy::uniform(1.0e-9);

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Contour {
  Contour::from_points(
    &[
      Point2::new(x0, y0),
      Point2::new(x1, y0),
      Point2::new(x1, y1),
      Point2::new(x0, y1),
    ],
    true,
  )
  .expect("rectangle")
}

fn region_area(region: &Region) -> f64 {
  region
    .boundaries
    .iter()
    .map(|boundary| boundary.signed_area())
    .sum::<f64>()
    .abs()
}

fn close(actual: f64, expected: f64, scale: f64) -> bool {
  (actual - expected).abs() <= 1.0e-9 * scale.max(1.0)
}

// --- E3.1: copper union ---

#[test]
fn unite_copper_of_no_contours_is_empty() {
  let copper = unite_copper(&[], TOLERANCE).expect("empty copper");
  assert!(copper.boundaries.is_empty(), "no copper, no boundaries");
}

#[test]
fn unite_copper_of_one_contour_is_that_area() {
  let contour = rect(0.0, 0.0, 10.0, 5.0);
  let copper = unite_copper(&[contour], TOLERANCE).expect("one contour");
  assert!(
    close(region_area(&copper), 50.0, 50.0),
    "area {}",
    region_area(&copper)
  );
}

#[test]
fn unite_copper_merges_overlapping_contours() {
  let a = rect(0.0, 0.0, 10.0, 10.0);
  let b = rect(5.0, 0.0, 15.0, 10.0);
  let copper = unite_copper(&[a, b], TOLERANCE).expect("overlapping copper");
  // Union of 10x10 at 0 and 10x10 at 5 = 15x10 = 150
  assert!(
    close(region_area(&copper), 150.0, 150.0),
    "area {}",
    region_area(&copper)
  );
}

#[test]
fn unite_copper_keeps_disjoint_contours() {
  let a = rect(0.0, 0.0, 5.0, 5.0);
  let b = rect(20.0, 20.0, 25.0, 25.0);
  let copper = unite_copper(&[a, b], TOLERANCE).expect("disjoint copper");
  assert!(
    close(region_area(&copper), 50.0, 50.0),
    "area {}",
    region_area(&copper)
  );
  assert_eq!(copper.boundaries.len(), 2, "two separate copper areas");
}

// --- E3.1: corpus/FlatCAM criterion (S2, S7) ---

/// Sum of absolute boundary areas, the contour-area metric used by the parity harness and the
/// FlatCAM baseline (`fixtures/baseline/README.md`, `scripts/parity/measure/src/main.rs`).
fn contour_area(region: &Region) -> f64 {
  region
    .boundaries
    .iter()
    .map(|boundary| boundary.signed_area().abs())
    .sum()
}

fn close_relative(actual: f64, expected: f64, relative: f64) -> bool {
  let scale = expected.abs().max(actual.abs()).max(1.0e-12);
  (actual - expected).abs() <= relative * scale
}

#[test]
fn unite_copper_of_corpus_layer_matches_flatcam_area() {
  // S2/S7: la unión de una capa del corpus coincide en área con FlatCAM dentro de tolerancia.
  // Corpus layer: substitute-altium outline (fixtures/corpus/substitute-altium/outline.gbr).
  // FlatCAM reference: fixtures/baseline/substitute-altium.json artefacts.outline.contour_area_mm2.
  // This tree's parity golden (fixtures/corpus/substitute-altium/parity.json) agrees with FlatCAM
  // for this layer within tolerance, so the united contour area (sum of absolute boundary areas,
  // the metric shared by parity and baseline) is compared against FlatCAM's value.
  let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
  let gerber_path = root.join("fixtures/corpus/substitute-altium/outline.gbr");
  let text = std::fs::read_to_string(&gerber_path)
    .unwrap_or_else(|e| panic!("read {}: {e}", gerber_path.display()));
  let gerber = cam_gerber::parse(&text).expect("parse outline.gbr");
  let image = gerber
    .image(TolerancePolicy::uniform(1.0e-6))
    .expect("image outline.gbr");

  let copper = unite_copper(&image.boundaries, TOLERANCE).expect("unite corpus layer");
  let actual = contour_area(&copper);
  let flatcam = 28800.201818089998_f64; // fixtures/baseline/substitute-altium.json artefacts.outline

  assert!(
    close_relative(actual, flatcam, 1.0e-3),
    "united contour area {actual} mm² differs from FlatCAM {flatcam} mm²"
  );
}



// --- E3.4: anti-gouge guard ---

#[test]
fn injected_penetration_fails_isolation_with_actionable_diagnosis() {
  // S4: inyectar un caso de penetración hace fallar la operación con mensaje accionable.
  let protected = Region::new(vec![rect(0.0, 0.0, 10.0, 10.0)]).expect("protected copper");
  // Swept cut fully inside protected copper -> large penetration.
  let swept = Region::new(vec![rect(2.0, 2.0, 7.0, 7.0)]).expect("swept cut");

  let err = check_isolation_anti_gouge(&swept, &protected, TOLERANCE, PENETRATION_THRESHOLD)
    .expect_err("penetration must be refused");

  let message = err.to_string();
  assert!(
    message.contains("isolation anti-gouge"),
    "diagnosis names the guard: {message}"
  );
  assert!(
    message.contains("penetrates protected copper"),
    "diagnosis states the problem: {message}"
  );
  assert!(
    message.contains("ratio") && message.contains("threshold"),
    "diagnosis reports measured ratio and threshold: {message}"
  );
  assert!(
    message.contains("reduce tool diameter")
      || message.contains("increase isolation clearance")
      || message.contains("adjust isolation parameters"),
    "diagnosis is actionable: {message}"
  );
}

#[test]
fn non_penetrating_cut_passes_anti_gouge_guard() {
  let protected = Region::new(vec![rect(0.0, 0.0, 10.0, 10.0)]).expect("protected copper");
  // Swept cut entirely outside protected copper.
  let swept = Region::new(vec![rect(20.0, 20.0, 25.0, 25.0)]).expect("swept cut");
  check_isolation_anti_gouge(&swept, &protected, TOLERANCE, PENETRATION_THRESHOLD)
    .expect("no penetration");
}

// --- E3.2: pass distance formula ---

#[test]
fn pass_distances_match_formula_for_overlap_half() {
  let d = 1.0;
  let overlap = 0.5;
  assert!(close(pass_distance(d, 0, overlap), 0.5, 1.0));
  assert!(close(pass_distance(d, 1, overlap), 1.0, 1.0));
  assert!(close(pass_distance(d, 2, overlap), 1.5, 1.0));
}

#[test]
fn pass_distances_match_formula_for_no_overlap() {
  let d = 2.0;
  let overlap = 0.0;
  assert!(close(pass_distance(d, 0, overlap), 1.0, 2.0));
  assert!(close(pass_distance(d, 1, overlap), 3.0, 3.0));
  assert!(close(pass_distance(d, 2, overlap), 5.0, 5.0));
}

// --- E3.2: multipass isolation geometry ---

/// Minimum distance from any vertex of any contour in `pass_contours` to any segment of any
/// contour in `other_contours`.
fn min_distance_between_passes(pass_contours: &[Contour], other_contours: &[Contour]) -> f64 {
  let mut min_d = f64::INFINITY;
  for contour in pass_contours {
    for v in contour.vertices() {
      for other in other_contours {
        for seg in other.segments() {
          let d = seg.distance_to_point(v.point);
          if d < min_d {
            min_d = d;
          }
        }
      }
    }
  }
  min_d
}

#[test]
fn three_passes_overlap_half_produce_rings_without_gouge_or_collision() {
  // E3.2 acceptance: 3 pasadas con overlap 0.5 producen anillos sin colisión entre sí y sin gouge.
  let copper_contour = rect(20.0, 20.0, 40.0, 35.0);
  let copper = unite_copper(std::slice::from_ref(&copper_contour), TOLERANCE).expect("copper");
  let tool_diameter = 2.0;
  let overlap = 0.5;
  let result = isolation_passes(
    &copper,
    tool_diameter,
    3,
    overlap,
    MillingDirection::Climb,
    TOLERANCE,
  )
  .expect("3 isolation passes");

  assert_eq!(result.passes.len(), 3, "three passes produced");

  // Expected distances: 1.0, 2.0, 3.0 mm from copper
  for (i, pass) in result.passes.iter().enumerate() {
    assert_eq!(pass.index, i);
    let expected = pass_distance(tool_diameter, i, overlap);
    assert!(
      (pass.distance - expected).abs() < 1.0e-12,
      "pass {i} distance {} != {expected}",
      pass.distance
    );
    assert!(!pass.contours.is_empty(), "pass {i} has contours");
  }

  // No gouge: every pass contour vertex is at least ~expected distance from original copper.
  // Kernel allows small slack (chord tolerance related), so allow a small margin.
  for pass in &result.passes {
    let pass_region = Region::new(pass.contours.clone()).expect("pass region");
    for boundary in &pass_region.boundaries {
      for v in boundary.vertices() {
        let d = copper
          .distance_to_boundary(v.point, TOLERANCE)
          .expect("distance to copper");
        assert!(
          d.distance + 0.01 >= pass.distance,
          "pass {} vertex {:?} too close to copper: {} < {} (gouge risk)",
          pass.index,
          v.point,
          d.distance,
          pass.distance
        );
        assert!(!d.inside, "pass {} vertex inside copper", pass.index);
      }
    }
  }

  // No collision between rings: each pair of pass paths is separated by roughly the stepover.
  let stepover = tool_diameter * (1.0 - overlap);
  for i in 0..result.passes.len() {
    for j in (i + 1)..result.passes.len() {
      let d = min_distance_between_passes(&result.passes[i].contours, &result.passes[j].contours);
      let expected_sep = (result.passes[j].distance - result.passes[i].distance).abs();
      assert!(
        d >= expected_sep - 0.05,
        "passes {i} and {j} collide or too close: min distance {d}, expected ~{expected_sep}"
      );
      assert!(stepover > 0.0);
    }
  }
}

#[test]
fn isolation_passes_reject_invalid_parameters() {
  let copper = unite_copper(&[rect(0.0, 0.0, 10.0, 10.0)], TOLERANCE).unwrap();
  assert!(isolation_passes(&copper, 0.0, 1, 0.5, MillingDirection::Climb, TOLERANCE).is_err());
  assert!(isolation_passes(&copper, -1.0, 1, 0.5, MillingDirection::Climb, TOLERANCE).is_err());
  assert!(isolation_passes(&copper, 1.0, 0, 0.5, MillingDirection::Climb, TOLERANCE).is_err());
  assert!(isolation_passes(&copper, 1.0, 1, 1.0, MillingDirection::Climb, TOLERANCE).is_err());
  assert!(isolation_passes(&copper, 1.0, 1, -0.1, MillingDirection::Climb, TOLERANCE).is_err());
}

#[test]
fn milling_direction_changes_contour_orientation_not_geometry() {
  let copper = unite_copper(&[rect(20.0, 20.0, 40.0, 35.0)], TOLERANCE).unwrap();
  let climb = isolation_passes(&copper, 2.0, 2, 0.5, MillingDirection::Climb, TOLERANCE).unwrap();
  let conv = isolation_passes(
    &copper,
    2.0,
    2,
    0.5,
    MillingDirection::Conventional,
    TOLERANCE,
  )
  .unwrap();
  assert_eq!(climb.passes.len(), conv.passes.len());
  // same areas/distances, reversed contours
  for (a, b) in climb.passes.iter().zip(conv.passes.iter()) {
    assert!((a.distance - b.distance).abs() < 1.0e-12);
    assert_eq!(a.contours.len(), b.contours.len());
    for (ca, cb) in a.contours.iter().zip(b.contours.iter()) {
      assert!(
        (ca.signed_area().abs() - cb.signed_area().abs()).abs() < 1.0e-9,
        "same geometry area"
      );
      // reversed orientation => opposite signed area (for non-degenerate)
      if ca.signed_area().abs() > 1.0e-6 {
        assert!(
          ca.signed_area() * cb.signed_area() < 0.0,
          "opposite orientation"
        );
      }
    }
  }
}

#[test]
fn catalogue_vbit_produces_expected_isolation_depth() {
  // E1.8 / E3.3: una V-bit de 30° y 0.2 mm de punta da profundidad calculada.
  // For a catalogue V-bit (30° included angle, 0.2 mm tip/flat diameter), the isolation
  // depth to achieve an effective isolation width equal to tip + 2*tan(15°) mm is 1.0 mm.
  let included_angle_deg = 30.0_f64;
  let tip_diameter_mm = 0.2_f64;
  let target_isolation_diameter_mm = tip_diameter_mm + 2.0 * (15.0_f64.to_radians()).tan();

  let depth = vbit_isolation_depth_mm(
    target_isolation_diameter_mm,
    included_angle_deg,
    tip_diameter_mm,
  )
  .expect("catalogue V-bit depth");

  assert!(
    (depth - 1.0).abs() < 1.0e-12,
    "expected isolation depth 1.0 mm, got {depth} mm for target {target_isolation_diameter_mm} mm"
  );

  // At the tip width, expected depth is zero.
  let tip_depth =
    vbit_isolation_depth_mm(tip_diameter_mm, included_angle_deg, tip_diameter_mm).unwrap();
  assert!(tip_depth.abs() < 1.0e-12, "tip depth {tip_depth}");

  // Cannot cut narrower than the tip.
  assert!(vbit_isolation_depth_mm(0.1, included_angle_deg, tip_diameter_mm).is_err());
}
