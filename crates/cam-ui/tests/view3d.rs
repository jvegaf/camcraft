//! Headless tests for the 3D preview panel (M5 A9, A10, A11, A12).
//!
//! No display and no GPU: `egui::Context` runs a full frame on the CPU, the panel reports how many
//! triangles and 3D lines it painted, and the shape list says whether the surface really reached the
//! painter as an `egui::Mesh` (A11). The panel is additive (A12): while it is open the 2D canvas keeps
//! painting its own items, and closing it restores the M1–M3 picture exactly.
//!
//! The playback tests (A10) drive [`cam_ui::Preview3d::advance`] with an injected delta and no clock:
//! the program's own time is a closed form — each move's duration is its length over the segment's
//! feed rate — so the cursor, the fraction of the current move and the removed volume are all
//! asserted against numbers written out by hand, and the field at the end is compared cell for cell
//! with the whole-program [`sweep::apply`] the report uses.
//!
//! The published seconds (`seconds_for`) are held to the same standard: the estimate beside the
//! toolpath card may not be a second time formula, so a test asserts it is exactly the number the
//! playback clock reports for the same program (camui-toolpath S5).

use cam_app::AppState;
use cam_geom::{Contour, Point2};
use cam_model::{Document, Job, Layer, Operation, ProfileParams, Toolpath};
use cam_move::{Move, MoveStream, Point3, Segment, ToolShape};
use cam_sim::{sweep, Heightfield, Volumes};
use cam_store::tools::{CuttingData, StoredTool, ToolGeometry};
use cam_ui::theme::{Palette, Role, Theme};
use cam_ui::view3d::{clock_text, seconds_for};
use cam_ui::{Preview3d, Shell, ShellOutput};
use cam_units::{Length, UnitSystem};

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
/// The tool number the drawing's own toolpath cuts with (M13 S3).
const TOOL_NUMBER: u32 = 4;

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

/// The fixture drawing plus one profile toolpath cutting the rectangle it already holds (M13 S3).
///
/// `document()` alone is geometry and no path, which is exactly the drawing the empty half must refuse: this
/// one is the drawing that has something of its own to simulate.
fn document_with_toolpath() -> Document {
  let mut document = document();
  document.toolpaths.push(Toolpath {
    id: "P1".to_owned(),
    name: "Profile 01".to_owned(),
    entity_ids: vec!["E1".to_owned()],
    island_ids: Vec::new(),
    tool_number: TOOL_NUMBER,
    operation: Operation::Profile(ProfileParams::default()),
  });
  document
}

/// One tool as a library snapshot holds it, so the drawing's own `tool_number` resolves.
///
/// The rates are the ones the shell's other fixtures use: a 6 mm cutter at 100 mm/s and 30 mm/s of plunge.
fn stored_tool() -> StoredTool {
  StoredTool {
    geometry: ToolGeometry {
      id: format!("G{TOOL_NUMBER}"),
      name_format: "{Tool Type} ({Diameter|F}{Units Short})".to_owned(),
      tool_type: 0,
      units: UnitSystem::Mm,
      diameter: Some(Length::from_mm(6.0)),
      included_angle: None,
      flat_diameter: None,
      num_flutes: Some(2),
      flute_length: None,
      tip_radius: None,
      custom_attributes: None,
    },
    cutting: CuttingData {
      id: format!("G{TOOL_NUMBER}-cut"),
      rate_units: UnitSystem::Mm,
      feed_rate: Some(100.0),
      plunge_rate: Some(30.0),
      spindle_speed: Some(18_000),
      stepdown: None,
      stepover: None,
      clear_stepover: None,
      tool_number: Some(TOOL_NUMBER),
      notes: None,
    },
  }
}

/// The program the drawing's own toolpath makes, exactly as the shell builds it for a simulation.
///
/// Built here rather than read off the shell, so the clock a press reports is compared with the program the
/// document describes instead of with itself.
fn document_program() -> MoveStream {
  cam_program::program_for_snapshot(&document_with_toolpath(), &[stored_tool()], None)
    .expect("the fixture's toolpath and its tool agree")
}

/// A tiny deterministic program: one pocket run at `CUT_DEPTH` with a 6 mm tool.
fn program() -> MoveStream {
  let mut stream = MoveStream::new(Length::from_mm(5.0), Point2::new(0.0, 0.0));
  stream.segments.push(Segment {
    name: "pocket 1".to_owned(),
    tool_number: 1,
    tool: ToolShape::cylinder(Length::from_mm(6.0)),
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

/// The stock the shell is handed: untouched, the one construction `Heightfield::new` defines.
fn untouched() -> Heightfield {
  Heightfield::new(&job(), Length::from_mm(CELL_MM)).expect("the grid fits the cap")
}

/// The fixture's program simulated over its own stock from scratch: the expectation playback must
/// reproduce (the report's own numbers, A10).
fn whole_program_field() -> Heightfield {
  let mut field = untouched();
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

/// A playback fixture whose four moves last exactly two, two, five and one tenths of a second.
///
/// The numbers are chosen so the closed form is arithmetic a reader can do in their head: a rapid of
/// 20 mm at 100 mm/s, a plunge of 10 mm at 50 mm/s, a feed of 50 mm at 100 mm/s and a retract of
/// 10 mm at 100 mm/s. `MoveStream::new` places the machine at `(10, 0, safe_z)` before the first move,
/// so the rapid's length is the 20 mm the numbers state. The plunge cuts a disc and the feed cuts a
/// capsule, so the removed volume changes when the cursor reaches the third move, not before.
fn playback_program() -> MoveStream {
  let mut stream = MoveStream::new(Length::from_mm(5.0), Point2::new(10.0, 0.0));
  stream.segments.push(Segment {
    name: "playback".to_owned(),
    tool_number: 1,
    tool: ToolShape::cylinder(Length::from_mm(6.0)),
    feed_rate: 100.0,
    plunge_rate: 50.0,
    spindle_speed: 18_000,
    moves: vec![
      Move::Rapid {
        to: Point3::new(10.0, 20.0, 5.0),
      },
      Move::Feed {
        to: Point3::new(10.0, 20.0, -5.0),
      },
      Move::Feed {
        to: Point3::new(60.0, 20.0, -5.0),
      },
      Move::Retract {
        to: Point3::new(60.0, 20.0, 5.0),
      },
    ],
  });
  stream
}

/// The first `moves` movements of `program`, as a program of its own.
fn prefix_of(program: &MoveStream, moves: usize) -> MoveStream {
  let mut prefix = MoveStream::new(program.safe_z, program.home);
  let mut left = moves;
  for segment in &program.segments {
    let take = left.min(segment.moves.len());
    let mut copy = segment.clone();
    copy.moves.truncate(take);
    prefix.segments.push(copy);
    left -= take;
  }
  prefix
}

/// The closed form of the removed volume after `moves` movements: the same program without the rest.
fn closed_form_volumes(moves: usize) -> Volumes {
  let mut field = untouched();
  sweep::apply(&prefix_of(&playback_program(), moves), &mut field);
  field.volumes()
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
  /// Every shape the frame drew, so a test can read the colours the panel painted (T9).
  shapes: Vec<egui::epaint::ClippedShape>,
}

impl Harness {
  /// A harness with no preview at all, whose layout has settled: the M1–M3 shell exactly.
  fn bare() -> Self {
    Self::with_document(document())
  }

  /// A harness over `document`, with no preview and its layout settled.
  ///
  /// The same three framing frames [`Harness::bare`] runs: a test that puts another drawing in the window
  /// still needs the panels laid out before it presses anything (M13 S3's fixtures).
  fn with_document(document: Document) -> Self {
    let mut harness = Self {
      ctx: egui::Context::default(),
      shell: Shell::new(),
      app: AppState::new(document),
      output: ShellOutput::default(),
      mesh_vertices: Vec::new(),
      texts: Vec::new(),
      shapes: Vec::new(),
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
    harness.shell.set_preview(untouched(), program(), &job());
    harness.frame(Vec::new());
    harness
  }

  /// A harness holding the playback fixture, whose move durations the closed form states exactly.
  fn playing() -> Self {
    let mut harness = Self::bare();
    harness
      .shell
      .set_preview(untouched(), playback_program(), &job());
    harness.frame(Vec::new());
    harness
  }

  /// The preview the shell holds, which every playback test drives.
  fn preview(&self) -> &Preview3d {
    self.shell.preview().expect("the harness loaded a preview")
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
    self.shapes = full.shapes.clone();
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
  // Twelve stock-box edges, the document's four vector edges, the program's four projected edges — the first
  // move opens a polyline instead of being joined to a start position the stream does not carry — and the tool
  // marker's axis and diameter. Each layer is counted on its own because the overlay row hides them one at a
  // time (S8), and a single total could not say which one went missing.
  let drawn = &harness.output.drawn;
  assert_eq!(drawn.painted_material_lines, 12, "twelve box edges");
  assert_eq!(drawn.painted_vector_lines, 4, "the fixture's rectangle");
  assert_eq!(drawn.painted_program_lines, 4, "four projected edges");
  assert_eq!(drawn.painted_tool_lines, 2, "the tool's axis and diameter");
  assert_eq!(drawn.painted_3d_lines, 22, "and the total is their sum");
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
fn the_split_paints_both_halves_and_closing_it_paints_one() {
  // T6 replaces the exclusive mode M5 shipped: S8's 3D is a *split*, so the drawing is painted beside the 3D
  // half rather than instead of it. This test is the M5 test rewritten to the rule that replaced it.
  let mut harness = Harness::new();
  harness.frame(Vec::new());

  assert!(
    harness.output.drawn.painted_triangles > 0,
    "the 3D half paints the simulated surface"
  );
  assert!(
    harness.output.drawn.painted_items > 0,
    "and the drawing half paints the document beside it"
  );
  assert!(
    harness.output.drawn.painted_grid_lines > 0,
    "including its grid"
  );

  // Back to the drawing alone: the triangle count drops to zero and the scene keeps its own half's width.
  let (two_d, _) = harness.halves();
  harness.shell.set_view_3d(false);
  harness.frame(Vec::new());
  assert_eq!(
    harness.output.drawn.painted_triangles, 0,
    "a closed split paints no mesh"
  );
  assert_eq!(
    harness.output.drawn.painted_items, 4,
    "the four rectangle edges are still there (A12)"
  );
  assert!(
    harness.output.drawn.painted_grid_lines > 0,
    "and so is the grid"
  );
  assert!(
    harness.output.canvas.width as f32 > two_d.width(),
    "with the split closed the drawing gets the whole canvas back: {} against {}",
    harness.output.canvas.width,
    two_d.width()
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
fn the_toggle_paints_the_view_it_offers_where_a_hand_can_reach_it() {
  // The toggle reports its cells (a test presses them) *and* paints them: a control that only reported its
  // words would be invisible to the operator. Its home moved to the canvas in camui-prancheta T6 (S8), so this
  // no longer looks at the app bar.
  let mut harness = Harness::new();
  harness.frame(Vec::new());

  assert!(
    harness.texts.iter().any(|text| text == "3D"),
    "the canvas offers the 3D toggle, painted texts: {:?}",
    harness.texts
  );
  assert!(
    harness
      .output
      .drawn
      .view_toggle
      .iter()
      .any(|(word, _)| word == "3D"),
    "and reports where it drew it"
  );
}

// ---------------------------------------------------------------------------------------------
// V3 — playback (A10): the program plays, and the stock disappears as it cuts.
// ---------------------------------------------------------------------------------------------

#[test]
fn a_fixed_dt_sequence_reaches_the_closed_form_cursor() {
  // Four moves lasting 0.2, 0.2, 0.5 and 0.1 s. The frames never land on a move boundary, so the
  // expectation is the arithmetic in the fixture's own doc comment and not a floating-point edge.
  let mut preview = Preview3d::new(untouched(), playback_program(), &job());
  // One second of wall clock per second of program time, so the closed form is in program seconds.
  preview.set_speed(1.0);
  preview.set_playing(true);

  let frames = [
    (0.1, 0usize, 0.5, 0.1),
    (0.15, 1, 0.25, 0.25),
    (0.25, 2, 0.2, 0.5),
    (0.2, 2, 0.6, 0.7),
    (0.15, 2, 0.9, 0.85),
    // The last frame overshoots the end, and the clock stops at the program's end rather than at the
    // delta that crossed it: 1.15 s of frames is 1 s of program.
    (0.3, 4, 1.0, 1.0),
  ];
  for (dt, cursor, fraction, elapsed) in frames {
    preview.advance(dt);
    assert_eq!(
      preview.cursor(),
      cursor,
      "after {elapsed} s of program time the cursor must be at move {cursor}"
    );
    assert!(
      (preview.fraction() - fraction).abs() < 1.0e-9,
      "after {elapsed} s the current move must be {fraction} consumed, got {}",
      preview.fraction()
    );
    assert!(
      (preview.elapsed() - elapsed).abs() < 1.0e-9,
      "the clock must read {elapsed} s, got {}",
      preview.elapsed()
    );
  }
  assert!(
    preview.finished(),
    "the sequence adds up to the whole program"
  );
  assert_eq!(preview.progress(), 1.0);
}

#[test]
fn a_fixed_dt_sequence_removes_the_closed_form_volume() {
  let mut preview = Preview3d::new(untouched(), playback_program(), &job());
  preview.set_speed(1.0);
  preview.set_playing(true);
  assert_eq!(
    preview.field().volumes(),
    closed_form_volumes(0),
    "playback starts on untouched stock"
  );

  // 0.45 s in: both the rapid and the plunge are whole moves behind the cursor.
  preview.advance(0.45);
  assert_eq!(preview.cursor(), 2);
  assert_eq!(preview.field().volumes(), closed_form_volumes(2));
  assert!(
    preview.field().volumes().removed > 0.0,
    "the plunge is a cutting move, so material is already gone"
  );

  // 0.75 s in: part of the feed is consumed, and a partly consumed move removes nothing yet.
  preview.advance(0.3);
  assert_eq!(preview.cursor(), 2);
  assert_eq!(
    preview.field().volumes(),
    closed_form_volumes(2),
    "a half-consumed move must not have been applied to the field"
  );

  // 0.95 s in: the feed is behind the cursor and the capsule it swept is gone.
  preview.advance(0.2);
  assert_eq!(preview.cursor(), 3);
  assert_eq!(preview.field().volumes(), closed_form_volumes(3));
  assert!(
    preview.field().volumes().removed > closed_form_volumes(2).removed,
    "the feed removes more material than the plunge alone"
  );

  // 1.05 s in: the retract ends the program, and its own cut is nothing.
  preview.advance(0.1);
  assert!(preview.finished());
  assert_eq!(preview.field().volumes(), closed_form_volumes(4));
}

#[test]
fn a_zero_dt_changes_nothing() {
  let mut preview = Preview3d::new(untouched(), playback_program(), &job());
  preview.set_speed(1.0);
  preview.set_playing(true);
  preview.advance(0.45);
  let cursor = preview.cursor();
  let fraction = preview.fraction();
  let volumes = preview.field().volumes();
  let elapsed = preview.elapsed();

  preview.advance(0.0);

  assert_eq!(preview.cursor(), cursor);
  assert_eq!(preview.fraction(), fraction);
  assert_eq!(preview.elapsed(), elapsed);
  assert_eq!(preview.field().volumes(), volumes);
  assert!(preview.playing(), "a zero frame must not stop playback");
}

#[test]
fn pausing_holds_the_cursor_and_playing_resumes_from_it() {
  let mut preview = Preview3d::new(untouched(), playback_program(), &job());
  preview.set_speed(1.0);
  preview.set_playing(true);
  preview.advance(0.45);
  assert_eq!(preview.cursor(), 2);

  preview.set_playing(false);
  preview.advance(10.0);
  assert_eq!(preview.cursor(), 2, "a paused preview must not move");
  assert!(
    (preview.fraction() - 0.1).abs() < 1.0e-9,
    "a paused preview must not consume part of a move either, got {}",
    preview.fraction()
  );
  assert_eq!(preview.field().volumes(), closed_form_volumes(2));

  preview.set_playing(true);
  preview.advance(0.4);
  assert_eq!(
    preview.cursor(),
    2,
    "playing resumes where the pause left it"
  );
  assert!(
    (preview.fraction() - 0.9).abs() < 1.0e-9,
    "0.85 s into a 1 s program is 0.9 of its third move, got {}",
    preview.fraction()
  );
}

#[test]
fn a_step_advances_exactly_one_move() {
  let mut preview = Preview3d::new(untouched(), playback_program(), &job());
  assert!(!preview.playing(), "a preview opens paused");

  preview.step();
  assert_eq!(preview.cursor(), 1);
  assert_eq!(preview.fraction(), 0.0);
  assert!(!preview.playing(), "stepping is a paused advance");
  assert_eq!(
    preview.field().volumes(),
    closed_form_volumes(1),
    "one step applies exactly the first move"
  );

  preview.step();
  assert_eq!(preview.cursor(), 2);
  assert_eq!(preview.field().volumes(), closed_form_volumes(2));
  assert!(
    preview.field().volumes().removed > 0.0,
    "the second move is the plunge"
  );

  preview.step();
  preview.step();
  assert_eq!(preview.cursor(), 4);
  assert!(preview.finished());
  assert_eq!(preview.progress(), 1.0);

  preview.step();
  assert_eq!(preview.cursor(), 4, "a step past the end changes nothing");
}

#[test]
fn reset_restores_the_untouched_stock() {
  let mut preview = Preview3d::new(untouched(), playback_program(), &job());
  preview.set_playing(true);
  preview.advance(10.0);
  assert!(preview.finished());
  assert!(preview.field().volumes().removed > 0.0);

  preview.reset();

  assert_eq!(preview.cursor(), 0);
  assert_eq!(preview.fraction(), 0.0);
  assert_eq!(preview.progress(), 0.0);
  assert!(!preview.playing());
  assert_eq!(
    preview.field().volumes(),
    untouched().volumes(),
    "reset restores the whole stock, not just the cursor"
  );
  let fresh = untouched();
  let grid = fresh.grid();
  for row in 0..grid.rows {
    for column in 0..grid.columns {
      assert_eq!(
        preview.field().surface(column, row),
        fresh.surface(column, row),
        "cell ({column}, {row}) must be back at the stock top"
      );
    }
  }
}

#[test]
fn the_final_field_equals_the_whole_program_sweep_cell_for_cell() {
  let expected = whole_program_field();
  let grid = expected.grid();

  // One call and many frames must end in the same place: `advance` is a pure function of the deltas
  // it is given, and it applies whole moves as the cursor crosses them.
  for (how, deltas) in [
    ("one frame", vec![10.0_f64]),
    ("many frames", vec![0.03_f64; 40]),
  ] {
    let mut preview = Preview3d::new(untouched(), program(), &job());
    preview.set_playing(true);
    for dt in deltas {
      preview.advance(dt);
    }
    assert!(
      preview.finished(),
      "{how}: the whole program must have been played"
    );
    assert_eq!(preview.field().grid(), grid, "{how}");
    for row in 0..grid.rows {
      for column in 0..grid.columns {
        assert_eq!(
          preview.field().surface(column, row),
          expected.surface(column, row),
          "{how}: cell ({column}, {row}) must match `sweep::apply` over the whole program"
        );
      }
    }
    assert_eq!(
      preview.field().volumes(),
      expected.volumes(),
      "{how}: the animation and the report cannot disagree"
    );
  }
}

#[test]
fn playback_stops_at_the_end_of_the_program() {
  let mut preview = Preview3d::new(untouched(), playback_program(), &job());
  preview.set_playing(true);
  preview.advance(10.0);

  assert!(preview.finished());
  assert!(!preview.playing(), "playback stops at the end");
  assert_eq!(preview.progress(), 1.0);
  assert_eq!(preview.fraction(), 1.0);
  let volumes = preview.field().volumes();
  let elapsed = preview.elapsed();

  preview.set_playing(true);
  preview.advance(10.0);
  assert_eq!(preview.cursor(), 4);
  assert_eq!(preview.field().volumes(), volumes);
  assert_eq!(preview.elapsed(), elapsed);
  assert!(
    !preview.playing(),
    "playing a finished program stops immediately"
  );
}

#[test]
fn the_panel_reports_playback_progress_from_the_start_to_the_end() {
  let mut harness = Harness::playing();

  // Paused by default: two frames leave the program where it started.
  harness.frame(Vec::new());
  harness.frame(Vec::new());
  assert_eq!(
    harness.output.drawn.preview_progress, 0.0,
    "the readout starts at the first move"
  );
  assert_eq!(harness.preview().cursor(), 0);

  // Playing: the shell reads the clock and hands the delta in, so the readout moves on its own.
  harness
    .shell
    .preview_mut()
    .expect("the harness loaded a preview")
    .set_speed(0.5);
  harness
    .shell
    .preview_mut()
    .expect("the harness loaded a preview")
    .set_playing(true);
  harness.frame(Vec::new());
  let midway = harness.output.drawn.preview_progress;
  assert!(
    midway > 0.0 && midway < 1.0,
    "one frame of a playing program is part of the way through, not at either end: {midway}"
  );

  harness
    .shell
    .preview_mut()
    .expect("the harness loaded a preview")
    .set_speed(600.0);
  for _ in 0..10 {
    harness.frame(Vec::new());
    if harness.preview().finished() {
      break;
    }
  }
  assert!(
    harness.preview().finished(),
    "the panel must play the program to its end"
  );
  assert_eq!(
    harness.output.drawn.preview_progress, 1.0,
    "the readout reaches its end value when the program finishes"
  );
  assert_eq!(harness.preview().cursor(), playback_program().move_count());
}

#[test]
fn the_panel_offers_play_step_and_reset() {
  let mut harness = Harness::playing();
  harness.frame(Vec::new());

  // The transport is S8's painted `▶` rather than the word `Play`, so the three controls are read from the
  // report the bar publishes — the same surface a test presses them through (camui-prancheta S8).
  let words: Vec<&str> = harness
    .output
    .drawn
    .playback_hits
    .iter()
    .map(|(word, _)| word.as_str())
    .collect();
  for text in ["Play", "Step", "Reset"] {
    assert!(
      words.contains(&text),
      "the panel must offer a {text} control, reported: {words:?}"
    );
  }
  // `Step` and `Reset` are words on the readout's line, so they are painted as well as reported.
  for text in ["Step", "Reset"] {
    assert!(
      harness.texts.iter().any(|painted| painted == text),
      "the panel must paint {text}, painted texts: {:?}",
      harness.texts
    );
  }
}

// ---------------------------------------------------------------------------------------------
// M6 — a helical move (A7): the preview times it along its own spiral and draws it descending.
// ---------------------------------------------------------------------------------------------

#[test]
fn a_helical_move_is_timed_along_its_own_spiral_and_played_to_the_end() {
  // The preview's clock is a story about progress, and a helix travels the hypotenuse of a turn rather
  // than the turn itself: one full revolution of radius 10 descending 2 mm is
  // `sqrt((2π·10)² + 2²) = 62.8638` mm, which at the segment's 100 mm/s feed is 0.628638 s — where an
  // arc's own timing, the turn alone, would say 0.628319. The fixture's four moves are therefore a
  // rapid of 20 mm (0.2 s), a plunge of 10 mm at 50 mm/s (0.2 s), the helix, and a retract of 12 mm
  // (0.12 s): `1.148638` s in all, and the program must reach its end.
  let mut stream = MoveStream::new(Length::from_mm(5.0), Point2::new(10.0, 0.0));
  stream.segments.push(Segment {
    name: "thread".to_owned(),
    tool_number: 1,
    tool: ToolShape::cylinder(Length::from_mm(4.0)),
    feed_rate: 100.0,
    plunge_rate: 50.0,
    spindle_speed: 18_000,
    moves: vec![
      Move::Rapid {
        to: Point3::new(10.0, 20.0, 5.0),
      },
      Move::Feed {
        to: Point3::new(10.0, 20.0, -5.0),
      },
      Move::Helix {
        to: Point3::new(10.0, 20.0, -7.0),
        center: Point2::new(20.0, 20.0),
        ccw: true,
      },
      Move::Retract {
        to: Point3::new(10.0, 20.0, 5.0),
      },
    ],
  });

  let spiral = ((std::f64::consts::TAU * 10.0_f64).powi(2) + 4.0).sqrt();
  let expected = 0.2 + 0.2 + spiral / 100.0 + 12.0 / 100.0;

  let mut preview = Preview3d::new(untouched(), stream, &job());
  preview.set_speed(1.0);
  preview.set_playing(true);
  preview.advance(10.0);
  assert!(preview.finished(), "the program plays to its end");
  assert!(
    (preview.elapsed() - expected).abs() < 1.0e-9,
    "the program's own time is {expected} s, the preview says {} s",
    preview.elapsed()
  );
  assert_eq!(
    preview.cursor(),
    4,
    "four moves: the helix is one of them, not a staircase"
  );
}

// ---------------------------------------------------------------------------------------------
// T9 — the 3D view's colours are the design's, not its own (L7 left this open; L34 authorises it).
// ---------------------------------------------------------------------------------------------

/// Every colour the 3D panel painted in the last frame.
///
/// The panel is found by its surface mesh — the one mesh whose vertex count is three per painted
/// triangle — and every shape clipped to that same rectangle is the panel's own output: the bars,
/// the palette, the dock and the canvas each paint inside their own clip rectangle, so this is the
/// frame's 3D view and nothing else.
#[test]
fn seconds_for_is_the_time_laws_total_and_not_a_second_opinion() {
  // `playback_program`'s four moves are 20 mm at 100 mm/s, a 10 mm plunge at 50 mm/s, a 50 mm feed at
  // 100 mm/s and a 10 mm retract at 100 mm/s: one second, exactly what the clock's own arithmetic
  // answers. The plunge is the case a second formula gets wrong — it is timed at the *plunge* rate.
  let seconds = seconds_for(&playback_program());
  assert!(
    (seconds - 1.0).abs() < 1e-12,
    "the four moves are one second, not {seconds}"
  );
}

#[test]
fn the_published_seconds_agree_with_the_playback_clock() {
  let program = playback_program();
  let mut preview = Preview3d::new(untouched(), program.clone(), &job());

  // The estimate and the playback bar are one number with two readers (S5): the published total is
  // the preview's own duration, not a parallel computation that happens to agree here.
  assert_eq!(seconds_for(&program), preview.duration());

  // And it is the number the clock really runs out on: played to its end, the readout has consumed
  // exactly the published seconds.
  preview.set_playing(true);
  preview.advance(10.0);
  assert!(
    preview.finished(),
    "ten seconds of wall clock play one second out"
  );
  assert_eq!(preview.elapsed(), seconds_for(&program));
  assert_eq!(clock_text(seconds_for(&program)), "0:01");
}

fn colours_in_the_3d_panel(harness: &Harness) -> Vec<egui::Color32> {
  let surface_vertices = 3 * harness.output.drawn.painted_triangles;
  assert!(surface_vertices > 0, "the frame painted no surface");
  let panel = harness
    .shapes
    .iter()
    .find(|clipped| {
      matches!(&clipped.shape, egui::Shape::Mesh(mesh) if mesh.vertices.len() == surface_vertices)
    })
    .map(|clipped| clipped.clip_rect)
    .expect("the surface reached the painter as a mesh");
  let mut colours = Vec::new();
  for clipped in harness
    .shapes
    .iter()
    .filter(|clipped| clipped.clip_rect == panel)
  {
    collect_colours(&clipped.shape, &mut colours);
  }
  colours
}

/// The colours one shape paints, appended to `colours`.
fn collect_colours(shape: &egui::Shape, colours: &mut Vec<egui::Color32>) {
  match shape {
    egui::Shape::Vec(shapes) => {
      for shape in shapes {
        collect_colours(shape, colours);
      }
    }
    egui::Shape::Mesh(mesh) => colours.extend(mesh.vertices.iter().map(|vertex| vertex.color)),
    egui::Shape::LineSegment { stroke, .. } => colours.push(stroke.color),
    egui::Shape::Path(path) => {
      if let egui::epaint::ColorMode::Solid(colour) = path.stroke.color {
        colours.push(colour);
      }
      colours.push(path.fill);
    }
    egui::Shape::Rect(rect) => {
      colours.push(rect.fill);
      colours.push(rect.stroke.color);
    }
    egui::Shape::Circle(circle) => {
      colours.push(circle.fill);
      colours.push(circle.stroke.color);
    }
    egui::Shape::Text(text) => colours.push(text.fallback_color),
    _ => {}
  }
}

/// Whether `colour` sits on the closed segment between the ramp's two ends.
///
/// The ramp is a mix, so a painted vertex can be any stop on it, not just a named role. Each channel
/// pins `t` to an interval — a channel's value is the mix rounded, so `t` may be half a step either
/// side — and the colour is on the ramp when all three intervals share a point. This is the honest
/// form of "reachable from the theme": it accepts exactly the colours [`Palette::stock_shade`] can
/// produce and nothing else.
fn on_the_stock_ramp(palette: &Palette, colour: egui::Color32) -> bool {
  let (top, bottom) = (palette.get(Role::StockTop), palette.get(Role::StockBottom));
  let mut low = 0.0_f32;
  let mut high = 1.0_f32;
  for (top, bottom, value) in [
    (top.r(), bottom.r(), colour.r()),
    (top.g(), bottom.g(), colour.g()),
    (top.b(), bottom.b(), colour.b()),
  ] {
    let (top, bottom, value) = (f32::from(top), f32::from(bottom), f32::from(value));
    let delta = bottom - top;
    if delta == 0.0 {
      if (value - top).abs() > 0.5 {
        return false;
      }
      continue;
    }
    let edge = |offset: f32| (value + offset - top) / delta;
    let (first, second) = (edge(-0.5), edge(0.5));
    low = low.max(first.min(second));
    high = high.min(first.max(second));
  }
  low <= high + 1.0e-4
}

/// The colours the audit collected, with the invisible ones dropped and the rest deduplicated so a
/// failure names each offender once instead of once per vertex.
fn distinct_visible(colours: Vec<egui::Color32>) -> Vec<egui::Color32> {
  let mut distinct: Vec<egui::Color32> = Vec::new();
  for colour in colours {
    // An alpha-zero fill paints nothing, so it is not a colour the view chooses.
    if colour.a() == 0 || distinct.contains(&colour) {
      continue;
    }
    distinct.push(colour);
  }
  distinct
}

#[test]
fn the_3d_panel_paints_only_colours_the_theme_owns() {
  // The claim the slice makes: the 3D view no longer owns a single colour. Everything it paints is
  // either a role of the set that is showing or a stop on the stock's ramp between two of them, and
  // the audit reads what actually reached the painter rather than what the code meant to paint.
  for theme in [Theme::Dark, Theme::Light] {
    let mut harness = Harness::new();
    harness.shell.set_theme(theme);
    harness.frame(Vec::new());
    let painted = distinct_visible(colours_in_the_3d_panel(&harness));
    assert!(
      painted.len() >= 4,
      "{theme:?}: the audit saw only {painted:?}, so it did not read the panel"
    );
    let palette = theme.palette();
    let roles: Vec<egui::Color32> = Role::ALL.iter().map(|role| palette.get(*role)).collect();
    let stray: Vec<egui::Color32> = painted
      .iter()
      .copied()
      .filter(|colour| !roles.contains(colour) && !on_the_stock_ramp(&palette, *colour))
      .collect();
    assert!(
      stray.is_empty(),
      "{theme:?}: the 3D panel painted {stray:?}, which no `Role` owns — the view is still \
       shading with literals of its own"
    );
  }
}

#[test]
fn a_cut_stock_is_still_shaded_by_depth_after_the_ramp_became_tokens() {
  // The one thing this slice could quietly destroy: the surface used to interpolate between two
  // literals, and now interpolates between two roles. A field that has been cut must still show more
  // than one shade, every shade must be on the ramp, and the shades must still be ordered by depth —
  // darker where the tool went deeper.
  for theme in [Theme::Dark, Theme::Light] {
    let mut harness = Harness::bare();
    harness
      .shell
      .set_preview(whole_program_field(), program(), &job());
    harness.shell.set_theme(theme);
    harness.frame(Vec::new());
    let palette = theme.palette();
    let painted = distinct_visible(colours_in_the_3d_panel(&harness));
    let shades: Vec<egui::Color32> = painted
      .into_iter()
      .filter(|colour| on_the_stock_ramp(&palette, *colour))
      .collect();
    assert!(
      shades.len() >= 2,
      "{theme:?}: the cut surface is painted in one shade ({shades:?}), so the depth ramp is gone"
    );
    let deepest = shades
      .iter()
      .map(|colour| cam_ui::theme::luminance(*colour))
      .fold(1.0_f32, f32::min);
    assert!(
      deepest < cam_ui::theme::luminance(palette.get(Role::StockTop)),
      "{theme:?}: every shade is the untouched top, so nothing reads as cut"
    );
    assert!(
      deepest > cam_ui::theme::luminance(palette.get(Role::StockBottom)),
      "{theme:?}: the deepest shade is off the bottom of the ramp"
    );
  }
}

// ---------------------------------------------------------------------------------------------
// T11 — the simulation's progress is a bar with a clock on both sides, not a text percentage.
// ---------------------------------------------------------------------------------------------

/// A program whose moves all take no time at all: every rate is zero, so the clock's total is `0.0`.
///
/// The fixture exists for the one case the readout must not lie about: a program the shell can still
/// play to its end while computing no time at all to spend.
fn timeless_program() -> MoveStream {
  let mut stream = playback_program();
  for segment in &mut stream.segments {
    segment.feed_rate = 0.0;
    segment.plunge_rate = 0.0;
  }
  stream
}

/// The two rectangles the progress bar painted in the last frame: its trough, then its fill.
///
/// Found by colour rather than by position, which is the point: the trough is the theme's deepest
/// surface and the fill is the cut signal, so a bar painted with a literal of its own would not be
/// found at all. The bar is not inside the surface's own clip rectangle — the playback controls sit
/// above it — so the panel is the clip rect that *contains* the surface mesh's viewport, and every
/// other panel's clip is disjoint from it.
fn bar_rects(harness: &Harness) -> (egui::Rect, egui::Rect) {
  let palette = Theme::Dark.palette();
  let surface_vertices = 3 * harness.output.drawn.painted_triangles;
  assert!(surface_vertices > 0, "the frame painted no surface");
  let viewport = harness
    .shapes
    .iter()
    .find(|clipped| {
      matches!(&clipped.shape, egui::Shape::Mesh(mesh) if mesh.vertices.len() == surface_vertices)
    })
    .map(|clipped| clipped.clip_rect)
    .expect("the surface reached the painter as a mesh");
  // The bar has a clip of its own since the T6 correction (it sits below the picture, and a painter clipped to
  // the picture erased it: R2-001/R3-001), so it is found by where it is — under the surface's viewport — rather
  // than by sharing the surface's own clip. The surface's viewport still proves the 3D half is the one asked.
  let rects_in_panel = || {
    harness
      .shapes
      .iter()
      .filter(|clipped| clipped.shape.visual_bounding_rect().max.y >= viewport.min.y)
      .filter_map(|clipped| match &clipped.shape {
        egui::Shape::Rect(rect) if rect.rect.max.y >= viewport.max.y => {
          Some((rect.fill, rect.rect))
        }
        _ => None,
      })
  };
  let fill = rects_in_panel()
    .find(|(colour, _)| *colour == palette.get(Role::Cut))
    .map(|(_, rect)| rect)
    .expect("the panel painted no progress bar fill in the cut signal");
  // The smallest `Role::Canvas` rectangle that contains the fill: the central panel's own ground is painted in the
  // same role now that the split shares one panel with the drawing (S8), so "the first one" would be the whole
  // canvas rather than the bar's track.
  let trough = rects_in_panel()
    .filter(|(colour, rect)| *colour == palette.get(Role::Canvas) && rect.contains_rect(fill))
    .map(|(_, rect)| rect)
    .min_by(|a, b| {
      (a.width() * a.height())
        .partial_cmp(&(b.width() * b.height()))
        .unwrap_or(std::cmp::Ordering::Equal)
    })
    .expect("the panel painted no progress bar trough under its fill");
  (trough, fill)
}

#[test]
fn the_panel_states_the_elapsed_time_and_the_total_as_clocks() {
  let mut harness = Harness::playing();
  // The fixture lasts exactly one second: 0.2 s rapid, 0.2 s plunge, 0.5 s feed, 0.1 s retract.
  assert_eq!(harness.output.drawn.preview_progress, 0.0);
  assert_eq!(
    harness.output.drawn.preview_clock, "0:00 / 0:01",
    "a program that has not started reads as nothing spent of its total"
  );

  harness
    .shell
    .preview_mut()
    .expect("the harness loaded a preview")
    .set_speed(600.0);
  harness
    .shell
    .preview_mut()
    .expect("the harness loaded a preview")
    .set_playing(true);
  for _ in 0..10 {
    harness.frame(Vec::new());
    if harness.preview().finished() {
      break;
    }
  }
  assert!(
    harness.preview().finished(),
    "the panel must play the program to its end"
  );
  assert_eq!(
    harness.output.drawn.preview_clock, "0:01 / 0:01",
    "a finished program reads as all of its time spent"
  );
  assert_eq!(harness.output.drawn.preview_bar, 1.0);
}

#[test]
fn the_progress_bar_is_painted_in_the_cut_signal_and_follows_the_clock() {
  let mut harness = Harness::playing();
  let (trough, start_fill) = bar_rects(&harness);
  assert_eq!(
    start_fill.width(),
    0.0,
    "a program that has not started fills nothing, not even a bead of progress"
  );
  assert!(
    trough.width() > 0.0 && trough.height() > 0.0,
    "the empty bar still draws its track, so the row does not jump when playback starts"
  );

  harness
    .shell
    .preview_mut()
    .expect("the harness loaded a preview")
    .set_speed(600.0);
  harness
    .shell
    .preview_mut()
    .expect("the harness loaded a preview")
    .set_playing(true);
  for _ in 0..10 {
    harness.frame(Vec::new());
    if harness.preview().finished() {
      break;
    }
  }
  let (end_trough, end_fill) = bar_rects(&harness);
  assert_eq!(
    (end_trough.width(), end_trough.height()),
    (trough.width(), trough.height()),
    "the bar cannot change size as it fills"
  );
  assert_eq!(
    end_fill.width(),
    end_trough.width(),
    "a finished program fills the track exactly"
  );
}

#[test]
fn a_program_with_no_time_paints_an_empty_bar_and_says_so() {
  let mut harness = Harness::bare();
  harness
    .shell
    .set_preview(untouched(), timeless_program(), &job());
  harness.frame(Vec::new());
  assert_eq!(
    harness.output.drawn.preview_clock, "0:00 / 0:00",
    "a program with no time to spend states that on both sides"
  );
  assert_eq!(
    harness.output.drawn.preview_bar, 0.0,
    "a clock with no total must not fill the bar"
  );

  harness
    .shell
    .preview_mut()
    .expect("the harness loaded a preview")
    .set_playing(true);
  harness.frame(Vec::new());
  assert!(
    harness.preview().finished(),
    "every move takes no time, so one frame plays the program to its end"
  );
  assert_eq!(
    harness.output.drawn.preview_progress, 1.0,
    "the clock's own answer is unchanged: the program really is played out"
  );
  assert_eq!(
    harness.output.drawn.preview_bar, 0.0,
    "and the bar still claims no time, because there is no total to spend"
  );
  assert_eq!(harness.output.drawn.preview_clock, "0:00 / 0:00");
}

// ---------------------------------------------------------------------------------------------
// S8 — `2D | 3D` is a segmented toggle at the top right of the canvas, and 3D is a mode: the canvas
// splits in two, and the 3D half carries its own chrome (camui-prancheta T6).
// ---------------------------------------------------------------------------------------------

/// A program that quotes the reference frame's own playback readout: `T22 · Drilling 1 · Z 5.000`.
///
/// One segment, one rapid: 500 mm at the segment's 100 mm/s feed is exactly five seconds, and the move
/// arrives at `Z 5.000`, which is the line S8 writes down. Nothing is cut — the point of this fixture is the
/// clock and the name of the move, not the surface.
fn drilling_program() -> MoveStream {
  let mut stream = MoveStream::new(Length::from_mm(5.0), Point2::new(0.0, 0.0));
  stream.segments.push(Segment {
    name: "Drilling 1".to_owned(),
    tool_number: 22,
    tool: ToolShape::cylinder(Length::from_mm(5.0)),
    feed_rate: 100.0,
    plunge_rate: 50.0,
    spindle_speed: 12_000,
    moves: vec![Move::Rapid {
      to: Point3::new(500.0, 0.0, 5.0),
    }],
  });
  stream
}

impl Harness {
  /// A harness holding the fixture whose readout is S8's own, past its framing frames.
  fn drilling() -> Self {
    let mut harness = Self::bare();
    harness
      .shell
      .set_preview(untouched(), drilling_program(), &job());
    harness.frame(Vec::new());
    harness
  }

  /// The 2D half and the 3D half the last frame reported, as window rectangles.
  fn halves(&self) -> (egui::Rect, egui::Rect) {
    let (two_d, three_d) = self
      .output
      .drawn
      .split
      .expect("the frame reported no split, so the toggle did not open one");
    (rect_of(two_d), rect_of(three_d))
  }

  /// Press and release the point a frame reported, in three passes like a real pointer.
  fn click(&mut self, at: (f32, f32)) {
    let pos = egui::pos2(at.0, at.1);
    let modifiers = egui::Modifiers::default();
    self.frame(vec![egui::Event::PointerMoved(pos)]);
    self.frame(vec![egui::Event::PointerButton {
      pos,
      button: egui::PointerButton::Primary,
      pressed: true,
      modifiers,
    }]);
    self.frame(vec![egui::Event::PointerButton {
      pos,
      button: egui::PointerButton::Primary,
      pressed: false,
      modifiers,
    }]);
  }

  /// Drag the primary button from one point to another, in four passes.
  fn drag(&mut self, from: egui::Pos2, to: egui::Pos2) {
    let modifiers = egui::Modifiers::default();
    self.frame(vec![egui::Event::PointerMoved(from)]);
    self.frame(vec![egui::Event::PointerButton {
      pos: from,
      button: egui::PointerButton::Primary,
      pressed: true,
      modifiers,
    }]);
    self.frame(vec![egui::Event::PointerMoved(to)]);
    self.frame(vec![egui::Event::PointerButton {
      pos: to,
      button: egui::PointerButton::Primary,
      pressed: false,
      modifiers,
    }]);
  }

  /// Scroll one notch at a point, in two passes so the pointer is known to be there first.
  fn wheel(&mut self, at: egui::Pos2, delta: f32) {
    self.frame(vec![egui::Event::PointerMoved(at)]);
    self.frame(vec![egui::Event::MouseWheel {
      unit: egui::MouseWheelUnit::Point,
      delta: egui::vec2(0.0, delta),
      phase: egui::TouchPhase::Move,
      source: egui::MouseWheelSource::Wheel,
      modifiers: egui::Modifiers::default(),
    }]);
  }
}

/// A window rectangle from the four numbers a report uses.
fn rect_of(rect: (f32, f32, f32, f32)) -> egui::Rect {
  egui::Rect::from_min_size(egui::pos2(rect.0, rect.1), egui::vec2(rect.2, rect.3))
}

/// The point a report gave for `word`, in the list it gave it in.
fn hit(hits: &[(String, (f32, f32))], word: &str) -> (f32, f32) {
  hits
    .iter()
    .find(|(label, _)| label == word)
    .unwrap_or_else(|| panic!("no `{word}` was reported, painted: {hits:?}"))
    .1
}

/// The window point a world coordinate was drawn at in the last frame.
fn window_point(harness: &Harness, x: f64, y: f64) -> egui::Pos2 {
  harness
    .output
    .canvas
    .to_window(harness.app.camera().world_to_screen(Point2::new(x, y)))
}

#[test]
fn the_view_toggle_is_drawn_once_at_the_canvases_top_right() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());

  let words: Vec<&str> = harness
    .output
    .drawn
    .view_toggle
    .iter()
    .map(|(word, _)| word.as_str())
    .collect();
  assert_eq!(words, ["2D", "3D"], "the toggle offers the two views");

  let (x, y, width, height) = harness
    .output
    .drawn
    .view_toggle_rect
    .expect("the toggle reported where it was drawn");
  // The canvas is the whole drawing area: the 2D half when the split is open (which is what `canvas` reports), and
  // the 3D half beside it. The toggle sits at the right end of that area, which in this mode is the 3D half's.
  let (two_d, three_d) = harness.halves();
  let (left, top) = (two_d.min.x, two_d.min.y);
  let right = three_d.max.x;
  assert!(
    x >= left + (right - left) / 2.0,
    "the toggle sits in the canvas' right half: {x} in {left}..{right}"
  );
  assert!(
    x + width <= right,
    "and inside the canvas: {} against {right}",
    x + width
  );
  // The top right of the canvas is the strip the ruler's band occupies (S5), which is the canvas' own top
  // edge and not a corner of the drawing area.
  let thickness = cam_ui::theme::Theme::Dark.metrics().ruler_thickness;
  assert!(
    y + height <= top + 1.0 && y >= top - thickness - 1.0,
    "the toggle sits on the canvas' top edge, not down in the drawing: y {y}..{} against {top}",
    y + height
  );

  // One toggle and no second copy: the bar used to carry this control, and two of one control is the
  // defect this rule exists to prevent.
  let painted = |word: &str| harness.texts.iter().filter(|text| *text == word).count();
  assert_eq!(
    painted("2D"),
    1,
    "exactly one `2D`, painted: {:?}",
    harness.texts
  );
  assert_eq!(
    painted("3D"),
    1,
    "exactly one `3D`, painted: {:?}",
    harness.texts
  );
}

#[test]
fn pressing_3d_splits_the_canvas_and_pressing_2d_puts_it_back() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());
  assert!(
    harness.output.drawn.split.is_some(),
    "a shell holding a preview opens on the split"
  );

  let two_d = hit(&harness.output.drawn.view_toggle, "2D");
  harness.click(two_d);
  assert!(!harness.shell.view_3d(), "`2D` turns the mode off");
  assert!(
    harness.output.drawn.split.is_none(),
    "with the mode off there is one canvas, not two halves"
  );
  assert_eq!(
    harness.output.drawn.painted_triangles, 0,
    "and no 3D surface is painted"
  );

  let three_d = hit(&harness.output.drawn.view_toggle, "3D");
  harness.click(three_d);
  assert!(harness.shell.view_3d(), "`3D` turns the mode back on");
  assert!(
    harness.output.drawn.split.is_some(),
    "and the canvas splits again"
  );
}

#[test]
fn the_split_puts_the_drawing_on_the_left_and_the_three_d_view_on_the_right() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());

  let (two_d, three_d) = harness.halves();
  let canvas = harness.output.canvas;
  assert_eq!(
    rect_of((
      canvas.x as f32,
      canvas.y as f32,
      canvas.width as f32,
      canvas.height as f32
    )),
    two_d,
    "the drawing area is the 2D half, which is the rectangle the 2D camera is told about"
  );
  assert!(
    (three_d.min.x - two_d.max.x).abs() <= 1.0,
    "the 3D half begins where the drawing half ends: {} then {}",
    two_d.max.x,
    three_d.min.x
  );
  assert!(
    (three_d.width() - two_d.width()).abs() <= 1.0
      && (three_d.height() - two_d.height()).abs() <= 1.0,
    "the two halves are the same size: {two_d:?} and {three_d:?}"
  );
  // The 3D half inherits the ruler's inset (L20), so both halves' content starts on one line rather than
  // the 3D view reaching up into the band the 2D half gave away.
  assert!(
    (three_d.min.y - two_d.min.y).abs() <= 1.0,
    "the 3D half starts on the drawing's own top line: {} against {}",
    three_d.min.y,
    two_d.min.y
  );
}

#[test]
fn the_drawing_half_keeps_its_pick_and_its_picture_while_the_split_is_open() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());
  assert!(
    harness.output.drawn.painted_items > 0,
    "the drawing is painted in its own half, not hidden behind the 3D view"
  );

  // A point on the fixture rectangle's bottom edge, in the drawing half's own coordinates.
  let edge = window_point(&harness, 50.0, 10.0);
  harness.click((edge.x, edge.y));
  assert!(
    harness.app.selection().contains("E1"),
    "a click in the drawing half still picks: {:?}",
    harness.app.selection().ids().collect::<Vec<_>>()
  );

  // And a drawing tool still draws there: the split takes half the width, not the gesture.
  let before: usize = harness
    .app
    .document()
    .layers
    .iter()
    .map(|layer| layer.entities.len())
    .sum();
  harness.shell.active_tool = cam_ui::Tool::Line;
  harness.drag(
    window_point(&harness, 20.0, 20.0),
    window_point(&harness, 60.0, 60.0),
  );
  let after: usize = harness
    .app
    .document()
    .layers
    .iter()
    .map(|layer| layer.entities.len())
    .sum();
  assert_eq!(
    after,
    before + 1,
    "a drag in the drawing half with a drawing tool armed is still one command"
  );
}

#[test]
fn each_half_keeps_its_own_gesture_while_the_split_is_open() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());
  let (two_d, three_d) = harness.halves();
  let before = *harness.preview().camera();
  let zoom = harness.app.camera().pixels_per_mm();

  // A drag in the drawing half is a marquee: it moves neither the 3D eye nor the 2D zoom.
  let from = two_d.center();
  harness.drag(from, egui::pos2(from.x + 60.0, from.y + 40.0));
  let after_two_d = *harness.preview().camera();
  assert!(
    (after_two_d.yaw - before.yaw).abs() < 1.0e-9
      && (after_two_d.pitch - before.pitch).abs() < 1.0e-9,
    "the drawing half's drag must not orbit the 3D view: {} -> {}",
    before.yaw,
    after_two_d.yaw
  );

  // A wheel over the drawing half zooms the drawing, exactly as it does without the split.
  harness.wheel(two_d.center(), 120.0);
  assert!(
    harness.app.camera().pixels_per_mm() > zoom,
    "the drawing half still zooms about the pointer"
  );

  // A drag in the 3D half orbits it, and a wheel there zooms the 3D eye instead of the drawing.
  let from = three_d.center();
  let drawing_zoom = harness.app.camera().pixels_per_mm();
  let distance = harness.preview().camera().distance.mm();
  harness.wheel(from, 120.0);
  assert!(
    (harness.app.camera().pixels_per_mm() - drawing_zoom).abs() < 1.0e-9,
    "a wheel over the 3D half is not the drawing's zoom"
  );
  assert!(
    harness.preview().camera().distance.mm() < distance,
    "it is the 3D camera's own zoom: {distance} -> {}",
    harness.preview().camera().distance.mm()
  );
  harness.drag(from, egui::pos2(from.x + 80.0, from.y + 40.0));
  let after_three_d = *harness.preview().camera();
  assert!(
    (after_three_d.yaw - before.yaw).abs() > 1.0e-6,
    "the 3D half's drag orbits the eye: {} -> {}",
    before.yaw,
    after_three_d.yaw
  );
}

#[test]
fn the_overlay_row_names_the_four_layers_and_every_chip_hides_exactly_its_own() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());

  let words: Vec<&str> = harness
    .output
    .drawn
    .layer_chips
    .iter()
    .map(|(word, _)| word.as_str())
    .collect();
  assert_eq!(
    words,
    [
      "Material",
      "Vectors",
      "Toolpaths",
      "Tool",
      "Simulate again",
      "Centre",
      "?"
    ],
    "S8's overlay row, in its own order, with M13 S3's `Simulate again` before `Centre`"
  );
  let showing: Vec<(String, bool)> = vec![
    ("Material".to_owned(), true),
    ("Vectors".to_owned(), true),
    ("Toolpaths".to_owned(), true),
    ("Tool".to_owned(), true),
  ];
  assert_eq!(
    harness.output.drawn.scene_layers, showing,
    "every layer starts visible"
  );

  // The fixture document holds one closed four-point rectangle and the preview one segment of four drawn
  // edges, drawn with a tool marker of an axis and a diameter.
  let counts = |harness: &Harness| {
    let drawn = &harness.output.drawn;
    (
      drawn.painted_material_lines,
      drawn.painted_vector_lines,
      drawn.painted_program_lines,
      drawn.painted_tool_lines,
    )
  };
  let whole = [12, 4, 4, 2];
  assert_eq!(
    counts(&harness),
    (whole[0], whole[1], whole[2], whole[3]),
    "twelve stock edges, four vectors, four program edges and the tool's axis and diameter"
  );

  for (word, hidden) in [
    ("Material", 0usize),
    ("Vectors", 1),
    ("Toolpaths", 2),
    ("Tool", 3),
  ] {
    let at = hit(&harness.output.drawn.layer_chips, word);
    harness.click(at);
    let after = counts(&harness);
    if word == "Material" {
      // Hiding the material hides the surface as well as the box: the triangles are the material, and a checkbox
      // that left them on would be half a control.
      assert_eq!(
        harness.output.drawn.painted_triangles, 0,
        "`Material` must take the simulated surface with it"
      );
    }
    for (index, expected) in whole.iter().enumerate() {
      let drawn = [after.0, after.1, after.2, after.3][index];
      if index == hidden {
        assert_eq!(drawn, 0, "`{word}` must hide its own layer");
      } else {
        assert_eq!(drawn, *expected, "`{word}` must leave layer {index} alone");
      }
    }
    assert!(
      !harness.output.drawn.scene_layers[hidden].1,
      "the row must say `{word}` is off in the frame it is pressed"
    );
    // Back on, so the next chip is measured against the whole picture.
    harness.click(at);
    assert_eq!(
      counts(&harness),
      (whole[0], whole[1], whole[2], whole[3]),
      "`{word}` comes back"
    );
    assert!(
      harness.output.drawn.painted_triangles > 0,
      "and so does the surface, with the material"
    );
  }
}

#[test]
fn the_centre_chip_reframes_the_stock_at_the_angle_the_eye_is_at() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());
  let framed = *harness.preview().home_camera();

  let (_, three_d) = harness.halves();
  harness.wheel(three_d.center(), 120.0);
  let from = three_d.center();
  harness.drag(from, egui::pos2(from.x + 80.0, from.y + 40.0));
  let away = *harness.preview().camera();
  assert!(
    (away.distance.mm() - framed.distance.mm()).abs() > 1.0e-6
      && (away.yaw - framed.yaw).abs() > 1.0e-6,
    "the fixture must have moved the eye before `Centre` is asked to put it back"
  );

  harness.click(hit(&harness.output.drawn.layer_chips, "Centre"));
  let back = *harness.preview().camera();
  assert_eq!(
    back.target, framed.target,
    "`Centre` puts the target back on the stock's centre"
  );
  assert!(
    (back.distance.mm() - framed.distance.mm()).abs() < 1.0e-9,
    "and the distance back to the one that frames it: {} against {}",
    back.distance.mm(),
    framed.distance.mm()
  );
  assert!(
    (back.yaw - away.yaw).abs() < 1.0e-9,
    "`Centre` keeps the angle the eye is at — that is what makes it not the home button"
  );
}

#[test]
fn the_home_button_returns_the_picture_to_the_state_it_opened_in() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());
  let framed = *harness.preview().home_camera();

  let (_, three_d) = harness.halves();
  harness.wheel(three_d.center(), 120.0);
  let from = three_d.center();
  harness.drag(from, egui::pos2(from.x + 80.0, from.y + 40.0));
  let away = *harness.preview().camera();
  assert!(
    (away.yaw - framed.yaw).abs() > 1.0e-6,
    "the fixture must have turned the eye before the button is asked to undo it"
  );

  let button = harness
    .output
    .drawn
    .home_button
    .expect("the 3D half must report where its home button was drawn");
  harness.click(button);
  let back = *harness.preview().camera();
  assert_eq!(back.yaw, framed.yaw, "the home button restores the angle");
  assert_eq!(back.pitch, framed.pitch);
  assert_eq!(back.target, framed.target);
  assert_eq!(
    back.distance, framed.distance,
    "and the distance, which is the whole framed state"
  );
}

#[test]
fn the_axis_triad_labels_three_axes_and_turns_with_the_camera() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());

  let words: Vec<&str> = harness
    .output
    .drawn
    .triad
    .iter()
    .map(|(word, _)| word.as_str())
    .collect();
  assert_eq!(words, ["TOP", "FRONT", "RIGHT"], "S8's three labels");

  let before = harness.output.drawn.triad.clone();
  let (_, three_d) = harness.halves();
  let from = three_d.center();
  harness.drag(from, egui::pos2(from.x + 90.0, from.y + 30.0));
  let after = harness.output.drawn.triad.clone();
  assert_ne!(
    before, after,
    "the triad is drawn from the camera, so orbiting must turn it: {before:?} then {after:?}"
  );
}

#[test]
fn the_playback_bar_plays_pauses_and_names_the_move_the_playhead_is_on() {
  let mut harness = Harness::drilling();
  harness.frame(Vec::new());

  let words: Vec<&str> = harness
    .output
    .drawn
    .playback_hits
    .iter()
    .map(|(word, _)| word.as_str())
    .collect();
  for word in ["Play", "To the end", "Step", "Reset", "60x"] {
    assert!(
      words.contains(&word),
      "the bar must offer `{word}`, painted: {words:?}"
    );
  }
  assert_eq!(
    harness.output.drawn.playback_readout, "0:00 / 0:05  T22 · Drilling 1 · Z 5.000",
    "the readout states the two clocks and names the move under the playhead"
  );

  harness.click(hit(&harness.output.drawn.playback_hits, "Play"));
  assert!(
    harness.preview().playing(),
    "`Play` starts the program: {:?}",
    harness.output.drawn.playback_readout
  );
  assert!(
    harness
      .output
      .drawn
      .playback_hits
      .iter()
      .any(|(word, _)| word == "Pause"),
    "and the control names what pressing it again would do"
  );

  harness.click(hit(&harness.output.drawn.playback_hits, "Pause"));
  assert!(!harness.preview().playing(), "`Pause` stops it again");

  // `Step` and `Reset` are the two controls S8's list does not name and this shell keeps (M5 A10): the buttons
  // are wired to the playhead they describe, which is a claim about the bar rather than about the preview.
  harness.click(hit(&harness.output.drawn.playback_hits, "Reset"));
  assert_eq!(
    harness.preview().cursor(),
    0,
    "`Reset` rewinds to the first move"
  );
  harness.click(hit(&harness.output.drawn.playback_hits, "Step"));
  assert_eq!(
    harness.preview().cursor(),
    1,
    "`Step` advances exactly one move"
  );
  assert!(
    !harness.preview().playing(),
    "and leaves playback paused, so the next frame cannot undo the step"
  );
}

#[test]
fn to_the_end_cuts_the_whole_program_and_reads_as_the_references_own_line() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());
  harness.click(hit(&harness.output.drawn.playback_hits, "To the end"));

  assert!(
    harness.preview().finished(),
    "`To the end` plays the program out"
  );
  assert_eq!(
    harness.preview().field().volumes(),
    whole_program_field().volumes(),
    "the field it leaves is the whole-program sweep, so `To the end` is a cut and not a jump cut"
  );
  assert_eq!(
    harness.output.drawn.preview_bar, 1.0,
    "and the bar's fill is the clock's own answer"
  );

  // The reference's own line, from a program that runs exactly five seconds and ends at Z 5.000.
  let mut drilling = Harness::drilling();
  drilling.frame(Vec::new());
  drilling.click(hit(&drilling.output.drawn.playback_hits, "To the end"));
  assert_eq!(
    drilling.output.drawn.playback_readout, "0:05 / 0:05  T22 · Drilling 1 · Z 5.000",
    "S8's readout, in its own words"
  );
}

#[test]
fn the_scrubber_seeks_the_playhead_to_the_point_it_was_pressed() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());
  harness.click(hit(&harness.output.drawn.playback_hits, "To the end"));
  assert!(harness.preview().finished());

  let (left, right, y) = harness
    .output
    .drawn
    .scrubber
    .expect("the bar must report the track it drew");
  assert!(right > left, "the scrubber has a length to seek along");

  // Back to the start: seeking is a re-cut, so nothing is removed at zero. The press is on the track's own left
  // end, because a pixel into a 129 px track is already a tenth of a second of program.
  harness.click((left, y));
  assert_eq!(
    harness.preview().cursor(),
    0,
    "the playhead is back at the first move"
  );
  assert!(
    harness.preview().elapsed().abs() < 1.0e-9,
    "and its clock reads nothing spent: {}",
    harness.preview().elapsed()
  );
  assert_eq!(
    harness.preview().field().volumes(),
    untouched().volumes(),
    "a playhead at zero must leave the stock untouched"
  );

  // Half way along the track is half of the program's own time.
  harness.click(((left + right) * 0.5, y));
  let half = harness.preview().duration() * 0.5;
  assert!(
    (harness.preview().elapsed() - half).abs() < 0.01 * harness.preview().duration(),
    "the middle of the track is half the program: {} against {half}",
    harness.preview().elapsed()
  );
}

#[test]
fn the_speed_control_names_the_speed_it_sets() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());

  let speed = harness
    .output
    .drawn
    .playback_hits
    .iter()
    .find(|(word, _)| word.ends_with('x'))
    .map(|(word, at)| (word.clone(), *at))
    .expect("the bar must name the speed it is set to");
  assert_eq!(speed.0, "60x", "the default speed is the preview's own");
  assert_eq!(harness.preview().speed(), 60.0);

  harness.click(speed.1);
  assert_eq!(
    harness.preview().speed(),
    200.0,
    "the chip steps to the next speed"
  );
  assert!(
    harness
      .output
      .drawn
      .playback_hits
      .iter()
      .any(|(word, _)| word == "200x"),
    "and the frame that set it is the frame that says so: {:?}",
    harness.output.drawn.playback_hits
  );
}

#[test]
fn the_question_mark_opens_the_help_panel() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());
  assert!(!harness.shell.show_help, "the panel starts closed");

  harness.click(hit(&harness.output.drawn.layer_chips, "?"));
  assert!(
    harness.shell.show_help,
    "`?` must do something: it opens the F1 panel, which teaches every command"
  );
}

#[test]
fn a_split_with_no_program_draws_no_overlay_row_and_says_why() {
  let mut harness = Harness::bare();
  harness.shell.set_view_3d(true);
  harness.frame(Vec::new());

  assert!(
    harness.output.drawn.split.is_some(),
    "the split is a mode, so it opens with or without a program"
  );
  assert!(
    harness.output.drawn.layer_chips.is_empty(),
    "no overlay row: a checkbox over nothing is a control that lies"
  );
  assert!(
    harness.output.drawn.playback_hits.is_empty()
      && harness.output.drawn.playback_readout.is_empty(),
    "and no playback bar, for the same reason"
  );
  assert!(
    harness
      .texts
      .iter()
      .any(|text| text.contains("half draws")),
    "the half says what it is for (M13 S3: it never sends the person to another program), painted: {:?}",
    harness.texts
  );
  assert!(
    !harness.texts.iter().any(|text| text.contains("cam-cli")),
    "and it names no other program: {:?}",
    harness.texts
  );
}

#[test]
fn the_rulers_frame_the_drawing_half_and_stop_where_the_three_d_half_begins() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());

  let rulers = &harness.output.drawn.rulers;
  let (_, three_d) = harness.halves();
  let band_end = rulers.top_band.0 + rulers.top_band.2;
  assert!(
    (band_end - three_d.min.x).abs() <= 1.0,
    "the top band ends where the 3D half begins: {band_end} against {}",
    three_d.min.x
  );
  assert!(
    rulers.top_band.3 > 0.0 && !rulers.top.is_empty(),
    "the band still numbers the drawing half: {:?}",
    rulers.top_band
  );
}

// ---- The 3D half simulates the drawing's own program (M13 S3) ------------------------------------

/// The point the empty half drew its `Simulate` chip at, which is the only way a test can press it.
fn simulate_chip(harness: &Harness) -> (f32, f32) {
  harness
    .output
    .drawn
    .simulate_chip
    .expect("the empty half must offer `Simulate`, and report where it drew it")
}

#[test]
fn the_empty_half_offers_simulate_and_pressing_it_draws_the_drawings_own_program() {
  let mut harness = Harness::with_document(document_with_toolpath());
  harness.shell.set_tools(vec![stored_tool()]);
  harness.shell.set_view_3d(true);
  harness.frame(Vec::new());

  assert!(
    harness.shell.preview().is_none(),
    "nothing has been simulated yet, so the half is empty"
  );
  assert_eq!(
    harness.output.drawn.painted_triangles, 0,
    "an empty half paints no picture"
  );
  assert!(
    harness.texts.iter().any(|text| text == "Simulate"),
    "the empty half carries the control that fills it: {:?}",
    harness.texts
  );
  // M13 S3: nothing in the shell may send the person out of it. The relief task's own sentence is chrome.rs'
  // (`chrome.rs:911`, outside this task's surface) and is drawn only while that task is open, which this frame
  // is not — what is asserted is that the half itself names no other program.
  assert!(
    !harness.texts.iter().any(|text| text.contains("cam-cli")),
    "no sentence in this frame sends the person to a terminal: {:?}",
    harness.texts
  );

  let chip = simulate_chip(&harness);
  harness.click(chip);

  assert!(
    harness.shell.preview().is_some(),
    "pressing `Simulate` turns the drawing's own toolpaths into the preview"
  );
  assert!(
    harness.shell.view_3d(),
    "and the picture is the view the person is looking at"
  );
  let expected = format!(
    "simulate — {} of cutting, drawn in 3D",
    clock_text(seconds_for(&document_program()))
  );
  assert_eq!(
    harness.shell.status, expected,
    "the status bar names what was simulated, in the clock's own words"
  );

  harness.frame(Vec::new());
  assert!(
    harness.output.drawn.painted_triangles > 0,
    "the frame after the press draws the document's own program cutting its stock"
  );
}

#[test]
fn simulating_a_drawing_whose_tools_are_not_in_the_shell_refuses_by_name_and_draws_nothing() {
  let mut harness = Harness::with_document(document_with_toolpath());
  // No `set_tools`: the library this window was given is empty, so the toolpath's tool cannot be resolved.
  harness.shell.set_view_3d(true);
  harness.frame(Vec::new());

  let chip = simulate_chip(&harness);
  harness.click(chip);

  assert!(
    harness.shell.status.starts_with("refused:"),
    "a missing tool is a refusal, not a picture: {}",
    harness.shell.status
  );
  assert!(
    harness.shell.status.contains(&format!("T{TOOL_NUMBER}")),
    "the refusal must name the missing tool: {}",
    harness.shell.status
  );
  assert!(
    harness.shell.preview().is_none(),
    "a refused simulation hands the half no preview"
  );

  harness.frame(Vec::new());
  assert_eq!(
    harness.output.drawn.painted_triangles, 0,
    "and the half stays empty: a refusal draws nothing"
  );
  assert!(
    harness.output.drawn.playback_hits.is_empty() && harness.output.drawn.layer_chips.is_empty(),
    "no playback bar and no overlay row over a picture that does not exist"
  );
}

#[test]
fn simulating_a_drawing_with_no_toolpaths_refuses_by_name_and_draws_nothing() {
  let mut harness = Harness::with_document(document());
  harness.shell.set_tools(vec![stored_tool()]);
  harness.shell.set_view_3d(true);
  harness.frame(Vec::new());

  let chip = simulate_chip(&harness);
  harness.click(chip);

  assert!(
    harness.shell.status.starts_with("refused:"),
    "a drawing with no toolpaths has nothing to simulate: {}",
    harness.shell.status
  );
  assert!(
    harness.shell.status.contains("toolpaths"),
    "the refusal must name the missing half: {}",
    harness.shell.status
  );
  assert!(
    harness.shell.preview().is_none(),
    "a refused simulation hands the half no preview"
  );

  harness.frame(Vec::new());
  assert_eq!(
    harness.output.drawn.painted_triangles, 0,
    "and the half stays empty: a refusal draws nothing"
  );
}

#[test]
fn simulate_again_rebuilds_the_picture_from_the_document() {
  let mut harness = Harness::with_document(document_with_toolpath());
  harness.shell.set_tools(vec![stored_tool()]);
  // A picture somebody else produced: the playback fixture, whose clock is its own and not the drawing's.
  harness
    .shell
    .set_preview(untouched(), playback_program(), &job());
  harness.frame(Vec::new());

  let old = harness.preview().duration();
  let drawing = seconds_for(&document_program());
  assert!(
    (old - drawing).abs() > 1.0e-9,
    "the fixture and the drawing must be tellable apart by their clocks: {old} against {drawing}"
  );

  // Play the handed-in program out, so "back at the start" is a fact about the rebuild and not about a
  // playhead that never moved.
  harness.click(hit(&harness.output.drawn.playback_hits, "To the end"));
  assert!(
    (harness.preview().elapsed() - old).abs() < 1.0e-6,
    "the old program has played out: {} of {old}",
    harness.preview().elapsed()
  );

  harness.click(hit(&harness.output.drawn.layer_chips, "Simulate again"));

  assert!(
    harness.shell.status.starts_with("simulate"),
    "the row's press reports itself in the status bar: {}",
    harness.shell.status
  );
  assert!(
    harness.preview().elapsed().abs() < 1.0e-9,
    "the rebuilt picture starts at the beginning of its own program: {}",
    harness.preview().elapsed()
  );
  assert!(
    (harness.preview().duration() - drawing).abs() < 1.0e-9,
    "and it lasts the drawing's own program, not the one that was handed in: {} against {drawing}",
    harness.preview().duration()
  );
  assert_eq!(
    harness.shell.status,
    format!("simulate — {} of cutting, drawn in 3D", clock_text(drawing)),
    "the note names the duration the rebuilt playback bar names"
  );
}
