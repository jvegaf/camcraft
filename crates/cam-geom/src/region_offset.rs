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
//! `clears_boundary` rule (D-M4-9) moved into the kernel (D-RB-6), so the pocket and every later
//! consumer of a region cannot drift apart — generalised from "every vertex" to "every point of
//! every arc, and the ends of every straight stretch" (D-RB-10), because a vertex is not the worst
//! point of an arc.
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

use crate::arc::Arc;
use crate::contour::Contour;
use crate::intersect::{intersect, Intersections};
use crate::offset::{offset_contour, OffsetOptions, OffsetWarning};
use crate::point::{Bounds, Point2, Vec2};
use crate::region::Region;
use crate::segment::{LineSeg, Segment};
use crate::winding::{relation, Relation};
use crate::GeomError;

use std::f64::consts::{PI, TAU};

/// The offset kernel's chord tolerance, in millimetres.
///
/// This is [`OffsetOptions::default`]'s tolerance, named once here because the distance proof's slack
/// is derived from it and both have to move together.
const OFFSET_CHORD_TOLERANCE: f64 = 0.001;

/// How far an offset vertex may fall short of the distance it claims, in millimetres.
///
/// A ring whose self-intersections were removed is flattened to [`OFFSET_CHORD_TOLERANCE`], so its
/// vertices can sit a chord's sagitta inside the exact offset curve. Two sagittae is the slack M4
/// chose (`RING_DISTANCE_SLACK` in `pocket.rs`), and it is kept at exactly that value so the region
/// offset reproduces M4's rings vertex for vertex when the region has no islands (A15).
const DISTANCE_SLACK: f64 = 2.0 * OFFSET_CHORD_TOLERANCE;

/// How long a stretch of curve one check point stands for, in millimetres (D-RB-10).
///
/// The distance proof is a Lipschitz argument, not a sampler: the distance to a set changes by at
/// most the distance travelled, so a piece cut into stretches no longer than this needs its check
/// points — their ends — to clear `distance - slack + stretch` for the whole stretch to clear
/// `distance - slack`. The budget is a quarter of [`DISTANCE_SLACK`], which keeps the extra
/// requirement it puts on a check point (`distance - slack + step`) well below `distance`, so a
/// piece whose points really are `distance` away is never dropped for the sake of the budget, while
/// the number of check points is still finite work. It is derived from the kernel's own tolerance
/// and not from anything the caller passes, so it is a proof and not a resolution knob.
const CHECK_STEP: f64 = DISTANCE_SLACK / 4.0;

/// The most check points one piece of curve may be cut into for the proof.
///
/// Ten million points is five metres of curve at [`CHECK_STEP`] — every arc a sheet-sized machine
/// cuts, and a full circle of any radius up to 795 mm exactly. Geometry beyond that is checked at
/// the step this many points really gives, which is what makes the cap safe: the step is measured
/// back out of it and the threshold widens to match, so the answer stays a proof — more
/// conservative, never weaker — and the loop is still bounded.
const MAX_CHECK_POINTS: usize = 10_000_000;

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
  pub fn offset(&self, distance: f64, tolerance: f64) -> Result<RegionOffset, GeomError> {
    if !distance.is_finite() {
      return Err(GeomError::EditRefused {
        reason: "a region offset needs a finite distance",
      });
    }
    if !tolerance.is_finite() || tolerance <= 0.0 {
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
    let options = OffsetOptions {
      tolerance: OFFSET_CHORD_TOLERANCE,
      ..OffsetOptions::default()
    };
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

    let survivors = distance_proof(self, normalized.region.boundaries, distance);
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
  pub fn raster_spans(&self, y: f64, tolerance: f64) -> Option<Vec<(f64, f64)>> {
    if self.boundaries.is_empty() {
      return Some(Vec::new());
    }
    if !y.is_finite() || !tolerance.is_finite() || tolerance <= 0.0 {
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
        .map_or(true, |last| (x - last).abs() > tolerance)
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
fn nesting_depths(region: &Region, tolerance: f64) -> Result<Vec<usize>, GeomError> {
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
/// D-RB-10).
///
/// A piece is kept or dropped whole, and the threshold carries M4's exact form,
/// `distance - slack - distance·1e-9`, so the region and the pocket agree to the last bit on when a
/// ring has escaped.
///
/// # The argument, and why its vertices were not enough
///
/// The distance to a set is 1-Lipschitz: moving a point by `s` changes its distance to the set by at
/// most `s`. So cut a piece's arc into stretches short enough that every point of a stretch lies
/// within `step` of one of its ends; if both ends clear `distance - slack + step`, then every point
/// of the stretch clears `distance - slack`, because it is at worst `step` closer than the end that
/// covers it. That is a proof about the whole arc, and `step` is derived from the kernel's own slack
/// ([`CHECK_STEP`]), so no resolution knob decides the answer.
///
/// The vertices alone were that same argument with `step = 0`, which is exactly where it stopped
/// being true for an arc (D-RB-10). A lens-shaped piece at a crossing stage has its two crossings as
/// its only vertices — both of them *exactly* `distance` from both originals — while the arc between
/// them bulges towards a wall that neither crossing is near. In the 4 mm island fixture that bulge
/// came 6 mm inside the requested distance (L9), and the vertex rule kept it.
///
/// The stretch a check point stands for is its **length**, not its sagitta. A sagitta bounds how far
/// a chord wanders from the curve; the Lipschitz argument needs how far a point of the curve is from
/// a point that was actually checked, and the farthest point of a stretch of circle from its nearer
/// end is half a chord away — for the same stretch several hundred times its sagitta. Cutting by
/// length is therefore strictly finer than cutting by sagitta, and it is what makes the bound true.
///
/// # The limit this keeps
///
/// A straight stretch keeps M4's rule exactly: its two ends are its check points and it has no
/// others (D-RB-9), which is the rule a pocket has always used. A straight edge can therefore still
/// come closer than `distance` to an original it does not belong to in the *middle* of its span —
/// the lens above has one, a chord that runs inside the grown island between its two crossings — and
/// that stays the per-piece limit D-RB-9 recorded. The case K5 fixes is the arc, which is the one
/// that was measured going wrong; subdividing straight edges as well would need a check point every
/// half micrometre of a 100 mm wall, which is sampling rather than a proof.
fn distance_proof(region: &Region, pieces: Vec<Contour>, distance: f64) -> Vec<Contour> {
  let original: Vec<Segment> = region
    .boundaries
    .iter()
    .flat_map(|boundary| boundary.segments())
    .collect();
  pieces
    .into_iter()
    .filter(|piece| proves_distance(&original, piece, distance))
    .collect()
}

/// Whether every point of `piece` clears `distance - DISTANCE_SLACK` from `original`.
///
/// The check points are the piece's vertices plus, for every arc, its interior cut into stretches
/// no longer than [`CHECK_STEP`]; the threshold is that much stricter than the distance the piece
/// claims, which is what covers the points between two checks (see [`distance_proof`]). A piece with
/// no arc has no interior check points and keeps the threshold M4 used.
fn proves_distance(original: &[Segment], piece: &Contour, distance: f64) -> bool {
  let segments = piece.segments();
  let mut arcs = Vec::new();
  // The longest stretch a single check point has to stand for. Zero when the piece is all lines:
  // every point of it is then a vertex or lies on a straight stretch, and the threshold is M4's.
  let mut step = 0.0_f64;
  for segment in &segments {
    let Some(arc) = segment.as_arc() else {
      continue;
    };
    let count = check_count(arc, CHECK_STEP);
    step = step.max(arc.length() / count as f64);
    arcs.push((arc, count));
  }

  let threshold = distance - DISTANCE_SLACK + step - distance.abs() * 1.0e-9;
  if piece
    .vertices()
    .iter()
    .any(|vertex| nearest_distance(vertex.point, original) < threshold)
  {
    return false;
  }
  for (arc, count) in arcs {
    // The closed form settles most arcs without a single check point: an arc that is an exact
    // offset of an original is exactly `distance` from it everywhere, and its nearest point to the
    // other originals is a turning point of a sinusoid, not something between two samples.
    if arc_lower_bound(arc, original) >= threshold {
      continue;
    }
    for index in 1..count {
      let point = arc.point_at_fraction(index as f64 / count as f64);
      if nearest_distance(point, original) < threshold {
        return false;
      }
    }
  }
  true
}

/// How many equal stretches `arc` has to be cut into so that each is at most `step` long.
///
/// A stretch of circle is no longer than its arc length, so the count is the length divided by the
/// step, rounded up: every point of a stretch is then within one stretch-length of its start, which
/// is the bound [`proves_distance`] needs. The count is capped at [`MAX_CHECK_POINTS`] so that
/// absurd geometry cannot turn the proof into an unbounded loop; the caller reads the step the cap
/// really gives back out of [`Arc::length`], so the cap can only make the proof more conservative.
fn check_count(arc: Arc, step: f64) -> usize {
  let length = arc.length();
  if !length.is_finite() || length <= 0.0 || !step.is_finite() || step <= 0.0 {
    return 1;
  }
  let count = (length / step).ceil();
  if !count.is_finite() || count >= MAX_CHECK_POINTS as f64 {
    return MAX_CHECK_POINTS;
  }
  (count as usize).max(1)
}

/// A lower bound on the distance from any point of `arc` to `original`.
///
/// This is a closed form over the arc's whole sweep, not a sample of it, and it is what keeps the
/// check points above from being the cost of every arc in every ring. Both distances it can meet are
/// sinusoids in the arc's angle: the distance from a point of an arc to a line's supporting line is
/// `|a + ρ·cos(φ - ψ)|`, and to an arc's circle it is `|‖p - C‖ - r|` with `‖p - C‖` the square root
/// of `D² + ρ² + 2ρD·cos(φ - ψ)`. Each one's extremes over the sweep are its two ends and its own
/// turning points, so the range is found in constant time, and the smallest distance in that range
/// is the bound. It is exact for a supporting line and a circle, and therefore never above the true
/// distance to the segment: the bound is only ever used to *skip* check points, so an inexact one
/// costs work and can never keep a piece that should have been dropped.
fn arc_lower_bound(arc: Arc, original: &[Segment]) -> f64 {
  original
    .iter()
    .map(|segment| arc_segment_lower_bound(arc, *segment))
    .fold(f64::INFINITY, f64::min)
}

/// One original segment's contribution to [`arc_lower_bound`].
fn arc_segment_lower_bound(arc: Arc, segment: Segment) -> f64 {
  let radius = arc.radius();
  match segment {
    Segment::Line(line) => {
      let Some(direction) = line.direction() else {
        return 0.0;
      };
      let normal = Vec2::new(-direction.y, direction.x);
      let base = (arc.center() - line.start).dot(normal);
      let (low, high) = cosine_range(arc, normal.angle());
      let (low, high) = (base + radius * low, base + radius * high);
      if low <= 0.0 && high >= 0.0 {
        0.0
      } else {
        low.abs().min(high.abs())
      }
    }
    Segment::Arc(other) => {
      let offset = arc.center() - other.center();
      let separation = offset.length();
      let (low, high) = cosine_range(arc, offset.angle());
      let base = separation * separation + radius * radius;
      let swing = 2.0 * separation * radius;
      let near = (base + swing * low).max(0.0).sqrt();
      let far = (base + swing * high).max(0.0).sqrt();
      let other_radius = other.radius();
      if other_radius >= near && other_radius <= far {
        0.0
      } else {
        (near - other_radius).abs().min((far - other_radius).abs())
      }
    }
  }
}

/// The range of `cos(angle - phase)` over `arc`'s sweep.
///
/// A cosine over an interval is extreme at the interval's ends and at its own turning points, so the
/// two ends and whichever of `phase` and `phase + π` the sweep covers are the whole candidate set.
fn cosine_range(arc: Arc, phase: f64) -> (f64, f64) {
  let start = arc.start_angle();
  let mut low = (start - phase).cos();
  let mut high = low;
  let end = (start + arc.sweep() - phase).cos();
  low = low.min(end);
  high = high.max(end);
  for turning in [phase, phase + PI] {
    if covers_angle(arc, turning) {
      let value = (turning - phase).cos();
      low = low.min(value);
      high = high.max(value);
    }
  }
  (low, high)
}

/// True when `angle` lies within `arc`'s sweep, measured in the sweep's direction.
///
/// The comparison leans outwards by a picoradian: counting a turning point a hair outside the sweep
/// only widens the range [`cosine_range`] reports, which can make [`arc_segment_lower_bound`] smaller
/// — more check points, never a piece kept that should have been dropped. Erring the other way would
/// narrow the range, and a narrowed range is the one way this bound could exceed the true distance.
fn covers_angle(arc: Arc, angle: f64) -> bool {
  let sweep = arc.sweep();
  if sweep == 0.0 {
    return false;
  }
  let delta = if sweep > 0.0 {
    wrap_turn(angle - arc.start_angle())
  } else {
    wrap_turn(arc.start_angle() - angle)
  };
  delta <= sweep.abs() + 1.0e-12
}

/// `angle` reduced to `[0, TAU)`.
fn wrap_turn(angle: f64) -> f64 {
  let wrapped = angle % TAU;
  if wrapped < 0.0 {
    wrapped + TAU
  } else {
    wrapped
  }
}

/// The shortest distance from `point` to any of `boundary`'s segments.
///
/// Segment distance is exact for arcs, so the proof measures against the geometry rather than against
/// a tessellation of it: an arc's own chord approximation would sit *inside* the boundary and make a
/// correct offset look short.
fn nearest_distance(point: Point2, boundary: &[Segment]) -> f64 {
  boundary
    .iter()
    .map(|segment| segment.distance_to_point(point))
    .fold(f64::INFINITY, f64::min)
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
