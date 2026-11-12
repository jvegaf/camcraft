//! 2D geometry kernel: points, segments, arcs, bulged contours, chaining, tessellation and
//! contour offsetting.
//!
//! # Invariants
//!
//! * All coordinates are **millimetres** (`f64`), per [`cam_units`].
//! * Arcs are **first-class**. Nothing in this crate pre-tessellates a contour: tessellation is an
//!   explicit, tolerance-driven operation that only the renderer and the simulator ask for. This is
//!   what lets an arc authored in a document reach the post-processor as a `G2`/`G3` move.
//! * Every operation that compares positions takes an explicit `tolerance`, because chaining and
//!   joining decisions are tolerance problems, not exact-arithmetic problems.
//!
//! Must not depend on `cam-store`, `cam-model`, `cam-post` or any UI crate.

#![warn(missing_docs)]

mod arc;
pub mod boolean;
mod chain;
mod contour;
pub mod distance;
pub mod edit;
pub mod intersect;
pub mod join;
pub mod nest;
pub mod nest_profile;
pub mod offset;
mod point;
pub mod primitive;
pub mod region;
pub mod region_offset;
mod segment;
pub mod transform;
pub mod trim;
pub mod validate;
pub mod winding;

pub use arc::Arc;
pub use chain::{chain, Chained};
pub use contour::{Contour, Vertex};
pub use distance::segment_to_segment;
pub use point::{Bounds, Point2, Vec2};
pub use segment::{LineSeg, NearestPoint, Segment};

/// Default tolerance for chaining and joining, in millimetres.
///
/// One micrometre: well below any real machine resolution, well above `f64` noise for
/// job-scale coordinates.
pub const DEFAULT_TOLERANCE: f64 = 1.0e-6;

/// Upper bound on tessellation segments for a single arc or curve.
///
/// A tolerance far below the arc radius must not be able to allocate without limit.
pub const MAX_TESSELLATION_STEPS: usize = 4096;

/// Errors produced by the geometry kernel.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum GeomError {
  /// A contour needs at least two vertices.
  #[error("a contour needs at least two vertices, got {vertices}")]
  ContourTooShort {
    /// Number of vertices supplied.
    vertices: usize,
  },
  /// An arc could not be built from the supplied definition.
  #[error("degenerate arc: {reason}")]
  DegenerateArc {
    /// What made the definition degenerate.
    reason: &'static str,
  },
  /// An offset collapsed a contour, usually because the distance exceeds the feature size.
  #[error("offset collapsed the contour: {reason}")]
  OffsetCollapsed {
    /// What collapsed.
    reason: &'static str,
  },
  /// A builder was asked for a shape that cannot exist.
  #[error("invalid primitive: {reason}")]
  InvalidPrimitive {
    /// What made the request impossible.
    reason: &'static str,
  },
  /// An edit was refused because it would break the contour.
  #[error("edit refused: {reason}")]
  EditRefused {
    /// Why the edit could not be applied.
    reason: &'static str,
  },
  /// A nesting run was given a gap that is not a distance: negative or not finite.
  ///
  /// A gap that is not a distance cannot separate anything, so it is refused rather than replaced
  /// by zero (M9 T2b).
  #[error("a nesting gap must be a finite number of millimetres zero or more, got {gap} mm")]
  GapNotMeasurable {
    /// The offending gap in millimetres.
    gap: f64,
  },
  /// A nesting sheet has no positive, finite side lengths, so it is not a board.
  #[error("a nesting sheet needs positive millimetre side lengths, got {width} mm by {height} mm")]
  SheetNotMeasurable {
    /// The sheet's width in millimetres.
    width: f64,
    /// The sheet's height in millimetres.
    height: f64,
  },
  /// A nesting part has a negative or non-finite side length, so it is not a rectangle.
  ///
  /// A part with a zero side is accepted: it covers no area, which is a shape the nester can carry.
  #[error(
    "nesting part {part} needs non-negative, finite millimetre side lengths, got {width} mm by \
     {height} mm"
  )]
  PartNotMeasurable {
    /// Index of the offending part in the caller's list.
    part: usize,
    /// The offending width in millimetres.
    width: f64,
    /// The offending height in millimetres.
    height: f64,
  },
  /// A free-profile nesting run was given a coarse search step that is not a distance.
  ///
  /// A step that cannot advance would search one anchor and report the board searched, so it is
  /// refused with its value (M9 T3).
  #[error(
    "a free-profile nesting search step must be a positive number of millimetres, got {step} mm"
  )]
  SearchStepNotMeasurable {
    /// The offending step in millimetres.
    step: f64,
  },
  /// A free-profile nesting run was given a search step fine enough to ask for more lattice anchors
  /// than the cap allows.
  ///
  /// The cost of a search is `(floor(width/step)+1) * (floor(height/step)+1)` anchors, so halving the
  /// step quadruples it and a step of a thousandth of a millimetre asks a full sheet for 10^12 of them.
  /// The refusal names the step, the lattice it would walk and the cap, in the words the simulator's
  /// cell cap already uses, because it is the same kind of cost (M9, advisory `R1-STEP-DOS`).
  #[error(
    "a {step} mm search step over this board needs {columns} x {rows} = {anchors} anchors, over the \
     {cap} anchor cap; use a coarser --step"
  )]
  SearchLatticeTooFine {
    /// The offending step in millimetres.
    step: f64,
    /// The lattice's columns, one per anchor position across the board.
    columns: usize,
    /// The lattice's rows, one per anchor position up the board.
    rows: usize,
    /// `columns * rows`: the anchors the step would walk.
    anchors: usize,
    /// The most anchors a single search may walk.
    cap: usize,
  },
  /// A free-profile nesting sheet is not a closed contour, so it encloses no board.
  #[error("a free-profile nesting sheet must be a closed contour")]
  SheetNotClosed,
  /// A free-profile nesting part is not a closed contour, so it is not a shape to nest.
  #[error("free-profile nesting part {part} must be a closed contour")]
  PartNotClosed {
    /// Index of the offending part in the caller's list.
    part: usize,
  },
}
