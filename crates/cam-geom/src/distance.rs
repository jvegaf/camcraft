//! The exact minimum distance between two segments, with no sampling anywhere (D-RB-11).
//!
//! [`segment_to_segment`] answers one question — how far apart are these two pieces of curve — and
//! answers it from a **closed-form candidate set**, never by subdividing. That is what the offset
//! distance proof needs ([`crate::region_offset`]): a piece survives only when its *whole* self is
//! `distance - slack` away from an original boundary, and "the whole self" cannot be sampled into
//! existence. A chord between two crossings can cut across a bulge that none of its ends sees (K5
//! measured 17 mm against ends at 23 mm), so the minimum must be exact.
//!
//! # The candidate set, and why it is complete
//!
//! The minimum of the distance over a pair of curves is attained either at an **endpoint** of one of
//! them, or at a point **interior to both** where the connecting vector is perpendicular to both
//! curves' tangents. Those are the only two cases, and each pair of segment kinds turns the second
//! one into a small finite set:
//!
//! * **line / line** — perpendicular to both *straight* tangents means the two directions are
//!   parallel, and the distance between two parallel segments is attained at an end of the overlap of
//!   their projections, which is an endpoint of at least one of them. So the four endpoint-to-segment
//!   distances are the whole candidate set, plus zero when the segments cross at all (a crossing is
//!   the one place where the function is not differentiable and neither case applies).
//! * **line / arc** — perpendicular to the line's tangent makes the connecting vector parallel to the
//!   line's normal, and perpendicular to the arc's tangent makes it **radial**, so the point of the
//!   arc is one of the two points where the normal through the circle's centre leaves the circle,
//!   `C ± r·n`. Both are candidates when they lie on the sweep; the four endpoint distances cover
//!   everything else, and zero covers a crossing.
//! * **arc / arc** — radial for both, so the connecting vector lies on the **centre line** through
//!   `C1` and `C2`: the four choices of `C1 ± r1·u` against `C2 ± r2·u`, kept when both points lie on
//!   their own sweeps. Concentric arcs have no centre line direction to speak of, and they need none:
//!   if their sweeps share an angle then an endpoint of one lies inside the other's sweep and the
//!   endpoint distances already return `|r1 - r2|`. Zero covers a crossing.
//!
//! Every candidate is evaluated as a genuine distance to the *segment*, never to its supporting line
//! or circle, so a candidate whose foot falls past an end is still correct — it simply is not the
//! minimum.
//!
//! # Tolerance
//!
//! `tolerance` is not a resolution knob; there is no resolution here. It is the kernel's "these two
//! curves meet" scale (see [`crate::intersect`]), so a minimum within `tolerance` is reported as `0.0`
//! and the answer agrees with [`crate::intersect`] about whether the two segments meet. Pass `0.0` for
//! the unsnapped minimum. The closed forms themselves are exact either way.

use crate::arc::Arc;
use crate::point::Point2;
use crate::segment::{LineSeg, Segment};

use std::f64::consts::TAU;

/// Angular slack, in radians, for deciding that a direction lies within an arc's sweep.
///
/// It only ever decides whether a *candidate* is considered (an extra candidate can never make the
/// answer too small, since every candidate is a real distance), so it is a floating-point guard rather
/// than a modelling tolerance.
const ANGLE_EPSILON: f64 = 1.0e-12;

/// The exact minimum distance between two segments, in millimetres (D-RB-11).
///
/// Exact for every pair the kernel has — line/line, line/arc, arc/line, arc/arc — and **symmetric**:
/// `segment_to_segment(first, second, t)` equals `segment_to_segment(second, first, t)`. The value is
/// `0.0` when the two segments meet (a crossing, a tangency, a shared endpoint or a coincidence) and
/// when the exact minimum is within `tolerance` of it; otherwise it is that exact minimum. See the
/// module documentation for the candidate set and why it is complete.
///
/// The distance is measured to the segments, not to the lines and circles that carry them: two
/// collinear segments that stop 10 mm short of each other are 10 mm apart, and an arc's nearest point
/// being an endpoint is respected because [`Arc::distance_to_point`] and [`LineSeg::distance_to_point`]
/// are themselves exact.
pub fn segment_to_segment(
  first: Segment,
  second: Segment,
  tolerance: crate::TolerancePolicy,
) -> f64 {
  let value = match (first, second) {
    (Segment::Line(first), Segment::Line(second)) => line_line(first, second),
    (Segment::Line(line), Segment::Arc(arc)) | (Segment::Arc(arc), Segment::Line(line)) => {
      line_arc(line, arc)
    }
    (Segment::Arc(first), Segment::Arc(second)) => arc_arc(first, second),
  };
  let eps = tolerance.epsilon.max(0.0);
  if value <= eps {
    0.0
  } else {
    value
  }
}

/// Two straight segments: an endpoint wins unless they cross.
///
/// Parallel segments are the case where a perpendicular foot can be interior to both, and there the
/// minimum is attained over a whole interval of the overlap, so an endpoint of the overlap — an
/// endpoint of one of the segments — is always a minimum. Crossing segments have no common normal to
/// find, which is why [`lines_meet`] is asked first.
fn line_line(first: LineSeg, second: LineSeg) -> f64 {
  if lines_meet(first, second) {
    return 0.0;
  }
  first
    .distance_to_point(second.start)
    .min(first.distance_to_point(second.end))
    .min(second.distance_to_point(first.start))
    .min(second.distance_to_point(first.end))
}

/// Whether two straight segments share at least one point.
///
/// Exact (no tolerance): the two segments cross when their endpoints lie on opposite sides of each
/// other's supporting line, and they also meet whenever an endpoint lies on the other segment, which
/// covers touching, collinear overlap and degenerate zero-length input. All four orientations are
/// needed: `o1`/`o2` alone accept a collinear pair that misses, and `o3`/`o4` alone accept a T that
/// does not reach.
fn lines_meet(first: LineSeg, second: LineSeg) -> bool {
  let o1 = side(first, second.start);
  let o2 = side(first, second.end);
  let o3 = side(second, first.start);
  let o4 = side(second, first.end);
  let straddle = |a: f64, b: f64| (a > 0.0 && b < 0.0) || (a < 0.0 && b > 0.0);
  if straddle(o1, o2) && straddle(o3, o4) {
    return true;
  }
  (o1 == 0.0 && on_segment(first, second.start))
    || (o2 == 0.0 && on_segment(first, second.end))
    || (o3 == 0.0 && on_segment(second, first.start))
    || (o4 == 0.0 && on_segment(second, first.end))
}

/// Which side of `line`'s supporting line `point` lies on; zero when it is on the line.
fn side(line: LineSeg, point: Point2) -> f64 {
  (line.end - line.start).cross(point - line.start)
}

/// Whether `point`, already known to be collinear with `line`, lies within the segment.
///
/// A zero-length segment has `min == max` on both axes, so this degenerates to "is the point that
/// point", which is the answer that case wants.
fn on_segment(line: LineSeg, point: Point2) -> bool {
  point.x >= line.start.x.min(line.end.x)
    && point.x <= line.start.x.max(line.end.x)
    && point.y >= line.start.y.min(line.end.y)
    && point.y <= line.start.y.max(line.end.y)
}

/// A straight segment and a circular arc.
///
/// The interior-of-both case is the arc point whose radius is parallel to the line's normal, so the
/// two points `C ± r·n` are the only extra candidates; everything else is an endpoint of one of the
/// two curves. Each candidate is measured with the *segment*'s own exact distance, so a normal foot
/// that falls beyond the line's end is handled without a special case.
fn line_arc(line: LineSeg, arc: Arc) -> f64 {
  if line_meets_arc(line, arc) {
    return 0.0;
  }
  let mut best = line
    .distance_to_point(arc.start_point())
    .min(line.distance_to_point(arc.end_point()))
    .min(arc.distance_to_point(line.start))
    .min(arc.distance_to_point(line.end));
  if let Some(direction) = line.direction() {
    let normal = direction.perp_ccw();
    for sign in [1.0_f64, -1.0] {
      let radial = normal * (arc.radius() * sign);
      // The point is on the circle by construction; only its angle decides whether it is on the arc.
      if arc_covers_angle(arc, radial.angle()) {
        best = best.min(line.distance_to_point(arc.center() + radial));
      }
    }
  }
  best
}

/// Whether a straight segment and an arc share at least one point.
///
/// Exact: the arc meets the segment where the circle crosses the segment's supporting line, which is
/// the perpendicular foot from the centre advanced by `±√(r² − k²)`, kept when the parameter is inside
/// the segment and the point's angle is inside the sweep. When the line misses the circle there is
/// nothing to check, and a zero-length "line" is a point, which is decided against the arc directly.
fn line_meets_arc(line: LineSeg, arc: Arc) -> bool {
  let Some(direction) = line.direction() else {
    return point_on_arc(arc, line.start);
  };
  let length = line.length();
  // Signed distance along the line from its start to the foot of the centre.
  let along = (arc.center() - line.start).dot(direction);
  let foot = line.start + direction * along;
  let separation = foot.distance_to(arc.center());
  let radius = arc.radius();
  if separation > radius {
    return false;
  }
  let half = (radius * radius - separation * separation).max(0.0).sqrt();
  let slack = length.max(radius).max(1.0) * 1.0e-12;
  for offset in [-half, half] {
    let parameter = along + offset;
    if parameter < -slack || parameter > length + slack {
      continue;
    }
    let point = line.start + direction * parameter;
    if arc_covers_angle(arc, (point - arc.center()).angle()) {
      return true;
    }
  }
  false
}

/// Two circular arcs.
///
/// The interior-of-both case puts both points on the line through the two centres, so the four sign
/// combinations of `C1 ± r1·u` against `C2 ± r2·u` are the extra candidates. Concentric arcs add
/// nothing: with `C1 == C2` the radial direction is free, and if the two sweeps share an angle then an
/// endpoint of one lies inside the other, where [`Arc::distance_to_point`] already answers `|r1 − r2|`.
fn arc_arc(first: Arc, second: Arc) -> f64 {
  if arcs_meet(first, second) {
    return 0.0;
  }
  let mut best = second
    .distance_to_point(first.start_point())
    .min(second.distance_to_point(first.end_point()))
    .min(first.distance_to_point(second.start_point()))
    .min(first.distance_to_point(second.end_point()));
  let Some(unit) = (second.center() - first.center()).normalized() else {
    return best;
  };
  for outward in [1.0_f64, -1.0] {
    for inward in [1.0_f64, -1.0] {
      let on_first = first.center() + unit * (first.radius() * outward);
      let on_second = second.center() + unit * (second.radius() * inward);
      if arc_covers_angle(first, (on_first - first.center()).angle())
        && arc_covers_angle(second, (on_second - second.center()).angle())
      {
        best = best.min(on_first.distance_to(on_second));
      }
    }
  }
  best
}

/// Whether two arcs share at least one point.
///
/// Exact: the two circles meet at the standard `base ± perpendicular·height` pair, kept when both of
/// their sweeps cover them. Coincident circles are decided by their sweeps instead, because every
/// point of one is on the other's circle and the question is only whether they overlap in angle.
fn arcs_meet(first: Arc, second: Arc) -> bool {
  let (r1, r2) = (first.radius(), second.radius());
  let between = second.center() - first.center();
  let distance = between.length();
  let slack = (r1 + r2 + distance).max(1.0) * 1.0e-12;
  if distance <= slack {
    if (r1 - r2).abs() > slack {
      return false;
    }
    return sweeps_share_angle(first, second);
  }
  if distance > r1 + r2 + slack || distance < (r1 - r2).abs() - slack {
    return false;
  }
  let unit = between * (1.0 / distance);
  let along = (distance * distance + r1 * r1 - r2 * r2) / (2.0 * distance);
  let height = (r1 * r1 - along * along).max(0.0).sqrt();
  let base = first.center() + unit * along;
  let perpendicular = unit.perp_ccw();
  // A tangency gives `height == 0` and the two candidates coincide, so one check covers it.
  [base + perpendicular * height, base - perpendicular * height]
    .into_iter()
    .any(|point| point_on_arc(first, point) && point_on_arc(second, point))
}

/// Whether two arcs on the **same** circle overlap in angle.
///
/// Two arcs are contiguous angle intervals, so if they share an angle then an endpoint of one of them
/// lies inside the other — which is exactly what this checks, and what makes concentric arcs safe
/// without a centre line.
fn sweeps_share_angle(first: Arc, second: Arc) -> bool {
  [first.start_angle(), first.start_angle() + first.sweep()]
    .into_iter()
    .any(|angle| arc_covers_angle(second, angle))
    || [second.start_angle(), second.start_angle() + second.sweep()]
      .into_iter()
      .any(|angle| arc_covers_angle(first, angle))
}

/// Whether `point` lies on `arc`: on its circle and within its sweep.
fn point_on_arc(arc: Arc, point: Point2) -> bool {
  let offset = point - arc.center();
  let radial = arc.radius().max(1.0) * 1.0e-12;
  if (offset.length() - arc.radius()).abs() > radial {
    return false;
  }
  arc_covers_angle(arc, offset.angle())
}

/// Whether `angle` lies within `arc`'s sweep, measured in the sweep's direction.
///
/// The same rule [`Arc::sweep`] means: a positive sweep turns counter-clockwise from
/// [`Arc::start_angle`] and a negative one clockwise, and the comparison leans outwards by
/// [`ANGLE_EPSILON`] so a turning point a hair outside the sweep still counts (an extra candidate can
/// only be a real distance, so erring outwards can never make the answer too small).
fn arc_covers_angle(arc: Arc, angle: f64) -> bool {
  let sweep = arc.sweep();
  if sweep == 0.0 {
    return false;
  }
  let delta = if sweep > 0.0 {
    wrap_turn(angle - arc.start_angle())
  } else {
    wrap_turn(arc.start_angle() - angle)
  };
  delta <= sweep.abs() + ANGLE_EPSILON
}

/// `angle` reduced to `[0, TAU)`.
fn wrap_turn(angle: f64) -> f64 {
  let wrapped: f64 = angle % TAU;
  if wrapped < 0.0 {
    wrapped + TAU
  } else {
    wrapped
  }
}
