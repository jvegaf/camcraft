use cam_app::AppState;
use cam_geom::{Contour, Point2};
use cam_model::{Document, Job, Layer, Operation, ProfileParams, Toolpath};
use cam_move::{Move, MoveStream, Point3, Segment, ToolShape};
use cam_sim::{sweep, Heightfield, Volumes};
use cam_store::tools::{CuttingData, StoredTool, ToolGeometry};
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
pub(crate) const TOOL_NUMBER: u32 = 4;

pub(crate) fn job() -> Job {
  Job {
    width: Length::from_mm(STOCK_WIDTH),
    height: Length::from_mm(STOCK_HEIGHT),
    thickness: Length::from_mm(STOCK_THICKNESS),
    ..Job::default_sheet()
  }
}

pub(crate) fn document() -> Document {
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
pub(crate) fn document_with_toolpath() -> Document {
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
/// The rates are the ones the shell's other fixtures use: a 6 mm cutter at 6000 mm/min and 1800 mm/min of
/// plunge — the same physical speeds the fixtures stated in millimetres per second before the library and the
/// clock were both settled on millimetres per minute.
pub(crate) fn stored_tool() -> StoredTool {
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
      shank_diameter: None,
      overall_length: None,
      custom_attributes: None,
    },
    cutting: CuttingData {
      id: format!("G{TOOL_NUMBER}-cut"),
      rate_units: UnitSystem::Mm,
      feed_rate: Some(6000.0),
      plunge_rate: Some(1800.0),
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
pub(crate) fn document_program() -> MoveStream {
  cam_program::program_for_snapshot(&document_with_toolpath(), &[stored_tool()], None)
    .expect("the fixture's toolpath and its tool agree")
}

/// A tiny deterministic program: one pocket run at `CUT_DEPTH` with a 6 mm tool.
pub(crate) fn program() -> MoveStream {
  let mut stream = MoveStream::new(Length::from_mm(5.0), Point2::new(0.0, 0.0));
  stream.segments.push(Segment {
    name: "pocket 1".to_owned(),
    tool_number: 1,
    tool: ToolShape::cylinder(Length::from_mm(6.0)),
    feed_rate: 6000.0,
    plunge_rate: 1800.0,
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
pub(crate) fn untouched() -> Heightfield {
  Heightfield::new(&job(), Length::from_mm(CELL_MM)).expect("the grid fits the cap")
}

/// The fixture's program simulated over its own stock from scratch: the expectation playback must
/// reproduce (the report's own numbers, A10).
pub(crate) fn whole_program_field() -> Heightfield {
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
/// 20 mm at 6000 mm/min, a plunge of 10 mm at 3000 mm/min, a feed of 50 mm at 6000 mm/min and a retract of
/// 10 mm at 6000 mm/min. `MoveStream::new` places the machine at `(10, 0, safe_z)` before the first move,
/// so the rapid's length is the 20 mm the numbers state. The plunge cuts a disc and the feed cuts a
/// capsule, so the removed volume changes when the cursor reaches the third move, not before.
pub(crate) fn playback_program() -> MoveStream {
  let mut stream = MoveStream::new(Length::from_mm(5.0), Point2::new(10.0, 0.0));
  stream.segments.push(Segment {
    name: "playback".to_owned(),
    tool_number: 1,
    tool: ToolShape::cylinder(Length::from_mm(6.0)),
    feed_rate: 6000.0,
    plunge_rate: 3000.0,
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
pub(crate) fn closed_form_volumes(moves: usize) -> Volumes {
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
pub(crate) struct Harness {
  ctx: egui::Context,
  pub(crate) shell: Shell,
  pub(crate) app: AppState,
  pub(crate) output: ShellOutput,
  /// Vertex count of every `egui::Mesh` the frame produced, which is what A11 is about.
  ///
  /// A vertex count rather than a mesh count because `egui` tessellates a few of its own decorations
  /// into small meshes too: the surface is the one whose vertex count is three per painted triangle.
  pub(crate) mesh_vertices: Vec<usize>,
  /// Every piece of text the frame drew.
  pub(crate) texts: Vec<String>,
  /// Every shape the frame drew, so a test can read the colours the panel painted (T9).
  pub(crate) shapes: Vec<egui::epaint::ClippedShape>,
}

impl Harness {
  /// A harness with no preview at all, whose layout has settled: the M1–M3 shell exactly.
  pub(crate) fn bare() -> Self {
    Self::with_document(document())
  }

  /// A harness over `document`, with no preview and its layout settled.
  ///
  /// The same three framing frames [`Harness::bare`] runs: a test that puts another drawing in the window
  /// still needs the panels laid out before it presses anything (M13 S3's fixtures).
  pub(crate) fn with_document(document: Document) -> Self {
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
  pub(crate) fn new() -> Self {
    let mut harness = Self::bare();
    harness.shell.set_preview(untouched(), program(), &job());
    harness.frame(Vec::new());
    harness
  }

  /// A harness holding the playback fixture, whose move durations the closed form states exactly.
  pub(crate) fn playing() -> Self {
    let mut harness = Self::bare();
    harness
      .shell
      .set_preview(untouched(), playback_program(), &job());
    harness.frame(Vec::new());
    harness
  }

  /// The preview the shell holds, which every playback test drives.
  pub(crate) fn preview(&self) -> &Preview3d {
    self.shell.preview().expect("the harness loaded a preview")
  }

  pub(crate) fn frame(&mut self, events: Vec<egui::Event>) {
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
  pub(crate) fn in_panel(&self) -> egui::Pos2 {
    egui::pos2(600.0, 200.0)
  }
}

/// A program that quotes the reference frame's own playback readout: `T22 · Drilling 1 · Z 5.000`.
///
/// One segment, one rapid: 500 mm at the segment's 6000 mm/min feed is exactly five seconds, and the move
/// arrives at `Z 5.000`, which is the line S8 writes down. Nothing is cut — the point of this fixture is the
/// clock and the name of the move, not the surface.
fn drilling_program() -> MoveStream {
  let mut stream = MoveStream::new(Length::from_mm(5.0), Point2::new(0.0, 0.0));
  stream.segments.push(Segment {
    name: "Drilling 1".to_owned(),
    tool_number: 22,
    tool: ToolShape::cylinder(Length::from_mm(5.0)),
    feed_rate: 6000.0,
    plunge_rate: 3000.0,
    spindle_speed: 12_000,
    moves: vec![Move::Rapid {
      to: Point3::new(500.0, 0.0, 5.0),
    }],
  });
  stream
}

impl Harness {
  /// A harness holding the fixture whose readout is S8's own, past its framing frames.
  pub(crate) fn drilling() -> Self {
    let mut harness = Self::bare();
    harness
      .shell
      .set_preview(untouched(), drilling_program(), &job());
    harness.frame(Vec::new());
    harness
  }

  /// The 2D half and the 3D half the last frame reported, as window rectangles.
  pub(crate) fn halves(&self) -> (egui::Rect, egui::Rect) {
    let (two_d, three_d) = self
      .output
      .drawn
      .split
      .expect("the frame reported no split, so the toggle did not open one");
    (rect_of(two_d), rect_of(three_d))
  }

  /// Press and release the point a frame reported, in three passes like a real pointer.
  pub(crate) fn click(&mut self, at: (f32, f32)) {
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
  pub(crate) fn drag(&mut self, from: egui::Pos2, to: egui::Pos2) {
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
  pub(crate) fn wheel(&mut self, at: egui::Pos2, delta: f32) {
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
pub(crate) fn rect_of(rect: (f32, f32, f32, f32)) -> egui::Rect {
  egui::Rect::from_min_size(egui::pos2(rect.0, rect.1), egui::vec2(rect.2, rect.3))
}

/// The point a report gave for `word`, in the list it gave it in.
pub(crate) fn hit(hits: &[(String, (f32, f32))], word: &str) -> (f32, f32) {
  hits
    .iter()
    .find(|(label, _)| label == word)
    .unwrap_or_else(|| panic!("no `{word}` was reported, painted: {hits:?}"))
    .1
}

/// The window point a world coordinate was drawn at in the last frame.
pub(crate) fn window_point(harness: &Harness, x: f64, y: f64) -> egui::Pos2 {
  harness
    .output
    .canvas
    .to_window(harness.app.camera().world_to_screen(Point2::new(x, y)))
}
