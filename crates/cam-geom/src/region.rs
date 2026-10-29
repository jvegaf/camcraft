//! Regions: the material a cut may remove, as a list of closed boundaries.
//!
//! Membership is **even-odd** (D-RB-2): a point is material when it is inside an odd number of
//! boundaries. Orientation is deliberately not part of the model — a reversed or mirrored import must
//! not turn a hole into material, and a mirrored contour is exactly what a DXF written by a different
//! package hands us. Parity also means a hole is expressed by *appending* its boundary (A3) and a
//! boss inside a hole needs no third concept at all: appending its boundary flips the parity again.
//!
//! A region is only as honest as its boundary list. Boundaries may be nested, disjoint or touching,
//! but two that *cross* mean the list no longer describes a material area: the parity rule still
//! answers for every point, yet a crossing is not a place a tool can cut and the crossing point
//! belongs to both boundaries. [`Region::normalize`] restores the invariant — it splits the
//! boundaries at their crossings, keeps the pieces that bound material and chains what is left into
//! closed contours (A4) — and [`Region::subtract`] is that same operation with one boundary appended
//! (A5).

use crate::boolean;
use crate::contour::Contour;
use crate::point::Point2;
use crate::winding::{classify, Membership};
use crate::GeomError;

pub use crate::region_offset::RegionOffset;

/// Material in the plane, as closed boundaries under the even-odd membership rule.
///
/// Build one with [`Region::new`]; `boundaries` is public because reading a region is a legitimate
/// thing to do, but a region built by hand must still satisfy the closedness invariant, which is why
/// the field is normally reached through [`Region::normalize`] instead.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Region {
  /// The closed boundaries. Membership is by parity across them, never by orientation.
  pub boundaries: Vec<Contour>,
}

/// A normalized region, and whether normalization had to rebuild anything as straight segments.
#[derive(Clone, Debug, PartialEq)]
pub struct Normalized {
  /// The region, with no crossing between its boundaries.
  pub region: Region,
  /// True when at least one surviving piece was rebuilt as straight segments instead of being kept
  /// as the segment it came from. Pieces are otherwise kept exact, arcs included (A8).
  pub flattened: bool,
}

impl Region {
  /// Build a region from its boundaries.
  ///
  /// # Errors
  ///
  /// [`GeomError::EditRefused`] when a boundary is not a closed contour: a region is an area, and an
  /// open contour encloses none. The refusal is by name rather than by index, because the kernel's
  /// refusal reasons are static strings the caller reports verbatim.
  pub fn new(boundaries: Vec<Contour>) -> Result<Self, GeomError> {
    if boundaries.iter().any(|boundary| !boundary.is_closed()) {
      return Err(GeomError::EditRefused {
        reason: "a region boundary must be a closed contour",
      });
    }
    Ok(Self { boundaries })
  }

  /// Whether `point` is material, i.e. enclosed by an odd number of boundaries.
  ///
  /// A point within `tolerance` of any boundary is not material: a boundary is the edge of what can be
  /// cut, so reporting it as inside would let a tool cut along an edge two regions share. The parity
  /// rule is the whole membership model (A3) — a hole needs no flag, only a second boundary.
  pub fn contains(&self, point: Point2, tolerance: f64) -> bool {
    let mut enclosed = 0_usize;
    for boundary in &self.boundaries {
      match classify(boundary, point, tolerance) {
        Membership::OnBoundary => return false,
        Membership::Inside => enclosed += 1,
        Membership::Outside => {}
      }
    }
    enclosed % 2 == 1
  }

  /// The region with `hole` appended and normalized (A5).
  ///
  /// There is deliberately no "hole" concept here: the boundary is appended and the parity rule does
  /// the rest, so the same call expresses a boss inside an island. Note that parity has no notion of
  /// which boundary is outer: appending a boundary that *contains* this region's own boundaries
  /// subtracts the other way round, which is the parity rule and not a bug. Normalization runs at
  /// [`crate::DEFAULT_TOLERANCE`], which is the tolerance a caller that did not name one is asking for.
  ///
  /// # Errors
  ///
  /// [`GeomError::EditRefused`] when `hole` is not closed, or when the subtraction would remove every
  /// piece of material.
  pub fn subtract(&self, hole: &Contour) -> Result<Self, GeomError> {
    let mut boundaries = self.boundaries.clone();
    boundaries.push(hole.clone());
    Self::new(boundaries)?
      .normalize(crate::DEFAULT_TOLERANCE)
      .map(|normalized| normalized.region)
  }

  /// Split the boundaries at their crossings, keep the pieces that bound material and chain them back
  /// into closed contours (A4).
  ///
  /// A region whose boundaries do not cross comes back unchanged and arc-exact, in the vertex order it
  /// arrived in. Anything else goes through the arrangement: every boundary is split at every
  /// transversal crossing with another boundary, each piece is classified by the parity of the
  /// material beside it, the pieces that border material are oriented with that material on their
  /// left, and the walk that chains them starts from the lowest piece in `(min.y, min.x, order)` so
  /// two runs on the same input produce the same contours vertex for vertex (S5).
  ///
  /// A boundary that crosses *itself* is not resolved here: the crossings this removes are the ones
  /// between boundaries (A4), and a self-crossing contour is passed through as it arrived. The
  /// even-odd rule still answers for every point of it, and its orientation is never consulted.
  ///
  /// # Errors
  ///
  /// [`GeomError::EditRefused`] when the tolerance is not positive and finite, when a piece cannot be
  /// rebuilt, when the arrangement is finer than the tolerance can resolve, when no material is left,
  /// or when the surviving pieces cannot be chained into closed contours. A half-built region is never
  /// returned.
  pub fn normalize(&self, tolerance: f64) -> Result<Normalized, GeomError> {
    boolean::normalize(self, tolerance)
  }
}
