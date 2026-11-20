//! Copper isolation: union the copper of a layer, then offset it in passes to route isolation paths.
//!
//! E3.1 computes the copper region of a layer by uniting its copper boundaries (a set union, so
//! overlapping copper becomes one area). E3.2 offsets that region outward by the isolation distance of
//! each pass, producing concentric rings around the copper. The distance of pass `i` (0-based) is
//! `tool_diameter * ((2*i + 1) / 2) - i * overlap * tool_diameter`, which is the accumulated
//! multipass offset law: the first pass clears half a tool diameter from the copper and later passes
//! step out by `tool_diameter * (1 - overlap)`.
//!
//! Geometry is never tessellated here: offsets keep their arcs wherever the kernel does, and the
//! kernel's distance proof refuses a piece that would gouge the copper it was offset from.

use cam_geom::boolean::{combine, BooleanOp, Combined};
use cam_geom::region::{Region, RegionOffset};
use cam_geom::{Contour, GeomError, TolerancePolicy};
use cam_model::MillingDirection;

/// The copper of one layer, united into a single region (E3.1).
///
/// Overlapping copper boundaries are merged by set union so the result describes the area the copper
/// actually covers, not a stack of separate shapes. Boundaries that cross or touch are resolved by the
/// same normalization the boolean engine uses.
///
/// An empty input produces an empty region, which is the copper of a layer with no copper.
///
/// # Errors
///
/// Returns the first geometric error reported by normalization or union: a boundary that cannot be
/// made into a region, a tolerance that cannot resolve the arrangement, or a union the engine refuses.
pub fn unite_copper(contours: &[Contour], tolerance: TolerancePolicy) -> Result<Region, GeomError> {
  let mut united = Region::new(Vec::new())?;
  for contour in contours {
    let piece = Region::new(vec![contour.clone()])?
      .normalize(tolerance)?
      .region;
    united = match combine(&united, &piece, BooleanOp::Union, tolerance)? {
      Combined::Empty => Region::new(Vec::new())?,
      Combined::Region { region, .. } => region,
    };
  }
  Ok(united)
}

/// One isolation pass: the offset geometry at a given distance from the copper.
#[derive(Clone, Debug, PartialEq)]
pub struct IsolationPass {
  /// Zero-based pass index.
  pub index: usize,
  /// Offset distance from the copper boundary, in millimetres.
  pub distance: f64,
  /// The isolation paths of this pass: boundaries of the offset region, in the order the kernel
  /// produced them.
  pub contours: Vec<Contour>,
  /// True when some path had to be rebuilt as straight segments instead of keeping its arcs.
  pub flattened: bool,
}

/// The isolation passes computed for a copper region (E3.2).
#[derive(Clone, Debug, PartialEq)]
pub struct IsolationPath {
  /// One entry per pass that produced geometry, in pass order.
  pub passes: Vec<IsolationPass>,
}

/// Compute the isolation passes around `copper` (E3.2).
///
/// For each pass `i` in `0..passes` the copper region is offset outward by
/// `tool_diameter * ((2*i + 1) / 2) - i * overlap * tool_diameter`. The offset is taken from the
/// original copper region, not accumulated from the previous pass, so the distance of every pass is
/// exact and independent.
///
/// Passes whose offset leaves no material because the distance is larger than the clearance around
/// the copper are omitted rather than treated as an error, because later passes are farther away and
/// a layer may admit some passes but not all of them. A request that produces no pass at all is
/// refused as a collapsed offset so the caller knows the tool does not fit.
///
/// `direction` sets the milling direction of the emitted paths (climb or conventional). Contours are
/// oriented with the cut material on the appropriate side for the requested direction.
///
/// # Errors
///
/// Returns an error when `tool_diameter` is not positive, when `passes` is zero, when `overlap` is
/// not in `[0, 1)`, when the copper region cannot be offset for the first pass, or when the kernel
/// reports a geometric or tolerance error that is not an exhausted-clearance case for a later pass.
pub fn isolation_passes(
  copper: &Region,
  tool_diameter: f64,
  passes: u32,
  overlap: f64,
  direction: MillingDirection,
  tolerance: TolerancePolicy,
) -> Result<IsolationPath, GeomError> {
  if !tool_diameter.is_finite() || tool_diameter <= 0.0 {
    return Err(GeomError::EditRefused {
      reason: "an isolation pass needs a positive, finite tool diameter",
    });
  }
  if passes == 0 {
    return Err(GeomError::EditRefused {
      reason: "an isolation pass needs at least one pass",
    });
  }
  if !overlap.is_finite() || !(0.0..1.0).contains(&overlap) {
    return Err(GeomError::EditRefused {
      reason: "an isolation overlap must be a finite number in [0, 1)",
    });
  }

  let mut result_passes = Vec::with_capacity(passes as usize);
  for index in 0..passes as usize {
    let distance = pass_distance(tool_diameter, index, overlap);
    let offset = match copper.offset(-distance, tolerance) {
      Ok(offset) => offset,
      Err(err) if index == 0 => return Err(err),
      Err(err) if is_clearance_exhausted(&err) => break,
      Err(err) => return Err(err),
    };
    let flattened = offset.flattened;
    if offset.region.boundaries.is_empty() {
      if index == 0 {
        return Err(GeomError::OffsetCollapsed {
          reason: "the isolation offset left no path to cut",
        });
      }
      break;
    }
    let contours = orient_pass(offset, direction);
    result_passes.push(IsolationPass {
      index,
      distance,
      contours,
      flattened,
    });
  }

  if result_passes.is_empty() {
    return Err(GeomError::OffsetCollapsed {
      reason: "the isolation offset left no path to cut",
    });
  }

  Ok(IsolationPath {
    passes: result_passes,
  })
}

/// The offset distance of pass `i` (0-based), in millimetres.
///
/// `dia * ((2*i + 1) / 2) - i * overlap * dia`, i.e. half a tool diameter for the first pass and one
/// stepover of `dia * (1 - overlap)` further out for each subsequent pass.
pub fn pass_distance(tool_diameter: f64, index: usize, overlap: f64) -> f64 {
  tool_diameter * ((2 * index + 1) as f64 / 2.0) - (index as f64) * overlap * tool_diameter
}

/// Depth a V-bit must cut to produce a given isolation width (E3.3, E1.8).
///
/// A V-bit with included angle `included_angle_deg` and tip flat diameter `tip_diameter_mm` cuts an
/// effective width of `tip_diameter_mm + 2 * depth * tan(included_angle_deg / 2)` at `depth` below the
/// tip. This function returns the depth that produces `target_diameter_mm` as that effective isolation
/// width.
///
/// `target_diameter_mm` is the isolation tool diameter/width to achieve (for example the
/// `tool_diameter` used by [`isolation_passes`]). All lengths are in millimetres and the angle is in
/// degrees.
///
/// # Errors
///
/// Returns an error when the angle or diameters are not finite, when the included angle is not in
/// `(0, 180)` degrees, when diameters are negative, or when `target_diameter_mm` is smaller than
/// `tip_diameter_mm` (a V-bit cannot cut narrower than its tip).
pub fn vbit_isolation_depth_mm(
  target_diameter_mm: f64,
  included_angle_deg: f64,
  tip_diameter_mm: f64,
) -> Result<f64, GeomError> {
  if !target_diameter_mm.is_finite()
    || !included_angle_deg.is_finite()
    || !tip_diameter_mm.is_finite()
  {
    return Err(GeomError::EditRefused {
      reason:
        "a V-bit isolation depth needs finite target diameter, included angle, and tip diameter",
    });
  }
  if !(included_angle_deg > 0.0 && included_angle_deg < 180.0) {
    return Err(GeomError::EditRefused {
      reason: "a V-bit isolation depth needs an included angle in (0, 180) degrees",
    });
  }
  if target_diameter_mm < 0.0 || tip_diameter_mm < 0.0 {
    return Err(GeomError::EditRefused {
      reason: "a V-bit isolation depth needs non-negative diameters",
    });
  }
  if target_diameter_mm + 1.0e-12 < tip_diameter_mm {
    return Err(GeomError::EditRefused {
      reason: "a V-bit cannot cut an isolation width smaller than its tip diameter",
    });
  }
  let half_angle_rad = (included_angle_deg * 0.5).to_radians();
  let slope = 2.0 * half_angle_rad.tan();
  if slope <= 0.0 || !slope.is_finite() {
    return Err(GeomError::EditRefused {
      reason: "a V-bit isolation depth needs a valid included angle",
    });
  }
  let depth = (target_diameter_mm - tip_diameter_mm).max(0.0) / slope;
  Ok(depth)
}

/// Whether an offset error means this pass cannot fit around the copper (clearance exhausted),
/// rather than a malformed or unresolvable geometry.
fn is_clearance_exhausted(err: &GeomError) -> bool {
  match err {
    GeomError::EditRefused { reason } => {
      reason.contains("leaves no material")
        || reason.contains("collapsed an enclosing boundary")
        || reason.contains("collapsed a hole boundary")
    }
    GeomError::OffsetCollapsed { .. } => true,
    _ => false,
  }
}

/// Orient the contours of an offset pass for the requested milling direction.
///
/// Conventional milling keeps the contour orientation as produced by the offset (material on the
/// conventional side). Climb milling reverses each contour so the tool climbs along the cut.
fn orient_pass(offset: RegionOffset, direction: MillingDirection) -> Vec<Contour> {
  let RegionOffset { region, .. } = offset;
  match direction {
    MillingDirection::Conventional => region.boundaries,
    MillingDirection::Climb => region
      .boundaries
      .into_iter()
      .map(|contour| contour.reverse())
      .collect(),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn pass_distance_matches_the_roadmap_formula() {
    let d = 1.0;
    let overlap = 0.5;
    assert!((pass_distance(d, 0, overlap) - 0.5).abs() < 1.0e-12);
    assert!((pass_distance(d, 1, overlap) - 1.0).abs() < 1.0e-12);
    assert!((pass_distance(d, 2, overlap) - 1.5).abs() < 1.0e-12);
  }

  #[test]
  fn vbit_isolation_depth_for_30deg_tip_0_2mm() {
    // E3.3 / E1.8: 30° V-bit with 0.2 mm tip. Effective width w(d) = tip + 2*d*tan(15°),
    // so depth for target width w is (w - tip) / (2*tan(15°)).
    // For target width 0.2 + 2*tan(15°) the expected depth is 1.0 mm.
    let included_angle_deg = 30.0;
    let tip_diameter_mm = 0.2;
    let target_diameter_mm = 0.2 + 2.0 * (15.0_f64.to_radians()).tan();
    let depth =
      vbit_isolation_depth_mm(target_diameter_mm, included_angle_deg, tip_diameter_mm).unwrap();
    assert!(
      (depth - 1.0).abs() < 1.0e-12,
      "depth {depth} for target {target_diameter_mm}"
    );
  }
}
