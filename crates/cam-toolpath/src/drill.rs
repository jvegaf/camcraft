//! Drilling: plunge at the centre of each selected circle, with pecks and retracts.
//!
//! The centre of a hole is the shared centre of its contour's arcs, so a circle drills where the user
//! drew it and anything else is refused by name instead of being drilled at a guessed point.

use std::f64::consts::TAU;

use cam_geom::{Contour, Point2, Segment};
use cam_model::{DrillParams, Job};
use cam_move::Move;
use cam_units::Length;

use crate::{ToolSpec, ToolpathError};

/// One hole to drill, with the entity it came from so refusals can name it.
#[derive(Clone, Debug, PartialEq)]
pub struct DrillHole {
  /// Id of the entity that defined the hole.
  pub entity: String,
  /// Hole centre in document coordinates.
  pub centre: Point2,
}

/// Everything a drilling pass needs.
#[derive(Clone, Copy, Debug)]
pub struct DrillRequest<'a> {
  /// The holes, in document order.
  pub holes: &'a [DrillHole],
  /// Peck depth, retract mode and depths.
  pub params: &'a DrillParams,
  /// The tool doing the drilling.
  pub tool: &'a ToolSpec,
  /// Job setup, used to map document coordinates onto machine coordinates.
  pub job: &'a Job,
}

/// What a drilling pass produced, beyond the moves themselves.
#[derive(Clone, Debug, PartialEq)]
pub struct Drillpath {
  /// The moves, in cutting order.
  pub moves: Vec<Move>,
  /// Entity ids in the order they are drilled.
  pub order: Vec<String>,
  /// Bank drill that cut each hole, parallel to [`Self::order`]. `None` without a bank.
  pub drill_index: Vec<Option<u32>>,
  /// Total XY distance the tool travels between holes.
  pub rapid_length: Length,
}

/// Compute the moves for a drilling pass.
///
/// Not implemented yet: M4 T3 fills this module.
pub fn drill(request: &DrillRequest<'_>) -> Result<Drillpath, ToolpathError> {
  let _ = request;
  Err(ToolpathError::NotImplemented)
}

/// The centre of a contour that is a circle, or `None` when it is any other shape.
///
/// A circle is closed, made only of arcs, all sharing one centre, sweeping a full turn in total. That
/// is exactly what [`cam_geom::primitive::circle`] builds, so a drilled hole lands where the user
/// drew it instead of at a bounding-box guess (D-M4-5).
pub fn circle_centre(contour: &Contour) -> Option<Point2> {
  if !contour.is_closed() {
    return None;
  }
  let mut centre: Option<Point2> = None;
  let mut sweep = 0.0;
  for segment in contour.segments() {
    let Segment::Arc(arc) = segment else {
      return None;
    };
    match centre {
      None => centre = Some(arc.center()),
      Some(known) => {
        if known.distance_to(arc.center()) > 1.0e-9 {
          return None;
        }
      }
    }
    sweep += arc.sweep();
  }
  let centre = centre?;
  // Half-turn arcs sum to one turn either way; a two-thirds circle must not pass as a hole.
  if (sweep.abs() - TAU).abs() > 1.0e-6 {
    return None;
  }
  Some(centre)
}
