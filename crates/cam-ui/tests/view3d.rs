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

use cam_app::AppState;
use cam_geom::{Contour, Point2};
use cam_model::{Document, Job, Layer};
use cam_move::{Move, MoveStream, Point3, Segment, ToolShape};
use cam_sim::{sweep, Heightfield, Volumes};
use cam_ui::theme::{Palette, Role, Theme};
use cam_ui::{Preview3d, Shell, ShellOutput};
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
    let mut harness = Self {
      ctx: egui::Context::default(),
      shell: Shell::new(),
      app: AppState::new(document()),
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
fn the_two_views_are_exclusive() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());

  // 3D is showing: it owns the central area, and the drawing is not painted at all. The old shell stacked
  // the preview over the canvas, which gave both half a screen and neither a usable one.
  assert!(
    harness.output.drawn.painted_triangles > 0,
    "the 3D view is the one showing"
  );
  assert_eq!(
    harness.output.drawn.painted_items, 0,
    "the 2D scene must not be painted behind the 3D view"
  );
  assert_eq!(
    harness.output.drawn.painted_grid_lines, 0,
    "and neither must the 2D grid"
  );

  // Back to the drawing: the triangle count drops to zero and the scene comes back, whole.
  harness.shell.set_view_3d(false);
  harness.frame(Vec::new());
  assert_eq!(
    harness.output.drawn.painted_triangles, 0,
    "a hidden view paints no mesh"
  );
  assert_eq!(
    harness.output.drawn.painted_items, 4,
    "the four rectangle edges are back (A12)"
  );
  assert!(
    harness.output.drawn.painted_grid_lines > 0,
    "and so is the grid"
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

  for text in ["Play", "Step", "Reset"] {
    assert!(
      harness.texts.iter().any(|painted| painted == text),
      "the panel must offer a {text} control, painted texts: {:?}",
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
