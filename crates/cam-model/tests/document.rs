//! Behaviour tests for the document model and its schema contract (A3).

use cam_geom::{Contour, Point2};
use cam_model::*;
use cam_units::Length;

fn square(size: f64) -> Contour {
  Contour::from_points(
    &[
      Point2::new(0.0, 0.0),
      Point2::new(size, 0.0),
      Point2::new(size, size),
      Point2::new(0.0, size),
    ],
    true,
  )
  .expect("square")
}

fn document_with_square() -> Document {
  let mut document = Document::new(Job::default_sheet());
  document.layers.push(Layer {
    id: "L1".to_owned(),
    name: "Part".to_owned(),
    visible: true,
    entities: vec![Entity {
      id: "E1".to_owned(),
      kind: EntityKind::Contour {
        contour: square(100.0),
      },
    }],
  });
  document.toolpaths.push(Toolpath {
    id: "TP1".to_owned(),
    name: "Outside profile".to_owned(),
    entity_ids: vec!["E1".to_owned()],
    tool_number: 1,
    operation: Operation::Profile(ProfileParams::default()),
  });
  document
}

#[test]
fn a_new_document_carries_the_schema_tag_and_millimetre_job() {
  let document = Document::new(Job::default_sheet());
  assert_eq!(document.schema, DOCUMENT_SCHEMA);
  let json = serde_json::to_string(&document).expect("serialises");
  assert!(json.contains("\"schema\":\"c2d/document@1\""), "{json}");
  assert!(json.contains("\"units\":\"mm\""), "{json}");
  document.validate().expect("valid empty document");
}

#[test]
fn round_trip_is_lossless() {
  let document = document_with_square();
  document.validate().expect("valid document");
  let json = serde_json::to_string(&document).expect("serialises");
  let parsed: Document = serde_json::from_str(&json).expect("deserialises");
  assert_eq!(parsed, document);
  assert_eq!(parsed.job.width, Length::from_mm(1219.0));
  assert_eq!(parsed.toolpaths.len(), 1);
  let contour = parsed.layers[0].entities[0]
    .kind
    .as_contour()
    .expect("contour entity");
  assert!(contour.is_closed());
  assert_eq!(contour.signed_area(), 10_000.0);
}

#[test]
fn an_unknown_schema_is_rejected_naming_both_versions() {
  let mut document = document_with_square();
  document.schema = "c2d/document@99".to_owned();
  let error = document.validate().expect_err("must reject");
  let message = error.to_string();
  assert!(message.contains("c2d/document@99"), "{message}");
  assert!(message.contains(DOCUMENT_SCHEMA), "{message}");
  assert!(matches!(error, ModelError::UnsupportedSchema { .. }));
}

#[test]
fn duplicate_entity_ids_are_rejected() {
  let mut document = document_with_square();
  let clone = document.layers[0].entities[0].clone();
  document.layers[0].entities.push(clone);
  let error = document.validate().expect_err("must reject");
  assert!(
    matches!(error, ModelError::DuplicateId { kind: "entity", .. }),
    "{error:?}"
  );
}

#[test]
fn a_toolpath_citing_missing_geometry_is_rejected() {
  let mut document = document_with_square();
  document.toolpaths[0].entity_ids = vec!["E-missing".to_owned()];
  let error = document.validate().expect_err("must reject");
  assert!(
    matches!(error, ModelError::UnknownEntity { .. }),
    "{error:?}"
  );
  assert!(error.to_string().contains("E-missing"));
}

#[test]
fn a_profile_toolpath_on_open_geometry_is_rejected() {
  let mut document = document_with_square();
  document.layers[0].entities[0].kind = EntityKind::Contour {
    contour: Contour::from_points(&[Point2::new(0.0, 0.0), Point2::new(10.0, 0.0)], false)
      .expect("open polyline"),
  };
  let error = document.validate().expect_err("must reject");
  assert!(
    matches!(error, ModelError::OpenContourNotCuttable { .. }),
    "{error:?}"
  );
}

#[test]
fn a_zero_or_negative_job_dimension_is_rejected() {
  let mut document = document_with_square();
  document.job.thickness = Length::ZERO;
  let error = document.validate().expect_err("must reject");
  assert!(
    matches!(
      error,
      ModelError::NonPositiveJobDimension {
        field: "thickness",
        ..
      }
    ),
    "{error:?}"
  );
}

#[test]
fn deserialising_invalid_geometry_fails_instead_of_producing_it() {
  let json = r#"{
    "schema": "c2d/document@1",
    "job": {"width": 100.0, "height": 100.0, "thickness": 18.0,
            "units": "mm", "origin": "bottom_left", "safe_z": 5.0},
    "layers": [{"id": "L1", "name": "L", "visible": true, "entities": [
      {"id": "E1", "type": "contour", "contour": {"vertices": [{"point": {"x": 0.0, "y": 0.0}, "bulge": 0.0}], "closed": true}}
    ]}],
    "toolpaths": []
  }"#;
  let result: Result<Document, _> = serde_json::from_str(json);
  let error = result.expect_err("a one-vertex contour must not deserialise");
  assert!(
    error.to_string().contains("at least two vertices"),
    "{error}"
  );
}

#[test]
fn an_unknown_entity_type_is_rejected() {
  let json = r#"{
    "schema": "c2d/document@1",
    "job": {"width": 100.0, "height": 100.0, "thickness": 18.0,
            "units": "mm", "origin": "bottom_left", "safe_z": 5.0},
    "layers": [{"id": "L1", "name": "L", "visible": true,
      "entities": [{"id": "E1", "type": "bitmap"}]}],
    "toolpaths": []
  }"#;
  assert!(serde_json::from_str::<Document>(json).is_err());
}

fn document_with(operation: Operation) -> Document {
  let mut document = document_with_square();
  document.toolpaths[0].operation = operation;
  document
}

#[test]
fn a_pocket_round_trips_with_its_strategy_and_parameters() {
  let params = PocketParams {
    strategy: PocketStrategy::Raster,
    allowance: Length::from_mm(0.5),
    stepover: Length::from_mm(2.25),
    pass_depth: Length::from_mm(4.0),
    start_depth: Length::from_mm(1.0),
    cut_depth: Length::from_mm(9.5),
  };
  let document = document_with(Operation::Pocket(params.clone()));
  document
    .validate()
    .expect("a closed region can be pocketed");

  let json = serde_json::to_string(&document).expect("serialises");
  assert!(json.contains("\"kind\":\"pocket\""), "{json}");
  assert!(json.contains("\"strategy\":\"raster\""), "{json}");
  let parsed: Document = serde_json::from_str(&json).expect("deserialises");
  assert_eq!(parsed, document);
  assert_eq!(parsed.toolpaths[0].operation, Operation::Pocket(params));
}

#[test]
fn a_drilling_toolpath_round_trips_its_retract_mode_and_bank() {
  let params = DrillParams {
    peck_depth: Length::from_mm(3.0),
    retract: RetractMode::Amount {
      amount: Length::from_mm(1.5),
    },
    start_depth: Length::ZERO,
    cut_depth: Length::from_mm(18.0),
    bank: Some(DrillBank {
      count: 3,
      pitch: Length::from_mm(32.0),
    }),
  };
  let document = document_with(Operation::Drilling(params.clone()));
  document.validate().expect("a closed hole is drillable");

  let json = serde_json::to_string(&document).expect("serialises");
  assert!(json.contains("\"kind\":\"drilling\""), "{json}");
  assert!(json.contains("\"mode\":\"amount\""), "{json}");
  assert!(json.contains("\"pitch\":32.0"), "{json}");
  let parsed: Document = serde_json::from_str(&json).expect("deserialises");
  assert_eq!(parsed, document);
  assert_eq!(parsed.toolpaths[0].operation, Operation::Drilling(params));
}

#[test]
fn a_full_retract_drilling_toolpath_omits_the_amount_and_the_bank() {
  let document = document_with(Operation::Drilling(DrillParams::default()));
  let json = serde_json::to_string(&document).expect("serialises");
  assert!(json.contains("\"retract\":{\"mode\":\"full\"}"), "{json}");
  assert!(
    !json.contains("\"bank\""),
    "no empty bank is written: {json}"
  );
  let parsed: Document = serde_json::from_str(&json).expect("deserialises");
  assert_eq!(parsed, document);
}

#[test]
fn a_pocket_toolpath_on_open_geometry_is_rejected() {
  let mut document = document_with(Operation::Pocket(PocketParams::default()));
  document.layers[0].entities[0].kind = EntityKind::Contour {
    contour: Contour::from_points(&[Point2::new(0.0, 0.0), Point2::new(10.0, 0.0)], false)
      .expect("open polyline"),
  };
  let error = document.validate().expect_err("must reject");
  assert!(
    matches!(error, ModelError::OpenContourNotCuttable { .. }),
    "{error:?}"
  );
}

#[test]
fn a_drilling_toolpath_on_open_geometry_is_rejected() {
  let mut document = document_with(Operation::Drilling(DrillParams::default()));
  document.layers[0].entities[0].kind = EntityKind::Contour {
    contour: Contour::from_points(&[Point2::new(0.0, 0.0), Point2::new(10.0, 0.0)], false)
      .expect("open polyline"),
  };
  let error = document.validate().expect_err("must reject");
  assert!(
    matches!(error, ModelError::OpenContourNotCuttable { .. }),
    "{error:?}"
  );
}

#[test]
fn an_unknown_operation_kind_is_a_hard_error() {
  let json = r#"{
    "schema": "c2d/document@1",
    "job": {"width": 100.0, "height": 100.0, "thickness": 18.0,
            "units": "mm", "origin": "bottom_left", "safe_z": 5.0},
    "layers": [], "toolpaths": [
      {"id": "TP1", "name": "Later", "entity_ids": [], "tool_number": 1,
       "operation": {"kind": "laser_cut"}}]
  }"#;
  let error = serde_json::from_str::<Document>(json).expect_err("must not load");
  assert!(error.to_string().contains("laser_cut"), "{error}");
}
