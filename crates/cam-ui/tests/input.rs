//! Headless tests for pointer, wheel and keyboard wiring (A8, A9).
//!
//! Every test drives the real shell through synthetic `egui` events: press, release, drag, wheel and
//! keys. Nothing here needs a display.

use cam_app::AppState;
use cam_geom::{Contour, Point2};
use cam_model::{Document, Job, Layer};
use cam_ui::{handle_action, Shell, ShellOutput};

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

fn document() -> Document {
  let mut document = Document::new(Job::default_sheet());
  document.layers.push(Layer {
    id: "L1".to_owned(),
    name: "Part".to_owned(),
    visible: true,
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
      let _ = handle_action(&mut self.app, &action);
    }
  }

  /// Convert a world position into a window position through the live camera.
  fn at(&self, world: Point2) -> egui::Pos2 {
    let screen = self.app.camera().world_to_screen(world);
    egui::Pos2::new(
      (screen.x + self.output.canvas.x) as f32,
      (screen.y + self.output.canvas.y) as f32,
    )
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
  // The shell frames the drawing, so the visible area is roughly 30 mm around the 100x60 rectangle.
  // This point is inside that view and well clear of every edge.
  let position = harness.at(Point2::new(-10.0, 90.0));
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
  let from = harness.at(Point2::new(-12.0, -30.0));
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
