//! Segment intersections, including the cases a naive solver drops.
//!
//! Trim is the first consumer, and trim is only correct if the caller can tell a crossing from a
//! tangency and a coincidence from a miss: a tangent point is not a place where a contour can be cut
//! in two, and two nearly-equal roots of a tangent are not two cut points.
//!
//! Everything here is tolerance-driven, because "do these two curves meet" is a modelling question,
//! not an exact-arithmetic one.

use crate::arc::Arc;
use crate::point::{Point2, Vec2};
use crate::segment::{LineSeg, Segment};

use std::f64::consts::{PI, TAU};

/// How two segments meet.
#[derive(Clone, Debug, PartialEq)]
pub enum Intersections {
  /// The segments do not meet within the tolerance.
  None,
  /// They cross at exactly one point: a transverse crossing, including a T-junction.
  Point(Point2),
  /// They cross at two distinct points, ordered along the **first** argument.
  Points(Point2, Point2),
  /// They touch at one point without crossing: a tangent line on a circle, or two tangent circles.
  Tangent(Point2),
  /// They overlap along a non-zero length, so there is no single intersection to speak of.
  Coincident,
}

impl Intersections {
  /// The intersection points, in order. Empty for [`Intersections::None`] and for a coincidence,
  /// which is a whole segment rather than a point.
  pub fn points(&self) -> Vec<Point2> {
    match self {
      Self::None | Self::Coincident => Vec::new(),
      Self::Point(point) | Self::Tangent(point) => vec![*point],
      Self::Points(first, second) => vec![*first, *second],
    }
  }

  /// True when the segments do not meet at all.
  pub fn is_empty(&self) -> bool {
    matches!(self, Self::None)
  }

  /// True when the two segments share more than a point.
  pub fn is_coincident(&self) -> bool {
    matches!(self, Self::Coincident)
  }

  /// True when the meeting point is a touch rather than a crossing, which cannot divide a contour.
  pub fn is_tangent(&self) -> bool {
    matches!(self, Self::Tangent(_))
  }
}

/// Intersect two segments within `tolerance` millimetres.
pub fn intersect(first: Segment, second: Segment, tolerance: f64) -> Intersections {
  match (first, second) {
    (Segment::Line(first), Segment::Line(second)) => line_line(first, second, tolerance),
    (Segment::Line(line), Segment::Arc(arc)) => line_arc(line, arc, tolerance),
    (Segment::Arc(arc), Segment::Line(line)) => {
      // The arc/line case is solved with the line first; the result does not depend on order.
      line_arc(line, arc, tolerance)
    }
    (Segment::Arc(first), Segment::Arc(second)) => arc_arc(first, second, tolerance),
  }
}

fn line_line(first: LineSeg, second: LineSeg, tolerance: f64) -> Intersections {
  let direction = second.end - second.start;
  let own = first.end - first.start;
  let own_length = own.length();
  let direction_length = direction.length();
  if own_length == 0.0 || direction_length == 0.0 {
    return degenerate_line(first, second, tolerance);
  }

  let denominator = own.cross(direction);
  let base = second.start - first.start;

  // Parallel when the perpendicular distance between the lines is within tolerance, so the test has
  // the units of a distance rather than of a cross product.
  if denominator.abs() <= tolerance * own_length {
    if direction.cross(base).abs() > tolerance * direction_length {
      return Intersections::None;
    }
    return collinear_overlap(first, second, tolerance);
  }

  let t = base.cross(direction) / denominator;
  let u = base.cross(own) / denominator;
  let slack_t = tolerance / own_length;
  let slack_u = tolerance / direction_length;
  let inside = |parameter: f64, slack: f64| parameter >= -slack && parameter <= 1.0 + slack;
  if inside(t, slack_t) && inside(u, slack_u) {
    return Intersections::Point(first.start + own * t);
  }
  Intersections::None
}

/// Two collinear segments: overlap, touch at an endpoint, or miss.
fn collinear_overlap(first: LineSeg, second: LineSeg, tolerance: f64) -> Intersections {
  let direction = first.end - first.start;
  let length = direction.length();
  if length == 0.0 {
    return degenerate_line(first, second, tolerance);
  }
  let unit = direction * (1.0 / length);
  let project = |point: Point2| (point - first.start).dot(unit);
  let (a0, a1): (f64, f64) = (0.0, length);
  let (b0, b1) = {
    let start = project(second.start);
    let end = project(second.end);
    if start <= end {
      (start, end)
    } else {
      (end, start)
    }
  };
  let overlap = a1.min(b1) - a0.max(b0);
  if overlap > tolerance {
    Intersections::Coincident
  } else if overlap.abs() <= tolerance {
    let meeting = first.start + unit * a0.max(b0);
    if meeting.distance_to(second.start) <= tolerance
      || meeting.distance_to(second.end) <= tolerance
      || meeting.distance_to(first.start) <= tolerance
      || meeting.distance_to(first.end) <= tolerance
    {
      Intersections::Point(meeting)
    } else {
      Intersections::None
    }
  } else {
    Intersections::None
  }
}

fn degenerate_line(line: LineSeg, other: LineSeg, tolerance: f64) -> Intersections {
  let point = line.start;
  if other.distance_to_point(point) <= tolerance {
    Intersections::Point(point)
  } else {
    Intersections::None
  }
}

fn line_arc(line: LineSeg, arc: Arc, tolerance: f64) -> Intersections {
  let direction = line.end - line.start;
  let length = direction.length();
  if length == 0.0 {
    let distance = arc.distance_to_point(line.start);
    return if distance <= tolerance {
      Intersections::Point(line.start)
    } else {
      Intersections::None
    };
  }
  let unit = direction * (1.0 / length);
  let to_center = arc.center() - line.start;
  let along = to_center.dot(unit);
  let closest = line.start + unit * along;
  let distance = closest.distance_to(arc.center());
  let radius = arc.radius();

  if (distance - radius).abs() <= tolerance {
    return if param_inside(along, length, tolerance) && arc_contains_angle(arc, closest, tolerance)
    {
      Intersections::Tangent(closest)
    } else {
      Intersections::None
    };
  }
  if distance > radius {
    return Intersections::None;
  }

  let half = (radius * radius - distance * distance).max(0.0).sqrt();
  let mut candidates: Vec<(f64, Point2)> = Vec::with_capacity(2);
  for offset in [-half, half] {
    let parameter = along + offset;
    if !param_inside(parameter, length, tolerance) {
      continue;
    }
    let point = line.start + unit * parameter;
    if arc_contains_angle(arc, point, tolerance) {
      candidates.push((parameter, point));
    }
  }
  candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

  match candidates.len() {
    0 => Intersections::None,
    1 => Intersections::Point(candidates[0].1),
    _ => {
      let (first, second) = (candidates[0].1, candidates[1].1);
      if first.distance_to(second) <= tolerance {
        Intersections::Point(first)
      } else {
        Intersections::Points(first, second)
      }
    }
  }
}

fn arc_arc(first: Arc, second: Arc, tolerance: f64) -> Intersections {
  let (r1, r2) = (first.radius(), second.radius());
  let between = second.center() - first.center();
  let distance = between.length();

  if distance <= tolerance {
    if (r1 - r2).abs() > tolerance {
      return Intersections::None;
    }
    // One circle: the answer is entirely a question about the two sweeps.
    return one_circle(first, second, tolerance);
  }
  let unit: Vec2 = between * (1.0 / distance);

  let external = (distance - (r1 + r2)).abs() <= tolerance;
  let internal = (distance - (r1 - r2).abs()).abs() <= tolerance;
  if external || internal {
    let sign = if external || r1 > r2 { 1.0 } else { -1.0 };
    let point = first.center() + unit * (r1 * sign);
    return if arc_contains_angle(first, point, tolerance)
      && arc_contains_angle(second, point, tolerance)
    {
      Intersections::Tangent(point)
    } else {
      Intersections::None
    };
  }
  if distance > r1 + r2 || distance < (r1 - r2).abs() {
    return Intersections::None;
  }

  let along = (distance * distance + r1 * r1 - r2 * r2) / (2.0 * distance);
  let height = (r1 * r1 - along * along).max(0.0).sqrt();
  let base = first.center() + unit * along;
  let perpendicular = unit.perp_ccw();
  let candidates = [base + perpendicular * height, base - perpendicular * height];

  let mut points: Vec<Point2> = candidates
    .into_iter()
    .filter(|point| {
      arc_contains_angle(first, *point, tolerance) && arc_contains_angle(second, *point, tolerance)
    })
    .collect();
  match points.len() {
    0 => Intersections::None,
    1 => Intersections::Point(points[0]),
    _ => {
      let (a, b) = (points[0], points[1]);
      if a.distance_to(b) <= tolerance {
        Intersections::Point(a)
      } else {
        // Ordered along the first arc's sweep, so callers can walk it consistently.
        points.sort_by(|a, b| {
          arc_fraction(first, *a)
            .partial_cmp(&arc_fraction(first, *b))
            .unwrap_or(std::cmp::Ordering::Equal)
        });
        Intersections::Points(points[0], points[1])
      }
    }
  }
}

/// Two arcs whose supporting circles coincide, decided by their sweeps.
///
/// The pair meets exactly where the sweeps do, so [`Intersections::Coincident`] is the answer only
/// when they share a length of curve: a pair that touches at a single angle meets at one point, and
/// a pair that shares no angle does not meet at all. Deciding this from the two centres and radii
/// answers for the *circles*, which is not the question an arc asks.
fn one_circle(first: Arc, second: Arc, tolerance: f64) -> Intersections {
  let radius = first.radius();
  let angular = if radius > 0.0 {
    tolerance / radius + 1.0e-12
  } else {
    1.0e-12
  };
  let mut shared = 0.0;
  let mut touch: Option<f64> = None;
  for (first_start, first_end) in coverage(first) {
    for (second_start, second_end) in coverage(second) {
      let overlap = first_end.min(second_end) - first_start.max(second_start);
      if overlap > angular {
        shared += overlap;
      } else if angle_apart(first_end, second_start) <= angular {
        touch = touch.or(Some(first_end));
      } else if angle_apart(second_end, first_start) <= angular {
        touch = touch.or(Some(second_end));
      }
    }
  }
  if shared > angular {
    return Intersections::Coincident;
  }
  match touch {
    Some(angle) => Intersections::Point(Point2::new(
      first.center().x + radius * angle.cos(),
      first.center().y + radius * angle.sin(),
    )),
    None => Intersections::None,
  }
}

/// An arc's sweep as intervals of absolute angle in `[0, TAU]`, split where it crosses the seam. A
/// full turn is the whole circle and needs no split.
fn coverage(arc: Arc) -> Vec<(f64, f64)> {
  let sweep = arc.sweep();
  if sweep.abs() >= TAU {
    return vec![(0.0, TAU)];
  }
  let start = arc.start_angle().rem_euclid(TAU);
  // A negative sweep runs *backwards* from the start angle, so the angles it covers are the interval
  // that ends there rather than the one that begins there. Reading it as positive would answer for a
  // quarter of the circle the arc never visits.
  let (low, high) = if sweep >= 0.0 {
    (start, start + sweep)
  } else {
    (start + sweep, start)
  };
  if low < 0.0 {
    vec![(0.0, high), (low + TAU, TAU)]
  } else if high > TAU {
    vec![(low, TAU), (0.0, high - TAU)]
  } else {
    vec![(low, high)]
  }
}

/// The shortest unsigned angle between two angles, in radians, in `[0, PI]`.
fn angle_apart(first: f64, second: f64) -> f64 {
  ((second - first + PI).rem_euclid(TAU) - PI).abs()
}

fn param_inside(parameter: f64, length: f64, tolerance: f64) -> bool {
  let slack = if length > 0.0 {
    tolerance / length
  } else {
    0.0
  };
  parameter >= -slack && parameter <= length + slack
}

/// Whether `point` lies on `arc`'s own sweep, within `tolerance` millimetres of its radius.
fn arc_contains_angle(arc: Arc, point: Point2, tolerance: f64) -> bool {
  let radius = arc.radius();
  if (point.distance_to(arc.center()) - radius).abs() > tolerance {
    return false;
  }
  let angular = if radius > 0.0 {
    tolerance / radius + 1.0e-12
  } else {
    1.0e-12
  };
  let angle = (point - arc.center()).angle();
  let sweep = arc.sweep();
  let (start, span) = if sweep >= 0.0 {
    (arc.start_angle(), sweep)
  } else {
    (arc.start_angle(), -sweep)
  };
  let forward = if sweep >= 0.0 {
    (angle - start).rem_euclid(TAU)
  } else {
    (start - angle).rem_euclid(TAU)
  };
  forward <= span + angular || (TAU - forward) <= angular
}

/// Where `point` falls along `arc`, as a fraction of its sweep, for deterministic ordering.
fn arc_fraction(arc: Arc, point: Point2) -> f64 {
  let angle = (point - arc.center()).angle();
  let sweep = arc.sweep();
  let travelled = if sweep >= 0.0 {
    (angle - arc.start_angle()).rem_euclid(TAU)
  } else {
    (arc.start_angle() - angle).rem_euclid(TAU)
  };
  if sweep == 0.0 {
    0.0
  } else {
    travelled / sweep.abs()
  }
}
