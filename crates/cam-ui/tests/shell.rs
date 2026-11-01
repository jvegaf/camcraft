//! Headless behaviour tests for the egui shell (A7, A8, A9).
//!
//! No display and no GPU: `egui::Context` runs a full frame on the CPU, the shell reports what it
//! displayed, and the painted text is read back out of the produced shapes. Behaviour — what a user
//! action does to the document — is asserted by applying the shell's actions through `handle_action`.

use cam_app::{AlignMode, AppState, Transform, VertexEdit};
use cam_geom::{Contour, Point2, Vec2};
use cam_model::{Document, Job, Layer};
use cam_ui::help::{Command as HelpCommand, Group};
use cam_ui::{
  handle_action, DrawShape, Readiness, Shell, ShellOutput, Stage, Tab, Tool, UiAction,
  DEFAULT_TOLERANCE_MM,
};

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
  frame_with_events(ctx, shell, app, Vec::new())
}

/// Run one frame with explicit events, and return what the shell produced plus its painted text.
fn frame_with_events(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
  events: Vec<egui::Event>,
) -> (ShellOutput, Vec<String>) {
  let mut captured: Option<ShellOutput> = None;
  let mut input = raw_input();
  input.events = events;
  let full = ctx.run_pass(input, |ctx| {
    captured = Some(shell.show(app, ctx));
  });
  let texts = painted_text(&full);
  // The texture deltas belong to a renderer we do not have in a headless test; dropping them
  // silently is exactly what epaint forbids, so say so explicitly.
  full.drop_without_applying_deltas();
  (captured.expect("the shell ran"), texts)
}

/// Press one key and report the frame the press was read in.
///
/// A press is two frames — down then up — the way a keyboard reports it. The second frame is what
/// makes the next press a press instead of a held key, and it is dropped: what a test asserts on is
/// the frame the shell acted in.
fn press(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
  key: egui::Key,
) -> (ShellOutput, Vec<String>) {
  let key_event = |pressed: bool| egui::Event::Key {
    key,
    physical_key: None,
    pressed,
    repeat: false,
    modifiers: egui::Modifiers::default(),
  };
  let frame = frame_with_events(ctx, shell, app, vec![key_event(true)]);
  let _ = frame_with_events(ctx, shell, app, vec![key_event(false)]);
  frame
}

fn frame(shell: &mut Shell, app: &mut AppState) -> (ShellOutput, Vec<String>) {
  let ctx = egui::Context::default();
  frame_with(&ctx, shell, app)
}

/// Press the pointer at one window point: move there, press, then release.
///
/// A click is two frames, the way a mouse reports it, and the frame that reports the click is the one
/// the release lands in. The move is repeated in both passes because egui decides what is under the
/// pointer from the input it ended the previous pass with.
fn click_at(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
  at: egui::Pos2,
) -> (ShellOutput, Vec<String>) {
  let mut last: Option<(ShellOutput, Vec<String>)> = None;
  for pressed in [true, false] {
    let mut input = raw_input();
    input.events.push(egui::Event::PointerMoved(at));
    input.events.push(egui::Event::PointerButton {
      pos: at,
      button: egui::PointerButton::Primary,
      pressed,
      modifiers: egui::Modifiers::default(),
    });
    let mut captured: Option<ShellOutput> = None;
    let full = ctx.run_pass(input, |ctx| {
      captured = Some(shell.show(app, ctx));
    });
    let texts = painted_text(&full);
    full.drop_without_applying_deltas();
    last = Some((captured.expect("the shell ran"), texts));
  }
  last.expect("a click is two frames")
}

/// Click one palette chip by the word on it.
///
/// The palette reports the point it drew each chip at ([`cam_ui::DrawnState::palette_hits`]), because
/// egui cannot be asked for a widget by its text from outside a frame and a test that guessed at the
/// layout would break the day a margin moved.
fn click_palette_chip(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
  label: &str,
) -> (ShellOutput, Vec<String>) {
  let (output, _) = frame_with(ctx, shell, app);
  let at = output
    .drawn
    .palette_hits
    .iter()
    .find(|(word, _)| word == label)
    .map(|(_, point)| egui::pos2(point.0, point.1))
    .unwrap_or_else(|| panic!("no {label:?} chip: {:?}", output.drawn.palette));
  click_at(ctx, shell, app, at)
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

/// A window tall enough to lay the whole Tools tab out, so every one of its buttons is pressable.
///
/// The inspector's body is a scroll area capped at 55 % of the window height, so at the standard 800 px the
/// lower half of the tab falls outside the clip rect and a click at a reported point lands on nothing above
/// it. Reporting a point is only useful while the widget it came from is still on screen.
fn tall_input() -> egui::RawInput {
  let mut input = raw_input();
  input.screen_rect = Some(egui::Rect::from_min_size(
    egui::Pos2::ZERO,
    egui::vec2(1200.0, 2000.0),
  ));
  input
}

/// Run one frame of the tall window, with events, and report what the shell produced.
fn tall_frame(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
  events: Vec<egui::Event>,
) -> ShellOutput {
  let mut captured: Option<ShellOutput> = None;
  let mut input = tall_input();
  input.events = events;
  let full = ctx.run_pass(input, |ctx| {
    captured = Some(shell.show(app, ctx));
  });
  full.drop_without_applying_deltas();
  captured.expect("the shell ran")
}

/// Click one Tools-tab button by the word on it.
///
/// The tab reports where it drew each parameterised command's button ([`cam_ui::DrawnState::authoring_hits`])
/// for the same reason the palette does: egui cannot be asked for a widget by its text from outside a frame.
fn click_authoring_button(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
  label: &str,
) -> ShellOutput {
  let output = tall_frame(ctx, shell, app, Vec::new());
  let at = output
    .drawn
    .authoring_hits
    .iter()
    .find(|(word, _)| word == label)
    .map(|(_, point)| egui::pos2(point.0, point.1))
    .unwrap_or_else(|| panic!("no {label:?} button in the Tools tab"));
  let mut last = None;
  for pressed in [true, false] {
    // The move is repeated in both passes because egui decides what is under the pointer from the input it
    // ended the previous pass with.
    last = Some(tall_frame(
      ctx,
      shell,
      app,
      vec![
        egui::Event::PointerMoved(at),
        egui::Event::PointerButton {
          pos: at,
          button: egui::PointerButton::Primary,
          pressed,
          modifiers: egui::Modifiers::default(),
        },
      ],
    ));
  }
  last.expect("a click is two frames")
}

/// Read a scrollable panel all the way down.
///
/// egui paints only what falls inside a scroll area's clip rect, so one frame shows the top of a long
/// tab and nothing below it. This walks the pointer into the panel and turns the wheel until the
/// painted text stops growing — which is what an operator does to read the same list.
fn texts_scrolled_to_the_end(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
  at: egui::Pos2,
) -> Vec<String> {
  let mut seen: Vec<String> = Vec::new();
  let mut input = raw_input();
  input.events.push(egui::Event::PointerMoved(at));
  for _ in 0..16 {
    input.events.push(egui::Event::MouseWheel {
      unit: egui::MouseWheelUnit::Point,
      delta: egui::vec2(0.0, -160.0),
      phase: egui::TouchPhase::Move,
      source: egui::MouseWheelSource::Unknown,
      modifiers: egui::Modifiers::default(),
    });
    let mut captured: Option<ShellOutput> = None;
    let full = ctx.run_pass(input.clone(), |ctx| {
      captured = Some(shell.show(app, ctx));
    });
    let texts = painted_text(&full);
    full.drop_without_applying_deltas();
    let before = seen.len();
    for text in texts {
      if !seen.contains(&text) {
        seen.push(text);
      }
    }
    let actions = captured.expect("the shell ran").actions;
    for action in actions {
      shell.apply_action(app, &action);
    }
    if seen.len() == before {
      break;
    }
  }
  seen
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
fn the_selection_tab_shows_the_job_and_the_selection() {
  let mut app = AppState::new(document_with_rectangle());
  app.select(vec!["E1".to_owned()]);
  let mut shell = Shell::new();
  shell.set_tab(Tab::Selection);
  let (output, texts) = frame(&mut shell, &mut app);

  let properties = output.drawn.property_lines.join(" | ");
  assert!(properties.contains("Job:"), "{properties}");
  assert!(properties.contains("E1: contour"), "{properties}");
  assert!(joined(&texts).contains("Inspector"));
  assert!(joined(&texts).contains("Delete selection"));
}

#[test]
fn the_stage_bar_reports_what_each_stage_can_prove() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let (output, texts) = frame(&mut shell, &mut app);
  let all = joined(&texts);
  assert_eq!(output.drawn.stage, Stage::Design);
  // One vector for Design, and the count the document can actually prove for toolpaths.
  assert!(
    all.contains("Design  1"),
    "the vector count must be real: {all}"
  );
  assert!(
    all.contains("Toolpaths  0"),
    "the document holds no toolpaths and the bar must say so: {all}"
  );
  // The stages the shell cannot finish yet show a dash, never a zero it did not measure.
  assert!(all.contains("Edit  —"), "{all}");
}

#[test]
fn switching_stage_changes_what_the_palette_offers() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let (design, _) = frame(&mut shell, &mut app);
  assert!(
    design.drawn.palette.iter().any(|entry| entry == "Line"),
    "the drawing tools belong on the Design palette: {:?}",
    design.drawn.palette
  );
  assert!(
    !design.drawn.palette.iter().any(|entry| entry == "Offset"),
    "an edit command must not be on the Design palette: {:?}",
    design.drawn.palette
  );

  shell.set_stage(Stage::Edit);
  let (edit, _) = frame(&mut shell, &mut app);
  assert_eq!(edit.drawn.stage, Stage::Edit);
  for expected in [
    "Move", "Rotate", "Scale", "Mirror", "Array", "Offset", "Join",
  ] {
    assert!(
      edit.drawn.palette.iter().any(|entry| entry == expected),
      "missing {expected:?} in {:?}",
      edit.drawn.palette
    );
  }
  assert!(
    !edit.drawn.palette.iter().any(|entry| entry == "Line"),
    "a drawing tool must not be on the Edit palette: {:?}",
    edit.drawn.palette
  );
}

#[test]
fn the_status_bar_offers_a_chip_per_snap_kind() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let (_, texts) = frame(&mut shell, &mut app);
  let all = joined(&texts);
  for label in ["Endpoint", "Midpoint", "Centre", "Grid", "magnet"] {
    assert!(all.contains(label), "missing {label:?} in {all}");
  }
}

#[test]
fn the_snap_chips_arm_and_disarm_a_kind_but_never_leave_none() {
  use cam_view::SnapKind;
  // No document and no frame: arming a snap kind is shell state, and this is the whole rule.
  let mut shell = Shell::new();
  assert!(shell.snap_armed(SnapKind::Midpoint));

  shell.toggle_snap(SnapKind::Midpoint);
  assert!(
    !shell.snap_armed(SnapKind::Midpoint),
    "a chip turns its kind off"
  );
  shell.toggle_snap(SnapKind::Midpoint);
  assert!(shell.snap_armed(SnapKind::Midpoint), "and back on");

  // The last armed kind stays: a snapping strip with everything off is a magnet with no field.
  for kind in [SnapKind::Endpoint, SnapKind::Midpoint, SnapKind::Center] {
    shell.toggle_snap(kind);
  }
  shell.toggle_snap(SnapKind::Grid);
  assert!(
    shell.snap_armed(SnapKind::Grid),
    "the last kind cannot be disarmed"
  );
}

#[test]
fn the_hint_names_the_current_gesture() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let (output, texts) = frame(&mut shell, &mut app);
  assert!(
    output.drawn.hint.contains("left→right"),
    "Select must explain the marquee directions: {}",
    output.drawn.hint
  );
  assert!(
    joined(&texts).contains("left→right"),
    "and the bar must show it"
  );

  shell.active_tool = Tool::Line;
  let (output, _) = frame(&mut shell, &mut app);
  assert!(
    output.drawn.hint.starts_with("Line"),
    "an armed tool names itself: {}",
    output.drawn.hint
  );

  shell.set_view_3d(true);
  let (output, _) = frame(&mut shell, &mut app);
  assert!(
    output.drawn.hint.contains("orbit"),
    "the 3D view has its own gestures: {}",
    output.drawn.hint
  );
}

#[test]
fn the_palette_labels_its_groups() {
  // The reference app's lesson: a labelled group is what keeps twenty commands findable, and an
  // unlabelled rail is what made the seven Edit commands nameless.
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let (_, texts) = frame(&mut shell, &mut app);
  let all = joined(&texts);
  assert!(
    all.contains("DRAW"),
    "Design must label its drawing group: {all}"
  );
  assert!(
    all.contains("VIEW"),
    "the invariants get their own group: {all}"
  );

  shell.set_stage(Stage::Edit);
  let (_, texts) = frame(&mut shell, &mut app);
  let all = joined(&texts);
  assert!(all.contains("TRANSFORM"), "{all}");
  assert!(all.contains("VECTOR EDIT"), "{all}");
}

#[test]
fn the_job_tab_shows_labelled_measurements_in_two_columns() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.set_tab(Tab::Job);
  let (_, texts) = frame(&mut shell, &mut app);
  let all = joined(&texts);
  for label in ["MATERIAL", "Width X", "Height Y", "Thickness Z", "DOCUMENT"] {
    assert!(all.contains(label), "missing {label:?} in {all}");
  }
}

#[test]
fn a_stage_with_no_tools_of_its_own_says_where_its_work_lives() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.set_stage(Stage::Toolpaths);
  let (_, texts) = frame(&mut shell, &mut app);
  let all = joined(&texts);
  assert!(
    all.contains("cam-cli toolpath"),
    "an empty stage must explain itself: {all}"
  );
}

#[test]
fn collapsing_the_inspector_widens_the_canvas_and_the_strip_opens_it_again() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let ctx = egui::Context::default();
  let (open, _) = frame_with(&ctx, &mut shell, &mut app);

  shell.set_inspector_open(false);
  let (closed, texts) = frame_with(&ctx, &mut shell, &mut app);
  assert!(
    !joined(&texts).contains("Inspector"),
    "the panel must be gone"
  );
  assert!(
    closed.canvas.width > open.canvas.width,
    "the canvas must take the width back: {} vs {}",
    closed.canvas.width,
    open.canvas.width
  );

  // Touching the strip brings it back: that is what makes collapsing usable rather than modal.
  let strip = egui::pos2(
    (closed.canvas.x + closed.canvas.width) as f32 + 6.0,
    (closed.canvas.y + 40.0) as f32,
  );
  // Two passes with the pointer there: egui decides what is hovered from the interaction state the
  // previous pass ended with, so one pass with a moved pointer reports hover on the next frame.
  for _ in 0..2 {
    let mut input = raw_input();
    input.events.push(egui::Event::PointerMoved(strip));
    let full = ctx.run_pass(input, |ctx| {
      shell.show(&mut app, ctx);
    });
    full.drop_without_applying_deltas();
  }
  assert!(shell.inspector_open(), "hovering the strip must reopen it");
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
fn the_app_bar_offers_the_document_the_units_and_the_primary_action() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let (output, texts) = frame(&mut shell, &mut app);
  let all = joined(&texts);
  for label in ["campilot", "untitled.c2d", "mm", "Generate G-code"] {
    assert!(all.contains(label), "missing {label:?} in {all}");
  }
  // The rail is where the gestures live now, and it says so in its own report rather than in pixels.
  for tool in Tool::ALL {
    assert!(
      output
        .drawn
        .palette
        .iter()
        .any(|entry| entry == tool.label()),
      "{:?} must be on the rail: {:?}",
      tool,
      output.drawn.palette
    );
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
  shell.set_tab(Tab::Selection);
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
fn the_tools_tab_offers_every_authoring_command() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let ctx = egui::Context::default();
  let (output, first) = frame_with(&ctx, &mut shell, &mut app);
  let inspector = egui::pos2(
    (output.canvas.x + output.canvas.width) as f32 + 100.0,
    (output.canvas.y + 120.0) as f32,
  );
  let mut texts = first;
  texts.extend(texts_scrolled_to_the_end(
    &ctx, &mut shell, &mut app, inspector,
  ));
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
    "Add rectangle",
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
    assert!(joined(&texts).contains("Inspector"));
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

// ---- T8.1: the command table and the F1 help panel --------------------------------------------

#[test]
fn every_command_has_a_what_and_a_how() {
  for command in HelpCommand::ALL {
    let help = command.help();
    assert!(
      !help.what.trim().is_empty(),
      "{command:?} has no consequence to state"
    );
    assert!(
      !help.how.trim().is_empty(),
      "{command:?} has no instruction to give"
    );
  }
}

#[test]
fn a_what_is_one_sentence() {
  // The design's first rule: one sentence of consequence, before any mechanics. A paragraph here is a
  // manual nobody reads, and a second line is a sentence the operator has to pick from.
  for command in HelpCommand::ALL {
    let what = command.help().what;
    assert!(
      !what.contains('\n'),
      "{command:?} says {what:?}, which is more than one line"
    );
    assert!(
      what.ends_with('.'),
      "{command:?} says {what:?}, which is not a sentence"
    );
    assert!(
      !what.contains('!'),
      "{command:?} says {what:?}, and the shell has no business shouting"
    );
    assert!(
      what.chars().count() <= 140,
      "{command:?} says {what:?} in {} characters, and one sentence is at most 140",
      what.chars().count()
    );
  }
}

#[test]
fn every_related_command_is_a_real_command() {
  for command in HelpCommand::ALL {
    for related in command.help().related {
      assert!(
        HelpCommand::ALL.contains(related),
        "{command:?} points at {related:?}, which this shell does not offer"
      );
    }
  }
}

#[test]
fn the_palette_and_the_table_agree_on_labels() {
  // The rail's own literals are the truth; the table follows them. Without this the help would slowly
  // become a second vocabulary for the same command.
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.set_stage(Stage::Edit);
  let (output, _) = frame(&mut shell, &mut app);
  let offered = output.drawn.palette;
  for command in [
    HelpCommand::Move,
    HelpCommand::Rotate,
    HelpCommand::Scale,
    HelpCommand::Mirror,
    HelpCommand::Array,
    HelpCommand::Offset,
    HelpCommand::Join,
  ] {
    assert!(
      offered.contains(&command.label().to_owned()),
      "the palette offers {offered:?} but the table calls this command {:?}",
      command.label()
    );
  }
}

#[test]
fn f1_opens_the_help_panel_and_it_names_every_group() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let ctx = egui::Context::default();
  let _ = frame_with(&ctx, &mut shell, &mut app);

  let _ = press(&ctx, &mut shell, &mut app, egui::Key::F1);
  assert!(shell.show_help, "F1 must open the help panel");
  // One more frame: the panel's own size settles on the second pass, exactly as a real window does.
  let (output, texts) = frame_with(&ctx, &mut shell, &mut app);

  for group in Group::ALL {
    assert!(
      output.drawn.help.contains(&group.label().to_owned()),
      "the panel must name the {} group: {:?}",
      group.label(),
      output.drawn.help
    );
  }
  for command in HelpCommand::ALL {
    assert!(
      output.drawn.help.contains(&command.label().to_owned()),
      "the panel must list {:?}: {:?}",
      command.label(),
      output.drawn.help
    );
  }
  assert!(
    joined(&texts).contains("Help"),
    "the window must say what it is: {}",
    joined(&texts)
  );
  // The panel has no scroll area, so a row that does not fit is a command nobody can read about. These
  // three words are painted by the panel and by nothing else in the frame, and they are at the bottom.
  for alone in ["Icon set", "Shortcuts", "Fit the job"] {
    assert!(
      joined(&texts).contains(alone),
      "{alone:?} is below the fold of the help panel: {}",
      joined(&texts)
    );
  }
}

#[test]
fn the_help_panel_says_what_a_command_does_and_how() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let ctx = egui::Context::default();
  let _ = frame_with(&ctx, &mut shell, &mut app);
  let _ = press(&ctx, &mut shell, &mut app, egui::Key::F1);
  let (output, texts) = frame_with(&ctx, &mut shell, &mut app);

  let focused = HelpCommand::ALL[0];
  assert_eq!(
    shell.help_focused(),
    focused,
    "the panel opens on the first command"
  );
  let help = focused.help();
  let all = joined(&texts);
  assert!(
    all.contains(help.what),
    "the panel must state what {focused:?} gives: {all}"
  );
  assert!(
    all.contains(help.how),
    "the panel must say how to run {focused:?}: {all}"
  );
  assert!(
    output.drawn.help.contains(&help.what.to_owned())
      && output.drawn.help.contains(&help.how.to_owned()),
    "the panel must report the two sentences it drew: {:?}",
    output.drawn.help
  );
}

#[test]
fn escape_closes_the_help_panel() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let ctx = egui::Context::default();
  let _ = frame_with(&ctx, &mut shell, &mut app);
  let _ = press(&ctx, &mut shell, &mut app, egui::Key::F1);
  assert!(shell.show_help);

  let _ = press(&ctx, &mut shell, &mut app, egui::Key::Escape);
  assert!(!shell.show_help, "Esc must close the help panel");
  let (output, texts) = frame_with(&ctx, &mut shell, &mut app);
  assert!(
    output.drawn.help.is_empty(),
    "a closed panel draws no rows: {:?}",
    output.drawn.help
  );
  assert!(
    !joined(&texts).contains(HelpCommand::ALL[0].help().what),
    "a closed panel teaches nothing: {}",
    joined(&texts)
  );
}

// ---- T8.2: the palette and the app bar read the command table --------------------------------

/// The groups one stage shows, VIEW last: the palette's four runs, and never HISTORY.
fn palette_groups(stage: Stage) -> Vec<Group> {
  let mut groups = match stage {
    Stage::Design => vec![Group::Draw],
    Stage::Edit => vec![Group::Transform, Group::VectorEdit],
    Stage::Relief | Stage::Toolpaths | Stage::Prove => Vec::new(),
  };
  groups.push(Group::View);
  groups
}

/// The words the palette must offer for one stage, in the table's own order.
///
/// Derived from [`HelpCommand::ALL`] and not from a list of expectations: the point of the slice is
/// that the table is the only list, so a test that wrote its own second list would not test that.
fn palette_words(stage: Stage) -> Vec<String> {
  let groups = palette_groups(stage);
  HelpCommand::ALL
    .iter()
    .filter(|command| groups.contains(&command.group()) && command.in_palette())
    .map(|command| command.label().to_owned())
    .collect()
}

#[test]
fn every_palette_entry_comes_from_the_table() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let ctx = egui::Context::default();

  for stage in [Stage::Design, Stage::Edit, Stage::Toolpaths] {
    shell.set_stage(stage);
    let (output, _) = frame_with(&ctx, &mut shell, &mut app);
    let offered = output.drawn.palette.clone();
    assert_eq!(
      offered,
      palette_words(stage),
      "the {stage:?} palette must be the table's own run, in its order"
    );
    for word in &offered {
      assert!(
        HelpCommand::ALL
          .iter()
          .any(|command| command.label() == word),
        "{word:?} is on the palette but no command in the table answers to it: {offered:?}"
      );
    }
  }
}

#[test]
fn the_palette_shows_no_history_run() {
  // HISTORY is the app bar's run: Undo and Redo are buttons up there, and filing them under VIEW in
  // the palette would be a lie in the one place the shell is supposed to be exact (L29).
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let ctx = egui::Context::default();

  shell.set_stage(Stage::Design);
  let (_, texts) = frame_with(&ctx, &mut shell, &mut app);
  let design = joined(&texts);
  assert!(
    design.contains(Group::Draw.label()) && design.contains(Group::View.label()),
    "Design must label DRAW and VIEW: {design}"
  );
  assert!(
    !design.contains(Group::History.label()),
    "the palette must never draw a HISTORY run: {design}"
  );

  shell.set_stage(Stage::Edit);
  let (_, texts) = frame_with(&ctx, &mut shell, &mut app);
  let edit = joined(&texts);
  for group in [Group::Transform, Group::VectorEdit, Group::View] {
    assert!(
      edit.contains(group.label()),
      "Edit must label the {} run: {edit}",
      group.label()
    );
  }
  assert!(
    !edit.contains(Group::History.label()),
    "the palette must never draw a HISTORY run: {edit}"
  );
}

#[test]
fn the_help_button_opens_the_help_panel() {
  // "Help" means the F1 panel now: the panel teaches every command, so a help button that opened a
  // list of chords instead would be answering a smaller question than the one asked.
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let ctx = egui::Context::default();
  let (output, _) = frame_with(&ctx, &mut shell, &mut app);
  assert!(
    output.drawn.help.is_empty(),
    "no panel is open before the click: {:?}",
    output.drawn.help
  );

  let _ = click_palette_chip(&ctx, &mut shell, &mut app, HelpCommand::Help.label());
  assert!(shell.show_help, "the VIEW Help chip must open the F1 panel");

  let (output, texts) = frame_with(&ctx, &mut shell, &mut app);
  assert!(
    !output.drawn.help.is_empty(),
    "the panel must draw its table"
  );
  assert!(
    output
      .drawn
      .help
      .contains(&HelpCommand::Help.label().to_owned()),
    "the panel must list the command that opened it: {:?}",
    output.drawn.help
  );
  assert!(
    !joined(&texts).contains("Zoom about the pointer"),
    "the Help chip must not open the shortcuts sheet: {}",
    joined(&texts)
  );
}

#[test]
fn the_shortcuts_chip_still_opens_the_shortcuts_sheet() {
  // The sheet keeps a mouse path of its own, so rewiring Help does not make the keys unreachable.
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let ctx = egui::Context::default();
  let _ = frame_with(&ctx, &mut shell, &mut app);

  let _ = click_palette_chip(&ctx, &mut shell, &mut app, HelpCommand::Shortcuts.label());
  let (_, texts) = frame_with(&ctx, &mut shell, &mut app);
  assert!(
    joined(&texts).contains("Zoom about the pointer"),
    "the Shortcuts chip must open the shortcuts sheet: {}",
    joined(&texts)
  );
  assert!(
    !shell.show_help,
    "the Shortcuts chip must not open the help panel"
  );
}

#[test]
fn the_app_bar_labels_match_the_table() {
  // Three icon-only buttons whose words live in the table now, in the order the bar draws them.
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let (output, _) = frame(&mut shell, &mut app);
  let expected: Vec<String> = [HelpCommand::Undo, HelpCommand::Redo, HelpCommand::FitJob]
    .iter()
    .map(|command| command.label().to_owned())
    .collect();
  assert_eq!(
    output.drawn.app_bar, expected,
    "the app bar must answer to the table's own words"
  );
}

#[test]
fn the_palette_chips_still_arm_toggle_and_run() {
  // PRESERVE (T8.2): the palette is built from the table now, and every chip has to do exactly what it
  // did when the list was hand-written — arm a gesture, flip a view flag, or push the Tools tab's
  // numbers as an action.
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let ctx = egui::Context::default();
  let _ = frame_with(&ctx, &mut shell, &mut app);

  let _ = click_palette_chip(&ctx, &mut shell, &mut app, HelpCommand::Line.label());
  assert_eq!(
    shell.active_tool,
    Tool::Line,
    "a DRAW chip arms its gesture"
  );

  let _ = click_palette_chip(&ctx, &mut shell, &mut app, HelpCommand::Grid.label());
  assert!(!shell.snapping, "the Grid chip toggles snapping off");

  let _ = click_palette_chip(&ctx, &mut shell, &mut app, HelpCommand::Layers.label());
  assert!(
    !shell.inspector_open(),
    "the Layers chip collapses the inspector"
  );

  let (output, _) = click_palette_chip(&ctx, &mut shell, &mut app, HelpCommand::AddText.label());
  match output.actions.as_slice() {
    [UiAction::AddText { text, height, .. }] => {
      assert_eq!(
        text.as_str(),
        "CAM",
        "the text chip reads the Tools tab's words"
      );
      assert_eq!(*height, 10.0, "and its cap height");
    }
    other => panic!("the Add text chip must push one AddText action, got {other:?}"),
  }

  shell.set_stage(Stage::Edit);
  let (output, _) = click_palette_chip(&ctx, &mut shell, &mut app, HelpCommand::Move.label());
  match output.actions.as_slice() {
    [UiAction::Transform(Transform::Translate { delta })] => assert_eq!(
      *delta,
      Vec2::new(10.0, 0.0),
      "the Move chip reads the Tools tab's offset"
    ),
    other => panic!("the Move chip must push one Translate action, got {other:?}"),
  }
}

#[test]
fn the_palette_runs_the_array_grid_its_own_form_can_type() {
  // The grid count is the one number with a range, and only the Tools tab's widget applies it: the field
  // itself can hold anything, and the palette hands those numbers straight to the command. A 0-row array
  // is refused downstream with a sentence, so the chip has to run what its own form can type instead of
  // letting the two disagree.
  //
  // The inspector is on Selection, not Tools, because a visible `DragValue` clamps its own field on the
  // frame it is drawn: with the form on screen the number is already legal and the disagreement is
  // invisible. The Tools tab being hidden is exactly the case a number that no widget guards can reach.
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let ctx = egui::Context::default();
  shell.set_stage(Stage::Edit);
  shell.set_tab(Tab::Selection);
  shell.array_grid = (0, 99);
  let _ = frame_with(&ctx, &mut shell, &mut app);
  assert_eq!(
    shell.array_grid,
    (0, 99),
    "nothing on screen may quietly rewrite the field this test just set"
  );

  let (output, _) = click_palette_chip(&ctx, &mut shell, &mut app, HelpCommand::Array.label());
  match output.actions.as_slice() {
    [UiAction::Array { rows, columns, .. }] => {
      assert_eq!(
        (*rows, *columns),
        (1, 64),
        "the Array chip must run a grid inside the form's range: 1..=64"
      );
    }
    other => panic!("the Array chip must push one Array action, got {other:?}"),
  }
  assert_eq!(
    shell.array_grid,
    (0, 99),
    "the clamp belongs to the action, not to what the operator typed"
  );
}

#[test]
fn the_palette_and_the_tools_tab_build_the_same_seven_actions() {
  // This is the test the fold exists for: the two surfaces that run a parameterised command used to build
  // the payload themselves, and the palette path also skipped the array grid's range. Pinning the exact
  // `UiAction` each route emits — not merely that the two agree — is what makes a route that reads the
  // wrong field visible, so the numbers below are deliberately not the defaults.
  let mut app = AppState::new(document_with_rectangle());
  let mut framing = Shell::new();
  let _ = frame_with(&egui::Context::default(), &mut framing, &mut app);
  // Every `about` and `origin` names the view centre, and the shell frames the drawing on its first frame:
  // the point a payload carries is a measurement of the camera, not the origin a default happens to hold.
  let centre = app.camera().center();
  assert!(
    !centre.is_near(Point2::ORIGIN, 1.0e-9),
    "the payload's point must be the framed view centre, not the origin: {centre:?}"
  );

  let mut numbers = Shell::new();
  numbers.text = "Hi".to_owned();
  numbers.text_height = 7.5;
  numbers.move_delta = (3.5, -2.0);
  numbers.rotate_degrees = 42.0;
  numbers.scale_factors = (1.5, 0.5);
  numbers.array_grid = (3, 4);
  numbers.array_spacing = (12.0, 6.0);
  numbers.offset_distance = 4.25;

  let cases = [
    (
      HelpCommand::AddText,
      Stage::Design,
      UiAction::AddText {
        text: "Hi".to_owned(),
        height: 7.5,
        origin: centre,
      },
    ),
    (
      HelpCommand::Move,
      Stage::Edit,
      UiAction::Transform(Transform::Translate {
        delta: Vec2::new(3.5, -2.0),
      }),
    ),
    (
      HelpCommand::Rotate,
      Stage::Edit,
      UiAction::Transform(Transform::Rotate {
        radians: 42.0_f64.to_radians(),
        about: centre,
      }),
    ),
    (
      HelpCommand::Scale,
      Stage::Edit,
      UiAction::Transform(Transform::Scale {
        sx: 1.5,
        sy: 0.5,
        about: centre,
        tolerance: DEFAULT_TOLERANCE_MM,
      }),
    ),
    (
      HelpCommand::Mirror,
      Stage::Edit,
      UiAction::Transform(Transform::Mirror {
        point: centre,
        direction: Vec2::Y_AXIS,
      }),
    ),
    (
      HelpCommand::Array,
      Stage::Edit,
      UiAction::Array {
        rows: 3,
        columns: 4,
        dx: 12.0,
        dy: 6.0,
      },
    ),
    (
      HelpCommand::Offset,
      Stage::Edit,
      UiAction::Offset { distance: 4.25 },
    ),
  ];

  for (command, stage, expected) in cases {
    let mut palette_shell = numbers.clone();
    palette_shell.set_stage(stage);
    let palette_ctx = egui::Context::default();
    let _ = frame_with(&palette_ctx, &mut palette_shell, &mut app);
    let (from_palette, _) =
      click_palette_chip(&palette_ctx, &mut palette_shell, &mut app, command.label());
    assert_eq!(
      from_palette.actions,
      vec![expected.clone()],
      "the palette's {} chip",
      command.label()
    );

    let mut tab_shell = numbers.clone();
    tab_shell.set_stage(stage);
    let tab_ctx = egui::Context::default();
    let from_tab = click_authoring_button(&tab_ctx, &mut tab_shell, &mut app, command.label());
    assert_eq!(
      from_tab.actions,
      vec![expected],
      "the Tools tab's {} button",
      command.label()
    );
  }
}

#[test]
fn every_palette_chip_is_named_by_its_tooltip() {
  // The T8.2 defect was invisible to every test here: the Shortcuts chip drew the same glyph as Add text
  // and its hover text was a sentence that never said "Shortcuts". A headless frame reads neither the
  // pixels nor the hover card, so the closest thing it can assert is that the words a chip shows on hover
  // contain the word the chip itself stands for.
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let ctx = egui::Context::default();

  for stage in Stage::ALL {
    shell.set_stage(stage);
    let (output, _) = frame_with(&ctx, &mut shell, &mut app);
    for word in &output.drawn.palette {
      let command = HelpCommand::ALL
        .iter()
        .copied()
        .find(|command| command.label() == word)
        .unwrap_or_else(|| panic!("{word:?} is drawn on the palette but no command answers to it"));
      let tooltip = cam_ui::chrome::palette_tooltip(command);
      assert!(
        tooltip.contains(word),
        "the {word:?} chip's tooltip is {tooltip:?}, which never names the chip"
      );
    }
  }
}

/// A document with one closed rectangle on one layer: Design's fact, and no toolpath yet.
fn document_with_one_vector() -> Document {
  document_with_rectangle()
}

/// The sentence a not-ready stage owes, as it is written down: the bar, the hover and the Job tab all
/// read it from one place, so the tests name it once.
const NOTHING_DRAWN: &str = "Nothing drawn yet";
const NOTHING_TO_CUT: &str = "Nothing to cut yet";

#[test]
fn the_job_tab_names_the_stage_in_view_and_never_another_one() {
  // The defect T13 fixes: the Job section printed `Stage::Toolpaths.note()` whatever stage was showing,
  // so on the four other stages the section talked about work the operator was not doing. The section
  // may only say what is true for the stage in view, and nothing at all when that stage owes nothing.
  let mut app = AppState::new(document_with_one_vector());
  let mut shell = Shell::new();
  shell.set_tab(Tab::Job);
  let (_, texts) = frame(&mut shell, &mut app);
  let all = joined(&texts);
  assert!(
    !all.contains("cam-cli toolpath"),
    "the Job tab names Toolpaths while Design is showing: {all}"
  );
  assert!(
    !all.contains(NOTHING_TO_CUT),
    "Design is ready with one vector, so the section owes no sentence: {all}"
  );

  // Toolpaths is showing and the document holds none, so the section names the missing step.
  shell.set_stage(Stage::Toolpaths);
  let (_, texts) = frame(&mut shell, &mut app);
  let all = joined(&texts);
  assert!(
    all.contains(NOTHING_TO_CUT),
    "the Job tab must say what Toolpaths is missing: {all}"
  );

  // An empty sheet on Design: the same rule, the other countable stage.
  let mut empty = AppState::new(Document::new(Job::default_sheet()));
  let mut shell = Shell::new();
  shell.set_tab(Tab::Job);
  let (_, texts) = frame(&mut shell, &mut empty);
  let all = joined(&texts);
  assert!(
    all.contains(NOTHING_DRAWN),
    "the Job tab must say what Design is missing: {all}"
  );

  // A stage nothing measures claims nothing: no sentence is true here, so none is printed.
  let mut unmeasured = AppState::new(Document::new(Job::default_sheet()));
  let mut shell = Shell::new();
  shell.set_stage(Stage::Edit);
  shell.set_tab(Tab::Job);
  let (_, texts) = frame(&mut shell, &mut unmeasured);
  let all = joined(&texts);
  assert!(
    !all.contains(NOTHING_DRAWN) && !all.contains(NOTHING_TO_CUT),
    "an unmeasured stage must not name a missing step: {all}"
  );
}

/// A document holding `count` toolpaths over the same one vector.
fn document_with_toolpaths(count: usize) -> Document {
  let mut document = document_with_one_vector();
  for index in 0..count {
    document.toolpaths.push(cam_model::Toolpath {
      id: format!("TP{}", index + 1),
      name: format!("Profile {}", index + 1),
      entity_ids: vec!["E1".to_owned()],
      island_ids: Vec::new(),
      tool_number: 1,
      operation: cam_model::Operation::Profile(cam_model::ProfileParams::default()),
    });
  }
  document
}

#[test]
fn design_is_ready_at_one_vector_and_not_before() {
  let empty = AppState::new(Document::new(Job::default_sheet()));
  assert!(
    matches!(
      Stage::Design.readiness(&empty),
      Readiness::NotReady(step) if step.starts_with(NOTHING_DRAWN)
    ),
    "an empty sheet is not a design, and the answer must name the step: {:?}",
    Stage::Design.readiness(&empty)
  );

  let drawn = AppState::new(document_with_one_vector());
  assert_eq!(
    Stage::Design.readiness(&drawn),
    Readiness::Ready(1),
    "one vector on the sheet is the whole rule for Design"
  );
}

#[test]
fn toolpaths_is_ready_at_one_toolpath_and_not_before() {
  let none = AppState::new(document_with_one_vector());
  assert!(
    matches!(
      Stage::Toolpaths.readiness(&none),
      Readiness::NotReady(step) if step.starts_with(NOTHING_TO_CUT)
    ),
    "a document with no toolpath cannot be a finished Toolpaths stage: {:?}",
    Stage::Toolpaths.readiness(&none)
  );

  let one = AppState::new(document_with_toolpaths(1));
  assert_eq!(
    Stage::Toolpaths.readiness(&one),
    Readiness::Ready(1),
    "one toolpath in the document is the whole rule for Toolpaths"
  );
}

#[test]
fn the_three_unmeasured_stages_claim_neither_state() {
  // Nothing in `AppState` measures Edit, Relief or Prove: no self-intersection report, no relief and no
  // simulation result reaches it. So they must answer `Unmeasured` for every document, never "ready" and
  // never "not ready" — a false `false` is exactly what the three-way answer exists to prevent (S9).
  let documents = [
    Document::new(Job::default_sheet()),
    document_with_one_vector(),
    document_with_toolpaths(1),
  ];
  for document in documents {
    let app = AppState::new(document);
    for stage in [Stage::Edit, Stage::Relief, Stage::Prove] {
      assert_eq!(
        stage.readiness(&app),
        Readiness::Unmeasured,
        "{stage:?} claims a state nothing measures"
      );
    }
  }
}

#[test]
fn every_refusal_names_the_number_it_counts() {
  // S4's third rule, and the one a test can hold: a refusal names a number. Both sentences say `0`, which
  // is the count the bar shows beside them — one answer, read twice.
  for (stage, fact) in [
    (Stage::Design, NOTHING_DRAWN),
    (Stage::Toolpaths, NOTHING_TO_CUT),
  ] {
    let app = AppState::new(Document::new(Job::default_sheet()));
    match stage.readiness(&app) {
      Readiness::NotReady(step) => {
        assert!(step.starts_with(fact), "{stage:?} says {step:?}");
        assert!(
          step.contains('0'),
          "{stage:?} refuses without naming the number it counted: {step:?}"
        );
      }
      other => panic!("{stage:?} on an empty document answered {other:?}"),
    }
  }
}

#[test]
fn the_number_on_the_bar_and_the_readiness_are_one_answer() {
  // The rule that makes this true is structural — `Stage::count` renders `Stage::readiness` — so the test
  // reads both through the frame: the painted chip must be the number the readiness carries, and a stage
  // that is not ready must show `0`, never a number that contradicts the sentence.
  for (name, document) in [
    ("empty", Document::new(Job::default_sheet())),
    ("one", document_with_one_vector()),
    ("three", document_with_toolpaths(3)),
  ] {
    let mut app = AppState::new(document);
    let mut shell = Shell::new();
    let (_, texts) = frame(&mut shell, &mut app);
    let all = joined(&texts);
    for stage in Stage::ALL {
      let chip = match stage.readiness(&app) {
        Readiness::Ready(count) => format!("{}  {count}", stage.label()),
        Readiness::NotReady(_) => format!("{}  0", stage.label()),
        Readiness::Unmeasured => format!("{}  —", stage.label()),
      };
      assert!(
        all.contains(&chip),
        "on the {name} document the bar must print {chip:?}: {all}"
      );
    }
  }
}

#[test]
fn a_stage_keeps_its_own_sentence_and_leads_with_the_missing_step() {
  // The chip's hover text is existing behaviour: each stage explains what it asks of the operator. T13
  // touches that string, so the previous reading is pinned — a ready stage and an unmeasured one still
  // show exactly their note — and the new one is pinned beside it: a not-ready stage says what is
  // missing first and how the work is done second (consequence before mechanics, S4).
  let drawn = AppState::new(document_with_one_vector());
  assert_eq!(
    Stage::Design.hover(&drawn),
    Stage::Design.note(),
    "a ready stage has nothing missing to announce"
  );
  assert_eq!(
    Stage::Edit.hover(&drawn),
    Stage::Edit.note(),
    "an unmeasured stage must not announce a missing step either"
  );

  let empty = AppState::new(Document::new(Job::default_sheet()));
  let hover = Stage::Design.hover(&empty);
  assert!(
    hover.starts_with(NOTHING_DRAWN),
    "the missing step leads the hover: {hover:?}"
  );
  assert!(
    hover.ends_with(Stage::Design.note()),
    "and the stage's own sentence is still there: {hover:?}"
  );
}
