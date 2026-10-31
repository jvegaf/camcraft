//! Fluting: one pass along each selected vector, open or closed, with a linear ramp on each end.
//!
//! A flute is one groove per vector (D-M6-5), cut in **one** pass with the tool's centre on the vector,
//! so none of the profile's machinery applies: no side offset, no pass law, no lead, no tab. What it
//! has instead is a **ramp**, and the ramp is a length rather than a depth: with `ramp > 0` the pass
//! enters over the first `ramp` millimetres of the vector and leaves over the last, so a round-nose bit
//! never plunges at full depth into the end of a visible flute. `ramp = 0` is the plain groove, a
//! plunge in place at the start and a retract in place at the end.
//!
//! The entry and the exit are **real moves**, and each follows the vector rather than cutting across it:
//! the tool descends from the surface to the cut depth over the first `ramp` millimetres of the vector,
//! and climbs back over the last. On a straight run that is one linear move; on a vector that contains an
//! arc it is one **helical** move per arc (`Move::Helix`), because a ramped move that rode on a
//! constant-Z arc would be cut as a straight chord — `ramp² / (8·r)` inside the vector, half a millimetre
//! for a 10 mm ramp on a 25 mm circle. T4 refused the arc case for exactly that reason and T7 lifts the
//! refusal now that the helix exists; `ramp = 0` never had a problem, because a constant-depth pass along
//! an arc *is* what `Move::Arc` is for.
//!
//! A real controller interpolates both, and `cam-sim` does too (`D-M6-12`), so the program says exactly
//! what the machine does: the floor of the ramp is a slope, and the simulation reports it as one.

use cam_geom::Contour;
use cam_model::{FlutingParams, Job};
use cam_move::{Move, Point3};
use cam_units::Length;

use crate::{machine_point, ramp_range, walk_contour, walk_range, ToolSpec, ToolpathError};

/// Everything a flute needs.
#[derive(Clone, Copy, Debug)]
pub struct FlutingRequest<'a> {
  /// The geometry to flute, open or closed.
  pub contour: &'a Contour,
  /// The depth the pass cuts to and the length of the ramp at each end.
  pub params: &'a FlutingParams,
  /// The tool doing the cutting.
  ///
  /// Deliberately not consulted for the geometry: the tool's centre runs on the vector, so a bit's
  /// diameter moves nothing about where the machine goes. It is carried because every strategy request
  /// carries the tool the emitter resolves once, and because the segment the emitter builds from the
  /// library is what tells the simulation the shape it sweeps.
  pub tool: &'a ToolSpec,
  /// Job setup, used to map document coordinates onto machine coordinates.
  pub job: &'a Job,
}

/// Compute the moves for one flute.
///
/// One pass along the vector at `-params.cut_depth`, the tool centre on it. With `ramp = 0` the pass
/// opens with a plunge in place at the first vertex and closes with a retract in place at the last.
/// With `ramp > 0` the tool descends onto the surface where the vector starts, ramps down over the
/// first `ramp` millimetres, cuts the plateau between the two ramps, ramps back up over the last
/// `ramp` millimetres and retracts from the surface — so nothing in the program plunges into material.
///
/// Three things are refused here, and all three are input errors rather than clamps: a `cut_depth` at or
/// below `start_depth` (`InvalidDepthRange`, the rule an engrave inherits from the profile), a negative
/// `ramp` (`InvalidRamp`), and a `ramp` longer than half the vector (`RampTooLong`), because the entry and
/// the exit have to fit on it and a silently shortened ramp cuts both ends at a depth nobody authored
/// (A4). The vector's length is its perimeter, so a closed contour measures the groove it actually cuts.
/// A contour that contains an arc is **not** refused: its ramp rides the arc as a helix (see the module
/// doc).
pub fn fluting(request: &FlutingRequest<'_>) -> Result<Vec<Move>, ToolpathError> {
  let params = request.params;
  if params.cut_depth.mm() <= params.start_depth.mm() {
    return Err(ToolpathError::InvalidDepthRange {
      cut_depth: params.cut_depth.mm(),
      start_depth: params.start_depth.mm(),
    });
  }
  let ramp = params.ramp.mm();
  if !ramp.is_finite() || ramp < 0.0 {
    return Err(ToolpathError::InvalidRamp { ramp });
  }
  // Every contour a document can hold has at least two vertices (`cam_geom::Contour::new` refuses
  // fewer), so this guard is the boundary's own, and it reuses `OpenContour` for the same reason an
  // engrave does: an empty contour is not closed either.
  let start = request
    .contour
    .segments()
    .first()
    .map(|segment| segment.start_point())
    .ok_or(ToolpathError::OpenContour)?;
  let start = machine_point(request.job, start);
  let safe_z = request.job.safe_z;
  let depth = params.cut_depth.mm();
  let length = request.contour.perimeter();
  if ramp > 0.0 && ramp * 2.0 > length + 1.0e-9 {
    return Err(ToolpathError::RampTooLong { ramp, length });
  }

  let mut moves = vec![Move::Rapid {
    to: Point3 {
      x: Length::from_mm(start.x),
      y: Length::from_mm(start.y),
      z: safe_z,
    },
  }];

  if ramp <= 0.0 {
    moves.push(Move::Feed {
      to: Point3 {
        x: Length::from_mm(start.x),
        y: Length::from_mm(start.y),
        z: Length::from_mm(-depth),
      },
    });
    // The contour's own walk at the one depth: no tabs, no leads and no pass loop.
    moves.extend(walk_contour(
      request.contour,
      request.job,
      -depth,
      &[],
      &|_| None,
    ));
  } else {
    // Onto the surface first, so the ramp begins where the material does: the entry is the ramp, not a
    // plunge followed by one. `start_depth` bounds the authored depth rather than moving the entry,
    // exactly as it does for an engrave — a premachined surface is air as far as this pass is
    // concerned. The plunge's own travel is zero, so it is not a plunge *into* material at full depth
    // (A4).
    moves.push(Move::Feed {
      to: Point3 {
        x: Length::from_mm(start.x),
        y: Length::from_mm(start.y),
        z: Length::ZERO,
      },
    });
    // The entry: the first `ramp` millimetres of the vector, descending from the surface to the cut
    // depth as it goes. One linear move on a straight run, one helical move per arc otherwise.
    moves.extend(ramp_range(
      request.contour,
      request.job,
      0.0,
      ramp,
      0.0,
      -depth,
    ));
    // The plateau between the two ramps, arcs preserved.
    moves.extend(walk_range(
      request.contour,
      request.job,
      ramp,
      length - ramp,
      -depth,
    ));
    // The exit: the mirror of the entry, back up to the surface over the last `ramp` millimetres.
    moves.extend(ramp_range(
      request.contour,
      request.job,
      length - ramp,
      length,
      -depth,
      0.0,
    ));
  }

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
