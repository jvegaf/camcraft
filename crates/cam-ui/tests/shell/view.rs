use super::support::*;
use cam_app::AppState;
use cam_geom::Point2;
use cam_model::{Document, Job};
use cam_ui::{handle_action, Shell, UiAction};

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
fn a_degenerate_canvas_is_never_framed() {
  // The first frames of a real window can report a canvas of a few dozen pixels while the panels take
  // their room. Fitting that geometry zoomed the drawing out by an order of magnitude.
  let mut app = AppState::new(document_with_rectangle());
  app
    .camera_mut()
    .set_viewport(cam_app::Viewport::new(1200.0, 700.0));
  let mut shell = Shell::new();

  let tiny = cam_ui::CanvasRect {
    x: 0.0,
    y: 0.0,
    width: 24.0,
    height: 18.0,
  };
  let _ = shell.frame_if_needed(&mut app, &tiny);
  assert_eq!(
    app.camera().pixels_per_mm(),
    1.0,
    "a 24x18 canvas must not be framed"
  );
  assert!(app.camera().center().is_near(Point2::ORIGIN, 1.0e-9));

  // The real layout arrives and the drawing is framed.
  let real = cam_ui::CanvasRect {
    x: 220.0,
    y: 30.0,
    width: 1000.0,
    height: 700.0,
  };
  // A canvas that changes size before the user touches the view is reframed for the new size.
  assert!(
    shell.frame_if_needed(&mut app, &real),
    "the first real canvas must be framed"
  );
  assert!(
    app.camera().pixels_per_mm() > 3.0,
    "framing a 100x60 drawing in 1000x700 must zoom in, got {}",
    app.camera().pixels_per_mm()
  );
  let centre = app.camera().center();
  assert!(
    (centre.x - 50.0).abs() < 1.0 && (centre.y - 30.0).abs() < 1.0,
    "the view must centre on the geometry, not the stock: {centre:?}"
  );
}

#[test]
fn a_view_the_user_owns_is_not_refitted() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let canvas = cam_ui::CanvasRect {
    x: 0.0,
    y: 0.0,
    width: 1000.0,
    height: 700.0,
  };
  assert!(shell.frame_if_needed(&mut app, &canvas));
  let zoom = app.camera().pixels_per_mm();

  // The user takes the view; the shell must not steal it back.
  shell.release_view();
  app
    .camera_mut()
    .zoom_at(cam_app::ScreenPoint::new(400.0, 300.0), 3.0);
  let manual = app.camera().pixels_per_mm();
  assert!(
    !shell.frame_if_needed(&mut app, &canvas),
    "a view the user owns must not be refitted"
  );
  assert_eq!(app.camera().pixels_per_mm(), manual);
  assert!(manual != zoom);
}

#[test]
fn a_frame_of_the_real_shell_ends_up_framed_on_the_geometry() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  // Framing happens on the frame that measures the canvas, so one frame is enough.
  let (output, _) = frame(&mut shell, &mut app);
  assert!(output.canvas.width > cam_ui::MIN_FRAME_CANVAS);
  let centre = app.camera().center();
  assert!(
    (centre.x - 50.0).abs() < 5.0 && (centre.y - 30.0).abs() < 5.0,
    "the shell must frame the geometry, got {centre:?} at {} px/mm",
    app.camera().pixels_per_mm()
  );
  assert!(
    app.camera().pixels_per_mm() > 2.0,
    "a 100x60 rectangle in a large canvas must not be tiny: {} px/mm",
    app.camera().pixels_per_mm()
  );
}
