//! Points, displacement vectors and axis-aligned bounds.

use std::ops::{Add, Div, Mul, Neg, Sub};

/// A point in the XY plane, in millimetres.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point2 {
  /// X coordinate, in millimetres.
  pub x: f64,
  /// Y coordinate, in millimetres.
  pub y: f64,
}

impl Point2 {
  /// The origin.
  pub const ORIGIN: Self = Self { x: 0.0, y: 0.0 };

  /// Build a point from millimetres.
  pub const fn new(x: f64, y: f64) -> Self {
    Self { x, y }
  }

  /// Distance to another point.
  pub fn distance_to(self, other: Self) -> f64 {
    (other - self).length()
  }

  /// True when the two points are within `tolerance` of each other.
  pub fn is_near(self, other: Self, tolerance: f64) -> bool {
    self.distance_to(other) <= tolerance
  }

  /// Linear interpolation towards `other` at parameter `t`.
  pub fn lerp(self, other: Self, t: f64) -> Self {
    Self {
      x: self.x + (other.x - self.x) * t,
      y: self.y + (other.y - self.y) * t,
    }
  }

  /// Translate by a displacement.
  pub fn translated(self, v: Vec2) -> Self {
    self + v
  }

  /// Rotate around `pivot` by `radians`, counter-clockwise.
  pub fn rotated_about(self, pivot: Self, radians: f64) -> Self {
    let (sin, cos) = radians.sin_cos();
    let d = self - pivot;
    Self {
      x: pivot.x + d.x * cos - d.y * sin,
      y: pivot.y + d.x * sin + d.y * cos,
    }
  }
}

impl Add<Vec2> for Point2 {
  type Output = Self;
  fn add(self, rhs: Vec2) -> Self {
    Self {
      x: self.x + rhs.x,
      y: self.y + rhs.y,
    }
  }
}

impl Sub<Vec2> for Point2 {
  type Output = Self;
  fn sub(self, rhs: Vec2) -> Self {
    Self {
      x: self.x - rhs.x,
      y: self.y - rhs.y,
    }
  }
}

impl Sub for Point2 {
  type Output = Vec2;
  fn sub(self, rhs: Self) -> Vec2 {
    Vec2 {
      x: self.x - rhs.x,
      y: self.y - rhs.y,
    }
  }
}

/// A displacement in the XY plane, in millimetres.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
  /// X component, in millimetres.
  pub x: f64,
  /// Y component, in millimetres.
  pub y: f64,
}

impl Vec2 {
  /// The zero vector.
  pub const ZERO: Self = Self { x: 0.0, y: 0.0 };
  /// The unit X vector.
  pub const X_AXIS: Self = Self { x: 1.0, y: 0.0 };
  /// The unit Y vector.
  pub const Y_AXIS: Self = Self { x: 0.0, y: 1.0 };

  /// Build a vector from millimetre components.
  pub const fn new(x: f64, y: f64) -> Self {
    Self { x, y }
  }

  /// Build a vector from an angle and a length.
  pub fn from_angle(radians: f64, length: f64) -> Self {
    let (sin, cos) = radians.sin_cos();
    Self {
      x: cos * length,
      y: sin * length,
    }
  }

  /// Euclidean length.
  pub fn length(self) -> f64 {
    self.x.hypot(self.y)
  }

  /// Squared length, for comparisons that do not need the square root.
  pub fn length_squared(self) -> f64 {
    self.x * self.x + self.y * self.y
  }

  /// Unit vector in the same direction, or `None` for a zero vector.
  pub fn normalized(self) -> Option<Self> {
    let len = self.length();
    if len > 0.0 && len.is_finite() {
      Some(Self {
        x: self.x / len,
        y: self.y / len,
      })
    } else {
      None
    }
  }

  /// Dot product.
  pub fn dot(self, other: Self) -> f64 {
    self.x * other.x + self.y * other.y
  }

  /// 2D cross product (the Z component of the 3D cross product).
  pub fn cross(self, other: Self) -> f64 {
    self.x * other.y - self.y * other.x
  }

  /// Rotate 90 degrees counter-clockwise.
  pub fn perp_ccw(self) -> Self {
    Self {
      x: -self.y,
      y: self.x,
    }
  }

  /// Rotate 90 degrees clockwise.
  pub fn perp_cw(self) -> Self {
    Self {
      x: self.y,
      y: -self.x,
    }
  }

  /// Direction angle in radians, measured from the X axis.
  pub fn angle(self) -> f64 {
    self.y.atan2(self.x)
  }
}

impl Add for Vec2 {
  type Output = Self;
  fn add(self, rhs: Self) -> Self {
    Self {
      x: self.x + rhs.x,
      y: self.y + rhs.y,
    }
  }
}

impl Sub for Vec2 {
  type Output = Self;
  fn sub(self, rhs: Self) -> Self {
    Self {
      x: self.x - rhs.x,
      y: self.y - rhs.y,
    }
  }
}

impl Mul<f64> for Vec2 {
  type Output = Self;
  fn mul(self, rhs: f64) -> Self {
    Self {
      x: self.x * rhs,
      y: self.y * rhs,
    }
  }
}

impl Div<f64> for Vec2 {
  type Output = Self;
  fn div(self, rhs: f64) -> Self {
    Self {
      x: self.x / rhs,
      y: self.y / rhs,
    }
  }
}

impl Neg for Vec2 {
  type Output = Self;
  fn neg(self) -> Self {
    Self {
      x: -self.x,
      y: -self.y,
    }
  }
}

/// An axis-aligned bounding box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
  /// Lower corner.
  pub min: Point2,
  /// Upper corner.
  pub max: Point2,
}

impl Default for Bounds {
  fn default() -> Self {
    Self::empty()
  }
}

impl Bounds {
  /// An empty box: every point expands it.
  pub fn empty() -> Self {
    Self {
      min: Point2::new(f64::INFINITY, f64::INFINITY),
      max: Point2::new(f64::NEG_INFINITY, f64::NEG_INFINITY),
    }
  }

  /// The box containing every supplied point.
  pub fn from_points(points: &[Point2]) -> Self {
    points
      .iter()
      .copied()
      .fold(Self::empty(), |b, p| b.include(p))
  }

  /// Grow the box to include a point.
  pub fn include(self, point: Point2) -> Self {
    Self {
      min: Point2::new(self.min.x.min(point.x), self.min.y.min(point.y)),
      max: Point2::new(self.max.x.max(point.x), self.max.y.max(point.y)),
    }
  }

  /// The box containing both boxes.
  pub fn union(self, other: Self) -> Self {
    self.include(other.min).include(other.max)
  }

  /// Grow the box by `by` millimetres on every side.
  pub fn expand(self, by: f64) -> Self {
    if self.is_empty() {
      return self;
    }
    Self {
      min: Point2::new(self.min.x - by, self.min.y - by),
      max: Point2::new(self.max.x + by, self.max.y + by),
    }
  }

  /// True when no point has been included yet.
  pub fn is_empty(self) -> bool {
    self.min.x > self.max.x || self.min.y > self.max.y
  }

  /// Width along X.
  pub fn width(self) -> f64 {
    self.max.x - self.min.x
  }

  /// Height along Y.
  pub fn height(self) -> f64 {
    self.max.y - self.min.y
  }

  /// Centre point.
  pub fn center(self) -> Point2 {
    Point2::new(
      (self.min.x + self.max.x) * 0.5,
      (self.min.y + self.max.y) * 0.5,
    )
  }

  /// True when `point` lies inside the box, grown by `tolerance`.
  pub fn contains(self, point: Point2, tolerance: f64) -> bool {
    point.x >= self.min.x - tolerance
      && point.x <= self.max.x + tolerance
      && point.y >= self.min.y - tolerance
      && point.y <= self.max.y + tolerance
  }
}
