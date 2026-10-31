//! Behaviour tests for the inlay pair (M6 A6, D-M6-7).
//!
//! An inlay is **two** pockets from one command: the female clears the selected region grown by the
//! allowance (the cavity), and the male clears the job rectangle with the region standing as an island
//! (the plug). These tests hold each half to that statement — the female to a pocket of the grown
//! region, the male to a pocket of the job with the region as an island — and hold the pair to the
//! property that makes it an inlay: their clearings meet in the allowance ring and **nowhere else**, so
//! the plug is one allowance smaller than the cavity it fits and neither program touches the other's
//! side.

use cam_geom::offset::{offset_contour, OffsetOptions};
use cam_geom::region::Region;
use cam_geom::{Contour, Point2, Segment};
use cam_model::{InlayParams, Job, PocketParams, PocketStrategy};
use cam_move::Move;
use cam_toolpath::inlay::{female, inlay, male};
use cam_toolpath::{pocket, InlayRequest, PocketRequest, ToolSpec, ToolpathError};
use cam_units::Length;

/// The stock both halves are cut out of: 100 mm square.
const STOCK: f64 = 100.0;
/// The region an inlay is cut from: 40 mm square, centred, so the male has 20 mm of stock around it.
const REGION_MIN: f64 = 20.0;
const REGION_MAX: f64 = 60.0;
/// The tool: 6 mm across, so the ring machinery insets by 3 mm.
const TOOL: f64 = 6.0;

fn job() -> Job {
  Job {
    width: Length::from_mm(STOCK),
    height: Length::from_mm(STOCK),
    ..Job::default_sheet()
  }
}

fn tool() -> ToolSpec {
  ToolSpec {
    number: 1,
    diameter: Length::from_mm(TOOL),
    included_angle: None,
    feed_rate: 100.0,
    plunge_rate: 30.0,
    spindle_speed: 18_000,
  }
}

/// A counter-clockwise square whose first vertex is `(min, min)`.
fn square(min: f64, max: f64) -> Contour {
  Contour::from_points(
    &[
      Point2::new(min, min),
      Point2::new(max, min),
      Point2::new(max, max),
      Point2::new(min, max),
    ],
    true,
  )
  .expect("a square")
}

fn region() -> Contour {
  square(REGION_MIN, REGION_MAX)
}

fn open_vector() -> Contour {
  Contour::from_points(&[Point2::new(10.0, 10.0), Point2::new(80.0, 10.0)], false)
    .expect("an open vector")
}

fn params(allowance: f64, stepover: f64) -> InlayParams {
  InlayParams {
    allowance: Length::from_mm(allowance),
    stepover: Length::from_mm(stepover),
    pass_depth: Length::from_mm(6.0),
    start_depth: Length::ZERO,
    cut_depth: Length::from_mm(6.0),
  }
}

fn request<'a>(
  contours: &'a [&'a Contour],
  params: &'a InlayParams,
  tool: &'a ToolSpec,
  job: &'a Job,
) -> InlayRequest<'a> {
  InlayRequest {
    contours,
    params,
    tool,
    job,
  }
}

/// The pocket parameters a half's clearing is described by, with no inlay allowance in them.
fn pocket_params(stepover: f64, strategy: PocketStrategy) -> PocketParams {
  PocketParams {
    strategy,
    allowance: Length::ZERO,
    stepover: Length::from_mm(stepover),
    pass_depth: Length::from_mm(6.0),
    start_depth: Length::ZERO,
    cut_depth: Length::from_mm(6.0),
  }
}

/// Where every cutting move ends, in document coordinates (the job's origin is its bottom-left corner).
fn cut_points(moves: &[Move]) -> Vec<Point2> {
  moves
    .iter()
    .filter(|movement| matches!(movement, Move::Feed { .. } | Move::Arc { .. }))
    .map(|movement| {
      let point = movement.destination();
      Point2::new(point.x.mm(), point.y.mm())
    })
    .collect()
}

/// Every straight cutting move as a segment, which is what a 6 mm tool sweeps.
fn cut_segments(moves: &[Move]) -> Vec<Segment> {
  let mut segments = Vec::new();
  let mut previous: Option<Point2> = None;
  for movement in moves {
    let destination = movement.destination();
    let point = Point2::new(destination.x.mm(), destination.y.mm());
    match movement {
      Move::Feed { .. } => {
        if let Some(from) = previous {
          if from != point {
            segments.push(Segment::Line(cam_geom::LineSeg::new(from, point)));
          }
        }
      }
      // The fixture is rectangular, so a rectangle's rings and rows are straight: an arc here would
      // mean the strategy had flattened the region, which nothing in this file asks for.
      Move::Arc { .. } => panic!("an inlay on rectangular geometry cuts no arcs"),
      _ => {}
    }
    previous = Some(point);
  }
  segments
}

/// Whether a tool riding `segments` reaches `point` with its own radius.
fn reachable(segments: &[Segment], point: Point2, radius: f64) -> bool {
  segments
    .iter()
    .any(|segment| segment.distance_to_point(point) <= radius + 1.0e-9)
}

#[test]
fn the_female_is_a_pocket_of_the_region_grown_by_the_allowance() {
  // The cavity is the region *grown* by the allowance, cleared by the pocket's own ring machinery: the
  // tool centre runs `radius - allowance` inside the region's boundary, which is `radius` inside the
  // boundary the allowance grows it to. Both statements are the same program, so the second one — a
  // pocket of the explicitly grown contour — is what this test holds the female to.
  let region = region();
  let params = params(2.0, 10.0);
  let job = job();
  let tool = tool();
  let contours = [&region];
  let female_half = female(&request(&contours, &params, &tool, &job)).expect("a female half");

  let grown = offset_contour(&region, 2.0, &OffsetOptions::default())
    .expect("a grown region")
    .contour;
  let expected = pocket(&PocketRequest {
    contour: &grown,
    islands: &[],
    params: &pocket_params(10.0, PocketStrategy::Offset),
    tool: &tool,
    job: &job,
  })
  .expect("a pocket of the grown region")
  .moves;
  assert_eq!(female_half, expected, "the female is a pocket of R + a");

  // And the numbers, so a wrong grown region cannot satisfy this by agreeing with itself: the first
  // ring's corners sit `radius - allowance = 1 mm` inside the region's own 20 mm corner, and the tool it
  // carries reaches exactly the grown boundary at 18 mm.
  let first_ring: Vec<Point2> = cut_points(&female_half)
    .into_iter()
    .filter(|point| (point.x - 21.0).abs() < 1.0e-9 || (point.x - 59.0).abs() < 1.0e-9)
    .collect();
  assert!(
    first_ring.contains(&Point2::new(21.0, 21.0)),
    "the outermost ring starts at the grown region's corner inset by the tool radius: {first_ring:?}"
  );
  let grown_region = Region::new(vec![grown]).expect("a grown region");
  assert!(
    grown_region.contains(Point2::new(18.5, 40.0), 1.0e-9),
    "and the cavity reaches 18 mm, one allowance outside the region"
  );
}

#[test]
fn the_male_is_a_pocket_of_the_job_rectangle_with_the_region_as_an_island() {
  // The plug is what the male leaves standing, so the male clears the job with the region appended as
  // one more boundary of the same region (parity makes it a hole, D-RB-2). Its boundary is the job's own
  // outline (`Job::outline`), never a rectangle built here.
  let region = region();
  let params = params(2.0, 10.0);
  let job = job();
  let tool = tool();
  let contours = [&region];
  let male_half = male(&request(&contours, &params, &tool, &job)).expect("a male half");

  let outline = job.outline();
  let islands = [&region];
  let expected = pocket(&PocketRequest {
    contour: &outline,
    islands: &islands,
    params: &pocket_params(10.0, PocketStrategy::Raster),
    tool: &tool,
    job: &job,
  })
  .expect("a pocket of the job around the region")
  .moves;
  assert_eq!(
    male_half, expected,
    "the male is a pocket of the job with R as an island"
  );

  // Nothing the male cuts enters the region: the plug is the region, wall to wall.
  let region_shape = Region::new(vec![region]).expect("a region");
  for point in cut_points(&male_half) {
    assert!(
      !region_shape.contains(point, 1.0e-9),
      "the male never puts its centre inside the plug: {point:?}"
    );
  }
  // Its innermost ring rides `radius` outside the region's own boundary, which is where the plug's wall
  // is cut.
  assert!(
    cut_points(&male_half)
      .iter()
      .any(|point| (point.x - 17.0).abs() < 1.0e-9),
    "the male's island ring is 3 mm outside the region's 20 mm wall"
  );
}

#[test]
fn the_two_halves_meet_in_the_allowance_ring_and_nowhere_else() {
  // The pair's whole point (A6, D-M6-7): the female clears `R + a` and the male clears `job - R`, so the
  // material both of them remove is the ring between the region and the region grown by the allowance —
  // the clearance the plug needs. A cell inside the region belongs to the female alone, a cell beyond the
  // ring to the male alone, and a cell in the ring to both.
  let region = region();
  let allowance = 2.0;
  let params = params(allowance, 10.0);
  let job = job();
  let tool = tool();
  let contours = [&region];
  let both = inlay(&request(&contours, &params, &tool, &job)).expect("an inlay pair");

  let female_segments = cut_segments(&both.female);
  let male_segments = cut_segments(&both.male);
  let radius = TOOL / 2.0;
  let region_shape = Region::new(vec![region.clone()]).expect("a region");
  let grown = offset_contour(&region, allowance, &OffsetOptions::default())
    .expect("a grown region")
    .contour;
  let grown_shape = Region::new(vec![grown]).expect("a grown region");

  let mut in_ring = 0;
  let mut in_region = 0;
  let mut beyond = 0;
  let mut steps = 0;
  while steps < (STOCK * 2.0) as usize {
    let x = steps as f64 / 2.0 + 0.5;
    let y = steps as f64 / 2.0 + 0.5;
    steps += 1;
    let point = Point2::new(x, y);
    let female_reaches = reachable(&female_segments, point, radius);
    let male_reaches = reachable(&male_segments, point, radius);
    let inside_region = region_shape.contains(point, 1.0e-9);
    let inside_grown = grown_shape.contains(point, 1.0e-9);
    if female_reaches && male_reaches {
      assert!(
        inside_grown && !inside_region,
        "both halves clear {point:?}, which is not in the allowance ring"
      );
      in_ring += 1;
    }
    if female_reaches {
      assert!(
        inside_grown,
        "the female clears {point:?}, which is outside the cavity R + a"
      );
    }
    if male_reaches {
      assert!(
        !inside_region,
        "the male clears {point:?}, which is inside the plug"
      );
    }
    if inside_region {
      in_region += 1;
    } else if !inside_grown {
      beyond += 1;
    }
  }
  assert!(
    in_ring > 0,
    "the allowance ring is where the two halves must meet, and nothing is there"
  );
  assert!(in_region > 0 && beyond > 0, "both sides of the pair exist");
}

#[test]
fn a_zero_allowance_is_the_plain_pocket_of_the_region_cell_for_cell() {
  // M5's rule for the pocket's own allowance, applied to the inlay's: zero must not perturb a single
  // cell. The female becomes a plain pocket of the region, move for move; the male still clears the job
  // around it, because the plug is the region whatever the clearance is.
  let region = region();
  let params = params(0.0, 10.0);
  let job = job();
  let tool = tool();
  let contours = [&region];
  let both = inlay(&request(&contours, &params, &tool, &job)).expect("an inlay pair");

  let plain = pocket(&PocketRequest {
    contour: &region,
    islands: &[],
    params: &pocket_params(10.0, PocketStrategy::Offset),
    tool: &tool,
    job: &job,
  })
  .expect("a plain pocket")
  .moves;
  assert_eq!(both.female, plain, "a zero allowance is the plain pocket");
  assert!(
    !both.male.is_empty(),
    "and the male still has the plug to leave standing"
  );
}

#[test]
fn an_open_contour_is_refused_by_name() {
  // A cavity is a region's, and an open vector bounds none: the refusal is `OpenContour`, the sentence
  // every closed-geometry strategy already speaks, and it comes before anything is cut.
  let open = open_vector();
  let params = params(2.0, 10.0);
  let job = job();
  let tool = tool();
  let contours = [&open];
  let request = request(&contours, &params, &tool, &job);
  assert_eq!(female(&request), Err(ToolpathError::OpenContour));
  assert_eq!(male(&request), Err(ToolpathError::OpenContour));
  assert_eq!(inlay(&request), Err(ToolpathError::OpenContour));
}

#[test]
fn an_allowance_that_collapses_the_region_is_refused_by_name() {
  // A negative allowance is an interference fit — a cavity smaller than the plug — and a large enough one
  // shaves the 40 mm region away entirely. That is an input error naming the allowance, not a pocket that
  // "does not fit": the tool is not what made the region disappear.
  let region = region();
  let job = job();
  let tool = tool();
  let contours = [&region];
  let collapsed = params(-25.0, 10.0);
  let error = inlay(&request(&contours, &collapsed, &tool, &job)).expect_err("must refuse");
  match &error {
    ToolpathError::InlayAllowanceCollapsed { allowance, .. } => {
      assert_eq!(*allowance, -25.0, "{error:?}");
      assert!(
        error.to_string().contains("-25"),
        "the refusal names the allowance: {error}"
      );
    }
    other => panic!("expected a collapsed allowance, got {other:?}"),
  }

  // A fit that still leaves material is cut, so the refusal is the collapse and not the sign.
  let fitted = params(-2.0, 10.0);
  inlay(&request(&contours, &fitted, &tool, &job)).expect("an interference fit is an inlay too");
}

#[test]
fn a_region_that_is_not_strictly_inside_the_stock_is_refused_by_name() {
  // The male clears the stock *around* the region, so a region that crosses the stock's edge has nothing
  // beside it to clear: refused naming the job, rather than cut as a plug bounded by the saw cut.
  let job = job();
  let tool = tool();
  let crossing = square(90.0, 110.0);
  let touching = square(0.0, 40.0);
  let params = params(2.0, 10.0);
  for region in [crossing, touching] {
    let contours = [&region];
    let error = inlay(&request(&contours, &params, &tool, &job)).expect_err("must refuse");
    assert!(
      matches!(error, ToolpathError::InlayRegionOutsideJob { .. }),
      "{region:?}: {error:?}"
    );
    assert!(
      error.to_string().contains("100 x 100"),
      "the refusal names the stock: {error}"
    );
  }
}

#[test]
fn both_halves_inherit_the_pockets_own_refusals() {
  // The clearing laws are the pocket's, so its refusals are too — named, and asked before any move is
  // produced: a stepover with no distance behind it, and a depth range with no pass in it.
  let region = region();
  let job = job();
  let tool = tool();
  let contours = [&region];

  let no_stepover = params(2.0, 0.0);
  for half in [
    female(&request(&contours, &no_stepover, &tool, &job)),
    male(&request(&contours, &no_stepover, &tool, &job)),
  ] {
    assert_eq!(half, Err(ToolpathError::InvalidStepover { stepover: 0.0 }));
  }

  let no_range = InlayParams {
    start_depth: Length::from_mm(6.0),
    ..params(2.0, 10.0)
  };
  for half in [
    female(&request(&contours, &no_range, &tool, &job)),
    male(&request(&contours, &no_range, &tool, &job)),
  ] {
    assert_eq!(
      half,
      Err(ToolpathError::InvalidDepthRange {
        cut_depth: 6.0,
        start_depth: 6.0,
      })
    );
  }
}

#[test]
fn two_calls_for_one_request_emit_the_same_pair() {
  // S6: the pair is two pockets in a fixed order over explicit vectors, so the second call is the first
  // one — the property a `toolpath` command's byte-identical output rests on.
  let region = region();
  let params = params(2.0, 7.0);
  let job = job();
  let tool = tool();
  let contours = [&region];
  let first = inlay(&request(&contours, &params, &tool, &job)).expect("an inlay pair");
  let second = inlay(&request(&contours, &params, &tool, &job)).expect("an inlay pair");
  assert_eq!(first, second);
  assert!(!first.female.is_empty() && !first.male.is_empty());
}
