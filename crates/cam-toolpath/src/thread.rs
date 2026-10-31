//! Thread milling: one helical descent around a circle, one pitch per revolution.
//!
//! A thread is not a pocket and not a profile: the tool orbits a circle at a fixed radius and drops
//! `pitch` millimetres per revolution, from the surface to `-cut_depth`, then retracts (M6 A7). That
//! makes it the one strategy whose program is not a stack of constant-Z moves, which is why it waited
//! for [`cam_move::Move::Helix`]: a descent around a circle cut as a chord or as a flat circle is a
//! defect, not an approximation, and the post contract grew the four helical block kinds to carry it
//! (D-M6-8).
//!
//! # Where the tool runs
//!
//! The selected entity is a **circle** of diameter `D` and the tool is one of diameter `t`, and between
//! them they fix the radius of the tool centre's own orbit:
//!
//! - an **internal** thread — a thread in a hole — runs at `(D − t) / 2`, so the tool's disc reaches the
//!   hole's wall at `D/2` from the inside and stops `t` short of the axis;
//! - an **external** thread — a thread on a boss — runs at `(D + t) / 2`, so the same disc reaches the
//!   boss's wall at `D/2` from the outside.
//!
//! An internal thread whose tool is as wide as the hole has no orbit at all — `(D − t)/2` is zero or
//! negative — and is refused by name rather than turned into a plunge. Nothing about the diameter is
//! stored in the document: the circle is the entity the toolpath names and the tool is the library's.
//!
//! # The descent
//!
//! `cut_depth / pitch` revolutions, from the surface to `-cut_depth`. The count is usually not a whole
//! number, and the move vocabulary covers at most one turn per move, so the descent is emitted as one
//! **full** helical move per whole revolution followed by one move carrying the fraction of a turn that
//! reaches the depth — a full turn being the move whose destination is its own start point, exactly as a
//! full-circle arc is. Every move is one continuous helix, so the controller interpolates what the
//! program says: the descent never leaves the orbit and never arrives at a depth in one jump.

use std::f64::consts::TAU;

use cam_geom::{Contour, Point2, Segment};
use cam_model::{Job, ThreadDirection, ThreadParams};
use cam_move::{Move, Point3};
use cam_units::Length;

use crate::{circle_centre, machine_point, ToolSpec, ToolpathError};

/// Everything a thread needs.
#[derive(Clone, Copy, Debug)]
pub struct ThreadRequest<'a> {
  /// Id of the entity being milled.
  ///
  /// The strategy takes no entity list of its own — a thread is one circle and one toolpath per
  /// selection, and the caller loops — but it needs a name for the entity in the one refusal only the
  /// shape can produce ([`ToolpathError::NotACircle`]).
  pub entity: &'a str,
  /// The circle to mill: a thread runs around a circle and nothing else.
  pub contour: &'a Contour,
  /// Pitch, direction and the depth range.
  pub params: &'a ThreadParams,
  /// The tool doing the cutting, which fixes the orbit's radius.
  pub tool: &'a ToolSpec,
  /// Job setup, used to map document coordinates onto machine coordinates.
  pub job: &'a Job,
}

/// Compute the moves for one thread.
///
/// An approach rapid to safe Z over the orbit's `+x` point, a feed down to the surface in place, the
/// helical descent, and a retract from wherever the descent ended. Four refusals, all of them input
/// errors rather than clamps: a `cut_depth` at or below `start_depth` (`InvalidDepthRange`), a pitch that
/// is not a positive number (`InvalidPitch`), a tool with no diameter (`InvalidToolDiameter`), a
/// selection that is not a circle (`NotACircle`), and an internal thread whose tool does not fit the
/// hole (`ThreadToolTooWide`).
///
/// `start_depth` bounds the authored depth rather than moving the entry, exactly as it does for an
/// engrave and a flute: a premachined surface is air as far as this pass is concerned, so the descent
/// still runs from the stock's face to `-cut_depth`.
pub fn thread(request: &ThreadRequest<'_>) -> Result<Vec<Move>, ToolpathError> {
  let params = request.params;
  if params.cut_depth.mm() <= params.start_depth.mm() {
    return Err(ToolpathError::InvalidDepthRange {
      cut_depth: params.cut_depth.mm(),
      start_depth: params.start_depth.mm(),
    });
  }
  let pitch = params.pitch.mm();
  if !pitch.is_finite() || pitch <= 0.0 {
    return Err(ToolpathError::InvalidPitch { pitch });
  }
  let diameter = request.tool.diameter.mm();
  if !diameter.is_finite() || diameter <= 0.0 {
    return Err(ToolpathError::InvalidToolDiameter);
  }
  let (centre, circle) =
    circle_geometry(request.contour).ok_or_else(|| ToolpathError::NotACircle {
      entity: request.entity.to_owned(),
    })?;
  let orbit = match params.direction {
    ThreadDirection::Internal => {
      // `(D − t)/2` is the orbit's radius, and it has none when the tool fills the hole: a 12 mm tool
      // in a 12 mm hole has nowhere inside to put its centre, and a bigger one is bigger than the hole
      // it was asked to cut inside. Refused by name rather than collapsed into a plunge on the axis.
      if diameter >= circle {
        return Err(ToolpathError::ThreadToolTooWide {
          diameter,
          hole: circle,
        });
      }
      (circle - diameter) / 2.0
    }
    ThreadDirection::External => (circle + diameter) / 2.0,
  };

  let centre = machine_point(request.job, centre);
  let safe_z = request.job.safe_z;
  let depth = params.cut_depth.mm();
  // The orbit's `+x` point, which is where the descent starts and where every whole turn ends. It is
  // computed once and reused, so a full turn's destination is bit-for-bit its start point — which is
  // what makes it one revolution to the sweep, to the preview and to `cut_length` alike.
  let start = Point2::new(centre.x + orbit, centre.y);
  let at = |x: f64, y: f64, z: f64| Point3 {
    x: Length::from_mm(x),
    y: Length::from_mm(y),
    z: Length::from_mm(z),
  };
  let mut moves = vec![
    Move::Rapid {
      to: at(start.x, start.y, safe_z.mm()),
    },
    // Onto the surface first: the helix is the descent, so nothing plunges to the depth and then
    // descends again from there.
    Move::Feed {
      to: at(start.x, start.y, 0.0),
    },
  ];

  // `cut_depth / pitch` revolutions, one move per whole turn and one last move for the fraction.
  let turns = depth / pitch;
  let whole = if turns.is_finite() {
    turns.floor().max(0.0)
  } else {
    0.0
  };
  for turn in 0..whole as usize {
    moves.push(Move::Helix {
      to: at(start.x, start.y, -(pitch * (turn + 1) as f64)),
      center: centre,
      ccw: true,
    });
  }
  let remainder = depth - pitch * whole;
  if remainder > 1.0e-9 {
    // The fraction of a turn that reaches the depth, by angle: the same fraction of the descent's Z, so
    // one revolution still drops one pitch.
    let angle = (remainder / pitch) * TAU;
    let end = Point2::new(
      centre.x + orbit * angle.cos(),
      centre.y + orbit * angle.sin(),
    );
    moves.push(Move::Helix {
      to: at(end.x, end.y, -depth),
      center: centre,
      ccw: true,
    });
  }

  let last = moves
    .last()
    .map(|movement| movement.destination())
    .unwrap_or(Point3::ORIGIN);
  moves.push(Move::Retract {
    to: Point3 {
      x: last.x,
      y: last.y,
      z: safe_z,
    },
  });
  Ok(moves)
}

/// The centre and the diameter of a contour that is a circle, or `None` when it is any other shape.
///
/// [`circle_centre`] already decides whether the contour *is* a circle — closed, all arcs, one centre,
/// a full turn of sweep — so the diameter only has to be read off one of its arcs.
fn circle_geometry(contour: &Contour) -> Option<(Point2, f64)> {
  let centre = circle_centre(contour)?;
  let radius = contour
    .segments()
    .iter()
    .find_map(|segment| match segment {
      Segment::Arc(arc) => Some(arc.radius()),
      Segment::Line(_) => None,
    })?;
  if !radius.is_finite() || radius <= 0.0 {
    return None;
  }
  Some((centre, 2.0 * radius))
}
