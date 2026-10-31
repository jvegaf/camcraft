//! Chamfer: one V-bit pass along the selected vectors, at a depth derived from the bit.
//!
//! A V-bit's bevel is the cone's flank, so the bevel's width and the bit's included angle fix how deep
//! the tip has to go for the flank to meet the surface exactly `chamfer_width` from the vector:
//! `start_depth + chamfer_width / tan(included_angle / 2)` (D-M6-2). The tip travels **on** the vector
//! and the bevel is symmetric about it, so there is no side to choose and no lateral offset to apply
//! (L7), and the derived Z is the only thing the bevel's width changes about the path.
//!
//! One pass, whatever the depth. A second pass at a deeper Z would cut a second bevel into the first,
//! and the profile's pass law is exactly the liability here — so none of that machinery is reused: the
//! walk is [`crate::walk_contour`] with no tabs, which is the same arc-preserving contour walk a
//! profile pass uses and nothing more (D-M6-2, D-M6-4).

use cam_geom::Contour;
use cam_model::{ChamferParams, Job};
use cam_move::{Move, Point3};
use cam_units::Length;

use crate::{machine_point, walk_contour, ToolSpec, ToolpathError};

/// Everything a chamfer pass needs.
#[derive(Clone, Copy, Debug)]
pub struct ChamferRequest<'a> {
  /// The closed geometry to bevel.
  pub contour: &'a Contour,
  /// The bevel's width, the bit's angle and the depth it starts from.
  pub params: &'a ChamferParams,
  /// The tool doing the cutting.
  ///
  /// Deliberately not consulted for the geometry: the tip runs on the vector and the depth comes from
  /// `params`, so a bit's diameter and its flat move nothing about where the machine goes (D-M6-2).
  /// It is carried because every strategy request carries the tool the emitter resolves once.
  pub tool: &'a ToolSpec,
  /// Job setup, used to map document coordinates onto machine coordinates.
  pub job: &'a Job,
}

/// Compute the moves for a chamfer pass.
///
/// One pass at `-params.depth()`, the tip on the contour: a rapid over the first vertex at safe Z, one
/// plunge there, the contour's own segments in order (arcs stay arcs), and a retract back to safe Z.
///
/// The parameters are not re-validated here, with one exception. A width with no bevel behind it is
/// refused by the document model before a toolpath is stored (`ModelError::ChamferWidthNotPositive`),
/// naming the value, so this layer asks for the derivation rather than guarding it twice. The **angle**
/// is checked here as well, because the tangent of zero is zero and a caller that reaches the strategy
/// without a validated document must not receive an infinite Z (D-M6-10).
pub fn chamfer(request: &ChamferRequest<'_>) -> Result<Vec<Move>, ToolpathError> {
  if !request.contour.is_closed() {
    return Err(ToolpathError::OpenContour);
  }
  let params = request.params;
  let angle = params.included_angle.radians();
  if !(angle > 0.0 && angle < std::f64::consts::PI) {
    return Err(ToolpathError::InvalidIncludedAngle {
      degrees: params.included_angle.degrees(),
    });
  }

  let start = request
    .contour
    .segments()
    .first()
    .map(|segment| segment.start_point())
    .ok_or(ToolpathError::OpenContour)?;
  let start = machine_point(request.job, start);
  let safe_z = request.job.safe_z;
  let depth = params.depth().mm();

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
