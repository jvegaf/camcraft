//! The 3D preview panel: a simulated stock, its camera, its playback, and the painter's mesh (M5 A9,
//! A10, A11, A12).
//!
//! # What this is, and what it is not
//!
//! The panel **shows** a program; it never authors one and never touches the document (D-M5-9, S3:
//! "La ventana muestra el preview del programa ya autorado por `cam-cli`"). The caller hands in the
//! stock it has not cut yet — [`cam_sim::Heightfield::new`] — and the program to cut it with, through
//! [`Shell::set_preview`](crate::Shell::set_preview), together with the [`Job`] the stock box is built
//! from. The panel adds no [`UiAction`](crate::UiAction) and no path from the picture back into the
//! geometry: playing a program moves a cursor, never the document.
//!
//! # Playback, and why the time is injected
//!
//! A10 asks for a program that plays, with material removed as the cursor advances. The arithmetic
//! lives here as a pure function of the delta a caller hands to [`Preview3d::advance`]: nothing in
//! this module reads a clock, so a test drives frames and asserts the same result a window would
//! show. The crate's single clock read is in [`Shell::show_in`](crate::Shell::show_in), which asks
//! `egui` for `stable_dt` and passes it down.
//!
//! The time model is the program's own: a move takes its length over the segment's rate —
//! `plunge_rate` for a move that only plunges, `feed_rate` otherwise, arcs measured along their own
//! circle — and `advance(dt)` consumes `dt * speed` of those seconds. A move is applied to the field
//! only when the cursor crosses it whole, so a half-played move has removed nothing yet, and the
//! field at the end is cell-for-cell the field [`cam_sim::sweep::apply`] produces for the whole
//! stream: the animation and the report cannot disagree.
//!
//! # Colours, and where they come from
//!
//! The panel chooses no colour of its own: every one it paints is a [`Role`] of the current theme,
//! or a stop on the depth ramp between two of them. The 3D view used to keep five shading literals
//! (L7 left them there on purpose), which made it the last place in the shell painting a colour no
//! role knew about. The mapping is a meaning, not a rename: the stock's box edges are
//! [`Role::StockEdge`] — a silhouette, not the [`Role::Border`] hairline the 2D canvas outlines the
//! stock with, because a block against a field with no grid needs a value that clears both the canvas
//! and its own face; cutting moves are
//! [`Role::Cut`], which is what the cut signal means; rapids and retracts are [`Role::TextDim`], the
//! role the 2D canvas paints construction geometry with — present, but not the thing being shown;
//! and the surface is [`Palette::stock_shade`], a ramp with a role at each end. The playback bar's
//! fill is [`Role::Cut`] for the same reason the cutting moves are: it measures how much of the
//! program has been cut, so the amber in the picture and the amber in the bar say one thing. Nothing
//! here keeps a colour literal, and the bar's own test finds both of its rectangles by role rather
//! than by position (`tests/view3d.rs`), because a literal would paint a bar no role test could see.
//!
//! # Why the geometry is not computed here
//!
//! The projection ([`Camera3d`]) and the surface ([`build_surface`]) are data, tested headless in
//! `cam-view` (A9, S2). This module is the painter: it turns that data into [`egui::Mesh`] triangles
//! and lines on the existing [`egui::Painter`], which is why the 3D view needs no new dependency
//! (A11, D-M5-7). A `wgpu` backend would consume the same [`cam_view::SurfaceMesh`] when S12's
//! trigger fires; nothing here would have to change to allow it.

use cam_app::{Selection, Viewport};
use cam_model::Job;
use cam_move::{Move, MoveStream, Point3, ToolShape};
use cam_sim::{sweep, Heightfield};
use cam_units::Length;
use cam_view::{
  build_scene, build_stock_box, build_surface, Camera3d, Line3, SceneItem, FIELD_OF_VIEW,
  MIN_DISTANCE,
};
use egui::{Pos2, Stroke};

use crate::theme::{self, Palette, Role};
use crate::{DrawnState, Shell};

/// How far the camera turns per pixel of drag, in radians.
///
/// A tenth of a radian per hundred pixels: a full turn is about 630 px of dragging, which is the
/// "pick it up and turn it" feel of a modelling view rather than the twitchy one of a trackball.
pub const ORBIT_RADIANS_PER_PIXEL: f64 = 0.01;

/// The largest number of grid lines the surface samples along its longer side.
///
/// The mesh's triangle count is bounded by the decimation stride, not by the job's cell count (S7,
/// D-M5-7): `200` lines give at most 199 x 199 x 2 triangles, which the CPU painter draws in one
/// frame for any stock A6's cell cap admits.
pub const MAX_LATTICE: usize = 200;

/// How many seconds of the program's own time one second of wall clock consumes by default.
///
/// A 6 mm tool cutting at 100 mm/s takes about seven minutes over a 40 m pocket: a correct program
/// and an unwatchable preview. Sixty makes that a seven second animation, and
/// [`Preview3d::set_speed`] is how a caller retells it.
pub const DEFAULT_SPEED: f64 = 60.0;

/// How much larger than the stock's diagonal the camera's distance is when the panel opens, so the
/// stock sits inside the picture with a margin rather than touching its edges.
const FRAME_MARGIN: f64 = 1.4;

/// How much of a pixel an arc may deviate by when the painter flattens it, in pixels.
///
/// The same half-pixel rule the 2D canvas uses: the flattening belongs to the painter, and the
/// tolerance is expressed in what the user can see, not in millimetres.
const ARC_TOLERANCE_PX: f64 = 0.5;

/// How long the tool marker's axis is, in millimetres, above the playhead's own point.
///
/// A marker's length and not the tool's: the program carries where the tip is and how wide the cutter is, and
/// no tool length at all, so this is the one dimension of the marker that is the shell's own. Twelve
/// millimetres reads as a tool beside a 6 mm cutter on an 18 mm sheet without pretending to be the spindle.
const TOOL_MARKER_MM: f64 = 12.0;

/// How far apart a vector is allowed to be from its arc when the 3D half projects one, in millimetres.
///
/// A fixed millimetre rather than a pixel, unlike the program's own flattening: a vector is a light overlay on
/// a 3D face, and a tolerance that followed the camera would rebuild the whole document's edges every time the
/// wheel turned.
const VECTOR_TOLERANCE_MM: f64 = 0.1;

/// How many lines the overlay row may wrap to before it stops.
///
/// Two, because that is what the reference frame uses: the four checkboxes and `Centre` sit on the first line and
/// the `?` drops to the second when the half is narrow. A row that wrapped for ever would walk down the picture.
const OVERLAY_ROWS: usize = 2;

/// How tall one line of the overlay row is, in points.
const OVERLAY_LINE_HEIGHT: f32 = 18.0;

/// How tall the 3D half's playback bar is, in points: one row of controls and the readout's own line.
const PLAYBACK_BAR_HEIGHT: f32 = 46.0;

/// How wide the square at the 3D half's top right is: the home button above, the axis triad below.
const CORNER_WIDTH: f32 = 86.0;

/// How tall that square is.
const CORNER_HEIGHT: f32 = 68.0;

/// How long one arm of the axis triad is, in points.
const TRIAD_LENGTH_PX: f32 = 20.0;

/// How far a triad's label sits beyond its arm, in points, and how far the triad sits above the corner's foot.
const TRIAD_LABEL_PAD: f32 = 10.0;

/// The speeds the speed chip steps through, in program seconds per second of wall clock.
///
/// Five stops instead of a slider: the reference's bar names one speed, and these are the five an operator
/// actually uses — real time, then fast enough to watch a pocket, then fast enough to watch a program. The
/// chip names the one it is set to, so the control says what it did rather than where a handle sits.
pub const SPEEDS: [f64; 5] = [1.0, 5.0, 20.0, 60.0, 200.0];

/// The word the speed chip writes: `60x`, and `1x` for one second per second rather than `1.0000001x` off a
/// float. A speed that is not a positive number has no word, and `—` says so instead of naming a lie.
pub fn speed_text(speed: f64) -> String {
  if !speed.is_finite() || speed <= 0.0 {
    return "—".to_owned();
  }
  let rounded = speed.round();
  if (speed - rounded).abs() < 1.0e-9 {
    format!("{}x", rounded as i64)
  } else {
    format!("{speed}x")
  }
}

/// The next speed in [`SPEEDS`] after `speed`, wrapping back to the first at the top of the ladder.
///
/// A speed a caller set outside the ladder steps to the first stop above it, so a host that handed in `30`
/// and pressed the chip gets `60`, not `1`.
pub fn next_speed(speed: f64) -> f64 {
  SPEEDS
    .iter()
    .copied()
    .find(|step| *step > speed + 1.0e-9)
    .unwrap_or(SPEEDS[0])
}

/// One of the four things the 3D half draws, and the overlay row's checkbox for it (camui-prancheta S8).
///
/// The four are the whole picture: a checkbox that hid nothing would be a control that lies, so this enum
/// and [`Preview3d`]'s painters are the same list read twice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
  /// The material: the stock's box and the simulated surface.
  Material,
  /// The document's vectors, laid on the stock's top face.
  Vectors,
  /// The program's moves.
  Toolpaths,
  /// The tool, at the playhead.
  Tool,
}

impl Layer {
  /// Every layer, in the overlay row's own order.
  pub const ALL: [Layer; 4] = [
    Layer::Material,
    Layer::Vectors,
    Layer::Toolpaths,
    Layer::Tool,
  ];

  /// The word the overlay row writes on this layer's chip.
  pub fn label(self) -> &'static str {
    match self {
      Self::Material => "Material",
      Self::Vectors => "Vectors",
      Self::Toolpaths => "Toolpaths",
      Self::Tool => "Tool",
    }
  }
}

/// Which of the 3D half's layers the overlay row is showing (camui-prancheta S8).
///
/// View state, like the camera: hiding the toolpaths changes what the picture shows and nothing about the
/// document, so it is neither an edit nor an undo step. Every layer starts visible, because a view that
/// opened with three of its four layers hidden would look like a bug in the program rather than a choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layers {
  /// The stock's box and the simulated surface.
  pub material: bool,
  /// The document's vectors.
  pub vectors: bool,
  /// The program's moves.
  pub toolpaths: bool,
  /// The tool, at the playhead.
  pub tool: bool,
}

impl Default for Layers {
  fn default() -> Self {
    Self {
      material: true,
      vectors: true,
      toolpaths: true,
      tool: true,
    }
  }
}

impl Layers {
  /// Whether this layer is showing.
  pub fn get(self, layer: Layer) -> bool {
    match layer {
      Layer::Material => self.material,
      Layer::Vectors => self.vectors,
      Layer::Toolpaths => self.toolpaths,
      Layer::Tool => self.tool,
    }
  }

  /// Show or hide this layer.
  pub fn set(&mut self, layer: Layer, on: bool) {
    match layer {
      Layer::Material => self.material = on,
      Layer::Vectors => self.vectors = on,
      Layer::Toolpaths => self.toolpaths = on,
      Layer::Tool => self.tool = on,
    }
  }

  /// The four chips as the frame drew them: each layer's word and whether it is showing, in
  /// [`Layer::ALL`]'s order.
  pub fn words(self) -> Vec<(String, bool)> {
    Layer::ALL
      .iter()
      .map(|layer| (layer.label().to_owned(), self.get(*layer)))
      .collect()
  }
}

/// One program's preview: the simulated stock, the camera that looks at it, and where playback stands.
///
/// Built by [`Preview3d::new`] and handed to the shell by
/// [`Shell::set_preview`](crate::Shell::set_preview). The field is cut **by playback**: `field` is
/// what the surface is drawn from, `pristine` is the stock before the first move, and
/// [`Preview3d::reset`] puts one back in the other.
#[derive(Clone, Debug, PartialEq)]
pub struct Preview3d {
  /// The live stock: the heights the surface is drawn from, cut as the cursor advances.
  field: Heightfield,
  /// The untouched stock, which is the field playback starts from and returns to.
  pristine: Heightfield,
  /// The program the surface came from, drawn as 3D lines.
  program: MoveStream,
  /// The program flattened into timed steps, in program order.
  steps: Vec<Step>,
  /// The running total of the steps' durations, in program seconds. Always `steps.len() + 1` long.
  cumulative: Vec<f64>,
  /// How many moves the cursor has crossed. A move is applied to the field when it is crossed.
  cursor: usize,
  /// How much of the move at [`Preview3d::cursor`] is consumed, in `0.0..=1.0`.
  ///
  /// It is exactly `1.0` only when the program has finished: a move still in progress is applied to
  /// the field only when the cursor crosses it, so this is where the frame is, not what is cut.
  fraction: f64,
  /// Seconds of the program's own time per second of the caller's, clamped to a finite `>= 0`.
  speed: f64,
  /// Whether [`Preview3d::advance`] consumes time, and whether the panel asks for another frame.
  playing: bool,
  /// Where the eye is.
  camera: Camera3d,
  /// The camera the view opens with: the stock framed at the default three-quarter angle.
  ///
  /// Kept so the two framing controls mean different things: [`Preview3d::home`] restores this whole
  /// camera, and [`Preview3d::recentre`] takes only its target and distance, which re-frames the stock at
  /// whatever angle the eye is at now (camui-prancheta S8).
  home: Camera3d,
  /// The stock's twelve edges, built once from the job so the panel does not need it later.
  stock: Vec<Line3>,
  /// Decimation factor for the surface mesh.
  stride: usize,
}

/// One movement of the program, with everything playback needs to apply it and to time it.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Step {
  /// The movement itself.
  movement: Move,
  /// Where the sweep starts: exactly the origin [`sweep::apply`] uses for this move.
  sweep_from: Point3,
  /// The tool's shape, carried from the segment so the preview's own field is cut by the tool the
  /// program names rather than by a widened disc (M6 A8).
  tool: ToolShape,
  /// How long the move takes, in the program's own seconds.
  seconds: f64,
  /// The segment the move belongs to, so the playback readout can name the tool and the toolpath the
  /// playhead is on without holding a second copy of either (camui-prancheta S8).
  segment: usize,
}

impl Preview3d {
  /// A preview of `program` cutting the untouched stock `field`, over the stock `job` describes.
  ///
  /// `field` must be the stock before the first move — [`Heightfield::new`] for `job`, at the cell
  /// size the caller chose — because playback cuts it as it plays and [`Preview3d::reset`] restores
  /// it. Handing in an already-simulated field would leave the picture and the program disagreeing
  /// about where the first move starts. `job` also supplies the stock box's extent. The camera is
  /// framed on the stock with a margin and the stride is chosen so the mesh's triangle count is
  /// bounded (see [`MAX_LATTICE`]).
  pub fn new(field: Heightfield, program: MoveStream, job: &Job) -> Self {
    let stride = stride_for(&field);
    let stock = build_stock_box(job);
    let camera = frame_camera(job);
    let (steps, cumulative) = steps_for(&program);
    let fraction = if steps.is_empty() { 1.0 } else { 0.0 };
    Self {
      pristine: field.clone(),
      field,
      program,
      steps,
      cumulative,
      cursor: 0,
      fraction,
      speed: DEFAULT_SPEED,
      playing: false,
      camera,
      home: camera,
      stock,
      stride,
    }
  }

  /// The simulated stock this preview draws.
  pub fn field(&self) -> &Heightfield {
    &self.field
  }

  /// The program this preview draws.
  pub fn program(&self) -> &MoveStream {
    &self.program
  }

  /// The camera, for a caller that wants to frame or test the view.
  pub fn camera(&self) -> &Camera3d {
    &self.camera
  }

  /// The camera, for a caller that sets a field directly instead of going through a gesture.
  pub fn camera_mut(&mut self) -> &mut Camera3d {
    &mut self.camera
  }

  /// The stock's edges, in machine space.
  pub fn stock(&self) -> &[Line3] {
    &self.stock
  }

  /// The camera the view opened with: the stock framed at the default three-quarter angle.
  ///
  /// The home button restores it and `Centre` re-frames to it, which is what makes the two controls
  /// different: one is the state the picture started in, the other is its framing at the angle the eye
  /// has turned to since (camui-prancheta S8).
  pub fn home_camera(&self) -> &Camera3d {
    &self.home
  }

  /// Return the picture to the state it opened in: the default angle, the stock's centre as the target
  /// and the distance that frames it.
  ///
  /// The viewport is **not** part of the framing — it is this frame's size for the region the half was
  /// given — so it survives the call; restoring it to the camera the preview was built with would make the
  /// picture vanish for a frame every time the button was pressed.
  pub fn home(&mut self) {
    let viewport = self.camera.viewport;
    self.camera = self.home;
    self.camera.viewport = viewport;
  }

  /// Re-frame the stock at the angle the eye is at now: the target back on the stock's centre and the
  /// distance back to the one that frames it, with the orbit's two angles untouched.
  ///
  /// This is `Centre`, and it is not the home button: a view that has been zoomed into a corner wants its
  /// framing back without losing the angle the operator turned to, and a view that has been turned wants the
  /// angle it opened at. Two controls, because they are two wishes.
  pub fn recentre(&mut self) {
    self.camera.target = self.home.target;
    self.camera.distance = self.home.distance;
  }

  /// Play the rest of the program at once, leaving playback paused at its end.
  ///
  /// Every move after the cursor is applied **in order** and then the cursor lands on the last one, so the
  /// field this leaves is the whole-program [`sweep::apply`] result: `To the end` is a cut and not a jump
  /// cut, and the bar's full fill is a measurement rather than a claim.
  pub fn to_the_end(&mut self) {
    while self.cursor < self.steps.len() {
      self.apply(self.cursor);
      self.cursor += 1;
    }
    self.fraction = 1.0;
    self.settle();
  }

  /// Put the playhead at `fraction` of the program's own time, in `0.0..=1.0`.
  ///
  /// Seeking is a **re-cut**, not a jump: material is removed by crossing a move, and nothing puts it back,
  /// so the field returns to the untouched stock and every move before the playhead is applied again in
  /// order. That is the only way back to a time before the cursor, and it is why the field after a seek is
  /// exactly the sweep of the program's prefix instead of a picture nobody could have cut.
  pub fn seek(&mut self, fraction: f64) {
    let fraction = if fraction.is_finite() {
      fraction.clamp(0.0, 1.0)
    } else {
      0.0
    };
    let target = fraction * self.duration();
    self.field = self.pristine.clone();
    self.playing = false;
    self.cursor = 0;
    self.fraction = 0.0;
    // Every move that ends at or before the target is crossed whole; `cumulative` is the clock's own running
    // total, so the walk uses the same numbers the readout does.
    while self.cursor < self.steps.len() && self.cumulative[self.cursor + 1] <= target {
      self.apply(self.cursor);
      self.cursor += 1;
    }
    if let Some(step) = self.steps.get(self.cursor) {
      let base = self.cumulative[self.cursor];
      if step.seconds > 0.0 && target > base {
        self.fraction = ((target - base) / step.seconds).clamp(0.0, 1.0);
      }
    }
    self.settle();
  }

  /// The move the playhead is on: the tool's number, the toolpath's name and the Z the move arrives at.
  ///
  /// A finished program answers with its last move, because that is where the playhead stands rather than
  /// nowhere. `None` for a program with no moves at all, which has no move to name.
  pub fn playhead(&self) -> Option<(u32, &str, f64)> {
    let step = self.step_under_playhead()?;
    let segment = self.program.segments.get(step.segment)?;
    Some((
      segment.tool_number,
      segment.name.as_str(),
      step.movement.destination().z.mm(),
    ))
  }

  /// The playhead as the playback bar writes it: `T22 · Drilling 1 · Z 5.000`.
  ///
  /// The three facts are the ones the reference frame's playback bar carries: which tool is cutting, which
  /// toolpath it is in, and where the tool is. A program with no moves says so rather than leaving the
  /// readout with a dangling separator.
  pub fn playhead_text(&self) -> String {
    match self.playhead() {
      Some((number, name, z)) => format!("T{number} · {name} · Z {z:.3}"),
      None => "no move to name".to_owned(),
    }
  }

  /// The move the playhead is on: the cursor's, or the last one once the program is played out.
  fn step_under_playhead(&self) -> Option<Step> {
    if self.cursor < self.steps.len() {
      self.steps.get(self.cursor).copied()
    } else {
      self.steps.last().copied()
    }
  }

  /// The surface mesh's decimation factor; `1` samples every cell.
  pub fn stride(&self) -> usize {
    self.stride
  }

  /// Set the surface mesh's decimation factor. `0` is read as `1`, never as a divisor.
  pub fn set_stride(&mut self, stride: usize) {
    self.stride = stride.max(1);
  }

  /// How many moves the cursor has crossed.
  ///
  /// This is the number of moves applied to [`Preview3d::field`], which is why a program played to
  /// its end leaves the same field [`sweep::apply`] does.
  pub fn cursor(&self) -> usize {
    self.cursor
  }

  /// How much of the move at the cursor is consumed, in `0.0..=1.0`.
  ///
  /// A move that is only partly consumed has not been applied to the field yet: whole moves are what
  /// remove material, so the cut is always a program state the machine really reaches.
  pub fn fraction(&self) -> f64 {
    self.fraction
  }

  /// Whether the program has been played to its end.
  pub fn finished(&self) -> bool {
    self.cursor >= self.steps.len()
  }

  /// Whether [`Preview3d::advance`] is consuming program time.
  pub fn playing(&self) -> bool {
    self.playing
  }

  /// Start or stop playback.
  ///
  /// Asking a finished program to play does nothing: a caller that wants to watch it again calls
  /// [`Preview3d::reset`] first, so "play" never silently rewinds the picture.
  pub fn set_playing(&mut self, playing: bool) {
    self.playing = playing && !self.finished();
  }

  /// Flip [`Preview3d::playing`].
  pub fn toggle_playing(&mut self) {
    self.set_playing(!self.playing);
  }

  /// Seconds of the program's own time consumed per second of the caller's.
  pub fn speed(&self) -> f64 {
    self.speed
  }

  /// Set the playback speed. A value that is not a finite, positive number keeps the current one;
  /// `0` freezes the clock without pausing, which is what a caller scrubbing the speed wants.
  pub fn set_speed(&mut self, speed: f64) {
    if speed.is_finite() && speed >= 0.0 {
      self.speed = speed;
    }
  }

  /// How many seconds of the program's own time it lasts.
  pub fn duration(&self) -> f64 {
    total_seconds(&self.cumulative)
  }

  /// How many seconds of the program's own time the cursor has consumed.
  pub fn elapsed(&self) -> f64 {
    let base = self
      .cumulative
      .get(self.cursor)
      .copied()
      .unwrap_or_else(|| self.duration());
    match self.steps.get(self.cursor) {
      Some(step) => base + step.seconds * self.fraction,
      None => base,
    }
  }

  /// How much of the program is behind the cursor, in `0.0..=1.0`.
  ///
  /// A finished program reports exactly `1.0`, which is the value the panel's readout ends on; a
  /// program whose moves all take zero time reports `1.0` once they are all crossed and `0.0`
  /// before, so the readout never claims progress the cursor does not have.
  pub fn progress(&self) -> f64 {
    if self.finished() {
      return 1.0;
    }
    let total = self.duration();
    if total <= 0.0 {
      return 0.0;
    }
    (self.elapsed() / total).clamp(0.0, 1.0)
  }

  /// Consume `dt * speed` seconds of the program's own time (A10).
  ///
  /// `dt` is a delta the caller measured and hands in — this module reads no clock, so a test that
  /// drives frames gets exactly what a window would show. A delta that is not a finite, positive
  /// number changes nothing, and neither does advancing while paused. Each move is applied to the
  /// field the moment the cursor crosses it whole; a partly consumed move removes nothing. Reaching
  /// the end stops playback rather than looping, so the picture never restarts behind the user's
  /// back. The whole function is a pure function of its inputs: same deltas in, same field out.
  pub fn advance(&mut self, dt: f64) {
    if !self.playing || !dt.is_finite() || dt <= 0.0 {
      return;
    }
    let mut budget = dt * self.speed;
    if !budget.is_finite() {
      budget = self.duration();
    }
    while self.cursor < self.steps.len() {
      let step = self.steps[self.cursor];
      let remaining = step.seconds * (1.0 - self.fraction);
      if budget < remaining {
        if step.seconds > 0.0 {
          self.fraction = (self.fraction + budget / step.seconds).clamp(0.0, 1.0);
        }
        return;
      }
      budget -= remaining.max(0.0);
      self.apply(self.cursor);
      self.cursor += 1;
      self.fraction = 0.0;
    }
    self.settle();
  }

  /// Advance exactly one whole move, and leave playback paused there.
  ///
  /// Stepping is a paused advance on purpose: a step the next frame would immediately undo is not a
  /// step. At the end of the program a step changes nothing — [`Preview3d::reset`] is the way back.
  pub fn step(&mut self) {
    self.playing = false;
    if self.cursor >= self.steps.len() {
      return;
    }
    self.apply(self.cursor);
    self.cursor += 1;
    self.fraction = 0.0;
    self.settle();
  }

  /// Put the whole stock back and rewind to the first move, paused.
  pub fn reset(&mut self) {
    self.field = self.pristine.clone();
    self.cursor = 0;
    self.fraction = if self.steps.is_empty() { 1.0 } else { 0.0 };
    self.playing = false;
  }

  /// Apply one whole move to the field.
  ///
  /// The crash a rapid represents is the report's business, not the preview's (D-M5-4): the caller
  /// already simulated this program to produce the report, and a preview that grew its own crash list
  /// would be a second opinion about a defect. [`sweep::apply_move`] never cuts a crashing rapid, so
  /// the picture stays honest either way.
  fn apply(&mut self, index: usize) {
    let step = self.steps[index];
    let _ = sweep::apply_move(&mut self.field, step.sweep_from, step.movement, step.tool);
  }

  /// Stop playback once there is nothing left to play, and mark the last move consumed.
  fn settle(&mut self) {
    if self.finished() {
      self.playing = false;
      self.fraction = 1.0;
    }
  }

  /// Paint the preview into the region the frame hands it, and take the orbit and zoom gestures it owns.
  ///
  /// A8 makes the 3D view a **half** of the canvas rather than the whole of it (camui-prancheta S8), so the
  /// region is an argument: the caller decides where the half is, and the camera learns that region's size
  /// every frame. The camera therefore learns a new viewport on resize, which is what a filling view does.
  /// `palette` is the current theme's colours: the half takes them as an argument instead of holding any of
  /// its own, so a theme flip repaints the stock without the picture knowing a theme exists.
  ///
  /// `vectors` are the document's own edges ([`vector_lines`]) and `layers` is the overlay row's answer to
  /// which of the four things the half draws (S8). Each layer is painted only when it is showing, and the
  /// report counts what reached the painter per layer, so "hidden" and "had nothing to draw" stay different
  /// answers rather than both reading zero in the same field.
  /// Take this frame's orbit and zoom gestures, and learn the region's size.
  ///
  /// Allocation and painting are separated on purpose: the chrome of the half has to be *registered* after this
  /// response (so a press on a chip is not a press on the picture) and *painted* after the picture (so it sits on
  /// top of it). `rect` is the picture's region, which is the camera's viewport for this frame.
  pub fn take_gestures(&mut self, ui: &egui::Ui, response: &egui::Response, rect: egui::Rect) {
    self.camera.viewport = Viewport::new(f64::from(rect.width()), f64::from(rect.height()));
    self.take_input(ui, response);
  }

  /// Paint the picture and report what each of S8's four layers put on the painter.
  ///
  /// `vectors` are the document's own edges ([`vector_lines`]) and `layers` is the overlay row's answer to which
  /// of the four things the half draws. Each layer is painted only when it is showing, and the report counts
  /// what reached the painter per layer, so "hidden" and "had nothing to draw" stay different answers instead of
  /// both reading zero in one field.
  pub fn paint_scene(
    &self,
    painter: &egui::Painter,
    rect: egui::Rect,
    drawn: &mut DrawnState,
    palette: &Palette,
    vectors: &[Line3],
    layers: Layers,
  ) {
    drawn.painted_material_lines = 0;
    drawn.painted_vector_lines = 0;
    drawn.painted_program_lines = 0;
    drawn.painted_tool_lines = 0;
    if layers.material {
      self.paint_surface(painter, rect, drawn, palette);
      self.paint_stock(painter, rect, drawn, palette);
    }
    if layers.vectors {
      self.paint_vectors(painter, rect, drawn, palette, vectors);
    }
    if layers.toolpaths {
      self.paint_program(painter, rect, drawn, palette);
    }
    if layers.tool {
      self.paint_tool(painter, rect, drawn, palette);
    }
    drawn.painted_3d_lines = drawn.painted_material_lines
      + drawn.painted_vector_lines
      + drawn.painted_program_lines
      + drawn.painted_tool_lines;
  }

  /// Turn the camera, and zoom it about the target.
  ///
  /// Both gestures go through [`Camera3d::orbit`] and [`Camera3d::zoom`] rather than assigning the
  /// fields: the pitch clamp and the distance clamp are the camera's rules, and a gesture is not
  /// allowed a second opinion about them.
  fn take_input(&mut self, ui: &egui::Ui, response: &egui::Response) {
    let delta = response.drag_delta();
    if delta.x != 0.0 || delta.y != 0.0 {
      // Dragging right turns the stock to the right and dragging down lowers the eye: the picture
      // follows the hand, the way a pan does on the 2D canvas.
      self.camera.orbit(
        -f64::from(delta.x) * ORBIT_RADIANS_PER_PIXEL,
        -f64::from(delta.y) * ORBIT_RADIANS_PER_PIXEL,
      );
    }
    if response.hovered() {
      let scroll = ui.ctx().input(|input| input.smooth_scroll_delta.y);
      if scroll.abs() > 0.01 {
        let factor = (1.0 + f64::from(scroll) * 0.002).clamp(0.2, 5.0);
        self.camera.zoom(factor);
      }
    }
  }

  /// Paint the document's vectors on the stock's top face (S8's `Vectors` layer).
  ///
  /// The role is the one the 2D canvas paints ordinary geometry in ([`Role::Text`]), because these are the
  /// same vectors: the document's own drawing, not the program and not a signal about either.
  fn paint_vectors(
    &self,
    painter: &egui::Painter,
    rect: egui::Rect,
    drawn: &mut DrawnState,
    palette: &Palette,
    vectors: &[Line3],
  ) {
    let stroke = Stroke::new(1.2, palette.get(Role::Text));
    for line in vectors {
      let (Some(start), Some(end)) = (self.screen(rect, line.start), self.screen(rect, line.end))
      else {
        continue;
      };
      painter.line_segment([start, end], stroke);
      drawn.painted_vector_lines += 1;
    }
  }

  /// Paint the tool at the playhead (S8's `Tool` layer): its axis and its diameter.
  ///
  /// Two edges, and no more, because the picture may not invent a fact the program does not carry: a spindle
  /// drawn to a guessed length would be exactly that. What the program does hold is where the tip is and how
  /// wide the tool is, and those are the two edges here — a marker of the tool, not a model of the machine.
  fn paint_tool(
    &self,
    painter: &egui::Painter,
    rect: egui::Rect,
    drawn: &mut DrawnState,
    palette: &Palette,
  ) {
    let Some(step) = self.step_under_playhead() else {
      return;
    };
    let tip = step.movement.destination();
    let radius = tool_radius(step.tool);
    let stroke = Stroke::new(1.6, palette.get(Role::TextHi));
    let axis = [
      self.screen(rect, tip),
      self.screen(
        rect,
        Point3::new(tip.x.mm(), tip.y.mm(), tip.z.mm() + TOOL_MARKER_MM),
      ),
    ];
    if let [Some(start), Some(end)] = axis {
      painter.line_segment([start, end], stroke);
      drawn.painted_tool_lines += 1;
    }
    let across = [
      self.screen(
        rect,
        Point3::new(tip.x.mm() - radius, tip.y.mm(), tip.z.mm()),
      ),
      self.screen(
        rect,
        Point3::new(tip.x.mm() + radius, tip.y.mm(), tip.z.mm()),
      ),
    ];
    if let [Some(start), Some(end)] = across {
      painter.line_segment([start, end], stroke);
      drawn.painted_tool_lines += 1;
    }
  }

  /// Paint the axis triad in `corner` and report where each label went (S8).
  ///
  /// The three directions are machine space's: `+Z` is `TOP`, `-Y` is `FRONT` — the face toward the
  /// operator, which is the one the reference labels `FRENTE` — and `+X` is `RIGHT`. They are projected
  /// through the same camera as the picture, so the triad is a *reading* of the view rather than a
  /// decoration beside it: orbit the stock and the triad turns with it, which is the only way it can be
  /// trusted about which way is up.
  fn triad(
    &self,
    painter: &egui::Painter,
    corner: egui::Rect,
    palette: &Palette,
  ) -> Vec<(String, (f32, f32))> {
    let base = match self.camera.project(self.camera.target) {
      Some(base) => base,
      None => return Vec::new(),
    };
    let origin = egui::pos2(
      corner.center().x,
      corner.max.y - TRIAD_LENGTH_PX - TRIAD_LABEL_PAD,
    );
    let mut labels = Vec::new();
    for (label, axis, role) in [
      ("TOP", Point3::new(0.0, 0.0, 1.0), Role::Sel),
      ("FRONT", Point3::new(0.0, -1.0, 0.0), Role::Danger),
      ("RIGHT", Point3::new(1.0, 0.0, 0.0), Role::Ok),
    ] {
      let tip = Point3::new(
        self.camera.target.x.mm() + axis.x.mm(),
        self.camera.target.y.mm() + axis.y.mm(),
        self.camera.target.z.mm() + axis.z.mm(),
      );
      let Some(tip) = self.camera.project(tip) else {
        continue;
      };
      let (across, up) = (tip.x - base.x, tip.y - base.y);
      let length = (across * across + up * up).sqrt();
      if !length.is_finite() || length <= 1.0e-9 {
        // An axis pointing straight at the eye has no direction on the screen, and drawing it in some
        // invented one would say the wrong thing about the view.
        continue;
      }
      let unit = egui::vec2((across / length) as f32, (up / length) as f32);
      let end = egui::pos2(
        origin.x + unit.x * TRIAD_LENGTH_PX,
        origin.y + unit.y * TRIAD_LENGTH_PX,
      );
      painter.line_segment([origin, end], Stroke::new(1.4, palette.get(role)));
      let at = egui::pos2(
        end.x + unit.x * TRIAD_LABEL_PAD,
        end.y + unit.y * TRIAD_LABEL_PAD,
      );
      painter.text(
        at,
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(theme::CAPTION),
        palette.get(Role::TextDim),
      );
      labels.push((label.to_owned(), (at.x, at.y)));
    }
    labels
  }

  /// Convert the surface mesh into one [`egui::Mesh`], shaded by each vertex's depth (A11).
  ///
  /// A triangle with a vertex the camera cannot project is skipped, not clamped: drawing it would
  /// mean inventing a screen position, and the surface would then say something the field does not.
  fn paint_surface(
    &self,
    painter: &egui::Painter,
    rect: egui::Rect,
    drawn: &mut DrawnState,
    palette: &Palette,
  ) {
    let surface = build_surface(&self.field, &self.camera, self.stride);
    if surface.is_empty() {
      return;
    }
    let mut mesh = egui::Mesh::default();
    for triangle in &surface.triangles {
      let mut corners = [Pos2::ZERO; 3];
      let mut depths = [0.0_f32; 3];
      let mut visible = true;
      for (slot, index) in triangle.iter().enumerate() {
        let vertex = surface.vertices[*index];
        match self.screen(rect, vertex.position) {
          Some(position) => {
            corners[slot] = position;
            depths[slot] = vertex.depth;
          }
          None => {
            visible = false;
            break;
          }
        }
      }
      if !visible {
        continue;
      }
      let base = mesh.vertices.len() as u32;
      for corner in 0..3 {
        mesh.vertices.push(egui::epaint::Vertex {
          pos: corners[corner],
          uv: egui::epaint::WHITE_UV,
          // Depth becomes value through the theme's ramp: the same field says the same thing about
          // an 18 mm sheet and a 6 mm one, and both ends are roles like every other colour here.
          color: palette.stock_shade(depths[corner]),
        });
      }
      mesh.indices.extend([base, base + 1, base + 2]);
      drawn.painted_triangles += 1;
    }
    if !mesh.indices.is_empty() {
      painter.add(egui::Shape::mesh(mesh));
    }
  }

  /// Paint the stock's twelve edges.
  fn paint_stock(
    &self,
    painter: &egui::Painter,
    rect: egui::Rect,
    drawn: &mut DrawnState,
    palette: &Palette,
  ) {
    for line in &self.stock {
      let (Some(start), Some(end)) = (self.screen(rect, line.start), self.screen(rect, line.end))
      else {
        continue;
      };
      // The silhouette of the material. Not `Border`: a control's hairline divides two surfaces inside
      // one panel, and at 1.8:1 against the canvas it let a dark block's lower edges dissolve into the
      // background. This role is held to 3:1 against both the canvas and the face it bounds.
      // 1.6, the icon set's own line weight, not a hairline: measured on the running app, a 1.0 stroke is
      // antialiased over about two pixels and reaches 2.32:1 against the stock's face where the token
      // promises 4.90:1. A silhouette that carries an object's shape has to survive being rendered.
      painter.line_segment([start, end], Stroke::new(1.6, palette.get(Role::StockEdge)));
      drawn.painted_material_lines += 1;
    }
  }

  /// Paint the program's moves as projected 3D polylines.
  ///
  /// Arcs are flattened here, at half a pixel, for the same reason the 2D canvas flattens them: the
  /// painter owns the flattening, and every earlier stage keeps the arc as an arc. A movement with an
  /// end the camera cannot project is dropped rather than drawn through a made-up point. The
  /// segment's first move starts a polyline instead of being joined to the previous toolpath: the
  /// stream carries no start position, and inventing one would draw a line the program never made.
  fn paint_program(
    &self,
    painter: &egui::Painter,
    rect: egui::Rect,
    drawn: &mut DrawnState,
    palette: &Palette,
  ) {
    let tolerance = self.flatten_tolerance_mm();
    for segment in &self.program.segments {
      let mut previous: Option<Point3> = None;
      for movement in &segment.moves {
        let to = movement.destination();
        // Amber is the cut signal; a rapid's line exists only so the eye can find the cut between
        // them, so it is the secondary role rather than a signal of its own.
        let stroke = match movement {
          Move::Feed { .. } | Move::Arc { .. } | Move::Helix { .. } => {
            Stroke::new(1.6, palette.get(Role::Cut))
          }
          Move::Rapid { .. } | Move::Retract { .. } => Stroke::new(1.0, palette.get(Role::TextDim)),
        };
        if let Some(from) = previous {
          for pair in path_points(from, *movement, tolerance) {
            let (Some(start), Some(end)) = (self.screen(rect, pair.0), self.screen(rect, pair.1))
            else {
              continue;
            };
            painter.line_segment([start, end], stroke);
            drawn.painted_program_lines += 1;
          }
        }
        previous = Some(to);
      }
    }
  }

  /// Half a pixel, expressed in millimetres at the camera's own scale.
  fn flatten_tolerance_mm(&self) -> f64 {
    let distance = self.camera.distance.mm();
    let height = self.camera.viewport.height;
    if !distance.is_finite() || distance <= 0.0 || height <= 0.0 {
      return ARC_TOLERANCE_PX;
    }
    let pixels_per_mm = height / (2.0 * distance * (FIELD_OF_VIEW / 2.0).tan());
    if pixels_per_mm <= 0.0 || !pixels_per_mm.is_finite() {
      return ARC_TOLERANCE_PX;
    }
    ARC_TOLERANCE_PX / pixels_per_mm
  }

  /// Project a machine-space point into window coordinates, or `None` when it cannot be drawn.
  fn screen(&self, rect: egui::Rect, point: Point3) -> Option<Pos2> {
    self.camera.project(point).map(|screen| {
      Pos2::new(
        (f64::from(rect.min.x) + screen.x) as f32,
        (f64::from(rect.min.y) + screen.y) as f32,
      )
    })
  }
}

/// The document's vectors as 3D edges laid on the stock's top face, for S8's `Vectors` layer.
///
/// The projection is the same [`build_scene`] the 2D canvas paints, read as edges: a vector on a hidden layer
/// is hidden here too, so the two pictures cannot disagree about what the document holds. `z = 0` is the
/// stock's top face, which is where a vector is in machine space; arcs are flattened at
/// [`VECTOR_TOLERANCE_MM`] rather than at the painter's pixel tolerance, because this list is built before
/// there is a camera to express a pixel in.
pub fn vector_lines(document: &cam_model::Document) -> Vec<Line3> {
  let scene = build_scene(document, &Selection::default());
  let mut lines = Vec::new();
  for item in &scene.items {
    let points: Vec<cam_geom::Point2> = match item {
      SceneItem::Polyline { points, .. } => points.clone(),
      SceneItem::Arc {
        center,
        radius,
        start_angle,
        sweep,
        ..
      } => match cam_geom::Arc::new(*center, *radius, *start_angle, *sweep) {
        Ok(arc) => arc.tessellate(VECTOR_TOLERANCE_MM),
        // An arc the kernel will not build is dropped rather than replaced with an invented chord: a vector
        // drawn as a line it is not would be a picture of a document that does not exist.
        Err(_) => continue,
      },
    };
    for pair in points.windows(2) {
      lines.push(Line3::new(
        Point3::new(pair[0].x, pair[0].y, 0.0),
        Point3::new(pair[1].x, pair[1].y, 0.0),
      ));
    }
  }
  lines
}

/// Half the width a tool cuts across at its tip, from the shape the program carries.
///
/// A V-bit is measured at its flat: the cone above it widens with depth, and the marker is drawn at the one
/// depth the tool's own numbers describe — its tip.
fn tool_radius(shape: ToolShape) -> f64 {
  match shape {
    ToolShape::Cylinder { diameter } => diameter.mm() / 2.0,
    ToolShape::VBit { flat_diameter, .. } => flat_diameter.mm() / 2.0,
  }
}

/// Where one frame puts the 3D half's chrome (camui-prancheta S8).
///
/// The picture is the half less the playback bar's strip, and two strips float over it: the overlay row at its
/// top left, and the square at its top right that holds the home button and the axis triad. The playback bar
/// is reserved rather than floated, because a bar lying over the model would swallow the drags that orbit it.
#[derive(Clone, Copy, Debug)]
struct Half {
  /// The picture: the region the scene is painted and clipped into, and the one the orbit and wheel own.
  picture: egui::Rect,
  /// The overlay row's strip, at the picture's top left.
  overlay: egui::Rect,
  /// The square at the picture's top right: the home button above, the axis triad below.
  corner: egui::Rect,
  /// The playback bar's strip, along the half's bottom.
  bar: egui::Rect,
}

/// Where the chrome sits in one 3D half.
fn half_layout(rect: egui::Rect, metrics: &theme::Metrics) -> Half {
  let pad = metrics.space(3);
  let bar = egui::Rect::from_min_max(
    egui::pos2(rect.min.x + pad, rect.max.y - PLAYBACK_BAR_HEIGHT),
    egui::pos2(rect.max.x - pad, rect.max.y),
  );
  let picture = egui::Rect::from_min_max(
    rect.min,
    egui::pos2(rect.max.x, (bar.min.y - pad).max(rect.min.y)),
  );
  // The row spans the half and may use two lines, and the corner sits *below* it: that is the reference's own
  // arrangement, and it is what keeps the six words of S8's row readable in a half that is only half a screen.
  let overlay = egui::Rect::from_min_size(
    egui::pos2(picture.min.x + pad, picture.min.y + pad),
    egui::vec2(
      (picture.width() - 2.0 * pad).max(0.0),
      OVERLAY_LINE_HEIGHT * OVERLAY_ROWS as f32 + metrics.space(1),
    ),
  );
  let corner = egui::Rect::from_min_size(
    egui::pos2(picture.max.x - CORNER_WIDTH - pad, overlay.max.y + pad),
    egui::vec2(CORNER_WIDTH, CORNER_HEIGHT),
  );
  Half {
    picture,
    overlay,
    corner,
    bar,
  }
}

/// The pill a chip is drawn in: the selected surface when it is on, the raised one while it is hovered.
fn paint_pill(
  painter: &egui::Painter,
  rect: egui::Rect,
  palette: &Palette,
  on: bool,
  hovered: bool,
) {
  let radius = egui::CornerRadius::same(9);
  if on {
    painter.rect_filled(rect, radius, palette.get(Role::SelSoft));
    painter.rect_stroke(
      rect,
      radius,
      egui::Stroke::new(1.0, palette.get(Role::Sel)),
      egui::StrokeKind::Inside,
    );
  } else if hovered {
    painter.rect_filled(rect, radius, palette.get(Role::Raised));
  }
}

/// The play/pause mark, painted: a triangle while paused and two bars while playing.
fn paint_transport(painter: &egui::Painter, rect: egui::Rect, playing: bool, palette: &Palette) {
  paint_pill(painter, rect, palette, false, false);
  let ink = palette.get(Role::TextHi);
  let centre = rect.center();
  if playing {
    for half in [-1.0_f32, 1.0] {
      let slot = egui::Rect::from_min_size(
        egui::pos2(centre.x + half * 6.0 - 2.0, centre.y - 4.5),
        egui::vec2(3.5, 9.0),
      );
      painter.rect_filled(slot, egui::CornerRadius::same(1), ink);
    }
  } else {
    // A triangle rather than the glyph `▶`: a mark that renders as a box on a font without it would be worse
    // than no mark, which is the rule the rail's caret and the task editor's tick already keep.
    painter.add(egui::Shape::convex_polygon(
      vec![
        egui::pos2(centre.x - 3.5, centre.y - 6.0),
        egui::pos2(centre.x + 5.5, centre.y),
        egui::pos2(centre.x - 3.5, centre.y + 6.0),
      ],
      ink,
      egui::Stroke::NONE,
    ));
  }
}

/// The house the home button carries.
///
/// Drawn from five segments instead of added to the icon set: the set is a design artifact with its own sheet
/// and its own count, and one button does not earn a seventy-ninth glyph (`Icon` has no house).
fn paint_home(painter: &egui::Painter, rect: egui::Rect, palette: &Palette) {
  paint_pill(painter, rect, palette, false, false);
  let stroke = egui::Stroke::new(1.5, palette.get(Role::TextHi));
  let (left, right) = (rect.min.x + 5.0, rect.max.x - 5.0);
  let (top, bottom) = (rect.min.y + 5.0, rect.max.y - 5.0);
  let eave = top + (bottom - top) * 0.45;
  let apex = egui::pos2((left + right) * 0.5, top);
  painter.line_segment([egui::pos2(left, eave), apex], stroke);
  painter.line_segment([apex, egui::pos2(right, eave)], stroke);
  painter.line_segment(
    [egui::pos2(left + 1.0, eave), egui::pos2(left + 1.0, bottom)],
    stroke,
  );
  painter.line_segment(
    [
      egui::pos2(right - 1.0, eave),
      egui::pos2(right - 1.0, bottom),
    ],
    stroke,
  );
  painter.line_segment(
    [
      egui::pos2(left + 1.0, bottom),
      egui::pos2(right - 1.0, bottom),
    ],
    stroke,
  );
}

/// The playback bar's track and its fill.
///
/// The trough is [`Role::Canvas`] and the fill is [`Role::Cut`] — the same amber the cutting moves are painted
/// in, so the bar measures the cut and nothing else. `tests/view3d.rs` finds both rectangles by role, which is
/// what keeps a bar painted with a literal from passing.
fn paint_scrubber(painter: &egui::Painter, track: egui::Rect, fill: f32, palette: &Palette) {
  let radius = egui::CornerRadius::same(4);
  painter.rect_filled(track, radius, palette.get(Role::Canvas));
  let filled = egui::Rect::from_min_size(
    track.min,
    egui::vec2(track.width() * fill.clamp(0.0, 1.0), track.height()),
  );
  painter.rect_filled(filled, radius, palette.get(Role::Cut));
}

/// What the overlay row has to paint once the picture is under it: each chip's word, where it goes, and
/// whether its box is ticked (camui-prancheta S8).
type RowSlots = Vec<(String, egui::Rect, bool)>;

/// What the playback bar has to paint once the picture is under it.
///
/// The same split as [`RowSlots`], and for the same reason: every control in the bar has to be registered
/// before the picture's gesture region (so a press on it is not a press that orbits) and painted after the
/// picture (so it is not painted over). What is left when both are done is this: where everything goes, and
/// the state the presses above left behind.
#[derive(Clone, Debug)]
struct BarPaint {
  /// The transport button's rectangle.
  transport: egui::Rect,
  /// Whether the program is playing *after* this frame's presses.
  playing: bool,
  /// The speed pill's rectangle and the word it names.
  speed: (egui::Rect, String),
  /// `To the end`, `Step` and `Reset`: their words and rectangles.
  plain: Vec<(String, egui::Rect)>,
  /// The scrubber's track, when the row had any width for it.
  scrubber: Option<egui::Rect>,
  /// How full the track's fill is, from the clock's own two numbers.
  fill: f32,
  /// Every pressable control and the point that presses it, in the order drawn.
  hits: Vec<(String, (f32, f32))>,
}

impl Default for BarPaint {
  fn default() -> Self {
    Self {
      // A bar nobody drew has no rectangle to paint in; `Rect::NOTHING` is egui's own way of saying so.
      transport: egui::Rect::NOTHING,
      playing: false,
      speed: (egui::Rect::NOTHING, String::new()),
      plain: Vec::new(),
      scrubber: None,
      fill: 0.0,
      hits: Vec::new(),
    }
  }
}

/// How wide a chip is before it is drawn, so the row can be laid out and allocated before it is painted.
///
/// A checkbox carries its own box; a plain chip is a word in a pill. Measuring first is what lets the row be
/// allocated in one pass and painted in another, which is the only way a press in the row can hide a layer in
/// the same frame's picture (L13).
fn chip_size(
  painter: &egui::Painter,
  palette: &Palette,
  label: &str,
  checkbox: bool,
) -> egui::Vec2 {
  let galley = painter.layout_no_wrap(
    label.to_owned(),
    egui::FontId::proportional(theme::CAPTION),
    palette.get(Role::Text),
  );
  if checkbox {
    egui::vec2(11.0 + 6.0 + galley.size().x + 10.0, 18.0)
  } else {
    egui::vec2(galley.size().x + 14.0, 18.0)
  }
}

/// One of the overlay row's four checkboxes, painted into the rectangle it was allocated.
///
/// The tick is **painted**, not typeset, for the reason the rail's caret and the task editor's `✓` are (L8,
/// L13): a glyph that renders as a tofu box would be worse than no affordance, and the box has to say which of
/// its two states it is in at a glance.
fn paint_layer_chip(
  painter: &egui::Painter,
  rect: egui::Rect,
  palette: &Palette,
  label: &str,
  on: bool,
) {
  let font = egui::FontId::proportional(theme::CAPTION);
  paint_pill(painter, rect, palette, on, false);
  let box_size = 11.0;
  let mark = egui::Rect::from_center_size(
    egui::pos2(rect.min.x + 5.0 + box_size * 0.5, rect.center().y),
    egui::vec2(box_size, box_size),
  );
  let ink = if on {
    palette.get(Role::Sel)
  } else {
    palette.get(Role::TextDim)
  };
  painter.rect_stroke(
    mark,
    egui::CornerRadius::same(2),
    egui::Stroke::new(1.0, ink),
    egui::StrokeKind::Inside,
  );
  if on {
    let tick = egui::Stroke::new(1.6, ink);
    let elbow = egui::pos2(mark.center().x - 0.5, mark.max.y - 3.0);
    painter.line_segment([egui::pos2(mark.min.x + 2.5, mark.center().y), elbow], tick);
    painter.line_segment(
      [elbow, egui::pos2(mark.max.x - 2.5, mark.min.y + 2.5)],
      tick,
    );
  }
  painter.text(
    egui::pos2(mark.max.x + 6.0, rect.center().y),
    egui::Align2::LEFT_CENTER,
    label,
    font,
    if on {
      palette.get(Role::TextHi)
    } else {
      palette.get(Role::TextDim)
    },
  );
}

/// A plain chip of the playback bar: a raised pill with its word in it.
fn paint_plain_chip(painter: &egui::Painter, rect: egui::Rect, palette: &Palette, label: &str) {
  painter.rect_filled(rect, egui::CornerRadius::same(9), palette.get(Role::Raised));
  painter.text(
    rect.center(),
    egui::Align2::CENTER_CENTER,
    label,
    egui::FontId::proportional(theme::CAPTION),
    palette.get(Role::TextDim),
  );
}

impl Shell {
  /// The 3D half of the split (camui-prancheta S8): the picture, its overlay row, the home button with the
  /// axis triad, and the playback bar.
  ///
  /// # The order of the four steps, and why it is that order
  ///
  /// 1. the picture's gesture region is allocated **first**, so every control below is registered after it and
  ///    a press on a chip is a press on chrome rather than one that orbits the stock;
  /// 2. the chrome is allocated and each press applied where it happens, so the frame that hides the tool is
  ///    the frame whose picture has no tool in it (L13);
  /// 3. the picture is painted, from the state those presses left behind;
  /// 4. the chrome is painted over it.
  ///
  /// # What it refuses to draw
  ///
  /// With no program loaded the half says so and draws **no** overlay row and **no** playback bar. Every
  /// control in those two strips governs something in the picture or in the program, and a checkbox over an
  /// empty scene is the same lie as a count nobody measured (L6): there is nothing to hide and nothing to
  /// play, so there is nothing to draw.
  pub(crate) fn half_3d(
    &mut self,
    app: &cam_app::AppState,
    ui: &mut egui::Ui,
    rect: egui::Rect,
    output: &mut crate::ShellOutput,
    dt: f64,
  ) {
    if rect.width() <= 1.0 || rect.height() <= 1.0 {
      return;
    }
    let palette = self.palette();
    let metrics = self.metrics();
    let layout = half_layout(rect, &metrics);
    if self.preview.is_none() {
      ui.painter().text(
        egui::pos2(
          layout.picture.min.x + metrics.space(4),
          layout.picture.min.y + metrics.space(4),
        ),
        egui::Align2::LEFT_TOP,
        "No program to preview. Simulate one with `cam-cli simulate` and hand it in.",
        egui::FontId::proportional(theme::CAPTION),
        palette.get(Role::TextDim),
      );
      return;
    }

    let response = ui.allocate_rect(layout.picture, egui::Sense::click_and_drag());
    if let Some(preview) = self.preview.as_mut() {
      preview.take_gestures(ui, &response, layout.picture);
      // Playback advances only while the split is open: a hidden half is not a clock, and the 2D path pays
      // nothing for a preview nobody is looking at (A12).
      preview.advance(dt);
      if preview.playing() {
        // Without this the window would repaint only on input, and a program that plays to its end would
        // appear to play only when the mouse moves.
        ui.ctx().request_repaint();
      }
    }

    let row = self.overlay_row(ui, &layout, &palette, output);
    let home = self.home_button(ui, &layout, output);
    let bar = self.playback_controls(ui, &layout, &palette, output);

    // The vectors are the document's, and the layers are the row's answer taken *after* its presses: neither is
    // part of the preview, so both are handed in, and a hidden layer costs no projection at all.
    let layers = self.scene_layers;
    let vectors = if layers.vectors {
      vector_lines(app.document())
    } else {
      Vec::new()
    };
    let painter = ui.painter().with_clip_rect(layout.picture);
    if let Some(preview) = self.preview.as_ref() {
      preview.paint_scene(
        &painter,
        layout.picture,
        &mut output.drawn,
        &palette,
        &vectors,
        layers,
      );
      output.drawn.triad = preview.triad(&painter, layout.corner, &palette);
    }
    for (word, rect, on) in &row {
      if Layer::ALL.iter().any(|layer| layer.label() == word) {
        paint_layer_chip(&painter, *rect, &palette, word, *on);
      } else {
        paint_plain_chip(&painter, *rect, &palette, word);
      }
    }
    paint_home(&painter, home, &palette);
    // The bar sits *below* the picture, so it needs a painter that is allowed to be there: a painter clipped to
    // `layout.picture` erased every rectangle of the bar while its hit areas stayed live — an invisible control
    // that still takes clicks (R2-001, R3-001 in the T6 review). Its own strip is the honest clip.
    let bar_painter = ui.painter().with_clip_rect(layout.bar);
    self.paint_playback(&bar_painter, &layout, &palette, output, &bar);
  }

  /// The overlay row: the four layer checkboxes, `Centre` and `?` (camui-prancheta S8).
  ///
  /// Each press is applied where it happens, so the frame that hides the tool is also the frame that reports it
  /// hidden and the frame whose picture has no tool in it. `Centre` re-frames the stock at the angle the eye is
  /// at, and `?` opens the `F1` panel — the shell's one place that teaches every command — rather than a help
  /// window invented for this row. Returns what is left to paint.
  fn overlay_row(
    &mut self,
    ui: &mut egui::Ui,
    layout: &Half,
    palette: &Palette,
    output: &mut crate::ShellOutput,
  ) -> RowSlots {
    let metrics = self.metrics();
    let painter = ui.painter().clone();
    let row = layout.overlay;
    let mut items: Vec<(String, bool)> = Layer::ALL
      .iter()
      .map(|layer| (layer.label().to_owned(), true))
      .collect();
    items.push(("Centre".to_owned(), false));
    items.push(("?".to_owned(), false));
    let mut slots: RowSlots = Vec::new();
    let mut left = row.min.x;
    let mut line = 0;
    for (word, checkbox) in items {
      let size = chip_size(&painter, palette, &word, checkbox);
      if left + size.x > row.max.x {
        // The row wraps, like the reference's own: `?` is on a second line there, and a chip drawn past the
        // half's edge would be over the panel next door while one clipped in half is not pressable at all.
        line += 1;
        left = row.min.x;
        if line >= OVERLAY_ROWS {
          break;
        }
      }
      let rect = egui::Rect::from_min_size(
        egui::pos2(
          left,
          row.min.y + line as f32 * (OVERLAY_LINE_HEIGHT + metrics.space(1)),
        ),
        size,
      );
      left += size.x + metrics.space(1);
      slots.push((word, rect, false));
    }
    for (word, rect, on) in &mut slots {
      let response = ui.allocate_rect(*rect, egui::Sense::click());
      if !response.clicked() {
        if let Some(layer) = Layer::ALL.iter().find(|layer| layer.label() == word) {
          *on = self.scene_layers.get(*layer);
        }
        continue;
      }
      match word.as_str() {
        "Centre" => {
          if let Some(preview) = self.preview.as_mut() {
            preview.recentre();
          }
        }
        "?" => self.show_help = true,
        _ => {
          if let Some(layer) = Layer::ALL.iter().find(|layer| layer.label() == word) {
            let showing = self.scene_layers.get(*layer);
            self.scene_layers.set(*layer, !showing);
            *on = !showing;
          }
        }
      }
    }
    output.drawn.layer_chips = slots
      .iter()
      .map(|(word, rect, _)| (word.clone(), (rect.center().x, rect.center().y)))
      .collect();
    output.drawn.scene_layers = self.scene_layers.words();
    slots
  }

  /// The home button: allocated and applied here, painted by the caller after the picture.
  fn home_button(
    &mut self,
    ui: &mut egui::Ui,
    layout: &Half,
    output: &mut crate::ShellOutput,
  ) -> egui::Rect {
    let size = 22.0;
    let rect = egui::Rect::from_min_size(
      egui::pos2(layout.corner.center().x - size * 0.5, layout.corner.min.y),
      egui::vec2(size, size),
    );
    let response = ui.allocate_rect(rect, egui::Sense::click());
    if response.clicked() {
      if let Some(preview) = self.preview.as_mut() {
        preview.home();
      }
    }
    let centre = rect.center();
    output.drawn.home_button = Some((centre.x, centre.y));
    rect
  }

  /// The playback bar's controls: the transport, the speed, `To the end`, `Step`, `Reset` and the scrubber
  /// (camui-prancheta S8).
  ///
  /// `Step` and `Reset` are here as well. They are not in S8's list and they stay because they are behaviour this
  /// shell already had and already tests (M5 A10, `the_panel_offers_play_step_and_reset`): deleting a working
  /// control to match a list is a regression, and both move the same playhead the rest of the bar moves.
  fn playback_controls(
    &mut self,
    ui: &mut egui::Ui,
    layout: &Half,
    palette: &Palette,
    output: &mut crate::ShellOutput,
  ) -> BarPaint {
    let metrics = self.metrics();
    let painter = ui.painter().clone();
    let bar = layout.bar;
    let row = egui::Rect::from_min_size(
      egui::pos2(bar.min.x + 8.0, bar.min.y + 4.0),
      egui::vec2((bar.width() - 16.0).max(0.0), 20.0),
    );
    let Some(preview) = self.preview.as_mut() else {
      return BarPaint::default();
    };
    let mut paint = BarPaint::default();
    let mut left = row.min.x;
    let middle = row.center().y;

    // The transport: one button whose mark and word say what pressing it does now.
    paint.transport =
      egui::Rect::from_min_size(egui::pos2(left, middle - 9.0), egui::vec2(26.0, 18.0));
    left += 26.0 + metrics.space(1);
    let response = ui.allocate_rect(paint.transport, egui::Sense::click());
    if response.clicked() {
      preview.toggle_playing();
    }
    paint.playing = preview.playing();
    let centre = paint.transport.center();
    paint.hits.push((
      if paint.playing { "Pause" } else { "Play" }.to_owned(),
      (centre.x, centre.y),
    ));

    // The speed: a pill naming the stop it is set to, stepping to the next one on a press. Its width is the
    // widest word the ladder can produce, so pressing it never shifts the scrubber beside it.
    let size = chip_size(
      &painter,
      palette,
      &speed_text(SPEEDS[SPEEDS.len() - 1]),
      false,
    );
    let speed = egui::Rect::from_min_size(egui::pos2(left, middle - 9.0), size);
    left += size.x + metrics.space(1);
    let response = ui.allocate_rect(speed, egui::Sense::click());
    if response.clicked() {
      preview.set_speed(next_speed(preview.speed()));
    }
    paint.speed = (speed, speed_text(preview.speed()));
    let centre = speed.center();
    paint
      .hits
      .push((speed_text(preview.speed()), (centre.x, centre.y)));

    // `To the end` shares the control line with the transport and the speed; `Step` and `Reset` live on the
    // readout's line, at its right end, where a second pair of pacing controls does not crowd the scrubber.
    for word in ["To the end"] {
      let size = chip_size(&painter, palette, word, false);
      let rect = egui::Rect::from_min_size(egui::pos2(left, middle - 9.0), size);
      left += size.x + metrics.space(1);
      let response = ui.allocate_rect(rect, egui::Sense::click());
      if response.clicked() {
        preview.to_the_end();
      }
      let centre = rect.center();
      paint.hits.push((word.to_owned(), (centre.x, centre.y)));
      paint.plain.push((word.to_owned(), rect));
    }
    let mut right = row.max.x;
    for word in ["Reset", "Step"] {
      let size = chip_size(&painter, palette, word, false);
      right -= size.x;
      let rect = egui::Rect::from_min_size(egui::pos2(right, bar.max.y - 6.0 - size.y), size);
      right -= metrics.space(1);
      let response = ui.allocate_rect(rect, egui::Sense::click());
      if response.clicked() {
        match word {
          "Step" => preview.step(),
          _ => preview.reset(),
        }
      }
      let centre = rect.center();
      paint.hits.push((word.to_owned(), (centre.x, centre.y)));
      paint.plain.push((word.to_owned(), rect));
    }

    // The scrubber takes the row's remaining width: how far the program has played is the one thing in the bar
    // that is a measurement rather than a control, so it is the part that stretches.
    let track = egui::Rect::from_min_size(
      egui::pos2(left + metrics.space(1), middle - 4.0),
      egui::vec2((row.max.x - left - metrics.space(2)).max(0.0), 8.0),
    );
    if track.width() > 0.0 {
      let response = ui.allocate_rect(track, egui::Sense::click_and_drag());
      if response.clicked() || response.dragged() {
        if let Some(at) = response.interact_pointer_pos() {
          let fraction = f64::from(((at.x - track.min.x) / track.width()).clamp(0.0, 1.0));
          preview.seek(fraction);
        }
      }
      paint.fill = bar_fill(preview.progress(), preview.duration());
      paint.scrubber = Some(track);
    }
    output.drawn.playback_hits = paint.hits.clone();
    paint
  }

  /// Paint the playback bar over the picture, and report what it reads.
  ///
  /// The readout is written **here**, after every press above has reached the preview, so the clock in the bar
  /// and the clock in the report are the same moment (`tests/view3d.rs` reads both).
  fn paint_playback(
    &mut self,
    painter: &egui::Painter,
    layout: &Half,
    palette: &Palette,
    output: &mut crate::ShellOutput,
    bar: &BarPaint,
  ) {
    let strip = layout.bar;
    let radius = egui::CornerRadius::same(8);
    painter.rect_filled(strip, radius, palette.get(Role::Overlay));
    painter.rect_stroke(
      strip,
      radius,
      egui::Stroke::new(1.0, palette.get(Role::Border)),
      egui::StrokeKind::Inside,
    );
    paint_transport(painter, bar.transport, bar.playing, palette);
    paint_plain_chip(painter, bar.speed.0, palette, &bar.speed.1);
    for (word, rect) in &bar.plain {
      paint_plain_chip(painter, *rect, palette, word);
    }
    if let Some(track) = bar.scrubber {
      paint_scrubber(painter, track, bar.fill, palette);
    }
    let Some(preview) = self.preview.as_ref() else {
      return;
    };
    output.drawn.preview_progress = preview.progress();
    output.drawn.preview_bar = bar_fill(preview.progress(), preview.duration());
    output.drawn.preview_clock = progress_text(preview.elapsed(), preview.duration());
    output.drawn.playback_readout = format!(
      "{}  {}",
      output.drawn.preview_clock,
      preview.playhead_text()
    );
    painter.text(
      egui::pos2(strip.min.x + 10.0, strip.max.y - 6.0),
      egui::Align2::LEFT_BOTTOM,
      &output.drawn.playback_readout,
      egui::FontId::monospace(theme::CAPTION),
      palette.get(Role::TextHi),
    );
    output.drawn.scrubber = bar
      .scrubber
      .map(|track| (track.min.x, track.max.x, track.center().y));
  }
}

/// The decimation factor that keeps the mesh's lattice within [`MAX_LATTICE`] lines per side.
///
/// A grid smaller than that is sampled cell for cell: decimating a mesh already inside the budget
/// would only lose detail.
fn stride_for(field: &Heightfield) -> usize {
  let grid = field.grid();
  grid.columns.max(grid.rows).div_ceil(MAX_LATTICE).max(1)
}

/// Flatten a program into the timed steps playback consumes, and their running total.
///
/// The sweep's origin is [`sweep::apply`]'s own rule — a segment's first move starts where it ends —
/// so playing the program reaches the field the report describes. The clock's origin is the stream's
/// own start, `home` at `safe_z`, and then each move's own destination, so an approach rapid is
/// timed even though it cuts nothing.
fn steps_for(program: &MoveStream) -> (Vec<Step>, Vec<f64>) {
  let mut steps = Vec::with_capacity(program.move_count());
  let mut cumulative = Vec::with_capacity(program.move_count() + 1);
  cumulative.push(0.0);
  let mut departure = Point3::new(program.home.x, program.home.y, program.safe_z.mm());
  for (index, segment) in program.segments.iter().enumerate() {
    let tool = segment.tool;
    let mut previous: Option<Point3> = None;
    for movement in &segment.moves {
      let to = movement.destination();
      let seconds = move_seconds(departure, *movement, segment.feed_rate, segment.plunge_rate);
      let total = cumulative.last().copied().unwrap_or(0.0) + seconds;
      steps.push(Step {
        movement: *movement,
        sweep_from: previous.unwrap_or(to),
        tool,
        seconds,
        segment: index,
      });
      cumulative.push(total);
      previous = Some(to);
      departure = to;
    }
  }
  (steps, cumulative)
}

/// The last entry of a running total: how long the whole program lasts, and `0.0` for a total with no
/// moves in it.
///
/// One projection, two readers — [`seconds_for`] over a program it has just timed, and
/// [`Preview3d::duration`] over the total it already keeps — so the estimate beside a toolpath and the
/// playback clock cannot drift apart.
fn total_seconds(cumulative: &[f64]) -> f64 {
  cumulative.last().copied().unwrap_or(0.0)
}

/// How many seconds of its own time `program` lasts.
///
/// The published form of the law [`steps_for`] already implements: each move is timed by
/// [`move_seconds`] — the segment's plunge rate for a move that travels in Z alone, its feed rate
/// otherwise, an arc along its own circle, a helix along the hypotenuse — and the program's length is
/// their sum (camui-toolpath S5). Nothing here holds a second opinion, which is what makes the
/// estimate beside the current-tool card the same number the playback clock reports for the same
/// program rather than a number that merely looks like it.
///
/// A program with no timed moves lasts `0.0`, which [`clock_text`] writes as `0:00` rather than as a
/// lie.
pub fn seconds_for(program: &MoveStream) -> f64 {
  total_seconds(&steps_for(program).1)
}

/// How many seconds of the program's own time one move takes.
///
/// The rates are the segment's: `plunge_rate` for a move that travels in Z alone, `feed_rate`
/// otherwise. A rapid and a retract carry no rate of their own, so they are timed at the segment's
/// feed rate — the preview's clock is a story about progress, and a machine's rapid rate is a number
/// the program does not contain. An arc is measured along its own circle, for the same reason the
/// sweep follows it rather than its chord, and a helix adds the Z it descends to that arc length — the
/// tool travels the hypotenuse of a turn, not the turn alone. A length or a rate that is not a positive
/// number gives a zero-duration move, which playback crosses without spending any of its budget.
fn move_seconds(departure: Point3, movement: Move, feed_rate: f64, plunge_rate: f64) -> f64 {
  let to = movement.destination();
  let (length, rate) = match movement {
    Move::Arc { center, ccw, .. } => {
      let radius = center.distance_to(to.xy());
      let sweep = arc_sweep(departure.xy(), to.xy(), center, ccw);
      (radius * sweep.abs(), feed_rate)
    }
    Move::Helix { center, ccw, .. } => {
      let radius = center.distance_to(to.xy());
      let sweep = arc_sweep(departure.xy(), to.xy(), center, ccw);
      let (arc, rise) = (radius * sweep.abs(), to.z.mm() - departure.z.mm());
      ((arc * arc + rise * rise).sqrt(), feed_rate)
    }
    Move::Feed { .. } => {
      let flat = departure.xy().distance_to(to.xy());
      let rises = to.z.mm() != departure.z.mm();
      (
        distance(departure, to),
        if flat <= 0.0 && rises {
          plunge_rate
        } else {
          feed_rate
        },
      )
    }
    Move::Rapid { .. } | Move::Retract { .. } => (distance(departure, to), feed_rate),
  };
  if !length.is_finite() || length <= 0.0 || !rate.is_finite() || rate <= 0.0 {
    return 0.0;
  }
  length / rate
}

/// The straight-line distance between two machine positions, in millimetres.
fn distance(from: Point3, to: Point3) -> f64 {
  let (dx, dy, dz) = (
    to.x.mm() - from.x.mm(),
    to.y.mm() - from.y.mm(),
    to.z.mm() - from.z.mm(),
  );
  (dx * dx + dy * dy + dz * dz).sqrt()
}

/// A camera framed on the stock `job` describes, at the default three-quarter viewpoint.
///
/// The target is the stock's centre, half way down its thickness, so orbiting turns the material
/// rather than swinging it around the table. A degenerate job — no width, or a value that is not a
/// number — lands on [`MIN_DISTANCE`], where the camera's own projection rule reports that nothing
/// can be drawn instead of dividing by zero.
fn frame_camera(job: &Job) -> Camera3d {
  let (width, height, thickness) = (job.width.mm(), job.height.mm(), job.thickness.mm());
  let target = Point3::new(width / 2.0, height / 2.0, -thickness / 2.0);
  let mut camera = Camera3d::new(target, Viewport::new(0.0, 0.0));
  let diagonal = (width * width + height * height + thickness * thickness).sqrt();
  let distance = if diagonal.is_finite() {
    (diagonal * FRAME_MARGIN).max(MIN_DISTANCE.mm())
  } else {
    MIN_DISTANCE.mm()
  };
  camera.distance = Length::from_mm(distance);
  camera
}

/// The straight edges a movement contributes, in machine space.
///
/// A feed, rapid or retract is one edge. An arc or a helix is flattened into the chords that stay within
/// `tolerance` of its circle, with its Z interpolated along the way — the arc model is planar in XY, so
/// the height comes from the movement, not from the circle, which is what draws a helix as the descending
/// spiral it is. A degenerate arc falls back to its chord rather than disappearing: showing where the
/// tool went is more useful than showing nothing.
fn path_points(from: Point3, movement: Move, tolerance: f64) -> Vec<(Point3, Point3)> {
  let to = movement.destination();
  let (center, ccw) = match movement {
    Move::Arc { center, ccw, .. } | Move::Helix { center, ccw, .. } => (center, ccw),
    _ => return vec![(from, to)],
  };
  let radius = center.distance_to(to.xy());
  if !radius.is_finite() || radius <= 0.0 {
    return vec![(from, to)];
  }
  let start_angle = (from.y.mm() - center.y).atan2(from.x.mm() - center.x);
  let sweep = arc_sweep(from.xy(), to.xy(), center, ccw);
  let Ok(arc) = cam_geom::Arc::new(center, radius, start_angle, sweep) else {
    return vec![(from, to)];
  };
  let tolerance = if tolerance.is_finite() && tolerance > 0.0 {
    tolerance
  } else {
    ARC_TOLERANCE_PX
  };
  let points = arc.tessellate(tolerance);
  let (start_z, end_z) = (from.z.mm(), to.z.mm());
  let last = points.len().saturating_sub(1).max(1) as f64;
  let mut edges = Vec::with_capacity(points.len());
  let mut previous: Option<Point3> = None;
  for (index, point) in points.into_iter().enumerate() {
    let t = index as f64 / last;
    let position = Point3::new(point.x, point.y, start_z + (end_z - start_z) * t);
    if let Some(from) = previous {
      edges.push((from, position));
    }
    previous = Some(position);
  }
  edges
}

/// The signed sweep of an arc from `from` to `to` about `center`, in radians.
///
/// The direction comes from the movement's own `ccw` flag, never from the smaller of the two
/// rotations: an arc that goes the long way round must be drawn the long way round, or the picture
/// shows a cut the machine did not make.
fn arc_sweep(
  from: cam_geom::Point2,
  to: cam_geom::Point2,
  center: cam_geom::Point2,
  ccw: bool,
) -> f64 {
  let start = (from.y - center.y).atan2(from.x - center.x);
  let end = (to.y - center.y).atan2(to.x - center.x);
  let mut sweep = end - start;
  if ccw {
    while sweep <= 0.0 {
      sweep += std::f64::consts::TAU;
    }
  } else {
    while sweep >= 0.0 {
      sweep -= std::f64::consts::TAU;
    }
  }
  sweep
}

/// A moment of the program's own time, in the form the panel writes it: `m:ss`, seconds truncated.
///
/// Minutes accumulate instead of opening an hour field — a 40 m pocket reads as `40:00`, not `0:40:00` —
/// because the readout sits inside one line of a control row. Truncating rather than rounding is the
/// only honest direction on both sides of the sentence: `3:07` for a 187.9 s program and `0:42` for a
/// 42.9 s elapsed never have the clocks claim a second the machine has not spent. A value that is not
/// a finite, non-negative number is no time at all, so it reads as `0:00` rather than as a lie.
pub fn clock_text(seconds: f64) -> String {
  let whole = if seconds.is_finite() && seconds > 0.0 {
    seconds.floor() as u64
  } else {
    0
  };
  format!("{}:{:02}", whole / 60, whole % 60)
}

/// The bar's readout: how much of the program has played, and of how much, as one sentence.
///
/// `0:42 / 3:07` rather than `42`, because a number without a unit is not something a person can read (S7):
/// the pair of clocks says what is being measured, and the slash is how the reference frame's playback bar
/// writes the pair — `0:05 / 0:05  T22 · Drilling 1 · Z 5.000` (camui-prancheta S8). Both sides are
/// [`clock_text`], so there is one clock in the shell and no second opinion about what a moment looks like.
pub fn progress_text(elapsed: f64, total: f64) -> String {
  format!("{} / {}", clock_text(elapsed), clock_text(total))
}

/// How full the playback bar is painted, in `0.0..=1.0`, from the clock's own two numbers.
///
/// The bar's length is *time*, so a program whose clock reports no total — an empty program, or one
/// whose moves all take zero seconds — paints an empty bar instead of a full one. That case is not
/// hypothetical: [`Preview3d::progress`] answers `1.0` for such a program once it is played out,
/// which is correct about the program and would be a lie about the time, so the bar does not take
/// its fill from `progress` alone. A value outside the range, or one that is not a number, is
/// clamped rather than trusted: the bar can neither overfill nor run backwards.
pub fn bar_fill(progress: f64, total: f64) -> f32 {
  if !total.is_finite() || total <= 0.0 || !progress.is_finite() {
    return 0.0;
  }
  progress.clamp(0.0, 1.0) as f32
}

#[cfg(test)]
mod tests {
  use super::{bar_fill, clock_text, next_speed, progress_text, speed_text, SPEEDS};

  /// The form a moment of the program's own time is written in: `m:ss`, seconds truncated.
  #[test]
  fn a_known_moment_reads_as_a_known_clock() {
    for (seconds, expected) in [
      (0.0, "0:00"),
      // Seconds are two digits, so a seven second total is not `0:7` beside a `3:07`.
      (7.0, "0:07"),
      // Truncated, not rounded: a clock that says `3:08` for 3:07.9 is a second nobody spent.
      (187.9, "3:07"),
      (42.4, "0:42"),
      (60.0, "1:00"),
      // Minutes accumulate instead of opening an hour field: a 40 m pocket is `40:00`, and an operator
      // reads it faster than `0:40:00` on a control this small.
      (3_600.0, "60:00"),
    ] {
      assert_eq!(
        clock_text(seconds),
        expected,
        "{seconds} s must read as {expected}"
      );
    }
  }

  /// A moment that is not a number is not a moment: it reads as nothing spent rather than as a lie.
  #[test]
  fn a_moment_that_is_not_a_number_reads_as_no_time() {
    for seconds in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
      assert_eq!(
        clock_text(seconds),
        "0:00",
        "{seconds} must not format as a time"
      );
    }
  }

  /// The readout names what both numbers are: `0:42 / 3:07`, never a bare pair of counts.
  #[test]
  fn the_readout_states_the_total_beside_the_elapsed() {
    assert_eq!(progress_text(42.0, 187.0), "0:42 / 3:07");
    // A program with no time to spend still reads as a time, on both sides (S7: no number without a unit).
    assert_eq!(progress_text(0.0, 0.0), "0:00 / 0:00");
    // The slash is the reference's own: `0:05 / 0:05  T22 · Drilling 1 · Z 5.000` (camui-prancheta S8).
    assert_eq!(progress_text(5.0, 5.0), "0:05 / 0:05");
  }

  /// The speed chip's word, and the ladder it steps through.
  #[test]
  fn the_speed_is_named_and_stepped_by_one_ladder() {
    assert_eq!(speed_text(60.0), "60x");
    assert_eq!(speed_text(1.0), "1x");
    assert_eq!(speed_text(0.5), "0.5x");
    // A speed that is not a positive number is not a speed, and the chip says so rather than naming one.
    assert_eq!(speed_text(0.0), "—");
    assert_eq!(speed_text(f64::NAN), "—");
    // The ladder wraps at the top and steps to the first stop above a speed off it.
    assert_eq!(next_speed(60.0), 200.0);
    assert_eq!(next_speed(200.0), SPEEDS[0]);
    assert_eq!(next_speed(30.0), 60.0);
  }

  /// The bar's fill, and the one case it refuses to fill: a clock with no total.
  #[test]
  fn a_clock_with_no_total_paints_an_empty_bar() {
    assert_eq!(bar_fill(0.5, 10.0), 0.5);
    assert_eq!(bar_fill(0.0, 10.0), 0.0);
    assert_eq!(bar_fill(1.0, 10.0), 1.0);
    // Out of range values are clamped rather than trusted: the bar cannot overfill or run backwards.
    assert_eq!(bar_fill(2.0, 10.0), 1.0);
    assert_eq!(bar_fill(-2.0, 10.0), 0.0);
    assert_eq!(bar_fill(f64::NAN, 10.0), 0.0);
    // Nothing to divide by, so nothing is claimed: a finished program with a zero-duration clock does not
    // paint a full bar even though `progress()` reports `1.0` for it.
    assert_eq!(bar_fill(1.0, 0.0), 0.0);
    assert_eq!(bar_fill(1.0, -1.0), 0.0);
    assert_eq!(bar_fill(1.0, f64::NAN), 0.0);
  }
}
