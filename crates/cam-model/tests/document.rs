//! Behaviour tests for the document model and its schema contract (A3).

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
    island_ids: Vec::new(),
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
  assert!(json.contains("\"schema\":\"c2d/document@2\""), "{json}");
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

// --- P1: islands and the c2d/document@2 migration (A9, A10) ---

fn add_circle(document: &mut Document, id: &str, centre: Point2, radius: f64) {
  document.layers[0].entities.push(Entity {
    id: id.to_owned(),
    kind: EntityKind::Contour {
      contour: cam_geom::primitive::circle(centre, radius).expect("circle"),
    },
  });
}

fn pocket_document_with_island() -> Document {
  let mut document = document_with(Operation::Pocket(PocketParams::default()));
  add_circle(&mut document, "E2", Point2::new(50.0, 50.0), 10.0);
  document.toolpaths[0].island_ids = vec!["E2".to_owned()];
  document
}

#[test]
fn an_island_round_trips_and_is_omitted_when_there_is_none() {
  let document = pocket_document_with_island();
  document.validate().expect("an island inside the pocket");
  let json = serde_json::to_string(&document).expect("serialises");
  assert!(json.contains("\"island_ids\":[\"E2\"]"), "{json}");
  let parsed: Document = serde_json::from_str(&json).expect("deserialises");
  assert_eq!(parsed, document);

  let without = document_with(Operation::Pocket(PocketParams::default()));
  let plain = serde_json::to_string(&without).expect("serialises");
  assert!(
    !plain.contains("island_ids"),
    "a document without islands is written as before: {plain}"
  );
}

#[test]
fn an_island_outside_its_pocket_is_rejected() {
  let mut document = pocket_document_with_island();
  document.toolpaths[0].island_ids = vec!["E2".to_owned()];
  // Move the island outside the 100 mm square the pocket is cut from.
  add_circle(&mut document, "E3", Point2::new(400.0, 400.0), 10.0);
  document.toolpaths[0].island_ids = vec!["E3".to_owned()];
  let error = document.validate().expect_err("must reject");
  assert!(
    matches!(error, ModelError::IslandOutsideBoundary { ref island, .. } if island == "E3"),
    "{error:?}"
  );
}

#[test]
fn an_island_in_the_notch_of_a_c_shaped_pocket_is_rejected() {
  // The island's bounds are inside the pocket's bounds, and the material is not: containment is the
  // winding question, not a bounds question.
  let mut document = document_with(Operation::Pocket(PocketParams::default()));
  document.layers[0].entities[0].kind = EntityKind::Contour {
    contour: Contour::from_points(
      &[
        Point2::new(0.0, 0.0),
        Point2::new(100.0, 0.0),
        Point2::new(100.0, 30.0),
        Point2::new(40.0, 30.0),
        Point2::new(40.0, 70.0),
        Point2::new(100.0, 70.0),
        Point2::new(100.0, 100.0),
        Point2::new(0.0, 100.0),
      ],
      true,
    )
    .expect("c shaped pocket"),
  };
  add_circle(&mut document, "E2", Point2::new(70.0, 50.0), 8.0);
  document.toolpaths[0].island_ids = vec!["E2".to_owned()];
  let error = document.validate().expect_err("must reject");
  assert!(
    matches!(error, ModelError::IslandOutsideBoundary { .. }),
    "{error:?}"
  );
}

#[test]
fn two_overlapping_islands_are_rejected() {
  let mut document = pocket_document_with_island();
  add_circle(&mut document, "E3", Point2::new(56.0, 50.0), 10.0);
  document.toolpaths[0].island_ids = vec!["E2".to_owned(), "E3".to_owned()];
  let error = document.validate().expect_err("must reject");
  assert!(
    matches!(error, ModelError::IslandsOverlap { .. }),
    "{error:?}"
  );
}

#[test]
fn an_island_nested_inside_another_is_a_boss_and_is_accepted() {
  // A boss inside a hole: the r12 island sits strictly inside the r24 one, so a point in the boss is
  // inside three boundaries — the pocket's, the hole's and the boss's — which is odd, and the parity
  // rule therefore makes the boss material again (D-RB-2). The pair is *nested*, not overlapping, so
  // `validate` must accept it rather than refuse it as an overlap.
  let mut document = pocket_document_with_island();
  document.layers[0].entities[1].kind = EntityKind::Contour {
    contour: cam_geom::primitive::circle(Point2::new(50.0, 50.0), 24.0).expect("hole"),
  };
  add_circle(&mut document, "E3", Point2::new(50.0, 50.0), 12.0);
  document.toolpaths[0].island_ids = vec!["E2".to_owned(), "E3".to_owned()];

  document.validate().expect("a boss inside a hole is legal");

  // What the islands are to each other, asked the way validation asks it: the boss is strictly
  // inside the hole, so the pair reports a nesting and never a crossing.
  let hole = document.layers[0].entities[1]
    .kind
    .as_contour()
    .expect("contour");
  let boss = document.layers[0].entities[2]
    .kind
    .as_contour()
    .expect("contour");
  assert_eq!(
    cam_geom::winding::relation(hole, boss, cam_geom::DEFAULT_TOLERANCE),
    cam_geom::winding::Relation::SecondInside,
    "the boss is nested inside the hole"
  );
}

#[test]
fn two_islands_that_touch_are_rejected() {
  // Two rectangles sharing the edge x = 40: they are not disjoint and not nested either, so the wall
  // between them has no width and there is no region to cut.
  let mut document = pocket_document_with_island();
  document.layers[0].entities[1].kind = EntityKind::Contour {
    contour: cam_geom::primitive::rectangle(Point2::new(20.0, 40.0), 20.0, 20.0).expect("island"),
  };
  document.layers[0].entities.push(Entity {
    id: "E3".to_owned(),
    kind: EntityKind::Contour {
      contour: cam_geom::primitive::rectangle(Point2::new(40.0, 40.0), 20.0, 20.0).expect("island"),
    },
  });
  document.toolpaths[0].island_ids = vec!["E2".to_owned(), "E3".to_owned()];

  let first = document.layers[0].entities[1]
    .kind
    .as_contour()
    .expect("contour");
  let second = document.layers[0].entities[2]
    .kind
    .as_contour()
    .expect("contour");
  assert_eq!(
    cam_geom::winding::relation(first, second, cam_geom::DEFAULT_TOLERANCE),
    cam_geom::winding::Relation::Touching,
    "the pair shares an edge"
  );

  let error = document.validate().expect_err("must reject");
  assert!(
    matches!(error, ModelError::IslandsOverlap { .. }),
    "{error:?}"
  );
}

#[test]
fn a_repeated_island_is_rejected() {
  let mut document = pocket_document_with_island();
  document.toolpaths[0].island_ids = vec!["E2".to_owned(), "E2".to_owned()];
  let error = document.validate().expect_err("must reject");
  assert!(
    matches!(error, ModelError::DuplicateId { kind: "island", .. }),
    "{error:?}"
  );
}

#[test]
fn an_open_island_is_rejected() {
  let mut document = pocket_document_with_island();
  document.layers[0].entities[1].kind = EntityKind::Contour {
    contour: Contour::from_points(&[Point2::new(40.0, 50.0), Point2::new(60.0, 50.0)], false)
      .expect("open line"),
  };
  let error = document.validate().expect_err("must reject");
  assert!(
    matches!(error, ModelError::IslandNotClosed { ref island, .. } if island == "E2"),
    "{error:?}"
  );
}

#[test]
fn islands_on_a_profile_toolpath_are_rejected() {
  let mut document = document_with_square();
  add_circle(&mut document, "E2", Point2::new(50.0, 50.0), 10.0);
  document.toolpaths[0].island_ids = vec!["E2".to_owned()];
  let error = document.validate().expect_err("must reject");
  assert!(
    matches!(error, ModelError::IslandsRequirePocketOrInlay { .. }),
    "{error:?}"
  );
  // The sentence is the contract, not just the variant: `cam-cli` prints it verbatim and exits `2`.
  // It names **both** operations that take islands, because an inlay male has no boundary entity at
  // all — its boundary is the job rectangle — and its island list *is* the region it leaves standing
  // (M6 D-M6-13), so "only for a pocket" stopped being true when the pair landed (M6 L16).
  assert_eq!(
    error.to_string(),
    "toolpath \"TP1\" takes islands only for a pocket or an inlay"
  );
}

#[test]
fn a_legacy_schema_tag_is_migrated_and_an_unknown_one_is_not() {
  let mut document = pocket_document_with_island();
  document.schema = "c2d/document@1".to_owned();
  assert!(
    document.migrate(),
    "a legacy document reports the migration"
  );
  assert_eq!(document.schema, DOCUMENT_SCHEMA);
  assert!(
    !document.migrate(),
    "a current document is not migrated again"
  );

  let mut unknown = document.clone();
  unknown.schema = "c2d/document@3".to_owned();
  assert!(!unknown.migrate());
  let error = unknown.validate().expect_err("must reject");
  assert!(
    matches!(error, ModelError::UnsupportedSchema { .. }),
    "{error:?}"
  );
}

// --- fluting (M6 A4, D-M6-5, D-M6-12) --------------------------------------------------------

/// A fluting toolpath on the square's one entity, ramped over `ramp` millimetres.
fn fluting_document(ramp: f64) -> Document {
  document_with(Operation::Fluting(FlutingParams {
    start_depth: Length::ZERO,
    cut_depth: Length::from_mm(3.0),
    ramp: Length::from_mm(ramp),
  }))
}

#[test]
fn a_fluting_toolpath_round_trips_with_its_ramp() {
  let document = fluting_document(10.0);
  document
    .validate()
    .expect("a flute on closed geometry is valid");

  let json = serde_json::to_string(&document).expect("serialises");
  assert!(json.contains("\"kind\":\"fluting\""), "{json}");
  assert!(json.contains("\"ramp\":10.0"), "{json}");
  let parsed: Document = serde_json::from_str(&json).expect("deserialises");
  assert_eq!(parsed, document);
}

#[test]
fn a_fluting_toolpath_written_without_a_ramp_loads_as_the_plain_groove() {
  // `ramp` is defaulted rather than required, so a hand-written or older document that names a
  // `fluting` toolpath without the field loads as the in-place plunge it describes (A1, D-M6-5).
  let json = serde_json::to_string(&fluting_document(10.0)).expect("serialises");
  let without = json.replace(",\"ramp\":10.0", "");
  assert_ne!(without, json, "the archive carries the ramp: {json}");
  let parsed: Document = serde_json::from_str(&without).expect("a document without a ramp loads");
  match &parsed.toolpaths[0].operation {
    Operation::Fluting(params) => assert_eq!(params.ramp, Length::ZERO),
    other => panic!("expected a fluting toolpath, got {other:?}"),
  }
  parsed.validate().expect("and it is valid");
}

#[test]
fn a_negative_fluting_ramp_is_refused_at_load_naming_the_value() {
  // A negative ramp would enter upwards, so the document model refuses it as it loads rather than
  // leaving it for `emit` to discover (D-M6-5, A4). It is the same refusal `cam-toolpath` makes, at the
  // boundary that sees the document before any toolpath exists.
  let document = fluting_document(-2.0);
  let error = document
    .validate()
    .expect_err("a negative ramp must be refused");
  assert_eq!(
    error,
    ModelError::FlutingRampNegative {
      toolpath: "TP1".to_owned(),
      value: -2.0,
    }
  );
  assert!(
    error.to_string().contains("-2"),
    "the refusal names the value: {error}"
  );
  // Zero is the plain groove, not a negative ramp, and half the vector is the longest legal ramp this
  // model can see without measuring the geometry (that check is the strategy's).
  fluting_document(0.0).validate().expect("no ramp is legal");
}

#[test]
fn a_flute_may_cut_an_open_vector() {
  // A flute is one groove per vector, and a vector drawn as a line is the commonest case. Every other
  // cutting operation in the model needs closed geometry; engrave and fluting are the two that do not.
  let mut document = fluting_document(5.0);
  document.layers[0].entities[0].kind = EntityKind::Contour {
    contour: Contour::from_points(&[Point2::new(0.0, 0.0), Point2::new(10.0, 0.0)], false)
      .expect("open polyline"),
  };
  document
    .validate()
    .expect("a flute of an open vector is valid");
}

// --- texture (M6 A5, D-M6-6) -----------------------------------------------------------------

/// A texture toolpath over a square `size` millimetres across, `spacing` apart at `angle` degrees.
fn texture_document(size: f64, cut_depth: f64, spacing: f64, degrees: f64) -> Document {
  let mut document = document_with(Operation::Texture(TextureParams {
    start_depth: Length::ZERO,
    cut_depth: Length::from_mm(cut_depth),
    spacing: Length::from_mm(spacing),
    angle: Angle::from_degrees(degrees),
  }));
  document.layers[0].entities[0].kind = EntityKind::Contour {
    contour: square(size),
  };
  document
}

/// The grooves the document's own pattern cuts, as document XY polylines.
fn grooves_of(document: &Document) -> Vec<Vec<Point2>> {
  let Operation::Texture(params) = &document.toolpaths[0].operation else {
    panic!("expected a texture toolpath");
  };
  let contours: Vec<&Contour> = document.toolpaths[0]
    .entity_ids
    .iter()
    .map(|id| {
      document
        .entity(id)
        .and_then(|entity| entity.kind.as_contour())
        .expect("a contour")
    })
    .collect();
  params.grooves(&contours)
}

#[test]
fn a_texture_pattern_across_a_rectangle_is_rows_a_spacing_apart() {
  // D-M6-6's pattern, stated arithmetically: a 10 x 10 mm square at 0 degrees with a 2 mm spacing. The
  // first groove sits half a spacing inside the region, so the rows are y = 1, 3, 5, 7 and 9, and each
  // is clipped to the square's own extent, x = 0 to 10. Five grooves, each 10 mm long — the pattern,
  // and never a clearing of the 100 mm2 between them.
  let document = texture_document(10.0, 3.0, 2.0, 0.0);
  document.validate().expect("a texture of a closed region");
  let grooves = grooves_of(&document);

  assert_eq!(
    grooves.len(),
    5,
    "five rows a 2 mm spacing apart: {grooves:?}"
  );
  assert_eq!(
    grooves[0],
    vec![Point2::new(0.0, 1.0), Point2::new(10.0, 1.0)],
    "the first groove is half a spacing inside the region"
  );
  assert_eq!(
    grooves[4],
    vec![Point2::new(0.0, 9.0), Point2::new(10.0, 9.0)],
    "and the last is the furthest one that still crosses material"
  );
  for (index, groove) in grooves.iter().enumerate() {
    let y = 1.0 + 2.0 * index as f64;
    assert_eq!(groove.len(), 2, "one straight span per row: {groove:?}");
    assert!((groove[0].y - y).abs() < 1e-12, "{groove:?} at y = {y}");
    assert!((groove[1].y - y).abs() < 1e-12, "{groove:?} at y = {y}");
    assert!(
      (groove[1].x - groove[0].x - 10.0).abs() < 1e-12,
      "{groove:?}"
    );
  }
}

#[test]
fn a_texture_pattern_at_forty_five_degrees_is_clipped_by_the_region_and_not_by_its_bounds() {
  // The same square at 45 degrees. In the groove frame the region is a diamond whose extent is the
  // diagonal, `2 * 10 / sqrt(2) = 14.142` mm, so a 2 mm spacing gives seven rows starting half a
  // spacing inside it. Each groove is the diamond's own chord, so the pattern is the region's — a
  // rotation of the region, never of the document and never a rotation of its bounding box.
  let document = texture_document(10.0, 3.0, 2.0, 45.0);
  document.validate().expect("a texture of a closed region");
  let grooves = grooves_of(&document);

  let sqrt2 = std::f64::consts::SQRT_2;
  let half_spacing = 1.0;
  let diagonal = 10.0 / sqrt2;
  assert_eq!(
    grooves.len(),
    7,
    "the diagonal is 14.142 mm, so a 2 mm spacing gives 7 rows: {grooves:?}"
  );

  // The first row is `diagonal/2 - half_spacing` from the diamond's low corner, which puts it
  // `half_spacing * sqrt(2)` from the square's corner (10, 0) along each of the two edges it meets.
  let first = &grooves[0];
  let corner = Point2::new(10.0 - half_spacing * sqrt2, 0.0);
  let edge = Point2::new(10.0, half_spacing * sqrt2);
  assert!(
    (first[0].x - corner.x).abs() < 1e-9 && (first[0].y - corner.y).abs() < 1e-9,
    "the first groove starts {half_spacing} mm across from the corner: {first:?} against {corner:?}"
  );
  assert!(
    (first[1].x - edge.x).abs() < 1e-9 && (first[1].y - edge.y).abs() < 1e-9,
    "and meets the next edge the same distance up it: {first:?} against {edge:?}"
  );

  // The last row is six spacings further in, and the groove `y - x = offset` crosses the square's two
  // other edges where the pattern says it does.
  let offset = (-diagonal + half_spacing + 6.0 * 2.0) * sqrt2;
  let last = &grooves[6];
  assert!(
    (last[0].x).abs() < 1e-9 && (last[0].y - offset).abs() < 1e-9,
    "the last groove starts on the left edge: {last:?} at offset {offset}"
  );
  assert!(
    (last[1].y - 10.0).abs() < 1e-9 && (last[1].x - (10.0 - offset)).abs() < 1e-9,
    "and ends on the top edge: {last:?} at offset {offset}"
  );

  // Every groove runs along the 45 degree diagonal, and none of them is axis-aligned: a pattern that
  // rotated the square's bounding box instead of the region would give rows parallel to the axes.
  for groove in &grooves {
    let span = groove[1] - groove[0];
    assert!(
      (span.x.abs() - span.y.abs()).abs() < 1e-9,
      "a 45 degree groove: {groove:?}"
    );
    assert!(
      span.length() > 0.0,
      "and it has material to cut: {groove:?}"
    );
  }
}

#[test]
fn a_texture_toolpath_round_trips_with_its_spacing_and_angle() {
  let document = texture_document(10.0, 3.0, 2.5, 30.0);
  document
    .validate()
    .expect("a texture of a closed region is valid");
  let json = serde_json::to_string(&document).expect("serialises");
  assert!(json.contains("\"kind\":\"texture\""), "{json}");
  assert!(json.contains("\"spacing\":2.5"), "{json}");
  let parsed: Document = serde_json::from_str(&json).expect("deserialises");
  assert_eq!(parsed, document);
}

#[test]
fn a_texture_toolpath_written_without_its_spacing_and_angle_loads_with_the_defaults() {
  // Both fields are defaulted rather than required, so a hand-written document that names a `texture`
  // toolpath without them loads as the pattern it describes — 3 mm apart, along X — instead of failing
  // the load (A1, D-M6-6).
  let document = texture_document(10.0, 3.0, 5.0, 45.0);
  let json = serde_json::to_string(&document).expect("serialises");
  let angle = Angle::from_degrees(45.0).radians();
  let without = json
    .replace(",\"spacing\":5.0", "")
    .replace(&format!(",\"angle\":{angle}"), "");
  assert_ne!(without, json, "the archive carries both fields: {json}");
  let parsed: Document = serde_json::from_str(&without).expect("a document without them loads");
  match &parsed.toolpaths[0].operation {
    Operation::Texture(params) => {
      assert_eq!(params.spacing, TextureParams::default().spacing);
      assert_eq!(params.angle, Angle::ZERO);
    }
    other => panic!("expected a texture toolpath, got {other:?}"),
  }
  parsed.validate().expect("and it is valid");
}

#[test]
fn a_non_positive_texture_spacing_is_refused_at_load_naming_the_value() {
  // A lattice of grooves `spacing` apart, with no spacing, is an input error and never an empty
  // pattern: it is refused here, at the boundary that sees the document before any toolpath exists,
  // and again by `cam-toolpath::texture` (D-M6-6).
  for spacing in [0.0, -2.0] {
    let document = texture_document(10.0, 3.0, spacing, 0.0);
    let error = document
      .validate()
      .expect_err("a spacing with no distance behind it must be refused");
    assert_eq!(
      error,
      ModelError::TextureSpacingNotPositive {
        toolpath: "TP1".to_owned(),
        value: spacing,
      }
    );
    assert!(
      error.to_string().contains(&spacing.to_string()),
      "the refusal names the value: {error}"
    );
  }
}

#[test]
fn a_texture_toolpath_on_open_geometry_is_rejected() {
  // A texture's grooves are clipped to a region, and an open vector bounds none (D-M6-6). A flute and
  // an engrave are the two strategies that accept one.
  let mut document = texture_document(10.0, 3.0, 2.0, 0.0);
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

// --- inlay (M6 A6, D-M6-7) -------------------------------------------------------------------

/// The pair one `toolpath --kind inlay` command appends: a female that clears `E1` and a male that
/// leaves it standing inside the job rectangle, both carrying the same parameters.
///
/// The region is a 100 mm square strictly inside the default 1219 x 2438 sheet, because an inlay's
/// region has to sit inside the stock: the male clears the material around it.
fn inlay_pair(params: InlayParams) -> Document {
  let mut document = Document::new(Job::default_sheet());
  document.layers.push(Layer {
    id: "L1".to_owned(),
    name: "Part".to_owned(),
    visible: true,
    entities: vec![Entity {
      id: "E1".to_owned(),
      kind: EntityKind::Contour {
        contour: Contour::from_points(
          &[
            Point2::new(100.0, 100.0),
            Point2::new(200.0, 100.0),
            Point2::new(200.0, 200.0),
            Point2::new(100.0, 200.0),
          ],
          true,
        )
        .expect("a square inside the stock"),
      },
    }],
  });
  document.toolpaths.push(Toolpath {
    id: "TP1".to_owned(),
    name: "inlay female".to_owned(),
    entity_ids: vec!["E1".to_owned()],
    island_ids: Vec::new(),
    tool_number: 1,
    operation: Operation::Inlay(params.clone()),
  });
  document.toolpaths.push(Toolpath {
    id: "TP2".to_owned(),
    name: "inlay male".to_owned(),
    entity_ids: Vec::new(),
    island_ids: vec!["E1".to_owned()],
    tool_number: 1,
    operation: Operation::Inlay(params),
  });
  document
}

fn inlay_params(allowance: f64, stepover: f64) -> InlayParams {
  InlayParams {
    allowance: Length::from_mm(allowance),
    stepover: Length::from_mm(stepover),
    ..InlayParams::default()
  }
}

#[test]
fn an_inlay_pair_round_trips_and_is_two_toolpaths_sharing_one_operation() {
  let document = inlay_pair(inlay_params(1.5, 2.4));
  document.validate().expect("a closed region inlay");

  let json = serde_json::to_string(&document).expect("serialises");
  assert!(json.contains("\"kind\":\"inlay\""), "{json}");
  assert!(json.contains("\"allowance\":1.5"), "{json}");
  let parsed: Document = serde_json::from_str(&json).expect("deserialises");
  assert_eq!(parsed, document);

  // The two halves carry the same parameters: the document states which half each one is with what it
  // already records about a toolpath's work — the entities it clears and the islands it leaves standing.
  match (
    &parsed.toolpaths[0].operation,
    &parsed.toolpaths[1].operation,
  ) {
    (Operation::Inlay(female), Operation::Inlay(male)) => assert_eq!(female, male),
    other => panic!("expected two inlay halves, got {other:?}"),
  }
  assert_eq!(parsed.toolpaths[0].island_ids, Vec::<String>::new());
  assert_eq!(parsed.toolpaths[1].entity_ids, Vec::<String>::new());
  assert_eq!(parsed.toolpaths[1].island_ids, vec!["E1".to_owned()]);
}

#[test]
fn an_inlay_defaults_to_the_pockets_own_numbers() {
  // D-M6-7's "one depth and one stepover rather than two extra flags": an inlay that authors nothing
  // clears at the depth and the stepover a pocket would, taken from the pocket rather than restated.
  let inlay = InlayParams::default();
  let pocket = PocketParams::default();
  assert_eq!(inlay.allowance, Length::ZERO);
  assert_eq!(inlay.stepover, pocket.stepover);
  assert_eq!(inlay.pass_depth, pocket.pass_depth);
  assert_eq!(inlay.start_depth, pocket.start_depth);
  assert_eq!(inlay.cut_depth, pocket.cut_depth);
}

#[test]
fn a_non_positive_inlay_stepover_is_refused_at_load_naming_the_value() {
  // Both halves clear with the pocket's ring machinery, so both need a distance between rings: a
  // lattice with no stepover leaves material standing between the rings.
  for stepover in [0.0, -2.0] {
    let document = inlay_pair(inlay_params(1.0, stepover));
    let error = document
      .validate()
      .expect_err("a stepover with no distance behind it must be refused");
    assert_eq!(
      error,
      ModelError::InlayStepoverNotPositive {
        toolpath: "TP1".to_owned(),
        value: stepover,
      }
    );
    assert!(
      error.to_string().contains(&stepover.to_string()),
      "the refusal names the value: {error}"
    );
  }
}

#[test]
fn an_inlay_depth_range_with_no_pass_in_it_is_refused_at_load_naming_both_depths() {
  for (start, cut) in [(6.0, 6.0), (6.0, 2.0)] {
    let params = InlayParams {
      start_depth: Length::from_mm(start),
      cut_depth: Length::from_mm(cut),
      ..inlay_params(1.0, 2.0)
    };
    let document = inlay_pair(params);
    let error = document
      .validate()
      .expect_err("a cut above its start depth has no pass behind it");
    assert_eq!(
      error,
      ModelError::InlayDepthRange {
        toolpath: "TP1".to_owned(),
        cut_depth: cut,
        start_depth: start,
      }
    );
    assert!(
      error.to_string().contains(&cut.to_string())
        && error.to_string().contains(&start.to_string()),
      "the refusal names both depths: {error}"
    );
  }
}

#[test]
fn an_inlay_toolpath_that_is_both_halves_is_refused_rather_than_guessed() {
  // A female names the region it clears; a male names it as the islands it leaves standing and takes no
  // boundary entity, because its boundary is the job rectangle. A record that names both is two halves in
  // one toolpath, and which of them a program would cut is not something to guess (see `InlayParams`).
  let mut document = inlay_pair(inlay_params(1.0, 2.0));
  document.toolpaths[1].entity_ids = vec!["E1".to_owned()];
  let error = document.validate().expect_err("must reject");
  assert!(
    matches!(error, ModelError::InlayMaleTakesNoEntities { ref toolpath } if toolpath == "TP2"),
    "{error:?}"
  );
}

#[test]
fn an_inlay_toolpath_on_open_geometry_is_rejected() {
  let mut document = inlay_pair(inlay_params(1.0, 2.0));
  document.layers[0].entities[0].kind = EntityKind::Contour {
    contour: Contour::from_points(&[Point2::new(0.0, 0.0), Point2::new(10.0, 0.0)], false)
      .expect("open polyline"),
  };
  // The female's own rule fires first, and the male's island is refused for the same reason: a cavity
  // and a plug are both regions, and an open vector bounds none.
  let error = document.validate().expect_err("must reject");
  assert!(
    matches!(error, ModelError::OpenContourNotCuttable { .. }),
    "{error:?}"
  );
}

#[test]
fn an_inlay_male_stands_its_islands_inside_the_job_rectangle() {
  // A male's boundary is the job, which is not an entity, so the model checks its islands against
  // `Job::outline()`: a region outside the stock has nothing beside it for the male to clear.
  let mut document = inlay_pair(inlay_params(1.0, 2.0));
  let outside = Contour::from_points(
    &[
      Point2::new(1500.0, 1200.0),
      Point2::new(1600.0, 1200.0),
      Point2::new(1600.0, 1300.0),
      Point2::new(1500.0, 1300.0),
    ],
    true,
  )
  .expect("a square past the stock's corner");
  document.layers[0].entities.push(Entity {
    id: "E2".to_owned(),
    kind: EntityKind::Contour { contour: outside },
  });
  document.toolpaths[1].island_ids = vec!["E2".to_owned()];
  let error = document.validate().expect_err("must reject");
  assert!(
    matches!(error, ModelError::IslandOutsideBoundary { ref island, .. } if island == "E2"),
    "{error:?}"
  );
}

#[test]
fn the_job_outline_is_one_rectangle_in_document_coordinates() {
  // The one place the job rectangle is built (M6 A6): `cam-toolpath::inlay`'s male half clears it and
  // `cam-sim` authorises it, so both ask here.
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
  // Document coordinates, not machine ones: the origin corner is applied later, once, by the toolpath
  // layer, so a top-left job's outline is the same rectangle.
  let top_left = Job {
    origin: OriginCorner::TopLeft,
    ..job
  };
  assert_eq!(top_left.outline(), outline);
}

// --- thread milling (M6 A7, D-M6-8) ----------------------------------------------------------

/// A thread toolpath on the square's one entity: `pitch` millimetres a turn, `cut` deep, from `start`.
fn thread_document(pitch: f64, direction: ThreadDirection, start: f64, cut: f64) -> Document {
  document_with(Operation::Thread(ThreadParams {
    pitch: Length::from_mm(pitch),
    direction,
    start_depth: Length::from_mm(start),
    cut_depth: Length::from_mm(cut),
  }))
}

#[test]
fn a_thread_toolpath_round_trips_with_its_direction() {
  // `ThreadDirection` is a snake_case tag like every other enum in the schema, and the operation is
  // additive under the existing `kind` tag (A1): the schema stays `c2d/document@2`.
  let document = thread_document(1.5, ThreadDirection::External, 1.0, 9.0);
  document
    .validate()
    .expect("a thread on closed geometry is valid");

  let json = serde_json::to_string(&document).expect("serialises");
  assert!(json.contains("\"kind\":\"thread\""), "{json}");
  assert!(json.contains("\"direction\":\"external\""), "{json}");
  assert!(json.contains("\"pitch\":1.5"), "{json}");
  let parsed: Document = serde_json::from_str(&json).expect("deserialises");
  assert_eq!(parsed, document);
  assert!(
    json.contains(DOCUMENT_SCHEMA),
    "the schema tag is unchanged"
  );
}

#[test]
fn a_threads_pitch_must_be_a_positive_number_of_millimetres() {
  // A pitch of zero is a descent of zero per revolution, which is not a thread: it is refused naming the
  // toolpath and the value rather than clamped into one.
  for pitch in [0.0, -1.5] {
    let document = thread_document(pitch, ThreadDirection::Internal, 0.0, 6.0);
    let error = document
      .validate()
      .expect_err("must refuse a pitch with no thread behind it");
    assert_eq!(
      error,
      ModelError::ThreadPitchNotPositive {
        toolpath: "TP1".to_owned(),
        value: pitch,
      }
    );
    assert!(
      error.to_string().contains(&pitch.to_string()),
      "the refusal names the value: {error}"
    );
  }
}

#[test]
fn a_threads_cut_depth_must_extend_below_its_start_depth() {
  let document = thread_document(1.5, ThreadDirection::Internal, 6.0, 6.0);
  let error = document.validate().expect_err("nothing to cut");
  assert_eq!(
    error,
    ModelError::ThreadDepthRange {
      toolpath: "TP1".to_owned(),
      cut_depth: 6.0,
      start_depth: 6.0,
    }
  );
}

#[test]
fn a_thread_needs_closed_geometry() {
  // A thread runs around a circle, and an open vector bounds no circle. The model refuses the open
  // vector before the strategy has to, and `cam-toolpath::thread` refuses a closed non-circle too.
  let mut document = thread_document(1.5, ThreadDirection::Internal, 0.0, 6.0);
  document.layers[0].entities[0].kind = EntityKind::Contour {
    contour: Contour::from_points(&[Point2::new(0.0, 0.0), Point2::new(10.0, 0.0)], false)
      .expect("an open vector"),
  };
  let error = document.validate().expect_err("must reject");
  assert!(
    matches!(error, ModelError::OpenContourNotCuttable { .. }),
    "{error:?}"
  );
}

// --- V-carve (M7 A1, A4, D-M7-3, D-M7-5) ------------------------------------------------------

/// A V-carve toolpath on the square's one entity: a bit of `degrees`, its axis refined to `tolerance`.
fn v_carve_document(degrees: f64, tolerance: f64) -> Document {
  document_with(Operation::VCarve(VCarveParams {
    included_angle: Angle::from_degrees(degrees),
    tolerance: Length::from_mm(tolerance),
  }))
}

#[test]
fn a_v_carve_toolpath_round_trips_with_its_angle_and_tolerance() {
  // The variant is additive under the existing `kind` tag, so the schema does not move (A4, D-M4-1):
  // `c2d/document@2` reads and writes it, and a document written before it existed is untouched.
  let document = v_carve_document(60.0, 0.25);
  document
    .validate()
    .expect("a V-carve on closed geometry is valid");

  let json = serde_json::to_string(&document).expect("serialises");
  assert!(json.contains("\"kind\":\"v_carve\""), "{json}");
  assert!(json.contains("\"tolerance\":0.25"), "{json}");
  assert!(
    json.contains("\"schema\":\"c2d/document@2\""),
    "the schema tag does not move for an additive variant: {json}"
  );
  let parsed: Document = serde_json::from_str(&json).expect("deserialises");
  assert_eq!(parsed, document);
  match &parsed.toolpaths[0].operation {
    Operation::VCarve(params) => {
      assert_eq!(params.included_angle, Angle::from_degrees(60.0));
      assert_eq!(params.tolerance, Length::from_mm(0.25));
    }
    other => panic!("expected a V-carve, got {other:?}"),
  }

  // The tolerance is defaulted rather than required, so a hand-written or older document that names a
  // `v_carve` toolpath without the field loads as the axis the CLI's own documented default writes
  // (D-M7-5), instead of failing the load.
  let without = json.replace(",\"tolerance\":0.25", "");
  assert_ne!(without, json, "the archive carries the tolerance: {json}");
  let reloaded: Document =
    serde_json::from_str(&without).expect("a document without a tolerance loads");
  match &reloaded.toolpaths[0].operation {
    Operation::VCarve(params) => assert_eq!(params.tolerance, Length::from_mm(0.1)),
    other => panic!("expected a V-carve, got {other:?}"),
  }
  reloaded.validate().expect("and it is valid");
}

#[test]
fn a_v_carves_depth_is_the_clearance_over_the_tangent_of_the_half_angle() {
  // The law lives beside the tool (D-M6-2's precedent, A1): a tip whose clearance is `d` sits
  // `d / tan(theta / 2)` below the surface. Written here from the geometry and never read back from
  // the function under test, so a derivation that forgot the half angle or the tangent fails.
  for (degrees, expected) in [
    (90.0, 4.0_f64),              // tan(45) = 1
    (60.0, 4.0 * 3.0_f64.sqrt()), // tan(30) = 1/sqrt(3)
    (30.0, 4.0 / 15.0_f64.to_radians().tan()),
  ] {
    let params = VCarveParams {
      included_angle: Angle::from_degrees(degrees),
      tolerance: Length::from_mm(0.1),
    };
    let depth = params.depth(Length::from_mm(4.0)).mm();
    assert!(
      (depth - expected).abs() < 1.0e-9,
      "a {degrees} degree bit carves 4 mm of clearance {expected} mm deep, got {depth}"
    );
  }
}

#[test]
fn a_v_carve_record_carries_no_start_depth() {
  // D-M7-3: with a start depth the cone's top reaches `start * tan(theta/2)` outside the vectors and
  // the carve stops being the distance field, so it is refused by name in the CLI rather than stored
  // as a field that defaults to zero. This is the structural half of that refusal: there is no field
  // to default.
  let json = serde_json::to_string(&v_carve_document(90.0, 0.1)).expect("serialises");
  assert!(
    !json.contains("start_depth"),
    "a V-carve has no start depth to store: {json}"
  );
}

#[test]
fn a_v_carve_angle_outside_the_range_is_refused_at_load_naming_the_value() {
  // An angle at either end of `(0, 180)` has a tangent of zero or infinity, so the depth law has no
  // depth behind it. The document is the boundary that sees the value before any toolpath exists (A4).
  for degrees in [0.0, 180.0] {
    let document = v_carve_document(degrees, 0.1);
    let error = document
      .validate()
      .expect_err("must refuse an angle with no cone behind it");
    assert_eq!(
      error,
      ModelError::VCarveAngleOutOfRange {
        toolpath: "TP1".to_owned(),
        degrees,
      }
    );
    assert!(
      error.to_string().contains(&degrees.to_string()),
      "the refusal names the value: {error}"
    );
  }
}

#[test]
fn a_v_carve_tolerance_must_be_a_positive_number_of_millimetres() {
  // The tolerance is the medial axis's lattice step (D-M7-5), so it is stored with the operation and
  // refused by name when it is not a step: a zero or negative one would ask `cam_geom::medial` for an
  // axis it refuses, and a document that can never emit is worse than a refusal at load.
  for tolerance in [0.0, -0.1] {
    let document = v_carve_document(90.0, tolerance);
    let error = document
      .validate()
      .expect_err("must refuse a tolerance with no step behind it");
    assert_eq!(
      error,
      ModelError::VCarveToleranceNotPositive {
        toolpath: "TP1".to_owned(),
        value: tolerance,
      }
    );
    assert!(
      error.to_string().contains(&tolerance.to_string()),
      "the refusal names the value: {error}"
    );
  }
}

#[test]
fn a_v_carve_needs_closed_geometry() {
  // A V-carve's region is what the contours enclose, so it extends the one closed-contour rule every
  // cutting operation already obeys rather than adding a second one (A4).
  let mut document = v_carve_document(90.0, 0.1);
  document.layers[0].entities[0].kind = EntityKind::Contour {
    contour: Contour::from_points(&[Point2::new(0.0, 0.0), Point2::new(10.0, 0.0)], false)
      .expect("an open vector"),
  };
  let error = document.validate().expect_err("must reject");
  assert!(
    matches!(error, ModelError::OpenContourNotCuttable { .. }),
    "{error:?}"
  );
}

#[test]
fn a_pre_existing_document_still_re_saves_byte_identically() {
  // A4's last clause: every previously serialised document loads and re-saves byte-identically, so
  // the new variant must not have moved a single byte of what an older document already says.
  let mut document = pocket_document_with_island();
  document.toolpaths[0].operation = Operation::Pocket(PocketParams {
    strategy: PocketStrategy::Raster,
    allowance: Length::from_mm(0.4),
    stepover: Length::from_mm(2.5),
    pass_depth: Length::from_mm(6.0),
    start_depth: Length::ZERO,
    cut_depth: Length::from_mm(9.0),
  });
  document.toolpaths.push(Toolpath {
    id: "TP2".to_owned(),
    name: "Bevel".to_owned(),
    entity_ids: vec!["E1".to_owned()],
    island_ids: Vec::new(),
    tool_number: 2,
    operation: Operation::Chamfer(ChamferParams {
      chamfer_width: Length::from_mm(3.0),
      included_angle: Angle::from_degrees(60.0),
      start_depth: Length::ZERO,
    }),
  });
  document.validate().expect("a valid document");

  let stored = serde_json::to_string(&document).expect("serialises");
  let reloaded: Document = serde_json::from_str(&stored).expect("deserialises");
  assert_eq!(
    serde_json::to_string(&reloaded).expect("serialises"),
    stored,
    "a document written before M7 is re-saved byte for byte"
  );
  reloaded.validate().expect("and it still validates");
}

#[test]
fn a_moulding_toolpath_round_trips_its_rail_and_profile() {
  // S3: the rail goes in XY as the selected entity; the profile carries (lateral offset, Z depth)
  // points and the stepover spaces the overlapped lateral passes.
  let mut document = document_with_square();
  document.toolpaths[0].entity_ids = vec!["E1".to_owned()];
  document.toolpaths[0].operation = Operation::Moulding(MouldingParams {
    profile: vec![
      MouldingProfilePoint {
        lateral: Length::from_mm(0.0),
        depth: Length::ZERO,
      },
      MouldingProfilePoint {
        lateral: Length::from_mm(6.0),
        depth: Length::from_mm(-4.0),
      },
      MouldingProfilePoint {
        lateral: Length::from_mm(12.0),
        depth: Length::ZERO,
      },
    ],
    stepover: Length::from_mm(2.0),
  });
  document.validate().expect("valid moulding document");
  let json = serde_json::to_string(&document).expect("serialises");
  assert!(json.contains("\"kind\":\"moulding\""), "{json}");
  let parsed: Document = serde_json::from_str(&json).expect("deserialises");
  assert_eq!(parsed, document);
}

#[test]
fn a_moulding_toolpath_without_a_profile_is_refused() {
  let mut document = document_with_square();
  document.toolpaths[0].operation = Operation::Moulding(MouldingParams {
    profile: Vec::new(),
    stepover: Length::from_mm(2.0),
  });
  let error = document
    .validate()
    .expect_err("must refuse an empty profile");
  assert!(
    matches!(error, ModelError::MouldingProfileTooShort { .. }),
    "{error:?}"
  );
}

#[test]
fn a_moulding_toolpath_with_a_non_positive_stepover_is_refused() {
  let mut document = document_with_square();
  document.toolpaths[0].operation = Operation::Moulding(MouldingParams {
    profile: vec![
      MouldingProfilePoint {
        lateral: Length::from_mm(0.0),
        depth: Length::ZERO,
      },
      MouldingProfilePoint {
        lateral: Length::from_mm(6.0),
        depth: Length::from_mm(-4.0),
      },
    ],
    stepover: Length::ZERO,
  });
  let error = document
    .validate()
    .expect_err("must refuse a zero stepover");
  assert!(
    matches!(error, ModelError::MouldingStepoverNotPositive { .. }),
    "{error:?}"
  );
}

#[test]
fn a_moulding_toolpath_with_a_non_finite_profile_point_is_refused() {
  let mut document = document_with_square();
  document.toolpaths[0].operation = Operation::Moulding(MouldingParams {
    profile: vec![
      MouldingProfilePoint {
        lateral: Length::from_mm(0.0),
        depth: Length::ZERO,
      },
      MouldingProfilePoint {
        lateral: Length::from_mm(f64::INFINITY),
        depth: Length::from_mm(-4.0),
      },
    ],
    stepover: Length::from_mm(2.0),
  });
  let error = document
    .validate()
    .expect_err("must refuse a non-finite point");
  assert!(
    matches!(error, ModelError::MouldingProfilePointNotFinite { .. }),
    "{error:?}"
  );
}
