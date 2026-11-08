//! The 3D preview panel: a simulated stock, its camera, its playback, and the painter's mesh (M5 A9,
//! A10, A11, A12).
//!
//! # What this is, and what it is not
//!
//! The panel **shows** a program; it never authors one and never touches the document (D-M5-9). The
//! caller hands in the stock it has not cut yet — [`cam_sim::Heightfield::new`] — and the program to cut
//! it with, through [`Shell::set_preview`](crate::Shell::set_preview), together with the
//! [`Job`](cam_model::Job) the stock box is built from. M13 S3 closes the loop inside the shell:
//! `Shell::simulate` generates the document's own toolpaths in-process and hands the result in here, so
//! the picture is the drawing's own program rather than one a terminal produced. The panel still adds no
//! [`UiAction`](crate::UiAction) and no path from the picture back into the geometry: playing a program
//! moves a cursor, never the document.
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
//! The panel chooses no colour of its own: every one it paints is a [`Role`](crate::theme::Role) of the
//! current theme, or a stop on the depth ramp between two of them. The 3D view used to keep five shading
//! literals (L7 left them there on purpose), which made it the last place in the shell painting a colour
//! no role knew about. The mapping is a meaning, not a rename: the stock's box edges are
//! [`Role::StockEdge`](crate::theme::Role::StockEdge) — a silhouette, not the
//! [`Role::Border`](crate::theme::Role::Border) hairline the 2D canvas outlines the stock with, because a
//! block against a field with no grid needs a value that clears both the canvas and its own face; cutting
//! moves are [`Role::Cut`](crate::theme::Role::Cut), which is what the cut signal means; rapids and
//! retracts are [`Role::TextDim`](crate::theme::Role::TextDim), the role the 2D canvas paints construction
//! geometry with — present, but not the thing being shown; and the surface is
//! [`Palette::stock_shade`](crate::theme::Palette::stock_shade), a ramp with a role at each end. The
//! playback bar's fill is [`Role::Cut`](crate::theme::Role::Cut) for the same reason the cutting moves are:
//! it measures how much of the program has been cut, so the amber in the picture and the amber in the bar
//! say one thing. Nothing here keeps a colour literal, and the bar's own test finds both of its rectangles
//! by role rather than by position (`tests/view3d.rs`), because a literal would paint a bar no role test
//! could see.
//!
//! # Why the geometry is not computed here
//!
//! The projection ([`Camera3d`]) and the surface ([`build_surface`](cam_view::build_surface)) are data,
//! tested headless in `cam-view` (A9, S2). This module is the painter: it turns that data into
//! [`egui::Mesh`] triangles and lines on the existing [`egui::Painter`], which is why the 3D view needs no
//! new dependency (A11, D-M5-7). A `wgpu` backend would consume the same [`cam_view::SurfaceMesh`] when
//! S12's trigger fires; nothing here would have to change to allow it.

use cam_move::{Move, MoveStream, Point3, ToolShape};
use cam_sim::Heightfield;
use cam_view::{Camera3d, Line3};

mod chrome;
mod playback;
mod scene;
mod shell;

pub use playback::{bar_fill, clock_text, next_speed, progress_text, seconds_for, speed_text};
pub use scene::vector_lines;

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

/// The speeds the speed chip steps through, in program seconds per second of wall clock.
///
/// Five stops instead of a slider: the reference's bar names one speed, and these are the five an operator
/// actually uses — real time, then fast enough to watch a pocket, then fast enough to watch a program. The
/// chip names the one it is set to, so the control says what it did rather than where a handle sits.
pub const SPEEDS: [f64; 5] = [1.0, 5.0, 20.0, 60.0, 200.0];

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
  /// Where the sweep starts: exactly the origin [`sweep::apply`](cam_sim::sweep::apply) uses for this move.
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
