//! Drilling: plunge at the centre of each selected circle, with pecks and retracts.
//!
//! The centre of a hole is the shared centre of its contour's arcs, so a circle drills where the user
//! drew it and anything else is refused by name instead of being drilled at a guessed point.
//!
//! A drill bank ([`DrillParams::bank`]) is a gang of drills mounted a fixed `pitch` apart along X. The
//! machine cannot swap drills, so the head moves instead: drill `k` is put over a hole at `x` by
//! parking the head at `x - k * pitch`. Picking `k` for a hole is therefore *selection*, not a machine
//! command, and this module emits nothing else for it — no `G` or `M` code, no toolchange, no tool
//! number — because [`cam_move`] has no bank vocabulary at all and drill-bank hardware integration is
//! deferred by the plan (S9, A9). What a caller gets instead is the per-hole drill index and the head
//! travel that selection costs.

use std::f64::consts::TAU;

use cam_geom::{Contour, Point2, Segment};
use cam_model::{DrillBank, DrillParams, Job, RetractMode};
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
  /// Total XY distance the holes are apart, in cutting order, counted from the machine origin.
  pub rapid_length: Length,
  /// Total XY distance the *head* travels between holes, counted from machine `(0, 0)` for the first.
  ///
  /// Without a bank the head visits the holes themselves, so this equals [`Self::rapid_length`].
  /// With a bank it is what the compensated program actually costs, and it is smaller.
  pub bank_travel: Length,
}

/// Compute the moves for a drilling pass.
///
/// Holes are drilled in a nearest-neighbour tour seeded at the lowest-Y-then-lowest-X hole, with
/// `Vec` scans only: the same input always produces the same moves (S7). Every hole is entered at
/// the job's safe Z, plunged in pecks, and left at safe Z, so the tool is never down while it travels
/// between holes (A7, A8, D-M4-6).
///
/// With a drill bank, each hole in that tour picks the drill whose head position is nearest the
/// previous head, and the moves name the compensated head position instead of the hole (A9). The
/// order the holes are drilled in is unchanged: a bank selects, it does not re-plan.
pub fn drill(request: &DrillRequest<'_>) -> Result<Drillpath, ToolpathError> {
  let params = request.params;
  // A bank of no drills, or of drills sitting on top of each other, has nothing to select from, so
  // it is refused before a single move exists.
  if let Some(bank) = params.bank {
    if bank.count == 0 || bank.pitch.mm() <= 0.0 {
      return Err(ToolpathError::InvalidBank);
    }
  }
  if request.holes.is_empty() {
    return Err(ToolpathError::NoHoles);
  }
  if request.tool.diameter.mm() <= 0.0 {
    return Err(ToolpathError::InvalidToolDiameter);
  }
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

  // Where the head goes for each hole in the tour, and which drill that puts over it. Without a bank
  // the head goes to the hole itself, which is why `bank_travel == rapid_length` there.
  let mut heads = Vec::with_capacity(tour.order.len());
  let mut drill_index = Vec::with_capacity(tour.order.len());
  let mut head_travel = 0.0;
  let mut previous_head = Point2::new(0.0, 0.0);
  for &index in &tour.order {
    let centre = machine[index];
    let (head, drill) = match params.bank {
      None => (centre, None),
      Some(bank) => {
        let chosen = nearest_drill(centre.x - previous_head.x, &bank);
        (
          Point2::new(centre.x - f64::from(chosen) * bank.pitch.mm(), centre.y),
          Some(chosen),
        )
      }
    };
    // Counted from the machine origin, so the head pays for the travel onto the part.
    head_travel += previous_head.distance_to(head);
    previous_head = head;
    heads.push(head);
    drill_index.push(drill);
  }

  let safe_z = request.job.safe_z;
  let mut moves = Vec::new();
  for head in &heads {
    let at = Point3 {
      x: Length::from_mm(head.x),
      y: Length::from_mm(head.y),
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
    moves,
    order,
    drill_index,
    rapid_length: Length::from_mm(tour.travel),
    bank_travel: Length::from_mm(head_travel),
  })
}

/// The bank drill whose head position is nearest to a hole `dx` millimetres along X from the head.
///
/// Drill `k` sits `k * pitch` to the left of the head's reference drill, so cutting a hole `dx` ahead
/// asks for the whole number of pitches nearest `dx`, clamped to the drills the bank actually has. A
/// tie keeps the lower index, so the choice depends on the numbers and nothing else (S7).
fn nearest_drill(dx: f64, bank: &DrillBank) -> u32 {
  let pitch = bank.pitch.mm();
  let last = f64::from(bank.count.saturating_sub(1));
  let pitches = dx / pitch;
  let chosen = if pitches <= 0.0 {
    0.0
  } else if pitches >= last {
    last
  } else {
    let below = pitches.floor();
    if pitches - below > 0.5 {
      below + 1.0
    } else {
      below
    }
  };
  chosen as u32
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
