//! Free-profile nesting: a **sound but incomplete** no-fit heuristic for parts that are contours
//! (M9 T3; S6, S7).
//!
//! S7 ships rectangles first because they cover cabinetry, and then asks for the parts a rectangle
//! cannot express: a curved rail, a moulded panel, an L-shaped bracket. This module nests those, and it
//! holds plain geometry only — the board is a [`Contour`] and every part is its own [`Contour`],
//! anchored by its own bounding box, so nothing here knows about documents, tools or authored sheets.
//! `cam-model` and `cam-cli` map their own types onto these, exactly as they do for [`crate::nest`].
//!
//! # What this is not: an exact NFP
//!
//! The published answer to a free-profile nest is the **no-fit polygon**: the locus of the moving
//! part's anchor for which it collides with a part already placed, which is the Minkowski difference of
//! the two outlines. Building that polygon needs a boolean **union** — one placed part grown around
//! each of its own edges and corners — and this kernel has none: [`Region`] offers
//! [`Region::subtract`] and nothing to build a region *with*. So this module claims no NFP anywhere,
//! and it is not trying to be optimal: roadmap decision D4 and spec S5 ask for a **heuristic** engine,
//! not for optimality. What it does claim is the pair of properties that make a heuristic usable:
//!
//! * **Sound.** A placement is accepted only when the candidate's own outline, and the no-fit region of
//!   *every* part already on that board, are [`Relation::Disjoint`] from it. Overlap is therefore
//!   impossible, not unlikely.
//! * **Incomplete, and it says so.** Candidate anchors come from a coarse lattice of
//!   [`ProfileNestOptions::step`] millimetres, so a placement that exists between two lattice points is
//!   missed. The cost is pinned by `the_coarse_search_step_misses_a_tight_placement_that_exists` in
//!   `tests/nest_profile.rs`, with every number written out and with what a complete nester would have
//!   had to do instead, because a limit this repo pays for is measured and stated rather than left
//!   implicit.
//!
//! # The no-fit region, without a union
//!
//! For one placed part, the region a candidate must stay out of is that part **grown outward by the
//! gap**: a negative distance grows a region's material (A6), so [`Region::offset`] produces it, and
//! [`crate::winding::relation`] decides disjointness. No second intersector is written here; the
//! kernel's own relation is the only thing that ever answers "do these two shapes meet".
//!
//! Two rules then make a placement sound, and they are kept separate on purpose:
//!
//! 1. The candidate outline must be **disjoint from every outline already on the board**. This is the
//!    no-overlap guarantee, and it never depends on the offset kernel's accuracy: it is decided between
//!    the parts themselves.
//! 2. The candidate outline must be **disjoint from every no-fit region** on the board. Being outside a
//!    part grown by the gap is what "at least the gap away" means, and it is the offset kernel's job.
//!
//! [`Relation::Disjoint`] is **strict**: two outlines that share a single point are
//! [`Relation::Touching`], and a touching candidate is refused. That is deliberate, and it is the
//! price of soundness rather than an oversight: a candidate lying *inside* a no-fit region and
//! touching its boundary — a thin cap hugging the grown edge of the placed part — is also
//! `Touching`, and accepting tangency would accept that cap and break the gap by an arbitrary amount.
//! Refusing contact means the plan pays one search step beyond the gap, which the fixtures measure, and
//! a zero gap means "further apart than touching": fewer placements than an exact nester would find,
//! never an illegal one.
//!
//! # The gap
//!
//! The gap is the minimum separation between two parts on a board, and the plan honours it as a
//! **floor**: no two parts are ever closer than the gap, and the lattice decides how much further apart
//! neighbours end up. A part may sit flush against the board's own edge, because the gap is the room a
//! cut needs *between* parts.
//!
//! # Rotation
//!
//! A quarter turn is offered for a part that has **no** upright placement on any open board — never as
//! a tie-break: every open board is searched upright first, and only then turned. A turned part is
//! turned because that is the only way it enters a board, and the placement says so
//! ([`crate::nest::NestRotation`], the same vocabulary the rectangle nester uses, so a caller has one
//! rotation concept and not two).
//!
//! # Sheets
//!
//! Boards are opened one at a time, only when the parts already placed leave no room, and a part that
//! fits no board is reported in [`ProfileNestPlan::unplaced`] — never dropped, and never squeezed past a
//! board edge. The first anchor tried on an empty board is the board's own lower-left corner and a part
//! may sit flush against the edges, so a part that fits an empty convex board — every board a shop cuts
//! from — is never reported unplaced: "unplaced" says the part does not fit, not that it did not fit
//! the board that happened to be tried.
//!
//! # Determinism
//!
//! Parts are sorted by area descending with a **stable** sort, so equal areas keep the caller's order.
//! Candidate anchors are the lattice `step` apart in `y` then `x` order, the boards are a `Vec` searched
//! from the first, and nothing here iterates a hash map. Two runs over the same input therefore return
//! the same placements **in the same order**.

use crate::contour::Contour;
use crate::point::{Bounds, Point2, Vec2};
use crate::region::Region;
use crate::transform;
use crate::winding::{relation, Relation};
use crate::GeomError;

pub use crate::nest::NestRotation;

/// The coarse search step a run uses when the caller authors none, in millimetres.
pub const DEFAULT_SEARCH_STEP: f64 = 10.0;

/// The name of the one algorithm this module implements, reported by
/// [`ProfileNestPlan::algorithm`].
///
/// One name and not an enum with one variant: the module has a single heuristic, and the point of
/// naming it is that a report says which engine produced a plan instead of leaving the caller to infer
/// it from the module path.
pub const NO_FIT_HEURISTIC: &str = "free-profile no-fit heuristic";

/// Tolerance for every position comparison this module makes, in millimetres.
///
/// The kernel's own default: one micrometre. Every decision below is a tolerance problem, and taking
/// the crate's tolerance keeps this module's answers consistent with the region and winding code it
/// asks.
const TOLERANCE: f64 = crate::DEFAULT_TOLERANCE;

/// The options a free-profile nesting run is driven by.
///
/// The numbers are decisions the caller authors — the gap between parts, whether a part may be turned,
/// and how finely the heuristic searches — so they arrive together rather than as positional arguments
/// that a call site can swap. [`ProfileNestOptions::new`] takes the two a caller always has and starts
/// the step at [`DEFAULT_SEARCH_STEP`]; [`ProfileNestOptions::with_step`] sets the step, which is the
/// knob that trades search time for placements found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProfileNestOptions {
  gap: f64,
  allow_rotation: bool,
  step: f64,
}

impl ProfileNestOptions {
  /// Build the options for a run: `gap` millimetres between parts, and whether a quarter turn is
  /// allowed.
  ///
  /// The search step starts at [`DEFAULT_SEARCH_STEP`].
  ///
  /// # Errors
  ///
  /// [`GeomError::GapNotMeasurable`] when `gap` is negative or not finite, with its value: a gap that
  /// is not a distance cannot separate anything, and it is refused rather than replaced by zero.
  pub fn new(gap: f64, allow_rotation: bool) -> Result<Self, GeomError> {
    if !gap.is_finite() || gap < 0.0 {
      return Err(GeomError::GapNotMeasurable { gap });
    }
    Ok(Self {
      gap,
      allow_rotation,
      step: DEFAULT_SEARCH_STEP,
    })
  }

  /// The same options with the coarse search step set, in millimetres.
  ///
  /// # Errors
  ///
  /// [`GeomError::SearchStepNotMeasurable`] when `step` is zero, negative or not finite, with its
  /// value: a step that cannot advance would search one anchor and claim to have searched the board.
  pub fn with_step(self, step: f64) -> Result<Self, GeomError> {
    if !step.is_finite() || step <= 0.0 {
      return Err(GeomError::SearchStepNotMeasurable { step });
    }
    Ok(Self { step, ..self })
  }

  /// The gap between two parts on a board, in millimetres.
  pub fn gap(&self) -> f64 {
    self.gap
  }

  /// True when a part may be turned a quarter turn to enter a board.
  pub fn allows_rotation(&self) -> bool {
    self.allow_rotation
  }

  /// The coarse search step, in millimetres.
  pub fn step(&self) -> f64 {
    self.step
  }
}

/// One part placed on one board.
///
/// `origin` is where the part's own bounding box's lower-left corner went, so the part keeps its
/// authored orientation (or its quarter turn) and [`ProfileNestPlan::outline`] is the placed shape
/// itself.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProfilePlacement {
  part: usize,
  sheet: usize,
  origin: Point2,
  rotation: NestRotation,
}

impl ProfilePlacement {
  /// Index of the part in the caller's list.
  pub fn part(&self) -> usize {
    self.part
  }

  /// Index of the board it sits on, from zero.
  pub fn sheet(&self) -> usize {
    self.sheet
  }

  /// Where the part's bounding box lower-left corner went, in board coordinates.
  pub fn origin(&self) -> Point2 {
    self.origin
  }

  /// The quarter turn applied to the part.
  pub fn rotation(&self) -> NestRotation {
    self.rotation
  }
}

/// The result of one free-profile nesting run: where the parts went, what did not fit, how many boards
/// it took, and which algorithm produced it.
#[derive(Clone, Debug, PartialEq)]
pub struct ProfileNestPlan {
  placements: Vec<ProfilePlacement>,
  outlines: Vec<Contour>,
  unplaced: Vec<usize>,
  sheets_used: usize,
}

impl ProfileNestPlan {
  /// Nest `parts` onto identical `sheet`s, keeping neighbours the option's gap apart.
  ///
  /// Boards are opened one at a time, only when the parts already placed leave no room, and every open
  /// board is searched upright before any part is turned.
  ///
  /// # Errors
  ///
  /// [`GeomError::SheetNotClosed`] when `sheet` is not a closed contour,
  /// [`GeomError::SheetNotMeasurable`] when it encloses no area, and [`GeomError::PartNotClosed`] when
  /// a part is not a closed contour — each naming what it refused. A part larger than the board is not
  /// an error: it is reported in [`ProfileNestPlan::unplaced`].
  pub fn new(
    sheet: &Contour,
    parts: &[Contour],
    options: ProfileNestOptions,
  ) -> Result<Self, GeomError> {
    if !sheet.is_closed() {
      return Err(GeomError::SheetNotClosed);
    }
    let sheet_bounds = sheet.bounds();
    let width = sheet_bounds.width();
    let height = sheet_bounds.height();
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
      return Err(GeomError::SheetNotMeasurable { width, height });
    }
    for (index, part) in parts.iter().enumerate() {
      if !part.is_closed() {
        return Err(GeomError::PartNotClosed { part: index });
      }
    }

    let mut packer = Packer::new(sheet, sheet_bounds, options);
    let mut placements = Vec::with_capacity(parts.len());
    let mut outlines = Vec::with_capacity(parts.len());
    let mut unplaced = Vec::new();

    // Area descending, `sort_by` is stable so equal areas keep the caller's order.
    let mut order: Vec<usize> = (0..parts.len()).collect();
    order.sort_by(|&left, &right| {
      parts[right]
        .signed_area()
        .abs()
        .total_cmp(&parts[left].signed_area().abs())
    });

    for part in order {
      let upright = oriented(&parts[part], NestRotation::None)?;
      let turned = if options.allows_rotation() {
        Some(oriented(&parts[part], NestRotation::QuarterTurn)?)
      } else {
        None
      };

      // An open board first, upright before turned: a quarter turn is a fallback, never a tie-break.
      let mut fit = packer
        .first_fit(&upright)
        .map(|(sheet, origin, outline)| (sheet, origin, outline, NestRotation::None));
      if fit.is_none() {
        if let Some(turned) = &turned {
          fit = packer
            .first_fit(turned)
            .map(|(sheet, origin, outline)| (sheet, origin, outline, NestRotation::QuarterTurn));
        }
      }
      // Only then a fresh board, and only if the part fits it: an oversized part opens nothing.
      if fit.is_none() {
        fit = packer
          .fresh_fit(&upright, NestRotation::None)
          .or_else(|| match &turned {
            Some(turned) => packer.fresh_fit(turned, NestRotation::QuarterTurn),
            None => None,
          });
      }
      let Some((sheet, origin, outline, rotation)) = fit else {
        unplaced.push(part);
        continue;
      };
      packer.admit(sheet, &outline);
      placements.push(ProfilePlacement {
        part,
        sheet,
        origin,
        rotation,
      });
      outlines.push(outline);
    }

    unplaced.sort_unstable();
    Ok(Self {
      placements,
      outlines,
      unplaced,
      sheets_used: packer.sheets.len(),
    })
  }

  /// The placements, in the order they were made.
  pub fn placements(&self) -> &[ProfilePlacement] {
    &self.placements
  }

  /// The placed outline of the placement at `index`, in board coordinates.
  ///
  /// This is the part's own contour, turned if it was turned and moved to its origin: the shape a
  /// report, a preview or a cutpath needs, without re-deriving the placement.
  pub fn outline(&self, index: usize) -> Option<&Contour> {
    self.outlines.get(index)
  }

  /// The indices of the parts that fit no board, in the caller's order.
  pub fn unplaced(&self) -> &[usize] {
    &self.unplaced
  }

  /// How many identical boards the plan needed.
  pub fn sheets_used(&self) -> usize {
    self.sheets_used
  }

  /// The algorithm that produced this plan.
  ///
  /// Always [`NO_FIT_HEURISTIC`]: naming it is how a caller reports what it got instead of guessing
  /// from which module it called.
  pub fn algorithm(&self) -> &'static str {
    NO_FIT_HEURISTIC
  }

  /// True when this plan keeps the promise nesting makes: every placement is inside the board, and
  /// every pair on one board is further apart than `gap` millimetres and does not intersect.
  ///
  /// The check is the packer's own rule, applied to the plan instead of believed: each outline must be
  /// inside the board, and each ordered pair on one board must be `Disjoint` from the other outline and
  /// from the other outline's no-fit region, exactly as [`ProfileNestPlan::new`] demanded when it
  /// accepted the placement. One arithmetic, so a plan and its certificate cannot disagree. The pairs
  /// are checked in one direction only, because the distance between two outlines is symmetric.
  ///
  /// `gap` is the separation the caller demanded, so a plan nested with one gap is not certified by
  /// asserting a smaller one. A gap that is not a distance certifies nothing.
  pub fn certifies(&self, sheet: &Contour, gap: f64) -> bool {
    if !gap.is_finite() || gap < 0.0 {
      return false;
    }
    let sheet_bounds = sheet.bounds();
    let blocked: Vec<Blocked> = self
      .outlines
      .iter()
      .map(|outline| no_fit(outline, gap))
      .collect();
    for (index, outline) in self.outlines.iter().enumerate() {
      if self.placements[index].sheet >= self.sheets_used {
        return false;
      }
      if !inside_sheet(outline, sheet, sheet_bounds) {
        return false;
      }
    }
    for (index, first) in self.outlines.iter().enumerate() {
      for (other, second) in self.outlines.iter().enumerate().skip(index + 1) {
        if self.placements[index].sheet != self.placements[other].sheet {
          continue;
        }
        if overlaps(first, second, &blocked[other]) {
          return false;
        }
      }
    }
    true
  }
}

/// What a placed part does to the anchors around it: the region a candidate may not enter.
///
/// The three cases are the three honest answers, and the third is why this is an enum rather than an
/// `Option`: "no gap to prove" and "the gap could not be grown" are different, and only the second has
/// to refuse everything that tries to join the board.
#[derive(Debug)]
enum Blocked {
  /// The part itself. A zero gap means the two outlines may not even touch, so the part is the region.
  Shape,
  /// The part grown outward by the gap: the region a candidate has to stay clear of.
  Grown(Region),
  /// The gap could not be grown, so no further part may join this board: refusal, not a guess.
  Unprovable,
}

/// One part already on a board: its outline, and the region it blocks.
#[derive(Debug)]
struct PlacedPart {
  outline: Contour,
  blocked: Blocked,
}

impl PlacedPart {
  /// Record `outline` as placed, with the gap it must keep from its neighbours.
  fn new(outline: Contour, gap: f64) -> Self {
    let blocked = no_fit(&outline, gap);
    Self { outline, blocked }
  }

  /// True when `candidate` may not be placed beside this part.
  fn blocks(&self, candidate: &Contour) -> bool {
    overlaps(candidate, &self.outline, &self.blocked)
  }
}

/// One board being packed: the parts already on it, in placement order.
#[derive(Debug, Default)]
struct Sheet {
  placed: Vec<PlacedPart>,
}

/// The packing state of one run: the board, the options that matter while packing, and the boards.
struct Packer<'a> {
  sheet: &'a Contour,
  sheet_bounds: Bounds,
  gap: f64,
  step: f64,
  sheets: Vec<Sheet>,
}

impl<'a> Packer<'a> {
  fn new(sheet: &'a Contour, sheet_bounds: Bounds, options: ProfileNestOptions) -> Self {
    Self {
      sheet,
      sheet_bounds,
      gap: options.gap(),
      step: options.step(),
      sheets: Vec::new(),
    }
  }

  /// The first open board, in order, that holds `oriented`, with the anchor and the outline it makes.
  fn first_fit(&self, oriented: &Contour) -> Option<(usize, Point2, Contour)> {
    for (index, open) in self.sheets.iter().enumerate() {
      if let Some((origin, outline)) = self.search(&open.placed, oriented) {
        return Some((index, origin, outline));
      }
    }
    None
  }

  /// A board of its own for `oriented`, opened only when it holds the part.
  fn fresh_fit(
    &mut self,
    oriented: &Contour,
    rotation: NestRotation,
  ) -> Option<(usize, Point2, Contour, NestRotation)> {
    let (origin, outline) = self.search(&[], oriented)?;
    self.sheets.push(Sheet {
      placed: vec![PlacedPart::new(outline.clone(), self.gap)],
    });
    Some((self.sheets.len() - 1, origin, outline, rotation))
  }

  /// Record `outline` on an open board, beside the parts already there.
  fn admit(&mut self, sheet: usize, outline: &Contour) {
    self.sheets[sheet]
      .placed
      .push(PlacedPart::new(outline.clone(), self.gap));
  }

  /// The first anchor of the coarse lattice at which `oriented` fits beside `placed`, and the outline
  /// it produces there.
  ///
  /// The lattice starts at the board's own lower-left corner and advances by the option's step in `y`
  /// then `x`, so the answer is the lowest, then leftmost, anchor that fits — a fixed rule, which is
  /// what makes two runs agree.
  fn search(&self, placed: &[PlacedPart], oriented: &Contour) -> Option<(Point2, Contour)> {
    let footprint = oriented.bounds();
    let spare_x = self.sheet_bounds.width() - footprint.width();
    let spare_y = self.sheet_bounds.height() - footprint.height();
    if spare_x < -TOLERANCE || spare_y < -TOLERANCE {
      // The part does not fit an empty board, whatever the anchors say.
      return None;
    }
    let columns = lattice_steps(spare_x, self.step);
    let rows = lattice_steps(spare_y, self.step);
    for row in 0..=rows {
      for column in 0..=columns {
        let origin = Point2::new(
          self.sheet_bounds.min.x + column as f64 * self.step,
          self.sheet_bounds.min.y + row as f64 * self.step,
        );
        // A translation cannot fail: it is the same geometry, moved.
        let candidate = transform::translate(oriented, Vec2::new(origin.x, origin.y)).ok()?;
        if !inside_sheet(&candidate, self.sheet, self.sheet_bounds) {
          continue;
        }
        if placed.iter().any(|part| part.blocks(&candidate)) {
          continue;
        }
        return Some((origin, candidate));
      }
    }
    None
  }
}

/// How many lattice steps fit in `spare` millimetres, never negative.
fn lattice_steps(spare: f64, step: f64) -> usize {
  (spare / step).floor().max(0.0) as usize
}

/// `part` in its placed frame: turned if a quarter turn was asked for, with its bounding box's
/// lower-left corner at the origin.
///
/// Anchoring at the bounding box is the rectangle nester's convention kept for contours, and turning
/// about the part's own lower-left corner before re-anchoring keeps the placed footprint's corner at
/// the origin rather than wherever the rotation happened to leave it.
fn oriented(part: &Contour, rotation: NestRotation) -> Result<Contour, GeomError> {
  let turned = match rotation {
    NestRotation::None => part.clone(),
    NestRotation::QuarterTurn => {
      transform::rotate(part, std::f64::consts::FRAC_PI_2, part.bounds().min)?
    }
  };
  let bounds = turned.bounds();
  transform::translate(&turned, Vec2::new(-bounds.min.x, -bounds.min.y))
}

/// True when `candidate` lies inside `sheet`, its own edges included.
///
/// The bounding box decides first — it is the cheap half, and it also covers an arc bulging past the
/// convex hull of its vertices — and the kernel's relation decides the rest: a part inside the board is
/// `FirstInside`, one flush against an edge is `Touching`, and one poking through a concave board edge
/// is `Crossing`, which is not inside at all.
fn inside_sheet(candidate: &Contour, sheet: &Contour, sheet_bounds: Bounds) -> bool {
  let bounds = candidate.bounds();
  if bounds.min.x < sheet_bounds.min.x - TOLERANCE
    || bounds.min.y < sheet_bounds.min.y - TOLERANCE
    || bounds.max.x > sheet_bounds.max.x + TOLERANCE
    || bounds.max.y > sheet_bounds.max.y + TOLERANCE
  {
    return false;
  }
  matches!(
    relation(candidate, sheet, TOLERANCE),
    Relation::FirstInside | Relation::Touching
  )
}

/// True when `candidate` may not be placed beside a part with this `outline` and blocked region.
///
/// The two halves are the module's soundness, in order: disjoint from the part itself (no overlap,
/// decided between the parts and not by the offset kernel), then disjoint from the no-fit region (the
/// gap, decided by the offset kernel). Strict disjointness on both, for the reason the module
/// documentation gives: a candidate that touches a grown boundary may be inside it.
fn overlaps(candidate: &Contour, placed: &Contour, blocked: &Blocked) -> bool {
  if relation(candidate, placed, TOLERANCE) != Relation::Disjoint {
    return true;
  }
  match blocked {
    Blocked::Shape => false,
    Blocked::Unprovable => true,
    Blocked::Grown(region) => region
      .boundaries
      .iter()
      .any(|boundary| relation(candidate, boundary, TOLERANCE) != Relation::Disjoint),
  }
}

/// The region `outline` blocks for its neighbours: the outline grown outward by `gap`.
///
/// A negative distance grows material, which is the sign A6 defines, so no union and no new intersector
/// is needed to say "one gap away". A zero gap blocks the outline itself — see the module
/// documentation on why a zero gap may not even touch. A growth the kernel refuses is
/// [`Blocked::Unprovable`], which closes the board to further parts instead of silently dropping the
/// gap the caller authored.
fn no_fit(outline: &Contour, gap: f64) -> Blocked {
  if gap <= 0.0 {
    return Blocked::Shape;
  }
  let Ok(region) = Region::new(vec![outline.clone()]) else {
    return Blocked::Unprovable;
  };
  match region.offset(-gap, TOLERANCE) {
    Ok(offset) => Blocked::Grown(offset.region),
    Err(_) => Blocked::Unprovable,
  }
}
