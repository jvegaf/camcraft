//! Behaviour tests for the editing commands (A1, A5, A8).

use cam_app::{AppState, Command, CommandError};
use cam_geom::Point2;
use cam_model::{Document, EntityKind, Job, Layer};

fn state_with_layer() -> AppState {
  let mut document = Document::new(Job::default_sheet());
  document.layers.push(Layer {
    id: "L1".to_owned(),
    name: "Part".to_owned(),
    visible: true,
    entities: Vec::new(),
  });
  AppState::new(document)
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
  ];
  for command in commands {
    assert!(!command.label().is_empty());
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
