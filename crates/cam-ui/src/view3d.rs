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
//! # Why the geometry is not computed here
//!
//! The projection ([`Camera3d`]) and the surface ([`build_surface`]) are data, tested headless in
//! `cam-view` (A9, S2). This module is the painter: it turns that data into [`egui::Mesh`] triangles
//! and lines on the existing [`egui::Painter`], which is why the 3D view needs no new dependency
//! (A11, D-M5-7). A `wgpu` backend would consume the same [`cam_view::SurfaceMesh`] when S12's
//! trigger fires; nothing here would have to change to allow it.

use cam_app::Viewport;
use cam_model::Job;
use cam_move::{Move, MoveStream, Point3, ToolShape};
use cam_sim::{sweep, Heightfield};
use cam_units::Length;
use cam_view::{build_stock_box, build_surface, Camera3d, Line3, FIELD_OF_VIEW, MIN_DISTANCE};
use egui::{Color32, Pos2, Stroke};

use crate::DrawnState;

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

/// Colours, in one place so the picture stays consistent with the 2D canvas.
mod palette {
  use egui::Color32;

  /// The stock's edges.
  pub const STOCK: Color32 = Color32::from_rgb(88, 132, 178);
  /// Cutting moves of the previewed program.
  pub const CUT: Color32 = Color32::from_rgb(255, 176, 59);
  /// Rapids and retracts: present, but not the thing being shown.
  pub const TRAVEL: Color32 = Color32::from_rgb(96, 104, 116);
  /// The untouched top of the stock, at the `0.0` end of the depth ramp.
  pub const SURFACE_TOP: Color32 = Color32::from_rgb(214, 220, 230);
  /// The stock's bottom, at the `1.0` end of the depth ramp.
  pub const SURFACE_BOTTOM: Color32 = Color32::from_rgb(38, 74, 122);
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
    self.cumulative.last().copied().unwrap_or(0.0)
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

  /// The play/pause, step and reset controls, with the program's progress readout.
  ///
  /// These are view state: they move the cursor over the program the caller handed in and push no
  /// [`UiAction`](crate::UiAction), which is what keeps the preview off the document's authoring
  /// path (D-M5-9). The speed slider is limited to the range the panel offers rather than the one
  /// [`Preview3d::set_speed`] accepts, so a drag cannot make the animation meaningless.
  pub fn controls(&mut self, ui: &mut egui::Ui) {
    let label = if self.playing { "Pause" } else { "Play" };
    if ui
      .add_enabled(self.playing || !self.finished(), egui::Button::new(label))
      .clicked()
    {
      self.toggle_playing();
    }
    if ui
      .add_enabled(!self.playing && !self.finished(), egui::Button::new("Step"))
      .clicked()
    {
      self.step();
    }
    if ui
      .add_enabled(
        self.cursor > 0 || self.fraction > 0.0,
        egui::Button::new("Reset"),
      )
      .clicked()
    {
      self.reset();
    }
    ui.add(
      egui::DragValue::new(&mut self.speed)
        .speed(1.0)
        .range(0.1..=600.0)
        .suffix("x"),
    );
    ui.label(format!(
      "{:>5.1}% · move {} / {}",
      self.progress() * 100.0,
      self.cursor,
      self.steps.len()
    ));
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

  /// Paint the preview into the region `ui` has left, and take the orbit and zoom gestures it owns.
  ///
  /// The region is whatever the frame gives it, because the 3D view now *is* the central area rather
  /// than a strip stacked over the drawing. The camera therefore learns a new viewport on resize, which
  /// is what a filling view does; the old fixed height existed to stop the framing jumping while the
  /// strip resized itself, and that trade disappeared with the stacking.
  pub fn show(&mut self, ui: &mut egui::Ui, drawn: &mut DrawnState) {
    // A region squeezed narrower than nothing paints nothing rather than allocating a negative area:
    // `egui` asserts on that, and a window that small is a layout in progress.
    let size = egui::vec2(
      ui.available_width().max(0.0),
      ui.available_height().max(0.0),
    );
    let (response, painter) = ui.allocate_painter(size, egui::Sense::click_and_drag());
    let rect = response.rect;
    // The camera projects into its own viewport, so it learns this frame's size before anything is
    // drawn; `cam-view` owns the arithmetic and knows nothing about egui's rectangles.
    self.camera.viewport = Viewport::new(f64::from(rect.width()), f64::from(rect.height()));
    self.take_input(ui, &response);
    self.paint(&painter, rect, drawn);
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

  /// Paint the surface, then the stock's edges, then the program.
  ///
  /// That order is the only depth ordering `egui::Mesh` can be given: it has no depth buffer, so the
  /// surface's own triangles arrive back to front from [`build_surface`] and the lines are drawn on
  /// top of a picture that is already correct.
  fn paint(&self, painter: &egui::Painter, rect: egui::Rect, drawn: &mut DrawnState) {
    self.paint_surface(painter, rect, drawn);
    self.paint_stock(painter, rect, drawn);
    self.paint_program(painter, rect, drawn);
  }

  /// Convert the surface mesh into one [`egui::Mesh`], shaded by each vertex's depth (A11).
  ///
  /// A triangle with a vertex the camera cannot project is skipped, not clamped: drawing it would
  /// mean inventing a screen position, and the surface would then say something the field does not.
  fn paint_surface(&self, painter: &egui::Painter, rect: egui::Rect, drawn: &mut DrawnState) {
    let surface = build_surface(&self.field, &self.camera, self.stride);
    if surface.is_empty() {
      return;
    }
    let mut mesh = egui::Mesh::default();
    for triangle in &surface.triangles {
      let mut corners = [Pos2::ZERO; 3];
      let mut colours = [Color32::BLACK; 3];
      let mut visible = true;
      for (slot, index) in triangle.iter().enumerate() {
        let vertex = surface.vertices[*index];
        match self.screen(rect, vertex.position) {
          Some(position) => {
            corners[slot] = position;
            colours[slot] = depth_colour(vertex.depth);
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
          color: colours[corner],
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
  fn paint_stock(&self, painter: &egui::Painter, rect: egui::Rect, drawn: &mut DrawnState) {
    for line in &self.stock {
      let (Some(start), Some(end)) = (self.screen(rect, line.start), self.screen(rect, line.end))
      else {
        continue;
      };
      painter.line_segment([start, end], Stroke::new(1.0, palette::STOCK));
      drawn.painted_3d_lines += 1;
    }
  }

  /// Paint the program's moves as projected 3D polylines.
  ///
  /// Arcs are flattened here, at half a pixel, for the same reason the 2D canvas flattens them: the
  /// painter owns the flattening, and every earlier stage keeps the arc as an arc. A movement with an
  /// end the camera cannot project is dropped rather than drawn through a made-up point. The
  /// segment's first move starts a polyline instead of being joined to the previous toolpath: the
  /// stream carries no start position, and inventing one would draw a line the program never made.
  fn paint_program(&self, painter: &egui::Painter, rect: egui::Rect, drawn: &mut DrawnState) {
    let tolerance = self.flatten_tolerance_mm();
    for segment in &self.program.segments {
      let mut previous: Option<Point3> = None;
      for movement in &segment.moves {
        let to = movement.destination();
        let stroke = match movement {
          Move::Feed { .. } | Move::Arc { .. } | Move::Helix { .. } => {
            Stroke::new(1.6, palette::CUT)
          }
          Move::Rapid { .. } | Move::Retract { .. } => Stroke::new(1.0, palette::TRAVEL),
        };
        if let Some(from) = previous {
          for pair in path_points(from, *movement, tolerance) {
            let (Some(start), Some(end)) = (self.screen(rect, pair.0), self.screen(rect, pair.1))
            else {
              continue;
            };
            painter.line_segment([start, end], stroke);
            drawn.painted_3d_lines += 1;
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
  for segment in &program.segments {
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
      });
      cumulative.push(total);
      previous = Some(to);
      departure = to;
    }
  }
  (steps, cumulative)
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

/// The colour a vertex with this depth is painted with.
///
/// `depth` is the mesh's own ramp coordinate, `0.0` at the untouched top and `1.0` at the stock's
/// bottom, so the same palette says the same thing about an 18 mm sheet and a 6 mm one. A value that
/// is not a number reads as the top rather than as a colour a shader would have to defend against.
fn depth_colour(depth: f32) -> Color32 {
  let t = if depth.is_finite() {
    depth.clamp(0.0, 1.0)
  } else {
    0.0
  };
  let mix = |top: u8, bottom: u8| -> u8 {
    let top = f32::from(top);
    let bottom = f32::from(bottom);
    (top + (bottom - top) * t).round().clamp(0.0, 255.0) as u8
  };
  Color32::from_rgb(
    mix(palette::SURFACE_TOP.r(), palette::SURFACE_BOTTOM.r()),
    mix(palette::SURFACE_TOP.g(), palette::SURFACE_BOTTOM.g()),
    mix(palette::SURFACE_TOP.b(), palette::SURFACE_BOTTOM.b()),
  )
}
