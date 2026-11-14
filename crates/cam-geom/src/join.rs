//! Joining open contours end to end.
//!
//! The chain is built greedily from the first contour's free end and every step must be
//! unambiguous. A group that could be chained two ways is refused instead of guessing, because a
//! wrong chain produces a cut path nobody asked for and the mistake only shows up at the machine.

use crate::contour::Contour;
use crate::point::Point2;
use crate::GeomError;

/// How many free ends may share one point before the chain is ambiguous.
///
/// Two is a join; three is a fork, and there is no principled way to pick a branch.
const AMBIGUOUS_ENDS: usize = 3;

/// Join a group of open contours into one contour, reversing them as needed.
///
/// When the two free ends of the chain meet within `tolerance` the result is **closed**, so a ring of
/// four lines becomes a rectangle that a profile toolpath can cut. Joining a single contour returns
/// it unchanged.
///
/// # Errors
///
/// [`GeomError::EditRefused`] when the group is empty, holds a closed contour (it has no free ends),
/// has three or more ends meeting at one point, has more than one candidate continuation at a step,
/// or does not form exactly one chain.
pub fn join(contours: &[Contour], tolerance: crate::TolerancePolicy) -> Result<Contour, GeomError> {
  let Some(first) = contours.first() else {
    return Err(GeomError::EditRefused {
      reason: "there is nothing to join",
    });
  };
  if contours.len() == 1 {
    return Ok(first.clone());
  }
  if contours.iter().any(Contour::is_closed) {
    return Err(GeomError::EditRefused {
      reason: "a closed contour has no free ends to join",
    });
  }
  if has_a_fork(contours, tolerance) {
    return Err(GeomError::EditRefused {
      reason: "three or more endpoints meet at one point, so the join is ambiguous",
    });
  }

  let mut chain = first.clone();
  let mut pending: Vec<usize> = (1..contours.len()).collect();
  while !pending.is_empty() {
    let tail = chain.vertices()[chain.len() - 1].point;
    let mut matches = Vec::new();
    for (slot, index) in pending.iter().enumerate() {
      let candidate = &contours[*index];
      if candidate.vertices()[0].point.is_near(tail, tolerance) {
        matches.push((slot, *index, false));
      } else if candidate.vertices()[candidate.len() - 1]
        .point
        .is_near(tail, tolerance)
      {
        matches.push((slot, *index, true));
      }
    }
    match matches.len() {
      0 => {
        return Err(GeomError::EditRefused {
          reason: "the contours do not form exactly one chain",
        })
      }
      1 => {}
      _ => {
        return Err(GeomError::EditRefused {
          reason: "more than one contour continues from the same point, so the join is ambiguous",
        })
      }
    }
    let (slot, index, reverse) = matches[0];
    let piece = if reverse {
      contours[index].reverse()
    } else {
      contours[index].clone()
    };
    chain = append(chain, &piece)?;
    pending.remove(slot);
  }

  let head = chain.vertices()[0].point;
  let tail = chain.vertices()[chain.len() - 1].point;
  if tail.is_near(head, tolerance) && chain.len() > 3 {
    // The last vertex is the first one again: drop it, and let its predecessor's bulge become the
    // closing segment.
    let vertices = chain.vertices()[..chain.len() - 1].to_vec();
    return Contour::new(vertices, true);
  }
  Ok(chain)
}

/// True when any free end coincides with two or more other free ends.
fn has_a_fork(contours: &[Contour], tolerance: crate::TolerancePolicy) -> bool {
  let ends: Vec<Point2> = contours
    .iter()
    .flat_map(|contour| {
      [
        contour.vertices()[0].point,
        contour.vertices()[contour.len() - 1].point,
      ]
    })
    .collect();
  ends.iter().enumerate().any(|(index, point)| {
    ends
      .iter()
      .enumerate()
      .filter(|(other, candidate)| *other != index && candidate.is_near(*point, tolerance))
      .count()
      >= AMBIGUOUS_ENDS - 1
  })
}

/// Append `piece` after `chain`, carrying the piece's first bulge onto the chain's last vertex.
fn append(chain: Contour, piece: &Contour) -> Result<Contour, GeomError> {
  let mut vertices = chain.vertices().to_vec();
  if let Some(last) = vertices.last_mut() {
    last.bulge = piece.vertices()[0].bulge;
  }
  vertices.extend_from_slice(&piece.vertices()[1..]);
  Contour::new(vertices, false)
}
