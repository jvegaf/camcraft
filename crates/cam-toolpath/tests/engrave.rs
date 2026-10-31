//! Behaviour tests for quick engrave (M6 A3, D-M6-4).
//!
//! A quick engrave is the profile's `On` side with the pass law removed (D-M6-4): one pass at the
//! authored `cut_depth`, the tool centre **on** the vector, no stepdown, no lead-in or lead-out and
//! no tabs. The one thing it accepts and the profile refuses is an **open** vector — engraving a
//! single line is the strategy's point — so the open case gets its own test here.

use cam_geom::{Contour, Point2};
use cam_model::{EngraveParams, Job};
use cam_move::{Move, Point3};
use cam_toolpath::{engrave, EngraveRequest, ToolSpec, ToolpathError};
use cam_units::{Angle, Length};

fn rectangle(width: f64, height: f64) -> Contour {
  Contour::from_points(
    &[
      Point2::new(0.0, 0.0),
      Point2::new(width, 0.0),
      Point2::new(width, height),
      Point2::new(0.0, height),
    ],
    true,
  )
  .expect("rectangle")
}

fn open_polyline() -> Contour {
  Contour::from_points(
    &[
      Point2::new(0.0, 0.0),
      Point2::new(10.0, 0.0),
      Point2::new(10.0, 5.0),
    ],
    false,
  )
  .expect("open polyline")
}

fn job() -> Job {
  Job::default_sheet()
}

fn tool(diameter: f64) -> ToolSpec {
  ToolSpec {
    number: 1,
    diameter: Length::from_mm(diameter),
    included_angle: Some(Angle::from_degrees(90.0)),
    feed_rate: 100.0,
    plunge_rate: 30.0,
    spindle_speed: 18_000,
  }
}

fn params(cut_depth: f64, start_depth: f64) -> EngraveParams {
  EngraveParams {
    start_depth: Length::from_mm(start_depth),
    cut_depth: Length::from_mm(cut_depth),
  }
}

fn request<'a>(
  contour: &'a Contour,
  params: &'a EngraveParams,
  tool: &'a ToolSpec,
  job: &'a Job,
) -> EngraveRequest<'a> {
  EngraveRequest {
    contour,
    params,
    tool,
    job,
  }
}

fn run(contour: &Contour, params: &EngraveParams, tool: &ToolSpec) -> Vec<Move> {
  engrave(&request(contour, params, tool, &job())).expect("toolpath computes")
}

/// Z values of the plunges: a vertical feed straight after a rapid, going down.
fn plunge_depths(moves: &[Move]) -> Vec<f64> {
  let mut depths = Vec::new();
  let mut previous = Point3::ORIGIN;
  let mut after_rapid = false;
  for movement in moves {
    let destination = movement.destination();
    if let Move::Feed { .. } = movement {
      if after_rapid
        && destination.xy().is_near(previous.xy(), 1.0e-9)
        && destination.z.mm() < previous.z.mm()
      {
        depths.push(destination.z.mm());
      }
    }
    after_rapid = matches!(movement, Move::Rapid { .. });
    previous = destination;
  }
  depths
}

/// The XY of every cutting move, in order.
fn cutting_points(moves: &[Move]) -> Vec<Point2> {
  moves
    .iter()
    .filter(|movement| matches!(movement, Move::Feed { .. } | Move::Arc { .. }))
    .map(|movement| movement.destination().xy())
    .collect()
}

#[test]
fn the_path_is_the_contour_itself_at_the_authored_depth() {
  // The tool centre runs on the 20 x 10 rectangle exactly — no offset anywhere — at the one depth the
  // document authored, because an engrave is the profile's `On` side with the pass law removed
  // (D-M6-4).
  let contour = rectangle(20.0, 10.0);
  let engine = params(3.0, 0.0);
  let moves = run(&contour, &engine, &tool(6.0));

  assert_eq!(
    moves.first(),
    Some(&Move::Rapid {
      to: Point3::new(0.0, 0.0, 5.0),
    }),
    "the pass opens with a rapid over the contour's first vertex, at safe Z"
  );
  assert_eq!(
    moves.get(1),
    Some(&Move::Feed {
      to: Point3::new(0.0, 0.0, -3.0),
    }),
    "then plunges in place to the authored depth"
  );
  assert_eq!(
    cutting_points(&moves),
    vec![
      Point2::new(0.0, 0.0),
      Point2::new(20.0, 0.0),
      Point2::new(20.0, 10.0),
      Point2::new(0.0, 10.0),
      Point2::new(0.0, 0.0),
    ],
    "the tool's own path is the contour's vertices, unoffset"
  );
  let last = moves.last().expect("a move");
  assert!(
    matches!(last, Move::Retract { to } if to.z.mm() == 5.0),
    "the pass ends back at safe Z, got {last:?}"
  );

  // The tool's diameter moves nothing: a profile would be offset by half of it, an engrave is not.
  assert_eq!(
    cutting_points(&run(&contour, &engine, &tool(12.0))),
    cutting_points(&moves),
    "a 12 mm bit follows the same path as a 6 mm one"
  );
}

#[test]
fn a_closed_contour_comes_back_to_its_start() {
  // Engraving a closed vector grooves the whole loop and stops where it began: the walk is the
  // contour's own, so the last cutting move lands on the first vertex.
  let contour = rectangle(20.0, 10.0);
  let moves = run(&contour, &params(2.0, 0.0), &tool(6.0));
  let points = cutting_points(&moves);
  assert_eq!(
    points.last(),
    points.first(),
    "a closed contour closes the loop: {points:?}"
  );
  assert!(
    points.iter().all(|point| point.y >= 0.0 && point.y <= 10.0),
    "and stays on the contour: {points:?}"
  );
}

#[test]
fn an_open_contour_is_engraved_from_its_first_vertex_to_its_last() {
  // The case a profile refuses and an engrave exists for: a single open vector. It starts at the
  // first vertex, stops at the last, and retracts there — nothing closes the loop on the user's
  // behalf.
  let contour = open_polyline();
  let moves = run(&contour, &params(2.0, 0.0), &tool(6.0));

  assert_eq!(
    cutting_points(&moves),
    vec![
      Point2::new(0.0, 0.0),
      Point2::new(10.0, 0.0),
      Point2::new(10.0, 5.0),
    ],
    "the open vector is cut from its first vertex to its last, and no further"
  );
  assert_eq!(
    moves.last(),
    Some(&Move::Retract {
      to: Point3::new(10.0, 5.0, 5.0),
    }),
    "and the tool lifts straight up at the far end"
  );
}

#[test]
fn an_engrave_is_one_pass_however_deep_the_cut_is() {
  // A pass law is a liability when the feature is a single V-groove (D-M6-4): a second pass at a
  // deeper Z would cut a second groove. So the pass count is one whatever the depth.
  let contour = rectangle(20.0, 10.0);
  for depth in [1.0, 12.0] {
    let moves = run(&contour, &params(depth, 0.0), &tool(6.0));
    assert_eq!(
      plunge_depths(&moves),
      vec![-depth],
      "a {depth} mm engrave is one plunge, at the authored depth"
    );
    // Rapid, plunge, four sides, retract: one pass around a four-segment contour.
    assert_eq!(moves.len(), 7, "one pass, no stepdown and no tab machinery");
  }
}

#[test]
fn the_start_depth_only_has_to_be_above_the_cut_depth() {
  // A part already machined to 2 mm takes a 5 mm engrave as one pass that ends at -5: the start
  // depth is where the pass begins, not something added to it.
  let contour = rectangle(20.0, 10.0);
  assert_eq!(
    plunge_depths(&run(&contour, &params(5.0, 2.0), &tool(6.0))),
    vec![-5.0]
  );
}

#[test]
fn a_cut_depth_at_or_below_the_start_depth_is_refused() {
  // The profile's own rule (D-M6-4), naming both depths so the message says which pair is wrong.
  let contour = rectangle(20.0, 10.0);
  for (cut_depth, start_depth) in [(3.0, 3.0), (2.0, 3.0)] {
    assert_eq!(
      engrave(&request(
        &contour,
        &params(cut_depth, start_depth),
        &tool(6.0),
        &job()
      )),
      Err(ToolpathError::InvalidDepthRange {
        cut_depth,
        start_depth,
      }),
      "cut {cut_depth} against start {start_depth}"
    );
  }
}
