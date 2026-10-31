//! Quick engrave: one constant-depth pass along the selected vectors, open or closed.
//!
//! An engrave is the profile's `On` side with the pass law removed (D-M6-4). The original's Quick
//! Engrave engraves along vectors at one depth, and a pass law is a liability when the feature is a
//! single V-groove: a second pass at a deeper Z would cut a second groove into the first. So none of
//! the profile's pass, lead, tab or side machinery is reused — the walk is [`crate::walk_contour`] at
//! one depth with no tabs, which is the same arc-preserving contour walk a profile pass uses and
//! nothing more.
//!
//! The one thing this strategy accepts and a profile refuses is an **open** vector. Engraving a single
//! line is the strategy's point, so the pass starts at the contour's first vertex, follows its own
//! segments — arcs stay arcs — and stops at its last; a closed contour simply ends where it began.

use cam_geom::Contour;
use cam_model::{EngraveParams, Job};
use cam_move::{Move, Point3};
use cam_units::Length;

use crate::{machine_point, walk_contour, ToolSpec, ToolpathError};

/// Everything an engrave pass needs.
#[derive(Clone, Copy, Debug)]
pub struct EngraveRequest<'a> {
  /// The geometry to engrave, open or closed.
  pub contour: &'a Contour,
  /// The one depth the pass cuts to, and the depth it starts from.
  pub params: &'a EngraveParams,
  /// The tool doing the cutting.
  ///
  /// Deliberately not consulted for the geometry: the tool's centre runs on the vector and the depth
  /// is authored, so a bit's diameter and its angle move nothing about where the machine goes. It is
  /// carried because every strategy request carries the tool the emitter resolves once, and because
  /// the segment the emitter builds from the library is what tells the simulation the shape it sweeps.
  pub tool: &'a ToolSpec,
  /// Job setup, used to map document coordinates onto machine coordinates.
  pub job: &'a Job,
}

/// Compute the moves for an engrave pass.
///
/// One pass at `-params.cut_depth`, the tool centre on the vector: a rapid over the first vertex at
/// safe Z, one plunge there, the contour's own segments in order (arcs stay arcs), and a retract back
/// to safe Z. For a closed contour that ends where it started; for an open one it ends at its last
/// vertex.
///
/// The only parameter checked here is the depth range, and it is checked because the pass is the whole
/// strategy: a `cut_depth` at or below `start_depth` has no pass behind it, and
/// [`ToolpathError::InvalidDepthRange`] names both depths. Everything else an engrave does not have —
/// a side, a pass depth, a tab, a lead — is absent from [`EngraveParams`] by construction rather than
/// ignored here.
pub fn engrave(request: &EngraveRequest<'_>) -> Result<Vec<Move>, ToolpathError> {
  let params = request.params;
  if params.cut_depth.mm() <= params.start_depth.mm() {
    return Err(ToolpathError::InvalidDepthRange {
      cut_depth: params.cut_depth.mm(),
      start_depth: params.start_depth.mm(),
    });
  }

  // Every contour a document can hold has at least two vertices (`cam_geom::Contour::new` refuses
  // fewer), so this guard is the boundary's own. It reuses `OpenContour` rather than growing a variant
  // of its own: an empty contour is not closed either, and the error contract in
  // `cam-toolpath/tests/errors.rs` is the file that would have to learn a new sentence.
  let start = request
    .contour
    .segments()
    .first()
    .map(|segment| segment.start_point())
    .ok_or(ToolpathError::OpenContour)?;
  let start = machine_point(request.job, start);
  let safe_z = request.job.safe_z;
  // The authored depth is the pass's depth: the last pass lands exactly on `cut_depth`, like a
  // profile's, so nothing is added to it.
  let depth = params.cut_depth.mm();

  let mut moves = vec![
    Move::Rapid {
      to: Point3 {
        x: Length::from_mm(start.x),
        y: Length::from_mm(start.y),
        z: safe_z,
      },
    },
    Move::Feed {
      to: Point3 {
        x: Length::from_mm(start.x),
        y: Length::from_mm(start.y),
        z: Length::from_mm(-depth),
      },
    },
  ];
  // The contour's own walk at the one depth, with no tabs, no leads and no pass loop.
  moves.extend(walk_contour(
    request.contour,
    request.job,
    -depth,
    &[],
    &|_| None,
  ));

  let last = moves
    .last()
    .map(|movement| movement.destination())
    .unwrap_or(Point3::ORIGIN);
  moves.push(Move::Retract {
    to: Point3 {
      x: last.x,
      y: last.y,
      z: safe_z,
    },
  });
  Ok(moves)
}
