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
pub mod medial;
pub mod moulding;
pub mod nest;
pub mod nest_profile;
pub mod offset;
mod point;
pub mod primitive;
pub mod region;
pub mod region_offset;
mod segment;
pub mod text;
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
  /// A glyph source cannot draw a character.
  #[error("the glyph source has no glyph for {character:?}")]
  UnsupportedCharacter {
    /// The character that could not be drawn.
    character: char,
  },
  /// A moulding rail has no length for the sweep to advance along.
  ///
  /// A rail with no perimeter has no sweep behind it, and a point asked of it would be an invention
  /// rather than a place on a shape (M8 T2).
  #[error("a moulding rail needs a length to sweep along, got {length} mm")]
  MouldingRailTooShort {
    /// The rail's perimeter in millimetres.
    length: f64,
  },
  /// A moulding profile has fewer than two points, so it is not a cross-section.
  #[error("a moulding profile needs at least two points, got {points}")]
  MouldingProfileTooShort {
    /// How many points were supplied.
    points: usize,
  },
  /// A moulding profile point is not a finite pair of millimetres.
  #[error(
    "a moulding profile point must use finite millimetres, got lateral {lateral} mm and depth \
     {depth} mm"
  )]
  MouldingProfilePointNotFinite {
    /// The offending lateral distance in millimetres.
    lateral: f64,
    /// The offending depth in millimetres.
    depth: f64,
  },
  /// A moulding profile's lateral distances are not strictly increasing.
  ///
  /// A cross-section is one depth per lateral distance, so a lateral that does not exceed the one
  /// before it is two depths for the same place. Refused with both values rather than resolved by
  /// picking one (M8 T2, T3).
  #[error(
    "moulding profile point {index} at lateral {lateral} mm must be greater than the previous \
     {previous} mm: a cross-section is one depth per lateral distance"
  )]
  MouldingProfileLateralOrder {
    /// Index of the offending point in the profile.
    index: usize,
    /// The lateral distance of the point before it, in millimetres.
    previous: f64,
    /// The offending lateral distance in millimetres.
    lateral: f64,
  },
  /// A moulding stepover is zero, negative or not finite, so it cannot space passes.
  #[error("a moulding stepover must be a positive number of millimetres, got {stepover} mm")]
  MouldingStepoverNotPositive {
    /// The offending stepover in millimetres.
    stepover: f64,
  },
  /// A moulding walk was asked for a chord tolerance that is zero, negative or not finite.
  #[error(
    "a moulding rail sample tolerance must be a positive number of millimetres, got {tolerance} mm"
  )]
  MouldingToleranceNotPositive {
    /// The offending tolerance in millimetres.
    tolerance: f64,
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
