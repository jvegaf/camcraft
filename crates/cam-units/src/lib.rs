//! Units and number formatting for the CAM pipeline.
//!
//! # Invariants established here
//!
//! * Internal geometry is **millimetres** everywhere. [`Length`] wraps an `f64` that always means
//!   millimetres; no layer above this crate converts units while computing.
//! * A document declares a [`UnitSystem`] as metadata. Conversion happens **only** at the document
//!   boundary, through [`Length::from_value_in`] / [`Length::value_in`], and at emission time
//!   through [`Format`].
//! * Rounding happens **only** when text is produced, driven by the post-processor's per-variable
//!   decimal count. Rounding earlier would compound error across depth passes.
//!
//! Must not depend on `cam-geom`, `cam-store`, `cam-post` or any UI crate.

#![warn(missing_docs)]

use std::fmt;
use std::ops::{Add, Div, Mul, Neg, Sub};

use serde::{Deserialize, Serialize};

/// Millimetres per inch, exactly, per the international yard and pound agreement.
pub const MM_PER_INCH: f64 = 25.4;

/// The unit system a document declares. Internal values are always millimetres.
#[derive(
  Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum UnitSystem {
  /// Millimetres.
  #[default]
  Mm,
  /// Inches. Only used to convert at the document boundary.
  Inch,
}

impl UnitSystem {
  /// Short suffix used in human-facing tool names and labels.
  pub const fn short_name(self) -> &'static str {
    match self {
      Self::Mm => "mm",
      Self::Inch => "in",
    }
  }
}

/// A length stored in millimetres.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct Length(f64);

impl Length {
  /// Zero length.
  pub const ZERO: Self = Self(0.0);

  /// Build a length from a millimetre value.
  pub const fn from_mm(mm: f64) -> Self {
    Self(mm)
  }

  /// The value in millimetres.
  pub const fn mm(self) -> f64 {
    self.0
  }

  /// Boundary helper: build a length from `value` expressed in `system`.
  ///
  /// Only call this when reading a document or user input; never inside geometry or toolpath
  /// code.
  pub fn from_value_in(system: UnitSystem, value: f64) -> Self {
    match system {
      UnitSystem::Mm => Self(value),
      UnitSystem::Inch => Self(value * MM_PER_INCH),
    }
  }

  /// Boundary helper: express this length in `system`.
  ///
  /// Only call this when writing a document or user-facing text; never inside geometry or
  /// toolpath code.
  pub fn value_in(self, system: UnitSystem) -> f64 {
    match system {
      UnitSystem::Mm => self.0,
      UnitSystem::Inch => self.0 / MM_PER_INCH,
    }
  }

  /// Absolute value.
  pub fn abs(self) -> Self {
    Self(self.0.abs())
  }

  /// True when the value is neither infinite nor NaN.
  pub fn is_finite(self) -> bool {
    self.0.is_finite()
  }

  /// The larger of two lengths.
  pub fn max(self, other: Self) -> Self {
    Self(self.0.max(other.0))
  }

  /// The smaller of two lengths.
  pub fn min(self, other: Self) -> Self {
    Self(self.0.min(other.0))
  }
}

impl Add for Length {
  type Output = Self;
  fn add(self, rhs: Self) -> Self {
    Self(self.0 + rhs.0)
  }
}

impl Sub for Length {
  type Output = Self;
  fn sub(self, rhs: Self) -> Self {
    Self(self.0 - rhs.0)
  }
}

impl Neg for Length {
  type Output = Self;
  fn neg(self) -> Self {
    Self(-self.0)
  }
}

impl Mul<f64> for Length {
  type Output = Self;
  fn mul(self, rhs: f64) -> Self {
    Self(self.0 * rhs)
  }
}

impl Div<f64> for Length {
  type Output = Self;
  fn div(self, rhs: f64) -> Self {
    Self(self.0 / rhs)
  }
}

/// An angle stored in radians.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct Angle(f64);

impl Angle {
  /// Zero angle.
  pub const ZERO: Self = Self(0.0);

  /// Build an angle from radians.
  pub const fn from_radians(radians: f64) -> Self {
    Self(radians)
  }

  /// Build an angle from degrees.
  pub fn from_degrees(degrees: f64) -> Self {
    Self(degrees.to_radians())
  }

  /// The value in radians.
  pub const fn radians(self) -> f64 {
    self.0
  }

  /// The value in degrees.
  pub fn degrees(self) -> f64 {
    self.0.to_degrees()
  }
}

/// Fixed-decimal, locale-independent number formatting.
///
/// One `Format` value corresponds to one `VAR` declaration of a post-processor: the emitter asks
/// for a decimal count and gets text that never depends on the host locale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Format {
  decimals: u8,
}

impl Format {
  /// Format with `decimals` digits after the separator.
  ///
  /// Decimal counts above 15 are clamped: `f64` cannot carry more precision than that, and
  /// silently padding zeroes would misrepresent the value.
  pub const fn fixed(decimals: u8) -> Self {
    Self {
      decimals: if decimals > 15 { 15 } else { decimals },
    }
  }

  /// The configured decimal count.
  pub const fn decimals(self) -> u8 {
    self.decimals
  }

  /// Round `value` to this format's precision.
  ///
  /// Ties round **away from zero**, so a 0.5 mm feature never silently shrinks to zero. A result
  /// that rounds to zero is normalised to positive zero, so text never carries a `-0` sign.
  pub fn round(self, value: f64) -> f64 {
    let scale = 10f64.powi(i32::from(self.decimals));
    let scaled = (value * scale).round();
    if scaled == 0.0 || !scaled.is_finite() {
      0.0
    } else {
      scaled / scale
    }
  }

  /// Render `value` as fixed-decimal text using `.` as the separator.
  pub fn apply(self, value: f64) -> String {
    let rounded = self.round(value);
    format!("{:.*}", usize::from(self.decimals), rounded)
  }
}

impl fmt::Display for Format {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "fixed({})", self.decimals)
  }
}
