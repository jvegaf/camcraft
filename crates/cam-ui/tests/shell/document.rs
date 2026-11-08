use super::support::*;
use cam_app::AppState;
use cam_model::{Document, Job};
use cam_ui::{handle_action, Shell, UiAction};

#[test]
fn adding_a_rectangle_draws_it_and_selects_it() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  // One frame so the camera knows the canvas size and the world centre.
  let _ = frame(&mut shell, &mut app);

  handle_action(
    &mut app,
    &UiAction::AddRectangle {
      width: 40.0,
      height: 20.0,
    },
  )
  .expect("the rectangle is added");

  assert_eq!(app.document().entities().count(), 2);
  assert_eq!(
    app.selection().len(),
    1,
    "a freshly drawn entity is selected"
  );
  let id = app.selection().ids().next().expect("one id").to_owned();
  let contour = app
    .document()
    .entity(&id)
    .and_then(|entity| entity.kind.as_contour())
    .expect("contour");
  assert!((contour.signed_area() - 800.0).abs() < 1.0e-6);

  let (output, _) = frame(&mut shell, &mut app);
  assert_eq!(output.drawn.painted_items, 8, "both rectangles are painted");
}

#[test]
fn undo_and_redo_round_trip_through_actions() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let _ = frame(&mut shell, &mut app);
  handle_action(
    &mut app,
    &UiAction::AddRectangle {
      width: 10.0,
      height: 10.0,
    },
  )
  .expect("added");

  handle_action(&mut app, &UiAction::Undo).expect("undo");
  assert_eq!(app.document().entities().count(), 1);

  handle_action(&mut app, &UiAction::Redo).expect("redo");
  assert_eq!(app.document().entities().count(), 2);
}

#[test]
fn deleting_the_selection_removes_it_and_clears_the_selection() {
  let mut app = AppState::new(document_with_rectangle());
  app.select(vec!["E1".to_owned()]);
  let mut shell = Shell::new();
  let _ = frame(&mut shell, &mut app);

  handle_action(&mut app, &UiAction::DeleteSelection).expect("deleted");
  assert_eq!(app.document().entities().count(), 0);
  assert!(app.selection().is_empty());

  let (output, _) = frame(&mut shell, &mut app);
  assert_eq!(output.drawn.painted_items, 0);
}

#[test]
fn hiding_a_layer_removes_it_from_the_picture_and_from_picking() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let _ = frame(&mut shell, &mut app);

  handle_action(
    &mut app,
    &UiAction::ToggleLayerVisible {
      layer_id: "L1".to_owned(),
    },
  )
  .expect("toggled");
  assert!(!app.document().layers[0].visible);

  let (output, _) = frame(&mut shell, &mut app);
  assert_eq!(output.drawn.painted_items, 0);
  assert_eq!(
    output.drawn.layer_rows,
    vec![("L1".to_owned(), 1, false)],
    "the dock must show the layer as hidden while still listing it"
  );
}

#[test]
fn drawing_into_a_document_without_layers_reports_why() {
  let mut app = AppState::new(Document::new(Job::default_sheet()));
  let error = handle_action(
    &mut app,
    &UiAction::AddRectangle {
      width: 10.0,
      height: 10.0,
    },
  )
  .expect_err("no layer to draw into");
  assert!(error.contains("no layer"), "{error}");
}

#[test]
fn toggling_a_missing_layer_reports_why() {
  let mut app = AppState::new(document_with_rectangle());
  let error = handle_action(
    &mut app,
    &UiAction::ToggleLayerVisible {
      layer_id: "L-missing".to_owned(),
    },
  )
  .expect_err("must fail");
  assert!(error.contains("L-missing"), "{error}");
}
