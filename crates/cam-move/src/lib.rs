//! The machining move vocabulary and move streams.
//!
//! This crate is the contract between the toolpath engine and the post-processor emitter: the
//! toolpath produces a [`MoveStream`], the emitter turns it into machine text. Keeping the
//! vocabulary here means neither side depends on the other, and each can be tested alone.
//!
//! Must not depend on `cam-post`, `cam-toolpath`, `cam-store` or any UI crate.

#![warn(missing_docs)]

use cam_geom::Point2;
use cam_units::{Angle, Length};

/// A position in machine space.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point3 {
  /// X position, in millimetres.
  pub x: Length,
  /// Y position, in millimetres.
  pub y: Length,
  /// Z position, in millimetres. Positive is up; cutting depths are positive magnitudes.
  pub z: Length,
}

impl Point3 {
  /// The machine origin.
  pub const ORIGIN: Self = Self {
    x: Length::ZERO,
    y: Length::ZERO,
    z: Length::ZERO,
  };

  /// Build a position from millimetre components.
  pub const fn new(x: f64, y: f64, z: f64) -> Self {
    Self {
      x: Length::from_mm(x),
      y: Length::from_mm(y),
      z: Length::from_mm(z),
    }
  }

  /// The XY projection.
  pub fn xy(self) -> Point2 {
    Point2::new(self.x.mm(), self.y.mm())
  }
}

/// One machine movement.
///
/// The kinds are explicit rather than inferred from coordinates: "the Z went up, so this must be a
/// retract" is exactly the kind of guessing that produces a crash move in a real program.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Move {
  /// Positioning move at rapid rate, usually at clearance height.
  Rapid {
    /// Destination.
    to: Point3,
  },
  /// Cutting move at feed rate.
  Feed {
    /// Destination.
    to: Point3,
  },
  /// Circular cutting move in the XY plane at constant Z.
  Arc {
    /// Destination.
    to: Point3,
    /// Arc centre in XY.
    center: Point2,
    /// True for counter-clockwise.
    ccw: bool,
  },
  /// Lift to clearance. Distinct from a rapid so the emitter can use the retract block.
  Retract {
    /// Destination.
    to: Point3,
  },
}

impl Move {
  /// The position this move ends at.
  pub fn destination(self) -> Point3 {
    match self {
      Self::Rapid { to } | Self::Feed { to } | Self::Arc { to, .. } | Self::Retract { to } => to,
    }
  }
}

/// The shape of a tool's cutting end, which decides the footprint a move sweeps.
///
/// A move's footprint is not a property of the move: a cylinder sweeps the same width at every depth,
/// while a V-bit's radius grows with depth below its tip. Carrying the shape next to the moves keeps the
/// tool joined to the geometry that uses it (M5 S2) and lets the simulation sweep the shape a program
/// would really cut (M6 A8). The emitter ignores it: a post-processor has no block that states a tool's
/// shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ToolShape {
  /// A cylindrical cutter: one radius at every depth.
  Cylinder {
    /// Cutting diameter.
    diameter: Length,
  },
  /// A V-bit: a cone whose radius grows with depth below the tip.
  VBit {
    /// Full included angle of the cone, measured across the tip.
    included_angle: Angle,
    /// Width of the flat at the tip; zero for a sharp point.
    flat_diameter: Length,
  },
}

impl ToolShape {
  /// A cylindrical cutter of `diameter`.
  pub const fn cylinder(diameter: Length) -> Self {
    Self::Cylinder { diameter }
  }

  /// A V-bit with an `included_angle` and a `flat_diameter` tip.
  pub const fn v_bit(included_angle: Angle, flat_diameter: Length) -> Self {
    Self::VBit {
      included_angle,
      flat_diameter,
    }
  }

  /// Half the included angle of a cone, in radians. Zero for a cylinder, whose radius never grows.
  pub fn half_angle(self) -> f64 {
    match self {
      Self::Cylinder { .. } => 0.0,
      Self::VBit { included_angle, .. } => (included_angle.radians() / 2.0).max(0.0),
    }
  }

  /// The radius of the footprint `depth_below_tip` millimetres below the tip.
  ///
  /// A negative or non-finite depth is read as zero: the tool cannot be above its own tip, and the
  /// answer must stay finite where a caller hands in a surface it did not clamp.
  pub fn radius_at(self, depth_below_tip: f64) -> f64 {
    let depth = if depth_below_tip.is_finite() {
      depth_below_tip.max(0.0)
    } else {
      0.0
    };
    match self {
      Self::Cylinder { diameter } => (diameter.mm() / 2.0).max(0.0),
      Self::VBit { flat_diameter, .. } => {
        (flat_diameter.mm() / 2.0).max(0.0) + depth * self.half_angle().tan()
      }
    }
  }

  /// How far above its tip the tool's own surface first reaches `radius`.
  ///
  /// This is the quantity a sweep needs. A cone at tip depth `d` removes material at horizontal
  /// distance `x` down to `d − rise(x)`, and reaches nothing at all where `rise(x) ≥ d`; a cylinder is
  /// vertical, so it reaches its own radius at the tip itself and removes the full depth everywhere
  /// inside its footprint.
  pub fn rise_at_radius(self, radius: f64) -> f64 {
    if !radius.is_finite() {
      return f64::INFINITY;
    }
    match self {
      Self::Cylinder { .. } => 0.0,
      Self::VBit { flat_diameter, .. } => {
        let slope = self.half_angle().tan();
        if slope <= 0.0 {
          return f64::INFINITY;
        }
        ((radius - (flat_diameter.mm() / 2.0).max(0.0)) / slope).max(0.0)
      }
    }
  }
}

/// One toolpath: a run of moves with a single tool and feed rates.
#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
  /// Name, emitted as a comment and used in reports.
  pub name: String,
  /// Tool number.
  pub tool_number: u32,
  /// The shape of the tool that cuts these moves.
  ///
  /// The emitter ignores it — a post-processor has no block that states a tool's shape — but the
  /// simulation needs the footprint a move sweeps, and taking it from the segment keeps the tool
  /// joined to the moves that use it (M5 S2). It was a bare diameter until M6 needed a V-bit's cone.
  pub tool: ToolShape,
  /// Cutting feed rate in millimetres per second.
  pub feed_rate: f64,
  /// Plunge feed rate in millimetres per second.
  pub plunge_rate: f64,
  /// Spindle speed in rpm.
  pub spindle_speed: u32,
  /// The moves, in order.
  pub moves: Vec<Move>,
}

/// A whole program: the segments to emit, plus the machine's clearance references.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MoveStream {
  /// Segments, emitted in order.
  pub segments: Vec<Segment>,
  /// Safe Z height.
  pub safe_z: Length,
  /// Machine home position in XY, used by the header and footer blocks.
  pub home: Point2,
}

impl MoveStream {
  /// An empty stream for a machine whose clearance height is `safe_z`.
  pub fn new(safe_z: Length, home: Point2) -> Self {
    Self {
      segments: Vec::new(),
      safe_z,
      home,
    }
  }

  /// Number of moves across every segment.
  pub fn move_count(&self) -> usize {
    self
      .segments
      .iter()
      .map(|segment| segment.moves.len())
      .sum()
  }

  /// True when there is nothing to emit.
  pub fn is_empty(&self) -> bool {
    self.move_count() == 0
  }

  /// Total cutting length in XY, ignoring rapids and retracts.
  pub fn cut_length(&self) -> Length {
    let mut total = 0.0;
    for segment in &self.segments {
      let mut previous: Option<Point3> = None;
      for movement in &segment.moves {
        let destination = movement.destination();
        if let Some(from) = previous {
          if matches!(movement, Move::Feed { .. } | Move::Arc { .. }) {
            total += from.xy().distance_to(destination.xy());
          }
        }
        previous = Some(destination);
      }
    }
    Length::from_mm(total)
  }
}
