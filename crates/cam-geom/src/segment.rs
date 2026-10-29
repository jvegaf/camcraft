//! Line segments and the uniform segment type that keeps arcs first-class.

use crate::arc::Arc;
use crate::point::{Bounds, Point2, Vec2};

/// A straight segment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineSeg {
  /// Start point.
  pub start: Point2,
  /// End point.
  pub end: Point2,
}

impl LineSeg {
  /// Build a segment.
  pub const fn new(start: Point2, end: Point2) -> Self {
    Self { start, end }
  }

  /// Length.
  pub fn length(self) -> f64 {
    self.start.distance_to(self.end)
  }

  /// Unit direction, or `None` for a zero-length segment.
  pub fn direction(self) -> Option<Vec2> {
    (self.end - self.start).normalized()
  }

  /// Point at parameter `t`, where 0 is the start and 1 the end.
  pub fn point_at(self, t: f64) -> Point2 {
    self.start.lerp(self.end, t)
  }

  /// The same segment travelled in the opposite direction.
  pub fn reversed(self) -> Self {
    Self {
      start: self.end,
      end: self.start,
    }
  }

  /// Sample the segment, including both endpoints.
  pub fn tessellate(self) -> Vec<Point2> {
    vec![self.start, self.end]
  }

  /// Signed area contribution around the origin.
  pub fn area_contribution(self) -> f64 {
    0.5 * (self.start.x * self.end.y - self.end.x * self.start.y)
  }

  /// Shortest distance from `point` to the segment.
  pub fn distance_to_point(self, point: Point2) -> f64 {
    let span = self.end - self.start;
    let length_squared = span.length_squared();
    if length_squared <= 0.0 {
      return point.distance_to(self.start);
    }
    let projection = ((point - self.start).dot(span) / length_squared).clamp(0.0, 1.0);
    point.distance_to(self.start + span * projection)
  }

  /// Bounds of the segment.
  pub fn bounds(self) -> Bounds {
    Bounds::from_points(&[self.start, self.end])
  }
}

/// One piece of a contour: either straight or circular.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Segment {
  /// A straight segment.
  Line(LineSeg),
  /// A circular arc.
  Arc(Arc),
}

impl Segment {
  /// The point where the segment starts.
  pub fn start_point(self) -> Point2 {
    match self {
      Self::Line(line) => line.start,
      Self::Arc(arc) => arc.start_point(),
    }
  }

  /// The point where the segment ends.
  pub fn end_point(self) -> Point2 {
    match self {
      Self::Line(line) => line.end,
      Self::Arc(arc) => arc.end_point(),
    }
  }

  /// Length along the segment.
  pub fn length(self) -> f64 {
    match self {
      Self::Line(line) => line.length(),
      Self::Arc(arc) => arc.length(),
    }
  }

  /// The same segment travelled in the opposite direction.
  pub fn reversed(self) -> Self {
    match self {
      Self::Line(line) => Self::Line(line.reversed()),
      Self::Arc(arc) => Self::Arc(arc.reversed()),
    }
  }

  /// Signed area contribution around the origin, exact for arcs.
  pub fn area_contribution(self) -> f64 {
    match self {
      Self::Line(line) => line.area_contribution(),
      Self::Arc(arc) => arc.area_contribution(),
    }
  }

  /// The DXF bulge of this segment: zero for a line, `tan(sweep / 4)` for an arc.
  pub fn bulge(self) -> f64 {
    match self {
      Self::Line(_) => 0.0,
      Self::Arc(arc) => arc.bulge(),
    }
  }

  /// The arc, if this segment is one.
  pub fn as_arc(self) -> Option<Arc> {
    match self {
      Self::Line(_) => None,
      Self::Arc(arc) => Some(arc),
    }
  }

  /// Bounds of the segment.
  pub fn bounds(self) -> Bounds {
    match self {
      Self::Line(line) => line.bounds(),
      Self::Arc(arc) => arc.bounds(),
    }
  }

  /// Shortest distance from `point` to the segment.
  pub fn distance_to_point(self, point: Point2) -> f64 {
    match self {
      Self::Line(line) => line.distance_to_point(point),
      Self::Arc(arc) => arc.distance_to_point(point),
    }
  }

  /// Append this segment's samples to `out`, skipping the duplicated start point.
  pub fn tessellate_into(self, tolerance: f64, out: &mut Vec<Point2>) {
    match self {
      Self::Line(line) => out.push(line.end),
      Self::Arc(arc) => {
        for point in arc.tessellate(tolerance).into_iter().skip(1) {
          out.push(point);
        }
      }
    }
  }
}
