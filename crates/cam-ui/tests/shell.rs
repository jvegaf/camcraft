//! Headless behaviour tests for the egui shell (A7, A8, A9).
//!
//! No display and no GPU: `egui::Context` runs a full frame on the CPU, the shell reports what it
//! displayed, and the painted text is read back out of the produced shapes. Behaviour — what a user
//! action does to the document — is asserted by applying the shell's actions through `handle_action`.

use cam_app::{AlignMode, AppState, Transform, VertexEdit};
use cam_geom::{Contour, Point2, Vec2};
use cam_model::{Document, Job, Layer};
use cam_ui::{handle_action, DrawShape, Shell, ShellOutput, Tool, UiAction, DEFAULT_TOLERANCE_MM};

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

#[test]
fn the_status_bar_distinguishes_undo_from_redo() {
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
  handle_action(&mut app, &UiAction::Undo).expect("undone");

  let (output, _) = frame(&mut shell, &mut app);
  let line = &output.drawn.status_line;
  assert!(line.contains("undo 0 / redo 1"), "{line}");
  assert!(!line.contains("dropped"), "{line}");
}

#[test]
fn dropped_history_is_reported_separately_from_redo() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let _ = frame(&mut shell, &mut app);
  for _ in 0..(cam_app::MAX_HISTORY + 3) {
    handle_action(&mut app, &UiAction::SetSelection(Vec::new())).expect("no-op");
    app
      .execute(cam_app::Command::SetJob {
        width: 100.0,
        height: 100.0,
        thickness: 10.0,
      })
      .expect("applies");
  }
  let (output, _) = frame(&mut shell, &mut app);
  let line = &output.drawn.status_line;
  assert!(line.contains("3 step(s) dropped"), "{line}");
  assert!(line.contains("redo 0"), "{line}");
}

// ---------------------------------------------------------------------------------------------
// M2 authoring (A13): every authoring command is one `UiAction` handled by the same function the
// shell uses, and the status bar reports what the command produced.
// ---------------------------------------------------------------------------------------------

/// The contour of `id`, cloned so the document borrow ends before the assertions.
fn contour_of(app: &AppState, id: &str) -> Contour {
  app
    .document()
    .entity(id)
    .and_then(|entity| entity.kind.as_contour())
    .unwrap_or_else(|| panic!("{id} must be a contour"))
    .clone()
}

fn entity_count(app: &AppState) -> usize {
  app.document().entities().count()
}

/// The only selected id.
fn only_selected(app: &AppState) -> String {
  let ids: Vec<String> = app.selection().ids().map(str::to_owned).collect();
  assert_eq!(ids.len(), 1, "exactly one entity must be selected: {ids:?}");
  ids.into_iter().next().expect("one id")
}

#[test]
fn the_draw_group_reaches_every_drawing_command() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let shapes = [
    DrawShape::Rectangle {
      at: Point2::new(0.0, 0.0),
      width: 10.0,
      height: 5.0,
    },
    DrawShape::Line {
      from: Point2::new(0.0, 0.0),
      to: Point2::new(10.0, 0.0),
    },
    DrawShape::Polyline {
      points: vec![
        Point2::new(0.0, 0.0),
        Point2::new(10.0, 0.0),
        Point2::new(10.0, 10.0),
      ],
      closed: false,
    },
    DrawShape::Arc {
      start: Point2::new(0.0, 0.0),
      through: Point2::new(10.0, 10.0),
      end: Point2::new(20.0, 0.0),
    },
    DrawShape::Circle {
      at: Point2::new(50.0, 50.0),
      radius: 10.0,
    },
    DrawShape::Ellipse {
      at: Point2::new(50.0, 50.0),
      rx: 20.0,
      ry: 10.0,
      tolerance: DEFAULT_TOLERANCE_MM,
    },
    DrawShape::Polygon {
      at: Point2::new(50.0, 50.0),
      radius: 10.0,
      sides: 6,
      rotation: 0.0,
    },
    DrawShape::Star {
      at: Point2::new(50.0, 50.0),
      outer_radius: 10.0,
      inner_radius: 5.0,
      points: 5,
      rotation: 0.0,
    },
  ];

  for shape in shapes {
    let before = entity_count(&app);
    shell.apply_action(&mut app, &UiAction::Draw(shape.clone()));
    assert_eq!(
      entity_count(&app),
      before + 1,
      "{shape:?} must add exactly one entity: {}",
      shell.status
    );
    let drawn = only_selected(&app);
    assert_ne!(drawn, "E1", "{shape:?} must draw a new entity");
    assert!(
      app.document().entity(&drawn).is_some(),
      "{shape:?} must select what it drew: {}",
      shell.status
    );
    assert!(
      shell.status.contains("1 entity created"),
      "{shape:?}: {}",
      shell.status
    );
    assert_eq!(app.history_depth(), 1, "{shape:?} must be one undo step");
    shell.apply_action(&mut app, &UiAction::Undo);
    assert_eq!(
      entity_count(&app),
      before,
      "{shape:?} must be undone in one step"
    );
  }
}

#[test]
fn the_transform_group_reaches_translate_rotate_scale_and_mirror() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(&mut app, &UiAction::SetSelection(vec!["E1".to_owned()]));

  shell.apply_action(
    &mut app,
    &UiAction::Transform(Transform::Translate {
      delta: Vec2::new(10.0, 5.0),
    }),
  );
  let bounds = contour_of(&app, "E1").bounds();
  assert!(
    bounds.min.is_near(Point2::new(10.0, 5.0), 1.0e-9)
      && bounds.max.is_near(Point2::new(110.0, 65.0), 1.0e-9),
    "translate: {bounds:?}"
  );

  shell.apply_action(
    &mut app,
    &UiAction::Transform(Transform::Rotate {
      radians: std::f64::consts::PI,
      about: Point2::ORIGIN,
    }),
  );
  let bounds = contour_of(&app, "E1").bounds();
  assert!(
    bounds.min.is_near(Point2::new(-110.0, -65.0), 1.0e-9)
      && bounds.max.is_near(Point2::new(-10.0, -5.0), 1.0e-9),
    "rotate: {bounds:?}"
  );

  shell.apply_action(
    &mut app,
    &UiAction::Transform(Transform::Scale {
      sx: 2.0,
      sy: 2.0,
      about: Point2::ORIGIN,
      tolerance: DEFAULT_TOLERANCE_MM,
    }),
  );
  let bounds = contour_of(&app, "E1").bounds();
  assert!(
    bounds.min.is_near(Point2::new(-220.0, -130.0), 1.0e-9)
      && bounds.max.is_near(Point2::new(-20.0, -10.0), 1.0e-9),
    "scale: {bounds:?}"
  );

  shell.apply_action(
    &mut app,
    &UiAction::Transform(Transform::Mirror {
      point: Point2::ORIGIN,
      direction: Vec2::Y_AXIS,
    }),
  );
  let bounds = contour_of(&app, "E1").bounds();
  assert!(
    bounds.min.is_near(Point2::new(20.0, -130.0), 1.0e-9)
      && bounds.max.is_near(Point2::new(220.0, -10.0), 1.0e-9),
    "mirror: {bounds:?}"
  );
  assert_eq!(
    app.history_depth(),
    4,
    "each transform is one undo step: {}",
    shell.status
  );
}

#[test]
fn the_array_group_copies_the_selection_and_keeps_the_id_counter_in_step() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(&mut app, &UiAction::SetSelection(vec!["E1".to_owned()]));

  shell.apply_action(
    &mut app,
    &UiAction::Array {
      rows: 2,
      columns: 2,
      dx: 200.0,
      dy: 100.0,
    },
  );
  assert_eq!(
    entity_count(&app),
    5,
    "a 2x2 array of one entity adds four copies: {}",
    shell.status
  );
  assert!(
    shell.status.contains("4 entities created"),
    "{}",
    shell.status
  );
  assert_eq!(
    app.selection().len(),
    4,
    "the copies are selected, the source is not"
  );

  // The ids the array minted must not collide with the next draw: the shell has to move its id
  // counter with the batch, or the next shape is refused as a duplicate.
  shell.apply_action(
    &mut app,
    &UiAction::Draw(DrawShape::Rectangle {
      at: Point2::new(0.0, 0.0),
      width: 1.0,
      height: 1.0,
    }),
  );
  assert!(
    !shell.status.contains("refused"),
    "a draw after an array must not collide: {}",
    shell.status
  );
  assert_eq!(entity_count(&app), 6, "{}", shell.status);
}

#[test]
fn the_align_group_lines_the_selection_up_on_its_own_bounds() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(
    &mut app,
    &UiAction::Draw(DrawShape::Rectangle {
      at: Point2::new(200.0, 20.0),
      width: 20.0,
      height: 20.0,
    }),
  );
  let added = only_selected(&app);
  shell.apply_action(
    &mut app,
    &UiAction::SetSelection(vec!["E1".to_owned(), added.clone()]),
  );

  shell.apply_action(&mut app, &UiAction::Align(AlignMode::Left));

  let bounds = contour_of(&app, &added).bounds();
  assert!(
    (bounds.min.x - 0.0).abs() < 1.0e-9,
    "aligning left puts every left edge on the group's left edge: {bounds:?}"
  );
  assert!(
    (bounds.min.y - 20.0).abs() < 1.0e-9,
    "aligning left leaves the other axis alone: {bounds:?}"
  );
}

#[test]
fn the_offset_group_adds_a_contour_and_keeps_the_source() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(&mut app, &UiAction::SetSelection(vec!["E1".to_owned()]));

  shell.apply_action(&mut app, &UiAction::Offset { distance: 2.0 });

  assert_eq!(entity_count(&app), 2, "{}", shell.status);
  let created = only_selected(&app);
  assert_ne!(created, "E1", "the offset is a new entity");
  let bounds = contour_of(&app, &created).bounds();
  assert!(
    bounds.min.is_near(Point2::new(-2.0, -2.0), 1.0e-9)
      && bounds.max.is_near(Point2::new(102.0, 62.0), 1.0e-9),
    "a rectangle offset by 2 mm grows by 2 mm on every side: {bounds:?}"
  );
  let source = contour_of(&app, "E1").bounds();
  assert!(
    source.min.is_near(Point2::ORIGIN, 1.0e-9),
    "the source must not move: {source:?}"
  );
}

#[test]
fn the_join_group_merges_touching_entities_into_one() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(
    &mut app,
    &UiAction::Draw(DrawShape::Line {
      from: Point2::new(0.0, 0.0),
      to: Point2::new(10.0, 0.0),
    }),
  );
  let first = only_selected(&app);
  shell.apply_action(
    &mut app,
    &UiAction::Draw(DrawShape::Line {
      from: Point2::new(10.0, 0.0),
      to: Point2::new(10.0, 10.0),
    }),
  );
  let second = only_selected(&app);
  shell.apply_action(
    &mut app,
    &UiAction::SetSelection(vec![first.clone(), second.clone()]),
  );

  shell.apply_action(&mut app, &UiAction::Join);

  assert_eq!(
    entity_count(&app),
    2,
    "the rectangle plus the joined line: {}",
    shell.status
  );
  assert!(
    app.document().entity(&second).is_none(),
    "the second line is merged into the first"
  );
  let joined = contour_of(&app, &first);
  assert!(!joined.is_closed(), "two lines joined end to end stay open");
  assert!(
    (joined.perimeter() - 20.0).abs() < 1.0e-9,
    "joining loses no length, got {}",
    joined.perimeter()
  );
}

#[test]
fn the_trim_group_replaces_the_target_with_the_kept_piece() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(
    &mut app,
    &UiAction::Draw(DrawShape::Line {
      from: Point2::new(50.0, -10.0),
      to: Point2::new(50.0, 70.0),
    }),
  );
  let cutter = only_selected(&app);

  shell.apply_action(
    &mut app,
    &UiAction::Trim {
      target: "E1".to_owned(),
      cutter: cutter.clone(),
    },
  );

  assert_eq!(
    entity_count(&app),
    2,
    "the rectangle becomes one open piece beside the cutter: {}",
    shell.status
  );
  assert!(
    app.document().entity("E1").is_none(),
    "the trimmed target id is freed"
  );
  let kept = only_selected(&app);
  assert_ne!(kept, cutter, "the kept piece is the new entity");
  assert!(
    !contour_of(&app, &kept).is_closed(),
    "trimming a closed contour opens it"
  );
}

#[test]
fn the_vertex_edit_group_moves_inserts_and_deletes_nodes() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(&mut app, &UiAction::SetSelection(vec!["E1".to_owned()]));

  shell.apply_action(
    &mut app,
    &UiAction::EditVertex(VertexEdit::Move {
      index: 0,
      to: Point2::new(-10.0, -10.0),
    }),
  );
  assert!(
    contour_of(&app, "E1").vertices()[0]
      .point
      .is_near(Point2::new(-10.0, -10.0), 1.0e-9),
    "moving vertex 0 must move it: {}",
    shell.status
  );

  shell.apply_action(
    &mut app,
    &UiAction::EditVertex(VertexEdit::Insert {
      segment_index: 1,
      at: Point2::new(110.0, 30.0),
    }),
  );
  assert_eq!(
    contour_of(&app, "E1").len(),
    5,
    "inserting a node adds one vertex: {}",
    shell.status
  );

  shell.apply_action(
    &mut app,
    &UiAction::EditVertex(VertexEdit::Delete { index: 2 }),
  );
  assert_eq!(
    contour_of(&app, "E1").len(),
    4,
    "deleting the inserted node removes it again: {}",
    shell.status
  );
  assert_eq!(app.history_depth(), 3, "each node edit is one undo step");
}

#[test]
fn the_text_group_adds_one_entity_per_glyph_stroke() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();

  shell.apply_action(
    &mut app,
    &UiAction::AddText {
      text: "AB".to_owned(),
      height: 10.0,
      origin: Point2::new(0.0, -20.0),
    },
  );

  let added = entity_count(&app) - 1;
  assert!(
    added >= 2,
    "\"AB\" must draw more than one stroke, got {added}: {}",
    shell.status
  );
  assert_eq!(
    app.selection().len(),
    added,
    "the whole word is selected: {}",
    shell.status
  );
  assert!(
    shell.status.contains("entities created"),
    "{}",
    shell.status
  );

  // Text mints an unknown number of ids, so the counter has to be topped up by what was created.
  shell.apply_action(
    &mut app,
    &UiAction::Draw(DrawShape::Rectangle {
      at: Point2::new(0.0, 0.0),
      width: 1.0,
      height: 1.0,
    }),
  );
  assert!(
    !shell.status.contains("refused"),
    "a draw after text must not collide: {}",
    shell.status
  );
}

#[test]
fn a_non_uniform_scale_reports_the_arcs_it_lost_in_the_status_bar() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(
    &mut app,
    &UiAction::Draw(DrawShape::Circle {
      at: Point2::ORIGIN,
      radius: 10.0,
    }),
  );

  shell.apply_action(
    &mut app,
    &UiAction::Transform(Transform::Scale {
      sx: 2.0,
      sy: 1.0,
      about: Point2::ORIGIN,
      tolerance: DEFAULT_TOLERANCE_MM,
    }),
  );

  let (output, _) = frame(&mut shell, &mut app);
  assert!(
    output
      .drawn
      .status_line
      .contains("2 arc(s) lost to tessellation"),
    "a circle has two arcs, and the status bar must say both were lost: {}",
    output.drawn.status_line
  );
}

#[test]
fn drawing_an_ellipse_reports_the_radial_error_its_arcs_achieved() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(
    &mut app,
    &UiAction::Draw(DrawShape::Ellipse {
      at: Point2::ORIGIN,
      rx: 40.0,
      ry: 10.0,
      tolerance: DEFAULT_TOLERANCE_MM,
    }),
  );

  let (output, _) = frame(&mut shell, &mut app);
  assert!(
    output.drawn.status_line.contains("radial error"),
    "the achieved error must reach the user: {}",
    output.drawn.status_line
  );
}

#[test]
fn a_refused_command_puts_the_kernel_reason_in_the_status_bar_and_changes_nothing() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(
    &mut app,
    &UiAction::Draw(DrawShape::Circle {
      at: Point2::ORIGIN,
      radius: 10.0,
    }),
  );
  let before = app.document().clone();
  let depth = app.history_depth();

  // Offsetting a radius-10 circle inwards by 12 mm has nowhere to go; the kernel refuses it.
  shell.apply_action(&mut app, &UiAction::Offset { distance: -12.0 });

  assert!(
    shell.status.contains("refused"),
    "a refusal must say so: {}",
    shell.status
  );
  assert!(
    shell.status.contains("arc radius would collapse"),
    "the kernel's own reason must survive: {}",
    shell.status
  );
  assert_eq!(
    app.document(),
    &before,
    "a refused command must leave the document untouched"
  );
  assert_eq!(
    app.history_depth(),
    depth,
    "a refused command must not grow the history"
  );

  let (output, _) = frame(&mut shell, &mut app);
  assert!(
    output
      .drawn
      .status_line
      .contains("arc radius would collapse"),
    "the refusal must be on screen: {}",
    output.drawn.status_line
  );
}

#[test]
fn the_tools_area_offers_every_authoring_command() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let (_, texts) = frame(&mut shell, &mut app);
  let all = joined(&texts);
  for label in [
    "Select",
    "Line",
    "Rectangle",
    "Polyline",
    "Arc",
    "Circle",
    "Ellipse",
    "Polygon",
    "Star",
    "Move",
    "Rotate",
    "Scale",
    "Mirror",
    "Array",
    "Left",
    "Right",
    "Top",
    "Bottom",
    "Centre H",
    "Centre V",
    "Offset",
    "Join",
    "Trim",
    "Move node",
    "Insert node",
    "Delete node",
    "Add text",
  ] {
    assert!(all.contains(label), "missing {label:?} in {all}");
  }
}

#[test]
fn every_tool_can_be_armed_and_the_shell_still_runs_headless() {
  for tool in Tool::ALL {
    let mut app = AppState::new(document_with_rectangle());
    let mut shell = Shell::new();
    shell.active_tool = tool;
    let (output, texts) = frame(&mut shell, &mut app);
    assert!(
      output.canvas.width > 0.0 && output.canvas.height > 0.0,
      "{tool:?} must still lay out a canvas"
    );
    assert!(
      output.drawn.painted_items > 0,
      "{tool:?} must still paint the document"
    );
    assert!(
      output.actions.is_empty(),
      "{tool:?} must not act without input: {:?}",
      output.actions
    );
    assert!(joined(&texts).contains("Properties"));
  }
}

/// A two-line DXF, written to a scratch directory, so the shell test drives a real importer.
fn write_dxf(directory: &std::path::Path) -> std::path::PathBuf {
  let dxf = "0\nSECTION\n2\nENTITIES\n\
0\nLINE\n8\n0\n10\n0.0\n20\n0.0\n11\n100.0\n21\n0.0\n\
0\nLINE\n8\n0\n10\n100.0\n20\n0.0\n11\n100.0\n21\n60.0\n\
0\nENDSEC\n0\nEOF\n";
  let path = directory.join("part.dxf");
  std::fs::write(&path, dxf).expect("dxf written");
  path
}

fn scratch(name: &str) -> std::path::PathBuf {
  let directory = std::env::temp_dir().join(format!("campilot-ui-{}-{name}", std::process::id()));
  let _ = std::fs::remove_dir_all(&directory);
  std::fs::create_dir_all(&directory).expect("scratch directory");
  directory
}

#[test]
fn importing_a_file_replaces_the_drawing_and_says_what_arrived() {
  let directory = scratch("import");
  let path = write_dxf(&directory);
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();

  shell.apply_action(
    &mut app,
    &UiAction::ImportFile {
      path: path.display().to_string(),
    },
  );

  assert_eq!(app.document().entities().count(), 2);
  assert_eq!(
    shell.status,
    "import file — 2 entities created, 2 contour(s) from part.dxf, 0 curve(s) flattened, 0 skipped"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_failed_import_leaves_the_drawing_untouched() {
  let mut app = AppState::new(document_with_rectangle());
  let before = app.document_json();
  let mut shell = Shell::new();

  shell.apply_action(
    &mut app,
    &UiAction::ImportFile {
      path: "/nonexistent/part.dxf".to_owned(),
    },
  );

  assert_eq!(app.document_json(), before);
  assert_eq!(app.history_depth(), 0);
  assert!(!app.is_dirty());
  assert!(shell.status.starts_with("refused: "), "{}", shell.status);
  assert!(
    shell.status.contains("/nonexistent/part.dxf"),
    "{}",
    shell.status
  );
}

#[test]
fn one_undo_after_an_import_brings_the_previous_drawing_back() {
  let directory = scratch("import-undo");
  let path = write_dxf(&directory);
  let mut app = AppState::new(document_with_rectangle());
  let original = app.document_json();
  let mut shell = Shell::new();

  shell.apply_action(
    &mut app,
    &UiAction::ImportFile {
      path: path.display().to_string(),
    },
  );
  assert_eq!(app.document().entities().count(), 2);

  shell.apply_action(&mut app, &UiAction::Undo);

  assert_eq!(app.document_json(), original);
  std::fs::remove_dir_all(&directory).ok();
}
