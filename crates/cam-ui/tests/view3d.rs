//! Headless tests for the 3D preview panel (M5 A9, A11, A12).
//!
//! No display and no GPU: `egui::Context` runs a full frame on the CPU, the panel reports how many
//! triangles and 3D lines it painted, and the shape list says whether the surface really reached the
//! painter as an `egui::Mesh` (A11). The panel is additive (A12): while it is open the 2D canvas keeps
//! painting its own items, and closing it restores the M1–M3 picture exactly.

use cam_app::AppState;
use cam_geom::{Contour, Point2};
use cam_model::{Document, Job, Layer};
use cam_move::{Move, MoveStream, Point3, Segment};
use cam_sim::{sweep, Heightfield};
use cam_ui::{Shell, ShellOutput};
use cam_units::Length;

/// The stock the fixture simulates, in millimetres.
const STOCK_WIDTH: f64 = 100.0;
/// Stock depth along Y, in millimetres.
const STOCK_HEIGHT: f64 = 80.0;
/// Stock thickness, in millimetres.
const STOCK_THICKNESS: f64 = 18.0;
/// The simulation's cell size for the fixture, in millimetres.
const CELL_MM: f64 = 2.0;
/// The cut depth the fixture's program reaches, in millimetres.
const CUT_DEPTH: f64 = 6.0;

fn job() -> Job {
  Job {
    width: Length::from_mm(STOCK_WIDTH),
    height: Length::from_mm(STOCK_HEIGHT),
    thickness: Length::from_mm(STOCK_THICKNESS),
    ..Job::default_sheet()
  }
}

fn document() -> Document {
  let contour = Contour::from_points(
    &[
      Point2::new(10.0, 10.0),
      Point2::new(90.0, 10.0),
      Point2::new(90.0, 70.0),
      Point2::new(10.0, 70.0),
    ],
    true,
  )
  .expect("rectangle");
  let mut document = Document::new(job());
  document.layers.push(Layer {
    id: "L1".to_owned(),
    name: "Part".to_owned(),
    visible: true,
    entities: vec![cam_model::Entity {
      id: "E1".to_owned(),
      kind: cam_model::EntityKind::Contour { contour },
    }],
  });
  document
}

/// A tiny deterministic program: one pocket run at `CUT_DEPTH` with a 6 mm tool.
fn program() -> MoveStream {
  let mut stream = MoveStream::new(Length::from_mm(5.0), Point2::new(0.0, 0.0));
  stream.segments.push(Segment {
    name: "pocket 1".to_owned(),
    tool_number: 1,
    tool_diameter: Length::from_mm(6.0),
    feed_rate: 100.0,
    plunge_rate: 30.0,
    spindle_speed: 18_000,
    moves: vec![
      Move::Rapid {
        to: Point3::new(10.0, 10.0, 5.0),
      },
      Move::Feed {
        to: Point3::new(10.0, 10.0, -CUT_DEPTH),
      },
      Move::Feed {
        to: Point3::new(90.0, 10.0, -CUT_DEPTH),
      },
      Move::Feed {
        to: Point3::new(90.0, 70.0, -CUT_DEPTH),
      },
      Move::Retract {
        to: Point3::new(90.0, 70.0, 5.0),
      },
    ],
  });
  stream
}

/// The fixture's program simulated over its own stock: what the shell is handed.
fn simulated() -> Heightfield {
  let mut field =
    Heightfield::new(&job(), Length::from_mm(CELL_MM)).expect("the grid fits the cap");
  let removal = sweep::apply(&program(), &mut field);
  assert!(
    removal.crashes.is_empty(),
    "the fixture's program must not ram the stock"
  );
  assert!(
    field.volumes().removed > 0.0,
    "the fixture's program must actually cut"
  );
  field
}

fn raw_input(events: Vec<egui::Event>) -> egui::RawInput {
  egui::RawInput {
    screen_rect: Some(egui::Rect::from_min_size(
      egui::Pos2::ZERO,
      egui::vec2(1200.0, 800.0),
    )),
    events,
    ..Default::default()
  }
}

/// A harness that keeps one egui context alive across frames, like a real window.
struct Harness {
  ctx: egui::Context,
  shell: Shell,
  app: AppState,
  output: ShellOutput,
  /// Vertex count of every `egui::Mesh` the frame produced, which is what A11 is about.
  ///
  /// A vertex count rather than a mesh count because `egui` tessellates a few of its own decorations
  /// into small meshes too: the surface is the one whose vertex count is three per painted triangle.
  mesh_vertices: Vec<usize>,
  /// Every piece of text the frame drew.
  texts: Vec<String>,
}

impl Harness {
  /// A harness with no preview at all, whose layout has settled: the M1–M3 shell exactly.
  fn bare() -> Self {
    let mut harness = Self {
      ctx: egui::Context::default(),
      shell: Shell::new(),
      app: AppState::new(document()),
      output: ShellOutput::default(),
      mesh_vertices: Vec::new(),
      texts: Vec::new(),
    };
    // The first frames lay the panels out and frame the 2D document, exactly as in `input.rs`.
    for _ in 0..3 {
      harness.frame(Vec::new());
    }
    harness
  }

  /// A harness whose shell already holds the fixture's preview and whose layout has settled.
  fn new() -> Self {
    let mut harness = Self::bare();
    harness.shell.set_preview(simulated(), program(), &job());
    harness.frame(Vec::new());
    harness
  }

  fn frame(&mut self, events: Vec<egui::Event>) {
    let mut captured: Option<ShellOutput> = None;
    let full = self.ctx.run_pass(raw_input(events), |ctx| {
      captured = Some(self.shell.show(&mut self.app, ctx));
    });
    self.mesh_vertices = full
      .shapes
      .iter()
      .filter_map(|clipped| match &clipped.shape {
        egui::Shape::Mesh(mesh) => Some(mesh.vertices.len()),
        _ => None,
      })
      .collect();
    self.texts = full
      .shapes
      .iter()
      .filter_map(|clipped| match &clipped.shape {
        egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
        _ => None,
      })
      .collect();
    // The texture deltas belong to a renderer we do not have headless; dropping them silently is
    // exactly what epaint forbids, so say so explicitly.
    full.drop_without_applying_deltas();
    self.output = captured.expect("the shell ran");
    let actions = self.output.actions.clone();
    for action in actions {
      self.shell.apply_action(&mut self.app, &action);
    }
  }

  /// A point inside the 3D panel, well clear of every other panel and of the canvas.
  fn in_panel(&self) -> egui::Pos2 {
    egui::pos2(600.0, 200.0)
  }
}

#[test]
fn an_open_panel_paints_triangles_and_the_stock_box() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());

  assert!(
    harness.output.drawn.painted_triangles > 0,
    "the open panel must paint the simulated surface"
  );
  // Twelve stock-box edges plus the program's four projected edges: the first move opens a
  // polyline instead of being joined to a start position the stream does not carry.
  assert_eq!(
    harness.output.drawn.painted_3d_lines, 16,
    "twelve box edges and the program's four edges"
  );
}

#[test]
fn a_closed_panel_paints_no_3d_at_all() {
  let mut harness = Harness::new();
  harness.shell.show_3d = false;
  harness.frame(Vec::new());

  assert_eq!(harness.output.drawn.painted_triangles, 0);
  assert_eq!(harness.output.drawn.painted_3d_lines, 0);
  assert!(
    harness.mesh_vertices.iter().all(|&vertices| vertices < 100),
    "no surface reached the painter, only egui's own decorations: {:?}",
    harness.mesh_vertices
  );
}

#[test]
fn the_surface_reaches_the_painter_as_an_egui_mesh() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());

  assert!(harness.output.drawn.painted_triangles > 0);
  let surface_vertices = 3 * harness.output.drawn.painted_triangles;
  assert_eq!(
    harness
      .mesh_vertices
      .iter()
      .filter(|&&vertices| vertices == surface_vertices)
      .count(),
    1,
    "the surface is one `egui::Mesh` of {surface_vertices} vertices, with no new dependency \
     behind it (A11); the frame's meshes were {:?}",
    harness.mesh_vertices
  );
}

#[test]
fn a_drag_orbits_the_camera() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());
  let before = *harness
    .shell
    .preview()
    .expect("the fixture loaded a preview")
    .camera();

  let from = harness.in_panel();
  let to = egui::pos2(from.x + 120.0, from.y + 60.0);
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

  let after = *harness
    .shell
    .preview()
    .expect("the fixture loaded a preview")
    .camera();
  assert!(
    (after.yaw - before.yaw).abs() > 1.0e-6,
    "a horizontal drag must turn the camera: {} -> {}",
    before.yaw,
    after.yaw
  );
  assert!(
    (after.pitch - before.pitch).abs() > 1.0e-6,
    "a vertical drag must raise or lower the camera: {} -> {}",
    before.pitch,
    after.pitch
  );
  assert_eq!(
    after.target, before.target,
    "orbiting turns the eye, it does not move the target"
  );
}

#[test]
fn the_wheel_zooms_about_the_target() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());
  let before = *harness
    .shell
    .preview()
    .expect("the fixture loaded a preview")
    .camera();

  harness.frame(vec![
    egui::Event::PointerMoved(harness.in_panel()),
    egui::Event::MouseWheel {
      unit: egui::MouseWheelUnit::Point,
      delta: egui::vec2(0.0, 120.0),
      phase: egui::TouchPhase::Move,
      source: egui::MouseWheelSource::Wheel,
      modifiers: egui::Modifiers::default(),
    },
  ]);

  let after = *harness
    .shell
    .preview()
    .expect("the fixture loaded a preview")
    .camera();
  assert!(
    after.distance.mm() < before.distance.mm(),
    "scrolling up must move the eye closer: {} -> {}",
    before.distance.mm(),
    after.distance.mm()
  );
  assert_eq!(
    after.target, before.target,
    "the wheel zooms about the target, it does not shift it"
  );
}

#[test]
fn a_camera_the_projection_rejects_paints_nothing() {
  let mut harness = Harness::new();
  harness
    .shell
    .preview_mut()
    .expect("the fixture loaded a preview")
    .camera_mut()
    .distance = Length::ZERO;
  harness.frame(Vec::new());

  assert_eq!(
    harness.output.drawn.painted_triangles, 0,
    "a camera with no usable projection paints no surface"
  );
  assert_eq!(
    harness.output.drawn.painted_3d_lines, 0,
    "and no lines either"
  );
}

#[test]
fn the_2d_canvas_still_paints_while_the_panel_is_open() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());

  assert!(
    harness.output.drawn.painted_triangles > 0,
    "the panel is open, so this is the additive case"
  );
  assert_eq!(
    harness.output.drawn.painted_items, 4,
    "the four rectangle edges still reach the 2D canvas (A12)"
  );
  assert!(
    harness.output.drawn.painted_grid_lines > 0,
    "and so does the grid"
  );
}

#[test]
fn a_closed_panel_leaves_the_2d_frame_exactly_as_it_was() {
  // Two shells, one holding a preview and one holding nothing, both past their framing frames: with
  // the panel closed they must paint the same 2D picture, which is what A12 asks of the addition.
  let mut with_preview = Harness::new();
  with_preview.shell.show_3d = false;
  let mut without_preview = Harness::bare();

  with_preview.frame(Vec::new());
  without_preview.frame(Vec::new());

  assert_eq!(
    with_preview.output.drawn.painted_items,
    without_preview.output.drawn.painted_items
  );
  assert_eq!(
    with_preview.output.drawn.painted_grid_lines,
    without_preview.output.drawn.painted_grid_lines
  );
  assert_eq!(with_preview.output.canvas, without_preview.output.canvas);
  assert_eq!(with_preview.output.drawn.painted_triangles, 0);
  assert_eq!(with_preview.output.drawn.painted_3d_lines, 0);
}

#[test]
fn the_panel_is_reachable_from_its_command_bar_button() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());

  assert!(
    harness.texts.iter().any(|text| text == "3D"),
    "the command bar offers the 3D toggle, painted texts: {:?}",
    harness.texts
  );
}
