//! Trimming a contour against a cutter.
//!
//! The cut leaves one span removed and the rest of the contour in pieces. Which span is removed is
//! fixed and documented rather than chosen interactively: the span between the two cut points walking
//! the contour forward from the first — the middle of an open contour, and the forward half of a
//! closed one. Determinism is what makes a trim reproducible in a test and in a bug report.
//!
//! Anything that would make the choice a guess is refused: no cut points, one tangent touch, four
//! crossings, or two cut points at the same place.

use crate::arc::Arc;
use crate::contour::Contour;
use crate::intersect::{self, Intersections};
use crate::point::Point2;
use crate::segment::Segment;
use crate::GeomError;

use std::f64::consts::TAU;

/// How close to a segment boundary a fractional position may be before it counts as the boundary.
const FRACTION_EPSILON: f64 = 1.0e-9;

/// The cut points a trim needs, and no more.
const REQUIRED_CUTS: usize = 2;

/// What a trim produced.
#[derive(Clone, Debug, PartialEq)]
pub struct TrimOutcome {
  /// The pieces of the target that survive, in their order along the original contour.
  pub kept: Vec<Contour>,
  /// The span that was cut away.
  pub removed: Contour,
}

/// Remove the span of `target` between its two crossings with `cutter`.
///
/// # Errors
///
/// [`GeomError::EditRefused`] unless the cutter crosses the target at exactly two usable points. A
/// tangential touch and a coincident overlap are not usable cut points: neither divides a contour.
pub fn trim(target: &Contour, cutter: &Contour, tolerance: f64) -> Result<TrimOutcome, GeomError> {
  let segments = target.segments();
  let cuts = collect_cuts(&segments, cutter, tolerance);
  if cuts.len() != REQUIRED_CUTS {
    return Err(GeomError::EditRefused {
      reason: "trim needs exactly two cut points where the cutter crosses the target",
    });
  }
  let (first, second) = (cuts[0], cuts[1]);
  if first
    .physical(&segments)
    .is_near(second.physical(&segments), tolerance)
  {
    return Err(GeomError::EditRefused {
      reason: "trim needs exactly two cut points where the cutter crosses the target",
    });
  }

  let removed_segments = extract(&segments, target.is_closed(), first, second)?;
  let removed = Contour::from_segments(&removed_segments, false)?;

  let kept_segments: Vec<Vec<Segment>> = if target.is_closed() {
    vec![extract(&segments, true, second, first)?]
  } else {
    let start = Position::start();
    let end = Position::end(segments.len() - 1);
    let mut pieces = Vec::new();
    if first != start {
      pieces.push(extract(&segments, false, start, first)?);
    }
    if second != end {
      pieces.push(extract(&segments, false, second, end)?);
    }
    pieces
  };

  let mut kept = Vec::new();
  for piece in kept_segments {
    if piece.is_empty() {
      continue;
    }
    let contour = Contour::from_segments(&piece, false)?;
    if contour.len() >= 2 {
      kept.push(contour);
    }
  }
  Ok(TrimOutcome { kept, removed })
}

/// A point on the target, as a segment index and a fraction along that segment.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Position {
  segment: usize,
  fraction: f64,
}

impl Position {
  fn start() -> Self {
    Self {
      segment: 0,
      fraction: 0.0,
    }
  }

  fn end(segment: usize) -> Self {
    Self {
      segment,
      fraction: 1.0,
    }
  }

  fn physical(self, segments: &[Segment]) -> Point2 {
    match segments[self.segment] {
      Segment::Line(line) => line.point_at(self.fraction),
      Segment::Arc(arc) => arc.point_at_fraction(self.fraction),
    }
  }
}

fn collect_cuts(segments: &[Segment], cutter: &Contour, tolerance: f64) -> Vec<Position> {
  let cutter_segments = cutter.segments();
  let mut cuts: Vec<Position> = Vec::new();
  for (index, segment) in segments.iter().enumerate() {
    for against in &cutter_segments {
      let meeting = intersect::intersect(*segment, *against, tolerance);
      if meeting.is_tangent() || meeting.is_coincident() {
        continue;
      }
      for point in meeting.points() {
        if let Some(fraction) = fraction_along(*segment, point) {
          cuts.push(Position {
            segment: index,
            fraction: fraction.clamp(0.0, 1.0),
          });
        }
      }
    }
  }
  cuts.sort_by(|a, b| {
    a.segment.cmp(&b.segment).then(
      a.fraction
        .partial_cmp(&b.fraction)
        .unwrap_or(std::cmp::Ordering::Equal),
    )
  });
  let mut deduped: Vec<Position> = Vec::with_capacity(cuts.len());
  for cut in cuts {
    let repeated = deduped.last().is_some_and(|previous| {
      previous
        .physical(segments)
        .is_near(cut.physical(segments), tolerance)
    });
    if !repeated {
      deduped.push(cut);
    }
  }
  deduped
}

fn fraction_along(segment: Segment, point: Point2) -> Option<f64> {
  match segment {
    Segment::Line(line) => {
      let direction = line.end - line.start;
      let length_squared = direction.dot(direction);
      if length_squared <= 0.0 {
        None
      } else {
        Some((point - line.start).dot(direction) / length_squared)
      }
    }
    Segment::Arc(arc) => {
      let sweep = arc.sweep();
      if sweep == 0.0 {
        return None;
      }
      let angle = (point - arc.center()).angle();
      let travelled = if sweep >= 0.0 {
        (angle - arc.start_angle()).rem_euclid(TAU)
      } else {
        (arc.start_angle() - angle).rem_euclid(TAU)
      };
      Some(travelled / sweep.abs())
    }
  }
}

/// Walk the contour forward from `from` to `to`, collecting the segments in between.
fn extract(
  segments: &[Segment],
  closed: bool,
  from: Position,
  to: Position,
) -> Result<Vec<Segment>, GeomError> {
  let count = segments.len();
  let mut out: Vec<Segment> = Vec::new();
  let mut index = from.segment;
  let mut fraction = from.fraction;
  let mut steps = 0;

  loop {
    steps += 1;
    if steps > count + 2 || index >= count {
      return Err(GeomError::EditRefused {
        reason: "the cut points do not lie on one span of the contour",
      });
    }
    if index == to.segment {
      if to.fraction - fraction <= FRACTION_EPSILON {
        return Err(GeomError::EditRefused {
          reason: "the two cut points are the same point",
        });
      }
      out.push(piece_of(segments[index], fraction, to.fraction)?);
      return Ok(out);
    }
    if fraction < 1.0 - FRACTION_EPSILON {
      out.push(piece_of(segments[index], fraction, 1.0)?);
    }
    index += 1;
    if index == count {
      if !closed {
        return Err(GeomError::EditRefused {
          reason: "the cut points do not lie on one span of the contour",
        });
      }
      index = 0;
    }
    fraction = 0.0;
  }
}

/// The part of `segment` between two fractional positions, with `from` strictly before `to`.
fn piece_of(segment: Segment, from: f64, to: f64) -> Result<Segment, GeomError> {
  let tail = if from <= FRACTION_EPSILON {
    segment
  } else {
    split(segment, from)?.1
  };
  let span = if from <= FRACTION_EPSILON {
    to
  } else {
    (to - from) / (1.0 - from)
  };
  if span >= 1.0 - FRACTION_EPSILON {
    return Ok(tail);
  }
  Ok(split(tail, span)?.0)
}

/// Split a segment at a fraction strictly inside it.
fn split(segment: Segment, fraction: f64) -> Result<(Segment, Segment), GeomError> {
  if !(FRACTION_EPSILON..=1.0 - FRACTION_EPSILON).contains(&fraction) {
    return Err(GeomError::EditRefused {
      reason: "a cut point sits exactly on a segment boundary",
    });
  }
  match segment {
    Segment::Line(line) => {
      let point = line.point_at(fraction);
      Ok((
        Segment::Line(crate::segment::LineSeg::new(line.start, point)),
        Segment::Line(crate::segment::LineSeg::new(point, line.end)),
      ))
    }
    Segment::Arc(arc) => {
      let point = arc.point_at_fraction(fraction);
      let first = Arc::new(
        arc.center(),
        arc.radius(),
        arc.start_angle(),
        arc.sweep() * fraction,
      )?;
      let angle = (point - arc.center()).angle();
      let second = Arc::new(
        arc.center(),
        arc.radius(),
        angle,
        arc.sweep() * (1.0 - fraction),
      )?;
      Ok((Segment::Arc(first), Segment::Arc(second)))
    }
  }
}

/// Whether a meeting carries usable cut points. Kept for callers that want to ask before trimming.
pub fn is_usable_cut(meeting: &Intersections) -> bool {
  !meeting.is_tangent() && !meeting.is_coincident()
}
