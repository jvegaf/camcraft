//! Drilling: plunge at the centre of each selected circle, with pecks and retracts.
//!
//! The centre of a hole is the shared centre of its contour's arcs, so a circle drills where the user
//! drew it and anything else is refused by name instead of being drilled at a guessed point.

use std::f64::consts::TAU;

use cam_geom::{Contour, Point2, Segment};
use cam_model::{DrillParams, Job, RetractMode};
use cam_move::{Move, Point3};
use cam_units::Length;

use crate::{machine_point, pass_depths, ToolSpec, ToolpathError};

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
/// Holes are drilled in a nearest-neighbour tour seeded at the lowest-Y-then-lowest-X hole, with
/// `Vec` scans only: the same input always produces the same moves (S7). Every hole is entered at
/// the job's safe Z, plunged in pecks, and left at safe Z, so the tool is never down while it travels
/// between holes (A7, A8, D-M4-6).
pub fn drill(request: &DrillRequest<'_>) -> Result<Drillpath, ToolpathError> {
  if request.holes.is_empty() {
    return Err(ToolpathError::NoHoles);
  }
  if request.tool.diameter.mm() <= 0.0 {
    return Err(ToolpathError::InvalidToolDiameter);
  }
  let params = request.params;
  let peck = params.peck_depth.mm();
  if peck < 0.0 {
    return Err(ToolpathError::InvalidPeckDepth { peck_depth: peck });
  }
  if params.cut_depth.mm() <= params.start_depth.mm() {
    return Err(ToolpathError::InvalidDepthRange {
      cut_depth: params.cut_depth.mm(),
      start_depth: params.start_depth.mm(),
    });
  }

  // A zero peck is one plunge to the cut depth; anything else is chunked like a profile pass so
  // both strategies reach the same depths for the same numbers.
  let pecks = if peck == 0.0 {
    vec![params.cut_depth.mm()]
  } else {
    pass_depths(params.start_depth.mm(), params.cut_depth.mm(), peck)?
  };

  let machine: Vec<Point2> = request
    .holes
    .iter()
    .map(|hole| machine_point(request.job, hole.centre))
    .collect();
  let tour = tour(request.holes, &machine);

  let safe_z = request.job.safe_z;
  let mut moves = Vec::new();
  for &index in &tour.order {
    let centre = machine[index];
    let at = Point3 {
      x: Length::from_mm(centre.x),
      y: Length::from_mm(centre.y),
      z: safe_z,
    };
    moves.push(Move::Rapid { to: at });

    for (position, depth) in pecks.iter().enumerate() {
      moves.push(Move::Feed {
        to: Point3 {
          z: Length::from_mm(-depth),
          ..at
        },
      });
      // The retract that ends the hole always clears it, whatever the peck mode says.
      let last = position + 1 == pecks.len();
      let retract_z = if last {
        safe_z
      } else {
        match params.retract {
          RetractMode::Full => safe_z,
          // Measured from the bottom of the peck just finished, not from the surface.
          RetractMode::Amount { amount } => Length::from_mm(-depth + amount.mm()),
        }
      };
      moves.push(Move::Retract {
        to: Point3 { z: retract_z, ..at },
      });
    }
  }

  let order: Vec<String> = tour
    .order
    .iter()
    .map(|&index| request.holes[index].entity.clone())
    .collect();
  Ok(Drillpath {
    // The drill bank is T4's task: every hole is cut by the head as drawn.
    drill_index: vec![None; order.len()],
    moves,
    order,
    rapid_length: Length::from_mm(tour.travel),
  })
}

/// The order the holes are drilled in, and the head travel that order costs.
struct Tour {
  /// Indices into the request's holes, in cutting order.
  order: Vec<usize>,
  /// Summed XY distance between consecutive holes, measured from the machine origin for the first.
  travel: f64,
}

/// Distances closer than this are a tie, resolved by entity id so the tour never depends on the
/// order the entities happened to be listed in.
const TIE: f64 = 1.0e-9;

/// Nearest-neighbour tour from the lowest-Y-then-lowest-X hole, by scanning the holes every step.
///
/// Ordering is decided on the document centre the user drew, so "lowest y" means what the drawing
/// shows whatever the job's origin corner is; the travel figure is measured in machine space, which
/// the origin transform translates or flips without changing any distance.
fn tour(holes: &[DrillHole], machine: &[Point2]) -> Tour {
  let mut visited = vec![false; holes.len()];
  let mut order = Vec::with_capacity(holes.len());
  let mut travel = 0.0;

  // Lowest y, then lowest x, then entity id: the seed is a property of the geometry, not of the
  // document's listing order.
  let mut current = 0;
  for index in 1..holes.len() {
    let candidate = holes[index].centre;
    let best = holes[current].centre;
    let lower_y = candidate.y < best.y - TIE;
    let same_y = (candidate.y - best.y).abs() <= TIE;
    let lower_x = candidate.x < best.x - TIE;
    let same_x = (candidate.x - best.x).abs() <= TIE;
    let lower = lower_y || (same_y && lower_x);
    let tied = same_y && same_x;
    if lower || (tied && holes[index].entity < holes[current].entity) {
      current = index;
    }
  }
  visited[current] = true;

  let mut previous = Point2::new(0.0, 0.0);
  for step in 0..holes.len() {
    if step > 0 {
      let mut nearest: Option<usize> = None;
      let mut distance = f64::INFINITY;
      for index in 0..holes.len() {
        if visited[index] {
          continue;
        }
        let candidate = holes[current].centre.distance_to(holes[index].centre);
        let replace = match nearest {
          None => true,
          Some(existing) => {
            candidate < distance - TIE
              || ((candidate - distance).abs() <= TIE
                && holes[index].entity < holes[existing].entity)
          }
        };
        if replace {
          nearest = Some(index);
          distance = candidate;
        }
      }
      current = nearest.expect("holes are left to visit inside the tour");
      visited[current] = true;
    }
    travel += previous.distance_to(machine[current]);
    previous = machine[current];
    order.push(current);
  }

  Tour { order, travel }
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
