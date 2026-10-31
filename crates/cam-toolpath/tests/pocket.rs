//! Behaviour tests for the pocket toolpath (A3, A4, A5, A6).
//!
//! Every assertion is made through the move stream, never through the module's internals: what the
//! machine is told to do is the contract.

use std::f64::consts::{FRAC_PI_2, PI};

use cam_geom::{Arc, Contour, LineSeg, Point2, Segment};
use cam_model::{Job, OriginCorner, PocketParams, PocketStrategy};
use cam_move::Move;
use cam_toolpath::{pocket, PocketRequest, Pocketpath, ToolSpec, ToolpathError};
use cam_units::Length;

const TOLERANCE: f64 = 1.0e-6;

/// Tolerance for a raster row's Y, in millimetres.
///
/// A row that lands exactly on the region's lower edge is ambiguous (it is collinear with a boundary
/// segment), so the strategy retries it a hair inside — see `ROW_RETRY_NUDGE` in the implementation.
/// The grid itself is exact, and the retry is far below machine resolution.
const ROW_TOLERANCE: f64 = 1.0e-4;

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

/// A counter-clockwise rounded rectangle: four lines joined by four quarter arcs.
fn rounded_rectangle(width: f64, height: f64, radius: f64) -> Contour {
  let (w, h, r) = (width, height, radius);
  let arc = |center: Point2, start: f64| {
    Segment::Arc(Arc::new(center, r, start, FRAC_PI_2).expect("quarter arc"))
  };
  let segments = vec![
    Segment::Line(LineSeg::new(Point2::new(r, 0.0), Point2::new(w - r, 0.0))),
    arc(Point2::new(w - r, r), -FRAC_PI_2),
    Segment::Line(LineSeg::new(Point2::new(w, r), Point2::new(w, h - r))),
    arc(Point2::new(w - r, h - r), 0.0),
    Segment::Line(LineSeg::new(Point2::new(w - r, h), Point2::new(r, h))),
    arc(Point2::new(r, h - r), FRAC_PI_2),
    Segment::Line(LineSeg::new(Point2::new(0.0, h - r), Point2::new(0.0, r))),
    arc(Point2::new(r, r), PI),
  ];
  Contour::from_segments(&segments, true).expect("rounded rectangle")
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

fn params(
  strategy: PocketStrategy,
  allowance: f64,
  stepover: f64,
  pass_depth: f64,
  cut_depth: f64,
) -> PocketParams {
  PocketParams {
    strategy,
    allowance: Length::from_mm(allowance),
    stepover: Length::from_mm(stepover),
    pass_depth: Length::from_mm(pass_depth),
    start_depth: Length::ZERO,
    cut_depth: Length::from_mm(cut_depth),
  }
}

fn run(contour: &Contour, params: &PocketParams) -> Pocketpath {
  run_with(contour, params, &tool(6.0))
}

fn run_with(contour: &Contour, params: &PocketParams, tool: &ToolSpec) -> Pocketpath {
  pocket(&PocketRequest {
    contour,
    islands: &[],
    params,
    tool,
    job: &job(),
  })
  .expect("toolpath computes")
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

/// The Z of a unit's plunge: the first feed after its rapid.
fn plunge_z(unit: &[Move]) -> f64 {
  unit
    .iter()
    .find_map(|movement| match movement {
      Move::Feed { to } => Some(to.z.mm()),
      _ => None,
    })
    .expect("every unit plunges")
}

/// The XY path a unit cuts: its opening position, then every horizontal cutting destination.
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
      Move::Feed { .. } | Move::Arc { .. } => {
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
///
/// A closed ring therefore has as many pieces as the contour has segments, and a raster span has one.
fn cut_spans(unit: &[Move]) -> Vec<(Point2, Point2)> {
  let path = cut_path(unit);
  path.windows(2).map(|pair| (pair[0], pair[1])).collect()
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

/// The distance from a point to a straight span of material: the tool's reach is a disc about it.
fn distance_to_span(point: Point2, start: Point2, end: Point2) -> f64 {
  let clamped_x = point.x.clamp(start.x.min(end.x), start.x.max(end.x));
  Point2::new(clamped_x, start.y).distance_to(point)
}

/// How many cell centres of an `extent` square at a 1 mm lattice lie within `radius` of a span the
/// toolpath cuts: the area the tool really clears, counted the way the simulation counts it.
fn covered_cells(units: &[Vec<Move>], radius: f64, extent: usize) -> usize {
  let mut count = 0;
  for row in 0..extent {
    for column in 0..extent {
      let centre = Point2::new(column as f64 + 0.5, row as f64 + 0.5);
      let covered = units.iter().any(|unit| {
        cut_spans(unit)
          .iter()
          .any(|(start, end)| distance_to_span(centre, *start, *end) <= radius)
      });
      if covered {
        count += 1;
      }
    }
  }
  count
}

fn arc_moves(unit: &[Move]) -> usize {
  unit
    .iter()
    .filter(|movement| matches!(movement, Move::Arc { .. }))
    .count()
}

#[test]
fn offset_square_cuts_twelve_rings_at_expected_insets() {
  // 6 mm tool, 4 mm stepover: insets 3, 7, … 47, then the 51 mm inset no longer fits a 100 mm
  // square and ends the ringing.
  let path = run(
    &square(100.0),
    &params(PocketStrategy::Offset, 0.0, 4.0, 20.0, 12.0),
  );

  assert_eq!(path.rings, 12, "a 100 mm square at 4 mm stepover");
  let units = work_units(&path.moves);
  assert_eq!(units.len(), 12, "one ring per unit, one depth pass");

  for (index, unit) in units.iter().enumerate() {
    let inset = 3.0 + 4.0 * index as f64;
    let spans = cut_spans(unit);
    assert_eq!(
      spans.len(),
      4,
      "ring {index} is a rectangle: four straight cutting moves"
    );
    let points: Vec<Point2> = spans.iter().flat_map(|(a, b)| [*a, *b]).collect();
    let (min_x, min_y, max_x, max_y) = xy_bounds(&points);
    let side = 100.0 - 2.0 * inset;
    assert!(
      (min_x - inset).abs() < TOLERANCE
        && (min_y - inset).abs() < TOLERANCE
        && (max_x - (100.0 - inset)).abs() < TOLERANCE
        && (max_y - (100.0 - inset)).abs() < TOLERANCE,
      "ring {index} should sit at inset {inset}, got {min_x}..{max_x} x {min_y}..{max_y}"
    );
    assert!(
      (max_x - min_x - side).abs() < TOLERANCE && (max_y - min_y - side).abs() < TOLERANCE,
      "ring {index} should be a {side} mm square"
    );
    assert_eq!(arc_moves(unit), 0, "a square ring has no arcs");
  }
}

#[test]
fn raster_square_cuts_rows_at_stepover_spacing() {
  // 6 mm tool, 3 mm stepover on a 100 mm square: the tool centre spans 3…97, so rows start at
  // y = 3 and step by 3 while they stay inside the region.
  let path = run(
    &square(100.0),
    &params(PocketStrategy::Raster, 0.0, 3.0, 20.0, 12.0),
  );

  assert_eq!(path.rings, 32, "a 94 mm span at 3 mm stepover");
  let units = work_units(&path.moves);
  assert_eq!(units.len(), 32, "one span per row on a convex square");

  for (index, unit) in units.iter().enumerate() {
    let y = 3.0 + 3.0 * index as f64;
    let spans = cut_spans(unit);
    assert_eq!(spans.len(), 1, "row {index} is one straight piece");
    let (start, end) = spans[0];
    assert!(
      (start.y - y).abs() < ROW_TOLERANCE && (end.y - y).abs() < ROW_TOLERANCE,
      "row {index} should lie at y = {y}, got {} and {}",
      start.y,
      end.y
    );
    let (min_x, _, max_x, _) = xy_bounds(&[start, end]);
    assert!(
      (min_x - 3.0).abs() < TOLERANCE && (max_x - 97.0).abs() < TOLERANCE,
      "row {index} should span x 3…97, got {min_x}…{max_x}"
    );
    assert_eq!(arc_moves(unit), 0, "raster rows are straight");
  }

  // A uniform grid from the region's lower edge: the last row is at 3 + 31 * 3 = 96, one step
  // below the region's upper edge at 97.
  let last = units.last().expect("at least one row");
  let (start, _) = cut_spans(last)[0];
  assert!(
    (start.y - 96.0).abs() < ROW_TOLERANCE,
    "the last row sits at y = 96, got {}",
    start.y
  );
}

#[test]
fn a_raster_row_on_the_region_far_edge_is_cut() {
  // A 2 mm tool at a 2 mm stepover on a 100 mm square, no allowance: the tool centre rides 1 mm
  // inside the square, so the region it rasters is 1…99 in both axes, and the row lattice from its
  // low edge is 1, 3, … 99 — **50** rows, the last one lying exactly on the region's far edge.
  //
  // A row on a boundary is material the region's edge sits on, not an empty row: it is the row that
  // clears the strip the edge is the end of. Dropping it puts the last row at 97, and a 2 mm tool at
  // y = 97 reaches y = 98 — so the 2 mm strip from 98 to the square's edge is left standing, which
  // on a 1 mm lattice is 200 cells at 6 mm deep = 1200 mm3 (what M5's simulation oracle measured:
  // `removed: 58800.000 mm3`, `uncut: 1200.000 mm3 over 200 cell(s)`, exit 6).
  let path = run_with(
    &square(100.0),
    &params(PocketStrategy::Raster, 0.0, 2.0, 20.0, 6.0),
    &tool(2.0),
  );

  assert_eq!(path.rings, 50, "a 98 mm span at a 2 mm stepover: 50 rows");
  let units = work_units(&path.moves);
  assert_eq!(units.len(), 50, "one span per row on a convex square");

  for (index, unit) in units.iter().enumerate() {
    let y = 1.0 + 2.0 * index as f64;
    let spans = cut_spans(unit);
    assert_eq!(spans.len(), 1, "row {index} is one straight piece");
    let (start, end) = spans[0];
    assert!(
      (start.y - y).abs() < ROW_TOLERANCE && (end.y - y).abs() < ROW_TOLERANCE,
      "row {index} should lie at y = {y}, got {} and {}",
      start.y,
      end.y
    );
    let (min_x, _, max_x, _) = xy_bounds(&[start, end]);
    assert!(
      (min_x - 1.0).abs() < TOLERANCE && (max_x - 99.0).abs() < TOLERANCE,
      "row {index} should span x 1…99, got {min_x}…{max_x}"
    );
  }

  // The last row is the far edge itself, cut a hair inside it rather than dropped.
  let last = units.last().expect("at least one row");
  let (start, _) = cut_spans(last)[0];
  assert!(
    (start.y - 99.0).abs() < ROW_TOLERANCE,
    "the last row sits on the region's far edge at y = 99, got {}",
    start.y
  );

  // And the pocket covers the whole square: every one of the 10000 cell centres of a 1 mm lattice is
  // within the 1 mm the tool reaches, so the covered area is 100 x 100 = 10000 mm2 and the analytic
  // volume at 6 mm deep is 60000 mm3. Without the far-edge row it is 9800 cells and 58800 mm3.
  let covered = covered_cells(&units, 1.0, 100);
  assert_eq!(covered, 10_000, "the raster clears the whole 100 mm square");
  let volume = covered as f64 * 1.0 * 1.0 * 6.0;
  assert_eq!(volume, 60_000.0, "10000 cells x 1 mm2 x 6 mm deep");
}

#[test]
fn depth_passes_chunk_the_cut_and_cut_every_ring_once() {
  let path = run(
    &square(100.0),
    &params(PocketStrategy::Offset, 0.0, 4.0, 5.0, 12.0),
  );

  assert_eq!(path.rings, 12, "the ring count ignores depth passes");
  let units = work_units(&path.moves);
  assert_eq!(units.len(), 36, "3 passes over 12 rings");
  let depths: Vec<f64> = units.iter().map(|unit| plunge_z(unit)).collect();
  let mut expected = Vec::new();
  for depth in [-5.0, -10.0, -12.0] {
    for _ in 0..12 {
      expected.push(depth);
    }
  }
  assert_eq!(depths, expected, "passes are [5, 10, 12], shallow to deep");

  // Every ring is cut once per pass, in the same order and at the same place.
  for index in 0..12 {
    assert_eq!(
      cut_spans(&units[index]),
      cut_spans(&units[index + 12]),
      "ring {index} repeats in the second pass"
    );
    assert_eq!(
      cut_spans(&units[index]),
      cut_spans(&units[index + 24]),
      "ring {index} repeats in the third pass"
    );
  }
}

#[test]
fn allowance_insets_every_ring_and_row_by_one_millimetre() {
  let offset = run(
    &square(100.0),
    &params(PocketStrategy::Offset, 1.0, 4.0, 20.0, 12.0),
  );
  assert_eq!(offset.rings, 12);
  let units = work_units(&offset.moves);
  let first: Vec<Point2> = cut_spans(&units[0])
    .iter()
    .flat_map(|(a, b)| [*a, *b])
    .collect();
  let (min_x, min_y, max_x, max_y) = xy_bounds(&first);
  assert!((min_x - 4.0).abs() < TOLERANCE, "min x {min_x}");
  assert!((min_y - 4.0).abs() < TOLERANCE, "min y {min_y}");
  assert!((max_x - 96.0).abs() < TOLERANCE, "max x {max_x}");
  assert!((max_y - 96.0).abs() < TOLERANCE, "max y {max_y}");
  let second: Vec<Point2> = cut_spans(&units[1])
    .iter()
    .flat_map(|(a, b)| [*a, *b])
    .collect();
  let (second_min_x, _, _, _) = xy_bounds(&second);
  assert!(
    (second_min_x - 8.0).abs() < TOLERANCE,
    "rings stay one stepover apart, got {second_min_x}"
  );

  let raster = run(
    &square(100.0),
    &params(PocketStrategy::Raster, 1.0, 3.0, 20.0, 12.0),
  );
  let units = work_units(&raster.moves);
  let (start, end) = cut_spans(&units[0])[0];
  let (min_x, _, max_x, _) = xy_bounds(&[start, end]);
  assert!((min_x - 4.0).abs() < TOLERANCE, "row min x {min_x}");
  assert!((max_x - 96.0).abs() < TOLERANCE, "row max x {max_x}");
  assert!((start.y - 4.0).abs() < ROW_TOLERANCE, "row y {}", start.y);
}

#[test]
fn tool_wider_than_the_pocket_is_refused_as_offset_collapsed() {
  // A 4 mm tool needs a 2 mm inset, which cannot fit a 3 mm wide pocket in either mode.
  let narrow = square(3.0);
  for strategy in [PocketStrategy::Offset, PocketStrategy::Raster] {
    let error = pocket(&PocketRequest {
      contour: &narrow,
      islands: &[],
      params: &params(strategy, 0.0, 3.0, 6.0, 12.0),
      tool: &tool(4.0),
      job: &job(),
    })
    .expect_err("the tool does not fit");
    assert!(
      matches!(error, ToolpathError::OffsetCollapsed { .. }),
      "expected OffsetCollapsed for {strategy:?}, got {error:?}"
    );
  }

  // The spec's 5 mm example does fit: a 2 mm inset leaves a 1 mm ring, so it is cut, not refused.
  let snug = square(5.0);
  let path = run_with(
    &snug,
    &params(PocketStrategy::Offset, 0.0, 3.0, 6.0, 12.0),
    &tool(4.0),
  );
  assert_eq!(path.rings, 1, "a 1 mm ring is the only one that fits");
  let points: Vec<Point2> = cut_spans(&work_units(&path.moves)[0])
    .iter()
    .flat_map(|(a, b)| [*a, *b])
    .collect();
  let (min_x, _, max_x, _) = xy_bounds(&points);
  assert!((min_x - 2.0).abs() < TOLERANCE && (max_x - 3.0).abs() < TOLERANCE);
}

#[test]
fn circle_raster_spans_match_the_analytic_chord() {
  // A 40 mm circle with a 6 mm tool leaves a 17 mm radius for the tool centre.
  let circle = cam_geom::primitive::circle(Point2::new(50.0, 50.0), 20.0).expect("circle");
  let path = run(
    &circle,
    &params(PocketStrategy::Raster, 0.0, 4.0, 20.0, 12.0),
  );
  let units = work_units(&path.moves);
  assert_eq!(path.rings, units.len(), "one span per row");
  assert_eq!(units.len(), 9, "rows at y = 33, 37, … 65");

  for unit in &units {
    let spans = cut_spans(unit);
    assert_eq!(spans.len(), 1, "a row across a circle is one span");
    let (start, end) = spans[0];
    assert!(
      (start.y - end.y).abs() < TOLERANCE,
      "a row is horizontal, got {} and {}",
      start.y,
      end.y
    );
    let y = start.y;
    let half_chord = (17.0f64.powi(2) - (y - 50.0).powi(2)).max(0.0).sqrt();
    let width = (end.x - start.x).abs();
    assert!(
      (width - 2.0 * half_chord).abs() < TOLERANCE,
      "row at y = {y} should be {} mm wide, got {width}",
      2.0 * half_chord
    );
    assert!(
      ((start.x + end.x) / 2.0 - 50.0).abs() < TOLERANCE,
      "the span is centred on the inset circle"
    );
  }
}

#[test]
fn tangent_raster_row_never_emits_an_unpaired_span() {
  // Rows start exactly at the region's lower edge, which is tangent to the inset circle: the row is
  // ambiguous, so it is retried a hair inside, never emitted unpaired.
  let circle = cam_geom::primitive::circle(Point2::new(50.0, 50.0), 20.0).expect("circle");
  let path = run(
    &circle,
    &params(PocketStrategy::Raster, 0.0, 4.0, 20.0, 12.0),
  );

  let units = work_units(&path.moves);
  assert!(!units.is_empty(), "the rows inside the circle are cut");
  for unit in &units {
    let spans = cut_spans(unit);
    assert_eq!(spans.len(), 1, "every span is a single paired piece");
    let (start, end) = spans[0];
    assert!(
      (start.y - end.y).abs() < TOLERANCE && (start.x - end.x).abs() > 0.0,
      "a span is a horizontal piece of material, not a point"
    );
    for point in [start, end] {
      let radius = point.distance_to(Point2::new(50.0, 50.0));
      assert!(
        (radius - 17.0).abs() < TOLERANCE,
        "span end {point:?} must lie on the inset circle, radius {radius}"
      );
    }
  }

  // The tangent row itself is never cut as a zero-length move at the exact tangent height.
  for unit in &units {
    for point in cut_path(unit) {
      assert!(
        (point.y - 33.0).abs() > 1.0e-9,
        "the ambiguous row must not be emitted as-is"
      );
    }
  }
}

#[test]
fn arc_shapes_keep_arcs_in_offset_rings_and_none_in_raster_rows() {
  // A 10 mm corner radius allows two rings at a 3 mm inset and a 4 mm stepover.
  let shape = rounded_rectangle(40.0, 30.0, 10.0);

  let offset = run(
    &shape,
    &params(PocketStrategy::Offset, 0.0, 4.0, 20.0, 12.0),
  );
  assert_eq!(offset.rings, 2, "insets 3 and 7, then the corners collapse");
  let units = work_units(&offset.moves);
  for (index, unit) in units.iter().enumerate() {
    assert_eq!(
      arc_moves(unit),
      4,
      "ring {index} keeps its four corner arcs as arcs"
    );
    assert_eq!(
      cut_spans(unit).len(),
      8,
      "ring {index} is still four lines and four arcs"
    );
  }

  let raster = run(
    &shape,
    &params(PocketStrategy::Raster, 0.0, 5.0, 20.0, 12.0),
  );
  let units = work_units(&raster.moves);
  assert!(!units.is_empty(), "the shape is rastered");
  for (index, unit) in units.iter().enumerate() {
    assert_eq!(
      arc_moves(unit),
      0,
      "raster row {index} is a straight piece of material"
    );
  }
}

#[test]
fn zero_stepover_is_refused() {
  for strategy in [PocketStrategy::Offset, PocketStrategy::Raster] {
    let error = pocket(&PocketRequest {
      contour: &square(100.0),
      islands: &[],
      params: &params(strategy, 0.0, 0.0, 6.0, 12.0),
      tool: &tool(6.0),
      job: &job(),
    })
    .expect_err("a zero stepover leaves material between rings");
    assert!(
      matches!(error, ToolpathError::InvalidStepover { .. }),
      "expected InvalidStepover, got {error:?}"
    );
  }
}

#[test]
fn open_contour_is_refused() {
  let open =
    Contour::from_points(&[Point2::new(0.0, 0.0), Point2::new(10.0, 0.0)], false).expect("open");
  for strategy in [PocketStrategy::Offset, PocketStrategy::Raster] {
    let error = pocket(&PocketRequest {
      contour: &open,
      islands: &[],
      params: &params(strategy, 0.0, 3.0, 6.0, 12.0),
      tool: &tool(6.0),
      job: &job(),
    })
    .expect_err("a pocket needs a closed region");
    assert_eq!(error, ToolpathError::OpenContour);
  }
}

#[test]
fn every_cutting_move_is_at_its_pass_depth() {
  let path = run(
    &square(100.0),
    &params(PocketStrategy::Offset, 0.0, 4.0, 5.0, 12.0),
  );
  let safe_z = job().safe_z.mm();
  let mut seen_depths = Vec::new();
  for unit in work_units(&path.moves) {
    let depth = plunge_z(&unit);
    seen_depths.push(depth);
    for movement in &unit {
      match movement {
        Move::Feed { to } | Move::Arc { to, .. } => {
          let z = to.z.mm();
          assert!(
            (z - depth).abs() < 1.0e-9,
            "a cutting move must sit at its pass depth {depth}, got {z}"
          );
        }
        Move::Rapid { to } | Move::Retract { to } => {
          assert!(
            (to.z.mm() - safe_z).abs() < 1.0e-9,
            "a rapid or retract must sit at safe Z"
          );
        }
      }
    }
  }
  seen_depths.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
  seen_depths.dedup_by(|a, b| (*a - *b).abs() < 1.0e-9);
  assert_eq!(seen_depths, vec![-12.0, -10.0, -5.0], "exact pass depths");
}

#[test]
fn invalid_tool_depth_and_range_are_refused() {
  let cases: Vec<(ToolSpec, PocketParams, ToolpathError)> = vec![
    (
      tool(0.0),
      params(PocketStrategy::Offset, 0.0, 3.0, 6.0, 12.0),
      ToolpathError::InvalidToolDiameter,
    ),
    (
      tool(6.0),
      params(PocketStrategy::Offset, 0.0, 3.0, 0.0, 12.0),
      ToolpathError::InvalidPassDepth,
    ),
    (
      tool(6.0),
      params(PocketStrategy::Offset, 0.0, 3.0, 6.0, 0.0),
      ToolpathError::InvalidDepthRange {
        cut_depth: 0.0,
        start_depth: 0.0,
      },
    ),
  ];
  for (tool, params, expected) in cases {
    let error = pocket(&PocketRequest {
      contour: &square(100.0),
      islands: &[],
      params: &params,
      tool: &tool,
      job: &job(),
    })
    .expect_err("refused");
    assert_eq!(error, expected);
  }
}
