//! Behaviour tests for the editing commands (A1, A5, A8).

use cam_app::{AppState, Command, CommandError};
use cam_geom::{primitive, Point2};
use cam_model::{Document, EntityKind, Job, Layer};

fn state_with_layers(layer_ids: &[&str]) -> AppState {
  let mut document = Document::new(Job::default_sheet());
  for id in layer_ids {
    document.layers.push(Layer {
      id: (*id).to_owned(),
      name: (*id).to_owned(),
      visible: true,
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
