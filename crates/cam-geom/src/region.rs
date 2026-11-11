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
//!
//! [`Region::distance_to_boundary`] is the region's distance field: the exact distance from a point to
//! the boundary, and whether the point is material, in one answer (M7 A1). It is the surface a V-carve
//! cuts, so it is exact — arcs native, no tessellation, no sampling — and it is measured against the
//! *boundary*, not against a filled area.

use crate::boolean;
use crate::contour::Contour;
use crate::point::Point2;
use crate::winding::{classify_with_distance, Membership};
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

/// The exact distance from a point to a region's boundary, and whether the point is material.
///
/// Both halves are answered together because neither is usable alone: a point 2 mm inside a wall and
/// one 2 mm outside it report the same `distance`, and only `inside` tells them apart (M7 A1). See
/// [`Region::distance_to_boundary`] for the contract, including the region with no boundary at all.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoundaryDistance {
  /// Exact distance to the nearest point of any boundary, in millimetres. Never negative, and never
  /// snapped to zero by a tolerance: a point on the boundary reports exactly `0.0`.
  pub distance: f64,
  /// Whether the point is material, i.e. [`Region::contains`] at the same point and tolerance. A point
  /// on (or within `tolerance` of) a boundary is **not** material.
  pub inside: bool,
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
  ///
  /// This is the membership half of [`Region::distance_to_boundary`], which answers both questions for
  /// the price of one walk over the boundaries; neither restates the rule.
  pub fn contains(&self, point: Point2, tolerance: f64) -> bool {
    self
      .distance_to_boundary(point, tolerance)
      .is_some_and(|answer| answer.inside)
  }

  /// The exact distance from `point` to this region's boundary, and whether the point is material.
  ///
  /// The distance is the **minimum over every segment of every boundary** of that segment's own exact
  /// point distance ([`crate::Segment::distance_to_point`]): arcs stay arcs, so the centre of a circle
  /// reports the radius itself and not the apothem of a chord inscribed in it, and there is no
  /// tessellation, no sampling and no tolerance in the answer. `tolerance` decides only membership,
  /// exactly as it does in [`Region::contains`]: it never snaps a distance to zero, so a point a tenth
  /// of a nanometre off a wall reports that tenth even when it is too close to be cut.
  ///
  /// Membership is reported in the **same call** because every caller that wants the distance wants it
  /// too — the medial axis and the V-carve law both measure a point that may lie either side of the
  /// boundary — and one walk is cheaper than two. `inside` is [`Region::contains`] at the same tolerance
  /// by construction, so the two cannot disagree.
  ///
  /// # The region with no boundary
  ///
  /// `None` when the region has no boundary at all — an empty boundary list, or a contour that carries
  /// no segment. There is then no distance to report, and both alternatives would be wrong answers:
  /// `0.0` would claim the point lies *on* a boundary that does not exist (and would put the V-carve
  /// surface at full depth everywhere), and a panic would turn an empty region into a crash instead of
  /// a shape. `Region::default()` is a legal region, so callers must handle `None` rather than assume.
  pub fn distance_to_boundary(&self, point: Point2, tolerance: f64) -> Option<BoundaryDistance> {
    let mut nearest = f64::INFINITY;
    let mut enclosed = 0_usize;
    let mut on_boundary = false;
    for boundary in &self.boundaries {
      let (membership, distance) = classify_with_distance(boundary, point, tolerance);
      nearest = nearest.min(distance);
      match membership {
        Membership::OnBoundary => on_boundary = true,
        Membership::Inside => enclosed += 1,
        Membership::Outside => {}
      }
    }
    if !nearest.is_finite() {
      return None;
    }
    Some(BoundaryDistance {
      distance: nearest,
      inside: !on_boundary && enclosed % 2 == 1,
    })
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
  /// A boundary that crosses *itself* is resolved as well: it is cut at its own crossings, because a
  /// contour that crosses itself is two loops wearing one contour, and no piece of it can be
  /// classified until that crossing is a vertex. This used to pass a self-crossing contour through
  /// untouched, which left a boundary whose signed area was zero while its material was not.
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
