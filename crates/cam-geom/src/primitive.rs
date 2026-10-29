//! Authoring builders: the shapes a user draws.
//!
//! Every builder here returns a [`Contour`] and **pre-tessellates nothing**. A circle is two half-turn
//! arcs, a polygon is straight segments, and an arc drawn through three points stays one arc, so what
//! the user draws is what reaches the post-processor as `G2`/`G3`.
//!
//! Closed builders emit counter-clockwise geometry: winding is part of the shape's contract, not an
//! accident of the order the vertices were computed in.

use crate::arc::Arc;
use crate::contour::{Contour, Vertex};
use crate::point::Point2;
use crate::GeomError;

use std::f64::consts::TAU;

/// The bulge of a half turn: `tan(PI / 4)`.
///
/// A full turn is not representable as one arc, which is why a circle needs two vertices.
const HALF_TURN_BULGE: f64 = 1.0;

fn require_positive(what: &'static str, value: f64) -> Result<(), GeomError> {
  if !value.is_finite() || value <= 0.0 {
    return Err(GeomError::InvalidPrimitive { reason: what });
  }
  Ok(())
}

fn require_at_least(what: &'static str, value: usize, minimum: usize) -> Result<(), GeomError> {
  if value < minimum {
    return Err(GeomError::InvalidPrimitive { reason: what });
  }
  Ok(())
}

/// A straight line from `from` to `to`, as an open contour.
///
/// # Errors
///
/// [`GeomError::DegenerateArc`] when both endpoints coincide: a zero-length line is not geometry.
pub fn line(from: Point2, to: Point2) -> Result<Contour, GeomError> {
  Contour::new(vec![Vertex::corner(from), Vertex::corner(to)], false)
}

/// A chain of straight segments through `points`.
///
/// # Errors
///
/// [`GeomError::ContourTooShort`] with fewer than two points, and
/// [`GeomError::DegenerateArc`] when two consecutive points coincide.
pub fn polyline(points: &[Point2], closed: bool) -> Result<Contour, GeomError> {
  Contour::from_points(points, closed)
}

/// A counter-clockwise rectangle whose lower-left corner is at `at`.
///
/// # Errors
///
/// [`GeomError::InvalidPrimitive`] when either side is zero, negative or not finite.
pub fn rectangle(at: Point2, width: f64, height: f64) -> Result<Contour, GeomError> {
  require_positive(
    "the rectangle width must be a positive finite number",
    width,
  )?;
  require_positive(
    "the rectangle height must be a positive finite number",
    height,
  )?;
  polyline(
    &[
      at,
      Point2::new(at.x + width, at.y),
      Point2::new(at.x + width, at.y + height),
      Point2::new(at.x, at.y + height),
    ],
    true,
  )
}

/// A counter-clockwise circle, expressed as two half-turn arcs.
///
/// # Errors
///
/// [`GeomError::InvalidPrimitive`] when the radius is zero, negative or not finite.
pub fn circle(center: Point2, radius: f64) -> Result<Contour, GeomError> {
  require_positive("the circle radius must be a positive finite number", radius)?;
  // An arc may never be a full turn, so the two vertices carry a half turn each.
  Contour::new(
    vec![
      Vertex::new(Point2::new(center.x + radius, center.y), HALF_TURN_BULGE),
      Vertex::new(Point2::new(center.x - radius, center.y), HALF_TURN_BULGE),
    ],
    true,
  )
}

/// A counter-clockwise regular polygon with `sides` straight segments.
///
/// `rotation` is the angle of the first vertex, in radians from the positive X axis.
///
/// # Errors
///
/// [`GeomError::InvalidPrimitive`] with fewer than three sides, or a radius that is not positive.
pub fn polygon(
  center: Point2,
  radius: f64,
  sides: usize,
  rotation: f64,
) -> Result<Contour, GeomError> {
  require_at_least("a polygon needs at least three sides", sides, 3)?;
  require_positive(
    "the polygon radius must be a positive finite number",
    radius,
  )?;
  let step = TAU / sides as f64;
  let points: Vec<Point2> = (0..sides)
    .map(|index| {
      let angle = rotation + step * index as f64;
      Point2::new(
        center.x + radius * angle.cos(),
        center.y + radius * angle.sin(),
      )
    })
    .collect();
  polyline(&points, true)
}

/// A counter-clockwise star alternating `points` outer tips and `points` inner valleys.
///
/// `rotation` is the angle of the first outer tip, in radians from the positive X axis.
///
/// # Errors
///
/// [`GeomError::InvalidPrimitive`] with fewer than three points, or when
/// `0 < inner_radius < outer_radius` does not hold. An inner radius equal to the outer one is a
/// polygon, not a star, and is refused rather than silently producing a degenerate shape.
pub fn star(
  center: Point2,
  outer_radius: f64,
  inner_radius: f64,
  points: usize,
  rotation: f64,
) -> Result<Contour, GeomError> {
  require_at_least("a star needs at least three points", points, 3)?;
  require_positive(
    "the star outer radius must be a positive finite number",
    outer_radius,
  )?;
  require_positive(
    "the star inner radius must be a positive finite number",
    inner_radius,
  )?;
  if inner_radius >= outer_radius {
    return Err(GeomError::InvalidPrimitive {
      reason: "the star inner radius must be smaller than the outer one",
    });
  }
  let step = TAU / (points * 2) as f64;
  let vertices: Vec<Point2> = (0..points * 2)
    .map(|index| {
      let radius = if index % 2 == 0 {
        outer_radius
      } else {
        inner_radius
      };
      let angle = rotation + step * index as f64;
      Point2::new(
        center.x + radius * angle.cos(),
        center.y + radius * angle.sin(),
      )
    })
    .collect();
  polyline(&vertices, true)
}

/// A single arc through the ordered points `a`, `b` and `c`, as an open contour.
///
/// # Errors
///
/// [`GeomError::DegenerateArc`] when the three points are collinear or two of them coincide: three
/// points on a straight line do not define a circle.
pub fn arc_through(a: Point2, b: Point2, c: Point2) -> Result<Contour, GeomError> {
  let arc = Arc::from_three_points(a, b, c)?;
  Contour::new(
    vec![
      Vertex::new(arc.start_point(), arc.bulge()),
      Vertex::corner(arc.end_point()),
    ],
    false,
  )
}
