//! Rectangle nesting: sheets, parts, and the placement certificate (M9 T2; S5–S7).
//!
//! S7 asks for rectangles first because they cover cabinetry parts, and S6 names the two heuristics
//! that do it: **maxrects** and a **guillotine** pass. This module holds plain geometry only — the
//! sheet is a [`Bounds`], every part is its own [`Bounds`], and nothing here knows about documents,
//! tools or sheets as authored records. `cam-model` and `cam-cli` map their own types onto these, so
//! the nester can be driven by a test, a command or a report without any of them re-implementing it.
//!
//! # What a plan guarantees
//!
//! [`NestPlan`] answers three questions: where each part went ([`NestPlan::placements`]), which parts
//! did not fit ([`NestPlan::unplaced`]) and how many sheets that took ([`NestPlan::sheets_used`]) —
//! and names the algorithm that produced the answer ([`NestPlan::algorithm`]).
//! The guarantee that ties them together is [`NestPlan::certifies`], one predicate stating that
//!
//! * every placement lies **inside its sheet**, the sheet's own edges included, and
//! * every pair of placements **on one sheet** is at least the authored gap apart,
//!
//! which is also the statement that no two placements overlap. It is exposed as a predicate for the
//! reason [`StepoverLaw::covers`](crate::moulding::StepoverLaw::covers) is: a caller that needs the
//! guarantee asserts the sentence instead of restating the arithmetic, so there is one arithmetic and
//! not two that can disagree. `certifies` is a *check*, not a precondition — the nester builds plans
//! that satisfy it by construction, and a caller asserts it to prove the plan it was handed did.
//!
//! # The gap
//!
//! The gap is the minimum separation between two parts on a sheet. It is imposed by packing each part
//! **grown by the gap on both axes** into a sheet grown by the same gap: two grown rectangles that do
//! not overlap are, by construction, at least one gap apart in the parts they stand for, and a grown
//! part that fits the grown sheet fits the real sheet. The sheet's own edges are **not** inset — a
//! part may sit flush against them — because the gap is the room a cut needs *between* parts, and
//! whether the board's edge leaves room for the cutter is the author's business, not the nester's.
//! A part exactly the size of the sheet therefore still fits, flush on all four edges.
//!
//! # Rotation
//!
//! A quarter turn is offered for a part that has **no** upright placement left on the sheet being
//! tried — never as a tie-break, and never to make a fit look tidier. So a part that fits upright is
//! never turned, and a turned part is turned because that is the only way it enters the sheet. A
//! square part is never turned at all: the turn is the same rectangle.
//!
//! # Two heuristics, one answer, reachable by name
//!
//! Both passes run on the same sorted part list, and which of them packs is a setting on
//! [`NestOptions`] ([`NestAlgorithm`]): **`Auto`**, the default, runs both and keeps the better plan;
//! `MaxRects` and `Guillotine` run one and return its plan. The plan names the algorithm that
//! produced it ([`NestPlan::algorithm`]), so a caller never has to infer which heuristic it is
//! looking at.
//!
//! The auto rule prefers **fewer unplaced parts**, then **fewer sheets**. When they tie — the common
//! case — it keeps the maxrects plan, because maxrects is the primary heuristic S6 names and a fixed
//! preference is what keeps the answer reproducible. Neither rule is decoration: the fixtures in
//! `tests/nest.rs` pin both plans by name, and one fixture is pinned where guillotine needs one
//! board where maxrects needs two.
//!
//! * Maxrects keeps a list of maximal free rectangles, splits every free rectangle a placement
//!   touches into the strips around it, and prunes the ones another contains. Because those
//!   rectangles overlap, a split can leave two adjacent strips where a single wider one would do.
//! * The guillotine pass keeps a list of free rectangles that never overlap: it packs into one of
//!   them and splits the remainder with a single cut that runs the **full length** of the rectangle
//!   being split, along the axis with more room left, then splits the remainders the same way as
//!   later parts land in them. That is the cut a panel saw can actually make, and it is why
//!   guillotine can win: the strip it leaves is full-width where maxrects leaves two part-width
//!   ones.
//!
//! # Determinism
//!
//! Parts are sorted by area descending with a **stable** sort, so equal areas keep the caller's input
//! order. Free rectangles are a `Vec`, never a hash map, and every tie between two candidate
//! placements is broken by the ordered score below (`short side fit`, `long side fit`, then the
//! lower `y`, then the lower `x`) and then by the free rectangle's index. Two runs over the same input
//! therefore return the same placements **in the same order**.

use std::cmp::Ordering;

use crate::point::{Bounds, Point2, Vec2};
use crate::GeomError;

/// Tolerance for comparing millimetre positions and sizes, in millimetres.
///
/// One nanometre: far below any machine resolution, and comfortably above the noise of adding and
/// subtracting millimetre lengths at board scale.
const EPS: f64 = 1.0e-9;

/// The options a nesting run is driven by.
///
/// The numbers are decisions the caller authors — the gap between parts, and whether a part may be
/// turned a quarter turn — so they arrive together rather than as two positional arguments that a
/// call site can swap. [`NestOptions::new`] sets the algorithm to [`NestAlgorithm::Auto`], the rule
/// this module has always applied; [`NestOptions::with_algorithm`] asks for one by name.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NestOptions {
  gap: f64,
  allow_rotation: bool,
  algorithm: NestAlgorithm,
}

impl NestOptions {
  /// Build the options for a run: `gap` millimetres between parts, and whether a quarter turn is
  /// allowed.
  ///
  /// The algorithm starts as [`NestAlgorithm::Auto`].
  ///
  /// # Errors
  ///
  /// [`GeomError::GapNotMeasurable`] when `gap` is negative or not finite, with its value: a gap
  /// that is not a distance cannot separate anything, and it is refused rather than replaced by zero.
  pub fn new(gap: f64, allow_rotation: bool) -> Result<Self, GeomError> {
    if !gap.is_finite() || gap < 0.0 {
      return Err(GeomError::GapNotMeasurable { gap });
    }
    Ok(Self {
      gap,
      allow_rotation,
      algorithm: NestAlgorithm::Auto,
    })
  }

  /// The same options with the algorithm set: one heuristic by name, or the auto rule.
  pub fn with_algorithm(self, algorithm: NestAlgorithm) -> Self {
    Self { algorithm, ..self }
  }

  /// The gap between two parts on a sheet, in millimetres.
  pub fn gap(&self) -> f64 {
    self.gap
  }

  /// True when a part may be turned a quarter turn to enter a sheet.
  pub fn allows_rotation(&self) -> bool {
    self.allow_rotation
  }

  /// The algorithm the run is asked for.
  pub fn algorithm(&self) -> NestAlgorithm {
    self.algorithm
  }
}

/// Which of the two heuristics S6 names a nesting run uses.
///
/// [`NestAlgorithm::Auto`] is the default and the behaviour every run had before the setting
/// existed: run both, keep the better plan, and keep maxrects on a tie.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum NestAlgorithm {
  /// Run both heuristics and keep the better plan: fewer unplaced parts, then fewer sheets, then
  /// maxrects.
  #[default]
  Auto,
  /// Maximal free rectangles, split around every placement and pruned of the ones another contains.
  MaxRects,
  /// Non-overlapping free rectangles, split by one cut along the axis with more room left.
  Guillotine,
}

/// The quarter turn a part was placed with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NestRotation {
  /// The part is placed as authored.
  None,
  /// The part is turned 90 degrees, so its footprint is its authored size with the axes swapped.
  QuarterTurn,
}

impl NestRotation {
  /// True when the part was turned.
  pub fn is_turned(self) -> bool {
    matches!(self, Self::QuarterTurn)
  }

  /// The footprint side lengths this rotation gives a part authored at `size`, in millimetres.
  pub fn footprint_size(self, size: Vec2) -> Vec2 {
    match self {
      Self::None => size,
      Self::QuarterTurn => Vec2::new(size.y, size.x),
    }
  }
}

/// One part placed on one sheet.
///
/// `origin` is where the part's own lower-left corner went: the part keeps its authored orientation
/// (or its quarter turn), so its footprint is `origin` grown by [`NestPlacement::footprint_size`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NestPlacement {
  part: usize,
  sheet: usize,
  origin: Point2,
  rotation: NestRotation,
  size: Vec2,
}

impl NestPlacement {
  /// Index of the part in the caller's list.
  pub fn part(&self) -> usize {
    self.part
  }

  /// Index of the sheet it sits on, from zero.
  pub fn sheet(&self) -> usize {
    self.sheet
  }

  /// Where the part's lower-left corner went, in sheet coordinates.
  pub fn origin(&self) -> Point2 {
    self.origin
  }

  /// The quarter turn applied to the part.
  pub fn rotation(&self) -> NestRotation {
    self.rotation
  }

  /// The part's side lengths as placed, in millimetres, after any quarter turn.
  pub fn footprint_size(&self) -> Vec2 {
    self.size
  }

  /// The area the part occupies on its sheet, as a box in sheet coordinates.
  pub fn footprint(&self) -> Bounds {
    Bounds {
      min: self.origin,
      max: Point2::new(self.origin.x + self.size.x, self.origin.y + self.size.y),
    }
  }
}

/// The result of one nesting run: where the parts went, what did not fit, how many sheets it took,
/// and which algorithm produced it.
#[derive(Clone, Debug, PartialEq)]
pub struct NestPlan {
  placements: Vec<NestPlacement>,
  unplaced: Vec<usize>,
  sheets_used: usize,
  algorithm: NestAlgorithm,
}

impl NestPlan {
  /// Nest `parts` onto identical `sheet`s, separating neighbours by the option's gap.
  ///
  /// The option's [`NestAlgorithm`] decides which heuristic packs: `Auto` runs both and keeps the
  /// better plan, `MaxRects` and `Guillotine` run one. Sheets are opened one at a time, only when
  /// the parts already placed no longer leave room.
  ///
  /// # Errors
  ///
  /// [`GeomError::SheetNotMeasurable`] when the sheet has no positive, finite side lengths, and
  /// [`GeomError::PartNotMeasurable`] when a part has a negative or non-finite one — each with the
  /// value it refused. A part larger than the sheet is not an error: it is reported in
  /// [`NestPlan::unplaced`].
  pub fn new(sheet: Bounds, parts: &[Bounds], options: NestOptions) -> Result<Self, GeomError> {
    let width = sheet.width();
    let height = sheet.height();
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
      return Err(GeomError::SheetNotMeasurable { width, height });
    }
    let mut sizes = Vec::with_capacity(parts.len());
    for (index, part) in parts.iter().enumerate() {
      let width = part.width();
      let height = part.height();
      if !width.is_finite() || !height.is_finite() || width < 0.0 || height < 0.0 {
        return Err(GeomError::PartNotMeasurable {
          part: index,
          width,
          height,
        });
      }
      sizes.push(Vec2::new(width, height));
    }

    let (packing, algorithm) = match options.algorithm() {
      NestAlgorithm::MaxRects => (
        pack(sheet, &sizes, options, Strategy::MaxRects),
        NestAlgorithm::MaxRects,
      ),
      NestAlgorithm::Guillotine => (
        pack(sheet, &sizes, options, Strategy::Guillotine),
        NestAlgorithm::Guillotine,
      ),
      NestAlgorithm::Auto => {
        let maxrects = pack(sheet, &sizes, options, Strategy::MaxRects);
        let guillotine = pack(sheet, &sizes, options, Strategy::Guillotine);
        // Fewer unplaced parts first, then fewer sheets; a tie keeps maxrects, the primary
        // heuristic.
        if rank(&guillotine) < rank(&maxrects) {
          (guillotine, NestAlgorithm::Guillotine)
        } else {
          (maxrects, NestAlgorithm::MaxRects)
        }
      }
    };
    Ok(Self {
      placements: packing.placements,
      unplaced: packing.unplaced,
      sheets_used: packing.sheets,
      algorithm,
    })
  }

  /// The placements, in the order they were made.
  pub fn placements(&self) -> &[NestPlacement] {
    &self.placements
  }

  /// The indices of the parts that fit no sheet, in the caller's order.
  pub fn unplaced(&self) -> &[usize] {
    &self.unplaced
  }

  /// How many identical sheets the plan needed.
  pub fn sheets_used(&self) -> usize {
    self.sheets_used
  }

  /// The algorithm that produced this plan: the one asked for, or the one the auto rule picked.
  ///
  /// Never [`NestAlgorithm::Auto`] — the plan is the work of one heuristic, and naming it is how a
  /// caller reports what it got instead of guessing.
  pub fn algorithm(&self) -> NestAlgorithm {
    self.algorithm
  }

  /// The area the placed parts occupy, in square millimetres.
  pub fn placed_area(&self) -> f64 {
    self
      .placements
      .iter()
      .map(|placement| placement.size.x * placement.size.y)
      .sum()
  }

  /// The fraction of the sheets the plan used that carries parts, between 0 and 1.
  ///
  /// The one place utilisation is computed: a report and a stored cost read this instead of each
  /// dividing the same two numbers themselves (M9 invariant 2). A plan that used no sheet uses
  /// nothing, so it reports zero.
  pub fn utilisation(&self, sheet: Bounds) -> f64 {
    let sheet_area = sheet.width() * sheet.height();
    if self.sheets_used == 0 || sheet_area <= 0.0 || !sheet_area.is_finite() {
      return 0.0;
    }
    self.placed_area() / (sheet_area * self.sheets_used as f64)
  }

  /// True when this plan keeps the promise nesting makes: every placement is inside its sheet and
  /// every pair on one sheet is at least `gap` millimetres apart.
  ///
  /// `gap` is the separation the caller demanded, so a plan nested with one gap is not certified by
  /// asserting a smaller one. Two footprints satisfy the promise when the measured clearance between
  /// them — the larger of the two axis gaps — is at least `gap`; touching footprints are exactly at
  /// zero, and overlapping ones measure negative.
  pub fn certifies(&self, sheet: Bounds, gap: f64) -> bool {
    if !gap.is_finite() || gap < 0.0 {
      return false;
    }
    for placement in &self.placements {
      if placement.sheet >= self.sheets_used {
        return false;
      }
      let footprint = placement.footprint();
      if footprint.min.x < sheet.min.x - EPS
        || footprint.min.y < sheet.min.y - EPS
        || footprint.max.x > sheet.max.x + EPS
        || footprint.max.y > sheet.max.y + EPS
      {
        return false;
      }
    }
    for (index, first) in self.placements.iter().enumerate() {
      for second in &self.placements[index + 1..] {
        if first.sheet != second.sheet {
          continue;
        }
        if clearance(first.footprint(), second.footprint()) < gap - EPS {
          return false;
        }
      }
    }
    true
  }
}

/// The larger of the two axis gaps between two boxes: positive when they are apart, zero when they
/// touch, negative when they overlap.
fn clearance(a: Bounds, b: Bounds) -> f64 {
  let dx = (b.min.x - a.max.x).max(a.min.x - b.max.x);
  let dy = (b.min.y - a.max.y).max(a.min.y - b.max.y);
  dx.max(dy)
}

/// Which of the two heuristics S6 names is packing.
///
/// The private half of [`NestAlgorithm`]: [`NestAlgorithm::Auto`] resolves to one of these before
/// any packing starts, so `pack` never has to decide what the auto rule means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Strategy {
  /// Maximal free rectangles, split around every placement and pruned of the ones another contains.
  MaxRects,
  /// Non-overlapping free rectangles, split by one full-length cut along the axis with more room
  /// left, then split again as later parts land in the remainders.
  Guillotine,
}

/// A free rectangle of a sheet, in sheet-local millimetres from the sheet's own lower-left corner.
#[derive(Clone, Copy, Debug, PartialEq)]
struct FreeRect {
  x: f64,
  y: f64,
  w: f64,
  h: f64,
}

impl FreeRect {
  /// True when the two share a positive area. Touching edges do not overlap.
  fn overlaps(self, other: Self) -> bool {
    self.x + EPS < other.x + other.w
      && other.x + EPS < self.x + self.w
      && self.y + EPS < other.y + other.h
      && other.y + EPS < self.y + self.h
  }

  /// True when `other` is inside this rectangle, edges included.
  fn contains_rect(self, other: Self) -> bool {
    other.x >= self.x - EPS
      && other.y >= self.y - EPS
      && other.x + other.w <= self.x + self.w + EPS
      && other.y + other.h <= self.y + self.h + EPS
  }
}

/// A candidate placement inside one free rectangle.
#[derive(Clone, Copy, Debug)]
struct Fit {
  /// Index of the free rectangle it uses, which the guillotine split needs.
  free: usize,
  /// Sheet-local lower-left corner.
  x: f64,
  y: f64,
  /// Grown footprint side lengths, gap included.
  size: Vec2,
  rotation: NestRotation,
  /// Ordered placement score: lower is better on every component.
  score: [f64; 4],
}

/// Everything one heuristic produced before the better of the two is chosen.
#[derive(Clone, Debug)]
struct Packing {
  placements: Vec<NestPlacement>,
  unplaced: Vec<usize>,
  sheets: usize,
}

/// The ordering the better plan wins by: fewer unplaced parts, then fewer sheets.
fn rank(packing: &Packing) -> (usize, usize) {
  (packing.unplaced.len(), packing.sheets)
}

/// Run one heuristic over `sizes` and return what it placed.
fn pack(sheet: Bounds, sizes: &[Vec2], options: NestOptions, strategy: Strategy) -> Packing {
  let gap = options.gap();
  let mut open: Vec<Vec<FreeRect>> = Vec::new();
  let mut placements = Vec::with_capacity(sizes.len());
  let mut unplaced = Vec::new();

  // Area descending, `sort_by` is stable so equal areas keep the caller's order.
  let mut order: Vec<usize> = (0..sizes.len()).collect();
  order.sort_by(|&a, &b| {
    let left = sizes[a].x * sizes[a].y;
    let right = sizes[b].x * sizes[b].y;
    right.total_cmp(&left)
  });

  for part in order {
    let authored = sizes[part];
    // Growing the part and the sheet by the same gap is what makes the gap a distance: see the
    // module documentation.
    let grown = Vec2::new(authored.x + gap, authored.y + gap);
    let mut placed = false;
    for (index, free) in open.iter_mut().enumerate() {
      if let Some(fit) = find_fit(free, grown, options.allows_rotation()) {
        commit(free, strategy, fit);
        placements.push(placement(part, index, fit, authored, sheet));
        placed = true;
        break;
      }
    }
    if placed {
      continue;
    }
    // No room on an open sheet: try a fresh one, and only open it if the part fits it.
    let mut free = vec![FreeRect {
      x: 0.0,
      y: 0.0,
      w: sheet.width() + gap,
      h: sheet.height() + gap,
    }];
    if let Some(fit) = find_fit(&free, grown, options.allows_rotation()) {
      commit(&mut free, strategy, fit);
      let index = open.len();
      open.push(free);
      placements.push(placement(part, index, fit, authored, sheet));
    } else {
      unplaced.push(part);
    }
  }

  unplaced.sort_unstable();
  Packing {
    placements,
    unplaced,
    sheets: open.len(),
  }
}

/// Turn a fit into the placement the caller reads, in sheet coordinates.
fn placement(part: usize, sheet: usize, fit: Fit, authored: Vec2, board: Bounds) -> NestPlacement {
  NestPlacement {
    part,
    sheet,
    origin: Point2::new(board.min.x + fit.x, board.min.y + fit.y),
    rotation: fit.rotation,
    size: fit.rotation.footprint_size(authored),
  }
}

/// The best placement for `size` in `free`, upright first and turned only when upright finds none.
fn find_fit(free: &[FreeRect], size: Vec2, allow_rotation: bool) -> Option<Fit> {
  if let Some(fit) = best_fit(free, size, NestRotation::None) {
    return Some(fit);
  }
  // A square turned is the same rectangle, so it is never turned.
  if !allow_rotation || size.x == size.y {
    return None;
  }
  best_fit(free, Vec2::new(size.y, size.x), NestRotation::QuarterTurn)
}

/// The best of the free rectangles that hold `size` in `rotation`.
///
/// The score is the two fit lengths (short side first, as maxrects' best-short-side-fit does) and then
/// the lower corner, so the first candidate wins ties and the answer does not depend on iteration
/// order beyond this.
fn best_fit(free: &[FreeRect], size: Vec2, rotation: NestRotation) -> Option<Fit> {
  let mut best: Option<Fit> = None;
  for (index, rect) in free.iter().enumerate() {
    if size.x > rect.w + EPS || size.y > rect.h + EPS {
      continue;
    }
    let spare_x = rect.w - size.x;
    let spare_y = rect.h - size.y;
    let score = [spare_x.min(spare_y), spare_x.max(spare_y), rect.y, rect.x];
    if best.is_some_and(|current| !score_less(score, current.score)) {
      continue;
    }
    best = Some(Fit {
      free: index,
      x: rect.x,
      y: rect.y,
      size,
      rotation,
      score,
    });
  }
  best
}

/// True when `a` orders before `b` component by component.
fn score_less(a: [f64; 4], b: [f64; 4]) -> bool {
  for (left, right) in a.iter().zip(b.iter()) {
    match left.total_cmp(right) {
      Ordering::Less => return true,
      Ordering::Greater => return false,
      Ordering::Equal => {}
    }
  }
  false
}

/// Record `fit` on `free` for the given strategy.
fn commit(free: &mut Vec<FreeRect>, strategy: Strategy, fit: Fit) {
  let filled = FreeRect {
    x: fit.x,
    y: fit.y,
    w: fit.size.x,
    h: fit.size.y,
  };
  match strategy {
    Strategy::MaxRects => {
      let mut next = Vec::with_capacity(free.len() + 4);
      for rect in free.iter().copied() {
        split_around(rect, filled, &mut next);
      }
      *free = prune(next);
    }
    Strategy::Guillotine => {
      let rect = free.remove(fit.free);
      let spare_x = rect.w - filled.w;
      let spare_y = rect.h - filled.h;
      let (first, second) = if spare_x > spare_y {
        (
          FreeRect {
            x: filled.x + filled.w,
            y: rect.y,
            w: spare_x,
            h: rect.h,
          },
          FreeRect {
            x: rect.x,
            y: filled.y + filled.h,
            w: filled.w,
            h: spare_y,
          },
        )
      } else {
        (
          FreeRect {
            x: rect.x,
            y: filled.y + filled.h,
            w: rect.w,
            h: spare_y,
          },
          FreeRect {
            x: filled.x + filled.w,
            y: rect.y,
            w: spare_x,
            h: filled.h,
          },
        )
      };
      for remainder in [first, second] {
        push_rect(free, remainder);
      }
    }
  }
}

/// The part of `rect` that `filled` leaves, as up to four strips around it.
fn split_around(rect: FreeRect, filled: FreeRect, out: &mut Vec<FreeRect>) {
  if !rect.overlaps(filled) {
    out.push(rect);
    return;
  }
  if filled.x > rect.x + EPS {
    push_rect(
      out,
      FreeRect {
        x: rect.x,
        y: rect.y,
        w: filled.x - rect.x,
        h: rect.h,
      },
    );
  }
  if filled.x + filled.w < rect.x + rect.w - EPS {
    push_rect(
      out,
      FreeRect {
        x: filled.x + filled.w,
        y: rect.y,
        w: rect.x + rect.w - (filled.x + filled.w),
        h: rect.h,
      },
    );
  }
  if filled.y > rect.y + EPS {
    push_rect(
      out,
      FreeRect {
        x: rect.x,
        y: rect.y,
        w: rect.w,
        h: filled.y - rect.y,
      },
    );
  }
  if filled.y + filled.h < rect.y + rect.h - EPS {
    push_rect(
      out,
      FreeRect {
        x: rect.x,
        y: filled.y + filled.h,
        w: rect.w,
        h: rect.y + rect.h - (filled.y + filled.h),
      },
    );
  }
}

/// Keep `rect` only when it has room for something, so slivers do not accumulate.
fn push_rect(out: &mut Vec<FreeRect>, rect: FreeRect) {
  if rect.w > EPS && rect.h > EPS {
    out.push(rect);
  }
}

/// Drop every free rectangle another one already contains, keeping the earlier of two equals.
fn prune(rects: Vec<FreeRect>) -> Vec<FreeRect> {
  let mut kept: Vec<FreeRect> = Vec::with_capacity(rects.len());
  for (index, rect) in rects.iter().copied().enumerate() {
    if rects[..index].iter().any(|other| other.contains_rect(rect)) {
      continue;
    }
    let swallowed = rects[index + 1..]
      .iter()
      .any(|other| other.contains_rect(rect) && !rect.contains_rect(*other));
    if !swallowed {
      kept.push(rect);
    }
  }
  kept
}
