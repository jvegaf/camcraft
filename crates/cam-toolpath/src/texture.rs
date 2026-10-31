//! Texture: parallel grooves across a closed region, at a spacing and an angle (M6 A5, D-M6-6).
//!
//! The strategy's value is the **pattern**, not the clearing: the material between the grooves is
//! meant to stand, so this cuts grooves only — one pass each, at the authored depth, with the tool
//! centre on the groove — and never blends them into a pocket. That is why `--kind texture` says so in
//! its help and why the CLI refuses the flags a clearing operation would take (a stepover is this
//! strategy's spacing and nothing else, and a pass law would cut each groove twice).
//!
//! The pattern is **not computed here**. `TextureParams::grooves` derives it once, in `cam-model`, from
//! the region and the two parameters, and `cam-sim::target` authorises along the same polyline
//! vertices: a stored derived pattern and a recomputed one would eventually disagree, which is D-M6-2's
//! argument for a chamfer's depth applied to a whole lattice (D-M6-6). This module decides only how a
//! groove is cut, and the clipping into the region is the kernel's `Region::raster_spans` in the groove
//! frame, so no intersector lives here.

use cam_geom::Contour;
use cam_model::{Job, TextureParams};
use cam_move::{Move, Point3};
use cam_units::Length;

use crate::{machine_point, walk_contour, ToolSpec, ToolpathError};

/// Everything a texture needs.
#[derive(Clone, Copy, Debug)]
pub struct TextureRequest<'a> {
  /// The closed boundaries of the region the grooves are clipped to.
  ///
  /// The whole selection is one region and its parity rule does the work: a contour appended to the
  /// list turns the material inside it back into a hole, so a pattern can be cut around an island
  /// without an island concept anywhere (D-RB-2, D-M6-6).
  pub contours: &'a [&'a Contour],
  /// The spacing, the angle and the one depth the grooves cut to.
  pub params: &'a TextureParams,
  /// The tool doing the cutting.
  ///
  /// Deliberately not consulted for the geometry: the tool's centre runs on each groove, so a bit's
  /// diameter moves nothing about where the machine goes. It is carried because every strategy request
  /// carries the tool the emitter resolves once, and because the segment the emitter builds from the
  /// library is what tells the simulation the shape it sweeps.
  pub tool: &'a ToolSpec,
  /// Job setup, used to map document coordinates onto machine coordinates.
  pub job: &'a Job,
}

/// Compute the moves for a textured region.
///
/// One pass per groove of [`TextureParams::grooves`], at `-params.cut_depth`, with the tool centre on
/// the groove: a rapid over the groove's first vertex at the job's safe Z, one plunge there, the groove's
/// own vertices in order, and a retract at its last. Between grooves the tool goes back to `safe_z`, and
/// that is D-M4-6's linking rule unchanged — staying down would drag the bit across the material the
/// pattern is meant to leave standing (D-M6-6).
///
/// Three things are refused here, and all three are input errors rather than clamps: a `cut_depth` at or
/// below `start_depth` (`InvalidDepthRange`, the rule every one-depth strategy inherits from the profile),
/// a spacing that is not a positive distance (`InvalidSpacing`), and a pattern that yields no groove across
/// the region (`NoGroove`), which is a spacing with no lattice behind it rather than a texture with
/// nothing to do. An open contour is refused as `OpenContour` before any of that matters, because a
/// pattern is clipped to a region and an open vector bounds none.
///
/// The tool is not consulted: the depth is authored and the centre runs on the groove, so a bit's diameter
/// moves nothing about where the machine goes. It travels in the request because the segment the emitter
/// builds from the library is what tells the simulation the shape it sweeps.
pub fn texture(request: &TextureRequest<'_>) -> Result<Vec<Move>, ToolpathError> {
  let params = request.params;
  if params.cut_depth.mm() <= params.start_depth.mm() {
    return Err(ToolpathError::InvalidDepthRange {
      cut_depth: params.cut_depth.mm(),
      start_depth: params.start_depth.mm(),
    });
  }
  let spacing = params.spacing.mm();
  if !spacing.is_finite() || spacing <= 0.0 {
    return Err(ToolpathError::InvalidSpacing { spacing });
  }
  if request.contours.iter().any(|contour| !contour.is_closed()) {
    return Err(ToolpathError::OpenContour);
  }

  let grooves = params.grooves(request.contours);
  if grooves.is_empty() {
    return Err(ToolpathError::NoGroove {
      spacing,
      angle: params.angle.degrees(),
    });
  }

  let depth = params.cut_depth.mm();
  let safe_z = request.job.safe_z;
  let mut moves = Vec::new();
  for groove in grooves {
    // A groove is a polyline of the document's pattern, so it is walked as one: the same
    // `walk_contour` call an engrave makes, at the one depth, with no tabs and no pass loop.
    let contour = Contour::from_points(&groove, false).map_err(|_| ToolpathError::OpenContour)?;
    let start = contour
      .segments()
      .first()
      .map(|segment| segment.start_point())
      .ok_or(ToolpathError::OpenContour)?;
    let start = machine_point(request.job, start);
    moves.push(Move::Rapid {
      to: Point3 {
        x: Length::from_mm(start.x),
        y: Length::from_mm(start.y),
        z: safe_z,
      },
    });
    moves.push(Move::Feed {
      to: Point3 {
        x: Length::from_mm(start.x),
        y: Length::from_mm(start.y),
        z: Length::from_mm(-depth),
      },
    });
    moves.extend(walk_contour(&contour, request.job, -depth, &[], &|_| None));
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
  }
  Ok(moves)
}
