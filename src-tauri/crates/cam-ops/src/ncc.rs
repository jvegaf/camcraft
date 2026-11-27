//! Non-copper clearing (NCC): compute the envelope of non-copper to remove (E4.1) and,
//! in later slices, fill it with toolpaths.
//!
//! The non-copper envelope is the area that must be cleared: the clearing boundary (for example the
//! board outline) inset by `margin`, minus the copper of the layer kept with a clearance of
//! `copper_offset`. Copper contours are united first (E3.1) so overlapping copper becomes one keep-out
//! region; that copper keep-out is then grown outward by `copper_offset` so the envelope stays on the
//! non-copper side of the copper ("to the left of the material", FlatCAM semantics for the cleared
//! side adjacent to copper). Geometry is never tessellated here except where the underlying kernel
//! already requires it.

use cam_geom::boolean::{combine, BooleanOp, Combined};
use cam_geom::region::Region;
use cam_geom::{Bounds, Contour, GeomError, Point2, TolerancePolicy};
use cam_model::Job;
use cam_move::{Move, Point3};
use cam_units::Length;
use rayon::prelude::*;

/// The non-copper envelope: the region to clear (E4.1).
///
/// `boundary` is the outer clearing boundary (typically the board outline as a region). `copper`
/// contours are the copper of the layer; they are united into a single copper region and then grown
/// outward by `copper_offset` millimetres to form the copper keep-out (clearance from copper). The
/// boundary is inset inward by `margin` millimetres to form the clearing limit. The envelope is the
/// clearing limit minus the copper keep-out.
///
/// With `margin == 0.0` and `copper_offset == 0.0` the envelope is the non-copper area inside the
/// boundary: `boundary \ united_copper`.
///
/// # Errors
///
/// Returns an error when `margin` or `copper_offset` is negative or not finite, when the copper
/// cannot be united, when an offset cannot be computed, when the boundary/margin leaves no clearing
/// area, or when the boolean difference cannot be produced.
pub fn non_copper_envelope(
  boundary: &Region,
  copper: &[Contour],
  margin: f64,
  copper_offset: f64,
  tolerance: TolerancePolicy,
) -> Result<Region, GeomError> {
  if !margin.is_finite() || margin < 0.0 {
    return Err(GeomError::EditRefused {
      reason: "a non-copper clear margin must be a finite, non-negative number",
    });
  }
  if !copper_offset.is_finite() || copper_offset < 0.0 {
    return Err(GeomError::EditRefused {
      reason: "a non-copper clear copper offset must be a finite, non-negative number",
    });
  }

  let clearing_limit = if margin == 0.0 {
    boundary.clone()
  } else {
    // Positive distance removes material / shrinks the boundary inward.
    boundary.offset(margin, tolerance)?.region
  };

  let united_copper = crate::unite_copper(copper, tolerance)?;
  if united_copper.boundaries.is_empty() {
    return Ok(clearing_limit);
  }

  let keep_out = if copper_offset == 0.0 {
    united_copper
  } else {
    // Negative distance grows the material outward, expanding copper into the non-copper side
    // (clearance kept around copper, FlatCAM "left of material" for the preserved copper).
    united_copper.offset(-copper_offset, tolerance)?.region
  };

  subtract_region(&clearing_limit, &keep_out, tolerance)
}

fn subtract_region(
  minuend: &Region,
  subtrahend: &Region,
  tolerance: TolerancePolicy,
) -> Result<Region, GeomError> {
  if subtrahend.boundaries.is_empty() {
    return Ok(minuend.clone());
  }
  match combine(minuend, subtrahend, BooleanOp::Difference, tolerance)? {
    Combined::Empty => Region::new(Vec::new()),
    Combined::Region { region, .. } => Ok(region),
  }
}

/// Concentric fill rings over a non-copper envelope (E4.2).
#[derive(Clone, Debug, PartialEq)]
pub struct NccFill {
  /// Concentric offset contours, outermost first.
  pub contours: Vec<Contour>,
  /// True when some ring was rebuilt as straight segments.
  pub flattened: bool,
}

/// Fill a non-copper envelope with concentric offset rings (E4.2, Standard/Rings fill).
///
/// The first ring is inset by `tool_diameter / 2` from the envelope boundary so the tool stays
/// inside the clearable area. Subsequent rings step inward by `tool_diameter * (1.0 - overlap)`.
/// Offsets are computed from the original envelope region, not accumulated from the previous ring,
/// so stepover is exact. Rings that collapse because the area is exhausted are omitted; the first
/// ring must fit.
///
/// Independent island components are filled in parallel (E4.4) and merged in a stable deterministic
/// order, so the output is identical regardless of `RAYON_NUM_THREADS`.
///
/// # Errors
///
/// Returns an error when `tool_diameter` is not positive and finite, when `overlap` is not in
/// `[0, 1)`, or when the envelope cannot be offset for a required ring.
pub fn ncc_fill_rings(
  envelope: &Region,
  tool_diameter: f64,
  overlap: f64,
  tolerance: TolerancePolicy,
) -> Result<NccFill, GeomError> {
  if !tool_diameter.is_finite() || tool_diameter <= 0.0 {
    return Err(GeomError::EditRefused {
      reason: "a non-copper clear fill needs a positive, finite tool diameter",
    });
  }
  if !overlap.is_finite() || !(0.0..1.0).contains(&overlap) {
    return Err(GeomError::EditRefused {
      reason: "a non-copper clear fill overlap must be a finite number in [0, 1)",
    });
  }

  if envelope.boundaries.is_empty() {
    return Ok(NccFill {
      contours: Vec::new(),
      flattened: false,
    });
  }

  let components = island_components(envelope, tolerance);
  let mut filled: Vec<(usize, NccFill)> = components
    .par_iter()
    .enumerate()
    .map(|(idx, component)| {
      fill_component(component, tool_diameter, overlap, tolerance).map(|fill| (idx, fill))
    })
    .collect::<Result<Vec<_>, _>>()?;

  filled.sort_by_key(|(idx, _)| *idx);
  let mut contours = Vec::new();
  let mut flattened = false;
  for (_, fill) in filled {
    flattened |= fill.flattened;
    contours.extend(fill.contours);
  }

  Ok(NccFill {
    contours,
    flattened,
  })
}

fn fill_component(
  envelope: &Region,
  tool_diameter: f64,
  overlap: f64,
  tolerance: TolerancePolicy,
) -> Result<NccFill, GeomError> {
  let inset = tool_diameter / 2.0;
  let stepover = tool_diameter * (1.0 - overlap);
  let mut contours = Vec::new();
  let mut flattened = false;
  let mut index = 0usize;
  loop {
    let distance = inset + index as f64 * stepover;
    let offset = match envelope.offset(distance, tolerance) {
      Ok(offset) => offset,
      Err(err) => {
        if is_clearance_exhausted(&err) {
          break;
        }
        if index == 0 {
          return Err(err);
        }
        // Non-clearance geometry error after progress: keep what we have.
        break;
      }
    };
    if offset.region.boundaries.is_empty() {
      if index == 0 {
        return Ok(NccFill {
          contours: Vec::new(),
          flattened: false,
        });
      }
      break;
    }
    flattened |= offset.flattened;
    contours.extend(offset.region.boundaries);
    index += 1;
  }

  Ok(NccFill {
    contours,
    flattened,
  })
}

/// Split envelope into disjoint island components for parallel fill.
///
/// Outer (even-depth) boundaries form component roots; hole boundaries are assigned to the
/// smallest-depth outer boundary that contains a representative point. Components are returned in
/// stable source order (by first boundary index).
fn island_components(envelope: &Region, tolerance: TolerancePolicy) -> Vec<Region> {
  let n = envelope.boundaries.len();
  if n <= 1 {
    return vec![envelope.clone()];
  }

  let mut depths = vec![0usize; n];
  for i in 0..n {
    let Some(rep) = representative_point(&envelope.boundaries[i]) else {
      continue;
    };
    let mut depth = 0usize;
    for (j, boundary) in envelope.boundaries.iter().enumerate() {
      if i == j {
        continue;
      }
      if point_in_contour(boundary, rep, tolerance) {
        depth += 1;
      }
    }
    depths[i] = depth;
  }

  let mut outers: Vec<usize> = (0..n).filter(|&i| depths[i] % 2 == 0).collect();
  if outers.len() <= 1 {
    return vec![envelope.clone()];
  }
  outers.sort_unstable();

  let mut groups: Vec<Vec<Contour>> = outers
    .iter()
    .map(|&i| vec![envelope.boundaries[i].clone()])
    .collect();
  for i in 0..n {
    if depths[i] % 2 == 0 {
      continue;
    }
    let Some(rep) = representative_point(&envelope.boundaries[i]) else {
      continue;
    };
    let mut owner: Option<usize> = None;
    let mut owner_depth = usize::MAX;
    for (gi, &outer_i) in outers.iter().enumerate() {
      if point_in_contour(&envelope.boundaries[outer_i], rep, tolerance)
        && depths[outer_i] < owner_depth
      {
        owner = Some(gi);
        owner_depth = depths[outer_i];
      }
    }
    if let Some(gi) = owner {
      groups[gi].push(envelope.boundaries[i].clone());
    } else {
      groups[0].push(envelope.boundaries[i].clone());
    }
  }

  let mut components = Vec::with_capacity(groups.len());
  for group in groups {
    if let Ok(region) = Region::new(group) {
      components.push(region);
    }
  }
  if components.is_empty() {
    vec![envelope.clone()]
  } else {
    components
  }
}

fn representative_point(contour: &Contour) -> Option<Point2> {
  contour.vertices().first().map(|v| v.point)
}

fn point_in_contour(contour: &Contour, point: Point2, tolerance: TolerancePolicy) -> bool {
  Region::new(vec![contour.clone()])
    .map(|r| r.contains(point, tolerance))
    .unwrap_or(false)
}

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

/// Swept area of a tool following `contours` (stroke band of `tool_diameter`).
///
/// For each contour, the band is `grow(radius) \ shrink(radius)`; bands are unioned.
pub fn swept_band_region(
  contours: &[Contour],
  tool_diameter: f64,
  tolerance: TolerancePolicy,
) -> Result<Region, GeomError> {
  if !tool_diameter.is_finite() || tool_diameter <= 0.0 {
    return Err(GeomError::EditRefused {
      reason: "a swept band needs a positive, finite tool diameter",
    });
  }
  let radius = tool_diameter / 2.0;
  let mut swept = Region::new(Vec::new())?;
  for contour in contours {
    let c_region = Region::new(vec![contour.clone()])?
      .normalize(tolerance)?
      .region;
    let grown = match c_region.offset(-radius, tolerance) {
      Ok(o) => o.region,
      Err(err) if is_clearance_exhausted(&err) => continue,
      Err(err) => return Err(err),
    };
    let band = match c_region.offset(radius, tolerance) {
      Ok(inner) => match combine(&grown, &inner.region, BooleanOp::Difference, tolerance)? {
        Combined::Empty => grown,
        Combined::Region { region, .. } => region,
      },
      Err(err) if is_clearance_exhausted(&err) => grown,
      Err(err) => return Err(err),
    };
    swept = match combine(&swept, &band, BooleanOp::Union, tolerance)? {
      Combined::Empty => Region::new(Vec::new())?,
      Combined::Region { region, .. } => region,
    };
  }
  Ok(swept)
}

/// Remaining non-copper region after a coarse tool pass (E4.5 rest machining).
///
/// Subtracts the swept band of `coarse_contours` (diameter `coarse_diameter`) from `envelope`.
/// The result is the region a finer tool should clear, avoiding redundant recuts.
pub fn ncc_rest_region(
  envelope: &Region,
  coarse_contours: &[Contour],
  coarse_diameter: f64,
  tolerance: TolerancePolicy,
) -> Result<Region, GeomError> {
  let swept = swept_band_region(coarse_contours, coarse_diameter, tolerance)?;
  subtract_region(envelope, &swept, tolerance)
}

/// Fill a non-copper envelope with parallel scanlines (E4.6, Lines method).
///
/// Lines are spaced by `tool_diameter * (1.0 - overlap)` and clipped to the envelope via
/// `Region::raster_spans`. Each span becomes an open 2-point contour.
pub fn ncc_fill_lines(
  envelope: &Region,
  tool_diameter: f64,
  overlap: f64,
  tolerance: TolerancePolicy,
) -> Result<NccFill, GeomError> {
  if !tool_diameter.is_finite() || tool_diameter <= 0.0 {
    return Err(GeomError::EditRefused {
      reason: "a non-copper clear fill needs a positive, finite tool diameter",
    });
  }
  if !overlap.is_finite() || !(0.0..1.0).contains(&overlap) {
    return Err(GeomError::EditRefused {
      reason: "a non-copper clear fill overlap must be a finite number in [0, 1)",
    });
  }
  if envelope.boundaries.is_empty() {
    return Ok(NccFill {
      contours: Vec::new(),
      flattened: false,
    });
  }
  let Some(bounds) = region_bounds(envelope) else {
    return Ok(NccFill {
      contours: Vec::new(),
      flattened: false,
    });
  };
  let stepover = tool_diameter * (1.0 - overlap);
  let inset = tool_diameter / 2.0;
  let y_start = bounds.min.y + inset;
  let y_end = bounds.max.y - inset;
  let mut contours = Vec::new();
  let mut y = y_start;
  let mut row = 0usize;
  while y <= y_end + tolerance.epsilon {
    if let Some(spans) = envelope.raster_spans(y, tolerance) {
      let iter: Box<dyn Iterator<Item = (f64, f64)>> = if row % 2 == 0 {
        Box::new(spans.into_iter())
      } else {
        Box::new(spans.into_iter().rev())
      };
      for (left, right) in iter {
        let l = left + inset;
        let r = right - inset;
        if r - l <= tolerance.epsilon {
          continue;
        }
        let (p1, p2) = if row % 2 == 0 {
          (Point2::new(l, y), Point2::new(r, y))
        } else {
          (Point2::new(r, y), Point2::new(l, y))
        };
        if let Ok(c) = Contour::from_points(&[p1, p2], false) {
          contours.push(c);
        }
      }
    }
    y += stepover;
    row += 1;
  }
  Ok(NccFill {
    contours,
    flattened: false,
  })
}

/// Fill a non-copper envelope with seed (expanding circles from a seed point) (E4.6).
///
/// Starts from a representative point inside the envelope and grows circular offsets by
/// `tool_diameter * (1.0 - overlap)`, intersecting each ring with the envelope.
pub fn ncc_fill_seed(
  envelope: &Region,
  tool_diameter: f64,
  overlap: f64,
  tolerance: TolerancePolicy,
) -> Result<NccFill, GeomError> {
  if !tool_diameter.is_finite() || tool_diameter <= 0.0 {
    return Err(GeomError::EditRefused {
      reason: "a non-copper clear fill needs a positive, finite tool diameter",
    });
  }
  if !overlap.is_finite() || !(0.0..1.0).contains(&overlap) {
    return Err(GeomError::EditRefused {
      reason: "a non-copper clear fill overlap must be a finite number in [0, 1)",
    });
  }
  if envelope.boundaries.is_empty() {
    return Ok(NccFill {
      contours: Vec::new(),
      flattened: false,
    });
  }

  let Some(seed) = find_seed_point(envelope, tolerance) else {
    return Ok(NccFill {
      contours: Vec::new(),
      flattened: false,
    });
  };
  let Some(bounds) = region_bounds(envelope) else {
    return Ok(NccFill {
      contours: Vec::new(),
      flattened: false,
    });
  };
  let max_r = ((bounds.max.x - bounds.min.x).powi(2) + (bounds.max.y - bounds.min.y).powi(2))
    .sqrt()
    + tool_diameter;
  let stepover = tool_diameter * (1.0 - overlap);
  let mut radius = tool_diameter / 2.0;
  let mut contours = Vec::new();
  let mut flattened = false;

  while radius <= max_r {
    let circle = match cam_geom::primitive::circle(seed, radius) {
      Ok(c) => c,
      Err(_) => {
        radius += stepover;
        continue;
      }
    };
    let circle_region =
      match Region::new(vec![circle]).and_then(|r| r.normalize(tolerance).map(|n| n.region)) {
        Ok(r) => r,
        Err(_) => {
          radius += stepover;
          continue;
        }
      };
    match combine(&circle_region, envelope, BooleanOp::Intersection, tolerance)? {
      Combined::Empty => {}
      Combined::Region {
        region,
        flattened: f,
        ..
      } => {
        flattened |= f;
        contours.extend(region.boundaries);
      }
    }
    radius += stepover;
  }

  Ok(NccFill {
    contours,
    flattened,
  })
}

fn region_bounds(region: &Region) -> Option<Bounds> {
  let mut iter = region.boundaries.iter();
  let first = iter.next()?;
  let mut b = first.bounds();
  for boundary in iter {
    let bb = boundary.bounds();
    b.min.x = b.min.x.min(bb.min.x);
    b.min.y = b.min.y.min(bb.min.y);
    b.max.x = b.max.x.max(bb.max.x);
    b.max.y = b.max.y.max(bb.max.y);
  }
  Some(b)
}
fn find_seed_point(envelope: &Region, tolerance: TolerancePolicy) -> Option<Point2> {
  let bounds = region_bounds(envelope)?;
  let cx = (bounds.min.x + bounds.max.x) * 0.5;
  let cy = (bounds.min.y + bounds.max.y) * 0.5;
  let center = Point2::new(cx, cy);
  if envelope.contains(center, tolerance) {
    return Some(center);
  }
  // Try representative points from boundary vertices nudged inward.
  for b in &envelope.boundaries {
    let verts: Vec<_> = b.vertices().iter().map(|v| v.point).collect();
    if verts.is_empty() {
      continue;
    }
    let mx = verts.iter().map(|p| p.x).sum::<f64>() / verts.len() as f64;
    let my = verts.iter().map(|p| p.y).sum::<f64>() / verts.len() as f64;
    for (px, py) in [(mx, my)].into_iter().chain(
      verts
        .iter()
        .take(20)
        .map(|v| (v.x * 0.8 + mx * 0.2, v.y * 0.8 + my * 0.2)),
    ) {
      let p = Point2::new(px, py);
      if envelope.contains(p, tolerance) {
        return Some(p);
      }
    }
  }
  None
}

/// Convert NCC fill contours to cut moves (E4.8/T8 integration helper).
///
/// Each contour is cut at `z = -cut_depth` with rapid in, plunge, contour feed, retract to safe Z.
/// Open contours (e.g. Lines fill) are traversed start-to-end without closing.
pub fn ncc_moves(contours: &[Contour], cut_depth: f64, job: &Job) -> Result<Vec<Move>, GeomError> {
  if !cut_depth.is_finite() || cut_depth <= 0.0 {
    return Err(GeomError::EditRefused {
      reason: "a non-copper clear cut depth must be a positive, finite number",
    });
  }
  let safe_z = Length::from_mm(cam_model::DEFAULT_SAFE_Z_MM);
  let z = -cut_depth;
  let mut moves = Vec::new();
  for contour in contours {
    let segments = contour.segments();
    let Some(first) = segments.first() else {
      continue;
    };
    let start = first.start_point();
    let start_m = crate::shared::machine_point(job, start);
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
      crate::shared::push_horizontal(&mut moves, job, *segment, segment.start_point(), to, z);
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
  Ok(moves)
}
