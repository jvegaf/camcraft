//! Explicit tolerance policy for the geometry kernel.
//!
//! A single object carries every tolerance an operation may need, so no public operation invents its
//! own literal and no tolerance is read from ambient state. This is the kernel's answer to E0.3 and
//! to §5.2 of `docs/SYSTEM_DESIGN_CAMCRAFT.md`: tolerance is a parameter, never a literal and never
//! an environment.

/// How arcs are handled when an operation must choose between keeping them exact and flattening.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ArcPolicy {
  /// Keep arcs as arcs wherever the operation can.
  Preserve,
  /// Flatten arcs to chords, staying within the given chord error in millimetres.
  Flatten {
    /// Maximum chord error, in millimetres.
    chord: f64,
  },
}

/// How (and whether) contours are simplified.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Simplify {
  /// No simplification.
  Off,
  /// Drop points that stay collinear within `epsilon`, in millimetres.
  Collinear {
    /// Collinearity tolerance, in millimetres.
    epsilon: f64,
  },
  /// Douglas–Peucker simplification within `epsilon`, in millimetres.
  DouglasPeucker {
    /// Maximum deviation, in millimetres.
    epsilon: f64,
  },
}

/// The tolerance policy a geometric operation runs under.
///
/// `chord` bounds the error of a chord against the true curve when tessellating; `epsilon` bounds
/// point and area comparisons. `arc` and `simplify` describe how arcs and simplification are treated
/// when an operation has that choice to make. Nothing in the kernel constructs a policy on its own:
/// callers pass one in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TolerancePolicy {
  /// Maximum chord error when tessellating, in millimetres.
  pub chord: f64,
  /// Comparison tolerance for points and areas, in millimetres.
  pub epsilon: f64,
  /// Arc handling policy.
  pub arc: ArcPolicy,
  /// Simplification policy.
  pub simplify: Simplify,
}

/// Canonical comparison/chaining tolerance, in millimetres (one micrometre).
pub const DEFAULT_EPSILON: f64 = 1.0e-6;

/// Canonical chord/tessellation tolerance, in millimetres.
pub const DEFAULT_CHORD: f64 = 0.001;

impl TolerancePolicy {
  /// The canonical policy: one-micrometre comparisons, one-micrometre-of-a-millimetre chord error
  /// is too tight for general tessellation so chord defaults to one thousandth of a millimetre,
  /// arcs preserved, no simplification.
  pub const fn standard() -> Self {
    Self {
      chord: DEFAULT_CHORD,
      epsilon: DEFAULT_EPSILON,
      arc: ArcPolicy::Preserve,
      simplify: Simplify::Off,
    }
  }

  /// A policy that uses the same value for comparison and for chord error.
  pub const fn uniform(tolerance: f64) -> Self {
    Self {
      chord: tolerance,
      epsilon: tolerance,
      arc: ArcPolicy::Preserve,
      simplify: Simplify::Off,
    }
  }

  /// Return a copy with a different epsilon (comparison) tolerance.
  pub const fn with_epsilon(self, epsilon: f64) -> Self {
    Self { epsilon, ..self }
  }

  /// Return a copy with a different chord (tessellation) tolerance.
  pub const fn with_chord(self, chord: f64) -> Self {
    Self { chord, ..self }
  }

  /// Return a copy with a different arc policy.
  pub const fn with_arc(self, arc: ArcPolicy) -> Self {
    Self { arc, ..self }
  }

  /// Return a copy with a different simplification policy.
  pub const fn with_simplify(self, simplify: Simplify) -> Self {
    Self { simplify, ..self }
  }

  /// A policy whose epsilon and chord are both `tolerance` millimetres.
  pub const fn of(tolerance: f64) -> Self {
    Self::uniform(tolerance)
  }

  /// A policy using `epsilon` for comparisons and the canonical chord tolerance for tessellation.
  pub const fn with_default_chord(epsilon: f64) -> Self {
    Self {
      chord: DEFAULT_CHORD,
      epsilon,
      arc: ArcPolicy::Preserve,
      simplify: Simplify::Off,
    }
  }

  /// A policy using the canonical epsilon and `chord` for tessellation.
  pub const fn with_default_epsilon(chord: f64) -> Self {
    Self {
      chord,
      epsilon: DEFAULT_EPSILON,
      arc: ArcPolicy::Preserve,
      simplify: Simplify::Off,
    }
  }
}

impl Default for TolerancePolicy {
  fn default() -> Self {
    Self::standard()
  }
}
