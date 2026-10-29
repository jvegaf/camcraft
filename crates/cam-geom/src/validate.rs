//! Vector validation: is this contour cuttable, and if not, where exactly.
//!
//! The report a user needs is specific — which entity, which kind of problem, where — because a
//! profile cut through an open or self-crossing contour is a broken part, not a warning.
//!
//! Rules:
//!
//! * A closed contour is never "open". An open one reports the gap between its ends.
//! * Two **adjacent** segments share a vertex by construction; they are not intersected.
//! * Every other pair that meets in a point is reported, tangent contact included in a `Points`
//!   result but not as `Tangent` (a tangency does not divide the contour).
//! * A pair that overlaps completely is reported once, at the first segment's start.
//! * A segment shorter than the tolerance is reported as degenerate.

use crate::intersect::{intersect, Intersections};
use crate::{Bounds, Contour, Point2};

/// One place where a contour crosses or touches itself.
#[derive(Debug, Clone, PartialEq)]
pub struct SelfIntersection {
  /// Where the two segments meet.
  pub point: Point2,
  /// Index of the first segment in the contour.
  pub first: usize,
  /// Index of the second segment in the contour.
  pub second: usize,
}

/// Everything wrong with one contour.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ContourIssues {
  /// The gap between the ends of an open contour, in millimetres; `None` when it is closed.
  pub open: Option<f64>,
  /// Self-intersections, ordered by segment index and then by coordinate.
  pub intersections: Vec<SelfIntersection>,
  /// Indices of segments shorter than the tolerance.
  pub degenerate: Vec<usize>,
}

impl ContourIssues {
  /// True when nothing was reported.
  pub fn is_clean(&self) -> bool {
    self.open.is_none() && self.intersections.is_empty() && self.degenerate.is_empty()
  }

  /// How many problems were reported.
  pub fn count(&self) -> usize {
    usize::from(self.open.is_some()) + self.intersections.len() + self.degenerate.len()
  }
}

/// Inspect a contour for the problems that make it uncuttable.
pub fn inspect(contour: &Contour, tolerance: f64) -> ContourIssues {
  let segments = contour.segments();
  let mut issues = ContourIssues::default();
  if segments.is_empty() {
    return issues;
  }

  if !contour.is_closed() {
    let first = segments[0].start_point();
    let last = segments[segments.len() - 1].end_point();
    issues.open = Some(last.distance_to(first));
  }

  let closed = contour.is_closed();
  let bounds: Vec<Bounds> = segments.iter().map(|segment| segment.bounds()).collect();
  for (index, segment) in segments.iter().enumerate() {
    if segment.length() <= tolerance {
      issues.degenerate.push(index);
    }
  }

  for first in 0..segments.len() {
    for second in (first + 1)..segments.len() {
      if adjacent(first, second, segments.len(), closed) {
        continue;
      }
      if !overlaps(bounds[first], bounds[second], tolerance) {
        continue;
      }
      let mut points = match intersect(segments[first], segments[second], tolerance) {
        Intersections::Point(point) => vec![point],
        Intersections::Points(a, b) => vec![a, b],
        Intersections::Coincident => vec![segments[first].start_point()],
        // A tangency touches without dividing the contour, and `None` is the easy case.
        Intersections::Tangent(_) | Intersections::None => Vec::new(),
      };
      points.sort_by(|a, b| {
        a.x
          .partial_cmp(&b.x)
          .unwrap_or(std::cmp::Ordering::Equal)
          .then(a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
      });
      for point in points {
        issues.intersections.push(SelfIntersection {
          point,
          first,
          second,
        });
      }
    }
  }

  issues
}

/// True when the two segments share a vertex by construction.
fn adjacent(first: usize, second: usize, count: usize, closed: bool) -> bool {
  if second == first + 1 {
    return true;
  }
  closed && first == 0 && second + 1 == count
}

/// True when the two bounding boxes can touch within tolerance.
fn overlaps(first: Bounds, second: Bounds, tolerance: f64) -> bool {
  first.min.x - tolerance <= second.max.x
    && second.min.x - tolerance <= first.max.x
    && first.min.y - tolerance <= second.max.y
    && second.min.y - tolerance <= first.max.y
}
