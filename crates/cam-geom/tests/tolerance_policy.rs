//! E0.3 (W3): tolerance policy is explicit and operations use it.

use cam_geom::primitive;
use cam_geom::{
  Arc, Bounds, Contour, Point2, Segment, TolerancePolicy, Vertex, DEFAULT_CHORD, DEFAULT_EPSILON,
};

#[test]
fn policy_defaults_match_system_design() {
  let p = TolerancePolicy::standard();
  assert_eq!(p.chord, DEFAULT_CHORD);
  assert_eq!(p.epsilon, DEFAULT_EPSILON);
  assert_eq!(p, TolerancePolicy::default());
}

#[test]
fn proximity_uses_policy_epsilon() {
  let a = Point2::new(0.0, 0.0);
  let b = Point2::new(0.5e-6, 0.0);
  assert!(
    a.is_near(b, TolerancePolicy::uniform(1.0e-6)),
    "points closer than epsilon are near"
  );
  assert!(
    !a.is_near(b, TolerancePolicy::uniform(1.0e-7)),
    "points farther than epsilon are not near"
  );
}

#[test]
fn tessellation_uses_policy_chord() {
  let arc = Arc::new(Point2::ORIGIN, 10.0, 0.0, std::f64::consts::PI).expect("arc");
  let coarse = arc.step_count(TolerancePolicy::standard().with_chord(1.0));
  let fine = arc.step_count(TolerancePolicy::standard().with_chord(0.01));
  assert!(
    fine > coarse,
    "finer chord tolerance must produce more steps: coarse={coarse} fine={fine}"
  );
  let pts_coarse = arc.tessellate(TolerancePolicy::standard().with_chord(1.0));
  let pts_fine = arc.tessellate(TolerancePolicy::standard().with_chord(0.01));
  assert!(pts_fine.len() > pts_coarse.len());
}

#[test]
fn bounds_containment_uses_policy_epsilon() {
  let bounds = Bounds::from_points(&[Point2::new(0.0, 0.0), Point2::new(10.0, 10.0)]);
  let outside = Point2::new(10.0 + 5.0e-7, 5.0);
  assert!(bounds.contains(outside, TolerancePolicy::uniform(1.0e-6)));
  assert!(!bounds.contains(outside, TolerancePolicy::uniform(1.0e-8)));
}

#[test]
fn contour_tessellation_respects_policy() {
  let contour = primitive::circle(Point2::ORIGIN, 10.0).expect("circle");
  let coarse = contour.tessellate(TolerancePolicy::standard().with_chord(1.0));
  let fine = contour.tessellate(TolerancePolicy::standard().with_chord(0.01));
  assert!(
    fine.len() > coarse.len(),
    "contour tessellation must use policy chord: coarse={} fine={}",
    coarse.len(),
    fine.len()
  );
}

#[test]
fn public_operations_accept_explicit_policy() {
  let tol = TolerancePolicy::standard();
  let a = Point2::ORIGIN;
  let b = Point2::new(1.0, 0.0);
  let _ = a.is_near(b, tol);
  let bounds = Bounds::from_points(&[a, b]);
  let _ = bounds.contains(a, tol);
  let arc = Arc::new(Point2::ORIGIN, 5.0, 0.0, std::f64::consts::FRAC_PI_2).expect("arc");
  let _ = arc.step_count(tol);
  let _ = arc.tessellate(tol);
  let seg = Segment::Line(cam_geom::LineSeg::new(a, b));
  let mut out = Vec::new();
  seg.tessellate_into(tol, &mut out);
  let contour = Contour::new(
    vec![
      Vertex::new(Point2::new(5.0, 0.0), 1.0),
      Vertex::new(Point2::new(-5.0, 0.0), 1.0),
    ],
    true,
  )
  .expect("contour");
  let _ = contour.tessellate(tol);
  let _ = cam_geom::chain(&contour.segments(), tol);
}

#[test]
fn no_public_geom_operation_creates_its_own_tolerance_literal() {
  let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
  let excluded = ["tolerance.rs"]; // defaults are defined here by design
  let suspicious = [
    "tolerance: 0.001",
    "tolerance: 1.0e-6",
    "tolerance: 1e-6",
    "chord_tolerance: 0.001",
    "OFFSET_CHORD_TOLERANCE",
    "const DEFAULT_TOLERANCE: f64 = 1.0e-6",
  ];
  let mut violations = Vec::new();
  for entry in std::fs::read_dir(&src).expect("read cam-geom/src") {
    let entry = entry.expect("entry");
    let path = entry.path();
    if path.extension().and_then(|e| e.to_str()) != Some("rs") {
      continue;
    }
    let name = path.file_name().unwrap().to_string_lossy().to_string();
    if excluded.iter().any(|e| *e == name) {
      continue;
    }
    let text = std::fs::read_to_string(&path).expect("read source");
    for needle in suspicious {
      if text.contains(needle) {
        violations.push(format!("{name}: contains `{needle}`"));
      }
    }
  }
  assert!(
    violations.is_empty(),
    "public geom operations must not create their own tolerance literals:\n{}",
    violations.join("\n")
  );
}
