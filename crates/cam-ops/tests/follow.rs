//! Behaviour tests for the profile toolpath (A2, A6).

use cam_geom::{Contour, Point2};
use cam_model::{Job, OriginCorner, Tab};
use cam_move::{Move, Point3};
use cam_ops::{profile, ProfileRequest, ToolSpec, ToolpathError};
use cam_ops::{ProfileParams, ProfileSide};
use cam_units::Length;

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

fn circle(radius: f64) -> Contour {
  Contour::new(
    vec![
      cam_geom::Vertex::new(Point2::new(radius, 0.0), 1.0),
      cam_geom::Vertex::new(Point2::new(-radius, 0.0), 1.0),
    ],
    true,
  )
  .expect("circle")
}

fn job_with(origin: OriginCorner, height: f64) -> Job {
  Job {
    origin,
    width: Length::from_mm(100.0),
    height: Length::from_mm(height),
    ..Job::default_sheet()
  }
}

fn tool() -> ToolSpec {
  ToolSpec {
    number: 1,
    diameter: Length::from_mm(6.0),
    included_angle: None,
    feed_rate: 100.0,
    plunge_rate: 30.0,
    spindle_speed: 18_000,
  }
}

fn params(side: ProfileSide, cut_depth: f64, pass_depth: f64) -> ProfileParams {
  ProfileParams {
    side,
    allowance: Length::ZERO,
    pass_depth: Length::from_mm(pass_depth),
    start_depth: Length::ZERO,
    cut_depth: Length::from_mm(cut_depth),
    lead_in: Length::ZERO,
    lead_out: Length::ZERO,
    tabs: Vec::new(),
  }
}

fn run(contour: &Contour, params: &ProfileParams, job: &Job) -> Vec<Move> {
  profile(&ProfileRequest {
    contour,
    params,
    tool: &tool(),
    job,
  })
  .expect("toolpath computes")
}

/// Z values of the plunge that opens each pass: a vertical feed straight after a rapid.
///
/// A tab drop is also a vertical feed, but it follows a cutting move, not a rapid, so it is not a
/// new pass.
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

/// Destinations of cutting moves at a given Z.
fn cutting_points(moves: &[Move], z: f64) -> Vec<Point2> {
  moves
    .iter()
    .filter(|movement| {
      matches!(movement, Move::Feed { .. } | Move::Arc { .. })
        && (movement.destination().z.mm() - z).abs() < 1.0e-9
    })
    .map(|movement| movement.destination().xy())
    .collect()
}

fn bounds_of(points: &[Point2]) -> (f64, f64, f64, f64) {
  let mut min_x = f64::INFINITY;
  let mut min_y = f64::INFINITY;
  let mut max_x = f64::NEG_INFINITY;
  let mut max_y = f64::NEG_INFINITY;
  for point in points {
    min_x = min_x.min(point.x);
    min_y = min_y.min(point.y);
    max_x = max_x.max(point.x);
    max_y = max_y.max(point.y);
  }
  (min_x, min_y, max_x, max_y)
}

#[test]
fn passes_chunk_the_cut_depth_including_the_remainder() {
  // The spec's example: 20 mm deep with a 6 mm pass depth is 6, 12, 18 and 20.
  let contour = rectangle(100.0, 50.0);
  let request = params(ProfileSide::Outside, 20.0, 6.0);
  let moves = run(
    &contour,
    &request,
    &job_with(OriginCorner::BottomLeft, 100.0),
  );
  assert_eq!(plunge_depths(&moves), vec![-6.0, -12.0, -18.0, -20.0]);
}

#[test]
fn start_depth_shifts_the_first_pass() {
  let contour = rectangle(100.0, 50.0);
  let mut request = params(ProfileSide::Outside, 9.0, 3.0);
  request.start_depth = Length::from_mm(3.0);
  let moves = run(
    &contour,
    &request,
    &job_with(OriginCorner::BottomLeft, 100.0),
  );
  assert_eq!(plunge_depths(&moves), vec![-6.0, -9.0]);
}

#[test]
fn every_pass_retracts_to_safe_z() {
  let contour = rectangle(100.0, 50.0);
  let job = job_with(OriginCorner::BottomLeft, 100.0);
  let request = params(ProfileSide::Outside, 12.0, 6.0);
  let moves = run(&contour, &request, &job);
  let retracts = moves
    .iter()
    .filter(|movement| matches!(movement, Move::Retract { .. }))
    .count();
  assert_eq!(retracts, 2, "one retract per pass");
  for movement in &moves {
    if let Move::Retract { to } = movement {
      assert_eq!(
        to.z,
        cam_units::Length::from_mm(cam_model::DEFAULT_SAFE_Z_MM)
      );
    }
  }
}

#[test]
fn an_outside_profile_offsets_by_the_tool_radius() {
  let contour = rectangle(100.0, 50.0);
  let request = params(ProfileSide::Outside, 6.0, 6.0);
  let moves = run(
    &contour,
    &request,
    &job_with(OriginCorner::BottomLeft, 100.0),
  );
  let points = cutting_points(&moves, -6.0);
  let (min_x, min_y, max_x, max_y) = bounds_of(&points);
  assert!((min_x + 3.0).abs() < 1.0e-6, "min_x {min_x}");
  assert!((min_y + 3.0).abs() < 1.0e-6, "min_y {min_y}");
  assert!((max_x - 103.0).abs() < 1.0e-6, "max_x {max_x}");
  assert!((max_y - 53.0).abs() < 1.0e-6, "max_y {max_y}");
}

#[test]
fn an_inside_profile_shrinks_by_the_tool_radius() {
  let contour = rectangle(100.0, 50.0);
  let request = params(ProfileSide::Inside, 6.0, 6.0);
  let moves = run(
    &contour,
    &request,
    &job_with(OriginCorner::BottomLeft, 100.0),
  );
  let points = cutting_points(&moves, -6.0);
  let (min_x, min_y, max_x, max_y) = bounds_of(&points);
  assert!((min_x - 3.0).abs() < 1.0e-6, "min_x {min_x}");
  assert!((max_x - 97.0).abs() < 1.0e-6, "max_x {max_x}");
  assert!((min_y - 3.0).abs() < 1.0e-6);
  assert!((max_y - 47.0).abs() < 1.0e-6);
}

#[test]
fn an_allowance_widens_the_offset() {
  let contour = rectangle(100.0, 50.0);
  let mut request = params(ProfileSide::Outside, 6.0, 6.0);
  request.allowance = Length::from_mm(1.5);
  let moves = run(
    &contour,
    &request,
    &job_with(OriginCorner::BottomLeft, 100.0),
  );
  let (min_x, _, max_x, _) = bounds_of(&cutting_points(&moves, -6.0));
  assert!((min_x + 4.5).abs() < 1.0e-6, "min_x {min_x}");
  assert!((max_x - 104.5).abs() < 1.0e-6, "max_x {max_x}");
}

#[test]
fn an_on_profile_follows_the_vector_exactly() {
  let contour = rectangle(100.0, 50.0);
  let request = params(ProfileSide::On, 6.0, 6.0);
  let moves = run(
    &contour,
    &request,
    &job_with(OriginCorner::BottomLeft, 100.0),
  );
  let (min_x, min_y, max_x, max_y) = bounds_of(&cutting_points(&moves, -6.0));
  assert!((min_x).abs() < 1.0e-9);
  assert!((min_y).abs() < 1.0e-9);
  assert!((max_x - 100.0).abs() < 1.0e-9);
  assert!((max_y - 50.0).abs() < 1.0e-9);
}

#[test]
fn a_circular_contour_keeps_its_arcs() {
  // A2 in the toolpath layer: no tessellation on the way to the emitter.
  let contour = circle(20.0);
  let request = params(ProfileSide::Outside, 6.0, 6.0);
  let moves = run(
    &contour,
    &request,
    &job_with(OriginCorner::BottomLeft, 100.0),
  );
  let arcs: Vec<f64> = moves
    .iter()
    .filter_map(|movement| match movement {
      Move::Arc { center, to, .. } => Some(center.distance_to(to.xy())),
      _ => None,
    })
    .collect();
  assert!(!arcs.is_empty(), "arcs must survive the profile strategy");
  for radius in arcs {
    assert!((radius - 23.0).abs() < 1.0e-6, "radius {radius}");
  }
}

#[test]
fn tabs_lift_only_on_passes_below_the_keep_level() {
  let contour = rectangle(100.0, 50.0);
  let mut request = params(ProfileSide::Outside, 18.0, 6.0);
  request.tabs = vec![Tab {
    position: 0.25,
    width: Length::from_mm(10.0),
    height: Length::from_mm(3.0),
  }];
  let moves = run(
    &contour,
    &request,
    &job_with(OriginCorner::BottomLeft, 100.0),
  );

  // Passes run at -6, -12 and -18; the tab keeps material from -15 upwards, so only the final pass
  // has to lift, and it lifts by exactly the tab height.
  let lifted: Vec<f64> = moves
    .iter()
    .filter(|movement| matches!(movement, Move::Feed { .. } | Move::Arc { .. }))
    .map(|movement| movement.destination().z.mm())
    .filter(|z| (z - -15.0).abs() < 1.0e-9)
    .collect();
  assert_eq!(
    lifted,
    vec![-15.0, -15.0],
    "the tab runs from the lift to the drop, at exactly pass depth plus tab height"
  );
  assert_eq!(plunge_depths(&moves), vec![-6.0, -12.0, -18.0]);
}

#[test]
fn the_lifted_stretch_is_as_wide_as_the_tab() {
  let contour = rectangle(100.0, 50.0);
  let mut request = params(ProfileSide::Outside, 6.0, 6.0);
  request.tabs = vec![Tab {
    position: 0.10,
    width: Length::from_mm(12.0),
    height: Length::from_mm(2.0),
  }];
  let moves = run(
    &contour,
    &request,
    &job_with(OriginCorner::BottomLeft, 100.0),
  );

  let mut lifted_length = 0.0;
  let mut previous: Option<Point3> = None;
  for movement in &moves {
    let destination = movement.destination();
    if let Some(from) = previous {
      let raised = (destination.z.mm() - -4.0).abs() < 1.0e-9;
      if raised && (from.z.mm() - destination.z.mm()).abs() < 1.0e-9 {
        lifted_length += from.xy().distance_to(destination.xy());
      }
    }
    previous = Some(destination);
  }
  assert!(
    (lifted_length - 12.0).abs() < 0.05,
    "lifted {lifted_length} mm of a 12 mm tab"
  );
}

#[test]
fn a_tab_straddling_the_start_of_the_contour_is_handled() {
  let contour = rectangle(100.0, 50.0);
  let mut request = params(ProfileSide::Outside, 6.0, 6.0);
  request.tabs = vec![Tab {
    position: 0.0,
    width: Length::from_mm(10.0),
    height: Length::from_mm(2.0),
  }];
  let moves = run(
    &contour,
    &request,
    &job_with(OriginCorner::BottomLeft, 100.0),
  );
  let lifted_moves = moves
    .iter()
    .filter(|movement| (movement.destination().z.mm() - -4.0).abs() < 1.0e-9)
    .count();
  assert!(
    lifted_moves >= 3,
    "wrap-around tab produces lifts on both ends"
  );
}

#[test]
fn leads_run_along_the_start_tangent_and_out_again() {
  let contour = rectangle(100.0, 50.0);
  let mut request = params(ProfileSide::Outside, 6.0, 6.0);
  request.lead_in = Length::from_mm(5.0);
  request.lead_out = Length::from_mm(4.0);
  let moves = run(
    &contour,
    &request,
    &job_with(OriginCorner::BottomLeft, 100.0),
  );

  // rapid to the lead-in origin, plunge there, then the lead-in feed onto the contour.
  let plunge_index = moves
    .iter()
    .position(|movement| matches!(movement, Move::Feed { .. }))
    .expect("a plunge");
  let approach = moves[plunge_index].destination().xy();
  let contour_start = moves[plunge_index + 1].destination().xy();
  assert!(
    (approach.distance_to(contour_start) - 5.0).abs() < 1.0e-6,
    "lead-in length was {}",
    approach.distance_to(contour_start)
  );
  // The lead-out is a parameter of this request too, and asserting only that *a* retract exists
  // would pass with lead_out set to anything, including zero.
  let retract_index = moves
    .iter()
    .rposition(|movement| matches!(movement, Move::Retract { .. }))
    .expect("a retract");
  assert!(
    retract_index >= 2,
    "the retract follows the contour and its lead-out"
  );
  let contour_end = moves[retract_index - 2].destination().xy();
  let departure = moves[retract_index - 1].destination().xy();
  assert!(
    (departure.distance_to(contour_end) - 4.0).abs() < 1.0e-6,
    "lead-out length was {}",
    departure.distance_to(contour_end)
  );
}

#[test]
fn a_top_left_origin_flips_y_at_the_machine_boundary() {
  let contour = rectangle(100.0, 50.0);
  let request = params(ProfileSide::On, 6.0, 6.0);
  let moves = run(&contour, &request, &job_with(OriginCorner::TopLeft, 100.0));
  let points = cutting_points(&moves, -6.0);
  let (_, min_y, _, max_y) = bounds_of(&points);
  // Document y in [0, 50] becomes machine y in [50, 100] for a 100 mm tall job.
  assert!((min_y - 50.0).abs() < 1.0e-9, "min_y {min_y}");
  assert!((max_y - 100.0).abs() < 1.0e-9, "max_y {max_y}");
}

#[test]
fn a_centred_origin_maps_to_the_job_centre() {
  let contour = rectangle(100.0, 50.0);
  let request = params(ProfileSide::On, 6.0, 6.0);
  let moves = run(&contour, &request, &job_with(OriginCorner::Center, 100.0));
  let (min_x, min_y, max_x, max_y) = bounds_of(&cutting_points(&moves, -6.0));
  // A 100 x 100 job with a centred origin puts document (0..100, 0..50) at
  // machine (-50..50, -50..0).
  assert!((min_x + 50.0).abs() < 1.0e-9);
  assert!((max_x - 50.0).abs() < 1.0e-9);
  assert!((min_y + 50.0).abs() < 1.0e-9);
  assert!((max_y).abs() < 1.0e-9);
}

#[test]
fn an_open_contour_is_refused() {
  let contour =
    Contour::from_points(&[Point2::new(0.0, 0.0), Point2::new(10.0, 0.0)], false).expect("line");
  let request = params(ProfileSide::Outside, 6.0, 6.0);
  let error = profile(&ProfileRequest {
    contour: &contour,
    params: &request,
    tool: &tool(),
    job: &job_with(OriginCorner::BottomLeft, 100.0),
  })
  .expect_err("must refuse");
  assert!(matches!(error, ToolpathError::OpenContour), "{error:?}");
}

#[test]
fn a_zero_diameter_tool_is_refused() {
  let contour = rectangle(100.0, 50.0);
  let request = params(ProfileSide::Outside, 6.0, 6.0);
  let mut bad_tool = tool();
  bad_tool.diameter = Length::ZERO;
  let error = profile(&ProfileRequest {
    contour: &contour,
    params: &request,
    tool: &bad_tool,
    job: &job_with(OriginCorner::BottomLeft, 100.0),
  })
  .expect_err("must refuse");
  assert!(
    matches!(error, ToolpathError::InvalidToolDiameter),
    "{error:?}"
  );
}

#[test]
fn a_tool_too_big_for_the_feature_is_refused_not_silently_cut() {
  let contour = rectangle(4.0, 4.0);
  let request = params(ProfileSide::Inside, 6.0, 6.0);
  let error = profile(&ProfileRequest {
    contour: &contour,
    params: &request,
    tool: &tool(),
    job: &job_with(OriginCorner::BottomLeft, 100.0),
  })
  .expect_err("must refuse");
  assert!(
    matches!(error, ToolpathError::OffsetCollapsed { .. }),
    "{error:?}"
  );
}

#[test]
fn a_non_positive_pass_depth_is_refused() {
  let contour = rectangle(100.0, 50.0);
  let request = params(ProfileSide::Outside, 6.0, 0.0);
  let error = profile(&ProfileRequest {
    contour: &contour,
    params: &request,
    tool: &tool(),
    job: &job_with(OriginCorner::BottomLeft, 100.0),
  })
  .expect_err("must refuse");
  assert!(
    matches!(error, ToolpathError::InvalidPassDepth),
    "{error:?}"
  );
}

#[test]
fn a_cut_depth_above_the_start_depth_is_refused() {
  let contour = rectangle(100.0, 50.0);
  let mut request = params(ProfileSide::Outside, 6.0, 3.0);
  request.start_depth = Length::from_mm(8.0);
  let error = profile(&ProfileRequest {
    contour: &contour,
    params: &request,
    tool: &tool(),
    job: &job_with(OriginCorner::BottomLeft, 100.0),
  })
  .expect_err("must refuse");
  assert!(
    matches!(error, ToolpathError::InvalidDepthRange { .. }),
    "{error:?}"
  );
}

#[test]
fn out_of_range_tab_positions_are_refused() {
  let contour = rectangle(100.0, 50.0);
  let mut request = params(ProfileSide::Outside, 6.0, 6.0);
  request.tabs = vec![Tab {
    position: 1.0,
    width: Length::from_mm(5.0),
    height: Length::from_mm(2.0),
  }];
  let error = profile(&ProfileRequest {
    contour: &contour,
    params: &request,
    tool: &tool(),
    job: &job_with(OriginCorner::BottomLeft, 100.0),
  })
  .expect_err("must refuse");
  assert!(
    matches!(error, ToolpathError::TabOutOfRange { .. }),
    "{error:?}"
  );
}
