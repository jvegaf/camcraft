//! The machining move vocabulary and move streams.
//!
//! This crate is the contract between the toolpath engine and the post-processor emitter: the
//! toolpath produces a [`MoveStream`], the emitter turns it into machine text. Keeping the
//! vocabulary here means neither side depends on the other, and each can be tested alone.
//!
//! Must not depend on `cam-post`, `cam-toolpath`, `cam-store` or any UI crate.

#![warn(missing_docs)]

use cam_geom::Point2;
use cam_units::Length;

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

/// One toolpath: a run of moves with a single tool and feed rates.
#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
  /// Name, emitted as a comment and used in reports.
  pub name: String,
  /// Tool number.
  pub tool_number: u32,
  /// Cutting diameter of the tool, in millimetres.
  ///
  /// The emitter ignores it — a post-processor has no block that states a tool's diameter — but the
  /// simulation needs the footprint a move sweeps, and taking it from the segment keeps the tool
  /// joined to the moves that use it (M5 S2).
  pub tool_diameter: Length,
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
