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

// M9: the optional nesting section (S1-S4). A master document carries N sheets, N parts and the
// placements the nester chose, plus the cost record the report reads.

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

fn nesting_document() -> Document {
  let mut document = Document::new(Job::default_sheet());
  document.layers.push(Layer {
    id: "L1".to_owned(),
    name: "Parts".to_owned(),
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
fn a_document_written_before_nesting_loads_and_re_saves_byte_identically() {
  // The section is optional and top level, the `tools_ref`/`postp` precedent: a document authored
  // before M9 must not gain a byte, so the comparison is against the bytes an older build wrote.
  let before_m9 = r#"{"schema":"c2d/document@2","job":{"width":1219.0,"height":2438.0,"thickness":18.0,"units":"mm","origin":"bottom_left","safe_z":5.0},"layers":[{"id":"L1","name":"Part","visible":true,"entities":[{"id":"E1","type":"contour","contour":{"vertices":[{"point":{"x":0.0,"y":0.0},"bulge":0.0},{"point":{"x":100.0,"y":0.0},"bulge":0.0},{"point":{"x":100.0,"y":100.0},"bulge":0.0},{"point":{"x":0.0,"y":100.0},"bulge":0.0}],"closed":true}}]}],"toolpaths":[]}"#;
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
  // M11 S3: the material is optional and top level inside the job, the `nesting`/`tools_ref`
  // precedent, so a document whose job names no material must not gain a byte — the six keys a
  // pre-M11 build wrote are all it says.
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
  // M11 S1: the document is where the selector lives, so the material it names has to survive a save
  // and a load unchanged — it is the only thing that decides which cutting data the program uses.
  let mut document = document_with_square();
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
  assert_eq!(DOCUMENT_SCHEMA, "c2d/document@2", "no schema bump (S3)");
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
  assert_eq!(DOCUMENT_SCHEMA, "c2d/document@2");
  let json = serde_json::to_string(&nesting_document()).expect("serialises");
  assert!(json.contains("\"schema\":\"c2d/document@2\""), "{json}");
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
