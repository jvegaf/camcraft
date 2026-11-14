//! Region offsetting, the distance proof behind it, and raster spans (A6, A7).
//!
//! [`Region::offset`] moves every boundary the way that *grows the material*: a boundary with
//! material inside it (an even nesting depth) shrinks, and a boundary with material outside it (an
//! odd depth, a hole) grows. Depth is the number of other boundaries that strictly contain this one,
//! decided by [`crate::winding::relation`] — never by orientation, because the region model does not
//! consult orientation (D-RB-2). That is the region form of what a single-contour pocket does: the
//! outer boundary is offset by `-distance` (exactly `ProfileSide::Inside`) and a hole by `+distance`,
//! so one boundary reproduces the single-contour ring geometry the pocket already had.
//!
//! # Why the result is not just the offsets
//!
//! Offsetting is where a shrink *escapes* the material. `offset_contour` refuses an offset that
//! crosses the interior in the cases it can see, but a 3 mm square inset by 2 mm comes back as a
//! well-formed 1 mm ring with its winding unchanged (D-M4-9). Cutting that ring gouges the wall it
//! was supposed to leave standing, and in a ring loop it makes the rings grow instead of ending. So
//! every offset piece is dropped unless it proves it really is the offset it claims to be: every
//! point of it at least `distance - slack` from an **original** boundary. That is M4's
//! `clears_boundary` rule (D-M4-9) moved into the kernel (D-RB-6), and it is a proof about the whole
//! piece rather than about its ends because the minimum distance from the piece to an original is
//! measured **exactly**, segment pair by segment pair (D-RB-11, [`segment_to_segment`]).
//!
//! There is no resolution knob in that sentence. Earlier versions of this proof asked only about a
//! piece's vertices, and then about the ends of stretches of arc against a Lipschitz budget; both
//! were answered by looking at finitely many points and hoping the worst point was one of them, and
//! both were unsound — a lens-shaped piece bulged 6 mm inside the distance between its two crossings
//! (D-RB-10), and a straight chord cut 6 mm inside the distance between the crossings that bounded
//! it (D-RB-11). The exact minimum deletes the question: a piece survives when *no* pair of (its
//! segment, an original segment) comes closer than `distance - slack`, and that is a statement about
//! every point of the piece.
//!
//! The proof is stated per **piece**, and a piece here is a chained contour: normalization has
//! already walked the arrangement and closed its survivors, so a contour is the finest unit the
//! kernel can still name. Dropping a contour costs at most the material of that contour, never a
//! gouge.
//!
//! # Raster spans
//!
//! [`Region::raster_spans`] is A7: the material's x spans at a height `y`, by intersecting a long
//! horizontal line with every boundary and pairing the crossings even-odd. An odd count is a
//! tangency that could not be paired, and a row lying along a straight boundary segment cannot be
//! clipped at all; both answer `None` so the caller keeps owning its own retry policy, which is the
//! policy `pocket.rs` already has.
//!
//! # Arcs
//!
//! Offsetting preserves arcs wherever `offset_contour` does (A8): a line shifts, an arc keeps its
//! centre and changes radius, and only the kernel's self-intersection removal or a piece the contour
//! model's bulge cannot hold flattens anything. [`RegionOffset::flattened`] says when it happened and
//! the warnings say which step did it, exactly as [`crate::offset::Offset`] already does.

use crate::contour::Contour;
use crate::distance::segment_to_segment;
use crate::intersect::{intersect, Intersections};
use crate::offset::{offset_contour, OffsetOptions, OffsetWarning};
use crate::point::{Bounds, Point2};
use crate::region::Region;
use crate::segment::{LineSeg, Segment};
use crate::winding::{relation, Relation};
use crate::{GeomError, TolerancePolicy};

/// How far an offset vertex may fall short of the distance it claims, as a multiple of the
/// chord tolerance. Two sagittae keep the distance proof aligned with M4's rings.
const DISTANCE_SLACK_FACTOR: f64 = 2.0;

/// How much longer than the region's own extent the raster line reaches, in millimetres.
///
/// The row has to cross every boundary rather than end on one, and ending on one would report a
/// tangency that is really the caller's clip. `pocket.rs` uses the same margin, for the same reason.
const RASTER_REACH: f64 = 10.0;

/// A region offset, and what the caller should be told about it.
#[derive(Clone, Debug, PartialEq)]
pub struct RegionOffset {
  /// The offset region, with no crossing between its boundaries.
  pub region: Region,
  /// True when some piece had to be rebuilt as straight segments instead of being kept exact: the
  /// offset kernel's self-intersection removal, or a piece the contour model's bulge cannot hold.
  pub flattened: bool,
  /// What went wrong without being fatal, in the order it was produced.
  pub warnings: Vec<OffsetWarning>,
}

impl Region {
  /// Move every boundary the way that grows the material (A6).
  ///
  /// A positive `distance` **removes** material: the area shrinks by roughly `perimeter * distance`.
  /// Each boundary's [`offset_contour`] sign comes from its nesting depth — an even depth (material
  /// inside) is offset by `-distance`, an odd depth (a hole) by `+distance` — which is the only rule
  /// that keeps a hole and a boss inside it consistent, and which reproduces the single-contour ring
  /// exactly for a region with one boundary. The offsets are then normalized (their corners can
  /// cross) and every piece that cannot prove it is `distance` away from the original boundaries is
  /// dropped, so an over-shrink is refused instead of returning a ring that gouges the wall.
  ///
  /// A negative `distance` grows the material, with the same parity rule in the other direction. The
  /// distance proof degenerates there — its threshold is negative, so nothing is dropped — which is
  /// correct: a growing offset cannot escape through the material it is growing into.
  ///
  /// Tolerance is the tolerance normalization runs at.
  ///
  /// # Errors
  ///
  /// [`GeomError::EditRefused`] when the distance or the tolerance is not finite, when the tolerance
  /// is not positive, when the region has no boundaries, when two of its boundaries cross (the
  /// distance proof needs the given boundaries to *be* the material boundary, so a region must be
  /// normalized before it is offset), when a boundary's own offset collapses, when normalization
  /// cannot chain the offset pieces, or when nothing survives the distance proof. A partially offset
  /// region is never returned, and neither is an empty one: "no material left" is a refusal naming
  /// what collapsed, not a region with no boundaries.
  pub fn offset(
    &self,
    distance: f64,
    tolerance: TolerancePolicy,
  ) -> Result<RegionOffset, GeomError> {
    if !distance.is_finite() {
      return Err(GeomError::EditRefused {
        reason: "a region offset needs a finite distance",
      });
    }
    if !tolerance.epsilon.is_finite() || tolerance.epsilon <= 0.0 {
      return Err(GeomError::EditRefused {
        reason: "a region offset needs a positive, finite tolerance",
      });
    }
    if self.boundaries.is_empty() {
      return Err(GeomError::EditRefused {
        reason: "a region with no boundaries has no material to offset",
      });
    }

    let depths = nesting_depths(self, tolerance)?;
    let options = OffsetOptions::new(4.0, tolerance);
    let mut warnings = Vec::new();
    let mut flattened = false;
    let mut boundaries = Vec::with_capacity(self.boundaries.len());
    for (boundary, depth) in self.boundaries.iter().zip(&depths) {
      // Even depth: material lies inside this boundary, so it shrinks. Odd depth: the boundary is a
      // hole, material lies outside it, and it has to grow for the material to recede from it.
      let signed = if depth % 2 == 0 { -distance } else { distance };
      let offset = offset_contour(boundary, signed, &options).map_err(|_| GeomError::EditRefused {
        reason: if depth % 2 == 0 {
          "a region offset collapsed an enclosing boundary: the distance is larger than the material it encloses"
        } else {
          "a region offset collapsed a hole boundary: the distance is larger than the hole it encloses"
        },
      })?;
      flattened |= offset.flattened;
      warnings.extend(offset.warnings);
      boundaries.push(offset.contour);
    }

    // The offset corners cross each other wherever the distance is wider than a feature, and the
    // crossings are exactly what normalization resolves.
    let normalized = Region { boundaries }.normalize(tolerance)?;
    flattened |= normalized.flattened;
    if normalized.flattened && !warnings.contains(&OffsetWarning::Flattened) {
      warnings.push(OffsetWarning::Flattened);
    }

    let survivors = distance_proof(
      self,
      normalized.region.boundaries,
      distance,
      tolerance.chord,
    );
    if survivors.is_empty() {
      return Err(GeomError::EditRefused {
        reason: "a region offset leaves no material: every offset piece is closer to an original boundary than the distance",
      });
    }
    Ok(RegionOffset {
      region: Region {
        boundaries: survivors,
      },
      flattened,
      warnings,
    })
  }

  /// The material's x spans at height `y`, as `(left, right)` pairs (A7).
  ///
  /// A long horizontal line is intersected with every boundary, the crossing x's are sorted and
  /// deduped within `tolerance`, and the survivors are paired even-odd — one contour or twenty, the
  /// rule is the same, which is what makes a row crossing an island come back as two spans.
  ///
  /// `None` means the row is **ambiguous**, never that the region is empty: a tangency (an unpaired
  /// crossing) and a row that lies along a straight boundary segment (a coincidence) cannot be
  /// clipped into spans without guessing, so the caller keeps owning its retry, which is the retry
  /// `pocket.rs` already performs. A `y` that is not a number and a tolerance that cannot dedupe are
  /// ambiguous in the same way. An empty `Vec` means the row misses the material entirely.
  pub fn raster_spans(&self, y: f64, tolerance: TolerancePolicy) -> Option<Vec<(f64, f64)>> {
    if self.boundaries.is_empty() {
      return Some(Vec::new());
    }
    if !y.is_finite() || !tolerance.epsilon.is_finite() || tolerance.epsilon <= 0.0 {
      return None;
    }
    let bounds = region_bounds(self);
    let reach = bounds.width().max(bounds.height()) + RASTER_REACH;
    let line = LineSeg::new(
      Point2::new(bounds.min.x - reach, y),
      Point2::new(bounds.max.x + reach, y),
    );

    let mut xs = Vec::new();
    for boundary in &self.boundaries {
      for segment in boundary.segments() {
        match intersect(Segment::Line(line), segment, tolerance) {
          // The row lies along this segment, so it has no single crossing to pair.
          Intersections::Coincident => return None,
          intersections => xs.extend(intersections.points().into_iter().map(|point| point.x)),
        }
      }
    }
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // Two segments meeting at a vertex both report that vertex, so a shared crossing is deduped
    // before pairing. A tangency survives this as a single x, which is what makes it unpaired.
    let mut unique: Vec<f64> = Vec::with_capacity(xs.len());
    for x in xs {
      if unique
        .last()
        .map_or(true, |last| (x - last).abs() > tolerance.epsilon)
      {
        unique.push(x);
      }
    }
    if unique.len() % 2 != 0 {
      return None;
    }
    Some(unique.chunks(2).map(|pair| (pair[0], pair[1])).collect())
  }
}

/// How many other boundaries strictly contain each boundary, refusing a crossing between any two.
///
/// Depth is what decides an offset's sign: material inside an even depth, outside an odd one. A
/// crossing has no depth to speak of — the material boundary is not the boundary list — so it is
/// refused here, before any offset is computed.
fn nesting_depths(region: &Region, tolerance: TolerancePolicy) -> Result<Vec<usize>, GeomError> {
  let count = region.boundaries.len();
  let mut depths = vec![0_usize; count];
  for first in 0..count {
    for second in (first + 1)..count {
      match relation(
        &region.boundaries[first],
        &region.boundaries[second],
        tolerance,
      ) {
        Relation::Disjoint | Relation::Touching => {}
        Relation::FirstInside => depths[first] += 1,
        Relation::SecondInside => depths[second] += 1,
        Relation::Crossing => {
          return Err(GeomError::EditRefused {
            reason: "a region whose boundaries cross cannot be offset: normalize it first",
          })
        }
      }
    }
  }
  Ok(depths)
}

/// The offset pieces that prove they are `distance` away from an original boundary (D-M4-9,
/// D-RB-11).
///
/// A piece is kept or dropped whole, and the threshold carries M4's exact form,
/// `distance - slack - distance·1e-9`, so the region and the pocket agree to the last bit on when a
/// ring has escaped.
///
/// # The argument
///
/// The distance from a piece to the originals is the minimum over the pairs, and
/// [`segment_to_segment`] returns each pair's minimum **exactly**. So if every pair's minimum is at
/// least `distance - slack`, then no point of the piece is closer than that to any original: the
/// minimum over a set of curves is attained *somewhere* on them, and the pair that attains it is one
/// of the pairs that was measured. Keeping on that condition and dropping otherwise is therefore a
/// statement about every point of the piece, and it needs no resolution parameter — there is no step
/// to choose and nothing between two measurements that could go unchecked.
///
/// # Why the ends were not enough, twice
///
/// The vertex rule this replaced was that same argument with the piece assumed to be worst at its
/// ends. It is not. A lens-shaped piece at a crossing stage has its two crossings as its only
/// vertices — both of them *exactly* `distance` from both originals — while the arc between them
/// bulges towards a wall that neither crossing is near; in the 4 mm island fixture that bulge came
/// 6 mm inside the requested distance (L9, D-RB-10). And a **straight** piece fails the same way:
/// in K5's 23 mm fixture the chord between a lens's two crossings has both ends `23` mm from the
/// island and comes `17` mm from it in the middle, because the island's circle bulges under the
/// chord (L10, D-RB-11). A subdivision fine enough to catch a chord that dips six millimetres over a
/// thirty-eight millimetre span is sampling dressed as a proof, which is why the answer is the exact
/// pair distance instead of a finer step.
///
/// # The limit this keeps
///
/// A piece is still kept or dropped whole, so a failing piece costs the material of that piece and
/// never a gouge (D-RB-9's recorded limit). What is gone is the escape hatch: a piece that fails
/// *any* pair test is dropped, wherever on the piece the closeness happened.
fn distance_proof(
  region: &Region,
  pieces: Vec<Contour>,
  distance: f64,
  chord_tolerance: f64,
) -> Vec<Contour> {
  let original: Vec<Segment> = region
    .boundaries
    .iter()
    .flat_map(|boundary| boundary.segments())
    .collect();
  let threshold = distance - DISTANCE_SLACK_FACTOR * chord_tolerance - distance.abs() * 1.0e-9;
  pieces
    .into_iter()
    .filter(|piece| proves_distance(&original, piece, threshold))
    .collect()
}

/// Whether every point of `piece` clears `threshold` from every segment of `original`.
///
/// One comparison per (piece segment, original segment) pair, each of them exact (D-RB-11): the
/// proof asks [`segment_to_segment`] for the unsnapped minimum, because `threshold` is already the
/// distance the piece claims less the kernel's slack, and snapping a small positive distance to zero
/// would only make the test stricter than the geometry. Nothing here is sampled, so there is no
/// check point count, no step, and no bound that decides whether a pair has to be looked at.
fn proves_distance(original: &[Segment], piece: &Contour, threshold: f64) -> bool {
  piece.segments().into_iter().all(|segment| {
    original.iter().all(|other| {
      segment_to_segment(segment, *other, crate::TolerancePolicy::uniform(0.0)) >= threshold
    })
  })
}

/// The box containing every boundary of the region.
fn region_bounds(region: &Region) -> Bounds {
  region
    .boundaries
    .iter()
    .fold(Bounds::empty(), |bounds, boundary| {
      bounds.union(boundary.bounds())
    })
}
