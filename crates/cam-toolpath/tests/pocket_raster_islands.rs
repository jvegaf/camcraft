//! Behaviour tests for the raster pocket over a region with islands (A13).
//!
//! Every assertion is made through the move stream and the returned summary, never through the
//! strategy's internals: what the machine is told to do is the contract. A row is cut as one straight
//! span per piece of material it crosses, so a row that crosses an island must arrive as **two**
//! spans, and the ends of those spans must be the true line/arc intersections — never a tessellated
//! approximation of them.

use cam_geom::{Contour, Point2};
use cam_model::{Job, OriginCorner, PocketParams, PocketStrategy};
use cam_move::Move;
use cam_toolpath::{pocket, PocketRequest, Pocketpath, ToolSpec};
use cam_units::Length;

/// Tolerance for a coordinate the kernel computes analytically.
const TOLERANCE: f64 = 1.0e-6;

/// Tolerance for a raster row's Y, in millimetres.
///
/// A row that lands exactly on a boundary edge or on a tangency is ambiguous, so the strategy retries
/// it a hair inside (see `ROW_RETRY_NUDGE` in the implementation). The grid itself is exact and the
/// retry is far below machine resolution.
const ROW_TOLERANCE: f64 = 1.0e-4;

/// How far apart the samples of a cutting move are, in millimetres.
///
/// A cutting move is a straight span, so sampling it this finely leaves at most one spacing of it
/// unmeasured — four times finer than the 6 mm tool whose clearance the test asserts.
const CUT_STEP: f64 = 0.25;

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
    strategy: PocketStrategy::Raster,
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

/// Split the move stream into units of work: each unit opens with a rapid, plunges, cuts, retracts.
///
/// One raster span is one unit, so a row across the region is one unit and a row across an island is
/// two.
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
        if previous.map_or(true, |last| !last.is_near(to, 1.0e-9)) {
          points.push(to);
          previous = Some(to);
        }
      }
      Move::Retract { .. } => {}
    }
  }
  points
}

/// The straight pieces a unit cuts, as consecutive pairs of its XY path.
fn cut_spans(unit: &[Move]) -> Vec<(Point2, Point2)> {
  let path = cut_path(unit);
  path.windows(2).map(|pair| (pair[0], pair[1])).collect()
}

/// The single span a one-piece row cuts, with its ends ordered left to right.
fn row_span(unit: &[Move]) -> (Point2, Point2) {
  let spans = cut_spans(unit);
  assert_eq!(spans.len(), 1, "this row is one piece of material");
  ordered(spans[0])
}

/// A span's ends ordered left to right.
fn ordered(span: (Point2, Point2)) -> (Point2, Point2) {
  if span.0.x <= span.1.x {
    span
  } else {
    (span.1, span.0)
  }
}

/// The shortest distance from `point` to a closed contour, measured on its segments (arcs exact).
fn distance_to(contour: &Contour, point: Point2) -> f64 {
  contour
    .segments()
    .iter()
    .map(|segment| segment.distance_to_point(point))
    .fold(f64::INFINITY, f64::min)
}

/// Points along a straight span, both of its ends included.
fn sampled(span: (Point2, Point2), step: f64) -> Vec<Point2> {
  let length = span.0.distance_to(span.1);
  let steps = ((length / step).ceil() as usize).max(1);
  (0..=steps)
    .map(|index| span.0.lerp(span.1, index as f64 / steps as f64))
    .collect()
}

/// The `y` a row was really cut at, read off the move stream.
fn row_y(unit: &[Move]) -> f64 {
  let (start, end) = row_span(unit);
  assert!(
    (start.y - end.y).abs() < TOLERANCE,
    "a raster row is horizontal, got {} and {}",
    start.y,
    end.y
  );
  start.y
}

#[test]
fn a_row_crossing_an_island_is_cut_as_two_spans() {
  // A 100 mm square with a centred r10 island, 6 mm tool, 3 mm stepover. The tool centre rides 3 mm
  // inside the wall, so the outer region is 3…97 and the island grows to r = 10 + 3 = 13 about
  // (50, 50). Rows sit on the M4 grid y = 3 + 3k while they fit the region, so k = 0…31: **32 rows**,
  // exactly the island-free row count in `tests/pocket.rs`, because the island is inside the boundary
  // and cannot move it.
  let boundary = square(100.0);
  let island = circle(Point2::new(50.0, 50.0), 10.0);
  let path = run(&boundary, &[&island], 3.0);

  assert_eq!(path.rings, 32, "the row grid is the island-free grid");

  // A row crosses the grown island where |y - 50| < 13, which on this grid is y = 39, 42, … 60:
  // k = 12…19, **8 rows**. Those cut **two** spans; the other 24 cut one. y = 63 is exactly tangent
  // (|63 - 50| = 13), so the kernel refuses it as ambiguous and the strategy's retry resolves it just
  // outside the island: it still cuts one span.
  let expected_spans: Vec<usize> = (0..32)
    .map(|k| if (12..=19).contains(&k) { 2 } else { 1 })
    .collect();
  assert_eq!(
    expected_spans.iter().sum::<usize>(),
    40,
    "24 single-span rows plus 8 two-span rows"
  );

  let units = work_units(&path.moves);
  assert_eq!(units.len(), 40, "one work unit per span");

  let mut unit = 0usize;
  for (k, span_count) in expected_spans.iter().enumerate() {
    let y = 3.0 + 3.0 * k as f64;
    let first = &units[unit];
    assert!(
      (row_y(first) - y).abs() < ROW_TOLERANCE,
      "row {k} should lie at y = {y}, got {}",
      row_y(first)
    );
    if *span_count == 1 {
      let (start, end) = row_span(first);
      assert!(
        (start.x - 3.0).abs() < TOLERANCE && (end.x - 97.0).abs() < TOLERANCE,
        "row {k} misses the island and spans x 3…97, got {}…{}",
        start.x,
        end.x
      );
      unit += 1;
      continue;
    }

    // The two spans of an island row, left to right — the row's direction alternates, so they are
    // ordered by their left end instead of read off the move order. The first ends on the circle's
    // left crossing and the second starts on its right one, both analytic.
    let left = ordered(cut_spans(first)[0]);
    let right = ordered(cut_spans(&units[unit + 1])[0]);
    let (left, right) = if left.0.x <= right.0.x {
      (left, right)
    } else {
      (right, left)
    };
    let half = (13.0f64.powi(2) - (y - 50.0).powi(2)).sqrt();
    assert!(
      (left.0.x - 3.0).abs() < TOLERANCE,
      "row {k} starts on the wall, got {}",
      left.0.x
    );
    assert!(
      (left.1.x - (50.0 - half)).abs() < TOLERANCE,
      "row {k} should end on the island at x = {}, got {}",
      50.0 - half,
      left.1.x
    );
    assert!(
      (right.0.x - (50.0 + half)).abs() < TOLERANCE,
      "row {k} should resume at x = {}, got {}",
      50.0 + half,
      right.0.x
    );
    assert!(
      (right.1.x - 97.0).abs() < TOLERANCE,
      "row {k} ends on the far wall, got {}",
      right.1.x
    );
    for span in [left, right] {
      assert!(
        (span.0.y - y).abs() < TOLERANCE && (span.1.y - y).abs() < TOLERANCE,
        "both spans of row {k} lie at y = {y}"
      );
    }
    unit += 2;
  }
  assert_eq!(unit, units.len(), "the walk covers every unit");
}

#[test]
fn no_cutting_move_passes_through_the_island() {
  // The same fixture: the tool centre must never come closer than its own radius to the island, so
  // the cut leaves the island standing instead of cutting it away.
  let boundary = square(100.0);
  let island = circle(Point2::new(50.0, 50.0), 10.0);
  let path = run(&boundary, &[&island], 3.0);

  let units = work_units(&path.moves);
  assert!(!units.is_empty(), "the pocket is cut");
  for (index, unit) in units.iter().enumerate() {
    for span in cut_spans(unit) {
      for point in sampled(span, CUT_STEP) {
        let clearance = distance_to(&island, point);
        assert!(
          clearance >= 3.0 - TOLERANCE,
          "unit {index} cuts {point:?}, only {clearance} mm from the island: the tool radius is 3"
        );
      }
    }
  }
}

#[test]
fn a_row_the_island_covers_entirely_emits_nothing() {
  // A 100 mm square split by a full-width island band: the island spans x 4…96, so once it grows by
  // the 3 mm inset it reaches past the shrunken wall on both sides and the material is two bands —
  // y 3…37 and y 63…97. Rows live on y = 3 + 3k as before, so 32 rows fit; the 8 of them that fall
  // **inside** the island band (y = 39, 42, … 60) cross no material at all and are not cut. The
  // arithmetic the test asserts is 32 - 8 = 24 rows and 24 spans.
  let boundary = square(100.0);
  let island = rectangle(Point2::new(4.0, 40.0), Point2::new(96.0, 60.0));
  let path = run(&boundary, &[&island], 3.0);

  assert_eq!(path.rings, 24, "32 grid rows less the 8 the island covers");

  let units = work_units(&path.moves);
  assert_eq!(units.len(), 24, "every surviving row is one span");
  let rows: Vec<f64> = units.iter().map(|unit| row_y(unit)).collect();

  for (index, y) in rows.iter().enumerate() {
    assert!(
      *y < 37.0 || *y > 63.0,
      "row {index} at y = {y} is inside the band the island covers and must not be cut"
    );
    let (start, end) = row_span(&units[index]);
    assert!(
      (start.x - 3.0).abs() < TOLERANCE && (end.x - 97.0).abs() < TOLERANCE,
      "row {index} should span the full wall, got {}…{}",
      start.x,
      end.x
    );
  }

  // The surviving rows are the 12 below the band and the 12 above it, on the same grid.
  let expected: Vec<f64> = (0..12)
    .map(|k| 3.0 + 3.0 * k as f64)
    .chain((0..12).map(|k| 63.0 + 3.0 * k as f64))
    .collect();
  for (index, expected) in expected.iter().enumerate() {
    assert!(
      (rows[index] - expected).abs() < ROW_TOLERANCE,
      "surviving row {index} should sit at y = {expected}, got {}",
      rows[index]
    );
  }
}

#[test]
fn a_row_touching_a_narrow_island_strip_never_emits_an_unpaired_span() {
  // A narrow strip (x 40…60, y 20…80) grows by the 3 mm inset to x 37…63, y 17…83. A 7 mm stepover
  // puts a row exactly on the grown strip's lower edge at y = 17: the row is collinear with a straight
  // boundary segment, the kernel refuses it as ambiguous, and the strategy's retry resolves it just
  // above the edge — two paired spans, never one unpaired crossing.
  let boundary = square(100.0);
  let island = rectangle(Point2::new(40.0, 20.0), Point2::new(60.0, 80.0));
  let path = run(&boundary, &[&island], 7.0);

  // Rows at y = 3 + 7k fit the 3…97 region up to k = 13: **14 rows**. Ten of them cross the strip —
  // y = 17, 24, … 80 (k = 2…11), the first of those the retried tangent row — and cut 2 spans each.
  // The other four (y = 3, 10, 87, 94) miss it and cut 1 span each: **24 spans**.
  let expected_spans: Vec<usize> = vec![1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 1];
  assert_eq!(
    expected_spans.iter().sum::<usize>(),
    24,
    "10 two-span rows and 4 single-span rows"
  );

  let units = work_units(&path.moves);
  assert_eq!(path.rings, 14, "14 rows fit the grid");
  assert_eq!(units.len(), 24, "one work unit per span");

  let mut unit = 0usize;
  for (k, span_count) in expected_spans.iter().enumerate() {
    let y = 3.0 + 7.0 * k as f64;
    let first = &units[unit];
    assert!(
      (row_y(first) - y).abs() < ROW_TOLERANCE,
      "row {k} should sit at y = {y}, got {}",
      row_y(first)
    );
    if *span_count == 1 {
      let (start, end) = row_span(first);
      assert!(
        (start.x - 3.0).abs() < TOLERANCE && (end.x - 97.0).abs() < TOLERANCE,
        "row {k} misses the strip, got {}…{}",
        start.x,
        end.x
      );
    } else {
      let left = ordered(cut_spans(&units[unit])[0]);
      let right = ordered(cut_spans(&units[unit + 1])[0]);
      let (left, right) = if left.0.x <= right.0.x {
        (left, right)
      } else {
        (right, left)
      };
      assert!(
        (left.1.x - 37.0).abs() < TOLERANCE && (right.0.x - 63.0).abs() < TOLERANCE,
        "row {k} should stop at x = 37 and resume at x = 63, got {} and {}",
        left.1.x,
        right.0.x
      );
      for span in [left, right] {
        assert!(
          span.1.x - span.0.x > 0.0,
          "row {k} emitted an unpaired or empty span: {span:?}"
        );
      }
    }
    unit += span_count;
  }
  assert_eq!(unit, units.len(), "the walk covers every unit");

  // The ambiguous row itself is never emitted at the exact tangent height.
  for unit in &units {
    let (start, _) = row_span(unit);
    assert!(
      (start.y - 17.0).abs() > 1.0e-9,
      "the ambiguous row must be retried, not emitted as-is"
    );
  }
}

#[test]
fn raster_pocket_with_an_island_is_deterministic() {
  // Two identical requests must produce the same program, move for move (S5).
  let boundary = square(100.0);
  let island = circle(Point2::new(50.0, 50.0), 10.0);
  let first = run(&boundary, &[&island], 3.0);
  let second = run(&boundary, &[&island], 3.0);

  assert_eq!(first.rings, second.rings);
  assert_eq!(first.flattened, second.flattened);
  assert_eq!(
    first.moves, second.moves,
    "identical input, identical moves"
  );
  assert_eq!(work_units(&first.moves).len(), 40);
}
