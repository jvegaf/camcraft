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
use cam_geom::region::Region;
use cam_geom::{Contour, GeomError, TolerancePolicy};
use cam_model::{IsolationKind, Job, MillingDirection};
use cam_move::{Move, Point3};
use cam_units::Length;

use crate::penetration;
use crate::shared::{machine_point, push_horizontal, ToolpathError};

/// The copper of one layer, united into a single region (E3.1).
///
/// Overlapping copper boundaries are merged by set union so the result describes the area the copper
/// actually covers, not a stack of separate shapes. Boundaries that cross or touch are resolved by the
/// same normalization the boolean engine uses. A contour whose winding is opposite (negative signed
/// area, a hole boundary in a resolved region) is subtracted so the material region is preserved.
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
    let subtractive = contour.signed_area() < 0.0;
    let piece = Region::new(vec![contour.clone()])?
      .normalize(tolerance)?
      .region;
    let op = if subtractive {
      BooleanOp::Difference
    } else {
      BooleanOp::Union
    };
    united = match combine(&united, &piece, op, tolerance)? {
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
/// For each pass `i` in `0..passes` the copper region is offset by
/// `tool_diameter * ((2*i + 1) / 2) - i * overlap * tool_diameter`. The offset is taken from the
/// original copper region, not accumulated from the previous pass, so the distance of every pass is
/// exact and independent.
///
/// This function emits full isolation (`Exterior` and `Interior`) and does not keep pours; it is
/// equivalent to [`isolation_passes_with_kind`] with [`IsolationKind::Full`] and `keep_pours = false`.
/// Use [`isolation_passes_with_kind`] to select `iso_type` or to preserve pours (E3.5).
///
/// Passes whose offset leaves no material because the distance is larger than the clearance are
/// omitted rather than treated as an error once at least one pass has been produced.
///
/// # Errors
///
/// Returns an error when `tool_diameter` is not positive, when `passes` is zero, when `overlap` is
/// not in `[0, 1)`, when the copper region cannot be offset for the first pass that must produce
/// geometry, or when the kernel reports a geometric or tolerance error that is not an exhausted
/// clearance case for a later pass.
pub fn isolation_passes(
  copper: &Region,
  tool_diameter: f64,
  passes: u32,
  overlap: f64,
  direction: MillingDirection,
  tolerance: TolerancePolicy,
) -> Result<IsolationPath, GeomError> {
  isolation_passes_with_kind(
    copper,
    tool_diameter,
    passes,
    overlap,
    direction,
    IsolationKind::Full,
    false,
    tolerance,
  )
}

/// Compute isolation passes with explicit `iso_type` and `keep_pours` selection (E3.5).
///
/// All isolation paths are cut outside the copper material: the copper region is offset outward by
/// the pass distance (`offset(-distance)`), so outer copper boundaries expand and hole boundaries
/// inside copper contract. `kind` selects which of those paths to emit: [`IsolationKind::Exterior`]
/// keeps paths around copper exteriors (outer boundaries), [`IsolationKind::Interior`] keeps paths
/// around interior clearances (hole boundaries inside copper), and [`IsolationKind::Full`] keeps
/// both. When `keep_pours` is true, interior paths inside copper pour areas are omitted.
#[allow(clippy::too_many_arguments)]
pub fn isolation_passes_with_kind(
  copper: &Region,
  tool_diameter: f64,
  passes: u32,
  overlap: f64,
  direction: MillingDirection,
  kind: IsolationKind,
  keep_pours: bool,
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

  let pours = if keep_pours && matches!(kind, IsolationKind::Interior | IsolationKind::Full) {
    pour_regions(copper)
  } else {
    Vec::new()
  };

  let exterior_source = if matches!(kind, IsolationKind::Exterior | IsolationKind::Full) {
    Some(exteriors_only_region(copper, tolerance)?)
  } else {
    None
  };

  let mut result_passes = Vec::with_capacity(passes as usize);
  for index in 0..passes as usize {
    let distance = pass_distance(tool_diameter, index, overlap);
    let mut contours = Vec::new();
    let mut flattened = false;

    match copper.offset(-distance, tolerance) {
      Ok(offset) => {
        flattened |= offset.flattened;
        for contour in offset.region.boundaries {
          if !select_boundary_for_kind(&contour, kind) {
            continue;
          }
          if keep_pours
            && is_interior_boundary(&contour)
            && contour_inside_any_pour(&contour, &pours)
          {
            continue;
          }
          contours.push(contour);
        }
      }
      Err(err) if is_hole_collapse_error(&err) => {
        // Some holes collapsed growing copper outward; get exterior from holes-filled region and/or
        // interior per-hole (skipping collapsed holes).
        if matches!(kind, IsolationKind::Exterior | IsolationKind::Full) {
          if let Some(ref src) = exterior_source {
            match src.offset(-distance, tolerance) {
              Ok(o) => {
                flattened |= o.flattened;
                for contour in o.region.boundaries {
                  if select_boundary_for_kind(&contour, kind) {
                    contours.push(contour);
                  }
                }
              }
              Err(e) if index == 0 && kind == IsolationKind::Exterior => return Err(e),
              Err(e) if is_clearance_exhausted(&e) && kind == IsolationKind::Exterior => break,
              Err(e) if kind == IsolationKind::Exterior => return Err(e),
              Err(_) => {}
            }
          }
        }
        if matches!(kind, IsolationKind::Interior | IsolationKind::Full) {
          for contour in interior_boundaries_per_hole(copper, distance, tolerance)? {
            if keep_pours && contour_inside_any_pour(&contour, &pours) {
              continue;
            }
            contours.push(contour);
          }
        }
        if contours.is_empty() {
          if index == 0 {
            return Err(err);
          }
          break;
        }
      }
      Err(err) if index == 0 => return Err(err),
      Err(err) if is_clearance_exhausted(&err) => break,
      Err(err) => return Err(err),
    }

    if contours.is_empty() {
      if index == 0 {
        return Err(GeomError::OffsetCollapsed {
          reason: "the isolation offset left no path to cut",
        });
      }
      if !result_passes.is_empty() {
        break;
      }
      continue;
    }

    let contours = orient_contours(contours, direction);
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

/// Interior isolation paths computed per hole boundary when full-region offset collapses holes.
///
/// Shrinks each hole (filled region of the hole contour) by `distance` so the path lies inside the
/// original hole, `distance` away from its edge (outside the copper material). Exhausted/collapsed
/// holes are skipped; unexpected geometric errors are reported instead of silently omitting paths.
///
/// # Errors
///
/// Returns an error when a hole boundary cannot be represented, normalized, or offset for reasons
/// other than exhausted clearance or hole collapse.
fn interior_boundaries_per_hole(
  copper: &Region,
  distance: f64,
  tolerance: TolerancePolicy,
) -> Result<Vec<Contour>, GeomError> {
  let mut contours = Vec::new();
  for boundary in &copper.boundaries {
    if boundary.signed_area() >= 0.0 {
      continue;
    }
    let hole = Region::new(vec![boundary.clone()]).map_err(|_| GeomError::EditRefused {
      reason:
        "could not recover an interior isolation path for a hole boundary during hole-collapse recovery",
    })?;
    let hole = match hole.normalize(tolerance) {
      Ok(n) => n.region,
      Err(err) if is_clearance_exhausted(&err) || is_hole_collapse_error(&err) => continue,
      Err(_) => {
        return Err(GeomError::EditRefused {
          reason:
            "could not normalize a hole boundary while recovering interior isolation after hole collapse",
        });
      }
    };
    match hole.offset(distance, tolerance) {
      Ok(offset) => contours.extend(offset.region.boundaries),
      Err(err) if is_clearance_exhausted(&err) || is_hole_collapse_error(&err) => {}
      Err(_) => {
        return Err(GeomError::EditRefused {
          reason:
            "could not compute an interior isolation path for a hole boundary during hole-collapse recovery",
        });
      }
    }
  }
  Ok(contours)
}

fn is_hole_collapse_error(err: &GeomError) -> bool {
  matches!(err, GeomError::EditRefused { reason } if reason.contains("collapsed a hole boundary"))
}

/// Copper region with holes removed (outer boundaries only), for exterior isolation fallback.
fn exteriors_only_region(copper: &Region, tolerance: TolerancePolicy) -> Result<Region, GeomError> {
  let outers: Vec<Contour> = copper
    .boundaries
    .iter()
    .filter(|b| b.signed_area() >= 0.0)
    .cloned()
    .collect();
  if outers.is_empty() {
    return Region::new(Vec::new());
  }
  Ok(Region::new(outers)?.normalize(tolerance)?.region)
}

/// Whether a contour is an interior isolation boundary (around a hole/clearance inside copper).
///
/// After offsetting a copper region outward, hole boundaries remain hole-like in the offset region
/// (material outside them in the local sense, negative signed area under the region's convention).
fn is_interior_boundary(contour: &Contour) -> bool {
  contour.signed_area() < 0.0
}

fn select_boundary_for_kind(contour: &Contour, kind: IsolationKind) -> bool {
  match kind {
    IsolationKind::Full => true,
    IsolationKind::Exterior => !is_interior_boundary(contour),
    IsolationKind::Interior => is_interior_boundary(contour),
  }
}

/// Identify copper pour regions: outer copper boundaries that contain at least one hole boundary.
///
/// A copper area with interior clearances is treated as a pour to preserve when `keep_pours` is
/// enabled, so interior isolation paths inside it are omitted.
fn pour_regions(copper: &Region) -> Vec<Region> {
  let mut pours = Vec::new();
  for boundary in &copper.boundaries {
    if boundary.signed_area() <= 0.0 {
      continue;
    }
    if let Ok(outer) = Region::new(vec![boundary.clone()]) {
      let has_hole_inside = copper.boundaries.iter().any(|other| {
        if other.signed_area() >= 0.0 {
          return false;
        }
        other
          .vertices()
          .first()
          .is_some_and(|v| outer.contains(v.point, TolerancePolicy::uniform(1.0e-9)))
      });
      if has_hole_inside {
        pours.push(outer);
      }
    }
  }
  pours
}

fn contour_inside_any_pour(contour: &Contour, pours: &[Region]) -> bool {
  if pours.is_empty() {
    return false;
  }
  let Some(v) = contour.vertices().first() else {
    return false;
  };
  pours
    .iter()
    .any(|pour| pour.contains(v.point, TolerancePolicy::uniform(1.0e-9)))
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

/// Orient contours for the requested milling direction.
///
/// Conventional milling keeps contour orientation as produced. Climb milling reverses each contour.
fn orient_contours(contours: Vec<Contour>, direction: MillingDirection) -> Vec<Contour> {
  match direction {
    MillingDirection::Conventional => contours,
    MillingDirection::Climb => contours
      .into_iter()
      .map(|contour| contour.reverse())
      .collect(),
  }
}

/// Generate machining moves for isolation passes (E3.6).
///
/// Computes isolation paths for `copper` with the given parameters, verifies the anti-gouge guard
/// against the protected copper region, and emits moves: for each pass contour, rapid to safe Z,
/// plunge to `cut_depth`, feed along the contour, and retract to safe Z.
///
/// `protected_copper` is the copper that must remain (typically the united copper region). The
/// swept cut is approximated as the isolation path contours buffered by `tool_diameter / 2` for the
/// anti-gouge check.
///
/// # Errors
///
/// Returns a geometric/toolpath error if paths cannot be computed, or [`ToolpathError::IsolationGouge`]
/// if the anti-gouge guard detects penetration above [`penetration::PENETRATION_THRESHOLD`].
#[allow(clippy::too_many_arguments)]
pub fn isolation_moves(
  copper: &Region,
  tool_diameter: f64,
  passes: u32,
  overlap: f64,
  direction: MillingDirection,
  kind: IsolationKind,
  keep_pours: bool,
  cut_depth: f64,
  tolerance: TolerancePolicy,
  job: &Job,
  protected_copper: &Region,
) -> Result<Vec<Move>, ToolpathError> {
  if !cut_depth.is_finite() || cut_depth <= 0.0 {
    return Err(ToolpathError::InvalidDepthRange {
      cut_depth,
      start_depth: 0.0,
    });
  }
  let path = isolation_passes_with_kind(
    copper,
    tool_diameter,
    passes,
    overlap,
    direction,
    kind,
    keep_pours,
    tolerance,
  )
  .map_err(|err| ToolpathError::OffsetCollapsed {
    reason: err.to_string(),
  })?;

  // Anti-gouge guard (E3.4): check swept cut (paths buffered by tool radius) against protected copper.
  let tool_radius = tool_diameter / 2.0;
  let swept = swept_cut_region(&path, tool_radius, tolerance).map_err(|err| {
    ToolpathError::OffsetCollapsed {
      reason: format!("anti-gouge guard could not build swept cut: {err}"),
    }
  })?;
  check_isolation_anti_gouge(
    &swept,
    protected_copper,
    tolerance,
    penetration::PENETRATION_THRESHOLD,
  )?;

  let safe_z = Length::from_mm(cam_model::DEFAULT_SAFE_Z_MM);
  let z = -cut_depth;
  let mut moves = Vec::new();
  for pass in &path.passes {
    for contour in &pass.contours {
      let segments = contour.segments();
      let Some(first) = segments.first() else {
        continue;
      };
      let start = first.start_point();
      let start_m = machine_point(job, start);
      moves.push(Move::Rapid {
        to: Point3 {
          x: Length::from_mm(start_m.x),
          y: Length::from_mm(start_m.y),
          z: safe_z,
        },
      });
      moves.push(Move::Feed {
        to: Point3 {
          x: Length::from_mm(start_m.x),
          y: Length::from_mm(start_m.y),
          z: Length::from_mm(z),
        },
      });
      for segment in &segments {
        let to = segment.end_point();
        push_horizontal(&mut moves, job, *segment, segment.start_point(), to, z);
      }
      let end_m = moves.last().map(|m| m.destination()).unwrap_or(Point3 {
        x: Length::from_mm(start_m.x),
        y: Length::from_mm(start_m.y),
        z: Length::from_mm(z),
      });
      moves.push(Move::Retract {
        to: Point3 {
          x: end_m.x,
          y: end_m.y,
          z: safe_z,
        },
      });
    }
  }
  Ok(moves)
}

/// Approximate swept cut region: isolation path contours buffered by tool radius.
///
/// For each path contour, the tool sweeps a ring of width `2 * tool_radius` around the centerline.
/// This is approximated as `offset(contour, -tool_radius) \\ offset(contour, +tool_radius)`.
fn swept_cut_region(
  path: &IsolationPath,
  tool_radius: f64,
  tolerance: TolerancePolicy,
) -> Result<Region, GeomError> {
  let mut swept = Region::new(Vec::new())?;
  for pass in &path.passes {
    for contour in &pass.contours {
      let contour_region = Region::new(vec![contour.clone()])?
        .normalize(tolerance)?
        .region;
      let outer = match contour_region.offset(-tool_radius, tolerance) {
        Ok(o) => o.region,
        Err(err) if is_clearance_exhausted(&err) => continue,
        Err(err) => return Err(err),
      };
      let ring = match contour_region.offset(tool_radius, tolerance) {
        Ok(inner) => match combine(&outer, &inner.region, BooleanOp::Difference, tolerance)? {
          Combined::Empty => outer,
          Combined::Region { region, .. } => region,
        },
        // Inward offset collapsed: the whole outer buffer is swept.
        Err(err) if is_clearance_exhausted(&err) => outer,
        Err(err) => return Err(err),
      };
      swept = match combine(&swept, &ring, BooleanOp::Union, tolerance)? {
        Combined::Empty => Region::new(Vec::new())?,
        Combined::Region { region, .. } => region,
      };
    }
  }
  Ok(swept)
}

/// Anti-gouge guard (E3.4): fail when the swept isolation cut penetrates protected copper.
///
/// `swept_cut` is the tool's swept region (the area removed by the isolation cut, e.g. cut paths
/// buffered by tool radius). `protected_copper` is the copper that must remain. When the penetration
/// ratio `intersection(swept_cut, protected_copper) / area(protected_copper)` exceeds `threshold`,
/// the operation is refused with an actionable diagnosis.
///
/// # Errors
///
/// Returns [`ToolpathError::IsolationGouge`] with the measured ratio, threshold, overlap and
/// protected area when penetration exceeds the threshold, so the caller can report what to change.
/// Returns [`ToolpathError::OffsetCollapsed`] (as a geometric refusal) is not used here; geometric
/// errors from intersection/area computation are mapped into an actionable refusal.
pub fn check_isolation_anti_gouge(
  swept_cut: &Region,
  protected_copper: &Region,
  tolerance: TolerancePolicy,
  threshold: f64,
) -> Result<(), ToolpathError> {
  let protected_area_mm2 = penetration::region_area(protected_copper);
  if protected_area_mm2 <= 0.0 {
    return Ok(());
  }
  let overlap_area_mm2 = penetration::intersection_area(swept_cut, protected_copper, tolerance)
    .map_err(|err| ToolpathError::OffsetCollapsed {
      reason: format!("anti-gouge guard could not measure penetration: {err}"),
    })?;
  let ratio = overlap_area_mm2 / protected_area_mm2;
  if ratio > threshold {
    return Err(ToolpathError::IsolationGouge {
      ratio,
      threshold,
      protected_area_mm2,
      overlap_area_mm2,
    });
  }
  Ok(())
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
