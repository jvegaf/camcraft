//! Excellon round-trip: export drill features, parse with `cam-excellon`, and compare geometry
//! within tolerance (E5.5 acceptance).

use cam_export::{excellon, export_bytes, Format};
use cam_geom::Point2;
use cam_model::{Document, Entity, EntityKind, Job, Layer, LayerRole, Provenance, Side};
use cam_units::Length;

fn close(a: f64, b: f64, tol: f64) -> bool {
  (a - b).abs() <= tol
}

fn point_close(a: Point2, b: (f64, f64), tol: f64) -> bool {
  close(a.x, b.0, tol) && close(a.y, b.1, tol)
}

#[test]
fn holes_round_trip_through_excellon() {
  let bytes = excellon::export_features(
    &[
      (Point2::new(10.0, 20.0), 0.8),
      (Point2::new(30.0, 40.0), 1.2),
    ],
    &[],
  )
  .expect("export");
  let text = String::from_utf8(bytes).expect("utf8");
  let parsed = cam_excellon::parse(&text).expect("parse");
  assert_eq!(parsed.features.len(), 2);
  let mut holes: Vec<_> = parsed
    .features
    .iter()
    .filter_map(|f| match f {
      cam_excellon::Feature::Hole { tool, at } => Some((*tool, *at)),
      _ => None,
    })
    .collect();
  assert_eq!(holes.len(), 2);
  holes.sort_by(|a, b| a.1.x.partial_cmp(&b.1.x).unwrap());
  let t1 = holes[0].0;
  let t2 = holes[1].0;
  let d1 = parsed
    .tools
    .iter()
    .find(|t| t.number == t1)
    .unwrap()
    .diameter
    .mm();
  let d2 = parsed
    .tools
    .iter()
    .find(|t| t.number == t2)
    .unwrap()
    .diameter
    .mm();
  assert!(close(d1, 0.8, 1e-6));
  assert!(close(d2, 1.2, 1e-6));
  assert!(point_close(holes[0].1, (10.0, 20.0), 1e-6));
  assert!(point_close(holes[1].1, (30.0, 40.0), 1e-6));
}

#[test]
fn slots_round_trip_through_excellon() {
  let bytes = excellon::export_features(
    &[],
    &[(Point2::new(-17.272, 1.75), Point2::new(-17.272, -1.75), 1.0)],
  )
  .expect("export");
  let text = String::from_utf8(bytes).expect("utf8");
  let parsed = cam_excellon::parse(&text).expect("parse");
  assert_eq!(parsed.features.len(), 1);
  match parsed.features[0] {
    cam_excellon::Feature::Slot { tool, from, to } => {
      let d = parsed
        .tools
        .iter()
        .find(|t| t.number == tool)
        .unwrap()
        .diameter
        .mm();
      assert!(close(d, 1.0, 1e-6));
      assert!(point_close(from, (-17.272, 1.75), 1e-6));
      assert!(point_close(to, (-17.272, -1.75), 1e-6));
    }
    other => panic!("expected slot, got {other:?}"),
  }
}

#[test]
fn document_export_excellon_round_trips() {
  let mut document = Document::new(Job::default_sheet());
  document.layers.push(Layer {
    id: "drill".to_string(),
    role: LayerRole::Drill,
    side: Side::Both,
    color: "#000".to_string(),
    visible: true,
    source: Provenance::default(),
    entities: vec![
      Entity {
        id: "h1".to_string(),
        kind: EntityKind::Drill {
          center: Point2::new(5.0, 5.0),
          diameter: Length::from_mm(0.9),
        },
        source: Provenance::default(),
      },
      Entity {
        id: "s1".to_string(),
        kind: EntityKind::Slot {
          start: Point2::new(10.0, 0.0),
          end: Point2::new(10.0, 5.0),
          width: Length::from_mm(1.0),
        },
        source: Provenance::default(),
      },
    ],
  });
  let bytes = export_bytes(&document, Format::Excellon).expect("export");
  let text = String::from_utf8(bytes).expect("utf8");
  let parsed = cam_excellon::parse(&text).expect("parse");
  assert_eq!(parsed.features.len(), 2);
  let holes = parsed
    .features
    .iter()
    .filter(|f| matches!(f, cam_excellon::Feature::Hole { .. }))
    .count();
  let slots = parsed
    .features
    .iter()
    .filter(|f| matches!(f, cam_excellon::Feature::Slot { .. }))
    .count();
  assert_eq!(holes, 1);
  assert_eq!(slots, 1);
}
