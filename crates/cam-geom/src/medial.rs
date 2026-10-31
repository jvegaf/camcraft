//! The medial axis of a region, verified rather than guessed (M7 A2, D-M7-2, D-M7-4, D-M7-5).
//!
//! A V-bit whose tip sits on a medial point `p` at depth `d(p) / tan(θ/2)` removes exactly the medial
//! ball `B(p, d(p))` and can never leave the region, and the union of those balls over the medial axis
//! is the whole region. That is why the medial axis *is* the V-carve toolpath, and why this module's
//! contract is the **certificate** and not the shape it happens to produce: a tip that is not a medial
//! point is a tip whose ball is smaller than it claims, which costs coverage (`uncut`) and nothing
//! else. Error is one-signed by construction, and a dropped piece can only ever show up as material
//! left standing.
//!
//! # The certificate
//!
//! A point of the region is on the medial axis exactly when its largest empty ball touches the
//! boundary in more than one place: the nearest set has **at least two distinct boundary points**, or
//! the nearest set is an interval and every point of it is at the same exact distance (an arc asked
//! about its own centre). The test is exact and cheap, and this module never trusts a candidate
//! without it:
//!
//! * [`Region::distance_to_boundary`] supplies the exact clearance, arc-native, from the same walk the
//!   V-carve's depth field uses — so a reported clearance and the surface it will cut cannot disagree;
//! * [`Segment::nearest_point`] supplies *which* boundary point each segment is nearest at, so the
//!   nearest set can be collected and deduped **by point**. The dedup is not a formality: two segments
//!   that share a vertex both report that one vertex, and without it every corner of every polygon
//!   would certify itself as a medial point. The dedup is a *distance*, and the resolution it is read at
//!   is the caller's tolerance — the same declaration the membership test makes, read on the boundary
//!   instead of in the region. Two contacts closer together than that are one point of the boundary,
//!   which is the only reading that survives a boundary approximated piece by piece: the arcs of an
//!   ellipse do not even agree about the coordinates of the vertex they share.
//!
//! A candidate that fails is dropped **whole** and counted in [`MedialAxis::candidates_dropped`]. It
//! is never nudged towards the axis, because a nudged tip is a lie about its own clearance.
//!
//! # Where the candidates come from
//!
//! Deriving pair bisectors analytically needs conics — every bisector involving an arc is a parabola
//! or a hyperbola — and this kernel has none. So candidates are *generated* cheaply and *verified*
//! exactly, in four steps:
//!
//! 1. The distance field is sampled on a lattice whose step **is** the caller's `tolerance`, and each
//!    sample is labelled with the boundary segment nearest to it.
//! 2. Where two neighbouring samples disagree about their segment, the two distances cross somewhere
//!    on that lattice edge, and a bisection finds the crossing. The crossing is on the bisector of
//!    *those two features* exactly, so a retained vertex is exact in that direction; the only error
//!    left is the chording between consecutive vertices. A crossing whose true nearest set turns out
//!    to be some third feature is dropped by (4) and counted.
//! 3. The crossings are connected within each lattice cell the way a marching-squares walk does, in
//!    the cell's own order, and walked into branches in a fixed order. A cell where all four edges
//!    cross is a junction: the point equidistant from the three nearest features is found by Newton
//!    and certified, and the four crossings meet there — so a square's diagonals really do meet at its
//!    centre and not at the middle of some cell.
//! 4. Every vertex is certified by the empty-ball test above before it is kept.
//!
//! 5. Two branch ends that are one place on the axis — a seam — are joined, so a candidate the walk
//!    lost cannot turn one piece of axis into two branches. The joining invents no vertex: it keeps
//!    every certified vertex, in its own order, and only adds the segment that closes the hole.
//!
//! An **isolated** medial point cannot be found by step 2, because it has no second feature to
//! disagree with: a disc's axis is one point and nothing else. Every boundary arc's centre is
//! therefore offered as a candidate as well, and the certificate decides.
//!
//! # Arcs are the original geometry, and no chord tolerance is introduced anywhere
//!
//! D-M7-4 assumed the axis would have to tessellate arcs, because a bisector involving an arc is a
//! conic and this kernel has none. The premise does not survive the implementation, and the reason is
//! the whole shape of the method: **no bisector is ever constructed.** The step-2 search is a root
//! find of the *scalar* function `dist(p, i) - dist(p, j)` along a lattice edge, evaluating the
//! kernel's exact point-to-arc distance, and the certificate asks the original arc for its exact
//! nearest point. A conic is therefore never needed, nothing is tessellated, and no chord tolerance is
//! introduced anywhere: **the axis is computed on the original geometry, arcs included.**
//!
//! What *is* approximate is short and named in full in "what the tolerance costs" below: the lattice's
//! step, the chording between consecutive vertices, a piece of axis shorter than the step, the
//! tolerance's effect near a convex corner, and the truncation at a curvature extremum. Every one of
//! them is bounded by the caller's `tolerance`, and not one of them is a chord of a boundary arc.
//!
//! The difference is measurable and is pinned by a test: a disc's axis is its centre, while the same
//! disc tessellated by the **caller** into chords has a star of spikes, one per vertex of the polygon.
//! The count that explains the difference is the caller's chord count, not this module's.
//!
//! # The accuracy contract
//!
//! * Every retained vertex is a medial point: its nearest set holds at least two distinct boundary
//!   points, or an interval of them, all at exactly its reported clearance — which is the exact
//!   distance to the boundary, not an estimate.
//! * A retained vertex lies on the axis of the two features whose bisector produced it to within
//!   `tolerance * 1e-9`, the bisection's resolution, and a junction vertex is refined to the same
//!   scale.
//! * The polyline **between** consecutive vertices chords the true axis, and that chording is the
//!   lattice's own step: at most `tolerance`. A piece of axis shorter than the step can be missed
//!   entirely, which costs coverage — the one signed direction — and never depth.
//! * The one exception to that chording is a **seam** (step 5): where a candidate was refused, the
//!   two ends it left are joined by a single straight segment of at most two lattice steps. Nothing
//!   is interpolated into it and no vertex is invented; the seam is the hole being closed, and the
//!   coverage measurement samples along it like any other polyline.
//! * A region thinner than `tolerance` has no axis at all rather than a wrong one: every point of it is
//!   within `tolerance` of the boundary, so no point qualifies as material (D-M7-5).
//!
//! # Topology is an answer, so it is repaired rather than inherited
//!
//! The certificate decides *which points are on the axis* and says nothing about how many branches
//! they form, and the walk that chains them can only see one lattice cell at a time. That leaves two
//! ways for the branch **count** to depend on the tolerance instead of on the shape, and both were
//! measured on the fixtures before they were fixed:
//!
//! * **An unverified contact.** Two arcs that meet at a vertex disagree about that vertex's
//!   coordinates by more than floating-point noise — the 5 x 2 ellipse's 16-arc boundary does, by up
//!   to `1.3e-5` mm at the joints — so a point whose nearest set is that one joint can be certified as
//!   having two. Every joint then grows a stub of "axis" whose two contacts are the same point of the
//!   boundary, and the ellipse reported **13** branches at 0.1 mm against **1** at 0.5 mm. Reading the
//!   contact dedup at a tenth of the tolerance removes every one of them (the number and the
//!   measurements either side of it are in `SAME_CONTACT_FRACTION`'s own docs): the change can only
//!   reject a candidate, and a rejected candidate is a hole, not a wrong tip.
//! * **A hole in the chain.** A candidate the certificate refuses is a crossing that disappears, and
//!   the walk, which wires one cell at a time, ends up with two branches where the axis is continuous.
//!   At 0.1 mm the same ellipse still split into three at the two columns whose sample row lies exactly
//!   on the axis, where the third feature nearest that row is decided by rounding noise. Step 5 closes
//!   those, and only those: an end is joined to another end within two lattice steps of it — as far as
//!   one refused crossing can hold two ends apart — when each branch points *into* the seam and the
//!   seam segment crosses nothing else.
//!
//! The result is the property the V-carve needs, and it is asserted rather than hoped for: **a finer
//! tolerance never returns more branches than a coarser one**, and a shape whose axis is one curve
//! reports one branch at every tolerance. The certificate is untouched by all of it: a joined branch
//! holds the same certified vertices in the same order, and `candidates_dropped` still counts every
//! refusal.
//!
//! # What the tolerance costs, declared
//!
//! Two costs are worth stating apart from the contract above, because they are what the milestone's
//! `uncut` will be made of and they must be a written bound rather than a surprise. Both cost
//! **coverage only**: by the claim's point 1 the surface can never rise above `-d / tan(θ/2)`, so a
//! missing tip leaves material standing and never cuts deeper.
//! * **The convex corner band.** Near a convex corner the walk stops one lattice step short, because a
//!   tip whose clearance is at or below the tolerance is not material. **Near a convex corner the
//!   carve therefore leaves at most `tolerance` of material standing, and the simulation reports it as
//!   `uncut`.** Measured on a square: 0.20 mm of missed depth at a 0.5 mm tolerance, 0.00 mm at 0.1 mm.
//!   The law itself is untouched — the surface still tapers to zero depth exactly at the edge, sharp
//!   corners included (A1) — what is missing is a tip, not depth.
//! * **A curvature extremum truncates a branch.** At the ends of an ellipse's major axis the two
//!   nearest contacts converge on one region of boundary, so the coarse lattice stops offering a
//!   candidate before the analytic endpoint and the branch ends early: measured on a 5 x 2 ellipse, the
//!   axis ends at `x = ±3.5` instead of `±4.2` at a 0.5 mm tolerance (0.7 mm short) and reaches `4.8`
//!   at 0.1 mm. The shortfall is the tolerance's, not the method's, and it is coverage, never depth.
//!
//! # Determinism (S10)
//!
//! The lattice is fixed by the region's bounds and the step, the labels are broken by segment index,
//! the cells are visited in row-major order, the vertex dedup index and the branch sort are ordered
//! containers, and no hash iteration reaches the output. Seams are joined in order of gap and then of
//! branch index, so even the repair is a function of the region. Two runs on the same region and
//! tolerance return the same branches, in the same order, vertex for vertex.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use crate::intersect::{intersect, Intersections};
use crate::point::{Bounds, Point2, Vec2};
use crate::region::Region;
use crate::segment::{LineSeg, Segment};
use crate::GeomError;

/// The bisection's resolution, as a fraction of the caller's tolerance.
///
/// Nine orders of magnitude below the tolerance: the bisection is a root finder, not a sampling step,
/// and it is run until the two features agree to floating-point noise. That is what makes a retained
/// vertex exact *in the direction that matters* — the only error left is the chording between
/// consecutive vertices, which the tolerance already bounds.
const BISECTION_FRACTION: f64 = 1.0e-9;

/// How far apart two boundary points may be and still count as one point, as a fraction of the
/// tolerance.
///
/// Two segments that share a vertex compute that vertex from their own ends, so the two answers differ
/// by floating-point noise; a genuine pair of distinct nearest points closer together than this is
/// indistinguishable from a shared vertex, and treating it as one is the conservative direction (it can
/// only reject a candidate).
const SAME_POINT_FRACTION: f64 = 1.0e-6;

/// Two **nearest boundary points** that are closer together than this fraction of the tolerance are
/// one contact, not two.
///
/// The certificate asks for at least two *distinct* nearest points, so "distinct" has to be said at a
/// resolution, and this is it. It is not a floating-point window: it is the scale at which a piecewise
/// boundary disagrees with itself about where it is. Two arcs that meet at a vertex do not agree about
/// that vertex's coordinates — the 5 x 2 ellipse's 16-arc chain is off by up to `1.3e-5` mm at every
/// joint — and worse, a query just off a joint's normal gets an interior foot on one arc and that
/// joint on the other, two points `1.3e-5` mm apart at *exactly* the same distance. The certificate
/// cannot tell that from a genuine pair of contacts, and it should not want to: it is one place on the
/// boundary. Without this window every joint of every arc chain grows a piece of "axis" whose two
/// contacts are one point, which is how the ellipse came to report thirteen branches at a fine
/// tolerance.
///
/// A tenth, and the number is a decision with measurements on both sides rather than a guess:
///
/// * it is three orders of magnitude above the measured disagreement (`1.3e-5` mm against a `0.01` mm
///   window at a `0.1` mm tolerance), so the artefacts are gone with room to spare;
/// * it is an order of magnitude **below** the caller's own resolution, because merging contacts that
///   are genuinely apart costs axis. At the full tolerance a regular 64-gon's shallow vertices keep
///   only 349 of their 1182 tips — the two contacts near a `174°` corner are closer than `0.5` mm for
///   a tip `5 mm` away from it — while at a tenth every one of them is kept.
///
/// It is also the conservative direction: it can only ever **reject** a candidate, and a rejected
/// candidate costs coverage, never depth (the claim's point 1).
const SAME_CONTACT_FRACTION: f64 = 0.1;

/// How far apart two branch ends may be and still be the same place on the axis, as a multiple of the
/// tolerance.
///
/// The walk is repaired, not guessed: a candidate the certificate refused is a lattice crossing that
/// is gone, and the crossings that survive on either side of it are two lattice edges apart — one
/// cell's width each way. So an end may be joined to another end within two steps, and no further:
/// a longer reach would start joining branches that merely point at each other rather than branches
/// that are one branch.
const STITCH_REACH_FRACTION: f64 = 2.0;

/// Safety bound on the bisection. A lattice edge is at most `tolerance` long and the loop halves it,
/// so ~60 steps reach any representable tolerance; the bound only stops a pathological input from
/// spinning.
const MAX_BISECTION_STEPS: usize = 100;

/// Safety bound on the junction's Newton iteration. A junction between line features converges in one
/// step; the bound covers the curved cases.
const MAX_JUNCTION_STEPS: usize = 24;

/// Upper bound on lattice samples, so a tolerance far below the region's size refuses instead of
/// allocating without limit (the same discipline as [`crate::MAX_TESSELLATION_STEPS`]).
pub const MAX_LATTICE_SAMPLES: usize = 4_000_000;

/// One vertex of a medial branch: where the tip goes, and how deep it is.
///
/// `clearance` is the **exact** distance from `point` to the region's boundary, so a V-carve's depth
/// at that tip is `clearance / tan(θ/2)` for a bit of included angle `θ` (M7 A1). It is not a lower
/// bound and not a chord estimate: it comes from [`Region::distance_to_boundary`], arcs native.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MedialVertex {
  /// The tip, in the document frame, in millimetres.
  pub point: Point2,
  /// The exact distance from `point` to the boundary, in millimetres. Always positive.
  pub clearance: f64,
}

/// An ordered polyline of the medial axis: one piece of toolpath (A3).
///
/// A closed branch does **not** repeat its first vertex at the end; the caller closes it, the way
/// [`crate::Contour`] does. A branch of one vertex is legal and is exactly a disc's axis.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MedialBranch {
  /// The vertices, in walk order.
  pub vertices: Vec<MedialVertex>,
  /// True when the branch is a loop rather than a path with two ends.
  pub closed: bool,
}

/// The medial axis of a region, with the count the contract requires.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MedialAxis {
  /// The branches, ordered by their first vertex, then by their vertex sequence (S10).
  pub branches: Vec<MedialBranch>,
  /// Candidates the certificate examined and **dropped whole** (D-M7-2).
  ///
  /// Every way a candidate can fail is counted here and nothing is repaired in place: a candidate
  /// outside the material (the inside of a hole, the far side of a wall), a candidate whose nearest
  /// set holds only one point, and a candidate whose two features turned out not to be the nearest
  /// ones. The count is what makes a silent axis impossible to mistake for a complete one.
  pub candidates_dropped: usize,
}

/// The medial axis of `region`, refined to `tolerance` millimetres (M7 A2).
///
/// See the module documentation for the certificate every retained vertex passes, for the accuracy
/// contract, and for what the tolerance costs. The tolerance is the caller's: it is the lattice step
/// and therefore the chording bound, and it is never a hidden sampling resolution. A region thinner
/// than the tolerance has an empty axis.
///
/// # Errors
///
/// [`GeomError::EditRefused`] when `tolerance` is not positive and finite, and when the lattice the
/// tolerance implies would need more than [`MAX_LATTICE_SAMPLES`] samples — a refusal by name instead
/// of a hang or a silent coarsening of the caller's tolerance.
pub fn medial_axis(region: &Region, tolerance: f64) -> Result<MedialAxis, GeomError> {
  if !tolerance.is_finite() || tolerance <= 0.0 {
    return Err(GeomError::EditRefused {
      reason: "the medial axis needs a positive finite tolerance, in millimetres",
    });
  }
  let features: Vec<Segment> = region
    .boundaries
    .iter()
    .flat_map(|boundary| boundary.segments())
    .collect();
  if features.is_empty() {
    return Ok(MedialAxis::default());
  }
  Axis::new(region, features, tolerance).build()
}

/// The construction's working state, so the steps can stay small and the borrows tidy.
struct Axis<'a> {
  region: &'a Region,
  features: Vec<Segment>,
  tolerance: f64,
  /// The bisection's resolution, and the width of the certificate's equality window.
  resolution: f64,
  /// Two boundary points within this are one point.
  same_point: f64,
  /// Two *nearest* boundary points within this are one contact (see [`SAME_CONTACT_FRACTION`]).
  same_contact: f64,
  nodes: Vec<MedialVertex>,
  links: Vec<(usize, usize)>,
  /// Distinct links already made, so a cell cannot connect the same pair twice.
  made: BTreeSet<(usize, usize)>,
  /// Vertex lookup by quantised position, for the dedup of crossings that land on a lattice sample.
  near: BTreeMap<(i64, i64), Vec<usize>>,
  dropped: usize,
}

impl<'a> Axis<'a> {
  fn new(region: &'a Region, features: Vec<Segment>, tolerance: f64) -> Self {
    let resolution = tolerance * BISECTION_FRACTION;
    Self {
      region,
      features,
      tolerance,
      resolution,
      same_point: tolerance * SAME_POINT_FRACTION,
      same_contact: tolerance * SAME_CONTACT_FRACTION,
      nodes: Vec::new(),
      links: Vec::new(),
      made: BTreeSet::new(),
      near: BTreeMap::new(),
      dropped: 0,
    }
  }

  /// Run the four steps and hand back the axis, its branches in a fixed order.
  fn build(mut self) -> Result<MedialAxis, GeomError> {
    let bounds = self
      .features
      .iter()
      .fold(Bounds::empty(), |bounds, feature| {
        bounds.union(feature.bounds())
      });
    let step = self.tolerance;
    let cols = (bounds.width() / step).ceil() as usize + 1;
    let rows = (bounds.height() / step).ceil() as usize + 1;
    if cols.saturating_mul(rows) > MAX_LATTICE_SAMPLES {
      return Err(GeomError::EditRefused {
        reason: "the medial axis lattice needs more samples than this tolerance allows",
      });
    }

    // (0) The candidates a distance-field lattice cannot see: an isolated medial point, of which a
    // disc's centre is the whole population.
    let centres: Vec<Point2> = self
      .features
      .iter()
      .filter_map(|feature| feature.as_arc().map(|arc| arc.center()))
      .collect();
    for centre in centres {
      if let Some(clearance) = self.certify(centre) {
        self.add_node(centre, clearance);
      }
    }

    // (1) Label every sample with the segment that is nearest to it.
    let sample = |col: usize, row: usize| {
      Point2::new(
        bounds.min.x + col as f64 * step,
        bounds.min.y + row as f64 * step,
      )
    };
    let mut labels = vec![0_usize; cols * rows];
    for row in 0..rows {
      for col in 0..cols {
        labels[row * cols + col] = self.label(sample(col, row));
      }
    }

    // (2) Bisect every lattice edge whose ends disagree. Each edge yields at most one vertex.
    let mut horizontal = vec![None; cols.saturating_sub(1) * rows];
    let mut vertical = vec![None; cols * rows.saturating_sub(1)];
    for row in 0..rows {
      for col in 0..cols.saturating_sub(1) {
        let low = labels[row * cols + col];
        let high = labels[row * cols + col + 1];
        if low == high {
          continue;
        }
        let crossing = self.crossing(sample(col, row), sample(col + 1, row), low, high);
        if let Some(clearance) = self.certify(crossing) {
          horizontal[row * (cols - 1) + col] = Some(self.add_node(crossing, clearance));
        }
      }
    }
    for row in 0..rows.saturating_sub(1) {
      for col in 0..cols {
        let low = labels[row * cols + col];
        let high = labels[(row + 1) * cols + col];
        if low == high {
          continue;
        }
        let crossing = self.crossing(sample(col, row), sample(col, row + 1), low, high);
        if let Some(clearance) = self.certify(crossing) {
          vertical[row * cols + col] = Some(self.add_node(crossing, clearance));
        }
      }
    }

    // (3) Wire the crossings within each cell, the way a marching-squares walk does.
    for row in 0..rows.saturating_sub(1) {
      for col in 0..cols.saturating_sub(1) {
        let bottom = horizontal[row * (cols - 1) + col];
        let right = vertical[row * cols + col + 1];
        let top = horizontal[(row + 1) * (cols - 1) + col];
        let left = vertical[row * cols + col];
        self.wire(
          [bottom, right, top, left],
          labels[row * cols + col],
          [
            sample(col, row),
            sample(col + 1, row),
            sample(col + 1, row + 1),
            sample(col, row + 1),
          ],
        );
      }
    }

    // (4) Walk the graph into branches, then repair the seams a dropped candidate leaves.
    let branches = chains(&self.nodes, &self.links, self.same_point);
    let branches = stitch(
      branches,
      self.tolerance * STITCH_REACH_FRACTION,
      self.same_point,
    );
    Ok(MedialAxis {
      branches,
      candidates_dropped: self.dropped,
    })
  }

  /// The index of the boundary segment nearest to `point`, ties broken by the lower index.
  fn label(&self, point: Point2) -> usize {
    let mut nearest = f64::INFINITY;
    let mut label = 0;
    for (index, feature) in self.features.iter().enumerate() {
      let distance = feature.distance_to_point(point);
      if distance < nearest {
        nearest = distance;
        label = index;
      }
    }
    label
  }

  /// The point where the distances to features `low` and `high` are equal, on the edge `from..to`.
  ///
  /// `low` is the label at `from` and `high` the label at `to`, so the difference is non-positive at
  /// one end and non-negative at the other and a bracket always exists. The exact endpoints are
  /// checked first: when the lattice lands exactly on a bisector — a square's diagonal through its own
  /// centre, a rectangle's midline — the tie *is* the crossing, and returning the sample keeps the
  /// vertex exact instead of one bisection resolution to its side.
  fn crossing(&self, from: Point2, to: Point2, low: usize, high: usize) -> Point2 {
    let difference = |point: Point2| {
      self.features[low].distance_to_point(point) - self.features[high].distance_to_point(point)
    };
    if difference(from) == 0.0 {
      return from;
    }
    if difference(to) == 0.0 {
      return to;
    }
    let length = from.distance_to(to);
    let mut below = 0.0;
    let mut above = 1.0;
    for _ in 0..MAX_BISECTION_STEPS {
      if (above - below) * length <= self.resolution {
        break;
      }
      let middle = 0.5 * (below + above);
      let value = difference(from.lerp(to, middle));
      if value == 0.0 {
        return from.lerp(to, middle);
      }
      if value > 0.0 {
        above = middle;
      } else {
        below = middle;
      }
    }
    from.lerp(to, 0.5 * (below + above))
  }

  /// The empty-ball certificate (A2): the clearance at `point`, or `None` with a drop counted.
  ///
  /// The distance and the membership come from [`Region::distance_to_boundary`], so they are the
  /// V-carve's own depth field; the nearest *points* come from each segment so the set can be deduped
  /// by point. A candidate within `tolerance` of the boundary is not material, which is what makes a
  /// region thinner than the tolerance report an empty axis.
  fn certify(&mut self, point: Point2) -> Option<f64> {
    // Membership at the caller's own tolerance: a point within `tolerance` of the wall is not
    // material, which is what makes a region thinner than the tolerance report an empty axis instead of
    // an axis at a clearance the caller already called indistinguishable from the boundary (D-M7-5).
    let Some(answer) = self.region.distance_to_boundary(point, self.tolerance) else {
      self.dropped += 1;
      return None;
    };
    if !answer.inside {
      self.dropped += 1;
      return None;
    }
    // A crossing bisected to `resolution` has its two features within `2 * resolution` of each other,
    // plus the floating-point noise of comparing distances of that magnitude.
    let window = answer.distance + 4.0 * self.resolution + 1.0e-12 * (1.0 + answer.distance);
    let mut contacts: Vec<Point2> = Vec::with_capacity(4);
    let mut interval = false;
    for feature in &self.features {
      let nearest = feature.nearest_point(point);
      if nearest.distance > window {
        continue;
      }
      interval |= nearest.interval;
      if !contacts
        .iter()
        .any(|other| other.is_near(nearest.point, self.same_contact))
      {
        contacts.push(nearest.point);
      }
    }
    if contacts.len() >= 2 || interval {
      Some(answer.distance)
    } else {
      self.dropped += 1;
      None
    }
  }

  /// Record a certified vertex, reusing an existing one at the same point.
  fn add_node(&mut self, point: Point2, clearance: f64) -> usize {
    let key = (
      (point.x / self.same_point).floor() as i64,
      (point.y / self.same_point).floor() as i64,
    );
    for dx in -1..=1 {
      for dy in -1..=1 {
        if let Some(existing) = self.near.get(&(key.0 + dx, key.1 + dy)) {
          for &index in existing {
            if self.nodes[index].point.is_near(point, self.same_point) {
              return index;
            }
          }
        }
      }
    }
    let index = self.nodes.len();
    self.nodes.push(MedialVertex { point, clearance });
    self.near.entry(key).or_default().push(index);
    index
  }

  /// Connect two certified vertices, once.
  fn link(&mut self, first: usize, second: usize) {
    if first == second {
      return;
    }
    let pair = (first.min(second), first.max(second));
    if self.made.insert(pair) {
      self.links.push(pair);
    }
  }

  /// Connect the crossings of one lattice cell.
  ///
  /// Zero or one crossing is a branch end and needs no wiring. Two is the ordinary case. Four is the
  /// ambiguous one, and the certificate decides it: if the point equidistant from the three nearest
  /// features is a medial point, the cell holds a **junction** and all four crossings meet there —
  /// which is how a square's two diagonals meet at the centre and not in the middle of a cell. If it
  /// is not, the crossings pair up around the cell's odd corner, as a marching-squares walk does.
  fn wire(&mut self, edges: [Option<usize>; 4], corner_label: usize, corners: [Point2; 4]) {
    // A crossing that lands on a lattice sample can be reached from more than one edge of the cell,
    // so the list is deduped by node *before* it is counted: the square's own corner cells agree about
    // their one crossing three times over, and counting them as four would invent a junction.
    let mut present: Vec<usize> = Vec::with_capacity(4);
    for edge in edges.iter().flatten() {
      if !present.contains(edge) {
        present.push(*edge);
      }
    }
    match present.len() {
      0 | 1 => {}
      2 => self.link(present[0], present[1]),
      3 => {
        // Two of the four crossings collapsed into one vertex, so this is a very small junction. The
        // three remaining vertices are within `same_point` of each other and the branch collapse that
        // follows folds them back into one.
        for index in 0..3 {
          self.link(present[index], present[(index + 1) % 3]);
        }
      }
      _ => {
        let middle = Point2::new(
          (corners[0].x + corners[1].x + corners[2].x + corners[3].x) * 0.25,
          (corners[0].y + corners[1].y + corners[2].y + corners[3].y) * 0.25,
        );
        if let Some(junction) = self.junction(middle) {
          for index in present {
            self.link(junction, index);
          }
          return;
        }
        // The odd corner is the one whose label differs from the cell's middle: if it is the
        // bottom-left corner the crossings pair around it, otherwise they pair around its opposite.
        let centre_label = self.label(middle);
        let pairs: [(usize, usize); 2] = if centre_label == corner_label {
          [(0, 3), (1, 2)]
        } else {
          [(0, 1), (2, 3)]
        };
        for (first, second) in pairs {
          if let (Some(a), Some(b)) = (edges[first], edges[second]) {
            self.link(a, b);
          }
        }
      }
    }
  }

  /// The point equidistant from the three features nearest `seeded`, if the certificate accepts it.
  ///
  /// Two equations, two unknowns: `d_a = d_b` and `d_b = d_c`, solved by Newton with the distances'
  /// exact gradients `(p - nearest) / |p - nearest|`, which every segment kind provides through
  /// [`Segment::nearest_point`].
  fn junction(&mut self, seeded: Point2) -> Option<usize> {
    let mut order: Vec<(usize, f64)> = self
      .features
      .iter()
      .enumerate()
      .map(|(index, feature)| (index, feature.distance_to_point(seeded)))
      .collect();
    order.sort_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
    if order.len() < 3 {
      return None;
    }
    let picks = [order[0].0, order[1].0, order[2].0];
    let mut point = seeded;
    for _ in 0..MAX_JUNCTION_STEPS {
      let mut gradients: Vec<Vec2> = Vec::with_capacity(3);
      let mut distances: Vec<f64> = Vec::with_capacity(3);
      for pick in picks {
        let nearest = self.features[pick].nearest_point(point);
        if nearest.interval {
          return None;
        }
        let away = point - nearest.point;
        gradients.push(away.normalized()?);
        distances.push(nearest.distance);
      }
      let target = [distances[0] - distances[1], distances[1] - distances[2]];
      let row0 = gradients[0] - gradients[1];
      let row1 = gradients[1] - gradients[2];
      let determinant = row0.x * row1.y - row0.y * row1.x;
      if determinant.abs() <= 1.0e-14 {
        return None;
      }
      let step = Vec2::new(
        (-target[0] * row1.y + target[1] * row0.y) / determinant,
        (-target[1] * row0.x + target[0] * row1.x) / determinant,
      );
      point = Point2::new(point.x + step.x, point.y + step.y);
      if step.length() <= self.resolution {
        break;
      }
    }
    // A four-crossing cell holds its junction: two bisectors cross inside the cell, so the point
    // equidistant from three features is within a couple of steps of its middle. A Newton iteration
    // that wandered further found some other medial point and must not be wired here.
    if point.distance_to(seeded) > 2.0 * self.tolerance {
      return None;
    }
    let clearance = self.certify(point)?;
    Some(self.add_node(point, clearance))
  }
}

/// Walk the link graph into branches, in a fixed order (S10).
///
/// A node of degree two is an ordinary point of a branch; anything else — an end, a junction, or an
/// isolated point with no links at all — starts a branch, and each of its unvisited links is walked
/// through degree-two nodes until the next such node. Whatever is left over is a loop with no junction
/// on it, which a closed mid-line between two boundaries is.
fn chains(nodes: &[MedialVertex], links: &[(usize, usize)], same_point: f64) -> Vec<MedialBranch> {
  let mut adjacency: Vec<Vec<(usize, usize)>> = vec![Vec::new(); nodes.len()];
  for (index, &(first, second)) in links.iter().enumerate() {
    adjacency[first].push((second, index));
    adjacency[second].push((first, index));
  }
  for list in &mut adjacency {
    list.sort_unstable();
  }

  let mut visited = vec![false; links.len()];
  let mut branches = Vec::new();
  for node in 0..nodes.len() {
    if adjacency[node].len() == 2 {
      continue;
    }
    if adjacency[node].is_empty() {
      branches.push(MedialBranch {
        vertices: vec![nodes[node]],
        closed: false,
      });
      continue;
    }
    for (_, link) in adjacency[node].clone() {
      if visited[link] {
        continue;
      }
      let (path, closed) = walk(links, &adjacency, &mut visited, node, link);
      branches.push(make_branch(nodes, path, closed, same_point));
    }
  }
  for link in 0..links.len() {
    if visited[link] {
      continue;
    }
    let (path, closed) = walk(links, &adjacency, &mut visited, links[link].0, link);
    branches.push(make_branch(nodes, path, closed, same_point));
  }
  branches.sort_by(compare_branches);
  branches
}

/// Walk from `start` along `first` until the path closes or reaches a node that is not of degree two.
fn walk(
  links: &[(usize, usize)],
  adjacency: &[Vec<(usize, usize)>],
  visited: &mut [bool],
  start: usize,
  first: usize,
) -> (Vec<usize>, bool) {
  let mut path = vec![start];
  let mut current = start;
  let mut link = first;
  loop {
    visited[link] = true;
    let (first_end, second_end) = links[link];
    let next = if first_end == current {
      second_end
    } else {
      first_end
    };
    path.push(next);
    if next == start {
      return (path, true);
    }
    if adjacency[next].len() != 2 {
      return (path, false);
    }
    let onward = adjacency[next]
      .iter()
      .find(|(_, other)| *other != link)
      .map(|(_, other)| *other);
    match onward {
      Some(other) => {
        current = next;
        link = other;
      }
      None => return (path, false),
    }
  }
}

/// Turn a walk of node indices into a branch: collapse repeated positions, then fix the order.
fn make_branch(
  nodes: &[MedialVertex],
  path: Vec<usize>,
  closed: bool,
  same_point: f64,
) -> MedialBranch {
  let mut vertices: Vec<MedialVertex> = Vec::with_capacity(path.len());
  for index in path {
    let vertex = nodes[index];
    if let Some(last) = vertices.last() {
      if last.point.is_near(vertex.point, same_point) {
        continue;
      }
    }
    vertices.push(vertex);
  }
  let mut closed = closed;
  if closed {
    // The walk ends where it started; the caller closes the loop, as a `Contour` does.
    if let (Some(first), Some(last)) = (vertices.first(), vertices.last()) {
      if first.point.is_near(last.point, same_point) {
        vertices.pop();
      }
    }
    // A loop of one or two collapsed vertices is not a loop.
    closed = vertices.len() >= 3;
  }
  orient(vertices, closed)
}

/// Put a branch's vertices in the one order every run agrees on.
///
/// A closed branch starts at its lowest vertex and turns towards the lower neighbour first, so two
/// runs agree on where a loop begins and which way it turns; an open branch starts at whichever of
/// its two ends is lower, so "the first vertex" is a property of the axis and not of the walk.
fn orient(mut vertices: Vec<MedialVertex>, closed: bool) -> MedialBranch {
  if closed {
    let start = (0..vertices.len())
      .min_by(|&a, &b| compare_vertices(&vertices[a], &vertices[b]))
      .unwrap_or(0);
    vertices.rotate_left(start);
    let count = vertices.len();
    if count > 2 && compare_vertices(&vertices[count - 1], &vertices[1]) == Ordering::Less {
      let first = vertices[0];
      let mut reversed = vec![first];
      reversed.extend(vertices[1..].iter().rev().copied());
      vertices = reversed;
    }
  } else if vertices.len() > 1
    && compare_vertices(&vertices[vertices.len() - 1], &vertices[0]) == Ordering::Less
  {
    vertices.reverse();
  }
  MedialBranch { vertices, closed }
}

/// Which end of a branch a seam leaves from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum End {
  /// The first vertex.
  First,
  /// The last vertex.
  Last,
}

/// One end of a branch that faces `End::Last`'s way: its tip, and the direction out of the branch.
fn end_outward(branch: &MedialBranch, end: End) -> Option<(Point2, Vec2)> {
  if branch.closed || branch.vertices.len() < 2 {
    return None;
  }
  let (tip, behind) = match end {
    End::First => (branch.vertices[0].point, branch.vertices[1].point),
    End::Last => {
      let count = branch.vertices.len();
      (
        branch.vertices[count - 1].point,
        branch.vertices[count - 2].point,
      )
    }
  };
  Some((tip, (tip - behind).normalized()?))
}

/// A pair of branch ends that continue each other, and how far apart they are.
struct Seam {
  first: usize,
  first_end: End,
  second: usize,
  second_end: End,
  gap: f64,
}

/// Join branch ends that are the same place on the axis, once each, until none are left.
///
/// This is the repair pass, and it is deliberately the *only* thing it does: no vertex is invented,
/// no clearance is recomputed, and every vertex was certified before it got here. What it fixes is a
/// seam — a place where one candidate was refused and the walk therefore ended twice on either side
/// of the hole, which would otherwise turn one piece of axis into two branches (and, downstream, two
/// retracts for one continuous cut).
///
/// A junction whose own refined vertex was refused looks exactly the same from here: its arms end
/// within a step of each other rather than at a shared vertex. So a join can also carry the walk
/// *through* a junction, which costs nothing — the join is a straight step between two certified
/// convex combinations of certified tips — and buys the opposite of what the fragmentation cost: one
/// continuous cut where there would have been two. A junction whose vertex *was* found is untouched:
/// its arms share that vertex, and two ends at one point have no direction between them to join.
///
/// The three conditions an end pair must satisfy are what keep the repair honest:
///
/// * they are within [`STITCH_REACH_FRACTION`] lattice steps of each other, which is as far as one
///   refused crossing can hold them apart;
/// * each branch points **into** the seam, so a pair that merely ends near each other — the two tips
///   of one corner's branches, a hairpin — is not a seam and is left alone;
/// * the seam segment crosses nothing, so the walk cannot be made to cross itself.
fn stitch(branches: Vec<MedialBranch>, reach: f64, same_point: f64) -> Vec<MedialBranch> {
  let mut branches = branches;
  loop {
    let mut best: Option<Seam> = None;
    for first in 0..branches.len() {
      for second in (first + 1)..branches.len() {
        for first_end in [End::First, End::Last] {
          for second_end in [End::First, End::Last] {
            let Some(seam) = seam_between(
              &branches, first, first_end, second, second_end, reach, same_point,
            ) else {
              continue;
            };
            if best.as_ref().map_or(true, |current| seam.gap < current.gap) {
              best = Some(seam);
            }
          }
        }
      }
    }
    let Some(seam) = best else { break };
    let joined = join(
      &branches[seam.first],
      seam.first_end,
      &branches[seam.second],
      seam.second_end,
      same_point,
    );
    branches[seam.first] = joined;
    branches.remove(seam.second);
  }
  branches.sort_by(compare_branches);
  branches
}

/// Whether these two ends are one place on the axis, and how far apart they are.
fn seam_between(
  branches: &[MedialBranch],
  first: usize,
  first_end: End,
  second: usize,
  second_end: End,
  reach: f64,
  same_point: f64,
) -> Option<Seam> {
  let (from, out_of_first) = end_outward(&branches[first], first_end)?;
  let (to, out_of_second) = end_outward(&branches[second], second_end)?;
  let gap = from.distance_to(to);
  if gap > reach {
    return None;
  }
  let direction = (to - from).normalized()?;
  // Each branch must point into the seam. Two ends that point away from it, or sideways, are two
  // places on the axis that happen to be near each other.
  if out_of_first.dot(direction) <= 0.0 || out_of_second.dot(direction) >= 0.0 {
    return None;
  }
  if !seam_is_clear(&branches[first], &branches[second], from, to, same_point) {
    return None;
  }
  Some(Seam {
    first,
    first_end,
    second,
    second_end,
    gap,
  })
}

/// True when the seam segment meets nothing but the two ends it joins.
///
/// A join never reorders vertices within a branch, so the one way it could make a walk cross itself is
/// the seam itself running over some other part of the axis. It is checked against every edge of both
/// branches, and the only meetings it may have are at its own two endpoints.
fn seam_is_clear(
  first: &MedialBranch,
  second: &MedialBranch,
  from: Point2,
  to: Point2,
  same_point: f64,
) -> bool {
  let seam = Segment::Line(LineSeg::new(from, to));
  for branch in [first, second] {
    for pair in branch.vertices.windows(2) {
      let edge = Segment::Line(LineSeg::new(pair[0].point, pair[1].point));
      let meeting = intersect(seam, edge, same_point);
      if matches!(meeting, Intersections::Coincident) {
        return false;
      }
      for point in meeting.points() {
        if !point.is_near(from, same_point) && !point.is_near(to, same_point) {
          return false;
        }
      }
    }
  }
  true
}

/// Concatenate two branches at their seam ends, keeping every vertex in its own order.
fn join(
  first: &MedialBranch,
  first_end: End,
  second: &MedialBranch,
  second_end: End,
  same_point: f64,
) -> MedialBranch {
  let ends_last = |branch: &MedialBranch, end: End| -> Vec<MedialVertex> {
    let mut vertices = branch.vertices.clone();
    if end == End::First {
      vertices.reverse();
    }
    vertices
  };
  let mut vertices = ends_last(first, first_end);
  let mut tail = ends_last(second, second_end);
  tail.reverse();
  // The seam's two ends are the same place: one vertex carries it, and no vertex is invented.
  if let (Some(head), Some(next)) = (vertices.last(), tail.first()) {
    if head.point.is_near(next.point, same_point) {
      tail.remove(0);
    }
  }
  vertices.extend(tail);
  orient(vertices, false)
}

/// Order two vertices by position: `y` first, then `x`, the region booleans' own convention.
fn compare_vertices(first: &MedialVertex, second: &MedialVertex) -> Ordering {
  first
    .point
    .y
    .total_cmp(&second.point.y)
    .then_with(|| first.point.x.total_cmp(&second.point.x))
}

/// Order two branches by their vertex sequences, vertex for vertex, so the branch order is a function
/// of the region and nothing else (S10).
fn compare_branches(first: &MedialBranch, second: &MedialBranch) -> Ordering {
  for (a, b) in first.vertices.iter().zip(second.vertices.iter()) {
    let order = compare_vertices(a, b);
    if order != Ordering::Equal {
      return order;
    }
  }
  first
    .vertices
    .len()
    .cmp(&second.vertices.len())
    .then_with(|| first.closed.cmp(&second.closed))
}
