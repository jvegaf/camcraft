//! Chaining loose segments into ordered contours.

use crate::contour::Contour;
use crate::segment::Segment;

/// The outcome of chaining: contours that closed and chains that did not.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Chained {
  /// Contours whose last segment returns to the first point within tolerance.
  pub closed: Vec<Contour>,
  /// Chains that ran out of matching segments.
  pub open: Vec<Contour>,
  /// Pieces that could not become a contour at all, such as a sub-tolerance fragment that
  /// collapses to a single vertex. Counted rather than silently discarded so the caller can
  /// report it.
  pub dropped: usize,
}

impl Chained {
  /// Total number of contours, closed and open.
  pub fn total(&self) -> usize {
    self.closed.len() + self.open.len()
  }
}

/// Chain loose segments into contours by matching endpoints within `tolerance`.
///
/// The walk is greedy and deterministic: segments are consumed in input order, and the first
/// unused segment whose start *or* end matches the current tail is appended (reversed when it
/// matched by its end). Determinism matters because the resulting contour order reaches the
/// G-code (A9).
pub fn chain(segments: &[Segment], tolerance: f64) -> Chained {
  let mut used = vec![false; segments.len()];
  let mut result = Chained::default();

  for start_index in 0..segments.len() {
    if used[start_index] {
      continue;
    }
    used[start_index] = true;
    let first = segments[start_index];
    let contour_start = first.start_point();
    let mut pieces = vec![first];

    // A single segment that already returns to its start is a closed contour.
    let mut closed = first.end_point().is_near(contour_start, tolerance);

    while !closed {
      let tail_end = pieces
        .last()
        .map(|piece| piece.end_point())
        .expect("the chain always holds at least one piece");

      let mut next = None;
      for (index, candidate) in segments.iter().enumerate() {
        if used[index] {
          continue;
        }
        if candidate.start_point().is_near(tail_end, tolerance) {
          next = Some((index, *candidate));
          break;
        }
        if candidate.end_point().is_near(tail_end, tolerance) {
          next = Some((index, candidate.reversed()));
          break;
        }
      }

      match next {
        Some((index, piece)) => {
          used[index] = true;
          let piece_end = piece.end_point();
          pieces.push(piece);
          closed = piece_end.is_near(contour_start, tolerance);
        }
        None => break,
      }
    }

    match Contour::from_segments(&pieces, closed) {
      Ok(contour) if closed => result.closed.push(contour),
      Ok(contour) => result.open.push(contour),
      Err(_) => result.dropped += 1,
    }
  }

  result
}
