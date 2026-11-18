//! The arrangement behind [`crate::region::Region::normalize`].
//!
//! Normalization is what makes a boundary list honest again: the boundaries are split at every
//! transversal crossing, the pieces are classified by the parity of the material beside them, and the
//! survivors are chained back into closed contours that no longer cross.
//!
//! # Why the walk is not [`crate::chain`]
//!
//! [`crate::chain`] matches one piece's end to the next piece's start within a tolerance and takes the
//! first piece that matches. That is right for loose segments and wrong here: every crossing produces
//! a point where four pieces meet, and the greedy match happily joins a piece to its own boundary's
//! continuation, reproducing the input — a region *with* crossings, which is the one thing
//! normalization exists to remove. The survivors are therefore chained by walking the arrangement:
//! each piece is oriented with the material on its left, and at a shared point the walk continues with
//! the piece that comes next clockwise, which keeps the material on that side and turns at the
//! crossing instead of passing through it.
//!
//! # What is kept exact
//!
//! A piece of an arc is an arc (A8): splitting only changes an arc's start angle and sweep, never its
//! centre or radius, and a reversal only negates the sweep. Flattening happens in exactly one place —
//! a piece whose arc the contour model's bulge cannot hold — and it is reported, never hidden.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::f64::consts::TAU;

use crate::arc::Arc;
use crate::contour::Contour;
use crate::intersect::{intersect, Intersections};
use crate::point::{Bounds, Point2, Vec2};
use crate::region::{Normalized, Region};
use crate::segment::{LineSeg, Segment};
use crate::winding::{classify, winding_number, Membership};
use crate::GeomError;

/// How close to a segment's own end an intersection may sit before it stops counting as a split.
const FRACTION_EPSILON: f64 = 1.0e-9;

/// How far a bend shifts its end's direction, in radians. Small enough to leave genuine angular
/// differences alone, large enough to separate the two ends of a tangency, whose directions agree
/// exactly and would otherwise be ordered by nothing but their index.
const ANGLE_TIE_BREAK: f64 = 1.0e-12;

/// The first distance a side probe steps away from a piece, as a fraction of the piece's length.
const PROBE_STEP: f64 = 1.0e-5;

/// The smallest distance a side probe may step away from a piece, in millimetres.
const PROBE_FLOOR: f64 = 1.0e-9;

/// How much farther each retry steps, when the first probe landed inside some boundary's band.
const PROBE_GROWTH: f64 = 16.0;

/// How many probe distances are tried before the arrangement is declared unresolvable.
const PROBE_TRIES: usize = 5;

/// A piece of a boundary, before it is classified.
#[derive(Clone, Copy, Debug)]
struct Cut {
  /// Index of the boundary the piece came from, in the region's own order.
  path: usize,
  /// The piece itself.
  segment: Segment,
}

/// Which side of a piece holds material.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
  /// Material on the piece's left.
  Left,
  /// Material on the piece's right.
  Right,
  /// The same answer on both sides: the piece is an interior seam, not a boundary.
  Both,
  /// Nothing on either side: the piece bounds a slot of no material.
  Neither,
}

/// Which end of a piece an end refers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EndAt {
  /// The piece's start.
  Start,
  /// The piece's end.
  End,
}

impl EndAt {
  /// A tie-break rank, so equal directions still sort deterministically.
  fn rank(self) -> u8 {
    match self {
      Self::Start => 0,
      Self::End => 1,
    }
  }
}

/// A vertex of the arrangement: a point, and the piece ends that meet there.
struct Vertex {
  point: Point2,
  ends: Vec<(usize, EndAt)>,
}

fn is_unchainable_error(err: &GeomError) -> bool {
  matches!(
    err,
    GeomError::EditRefused { reason }
      if *reason == "the arrangement revisits a piece, so its pieces cannot be chained closed"
        || *reason == "the arrangement does not close, so its pieces cannot be chained closed"
  )
}

fn odd_degree_points(pieces: &[Segment], tolerance: crate::TolerancePolicy) -> Vec<Point2> {
  let mut vertices: Vec<(Point2, usize)> = Vec::new();
  for piece in pieces {
    for point in [piece.start_point(), piece.end_point()] {
      if let Some(idx) = vertices
        .iter()
        .position(|(vertex, _)| vertex.is_near(point, tolerance))
      {
        vertices[idx].1 += 1;
      } else {
        vertices.push((point, 1));
      }
    }
  }
  vertices
    .into_iter()
    .filter(|(_, degree)| degree % 2 == 1)
    .map(|(point, _)| point)
    .collect()
}

fn snap_relevant_to_odd_degree(
  snap: (Point2, Point2),
  odd_points: &[Point2],
  tolerance: crate::TolerancePolicy,
) -> bool {
  let (endpoint, projection) = snap;
  odd_points.iter().any(|point| {
    endpoint.is_near(*point, tolerance)
      || projection.is_near(*point, tolerance)
      || endpoint == *point
      || projection == *point
  })
}

fn normalize_with(
  region: &Region,
  tolerance: crate::TolerancePolicy,
  detect_near_misses: bool,
) -> Result<Normalized, GeomError> {
  let mut cuts: Vec<Cut> = Vec::new();
  let mut re_cut = false;
  let mut near_miss_snaps: Vec<(Point2, Point2)> = Vec::new();
  for (path, boundary) in region.boundaries.iter().enumerate() {
    for (index, segment) in boundary.segments().into_iter().enumerate() {
      let fractions = cut_fractions(
        segment,
        region,
        path,
        index,
        tolerance,
        &mut re_cut,
        &mut near_miss_snaps,
        detect_near_misses,
      )?;
      for piece in split_at(segment, &fractions)? {
        cuts.push(Cut {
          path,
          segment: piece,
        });
      }
    }
  }

  // Nothing crossed, so nothing needs re-making: the input comes back in its own vertex order and
  // arc-exact (A4). Near-miss detection may still be needed even without geometric crossings.
  if !re_cut {
    return Ok(Normalized {
      region: region.clone(),
      flattened: false,
    });
  }

  let mut survivors: Vec<Segment> = Vec::with_capacity(cuts.len());
  for cut in cuts {
    match material_side(cut.segment, cut.path, region, tolerance, &near_miss_snaps)? {
      Side::Left => survivors.push(cut.segment),
      Side::Right => survivors.push(cut.segment.reversed()),
      Side::Both | Side::Neither => {}
    }
  }
  if survivors.is_empty() {
    return Err(GeomError::EditRefused {
      reason: "normalization leaves no material: the boundaries enclose none",
    });
  }

  // On near-miss retry, keep only snaps relevant to vertices that remain odd-degree without snaps,
  // so unrelated legitimate gaps are not welded. No odd-degree vertices means retain none.
  if detect_near_misses && !near_miss_snaps.is_empty() {
    let odd_points = odd_degree_points(&survivors, tolerance);
    if odd_points.is_empty() {
      near_miss_snaps.clear();
    } else {
      near_miss_snaps.retain(|snap| snap_relevant_to_odd_degree(*snap, &odd_points, tolerance));
    }
  }

  // A normalization walks one region, so every survivor belongs to the same side of the arrangement.
  let owners = vec![true; survivors.len()];
  let mut flattened = false;
  let valid_snaps = filter_valid_snaps(&survivors, &near_miss_snaps);
  let boundaries = assemble(&survivors, &owners, tolerance, &mut flattened, &valid_snaps)?;
  Ok(Normalized {
    region: Region { boundaries },
    flattened,
  })
}

/// Split every boundary at its crossings, keep the pieces that border material and chain them closed.
pub fn normalize(
  region: &Region,
  tolerance: crate::TolerancePolicy,
) -> Result<Normalized, GeomError> {
  if !tolerance.epsilon.is_finite() || tolerance.epsilon <= 0.0 {
    return Err(GeomError::EditRefused {
      reason: "normalization needs a positive, finite tolerance",
    });
  }

  match normalize_with(region, tolerance, false) {
    Ok(result) => Ok(result),
    Err(err) if is_unchainable_error(&err) => normalize_with(region, tolerance, true),
    Err(err) => Err(err),
  }
}

/// The fractional positions at which `segment` must be split, including both of its ends.
#[allow(clippy::too_many_arguments)]
fn cut_fractions(
  segment: Segment,
  region: &Region,
  skip_path: usize,
  skip_index: usize,
  tolerance: crate::TolerancePolicy,
  re_cut: &mut bool,
  near_miss_snaps: &mut Vec<(Point2, Point2)>,
  detect_near_misses: bool,
) -> Result<Vec<f64>, GeomError> {
  let mut fractions = vec![0.0_f64, 1.0];
  // A boundary is cut against itself as well. A contour that crosses itself is two loops wearing one
  // contour, and the parity rule cannot classify a piece until that crossing is a vertex; passing it
  // through as it arrived leaves a boundary whose signed area and whose material disagree. Including
  // the piece's own segments costs nothing: a segment meets itself only along its whole length, and a
  // meeting at an end is not a split.
  for (other_path, boundary) in region.boundaries.iter().enumerate() {
    // The piece itself is excluded *by identity*: compared with itself a segment is `Coincident`,
    // which would mark every region as crossed. Excluding by equality instead would also drop a
    // genuine doubled edge — the other contour's segments carry the same coordinates — and then an
    // untouched input would come back with both copies still in it.
    let others = boundary
      .segments()
      .into_iter()
      .enumerate()
      .filter(|(index, _)| !(other_path == skip_path && *index == skip_index))
      .map(|(_, segment)| segment);
    push_fractions(
      &mut fractions,
      segment,
      others,
      tolerance,
      re_cut,
      near_miss_snaps,
      detect_near_misses,
    );
  }
  Ok(dedup(&mut fractions, segment, tolerance))
}

/// Add the fractions where `segment` meets any of `others`.
///
/// Split out of [`cut_fractions`] so a set operation cuts one region's segments against another
/// region's boundaries as well as against its own, under the same crossing rules.
fn push_fractions(
  fractions: &mut Vec<f64>,
  segment: Segment,
  others: impl IntoIterator<Item = Segment>,
  tolerance: crate::TolerancePolicy,
  re_cut: &mut bool,
  near_miss_snaps: &mut Vec<(Point2, Point2)>,
  detect_near_misses: bool,
) {
  for other in others {
    {
      match intersect(segment, other, tolerance) {
        Intersections::None => {}
        // A tangent touch does not cross, but it is still a contact point in the arrangement: when two
        // boundaries meet without crossing (e.g. a disc tangent inside a rectangle edge), the shared
        // point must become a vertex so the pieces can be chained and classified correctly.
        Intersections::Tangent(point) => {
          if let Some(fraction) = interior_fraction(segment, point, tolerance) {
            *re_cut = true;
            fractions.push(fraction);
          }
        }
        // Two boundaries that overlap along a length are a doubled edge: neither the overlap's ends
        // nor its interior is an intersection *point*, so the split points are the other boundary's
        // own ends where they land inside this segment.
        Intersections::Coincident => {
          *re_cut = true;
          for point in [other.start_point(), other.end_point()] {
            if let Some(fraction) = interior_fraction(segment, point, tolerance) {
              fractions.push(fraction);
            }
          }
        }
        Intersections::Point(point) => {
          if let Some(fraction) = interior_fraction(segment, point, tolerance) {
            *re_cut = true;
            fractions.push(fraction);
          }
        }
        Intersections::Points(first, second) => {
          for point in [first, second] {
            if let Some(fraction) = interior_fraction(segment, point, tolerance) {
              *re_cut = true;
              fractions.push(fraction);
            }
          }
        }
      }
    }
    if !detect_near_misses {
      continue;
    }
    // An endpoint of the other boundary that passes very close to this segment, but not within
    // `tolerance`, is a near-miss junction: the arrangement has no vertex there, yet the pieces
    // around it cannot be chained consistently. Split at the projection and record the snap so the
    // endpoint and the projection become one vertex. Near-miss detection is only enabled when the
    // arrangement is otherwise unchainable, so legitimate gaps in well-formed arrangements are
    // never snapped.
    let endpoints = [
      (other.start_point(), other.end_point()),
      (other.end_point(), other.start_point()),
    ];
    for (endpoint, other_end) in endpoints {
      let projection_distance = match fraction_along(segment, endpoint) {
        Some(f) => endpoint.distance_to(point_at(segment, f)),
        None => continue,
      };
      let own_len = endpoint.distance_to(other_end);
      // Endpoint-to-endpoint near-miss: the endpoint is close to an endpoint of `segment`
      // (beyond epsilon but within a small multiple), forming a tip-to-tip junction that
      // vertex welding missed. Record a direct snap between the two endpoints.
      let eps = tolerance.epsilon;
      if projection_distance > eps && projection_distance <= eps * 2.0 {
        for seg_end in [segment.start_point(), segment.end_point()] {
          let d = endpoint.distance_to(seg_end);
          if d > eps && d <= eps * 2.0 {
            near_miss_snaps.push((endpoint, seg_end));
          }
        }
      }
      // A near-miss junction arises from a micro-edge perturbation: the endpoint's own adjacent
      // segment must be short relative to the projection gap. Normal vertices of separate closed
      // contours have adjacent edges much longer than a small gap and must not be snapped.
      if own_len > projection_distance * 10.0 {
        continue;
      }
      // If the endpoint is much closer to its own neighbor than to the projection, snapping would
      // collapse that segment rather than resolve a near-miss junction.
      if own_len * 10.0 < projection_distance {
        continue;
      }
      // Reject near-miss if the endpoint's segment (other) and the target segment are roughly
      // parallel: that indicates separate parallel contours with a consistent gap, not a genuine
      // near-miss junction. The original micro-edge case has a vertical micro-edge meeting a
      // horizontal edge (perpendicular), so it passes this check.
      let other_dir = other_end - endpoint;
      let segment_dir = segment.end_point() - segment.start_point();
      if other_dir.length_squared() > 0.0 && segment_dir.length_squared() > 0.0 {
        let dot = other_dir.dot(segment_dir);
        let other_len = other_dir.length();
        let segment_len = segment_dir.length();
        let cos_angle = (dot / (other_len * segment_len)).abs();
        if cos_angle > 0.9998 {
          // ~1 degree threshold
          continue;
        }
      }
      if let Some((fraction, projection)) = near_miss_fraction(segment, endpoint, tolerance) {
        *re_cut = true;
        fractions.push(fraction);
        near_miss_snaps.push((endpoint, projection));
      }
    }
  }
}

/// The projection of `point` onto `segment` as a near-miss junction, if it is one.
///
/// A near-miss is an endpoint that lies outside `tolerance` of the segment yet projects into its
/// interior, closer to that projection than the projection is to either end of the segment and not
/// within `tolerance` of the segment. That means the endpoint and the segment are trying to meet in
/// the middle, and the arrangement must connect them. Points already within `tolerance` are not
/// near-misses: the intersection logic and vertex welding handle them. The caller must also reject
/// snaps that would collapse the endpoint's own adjacent segments.
fn near_miss_fraction(
  segment: Segment,
  point: Point2,
  tolerance: crate::TolerancePolicy,
) -> Option<(f64, Point2)> {
  let fraction = fraction_along(segment, point)?;
  if fraction <= FRACTION_EPSILON || fraction >= 1.0 - FRACTION_EPSILON {
    return None;
  }
  let projection = point_at(segment, fraction);
  let distance = point.distance_to(projection);
  let eps = tolerance.epsilon;
  if distance <= eps {
    return None;
  }
  // Absolute cap: avoid welding legitimate clearances far above tolerance on long edges.
  if distance > eps * 1.0e4 {
    return None;
  }
  if projection.is_near(segment.start_point(), tolerance)
    || projection.is_near(segment.end_point(), tolerance)
  {
    return None;
  }
  // Much closer to the projection than the projection is to either end of this segment.
  // A true near-miss junction in the interior, not a distant point whose projection happens to
  // fall inside.
  let to_start = projection.distance_to(segment.start_point());
  let to_end = projection.distance_to(segment.end_point());
  if distance < to_start * 1.0e-3 && distance < to_end * 1.0e-3 {
    Some((fraction, projection))
  } else {
    None
  }
}

/// Where along `segment` a point sits, if it sits *on* it and away from both of its ends.
///
/// A meeting point at an end is not a split: the contour already changes vertex there, and cutting it
/// would leave a zero-length piece at the end of every shared corner.
fn interior_fraction(
  segment: Segment,
  point: Point2,
  tolerance: crate::TolerancePolicy,
) -> Option<f64> {
  let fraction = fraction_along(segment, point)?;
  if !(0.0..=1.0).contains(&fraction) {
    return None;
  }
  if fraction <= FRACTION_EPSILON || fraction >= 1.0 - FRACTION_EPSILON {
    return None;
  }
  let at = point_at(segment, fraction);
  if point.distance_to(at) > tolerance.epsilon {
    return None;
  }
  if point.is_near(segment.start_point(), tolerance)
    || point.is_near(segment.end_point(), tolerance)
  {
    return None;
  }
  // Avoid creating sliver pieces shorter than the tolerance can reliably classify.
  if at.distance_to(segment.start_point()) <= tolerance.epsilon * 3.0
    || at.distance_to(segment.end_point()) <= tolerance.epsilon * 3.0
  {
    return None;
  }
  Some(fraction)
}

/// Sort the cut fractions and drop the ones that are the same place within `tolerance`.
fn dedup(fractions: &mut [f64], segment: Segment, tolerance: crate::TolerancePolicy) -> Vec<f64> {
  fractions.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
  let mut kept: Vec<f64> = Vec::with_capacity(fractions.len());
  for fraction in fractions.iter().copied() {
    let repeated = kept.last().is_some_and(|previous: &f64| {
      point_at(segment, *previous).is_near(point_at(segment, fraction), tolerance)
    });
    if !repeated {
      kept.push(fraction);
    }
  }
  kept
}

/// The pieces of `segment` between consecutive cut fractions.
fn split_at(segment: Segment, fractions: &[f64]) -> Result<Vec<Segment>, GeomError> {
  let mut pieces = Vec::with_capacity(fractions.len().saturating_sub(1));
  for window in fractions.windows(2) {
    let (from, to) = (window[0], window[1]);
    if to <= from {
      continue;
    }
    pieces.push(sub_segment(segment, from, to)?);
  }
  Ok(pieces)
}

/// The part of `segment` between two fractional positions, as the same kind of segment.
fn sub_segment(segment: Segment, from: f64, to: f64) -> Result<Segment, GeomError> {
  match segment {
    Segment::Line(line) => Ok(Segment::Line(LineSeg::new(
      line.point_at(from),
      line.point_at(to),
    ))),
    Segment::Arc(arc) => Arc::new(
      arc.center(),
      arc.radius(),
      arc.start_angle() + arc.sweep() * from,
      arc.sweep() * (to - from),
    )
    .map(Segment::Arc)
    .map_err(|_| GeomError::EditRefused {
      reason: "a boundary could not be split at an intersection",
    }),
  }
}

/// Where along a segment a point projects, as a fraction of its length or sweep.
fn fraction_along(segment: Segment, point: Point2) -> Option<f64> {
  match segment {
    Segment::Line(line) => {
      let span = line.end - line.start;
      let length_squared = span.length_squared();
      if length_squared <= 0.0 {
        None
      } else {
        Some((point - line.start).dot(span) / length_squared)
      }
    }
    Segment::Arc(arc) => {
      let sweep = arc.sweep();
      if sweep == 0.0 {
        return None;
      }
      let angle = (point - arc.center()).angle();
      let travelled = if sweep >= 0.0 {
        (angle - arc.start_angle()).rem_euclid(TAU)
      } else {
        (arc.start_angle() - angle).rem_euclid(TAU)
      };
      Some(travelled / sweep.abs())
    }
  }
}

/// The point at a fractional position along a segment.
fn point_at(segment: Segment, fraction: f64) -> Point2 {
  match segment {
    Segment::Line(line) => line.point_at(fraction),
    Segment::Arc(arc) => arc.point_at_fraction(fraction),
  }
}

/// Which side of a piece holds material, decided by probing a short distance to each side.
fn material_side(
  segment: Segment,
  path: usize,
  region: &Region,
  tolerance: crate::TolerancePolicy,
  near_miss_snaps: &[(Point2, Point2)],
) -> Result<Side, GeomError> {
  let length = segment.length();
  if length <= 0.0 {
    return Err(GeomError::EditRefused {
      reason: "a zero-length piece cannot be classified",
    });
  }
  let base = (length * PROBE_STEP).max(PROBE_FLOOR);
  let mut samples: Vec<(Side, usize)> = Vec::with_capacity(PROBE_TRIES);
  for attempt in 0..PROBE_TRIES {
    let distance = base * PROBE_GROWTH.powi(attempt as i32);
    if distance * 2.0 > length {
      break;
    }
    let Some((left_point, right_point)) = probe_points(segment, distance) else {
      break;
    };
    let left = material_beside(region, path, left_point, tolerance);
    let right = material_beside(region, path, right_point, tolerance);
    let side = match (left, right) {
      (Some(true), Some(false)) => Some(Side::Left),
      (Some(false), Some(true)) => Some(Side::Right),
      (Some(true), Some(true)) => Some(Side::Both),
      (Some(false), Some(false)) => Some(Side::Neither),
      _ => None,
    };
    if let Some(side) = side {
      samples.push((side, attempt));
    }
  }
  let first_definitive = if samples.is_empty() {
    None
  } else {
    let counts_for = |samples: &[(Side, usize)]| {
      let mut c = [0_u32; 4];
      for (s, _) in samples {
        let idx = match s {
          Side::Left => 0,
          Side::Right => 1,
          Side::Both => 2,
          Side::Neither => 3,
        };
        c[idx] += 1;
      }
      c
    };
    let total = samples.len();
    let filtered: Vec<(Side, usize)> = if total <= 2 {
      samples.clone()
    } else {
      let counts = counts_for(&samples);
      let kept: Vec<(Side, usize)> = samples
        .iter()
        .copied()
        .filter(|(s, _)| {
          let idx = match s {
            Side::Left => 0,
            Side::Right => 1,
            Side::Both => 2,
            Side::Neither => 3,
          };
          counts[idx] > 1
        })
        .collect();
      if kept.is_empty() {
        samples.clone()
      } else {
        kept
      }
    };
    let counts = counts_for(&filtered);
    let mut best = filtered[0].0;
    let mut best_count = 0_u32;
    let mut best_attempt = usize::MAX;
    for (s, attempt) in &filtered {
      let idx = match s {
        Side::Left => 0,
        Side::Right => 1,
        Side::Both => 2,
        Side::Neither => 3,
      };
      let count = counts[idx];
      let priority = |s: Side| match s {
        // On a genuine tie prefer a boundary classification over dropping a piece.
        Side::Left | Side::Right => 1_u8,
        Side::Both | Side::Neither => 0,
      };
      let better = if count != best_count {
        count > best_count
      } else if priority(*s) != priority(best) {
        priority(*s) > priority(best)
      } else {
        *attempt < best_attempt
      };
      if better {
        best = *s;
        best_count = count;
        best_attempt = *attempt;
      }
    }
    Some(best)
  };
  if let Some(side) = first_definitive {
    // A boundary at normal scale may straddle a thin sliver created by a near-miss junction;
    // a close probe detects it (both sides the same locally). Only run this guard on pieces
    // adjacent to a near-miss snap (one endpoint matches a snap endpoint or projection).
    let is_adjacent_to_snap = near_miss_snaps.iter().any(|(endpoint, projection)| {
      segment.start_point() == *endpoint
        || segment.end_point() == *endpoint
        || segment.start_point() == *projection
        || segment.end_point() == *projection
    });
    if is_adjacent_to_snap && matches!(side, Side::Left | Side::Right) {
      // Probe close to detect a thin sliver. Try smaller distances if a close probe suggests a
      // sliver, to avoid misclassifying a narrow valid feature that the probe stepped past.
      let mut probe = PROBE_FLOOR * PROBE_GROWTH.powi(2);
      let min_probe = (PROBE_FLOOR * 2.0).max(tolerance.epsilon);
      let mut sliver: Option<bool> = None;
      while probe * 2.0 <= length && probe >= min_probe {
        if let Some((left_point, right_point)) = probe_points(segment, probe) {
          let left = material_beside(region, path, left_point, tolerance);
          let right = material_beside(region, path, right_point, tolerance);
          match (left, right) {
            (Some(l), Some(r)) if l == r => {
              sliver = Some(l);
              probe /= PROBE_GROWTH;
              continue;
            }
            (Some(l), Some(r)) if l != r => {
              sliver = None;
              break;
            }
            _ => {
              sliver = None;
              break;
            }
          }
        } else {
          sliver = None;
          break;
        }
      }
      if let Some(l) = sliver {
        return Ok(if l { Side::Both } else { Side::Neither });
      }
    }
    return Ok(side);
  }
  Err(GeomError::EditRefused {
    reason: "the arrangement is finer than the tolerance can resolve",
  })
}

/// Two probe points a short distance either side of a piece's midpoint: left first, right second.
fn probe_points(segment: Segment, distance: f64) -> Option<(Point2, Point2)> {
  match segment {
    Segment::Line(line) => {
      let direction = line.direction()?;
      let midpoint = line.point_at(0.5);
      let normal = direction.perp_ccw();
      Some((midpoint + normal * distance, midpoint - normal * distance))
    }
    Segment::Arc(arc) => {
      let midpoint = arc.mid_point();
      let outward = (midpoint - arc.center()).normalized()?;
      // Travelling counter-clockwise around a circle the left-hand side is the inward one.
      let inward = midpoint - outward * distance;
      let beyond = midpoint + outward * distance;
      if arc.is_ccw() {
        Some((inward, beyond))
      } else {
        Some((beyond, inward))
      }
    }
  }
}

/// Whether the point is material, counting every boundary except `path`.
///
/// The piece's own boundary cannot be classified by distance at a probe deliberately close to it, so
/// its contribution is the winding number there: inside or outside *that* boundary, which is the
/// parity of one boundary and does not depend on how it happens to be oriented.
fn material_beside(
  region: &Region,
  path: usize,
  point: Point2,
  tolerance: crate::TolerancePolicy,
) -> Option<bool> {
  enclosed_by(region, Some(path), point, tolerance)
}

/// Whether `point` lies in `region`'s material, skipping one boundary the caller already owns.
///
/// Membership is parity across the boundaries, never orientation. `None` means a probe landed within
/// some boundary's tolerance band, so the question cannot be answered at that distance and the caller
/// must step farther out.
fn enclosed_by(
  region: &Region,
  skip: Option<usize>,
  point: Point2,
  tolerance: crate::TolerancePolicy,
) -> Option<bool> {
  let mut enclosed = 0_usize;
  for (index, boundary) in region.boundaries.iter().enumerate() {
    if Some(index) == skip {
      if winding_number(boundary, point).rem_euclid(2) == 1 {
        enclosed += 1;
      }
      continue;
    }
    match classify(boundary, point, tolerance) {
      Membership::OnBoundary => return None,
      Membership::Inside => enclosed += 1,
      Membership::Outside => {}
    }
  }
  Some(enclosed % 2 == 1)
}

/// Chain the survivors into closed contours by walking the arrangement.
fn assemble(
  pieces: &[Segment],
  owners: &[bool],
  tolerance: crate::TolerancePolicy,
  flattened: &mut bool,
  near_miss_snaps: &[(Point2, Point2)],
) -> Result<Vec<Contour>, GeomError> {
  let allowed: Vec<Point2> = pieces
    .iter()
    .flat_map(|piece| [piece.start_point(), piece.end_point()])
    .collect();
  let mut vertices: Vec<Vertex> = Vec::new();
  let mut vertex_grid: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
  let cell = tolerance.epsilon.max(1.0e-12);
  for (index, piece) in pieces.iter().enumerate() {
    if piece.length() <= 0.0 {
      return Err(GeomError::EditRefused {
        reason: "a surviving piece has no length",
      });
    }
    let start = vertex_index(
      &mut vertices,
      &mut vertex_grid,
      piece.start_point(),
      tolerance,
      cell,
      near_miss_snaps,
      &allowed,
    );
    let end = vertex_index(
      &mut vertices,
      &mut vertex_grid,
      piece.end_point(),
      tolerance,
      cell,
      near_miss_snaps,
      &allowed,
    );
    if start == end {
      return Err(GeomError::EditRefused {
        reason: "a surviving piece begins and ends at the same point",
      });
    }
    vertices[start].ends.push((index, EndAt::Start));
    vertices[end].ends.push((index, EndAt::End));
  }

  // The end a walk arrives at, in clockwise order, is the previous one in ascending angle order at
  // that vertex. Keeping the material on the left is what makes that the correct continuation.
  let mut clockwise_next = vec![0_usize; 2 * pieces.len()];
  for vertex in &mut vertices {
    vertex
      .ends
      .sort_by(|a, b| angular_order(pieces, owners, *a, *b));
    let count = vertex.ends.len();
    for (slot, end) in vertex.ends.iter().enumerate() {
      let previous = vertex.ends[(slot + count - 1) % count];
      clockwise_next[end_id(*end)] = end_id(previous);
    }
  }

  let mut order: Vec<usize> = (0..pieces.len()).collect();
  order.sort_by(|a, b| start_order(pieces, *a, *b));

  let mut used = vec![false; pieces.len()];
  let mut contours = Vec::new();
  for &first in &order {
    if used[first] {
      continue;
    }
    let seed = end_id((first, EndAt::Start));
    let mut current = seed;
    let mut chained: Vec<Segment> = Vec::new();
    let mut consumed = 0_usize;
    loop {
      consumed += 1;
      let (piece_index, at) = split_id(current);
      if used[piece_index] {
        return Err(GeomError::EditRefused {
          reason: "the arrangement revisits a piece, so its pieces cannot be chained closed",
        });
      }
      used[piece_index] = true;
      let segment = if at == EndAt::Start {
        pieces[piece_index]
      } else {
        pieces[piece_index].reversed()
      };
      chained.extend(exactly(segment, tolerance, flattened));
      let following = clockwise_next[other_end_id(current)];
      if following == seed {
        break;
      }
      if consumed > pieces.len() {
        return Err(GeomError::EditRefused {
          reason: "the arrangement does not close, so its pieces cannot be chained closed",
        });
      }
      current = following;
    }
    if chained.len() < 2 {
      return Err(GeomError::EditRefused {
        reason: "a chained contour has fewer than two pieces",
      });
    }
    let contour = Contour::from_segments(&chained, true).map_err(|_| GeomError::EditRefused {
      reason: "a chained contour is not a representable contour",
    })?;
    contours.push(contour);
  }
  Ok(contours)
}

/// The vertex a point belongs to, creating it if this is the first piece end here.
///
/// Near-miss snaps unify an endpoint with its projection on another segment: the two points are
/// farther apart than `tolerance` but are the same junction in the arrangement. A point matches a
/// vertex when it is within tolerance, or when either side of any snap pair identifies it with a
/// vertex already present through surviving endpoints.
fn vertex_index(
  vertices: &mut Vec<Vertex>,
  vertex_grid: &mut HashMap<(i64, i64), Vec<usize>>,
  point: Point2,
  tolerance: crate::TolerancePolicy,
  cell: f64,
  near_miss_snaps: &[(Point2, Point2)],
  allowed: &[Point2],
) -> usize {
  if near_miss_snaps.is_empty() {
    let cx = (point.x / cell).floor() as i64;
    let cy = (point.y / cell).floor() as i64;
    let mut found: Option<usize> = None;
    for dx in -1..=1 {
      for dy in -1..=1 {
        if let Some(bucket) = vertex_grid.get(&(cx + dx, cy + dy)) {
          for &index in bucket {
            if vertices[index].point.is_near(point, tolerance) {
              found = Some(match found {
                Some(prev) if prev < index => prev,
                _ => index,
              });
            }
          }
        }
      }
    }
    if let Some(index) = found {
      return index;
    }
    let index = vertices.len();
    vertices.push(Vertex {
      point,
      ends: Vec::new(),
    });
    vertex_grid.entry((cx, cy)).or_default().push(index);
    return index;
  }

  if let Some(index) = vertices.iter().position(|vertex| {
    vertex.point.is_near(point, tolerance)
      || points_identified(vertex.point, point, near_miss_snaps, allowed)
  }) {
    return index;
  }
  let index = vertices.len();
  vertices.push(Vertex {
    point,
    ends: Vec::new(),
  });
  let cx = (point.x / cell).floor() as i64;
  let cy = (point.y / cell).floor() as i64;
  vertex_grid.entry((cx, cy)).or_default().push(index);
  index
}

/// Drop snaps that would collapse a surviving piece: if a piece has one endpoint at `from` and the
/// other at `to` (or identified through the snap set), snapping makes it zero-length in the graph.
/// Identification is transitive through surviving endpoints only. Only snaps that participate in
/// collapsing a surviving piece are removed.
fn filter_valid_snaps(pieces: &[Segment], snaps: &[(Point2, Point2)]) -> Vec<(Point2, Point2)> {
  let allowed: Vec<Point2> = pieces
    .iter()
    .flat_map(|piece| [piece.start_point(), piece.end_point()])
    .collect();
  let mut current = snaps.to_vec();
  loop {
    // Find a piece whose endpoints would be identified by the current snap set.
    let collapse = pieces.iter().find_map(|piece| {
      let s = piece.start_point();
      let e = piece.end_point();
      if points_identified_transitive(s, e, &current, &[], &allowed) {
        Some((s, e))
      } else {
        None
      }
    });
    let Some((s, e)) = collapse else {
      return current;
    };
    // Remove one snap on a path connecting s and e (prefer a snap directly touching either).
    let path = identifying_path(s, e, &current, &allowed);
    let Some(path) = path else {
      // Should not happen, but avoid infinite loop.
      return current;
    };
    // Choose a snap in the path to drop: prefer one whose endpoint matches s or e.
    let drop_idx = path
      .iter()
      .copied()
      .find(|&i| {
        let (from, to) = current[i];
        from == s || to == s || from == e || to == e
      })
      .unwrap_or(path[0]);
    current.remove(drop_idx);
  }
}

/// Indices of snaps forming a path that identifies `a` and `b`, if any, through allowed points.
fn identifying_path(
  a: Point2,
  b: Point2,
  snaps: &[(Point2, Point2)],
  allowed: &[Point2],
) -> Option<Vec<usize>> {
  if a == b {
    return Some(Vec::new());
  }
  // BFS over points, tracking snap index used to reach each point.
  let mut queue: std::collections::VecDeque<Point2> = std::collections::VecDeque::new();
  let mut parent: Vec<Option<(Point2, usize)>> = Vec::new();
  let mut seen: Vec<Point2> = Vec::new();

  seen.push(a);
  parent.push(None);
  queue.push_back(a);

  while let Some(current) = queue.pop_front() {
    for (idx, (from, to)) in snaps.iter().copied().enumerate() {
      let next = if current == from {
        Some(to)
      } else if current == to {
        Some(from)
      } else {
        None
      };
      let Some(next) = next else { continue };
      if seen.contains(&next) {
        continue;
      }
      if next != b && !allowed.contains(&next) {
        continue;
      }
      seen.push(next);
      parent.push(Some((current, idx)));
      if next == b {
        // Reconstruct path
        let mut path = Vec::new();
        let mut cur = b;
        while cur != a {
          let pos = seen.iter().position(|p| *p == cur).unwrap();
          let (p, snap_idx) = parent[pos].unwrap();
          path.push(snap_idx);
          cur = p;
        }
        path.sort_unstable();
        path.dedup();
        return Some(path);
      }
      queue.push_back(next);
    }
  }
  None
}

/// Whether two points are the same junction via near-miss snap pairs (transitively) through
/// allowed intermediate points (surviving piece endpoints).
fn points_identified_transitive(
  a: Point2,
  b: Point2,
  snaps: &[(Point2, Point2)],
  extra: &[(Point2, Point2)],
  allowed: &[Point2],
) -> bool {
  if a == b {
    return true;
  }
  let mut seen: Vec<Point2> = vec![a];
  let mut stack: Vec<Point2> = vec![a];
  while let Some(current) = stack.pop() {
    if current == b {
      return true;
    }
    for (from, to) in snaps.iter().chain(extra.iter()).copied() {
      let next = if current == from {
        Some(to)
      } else if current == to {
        Some(from)
      } else {
        None
      };
      if let Some(next) = next {
        if next == b {
          return true;
        }
        if !allowed.contains(&next) || seen.contains(&next) {
          continue;
        }
        seen.push(next);
        stack.push(next);
      }
    }
  }
  false
}

/// Whether two points are the same junction via a near-miss snap pair, restricted to paths
/// through surviving endpoints.
fn points_identified(a: Point2, b: Point2, snaps: &[(Point2, Point2)], allowed: &[Point2]) -> bool {
  points_identified_transitive(a, b, snaps, &[], allowed)
}

/// The unique id of a piece end: pieces are pairs of ends, so the low bit is the end.
fn end_id(end: (usize, EndAt)) -> usize {
  end.0 * 2 + usize::from(end.1 == EndAt::End)
}

/// The piece and end an id names.
fn split_id(id: usize) -> (usize, EndAt) {
  (
    id / 2,
    if id % 2 == 0 {
      EndAt::Start
    } else {
      EndAt::End
    },
  )
}

/// The id of the piece's other end.
fn other_end_id(id: usize) -> usize {
  id ^ 1
}

/// The direction a piece leaves a vertex in, if its geometry defines one.
fn outward_vector(pieces: &[Segment], end: (usize, EndAt)) -> Option<Vec2> {
  let segment = pieces[end.0];
  match (segment, end.1) {
    (Segment::Line(line), EndAt::Start) => line.direction(),
    (Segment::Line(line), EndAt::End) => line.reversed().direction(),
    (Segment::Arc(arc), EndAt::Start) => Some(arc.tangent_at_start()),
    (Segment::Arc(arc), EndAt::End) => Some(arc.reversed().tangent_at_start()),
  }
}

/// The direction a piece leaves a vertex in, as an angle in `(-pi, pi]`.
fn outward_angle(pieces: &[Segment], end: (usize, EndAt)) -> f64 {
  outward_vector(pieces, end)
    .map(|vector: Vec2| vector.angle())
    .unwrap_or(0.0)
}

/// Which way a piece bends as it leaves a vertex: clockwise `-1`, straight `0`, counter-clockwise `1`.
///
/// Two boundaries that touch at a vertex leave it in the *same* direction, so the angle alone cannot
/// order them, and the order is what decides the chaining. The second-order answer is the only one
/// that tells them apart: an arc turns towards its centre, so the sign of the turn separates two
/// tangent arcs from one another.
fn curvature_sign(pieces: &[Segment], end: (usize, EndAt)) -> i8 {
  let Segment::Arc(arc) = pieces[end.0] else {
    return 0;
  };
  let Some(direction) = outward_vector(pieces, end) else {
    return 0;
  };
  let point = match end.1 {
    EndAt::Start => arc.start_point(),
    EndAt::End => arc.end_point(),
  };
  let offset = arc.center() - point;
  let turn = direction.x * offset.y - direction.y * offset.x;
  if turn > 0.0 {
    1
  } else if turn < 0.0 {
    -1
  } else {
    0
  }
}

/// Where a piece end sorts at its vertex: the direction it leaves in, with the way it bends folded in.
///
/// A tangency is two ends leaving one vertex in the same direction, so the direction alone cannot
/// order them, and the order is what decides the chaining. Folding the bend into the key as a
/// third-order perturbation separates that pair while keeping **one scalar per end**, which is what a
/// total order needs. Deciding "these two angles are close enough to be equal" instead is not
/// transitive -- three ends within a tolerance of each other can be ordered three different ways --
/// and `sort_by` on a comparator like that may cycle, panic, or chain the arrangement arbitrarily.
/// The counter-clockwise bend sorts first and the clockwise bend last, as the walk's left-hand rule
/// wants.
fn angular_key(pieces: &[Segment], end: (usize, EndAt)) -> f64 {
  outward_angle(pieces, end) - ANGLE_TIE_BREAK * f64::from(curvature_sign(pieces, end))
}

/// Ascending direction order at a vertex, over the whole key and then the identity as a tie-break.
///
/// Tangency is why the key carries the bend at all. Two pieces tangent at a vertex share a direction
/// exactly, and ordering such a pair by its index alone chains a hairpin that walks the same piece
/// twice. The input and piece order after the key keep the walk deterministic when the curvature
/// agrees too, and each of them is a total order, so the composition is one.
fn angular_order(
  pieces: &[Segment],
  owners: &[bool],
  a: (usize, EndAt),
  b: (usize, EndAt),
) -> Ordering {
  angular_key(pieces, a)
    .partial_cmp(&angular_key(pieces, b))
    .unwrap_or(Ordering::Equal)
    .then(owners[a.0].cmp(&owners[b.0]))
    .then(a.0.cmp(&b.0))
    .then(a.1.rank().cmp(&b.1.rank()))
}

/// The order chains start in: lowest `(bounds.min.y, bounds.min.x, input index)` first.
fn start_order(pieces: &[Segment], a: usize, b: usize) -> Ordering {
  let first = pieces[a].bounds();
  let second = pieces[b].bounds();
  first
    .min
    .y
    .partial_cmp(&second.min.y)
    .unwrap_or(Ordering::Equal)
    .then(
      first
        .min
        .x
        .partial_cmp(&second.min.x)
        .unwrap_or(Ordering::Equal),
    )
    .then(a.cmp(&b))
}

/// The piece as the contour model will keep it: exact, or rebuilt as straight segments and reported.
///
/// A contour stores an arc as a bulge and rebuilds it from its endpoints, so an arc the bulge cannot
/// hold would come back as a different arc — or, through the segment builder's own fallback, as a
/// straight chord. Such a piece is rebuilt here, at the caller's tolerance, and the caller is told
/// (A8, D-RB-5) instead of being handed geometry that quietly changed shape.
fn exactly(
  segment: Segment,
  tolerance: crate::TolerancePolicy,
  flattened: &mut bool,
) -> Vec<Segment> {
  let Segment::Arc(arc) = segment else {
    return vec![segment];
  };
  if survives_the_bulge(arc, tolerance) {
    return vec![segment];
  }
  *flattened = true;
  let points = arc.tessellate(tolerance);
  let mut lines: Vec<Segment> = Vec::with_capacity(points.len().saturating_sub(1));
  for window in points.windows(2) {
    if window[0].is_near(window[1], crate::TolerancePolicy::uniform(0.0)) {
      continue;
    }
    lines.push(Segment::Line(LineSeg::new(window[0], window[1])));
  }
  if lines.is_empty() {
    vec![Segment::Line(LineSeg::new(
      arc.start_point(),
      arc.end_point(),
    ))]
  } else {
    lines
  }
}

/// Whether the contour model's bulge round trip reproduces this arc within `tolerance`.
fn survives_the_bulge(arc: Arc, tolerance: crate::TolerancePolicy) -> bool {
  let Ok(rebuilt) = Arc::from_endpoints_bulge(arc.start_point(), arc.end_point(), arc.bulge())
  else {
    return false;
  };
  let radius_error = (rebuilt.radius() - arc.radius()).abs();
  let sweep_error = (rebuilt.sweep() - arc.sweep()).abs() * arc.radius().max(1.0);
  radius_error <= tolerance.epsilon && sweep_error <= tolerance.epsilon
}

/// Which set operation to keep.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BooleanOp {
  /// Material in either region.
  Union,
  /// Material in both regions.
  Intersection,
  /// Material in the first region and outside the second.
  Difference,
}

impl BooleanOp {
  /// Whether a point with these memberships is inside the result.
  fn holds(self, in_a: bool, in_b: bool) -> bool {
    match self {
      Self::Union => in_a || in_b,
      Self::Intersection => in_a && in_b,
      Self::Difference => in_a && !in_b,
    }
  }
}

/// The answer to a set operation, which may legitimately be nothing.
#[derive(Clone, Debug, PartialEq)]
pub enum Combined {
  /// No material at all.
  ///
  /// This is an answer and not a failure: two disjoint regions intersect in nothing, and so do a
  /// region and itself subtracted. [`normalize`] refuses an empty result because a normalization that
  /// produces nothing means the input was broken; a combination that produces nothing means the answer
  /// is nothing.
  Empty,
  /// The combined region, with no crossing between its boundaries.
  Region {
    /// The result.
    region: Region,
    /// True when a surviving piece was rebuilt as straight segments instead of being kept exact.
    flattened: bool,
  },
}

/// A piece of a boundary of either input, before it is classified.
#[derive(Clone, Copy, Debug)]
struct Piece {
  /// Whether the piece came from the first region.
  from_a: bool,
  /// Index of the boundary in that region's own order.
  path: usize,
  /// The piece itself.
  segment: Segment,
}

/// One segment of an operand region, kept with its boundary identity for indexed queries.
#[derive(Clone, Copy, Debug)]
struct IndexedSeg {
  /// Boundary index in the source region.
  boundary: usize,
  /// Segment index within that boundary's segment list, for self-skip identity.
  index: usize,
  /// The segment geometry.
  segment: Segment,
  /// Axis-aligned bounds of the segment.
  bounds: Bounds,
}

/// All segments of one boundary, precomputed once so membership probes need no contour rebuilds.
#[derive(Clone, Debug)]
struct BoundaryIndex {
  segments: Vec<Segment>,
  bounds: Bounds,
}

/// Uniform grid over operand segments, local to a single boolean operation.
#[derive(Clone, Debug)]
struct SegmentGrid {
  cell_size: f64,
  origin: Point2,
  cells: HashMap<(i32, i32), Vec<usize>>,
}

impl SegmentGrid {
  /// Build a grid over `segments`, inserting each by its bounds expanded by `margin`.
  fn build(segments: &[IndexedSeg], overall: Bounds, margin: f64) -> Self {
    let count = segments.len().max(1) as f64;
    let width = (overall.max.x - overall.min.x).abs().max(1.0);
    let height = (overall.max.y - overall.min.y).abs().max(1.0);
    let area_cell = (width * height / count).sqrt();
    let cell_size = area_cell.clamp(0.5, 50.0).max(margin.max(1.0e-6));
    let mut grid = Self {
      cell_size,
      origin: overall.min,
      cells: HashMap::new(),
    };
    for (id, seg) in segments.iter().enumerate() {
      let b = seg.bounds.expand(margin);
      let (min_cx, min_cy) = grid.cell(b.min.x, b.min.y);
      let (max_cx, max_cy) = grid.cell(b.max.x, b.max.y);
      for cx in min_cx..=max_cx {
        for cy in min_cy..=max_cy {
          grid.cells.entry((cx, cy)).or_default().push(id);
        }
      }
    }
    grid
  }

  fn cell(&self, x: f64, y: f64) -> (i32, i32) {
    let cx = ((x - self.origin.x) / self.cell_size).floor() as i32;
    let cy = ((y - self.origin.y) / self.cell_size).floor() as i32;
    (cx, cy)
  }

  /// Visit segment ids whose inserted bounds overlap `query` (expanded bounds query).
  fn query_bounds(&self, query: Bounds, mut visit: impl FnMut(usize)) {
    if self.cells.is_empty() {
      return;
    }
    let (min_cx, min_cy) = self.cell(query.min.x, query.min.y);
    let (max_cx, max_cy) = self.cell(query.max.x, query.max.y);
    for cx in min_cx..=max_cx {
      for cy in min_cy..=max_cy {
        if let Some(ids) = self.cells.get(&(cx, cy)) {
          for &id in ids {
            visit(id);
          }
        }
      }
    }
  }
}

/// Acceleration structure over one operand region, built once per boolean combination.
///
/// Used internally by [`combine`] for the cut phase and membership probes, and exposed so tests
/// can prove the indexed membership answer equals [`Region::contains`](crate::region::Region::contains).
pub struct RegionIndex {
  boundaries: Vec<BoundaryIndex>,
  segments: Vec<IndexedSeg>,
  grid: SegmentGrid,
  boundary_grid: HashMap<(i32, i32), Vec<usize>>,
  boundary_cell_size: f64,
  boundary_origin: Point2,
}

impl RegionIndex {
  /// Build the index over every segment of every boundary in `region`.
  pub fn build(region: &Region, tolerance: crate::TolerancePolicy) -> Self {
    let mut boundaries = Vec::with_capacity(region.boundaries.len());
    let mut segments = Vec::new();
    let mut overall = Bounds::empty();
    for (b_idx, contour) in region.boundaries.iter().enumerate() {
      let segs = contour.segments();
      let mut b_bounds = Bounds::empty();
      for (s_idx, &segment) in segs.iter().enumerate() {
        let bounds = segment.bounds();
        b_bounds = b_bounds.union(bounds);
        overall = overall.union(bounds);
        segments.push(IndexedSeg {
          boundary: b_idx,
          index: s_idx,
          segment,
          bounds,
        });
      }
      boundaries.push(BoundaryIndex {
        segments: segs,
        bounds: b_bounds,
      });
    }
    if overall.is_empty() {
      overall = Bounds {
        min: Point2::new(0.0, 0.0),
        max: Point2::new(0.0, 0.0),
      };
    }
    let margin = tolerance.epsilon.max(1.0e-9);
    let grid = SegmentGrid::build(&segments, overall, margin);

    let b_count = boundaries.len().max(1) as f64;
    let width = (overall.max.x - overall.min.x).abs().max(1.0);
    let height = (overall.max.y - overall.min.y).abs().max(1.0);
    let area_cell = (width * height / b_count).sqrt();
    let boundary_cell_size = area_cell.clamp(0.5, 100.0).max(margin.max(1.0e-6));
    let boundary_origin = overall.min;
    let mut boundary_grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    let b_cell = |x: f64, y: f64| -> (i32, i32) {
      (
        ((x - boundary_origin.x) / boundary_cell_size).floor() as i32,
        ((y - boundary_origin.y) / boundary_cell_size).floor() as i32,
      )
    };
    for (index, boundary) in boundaries.iter().enumerate() {
      let b = boundary.bounds.expand(margin);
      let (min_cx, min_cy) = b_cell(b.min.x, b.min.y);
      let (max_cx, max_cy) = b_cell(b.max.x, b.max.y);
      for cx in min_cx..=max_cx {
        for cy in min_cy..=max_cy {
          boundary_grid.entry((cx, cy)).or_default().push(index);
        }
      }
    }

    Self {
      boundaries,
      segments,
      grid,
      boundary_grid,
      boundary_cell_size,
      boundary_origin,
    }
  }

  /// Whether `point` is material, equivalent to
  /// [`Region::contains`](crate::region::Region::contains): a point within `tolerance` of any
  /// boundary is not material, otherwise parity of winding over the boundaries decides.
  pub fn contains(&self, point: Point2, tolerance: crate::TolerancePolicy) -> bool {
    if self.boundaries.is_empty() {
      return false;
    }
    let mut enclosed = 0_usize;
    self
      .for_candidate_boundaries(point, tolerance.epsilon, |index, boundary| {
        match self.classify_boundary(index, boundary, point, tolerance.epsilon) {
          Membership::OnBoundary => Err(()),
          Membership::Inside => {
            enclosed += 1;
            Ok(())
          }
          Membership::Outside => Ok(()),
        }
      })
      .is_ok()
      && enclosed % 2 == 1
  }

  /// Membership test equivalent to [`enclosed_by`], but using precomputed segments and spatial
  /// filtering for the boundary-distance check. Winding/parity semantics are preserved exactly.
  fn enclosed_by(
    &self,
    skip: Option<usize>,
    point: Point2,
    tolerance: crate::TolerancePolicy,
  ) -> Option<bool> {
    let eps = tolerance.epsilon;
    let mut enclosed = 0_usize;
    let result = self.for_candidate_boundaries(point, eps, |index, boundary| {
      if Some(index) == skip {
        // Same rule as the non-indexed path: parity of the boundary's winding number, without the
        // on-boundary short-circuit used for non-skipped boundaries.
        if !point_inside_bounds(boundary.bounds, point, 0.0) {
          return Ok(());
        }
        if winding_number_of_segments(&boundary.segments, point).rem_euclid(2) == 1 {
          enclosed += 1;
        }
        return Ok(());
      }
      match self.classify_boundary(index, boundary, point, eps) {
        Membership::OnBoundary => Err(()),
        Membership::Inside => {
          enclosed += 1;
          Ok(())
        }
        Membership::Outside => Ok(()),
      }
    });
    result.ok().map(|()| enclosed % 2 == 1)
  }

  fn for_candidate_boundaries(
    &self,
    point: Point2,
    eps: f64,
    mut visit: impl FnMut(usize, &BoundaryIndex) -> Result<(), ()>,
  ) -> Result<(), ()> {
    if self.boundaries.is_empty() {
      return Ok(());
    }
    let cx = ((point.x - self.boundary_origin.x) / self.boundary_cell_size).floor() as i32;
    let cy = ((point.y - self.boundary_origin.y) / self.boundary_cell_size).floor() as i32;
    let extra = ((eps / self.boundary_cell_size).ceil() as i32).max(0);
    let mut stamp = vec![false; self.boundaries.len()];
    for dx in -extra..=extra {
      for dy in -extra..=extra {
        if let Some(ids) = self.boundary_grid.get(&(cx + dx, cy + dy)) {
          for &id in ids {
            if stamp[id] {
              continue;
            }
            stamp[id] = true;
            visit(id, &self.boundaries[id])?;
          }
        }
      }
    }
    Ok(())
  }

  fn classify_boundary(
    &self,
    boundary_index: usize,
    boundary: &BoundaryIndex,
    point: Point2,
    eps: f64,
  ) -> Membership {
    let in_expanded = point_inside_bounds(boundary.bounds, point, eps);
    if !in_expanded {
      return Membership::Outside;
    }
    let in_tight = point_inside_bounds(boundary.bounds, point, 0.0);
    // A point within tolerance of any segment of this boundary is on it.
    if self.near_boundary(boundary_index, point, eps) {
      return Membership::OnBoundary;
    }
    // Outside the contour's bounds (and not on it) means outside the contour; a point in the
    // margin band cannot be inside the contour, so the winding test is unnecessary.
    if !in_tight {
      return Membership::Outside;
    }
    if winding_number_of_segments(&boundary.segments, point) == 0 {
      Membership::Outside
    } else {
      Membership::Inside
    }
  }

  /// True if `point` is within `eps` of any segment of `boundary_index`.
  fn near_boundary(&self, boundary_index: usize, point: Point2, eps: f64) -> bool {
    let query = Bounds {
      min: Point2::new(point.x, point.y),
      max: Point2::new(point.x, point.y),
    }
    .expand(eps);
    let mut found = false;
    self.grid.query_bounds(query, |id| {
      if found {
        return;
      }
      let seg = self.segments[id];
      if seg.boundary != boundary_index {
        return;
      }
      if seg
        .bounds
        .expand(eps)
        .contains(point, crate::TolerancePolicy::uniform(0.0))
        && seg.segment.distance_to_point(point) <= eps
      {
        found = true;
      }
    });
    found
  }
}

/// Point inside (or on edge of) bounds, with an optional outward expansion.
fn point_inside_bounds(bounds: Bounds, point: Point2, expand: f64) -> bool {
  if bounds.is_empty() {
    return false;
  }
  point.x >= bounds.min.x - expand
    && point.x <= bounds.max.x + expand
    && point.y >= bounds.min.y - expand
    && point.y <= bounds.max.y + expand
}

/// Winding number of a closed contour given as precomputed segments, same angle-sum algorithm as
/// [`crate::winding::winding_number`].
fn winding_number_of_segments(segments: &[Segment], point: Point2) -> i32 {
  if segments.is_empty() {
    return 0;
  }
  let total: f64 = segments
    .iter()
    .map(|segment| winding_contribution(*segment, point))
    .sum();
  (total / TAU).round() as i32
}

/// The angle one segment subtends at `point`, mirroring `crate::winding::contribution`.
fn winding_contribution(segment: Segment, point: Point2) -> f64 {
  match segment {
    Segment::Line(line) => {
      let from = line.start - point;
      let to = line.end - point;
      if from.length_squared() == 0.0 || to.length_squared() == 0.0 {
        0.0
      } else {
        from.cross(to).atan2(from.dot(to))
      }
    }
    Segment::Arc(arc) => arc_winding_contribution(arc, point),
  }
}

/// Arc winding contribution, mirroring `crate::winding::arc_contribution` and its helpers.
fn arc_winding_contribution(arc: Arc, point: Point2) -> f64 {
  use std::f64::consts::PI;
  let centre = arc.center();
  let radius = arc.radius();
  let height = point.y - centre.y;

  let mut cuts = vec![0.0_f64, 1.0];
  let remaining = radius * radius - height * height;
  if remaining > 0.0 {
    let half_chord = remaining.sqrt();
    for x in [centre.x - half_chord, centre.x + half_chord] {
      let angle = height.atan2(x - centre.x);
      if let Some(fraction) = fraction_at_angle_arc(arc, angle) {
        if fraction > 1.0e-12 && fraction < 1.0 - 1.0e-12 {
          cuts.push(fraction);
        }
      }
    }
  }
  cuts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
  cuts.dedup_by(|a, b| (*a - *b).abs() <= 1.0e-12);

  let mut total = 0.0;
  for window in cuts.windows(2) {
    let (from, to) = (window[0], window[1]);
    if to - from <= 1.0e-12 {
      continue;
    }
    let start = arc.point_at_fraction(from) - point;
    let end = arc.point_at_fraction(to) - point;
    let middle = arc.point_at_fraction((from + to) * 0.5) - point;
    let below = middle.y < 0.0;
    total += branch_angle_vec(end, below, PI) - branch_angle_vec(start, below, PI);
  }
  total
}

fn fraction_at_angle_arc(arc: Arc, angle: f64) -> Option<f64> {
  let sweep = arc.sweep();
  let magnitude = sweep.abs();
  let start = arc.start_angle();
  let travelled = if sweep >= 0.0 {
    (angle - start).rem_euclid(TAU)
  } else {
    (start - angle).rem_euclid(TAU)
  };
  if travelled > magnitude {
    return None;
  }
  Some(travelled / magnitude)
}

fn branch_angle_vec(vector: Vec2, below: bool, pi: f64) -> f64 {
  if vector.length_squared() == 0.0 {
    return 0.0;
  }
  let on_line = vector.y.abs() <= 1.0e-9 * vector.length();
  if on_line {
    return match (below, vector.x > 0.0) {
      (true, true) => TAU,
      (_, true) => 0.0,
      (_, false) => pi,
    };
  }
  if below && vector.y < 0.0 {
    let angle = vector.angle();
    return if angle < 0.0 { angle + TAU } else { angle };
  }
  vector.angle()
}

/// Where a piece goes in the result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Decision {
  /// Keep it as it is: the result's material is on its left.
  Keep,
  /// Reverse it: the result's material is on its right.
  Reverse,
  /// Drop it: the result looks the same on both sides.
  Drop,
}

fn combine_with_indexed(
  _a: &Region,
  _b: &Region,
  ia: &RegionIndex,
  ib: &RegionIndex,
  op: BooleanOp,
  tolerance: crate::TolerancePolicy,
  detect_near_misses: bool,
) -> Result<Combined, GeomError> {
  // One arrangement over both regions, so no piece ever straddles the predicate's answer: every
  // segment is cut where it meets the other region's boundaries as well as its own.
  let mut pieces: Vec<Piece> = Vec::new();
  let mut re_cut = false;
  // Near-miss junctions are detected only on retry after an unchainable arrangement, so
  // legitimate gaps in well-formed arrangements are never snapped.
  let mut near_miss_snaps: Vec<(Point2, Point2)> = Vec::new();

  let mut stamp_a = vec![0u32; ia.segments.len()];
  let mut stamp_b = vec![0u32; ib.segments.len()];
  let mut stamp: u32 = 1;

  let sides = [(true, ia, ib), (false, ib, ia)];
  for (from_a, own, foreign) in sides {
    let (own_stamp, foreign_stamp) = if from_a {
      (&mut stamp_a, &mut stamp_b)
    } else {
      (&mut stamp_b, &mut stamp_a)
    };
    for seg in &own.segments {
      let segment = seg.segment;
      let mut fractions = vec![0.0_f64, 1.0];

      // Cut against siblings in the same region, excluding the segment itself.
      if detect_near_misses {
        for other in &own.segments {
          if other.boundary == seg.boundary && other.index == seg.index {
            continue;
          }
          push_fractions(
            &mut fractions,
            segment,
            std::iter::once(other.segment),
            tolerance,
            &mut re_cut,
            &mut near_miss_snaps,
            detect_near_misses,
          );
        }
      } else {
        stamp = stamp.wrapping_add(1);
        if stamp == 0 {
          own_stamp.fill(0);
          stamp = 1;
        }
        let query = segment.bounds().expand(tolerance.epsilon);
        own.grid.query_bounds(query, |id| {
          if own_stamp[id] == stamp {
            return;
          }
          own_stamp[id] = stamp;
          let other = own.segments[id];
          if other.boundary == seg.boundary && other.index == seg.index {
            return;
          }
          push_fractions(
            &mut fractions,
            segment,
            std::iter::once(other.segment),
            tolerance,
            &mut re_cut,
            &mut near_miss_snaps,
            detect_near_misses,
          );
        });
      }

      // Cut against every segment of the other region.
      if detect_near_misses {
        for other in &foreign.segments {
          push_fractions(
            &mut fractions,
            segment,
            std::iter::once(other.segment),
            tolerance,
            &mut re_cut,
            &mut near_miss_snaps,
            detect_near_misses,
          );
        }
      } else {
        stamp = stamp.wrapping_add(1);
        if stamp == 0 {
          foreign_stamp.fill(0);
          stamp = 1;
        }
        let query = segment.bounds().expand(tolerance.epsilon);
        foreign.grid.query_bounds(query, |id| {
          if foreign_stamp[id] == stamp {
            return;
          }
          foreign_stamp[id] = stamp;
          let other = foreign.segments[id];
          push_fractions(
            &mut fractions,
            segment,
            std::iter::once(other.segment),
            tolerance,
            &mut re_cut,
            &mut near_miss_snaps,
            detect_near_misses,
          );
        });
      }

      let fractions = dedup(&mut fractions, segment, tolerance);
      for piece in split_at(segment, &fractions)? {
        pieces.push(Piece {
          from_a,
          path: seg.boundary,
          segment: piece,
        });
      }
    }
  }

  let mut survivors: Vec<(Segment, bool)> = Vec::with_capacity(pieces.len());
  for piece in &pieces {
    match decide_indexed(*piece, ia, ib, op, tolerance, &near_miss_snaps)? {
      Decision::Keep => survivors.push((piece.segment, piece.from_a)),
      Decision::Reverse => survivors.push((piece.segment.reversed(), piece.from_a)),
      Decision::Drop => {}
    }
  }
  // A doubled edge is not a boundary: crossing it flips parity twice, so a region that kept both
  // copies would enclose nothing at all while its boundaries still summed to a nonzero area. Two
  // inputs that share a boundary produce exactly that -- identical regions, or a hole touching the
  // outer edge along a segment -- so identical pieces are kept once.
  let survivors = dedup_pieces(survivors, tolerance);
  // A piece no longer than the welding distance has no representable length here: its two ends weld
  // to the same vertex. Neither keeping it (it collapses into a point) nor dropping it (it leaves the
  // junction degree three, which the walk then revisits) is right. The right operation is a
  // *contraction*: remove the piece and identify its ends, so the neighbours it joined still meet.
  let mut contracted: Vec<(Segment, bool)> = Vec::with_capacity(survivors.len());
  for (segment, owner) in survivors {
    if segment.length() <= tolerance.epsilon * 2.0 {
      near_miss_snaps.push((segment.start_point(), segment.end_point()));
    } else {
      contracted.push((segment, owner));
    }
  }
  let survivors = contracted;
  if survivors.is_empty() {
    return Ok(Combined::Empty);
  }
  let (segments, owners): (Vec<Segment>, Vec<bool>) = survivors.into_iter().unzip();
  if detect_near_misses && !near_miss_snaps.is_empty() {
    let odd_points = odd_degree_points(&segments, tolerance);
    if odd_points.is_empty() {
      near_miss_snaps.clear();
    } else {
      near_miss_snaps.retain(|snap| snap_relevant_to_odd_degree(*snap, &odd_points, tolerance));
    }
  }
  let mut flattened = false;
  let valid_snaps = filter_valid_snaps(&segments, &near_miss_snaps);
  let boundaries = assemble(&segments, &owners, tolerance, &mut flattened, &valid_snaps)?;
  Ok(Combined::Region {
    region: Region { boundaries },
    flattened,
  })
}

/// Combine two regions.
///
/// The rule is the one [`normalize`] already applies to a single region, with the predicate evaluated
/// on both probes: a point is inside the result when [`BooleanOp::holds`] says so for its membership of
/// each input, a piece survives when the answer differs across it, and it is oriented so the
/// **result's** material is on its left. There is no per-operation special casing: a difference
/// inverts the second region by itself, because a piece inside it has the predicate false.
///
/// # Errors
///
/// Returns the first piece the tolerance cannot resolve, with the inspection that failed. A piece that
/// lies within a boundary's band at every probe distance is refused rather than classified by guess.
pub fn combine(
  a: &Region,
  b: &Region,
  op: BooleanOp,
  tolerance: crate::TolerancePolicy,
) -> Result<Combined, GeomError> {
  if !tolerance.epsilon.is_finite() || tolerance.epsilon <= 0.0 {
    return Err(GeomError::EditRefused {
      reason: "a set operation needs a positive, finite tolerance",
    });
  }

  let ia = RegionIndex::build(a, tolerance);
  let ib = RegionIndex::build(b, tolerance);
  match combine_with_indexed(a, b, &ia, &ib, op, tolerance, false) {
    Ok(result) => Ok(result),
    Err(err) if is_unchainable_error(&err) => {
      combine_with_indexed(a, b, &ia, &ib, op, tolerance, true)
    }
    Err(err) => Err(err),
  }
}

/// Decide where a piece goes by asking the predicate on both of its sides.
fn decide_indexed(
  piece: Piece,
  a: &RegionIndex,
  b: &RegionIndex,
  op: BooleanOp,
  tolerance: crate::TolerancePolicy,
  near_miss_snaps: &[(Point2, Point2)],
) -> Result<Decision, GeomError> {
  let segment = piece.segment;
  let length = segment.length();
  if length <= 0.0 {
    return Err(GeomError::EditRefused {
      reason: "a zero-length piece cannot be classified",
    });
  }
  // A piece's own boundary is skipped on its own side, exactly as the single-region walk does; the
  // other region's boundaries are all consulted.
  let skip_a = if piece.from_a { Some(piece.path) } else { None };
  let skip_b = if piece.from_a { None } else { Some(piece.path) };
  let base = (length * PROBE_STEP).max(PROBE_FLOOR);
  let mut samples: Vec<(Decision, usize)> = Vec::with_capacity(PROBE_TRIES);
  for attempt in 0..PROBE_TRIES {
    let distance = base * PROBE_GROWTH.powi(attempt as i32);
    if distance * 2.0 > length {
      break;
    }
    let Some((left, right)) = probe_points(segment, distance) else {
      break;
    };
    let memberships = (
      a.enclosed_by(skip_a, left, tolerance),
      b.enclosed_by(skip_b, left, tolerance),
      a.enclosed_by(skip_a, right, tolerance),
      b.enclosed_by(skip_b, right, tolerance),
    );
    let (Some(left_a), Some(left_b), Some(right_a), Some(right_b)) = memberships else {
      // A probe landed within some boundary's band; step farther out and ask again.
      continue;
    };
    let left_inside = op.holds(left_a, left_b);
    let right_inside = op.holds(right_a, right_b);
    let decision = match (left_inside, right_inside) {
      (true, false) => Decision::Keep,
      (false, true) => Decision::Reverse,
      _ => Decision::Drop,
    };
    samples.push((decision, attempt));
  }
  let first_definitive = if samples.is_empty() {
    None
  } else {
    let counts_for = |samples: &[(Decision, usize)]| {
      let mut c = [0_u32; 3];
      for (d, _) in samples {
        let idx = match d {
          Decision::Keep => 0,
          Decision::Reverse => 1,
          Decision::Drop => 2,
        };
        c[idx] += 1;
      }
      c
    };
    let total = samples.len();
    let filtered: Vec<(Decision, usize)> = if total <= 2 {
      samples.clone()
    } else {
      let counts = counts_for(&samples);
      let kept: Vec<(Decision, usize)> = samples
        .iter()
        .copied()
        .filter(|(d, _)| {
          let idx = match d {
            Decision::Keep => 0,
            Decision::Reverse => 1,
            Decision::Drop => 2,
          };
          counts[idx] > 1
        })
        .collect();
      if kept.is_empty() {
        samples.clone()
      } else {
        kept
      }
    };
    let counts = counts_for(&filtered);
    let mut best = filtered[0].0;
    let mut best_count = 0_u32;
    let mut best_attempt = usize::MAX;
    for (d, attempt) in &filtered {
      let idx = match d {
        Decision::Keep => 0,
        Decision::Reverse => 1,
        Decision::Drop => 2,
      };
      let count = counts[idx];
      let priority = |d: Decision| match d {
        // Keeping a boundary avoids opening the arrangement; on a genuine tie prefer it.
        Decision::Keep => 2_u8,
        Decision::Reverse => 1,
        Decision::Drop => 0,
      };
      let better = if count != best_count {
        count > best_count
      } else if priority(*d) != priority(best) {
        priority(*d) > priority(best)
      } else {
        *attempt < best_attempt
      };
      if better {
        best = *d;
        best_count = count;
        best_attempt = *attempt;
      }
    }
    Some(best)
  };
  if let Some(decision) = first_definitive {
    // A boundary at normal scale may straddle a thin sliver created by a near-miss junction;
    // a close probe detects it (both sides the same locally). Only run this guard on pieces
    // adjacent to a near-miss snap (one endpoint matches a snap endpoint or projection).
    let is_adjacent_to_snap = near_miss_snaps.iter().any(|(endpoint, projection)| {
      segment.start_point() == *endpoint
        || segment.end_point() == *endpoint
        || segment.start_point() == *projection
        || segment.end_point() == *projection
    });
    if is_adjacent_to_snap && matches!(decision, Decision::Keep | Decision::Reverse) {
      // Probe close to detect a thin sliver. Try smaller distances if a close probe suggests a
      // sliver, to avoid misclassifying a narrow valid feature that the probe stepped past.
      let mut probe = PROBE_FLOOR * PROBE_GROWTH.powi(2);
      let min_probe = (PROBE_FLOOR * 2.0).max(tolerance.epsilon);
      let mut drop_as_sliver = false;
      while probe * 2.0 <= length && probe >= min_probe {
        if let Some((left, right)) = probe_points(segment, probe) {
          let memberships = (
            a.enclosed_by(skip_a, left, tolerance),
            b.enclosed_by(skip_b, left, tolerance),
            a.enclosed_by(skip_a, right, tolerance),
            b.enclosed_by(skip_b, right, tolerance),
          );
          match memberships {
            (Some(left_a), Some(left_b), Some(right_a), Some(right_b)) => {
              let left_inside = op.holds(left_a, left_b);
              let right_inside = op.holds(right_a, right_b);
              if left_inside == right_inside {
                drop_as_sliver = true;
                probe /= PROBE_GROWTH;
                continue;
              } else {
                drop_as_sliver = false;
                break;
              }
            }
            _ => {
              drop_as_sliver = false;
              break;
            }
          }
        } else {
          break;
        }
      }
      if drop_as_sliver {
        return Ok(Decision::Drop);
      }
    }
    return Ok(decision);
  }
  Err(GeomError::EditRefused {
    reason: "the arrangement is finer than the tolerance can resolve",
  })
}

/// Keep one copy of each geometrically identical piece.
///
/// Two pieces are the same edge only when they are the same curve traversed the same way. Sharing
/// endpoints is not enough: two semicircles of the same circle meet at the same seam points but are
/// different curves, and both must survive. Pieces whose oriented endpoints and geometry agree
/// within tolerance are treated as duplicates to avoid non-manifold slivers from near-coincident
/// representations.
fn dedup_pieces(
  pieces: Vec<(Segment, bool)>,
  tolerance: crate::TolerancePolicy,
) -> Vec<(Segment, bool)> {
  let tol = tolerance.epsilon;
  let cell = tol.max(1.0e-12);
  let key = |p: crate::point::Point2| -> (i64, i64) {
    ((p.x / cell).floor() as i64, (p.y / cell).floor() as i64)
  };
  let same_point = |a: crate::point::Point2, b: crate::point::Point2| a.distance_to(b) <= tol;

  let equiv = |a: Segment, b: Segment| -> bool {
    let same_dir =
      same_point(a.start_point(), b.start_point()) && same_point(a.end_point(), b.end_point());
    let opp_dir =
      same_point(a.start_point(), b.end_point()) && same_point(a.end_point(), b.start_point());
    if !same_dir && !opp_dir {
      return false;
    }
    match (a, b) {
      (Segment::Line(_), Segment::Line(_)) => true,
      (Segment::Arc(a1), Segment::Arc(a2)) => {
        let sweep2 = if same_dir { a2.sweep() } else { -a2.sweep() };
        if a1.center().distance_to(a2.center()) <= tol
          && (a1.radius() - a2.radius()).abs() <= tol
          && (a1.sweep() - sweep2).abs() * a1.radius().max(1.0) <= tol
        {
          return true;
        }
        let b_cmp = if same_dir {
          Segment::Arc(a2)
        } else {
          Segment::Arc(a2.reversed())
        };
        let a_seg = Segment::Arc(a1);
        same_point(point_at(a_seg, 0.25), point_at(b_cmp, 0.25))
          && same_point(point_at(a_seg, 0.5), point_at(b_cmp, 0.5))
          && same_point(point_at(a_seg, 0.75), point_at(b_cmp, 0.75))
      }
      _ => false,
    }
  };

  type EndpointPairKey = ((i64, i64), (i64, i64));
  let mut index: HashMap<EndpointPairKey, Vec<usize>> = HashMap::new();
  let mut kept: Vec<(Segment, bool)> = Vec::with_capacity(pieces.len());
  for (piece, owner) in pieces {
    let sc = key(piece.start_point());
    let ec = key(piece.end_point());
    let mut duplicate = false;
    'search: for dsx in -1..=1 {
      for dsy in -1..=1 {
        for dex in -1..=1 {
          for dey in -1..=1 {
            let sk = (sc.0 + dsx, sc.1 + dsy);
            let ek = (ec.0 + dex, ec.1 + dey);
            if let Some(candidates) = index.get(&(sk, ek)) {
              for &ki in candidates {
                if equiv(piece, kept[ki].0) {
                  duplicate = true;
                  break 'search;
                }
              }
            }
          }
        }
      }
    }
    if duplicate {
      continue;
    }
    let idx = kept.len();
    index.entry((sc, ec)).or_default().push(idx);
    index.entry((ec, sc)).or_default().push(idx);
    kept.push((piece, owner));
  }
  kept
}

#[cfg(test)]
mod angular_key_tests {
  use super::*;

  fn line(angle: f64) -> Segment {
    Segment::Line(LineSeg::new(
      Point2::new(0.0, 0.0),
      Point2::new(angle.cos(), angle.sin()),
    ))
  }

  /// An arc whose tangent at its start points at `angle` and turns counter-clockwise.
  fn ccw_arc_at_tangent(angle: f64) -> Segment {
    arc(
      0.0,
      angle - std::f64::consts::PI / 2.0,
      std::f64::consts::PI,
    )
  }

  /// An arc whose tangent at its start points at `angle` and turns clockwise.
  fn cw_arc_at_tangent(angle: f64) -> Segment {
    arc(
      0.0,
      angle + std::f64::consts::PI / 2.0,
      -std::f64::consts::PI,
    )
  }

  fn arc(center_x: f64, start_angle: f64, sweep: f64) -> Segment {
    Segment::Arc(Arc::new(Point2::new(center_x, 0.0), 5.0, start_angle, sweep).expect("an arc"))
  }

  /// Both ends of every piece, which is what the chaining walk sorts at a vertex.
  fn ends(pieces: &[Segment]) -> Vec<(usize, EndAt)> {
    (0..pieces.len())
      .flat_map(|index| [(index, EndAt::Start), (index, EndAt::End)])
      .collect()
  }

  /// The walk calls `sort_by` on this comparator, so an intransitive one can cycle or panic.
  ///
  /// This is the triple that makes a tolerance-based tie intransitive: a clockwise arc at angle 0, a
  /// straight line 0.6e-12 rad away and a counter-clockwise arc 1.2e-12 rad away. Each neighbour is
  /// within the tie tolerance, so the old comparator ordered them by bend (clockwise arc last, of
  /// course, the other way round), while the far pair fell back to the angle and reversed it. A total
  /// order cannot do that, and this one no longer does.
  #[test]
  fn the_order_is_total_so_a_sort_cannot_cycle() {
    let pieces = vec![
      cw_arc_at_tangent(0.0),
      line(0.6e-12),
      ccw_arc_at_tangent(1.2e-12),
    ];
    let owners = vec![true; pieces.len()];
    let all = ends(&pieces);

    for a in &all {
      for b in &all {
        for c in &all {
          let ab = angular_order(&pieces, &owners, *a, *b);
          let bc = angular_order(&pieces, &owners, *b, *c);
          let ac = angular_order(&pieces, &owners, *a, *c);
          if ab != Ordering::Greater && bc != Ordering::Greater {
            assert_ne!(
              ac,
              Ordering::Greater,
              "{a:?} <= {b:?} and {b:?} <= {c:?} must imply {a:?} <= {c:?}"
            );
          }
        }
      }
    }

    let mut sorted = all.clone();
    sorted.sort_by(|a, b| angular_order(&pieces, &owners, *a, *b));
    let mut by_key = all;
    by_key.sort_by(|a, b| {
      angular_key(&pieces, *a)
        .partial_cmp(&angular_key(&pieces, *b))
        .unwrap_or(Ordering::Equal)
    });
    assert_eq!(sorted, by_key, "one scalar per end, sorted the same way");
  }

  /// Two discs touching at one point leave it in the same direction: only the bend tells them apart.
  #[test]
  fn a_tangency_sorts_the_counter_clockwise_bend_first() {
    let pieces = vec![ccw_arc_at_tangent(0.0), cw_arc_at_tangent(0.0)];
    let owners = vec![true, true];
    let (left, right) = ((0, EndAt::Start), (1, EndAt::Start));

    assert_eq!(
      outward_angle(&pieces, left),
      outward_angle(&pieces, right),
      "the two ends leave in the same direction"
    );
    assert_eq!(curvature_sign(&pieces, left), 1);
    assert_eq!(curvature_sign(&pieces, right), -1);
    assert_eq!(
      angular_order(&pieces, &owners, left, right),
      Ordering::Less,
      "the counter-clockwise bend comes first"
    );
  }
}
