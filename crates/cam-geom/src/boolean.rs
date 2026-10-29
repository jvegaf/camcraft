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
use std::f64::consts::TAU;

use crate::arc::Arc;
use crate::contour::Contour;
use crate::intersect::{intersect, Intersections};
use crate::point::{Point2, Vec2};
use crate::region::{Normalized, Region};
use crate::segment::{LineSeg, Segment};
use crate::winding::{classify, winding_number, Membership};
use crate::GeomError;

/// How close to a segment's own end an intersection may sit before it stops counting as a split.
const FRACTION_EPSILON: f64 = 1.0e-9;

/// The first distance a side probe steps away from a piece, as a fraction of the piece's length.
const PROBE_STEP: f64 = 1.0e-5;

/// The smallest distance a side probe may step away from a piece, in millimetres.
const PROBE_FLOOR: f64 = 1.0e-9;

/// How much farther each retry steps, when the first probe landed inside some boundary's band.
const PROBE_GROWTH: f64 = 16.0;

/// How many probe distances are tried before the arrangement is declared unresolvable.
const PROBE_TRIES: usize = 4;

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

/// Split every boundary at its crossings, keep the pieces that border material and chain them closed.
pub fn normalize(region: &Region, tolerance: f64) -> Result<Normalized, GeomError> {
  if !tolerance.is_finite() || tolerance <= 0.0 {
    return Err(GeomError::EditRefused {
      reason: "normalization needs a positive, finite tolerance",
    });
  }

  let mut cuts: Vec<Cut> = Vec::new();
  let mut re_cut = false;
  for (path, boundary) in region.boundaries.iter().enumerate() {
    for segment in boundary.segments() {
      let fractions = cut_fractions(segment, region, path, tolerance, &mut re_cut)?;
      for piece in split_at(segment, &fractions)? {
        cuts.push(Cut {
          path,
          segment: piece,
        });
      }
    }
  }

  // Nothing crossed, so nothing needs re-making: the input comes back in its own vertex order and
  // arc-exact (A4).
  if !re_cut {
    return Ok(Normalized {
      region: region.clone(),
      flattened: false,
    });
  }

  let mut survivors: Vec<Segment> = Vec::with_capacity(cuts.len());
  for cut in cuts {
    match material_side(cut.segment, cut.path, region, tolerance)? {
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

  let mut flattened = false;
  let boundaries = assemble(&survivors, tolerance, &mut flattened)?;
  Ok(Normalized {
    region: Region { boundaries },
    flattened,
  })
}

/// The fractional positions at which `segment` must be split, including both of its ends.
fn cut_fractions(
  segment: Segment,
  region: &Region,
  path: usize,
  tolerance: f64,
  re_cut: &mut bool,
) -> Result<Vec<f64>, GeomError> {
  let mut fractions = vec![0.0_f64, 1.0];
  for (other_path, boundary) in region.boundaries.iter().enumerate() {
    if other_path == path {
      continue;
    }
    for other in boundary.segments() {
      match intersect(segment, other, tolerance) {
        Intersections::None | Intersections::Tangent(_) => {}
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
  }
  Ok(dedup(&mut fractions, segment, tolerance))
}

/// Where along `segment` a point sits, if it sits *on* it and away from both of its ends.
///
/// A meeting point at an end is not a split: the contour already changes vertex there, and cutting it
/// would leave a zero-length piece at the end of every shared corner.
fn interior_fraction(segment: Segment, point: Point2, tolerance: f64) -> Option<f64> {
  let fraction = fraction_along(segment, point)?;
  if !(0.0..=1.0).contains(&fraction) {
    return None;
  }
  if fraction <= FRACTION_EPSILON || fraction >= 1.0 - FRACTION_EPSILON {
    return None;
  }
  if point.distance_to(point_at(segment, fraction)) > tolerance {
    return None;
  }
  if point.is_near(segment.start_point(), tolerance)
    || point.is_near(segment.end_point(), tolerance)
  {
    return None;
  }
  Some(fraction)
}

/// Sort the cut fractions and drop the ones that are the same place within `tolerance`.
fn dedup(fractions: &mut [f64], segment: Segment, tolerance: f64) -> Vec<f64> {
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
  tolerance: f64,
) -> Result<Side, GeomError> {
  let length = segment.length();
  if length <= 0.0 {
    return Err(GeomError::EditRefused {
      reason: "a zero-length piece cannot be classified",
    });
  }
  let step = (length * PROBE_STEP).max(PROBE_FLOOR);
  for attempt in 0..PROBE_TRIES {
    let distance = step * PROBE_GROWTH.powi(attempt as i32);
    if distance * 2.0 > length {
      break;
    }
    let Some((left_point, right_point)) = probe_points(segment, distance) else {
      break;
    };
    let left = material_beside(region, path, left_point, tolerance);
    let right = material_beside(region, path, right_point, tolerance);
    match (left, right) {
      (Some(true), Some(false)) => return Ok(Side::Left),
      (Some(false), Some(true)) => return Ok(Side::Right),
      (Some(true), Some(true)) => return Ok(Side::Both),
      (Some(false), Some(false)) => return Ok(Side::Neither),
      // One probe landed within some other boundary's band; step farther out and ask again.
      _ => {}
    }
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
fn material_beside(region: &Region, path: usize, point: Point2, tolerance: f64) -> Option<bool> {
  let mut enclosed = 0_usize;
  for (index, boundary) in region.boundaries.iter().enumerate() {
    if index == path {
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
  tolerance: f64,
  flattened: &mut bool,
) -> Result<Vec<Contour>, GeomError> {
  let mut vertices: Vec<Vertex> = Vec::new();
  for (index, piece) in pieces.iter().enumerate() {
    if piece.length() <= 0.0 {
      return Err(GeomError::EditRefused {
        reason: "a surviving piece has no length",
      });
    }
    let start = vertex_index(&mut vertices, piece.start_point(), tolerance);
    let end = vertex_index(&mut vertices, piece.end_point(), tolerance);
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
    vertex.ends.sort_by(|a, b| angular_order(pieces, *a, *b));
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
fn vertex_index(vertices: &mut Vec<Vertex>, point: Point2, tolerance: f64) -> usize {
  if let Some(index) = vertices
    .iter()
    .position(|vertex| vertex.point.is_near(point, tolerance))
  {
    return index;
  }
  vertices.push(Vertex {
    point,
    ends: Vec::new(),
  });
  vertices.len() - 1
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

/// The direction a piece leaves a vertex in, as an angle in `(-pi, pi]`.
fn outward_angle(pieces: &[Segment], end: (usize, EndAt)) -> f64 {
  let segment = pieces[end.0];
  let direction = match (segment, end.1) {
    (Segment::Line(line), EndAt::Start) => line.direction(),
    (Segment::Line(line), EndAt::End) => line.reversed().direction(),
    (Segment::Arc(arc), EndAt::Start) => Some(arc.tangent_at_start()),
    (Segment::Arc(arc), EndAt::End) => Some(arc.reversed().tangent_at_start()),
  };
  direction.map(|vector: Vec2| vector.angle()).unwrap_or(0.0)
}

/// Ascending direction order at a vertex, with the piece order as the tie-break.
fn angular_order(pieces: &[Segment], a: (usize, EndAt), b: (usize, EndAt)) -> Ordering {
  outward_angle(pieces, a)
    .partial_cmp(&outward_angle(pieces, b))
    .unwrap_or(Ordering::Equal)
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
fn exactly(segment: Segment, tolerance: f64, flattened: &mut bool) -> Vec<Segment> {
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
    if window[0].is_near(window[1], 0.0) {
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
fn survives_the_bulge(arc: Arc, tolerance: f64) -> bool {
  let Ok(rebuilt) = Arc::from_endpoints_bulge(arc.start_point(), arc.end_point(), arc.bulge())
  else {
    return false;
  };
  let radius_error = (rebuilt.radius() - arc.radius()).abs();
  let sweep_error = (rebuilt.sweep() - arc.sweep()).abs() * arc.radius().max(1.0);
  radius_error <= tolerance && sweep_error <= tolerance
}
