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
mod chain;
mod contour;
pub mod edit;
pub mod intersect;
pub mod join;
pub mod offset;
mod point;
pub mod primitive;
mod segment;
pub mod transform;

pub use arc::Arc;
pub use chain::{chain, Chained};
pub use contour::{Contour, Vertex};
pub use point::{Bounds, Point2, Vec2};
pub use segment::{LineSeg, Segment};

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
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
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
}
