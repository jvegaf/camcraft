//! Behaviour tests for rectangle nesting (M9 T2; S5–S7).
//!
//! Every expectation here is written from the geometry rather than read back from the module under
//! test: a gap is a **measured distance** between two footprints, a part is inside a sheet when its
//! bounds are, and two parts do not overlap when the measured clearance between them is not
//! negative. The nester's own `certifies` predicate is asserted **alongside** those measurements,
//! never instead of them, so the sentence the CLI will rely on is the one this file proves.

use cam_geom::nest::{NestAlgorithm, NestOptions, NestPlan, NestRotation};
use cam_geom::{Bounds, GeomError, Point2};

/// Tolerance for comparing millimetre coordinates, well below any machine resolution and far above
/// `f64` noise at job scale.
const EPS: f64 = 1.0e-9;

/// A rectangle from its two side lengths, with its lower-left corner at the origin.
fn dims(width: f64, height: f64) -> Bounds {
  Bounds {
    min: Point2::new(0.0, 0.0),
    max: Point2::new(width, height),
  }
}

/// The board the cabinetry fixture is cut from: 2440 x 1220, the board every cabinet shop buys.
fn sheet() -> Bounds {
  dims(2440.0, 1220.0)
}

/// The gap between neighbouring parts the fixtures author, in millimetres.
const GAP: f64 = 8.0;

/// Two sides and three shelves of one base cabinet, the parts S7 says rectangle nesting must cover.
///
/// The sides are 720 x 580 (a 720 high base cabinet, 580 deep) and the shelves are 560 x 520 (the
/// shelf inside that carcass). All five fit one board.
fn cabinetry() -> Vec<Bounds> {
  vec![
    dims(720.0, 580.0),
    dims(720.0, 580.0),
    dims(560.0, 520.0),
    dims(560.0, 520.0),
    dims(560.0, 520.0),
  ]
}

/// A plan of `parts` on `sheet` at `gap`, with 90 degree rotation refused.
fn plan_at(sheet: Bounds, parts: &[Bounds], gap: f64) -> NestPlan {
  NestPlan::new(
    sheet,
    parts,
    NestOptions::new(gap, false).expect("a valid gap"),
  )
  .expect("a plan")
}

/// A plan of `parts` on `sheet` at `gap` asked of one named algorithm, rotation refused.
fn plan_by(sheet: Bounds, parts: &[Bounds], gap: f64, algorithm: NestAlgorithm) -> NestPlan {
  NestPlan::new(
    sheet,
    parts,
    NestOptions::new(gap, false)
      .expect("a valid gap")
      .with_algorithm(algorithm),
  )
  .expect("a plan")
}

/// The board of the fixture where the two heuristics disagree, 234 x 173 millimetres.
fn contested_sheet() -> Bounds {
  dims(234.0, 173.0)
}

/// Three parts that fit one 234 x 173 board only when the cuts are guillotine cuts.
///
/// Maxrects splits the board around the 111 x 87 part into a 123 mm wide strip on its right and an
/// 86 mm high strip above it; the 110 x 60 part goes on the right, and the 38 x 112 part then has no
/// free rectangle 112 mm tall and is pushed onto a second board. Guillotine keeps the full-width
/// strip above the first part, the 110 x 60 part goes there, and the 38 x 112 part takes the tall
/// strip on the right — one board.
fn contested_parts() -> Vec<Bounds> {
  vec![dims(110.0, 60.0), dims(111.0, 87.0), dims(38.0, 112.0)]
}

/// The rectilinear clearance between two footprints, measured in this file's own arithmetic.
///
/// Positive when the two are apart by that much along at least one axis, zero when they touch, and
/// negative when they overlap — which is the only definition that makes "the gap is honoured" and
/// "they do not overlap" one measurement instead of two flags.
fn clearance(a: Bounds, b: Bounds) -> f64 {
  let dx = (b.min.x - a.max.x).max(a.min.x - b.max.x);
  let dy = (b.min.y - a.max.y).max(a.min.y - b.max.y);
  dx.max(dy)
}

/// True when `footprint` lies inside `sheet`, edges included.
fn inside(footprint: Bounds, sheet: Bounds) -> bool {
  footprint.min.x >= sheet.min.x - EPS
    && footprint.min.y >= sheet.min.y - EPS
    && footprint.max.x <= sheet.max.x + EPS
    && footprint.max.y <= sheet.max.y + EPS
}

/// The part indices a plan places, in placement order.
fn placed_parts(plan: &NestPlan) -> Vec<usize> {
  plan.placements().iter().map(|p| p.part()).collect()
}

#[test]
fn cabinetry_sides_and_shelves_pack_on_one_sheet_without_overlap() {
  let parts = cabinetry();
  let plan = plan_at(sheet(), &parts, GAP);

  assert_eq!(
    plan.sheets_used(),
    1,
    "the fixture's five parts fit one board"
  );
  assert_eq!(plan.placements().len(), 5, "every part is placed");
  assert_eq!(
    plan.unplaced(),
    &[] as &[usize],
    "nothing is reported unplaced"
  );

  let mut placed = placed_parts(&plan);
  placed.sort_unstable();
  assert_eq!(placed, vec![0, 1, 2, 3, 4], "each input part exactly once");

  for placement in plan.placements() {
    let footprint = placement.footprint();
    assert!(
      inside(footprint, sheet()),
      "part {} at {:?} is outside the sheet",
      placement.part(),
      footprint
    );
    assert_eq!(
      (footprint.width(), footprint.height()),
      (
        parts[placement.part()].width(),
        parts[placement.part()].height()
      ),
      "no part is rotated on this fixture"
    );
    assert_eq!(
      placement.sheet(),
      0,
      "one sheet, so every placement is on it"
    );
  }

  for (index, first) in plan.placements().iter().enumerate() {
    for second in &plan.placements()[index + 1..] {
      let measured = clearance(first.footprint(), second.footprint());
      assert!(
        measured >= GAP - EPS,
        "parts {} and {} are {measured} mm apart, the authored gap is {GAP} mm",
        first.part(),
        second.part()
      );
    }
  }

  assert!(
    plan.certifies(sheet(), GAP),
    "the plan's own certificate holds on the cabinetry fixture"
  );

  let placed_area: f64 = parts
    .iter()
    .map(|part| part.width() * part.height())
    .sum::<f64>();
  let expected = placed_area / (sheet().width() * sheet().height());
  assert!(
    (plan.utilisation(sheet()) - expected).abs() <= 1.0e-12,
    "utilisation is the placed area over one board: got {}, expected {expected}",
    plan.utilisation(sheet())
  );
}

#[test]
fn the_authored_gap_is_measured_between_neighbours() {
  let parts = vec![dims(400.0, 200.0), dims(400.0, 200.0)];

  // With no gap authored the two neighbours touch: the measured clearance is zero, not a flag.
  let touching = plan_at(dims(1000.0, 500.0), &parts, 0.0);
  assert_eq!(touching.placements().len(), 2, "both parts are placed");
  let measured = clearance(
    touching.placements()[0].footprint(),
    touching.placements()[1].footprint(),
  );
  assert!(
    measured.abs() <= EPS,
    "a zero gap puts the neighbours flush, measured {measured} mm"
  );

  // With 60 mm authored the neighbours are 60 mm apart, measured on the footprints.
  let spaced = plan_at(dims(1000.0, 500.0), &parts, 60.0);
  assert_eq!(spaced.placements().len(), 2, "both parts are placed");
  let measured = clearance(
    spaced.placements()[0].footprint(),
    spaced.placements()[1].footprint(),
  );
  assert!(
    (measured - 60.0).abs() <= EPS,
    "the authored 60 mm gap is the measured clearance, got {measured} mm"
  );
  assert!(
    spaced.certifies(dims(1000.0, 500.0), 60.0),
    "the certificate holds at the authored gap"
  );
}

#[test]
fn a_part_is_turned_only_when_it_does_not_fit_upright() {
  let narrow = dims(600.0, 900.0);
  let part = dims(800.0, 500.0);

  // Upright 800 x 500 cannot enter a 600 wide sheet; turned 500 x 800 can.
  let turned = NestPlan::new(
    narrow,
    &[part],
    NestOptions::new(0.0, true).expect("a valid gap"),
  )
  .expect("a plan");
  assert_eq!(turned.placements().len(), 1, "the turned part is placed");
  assert_eq!(
    turned.placements()[0].rotation(),
    NestRotation::QuarterTurn,
    "the part had to be turned to fit"
  );
  let footprint = turned.placements()[0].footprint();
  assert_eq!(
    (footprint.width(), footprint.height()),
    (500.0, 800.0),
    "the turned footprint is the authored part on its side"
  );
  assert!(
    inside(footprint, narrow),
    "the turned footprint is inside the sheet"
  );
  assert!(turned.certifies(narrow, 0.0));

  // The same part on a sheet that holds it upright is never turned.
  let roomy = plan_at(dims(900.0, 900.0), &[part], 0.0);
  assert_eq!(
    roomy.placements()[0].rotation(),
    NestRotation::None,
    "a part that fits upright is not turned"
  );

  // A neighbour that fits upright keeps its neighbour's clearance: no overlap, measured.
  let pair = NestPlan::new(
    narrow,
    &[dims(600.0, 300.0), dims(600.0, 200.0)],
    NestOptions::new(0.0, true).expect("a valid gap"),
  )
  .expect("a plan");
  assert_eq!(pair.placements().len(), 2, "both parts are placed");
  for placement in pair.placements() {
    assert_eq!(
      placement.rotation(),
      NestRotation::None,
      "both parts fit upright, so neither is turned"
    );
  }
  let measured = clearance(
    pair.placements()[0].footprint(),
    pair.placements()[1].footprint(),
  );
  assert!(
    measured >= -EPS,
    "the two placements do not overlap, measured {measured} mm"
  );
  assert!(pair.certifies(narrow, 0.0));

  // Without rotation the same part is reported unplaced rather than forced onto its side.
  let refused = plan_at(narrow, &[part], 0.0);
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
}

#[test]
fn a_part_larger_than_the_sheet_is_reported_unplaced() {
  let sheet = dims(1000.0, 500.0);
  let parts = vec![dims(400.0, 200.0), dims(1200.0, 100.0), dims(100.0, 600.0)];
  let plan = plan_at(sheet, &parts, 0.0);

  assert_eq!(
    plan.placements().len(),
    1,
    "only the fitting part is placed"
  );
  assert_eq!(
    plan.placements()[0].part(),
    0,
    "the fitting part is the one"
  );
  assert_eq!(
    plan.unplaced(),
    &[1, 2],
    "both oversized parts are reported, in input order"
  );
  assert_eq!(
    plan.sheets_used(),
    1,
    "an oversized part does not open a sheet"
  );
  assert!(plan.certifies(sheet, 0.0));
}

#[test]
fn a_second_sheet_is_opened_only_when_the_first_is_full() {
  let sheet = dims(1000.0, 500.0);

  // Two parts that share a board keep it: one sheet, no second sheet opened on spec.
  let together = plan_at(sheet, &[dims(400.0, 200.0), dims(500.0, 300.0)], 0.0);
  assert_eq!(together.placements().len(), 2, "both parts are placed");
  assert_eq!(together.sheets_used(), 1, "one board is enough");

  // When the first board cannot hold the next part, a second identical board is opened and the
  // part sits at its origin.
  let split = plan_at(sheet, &[dims(1000.0, 500.0), dims(400.0, 200.0)], 0.0);
  assert_eq!(split.placements().len(), 2, "both parts are placed");
  assert_eq!(split.sheets_used(), 2, "a second board was needed");
  assert_eq!(split.unplaced(), &[] as &[usize], "nothing is left over");
  let sheets: Vec<usize> = split.placements().iter().map(|p| p.sheet()).collect();
  assert_eq!(sheets, vec![0, 1], "the second part is on the second board");
  assert_eq!(
    split.placements()[1].origin(),
    Point2::new(0.0, 0.0),
    "the second board starts at its own origin"
  );
  assert!(split.certifies(sheet, 0.0));
}

#[test]
fn the_same_input_twice_is_the_same_plan() {
  let mut parts = cabinetry();
  parts.push(dims(1100.0, 900.0));
  parts.push(dims(420.0, 380.0));
  parts.push(dims(300.0, 300.0));
  let options = || NestOptions::new(GAP, true).expect("a valid gap");

  let first = NestPlan::new(sheet(), &parts, options()).expect("a plan");
  let second = NestPlan::new(sheet(), &parts, options()).expect("a plan");

  assert_eq!(first, second, "the same input gives the same plan");
  assert_eq!(
    format!("{first:?}"),
    format!("{second:?}"),
    "the same input prints the same plan"
  );
  assert!(first.sheets_used() >= 1, "the fixture places something");
  assert!(first.certifies(sheet(), GAP), "and the certificate holds");

  for (a, b) in first.placements().iter().zip(second.placements()) {
    assert_eq!((a.part(), a.sheet()), (b.part(), b.sheet()), "same order");
    assert_eq!(a.origin(), b.origin(), "same origin");
    assert_eq!(a.rotation(), b.rotation(), "same rotation");
  }
}

#[test]
fn equal_area_parts_keep_the_callers_order() {
  let parts = vec![dims(300.0, 100.0), dims(100.0, 300.0), dims(200.0, 150.0)];
  let plan = plan_at(dims(2000.0, 1000.0), &parts, 0.0);

  assert_eq!(plan.placements().len(), 3, "every part is placed");
  assert_eq!(
    placed_parts(&plan),
    vec![0, 1, 2],
    "equal areas are tie-broken by the caller's order"
  );
}

#[test]
fn the_certificate_holds_at_the_sheets_own_edges() {
  // A part exactly the size of the sheet sits flush on all four edges and is still inside.
  for gap in [0.0, GAP] {
    let plan = plan_at(sheet(), &[dims(sheet().width(), sheet().height())], gap);
    assert_eq!(plan.placements().len(), 1, "the full-sheet part is placed");
    let footprint = plan.placements()[0].footprint();
    assert_eq!(
      footprint.min,
      Point2::new(0.0, 0.0),
      "flush on the low edges"
    );
    assert_eq!(
      footprint.max,
      Point2::new(sheet().width(), sheet().height()),
      "flush on the high edges"
    );
    assert!(inside(footprint, sheet()), "edges included, still inside");
    assert!(
      plan.certifies(sheet(), gap),
      "the certificate holds flush against the sheet's own edges at gap {gap} mm"
    );
  }

  // Every fixture in this file is certified: the same five parts at the authored gap, and the pair
  // that a second board holds.
  assert!(plan_at(sheet(), &cabinetry(), GAP).certifies(sheet(), GAP));
  assert!(
    plan_at(dims(1000.0, 500.0), &[dims(400.0, 200.0)], GAP).certifies(dims(1000.0, 500.0), GAP)
  );
}

#[test]
fn the_certificate_fails_when_a_plan_breaks_it() {
  let plan = plan_at(sheet(), &cabinetry(), GAP);
  assert!(plan.certifies(sheet(), GAP), "the fixture is certified");

  // Shrinking the board just past the furthest placement pushes it outside.
  let furthest = plan
    .placements()
    .iter()
    .map(|placement| placement.footprint().max.x)
    .fold(0.0_f64, f64::max);
  assert!(furthest > 0.0, "the fixture spreads across the board");
  assert!(
    !plan.certifies(dims(furthest - 1.0, sheet().height()), GAP),
    "a placement outside the sheet is refused"
  );
  // Demanding a wider gap than the neighbours keep is refused too.
  assert!(
    !plan.certifies(sheet(), GAP + 1000.0),
    "neighbours closer than the demanded gap are refused"
  );
  // A gap that is not a distance at all certifies nothing.
  assert!(
    !plan.certifies(sheet(), -1.0),
    "a negative gap is not a certificate"
  );
}

#[test]
fn a_refused_gap_or_sheet_names_its_value() {
  assert_eq!(
    NestOptions::new(-1.0, true),
    Err(GeomError::GapNotMeasurable { gap: -1.0 }),
    "a negative gap is refused with its value"
  );
  assert_eq!(
    GeomError::GapNotMeasurable { gap: -1.0 }.to_string(),
    "a nesting gap must be a finite number of millimetres zero or more, got -1 mm"
  );
  assert!(
    matches!(
      NestOptions::new(f64::NAN, false),
      Err(GeomError::GapNotMeasurable { gap }) if gap.is_nan()
    ),
    "a non-finite gap is refused with its value"
  );

  let options = NestOptions::new(GAP, true).expect("a valid gap");
  assert_eq!(
    NestPlan::new(dims(0.0, 1200.0), &[dims(10.0, 10.0)], options),
    Err(GeomError::SheetNotMeasurable {
      width: 0.0,
      height: 1200.0,
    }),
    "a sheet with no width is refused with both values"
  );
  assert_eq!(
    GeomError::SheetNotMeasurable {
      width: 0.0,
      height: 1200.0,
    }
    .to_string(),
    "a nesting sheet needs positive millimetre side lengths, got 0 mm by 1200 mm"
  );

  assert_eq!(
    NestPlan::new(dims(1000.0, 500.0), &[dims(-5.0, 10.0)], options),
    Err(GeomError::PartNotMeasurable {
      part: 0,
      width: -5.0,
      height: 10.0,
    }),
    "an inverted part is refused with its index and size"
  );
  assert_eq!(
    GeomError::PartNotMeasurable {
      part: 0,
      width: -5.0,
      height: 10.0,
    }
    .to_string(),
    "nesting part 0 needs non-negative, finite millimetre side lengths, got -5 mm by 10 mm"
  );
}

#[test]
fn maxrects_is_reachable_and_produces_the_maxrects_plan() {
  let plan = plan_by(
    contested_sheet(),
    &contested_parts(),
    GAP,
    NestAlgorithm::MaxRects,
  );

  assert_eq!(
    plan.algorithm(),
    NestAlgorithm::MaxRects,
    "the plan names the algorithm it was asked for"
  );
  assert_eq!(plan.placements().len(), 3, "every part is placed");
  assert_eq!(plan.unplaced(), &[] as &[usize], "nothing is unplaced");
  assert_eq!(
    plan.sheets_used(),
    2,
    "maxrects needs a second board on the contested fixture"
  );

  // The pinned geometry is what makes this the maxrects plan rather than any plan that packs: the
  // 110 x 60 part sits beside the 111 x 87 one on the narrow strip maxrects leaves there, so the tall
  // 38 x 112 part is pushed onto the second board.
  let origins: Vec<(usize, usize, Point2, NestRotation)> = plan
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
    .collect();
  assert_eq!(
    origins,
    vec![
      (1, 0, Point2::new(0.0, 0.0), NestRotation::None),
      (0, 0, Point2::new(119.0, 0.0), NestRotation::None),
      (2, 1, Point2::new(0.0, 0.0), NestRotation::None),
    ],
    "the maxrects plan is the maxrects one, origins included"
  );
  assert!(
    plan.certifies(contested_sheet(), GAP),
    "and the certificate holds"
  );
}

#[test]
fn guillotine_is_reachable_and_produces_the_guillotine_plan() {
  let sheet = contested_sheet();
  let parts = contested_parts();
  let plan = plan_by(sheet, &parts, GAP, NestAlgorithm::Guillotine);

  assert_eq!(
    plan.algorithm(),
    NestAlgorithm::Guillotine,
    "the plan names the algorithm it was asked for"
  );
  assert_eq!(plan.placements().len(), 3, "every part is placed");
  assert_eq!(plan.unplaced(), &[] as &[usize], "nothing is unplaced");
  assert_eq!(
    plan.sheets_used(),
    1,
    "guillotine packs all three parts on one board"
  );

  // Full-length splits put the 110 x 60 part in the strip above the first part and the tall
  // 38 x 112 part in the strip beside it — the cuts a panel saw makes.
  let origins: Vec<(usize, usize, Point2, NestRotation)> = plan
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
    .collect();
  assert_eq!(
    origins,
    vec![
      (1, 0, Point2::new(0.0, 0.0), NestRotation::None),
      (0, 0, Point2::new(0.0, 95.0), NestRotation::None),
      (2, 0, Point2::new(119.0, 0.0), NestRotation::None),
    ],
    "the guillotine plan is the guillotine one, origins included"
  );

  // The certificate, measured here as well as asserted through the plan's own predicate.
  for placement in plan.placements() {
    let footprint = placement.footprint();
    assert!(
      inside(footprint, sheet),
      "part {} at {:?} is outside the sheet",
      placement.part(),
      footprint
    );
    assert_eq!(
      (footprint.width(), footprint.height()),
      (
        parts[placement.part()].width(),
        parts[placement.part()].height()
      ),
      "no part is turned on this fixture"
    );
  }
  for (index, first) in plan.placements().iter().enumerate() {
    for second in &plan.placements()[index + 1..] {
      let measured = clearance(first.footprint(), second.footprint());
      assert!(
        measured >= GAP - EPS,
        "parts {} and {} are {measured} mm apart, the authored gap is {GAP} mm",
        first.part(),
        second.part()
      );
    }
  }
  assert!(plan.certifies(sheet, GAP), "the certificate holds");
}

#[test]
fn guillotine_wins_the_contested_fixture_on_sheets() {
  let sheet = contested_sheet();
  let parts = contested_parts();
  let maxrects = plan_by(sheet, &parts, GAP, NestAlgorithm::MaxRects);
  let guillotine = plan_by(sheet, &parts, GAP, NestAlgorithm::Guillotine);

  assert_eq!(
    (maxrects.unplaced().len(), guillotine.unplaced().len()),
    (0, 0),
    "both heuristics place every part, so the comparison is on sheets alone"
  );
  assert_eq!(
    (maxrects.sheets_used(), guillotine.sheets_used()),
    (2, 1),
    "the same input needs two boards with maxrects and one with guillotine"
  );
  assert!(
    maxrects.certifies(sheet, GAP) && guillotine.certifies(sheet, GAP),
    "both plans are certified, so the difference is packing and not legality"
  );
  assert_ne!(
    maxrects.placements(),
    guillotine.placements(),
    "the two heuristics are not the same algorithm under two names"
  );
  assert!(
    guillotine.utilisation(sheet) > maxrects.utilisation(sheet),
    "the guillotine plan uses the board it opened: {} against {}",
    guillotine.utilisation(sheet),
    maxrects.utilisation(sheet)
  );
}

#[test]
fn the_auto_rule_picks_the_better_of_the_two_and_names_it() {
  let options = NestOptions::new(GAP, false).expect("a valid gap");
  assert_eq!(
    options.algorithm(),
    NestAlgorithm::Auto,
    "the default is the auto rule, so nothing existing changes"
  );

  // The contested fixture: guillotine is better, so the auto rule takes the guillotine plan.
  let auto = NestPlan::new(contested_sheet(), &contested_parts(), options).expect("a plan");
  assert_eq!(
    auto.algorithm(),
    NestAlgorithm::Guillotine,
    "the auto rule names the algorithm it picked"
  );
  assert_eq!(
    auto,
    plan_by(
      contested_sheet(),
      &contested_parts(),
      GAP,
      NestAlgorithm::Guillotine
    ),
    "the auto plan is the guillotine plan"
  );
  assert_eq!(
    auto.sheets_used(),
    1,
    "it picked the plan that needs one board"
  );

  // The cabinetry fixture: the two heuristics reach the same placements, so the tie is real and the
  // auto rule keeps maxrects.
  let cabinetry = cabinetry();
  let tied = NestPlan::new(sheet(), &cabinetry, options).expect("a plan");
  assert_eq!(
    tied,
    plan_by(sheet(), &cabinetry, GAP, NestAlgorithm::MaxRects),
    "the auto plan is the maxrects plan"
  );
  let by_guillotine = plan_by(sheet(), &cabinetry, GAP, NestAlgorithm::Guillotine);
  assert_eq!(
    (tied.placements(), tied.unplaced(), tied.sheets_used()),
    (
      by_guillotine.placements(),
      by_guillotine.unplaced(),
      by_guillotine.sheets_used()
    ),
    "the two heuristics place the cabinetry parts identically, so the tie is a real one"
  );
  assert_eq!(
    tied.algorithm(),
    NestAlgorithm::MaxRects,
    "a tie between the two keeps maxrects, the primary heuristic"
  );
}
