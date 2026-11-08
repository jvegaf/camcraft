use cam_app::Viewport;
use cam_model::Job;
use cam_move::{Move, MoveStream, Point3};
use cam_sim::{sweep, Heightfield};
use cam_units::Length;
use cam_view::{build_stock_box, Camera3d, Line3, MIN_DISTANCE};

use super::scene::arc_sweep;
use super::{Preview3d, Step, DEFAULT_SPEED, MAX_LATTICE, SPEEDS};

/// How much larger than the stock's diagonal the camera's distance is when the panel opens, so the
/// stock sits inside the picture with a margin rather than touching its edges.
const FRAME_MARGIN: f64 = 1.4;

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
  pub(super) fn step_under_playhead(&self) -> Option<Step> {
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
