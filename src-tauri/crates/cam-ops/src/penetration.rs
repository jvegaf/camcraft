//! Penetration predicate: detects when a swept cut overlaps protected copper.
//!
//! Models the geometric oracle described in the roadmap/gaps: buffer of cut paths by tool radius
//! intersected with eroded (protected) copper. A non-negligible intersection means the tool would
//! gouge copper that must remain.
//!
//! This is a pure geometric predicate intended for CI and operation guards (E2.4 / L1).

use cam_geom::boolean::{self, BooleanOp, Combined};
use cam_geom::region::Region;
use cam_geom::{GeomError, TolerancePolicy};

/// Ratio above which an overlap is considered penetration.
///
/// Matches the roadmap/gaps threshold: "ratio de penetración > ~1e-9".
pub const PENETRATION_THRESHOLD: f64 = 1.0e-9;

/// Material area of a region, in square millimetres.
///
/// For a normalized region whose boundaries are oriented with material on the left, the signed area
/// of each boundary sums to the material area (outer boundaries positive, holes negative). The
/// absolute value is returned so callers get a non-negative area regardless of input orientation.
pub fn region_area(region: &Region) -> f64 {
  region
    .boundaries
    .iter()
    .map(|boundary| boundary.signed_area())
    .sum::<f64>()
    .abs()
}

/// Area of the intersection between two regions, in square millimetres.
///
/// Returns `0.0` when the regions do not overlap.
pub fn intersection_area(
  a: &Region,
  b: &Region,
  tolerance: TolerancePolicy,
) -> Result<f64, GeomError> {
  match boolean::combine(a, b, BooleanOp::Intersection, tolerance)? {
    Combined::Empty => Ok(0.0),
    Combined::Region { region, .. } => Ok(region_area(&region)),
  }
}

/// Ratio of protected copper penetrated by a swept cut.
///
/// The ratio is `intersection_area(swept_cut, protected_copper) / area(protected_copper)`. It is
/// `0.0` when there is no overlap, and `0.0` when `protected_copper` has no material area (nothing
/// protected can be penetrated).
///
/// `swept_cut` is the tool's swept region (cut paths buffered by tool radius). `protected_copper`
/// is the eroded copper region that must not be cut.
pub fn penetration_ratio(
  swept_cut: &Region,
  protected_copper: &Region,
  tolerance: TolerancePolicy,
) -> Result<f64, GeomError> {
  let protected = region_area(protected_copper);
  if protected <= 0.0 {
    return Ok(0.0);
  }
  let overlap = intersection_area(swept_cut, protected_copper, tolerance)?;
  Ok(overlap / protected)
}

/// Whether a swept cut overlaps protected copper beyond the given threshold.
///
/// This is the FlatCAM-style `_cut_model_overlaps_copper` predicate: it returns `true` when the
/// penetration ratio exceeds `threshold`.
pub fn cut_model_overlaps_copper(
  swept_cut: &Region,
  protected_copper: &Region,
  tolerance: TolerancePolicy,
  threshold: f64,
) -> Result<bool, GeomError> {
  Ok(penetration_ratio(swept_cut, protected_copper, tolerance)? > threshold)
}
