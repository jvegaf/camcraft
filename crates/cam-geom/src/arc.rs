//! Circular arcs as first-class geometry.

use std::f64::consts::TAU;

use crate::point::{Point2, Vec2};
use crate::{GeomError, MAX_TESSELLATION_STEPS};

/// A circular arc, stored as centre, radius, start angle and **signed** sweep.
///
/// A positive sweep turns counter-clockwise. Keeping the sweep signed is what allows an arc to be
/// reversed and offset without re-deriving its direction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arc {
  center: Point2,
  radius: f64,
  start_angle: f64,
  sweep: f64,
}

impl Arc {
  /// Build an arc from a centre, radius, start angle and signed sweep in radians.
  pub fn new(center: Point2, radius: f64, start_angle: f64, sweep: f64) -> Result<Self, GeomError> {
    if !radius.is_finite() || radius <= 0.0 {
      return Err(GeomError::DegenerateArc {
        reason: "radius must be finite and positive",
      });
    }
    if !sweep.is_finite() || sweep == 0.0 || sweep.abs() > TAU + 1.0e-12 {
      return Err(GeomError::DegenerateArc {
        reason: "sweep must be non-zero and at most one full turn",
      });
    }
    Ok(Self {
      center,
      radius,
      start_angle,
      sweep: sweep.clamp(-TAU, TAU),
    })
  }

  /// Build the arc that starts at `start`, ends at `end` and bulges by `bulge`.
  ///
  /// `bulge` follows the DXF convention: `tan(sweep / 4)`, positive for a counter-clockwise arc.
  pub fn from_endpoints_bulge(start: Point2, end: Point2, bulge: f64) -> Result<Self, GeomError> {
    let chord = end - start;
    let chord_length = chord.length();
    if chord_length <= 0.0 {
      return Err(GeomError::DegenerateArc {
        reason: "start and end coincide",
      });
    }
    let sweep = 4.0 * bulge.atan();
    if sweep.abs() < 1.0e-12 {
      return Err(GeomError::DegenerateArc {
        reason: "bulge is too small to define an arc",
      });
    }
    let half = sweep / 2.0;
    let sin_half = half.sin();
    if sin_half.abs() < 1.0e-12 {
      return Err(GeomError::DegenerateArc {
        reason: "bulge describes a straight half turn",
      });
    }
    let radius = chord_length / (2.0 * sin_half.abs());
    let normal = chord.perp_ccw() / chord_length;
    let offset = chord_length / (2.0 * half.tan());
    let center = start.lerp(end, 0.5) + normal * offset;
    let start_angle = (start - center).angle();
    Self::new(center, radius, start_angle, sweep)
  }

  /// Build the arc through three ordered points.
  pub fn from_three_points(a: Point2, b: Point2, c: Point2) -> Result<Self, GeomError> {
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    if d.abs() < 1.0e-12 {
      return Err(GeomError::DegenerateArc {
        reason: "the three points are collinear",
      });
    }
    let a2 = a.x * a.x + a.y * a.y;
    let b2 = b.x * b.x + b.y * b.y;
    let c2 = c.x * c.x + c.y * c.y;
    let center = Point2::new(
      (a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d,
      (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d,
    );
    let radius = center.distance_to(a);
    let start_angle = (a - center).angle();
    let counter_clockwise = (b - a).cross(c - b) > 0.0;
    let sweep = if counter_clockwise {
      wrap_positive((c - center).angle() - start_angle)
    } else {
      -wrap_positive(start_angle - (c - center).angle())
    };
    Self::new(center, radius, start_angle, sweep)
  }

  /// Arc centre.
  pub fn center(self) -> Point2 {
    self.center
  }

  /// Arc radius.
  pub fn radius(self) -> f64 {
    self.radius
  }

  /// Angle of the start point, in radians.
  pub fn start_angle(self) -> f64 {
    self.start_angle
  }

  /// Signed sweep in radians; positive is counter-clockwise.
  pub fn sweep(self) -> f64 {
    self.sweep
  }

  /// True when the arc turns counter-clockwise.
  pub fn is_ccw(self) -> bool {
    self.sweep > 0.0
  }

  /// The start point.
  pub fn start_point(self) -> Point2 {
    self.point_at_fraction(0.0)
  }

  /// The end point.
  pub fn end_point(self) -> Point2 {
    self.point_at_fraction(1.0)
  }

  /// The point halfway along the sweep.
  pub fn mid_point(self) -> Point2 {
    self.point_at_fraction(0.5)
  }

  /// The DXF bulge that expresses this arc's sweep.
  pub fn bulge(self) -> f64 {
    (self.sweep / 4.0).tan()
  }

  /// Arc length.
  pub fn length(self) -> f64 {
    self.radius * self.sweep.abs()
  }

  /// Unit tangent at the start point, in the direction of travel.
  pub fn tangent_at_start(self) -> Vec2 {
    let angle = if self.is_ccw() {
      self.start_angle + std::f64::consts::FRAC_PI_2
    } else {
      self.start_angle - std::f64::consts::FRAC_PI_2
    };
    Vec2::from_angle(angle, 1.0)
  }

  /// The point at `fraction` of the sweep, where 0 is the start and 1 the end.
  pub fn point_at_fraction(self, fraction: f64) -> Point2 {
    let angle = self.start_angle + self.sweep * fraction;
    Point2::new(
      self.center.x + self.radius * angle.cos(),
      self.center.y + self.radius * angle.sin(),
    )
  }

  /// The same arc travelled in the opposite direction.
  pub fn reversed(self) -> Self {
    Self {
      center: self.center,
      radius: self.radius,
      start_angle: self.start_angle + self.sweep,
      sweep: -self.sweep,
    }
  }

  /// Concentric arc with the radius grown by `distance` (negative shrinks).
  pub fn offset_radius(self, distance: f64) -> Result<Self, GeomError> {
    Self::new(
      self.center,
      self.radius + distance,
      self.start_angle,
      self.sweep,
    )
  }

  /// Signed area contribution around the origin, exact for arcs (Green's theorem).
  pub fn area_contribution(self) -> f64 {
    let a1 = self.start_angle;
    let a2 = self.start_angle + self.sweep;
    let r = self.radius;
    0.5
      * (r * self.center.x * (a2.sin() - a1.sin()) - r * self.center.y * (a2.cos() - a1.cos())
        + r * r * (a2 - a1))
  }

  /// Number of chord steps needed to stay within `tolerance` of the true arc.
  pub fn step_count(self, tolerance: f64) -> usize {
    let tolerance = tolerance.max(f64::MIN_POSITIVE);
    if self.radius <= tolerance {
      return 1;
    }
    let ratio = (1.0 - tolerance / self.radius).clamp(-1.0, 1.0);
    let max_step = 2.0 * ratio.acos();
    if max_step <= 0.0 {
      return MAX_TESSELLATION_STEPS;
    }
    let steps = (self.sweep.abs() / max_step).ceil();
    if !steps.is_finite() || steps < 1.0 {
      1
    } else {
      (steps as usize).min(MAX_TESSELLATION_STEPS)
    }
  }

  /// Sample the arc, including both endpoints.
  pub fn tessellate(self, tolerance: f64) -> Vec<Point2> {
    let steps = self.step_count(tolerance);
    (0..=steps)
      .map(|i| self.point_at_fraction(i as f64 / steps as f64))
      .collect()
  }
}

/// Normalise an angle into `[0, TAU)`.
fn wrap_positive(mut angle: f64) -> f64 {
  angle %= TAU;
  if angle < 0.0 {
    angle += TAU;
  }
  angle
}
