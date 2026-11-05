//! Behaviour tests for free-profile nesting (M9 T3; S6, S7).
//!
//! Every expectation here is written from the geometry rather than read back from the module under
//! test: a gap is a **measured distance** between two placed outlines, a part is on the board when the
//! kernel's membership says it is, and two parts do not overlap when [`relation`] puts them
//! `Disjoint`. The plan's own `certifies` predicate is asserted **alongside** those measurements,
//! never instead of them, so the sentence a report will rely on is the one this file proves.
//!
//! The one thing no measurement can conjure is the placement the heuristic misses, so the
//! incompleteness fixture builds that legal placement itself, checks it against the same measurements,
//! and then asserts the plan did not find it.

use cam_geom::nest::NestRotation;
use cam_geom::nest_profile::{ProfileNestOptions, ProfileNestPlan, MAX_SEARCH_ANCHORS};
use cam_geom::primitive;
use cam_geom::transform;
use cam_geom::winding::{relation, Relation};
use cam_geom::{Bounds, Contour, GeomError, Point2, Vec2};

/// Tolerance for comparing millimetre coordinates: well below any machine resolution, and far above
/// `f64` noise at board scale.
const EPS: f64 = 1.0e-9;

/// The kernel tolerance the nesting decisions are taken at, restated here so a test asks the kernel
/// the same question the nester asked instead of a looser one of its own.
const TOL: f64 = cam_geom::DEFAULT_TOLERANCE;

/// The gap the fixtures author between neighbouring parts, in millimetres.
const GAP: f64 = 10.0;

/// The coarse search step the fixtures author, in millimetres.
const STEP: f64 = 10.0;

/// The step by which the module insets a part that touches the board before it asks whether the
/// material is on the part's own side of the boundary, in millimetres.
///
/// Restated here for the reason [`TOL`] is: a test that pushes a part past the board's edge has to
/// push it by the same step the rule measures, and the module keeps that step private. If the
/// module's step moves, these boundary cases move with it and say so.
const INSET_STEP: f64 = 0.01;

/// A rectangular board with its lower-left corner at the origin.
fn board(width: f64, height: f64) -> Contour {
  primitive::rectangle(Point2::new(0.0, 0.0), width, height).expect("a board")
}

/// A rectangular part with its lower-left corner at the origin.
fn rect(width: f64, height: f64) -> Contour {
  primitive::rectangle(Point2::new(0.0, 0.0), width, height).expect("a part")
}

/// A circular part whose bounding box's lower-left corner is at the origin.
fn disc(radius: f64) -> Contour {
  primitive::circle(Point2::new(radius, radius), radius).expect("a disc")
}

/// An L: a 100 x 100 outline with its 60 x 60 top-right corner removed.
///
/// A 100 x 40 arm along the bottom and a 40 x 100 arm up the left, so the notch it leaves is the
/// 60 x 60 square from (40, 40) to (100, 100). It covers 6400 mm², less than its 10000 mm² bounding
/// box, which is exactly what makes it the concave fixture.
fn el() -> Contour {
  Contour::from_points(
    &[
      Point2::new(0.0, 0.0),
      Point2::new(100.0, 0.0),
      Point2::new(100.0, 40.0),
      Point2::new(40.0, 40.0),
      Point2::new(40.0, 100.0),
      Point2::new(0.0, 100.0),
    ],
    true,
  )
  .expect("an L")
}

/// The options a fixture authors: the gap, whether a quarter turn is allowed, and the search step.
fn options(gap: f64, rotation: bool, step: f64) -> ProfileNestOptions {
  ProfileNestOptions::new(gap, rotation)
    .expect("a valid gap")
    .with_step(step)
    .expect("a valid step")
}

/// Nest `parts` onto the identical `board` with the fixture's gap, rotation and step.
fn plan(
  board: &Contour,
  parts: &[Contour],
  gap: f64,
  rotation: bool,
  step: f64,
) -> ProfileNestPlan {
  ProfileNestPlan::new(board, parts, options(gap, rotation, step)).expect("a plan")
}

/// The part indices a plan places, in placement order.
fn placed_parts(plan: &ProfileNestPlan) -> Vec<usize> {
  plan.placements().iter().map(|p| p.part()).collect()
}

/// Every placement, as `(part, sheet, origin, rotation)`.
fn placements(plan: &ProfileNestPlan) -> Vec<(usize, usize, Point2, NestRotation)> {
  plan
    .placements()
    .iter()
    .map(|placement| {
      (
        placement.part(),
        placement.sheet(),
        placement.origin(),
        placement.rotation(),
      )
    })
    .collect()
}

/// The placed outline of the placement at `index`, in board coordinates.
fn outline(plan: &ProfileNestPlan, index: usize) -> &Contour {
  plan.outline(index).expect("a placed outline")
}

/// The rectilinear clearance between two boxes: positive apart, zero touching, negative overlapping.
fn clearance(a: Bounds, b: Bounds) -> f64 {
  let dx = (b.min.x - a.max.x).max(a.min.x - b.max.x);
  let dy = (b.min.y - a.max.y).max(a.min.y - b.max.y);
  dx.max(dy)
}

/// True when the kernel puts two outlines in `Relation::Disjoint`: apart, neither inside the other.
fn disjoint(a: &Contour, b: &Contour) -> bool {
  relation(a, b, TOL) == Relation::Disjoint
}

/// Every pair of placements on the same board is disjoint, and its measured clearance honours the gap
/// wherever the two outlines are separated by their boxes.
fn assert_no_overlap(plan: &ProfileNestPlan, gap: f64) {
  for (index, first) in plan.placements().iter().enumerate() {
    for (other, second) in plan.placements().iter().enumerate().skip(index + 1) {
      if first.sheet() != second.sheet() {
        continue;
      }
      let (left, right) = (outline(plan, index), outline(plan, other));
      assert!(
        disjoint(left, right),
        "parts {} and {} are not disjoint on sheet {}",
        first.part(),
        second.part(),
        first.sheet()
      );
      let measured = clearance(left.bounds(), right.bounds());
      if measured > 0.0 {
        assert!(
          measured >= gap - EPS,
          "parts {} and {} are {measured} mm apart, the authored gap is {gap} mm",
          first.part(),
          second.part()
        );
      }
    }
  }
}

#[test]
fn a_circle_and_a_rectangle_pack_on_one_sheet_and_the_certificate_holds() {
  let board = board(400.0, 300.0);
  let parts = [rect(200.0, 100.0), disc(50.0)];
  let plan = plan(&board, &parts, GAP, false, STEP);

  assert_eq!(plan.sheets_used(), 1, "the two parts fit one board");
  assert_eq!(plan.placements().len(), 2, "every part is placed");
  assert_eq!(plan.unplaced(), &[] as &[usize], "nothing is left over");
  assert_eq!(
    plan.algorithm(),
    "free-profile no-fit heuristic",
    "the plan names the algorithm that produced it"
  );
  assert_eq!(
    placed_parts(&plan),
    vec![0, 1],
    "the larger part is placed first, then the circle"
  );
  assert_eq!(
    placements(&plan),
    vec![
      (0, 0, Point2::new(0.0, 0.0), NestRotation::None),
      (1, 0, Point2::new(220.0, 0.0), NestRotation::None),
    ],
    "the rectangle takes the board's corner and the circle the first coarse step clear of it"
  );

  // Measured here, from the placed outlines: the circle's leftmost point against the rectangle's right
  // edge. 220 - 200 = 20 = the 10 mm gap plus the 10 mm search step the heuristic pays to stay
  // strictly clear of the no-fit region.
  let measured = outline(&plan, 1).bounds().min.x - outline(&plan, 0).bounds().max.x;
  assert!(
    (measured - 20.0).abs() <= EPS,
    "the neighbours measure {measured} mm, expected 20 mm"
  );
  assert!(
    outline(&plan, 1).bounds().max.x <= board.bounds().max.x + EPS,
    "the circle is inside the board"
  );

  assert_no_overlap(&plan, GAP);
  assert!(
    plan.certifies(&board, GAP),
    "the plan's own certificate holds on the circle and rectangle fixture"
  );
}

#[test]
fn two_contours_that_would_overlap_are_never_placed_overlapping() {
  // Three parts whose bounding boxes crowd a 400 x 300 board: two discs and a bar. Every pair is
  // checked with the kernel's relation and with a measured clearance, not with a flag the nester set.
  let board = board(400.0, 300.0);
  let parts = [rect(200.0, 100.0), disc(60.0), disc(60.0)];
  let plan = plan(&board, &parts, GAP, false, STEP);

  assert_eq!(
    plan.placements().len(),
    3,
    "all three parts are placed: {:?}",
    plan
  );
  assert_eq!(
    placements(&plan),
    vec![
      (0, 0, Point2::new(0.0, 0.0), NestRotation::None),
      (1, 0, Point2::new(220.0, 0.0), NestRotation::None),
      (2, 0, Point2::new(0.0, 120.0), NestRotation::None),
    ],
    "the bar takes the corner, the first disc the step clear of it, and the second disc above both"
  );

  // The measured clearances, in millimetres, are read off the placed outlines.
  let bar = outline(&plan, 0).bounds();
  let first = outline(&plan, 1).bounds();
  let second = outline(&plan, 2).bounds();
  assert!(
    (clearance(bar, first) - 20.0).abs() <= EPS,
    "the bar and the first disc measure {} mm",
    clearance(bar, first)
  );
  assert!(
    (clearance(bar, second) - 20.0).abs() <= EPS,
    "the bar and the second disc measure {} mm",
    clearance(bar, second)
  );
  assert!(
    clearance(first, second) > GAP,
    "the two discs are further apart than the gap, measured {} mm",
    clearance(first, second)
  );

  assert_no_overlap(&plan, GAP);
  assert!(plan.certifies(&board, GAP));
}

#[test]
fn the_authored_gap_is_measured_between_two_placed_contours() {
  let board = board(400.0, 300.0);
  let parts = [rect(200.0, 100.0), disc(50.0)];

  // With no gap authored the circle sits at the first coarse step that leaves the rectangle alone.
  let touching = plan(&board, &parts, 0.0, false, STEP);
  assert_eq!(touching.placements().len(), 2, "both parts are placed");
  let measured = outline(&touching, 1).bounds().min.x - outline(&touching, 0).bounds().max.x;
  assert!(
    (measured - 10.0).abs() <= EPS,
    "a zero gap puts the circle one 10 mm search step from the bar, measured {measured} mm"
  );

  // With 10 mm authored the same pair is 10 mm further apart. The number moved; it is not a flag.
  let spaced = plan(&board, &parts, GAP, false, STEP);
  assert_eq!(spaced.placements().len(), 2, "both parts are placed");
  let measured = outline(&spaced, 1).bounds().min.x - outline(&spaced, 0).bounds().max.x;
  assert!(
    (measured - 20.0).abs() <= EPS,
    "the authored 10 mm gap moves the circle to 20 mm, measured {measured} mm"
  );

  assert!(
    touching.certifies(&board, 0.0),
    "the flush plan is certified"
  );
  assert!(
    spaced.certifies(&board, GAP),
    "the spaced plan is certified at the authored gap"
  );
  assert!(
    !touching.certifies(&board, GAP),
    "the flush plan does not certify a gap it never honoured"
  );
}

#[test]
fn a_concave_part_sits_in_a_notch_only_its_exact_shape_can_reach() {
  // The board is exactly the L's bounding box, so the only room left for the 50 x 50 square is the
  // notch the L's missing corner leaves — a place where the two bounding boxes overlap and the two
  // outlines do not.
  let board = board(100.0, 100.0);
  let parts = [el(), rect(50.0, 50.0)];
  let plan = plan(&board, &parts, 0.0, false, STEP);

  assert_eq!(plan.sheets_used(), 1, "the square fits the L's board");
  assert_eq!(plan.unplaced(), &[] as &[usize], "nothing is left over");
  assert_eq!(
    placements(&plan),
    vec![
      (0, 0, Point2::new(0.0, 0.0), NestRotation::None),
      (1, 0, Point2::new(50.0, 50.0), NestRotation::None),
    ],
    "the square lands in the notch, at the one lattice corner that clears both arms"
  );

  // A bounding-box nester would refuse this placement: the square's box is inside the L's box.
  let l_box = outline(&plan, 0).bounds();
  let square_box = outline(&plan, 1).bounds();
  assert!(
    l_box.min.x <= square_box.min.x + EPS
      && l_box.min.y <= square_box.min.y + EPS
      && square_box.max.x <= l_box.max.x + EPS
      && square_box.max.y <= l_box.max.y + EPS,
    "the fixture is only a fixture if the square's box lies inside the L's box"
  );
  assert!(
    disjoint(outline(&plan, 0), outline(&plan, 1)),
    "the L's shape does not reach the square"
  );

  // Measured clearance, from the placed outlines: the square's low corner is 50 mm along each axis and
  // each arm stops at 40 mm, so the shape leaves 10 mm on both sides.
  assert!(
    (square_box.min.x - 40.0 - 10.0).abs() <= EPS && (square_box.min.y - 40.0 - 10.0).abs() <= EPS,
    "the square sits 10 mm from each arm, at {:?}",
    square_box.min
  );

  assert!(plan.certifies(&board, 0.0));
}

#[test]
fn the_coarse_search_step_misses_a_tight_placement_that_exists() {
  // The measured price of a sound but incomplete nester, with every number written out:
  //
  //   board   100 x 100
  //   part 0  a 40 x 100 bar, placed at (0, 0), so it reaches x = 40
  //   part 1  a 50 x 50 square
  //   gap     5 mm, so the square's own origin may start at x = 45 and still be exactly on the gap
  //   step    20 mm, so the only origins the heuristic tries are 0, 20, 40 and 60 along each axis
  //
  // A square at (45, 0) is legal: it is inside the board, it is 5 mm from the bar, and it leaves 5 mm
  // to the board's right edge. No lattice origin can reach it — 40 is inside the bar's no-fit region
  // and 60 puts the square past the board's edge — so the heuristic reports the square on a second
  // board. A **complete** nester would instead enumerate the boundary of the no-fit polygon itself,
  // whose corner in this fixture is the point (45, 0): the bar grown by 5 mm, a rectangle. Building
  // that boundary in general is the Minkowski difference of two outlines, which needs a boolean union
  // of the placed outline grown around each of its own edges and corners. `cam_geom::Region` has
  // `subtract` and no union and no intersection, so the polygon cannot be built today, and the lattice
  // below is what this module has instead. That is the documented limit, measured rather than implied.
  let board = board(100.0, 100.0);
  let parts = [rect(40.0, 100.0), rect(50.0, 50.0)];
  let gap = 5.0;
  let coarse = plan(&board, &parts, gap, false, 20.0);

  assert_eq!(
    placements(&coarse),
    vec![
      (0, 0, Point2::new(0.0, 0.0), NestRotation::None),
      (1, 1, Point2::new(0.0, 0.0), NestRotation::None),
    ],
    "the square is pushed onto a second board: {:?}",
    coarse
  );
  assert_eq!(coarse.sheets_used(), 2, "two boards where one would do");
  assert_eq!(
    coarse.unplaced(),
    &[] as &[usize],
    "the square is placed, just not where one board could have held it"
  );
  assert!(
    coarse.certifies(&board, gap),
    "what the heuristic produced is still legal"
  );

  // The placement the heuristic missed, built here and checked with this file's own measurements.
  let bar = outline(&coarse, 0);
  let missed = transform::translate(&parts[1], Vec2::new(45.0, 0.0)).expect("a translated outline");
  assert!(
    disjoint(bar, &missed),
    "the missed placement does not overlap the bar"
  );
  let missed_box = missed.bounds();
  assert!(
    (clearance(bar.bounds(), missed_box) - gap).abs() <= EPS,
    "the missed placement is exactly the 5 mm gap from the bar, measured {} mm",
    clearance(bar.bounds(), missed_box)
  );
  assert!(
    missed_box.max.x <= board.bounds().max.x + EPS
      && missed_box.max.y <= board.bounds().max.y + EPS,
    "the missed placement is inside the board"
  );
  assert!(
    (board.bounds().max.x - missed_box.max.x - gap).abs() <= EPS,
    "and leaves exactly one gap to the board's right edge"
  );

  // The step is the whole of the miss, not a broken rule: the same fixture with a 5 mm step finds a
  // one-board placement. It lands on the lattice point past the no-fit region rather than on its
  // corner, because the rule refuses contact, so it still pays one step: 10 mm from the bar, which is
  // the 5 mm gap plus the 5 mm step.
  let fine = plan(&board, &parts, gap, false, 5.0);
  assert_eq!(
    fine.sheets_used(),
    1,
    "a 5 mm step finds a one-board placement the 20 mm step missed"
  );
  assert_eq!(
    placements(&fine)[1],
    (1, 0, Point2::new(50.0, 0.0), NestRotation::None),
    "the finer lattice lands one step past the no-fit region's corner"
  );
  assert!(
    (clearance(bar.bounds(), outline(&fine, 1).bounds()) - 10.0).abs() <= EPS,
    "and the strict-disjoint rule leaves 10 mm, measured {} mm",
    clearance(bar.bounds(), outline(&fine, 1).bounds())
  );
  assert!(fine.certifies(&board, gap));
}

#[test]
fn a_part_is_turned_only_when_no_upright_placement_exists() {
  let narrow = board(50.0, 120.0);
  let part = rect(100.0, 40.0);

  // Upright the part is 100 wide and cannot enter a 50 mm board; turned it is 40 x 100 and can.
  let turned = plan(&narrow, std::slice::from_ref(&part), 0.0, true, STEP);
  assert_eq!(turned.placements().len(), 1, "the turned part is placed");
  assert_eq!(
    turned.placements()[0].rotation(),
    NestRotation::QuarterTurn,
    "the part had to be turned to fit"
  );
  let footprint = outline(&turned, 0).bounds();
  assert!(
    (footprint.width() - 40.0).abs() <= EPS && (footprint.height() - 100.0).abs() <= EPS,
    "the turned outline is the authored part on its side, {} by {} mm",
    footprint.width(),
    footprint.height()
  );
  assert!(
    footprint.max.x <= narrow.bounds().max.x + EPS
      && footprint.max.y <= narrow.bounds().max.y + EPS,
    "the turned outline is inside the board"
  );
  assert!(turned.certifies(&narrow, 0.0));

  // The same part on a board that holds it upright is never turned.
  let roomy = board(120.0, 60.0);
  let upright = plan(&roomy, std::slice::from_ref(&part), 0.0, true, STEP);
  assert_eq!(
    upright.placements()[0].rotation(),
    NestRotation::None,
    "a part that fits upright is not turned"
  );
  let footprint = outline(&upright, 0).bounds();
  assert!(
    (footprint.width() - 100.0).abs() <= EPS && (footprint.height() - 40.0).abs() <= EPS,
    "and its outline is the authored one, {} by {} mm",
    footprint.width(),
    footprint.height()
  );

  // Without rotation the same part is reported unplaced rather than forced onto its side.
  let refused = plan(&narrow, &[part], 0.0, false, STEP);
  assert_eq!(
    refused.placements().len(),
    0,
    "rotation refused means no placement"
  );
  assert_eq!(
    refused.unplaced(),
    &[0],
    "the part is reported, not dropped"
  );
  assert_eq!(refused.sheets_used(), 0, "and it opens no board");
}

#[test]
fn a_part_larger_than_the_board_is_reported_unplaced() {
  let board = board(100.0, 100.0);
  let parts = [disc(80.0), rect(300.0, 10.0), rect(40.0, 40.0)];
  let plan = plan(&board, &parts, 0.0, false, STEP);

  assert_eq!(
    plan.placements().len(),
    1,
    "only the fitting part is placed: {:?}",
    plan
  );
  assert_eq!(
    plan.placements()[0].part(),
    2,
    "the fitting part is the one"
  );
  assert_eq!(
    plan.unplaced(),
    &[0, 1],
    "both oversized parts are reported, in input order"
  );
  assert_eq!(
    plan.sheets_used(),
    1,
    "an oversized part does not open a board"
  );
  assert!(plan.certifies(&board, 0.0));
}

#[test]
fn a_part_that_fits_neither_arm_of_an_l_shaped_board_is_unplaced() {
  // Here the L is the **board**, not a part. Its 100 x 40 bottom arm and 40 x 100 left arm leave the
  // 60 x 60 notch from (40, 40) to (100, 100) as the only room in the bounding box, and a 50 x 50
  // square fits neither arm, so it fits no part of the material at all. A predicate that accepts
  // `Touching` as proof of inside-ness places it in the notch — a part off the stock, reported placed.
  let sheet = el();
  let parts = [rect(50.0, 50.0)];
  let plan = plan(&sheet, &parts, 0.0, false, STEP);

  assert_eq!(
    placements(&plan),
    vec![],
    "the square fits neither arm, so nothing is placed"
  );
  assert_eq!(plan.unplaced(), &[0], "the square is reported, not dropped");
  assert_eq!(plan.sheets_used(), 0, "and it opens no board");
}

#[test]
fn a_part_that_only_touches_a_circular_board_from_outside_is_unplaced() {
  // The board is the disc of radius 100 about (100, 100), so its bounding box is 200 x 200. The part
  // is the disc of radius r = 100 (3 - 2 sqrt 2) = 17.157287525380986 mm, anchored at that box's
  // corner: its centre is (r, r), and sqrt 2 (100 - r) = 100 + r, so the two circles are tangent at
  // (29.289321881345245, 29.289321881345245) and the part lies wholly outside the board, sharing
  // exactly one boundary point with it — `Touching`, and nothing more than `Touching`.
  let sheet = primitive::circle(Point2::new(100.0, 100.0), 100.0).expect("a circular board");
  let radius = 100.0 * (3.0 - 2.0 * 2.0_f64.sqrt());
  let part = disc(radius);
  assert_eq!(
    relation(&part, &sheet, TOL),
    Relation::Touching,
    "the fixture is only a fixture if the part touches the board from outside"
  );

  // The coarse step leaves the bounding box's own corner as the one anchor the heuristic tries.
  let touching = plan(&sheet, &[part], 0.0, false, 200.0);
  assert_eq!(
    placements(&touching),
    vec![],
    "the part is not placed off the stock"
  );
  assert_eq!(touching.unplaced(), &[0], "it is reported unplaced");
  assert_eq!(touching.sheets_used(), 0, "and it opens no board");

  // The same board holds a part that is inside it, so the refusal is the geometry, not the board.
  let inside = plan(&sheet, &[disc(50.0)], 0.0, false, STEP);
  assert_eq!(
    inside.placements().len(),
    1,
    "a disc of radius 50 fits the circular board: {inside:?}"
  );
  assert!(inside.certifies(&sheet, 0.0));
}

#[test]
fn a_part_flush_against_a_rectangular_board_is_placed_and_certified() {
  // The accept branch of the inside-ness rule, which the refusal tests above pin only from the other
  // side. A part that is legitimately flush against the board's edge touches the boundary, so the
  // kernel answers `Touching`, and the rule has to go on to prove the material is on the part's own
  // side. Invert `INSET_STEP`'s sign and every refusal above stays green while a flush part silently
  // stops being placed; this is the case that would notice.
  //
  // A 100 x 100 part on a 120 x 100 board: flush against the left, bottom and top edges, with its
  // lower-left corner at the board's own corner.
  let sheet = board(120.0, 100.0);
  let parts = [rect(100.0, 100.0)];

  // The 10 mm gap is authored and changes nothing here: the gap is the room a cut needs between
  // parts, never an inset of the board's own edges.
  let plan = plan(&sheet, &parts, GAP, false, STEP);

  assert_eq!(
    placements(&plan),
    vec![(0, 0, Point2::new(0.0, 0.0), NestRotation::None)],
    "the flush part is placed at the board's own corner: {plan:?}"
  );
  assert_eq!(
    plan.unplaced(),
    &[] as &[usize],
    "a flush part is not left over"
  );
  assert_eq!(plan.sheets_used(), 1, "and it opened exactly one board");

  let footprint = outline(&plan, 0).bounds();
  assert_eq!(
    footprint.min,
    Point2::new(0.0, 0.0),
    "the placed footprint starts at the board's corner"
  );
  assert_eq!(
    footprint.max,
    Point2::new(100.0, 100.0),
    "and measures the authored 100 x 100 part"
  );

  assert_eq!(
    relation(outline(&plan, 0), &sheet, TOL),
    Relation::Touching,
    "the fixture is only a fixture if the placed part touches the board"
  );
  assert!(
    plan.certifies(&sheet, GAP),
    "the flush part is certified at the authored gap"
  );
}

#[test]
fn a_part_flush_against_an_l_boards_arm_is_placed_and_certified() {
  // The same accept branch on a board that is not a rectangle, so the rule is pinned as "flush is
  // legal" rather than "flush is legal on rectangles". The board is `el()`: a 100 x 40 bottom arm
  // and a 40 x 100 left arm, with the 60 x 60 notch from (40, 40) to (100, 100) taken out of it, so
  // 6400 mm² of material sit inside a 10000 mm² box. That notch is where the refusal test above puts
  // a part; this one puts a part on the wall beside it.
  //
  //   part 0  a 10 x 100 strip along the left edge. Larger area, so it is placed first, and it is
  //           what pushes part 1 off the board's own corner: the lattice's first column is blocked.
  //   part 1  a 20 x 45 part anchored at (20, 0). Its right edge from y = 40 to y = 45 lies flush
  //           against the left arm's inner wall — the boundary with the notch on its far side —
  //           while its left, bottom and top edges are in the material. Touching the board is all the
  //           kernel reports, so the only proof it is inside is the material being on its own side.
  let sheet = el();
  let parts = [rect(10.0, 100.0), rect(20.0, 45.0)];

  // No gap between the parts: a 10 mm gap would grow part 0's no-fit region over the anchor at
  // x = 20, and the subject here is the board's boundary, not the gap between neighbours. The two
  // parts still end up 10 mm apart, which is a distance rather than a contact.
  let plan = plan(&sheet, &parts, 0.0, false, STEP);

  assert_eq!(
    placements(&plan),
    vec![
      (0, 0, Point2::new(0.0, 0.0), NestRotation::None),
      (1, 0, Point2::new(20.0, 0.0), NestRotation::None),
    ],
    "the strip takes the corner and the flush part is anchored one strip and one step along: {plan:?}"
  );
  assert_eq!(
    plan.unplaced(),
    &[] as &[usize],
    "a flush part is not left over"
  );
  assert_eq!(plan.sheets_used(), 1, "and both parts share one board");

  let footprint = outline(&plan, 1).bounds();
  assert_eq!(
    footprint.min,
    Point2::new(20.0, 0.0),
    "the flush part's footprint starts at its own anchor"
  );
  assert_eq!(
    footprint.max,
    Point2::new(40.0, 45.0),
    "and its right edge is the left arm's inner wall, the authored 20 x 45 part away"
  );

  assert_eq!(
    relation(outline(&plan, 1), &sheet, TOL),
    Relation::Touching,
    "the fixture is only a fixture if the placed part touches the board"
  );
  assert!(
    plan.certifies(&sheet, 0.0),
    "the flush part is certified against the L's material"
  );
}

#[test]
fn a_flush_part_one_inset_step_past_the_edge_is_refused() {
  // The boundary in the other direction, so nothing above can be satisfied by loosening the rule:
  // the same parts, one `INSET_STEP` past the edge they were flush against. A hundredth of a
  // millimetre is the whole distance between the two outcomes, which is what makes this pair a
  // boundary instead of two unrelated fixtures.

  // The rectangle: the 100 x 100 part of the first case, on a board one inset step narrower, pokes
  // 0.01 mm past the right edge.
  let rectangular = board(120.0, 100.0);
  let parts = [rect(100.0, 100.0)];
  let flush = plan(&rectangular, &parts, GAP, false, STEP);
  assert!(
    flush.certifies(&rectangular, GAP),
    "the fixture is certified while it is flush: {flush:?}"
  );
  // Exactly as wide as the part is still flush, and still certified: the accepted side of the
  // boundary is contact, not a clearance.
  assert!(
    flush.certifies(&board(100.0, 100.0), GAP),
    "a part exactly as large as the board sits flush on all four edges and is certified"
  );

  let narrow = board(100.0 - INSET_STEP, 100.0);
  let past = plan(&narrow, &parts, GAP, false, STEP);
  assert_eq!(
    placements(&past),
    vec![],
    "0.01 mm past the edge is not a placement: {past:?}"
  );
  assert_eq!(
    past.unplaced(),
    &[0],
    "the packer reports the part instead of squeezing it in"
  );
  assert_eq!(past.sheets_used(), 0, "and it opens no board");
  assert!(
    !flush.certifies(&narrow, GAP),
    "the certificate that accepted the flush part refuses the same 0.01 mm past the edge"
  );

  // The L: the same two parts, with the board moved one inset step to the left, so the flush part's
  // right edge sits 0.01 mm inside the notch instead of against the wall. Moving the board and
  // pushing the part are the same geometry; moving the board is the half a test can state without
  // describing a placement the packer would never make.
  let l_sheet = el();
  let l_parts = [rect(10.0, 100.0), rect(20.0, 45.0)];
  let flush = plan(&l_sheet, &l_parts, 0.0, false, STEP);
  assert!(
    flush.certifies(&l_sheet, 0.0),
    "the L fixture is certified at the wall: {flush:?}"
  );
  let shifted =
    transform::translate(&l_sheet, Vec2::new(-INSET_STEP, 0.0)).expect("a translated board");
  assert!(
    !flush.certifies(&shifted, 0.0),
    "a part one inset step past the left arm's inner wall is outside the material"
  );
}

#[test]
fn a_second_sheet_is_opened_only_when_the_first_cannot_hold_the_part() {
  let board = board(100.0, 100.0);

  // Two 40 x 40 parts share a board: one board, no second board opened on spec.
  let together = plan(
    &board,
    &[rect(40.0, 40.0), rect(40.0, 40.0)],
    0.0,
    false,
    STEP,
  );
  assert_eq!(together.placements().len(), 2, "both parts are placed");
  assert_eq!(together.sheets_used(), 1, "one board is enough");
  assert_eq!(
    placements(&together),
    vec![
      (0, 0, Point2::new(0.0, 0.0), NestRotation::None),
      (1, 0, Point2::new(50.0, 0.0), NestRotation::None),
    ],
    "the second part sits beside the first, one step clear of it"
  );

  // Two 60 x 60 parts cannot: the second opens a board of its own and starts at its origin.
  let tight = plan(
    &board,
    &[rect(60.0, 60.0), rect(60.0, 60.0)],
    0.0,
    false,
    STEP,
  );
  assert_eq!(tight.placements().len(), 2, "both parts are placed");
  assert_eq!(tight.sheets_used(), 2, "a second board was needed");
  assert_eq!(tight.unplaced(), &[] as &[usize], "nothing is left over");
  assert_eq!(
    tight.placements()[1].sheet(),
    1,
    "the second part is on the second board"
  );
  assert_eq!(
    tight.placements()[1].origin(),
    Point2::new(0.0, 0.0),
    "a board starts at its own origin"
  );
  assert!(tight.certifies(&board, 0.0));
}

#[test]
fn the_same_input_twice_is_the_same_plan() {
  let board = board(300.0, 200.0);
  let parts = [
    disc(45.0),
    rect(120.0, 80.0),
    el(),
    rect(60.0, 60.0),
    disc(25.0),
  ];
  let first = plan(&board, &parts, GAP, true, STEP);
  let second = plan(&board, &parts, GAP, true, STEP);

  assert_eq!(first, second, "the same input gives the same plan");
  assert_eq!(
    format!("{first:?}"),
    format!("{second:?}"),
    "the same input prints the same plan"
  );
  assert!(first.placements().len() >= 2, "the fixture places parts");
  assert!(
    first.certifies(&board, GAP),
    "and the plan's certificate holds"
  );

  for (left, right) in first.placements().iter().zip(second.placements()) {
    assert_eq!(
      (left.part(), left.sheet()),
      (right.part(), right.sheet()),
      "same order"
    );
    assert_eq!(left.origin(), right.origin(), "same origin");
    assert_eq!(left.rotation(), right.rotation(), "same rotation");
  }
  for index in 0..first.placements().len() {
    assert_eq!(
      outline(&first, index),
      outline(&second, index),
      "the same outline at placement {index}"
    );
  }
}

#[test]
fn the_certificate_fails_when_a_plan_breaks_it() {
  let sheet = board(400.0, 300.0);
  let parts = [rect(200.0, 100.0), disc(50.0)];
  let plan = plan(&sheet, &parts, GAP, false, STEP);
  assert!(plan.certifies(&sheet, GAP), "the fixture is certified");

  // A board that stops short of the furthest placement refuses it.
  assert!(
    !plan.certifies(&board(210.0, 300.0), GAP),
    "a placement outside the board is refused"
  );
  // Demanding a wider gap than the neighbours keep is refused too.
  assert!(
    !plan.certifies(&sheet, GAP + 1000.0),
    "neighbours closer than the demanded gap are refused"
  );
  // A gap that is not a distance at all certifies nothing.
  assert!(
    !plan.certifies(&sheet, -1.0),
    "a negative gap is not a certificate"
  );
  assert!(
    !plan.certifies(&sheet, f64::NAN),
    "a gap that is not a number is not a certificate"
  );
  // And a board with no room in it holds nothing.
  assert!(!plan.certifies(&board(1.0e-9, 1.0e-9), GAP));
}

#[test]
fn the_certificate_refuses_a_placement_the_board_does_not_contain() {
  // One plan, two boards. The plan is built on a board that contains its parts and then asked about a
  // board that does not, so the certificate — never the packer — is what answers. A certificate that
  // reads `Touching` as inside-ness answers `true` to both, which is the false positive pinned here.
  let square_board = board(100.0, 100.0);
  let l_plan = plan(&square_board, &[el(), rect(50.0, 50.0)], 0.0, false, STEP);
  assert!(
    l_plan.certifies(&square_board, 0.0),
    "the plan is legal on the square board: {l_plan:?}"
  );
  assert!(
    !l_plan.certifies(&el(), 0.0),
    "the 50 x 50 square sits in the L's notch, so the L does not contain the plan"
  );

  let box_board = board(200.0, 200.0);
  let circle = primitive::circle(Point2::new(100.0, 100.0), 100.0).expect("a circular board");
  let radius = 100.0 * (3.0 - 2.0 * 2.0_f64.sqrt());
  let circle_plan = plan(&box_board, &[disc(radius)], 0.0, false, STEP);
  assert!(
    circle_plan.certifies(&box_board, 0.0),
    "the disc sits in the square board's corner: {circle_plan:?}"
  );
  assert!(
    !circle_plan.certifies(&circle, 0.0),
    "and the same disc touches the circular board's boundary from outside it"
  );
}

#[test]
fn a_search_step_fine_enough_to_ask_for_more_anchors_than_the_cap_is_refused() {
  // The board the default job authors, 1219 by 2438 mm. A search walks
  // `(floor(width / step) + 1) * (floor(height / step) + 1)` anchors, so the cap is a statement about
  // arithmetic rather than a taste in millimetres: at 0.5 mm this board asks for
  // 2439 * 4877 = 11 895 003 of them.
  let sheet = board(1219.0, 2438.0);
  assert_eq!(
    ProfileNestPlan::new(&sheet, &[rect(100.0, 60.0)], options(GAP, false, 0.5)),
    Err(GeomError::SearchLatticeTooFine {
      step: 0.5,
      columns: 2439,
      rows: 4877,
      anchors: 11_895_003,
      cap: MAX_SEARCH_ANCHORS,
    }),
    "a step finer than the cap allows is refused with the lattice it asked for"
  );
  // The other side of the same boundary: the same board at 2 mm is 610 * 1220 = 744 200 anchors, and
  // which of the two the caller wants is still the caller's decision.
  let plan = ProfileNestPlan::new(&sheet, &[rect(100.0, 60.0)], options(GAP, false, 2.0))
    .expect("a 2 mm search step is under the cap");
  assert_eq!(plan.sheets_used(), 1);
}

#[test]
fn an_over_cap_step_does_not_hide_an_open_part_the_document_already_has() {
  // The cap is a cost refusal, not a document refusal. A document that already has a geometry fault
  // keeps the error that names the offending part, or the new refusal answers a question the caller
  // did not ask (review `review-2b7f8544a4b4dbd6`, finding `R3-001`).
  let sheet = board(1219.0, 2438.0);
  let open = primitive::line(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0)).expect("a line");
  assert_eq!(
    ProfileNestPlan::new(&sheet, &[rect(10.0, 10.0), open], options(GAP, false, 0.5)),
    Err(GeomError::PartNotClosed { part: 1 }),
    "the part that is not closed is named before the step's cost is refused"
  );
}

#[test]
fn the_anchor_cap_is_inclusive_at_exactly_the_cap() {
  // `anchors > MAX_SEARCH_ANCHORS` allows exactly the cap. Without this pair a mutation of the
  // comparison to `>=` would pass every other test, because the only counts pinned are 11 895 003
  // (refused) and 744 200 (accepted) — review `review-85153ac6e3a443c8`, finding `R3-CAP-BOUNDARY`.
  // 1999 x 1999 at 1 mm is (1999 + 1) * (1999 + 1) = exactly 4 000 000 anchors.
  let at_cap = ProfileNestPlan::new(
    &board(1999.0, 1999.0),
    &[rect(20.0, 10.0)],
    options(GAP, false, 1.0),
  )
  .expect("exactly the cap is allowed");
  assert_eq!(at_cap.sheets_used(), 1);
  // 2000 x 2000 at 1 mm is 2001 * 2001 = 4 004 001: three thousand and one anchors past it.
  assert_eq!(
    ProfileNestPlan::new(
      &board(2000.0, 2000.0),
      &[rect(20.0, 10.0)],
      options(GAP, false, 1.0)
    ),
    Err(GeomError::SearchLatticeTooFine {
      step: 1.0,
      columns: 2001,
      rows: 2001,
      anchors: 4_004_001,
      cap: MAX_SEARCH_ANCHORS,
    }),
    "one anchor past the cap is refused"
  );
}

#[test]
fn refusals_name_their_value() {
  assert_eq!(
    ProfileNestOptions::new(-1.0, true),
    Err(GeomError::GapNotMeasurable { gap: -1.0 }),
    "a negative gap is refused with its value"
  );
  assert_eq!(
    GeomError::GapNotMeasurable { gap: -1.0 }.to_string(),
    "a nesting gap must be a finite number of millimetres zero or more, got -1 mm"
  );
  assert!(
    matches!(
      ProfileNestOptions::new(f64::NAN, false),
      Err(GeomError::GapNotMeasurable { gap }) if gap.is_nan()
    ),
    "a non-finite gap is refused with its value"
  );

  let valid = ProfileNestOptions::new(0.0, false).expect("a valid gap");
  assert_eq!(
    valid.step(),
    cam_geom::nest_profile::DEFAULT_SEARCH_STEP,
    "the step starts at the module's default until a caller authors one"
  );
  assert_eq!(
    valid.with_step(0.0),
    Err(GeomError::SearchStepNotMeasurable { step: 0.0 }),
    "a step that cannot advance is refused with its value"
  );
  assert_eq!(
    GeomError::SearchStepNotMeasurable { step: 0.0 }.to_string(),
    "a free-profile nesting search step must be a positive number of millimetres, got 0 mm"
  );
  assert_eq!(
    valid.with_step(f64::INFINITY),
    Err(GeomError::SearchStepNotMeasurable {
      step: f64::INFINITY
    }),
    "a non-finite step is refused with its value"
  );

  // A board has to be a closed shape to enclose anything.
  let open = primitive::line(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0)).expect("a line");
  assert_eq!(
    ProfileNestPlan::new(&open, &[rect(10.0, 10.0)], valid),
    Err(GeomError::SheetNotClosed),
    "an open board is refused by name"
  );
  assert_eq!(
    GeomError::SheetNotClosed.to_string(),
    "a free-profile nesting sheet must be a closed contour"
  );

  // A closed board with no width has no room in it, and says so with both side lengths.
  let flat = Contour::from_points(
    &[
      Point2::new(0.0, 0.0),
      Point2::new(0.0, 100.0),
      Point2::new(0.0, 50.0),
    ],
    true,
  )
  .expect("a flat contour");
  assert_eq!(
    ProfileNestPlan::new(&flat, &[rect(10.0, 10.0)], valid),
    Err(GeomError::SheetNotMeasurable {
      width: 0.0,
      height: 100.0,
    }),
    "a board with no width is refused with both values"
  );

  // A part has to be a closed shape to be a shape.
  assert_eq!(
    ProfileNestPlan::new(&board(100.0, 100.0), &[open], valid),
    Err(GeomError::PartNotClosed { part: 0 }),
    "an open part is refused with its index"
  );
  assert_eq!(
    GeomError::PartNotClosed { part: 0 }.to_string(),
    "free-profile nesting part 0 must be a closed contour"
  );
}
