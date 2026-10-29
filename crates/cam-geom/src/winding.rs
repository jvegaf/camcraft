//! Point membership in a closed contour: the winding number, exact for arcs.
//!
//! Every later boolean asks this question — *is this point inside that contour* — so the answer has to
//! be geometric, not sampled. Tessellating the contour and counting crossings of the polygons would
//! make an inside/outside decision depend on a chord tolerance, and a point a nanometre inside a large
//! arc would flip. Here each segment contributes the angle it subtends at the query point: a line
//! contributes the angle between the vectors to its ends, an arc contributes its own sweep (when the
//! point is inside the arc's circle) or the angle between the vectors to its ends, cut at the tangency
//! points where the visual angle stops being monotone.
//!
//! A point **on** the boundary has no meaningful winding, which is why membership is reported through
//! [`Membership`] with an explicit `OnBoundary` case: two contours that share an edge must never have
//! that edge counted as interior to both, or a boolean produces a doubled edge.

use std::cmp::Ordering;
use std::f64::consts::{PI, TAU};

use crate::arc::Arc;
use crate::contour::Contour;
use crate::point::{Point2, Vec2};
use crate::segment::Segment;

/// Where a point sits relative to a closed contour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Membership {
  /// The point encloses no material.
  Outside,
  /// The point is within tolerance of the contour itself.
  OnBoundary,
  /// The point is enclosed by the contour.
  Inside,
}

/// The winding number of a closed contour about `point`: `+1` counter-clockwise, `-1` clockwise, `0`
/// outside.
///
/// Exact for arcs, never sampled. An open contour encloses nothing and returns `0`. A point that lies
/// exactly on the contour has no meaningful winding; callers decide membership with [`classify`],
/// which answers that case explicitly.
pub fn winding_number(contour: &Contour, point: Point2) -> i32 {
  if !contour.is_closed() {
    return 0;
  }
  let total: f64 = contour
    .segments()
    .iter()
    .map(|segment| contribution(*segment, point))
    .sum();
  (total / TAU).round() as i32
}

/// Whether `point` is inside, outside or on a closed contour, within `tolerance` millimetres.
///
/// The boundary test runs first and is a real distance, so a point within `tolerance` of any segment
/// is reported as `OnBoundary` rather than being assigned to one side by rounding.
pub fn classify(contour: &Contour, point: Point2, tolerance: f64) -> Membership {
  let nearest = contour
    .segments()
    .iter()
    .map(|segment| segment.distance_to_point(point))
    .fold(f64::INFINITY, f64::min);
  if nearest <= tolerance {
    return Membership::OnBoundary;
  }
  if winding_number(contour, point) == 0 {
    Membership::Outside
  } else {
    Membership::Inside
  }
}

/// The angle one segment subtends at `point`, in radians, signed by the direction of travel.
fn contribution(segment: Segment, point: Point2) -> f64 {
  match segment {
    Segment::Line(line) => subtended(line.start - point, line.end - point),
    Segment::Arc(arc) => arc_contribution(arc, point),
  }
}

/// The signed angle from `from` to `to`, in `(-pi, pi]`. Exact for a straight segment, whose argument
/// never spans half a turn as seen from a point that is not on it.
fn subtended(from: Vec2, to: Vec2) -> f64 {
  if from.length_squared() == 0.0 || to.length_squared() == 0.0 {
    // The query point is one of the ends: the angle is undefined, and `classify` owns that case.
    return 0.0;
  }
  from.cross(to).atan2(from.dot(to))
}

/// The angle a circular arc subtends at `point`.
///
/// An arc can wrap more than half a turn around the query point — the far side of a circle seen from
/// outside does exactly that — so the difference of the two endpoint directions is not enough, and
/// taking the shorter way silently reports the complement with the wrong sign. The arc is therefore
/// cut where it crosses the horizontal line through the point: inside one of the two half planes the
/// direction to the arc stays within a half turn, and the piece's angle is the difference of its
/// endpoints' directions *lifted into that half plane's branch*, which is exact and needs no
/// sampling and no tangency special case.
fn arc_contribution(arc: Arc, point: Point2) -> f64 {
  let centre = arc.center();
  let radius = arc.radius();
  let height = point.y - centre.y;

  let mut cuts = vec![0.0_f64, 1.0];
  let remaining = radius * radius - height * height;
  if remaining > 0.0 {
    let half_chord = remaining.sqrt();
    for x in [centre.x - half_chord, centre.x + half_chord] {
      let angle = height.atan2(x - centre.x);
      if let Some(fraction) = fraction_at_angle(arc, angle) {
        // Only interior cuts split a piece; the end of the arc is already a boundary.
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
    total += piece_angle(start, end, middle.y < 0.0);
  }
  total
}

/// The angle swept along a piece that lies in one half plane through the query point.
///
/// `below` says which half plane, and it decides the branch: a piece under the query point sweeps
/// arguments in `[pi, 2pi]`, so a direction on the query point's own level *ahead* of it is `2pi`, not
/// `0`. Getting that one case wrong is what made a bulge that dips below its own chord read as
/// outside.
fn piece_angle(start: Vec2, end: Vec2, below: bool) -> f64 {
  branch_angle(end, below) - branch_angle(start, below)
}

/// One endpoint's direction, carried into the branch its half plane uses.
///
/// Whether the endpoint sits *on* the cut line is decided by its distance from that line, never by the
/// sign of the `atan2` result: a point that is mathematically on the line comes back as `+5.5e-17` as
/// often as `-0.0`, and choosing the branch from that sign puts the piece `2*pi` away from where it
/// belongs.
fn branch_angle(vector: Vec2, below: bool) -> f64 {
  if vector.length_squared() == 0.0 {
    return 0.0;
  }
  let on_line = vector.y.abs() <= 1.0e-9 * vector.length();
  if on_line {
    return match (below, vector.x > 0.0) {
      (true, true) => TAU,
      (_, true) => 0.0,
      (_, false) => PI,
    };
  }
  if below && vector.y < 0.0 {
    // Under the line the continuous direction is in `(pi, 2pi)` while `atan2` reports `(-pi, 0)`.
    let angle = vector.angle();
    return if angle < 0.0 { angle + TAU } else { angle };
  }
  vector.angle()
}

/// Where along the arc (as a fraction of its sweep) an absolute angle is reached, if it is reached.
fn fraction_at_angle(arc: Arc, angle: f64) -> Option<f64> {
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
