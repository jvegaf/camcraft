//! Behaviour tests for the document model and its schema contract.

use cam_geom::{Contour, Point2};
use cam_model::*;
use cam_units::{Angle, Length};

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

fn rectangle(width: f64, height: f64, at: Point2) -> Contour {
  Contour::from_points(
    &[
      at,
      Point2::new(at.x + width, at.y),
      Point2::new(at.x + width, at.y + height),
      Point2::new(at.x, at.y + height),
    ],
    true,
  )
  .expect("rectangle")
}

fn isolation_params() -> IsolationParams {
  IsolationParams {
    passes: 1,
    overlap: 0.5,
    direction: MillingDirection::Climb,
    kind: IsolationKind::Full,
    cut_depth: Length::from_mm(0.1),
    keep_pours: false,
  }
}

fn document_with_operation(operation: Operation) -> Document {
  let mut document = Document::new(Job::default_sheet());
  document.layers.push(Layer {
    id: "L1".to_owned(),
    name: "Part".to_owned(),
    role: LayerRole::Geometry,
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
    name: "Op".to_owned(),
    entity_ids: vec!["E1".to_owned()],
    tool_number: 1,
    operation,
  });
  document
}

fn isolation_document(overlap: f64) -> Document {
  document_with_operation(Operation::Isolation(IsolationParams {
    overlap,
    ..isolation_params()
  }))
}

fn panelize_document(rows: u32, columns: u32) -> Document {
  document_with_operation(Operation::Panelize(PanelizeParams {
    rows,
    columns,
    spacing_x: Length::from_mm(2.0),
    spacing_y: Length::from_mm(2.0),
    constrain_x: None,
    constrain_y: None,
  }))
}

// --- restored core schema / validation coverage (c3-model S2, S3, S4; M1 L6) ---

#[test]
fn a_new_document_carries_the_schema_tag_and_millimetre_job() {
  let document = Document::new(Job::default_sheet());
  assert_eq!(document.schema, DOCUMENT_SCHEMA);
  let json = serde_json::to_string(&document).expect("serialises");
  assert!(
    json.contains("\"schema\":\"camcraft/document@1\""),
    "{json}"
  );
  assert!(json.contains("\"units\":\"mm\""), "{json}");
  document.validate().expect("valid empty document");
}

#[test]
fn round_trip_is_lossless() {
  let document = document_with_operation(Operation::Isolation(isolation_params()));
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
  let mut document = document_with_operation(Operation::Isolation(isolation_params()));
  document.schema = "c2d/document@99".to_owned();
  let error = document.validate().expect_err("must reject");
  let message = error.to_string();
  assert!(message.contains("c2d/document@99"), "{message}");
  assert!(message.contains(DOCUMENT_SCHEMA), "{message}");
  assert!(matches!(error, ModelError::UnsupportedSchema { .. }));
}

#[test]
fn a_legacy_schema_tag_is_refused_naming_the_schema() {
  // M1 introduced the refusal and forbids migration (S3, A2); M1 L6 fixed the to_bytes gate but the
  // document-level guarantee is pinned here.
  let mut document = document_with_operation(Operation::Isolation(isolation_params()));
  document.schema = "c2d/document@1".to_owned();
  let error = document
    .validate()
    .expect_err("must refuse a legacy schema");
  let message = error.to_string();
  assert!(message.contains("c2d/document@1"), "{message}");
  assert!(message.contains(DOCUMENT_SCHEMA), "{message}");
  assert!(
    matches!(
      error,
      ModelError::UnsupportedSchema { ref found, .. } if found == "c2d/document@1"
    ),
    "{error:?}"
  );

  let mut unknown = document_with_operation(Operation::Isolation(isolation_params()));
  unknown.schema = "c2d/document@3".to_owned();
  let error = unknown.validate().expect_err("must reject");
  assert!(
    matches!(
      error,
      ModelError::UnsupportedSchema { ref found, .. } if found == "c2d/document@3"
    ),
    "{error:?}"
  );
}

#[test]
fn duplicate_entity_ids_are_rejected() {
  let mut document = document_with_operation(Operation::Isolation(isolation_params()));
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
  let mut document = document_with_operation(Operation::Isolation(isolation_params()));
  document.toolpaths[0].entity_ids = vec!["E-missing".to_owned()];
  let error = document.validate().expect_err("must reject");
  assert!(
    matches!(error, ModelError::UnknownEntity { .. }),
    "{error:?}"
  );
  assert!(error.to_string().contains("E-missing"));
}

#[test]
fn a_closed_geometry_operation_on_open_geometry_is_rejected() {
  let mut document = document_with_operation(Operation::Isolation(isolation_params()));
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
  let mut document = document_with_operation(Operation::Isolation(isolation_params()));
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
    "schema": "camcraft/document@1",
    "job": {"width": 100.0, "height": 100.0, "thickness": 18.0,
            "units": "mm", "origin": "bottom_left", "safe_z": 5.0},
    "layers": [{"id": "L1", "name": "L", "role": "geometry", "visible": true, "entities": [
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
    "schema": "camcraft/document@1",
    "job": {"width": 100.0, "height": 100.0, "thickness": 18.0,
            "units": "mm", "origin": "bottom_left", "safe_z": 5.0},
    "layers": [{"id": "L1", "name": "L", "role": "geometry", "visible": true,
      "entities": [{"id": "E1", "type": "bitmap"}]}],
    "toolpaths": []
  }"#;
  assert!(serde_json::from_str::<Document>(json).is_err());
}

#[test]
fn an_unknown_operation_kind_is_a_hard_error() {
  let json = r#"{
    "schema": "camcraft/document@1",
    "job": {"width": 100.0, "height": 100.0, "thickness": 18.0,
            "units": "mm", "origin": "bottom_left", "safe_z": 5.0},
    "layers": [], "toolpaths": [
      {"id": "TP1", "name": "Later", "entity_ids": [], "tool_number": 1,
       "operation": {"kind": "laser_cut"}}]
  }"#;
  let error = serde_json::from_str::<Document>(json).expect_err("must not load");
  assert!(error.to_string().contains("laser_cut"), "{error}");
}

#[test]
fn the_job_outline_is_one_rectangle_in_document_coordinates() {
  let job = Job {
    width: Length::from_mm(200.0),
    height: Length::from_mm(100.0),
    ..Job::default_sheet()
  };
  let outline = job.outline();
  assert!(outline.is_closed());
  let bounds = outline.bounds();
  assert_eq!(
    (bounds.min.x, bounds.min.y, bounds.max.x, bounds.max.y),
    (0.0, 0.0, 200.0, 100.0)
  );
  let top_left = Job {
    origin: OriginCorner::TopLeft,
    ..job
  };
  assert_eq!(top_left.outline(), outline);
}

#[test]
fn a_document_written_before_nesting_loads_and_re_saves_byte_identically() {
  let before_m9 = r#"{"schema":"camcraft/document@1","job":{"width":1219.0,"height":2438.0,"thickness":18.0,"units":"mm","origin":"bottom_left","safe_z":5.0},"layers":[{"id":"L1","name":"Part","role":"geometry","visible":true,"entities":[{"id":"E1","type":"contour","contour":{"vertices":[{"point":{"x":0.0,"y":0.0},"bulge":0.0},{"point":{"x":100.0,"y":0.0},"bulge":0.0},{"point":{"x":100.0,"y":100.0},"bulge":0.0},{"point":{"x":0.0,"y":100.0},"bulge":0.0}],"closed":true}}]}],"toolpaths":[]}"#;
  let document: Document = serde_json::from_str(before_m9).expect("a document written before M9");
  assert!(document.nesting.is_none(), "no nesting section is present");
  document.validate().expect("and it still validates");
  assert_eq!(
    serde_json::to_string(&document).expect("serialises"),
    before_m9,
    "a document written before M9 is re-saved byte for byte"
  );
}

#[test]
fn a_job_without_a_material_serialises_without_the_key() {
  let document = Document::new(Job::default_sheet());
  assert!(
    document.job.material.is_none(),
    "the default sheet names none"
  );
  let json = serde_json::to_string(&document).expect("serialises");
  assert!(
    json.contains(
      r#""job":{"width":1219.0,"height":2438.0,"thickness":18.0,"units":"mm","origin":"bottom_left","safe_z":5.0}"#
    ),
    "a job with no material is the six keys and nothing else: {json}"
  );
  assert!(!json.contains("\"material\""), "the key is omitted: {json}");
}

#[test]
fn a_job_with_a_material_round_trips() {
  let mut document = document_with_operation(Operation::Isolation(isolation_params()));
  document.job.material = Some("MDF".to_owned());
  document.validate().expect("a valid document");

  let json = serde_json::to_string(&document).expect("serialises");
  assert!(json.contains(r#""material":"MDF""#), "{json}");

  let parsed: Document = serde_json::from_str(&json).expect("deserialises");
  assert_eq!(parsed.job.material.as_deref(), Some("MDF"));
  assert_eq!(parsed, document);
  assert_eq!(
    serde_json::to_string(&parsed).expect("re-serialises"),
    json,
    "a job that names a material re-saves byte for byte"
  );
  assert_eq!(
    DOCUMENT_SCHEMA, "camcraft/document@1",
    "schema is camcraft/document@1"
  );
}

// --- PCB operation validators ---

#[test]
fn isolation_rejects_invalid_overlap() {
  isolation_document(0.5).validate().expect("valid isolation");
  assert!(matches!(
    isolation_document(1.5).validate(),
    Err(ModelError::IsolationOverlapOutOfRange { .. })
  ));
  assert!(matches!(
    isolation_document(-0.1).validate(),
    Err(ModelError::IsolationOverlapOutOfRange { .. })
  ));
}

#[test]
fn isolation_rejects_zero_passes() {
  let mut document = isolation_document(0.5);
  if let Operation::Isolation(ref mut params) = document.toolpaths[0].operation {
    params.passes = 0;
  }
  assert!(matches!(
    document.validate(),
    Err(ModelError::IsolationPassesTooSmall { .. })
  ));
}

#[test]
fn isolation_rejects_non_positive_cut_depth() {
  let mut document = isolation_document(0.5);
  if let Operation::Isolation(ref mut params) = document.toolpaths[0].operation {
    params.cut_depth = Length::ZERO;
  }
  let error = document.validate().expect_err("must reject");
  assert!(
    matches!(error, ModelError::IsolationCutDepthNotPositive { .. }),
    "{error:?}"
  );
  assert!(error.to_string().contains("0"), "{error}");
}

#[test]
fn panelize_rejects_zero_rows_or_columns() {
  panelize_document(2, 3).validate().expect("valid panelize");
  assert!(matches!(
    panelize_document(0, 1).validate(),
    Err(ModelError::PanelizeRowsTooSmall { .. })
  ));
  assert!(matches!(
    panelize_document(1, 0).validate(),
    Err(ModelError::PanelizeColumnsTooSmall { .. })
  ));
}

#[test]
fn panelize_rejects_negative_spacing() {
  let mut document = panelize_document(2, 2);
  if let Operation::Panelize(ref mut params) = document.toolpaths[0].operation {
    params.spacing_x = Length::from_mm(-1.0);
  }
  let error = document.validate().expect_err("must reject");
  assert!(
    matches!(error, ModelError::PanelizeSpacingXNegative { .. }),
    "{error:?}"
  );
  assert!(error.to_string().contains("-1"), "{error}");

  let mut document = panelize_document(2, 2);
  if let Operation::Panelize(ref mut params) = document.toolpaths[0].operation {
    params.spacing_y = Length::from_mm(-0.5);
  }
  let error = document.validate().expect_err("must reject");
  assert!(
    matches!(error, ModelError::PanelizeSpacingYNegative { .. }),
    "{error:?}"
  );
  assert!(error.to_string().contains("-0.5"), "{error}");
}

#[test]
fn non_copper_clear_rejects_invalid_overlap_margin_cut_depth_and_copper_offset() {
  document_with_operation(Operation::NonCopperClear(NonCopperClearParams {
    overlap: 0.5,
    margin: Length::from_mm(1.0),
    method: FillMethod::Standard,
    connect: false,
    contour: false,
    copper_offset: Some(Length::from_mm(0.2)),
    direction: MillingDirection::Climb,
    cut_depth: Length::from_mm(0.1),
  }))
  .validate()
  .expect("valid non-copper clear");

  let mut document = document_with_operation(Operation::NonCopperClear(NonCopperClearParams {
    overlap: 1.5,
    margin: Length::from_mm(1.0),
    method: FillMethod::Standard,
    connect: false,
    contour: false,
    copper_offset: None,
    direction: MillingDirection::Climb,
    cut_depth: Length::from_mm(0.1),
  }));
  let error = document.validate().expect_err("must reject");
  assert!(
    matches!(error, ModelError::NonCopperClearOverlapOutOfRange { .. }),
    "{error:?}"
  );
  assert!(error.to_string().contains("1.5"), "{error}");
  if let Operation::NonCopperClear(ref mut params) = document.toolpaths[0].operation {
    params.overlap = -0.1;
  }
  let error = document
    .validate()
    .expect_err("must reject negative overlap");
  assert!(
    matches!(error, ModelError::NonCopperClearOverlapOutOfRange { .. }),
    "{error:?}"
  );
  assert!(error.to_string().contains("-0.1"), "{error}");

  let document = document_with_operation(Operation::NonCopperClear(NonCopperClearParams {
    overlap: 0.5,
    margin: Length::from_mm(-1.0),
    method: FillMethod::Standard,
    connect: false,
    contour: false,
    copper_offset: None,
    direction: MillingDirection::Climb,
    cut_depth: Length::from_mm(0.1),
  }));
  let error = document.validate().expect_err("must reject");
  assert!(
    matches!(error, ModelError::NonCopperClearMarginNegative { .. }),
    "{error:?}"
  );
  assert!(error.to_string().contains("-1"), "{error}");

  let document = document_with_operation(Operation::NonCopperClear(NonCopperClearParams {
    overlap: 0.5,
    margin: Length::from_mm(1.0),
    method: FillMethod::Standard,
    connect: false,
    contour: false,
    copper_offset: None,
    direction: MillingDirection::Climb,
    cut_depth: Length::ZERO,
  }));
  let error = document.validate().expect_err("must reject");
  assert!(
    matches!(error, ModelError::NonCopperClearCutDepthNotPositive { .. }),
    "{error:?}"
  );
  assert!(error.to_string().contains("0"), "{error}");

  let document = document_with_operation(Operation::NonCopperClear(NonCopperClearParams {
    overlap: 0.5,
    margin: Length::from_mm(1.0),
    method: FillMethod::Standard,
    connect: false,
    contour: false,
    copper_offset: Some(Length::from_mm(-0.2)),
    direction: MillingDirection::Climb,
    cut_depth: Length::from_mm(0.1),
  }));
  let error = document.validate().expect_err("must reject");
  assert!(
    matches!(error, ModelError::NonCopperClearCopperOffsetNegative { .. }),
    "{error:?}"
  );
  assert!(error.to_string().contains("-0.2"), "{error}");
}

#[test]
fn cutout_rejects_invalid_cut_depth_pass_depth_and_tabs() {
  document_with_operation(Operation::Cutout(CutoutParams {
    margin: Length::from_mm(0.0),
    tabs: vec![Tab {
      position: 0.25,
      width: Length::from_mm(2.0),
      height: Length::from_mm(0.5),
    }],
    tab_kind: TabKind::Straight,
    pass_depth: Length::from_mm(0.5),
    cut_depth: Length::from_mm(1.6),
    cut_interiors: false,
  }))
  .validate()
  .expect("valid cutout");

  let document = document_with_operation(Operation::Cutout(CutoutParams {
    margin: Length::from_mm(0.0),
    tabs: Vec::new(),
    tab_kind: TabKind::Straight,
    pass_depth: Length::from_mm(0.5),
    cut_depth: Length::ZERO,
    cut_interiors: false,
  }));
  let error = document.validate().expect_err("must reject");
  assert!(
    matches!(error, ModelError::CutoutCutDepthNotPositive { .. }),
    "{error:?}"
  );
  assert!(error.to_string().contains("0"), "{error}");

  let document = document_with_operation(Operation::Cutout(CutoutParams {
    margin: Length::from_mm(0.0),
    tabs: Vec::new(),
    tab_kind: TabKind::Straight,
    pass_depth: Length::from_mm(-0.5),
    cut_depth: Length::from_mm(1.6),
    cut_interiors: false,
  }));
  let error = document.validate().expect_err("must reject");
  assert!(
    matches!(error, ModelError::CutoutPassDepthNegative { .. }),
    "{error:?}"
  );
  assert!(error.to_string().contains("-0.5"), "{error}");

  let document = document_with_operation(Operation::Cutout(CutoutParams {
    margin: Length::from_mm(0.0),
    tabs: vec![Tab {
      position: 0.25,
      width: Length::ZERO,
      height: Length::from_mm(0.5),
    }],
    tab_kind: TabKind::Straight,
    pass_depth: Length::from_mm(0.5),
    cut_depth: Length::from_mm(1.6),
    cut_interiors: false,
  }));
  let error = document.validate().expect_err("must reject");
  assert!(
    matches!(error, ModelError::CutoutTabWidthNotPositive { .. }),
    "{error:?}"
  );
  assert!(error.to_string().contains("0"), "{error}");

  let document = document_with_operation(Operation::Cutout(CutoutParams {
    margin: Length::from_mm(0.0),
    tabs: vec![Tab {
      position: 1.5,
      width: Length::from_mm(2.0),
      height: Length::from_mm(0.5),
    }],
    tab_kind: TabKind::Straight,
    pass_depth: Length::from_mm(0.5),
    cut_depth: Length::from_mm(1.6),
    cut_interiors: false,
  }));
  let error = document.validate().expect_err("must reject");
  assert!(
    matches!(error, ModelError::CutoutTabOutsideContour { .. }),
    "{error:?}"
  );
  assert!(error.to_string().contains("1.5"), "{error}");
}

#[test]
fn paint_rejects_invalid_overlap_and_cut_depth() {
  document_with_operation(Operation::Paint(PaintParams {
    overlap: 0.5,
    offset: Length::from_mm(0.0),
    method: FillMethod::Standard,
    connect: false,
    contour: false,
    cut_depth: Length::from_mm(0.1),
  }))
  .validate()
  .expect("valid paint");

  let mut document = document_with_operation(Operation::Paint(PaintParams {
    overlap: 1.5,
    offset: Length::from_mm(0.0),
    method: FillMethod::Standard,
    connect: false,
    contour: false,
    cut_depth: Length::from_mm(0.1),
  }));
  let error = document.validate().expect_err("must reject");
  assert!(
    matches!(error, ModelError::PaintOverlapOutOfRange { .. }),
    "{error:?}"
  );
  assert!(error.to_string().contains("1.5"), "{error}");
  if let Operation::Paint(ref mut params) = document.toolpaths[0].operation {
    params.overlap = -0.1;
  }
  let error = document
    .validate()
    .expect_err("must reject negative overlap");
  assert!(
    matches!(error, ModelError::PaintOverlapOutOfRange { .. }),
    "{error:?}"
  );
  assert!(error.to_string().contains("-0.1"), "{error}");

  let document = document_with_operation(Operation::Paint(PaintParams {
    overlap: 0.5,
    offset: Length::from_mm(0.0),
    method: FillMethod::Standard,
    connect: false,
    contour: false,
    cut_depth: Length::ZERO,
  }));
  let error = document.validate().expect_err("must reject");
  assert!(
    matches!(error, ModelError::PaintCutDepthNotPositive { .. }),
    "{error:?}"
  );
  assert!(error.to_string().contains("0"), "{error}");
}

// --- nesting (restored from HEAD; nesting validators and types still exist) ---

fn nesting_document() -> Document {
  let mut document = Document::new(Job::default_sheet());
  document.layers.push(Layer {
    id: "L1".to_owned(),
    name: "Parts".to_owned(),
    role: LayerRole::Geometry,
    visible: true,
    entities: vec![
      Entity {
        id: "E1".to_owned(),
        kind: EntityKind::Contour {
          contour: square(100.0),
        },
      },
      Entity {
        id: "E2".to_owned(),
        kind: EntityKind::Contour {
          contour: rectangle(200.0, 50.0, Point2::new(300.0, 300.0)),
        },
      },
    ],
  });
  document.nesting = Some(Nesting {
    sheets: vec![NestingSheet {
      id: "S1".to_owned(),
      width: Length::from_mm(600.0),
      height: Length::from_mm(400.0),
    }],
    parts: vec![
      NestingPart {
        id: "P1".to_owned(),
        entity_ids: vec!["E1".to_owned()],
        instances: 2,
      },
      NestingPart {
        id: "P2".to_owned(),
        entity_ids: vec!["E2".to_owned()],
        instances: 1,
      },
    ],
    placements: vec![
      NestingPlacement {
        part_id: "P1".to_owned(),
        sheet_id: "S1".to_owned(),
        position: Point2::new(0.0, 0.0),
        rotation: Angle::ZERO,
      },
      NestingPlacement {
        part_id: "P1".to_owned(),
        sheet_id: "S1".to_owned(),
        position: Point2::new(200.0, 0.0),
        rotation: Angle::ZERO,
      },
      NestingPlacement {
        part_id: "P2".to_owned(),
        sheet_id: "S1".to_owned(),
        position: Point2::new(0.0, 150.0),
        rotation: Angle::from_degrees(90.0),
      },
    ],
    // Both fractions are what `Document::nesting_utilisation` derives: the placements cover 30 000
    // mm² of the one 600 x 400 = 240 000 mm² sheet they name, so utilisation is 0.125 and waste is
    // its complement, 0.875. A hand-invented pair — 0.25 and 0.75 sum to one and are still wrong about
    // the area — is exactly the record the cost rule refuses, so derive these; never pick them.
    cost: Some(NestingCost {
      sheets_used: 1,
      utilisation: 0.125,
      waste: 0.875,
    }),
  });
  document
}

/// Validate `document`, expecting a refusal that leaves the document exactly as it was.
fn refusal(document: &Document) -> ModelError {
  let stored = serde_json::to_string(document).expect("serialises");
  let error = document.validate().expect_err("must refuse");
  assert_eq!(
    serde_json::to_string(document).expect("serialises"),
    stored,
    "a refused document is unchanged"
  );
  error
}

#[test]
fn a_nesting_section_round_trips_losslessly() {
  let document = nesting_document();
  document.validate().expect("a valid nesting document");
  let json = serde_json::to_string(&document).expect("serialises");
  assert!(json.contains("\"nesting\":"), "{json}");
  let parsed: Document = serde_json::from_str(&json).expect("deserialises");
  assert_eq!(parsed, document);
  let nesting = parsed.nesting.as_ref().expect("the section survives");
  assert_eq!(nesting.sheets[0].width, Length::from_mm(600.0));
  assert_eq!(nesting.parts[1].instances, 1);
  assert_eq!(
    nesting.placements[2].rotation.degrees().round(),
    90.0,
    "the rotation the nester chose is stored as authored"
  );
  let cost = nesting.cost.as_ref().expect("the cost record is stored");
  assert_eq!(cost.sheets_used, 1);
  assert_eq!(cost.utilisation + cost.waste, 1.0);
}

#[test]
fn the_nesting_section_does_not_move_the_schema_tag() {
  assert_eq!(DOCUMENT_SCHEMA, "camcraft/document@1");
  let json = serde_json::to_string(&nesting_document()).expect("serialises");
  assert!(
    json.contains("\"schema\":\"camcraft/document@1\""),
    "{json}"
  );
}

#[test]
fn a_non_positive_sheet_dimension_is_refused_naming_the_value() {
  for value in [0.0, -5.0, f64::INFINITY] {
    let mut document = nesting_document();
    document.nesting.as_mut().expect("a nesting section").sheets[0].height = Length::from_mm(value);
    let error = refusal(&document);
    assert!(
      matches!(
        error,
        ModelError::NonPositiveSheetDimension {
          field: "height",
          ..
        }
      ),
      "{error:?}"
    );
    let message = error.to_string();
    assert!(message.contains("\"S1\""), "{message}");
    assert!(message.contains("height"), "{message}");
    assert!(message.contains(&value.to_string()), "{message}");
  }
}

#[test]
fn duplicate_nesting_sheet_ids_are_refused() {
  let mut document = nesting_document();
  let clone = document.nesting.as_ref().expect("a section").sheets[0].clone();
  document
    .nesting
    .as_mut()
    .expect("a section")
    .sheets
    .push(clone);
  let error = refusal(&document);
  assert!(
    matches!(
      error,
      ModelError::DuplicateId {
        kind: "nesting sheet",
        ..
      }
    ),
    "{error:?}"
  );
  assert!(error.to_string().contains("\"S1\""), "{error}");
}

#[test]
fn duplicate_nesting_part_ids_are_refused() {
  let mut document = nesting_document();
  let clone = document.nesting.as_ref().expect("a section").parts[1].clone();
  document
    .nesting
    .as_mut()
    .expect("a section")
    .parts
    .push(clone);
  let error = refusal(&document);
  assert!(
    matches!(
      error,
      ModelError::DuplicateId {
        kind: "nesting part",
        ..
      }
    ),
    "{error:?}"
  );
  assert!(error.to_string().contains("\"P2\""), "{error}");
}

#[test]
fn a_nesting_part_that_names_no_entity_is_refused() {
  let mut document = nesting_document();
  document.nesting.as_mut().expect("a section").parts[0].entity_ids = Vec::new();
  let error = refusal(&document);
  assert!(
    matches!(error, ModelError::NestingPartTakesNoEntities { .. }),
    "{error:?}"
  );
  assert!(error.to_string().contains("\"P1\""), "{error}");
}

#[test]
fn a_nesting_part_that_names_a_missing_entity_is_refused_naming_it() {
  let mut document = nesting_document();
  document.nesting.as_mut().expect("a section").parts[0].entity_ids = vec!["E-missing".to_owned()];
  let error = refusal(&document);
  assert!(
    matches!(error, ModelError::NestingPartUnknownEntity { .. }),
    "{error:?}"
  );
  let message = error.to_string();
  assert!(message.contains("\"P1\""), "{message}");
  assert!(message.contains("\"E-missing\""), "{message}");
}

#[test]
fn a_placement_that_names_a_missing_part_is_refused_naming_it() {
  let mut document = nesting_document();
  document.nesting.as_mut().expect("a section").placements[0].part_id = "P-missing".to_owned();
  let error = refusal(&document);
  assert!(
    matches!(error, ModelError::NestingPlacementUnknownPart { .. }),
    "{error:?}"
  );
  assert!(error.to_string().contains("\"P-missing\""), "{error}");
}

#[test]
fn a_placement_that_names_a_missing_sheet_is_refused_naming_it() {
  let mut document = nesting_document();
  document.nesting.as_mut().expect("a section").placements[1].sheet_id = "S-missing".to_owned();
  let error = refusal(&document);
  assert!(
    matches!(error, ModelError::NestingPlacementUnknownSheet { .. }),
    "{error:?}"
  );
  assert!(error.to_string().contains("\"S-missing\""), "{error}");
}

#[test]
fn a_negative_placement_is_refused_naming_the_value() {
  let mut document = nesting_document();
  document.nesting.as_mut().expect("a section").placements[1].position = Point2::new(-10.0, 0.0);
  let error = refusal(&document);
  assert!(
    matches!(error, ModelError::NestingPlacementPositionNotUsable { .. }),
    "{error:?}"
  );
  let message = error.to_string();
  assert!(message.contains("\"P1\""), "{message}");
  assert!(message.contains("-10"), "{message}");
}

#[test]
fn a_placement_that_leaves_its_sheet_is_refused_naming_the_footprint_and_the_sheet() {
  let mut document = nesting_document();
  // P1 is 100 mm square, so at x = 550 its footprint reaches 650 mm on a 600 mm sheet.
  document.nesting.as_mut().expect("a section").placements[1].position = Point2::new(550.0, 0.0);
  let error = refusal(&document);
  assert!(
    matches!(error, ModelError::NestingPlacementOutsideSheet { .. }),
    "{error:?}"
  );
  let message = error.to_string();
  assert!(message.contains("\"P1\""), "{message}");
  assert!(message.contains("\"S1\""), "{message}");
  assert!(message.contains("650"), "{message}");
  assert!(message.contains("600"), "{message}");
}

#[test]
fn the_pruned_overlap_check_names_the_same_pair_a_naive_scan_would() {
  // The overlap check buckets footprints by sheet, sweeps each bucket in x and prunes a pair by its
  // bounding box, so it no longer visits N(N-1)/2 pairs. The prune has to be *exactly* equivalent: the
  // same document refused, naming the same pair in document order. This is the differential, kept in
  // the suite rather than in a scratch copy, because a one-off run proves one shape and a seeded scan
  // proves the class. The reference can use strict inequalities and no second tolerance because a
  // shared edge is not an overlap by `footprints_overlap`'s own rule: a `Touching` pair is refused
  // only when the two bounds are equal, which is two copies stacked on one spot, so an edge that is
  // merely shared is accepted. Not because these coordinates are whole millimetres — that is a
  // property of the fixture, and a rule that rested on it would break on a sub-tolerance pair.
  let mut state: u64 = 0x5DEE_CE66;
  let mut next = move || {
    state = state
      .wrapping_mul(6_364_136_223_846_793_005)
      .wrapping_add(1_442_695_040_888_963_407);
    ((state >> 33) as f64) / ((u32::MAX >> 1) as f64)
  };
  for _ in 0..12 {
    let count = 2 + (next() * 8.0) as usize;
    let mut document = Document::new(Job::default_sheet());
    document.layers.push(Layer {
      id: "L1".to_owned(),
      name: "Parts".to_owned(),
      role: LayerRole::Geometry,
      visible: true,
      entities: Vec::new(),
    });
    let mut nesting = Nesting {
      sheets: vec![NestingSheet {
        id: "S1".to_owned(),
        width: Length::from_mm(600.0),
        height: Length::from_mm(400.0),
      }],
      parts: Vec::new(),
      placements: Vec::new(),
      cost: None,
    };
    // One part per placement, so an instance count can never be the reason a document is refused and
    // the overlap verdict is the only thing under comparison.
    let mut boxes: Vec<(f64, f64, f64, f64)> = Vec::new();
    for index in 0..count {
      let size = 20.0 + (next() * 60.0).floor();
      let part = format!("P{index}");
      let entity = format!("E{index}");
      document.layers[0].entities.push(Entity {
        id: entity.clone(),
        kind: EntityKind::Contour {
          contour: square(size),
        },
      });
      nesting.parts.push(NestingPart {
        id: part.clone(),
        entity_ids: vec![entity],
        instances: 1,
      });
      let x = (next() * 500.0).floor();
      let y = (next() * 300.0).floor();
      nesting.placements.push(NestingPlacement {
        part_id: part,
        sheet_id: "S1".to_owned(),
        position: Point2::new(x, y),
        rotation: Angle::ZERO,
      });
      boxes.push((x, y, x + size, y + size));
    }
    let mut expected: Option<(usize, usize)> = None;
    'scan: for first in 0..boxes.len() {
      for second in first + 1..boxes.len() {
        let (left, right) = (boxes[first], boxes[second]);
        if left.0 < right.2 && right.0 < left.2 && left.1 < right.3 && right.1 < left.3 {
          expected = Some((first, second));
          break 'scan;
        }
      }
    }
    document.nesting = Some(nesting);
    match expected {
      Some((first, second)) => {
        let error = document
          .validate()
          .expect_err("an overlapping pair is refused");
        assert_eq!(
          error,
          ModelError::NestingPlacementsOverlap {
            first: format!("P{first}"),
            second: format!("P{second}"),
            sheet: "S1".to_owned(),
          },
          "the pair is the one a naive scan names first, in document order"
        );
      }
      None => document
        .validate()
        .expect("a document whose placements are pairwise disjoint"),
    }
  }
}

#[test]
fn the_pruned_overlap_check_agrees_with_an_unpruned_scan_at_the_tolerance_scale() {
  // The seeded differential above draws whole millimetres, so the one tolerance the prune and the
  // predicate share is never exercised: a 1e-6 padding is invisible at that scale, which is why a prune
  // that dropped it would still pass it. This is the same differential with every position jittered by
  // fractions of the tolerance, and its reference is an **unpruned** quadratic scan asking the same
  // predicate. What it observes is therefore **keying**, not the padding: mutating the query to a single
  // cell fails this test and its whole-millimetre sibling, while removing the tolerance padding fails
  // neither — an offence requires the boxes to meet, and a meeting pair shares a cell even unpadded. The
  // padding is defensive and its guarantee is the argument in `check_nesting`'s comment, not this test
  // (`ca5c7eda/R2-001`, `R3-002`, and this review's `R3-001`, which caught the comment saying otherwise).
  let tolerance = cam_geom::DEFAULT_TOLERANCE;
  let jitters = [0.0, tolerance * 0.5, tolerance, tolerance * 2.0];
  let mut state: u64 = 0x00DD_BA11;
  let mut next = move || {
    state = state
      .wrapping_mul(6_364_136_223_846_793_005)
      .wrapping_add(1);
    ((state >> 33) as f64) / ((u32::MAX >> 1) as f64)
  };
  for _ in 0..24 {
    let count = 2 + (next() * 6.0) as usize;
    let mut document = Document::new(Job::default_sheet());
    document.layers.push(Layer {
      id: "L1".to_owned(),
      name: "Parts".to_owned(),
      role: LayerRole::Geometry,
      visible: true,
      entities: Vec::new(),
    });
    let mut nesting = Nesting {
      sheets: vec![NestingSheet {
        id: "S1".to_owned(),
        width: Length::from_mm(600.0),
        height: Length::from_mm(400.0),
      }],
      parts: Vec::new(),
      placements: Vec::new(),
      cost: None,
    };
    for index in 0..count {
      let size = 20.0 + (next() * 40.0).floor();
      let part = format!("P{index}");
      let entity = format!("E{index}");
      document.layers[0].entities.push(Entity {
        id: entity.clone(),
        kind: EntityKind::Contour {
          contour: square(size),
        },
      });
      nesting.parts.push(NestingPart {
        id: part.clone(),
        entity_ids: vec![entity],
        instances: 1,
      });
      // Whole millimetres plus a jitter at the tolerance's own scale, so a pair lands on either side of
      // it exactly as often as on it.
      let jitter = jitters[(next() * jitters.len() as f64) as usize % jitters.len()];
      let sign = if next() < 0.5 { -1.0 } else { 1.0 };
      let x = (next() * 500.0).floor() + sign * jitter;
      let y = (next() * 300.0).floor() + jitter;
      nesting.placements.push(NestingPlacement {
        part_id: part,
        sheet_id: "S1".to_owned(),
        position: Point2::new(x, y),
        rotation: Angle::ZERO,
      });
    }
    document.nesting = Some(nesting);

    // The reference: every pair, in document order, asking the same predicate the library asks, with no
    // prune and no cell between them.
    let mut expected: Option<(usize, usize)> = None;
    let placements = document
      .nesting
      .as_ref()
      .expect("a section")
      .placements
      .clone();
    let outlines: Vec<_> = placements
      .iter()
      .map(|placement| {
        document
          .placement_footprint(placement)
          .expect("a part with a square has a footprint")
      })
      .collect();
    'scan: for first in 0..outlines.len() {
      for second in first + 1..outlines.len() {
        let overlaps = match cam_geom::winding::relation(
          &outlines[first],
          &outlines[second],
          cam_geom::DEFAULT_TOLERANCE,
        ) {
          cam_geom::winding::Relation::Disjoint => false,
          cam_geom::winding::Relation::Touching => {
            outlines[first].bounds() == outlines[second].bounds()
          }
          _ => true,
        };
        if overlaps {
          expected = Some((first, second));
          break 'scan;
        }
      }
    }
    match expected {
      Some((first, second)) => {
        let error = document
          .validate()
          .expect_err("an unpruned scan found an overlap");
        assert_eq!(
          error,
          ModelError::NestingPlacementsOverlap {
            first: format!("P{first}"),
            second: format!("P{second}"),
            sheet: "S1".to_owned(),
          },
          "the prune must name the pair the unpruned scan names, at the tolerance's own scale"
        );
      }
      None => document
        .validate()
        .expect("the unpruned scan found no overlap, so the pruned one must not either"),
    }
  }
}

#[test]
fn an_overlap_of_twice_the_tolerance_is_refused_on_either_axis() {
  // The other side of the boundary the test above pins from below: two footprints that overlap by more
  // than the tolerance share material the predicate can see, so they are refused, and the acceptance
  // cases beside it are not the whole story (`ca5c7eda/R2-001`).
  let tolerance = cam_geom::DEFAULT_TOLERANCE;
  for axis in 0..2 {
    let mut document = nesting_document();
    let overlap = tolerance * 2.0;
    let position = if axis == 0 {
      Point2::new(100.0 - overlap, 0.0)
    } else {
      Point2::new(0.0, 100.0 - overlap)
    };
    document.nesting.as_mut().expect("a section").placements[1].position = position;
    let error = refusal(&document);
    assert!(
      matches!(error, ModelError::NestingPlacementsOverlap { .. }),
      "an overlap wider than the tolerance is material on axis {axis}: {error:?}"
    );
  }
}

#[test]
fn placements_on_one_sheet_that_share_x_but_not_y_are_accepted() {
  // The overlap check prunes a pair by bounding box before any winding relation is asked, and prunes
  // the whole rest of the bucket once a candidate starts past the box it is compared with. Both prunes
  // have to be sound: `footprints_overlap` answers `Disjoint` for a separated pair, and the only
  // relation that survives one, `Touching`, is reached exclusively through *equal* bounds. This pins
  // the y side of that: two 100 mm squares that share their whole x range and sit 50 mm apart in y are
  // not an overlap.
  let mut document = nesting_document();
  let nesting = document.nesting.as_mut().expect("a section");
  nesting.placements[1].position = Point2::new(0.0, 150.0);
  nesting.placements[2].position = Point2::new(400.0, 0.0);
  document
    .validate()
    .expect("a pair that shares x and not y is disjoint");
}

#[test]
fn a_placement_is_refused_when_its_part_has_no_positive_area_footprint() {
  // A closed two-vertex line is a legal `Contour` — two vertices, no consecutive pair coincident —
  // and its bounds are 100 x 0, so `placement_footprint` answers `None` for it. `check_nesting`
  // treated that `None` as "nothing to check", so a part with no area could be placed a kilometre
  // outside the sheet and `validate` answered `Ok`. A part with no footprint cannot be placed at
  // all, which is the refusal this pins.
  let mut document = nesting_document();
  document.layers[0].entities.push(Entity {
    id: "E-flat".to_owned(),
    kind: EntityKind::Contour {
      contour: Contour::from_points(&[Point2::new(0.0, 0.0), Point2::new(100.0, 0.0)], true)
        .expect("a closed two-vertex line is a legal contour"),
    },
  });
  let nesting = document.nesting.as_mut().expect("a section");
  nesting.parts[0].entity_ids = vec!["E-flat".to_owned()];
  nesting.placements[1].position = Point2::new(10_000.0, 10_000.0);
  // The cost record is set to what this build derives once P1's footprint is `None`: only P2's
  // 10 000 mm² is counted, so the utilisation is 10 000 / 240 000 and waste is its complement.
  // Without this, `check_nesting_cost` refuses the document and the missing footprint never gets to
  // be the defect under test — the refusal would be right for the wrong reason.
  nesting.cost = Some(NestingCost {
    sheets_used: 1,
    utilisation: 10_000.0 / 240_000.0,
    waste: 1.0 - 10_000.0 / 240_000.0,
  });
  let error = refusal(&document);
  assert!(
    matches!(error, ModelError::NestingPlacementWithoutFootprint { .. }),
    "{error:?}"
  );
  let message = error.to_string();
  assert!(message.contains("\"P1\""), "{message}");
}

#[test]
fn a_rotated_placement_is_measured_by_its_turned_footprint() {
  // P2 is 200 mm wide and 50 mm tall. Turned a quarter turn it is 50 mm wide, so it fits at x = 550
  // on a 600 mm sheet only because the rotation the nester chose is honoured.
  let mut document = nesting_document();
  document.nesting.as_mut().expect("a section").placements[2].position = Point2::new(550.0, 0.0);
  document
    .validate()
    .expect("a quarter turn swaps the footprint's width and height");
  document.nesting.as_mut().expect("a section").placements[2].rotation = Angle::ZERO;
  let error = refusal(&document);
  assert!(
    matches!(error, ModelError::NestingPlacementOutsideSheet { .. }),
    "{error:?}"
  );
  assert!(error.to_string().contains("750"), "{error}");
}

#[test]
fn two_placements_that_overlap_on_a_sheet_are_refused() {
  let mut document = nesting_document();
  // The same spot twice: the two footprints are identical, which is an overlap and not a touch.
  document.nesting.as_mut().expect("a section").placements[1].position = Point2::new(0.0, 0.0);
  let error = refusal(&document);
  assert!(
    matches!(error, ModelError::NestingPlacementsOverlap { .. }),
    "{error:?}"
  );
  let message = error.to_string();
  assert!(message.contains("\"P1\""), "{message}");
  assert!(message.contains("\"S1\""), "{message}");

  // A partial overlap: P2's turned footprint is 50 mm wide, so at x = 80 it bites into P1's 100 mm.
  let mut document = nesting_document();
  document.nesting.as_mut().expect("a section").placements[2].position = Point2::new(80.0, 80.0);
  let error = refusal(&document);
  assert!(
    matches!(error, ModelError::NestingPlacementsOverlap { .. }),
    "{error:?}"
  );
  let message = error.to_string();
  assert!(message.contains("\"P1\""), "{message}");
  assert!(message.contains("\"P2\""), "{message}");
}

#[test]
fn two_placements_that_share_an_edge_are_accepted() {
  // Parts nested edge to edge are the point of nesting, so touching is not overlapping.
  let mut document = nesting_document();
  document.nesting.as_mut().expect("a section").placements[1].position = Point2::new(100.0, 0.0);
  document
    .validate()
    .expect("two footprints that share an edge do not overlap");
}

#[test]
fn a_pair_separated_by_at_most_the_tolerance_is_accepted_on_either_axis() {
  // The prune's threshold and the relation's tolerance are one value, so a gap the relation still sees
  // is a gap the prune may not break on: inside the tolerance the relation answers `Touching`, which
  // is not an overlap unless the two bounds are equal. Both axes and both sides of the inclusive
  // boundary are pinned here, because a prune stricter than the relation would drop a pair the
  // relation would have judged (`94cfd234/R3-PRUNE-TOLERANCE`).
  let tolerance = cam_geom::DEFAULT_TOLERANCE;
  for gap in [tolerance * 0.5, tolerance] {
    for axis in 0..2 {
      let mut document = nesting_document();
      let position = if axis == 0 {
        Point2::new(100.0 + gap, 0.0)
      } else {
        Point2::new(0.0, 100.0 + gap)
      };
      document.nesting.as_mut().expect("a section").placements[1].position = position;
      // P2 is 200 x 50 and turned a quarter turn it stands 200 mm tall from x = 0, which would meet
      // the moved square in x. Out of the way, so the pair under test is the only pair.
      if axis == 1 {
        document.nesting.as_mut().expect("a section").placements[2].position =
          Point2::new(400.0, 0.0);
      }
      document
        .validate()
        .unwrap_or_else(|error| panic!("a gap of {gap:e} on axis {axis}: {error:?}"));
    }
  }
}

#[test]
fn a_pair_whose_boxes_meet_but_whose_material_only_touches_is_accepted() {
  // The prune compares bounding boxes; whether there is an offence is the winding relation's call.
  // This is the reachable shape where the two disagree: a 10 mm tab sitting in the notch of an
  // L-shaped part whose box swallows it whole. The boxes overlap by 10 x 10 mm, so no prune may drop
  // the pair, and the material shares only the notch's two edges, so the relation is `Touching` with
  // unequal bounds and the document loads (`94cfd234/R3-PRUNE-TOLERANCE`).
  let mut document = nesting_document();
  document.layers[0].entities = vec![
    Entity {
      id: "E-notch".to_owned(),
      kind: EntityKind::Contour {
        contour: Contour::from_points(
          &[
            Point2::new(10.0, 0.0),
            Point2::new(110.0, 0.0),
            Point2::new(110.0, 110.0),
            Point2::new(0.0, 110.0),
            Point2::new(0.0, 10.0),
            Point2::new(10.0, 10.0),
          ],
          true,
        )
        .expect("an L is a legal contour"),
      },
    },
    Entity {
      id: "E-tab".to_owned(),
      kind: EntityKind::Contour {
        contour: square(10.0),
      },
    },
  ];
  let nesting = document.nesting.as_mut().expect("a section");
  nesting.parts = vec![
    NestingPart {
      id: "P-notch".to_owned(),
      entity_ids: vec!["E-notch".to_owned()],
      instances: 1,
    },
    NestingPart {
      id: "P-tab".to_owned(),
      entity_ids: vec!["E-tab".to_owned()],
      instances: 1,
    },
  ];
  nesting.placements = vec![
    NestingPlacement {
      part_id: "P-notch".to_owned(),
      sheet_id: "S1".to_owned(),
      position: Point2::new(0.0, 0.0),
      rotation: Angle::ZERO,
    },
    NestingPlacement {
      part_id: "P-tab".to_owned(),
      sheet_id: "S1".to_owned(),
      position: Point2::new(0.0, 0.0),
      rotation: Angle::ZERO,
    },
  ];
  nesting.cost = None;
  document
    .validate()
    .expect("boxes that meet over material that only touches is not an overlap");
}

#[test]
fn placements_on_different_sheets_are_keyed_by_sheet_and_the_named_pair_is_the_earliest_in_the_document(
) {
  // Two things the single-sheet fixtures cannot assert (`94cfd234/R3-MULTISHEET-PRUNE`). A pair on
  // *different* sheets that coincides to the millimetre is not an overlap, which is the bucket key. And
  // when more than one sheet carries an offence the refusal names the earliest pair in document order,
  // not the earliest in the bucket the sweep happened to walk first — so the S2 pair below is the one
  // that must be named even though S1 sorts ahead of it.
  let mut coincident = nesting_document();
  {
    let nesting = coincident.nesting.as_mut().expect("a section");
    nesting.sheets.push(NestingSheet {
      id: "S2".to_owned(),
      width: Length::from_mm(600.0),
      height: Length::from_mm(400.0),
    });
    nesting.placements[1].sheet_id = "S2".to_owned();
    nesting.placements[1].position = Point2::new(0.0, 0.0);
    // The record measured one sheet; a second one with a placement on it is a different measurement, and
    // this test is about the overlap check rather than about the cost rule.
    nesting.cost = None;
  }
  coincident
    .validate()
    .expect("the same point on two sheets is not an overlap");

  let mut document = nesting_document();
  {
    let nesting = document.nesting.as_mut().expect("a section");
    nesting.sheets.push(NestingSheet {
      id: "S2".to_owned(),
      width: Length::from_mm(600.0),
      height: Length::from_mm(400.0),
    });
    nesting.parts[1].instances = 2;
    nesting.placements = vec![
      NestingPlacement {
        part_id: "P1".to_owned(),
        sheet_id: "S2".to_owned(),
        position: Point2::new(0.0, 0.0),
        rotation: Angle::ZERO,
      },
      NestingPlacement {
        part_id: "P1".to_owned(),
        sheet_id: "S2".to_owned(),
        position: Point2::new(80.0, 80.0),
        rotation: Angle::ZERO,
      },
      NestingPlacement {
        part_id: "P2".to_owned(),
        sheet_id: "S1".to_owned(),
        position: Point2::new(0.0, 0.0),
        rotation: Angle::ZERO,
      },
      NestingPlacement {
        part_id: "P2".to_owned(),
        sheet_id: "S1".to_owned(),
        position: Point2::new(150.0, 30.0),
        rotation: Angle::ZERO,
      },
    ];
    nesting.cost = None;
  }
  let error = refusal(&document);
  assert!(
    matches!(
      error,
      ModelError::NestingPlacementsOverlap {
        ref first,
        ref second,
        ref sheet,
      } if first == "P1" && second == "P1" && sheet == "S2"
    ),
    "the S2 pair is the earliest in the document, so S1's must not be named: {error:?}"
  );
}

#[test]
fn an_early_overlap_ends_the_check_instead_of_scanning_every_later_placement() {
  // The refusal names the earliest pair in document order, and the check now walks placements in that
  // same order, so the first `earlier` with any partner ends it. Before, the sweep visited pairs in x
  // order and had to accumulate a minimum, which meant a document with one overlap at the front paid
  // for every candidate pair behind it (`94cfd234/R1-001` and `R4-001`).
  //
  // This is a tripwire, not a benchmark: with these 5 000 stacked copies the accumulating scan asks for
  // 12 495 000 winding relations, and the ordered one asks for 4 999 — a factor of 2 500 that no
  // machine or load average moves. The ceiling is deliberately loose for that reason.
  const COPIES: usize = 5_000;
  let mut document = nesting_document();
  {
    let nesting = document.nesting.as_mut().expect("a section");
    nesting.parts[0].instances = COPIES;
    nesting.parts.truncate(1);
    nesting.placements = (0..COPIES)
      .map(|_| NestingPlacement {
        part_id: "P1".to_owned(),
        sheet_id: "S1".to_owned(),
        position: Point2::new(0.0, 0.0),
        rotation: Angle::ZERO,
      })
      .collect();
    nesting.cost = None;
  }
  let started = std::time::Instant::now();
  let error = document
    .validate()
    .expect_err("5 000 copies on one spot overlap, and the first pair is named");
  let elapsed = started.elapsed();
  assert!(
    matches!(
      error,
      ModelError::NestingPlacementsOverlap {
        ref first,
        ref second,
        ref sheet,
      } if first == "P1" && second == "P1" && sheet == "S1"
    ),
    "{error:?}"
  );
  assert!(
    elapsed < std::time::Duration::from_secs(5),
    "an early overlap must not wait for the whole document: {elapsed:?}"
  );
}

#[test]
fn a_wide_placement_does_not_drag_every_narrow_one_into_the_overlap_check() {
  // One part spans the sheet's whole x range in a y band of its own, and 20 000 small parts sit in a
  // second band, laid out in ascending x and authored in descending order. No pair is an offence — the
  // small parts are 10 mm wide and 11 mm apart, which is the spacing this fixture originally got wrong:
  // at 4.5 mm the copies overlapped each other, and `winding::relation` used to call that a touch
  // because their top and bottom edges are collinear (the same defect `odd/tasks/winding-collinear-
  // overlap.md` fixes), so the document validated for the wrong reason. An
  // index bounded by a running maximum of `max.x` collapses here — the wide box holds the maximum high
  // for every placement behind it — so each small part walks the whole prefix and asks for relations
  // that the guards then discard (review-ca5c7eda65951600's `R4-LOAD-REGRESSION`). This is a tripwire,
  // not a benchmark: the correct index does a few hundred thousand comparisons, the collapsed one 400
  // million.
  const NARROW: usize = 20_000;
  let mut document = nesting_document();
  document.layers[0].entities = vec![
    Entity {
      id: "E-wide".to_owned(),
      kind: EntityKind::Contour {
        contour: rectangle(100_000.0, 10.0, Point2::ORIGIN),
      },
    },
    Entity {
      id: "E-small".to_owned(),
      kind: EntityKind::Contour {
        contour: square(10.0),
      },
    },
  ];
  {
    let nesting = document.nesting.as_mut().expect("a section");
    nesting.sheets = vec![NestingSheet {
      id: "S1".to_owned(),
      width: Length::from_mm(300_000.0),
      height: Length::from_mm(100_000.0),
    }];
    nesting.parts = vec![
      NestingPart {
        id: "P-small".to_owned(),
        entity_ids: vec!["E-small".to_owned()],
        instances: NARROW,
      },
      NestingPart {
        id: "P-wide".to_owned(),
        entity_ids: vec!["E-wide".to_owned()],
        instances: 1,
      },
    ];
    let mut placements: Vec<NestingPlacement> = (0..NARROW)
      .rev()
      .map(|index| NestingPlacement {
        part_id: "P-small".to_owned(),
        sheet_id: "S1".to_owned(),
        position: Point2::new(index as f64 * 11.0, 50_000.0),
        rotation: Angle::ZERO,
      })
      .collect();
    placements.push(NestingPlacement {
      part_id: "P-wide".to_owned(),
      sheet_id: "S1".to_owned(),
      position: Point2::new(0.0, 0.0),
      rotation: Angle::ZERO,
    });
    nesting.placements = placements;
    nesting.cost = None;
  }
  let started = std::time::Instant::now();
  document
    .validate()
    .expect("one wide part and 20 000 disjoint small ones overlap nothing");
  let elapsed = started.elapsed();
  assert!(
    elapsed < std::time::Duration::from_secs(5),
    "a wide placement must not force a scan of the whole prefix: {elapsed:?}"
  );
}

#[test]
fn a_document_of_wide_strips_is_not_charged_the_cell_duplication() {
  // Every placement spans the sheet's whole width and its own 10 mm band in y, so no pair overlaps in
  // area — each pair of neighbours shares an edge with the material on opposite sides. The defect this
  // pins is in the index, not the classifier: a box wider than the cell span is stored in every cell it
  // covers, so each cell holds O(N) occupants and every placement iterates O(N·sqrt N) of them, even
  // though the stamp discards them one by one. That is O(N²·sqrt N) stamp checks where the dense case
  // is inherently Θ(N²) pairs (review-1567a12dd01c0090's `R3-001`). A box wider than the cell span now
  // lives in a short per-sheet wide list instead, which this document reduces to the optimal scan. This
  // is a tripwire, not a benchmark: the ceiling is loose because the difference is a factor of 2·sqrt N —
  // 16 000 000 stamp-loop steps with the wide list, 2 000 000 000 with every strip in every cell.
  const STRIPS: usize = 4_000;
  const BAND: f64 = 10.0;
  let mut document = nesting_document();
  document.layers[0].entities = vec![Entity {
    id: "E-strip".to_owned(),
    kind: EntityKind::Contour {
      contour: rectangle(100_000.0, BAND, Point2::ORIGIN),
    },
  }];
  {
    let nesting = document.nesting.as_mut().expect("a section");
    nesting.sheets = vec![NestingSheet {
      id: "S1".to_owned(),
      width: Length::from_mm(100_000.0),
      height: Length::from_mm(STRIPS as f64 * BAND + 100.0),
    }];
    nesting.parts = vec![NestingPart {
      id: "P-strip".to_owned(),
      entity_ids: vec!["E-strip".to_owned()],
      instances: STRIPS,
    }];
    nesting.placements = (0..STRIPS)
      .map(|index| NestingPlacement {
        part_id: "P-strip".to_owned(),
        sheet_id: "S1".to_owned(),
        position: Point2::new(0.0, index as f64 * BAND),
        rotation: Angle::ZERO,
      })
      .collect();
    nesting.cost = None;
  }
  let started = std::time::Instant::now();
  document
    .validate()
    .expect("full-width strips stacked in y overlap nothing");
  let elapsed = started.elapsed();
  assert!(
    elapsed < std::time::Duration::from_secs(5),
    "a wide placement must not be duplicated across the cells it spans: {elapsed:?}"
  );
}

#[test]
fn a_far_away_placement_on_one_sheet_does_not_collapse_another_sheets_cells() {
  // The cell width used to come from the global x extent over every sheet, so one placement far away in
  // x on one sheet stretched the other sheet's cells to millions of millimetres and dropped all 20 000
  // of its placements into a single cell — the scan the index exists to avoid, paid on a sheet that is
  // perfectly well spread out (review-1567a12dd01c0090's `R4-001`). The extent is per sheet now, so the
  // late sheet only sizes its own grid. This is a tripwire, not a benchmark: the collapsed scan is
  // quadratic in the placements of the crowded sheet, the per-sheet one is not.
  const NARROW: usize = 20_000;
  let mut document = nesting_document();
  document.layers[0].entities = vec![Entity {
    id: "E-small".to_owned(),
    kind: EntityKind::Contour {
      contour: square(10.0),
    },
  }];
  {
    let nesting = document.nesting.as_mut().expect("a section");
    nesting.sheets = vec![
      NestingSheet {
        id: "S1".to_owned(),
        width: Length::from_mm(300_000.0),
        height: Length::from_mm(10_000.0),
      },
      NestingSheet {
        id: "S-far".to_owned(),
        width: Length::from_mm(1_000_000_000.0),
        height: Length::from_mm(10_000.0),
      },
    ];
    nesting.parts = vec![NestingPart {
      id: "P-small".to_owned(),
      entity_ids: vec!["E-small".to_owned()],
      instances: NARROW + 1,
    }];
    let mut placements: Vec<NestingPlacement> = (0..NARROW)
      .map(|index| NestingPlacement {
        part_id: "P-small".to_owned(),
        sheet_id: "S1".to_owned(),
        position: Point2::new(index as f64 * 11.0, 500.0),
        rotation: Angle::ZERO,
      })
      .collect();
    placements.push(NestingPlacement {
      part_id: "P-small".to_owned(),
      sheet_id: "S-far".to_owned(),
      position: Point2::new(500_000_000.0, 500.0),
      rotation: Angle::ZERO,
    });
    nesting.placements = placements;
    nesting.cost = None;
  }
  let started = std::time::Instant::now();
  document
    .validate()
    .expect("the crowded sheet is spread out and the late sheet has one placement");
  let elapsed = started.elapsed();
  assert!(
    elapsed < std::time::Duration::from_secs(5),
    "a far-away placement must not collapse another sheet's cells: {elapsed:?}"
  );
}

#[test]
fn two_placements_that_overlap_with_collinear_edges_are_refused() {
  // The sixteenth advisory, at the level it hurt: P1's two 100 mm squares 50 mm apart in x share their
  // whole y extent, so their bottom and top edges coincide along a span while the interiors overlap by
  // 5 000 mm². `winding::relation` called that a touch — a coincident edge was enough to return
  // `Touching`, before any containment was asked — and this document loaded. The classifier now
  // answers `Crossing` for a same-side shared span with no containment, so the refusal is the one the
  // nesting rules always claimed (`odd/tasks/winding-collinear-overlap.md` S3).
  let mut document = nesting_document();
  document.nesting.as_mut().expect("a section").placements[1].position = Point2::new(50.0, 0.0);
  let error = refusal(&document);
  assert!(
    matches!(
      error,
      ModelError::NestingPlacementsOverlap {
        ref first,
        ref second,
        ref sheet,
      } if first == "P1" && second == "P1" && sheet == "S1"
    ),
    "{error:?}"
  );
}

#[test]
fn a_part_whose_instance_count_disagrees_with_its_placements_is_refused() {
  let mut document = nesting_document();
  document.nesting.as_mut().expect("a section").parts[0].instances = 3;
  let error = refusal(&document);
  assert!(
    matches!(error, ModelError::NestingInstanceCountMismatch { .. }),
    "{error:?}"
  );
  let message = error.to_string();
  assert!(message.contains("\"P1\""), "{message}");
  assert!(message.contains('3'), "{message}");
  assert!(message.contains('2'), "{message}");
}

// T1d: a document that authors sheets and parts and has never been nested. The user's workflow is
// "author the sheets and parts, then nest" (S1), so the state between those two steps is a request,
// not a corrupt document: the instance counts say how many copies the run must place, and there is
// nothing for them to disagree with until a placement exists.

/// A master document as a user authors it before the first nesting run: two sheets, two parts asking
/// for three and two instances, and **no placements at all**.
fn authored_nesting_document() -> Document {
  let mut document = nesting_document();
  let nesting = document.nesting.as_mut().expect("a section");
  nesting.sheets.push(NestingSheet {
    id: "S2".to_owned(),
    width: Length::from_mm(500.0),
    height: Length::from_mm(400.0),
  });
  nesting.parts[0].instances = 3;
  nesting.parts[1].instances = 2;
  nesting.placements.clear();
  // No cost record either: it is absent until a run writes it (D-M9-5).
  nesting.cost = None;
  document
}

/// The placements that honour the authored counts: three copies of `P1` and two of `P2`, all on `S1`.
fn fully_nested_placements() -> Vec<NestingPlacement> {
  [
    ("P1", 0.0, 0.0),
    ("P1", 100.0, 0.0),
    ("P1", 0.0, 100.0),
    ("P2", 0.0, 250.0),
    ("P2", 0.0, 320.0),
  ]
  .into_iter()
  .map(|(part_id, x, y)| NestingPlacement {
    part_id: part_id.to_owned(),
    sheet_id: "S1".to_owned(),
    position: Point2::new(x, y),
    rotation: Angle::ZERO,
  })
  .collect()
}

#[test]
fn an_authored_nesting_with_no_placements_validates_and_round_trips() {
  // The state the user authors (S1): sheets and parts, no nesting run yet. It has to load, validate
  // and save, because `nest` is the command that answers it and the first nesting has no other route.
  let document = authored_nesting_document();
  document
    .validate()
    .expect("a request that has not been nested yet is valid");
  assert_eq!(
    document.nesting_utilisation(),
    None,
    "nothing is placed, so nothing is used yet"
  );
  let json = serde_json::to_string(&document).expect("serialises");
  assert!(json.contains("\"nesting\":"), "{json}");
  assert!(!json.contains("\"placements\":"), "{json}");
  let reloaded: Document = serde_json::from_str(&json).expect("deserialises");
  assert_eq!(reloaded, document, "the authored request round-trips");
  assert_eq!(
    serde_json::to_string(&reloaded).expect("serialises"),
    json,
    "and it re-saves byte for byte"
  );
  reloaded.validate().expect("and it still validates");
  let nesting = reloaded.nesting.as_ref().expect("the section survives");
  assert_eq!(nesting.sheets.len(), 2);
  assert_eq!(nesting.parts[0].instances, 3);
  assert_eq!(nesting.parts[1].instances, 2);
  assert!(nesting.placements.is_empty(), "still not nested");
}

#[test]
fn a_partially_nested_document_is_still_refused() {
  // The relaxation stops exactly where the document holds one placement: from then on every count has
  // to agree with the placements naming it, because a half-nested document has no honest meaning. One
  // copy of a part that asks for three is stale, and a part no placement names is unplaced.
  let mut document = authored_nesting_document();
  document.nesting.as_mut().expect("a section").placements = vec![NestingPlacement {
    part_id: "P1".to_owned(),
    sheet_id: "S1".to_owned(),
    position: Point2::new(0.0, 0.0),
    rotation: Angle::ZERO,
  }];
  let error = refusal(&document);
  assert_eq!(
    error,
    ModelError::NestingInstanceCountMismatch {
      part: "P1".to_owned(),
      wanted: 3,
      placed: 1,
    },
    "{error:?}"
  );

  // Every other part is still measured: `P1` is fully placed, but `P2` asks for two and names none.
  let mut document = authored_nesting_document();
  document.nesting.as_mut().expect("a section").placements = fully_nested_placements()
    .into_iter()
    .filter(|placement| placement.part_id == "P1")
    .collect();
  let error = refusal(&document);
  assert_eq!(
    error,
    ModelError::NestingInstanceCountMismatch {
      part: "P2".to_owned(),
      wanted: 2,
      placed: 0,
    },
    "{error:?}"
  );
  let message = error.to_string();
  assert!(message.contains("\"P2\""), "{message}");
  assert!(message.contains('2'), "{message}");
  assert!(message.contains('0'), "{message}");
}

#[test]
fn a_fully_nested_document_still_validates_its_instance_counts() {
  // The other side of the narrowing: the rule was not removed, so a document whose placements honour
  // every count validates, and one copy too many is refused from the opposite direction.
  let mut document = authored_nesting_document();
  document.nesting.as_mut().expect("a section").placements = fully_nested_placements();
  document
    .validate()
    .expect("placements that honour every instance count");
  assert!(
    document.nesting_utilisation().is_some(),
    "something is used now"
  );

  let mut document = authored_nesting_document();
  let mut placements = fully_nested_placements();
  placements.push(NestingPlacement {
    part_id: "P2".to_owned(),
    sheet_id: "S1".to_owned(),
    position: Point2::new(200.0, 250.0),
    rotation: Angle::ZERO,
  });
  document.nesting.as_mut().expect("a section").placements = placements;
  let error = refusal(&document);
  assert_eq!(
    error,
    ModelError::NestingInstanceCountMismatch {
      part: "P2".to_owned(),
      wanted: 2,
      placed: 3,
    },
    "{error:?}"
  );
}

// T1b: the stored cost record's own refusals (S3). The user asked for utilisation and waste to be
// **both** the report and a stored field, so the field and the placements must not be able to
// disagree. Each rule refuses the record by name and never recomputes it: the numbers are the run's
// answer and validating does not fix them.

#[test]
fn a_cost_record_that_disagrees_with_its_placements_sheets_is_refused() {
  // The three placements all name sheet `S1`, so a record claiming two used sheets is stale. The
  // refusal states the recorded count and the count the placements imply (invariant 3).
  let mut document = nesting_document();
  document
    .nesting
    .as_mut()
    .expect("a section")
    .cost
    .as_mut()
    .expect("a cost record")
    .sheets_used = 2;
  let error = refusal(&document);
  assert!(
    matches!(
      error,
      ModelError::NestingCostSheetCountMismatch {
        recorded: 2,
        placed: 1
      }
    ),
    "{error:?}"
  );
  let message = error.to_string();
  assert!(message.contains('2'), "the recorded count: {message}");
  assert!(
    message.contains('1'),
    "the count the placements name: {message}"
  );
}

#[test]
fn a_cost_record_whose_fractions_are_not_fractions_is_refused_naming_field_and_value() {
  // Both fractions are fractions of the used sheets' area, so `NaN`, an infinity and anything outside
  // `0.0..=1.0` describe no area at all.
  for (field, value) in [
    ("utilisation", f64::NAN),
    ("waste", f64::INFINITY),
    ("utilisation", 1.5),
    ("waste", -0.1),
  ] {
    let mut document = nesting_document();
    let cost = document
      .nesting
      .as_mut()
      .expect("a section")
      .cost
      .as_mut()
      .expect("a cost record");
    match field {
      "utilisation" => cost.utilisation = value,
      _ => cost.waste = value,
    }
    let error = refusal(&document);
    assert!(
      matches!(
        error,
        ModelError::NestingCostFractionOutOfRange { field: named, .. } if named == field
      ),
      "{error:?}"
    );
    let message = error.to_string();
    assert!(message.contains(field), "the offending field: {message}");
    assert!(
      message.contains(&value.to_string()),
      "the offending value: {message}"
    );
  }
}

#[test]
fn a_cost_record_whose_waste_the_placements_do_not_imply_is_refused_naming_both_and_the_tolerance()
{
  // Utilisation and waste are fractions of the one used area (D-M9-5), so waste has to be the
  // complement of the utilisation the document derives, 0.125 — not merely a fraction that happens to
  // complete the record's own arithmetic. The refusal states the recorded waste, the waste the
  // document derives, and the tolerance they were measured against — `cam_geom::DEFAULT_TOLERANCE`,
  // the tolerance the rest of the crate already uses rather than a second epsilon.
  let mut document = nesting_document();
  document
    .nesting
    .as_mut()
    .expect("a section")
    .cost
    .as_mut()
    .expect("a cost record")
    .waste = 0.5;
  let error = refusal(&document);
  assert!(
    matches!(
      error,
      ModelError::NestingCostFractionMismatch {
        field: "waste",
        recorded,
        derived,
        ..
      } if recorded == 0.5 && derived == 0.875
    ),
    "{error:?}"
  );
  let message = error.to_string();
  assert!(message.contains("waste"), "the offending field: {message}");
  assert!(message.contains("0.5"), "the recorded waste: {message}");
  assert!(
    message.contains("0.875"),
    "the waste the document derives: {message}"
  );
  assert!(
    message.contains(&cam_geom::DEFAULT_TOLERANCE.to_string()),
    "the tolerance it was measured against: {message}"
  );
}

#[test]
fn a_cost_record_that_agrees_with_its_placements_loads() {
  // The accept side of every rule at once: one distinct sheet named, two fractions inside the unit
  // interval, and a sum of exactly one.
  let document = nesting_document();
  let cost = document
    .nesting
    .as_ref()
    .expect("a section")
    .cost
    .as_ref()
    .expect("a cost record");
  assert_eq!(
    cost.sheets_used, 1,
    "the placements name one distinct sheet"
  );
  assert_eq!(cost.utilisation + cost.waste, 1.0, "and they sum to one");
  document
    .validate()
    .expect("a cost record that agrees loads");
}

#[test]
fn a_nesting_section_without_a_cost_record_still_loads() {
  // `cost` is absent until a nesting run writes it (D-M9-5), so an authored document and one written
  // before the record existed are not asked for a record they do not have.
  let mut document = nesting_document();
  document.nesting.as_mut().expect("a section").cost = None;
  document.validate().expect("no cost record, no cost rules");
}

#[test]
fn a_cost_record_exactly_on_the_tolerance_still_loads() {
  // The comparison is inclusive — `|recorded - derived| <= tolerance` — so a record one tolerance away
  // from what the document derives is the last accepted one, and one more step is refused. It lives on
  // the five-part sheet for a reason: that derivation is 0.25 and 0.75, which are exact in binary,
  // while `0.125 + 1e-6` computes a distance of 1.000000000001e-06 and would be refused — a boundary
  // test on the 0.125 fixture would be testing the wrong side of the tolerance.
  let tolerance = cam_geom::DEFAULT_TOLERANCE;
  let mut document = five_parts_on_one_sheet();
  {
    let cost = document
      .nesting
      .as_mut()
      .expect("a section")
      .cost
      .insert(NestingCost {
        sheets_used: 1,
        utilisation: 0.25 + tolerance,
        waste: 1.0 - (0.25 + tolerance),
      });
    assert!(
      (cost.utilisation - 0.25).abs() <= tolerance,
      "the utilisation sits exactly on the tolerance: {}",
      cost.utilisation
    );
    assert!(
      (cost.waste - 0.75).abs() <= tolerance,
      "and so does the waste: {}",
      cost.waste
    );
  }
  document
    .validate()
    .expect("exactly on the tolerance is accepted, because the check is inclusive");

  document
    .nesting
    .as_mut()
    .expect("a section")
    .cost
    .as_mut()
    .expect("a cost record")
    .utilisation = 0.25 + 2.0 * tolerance;
  let error = refusal(&document);
  assert!(
    matches!(
      error,
      ModelError::NestingCostFractionMismatch {
        field: "utilisation",
        ..
      }
    ),
    "{error:?}"
  );
}

// T1c: one arithmetic for the fractions (S3). T1b proved that the stored record is consistent with
// itself; a record can pass that and still claim an area its placements do not cover. So
// `Document::nesting_utilisation` is now the only answer to "what is this nesting's utilisation", the
// stored record must equal what it says, and waste must be its complement.

/// A nesting whose arithmetic can be stated in exact numbers: five 100 x 100 parts on a 500 x 400
/// sheet — 50 000 mm² of material on 200 000 mm² of stock, so utilisation is exactly 0.25 — plus a
/// second sheet that is authored but named by no placement.
fn five_parts_on_one_sheet() -> Document {
  let mut document = Document::new(Job::default_sheet());
  document.layers.push(Layer {
    id: "L1".to_owned(),
    name: "Parts".to_owned(),
    role: LayerRole::Geometry,
    visible: true,
    entities: vec![Entity {
      id: "E1".to_owned(),
      kind: EntityKind::Contour {
        contour: square(100.0),
      },
    }],
  });
  document.nesting = Some(Nesting {
    sheets: vec![
      NestingSheet {
        id: "S1".to_owned(),
        width: Length::from_mm(500.0),
        height: Length::from_mm(400.0),
      },
      NestingSheet {
        id: "S2".to_owned(),
        width: Length::from_mm(1000.0),
        height: Length::from_mm(1000.0),
      },
    ],
    parts: vec![NestingPart {
      id: "P1".to_owned(),
      entity_ids: vec!["E1".to_owned()],
      instances: 5,
    }],
    placements: (0..5)
      .map(|index| NestingPlacement {
        part_id: "P1".to_owned(),
        sheet_id: "S1".to_owned(),
        position: Point2::new(f64::from(index) * 100.0, 0.0),
        rotation: Angle::ZERO,
      })
      .collect(),
    cost: None,
  });
  document
}

#[test]
fn the_utilisation_is_the_placed_area_over_the_used_sheet_area() {
  // 5 x 100 x 100 = 50 000 mm² of placed footprint on the one 500 x 400 = 200 000 mm² sheet the
  // placements name: utilisation is 50 000 / 200 000 = 0.25 and waste is its complement, 0.75.
  let mut document = five_parts_on_one_sheet();
  assert_eq!(document.nesting_utilisation(), Some(0.25));

  document.nesting.as_mut().expect("a section").cost = Some(NestingCost {
    sheets_used: 1,
    utilisation: 0.25,
    waste: 0.75,
  });
  document
    .validate()
    .expect("the record the document derives loads");

  // The second sheet is authored but carries no placement, so it is not used: counting it would make
  // the denominator 1 200 000 mm² and the utilisation 0.0416..., which no part of the run supports.
  let cost = document
    .nesting
    .as_mut()
    .expect("a section")
    .cost
    .as_mut()
    .expect("a cost record");
  cost.utilisation = 50_000.0 / 1_200_000.0;
  cost.waste = 1.0 - cost.utilisation;
  let error = refusal(&document);
  assert!(
    matches!(
      error,
      ModelError::NestingCostFractionMismatch {
        field: "utilisation",
        ..
      }
    ),
    "{error:?}"
  );
  assert!(
    error.to_string().contains("0.25"),
    "the derivation excludes the unused sheet: {error}"
  );
}

#[test]
fn a_cost_record_that_the_placements_imply_loads() {
  // 2 x 100 x 100 for `P1` plus 200 x 50 for `P2` is 30 000 mm² of footprint on the one
  // 600 x 400 = 240 000 mm² sheet the placements name, so utilisation is 0.125 and waste 0.875.
  let mut document = nesting_document();
  assert_eq!(document.nesting_utilisation(), Some(0.125));
  let cost = document
    .nesting
    .as_mut()
    .expect("a section")
    .cost
    .as_mut()
    .expect("a cost record");
  cost.utilisation = 0.125;
  cost.waste = 0.875;
  document
    .validate()
    .expect("the record the document derives loads");
}

#[test]
fn a_cost_record_whose_utilisation_the_placements_do_not_imply_is_refused() {
  // The gap this rule exists for: 0.25 and 0.75 sum to one, so a rule that only checked the record
  // against itself called it consistent, but the placements only cover 0.125 of the sheet. The fixture
  // now carries the derived pair, so the invented one is written here — which is the whole point: a
  // record nobody derived is refused naming the recorded fraction, the fraction the document derives
  // and the tolerance they were measured against (invariant 3).
  let mut document = nesting_document();
  {
    let cost = document
      .nesting
      .as_mut()
      .expect("a section")
      .cost
      .as_mut()
      .expect("a cost record");
    cost.utilisation = 0.25;
    cost.waste = 0.75;
    assert_eq!(
      cost.utilisation + cost.waste,
      1.0,
      "the record is consistent with itself and still wrong about the area"
    );
  }

  let error = refusal(&document);
  assert!(
    matches!(
      error,
      ModelError::NestingCostFractionMismatch {
        field: "utilisation",
        recorded,
        derived,
        ..
      } if recorded == 0.25 && derived == 0.125
    ),
    "{error:?}"
  );
  let message = error.to_string();
  assert!(message.contains("utilisation"), "{message}");
  assert!(message.contains("0.25"), "the recorded fraction: {message}");
  assert!(message.contains("0.125"), "the derived fraction: {message}");
  assert!(
    message.contains(&cam_geom::DEFAULT_TOLERANCE.to_string()),
    "the tolerance it was measured against: {message}"
  );
}

#[test]
fn a_cost_record_whose_waste_is_not_the_complement_is_refused() {
  // Waste is not merely small or merely positive: with the derived utilisation of 0.125 recorded, the
  // waste has to be the complement, 0.875. A record that says 0.5 is refused naming what it recorded
  // and what the document derives.
  let mut document = nesting_document();
  let cost = document
    .nesting
    .as_mut()
    .expect("a section")
    .cost
    .as_mut()
    .expect("a cost record");
  cost.utilisation = 0.125;
  cost.waste = 0.5;

  let error = refusal(&document);
  assert!(
    matches!(
      error,
      ModelError::NestingCostFractionMismatch {
        field: "waste",
        recorded,
        derived,
        ..
      } if recorded == 0.5 && derived == 0.875
    ),
    "{error:?}"
  );
  let message = error.to_string();
  assert!(message.contains("waste"), "{message}");
  assert!(message.contains("0.5"), "the recorded fraction: {message}");
  assert!(message.contains("0.875"), "the derived fraction: {message}");
  assert!(
    message.contains(&cam_geom::DEFAULT_TOLERANCE.to_string()),
    "the tolerance it was measured against: {message}"
  );
}

#[test]
fn a_cost_record_exactly_on_the_tolerance_against_the_derivation_still_loads() {
  // The comparison is inclusive — `|recorded - derived| <= tolerance` — so the last accepted record is
  // one tolerance away from what the document derives and one more step is refused. The derivation is
  // the 0.25 of the five-part sheet, whose fractions are exact in binary, so this measures the check
  // and not the rounding of the fixture.
  let tolerance = cam_geom::DEFAULT_TOLERANCE;
  let mut document = five_parts_on_one_sheet();
  let cost = document
    .nesting
    .as_mut()
    .expect("a section")
    .cost
    .insert(NestingCost {
      sheets_used: 1,
      utilisation: 0.25 + tolerance,
      waste: 1.0 - (0.25 + tolerance),
    });
  assert!(
    (cost.utilisation - 0.25).abs() <= tolerance,
    "the record sits on the tolerance, not past it: {}",
    cost.utilisation
  );
  document
    .validate()
    .expect("exactly on the tolerance is accepted, because the check is inclusive");

  let cost = document
    .nesting
    .as_mut()
    .expect("a section")
    .cost
    .as_mut()
    .expect("a cost record");
  cost.utilisation = 0.25 + 2.0 * tolerance;
  cost.waste = 1.0 - cost.utilisation;
  let error = refusal(&document);
  assert!(
    matches!(
      error,
      ModelError::NestingCostFractionMismatch {
        field: "utilisation",
        ..
      }
    ),
    "{error:?}"
  );
}

#[test]
fn a_nesting_that_cannot_say_what_it_used_loads_only_a_record_that_used_nothing() {
  // Two ways a document cannot answer: no sheets at all, and sheets with nothing placed on them. The
  // derivation returns `None` rather than inventing a fraction, so the record is measured against the
  // only two fractions a document that placed nothing can have used: none of the sheet, and all of it
  // wasted. A record claiming anything else has no placement to support it, so it is refused by name
  // instead of loading as a measurement nobody made (`39a6d85c/R4-003`, the owner's decision).
  let mut without_sheets = five_parts_on_one_sheet();
  {
    let nesting = without_sheets.nesting.as_mut().expect("a section");
    nesting.sheets.clear();
    nesting.placements.clear();
    nesting.parts[0].instances = 0;
    nesting.cost = Some(NestingCost {
      sheets_used: 0,
      utilisation: 0.0,
      waste: 1.0,
    });
  }
  assert_eq!(
    without_sheets.nesting_utilisation(),
    None,
    "no sheets, so there is nothing to divide by"
  );
  without_sheets
    .validate()
    .expect("a document that used nothing may say so");

  let mut nothing_placed = five_parts_on_one_sheet();
  {
    let nesting = nothing_placed.nesting.as_mut().expect("a section");
    nesting.placements.clear();
    nesting.parts[0].instances = 0;
    nesting.cost = Some(NestingCost {
      sheets_used: 0,
      utilisation: 0.0,
      waste: 1.0,
    });
  }
  assert_eq!(
    nothing_placed.nesting_utilisation(),
    None,
    "authored sheets with no placement on them say nothing about utilisation"
  );
  nothing_placed
    .validate()
    .expect("a document that used nothing may say so");

  let mut claiming = five_parts_on_one_sheet();
  {
    let nesting = claiming.nesting.as_mut().expect("a section");
    nesting.placements.clear();
    nesting.parts[0].instances = 0;
    nesting.cost = Some(NestingCost {
      sheets_used: 0,
      utilisation: 0.4,
      waste: 0.6,
    });
  }
  let error = claiming
    .validate()
    .expect_err("a record no placement supports is refused, not loaded");
  assert!(
    matches!(
      error,
      ModelError::NestingCostFractionMismatch {
        field: "utilisation",
        recorded,
        derived,
        ..
      } if recorded == 0.4 && derived == 0.0
    ),
    "{error:?}"
  );
}
