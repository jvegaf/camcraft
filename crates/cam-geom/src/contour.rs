//! Bulged contours: the authored shape of every vector in a document.

use crate::arc::Arc;
use crate::point::{Bounds, Point2};
use crate::segment::{LineSeg, Segment};
use crate::GeomError;

/// A contour vertex: a point plus the bulge of the segment leaving it.
///
/// `bulge == 0.0` leaves a straight segment; otherwise the segment is the arc with
/// `sweep = 4 * atan(bulge)`, positive counter-clockwise (DXF convention).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vertex {
  /// The vertex position, in millimetres.
  pub point: Point2,
  /// Bulge of the segment that leaves this vertex.
  pub bulge: f64,
}

impl Vertex {
  /// Build a vertex.
  pub const fn new(point: Point2, bulge: f64) -> Self {
    Self { point, bulge }
  }

  /// Build a sharp corner vertex.
  pub const fn corner(point: Point2) -> Self {
    Self::new(point, 0.0)
  }
}

/// An ordered, optionally closed sequence of straight and circular segments.
///
/// Closedness is stored explicitly rather than inferred from duplicated endpoints, so that a
/// contour never carries a phantom zero-length segment that later shows up as a stray move in the
/// G-code.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Contour {
  vertices: Vec<Vertex>,
  closed: bool,
}

impl Contour {
  /// Build a contour, validating every bulge against its following vertex.
  pub fn new(vertices: Vec<Vertex>, closed: bool) -> Result<Self, GeomError> {
    if vertices.len() < 2 {
      return Err(GeomError::ContourTooShort {
        vertices: vertices.len(),
      });
    }
    let contour = Self { vertices, closed };
    let n = contour.vertices.len();
    for index in 0..n {
      let from = contour.vertices[index];
      let to = if index + 1 < n {
        contour.vertices[index + 1]
      } else if closed {
        contour.vertices[0]
      } else {
        break;
      };
      if from.point.is_near(to.point, 0.0) {
        return Err(GeomError::DegenerateArc {
          reason: "consecutive vertices coincide",
        });
      }
      if from.bulge.abs() > f64::EPSILON {
        Arc::from_endpoints_bulge(from.point, to.point, from.bulge)?;
      }
    }
    Ok(contour)
  }

  /// Build a contour of straight segments from points.
  pub fn from_points(points: &[Point2], closed: bool) -> Result<Self, GeomError> {
    let vertices = points.iter().copied().map(Vertex::corner).collect();
    Self::new(vertices, closed)
  }

  /// Build a contour from an ordered chain of segments.
  pub fn from_segments(segments: &[Segment], closed: bool) -> Result<Self, GeomError> {
    let mut vertices: Vec<Vertex> = segments
      .iter()
      .map(|segment| Vertex::new(segment.start_point(), segment.bulge()))
      .collect();
    if !closed {
      if let Some(last) = segments.last() {
        vertices.push(Vertex::corner(last.end_point()));
      }
    }
    Self::new(vertices, closed)
  }

  /// The vertices, in order.
  pub fn vertices(&self) -> &[Vertex] {
    &self.vertices
  }

  /// True when the contour closes on itself.
  pub fn is_closed(&self) -> bool {
    self.closed
  }

  /// Number of vertices.
  pub fn len(&self) -> usize {
    self.vertices.len()
  }

  /// True when the contour has no vertices.
  pub fn is_empty(&self) -> bool {
    self.vertices.is_empty()
  }

  /// The ordered segments of the contour.
  pub fn segments(&self) -> Vec<Segment> {
    self.raw_segments()
  }

  /// True when the contour has at least one arc.
  pub fn has_arcs(&self) -> bool {
    self
      .vertices
      .iter()
      .any(|vertex| vertex.bulge.abs() > f64::EPSILON)
  }

  /// Signed area: positive counter-clockwise, exact for arcs (Green's theorem).
  pub fn signed_area(&self) -> f64 {
    self
      .raw_segments()
      .iter()
      .map(|segment| segment.area_contribution())
      .sum()
  }

  /// True when the contour winds counter-clockwise.
  pub fn is_ccw(&self) -> bool {
    self.signed_area() > 0.0
  }

  /// Total length along the contour.
  pub fn perimeter(&self) -> f64 {
    self
      .raw_segments()
      .iter()
      .map(|segment| segment.length())
      .sum()
  }

  /// Sample the contour into points, including arcs at `tolerance` chord error.
  pub fn tessellate(&self, tolerance: f64) -> Vec<Point2> {
    let mut points = Vec::new();
    if let Some(first) = self.vertices.first() {
      points.push(first.point);
    }
    for segment in self.raw_segments() {
      segment.tessellate_into(tolerance, &mut points);
    }
    dedup_near(&mut points, tolerance);
    points
  }

  /// The same contour travelled in the opposite direction, with arc bulges negated.
  pub fn reverse(&self) -> Self {
    let n = self.vertices.len();
    let mut vertices = Vec::with_capacity(n);
    for j in 0..n {
      let point = self.vertices[n - 1 - j].point;
      let bulge = if self.closed {
        -self.vertices[(2 * n - 2 - j) % n].bulge
      } else if j + 1 < n {
        -self.vertices[n - 2 - j].bulge
      } else {
        0.0
      };
      vertices.push(Vertex::new(point, bulge));
    }
    Self {
      vertices,
      closed: self.closed,
    }
  }

  /// Bounds of the contour.
  pub fn bounds(&self) -> Bounds {
    Bounds::from_points(&self.tessellate(crate::DEFAULT_TOLERANCE))
  }

  /// Distance between the last vertex and the first, if both exist.
  pub fn closing_distance(&self) -> Option<f64> {
    let first = self.vertices.first()?;
    let last = self.vertices.last()?;
    Some(first.point.distance_to(last.point))
  }

  /// The segments implied by the vertex list.
  fn raw_segments(&self) -> Vec<Segment> {
    let n = self.vertices.len();
    let mut segments = Vec::with_capacity(n);
    for index in 0..n {
      let from = self.vertices[index];
      let to = if index + 1 < n {
        self.vertices[index + 1]
      } else if self.closed {
        self.vertices[0]
      } else {
        break;
      };
      segments.push(segment_between(from, to));
    }
    segments
  }
}

/// The segment implied by a pair of vertices.
fn segment_between(from: Vertex, to: Vertex) -> Segment {
  if from.bulge.abs() <= f64::EPSILON {
    Segment::Line(LineSeg::new(from.point, to.point))
  } else {
    match Arc::from_endpoints_bulge(from.point, to.point, from.bulge) {
      Ok(arc) => Segment::Arc(arc),
      Err(_) => Segment::Line(LineSeg::new(from.point, to.point)),
    }
  }
}

/// Remove consecutive duplicate samples closer than `tolerance`.
fn dedup_near(points: &mut Vec<Point2>, tolerance: f64) {
  let mut write = 0;
  for read in 0..points.len() {
    if write == 0 || !points[read].is_near(points[write - 1], tolerance) {
      points[write] = points[read];
      write += 1;
    }
  }
  points.truncate(write);
}
