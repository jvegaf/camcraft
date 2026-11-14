//! Behaviour tests for the offset pocket over a region with islands (A12).
//!
//! Every assertion is made through the move stream and the returned summary, never through the
//! strategy's internals: what the machine is told to do is the contract.

use cam_geom::{Contour, Point2};
use cam_model::{Job, OriginCorner};
use cam_move::Move;
use cam_ops::{pocket, PocketRequest, Pocketpath, ToolSpec, ToolpathError};
use cam_ops::{PocketParams, PocketStrategy};
use cam_units::Length;

/// Tolerance for a distance the offset kernel guarantees to within its own slack.
///
/// `pocket.rs` inherits the kernel's 0.002 mm proof slack; a ring vertex can sit that far inside the
/// exact offset curve, so a distance assertion allows exactly that much and no more.
const SLACK: f64 = 0.002;

/// How far apart the samples of a cutting move are, in millimetres.
///
/// An arc move's bulge is not a vertex, so asserting over move *destinations* would miss exactly the
/// defect K5 fixes. A move is walked at this spacing instead, which leaves at most one spacing of it
/// unmeasured — a thousandth of the 6 mm gouge these tests pin, and well under the kernel's own slack.
const CUT_STEP: f64 = 0.005;

fn square(size: f64) -> Contour {
  Contour::from_points(
    &[
      Point2::new(0.0, 0.0),
      Point2::new(size, 0.0),
      Point2::new(size, size),
      Point2::new(0.0, size),
    ],
    true,
  )
  .expect("square")
}

fn circle(centre: Point2, radius: f64) -> Contour {
  cam_geom::primitive::circle(centre, radius).expect("circle")
}

fn rectangle(min: Point2, max: Point2) -> Contour {
  Contour::from_points(
    &[
      min,
      Point2::new(max.x, min.y),
      max,
      Point2::new(min.x, max.y),
    ],
    true,
  )
  .expect("rectangle")
}

/// The U bracket the kernel's own offset test uses for a narrow neck: a 100 mm base with two 15 mm
/// wide arms. Shrinking by more than half an arm's width makes that arm's two offset walls cross,
/// which is the one path that rebuilds a contour as straight segments.
fn bracket() -> Contour {
  cam_geom::primitive::polyline(
    &[
      Point2::new(0.0, 0.0),
      Point2::new(100.0, 0.0),
      Point2::new(100.0, 100.0),
      Point2::new(85.0, 100.0),
      Point2::new(85.0, 40.0),
      Point2::new(15.0, 40.0),
      Point2::new(15.0, 100.0),
      Point2::new(0.0, 100.0),
    ],
    true,
  )
  .expect("bracket")
}

fn job() -> Job {
  Job {
    origin: OriginCorner::BottomLeft,
    width: Length::from_mm(200.0),
    height: Length::from_mm(200.0),
    ..Job::default_sheet()
  }
}

fn tool(diameter: f64) -> ToolSpec {
  ToolSpec {
    number: 1,
    diameter: Length::from_mm(diameter),
    included_angle: None,
    feed_rate: 100.0,
    plunge_rate: 30.0,
    spindle_speed: 18_000,
  }
}

fn params(stepover: f64) -> PocketParams {
  PocketParams {
    strategy: PocketStrategy::Offset,
    allowance: Length::ZERO,
    stepover: Length::from_mm(stepover),
    pass_depth: Length::from_mm(20.0),
    start_depth: Length::ZERO,
    cut_depth: Length::from_mm(12.0),
  }
}

fn run(boundary: &Contour, islands: &[&Contour], stepover: f64) -> Pocketpath {
  let params = params(stepover);
  let tool = tool(6.0);
  let job = job();
  pocket(&PocketRequest {
    contour: boundary,
    islands,
    params: &params,
    tool: &tool,
    job: &job,
  })
  .expect("toolpath computes")
}

fn run_error(
  boundary: &Contour,
  islands: &[&Contour],
  stepover: f64,
  diameter: f64,
) -> ToolpathError {
  let params = params(stepover);
  let tool = tool(diameter);
  let job = job();
  pocket(&PocketRequest {
    contour: boundary,
    islands,
    params: &params,
    tool: &tool,
    job: &job,
  })
  .expect_err("refused")
}

/// Split the move stream into units of work: each unit opens with a rapid, plunges, cuts, retracts.
fn work_units(moves: &[Move]) -> Vec<Vec<Move>> {
  let mut work_units: Vec<Vec<Move>> = Vec::new();
  for movement in moves {
    if work_units.is_empty() || matches!(movement, Move::Rapid { .. }) {
      work_units.push(Vec::new());
    }
    if let Some(current) = work_units.last_mut() {
      current.push(*movement);
    }
  }
  work_units
}

/// The XY positions a unit visits, its opening position included.
fn cut_path(unit: &[Move]) -> Vec<Point2> {
  let mut points = Vec::new();
  let mut previous: Option<Point2> = None;
  for movement in unit {
    let to = movement.destination().xy();
    match movement {
      Move::Rapid { .. } => {
        points.push(to);
        previous = Some(to);
      }
      Move::Feed { .. } | Move::Arc { .. } | Move::Helix { .. } => {
        if previous.map_or(true, |last| {
          !last.is_near(to, cam_geom::TolerancePolicy::uniform(1.0e-9))
        }) {
          points.push(to);
          previous = Some(to);
        }
      }
      Move::Retract { .. } => {}
    }
  }
  points
}

fn arc_moves(unit: &[Move]) -> usize {
  unit
    .iter()
    .filter(|movement| matches!(movement, Move::Arc { .. }))
    .count()
}

/// The straight pieces a unit cuts, as consecutive pairs of its XY path.
fn cut_spans(unit: &[Move]) -> Vec<(Point2, Point2)> {
  let path = cut_path(unit);
  path.windows(2).map(|pair| (pair[0], pair[1])).collect()
}

/// Every XY position a unit actually **cuts through**, its arcs' interiors included.
///
/// A move carries where it ends, not the curve it travels, so an arc's bulge is invisible to
/// [`cut_path`] — which is exactly the bug K5 fixes, and a test that reused `cut_path` could not see
/// it either. An arc move carries its centre and its direction, and the machine cuts the shorter way
/// round (that is the arc G-code means), so that is the arc sampled here, at a spacing of [`CUT_STEP`].
fn cut_points(unit: &[Move]) -> Vec<Point2> {
  let mut points = Vec::new();
  let mut previous: Point2 = Point2::new(0.0, 0.0);
  let mut cutting = false;
  for movement in unit {
    let to = movement.destination().xy();
    match movement {
      Move::Rapid { .. } => {
        cutting = false;
      }
      Move::Retract { .. } => {
        cutting = false;
      }
      Move::Feed { .. } | Move::Helix { .. } => {
        if cutting {
          let span = previous.distance_to(to);
          let steps = ((span / CUT_STEP).ceil() as usize).max(1);
          for index in 1..=steps {
            points.push(previous.lerp(to, index as f64 / steps as f64));
          }
        } else {
          points.push(to);
        }
        cutting = true;
      }
      Move::Arc { center, ccw, .. } => {
        let radius = previous.distance_to(*center);
        let start = (previous.y - center.y).atan2(previous.x - center.x);
        let end = (to.y - center.y).atan2(to.x - center.x);
        let sweep = if *ccw {
          wrap_turn(end - start)
        } else {
          -wrap_turn(start - end)
        };
        assert!(
          sweep.abs() <= std::f64::consts::PI + 1.0e-9,
          "an emitted arc is the shorter way round: sweep {sweep}"
        );
        let steps = ((radius * sweep.abs() / CUT_STEP).ceil() as usize).max(1);
        for index in 1..=steps {
          let angle = start + sweep * index as f64 / steps as f64;
          points.push(Point2::new(
            center.x + radius * angle.cos(),
            center.y + radius * angle.sin(),
          ));
        }
        cutting = true;
      }
    }
    previous = to;
  }
  points
}

/// An angle reduced to `[0, TAU)`.
fn wrap_turn(angle: f64) -> f64 {
  let turn = std::f64::consts::TAU;
  let wrapped = angle % turn;
  if wrapped < 0.0 {
    wrapped + turn
  } else {
    wrapped
  }
}

fn xy_bounds(points: &[Point2]) -> (f64, f64, f64, f64) {
  let mut bounds = (
    f64::INFINITY,
    f64::INFINITY,
    f64::NEG_INFINITY,
    f64::NEG_INFINITY,
  );
  for point in points {
    bounds.0 = bounds.0.min(point.x);
    bounds.1 = bounds.1.min(point.y);
    bounds.2 = bounds.2.max(point.x);
    bounds.3 = bounds.3.max(point.y);
  }
  bounds
}

/// The shortest distance from `point` to a closed contour, measured on its segments (arcs exact).
fn distance_to(contour: &Contour, point: Point2) -> f64 {
  contour
    .segments()
    .iter()
    .map(|segment| segment.distance_to_point(point))
    .fold(f64::INFINITY, f64::min)
}

/// The stage distances a ring set was cut at, deduped and sorted.
///
/// A ring is offset from the **original** boundaries by `inset + k * stepover`, and the kernel keeps
/// only rings that prove that distance, so the closest a ring's vertices come to the union of the
/// originals *is* its stage distance. Reading the stages back this way checks the whole ring set
/// against the arithmetic the test states without depending on how many pieces a stage was cut in.
fn stages(units: &[Vec<Move>], originals: &[&Contour]) -> Vec<f64> {
  let mut found: Vec<f64> = units
    .iter()
    .map(|unit| {
      cut_path(unit)
        .iter()
        .map(|point| {
          originals
            .iter()
            .map(|contour| distance_to(contour, *point))
            .fold(f64::INFINITY, f64::min)
        })
        .fold(f64::INFINITY, f64::min)
    })
    .collect();
  found.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
  found.dedup_by(|a, b| (*a - *b).abs() < 1.0e-6);
  found
}

fn assert_stages(units: &[Vec<Move>], originals: &[&Contour], expected: &[f64]) {
  let found = stages(units, originals);
  assert_eq!(
    found.len(),
    expected.len(),
    "expected the stages {expected:?}, got {found:?}"
  );
  for (got, want) in found.iter().zip(expected) {
    assert!(
      (got - want).abs() < 1.0e-6,
      "expected the stages {expected:?}, got {found:?}"
    );
  }
}

/// Assert that no cutting move of `unit` comes closer than its stage's distance to either original,
/// and return how close it comes.
///
/// The unit's own closest approach *is* its stage distance — the kernel keeps a ring only when it
/// proves that distance — so the stage is read back off the measurement and then every point of every
/// move, arcs' interiors included, has to clear it. This is the assertion that fails for the 6 mm
/// gouge of D-RB-10, because the offending piece's two ends sat exactly on the stage distance while
/// its arc between them did not.
fn assert_no_cut_is_too_close(unit: &[Move], originals: &[&Contour], stages: &[f64]) -> f64 {
  let points = cut_points(unit);
  let clearance_at = |point: Point2| {
    originals
      .iter()
      .map(|contour| distance_to(contour, point))
      .fold(f64::INFINITY, f64::min)
  };
  let closest = points
    .iter()
    .map(|point| clearance_at(*point))
    .fold(f64::INFINITY, f64::min);
  let stage = stages
    .iter()
    .copied()
    .min_by(|a, b| {
      (closest - a)
        .abs()
        .partial_cmp(&(closest - b).abs())
        .expect("finite distances")
    })
    .expect("a stage to belong to");
  assert!(
    (closest - stage).abs() < 1.0e-6,
    "a ring cut at {closest} mm is not one of the stages {stages:?}"
  );
  for point in &points {
    let clearance = clearance_at(*point);
    assert!(
      clearance >= stage - SLACK - CUT_STEP,
      "a cutting position {point:?} is {clearance} mm from the originals, inside the {stage} mm stage"
    );
  }
  closest
}

#[test]
fn an_island_pocket_cuts_rings_that_follow_both_boundaries() {
  // A 100 mm square with a centred r10 island, a 6 mm tool (so the tool centre rides 3 mm inside the
  // wall and 3 mm outside the island) and a 4 mm stepover.
  let boundary = square(100.0);
  let island = circle(Point2::new(50.0, 50.0), 10.0);
  let path = run(&boundary, &[&island], 4.0);
  let inset = 3.0;

  assert!(!path.flattened, "a square and a circle rebuild nothing");
  let units = work_units(&path.moves);
  assert_eq!(path.rings, units.len(), "one ring per unit, one depth pass");

  // The first stage at inset 3: the wall ring is a 94 mm square, the island ring is a circle of
  // radius 10 + 3 grown out of the island.
  let wall = cut_spans(&units[0]);
  assert_eq!(wall.len(), 4, "the wall ring is a rectangle");
  let points: Vec<Point2> = wall.iter().flat_map(|(a, b)| [*a, *b]).collect();
  let (min_x, min_y, max_x, max_y) = xy_bounds(&points);
  assert!(
    (min_x - inset).abs() < 1.0e-6
      && (min_y - inset).abs() < 1.0e-6
      && (max_x - (100.0 - inset)).abs() < 1.0e-6
      && (max_y - (100.0 - inset)).abs() < 1.0e-6,
    "the wall ring sits at inset {inset}, got {min_x}..{max_x} x {min_y}..{max_y}"
  );
  assert_eq!(arc_moves(&units[0]), 0, "a square ring has no arcs");

  let island_ring = cut_path(&units[1]);
  assert_eq!(arc_moves(&units[1]), 2, "the island ring is two arcs");
  for point in &island_ring {
    let radius = point.distance_to(Point2::new(50.0, 50.0));
    assert!(
      (radius - (10.0 + inset)).abs() < 1.0e-6,
      "the island ring grows the island: expected radius {}, got {radius}",
      10.0 + inset
    );
  }

  // No cutting position comes closer to the island than the tool radius plus allowance: the island
  // is left standing, and no ring passes through it.
  for unit in &units {
    for point in cut_path(unit) {
      let clearance = distance_to(&island, point);
      assert!(
        clearance >= inset - SLACK,
        "a cutting position {point:?} is {clearance} mm from the island, less than the {inset} mm inset"
      );
    }
  }

  // Every ring's vertices prove their stage distance from **both** originals, and the stages are
  // exactly the offset sequence: inset + k * stepover.
  //
  // The arithmetic of where it ends: the wall ring's half-width is 50 - d and the island ring's
  // radius is 10 + d, so the two are still separate while 10 + d < 50 - d, i.e. d < 20 — the last
  // separate stage is d = 19. At d = 23 the grown island (r33) has crossed the shrunken wall
  // (23..77) and the stage is cut as its corner slivers; at d = 27 the wall's own inset (23 mm from
  // the wall) is nearer than 27, nothing is further than 27 from both originals, and the kernel
  // refuses.
  let originals: [&Contour; 2] = [&boundary, &island];
  let stages = [3.0, 7.0, 11.0, 15.0, 19.0, 23.0];
  assert_stages(&units, &originals, &stages);

  // The ring count is the analytic one, not a floor.
  //
  // 5 stages × 2 rings + 4 slivers = 14. The five two-ring stages are d = 3, 7, 11, 15 and 19, each
  // a wall inset and an island growth. At d = 23 the r33 island covers every edge of the shrunken
  // 23..77 wall but not its corners — a corner is 27·√2 = 38.18 from the centre, outside the disc —
  // so parity leaves exactly four corner slivers there. Parity *also* claims the two lenses where the
  // disc sticks out of the wall (below y = 23 and above y = 77), and those are the pieces D-RB-10 is
  // about: their crossings sit exactly 23 mm from both originals while the arc between them comes to
  // y = 17, 6 mm inside the stage's distance. The sound proof drops them, so the stage is four pieces
  // and not six.
  assert_eq!(
    path.rings, 14,
    "5 two-ring stages and the crossing stage's 4 corner slivers, got {}",
    path.rings
  );

  // And no cutting move anywhere in the program comes closer to either original than the distance its
  // own stage claims — measured over the emitted moves, arc interiors included.
  let mut closest = f64::INFINITY;
  for unit in &units {
    closest = closest.min(assert_no_cut_is_too_close(unit, &originals, &stages));
  }
  assert!(
    (closest - 3.0).abs() < 1.0e-6,
    "the closest the program comes is its first inset, 3 mm, got {closest}"
  );
}

#[test]
fn the_ring_count_matches_the_analytic_count() {
  // The same pocket at an 8 mm stepover, so the growing island never crosses the shrinking wall and
  // every stage is exactly two rings: the wall ring and the island ring.
  //
  // Arithmetic. Stage k is at d = 3 + 8k. A stage has material while the grown island (radius 10 + d)
  // is still strictly inside the shrunken wall (half-width 50 - d):
  //   10 + d < 50 - d  <=>  d < 20  =>  d in {3, 11, 19}   (k = 0, 1, 2)
  // That is 3 stages x 2 rings = 6 rings. At d = 27 (k = 3) the grown island has radius 37 and the
  // shrunken wall's corner is 23·sqrt(2) = 32.5 from the centre, so the island has swallowed the
  // pocket and the kernel leaves no material: the ringing ends there. A stage that leaves nothing is
  // not an error once k > 0 — only k = 0 means the tool never fitted.
  let boundary = square(100.0);
  let island = circle(Point2::new(50.0, 50.0), 10.0);
  let path = run(&boundary, &[&island], 8.0);
  let units = work_units(&path.moves);

  assert_eq!(path.rings, 6, "3 stages x 2 rings");
  assert_eq!(units.len(), 6, "one unit per ring, one depth pass");
  assert!(!path.flattened, "nothing here had to be rebuilt");

  let originals: [&Contour; 2] = [&boundary, &island];
  assert_stages(&units, &originals, &[3.0, 11.0, 19.0]);
  for stage in [0usize, 1, 2] {
    let wall = cut_spans(&units[stage * 2]);
    assert_eq!(wall.len(), 4, "stage {stage} starts with the wall ring");
    let points: Vec<Point2> = wall.iter().flat_map(|(a, b)| [*a, *b]).collect();
    let (min_x, _, max_x, _) = xy_bounds(&points);
    let distance = 3.0 + 8.0 * stage as f64;
    assert!(
      (min_x - distance).abs() < 1.0e-6 && (max_x - (100.0 - distance)).abs() < 1.0e-6,
      "stage {stage}'s wall ring sits at {distance}, got {min_x}..{max_x}"
    );
    assert_eq!(
      arc_moves(&units[stage * 2 + 1]),
      2,
      "stage {stage} follows the island with two arcs"
    );
  }
}

#[test]
fn an_island_that_leaves_no_material_is_refused() {
  // A 100 x 20 bar with a 90 x 10 island: at a 3 mm inset the island grows 3 mm and the walls shrink
  // 3 mm, so the grown island (2..98 x 2..18) reaches outside the shrunken bar (3..97 x 3..17).
  // Every candidate ring is then closer than the inset to one of the two originals — the island
  // ring's corners are 2 mm from the bar's walls — and the kernel leaves nothing.
  let boundary = rectangle(Point2::new(0.0, 0.0), Point2::new(100.0, 20.0));
  let island = rectangle(Point2::new(5.0, 5.0), Point2::new(95.0, 15.0));
  let error = run_error(&boundary, &[&island], 4.0, 6.0);
  let ToolpathError::OffsetCollapsed { reason } = &error else {
    panic!("an island that leaves no material is a collapse, got {error:?}");
  };
  assert!(
    reason.contains("no material"),
    "the kernel's own reason is reported verbatim: {reason}"
  );
}

#[test]
fn a_pocket_narrower_than_the_tool_is_refused_through_the_region() {
  // The M4 defect case, now going through the region kernel: a 4 mm tool needs a 2 mm inset and a
  // 3 mm square has no point 2 mm inside it. `offset_contour` alone hands back a well-formed 1 mm
  // ring, which is exactly why the proof lives in the kernel (D-RB-9).
  let error = run_error(&square(3.0), &[], 3.0, 4.0);
  assert!(
    matches!(error, ToolpathError::OffsetCollapsed { .. }),
    "expected OffsetCollapsed, got {error:?}"
  );

  // The same refusal when the request carries an island, so the region path is the one refusing.
  let island = circle(Point2::new(1.5, 1.5), 0.4);
  let error = run_error(&square(3.0), &[&island], 3.0, 4.0);
  assert!(
    matches!(error, ToolpathError::OffsetCollapsed { .. }),
    "expected OffsetCollapsed with an island too, got {error:?}"
  );
}

#[test]
fn a_narrow_neck_reports_flattened() {
  // The bracket's 15 mm arms at an 8 mm stage: each arm's two offset walls cross, and removing that
  // self-intersection is the one path that rebuilds a contour as straight segments. The pocket has to
  // report it rather than hand back a program that quietly lost its arcs.
  let path = run(&bracket(), &[], 5.0);
  assert!(path.rings >= 1, "the base is still cut");
  assert!(
    path.flattened,
    "the arms forced a rebuild and the path says so"
  );
}

#[test]
fn the_program_is_deterministic_across_two_runs() {
  let boundary = square(100.0);
  let island = circle(Point2::new(50.0, 50.0), 10.0);
  let first = run(&boundary, &[&island], 4.0);
  let second = run(&boundary, &[&island], 4.0);

  assert_eq!(
    first.moves, second.moves,
    "identical input, identical moves"
  );
  assert_eq!(first.rings, second.rings);
  assert_eq!(first.flattened, second.flattened);

  // And the same holds for a region whose ring is rebuilt (the narrow neck), where the chaining order
  // could otherwise leak into the program.
  let neck = bracket();
  let first = run(&neck, &[], 5.0);
  let second = run(&neck, &[], 5.0);
  assert_eq!(first.moves, second.moves);
  assert_eq!(first.flattened, second.flattened);
}

#[test]
fn an_open_island_is_refused_as_an_open_contour() {
  // An island is a region boundary too: an open one encloses nothing, and the pocket refuses it with
  // the reason it reserves for closed-geometry requests instead of a kernel message about regions.
  let open =
    Contour::from_points(&[Point2::new(40.0, 50.0), Point2::new(60.0, 50.0)], false).expect("open");
  let params = params(4.0);
  let tool = tool(6.0);
  let job = job();
  let boundary = square(100.0);
  let error = pocket(&PocketRequest {
    contour: &boundary,
    islands: &[&open],
    params: &params,
    tool: &tool,
    job: &job,
  })
  .expect_err("refused");
  assert_eq!(error, ToolpathError::OpenContour);
}
