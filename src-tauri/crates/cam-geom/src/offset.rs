//! Arc-preserving contour offsetting.
//!
//! Offsetting is where CAM geometry kernels usually break, so the rules are explicit:
//!
//! * Each segment is offset as a **curve**, never as a polyline: a line shifts along its normal, an
//!   arc keeps its centre and changes radius. Arcs stay arcs (A2).
//! * Adjacent offset curves are joined by intersecting their supporting curves near the original
//!   vertex. The join point lies exactly on both curves, so trimming an arc changes its sweep, never
//!   its shape.
//! * When a corner is too sharp for the miter limit, or two curves cannot meet, the join falls back
//!   to a straight bridge and reports [`OffsetWarning::MiterLimited`].
//! * Self-intersections created by the offset are removed by splitting at the intersection and
//!   keeping the larger loop. That is the only path that flattens arcs, and it reports
//!   [`OffsetWarning::Flattened`] when it happens.

use std::f64::consts::TAU;

use crate::arc::Arc;
use crate::boolean::normalize;
use crate::contour::{Contour, Vertex};
use crate::point::{Point2, Vec2};
use crate::region::Region;
use crate::segment::Segment;
use crate::{GeomError, TolerancePolicy};

/// Tuning for [`offset_contour`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OffsetOptions {
  /// Largest accepted join distance, as a multiple of the offset distance.
  ///
  /// A 90 degree corner needs `sqrt(2)`, a 30 degree corner about `3.9`.
  pub miter_limit: f64,
  /// Tolerance policy: chord tolerance drives tessellation/self-intersection/flattening, epsilon
  /// drives geometric comparisons.
  pub tolerance: TolerancePolicy,
}

impl OffsetOptions {
  /// Options with the given miter limit and tolerance policy.
  pub const fn new(miter_limit: f64, tolerance: TolerancePolicy) -> Self {
    Self {
      miter_limit,
      tolerance,
    }
  }

  /// The canonical options: miter limit 4 with the standard tolerance policy.
  pub const fn standard() -> Self {
    Self::new(4.0, TolerancePolicy::standard())
  }
}

/// Something the caller should tell the user about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OffsetWarning {
  /// A corner exceeded the miter limit and was joined with a straight bridge.
  MiterLimited,
  /// Self-intersections were removed, which flattened the contour to line segments.
  Flattened,
}

/// The result of offsetting a contour.
#[derive(Clone, Debug, PartialEq)]
pub struct Offset {
  /// The offset contour, always closed.
  pub contour: Contour,
  /// True when self-intersection removal forced a tessellated result.
  pub flattened: bool,
  /// Warnings, in the order they were produced.
  pub warnings: Vec<OffsetWarning>,
}

/// Offset a closed contour.
///
/// Positive `distance` grows the enclosed region of a counter-clockwise contour (and shrinks a
/// clockwise one), so the sign always means "away from the material". Offset the same contour by
/// `-distance` for the other side.
pub fn offset_contour(
  contour: &Contour,
  distance: f64,
  options: &OffsetOptions,
) -> Result<Offset, GeomError> {
  if !contour.is_closed() {
    // A precondition, not a collapse: the input was never offsettable, so callers must be able to
    // tell "wrong input" from "the geometry collapsed" and report the right thing to the user.
    return Err(GeomError::EditRefused {
      reason: "only closed contours can be offset",
    });
  }
  let mut warnings = Vec::new();
  if distance == 0.0 {
    return Ok(Offset {
      contour: contour.clone(),
      flattened: false,
      warnings,
    });
  }

  let winding = if contour.is_ccw() { 1.0 } else { -1.0 };
  let signed = distance * winding;

  let segments = contour.segments();
  let count = segments.len();

  let mut curves = Vec::with_capacity(count);
  for segment in &segments {
    curves.push(offset_segment(*segment, signed, options)?);
  }

  // Join every pair of adjacent offset curves at the original vertex.
  let limit = options.miter_limit * signed.abs();
  let mut points: Vec<(Point2, Option<usize>)> = Vec::new();
  for index in 0..count {
    let previous = &curves[(index + count - 1) % count];
    let next = &curves[index];
    let vertex = contour.vertices()[index].point;
    match choose_join(previous, next, vertex, limit, options.tolerance) {
      Some(point) => points.push((point, Some(index))),
      None => {
        warnings.push(OffsetWarning::MiterLimited);
        points.push((previous.end, None));
        points.push((next.start, Some(index)));
      }
    }
  }

  let mut vertices = Vec::with_capacity(points.len());
  for index in 0..points.len() {
    let (point, origin) = points[index];
    let following = points[(index + 1) % points.len()].0;
    let bulge = match origin {
      None => 0.0,
      Some(curve_index) => trimmed_bulge(&curves[curve_index], point, following, options)?,
    };
    vertices.push(Vertex::new(point, bulge));
  }

  let candidate = Contour::new(vertices, true)?;

  let original_area = contour.signed_area();
  let candidate_area = candidate.signed_area();
  let eps = options.tolerance.epsilon;
  if candidate_area.signum() != original_area.signum() || candidate_area.abs() <= eps * eps {
    return Err(GeomError::OffsetCollapsed {
      reason: "the offset would invert or eliminate the contour",
    });
  }

  // Guard against the classic offset failure: shrinking past the shape's own width produces a
  // valid-looking contour that has passed *through* the shape and come out mirrored. Every vertex of
  // a shrinking offset must remain at a sensible distance from the original boundary; a vertex far
  // closer than the offset distance means the offset crossed the shape.
  //
  // This is a safety guard, not a topological solution: a full arrangement-based offset with
  // skeleton handling is future work, and refusing loudly is the correct M0 behaviour.
  if signed < 0.0 {
    let guard = TolerancePolicy::standard()
      .with_chord(options.tolerance.chord.max(1.0e-6))
      .with_epsilon(options.tolerance.epsilon);
    let boundary = contour.tessellate(guard);
    let threshold = signed.abs() * 0.5;
    for vertex in candidate.vertices() {
      if distance_to_boundary(vertex.point, &boundary) < threshold {
        return Err(GeomError::OffsetCollapsed {
          reason: "the offset passed through the shape instead of inside it",
        });
      }
    }
  }

  let force_flatten = matches!(options.tolerance.arc, crate::ArcPolicy::Flatten { .. });
  let flatten_chord = match options.tolerance.arc {
    crate::ArcPolicy::Flatten { chord } => chord,
    crate::ArcPolicy::Preserve => options.tolerance.chord,
  };
  let flatten_policy = options.tolerance.with_chord(flatten_chord);

  let sampled = candidate.tessellate(flatten_policy);
  let Some(cleaned) = remove_self_intersections(&sampled, options.tolerance) else {
    if force_flatten && candidate.has_arcs() {
      let flattened = Contour::from_points(&sampled, true)?;
      warnings.push(OffsetWarning::Flattened);
      return Ok(Offset {
        contour: flattened,
        flattened: true,
        warnings,
      });
    }
    return Ok(Offset {
      contour: candidate,
      flattened: false,
      warnings,
    });
  };
  // The offset crosses itself. The arrangement can cut the candidate against itself and keep the
  // curve, so it is asked first whenever the candidate carries an arc; the sampled cleanup stays as
  // the fallback, because it is the contract this reader had before arrangements were available.
  if !force_flatten && candidate.has_arcs() {
    let arrangement_tolerance = options.tolerance.with_epsilon(options.tolerance.chord);
    if let Some(surviving) = arrangement_resolution(&candidate, arrangement_tolerance) {
      return Ok(Offset {
        contour: surviving,
        flattened: false,
        warnings,
      });
    }
  }
  let flattened = Contour::from_points(&cleaned, true)?;
  warnings.push(OffsetWarning::Flattened);
  Ok(Offset {
    contour: flattened,
    flattened: true,
    warnings,
  })
}

/// A segment's offset supporting curve.
#[derive(Clone, Copy, Debug)]
struct OffsetCurve {
  kind: Curve,
  /// Natural, untrimmed start point of the offset segment.
  start: Point2,
  /// Natural, untrimmed end point of the offset segment.
  end: Point2,
}

/// The supporting curve of an offset segment.
#[derive(Clone, Copy, Debug)]
enum Curve {
  Line {
    origin: Point2,
    direction: Vec2,
  },
  Arc {
    center: Point2,
    radius: f64,
    ccw: bool,
  },
}

/// Offset one segment as a curve.
fn offset_segment(
  segment: Segment,
  signed: f64,
  options: &OffsetOptions,
) -> Result<OffsetCurve, GeomError> {
  match segment {
    Segment::Line(line) => {
      let direction = line.direction().ok_or(GeomError::OffsetCollapsed {
        reason: "zero-length segment",
      })?;
      let shift = direction.perp_cw() * signed;
      Ok(OffsetCurve {
        kind: Curve::Line {
          origin: line.start + shift,
          direction,
        },
        start: line.start + shift,
        end: line.end + shift,
      })
    }
    Segment::Arc(arc) => {
      let radial_sign = if arc.is_ccw() { 1.0 } else { -1.0 };
      let radius = arc.radius() + signed * radial_sign;
      if radius <= options.tolerance.epsilon {
        return Err(GeomError::OffsetCollapsed {
          reason: "an arc radius would collapse",
        });
      }
      let offset_arc = Arc::new(arc.center(), radius, arc.start_angle(), arc.sweep())?;
      Ok(OffsetCurve {
        kind: Curve::Arc {
          center: arc.center(),
          radius,
          ccw: arc.is_ccw(),
        },
        start: offset_arc.start_point(),
        end: offset_arc.end_point(),
      })
    }
  }
}

/// The join point nearest to the original vertex, if one lies inside the miter limit.
fn choose_join(
  previous: &OffsetCurve,
  next: &OffsetCurve,
  vertex: Point2,
  limit: f64,
  tolerance: TolerancePolicy,
) -> Option<Point2> {
  // Smooth or concentric continuation: the curves already meet, so there is nothing to intersect
  // and nothing to bridge. Without this, offsetting a circle made of two half arcs would try to
  // intersect a circle with itself.
  let (first_end, second_start) = (previous.end, next.start);
  if first_end.is_near(second_start, tolerance) {
    return Some(first_end);
  }
  let mut best: Option<(f64, Point2)> = None;
  for point in curve_intersections(previous.kind, next.kind) {
    let distance = point.distance_to(vertex);
    if distance <= limit && best.map_or(true, |(best_distance, _)| distance < best_distance) {
      best = Some((distance, point));
    }
  }
  best.map(|(_, point)| point)
}

/// All intersection points of two supporting curves, ignoring span limits.
fn curve_intersections(a: Curve, b: Curve) -> Vec<Point2> {
  match (a, b) {
    (
      Curve::Line {
        origin: p1,
        direction: d1,
      },
      Curve::Line {
        origin: p2,
        direction: d2,
      },
    ) => line_line(p1, d1, p2, d2).into_iter().collect(),
    (
      Curve::Line { origin, direction },
      Curve::Arc {
        center,
        radius,
        ccw: _,
      },
    )
    | (
      Curve::Arc {
        center,
        radius,
        ccw: _,
      },
      Curve::Line { origin, direction },
    ) => line_circle(origin, direction, center, radius),
    (
      Curve::Arc {
        center: c1,
        radius: r1,
        ccw: _,
      },
      Curve::Arc {
        center: c2,
        radius: r2,
        ccw: _,
      },
    ) => circle_circle(c1, r1, c2, r2),
  }
}

/// Intersection of two infinite lines, or `None` when they are parallel.
fn line_line(p1: Point2, d1: Vec2, p2: Point2, d2: Vec2) -> Option<Point2> {
  let denominator = d1.cross(d2);
  if denominator.abs() < 1.0e-12 {
    return None;
  }
  let t = (p2 - p1).cross(d2) / denominator;
  Some(p1 + d1 * t)
}

/// Intersection of an infinite line with a circle.
fn line_circle(origin: Point2, direction: Vec2, center: Point2, radius: f64) -> Vec<Point2> {
  let to_center = origin - center;
  let b = to_center.dot(direction);
  let c = to_center.length_squared() - radius * radius;
  let discriminant = b * b - c;
  if discriminant < 0.0 {
    return Vec::new();
  }
  let root = discriminant.sqrt();
  let first = origin + direction * (-b - root);
  if root.abs() < 1.0e-12 {
    return vec![first];
  }
  vec![first, origin + direction * (-b + root)]
}

/// Intersection of two circles.
fn circle_circle(c1: Point2, r1: f64, c2: Point2, r2: f64) -> Vec<Point2> {
  let between = c2 - c1;
  let distance = between.length();
  if distance < 1.0e-12 || distance > r1 + r2 || distance < (r1 - r2).abs() {
    return Vec::new();
  }
  let a = (r1 * r1 - r2 * r2 + distance * distance) / (2.0 * distance);
  let height_squared = r1 * r1 - a * a;
  if height_squared < 0.0 {
    return Vec::new();
  }
  let base = c1 + between * (a / distance);
  let height = height_squared.sqrt();
  let offset = between.perp_ccw() * (height / distance);
  if height < 1.0e-12 {
    return vec![base];
  }
  vec![base + offset, base - offset]
}

/// The bulge of `curve` between two points that both lie on it.
fn trimmed_bulge(
  curve: &OffsetCurve,
  from: Point2,
  to: Point2,
  options: &OffsetOptions,
) -> Result<f64, GeomError> {
  let Curve::Arc {
    center,
    radius: _,
    ccw,
  } = curve.kind
  else {
    return Ok(0.0);
  };
  let start = (from - center).angle();
  let end = (to - center).angle();
  let mut sweep = end - start;
  if ccw {
    if sweep <= 1.0e-12 {
      sweep += TAU;
    }
  } else if sweep >= -1.0e-12 {
    sweep -= TAU;
  }
  if sweep.abs() >= TAU - 1.0e-6 {
    return Err(GeomError::OffsetCollapsed {
      reason: "an arc would be trimmed into a full turn",
    });
  }
  if sweep.abs() < 1.0e-12 {
    let _ = options;
    return Ok(0.0);
  }
  Ok((sweep / 4.0).tan())
}

/// Remove self-intersection loops, keeping the larger loop at each split.
///
/// Returns `None` when the contour is already simple.
fn remove_self_intersections(points: &[Point2], tolerance: TolerancePolicy) -> Option<Vec<Point2>> {
  let mut current = points.to_vec();
  let mut removed = 0;

  for _ in 0..64 {
    let count = current.len();
    if count < 4 {
      break;
    }
    let mut found: Option<(usize, usize, Point2)> = None;
    'search: for i in 0..count {
      let a1 = current[i];
      let a2 = current[(i + 1) % count];
      for j in (i + 2)..count {
        if i == 0 && j == count - 1 {
          continue;
        }
        let b1 = current[j];
        let b2 = current[(j + 1) % count];
        if let Some(point) = segment_intersection(a1, a2, b1, b2, tolerance) {
          found = Some((i, j, point));
          break 'search;
        }
      }
    }

    let Some((i, j, point)) = found else {
      break;
    };

    let first_loop: Vec<Point2> = current[i + 1..=j]
      .iter()
      .copied()
      .chain(std::iter::once(point))
      .collect();
    let second_loop: Vec<Point2> = current[j + 1..]
      .iter()
      .copied()
      .chain(current[..=i].iter().copied())
      .chain(std::iter::once(point))
      .collect();

    let first_area = polygon_area(&first_loop).abs();
    let second_area = polygon_area(&second_loop).abs();
    current = if first_area >= second_area {
      first_loop
    } else {
      second_loop
    };
    removed += 1;
    if current.len() < 3 {
      break;
    }
  }

  if removed == 0 {
    None
  } else {
    Some(current)
  }
}

/// Intersection of two finite segments.
fn segment_intersection(
  a1: Point2,
  a2: Point2,
  b1: Point2,
  b2: Point2,
  tolerance: TolerancePolicy,
) -> Option<Point2> {
  let r = a2 - a1;
  let s = b2 - b1;
  let denominator = r.cross(s);
  if denominator.abs() < 1.0e-15 {
    return None;
  }
  let base = b1 - a1;
  let t = base.cross(s) / denominator;
  let u = base.cross(r) / denominator;
  let r_length = r.length().max(f64::MIN_POSITIVE);
  let s_length = s.length().max(f64::MIN_POSITIVE);
  let eps = tolerance.epsilon;
  let t_slack = eps / r_length;
  let u_slack = eps / s_length;
  if t < -t_slack || t > 1.0 + t_slack || u < -u_slack || u > 1.0 + u_slack {
    return None;
  }
  Some(a1 + r * t)
}

/// Shortest distance from a point to a closed polyline.
fn distance_to_boundary(point: Point2, boundary: &[Point2]) -> f64 {
  if boundary.len() < 2 {
    return f64::INFINITY;
  }
  let mut best = f64::INFINITY;
  for index in 0..boundary.len() {
    let from = boundary[index];
    let to = boundary[(index + 1) % boundary.len()];
    best = best.min(crate::segment::LineSeg::new(from, to).distance_to_point(point));
  }
  best
}

/// Shoelace area of a polygon.
fn polygon_area(points: &[Point2]) -> f64 {
  let mut total = 0.0;
  for index in 0..points.len() {
    let current = points[index];
    let next = points[(index + 1) % points.len()];
    total += current.x * next.y - next.x * current.y;
  }
  total * 0.5
}

/// Resolve a self-intersecting contour on the arrangement, keeping the curve.
///
/// The sampled cleanup keeps the larger of the two loops a crossing divides the contour into, and
/// this keeps the same thing: the largest boundary by absolute area, so the surviving geometry is
/// the one this reader already promised. `None` means the arrangement could not express the answer -
/// it flattened, it left nothing, or it refused - and the caller falls back to the sampled path
/// rather than returning something weaker than it used to. A refusal is treated exactly like a
/// flattening because the arrangement is an improvement over the sampled cleanup, not the contract:
/// the contract is that a self-intersecting offset gets cleaned, and the sampled path can still
/// keep it.
fn arrangement_resolution(contour: &Contour, tolerance: TolerancePolicy) -> Option<Contour> {
  let Ok(normalized) =
    Region::new(vec![contour.clone()]).and_then(|region| normalize(&region, tolerance))
  else {
    return None;
  };
  if normalized.flattened {
    return None;
  }
  let mut best: Option<(f64, Contour)> = None;
  for boundary in normalized.region.boundaries {
    let area = boundary.signed_area().abs();
    let better = match &best {
      None => true,
      Some((largest, _)) => area > *largest,
    };
    if better {
      best = Some((area, boundary));
    }
  }
  best.map(|(_, contour)| contour)
}
