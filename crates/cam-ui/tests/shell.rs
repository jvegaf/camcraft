//! Headless behaviour tests for the egui shell (A7, A8, A9).
//!
//! No display and no GPU: `egui::Context` runs a full frame on the CPU, the shell reports what it
//! displayed, and the painted text is read back out of the produced shapes. Behaviour — what a user
//! action does to the document — is asserted by applying the shell's actions through `handle_action`.

use cam_app::AppState;
use cam_geom::{Contour, Point2};
use cam_model::{Document, Job, Layer};
use cam_ui::{handle_action, Shell, ShellOutput, UiAction};

fn document_with_rectangle() -> Document {
  let mut document = Document::new(Job::default_sheet());
  document.layers.push(Layer {
    id: "L1".to_owned(),
    name: "Part".to_owned(),
    visible: true,
    entities: vec![cam_model::Entity {
      id: "E1".to_owned(),
      kind: cam_model::EntityKind::Contour {
        contour: Contour::from_points(
          &[
            Point2::new(0.0, 0.0),
            Point2::new(100.0, 0.0),
            Point2::new(100.0, 60.0),
            Point2::new(0.0, 60.0),
          ],
          true,
        )
        .expect("rectangle"),
      },
    }],
  });
  document
}

fn raw_input() -> egui::RawInput {
  egui::RawInput {
    screen_rect: Some(egui::Rect::from_min_size(
      egui::Pos2::ZERO,
      egui::vec2(1200.0, 800.0),
    )),
    ..Default::default()
  }
}

/// Run one frame and return what the shell produced plus the text it painted.
fn frame_with(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
) -> (ShellOutput, Vec<String>) {
  let mut captured: Option<ShellOutput> = None;
  let full = ctx.run_pass(raw_input(), |ctx| {
    captured = Some(shell.show(app, ctx));
  });
  let texts = painted_text(&full);
  // The texture deltas belong to a renderer we do not have in a headless test; dropping them
  // silently is exactly what epaint forbids, so say so explicitly.
  full.drop_without_applying_deltas();
  (captured.expect("the shell ran"), texts)
}

fn frame(shell: &mut Shell, app: &mut AppState) -> (ShellOutput, Vec<String>) {
  let ctx = egui::Context::default();
  frame_with(&ctx, shell, app)
}

/// Every piece of text the frame actually drew.
fn painted_text(output: &egui::FullOutput) -> Vec<String> {
  output
    .shapes
    .iter()
    .filter_map(|clipped| match &clipped.shape {
      egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
      _ => None,
    })
    .collect()
}

fn joined(texts: &[String]) -> String {
  texts.join(" | ")
}

#[test]
fn the_shell_lays_out_a_canvas_between_the_panels() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let (output, _) = frame(&mut shell, &mut app);

  assert!(output.canvas.width > 0.0 && output.canvas.height > 0.0);
  assert!(
    output.canvas.width < 1200.0,
    "the side panels must take room from the canvas, got {}",
    output.canvas.width
  );
  // The camera follows the canvas, which is what makes picking and painting agree.
  assert_eq!(
    app.camera().viewport().width,
    output.canvas.width,
    "the camera viewport must match the canvas"
  );
}

#[test]
fn the_layer_dock_reports_every_layer_and_paints_its_name() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let (output, texts) = frame(&mut shell, &mut app);

  assert_eq!(
    output.drawn.layer_rows,
    vec![("L1".to_owned(), 1, true)],
    "the dock must report id, entity count and visibility"
  );
  assert!(
    joined(&texts).contains("L1 — Part"),
    "the layer row must be painted: {}",
    joined(&texts)
  );
  assert!(joined(&texts).contains("Layers"));
}

#[test]
fn the_properties_panel_shows_the_job_and_the_selection() {
  let mut app = AppState::new(document_with_rectangle());
  app.select(vec!["E1".to_owned()]);
  let mut shell = Shell::new();
  let (output, texts) = frame(&mut shell, &mut app);

  let properties = output.drawn.property_lines.join(" | ");
  assert!(properties.contains("Job:"), "{properties}");
  assert!(properties.contains("E1: contour"), "{properties}");
  assert!(joined(&texts).contains("Properties"));
}

#[test]
fn the_canvas_paints_the_geometry_the_grid_and_the_zoom_readout() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let (output, texts) = frame(&mut shell, &mut app);

  assert_eq!(output.drawn.painted_items, 4, "four rectangle edges");
  assert!(
    output.drawn.painted_grid_lines > 0,
    "the grid must be painted"
  );
  assert!(
    joined(&texts).contains("px/mm"),
    "the zoom readout must be painted: {}",
    joined(&texts)
  );
}

#[test]
fn the_command_bar_offers_undo_redo_and_the_drawing_tools() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let (_, texts) = frame(&mut shell, &mut app);
  let all = joined(&texts);
  for label in ["Undo", "Redo", "Zoom to fit", "Add", "Delete selection"] {
    assert!(all.contains(label), "missing {label:?} in {all}");
  }
}

#[test]
fn the_status_bar_reports_zoom_saved_state_and_history() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let (output, _) = frame(&mut shell, &mut app);
  let line = &output.drawn.status_line;
  assert!(line.contains("px/mm"), "{line}");
  assert!(line.contains("saved"), "{line}");
  assert!(line.contains("undo 0"), "{line}");
}

#[test]
fn a_frame_without_input_asks_for_nothing() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let (output, _) = frame(&mut shell, &mut app);
  assert!(
    output.actions.is_empty(),
    "no input must produce no actions, got {:?}",
    output.actions
  );
}

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
fn zoom_to_fit_frames_the_geometry() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let _ = frame(&mut shell, &mut app);
  app
    .camera_mut()
    .zoom_at(cam_app::ScreenPoint::new(0.0, 0.0), 40.0);
  let zoomed_in = app.camera().pixels_per_mm();

  handle_action(&mut app, &UiAction::ZoomToFit).expect("fit");
  let fitted = app.camera().pixels_per_mm();
  assert!(fitted < zoomed_in, "fit must zoom out to frame the job");
  assert!(fitted > 0.0);
}

#[test]
fn zoom_to_fit_on_an_empty_document_reports_why() {
  let mut app = AppState::new(Document::new(Job::default_sheet()));
  let error = handle_action(&mut app, &UiAction::ZoomToFit).expect_err("nothing to frame");
  assert!(error.contains("nothing to frame"), "{error}");
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

#[test]
fn selecting_through_an_action_updates_the_properties_panel() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let _ = frame(&mut shell, &mut app);
  handle_action(&mut app, &UiAction::SetSelection(vec!["E1".to_owned()])).expect("select");
  let (output, _) = frame(&mut shell, &mut app);
  assert!(output
    .drawn
    .property_lines
    .join(" ")
    .contains("E1: contour"));
}
