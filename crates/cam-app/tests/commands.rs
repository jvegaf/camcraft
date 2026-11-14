//! Behaviour tests for the editing commands (A1, A5, A8).

use cam_app::{AlignMode, AppState, Command, CommandError, Transform, VertexEdit};
use cam_geom::{primitive, transform, Point2, Vec2};
use cam_model::{
  CutoutParams, Document, Entity, EntityKind, IsolationKind, IsolationParams, Job, Layer,
  LayerRole, MillingDirection, ModelError, Operation, TabKind,
};
use cam_units::Length;

fn state_with_layers(layer_ids: &[&str]) -> AppState {
  let mut document = Document::new(Job::default_sheet());
  for (index, id) in layer_ids.iter().enumerate() {
    let (role, side) = if index == 0 {
      (LayerRole::Outline, cam_model::Side::Top)
    } else {
      (LayerRole::Keepout, cam_model::Side::Both)
    };
    document.layers.push(Layer {
      id: (*id).to_owned(),
      role,
      side,
      color: "#000000".to_owned(),
      visible: true,
      source: cam_model::Provenance::default(),
      entities: Vec::new(),
    });
  }
  AppState::new(document)
}

fn state_with_layer() -> AppState {
  state_with_layers(&["L1"])
}

/// The canonical JSON of the document, used to prove a refusal or an undo changed nothing.
fn document_text(app: &AppState) -> String {
  serde_json::to_string(&app.document_json()).expect("the document serialises")
}

/// The id of the layer holding `id`.
fn layer_holding<'a>(app: &'a AppState, id: &str) -> &'a str {
  app
    .document()
    .layers
    .iter()
    .find(|layer| layer.entities.iter().any(|entity| entity.id == id))
    .map(|layer| layer.id.as_str())
    .expect("the entity is on a layer")
}

/// Apply `command`, require a refusal, and prove it left no trace.
fn assert_refused_atomically(app: &mut AppState, command: Command) -> CommandError {
  let before = document_text(app);
  let depth = app.history_depth();
  let error = app
    .execute(command)
    .expect_err("the command must be refused");
  assert_eq!(
    before,
    document_text(app),
    "a refusal must not touch the document"
  );
  assert_eq!(
    depth,
    app.history_depth(),
    "a refusal must not grow the undo history"
  );
  error
}

/// Draw a valid shape of the same kind, so the refusal test starts with a non-empty history.
fn prime_with_line(app: &mut AppState) {
  let id = app.next_entity_id();
  app
    .execute(Command::AddLine {
      layer_id: "L1".to_owned(),
      entity_id: id,
      from: Point2::new(0.0, 0.0),
      to: Point2::new(10.0, 0.0),
    })
    .expect("the priming line is drawn");
}

fn prime_with_polyline(app: &mut AppState) {
  let id = app.next_entity_id();
  app
    .execute(Command::AddPolyline {
      layer_id: "L1".to_owned(),
      entity_id: id,
      points: vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0)],
      closed: false,
    })
    .expect("the priming polyline is drawn");
}

fn prime_with_arc(app: &mut AppState) {
  let id = app.next_entity_id();
  app
    .execute(Command::AddArc {
      layer_id: "L1".to_owned(),
      entity_id: id,
      start: Point2::new(0.0, 0.0),
      through: Point2::new(10.0, 10.0),
      end: Point2::new(20.0, 0.0),
    })
    .expect("the priming arc is drawn");
}

fn prime_with_ellipse(app: &mut AppState) {
  let id = app.next_entity_id();
  app
    .execute(Command::AddEllipse {
      layer_id: "L1".to_owned(),
      entity_id: id,
      at: Point2::ORIGIN,
      rx: 40.0,
      ry: 10.0,
      tolerance: 0.01,
    })
    .expect("the priming ellipse is drawn");
}

fn prime_with_polygon(app: &mut AppState) {
  let id = app.next_entity_id();
  app
    .execute(Command::AddPolygon {
      layer_id: "L1".to_owned(),
      entity_id: id,
      at: Point2::ORIGIN,
      radius: 10.0,
      sides: 6,
      rotation: 0.0,
    })
    .expect("the priming polygon is drawn");
}

fn prime_with_star(app: &mut AppState) {
  let id = app.next_entity_id();
  app
    .execute(Command::AddStar {
      layer_id: "L1".to_owned(),
      entity_id: id,
      at: Point2::ORIGIN,
      outer_radius: 20.0,
      inner_radius: 8.0,
      points: 5,
      rotation: 0.0,
    })
    .expect("the priming star is drawn");
}

fn entity_contour<'a>(app: &'a AppState, id: &str) -> &'a cam_geom::Contour {
  app
    .document()
    .entity(id)
    .and_then(|entity| entity.kind.as_contour())
    .expect("the entity exists and is a contour")
}

#[test]
fn adding_a_rectangle_creates_a_closed_contour_with_the_requested_size() {
  let mut app = state_with_layer();
  let id = app.next_entity_id();
  app
    .execute(Command::AddRectangle {
      layer_id: "L1".to_owned(),
      entity_id: id.clone(),
      at: Point2::new(10.0, 20.0),
      width: 50.0,
      height: 25.0,
    })
    .expect("rectangle added");

  let contour = entity_contour(&app, &id);
  assert!(contour.is_closed());
  assert!(
    contour.is_ccw(),
    "a drawn rectangle must wind counter-clockwise"
  );
  assert_eq!(contour.signed_area(), 1250.0);
  let bounds = contour.bounds();
  assert_eq!((bounds.min.x, bounds.min.y), (10.0, 20.0));
  assert_eq!((bounds.max.x, bounds.max.y), (60.0, 45.0));
}

#[test]
fn adding_a_circle_keeps_its_arcs() {
  let mut app = state_with_layer();
  let id = app.next_entity_id();
  app
    .execute(Command::AddCircle {
      layer_id: "L1".to_owned(),
      entity_id: id.clone(),
      at: Point2::new(100.0, 100.0),
      radius: 20.0,
    })
    .expect("circle added");

  let contour = entity_contour(&app, &id);
  assert!(contour.has_arcs(), "a circle must stay arcs, not a polygon");
  assert_eq!(contour.segments().len(), 2);
  let area = contour.signed_area();
  assert!(
    (area - std::f64::consts::PI * 400.0).abs() < 1.0e-6,
    "area {area}"
  );
}

#[test]
fn deleting_an_entity_removes_it_from_its_layer() {
  let mut app = state_with_layer();
  let id = app.next_entity_id();
  app
    .execute(Command::AddRectangle {
      layer_id: "L1".to_owned(),
      entity_id: id.clone(),
      at: Point2::ORIGIN,
      width: 10.0,
      height: 10.0,
    })
    .expect("added");
  app
    .execute(Command::DeleteEntity {
      entity_id: id.clone(),
    })
    .expect("deleted");
  assert!(app.document().entity(&id).is_none());
  assert_eq!(app.document().layers.len(), 1, "the layer itself stays");
}

#[test]
fn a_layer_can_be_hidden_and_shown_again() {
  let mut app = state_with_layer();
  app
    .execute(Command::SetLayerVisible {
      layer_id: "L1".to_owned(),
      visible: false,
    })
    .expect("hidden");
  assert!(!app.document().layers[0].visible);
  app
    .execute(Command::SetLayerVisible {
      layer_id: "L1".to_owned(),
      visible: true,
    })
    .expect("shown");
  assert!(app.document().layers[0].visible);
}

#[test]
fn a_command_targeting_a_missing_layer_or_entity_is_refused() {
  let mut app = state_with_layer();
  let error = app
    .execute(Command::AddCircle {
      layer_id: "L-none".to_owned(),
      entity_id: "E1".to_owned(),
      at: Point2::ORIGIN,
      radius: 5.0,
    })
    .expect_err("must refuse");
  assert!(matches!(error, CommandError::UnknownLayer(_)), "{error:?}");

  let error = app
    .execute(Command::DeleteEntity {
      entity_id: "E-none".to_owned(),
    })
    .expect_err("must refuse");
  assert!(matches!(error, CommandError::UnknownEntity(_)), "{error:?}");
  assert!(app.document().entities().count() == 0);
}

#[test]
fn a_duplicate_entity_id_is_refused() {
  let mut app = state_with_layer();
  app
    .execute(Command::AddRectangle {
      layer_id: "L1".to_owned(),
      entity_id: "E1".to_owned(),
      at: Point2::ORIGIN,
      width: 10.0,
      height: 10.0,
    })
    .expect("added");
  let error = app
    .execute(Command::AddRectangle {
      layer_id: "L1".to_owned(),
      entity_id: "E1".to_owned(),
      at: Point2::new(50.0, 50.0),
      width: 10.0,
      height: 10.0,
    })
    .expect_err("must refuse");
  assert!(
    matches!(error, CommandError::DuplicateEntity(_)),
    "{error:?}"
  );
  assert_eq!(app.document().entities().count(), 1);
}

#[test]
fn a_non_positive_size_is_refused_before_geometry_is_built() {
  let mut app = state_with_layer();
  for (width, height) in [(0.0, 10.0), (10.0, -1.0), (f64::NAN, 10.0)] {
    let error = app
      .execute(Command::AddRectangle {
        layer_id: "L1".to_owned(),
        entity_id: "E1".to_owned(),
        at: Point2::ORIGIN,
        width,
        height,
      })
      .expect_err("must refuse");
    assert!(
      matches!(error, CommandError::NonPositiveSize { .. }),
      "{error:?}"
    );
  }
  let error = app
    .execute(Command::AddCircle {
      layer_id: "L1".to_owned(),
      entity_id: "E1".to_owned(),
      at: Point2::ORIGIN,
      radius: -3.0,
    })
    .expect_err("must refuse");
  assert!(
    matches!(error, CommandError::NonPositiveSize { .. }),
    "{error:?}"
  );
}

#[test]
fn drawing_is_undoable_and_redoable() {
  let mut app = state_with_layer();
  let id = app.next_entity_id();
  app
    .execute(Command::AddRectangle {
      layer_id: "L1".to_owned(),
      entity_id: id.clone(),
      at: Point2::ORIGIN,
      width: 40.0,
      height: 20.0,
    })
    .expect("added");

  assert!(app.undo());
  assert!(
    app.document().entity(&id).is_none(),
    "undo must remove the drawn entity"
  );
  assert!(app.redo());
  assert!(
    app.document().entity(&id).is_some(),
    "redo must bring it back"
  );
  let contour = entity_contour(&app, &id);
  assert_eq!(contour.signed_area(), 800.0);
}

#[test]
fn every_command_has_a_label_for_the_interface() {
  let commands = [
    Command::SetJob {
      width: 100.0,
      height: 100.0,
      thickness: 10.0,
    },
    Command::AddRectangle {
      layer_id: "L1".to_owned(),
      entity_id: "E1".to_owned(),
      at: Point2::ORIGIN,
      width: 1.0,
      height: 1.0,
    },
    Command::AddCircle {
      layer_id: "L1".to_owned(),
      entity_id: "E1".to_owned(),
      at: Point2::ORIGIN,
      radius: 1.0,
    },
    Command::DeleteEntity {
      entity_id: "E1".to_owned(),
    },
    Command::SetLayerVisible {
      layer_id: "L1".to_owned(),
      visible: true,
    },
    Command::AddLine {
      layer_id: "L1".to_owned(),
      entity_id: "E1".to_owned(),
      from: Point2::ORIGIN,
      to: Point2::new(1.0, 1.0),
    },
    Command::AddPolyline {
      layer_id: "L1".to_owned(),
      entity_id: "E1".to_owned(),
      points: vec![Point2::ORIGIN, Point2::new(1.0, 1.0)],
      closed: false,
    },
    Command::AddArc {
      layer_id: "L1".to_owned(),
      entity_id: "E1".to_owned(),
      start: Point2::ORIGIN,
      through: Point2::new(1.0, 1.0),
      end: Point2::new(2.0, 0.0),
    },
    Command::AddEllipse {
      layer_id: "L1".to_owned(),
      entity_id: "E1".to_owned(),
      at: Point2::ORIGIN,
      rx: 2.0,
      ry: 1.0,
      tolerance: 0.01,
    },
    Command::AddPolygon {
      layer_id: "L1".to_owned(),
      entity_id: "E1".to_owned(),
      at: Point2::ORIGIN,
      radius: 1.0,
      sides: 5,
      rotation: 0.0,
    },
    Command::AddStar {
      layer_id: "L1".to_owned(),
      entity_id: "E1".to_owned(),
      at: Point2::ORIGIN,
      outer_radius: 2.0,
      inner_radius: 1.0,
      points: 5,
      rotation: 0.0,
    },
    Command::TransformEntities {
      entity_ids: vec!["E1".to_owned()],
      transform: Transform::Translate {
        delta: Vec2::new(1.0, 1.0),
      },
    },
    Command::ArrayCopy {
      entity_ids: vec!["E1".to_owned()],
      rows: 2,
      columns: 3,
      dx: 10.0,
      dy: 20.0,
      first_entity_index: 2,
    },
    Command::AlignEntities {
      entity_ids: vec!["E1".to_owned()],
      mode: AlignMode::Left,
    },
  ];
  for command in commands {
    assert!(!command.label().is_empty(), "{command:?} needs a label");
  }
}

#[test]
fn drawn_entities_carry_the_contour_kind_the_pipeline_expects() {
  let mut app = state_with_layer();
  let id = app.next_entity_id();
  app
    .execute(Command::AddCircle {
      layer_id: "L1".to_owned(),
      entity_id: id.clone(),
      at: Point2::ORIGIN,
      radius: 15.0,
    })
    .expect("added");
  let entity = app.document().entity(&id).expect("exists");
  assert!(matches!(entity.kind, EntityKind::Contour { .. }));
  // The document model accepts it for a profile cut, which is the point of drawing it.
  app.document().validate().expect("document is valid");
}

// --- M2 draw commands (A12) -------------------------------------------------------------------

/// Slack for a bound derived from arcs: `Contour::bounds` samples at `cam_geom::DEFAULT_TOLERANCE`.
const ARC_BOUNDS_TOLERANCE: f64 = 1.0e-5;

#[test]
fn adding_a_line_stores_an_open_two_vertex_contour() {
  let mut app = state_with_layers(&["L1", "L2"]);
  let id = app.next_entity_id();
  app
    .execute(Command::AddLine {
      layer_id: "L2".to_owned(),
      entity_id: id.clone(),
      from: Point2::new(5.0, 5.0),
      to: Point2::new(25.0, 5.0),
    })
    .expect("line added");

  assert!(app.is_dirty(), "drawing marks the document dirty");
  assert_eq!(layer_holding(&app, &id), "L2");
  let contour = entity_contour(&app, &id);
  assert!(!contour.is_closed(), "a line is an open contour");
  assert_eq!(contour.len(), 2);
  assert!(!contour.has_arcs());
  assert_eq!(contour.perimeter(), 20.0);
  let bounds = contour.bounds();
  assert_eq!((bounds.min.x, bounds.min.y), (5.0, 5.0));
  assert_eq!((bounds.max.x, bounds.max.y), (25.0, 5.0));
}

#[test]
fn adding_a_polyline_stores_the_requested_points() {
  let mut app = state_with_layers(&["L1", "L2"]);
  let id = app.next_entity_id();
  let points = vec![
    Point2::new(0.0, 0.0),
    Point2::new(10.0, 0.0),
    Point2::new(10.0, 10.0),
  ];
  app
    .execute(Command::AddPolyline {
      layer_id: "L2".to_owned(),
      entity_id: id.clone(),
      points: points.clone(),
      closed: false,
    })
    .expect("polyline added");

  assert!(app.is_dirty());
  assert_eq!(layer_holding(&app, &id), "L2");
  let contour = entity_contour(&app, &id);
  assert!(!contour.is_closed());
  assert_eq!(contour.len(), 3);
  assert!(!contour.has_arcs(), "a polyline is straight segments only");
  assert_eq!(contour.perimeter(), 20.0);
  let stored: Vec<Point2> = contour
    .vertices()
    .iter()
    .map(|vertex| vertex.point)
    .collect();
  assert_eq!(stored, points);
  let bounds = contour.bounds();
  assert_eq!((bounds.min.x, bounds.min.y), (0.0, 0.0));
  assert_eq!((bounds.max.x, bounds.max.y), (10.0, 10.0));
}

#[test]
fn adding_an_arc_keeps_a_single_arc() {
  let mut app = state_with_layers(&["L1", "L2"]);
  let id = app.next_entity_id();
  app
    .execute(Command::AddArc {
      layer_id: "L2".to_owned(),
      entity_id: id.clone(),
      start: Point2::new(0.0, 0.0),
      through: Point2::new(10.0, 10.0),
      end: Point2::new(20.0, 0.0),
    })
    .expect("arc added");

  assert!(app.is_dirty());
  assert_eq!(layer_holding(&app, &id), "L2");
  let contour = entity_contour(&app, &id);
  assert!(!contour.is_closed());
  assert_eq!(
    contour.len(),
    2,
    "one arc is two vertices, not a tessellation"
  );
  assert!(contour.has_arcs(), "an arc must stay an arc");
  // Through (0,0), (10,10) and (20,0) is the half turn of the circle at (10,0) with radius 10.
  let perimeter = contour.perimeter();
  assert!(
    (perimeter - std::f64::consts::PI * 10.0).abs() < 1.0e-9,
    "perimeter {perimeter}"
  );
  let bounds = contour.bounds();
  // `Contour::bounds` samples arcs at `cam_geom::DEFAULT_TOLERANCE`, so it can miss an extreme by
  // up to that much; the analytic extremes here are (0,0) and (20,10).
  assert!(
    bounds.min.x.abs() < ARC_BOUNDS_TOLERANCE,
    "{}",
    bounds.min.x
  );
  assert!(
    (bounds.max.x - 20.0).abs() < ARC_BOUNDS_TOLERANCE,
    "{}",
    bounds.max.x
  );
  assert!(
    bounds.min.y.abs() < ARC_BOUNDS_TOLERANCE,
    "{}",
    bounds.min.y
  );
  assert!(
    (bounds.max.y - 10.0).abs() < ARC_BOUNDS_TOLERANCE,
    "{}",
    bounds.max.y
  );
}

#[test]
fn adding_a_polygon_stores_one_straight_segment_per_side() {
  let mut app = state_with_layers(&["L1", "L2"]);
  let id = app.next_entity_id();
  app
    .execute(Command::AddPolygon {
      layer_id: "L2".to_owned(),
      entity_id: id.clone(),
      at: Point2::ORIGIN,
      radius: 10.0,
      sides: 6,
      rotation: 0.0,
    })
    .expect("polygon added");

  assert!(app.is_dirty());
  assert_eq!(layer_holding(&app, &id), "L2");
  let contour = entity_contour(&app, &id);
  assert!(contour.is_closed());
  assert!(contour.is_ccw());
  assert_eq!(contour.len(), 6);
  assert!(!contour.has_arcs(), "a polygon is straight segments");
  let expected = 6.0 / 2.0 * 100.0 * (std::f64::consts::TAU / 6.0).sin();
  assert!(
    (contour.signed_area() - expected).abs() < 1.0e-9,
    "area {}, expected {expected}",
    contour.signed_area()
  );
  let bounds = contour.bounds();
  assert!((bounds.min.x + 10.0).abs() < 1.0e-9, "{}", bounds.min.x);
  assert!((bounds.max.x - 10.0).abs() < 1.0e-9, "{}", bounds.max.x);
}

#[test]
fn adding_a_star_stores_two_vertices_per_point() {
  let mut app = state_with_layers(&["L1", "L2"]);
  let id = app.next_entity_id();
  app
    .execute(Command::AddStar {
      layer_id: "L2".to_owned(),
      entity_id: id.clone(),
      at: Point2::ORIGIN,
      outer_radius: 20.0,
      inner_radius: 8.0,
      points: 5,
      rotation: 0.0,
    })
    .expect("star added");

  assert!(app.is_dirty());
  assert_eq!(layer_holding(&app, &id), "L2");
  let contour = entity_contour(&app, &id);
  assert!(contour.is_closed());
  assert!(contour.is_ccw());
  assert_eq!(contour.len(), 10, "five points alternate ten vertices");
  assert!(!contour.has_arcs());
  // Alternating radii: area = points * outer * inner * sin(PI / points).
  let expected = 5.0 * 20.0 * 8.0 * (std::f64::consts::PI / 5.0).sin();
  assert!(
    (contour.signed_area() - expected).abs() < 1.0e-9,
    "area {}, expected {expected}",
    contour.signed_area()
  );
  let bounds = contour.bounds();
  assert!((bounds.max.x - 20.0).abs() < 1.0e-9, "{}", bounds.max.x);
}

#[test]
fn adding_an_ellipse_reports_the_radial_error_the_kernel_computed() {
  let mut app = state_with_layers(&["L1", "L2"]);
  let id = app.next_entity_id();
  let outcome = app
    .execute_with_outcome(Command::AddEllipse {
      layer_id: "L2".to_owned(),
      entity_id: id.clone(),
      at: Point2::ORIGIN,
      rx: 40.0,
      ry: 10.0,
      tolerance: 0.01,
    })
    .expect("ellipse added");

  assert!(app.is_dirty());
  assert_eq!(layer_holding(&app, &id), "L2");

  // The caller is told the error the kernel actually achieved, not a promise about the tolerance.
  let kernel = primitive::ellipse(Point2::ORIGIN, 40.0, 10.0, 0.01).expect("kernel ellipse");
  assert_eq!(outcome.max_radial_error, Some(kernel.max_radial_error));
  assert!(
    outcome.max_radial_error.expect("reported") <= 0.01,
    "the reported error must meet the requested tolerance"
  );

  let contour = entity_contour(&app, &id);
  assert_eq!(
    contour, &kernel.contour,
    "cam-app must store the kernel contour"
  );
  assert!(contour.is_closed());
  assert!(contour.is_ccw());
  assert!(contour.has_arcs(), "an ellipse is approximated by arcs");
  let bounds = contour.bounds();
  assert!(
    (bounds.min.x + 40.0).abs() < ARC_BOUNDS_TOLERANCE,
    "{}",
    bounds.min.x
  );
  assert!(
    (bounds.max.x - 40.0).abs() < ARC_BOUNDS_TOLERANCE,
    "{}",
    bounds.max.x
  );
  assert!(
    (bounds.min.y + 10.0).abs() < ARC_BOUNDS_TOLERANCE,
    "{}",
    bounds.min.y
  );
  assert!(
    (bounds.max.y - 10.0).abs() < ARC_BOUNDS_TOLERANCE,
    "{}",
    bounds.max.y
  );
}

#[test]
fn a_line_with_coincident_endpoints_is_refused_atomically() {
  let mut app = state_with_layer();
  prime_with_line(&mut app);
  let error = assert_refused_atomically(
    &mut app,
    Command::AddLine {
      layer_id: "L1".to_owned(),
      entity_id: "E9".to_owned(),
      from: Point2::new(4.0, 4.0),
      to: Point2::new(4.0, 4.0),
    },
  );
  assert!(matches!(error, CommandError::Geometry(_)), "{error:?}");
}

#[test]
fn a_polyline_with_a_single_point_is_refused_atomically() {
  let mut app = state_with_layer();
  prime_with_polyline(&mut app);
  let error = assert_refused_atomically(
    &mut app,
    Command::AddPolyline {
      layer_id: "L1".to_owned(),
      entity_id: "E9".to_owned(),
      points: vec![Point2::new(1.0, 1.0)],
      closed: false,
    },
  );
  assert!(matches!(error, CommandError::Geometry(_)), "{error:?}");
}

#[test]
fn an_arc_through_collinear_points_is_refused_atomically() {
  let mut app = state_with_layer();
  prime_with_arc(&mut app);
  let error = assert_refused_atomically(
    &mut app,
    Command::AddArc {
      layer_id: "L1".to_owned(),
      entity_id: "E9".to_owned(),
      start: Point2::new(0.0, 0.0),
      through: Point2::new(10.0, 0.0),
      end: Point2::new(20.0, 0.0),
    },
  );
  assert!(matches!(error, CommandError::Geometry(_)), "{error:?}");
}

#[test]
fn a_polygon_with_fewer_than_three_sides_is_refused_atomically() {
  let mut app = state_with_layer();
  prime_with_polygon(&mut app);
  let error = assert_refused_atomically(
    &mut app,
    Command::AddPolygon {
      layer_id: "L1".to_owned(),
      entity_id: "E9".to_owned(),
      at: Point2::ORIGIN,
      radius: 10.0,
      sides: 2,
      rotation: 0.0,
    },
  );
  assert!(
    matches!(
      error,
      CommandError::TooFew {
        kind: "polygon sides",
        minimum: 3,
        value: 2
      }
    ),
    "{error:?}"
  );
}

#[test]
fn a_star_with_a_bad_point_count_or_inner_radius_is_refused_atomically() {
  let mut app = state_with_layer();
  prime_with_star(&mut app);
  let error = assert_refused_atomically(
    &mut app,
    Command::AddStar {
      layer_id: "L1".to_owned(),
      entity_id: "E9".to_owned(),
      at: Point2::ORIGIN,
      outer_radius: 20.0,
      inner_radius: 8.0,
      points: 2,
      rotation: 0.0,
    },
  );
  assert!(
    matches!(
      error,
      CommandError::TooFew {
        kind: "star points",
        minimum: 3,
        value: 2
      }
    ),
    "{error:?}"
  );

  let error = assert_refused_atomically(
    &mut app,
    Command::AddStar {
      layer_id: "L1".to_owned(),
      entity_id: "E9".to_owned(),
      at: Point2::ORIGIN,
      outer_radius: 20.0,
      inner_radius: 20.0,
      points: 5,
      rotation: 0.0,
    },
  );
  assert!(
    matches!(error, CommandError::OutOfRange { value, .. } if value == 20.0),
    "{error:?}"
  );
}

#[test]
fn an_ellipse_with_a_bad_radius_or_tolerance_is_refused_atomically() {
  let mut app = state_with_layer();
  prime_with_ellipse(&mut app);
  for (rx, ry, tolerance) in [
    (0.0, 10.0, 0.01),
    (40.0, -1.0, 0.01),
    (40.0, 10.0, 0.0),
    (f64::NAN, 10.0, 0.01),
    (40.0, 10.0, f64::INFINITY),
  ] {
    let error = assert_refused_atomically(
      &mut app,
      Command::AddEllipse {
        layer_id: "L1".to_owned(),
        entity_id: "E9".to_owned(),
        at: Point2::ORIGIN,
        rx,
        ry,
        tolerance,
      },
    );
    assert!(
      matches!(error, CommandError::NonPositiveSize { .. }),
      "{error:?}"
    );
  }
}

// --- one undo per new command restores the document byte for byte (A12) ------------------------

fn assert_single_undo_restores(app: &mut AppState, command: Command) {
  let before = document_text(app);
  let depth = app.history_depth();
  app.execute(command).expect("the command applies");
  assert_eq!(app.history_depth(), depth + 1);
  assert!(app.undo(), "one undo must be available");
  assert_eq!(
    before,
    document_text(app),
    "one undo must restore the document byte for byte"
  );
  assert_eq!(app.history_depth(), depth);
}

#[test]
fn undoing_an_added_line_restores_the_document_exactly() {
  let mut app = state_with_layer();
  let id = app.next_entity_id();
  assert_single_undo_restores(
    &mut app,
    Command::AddLine {
      layer_id: "L1".to_owned(),
      entity_id: id,
      from: Point2::ORIGIN,
      to: Point2::new(10.0, 0.0),
    },
  );
}

#[test]
fn undoing_an_added_polyline_restores_the_document_exactly() {
  let mut app = state_with_layer();
  let id = app.next_entity_id();
  assert_single_undo_restores(
    &mut app,
    Command::AddPolyline {
      layer_id: "L1".to_owned(),
      entity_id: id,
      points: vec![
        Point2::new(0.0, 0.0),
        Point2::new(10.0, 0.0),
        Point2::new(10.0, 10.0),
      ],
      closed: true,
    },
  );
}

#[test]
fn undoing_an_added_arc_restores_the_document_exactly() {
  let mut app = state_with_layer();
  let id = app.next_entity_id();
  assert_single_undo_restores(
    &mut app,
    Command::AddArc {
      layer_id: "L1".to_owned(),
      entity_id: id,
      start: Point2::new(0.0, 0.0),
      through: Point2::new(10.0, 10.0),
      end: Point2::new(20.0, 0.0),
    },
  );
}

#[test]
fn undoing_an_added_ellipse_restores_the_document_exactly() {
  let mut app = state_with_layer();
  let id = app.next_entity_id();
  assert_single_undo_restores(
    &mut app,
    Command::AddEllipse {
      layer_id: "L1".to_owned(),
      entity_id: id,
      at: Point2::new(10.0, 10.0),
      rx: 40.0,
      ry: 10.0,
      tolerance: 0.01,
    },
  );
}

#[test]
fn undoing_an_added_polygon_restores_the_document_exactly() {
  let mut app = state_with_layer();
  let id = app.next_entity_id();
  assert_single_undo_restores(
    &mut app,
    Command::AddPolygon {
      layer_id: "L1".to_owned(),
      entity_id: id,
      at: Point2::ORIGIN,
      radius: 10.0,
      sides: 6,
      rotation: 0.3,
    },
  );
}

#[test]
fn undoing_an_added_star_restores_the_document_exactly() {
  let mut app = state_with_layer();
  let id = app.next_entity_id();
  assert_single_undo_restores(
    &mut app,
    Command::AddStar {
      layer_id: "L1".to_owned(),
      entity_id: id,
      at: Point2::ORIGIN,
      outer_radius: 20.0,
      inner_radius: 8.0,
      points: 5,
      rotation: 0.0,
    },
  );
}

// --- M2 transform, array copy and align commands (A11, A12) -----------------------------------

/// A state whose single layer already holds the given entities, one 10×10 rectangle each.
///
/// Built directly rather than through commands so the ids can have a gap, which `next_entity_id`
/// never produces; a gap is what lets a test place a collision in the middle of a minted id run.
fn state_with_entities(ids: &[&str]) -> AppState {
  let mut document = Document::new(Job::default_sheet());
  let entities = ids
    .iter()
    .enumerate()
    .map(|(n, id)| Entity {
      id: (*id).to_owned(),
      kind: EntityKind::Contour {
        contour: primitive::rectangle(Point2::new(n as f64 * 20.0, 0.0), 10.0, 10.0)
          .expect("the test fixture rectangle is valid"),
      },
    })
    .collect();
  document.layers.push(Layer {
    id: "L1".to_owned(),
    role: LayerRole::Outline, /*was Geometry*/
    side: cam_model::Side::Top,
    color: "#000000".to_owned(),
    visible: true,
    source: cam_model::Provenance::default(),
    entities,
  });
  AppState::new(document)
}

fn draw_rectangle_on(
  app: &mut AppState,
  layer_id: &str,
  at: Point2,
  width: f64,
  height: f64,
) -> String {
  let id = app.next_entity_id();
  app
    .execute(Command::AddRectangle {
      layer_id: layer_id.to_owned(),
      entity_id: id.clone(),
      at,
      width,
      height,
    })
    .expect("the rectangle is drawn");
  id
}

fn draw_rectangle(app: &mut AppState, at: Point2, width: f64, height: f64) -> String {
  draw_rectangle_on(app, "L1", at, width, height)
}

fn draw_circle(app: &mut AppState, at: Point2, radius: f64) -> String {
  let id = app.next_entity_id();
  app
    .execute(Command::AddCircle {
      layer_id: "L1".to_owned(),
      entity_id: id.clone(),
      at,
      radius,
    })
    .expect("the circle is drawn");
  id
}

fn array_copy(
  entity_ids: Vec<String>,
  rows: usize,
  columns: usize,
  dx: f64,
  dy: f64,
  first_entity_index: u32,
) -> Command {
  Command::ArrayCopy {
    entity_ids,
    rows,
    columns,
    dx,
    dy,
    first_entity_index,
  }
}

fn assert_bounds(contour: &cam_geom::Contour, min: (f64, f64), max: (f64, f64), slack: f64) {
  let bounds = contour.bounds();
  assert!(
    (bounds.min.x - min.0).abs() < slack,
    "min.x {} != {}",
    bounds.min.x,
    min.0
  );
  assert!(
    (bounds.min.y - min.1).abs() < slack,
    "min.y {} != {}",
    bounds.min.y,
    min.1
  );
  assert!(
    (bounds.max.x - max.0).abs() < slack,
    "max.x {} != {}",
    bounds.max.x,
    max.0
  );
  assert!(
    (bounds.max.y - max.1).abs() < slack,
    "max.y {} != {}",
    bounds.max.y,
    max.1
  );
}

#[test]
fn translating_a_circle_moves_it_and_keeps_its_arcs() {
  let mut app = state_with_layer();
  let id = draw_circle(&mut app, Point2::new(10.0, 20.0), 5.0);
  let before = entity_contour(&app, &id).clone();
  let delta = Vec2::new(100.0, -50.0);

  let outcome = app
    .execute_with_outcome(Command::TransformEntities {
      entity_ids: vec![id.clone()],
      transform: Transform::Translate { delta },
    })
    .expect("translated");

  assert!(app.is_dirty(), "an edit marks the document dirty");
  assert_eq!(outcome.lost_arcs, 0, "a translation is exact");
  assert!(
    outcome.created_entities.is_empty(),
    "a transform creates nothing"
  );

  let contour = entity_contour(&app, &id);
  assert_eq!(contour.len(), 2, "a translated circle is still two arcs");
  assert!(contour.is_closed());
  assert!(contour.has_arcs(), "a translation never tessellates");
  assert!(contour.is_ccw());
  assert_bounds(
    contour,
    (105.0, -35.0),
    (115.0, -25.0),
    ARC_BOUNDS_TOLERANCE,
  );
  let area = contour.signed_area();
  assert!(
    (area - std::f64::consts::PI * 25.0).abs() < 1.0e-9,
    "a translation keeps the area, got {area}"
  );
  assert_eq!(
    contour,
    &transform::translate(&before, delta).expect("kernel translate"),
    "cam-app must store the contour the kernel returned"
  );
}

#[test]
fn rotating_a_rectangle_turns_it_about_the_pivot() {
  let mut app = state_with_layer();
  // 10..14 by 0..2, turned a quarter turn about the origin.
  let id = draw_rectangle(&mut app, Point2::new(10.0, 0.0), 4.0, 2.0);
  let before = entity_contour(&app, &id).clone();
  let radians = std::f64::consts::FRAC_PI_2;
  let about = Point2::ORIGIN;

  let outcome = app
    .execute_with_outcome(Command::TransformEntities {
      entity_ids: vec![id.clone()],
      transform: Transform::Rotate { radians, about },
    })
    .expect("rotated");

  assert!(app.is_dirty());
  assert_eq!(outcome.lost_arcs, 0, "a rotation is exact");
  let contour = entity_contour(&app, &id);
  assert_eq!(
    contour.len(),
    4,
    "a rotated rectangle is still four corners"
  );
  assert!(contour.is_closed());
  assert!(!contour.has_arcs(), "a rectangle has no arcs to lose");
  assert!(contour.is_ccw());
  // (10,0) -> (0,10), (14,2) -> (-2,14): the rectangle now stands at -2..0 by 10..14.
  assert_bounds(contour, (-2.0, 10.0), (0.0, 14.0), 1.0e-9);
  let area = contour.signed_area();
  assert!((area - 8.0).abs() < 1.0e-9, "area {area}");
  assert_eq!(
    contour,
    &transform::rotate(&before, radians, about).expect("kernel rotate")
  );
}

#[test]
fn a_uniform_scale_grows_a_circle_and_keeps_its_arcs() {
  let mut app = state_with_layer();
  let id = draw_circle(&mut app, Point2::new(10.0, 10.0), 5.0);
  let before = entity_contour(&app, &id).clone();
  let tolerance = 0.01;

  let outcome = app
    .execute_with_outcome(Command::TransformEntities {
      entity_ids: vec![id.clone()],
      transform: Transform::Scale {
        sx: 2.0,
        sy: 2.0,
        about: Point2::ORIGIN,
        tolerance,
      },
    })
    .expect("scaled");

  assert!(app.is_dirty());
  assert_eq!(outcome.lost_arcs, 0, "a uniform scale is exact");
  let contour = entity_contour(&app, &id);
  assert_eq!(contour.len(), 2, "a scaled circle is still two arcs");
  assert!(contour.is_closed());
  assert!(contour.has_arcs(), "a uniform scale never tessellates");
  assert_bounds(contour, (10.0, 10.0), (30.0, 30.0), ARC_BOUNDS_TOLERANCE);
  let area = contour.signed_area();
  assert!(
    (area - std::f64::consts::PI * 100.0).abs() < 1.0e-9,
    "the area must quadruple, got {area}"
  );
  assert_eq!(
    contour,
    &transform::scale(&before, 2.0, 2.0, Point2::ORIGIN, tolerance)
      .expect("kernel scale")
      .contour
  );
}

#[test]
fn mirroring_a_circle_reflects_it_and_keeps_its_arcs() {
  let mut app = state_with_layer();
  let id = draw_circle(&mut app, Point2::new(10.0, 10.0), 5.0);
  let before = entity_contour(&app, &id).clone();
  let point = Point2::ORIGIN;
  let direction = Vec2::Y_AXIS;

  let outcome = app
    .execute_with_outcome(Command::TransformEntities {
      entity_ids: vec![id.clone()],
      transform: Transform::Mirror { point, direction },
    })
    .expect("mirrored");

  assert!(app.is_dirty());
  assert_eq!(outcome.lost_arcs, 0, "a mirror is exact");
  let contour = entity_contour(&app, &id);
  assert_eq!(contour.len(), 2, "a mirrored circle is still two arcs");
  assert!(contour.is_closed());
  assert!(contour.has_arcs(), "a mirror never tessellates");
  assert!(contour.is_ccw(), "winding is normalised after the mirror");
  // The centre lands on the reflected side of x = 0 with its radius untouched.
  assert_bounds(contour, (-15.0, 5.0), (-5.0, 15.0), ARC_BOUNDS_TOLERANCE);
  let area = contour.signed_area();
  assert!(
    (area - std::f64::consts::PI * 25.0).abs() < 1.0e-9,
    "a mirror keeps the area, got {area}"
  );
  assert_eq!(
    contour,
    &transform::mirror(&before, point, direction).expect("kernel mirror")
  );
}

#[test]
fn a_non_uniform_scale_over_a_circle_reports_the_lost_arcs() {
  let mut app = state_with_layer();
  let id = draw_circle(&mut app, Point2::ORIGIN, 10.0);
  let before = entity_contour(&app, &id).clone();
  let tolerance = 0.01;

  let outcome = app
    .execute_with_outcome(Command::TransformEntities {
      entity_ids: vec![id.clone()],
      transform: Transform::Scale {
        sx: 2.0,
        sy: 1.0,
        about: Point2::ORIGIN,
        tolerance,
      },
    })
    .expect("scaled");

  assert!(
    outcome.lost_arcs >= 1,
    "the caller must learn the arcs are gone, got {}",
    outcome.lost_arcs
  );
  let contour = entity_contour(&app, &id);
  assert!(
    !contour.has_arcs(),
    "a non-uniform scale cannot keep a circle's arcs"
  );
  assert!(contour.is_closed());
  assert!(contour.is_ccw());
  // An ellipse 2r wide and r tall, tessellated within the declared tolerance.
  assert_bounds(contour, (-20.0, -10.0), (20.0, 10.0), tolerance);
  assert_eq!(
    contour,
    &transform::scale(&before, 2.0, 1.0, Point2::ORIGIN, tolerance)
      .expect("kernel scale")
      .contour,
    "cam-app must store exactly the tessellated contour the kernel returned"
  );
}

#[test]
fn the_lost_arc_count_is_aggregated_over_the_selection() {
  let mut app = state_with_layer();
  let circle = draw_circle(&mut app, Point2::ORIGIN, 10.0);
  let rectangle = draw_rectangle(&mut app, Point2::new(50.0, 50.0), 10.0, 10.0);
  let before_circle = entity_contour(&app, &circle).clone();
  let before_rectangle = entity_contour(&app, &rectangle).clone();
  let tolerance = 0.01;

  let outcome = app
    .execute_with_outcome(Command::TransformEntities {
      entity_ids: vec![circle.clone(), rectangle.clone()],
      transform: Transform::Scale {
        sx: 2.0,
        sy: 1.0,
        about: Point2::ORIGIN,
        tolerance,
      },
    })
    .expect("scaled");

  assert_eq!(
    outcome.lost_arcs, 2,
    "a circle has two arcs and a rectangle none, so the outcome sums to two"
  );
  let stored_rectangle = entity_contour(&app, &rectangle);
  assert!(
    !stored_rectangle.has_arcs(),
    "an arc-free contour stays arc free"
  );
  assert_eq!(
    stored_rectangle.len(),
    4,
    "an arc-free contour stays exact: four corners, no tessellation"
  );
  assert_eq!(
    stored_rectangle,
    &transform::scale(&before_rectangle, 2.0, 1.0, Point2::ORIGIN, tolerance)
      .expect("kernel scale")
      .contour
  );
  assert_eq!(
    entity_contour(&app, &circle),
    &transform::scale(&before_circle, 2.0, 1.0, Point2::ORIGIN, tolerance)
      .expect("kernel scale")
      .contour
  );
}

#[test]
fn a_scale_with_a_zero_or_undefined_factor_is_refused_atomically() {
  let mut app = state_with_layer();
  let id = draw_circle(&mut app, Point2::ORIGIN, 10.0);
  for (sx, sy) in [
    (0.0, 1.0),
    (1.0, 0.0),
    (f64::NAN, 1.0),
    (1.0, f64::INFINITY),
  ] {
    let error = assert_refused_atomically(
      &mut app,
      Command::TransformEntities {
        entity_ids: vec![id.clone()],
        transform: Transform::Scale {
          sx,
          sy,
          about: Point2::ORIGIN,
          tolerance: 0.01,
        },
      },
    );
    assert!(
      matches!(error, CommandError::OutOfRange { .. }),
      "{sx} x {sy} must be refused: {error:?}"
    );
  }
  assert_bounds(
    entity_contour(&app, &id),
    (-10.0, -10.0),
    (10.0, 10.0),
    ARC_BOUNDS_TOLERANCE,
  );
}

#[test]
fn a_transform_naming_an_unknown_entity_is_refused_atomically() {
  let mut app = state_with_layer();
  let id = draw_circle(&mut app, Point2::ORIGIN, 10.0);

  let error = assert_refused_atomically(
    &mut app,
    Command::TransformEntities {
      entity_ids: vec![id.clone(), "E-none".to_owned()],
      transform: Transform::Translate {
        delta: Vec2::new(1.0, 1.0),
      },
    },
  );
  assert!(matches!(error, CommandError::UnknownEntity(_)), "{error:?}");
  assert_bounds(
    entity_contour(&app, &id),
    (-10.0, -10.0),
    (10.0, 10.0),
    ARC_BOUNDS_TOLERANCE,
  );
}

#[test]
fn a_transform_over_an_empty_selection_is_refused_atomically() {
  let mut app = state_with_layer();
  prime_with_line(&mut app);
  for transform in [
    Transform::Translate {
      delta: Vec2::new(1.0, 1.0),
    },
    Transform::Rotate {
      radians: 0.5,
      about: Point2::ORIGIN,
    },
    Transform::Scale {
      sx: 2.0,
      sy: 2.0,
      about: Point2::ORIGIN,
      tolerance: 0.01,
    },
    Transform::Mirror {
      point: Point2::ORIGIN,
      direction: Vec2::Y_AXIS,
    },
  ] {
    let error = assert_refused_atomically(
      &mut app,
      Command::TransformEntities {
        entity_ids: Vec::new(),
        transform: transform.clone(),
      },
    );
    assert!(
      matches!(error, CommandError::EmptySelection),
      "{transform:?} must be refused: {error:?}"
    );
  }
}

#[test]
fn a_two_by_three_array_copies_the_selection_in_row_major_order() {
  let mut app = state_with_layers(&["L1", "L2"]);
  let wide = draw_rectangle_on(&mut app, "L1", Point2::new(0.0, 0.0), 10.0, 10.0);
  let small = draw_rectangle_on(&mut app, "L2", Point2::new(20.0, 0.0), 5.0, 5.0);
  assert_eq!((wide.as_str(), small.as_str()), ("E1", "E2"));

  let outcome = app
    .execute_with_outcome(array_copy(
      vec![wide.clone(), small.clone()],
      2,
      3,
      100.0,
      200.0,
      3,
    ))
    .expect("array copied");

  assert!(app.is_dirty());
  let expected: Vec<String> = (3..=14).map(|n| format!("E{n}")).collect();
  assert_eq!(
    outcome.created_entities, expected,
    "ids run E3.. in row-major order: rows outer, columns inner, selection order inside a cell"
  );
  assert_eq!(
    app.document().entities().count(),
    14,
    "six cells of two entities, and the two originals stay put"
  );
  assert_eq!(outcome.lost_arcs, 0, "an array copy translates exactly");

  let cells = [
    (0.0, 0.0),
    (100.0, 0.0),
    (200.0, 0.0),
    (0.0, 200.0),
    (100.0, 200.0),
    (200.0, 200.0),
  ];
  let mut index = 0;
  for (x, y) in cells {
    let wide_copy = &expected[index];
    assert_bounds(
      entity_contour(&app, wide_copy),
      (x, y),
      (x + 10.0, y + 10.0),
      1.0e-9,
    );
    assert_eq!(
      layer_holding(&app, wide_copy),
      "L1",
      "a copy stays on its source's layer"
    );
    index += 1;
    let small_copy = &expected[index];
    assert_bounds(
      entity_contour(&app, small_copy),
      (x + 20.0, y),
      (x + 25.0, y + 5.0),
      1.0e-9,
    );
    assert_eq!(layer_holding(&app, small_copy), "L2");
    index += 1;
  }

  // Every copy is a full duplicate of its source, not a running accumulation.
  let wide_source = primitive::rectangle(Point2::new(0.0, 0.0), 10.0, 10.0).expect("rectangle");
  assert_eq!(
    entity_contour(&app, &expected[2]),
    &transform::translate(&wide_source, Vec2::new(100.0, 0.0)).expect("translate"),
    "cell (0,1) is the source moved by one X step"
  );
  assert_bounds(
    entity_contour(&app, &wide),
    (0.0, 0.0),
    (10.0, 10.0),
    1.0e-9,
  );
  assert_bounds(
    entity_contour(&app, &small),
    (20.0, 0.0),
    (25.0, 5.0),
    1.0e-9,
  );
}

#[test]
fn an_array_with_a_zero_count_or_non_finite_spacing_is_refused_atomically() {
  let mut app = state_with_entities(&["E1", "E2"]);
  let selection = vec!["E1".to_owned(), "E2".to_owned()];

  let error =
    assert_refused_atomically(&mut app, array_copy(selection.clone(), 0, 3, 10.0, 10.0, 3));
  assert!(
    matches!(
      error,
      CommandError::TooFew {
        kind: "array rows",
        minimum: 1,
        value: 0
      }
    ),
    "{error:?}"
  );

  let error =
    assert_refused_atomically(&mut app, array_copy(selection.clone(), 2, 0, 10.0, 10.0, 3));
  assert!(
    matches!(
      error,
      CommandError::TooFew {
        kind: "array columns",
        minimum: 1,
        value: 0
      }
    ),
    "{error:?}"
  );

  let error = assert_refused_atomically(
    &mut app,
    array_copy(selection.clone(), 2, 3, f64::NAN, 10.0, 3),
  );
  assert!(
    matches!(error, CommandError::OutOfRange { .. }),
    "{error:?}"
  );

  let error = assert_refused_atomically(
    &mut app,
    array_copy(selection.clone(), 2, 3, 10.0, f64::INFINITY, 3),
  );
  assert!(
    matches!(error, CommandError::OutOfRange { .. }),
    "{error:?}"
  );

  let error = assert_refused_atomically(&mut app, array_copy(Vec::new(), 2, 3, 10.0, 10.0, 3));
  assert!(matches!(error, CommandError::EmptySelection), "{error:?}");
}

#[test]
fn a_zero_index_is_taken_literally_by_array_copy() {
  // An id is an opaque string: what has to hold is that every minted id is free, not that the
  // counter would have produced it.
  let mut app = state_with_layers(&["L1"]);
  let outcome = app.execute_with_outcome(array_copy(vec!["E1".to_owned()], 1, 2, 5.0, 0.0, 0));
  // E1 does not exist yet, so the selection is refused for a different reason: prime it first.
  assert!(
    outcome.is_err(),
    "an unknown selection is refused: {outcome:?}"
  );

  let mut app = state_with_layers(&["L1"]);
  app
    .execute(Command::AddRectangle {
      layer_id: "L1".to_owned(),
      entity_id: "E1".to_owned(),
      at: Point2::new(0.0, 0.0),
      width: 1.0,
      height: 1.0,
    })
    .expect("the source rectangle is drawn");

  let outcome = app
    .execute_with_outcome(array_copy(vec!["E1".to_owned()], 1, 1, 5.0, 0.0, 0))
    .expect("a zero index is minted literally");
  assert_eq!(
    outcome.created_entities,
    vec!["E0".to_owned()],
    "the index is taken literally, so the first minted id is E0"
  );
  assert!(app.document().entity("E0").is_some(), "E0 was stored");
}

#[test]
fn an_array_colliding_late_in_the_grid_is_refused_without_storing_anything() {
  let mut app = state_with_entities(&["E1", "E2", "E9"]);

  // A 2x3 grid over two entities starting at 3 mints E3..E14, so the seventh minted id - E9 -
  // collides with an entity that already exists. Nothing may be stored before that check.
  let error = assert_refused_atomically(
    &mut app,
    array_copy(
      vec!["E1".to_owned(), "E2".to_owned()],
      2,
      3,
      100.0,
      200.0,
      3,
    ),
  );
  assert!(
    matches!(error, CommandError::DuplicateEntity(_)),
    "{error:?}"
  );
  assert_eq!(
    app.document().entities().count(),
    3,
    "not one copy may be stored"
  );
  for id in ["E3", "E4", "E5", "E6", "E7", "E8"] {
    assert!(
      app.document().entity(id).is_none(),
      "{id} must not exist after a refused grid"
    );
  }
}

/// The bounding coordinate an align mode is supposed to line up.
fn coordinate(contour: &cam_geom::Contour, axis: &str) -> f64 {
  let bounds = contour.bounds();
  match axis {
    "min.x" => bounds.min.x,
    "max.x" => bounds.max.x,
    "min.y" => bounds.min.y,
    "max.y" => bounds.max.y,
    "centre.x" => bounds.center().x,
    "centre.y" => bounds.center().y,
    other => panic!("unknown axis {other}"),
  }
}

#[test]
fn every_align_mode_moves_the_selection_to_its_combined_edge_or_centre() {
  // A occupies 10..20 x 10..20 and B occupies 40..60 x 50..60, so the selection's own combined
  // bounds are 10..60 x 10..60 with centre (35, 35). The job is 2438 mm wide, so an alignment
  // that used the job instead of the selection could not land on any of these numbers.
  let cases = [
    (AlignMode::Left, "min.x", 10.0),
    (AlignMode::Right, "max.x", 60.0),
    (AlignMode::Bottom, "min.y", 10.0),
    (AlignMode::Top, "max.y", 60.0),
    (AlignMode::HorizontalCentre, "centre.x", 35.0),
    (AlignMode::VerticalCentre, "centre.y", 35.0),
  ];

  for (mode, axis, expected) in cases {
    let mut app = state_with_layer();
    let a = draw_rectangle(&mut app, Point2::new(10.0, 10.0), 10.0, 10.0);
    let b = draw_rectangle(&mut app, Point2::new(40.0, 50.0), 20.0, 10.0);

    let outcome = app
      .execute_with_outcome(Command::AlignEntities {
        entity_ids: vec![a.clone(), b.clone()],
        mode,
      })
      .expect("aligned");

    assert!(app.is_dirty(), "{mode:?} marks the document dirty");
    assert_eq!(outcome.lost_arcs, 0, "{mode:?} only translates");
    assert_eq!(
      coordinate(entity_contour(&app, &a), axis),
      expected,
      "{mode:?} must put A on {axis} = {expected}"
    );
    assert_eq!(
      coordinate(entity_contour(&app, &b), axis),
      expected,
      "{mode:?} must put B on {axis} = {expected}"
    );

    // Only the aligned axis moves; a rectangle's bounds are exact, so this is an exact assertion.
    let a_bounds = entity_contour(&app, &a).bounds();
    let b_bounds = entity_contour(&app, &b).bounds();
    if axis.ends_with('x') {
      assert_eq!(
        (a_bounds.min.y, a_bounds.max.y),
        (10.0, 20.0),
        "{mode:?} must not move A in Y"
      );
      assert_eq!(
        (b_bounds.min.y, b_bounds.max.y),
        (50.0, 60.0),
        "{mode:?} must not move B in Y"
      );
    } else {
      assert_eq!(
        (a_bounds.min.x, a_bounds.max.x),
        (10.0, 20.0),
        "{mode:?} must not move A in X"
      );
      assert_eq!(
        (b_bounds.min.x, b_bounds.max.x),
        (40.0, 60.0),
        "{mode:?} must not move B in X"
      );
    }
  }
}

#[test]
fn align_refuses_an_empty_selection_and_an_unknown_id_atomically() {
  let mut app = state_with_entities(&["E1", "E2"]);

  let error = assert_refused_atomically(
    &mut app,
    Command::AlignEntities {
      entity_ids: Vec::new(),
      mode: AlignMode::Left,
    },
  );
  assert!(matches!(error, CommandError::EmptySelection), "{error:?}");

  let error = assert_refused_atomically(
    &mut app,
    Command::AlignEntities {
      entity_ids: vec!["E1".to_owned(), "E-none".to_owned()],
      mode: AlignMode::Right,
    },
  );
  assert!(matches!(error, CommandError::UnknownEntity(_)), "{error:?}");
  // E1 is still where the fixture put it: 0..10 by 0..10.
  assert_bounds(entity_contour(&app, "E1"), (0.0, 0.0), (10.0, 10.0), 1.0e-9);
}

#[test]
fn undoing_a_transform_restores_the_document_exactly() {
  let mut app = state_with_layer();
  let id = draw_circle(&mut app, Point2::new(10.0, 10.0), 5.0);
  assert_single_undo_restores(
    &mut app,
    Command::TransformEntities {
      entity_ids: vec![id],
      transform: Transform::Rotate {
        radians: 0.5,
        about: Point2::new(3.0, 4.0),
      },
    },
  );
}

#[test]
fn undoing_an_array_copy_restores_the_document_exactly() {
  let mut app = state_with_entities(&["E1", "E2"]);
  assert_single_undo_restores(
    &mut app,
    array_copy(
      vec!["E1".to_owned(), "E2".to_owned()],
      2,
      3,
      100.0,
      200.0,
      3,
    ),
  );
}

#[test]
fn undoing_an_align_restores_the_document_exactly() {
  let mut app = state_with_entities(&["E1", "E2"]);
  assert_single_undo_restores(
    &mut app,
    Command::AlignEntities {
      entity_ids: vec!["E1".to_owned(), "E2".to_owned()],
      mode: AlignMode::Right,
    },
  );
}

/// Draw an open line on `layer_id` and return its id.
fn draw_line_on(app: &mut AppState, layer_id: &str, from: Point2, to: Point2) -> String {
  let id = app.next_entity_id();
  app
    .execute(Command::AddLine {
      layer_id: layer_id.to_owned(),
      entity_id: id.clone(),
      from,
      to,
    })
    .expect("the line is drawn");
  id
}

fn draw_line(app: &mut AppState, from: Point2, to: Point2) -> String {
  draw_line_on(app, "L1", from, to)
}

/// Draw an open polyline through `points` and return its id.
fn draw_polyline(app: &mut AppState, points: &[Point2]) -> String {
  let id = app.next_entity_id();
  app
    .execute(Command::AddPolyline {
      layer_id: "L1".to_owned(),
      entity_id: id.clone(),
      points: points.to_vec(),
      closed: false,
    })
    .expect("the polyline is drawn");
  id
}

fn offset_command(entity_id: &str, distance: f64, first_entity_index: u32) -> Command {
  Command::OffsetContour {
    entity_id: entity_id.to_owned(),
    distance,
    first_entity_index,
  }
}

fn trim_command(target_id: &str, cutter_id: &str, first_entity_index: u32) -> Command {
  Command::TrimEntity {
    target_id: target_id.to_owned(),
    cutter_id: cutter_id.to_owned(),
    tolerance: 1.0e-6,
    first_entity_index,
  }
}

/// The kernel's own offset of `contour`, so a test compares stored geometry to the kernel, not to
/// a number copied out of it.
fn kernel_offset(contour: &cam_geom::Contour, distance: f64) -> cam_geom::Contour {
  cam_geom::offset::offset_contour(
    contour,
    distance,
    &cam_geom::offset::OffsetOptions::default(),
  )
  .expect("the kernel offsets the fixture")
  .contour
}

fn vertex_points(contour: &cam_geom::Contour) -> Vec<Point2> {
  contour
    .vertices()
    .iter()
    .map(|vertex| vertex.point)
    .collect()
}

#[test]
fn offsetting_a_circle_by_two_millimetres_adds_a_new_arc_contour() {
  let mut app = state_with_layer();
  let source = draw_circle(&mut app, Point2::new(50.0, 20.0), 10.0);
  let before = entity_contour(&app, &source).clone();

  let outcome = app
    .execute_with_outcome(offset_command(&source, 2.0, 2))
    .expect("the offset is applied");

  assert!(app.is_dirty(), "an offset marks the document dirty");
  assert_eq!(
    outcome.created_entities,
    vec!["E2".to_owned()],
    "the offset entity is minted from first_entity_index"
  );
  assert_eq!(
    app.document().entities().count(),
    2,
    "the source is kept and the offset is a second entity"
  );
  assert_eq!(layer_holding(&app, "E2"), "L1");

  let created = entity_contour(&app, "E2");
  assert!(
    created.is_closed(),
    "an offset of a closed entity is closed"
  );
  assert!(created.is_ccw(), "the offset keeps the winding");
  assert!(
    created.has_arcs(),
    "an offset circle stays arcs, not a polygon"
  );
  assert_eq!(created.segments().len(), 2, "two half-turn arcs");
  let expected_area = std::f64::consts::PI * 12.0 * 12.0;
  let area = created.signed_area();
  assert!(
    (area - expected_area).abs() / expected_area < 1.0e-9,
    "the offset circle has radius r + 2, so area {expected_area}, got {area}"
  );
  assert_bounds(created, (38.0, 8.0), (62.0, 32.0), ARC_BOUNDS_TOLERANCE);
  assert_eq!(
    created,
    &kernel_offset(&before, 2.0),
    "cam-app must store exactly the contour the kernel returned"
  );
  assert_eq!(
    entity_contour(&app, &source),
    &before,
    "the source entity must not move or change"
  );
}

#[test]
fn a_collapsing_offset_is_refused_with_the_kernel_reason_and_no_document_change() {
  let mut app = state_with_layer();
  let source = draw_circle(&mut app, Point2::ORIGIN, 10.0);

  let error = assert_refused_atomically(&mut app, offset_command(&source, -12.0, 2));

  let CommandError::Geometry(message) = error else {
    panic!("an offset collapse must surface the kernel's message, got {error:?}");
  };
  assert!(
    message.contains("offset collapsed"),
    "the refusal must name the collapse: {message}"
  );
  assert!(
    message.contains("arc radius would collapse"),
    "the kernel's reason must survive: {message}"
  );
  assert!(
    app.document().entity("E2").is_none(),
    "a refused offset must not store an entity"
  );
  assert_bounds(
    entity_contour(&app, &source),
    (-10.0, -10.0),
    (10.0, 10.0),
    ARC_BOUNDS_TOLERANCE,
  );
}

#[test]
fn joining_two_touching_lines_yields_one_entity_and_removes_the_other() {
  let mut app = state_with_layer();
  let first = draw_line(&mut app, Point2::ORIGIN, Point2::new(10.0, 0.0));
  let second = draw_line(&mut app, Point2::new(10.0, 0.0), Point2::new(10.0, 10.0));
  assert_eq!((first.as_str(), second.as_str()), ("E1", "E2"));
  let parts = [
    entity_contour(&app, &first).clone(),
    entity_contour(&app, &second).clone(),
  ];

  let outcome = app
    .execute_with_outcome(Command::JoinEntities {
      entity_ids: vec![first.clone(), second.clone()],
      tolerance: 1.0e-6,
    })
    .expect("the two lines join");

  assert!(app.is_dirty(), "a join marks the document dirty");
  assert!(
    outcome.created_entities.is_empty(),
    "a join reuses the first id and mints nothing"
  );
  assert_eq!(
    app.document().entities().count(),
    1,
    "the joined contour replaces the selection instead of adding to it"
  );
  assert!(
    app.document().entity(&second).is_none(),
    "the other selected entity is removed"
  );

  let joined = entity_contour(&app, &first);
  assert!(!joined.is_closed(), "two lines joined end to end stay open");
  assert_eq!(joined.len(), 3, "two lines become one three-vertex chain");
  assert_eq!(joined.perimeter(), 20.0, "joining loses no length");
  assert_eq!(
    joined,
    &cam_geom::join::join(&parts, 1.0e-6).expect("kernel join"),
    "cam-app must store exactly the contour the kernel returned"
  );
}

#[test]
fn an_ambiguous_join_is_refused_atomically_naming_the_fork() {
  let mut app = state_with_layer();
  let a = draw_line(&mut app, Point2::ORIGIN, Point2::new(10.0, 0.0));
  let b = draw_line(&mut app, Point2::new(10.0, 0.0), Point2::new(20.0, 0.0));
  let c = draw_line(&mut app, Point2::new(10.0, 0.0), Point2::new(10.0, 10.0));

  let error = assert_refused_atomically(
    &mut app,
    Command::JoinEntities {
      entity_ids: vec![a.clone(), b.clone(), c.clone()],
      tolerance: 1.0e-6,
    },
  );

  let CommandError::Geometry(message) = error else {
    panic!("an ambiguous join must surface the kernel's message, got {error:?}");
  };
  assert!(
    message.contains("three or more endpoints"),
    "the refusal must name the fork: {message}"
  );
  assert!(
    message.contains("ambiguous"),
    "the refusal must say the chain is ambiguous: {message}"
  );
  assert_eq!(
    app.document().entities().count(),
    3,
    "a refused join removes nothing"
  );
  for id in [&a, &b, &c] {
    assert!(
      app.document().entity(id).is_some(),
      "{id} must survive a refused join"
    );
  }
}

#[test]
fn trimming_a_rectangle_with_a_crossing_line_replaces_it_with_one_open_entity() {
  let mut app = state_with_layer();
  let target = draw_rectangle(&mut app, Point2::ORIGIN, 20.0, 10.0);
  let cutter = draw_line(&mut app, Point2::new(10.0, -5.0), Point2::new(10.0, 15.0));
  let before_target = entity_contour(&app, &target).clone();
  let before_cutter = entity_contour(&app, &cutter).clone();
  let kernel =
    cam_geom::trim::trim(&before_target, &before_cutter, 1.0e-6).expect("two clear cuts");
  assert_eq!(kernel.kept.len(), 1, "a closed target is opened by one cut");

  let outcome = app
    .execute_with_outcome(trim_command(&target, &cutter, 3))
    .expect("the rectangle is trimmed");

  assert!(app.is_dirty(), "a trim marks the document dirty");
  assert!(
    app.document().entity(&target).is_none(),
    "the closed target is replaced by its kept piece"
  );
  assert_eq!(
    outcome.created_entities,
    vec!["E3".to_owned()],
    "one entity per kept piece, minted from first_entity_index"
  );
  assert_eq!(
    app.document().entities().count(),
    2,
    "the kept piece and the untouched cutter"
  );
  assert_eq!(
    entity_contour(&app, &cutter),
    &before_cutter,
    "the cutter is untouched"
  );
  assert_eq!(layer_holding(&app, "E3"), "L1");

  let piece = entity_contour(&app, "E3");
  assert!(!piece.is_closed(), "the surviving piece is open");
  assert_eq!(
    piece, &kernel.kept[0],
    "cam-app must store exactly the piece the kernel returned"
  );
  let removed = kernel.removed.perimeter();
  assert!(
    (removed - 30.0).abs() < 1.0e-9,
    "the removed span is half the 60 mm perimeter, got {removed}"
  );
  assert!(
    (piece.perimeter() + removed - before_target.perimeter()).abs() < 1.0e-9,
    "the kept piece plus the removed span must equal the original perimeter"
  );
}

#[test]
fn trimming_an_open_line_mints_one_entity_per_kept_piece() {
  let mut app = state_with_layer();
  let target = draw_line(&mut app, Point2::ORIGIN, Point2::new(10.0, 0.0));
  let cutter = app.next_entity_id();
  app
    .execute(Command::AddCircle {
      layer_id: "L1".to_owned(),
      entity_id: cutter.clone(),
      at: Point2::new(5.0, 0.0),
      radius: 1.0,
    })
    .expect("the cutter circle is drawn");

  let outcome = app
    .execute_with_outcome(trim_command(&target, &cutter, 3))
    .expect("the line is trimmed");

  assert_eq!(
    outcome.created_entities,
    vec!["E3".to_owned(), "E4".to_owned()],
    "both kept ends are minted, in the kernel's order along the contour"
  );
  assert!(
    app.document().entity(&target).is_none(),
    "the trimmed line is replaced by its kept pieces"
  );
  assert_eq!(
    app.document().entities().count(),
    3,
    "two pieces and the cutter"
  );
  let first = entity_contour(&app, "E3");
  let second = entity_contour(&app, "E4");
  assert!(!first.is_closed() && !second.is_closed());
  assert!(
    (first.perimeter() + second.perimeter() - 8.0).abs() < 1.0e-9,
    "the two kept ends measure 8 mm around the 2 mm cut"
  );
  assert_bounds(first, (0.0, 0.0), (4.0, 0.0), 1.0e-9);
  assert_bounds(second, (6.0, 0.0), (10.0, 0.0), 1.0e-9);
  assert!(
    entity_contour(&app, &cutter).has_arcs(),
    "the cutter keeps its arcs"
  );
}

#[test]
fn a_trim_whose_cutter_misses_is_refused_atomically() {
  let mut app = state_with_layer();
  let target = draw_rectangle(&mut app, Point2::ORIGIN, 20.0, 10.0);
  let cutter = draw_line(&mut app, Point2::new(100.0, -5.0), Point2::new(100.0, 15.0));
  let before_target = entity_contour(&app, &target).clone();

  let error = assert_refused_atomically(&mut app, trim_command(&target, &cutter, 3));

  let CommandError::Geometry(message) = error else {
    panic!("a missed cut must surface the kernel's message, got {error:?}");
  };
  assert!(
    message.contains("exactly two cut points"),
    "the refusal must name what a trim needs: {message}"
  );
  assert!(
    app.document().entity("E3").is_none(),
    "a refused trim must not mint an entity"
  );
  assert_eq!(
    entity_contour(&app, &target),
    &before_target,
    "a refused trim must leave the target exactly as it was"
  );
}

#[test]
fn a_taken_minted_id_refuses_an_offset_and_a_trim_without_storing_anything() {
  let mut app = state_with_layer();
  let source = draw_circle(&mut app, Point2::ORIGIN, 10.0);
  let _taken = draw_rectangle(&mut app, Point2::new(50.0, 50.0), 5.0, 5.0);
  assert!(app.document().entity("E2").is_some(), "E2 is taken");

  let error = assert_refused_atomically(&mut app, offset_command(&source, 2.0, 2));
  assert!(
    matches!(error, CommandError::DuplicateEntity(_)),
    "{error:?}"
  );

  // A trim that would mint several ids is refused when one of them, late in the batch, is taken.
  let mut app = state_with_layer();
  let target = draw_line(&mut app, Point2::ORIGIN, Point2::new(10.0, 0.0));
  let cutter = app.next_entity_id();
  app
    .execute(Command::AddCircle {
      layer_id: "L1".to_owned(),
      entity_id: cutter.clone(),
      at: Point2::new(5.0, 0.0),
      radius: 1.0,
    })
    .expect("the cutter circle is drawn");
  let _taken = draw_rectangle(&mut app, Point2::new(50.0, 50.0), 5.0, 5.0);
  let _also_taken = draw_rectangle(&mut app, Point2::new(60.0, 50.0), 5.0, 5.0);
  assert!(app.document().entity("E4").is_some(), "E4 is taken");

  // Two kept pieces minted from 4 would be E4 and E5, and E4 is the second one: a collision must be
  // caught before the target is removed, not after the first piece is stored.
  let error = assert_refused_atomically(&mut app, trim_command(&target, &cutter, 4));
  assert!(
    matches!(error, CommandError::DuplicateEntity(_)),
    "{error:?}"
  );
  assert!(
    app.document().entity(&target).is_some(),
    "not one piece may be stored, and the target must survive"
  );
  assert!(
    app.document().entity("E5").is_none(),
    "the second minted id must not be stored either"
  );
}

#[test]
fn moving_a_vertex_keeps_the_rest_of_the_contour() {
  let mut app = state_with_layer();
  let id = draw_polyline(
    &mut app,
    &[
      Point2::new(0.0, 0.0),
      Point2::new(10.0, 0.0),
      Point2::new(10.0, 10.0),
    ],
  );
  let before = entity_contour(&app, &id).clone();
  let to = Point2::new(5.0, 5.0);

  let outcome = app
    .execute_with_outcome(Command::EditVertex {
      entity_id: id.clone(),
      edit: VertexEdit::Move { index: 1, to },
    })
    .expect("the vertex moves");

  assert!(app.is_dirty(), "a node edit marks the document dirty");
  assert!(outcome.created_entities.is_empty(), "no entity is created");
  let contour = entity_contour(&app, &id);
  assert_eq!(
    contour,
    &cam_geom::edit::move_vertex(&before, 1, to).expect("kernel move"),
    "cam-app must store exactly the contour the kernel returned"
  );
  assert_eq!(
    vertex_points(contour),
    vec![Point2::new(0.0, 0.0), to, Point2::new(10.0, 10.0)],
    "only the named vertex moves"
  );
  assert!(!contour.is_closed(), "the contour stays open");
}

#[test]
fn inserting_a_vertex_on_an_arc_yields_two_arcs_on_the_same_circle() {
  let mut app = state_with_layer();
  let id = draw_circle(&mut app, Point2::new(10.0, 10.0), 5.0);
  let before = entity_contour(&app, &id).clone();
  // The top of the circle is the midpoint of the first half-turn arc.
  let at = Point2::new(10.0, 15.0);

  app
    .execute(Command::EditVertex {
      entity_id: id.clone(),
      edit: VertexEdit::Insert {
        segment_index: 0,
        at,
      },
    })
    .expect("the vertex is inserted");

  let contour = entity_contour(&app, &id);
  assert!(contour.is_closed(), "the circle stays closed");
  assert_eq!(
    contour.len(),
    3,
    "one vertex per arc half plus the far vertex"
  );
  assert_eq!(
    contour,
    &cam_geom::edit::insert_vertex(&before, 0, at).expect("kernel insert"),
    "cam-app must store exactly the contour the kernel returned"
  );
  let segments = contour.segments();
  assert_eq!(segments.len(), 3, "the split arc contributes two segments");
  for segment in &segments {
    let arc = segment.as_arc().expect("the circle stays arcs");
    assert!(
      (arc.radius() - 5.0).abs() < 1.0e-9,
      "a radial insertion keeps the radius, got {}",
      arc.radius()
    );
    assert!(
      arc.center().is_near(Point2::new(10.0, 10.0), 1.0e-9),
      "both halves stay on the same circle"
    );
  }
  let area = contour.signed_area();
  assert!(
    (area - std::f64::consts::PI * 25.0).abs() < 1.0e-9,
    "splitting an arc keeps the circle's area, got {area}"
  );
}

#[test]
fn inserting_a_vertex_on_a_straight_segment_keeps_it_straight() {
  let mut app = state_with_layer();
  let id = draw_polyline(&mut app, &[Point2::new(0.0, 0.0), Point2::new(20.0, 0.0)]);
  let before = entity_contour(&app, &id).clone();
  let at = Point2::new(5.0, 3.0);

  app
    .execute(Command::EditVertex {
      entity_id: id.clone(),
      edit: VertexEdit::Insert {
        segment_index: 0,
        at,
      },
    })
    .expect("the vertex is inserted");

  let contour = entity_contour(&app, &id);
  assert_eq!(contour.len(), 3, "one segment becomes two");
  assert!(!contour.has_arcs(), "a straight segment stays straight");
  assert_eq!(
    vertex_points(contour),
    vec![
      Point2::new(0.0, 0.0),
      Point2::new(5.0, 0.0),
      Point2::new(20.0, 0.0)
    ],
    "the click is projected onto the segment"
  );
  assert_eq!(
    contour,
    &cam_geom::edit::insert_vertex(&before, 0, at).expect("kernel insert")
  );
  assert_eq!(contour.perimeter(), 20.0, "the length is unchanged");
}

#[test]
fn deleting_a_vertex_stores_the_remaining_contour() {
  let mut app = state_with_layer();
  let id = draw_polyline(
    &mut app,
    &[
      Point2::new(0.0, 0.0),
      Point2::new(10.0, 0.0),
      Point2::new(10.0, 10.0),
    ],
  );
  let before = entity_contour(&app, &id).clone();

  app
    .execute(Command::EditVertex {
      entity_id: id.clone(),
      edit: VertexEdit::Delete { index: 1 },
    })
    .expect("the vertex is deleted");

  let contour = entity_contour(&app, &id);
  assert_eq!(
    vertex_points(contour),
    vec![Point2::new(0.0, 0.0), Point2::new(10.0, 10.0)],
    "the named vertex is gone and the others stay"
  );
  assert_eq!(
    contour,
    &cam_geom::edit::delete_vertex(&before, 1).expect("kernel delete")
  );
}

#[test]
fn an_out_of_range_vertex_edit_is_refused_atomically() {
  let mut app = state_with_layer();
  let id = draw_polyline(
    &mut app,
    &[
      Point2::new(0.0, 0.0),
      Point2::new(10.0, 0.0),
      Point2::new(10.0, 10.0),
    ],
  );
  let before = entity_contour(&app, &id).clone();
  let cases = [
    (
      VertexEdit::Move {
        index: 99,
        to: Point2::new(1.0, 1.0),
      },
      "no vertex at that index",
    ),
    (
      VertexEdit::Insert {
        segment_index: 99,
        at: Point2::new(1.0, 1.0),
      },
      "no segment at that index",
    ),
    (VertexEdit::Delete { index: 99 }, "no vertex at that index"),
  ];

  for (edit, reason) in cases {
    let error = assert_refused_atomically(
      &mut app,
      Command::EditVertex {
        entity_id: id.clone(),
        edit,
      },
    );
    let CommandError::Geometry(message) = error else {
      panic!("{edit:?} must surface the kernel's message, got {error:?}");
    };
    assert!(
      message.contains(reason),
      "{edit:?} must be refused naming why: {message}"
    );
    assert_eq!(
      entity_contour(&app, &id),
      &before,
      "{edit:?} must leave the contour untouched"
    );
  }
}

#[test]
fn the_new_edit_commands_refuse_unknown_ids_and_bad_numbers_atomically() {
  let mut app = state_with_layer();
  let line = draw_line(&mut app, Point2::ORIGIN, Point2::new(10.0, 0.0));
  let cutter = draw_rectangle(&mut app, Point2::new(20.0, 0.0), 5.0, 5.0);

  let error = assert_refused_atomically(
    &mut app,
    Command::OffsetContour {
      entity_id: "E-none".to_owned(),
      distance: 1.0,
      first_entity_index: 3,
    },
  );
  assert!(matches!(error, CommandError::UnknownEntity(_)), "{error:?}");

  let error = assert_refused_atomically(&mut app, offset_command(&line, f64::NAN, 3));
  assert!(
    matches!(error, CommandError::OutOfRange { .. }),
    "{error:?}"
  );

  let error = assert_refused_atomically(
    &mut app,
    Command::OffsetContour {
      entity_id: line.clone(),
      distance: 1.0,
      first_entity_index: 3,
    },
  );
  assert!(
    matches!(error, CommandError::Geometry(_)),
    "an open contour cannot be offset: {error:?}"
  );

  let error = assert_refused_atomically(
    &mut app,
    Command::JoinEntities {
      entity_ids: Vec::new(),
      tolerance: 1.0e-6,
    },
  );
  assert!(matches!(error, CommandError::EmptySelection), "{error:?}");

  let error = assert_refused_atomically(
    &mut app,
    Command::JoinEntities {
      entity_ids: vec![line.clone()],
      tolerance: 0.0,
    },
  );
  assert!(
    matches!(
      error,
      CommandError::NonPositiveSize {
        kind: "join tolerance",
        ..
      }
    ),
    "{error:?}"
  );

  let error = assert_refused_atomically(
    &mut app,
    Command::TrimEntity {
      target_id: line.clone(),
      cutter_id: "E-none".to_owned(),
      tolerance: 1.0e-6,
      first_entity_index: 3,
    },
  );
  assert!(matches!(error, CommandError::UnknownEntity(_)), "{error:?}");

  let error = assert_refused_atomically(
    &mut app,
    Command::TrimEntity {
      target_id: line.clone(),
      cutter_id: cutter.clone(),
      tolerance: f64::INFINITY,
      first_entity_index: 3,
    },
  );
  assert!(
    matches!(
      error,
      CommandError::NonPositiveSize {
        kind: "trim tolerance",
        ..
      }
    ),
    "{error:?}"
  );

  let error = assert_refused_atomically(
    &mut app,
    Command::EditVertex {
      entity_id: "E-none".to_owned(),
      edit: VertexEdit::Delete { index: 0 },
    },
  );
  assert!(matches!(error, CommandError::UnknownEntity(_)), "{error:?}");

  assert_eq!(app.document().entities().count(), 2);
}

#[test]
fn the_new_edit_commands_have_human_labels() {
  assert_eq!(offset_command("E1", 2.0, 2).label(), "offset contour");
  assert_eq!(
    Command::JoinEntities {
      entity_ids: vec!["E1".to_owned()],
      tolerance: 1.0e-6,
    }
    .label(),
    "join entities"
  );
  assert_eq!(trim_command("E1", "E2", 3).label(), "trim entity");
  let edits = [
    (
      VertexEdit::Move {
        index: 0,
        to: Point2::ORIGIN,
      },
      "move vertex",
    ),
    (
      VertexEdit::Insert {
        segment_index: 0,
        at: Point2::ORIGIN,
      },
      "insert vertex",
    ),
    (VertexEdit::Delete { index: 0 }, "delete vertex"),
  ];
  for (edit, label) in edits {
    assert_eq!(
      Command::EditVertex {
        entity_id: "E1".to_owned(),
        edit,
      }
      .label(),
      label
    );
  }
}

#[test]
fn undoing_an_offset_restores_the_document_exactly() {
  let mut app = state_with_layer();
  let source = draw_circle(&mut app, Point2::ORIGIN, 10.0);
  assert_single_undo_restores(&mut app, offset_command(&source, 2.0, 2));
}

#[test]
fn undoing_a_join_restores_the_document_exactly() {
  let mut app = state_with_layer();
  let first = draw_line(&mut app, Point2::ORIGIN, Point2::new(10.0, 0.0));
  let second = draw_line(&mut app, Point2::new(10.0, 0.0), Point2::new(10.0, 10.0));
  assert_single_undo_restores(
    &mut app,
    Command::JoinEntities {
      entity_ids: vec![first, second],
      tolerance: 1.0e-6,
    },
  );
}

#[test]
fn undoing_a_trim_restores_the_document_exactly() {
  let mut app = state_with_layer();
  let target = draw_rectangle(&mut app, Point2::ORIGIN, 20.0, 10.0);
  let cutter = draw_line(&mut app, Point2::new(10.0, -5.0), Point2::new(10.0, 15.0));
  assert_single_undo_restores(&mut app, trim_command(&target, &cutter, 3));
}

#[test]
fn undoing_a_vertex_edit_restores_the_document_exactly() {
  let mut app = state_with_layer();
  let id = draw_circle(&mut app, Point2::new(10.0, 10.0), 5.0);
  assert_single_undo_restores(
    &mut app,
    Command::EditVertex {
      entity_id: id,
      edit: VertexEdit::Insert {
        segment_index: 0,
        at: Point2::new(10.0, 15.0),
      },
    },
  );
}

// ---------------------------------------------------------------------------------------------
// The material: the one command that writes `document.board` (T12, S4, S7).
// ---------------------------------------------------------------------------------------------

/// A `SetJob` carrying a dimension no stock can have is refused, with the number in the sentence.
///
/// `Job::outline` panics on a non-positive dimension. `AppState` only catches that by validating the
/// whole document *after* `apply` has run, so `Command::apply` — the public entry point a caller can
/// hand a document to directly — was the last place a zero could reach the panic. The shell's material
/// dialog is the first thing that can type one, so the refusal lives with the command and names the
/// field and the value (S4: every refusal names a number).
#[test]
fn set_job_refuses_a_dimension_a_stock_cannot_have() {
  let cases = [
    ("width", 0.0, "got 0"),
    ("height", 0.0, "got 0"),
    ("thickness", 0.0, "got 0"),
    ("width", -600.0, "got -600"),
    ("height", f64::NAN, "got NaN"),
  ];
  for (field, bad, number) in cases {
    let command = Command::SetJob {
      width: if field == "width" { bad } else { 600.0 },
      height: if field == "height" { bad } else { 400.0 },
      thickness: if field == "thickness" { bad } else { 18.0 },
    };
    let mut document = Document::new(Job::default_sheet());
    let before = document.board.clone();
    let error = command
      .apply(&mut document)
      .expect_err("a stock needs three positive dimensions")
      .to_string();
    assert!(
      error.contains(&format!("job {field}")),
      "the refusal does not name the {field}: {error}"
    );
    assert!(
      error.contains(number),
      "the refusal does not name the number the caller typed ({number}): {error}"
    );
    assert_eq!(
      document.board, before,
      "a refused job changed the job anyway"
    );
    // The proof that matters: the document still describes a rectangle, so nothing downstream can
    // reach the panic `Job::outline` documents.
    assert_eq!(document.board.outline(), Job::default_sheet().outline());
  }
}

// --- the toolpath vocabulary: add, retool, re-operate and delete (S7) --------------------------

fn isolation_op() -> Operation {
  Operation::Isolation(IsolationParams {
    passes: 1,
    overlap: 0.5,
    direction: MillingDirection::Climb,
    kind: IsolationKind::Full,
    cut_depth: Length::from_mm(0.1),
    keep_pours: false,
  })
}

fn cutout_op() -> Operation {
  Operation::Cutout(CutoutParams {
    margin: Length::ZERO,
    tabs: Vec::new(),
    tab_kind: TabKind::Straight,
    pass_depth: Length::ZERO,
    cut_depth: Length::from_mm(1.6),
    cut_interiors: false,
  })
}

/// One `AddToolpath` over `entity_ids` with an isolation operation and no islands.
fn add_toolpath(toolpath_id: &str, entity_ids: Vec<String>, tool_number: u32) -> Command {
  Command::AddToolpath {
    toolpath_id: toolpath_id.to_owned(),
    name: toolpath_id.to_owned(),
    entity_ids,
    tool_number,
    operation: isolation_op(),
  }
}
#[test]
fn adding_a_toolpath_stores_its_tool_number_and_operation() {
  let mut app = state_with_layer();
  let rectangle = draw_rectangle(&mut app, Point2::ORIGIN, 20.0, 10.0);

  app
    .execute(add_toolpath("TP1", vec![rectangle.clone()], 4))
    .expect("the toolpath is added");

  let stored = app
    .document()
    .toolpaths
    .first()
    .expect("the toolpath is stored");
  assert_eq!(stored.id, "TP1");
  assert_eq!(stored.name, "TP1");
  assert_eq!(stored.tool_number, 4);
  assert_eq!(stored.entity_ids, vec![rectangle]);
  assert_eq!(stored.operation, isolation_op());
}

#[test]
fn undoing_an_added_toolpath_restores_the_document_exactly() {
  let mut app = state_with_layer();
  let rectangle = draw_rectangle(&mut app, Point2::ORIGIN, 20.0, 10.0);
  assert_single_undo_restores(&mut app, add_toolpath("TP1", vec![rectangle], 4));
  assert!(
    app.document().toolpaths.is_empty(),
    "the undo leaves no toolpath behind"
  );
}

#[test]
fn adding_a_toolpath_whose_id_is_taken_is_refused_naming_the_toolpath() {
  let mut app = state_with_layer();
  let rectangle = draw_rectangle(&mut app, Point2::ORIGIN, 20.0, 10.0);
  app
    .execute(add_toolpath("TP1", vec![rectangle.clone()], 4))
    .expect("the first toolpath is added");

  let error = assert_refused_atomically(&mut app, add_toolpath("TP1", vec![rectangle], 5));
  assert_eq!(
    error,
    CommandError::DuplicateToolpath("TP1".to_owned()),
    "{error:?}"
  );
  assert_eq!(error.to_string(), "toolpath \"TP1\" already exists");
  assert_eq!(
    app.document().toolpaths.len(),
    1,
    "the refusal stores nothing"
  );
}

#[test]
fn adding_a_toolpath_naming_no_entity_is_refused() {
  let mut app = state_with_layer();
  let error = assert_refused_atomically(&mut app, add_toolpath("TP1", Vec::new(), 4));
  assert!(matches!(error, CommandError::EmptySelection), "{error:?}");
  assert_eq!(error.to_string(), "no entities were selected");
  assert!(app.document().toolpaths.is_empty());
}

#[test]
fn adding_a_toolpath_naming_an_absent_entity_is_refused_naming_both() {
  let mut app = state_with_layer();
  let error = assert_refused_atomically(&mut app, add_toolpath("TP1", vec!["E99".to_owned()], 4));
  assert_eq!(
    error,
    CommandError::Invalid(ModelError::UnknownEntity {
      toolpath: "TP1".to_owned(),
      entity: "E99".to_owned(),
    }),
    "{error:?}"
  );
  assert_eq!(
    error.to_string(),
    "the command was refused: toolpath \"TP1\" references unknown entity \"E99\""
  );
  assert!(app.document().toolpaths.is_empty());
}

#[test]
fn setting_a_toolpaths_tool_number_leaves_the_rest_of_it_alone() {
  let mut app = state_with_layer();
  let rectangle = draw_rectangle(&mut app, Point2::ORIGIN, 20.0, 10.0);
  app
    .execute(add_toolpath("TP1", vec![rectangle.clone()], 4))
    .expect("the toolpath is added");
  let before = app.document().toolpaths[0].clone();

  assert_single_undo_restores(
    &mut app,
    Command::SetToolpathToolNumber {
      toolpath_id: "TP1".to_owned(),
      tool_number: 7,
    },
  );
  // The undo above restored the old number, so set it again and compare the rest of the record.
  app
    .execute(Command::SetToolpathToolNumber {
      toolpath_id: "TP1".to_owned(),
      tool_number: 7,
    })
    .expect("the tool number is set");

  let after = app.document().toolpaths[0].clone();
  assert_eq!(after.tool_number, 7);
  assert_eq!(after.id, before.id);
  assert_eq!(after.name, before.name);
  assert_eq!(after.entity_ids, before.entity_ids);
  assert_eq!(after.operation, before.operation);
}

#[test]
fn setting_a_tool_number_on_an_absent_toolpath_is_refused_naming_it() {
  let mut app = state_with_layer();
  let error = assert_refused_atomically(
    &mut app,
    Command::SetToolpathToolNumber {
      toolpath_id: "TP9".to_owned(),
      tool_number: 7,
    },
  );
  assert_eq!(
    error,
    CommandError::UnknownToolpath("TP9".to_owned()),
    "{error:?}"
  );
  assert_eq!(error.to_string(), "toolpath \"TP9\" does not exist");
}

#[test]
fn setting_a_toolpaths_operation_keeps_the_tool_and_the_entities() {
  let mut app = state_with_layer();
  let rectangle = draw_rectangle(&mut app, Point2::ORIGIN, 20.0, 10.0);
  app
    .execute(add_toolpath("TP1", vec![rectangle.clone()], 4))
    .expect("the toolpath is added");

  app
    .execute(Command::SetToolpathOperation {
      toolpath_id: "TP1".to_owned(),
      operation: cutout_op(),
    })
    .expect("the operation is set");

  let stored = &app.document().toolpaths[0];
  assert_eq!(stored.operation, cutout_op());
  assert_eq!(stored.tool_number, 4);
  assert_eq!(stored.entity_ids, vec![rectangle]);
}

#[test]
fn setting_an_operation_on_an_absent_toolpath_is_refused_naming_it() {
  let mut app = state_with_layer();
  let error = assert_refused_atomically(
    &mut app,
    Command::SetToolpathOperation {
      toolpath_id: "TP9".to_owned(),
      operation: cutout_op(),
    },
  );
  assert_eq!(
    error,
    CommandError::UnknownToolpath("TP9".to_owned()),
    "{error:?}"
  );
}

#[test]
fn renaming_a_toolpath_changes_only_its_name_and_one_undo_restores_it() {
  let mut app = state_with_layer();
  let rectangle = draw_rectangle(&mut app, Point2::ORIGIN, 20.0, 10.0);
  app
    .execute(add_toolpath("TP1", vec![rectangle.clone()], 4))
    .expect("the toolpath is added");
  let before = app.document().toolpaths[0].clone();

  let rename = || Command::SetToolpathName {
    toolpath_id: "TP1".to_owned(),
    name: "Roughing pass".to_owned(),
  };
  assert_single_undo_restores(&mut app, rename());
  app.execute(rename()).expect("the name is set");

  let after = app.document().toolpaths[0].clone();
  assert_eq!(after.name, "Roughing pass");
  assert_eq!(after.id, before.id, "a rename keeps the id");
  assert_eq!(
    after.tool_number, before.tool_number,
    "a rename keeps the tool"
  );
  assert_eq!(
    after.entity_ids, before.entity_ids,
    "a rename keeps the geometry"
  );
  assert_eq!(
    after.operation, before.operation,
    "a rename keeps the operation"
  );
  assert_eq!(
    app.document().toolpaths.len(),
    1,
    "and touches no other toolpath"
  );
}

#[test]
fn renaming_a_toolpath_to_an_empty_or_blank_name_is_refused_and_writes_nothing() {
  let mut app = state_with_layer();
  let rectangle = draw_rectangle(&mut app, Point2::ORIGIN, 20.0, 10.0);
  app
    .execute(add_toolpath("TP1", vec![rectangle], 4))
    .expect("the toolpath is added");

  for name in ["", "   ", "\t\n "] {
    let error = assert_refused_atomically(
      &mut app,
      Command::SetToolpathName {
        toolpath_id: "TP1".to_owned(),
        name: name.to_owned(),
      },
    );
    assert_eq!(
      error,
      CommandError::EmptyToolpathName,
      "{name:?}: {error:?}"
    );
    assert_eq!(error.to_string(), "a toolpath's name must not be empty");
    assert_eq!(
      app.document().toolpaths[0].name,
      "TP1",
      "{name:?}: a refused rename leaves the old name standing"
    );
  }
}

#[test]
fn renaming_an_absent_toolpath_is_refused_naming_it() {
  let mut app = state_with_layer();
  let error = assert_refused_atomically(
    &mut app,
    Command::SetToolpathName {
      toolpath_id: "TP9".to_owned(),
      name: "Other".to_owned(),
    },
  );
  assert_eq!(
    error,
    CommandError::UnknownToolpath("TP9".to_owned()),
    "{error:?}"
  );
  assert_eq!(error.to_string(), "toolpath \"TP9\" does not exist");
}

#[test]
fn deleting_a_toolpath_removes_only_that_one() {
  let mut app = state_with_layer();
  let rectangle = draw_rectangle(&mut app, Point2::ORIGIN, 20.0, 10.0);
  app
    .execute(add_toolpath("TP1", vec![rectangle.clone()], 4))
    .expect("the first toolpath is added");
  app
    .execute(add_toolpath("TP2", vec![rectangle], 5))
    .expect("the second toolpath is added");

  assert_single_undo_restores(
    &mut app,
    Command::DeleteToolpath {
      toolpath_id: "TP1".to_owned(),
    },
  );
  app
    .execute(Command::DeleteToolpath {
      toolpath_id: "TP1".to_owned(),
    })
    .expect("the toolpath is deleted");

  let remaining: Vec<&str> = app
    .document()
    .toolpaths
    .iter()
    .map(|toolpath| toolpath.id.as_str())
    .collect();
  assert_eq!(remaining, vec!["TP2"]);
  assert_eq!(
    app.document().entities().count(),
    1,
    "geometry is untouched"
  );
}

#[test]
fn deleting_an_absent_toolpath_is_refused_naming_it_and_leaves_the_others() {
  let mut app = state_with_layer();
  let rectangle = draw_rectangle(&mut app, Point2::ORIGIN, 20.0, 10.0);
  app
    .execute(add_toolpath("TP1", vec![rectangle], 4))
    .expect("the toolpath is added");

  let error = assert_refused_atomically(
    &mut app,
    Command::DeleteToolpath {
      toolpath_id: "TP9".to_owned(),
    },
  );
  assert_eq!(
    error,
    CommandError::UnknownToolpath("TP9".to_owned()),
    "{error:?}"
  );
  assert_eq!(error.to_string(), "toolpath \"TP9\" does not exist");
  assert_eq!(app.document().toolpaths.len(), 1);
}

#[test]
fn the_toolpath_commands_have_human_labels() {
  assert_eq!(add_toolpath("TP1", Vec::new(), 4).label(), "add toolpath");
  assert_eq!(
    Command::SetToolpathToolNumber {
      toolpath_id: "TP1".to_owned(),
      tool_number: 4,
    }
    .label(),
    "set toolpath tool"
  );
  assert_eq!(
    Command::SetToolpathOperation {
      toolpath_id: "TP1".to_owned(),
      operation: isolation_op(),
    }
    .label(),
    "set toolpath operation"
  );
  assert_eq!(
    Command::SetToolpathName {
      toolpath_id: "TP1".to_owned(),
      name: "N".to_owned(),
    }
    .label(),
    "set toolpath name"
  );
  assert_eq!(
    Command::DeleteToolpath {
      toolpath_id: "TP1".to_owned(),
    }
    .label(),
    "delete toolpath"
  );
}
