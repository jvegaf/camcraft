//! Transforms: moving, turning, resizing and mirroring authored geometry.
//!
//! # Which properties survive
//!
//! * **Translate, rotate and mirror are exact.** They are isometries, so an arc stays an arc: the
//!   centre, the radius and the angles move and the bulge is untouched (mirror negates it, because a
//!   reflection does flip the turn direction).
//! * **A uniform scale is exact** for the same reason: every radius scales by the same factor.
//! * **A non-uniform scale cannot keep arcs.** Scaling X and Y differently turns a circle into an
//!   ellipse, which circular arcs cannot represent, so the affected contours are tessellated at the
//!   caller's tolerance and the outcome reports how many arcs that cost (M2 A5).
//! * **A closed contour always comes back counter-clockwise.** Winding is normalised after any
//!   transform that can flip it, so downstream code never has to guess.

use crate::arc::Arc;
use crate::contour::{Contour, Vertex};
use crate::point::{Point2, Vec2};
use crate::segment::Segment;
use crate::{GeomError, TolerancePolicy};

/// What a scale produced, including what it cost.
#[derive(Clone, Debug, PartialEq)]
pub struct ScaleOutcome {
  /// The transformed contour.
  pub contour: Contour,
  /// How many arcs the result no longer has, because a non-uniform scale had to tessellate them.
  ///
  /// Zero for every exact case: a uniform factor, or a contour that never had arcs.
  pub lost_arcs: usize,
  /// The largest deviation the tessellation actually introduced, in output millimetres.
  ///
  /// Measured against the true curve, not assumed from the requested tolerance: it is zero when
  /// nothing was approximated.
  pub approximation_error: f64,
}

impl ScaleOutcome {
  /// True when the scale changed the geometry's kind by dropping arcs.
  pub fn lost_geometry(&self) -> bool {
    self.lost_arcs > 0
  }
}

/// Move a contour by `delta`.
///
/// # Errors
///
/// [`GeomError`] if the result cannot be a valid contour, which a translation cannot cause.
pub fn translate(contour: &Contour, delta: Vec2) -> Result<Contour, GeomError> {
  rebuild(
    contour,
    contour
      .vertices()
      .iter()
      .map(|vertex| Vertex::new(vertex.point.translated(delta), vertex.bulge))
      .collect(),
  )
}

/// Turn a contour by `radians` counter-clockwise about `about`.
///
/// # Errors
///
/// [`GeomError`] if the result cannot be a valid contour, which a rotation cannot cause.
pub fn rotate(contour: &Contour, radians: f64, about: Point2) -> Result<Contour, GeomError> {
  rebuild(
    contour,
    contour
      .vertices()
      .iter()
      .map(|vertex| Vertex::new(vertex.point.rotated_about(about, radians), vertex.bulge))
      .collect(),
  )
}

/// Reflect a contour about the line through `point` with direction `direction`.
///
/// # Errors
///
/// [`GeomError::InvalidPrimitive`] when `direction` has no length, which does not define a line.
pub fn mirror(contour: &Contour, point: Point2, direction: Vec2) -> Result<Contour, GeomError> {
  let unit = direction.normalized().ok_or(GeomError::InvalidPrimitive {
    reason: "a mirror axis needs a direction with a non-zero length",
  })?;
  let vertices = contour
    .vertices()
    .iter()
    .map(|vertex| {
      // A reflection reverses the turn direction, so the bulge changes sign with the point.
      Vertex::new(reflect(vertex.point, point, unit), -vertex.bulge)
    })
    .collect();
  normalise_winding(rebuild(contour, vertices)?)
}

/// Scale a contour by `sx` and `sy` about `about`, tessellating at `tolerance` when it must.
///
/// # Errors
///
/// [`GeomError::InvalidPrimitive`] when a factor is zero or not finite, or when the policy's chord
/// tolerance is not positive.
pub fn scale(
  contour: &Contour,
  sx: f64,
  sy: f64,
  about: Point2,
  tolerance: TolerancePolicy,
) -> Result<ScaleOutcome, GeomError> {
  for (factor, what) in [
    (sx, "the X scale factor must be finite and non-zero"),
    (sy, "the Y scale factor must be finite and non-zero"),
  ] {
    if !factor.is_finite() || factor == 0.0 {
      return Err(GeomError::InvalidPrimitive { reason: what });
    }
  }
  if !tolerance.chord.is_finite() || tolerance.chord <= 0.0 {
    return Err(GeomError::InvalidPrimitive {
      reason: "the scale tolerance must be a positive finite number",
    });
  }

  let exact = sx == sy || !contour.has_arcs();
  if exact {
    let vertices = contour
      .vertices()
      .iter()
      .map(|vertex| Vertex::new(scale_point(vertex.point, about, sx, sy), vertex.bulge))
      .collect();
    return Ok(ScaleOutcome {
      contour: normalise_winding(rebuild(contour, vertices)?)?,
      lost_arcs: 0,
      approximation_error: 0.0,
    });
  }

  // Tessellate in input space at a tolerance tightened by the largest stretch, so the deviation
  // seen in the output never exceeds the tolerance the caller asked for.
  let stretch = sx.abs().max(sy.abs());
  let input_tolerance = tolerance.with_chord(tolerance.chord / stretch);
  let lost_arcs = contour
    .segments()
    .iter()
    .filter(|segment| matches!(segment, Segment::Arc(_)))
    .count();
  let approximation_error = achieved_sagitta(contour, input_tolerance) * stretch;
  let points: Vec<Point2> = contour
    .tessellate(input_tolerance)
    .into_iter()
    .map(|point| scale_point(point, about, sx, sy))
    .collect();
  let scaled = Contour::from_points(&points, contour.is_closed())?;
  Ok(ScaleOutcome {
    contour: normalise_winding(scaled)?,
    lost_arcs,
    approximation_error,
  })
}

fn rebuild(contour: &Contour, vertices: Vec<Vertex>) -> Result<Contour, GeomError> {
  Contour::new(vertices, contour.is_closed())
}

/// Give a closed contour its counter-clockwise winding back.
fn normalise_winding(contour: Contour) -> Result<Contour, GeomError> {
  if contour.is_closed() && contour.signed_area() < 0.0 {
    let reversed = contour.reverse();
    // Reversal of a valid contour is valid; the constructor only re-checks the invariant.
    return Contour::new(reversed.vertices().to_vec(), reversed.is_closed());
  }
  Ok(contour)
}

fn reflect(point: Point2, line_point: Point2, unit: Vec2) -> Point2 {
  let offset = point - line_point;
  let along = unit * offset.dot(unit);
  line_point + along - (offset - along)
}

fn scale_point(point: Point2, about: Point2, sx: f64, sy: f64) -> Point2 {
  Point2::new(
    about.x + (point.x - about.x) * sx,
    about.y + (point.y - about.y) * sy,
  )
}

/// The largest sagitta the tessellation of any arc in `contour` introduces at the policy's chord
/// tolerance.
///
/// Measured between each chord and the true arc point halfway along it, so the number reported is
/// what happened rather than what was requested.
fn achieved_sagitta(contour: &Contour, tolerance: TolerancePolicy) -> f64 {
  let mut worst: f64 = 0.0;
  for segment in contour.segments() {
    let Segment::Arc(arc) = segment else {
      continue;
    };
    worst = worst.max(arc_sagitta(arc, tolerance));
  }
  worst
}

fn arc_sagitta(arc: Arc, tolerance: TolerancePolicy) -> f64 {
  let steps = arc.step_count(tolerance);
  if steps == 0 {
    return 0.0;
  }
  let mut worst: f64 = 0.0;
  for step in 0..steps {
    let start = arc.point_at_fraction(step as f64 / steps as f64);
    let end = arc.point_at_fraction((step + 1) as f64 / steps as f64);
    let middle = arc.point_at_fraction((step as f64 + 0.5) / steps as f64);
    let chord = crate::segment::LineSeg::new(start, end);
    worst = worst.max(chord.distance_to_point(middle));
  }
  worst
}
