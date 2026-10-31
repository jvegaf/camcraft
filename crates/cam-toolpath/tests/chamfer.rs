//! Behaviour tests for the chamfer toolpath (A2, A9, D-M6-2 as amended in L7).
//!
//! The depth is always written here from the geometry — `width / tan(half angle)`, plus the start
//! depth — and never taken from `ChamferParams::depth()`. A test that read the depth back from the
//! function under test would agree with any derivation it was given, including a wrong one.

use cam_geom::{Contour, Point2};
use cam_model::{ChamferParams, Job};
use cam_move::{Move, Point3};
use cam_toolpath::{chamfer, ChamferRequest, ToolSpec, ToolpathError};
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

fn params(width: f64, degrees: f64, start_depth: f64) -> ChamferParams {
  ChamferParams {
    chamfer_width: Length::from_mm(width),
    included_angle: Angle::from_degrees(degrees),
    start_depth: Length::from_mm(start_depth),
  }
}

fn request<'a>(
  contour: &'a Contour,
  params: &'a ChamferParams,
  tool: &'a ToolSpec,
  job: &'a Job,
) -> ChamferRequest<'a> {
  ChamferRequest {
    contour,
    params,
    tool,
    job,
  }
}

/// The depth a bevel of `width` needs for a bit of `degrees`, written from the geometry.
fn derived_depth(width: f64, degrees: f64, start_depth: f64) -> f64 {
  start_depth + width / (Angle::from_degrees(degrees).radians() / 2.0).tan()
}

fn run(contour: &Contour, params: &ChamferParams, tool: &ToolSpec) -> Vec<Move> {
  chamfer(&request(contour, params, tool, &job())).expect("toolpath computes")
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
fn the_path_is_the_contour_itself_at_the_derived_depth() {
  // A 3 mm bevel on a 90 degree bit: `3 / tan(45) = 3` mm. The tip runs on the 20 x 10 rectangle
  // exactly — no offset anywhere — because the bevel is symmetric about the vector (D-M6-2, L7).
  let contour = rectangle(20.0, 10.0);
  let bevel = params(3.0, 90.0, 0.0);
  let moves = run(&contour, &bevel, &tool(6.0));
  let depth = derived_depth(3.0, 90.0, 0.0);

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
      to: Point3::new(0.0, 0.0, -depth),
    }),
    "then plunges in place to the derived depth"
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

  // The tool's diameter moves nothing: a profile would be offset by half of it, a chamfer is not.
  let wide = run(&contour, &bevel, &tool(12.0));
  assert_eq!(
    cutting_points(&wide),
    cutting_points(&moves),
    "a 12 mm bit follows the same path as a 6 mm one"
  );

  // And the width changes only the Z: the same XY at a deeper Z for a wider bevel.
  let wider = run(&contour, &params(8.0, 90.0, 0.0), &tool(6.0));
  assert_eq!(
    cutting_points(&wider),
    cutting_points(&moves),
    "the contour is the path whatever the bevel's width"
  );
  assert_eq!(
    plunge_depths(&wider),
    vec![-derived_depth(8.0, 90.0, 0.0)],
    "the wider bevel is the same path, deeper"
  );
}

#[test]
fn a_chamfer_is_one_pass_however_deep_the_bevel_is() {
  // A second pass at a deeper Z would cut a second bevel into the first (D-M6-2), so the pass count
  // is one whatever the depth: 12 mm of bevel on a 90 degree bit is still one plunge.
  let contour = rectangle(20.0, 10.0);
  for width in [1.0, 12.0] {
    let moves = run(&contour, &params(width, 90.0, 0.0), &tool(6.0));
    assert_eq!(
      plunge_depths(&moves),
      vec![-derived_depth(width, 90.0, 0.0)],
      "a {width} mm bevel is one plunge, at the derived depth"
    );
    // Rapid, plunge, four sides, retract: one pass around a four-segment contour.
    assert_eq!(moves.len(), 7, "one pass, no stepdown and no tab machinery");
  }
}

#[test]
fn a_ninety_degree_bit_gives_a_depth_equal_to_the_width() {
  // The case a user checks by eye: a 90 degree bit cuts a 45 degree chamfer, so a 3 mm bevel is
  // 3 mm deep.
  let contour = rectangle(20.0, 10.0);
  let depths = plunge_depths(&run(&contour, &params(3.0, 90.0, 0.0), &tool(6.0)));
  assert_eq!(depths.len(), 1, "one pass");
  assert!(
    (depths[0] + 3.0).abs() < 1.0e-9,
    "a 3 mm bevel on a 90 degree bit reaches -3 mm, got {}",
    depths[0]
  );
}

#[test]
fn a_sixty_degree_bit_gives_the_width_times_root_three() {
  // A 60 degree bit's half angle is 30 degrees, and `1 / tan(30) = sqrt(3)`: a 3 mm bevel is
  // 3 * sqrt(3) = 5.196 mm deep. This is the assertion a derivation that forgot the tangent fails.
  let contour = rectangle(20.0, 10.0);
  let expected = -3.0 * 3.0_f64.sqrt();
  let depths = plunge_depths(&run(&contour, &params(3.0, 60.0, 0.0), &tool(6.0)));
  assert_eq!(depths.len(), 1, "one pass");
  assert!(
    (depths[0] - expected).abs() < 1.0e-9,
    "a 3 mm bevel on a 60 degree bit reaches {expected} mm, got {}",
    depths[0]
  );
}

#[test]
fn the_start_depth_shifts_the_single_pass() {
  // A bevel on a part already machined 2 mm down starts 2 mm lower and adds to the derived depth.
  let contour = rectangle(20.0, 10.0);
  let depths = plunge_depths(&run(&contour, &params(3.0, 90.0, 2.0), &tool(6.0)));
  assert_eq!(
    depths,
    vec![-derived_depth(3.0, 90.0, 2.0)],
    "start depth 2 plus a 3 mm bevel on a 90 degree bit is -5"
  );
}

#[test]
fn an_open_contour_is_refused() {
  let open = open_polyline();
  assert_eq!(
    chamfer(&request(&open, &params(3.0, 90.0, 0.0), &tool(6.0), &job())),
    Err(ToolpathError::OpenContour)
  );
}

#[test]
fn an_angle_outside_the_bevel_is_refused_at_the_library_boundary() {
  // D-M6-10's second half: the model refuses an impossible angle for a document, but a caller that
  // hands the strategy a `ChamferParams` directly must not receive an infinite Z either. The tangent
  // of `0` is zero and of `180` degrees is zero, so both have no depth.
  let contour = rectangle(20.0, 10.0);
  assert_eq!(
    chamfer(&request(
      &contour,
      &params(3.0, 0.0, 0.0),
      &tool(6.0),
      &job()
    )),
    Err(ToolpathError::InvalidIncludedAngle { degrees: 0.0 })
  );
  assert_eq!(
    chamfer(&request(
      &contour,
      &params(3.0, 180.0, 0.0),
      &tool(6.0),
      &job()
    )),
    Err(ToolpathError::InvalidIncludedAngle { degrees: 180.0 })
  );
}
