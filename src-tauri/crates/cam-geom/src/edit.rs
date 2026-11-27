//! Node editing: moving, inserting and deleting the vertices of an authored contour.
//!
//! Insertion **projects the click onto the segment**, on a straight segment along the line and on an
//! arc along the circle, because that is what a drawing tool does and because the alternative —
//! trusting the raw click — would silently move the geometry. A projected point that falls outside
//! the segment is refused instead of being glued to an endpoint, which would create a zero-length
//! segment that only shows up much later as a stray move in the post-processed program.

use crate::arc::Arc;
use crate::contour::{Contour, Vertex};
use crate::point::Point2;
use crate::segment::Segment;
use crate::GeomError;

use std::f64::consts::TAU;

/// How close to an endpoint a projected insertion may land before it is refused as degenerate.
const ENDPOINT_EPSILON: f64 = 1.0e-9;

/// Move vertex `index` to `point`, leaving every bulge untouched.
///
/// # Errors
///
/// [`GeomError::EditRefused`] when the index does not exist, or when the new position would coincide
/// with a neighbour and produce a zero-length segment.
pub fn move_vertex(contour: &Contour, index: usize, point: Point2) -> Result<Contour, GeomError> {
  if index >= contour.len() {
    return Err(GeomError::EditRefused {
      reason: "the contour has no vertex at that index",
    });
  }
  let mut vertices = contour.vertices().to_vec();
  vertices[index].point = point;
  rebuild(contour, vertices).map_err(|_| GeomError::EditRefused {
    reason: "the move would make two consecutive vertices coincide",
  })
}

/// Insert a vertex on segment `segment_index`, projecting `point` onto that segment.
///
/// On a straight segment the projection is along the line; on an arc it is along the circle, so the
/// two halves are the same circle and their sweeps add up to the original's.
///
/// # Errors
///
/// [`GeomError::EditRefused`] when the segment does not exist, when the projection lands on an
/// endpoint, or when the point is not on the segment's extent at all.
pub fn insert_vertex(
  contour: &Contour,
  segment_index: usize,
  point: Point2,
) -> Result<Contour, GeomError> {
  let segments = contour.segments();
  let segment = segments.get(segment_index).ok_or(GeomError::EditRefused {
    reason: "the contour has no segment at that index",
  })?;

  let (first_bulge, second_bulge, inserted) = match segment {
    Segment::Line(line) => {
      let direction = line.end - line.start;
      let length_squared = direction.dot(direction);
      if length_squared <= 0.0 {
        return Err(GeomError::EditRefused {
          reason: "the segment has no length",
        });
      }
      let parameter = (point - line.start).dot(direction) / length_squared;
      if parameter <= ENDPOINT_EPSILON || parameter >= 1.0 - ENDPOINT_EPSILON {
        return Err(GeomError::EditRefused {
          reason: "the insertion point falls on the segment's ends, not inside it",
        });
      }
      (0.0, 0.0, line.point_at(parameter))
    }
    Segment::Arc(arc) => {
      let (first, second, inserted) = split_arc(*arc, point)?;
      (first, second, inserted)
    }
  };

  let mut vertices = contour.vertices().to_vec();
  if segment_index < vertices.len() {
    vertices[segment_index].bulge = first_bulge;
  }
  // For a closed contour the last segment wraps to the first vertex, so the new vertex goes at the
  // end and its bulge becomes the closing segment.
  let insert_at = segment_index + 1;
  let vertex = Vertex::new(inserted, second_bulge);
  if insert_at <= vertices.len() {
    vertices.insert(insert_at, vertex);
  } else {
    vertices.push(vertex);
  }
  rebuild(contour, vertices).map_err(|_| GeomError::EditRefused {
    reason: "the insertion would make the contour degenerate",
  })
}

/// Remove vertex `index`.
///
/// # Errors
///
/// [`GeomError::EditRefused`] when the index does not exist, or when the contour would be left with
/// fewer than two vertices.
pub fn delete_vertex(contour: &Contour, index: usize) -> Result<Contour, GeomError> {
  if index >= contour.len() {
    return Err(GeomError::EditRefused {
      reason: "the contour has no vertex at that index",
    });
  }
  if contour.len() <= 2 {
    return Err(GeomError::EditRefused {
      reason: "a contour needs at least two vertices",
    });
  }
  let mut vertices = contour.vertices().to_vec();
  vertices.remove(index);
  rebuild(contour, vertices).map_err(|_| GeomError::EditRefused {
    reason: "deleting that vertex would make the contour degenerate",
  })
}

fn rebuild(contour: &Contour, vertices: Vec<Vertex>) -> Result<Contour, GeomError> {
  Contour::new(vertices, contour.is_closed())
}

/// Split an arc at the point on its circle nearest to `point`.
///
/// The projection is radial, so the two halves share the original centre and radius; only the angles
/// change. A projection that lands on either end of the sweep is refused, because one of the halves
/// would be a zero-length arc.
fn split_arc(arc: Arc, point: Point2) -> Result<(f64, f64, Point2), GeomError> {
  let offset = point - arc.center();
  let unit = offset.normalized().ok_or(GeomError::EditRefused {
    reason: "the insertion point sits exactly on the arc's centre",
  })?;
  let projected = arc.center() + unit * arc.radius();

  let sweep = arc.sweep();
  let start = arc.start_angle();
  let angle = (projected - arc.center()).angle();
  let travelled = if sweep >= 0.0 {
    (angle - start).rem_euclid(TAU)
  } else {
    (start - angle).rem_euclid(TAU)
  };
  let span = sweep.abs();
  let slack = ENDPOINT_EPSILON.max(1.0e-12 * arc.radius());
  if travelled <= slack || span - travelled <= slack {
    return Err(GeomError::EditRefused {
      reason: "the insertion point falls on the arc's ends, not inside it",
    });
  }

  let first_sweep = if sweep >= 0.0 { travelled } else { -travelled };
  let second_sweep = if sweep >= 0.0 {
    span - travelled
  } else {
    -(span - travelled)
  };
  let first = Arc::new(arc.center(), arc.radius(), start, first_sweep)?;
  let second = Arc::new(arc.center(), arc.radius(), angle, second_sweep)?;
  Ok((first.bulge(), second.bulge(), projected))
}
