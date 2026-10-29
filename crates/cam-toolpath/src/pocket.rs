//! Pocket clearing: concentric offset rings, or parallel raster rows.
//!
//! Both modes cut the region inside the boundary with the tool centre `radius + allowance` away from
//! it, chunk the depth the way a profile does, and cut every ring or row at each depth pass.
//!
//! Geometry is never tessellated here: offset rings keep their arcs, and a raster row's ends are true
//! line/arc intersections.

use cam_geom::Contour;
use cam_model::{Job, PocketParams};
use cam_move::Move;

use crate::{ToolSpec, ToolpathError};

/// Everything a pocket pass needs.
#[derive(Clone, Copy, Debug)]
pub struct PocketRequest<'a> {
  /// The closed region to clear.
  pub contour: &'a Contour,
  /// Strategy, allowance, stepover and depths.
  pub params: &'a PocketParams,
  /// The tool doing the cutting.
  pub tool: &'a ToolSpec,
  /// Job setup, used to map document coordinates onto machine coordinates.
  pub job: &'a Job,
}

/// What a pocket produced, beyond the moves themselves.
#[derive(Clone, Debug, PartialEq)]
pub struct Pocketpath {
  /// The moves, in cutting order.
  pub moves: Vec<Move>,
  /// Number of rings (offset) or rows (raster) cut, before depth passes.
  pub rings: usize,
}

/// Compute the moves for a pocket cut.
///
/// Not implemented yet: M4 T2 fills this module.
pub fn pocket(request: &PocketRequest<'_>) -> Result<Pocketpath, ToolpathError> {
  let _ = request;
  Err(ToolpathError::NotImplemented)
}
