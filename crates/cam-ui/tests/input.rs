//! Headless tests for pointer, wheel and keyboard wiring (A8, A9).
//!
//! Every test drives the real shell through synthetic `egui` events: press, release, drag, wheel and
//! keys. Nothing here needs a display.

use cam_app::AppState;
use cam_geom::{Contour, Point2};
use cam_model::{Document, Job, Layer, LayerRole};
use cam_ui::help::Command as HelpCommand;
use cam_ui::{handle_action, DrawShape, Shell, ShellOutput, Tool, UiAction};

fn rectangle(x: f64, y: f64, width: f64, height: f64) -> Contour {
  Contour::from_points(
    &[
      Point2::new(x, y),
      Point2::new(x + width, y),
      Point2::new(x + width, y + height),
      Point2::new(x, y + height),
    ],
    true,
  )
  .expect("rectangle")
}

/// How closely a dragged world size can be recovered, in millimetres.
///
/// A window position is an `f32` pixel, so a world point that makes the round trip
/// world -> pixel -> world comes back with a few nanometres of error. A thousandth of a millimetre is
/// a safe bound and is still four orders of magnitude below a pixel at any usable zoom.
const GESTURE_TOLERANCE_MM: f64 = 1.0e-3;

fn document() -> Document {
  let mut document = Document::new(Job::default_sheet());
  document.layers.push(Layer {
    id: "L1".to_owned(),
    role: LayerRole::Outline, /*was Geometry*/
    side: cam_model::Side::Both,
    color: "#000000".to_owned(),
    visible: true,
    source: cam_model::Provenance::default(),
    entities: vec![cam_model::Entity {
      id: "E1".to_owned(),
      kind: cam_model::EntityKind::Contour {
        contour: rectangle(0.0, 0.0, 100.0, 60.0),
      },
    }],
  });
  document
}

fn raw_input(events: Vec<egui::Event>) -> egui::RawInput {
  // egui tracks the modifier state through `ModifiersChanged`, the way a real backend reports it; the
  // copy inside `Event::Key` is not enough on its own.
  let modifiers = events
    .iter()
    .find_map(|event| match event {
      egui::Event::Key { modifiers, .. } => Some(*modifiers),
      _ => None,
    })
    .unwrap_or_default();
  let mut all_events = vec![egui::Event::ModifiersChanged(modifiers)];
  all_events.extend(events);
  egui::RawInput {
    screen_rect: Some(egui::Rect::from_min_size(
      egui::Pos2::ZERO,
      egui::vec2(1200.0, 800.0),
    )),
    events: all_events,
    ..Default::default()
  }
}

/// A harness that keeps one egui context alive across frames, like a real window.
struct Harness {
  ctx: egui::Context,
  shell: Shell,
  app: AppState,
  output: ShellOutput,
}

impl Harness {
  fn new() -> Self {
    let mut harness = Self {
      ctx: egui::Context::default(),
      shell: Shell::new(),
      app: AppState::new(document()),
      output: ShellOutput::default(),
    };
    // A few quiet frames: the first lays the panels out, the next settles their width, and the shell
    // then frames the document. Only after that are world-to-screen positions stable.
    for _ in 0..3 {
      harness.frame(Vec::new());
    }
    harness
  }

  fn frame(&mut self, events: Vec<egui::Event>) {
    let mut captured: Option<ShellOutput> = None;
    let full = self.ctx.run_pass(raw_input(events), |ctx| {
      captured = Some(self.shell.show(&mut self.app, ctx));
    });
    full.drop_without_applying_deltas();
    self.output = captured.expect("the shell ran");
    let actions = self.output.actions.clone();
    for action in actions {
      // The host applies what the shell returned; going through the shell's own entry point is what
      // keeps the status bar in the loop, which the authoring tests assert on.
      self.shell.apply_action(&mut self.app, &action);
    }
  }

  /// Press at `from`, move to `to`, release: one complete drag gesture.
  fn drag(&mut self, from: egui::Pos2, to: egui::Pos2) {
    let modifiers = egui::Modifiers::default();
    self.frame(vec![
      egui::Event::PointerMoved(from),
      egui::Event::PointerButton {
        pos: from,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers,
      },
    ]);
    self.frame(vec![egui::Event::PointerMoved(to)]);
    self.frame(vec![egui::Event::PointerButton {
      pos: to,
      button: egui::PointerButton::Primary,
      pressed: false,
      modifiers,
    }]);
  }

  /// Convert a world position into a window position through the live camera.
  ///
  /// Asserts the result really lands on the drawing surface: the shell's chrome takes the frame's
  /// border, and a click that misses the canvas is swallowed by a rail or a bar. Without this an
  /// innocent layout change turns every canvas test into a silent pass or a mystery failure.
  fn at(&self, world: Point2) -> egui::Pos2 {
    let screen = self.app.camera().world_to_screen(world);
    let position = egui::Pos2::new(
      (screen.x + self.output.canvas.x) as f32,
      (screen.y + self.output.canvas.y) as f32,
    );
    let canvas = &self.output.canvas;
    let bounds = egui::Rect::from_min_size(
      egui::Pos2::new(canvas.x as f32, canvas.y as f32),
      egui::vec2(canvas.width as f32, canvas.height as f32),
    );
    assert!(
      bounds.contains(position),
      "{world:?} maps to {position:?}, outside the canvas {bounds:?}"
    );
    position
  }

  fn click(&mut self, position: egui::Pos2) {
    let modifiers = egui::Modifiers::default();
    self.frame(vec![
      egui::Event::PointerMoved(position),
      egui::Event::PointerButton {
        pos: position,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers,
      },
    ]);
    self.frame(vec![egui::Event::PointerButton {
      pos: position,
      button: egui::PointerButton::Primary,
      pressed: false,
      modifiers,
    }]);
  }

  /// Right-click at one window point: move there, then press and release the secondary button.
  ///
  /// A right-click is the two frames a left click is, with the other button, and this is the harness's
  /// only secondary gesture because the context menu is the shell's only secondary command.
  fn right_click(&mut self, position: egui::Pos2) {
    let modifiers = egui::Modifiers::default();
    self.frame(vec![
      egui::Event::PointerMoved(position),
      egui::Event::PointerButton {
        pos: position,
        button: egui::PointerButton::Secondary,
        pressed: true,
        modifiers,
      },
    ]);
    self.frame(vec![egui::Event::PointerButton {
      pos: position,
      button: egui::PointerButton::Secondary,
      pressed: false,
      modifiers,
    }]);
  }

  /// One quiet frame, so a menu opened by the last click has drawn its entries.
  fn settle(&mut self) {
    self.frame(Vec::new());
  }

  fn key(&mut self, key: egui::Key, modifiers: egui::Modifiers) {
    self.frame(vec![egui::Event::Key {
      key,
      physical_key: None,
      pressed: true,
      repeat: false,
      modifiers,
    }]);
    self.frame(vec![egui::Event::Key {
      key,
      physical_key: None,
      pressed: false,
      repeat: false,
      modifiers,
    }]);
  }
}

#[test]
fn clicking_an_edge_selects_its_entity() {
  let mut harness = Harness::new();
  let position = harness.at(Point2::new(50.0, 0.0));
  harness.click(position);
  assert_eq!(
    harness.app.selection().ids().collect::<Vec<_>>(),
    vec!["E1"],
    "a click on the bottom edge must select E1"
  );
}

#[test]
fn clicking_empty_space_clears_the_selection() {
  let mut harness = Harness::new();
  harness.app.select(vec!["E1".to_owned()]);
  harness.frame(Vec::new());
  // The shell frames the drawing, so the visible area reaches well past the 100x60 rectangle. This
  // point is inside that view and clear of every edge — but near it, because the chrome moved the
  // canvas and a point 90 mm up is no longer on screen.
  let position = harness.at(Point2::new(-8.0, 30.0));
  harness.click(position);
  assert!(
    harness.app.selection().is_empty(),
    "a click on nothing must clear the selection"
  );
}

#[test]
fn dragging_a_box_selects_what_it_touches() {
  let mut harness = Harness::new();
  // A box whose edges cross the rectangle's left edge (x = 0) and bottom edge (y = 0).
  let from = harness.at(Point2::new(-8.0, -8.0));
  let to = harness.at(Point2::new(30.0, 30.0));
  let modifiers = egui::Modifiers::default();

  harness.frame(vec![
    egui::Event::PointerMoved(from),
    egui::Event::PointerButton {
      pos: from,
      button: egui::PointerButton::Primary,
      pressed: true,
      modifiers,
    },
  ]);
  harness.frame(vec![egui::Event::PointerMoved(to)]);
  harness.frame(vec![egui::Event::PointerButton {
    pos: to,
    button: egui::PointerButton::Primary,
    pressed: false,
    modifiers,
  }]);

  assert_eq!(
    harness.app.selection().ids().collect::<Vec<_>>(),
    vec!["E1"],
    "the marquee crosses E1's left and bottom edges"
  );
}

#[test]
fn dragging_with_the_middle_button_pans_the_view() {
  let mut harness = Harness::new();
  let before = harness.app.camera().center();
  let from = harness.at(Point2::new(0.0, 0.0));
  let to = egui::Pos2::new(from.x + 60.0, from.y + 30.0);
  let modifiers = egui::Modifiers::default();

  harness.frame(vec![
    egui::Event::PointerMoved(from),
    egui::Event::PointerButton {
      pos: from,
      button: egui::PointerButton::Middle,
      pressed: true,
      modifiers,
    },
  ]);
  harness.frame(vec![egui::Event::PointerMoved(to)]);
  harness.frame(vec![egui::Event::PointerButton {
    pos: to,
    button: egui::PointerButton::Middle,
    pressed: false,
    modifiers,
  }]);

  let after = harness.app.camera().center();
  assert!(
    after.x < before.x,
    "dragging right must move the view left: {:?} -> {:?}",
    before,
    after
  );
}

#[test]
fn the_wheel_zooms_about_the_cursor() {
  let mut harness = Harness::new();
  let before = harness.app.camera().pixels_per_mm();
  let cursor = harness.at(Point2::new(30.0, 30.0));
  harness.frame(vec![
    egui::Event::PointerMoved(cursor),
    egui::Event::MouseWheel {
      unit: egui::MouseWheelUnit::Point,
      delta: egui::vec2(0.0, 120.0),
      phase: egui::TouchPhase::Move,
      source: egui::MouseWheelSource::Wheel,
      modifiers: egui::Modifiers::default(),
    },
  ]);
  assert!(
    harness.app.camera().pixels_per_mm() > before,
    "scrolling up must zoom in: {} -> {}",
    before,
    harness.app.camera().pixels_per_mm()
  );
}

#[test]
fn control_z_undoes_and_control_shift_z_redoes() {
  let mut harness = Harness::new();
  handle_action(
    &mut harness.app,
    &cam_ui::UiAction::AddRectangle {
      width: 10.0,
      height: 10.0,
    },
  )
  .expect("added");
  harness.frame(Vec::new());
  assert_eq!(harness.app.document().entities().count(), 2);

  harness.key(
    egui::Key::Z,
    egui::Modifiers {
      command: true,
      ..Default::default()
    },
  );
  assert_eq!(
    harness.app.document().entities().count(),
    1,
    "Ctrl+Z must undo the drawing"
  );

  harness.key(
    egui::Key::Z,
    egui::Modifiers {
      command: true,
      shift: true,
      ..Default::default()
    },
  );
  assert_eq!(
    harness.app.document().entities().count(),
    2,
    "Ctrl+Shift+Z must redo it"
  );
}

#[test]
fn the_delete_key_removes_the_selection() {
  let mut harness = Harness::new();
  harness.app.select(vec!["E1".to_owned()]);
  harness.frame(Vec::new());
  harness.key(egui::Key::Delete, egui::Modifiers::default());
  assert_eq!(harness.app.document().entities().count(), 0);
}

#[test]
fn f_frames_the_drawing() {
  let mut harness = Harness::new();
  harness
    .app
    .camera_mut()
    .zoom_at(cam_app::ScreenPoint::new(0.0, 0.0), 30.0);
  let zoomed = harness.app.camera().pixels_per_mm();
  harness.frame(Vec::new());
  harness.key(egui::Key::F, egui::Modifiers::default());
  assert!(
    harness.app.camera().pixels_per_mm() < zoomed,
    "F must frame the job"
  );
}

#[test]
fn hovering_a_corner_reports_a_snap_marker_and_the_status_readout() {
  let mut harness = Harness::new();
  let corner = harness.at(Point2::new(0.0, 0.0));
  harness.frame(vec![egui::Event::PointerMoved(corner)]);
  let marker = harness
    .shell
    .snap_marker
    .expect("a corner must offer a snap point");
  assert!(marker.1.is_near(Point2::ORIGIN, 1.0e-6), "{marker:?}");
  let world = harness.shell.cursor_world.expect("cursor world position");
  assert!(world.is_near(Point2::ORIGIN, 1.0e-6), "{world:?}");
  assert!(harness.output.drawn.status_line.contains("X"));
}

#[test]
fn a_rectangle_drag_draws_one_command_of_the_dragged_world_size() {
  let mut harness = Harness::new();
  // Snapping has its own tests; this one is about the gesture's geometry, so it asks for the world
  // points the cursor reports rather than the snapped ones.
  harness.shell.snapping = false;
  harness.shell.active_tool = Tool::Rectangle;
  harness.frame(Vec::new());

  let from = harness.at(Point2::new(10.0, 10.0));
  let to = harness.at(Point2::new(90.0, 50.0));
  harness.drag(from, to);

  assert_eq!(
    harness.output.actions.len(),
    1,
    "a press-move-release is exactly one command, got {:?}",
    harness.output.actions
  );
  let UiAction::Draw(DrawShape::Rectangle { at, width, height }) = &harness.output.actions[0]
  else {
    panic!(
      "the rectangle tool must draw a rectangle, got {:?}",
      harness.output.actions
    );
  };
  assert!(
    at.is_near(Point2::new(10.0, 10.0), GESTURE_TOLERANCE_MM),
    "{at:?}"
  );
  assert!(
    (width - 80.0).abs() < GESTURE_TOLERANCE_MM,
    "dragged width must be 80 mm, got {width}"
  );
  assert!(
    (height - 40.0).abs() < GESTURE_TOLERANCE_MM,
    "dragged height must be 40 mm, got {height}"
  );

  assert_eq!(
    harness.app.document().entities().count(),
    2,
    "the drag must add exactly one entity: {}",
    harness.shell.status
  );
  assert_eq!(
    harness.app.history_depth(),
    1,
    "one drag is one undo step: {}",
    harness.shell.status
  );
  let drawn = harness
    .app
    .selection()
    .ids()
    .next()
    .expect("the drawn rectangle is selected")
    .to_owned();
  let bounds = harness
    .app
    .document()
    .entity(&drawn)
    .and_then(|entity| entity.kind.as_contour())
    .expect("a contour")
    .bounds();
  assert!(
    bounds
      .min
      .is_near(Point2::new(10.0, 10.0), GESTURE_TOLERANCE_MM)
      && bounds
        .max
        .is_near(Point2::new(90.0, 50.0), GESTURE_TOLERANCE_MM),
    "the stored rectangle must be the dragged one, got {bounds:?}"
  );

  harness.key(
    egui::Key::Z,
    egui::Modifiers {
      command: true,
      ..Default::default()
    },
  );
  assert_eq!(
    harness.app.document().entities().count(),
    1,
    "one undo removes the dragged rectangle"
  );
}

#[test]
fn a_line_drag_draws_one_open_line_even_when_it_is_horizontal() {
  let mut harness = Harness::new();
  harness.shell.snapping = false;
  harness.shell.active_tool = Tool::Line;
  harness.frame(Vec::new());

  let from = harness.at(Point2::new(10.0, 10.0));
  let to = harness.at(Point2::new(90.0, 10.0));
  harness.drag(from, to);

  assert_eq!(
    harness.output.actions.len(),
    1,
    "{:?}",
    harness.output.actions
  );
  let UiAction::Draw(DrawShape::Line { from, to }) = &harness.output.actions[0] else {
    panic!(
      "the line tool must draw a line, got {:?}",
      harness.output.actions
    );
  };
  assert!(
    from.is_near(Point2::new(10.0, 10.0), GESTURE_TOLERANCE_MM),
    "{from:?}"
  );
  assert!(
    to.is_near(Point2::new(90.0, 10.0), GESTURE_TOLERANCE_MM),
    "{to:?}"
  );

  let drawn = harness
    .app
    .selection()
    .ids()
    .next()
    .expect("the drawn line is selected")
    .to_owned();
  let contour = harness
    .app
    .document()
    .entity(&drawn)
    .and_then(|entity| entity.kind.as_contour())
    .expect("a contour");
  assert!(!contour.is_closed(), "a line stays open");
  assert!(
    (contour.perimeter() - 80.0).abs() < GESTURE_TOLERANCE_MM,
    "the line must be the dragged length, got {}",
    contour.perimeter()
  );
}

#[test]
fn a_press_and_release_without_movement_draws_nothing() {
  let mut harness = Harness::new();
  harness.shell.snapping = false;
  harness.shell.active_tool = Tool::Rectangle;
  harness.frame(Vec::new());
  let depth = harness.app.history_depth();

  let position = harness.at(Point2::new(10.0, 10.0));
  harness.click(position);

  assert!(
    harness.output.actions.is_empty(),
    "a click with a drawing tool armed must not submit a shape, got {:?}",
    harness.output.actions
  );
  assert_eq!(
    harness.app.document().entities().count(),
    1,
    "no entity may be drawn"
  );
  assert_eq!(
    harness.app.history_depth(),
    depth,
    "a cancelled gesture must not grow the undo history"
  );
}

#[test]
fn zoom_and_pan_are_not_written_into_the_undo_history() {
  let mut harness = Harness::new();
  let depth = harness.app.history_depth();
  let cursor = harness.at(Point2::new(10.0, 10.0));
  harness.frame(vec![
    egui::Event::PointerMoved(cursor),
    egui::Event::MouseWheel {
      unit: egui::MouseWheelUnit::Point,
      delta: egui::vec2(0.0, 200.0),
      phase: egui::TouchPhase::Move,
      source: egui::MouseWheelSource::Wheel,
      modifiers: egui::Modifiers::default(),
    },
  ]);
  assert_eq!(
    harness.app.history_depth(),
    depth,
    "moving the view is not an edit"
  );
  assert!(!harness.app.is_dirty(), "nor does it dirty the document");
}

#[test]
fn a_rectangle_drag_along_one_axis_is_cancelled_with_a_hint() {
  let mut harness = Harness::new();
  harness.shell.snapping = false;
  harness.shell.active_tool = Tool::Rectangle;
  harness.frame(Vec::new());
  let depth = harness.app.history_depth();

  // A horizontal drag gives the rectangle no height: a zero-size shape is cancelled, not submitted.
  let from = harness.at(Point2::new(10.0, 10.0));
  let to = harness.at(Point2::new(90.0, 10.0));
  harness.drag(from, to);

  assert!(
    harness.output.actions.is_empty(),
    "a drag with no height must not submit a shape, got {:?}",
    harness.output.actions
  );
  assert_eq!(harness.app.document().entities().count(), 1);
  assert_eq!(harness.app.history_depth(), depth);
  assert!(
    harness.shell.status.contains("drag in both directions"),
    "the status bar must say why nothing was drawn: {}",
    harness.shell.status
  );
}

// ---- T8.3: the canvas context menu ------------------------------------------------------------

/// The point the context menu drew one entry at.
///
/// The menu reports where it drew each word ([`cam_ui::DrawnState::menu_hits`]), because egui cannot be
/// asked for a widget by its text from outside a frame and a test that guessed at the popup's layout
/// would break the day a margin moved. This is the same surface the palette reports its chips on.
fn menu_hit(harness: &Harness, label: &str) -> egui::Pos2 {
  harness
    .output
    .drawn
    .menu_hits
    .iter()
    .find(|(word, _)| word == label)
    .map(|(_, point)| egui::pos2(point.0, point.1))
    .unwrap_or_else(|| {
      panic!(
        "no {label:?} menu entry: drew {:?}",
        harness.output.drawn.menu
      )
    })
}

#[test]
fn right_clicking_an_entity_offers_its_commands() {
  let mut harness = Harness::new();
  // The bottom edge of E1: over an entity, and not its interior, which is what picking measures.
  let position = harness.at(Point2::new(50.0, 0.0));
  harness.right_click(position);
  harness.settle();

  let menu = &harness.output.drawn.menu;
  assert!(
    menu.iter().any(|entry| entry == "Move"),
    "an object menu must offer the transform family: {menu:?}"
  );
  assert!(
    menu.iter().any(|entry| entry == "Delete selection"),
    "and the command that removes what was pointed at: {menu:?}"
  );
  assert!(
    !menu.iter().any(|entry| entry == "Add rectangle"),
    "an object menu is not the canvas menu: {menu:?}"
  );
}

#[test]
fn right_clicking_empty_canvas_offers_canvas_commands() {
  let mut harness = Harness::new();
  // Clear of every edge, as in the click-to-clear test: the chrome leaves this much view around a
  // 100 x 60 mm rectangle.
  let position = harness.at(Point2::new(-8.0, 30.0));
  harness.right_click(position);
  harness.settle();

  let menu = &harness.output.drawn.menu;
  assert!(
    menu.iter().any(|entry| entry == "Add rectangle"),
    "the canvas menu must offer the typed rectangle: {menu:?}"
  );
  // The brief calls this entry "Zoom to fit"; the shell already has one command for it and its word
  // is `Fit the job`, so the menu draws the command's own label rather than a second name for it.
  assert!(
    menu.iter().any(|entry| entry == "Fit the job"),
    "and the command that frames the drawing: {menu:?}"
  );
  assert!(
    !menu.iter().any(|entry| entry == "Delete selection"),
    "empty canvas has nothing to delete: {menu:?}"
  );
}

#[test]
fn right_clicking_an_unselected_entity_selects_it() {
  let mut harness = Harness::new();
  assert!(
    harness.app.selection().is_empty(),
    "nothing starts selected"
  );

  let position = harness.at(Point2::new(50.0, 0.0));
  harness.right_click(position);
  harness.settle();

  assert_eq!(
    harness.app.selection().ids().collect::<Vec<_>>(),
    vec!["E1"],
    "pointing at E1 and right-clicking must select it, so the picked command acts on it"
  );
}

#[test]
fn a_menu_entry_runs_its_command() {
  let mut harness = Harness::new();
  let position = harness.at(Point2::new(50.0, 0.0));
  harness.right_click(position);
  harness.settle();

  let entry = menu_hit(&harness, "Delete selection");
  harness.click(entry);

  assert_eq!(
    harness.app.document().entities().count(),
    0,
    "pressing the menu's Delete selection must remove the entity it was opened on: {}",
    harness.shell.status
  );
  assert_eq!(
    harness.app.history_depth(),
    1,
    "and be one undo step, like the same command from any other place"
  );
}

#[test]
fn the_menu_entries_all_have_help() {
  // Both scopes, because a menu that offers a word the table does not know is a menu the `F1` panel
  // cannot explain.
  for position in [Point2::new(50.0, 0.0), Point2::new(-8.0, 30.0)] {
    let mut harness = Harness::new();
    let screen = harness.at(position);
    harness.right_click(screen);
    harness.settle();

    let menu = harness.output.drawn.menu.clone();
    assert!(
      !menu.is_empty(),
      "a menu over {position:?} must offer something"
    );
    for entry in menu {
      let command = HelpCommand::ALL
        .iter()
        .copied()
        .find(|command| command.label() == entry)
        .unwrap_or_else(|| panic!("{entry:?} is not a command in the table"));
      assert!(
        !command.help().what.trim().is_empty(),
        "{entry:?} has no consequence for the help panel to state"
      );
    }
  }
}

#[test]
fn the_trim_row_is_dark_until_two_vectors_are_selected() {
  let mut harness = Harness::new();
  let position = harness.at(Point2::new(50.0, 0.0));
  harness.right_click(position);
  harness.settle();

  // One vector, so a trim has a target and no cutter: the row is offered, dark, and its `how` says why.
  assert_eq!(harness.app.selection().len(), 1);
  let trim = menu_hit(&harness, "Trim");
  harness.click(trim);

  assert!(
    !harness
      .output
      .actions
      .iter()
      .any(|action| matches!(action, UiAction::Trim { .. })),
    "a dark row must not push the command it cannot run: {:?}",
    harness.output.actions
  );
  assert_eq!(
    harness.app.selection().len(),
    1,
    "and pressing it must not fall through to the canvas and clear the selection"
  );
}

#[test]
fn the_trim_row_cuts_the_lower_id_with_the_other_one() {
  let mut harness = Harness::new();
  // A second vector, then both selected by hand, so the row has its two operands.
  handle_action(
    &mut harness.app,
    &UiAction::AddRectangle {
      width: 10.0,
      height: 10.0,
    },
  )
  .expect("added");
  harness.app.select(vec!["E1".to_owned(), "E2".to_owned()]);
  harness.settle();
  assert_eq!(harness.app.selection().len(), 2);

  // E1 is already selected, so the right-click keeps the pair and the menu is scoped to the object under it.
  let position = harness.at(Point2::new(50.0, 0.0));
  harness.right_click(position);
  harness.settle();
  let trim = menu_hit(&harness, "Trim");
  harness.click(trim);

  assert!(
    harness.output.actions.iter().any(|action| matches!(
      action,
      UiAction::Trim { target, cutter } if target == "E1" && cutter == "E2"
    )),
    "the selection is ordered by id, so E1 is the target and E2 the cutter: {:?}",
    harness.output.actions
  );
}

#[test]
fn a_canvas_menu_row_runs_its_command_and_the_menu_closes() {
  let mut harness = Harness::new();
  let position = harness.at(Point2::new(-8.0, 30.0));
  harness.right_click(position);
  harness.settle();
  assert!(harness.shell.snapping, "snapping starts armed");

  let grid = menu_hit(&harness, "Grid");
  harness.click(grid);

  assert!(
    !harness.shell.snapping,
    "the canvas menu's Grid row must switch snapping off, like the palette chip does"
  );
  harness.settle();
  assert!(
    harness.output.drawn.menu.is_empty(),
    "one row runs one command and the menu closes: {:?}",
    harness.output.drawn.menu
  );
}

#[test]
fn a_left_click_opens_no_menu() {
  let mut harness = Harness::new();
  let position = harness.at(Point2::new(50.0, 0.0));
  harness.click(position);
  harness.settle();

  assert!(
    harness.output.drawn.menu.is_empty(),
    "the menu is the secondary button's, and a left click still just selects: {:?}",
    harness.output.drawn.menu
  );
  assert_eq!(
    harness.app.selection().ids().collect::<Vec<_>>(),
    vec!["E1"],
    "a left click on an edge still picks it"
  );
}
