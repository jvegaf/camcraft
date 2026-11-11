//! Behaviour tests for the drilling toolpath (A7, A8, A9).
//!
//! A drill program is only right if it goes to the right place, in the right order, and never leaves
//! the tool down between holes. Every test here asserts the emitted moves, never internals.

use cam_geom::Point2;
use cam_model::{Job, OriginCorner};
use cam_move::{Move, Point3};
use cam_ops::{drill, DrillHole, DrillRequest, Drillpath, ToolSpec, ToolpathError};
use cam_ops::{DrillBank, DrillParams, RetractMode};
use cam_units::Length;

fn job() -> Job {
  Job::default_sheet()
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

fn hole(entity: &str, x: f64, y: f64) -> DrillHole {
  DrillHole {
    entity: entity.to_owned(),
    centre: Point2::new(x, y),
  }
}

/// Four holes on a 20 mm square, listed out of tour order and named so the entity-id tie-break
/// decides which of the two equal-distance holes is drilled second.
fn square() -> Vec<DrillHole> {
  vec![
    hole("h3", 50.0, 60.0),
    hole("h1", 30.0, 40.0),
    hole("h4", 30.0, 60.0),
    hole("h2", 50.0, 40.0),
  ]
}

fn params(peck_depth: f64, retract: RetractMode, cut_depth: f64) -> DrillParams {
  DrillParams {
    peck_depth: Length::from_mm(peck_depth),
    retract,
    start_depth: Length::ZERO,
    cut_depth: Length::from_mm(cut_depth),
    bank: None,
  }
}

/// The same request with a gang of `count` drills `pitch` millimetres apart along X.
fn banked_params(count: u32, pitch: f64) -> DrillParams {
  DrillParams {
    bank: Some(DrillBank {
      count,
      pitch: Length::from_mm(pitch),
    }),
    ..params(0.0, RetractMode::Full, 7.0)
  }
}

/// One row of holes at y = 20 mm, left to right, so the tour is the listing order and the bank is the
/// only thing that can move the head.
fn row(xs: &[f64]) -> Vec<DrillHole> {
  xs.iter()
    .enumerate()
    .map(|(index, x)| hole(&format!("h{index}"), *x, 20.0))
    .collect()
}

fn run(holes: &[DrillHole], params: &DrillParams, job: &Job) -> Drillpath {
  drill(&DrillRequest {
    holes,
    params,
    tool: &tool(),
    job,
  })
  .expect("drilling computes")
}

fn refuse(holes: &[DrillHole], params: &DrillParams) -> ToolpathError {
  let job = job();
  drill(&DrillRequest {
    holes,
    params,
    tool: &tool(),
    job: &job,
  })
  .expect_err("must refuse")
}

fn rapid_positions(moves: &[Move]) -> Vec<Point3> {
  moves
    .iter()
    .filter_map(|movement| match movement {
      Move::Rapid { to } => Some(*to),
      _ => None,
    })
    .collect()
}

/// Z of every move that ends the hole, i.e. every retract.
fn retract_depths(moves: &[Move]) -> Vec<f64> {
  moves
    .iter()
    .filter_map(|movement| match movement {
      Move::Retract { to } => Some(to.z.mm()),
      _ => None,
    })
    .collect()
}

#[test]
fn four_holes_on_a_square_follow_the_nearest_neighbour_tour() {
  let holes = square();
  let path = run(&holes, &params(0.0, RetractMode::Full, 7.0), &job());

  // Start at the lowest y then the lowest x, then nearest unvisited: the two 20 mm candidates from
  // h1 are h2 and h4, and "h2" wins the tie even though h4 was listed first.
  assert_eq!(path.order, vec!["h1", "h2", "h3", "h4"]);
  assert_eq!(
    path.drill_index,
    vec![None, None, None, None],
    "no bank was asked for, so no hole names a drill"
  );
  assert_eq!(
    path.moves,
    vec![
      Move::Rapid {
        to: Point3::new(30.0, 40.0, 5.0)
      },
      Move::Feed {
        to: Point3::new(30.0, 40.0, -7.0)
      },
      Move::Retract {
        to: Point3::new(30.0, 40.0, 5.0)
      },
      Move::Rapid {
        to: Point3::new(50.0, 40.0, 5.0)
      },
      Move::Feed {
        to: Point3::new(50.0, 40.0, -7.0)
      },
      Move::Retract {
        to: Point3::new(50.0, 40.0, 5.0)
      },
      Move::Rapid {
        to: Point3::new(50.0, 60.0, 5.0)
      },
      Move::Feed {
        to: Point3::new(50.0, 60.0, -7.0)
      },
      Move::Retract {
        to: Point3::new(50.0, 60.0, 5.0)
      },
      Move::Rapid {
        to: Point3::new(30.0, 60.0, 5.0)
      },
      Move::Feed {
        to: Point3::new(30.0, 60.0, -7.0)
      },
      Move::Retract {
        to: Point3::new(30.0, 60.0, 5.0)
      },
    ]
  );
}

#[test]
fn pecks_step_down_and_full_retracts_reach_safe_z() {
  let holes = [hole("h1", 30.0, 40.0)];
  let job = job();
  let path = run(&holes, &params(3.0, RetractMode::Full, 7.0), &job);

  assert_eq!(
    path.moves,
    vec![
      Move::Rapid {
        to: Point3::new(30.0, 40.0, 5.0)
      },
      Move::Feed {
        to: Point3::new(30.0, 40.0, -3.0)
      },
      Move::Retract {
        to: Point3::new(30.0, 40.0, 5.0)
      },
      Move::Feed {
        to: Point3::new(30.0, 40.0, -6.0)
      },
      Move::Retract {
        to: Point3::new(30.0, 40.0, 5.0)
      },
      Move::Feed {
        to: Point3::new(30.0, 40.0, -7.0)
      },
      Move::Retract {
        to: Point3::new(30.0, 40.0, 5.0)
      },
    ],
    "a 3 mm peck on a 7 mm cut is -3, -6 and the 1 mm remainder at -7"
  );
  for depth in retract_depths(&path.moves) {
    assert_eq!(depth, job.safe_z.mm(), "full retracts clear the hole");
  }
}

#[test]
fn amount_retracts_lift_one_amount_above_the_peck_just_finished() {
  let holes = [hole("h1", 30.0, 40.0)];
  let job = job();
  let path = run(
    &holes,
    &params(
      3.0,
      RetractMode::Amount {
        amount: Length::from_mm(1.0),
      },
      7.0,
    ),
    &job,
  );

  assert_eq!(
    path.moves,
    vec![
      Move::Rapid {
        to: Point3::new(30.0, 40.0, 5.0)
      },
      Move::Feed {
        to: Point3::new(30.0, 40.0, -3.0)
      },
      Move::Retract {
        to: Point3::new(30.0, 40.0, -2.0)
      },
      Move::Feed {
        to: Point3::new(30.0, 40.0, -6.0)
      },
      Move::Retract {
        to: Point3::new(30.0, 40.0, -5.0)
      },
      Move::Feed {
        to: Point3::new(30.0, 40.0, -7.0)
      },
      Move::Retract {
        to: Point3::new(30.0, 40.0, 5.0)
      },
    ],
    "amount retracts stay down; only the retract that ends the hole clears it"
  );
  let depths = retract_depths(&path.moves);
  assert_eq!(&depths[..2], &[-2.0, -5.0]);
  assert!(
    depths[..2]
      .iter()
      .all(|depth| (depth - job.safe_z.mm()).abs() > 1.0e-9),
    "an amount retract must not reach safe Z: {depths:?}"
  );
  assert_eq!(
    depths.last().copied(),
    Some(job.safe_z.mm()),
    "the last retract of the hole always reaches safe Z: {depths:?}"
  );
}

#[test]
fn a_zero_peck_depth_cuts_the_hole_in_one_plunge() {
  let holes = [hole("h1", 30.0, 40.0)];
  let job = job();
  let path = run(&holes, &params(0.0, RetractMode::Full, 7.0), &job);

  let plunges: Vec<f64> = path
    .moves
    .iter()
    .filter_map(|movement| match movement {
      Move::Feed { to } => Some(to.z.mm()),
      _ => None,
    })
    .collect();
  assert_eq!(plunges, vec![-7.0], "no peck means one plunge");
  assert_eq!(
    path.moves.len(),
    3,
    "rapid, one plunge and one retract: {:?}",
    path.moves
  );
  assert_eq!(retract_depths(&path.moves), vec![job.safe_z.mm()]);
}

#[test]
fn start_depth_shifts_the_first_peck() {
  let holes = [hole("h1", 30.0, 40.0)];
  let mut request = params(3.0, RetractMode::Full, 7.0);
  request.start_depth = Length::from_mm(2.0);
  let path = run(&holes, &request, &job());

  let plunges: Vec<f64> = path
    .moves
    .iter()
    .filter_map(|movement| match movement {
      Move::Feed { to } => Some(to.z.mm()),
      _ => None,
    })
    .collect();
  assert_eq!(
    plunges,
    vec![-5.0, -7.0],
    "pecks are measured from the start depth, not from the surface"
  );
}

#[test]
fn rapid_length_is_the_hand_computed_tour_length() {
  let holes = square();
  let path = run(&holes, &params(0.0, RetractMode::Full, 7.0), &job());

  // (0, 0) -> h1 (30, 40) is 50 mm, then three sides of the 20 mm square.
  assert!(
    (path.rapid_length.mm() - 110.0).abs() < 1.0e-9,
    "{}",
    path.rapid_length.mm()
  );

  // The same number, summed from the rapids the program actually emits, starting at the origin.
  let mut previous = Point2::new(0.0, 0.0);
  let mut travelled = 0.0;
  for position in rapid_positions(&path.moves) {
    travelled += previous.distance_to(position.xy());
    previous = position.xy();
  }
  assert!(
    (travelled - path.rapid_length.mm()).abs() < 1.0e-9,
    "reported {} but emitted {travelled}",
    path.rapid_length.mm()
  );
}

#[test]
fn two_runs_on_the_same_input_produce_identical_moves() {
  let holes = square();
  let request = params(
    2.0,
    RetractMode::Amount {
      amount: Length::from_mm(0.5),
    },
    9.0,
  );
  let job = job();
  let first = run(&holes, &request, &job);
  let second = run(&holes, &request, &job);
  assert_eq!(first.moves, second.moves);
  assert_eq!(first.order, second.order);
  assert_eq!(first.drill_index, second.drill_index);
  assert_eq!(first.rapid_length, second.rapid_length);
}

#[test]
fn holes_are_drilled_at_their_machine_position() {
  let holes = [hole("h1", 30.0, 40.0)];
  let job = Job {
    origin: OriginCorner::TopLeft,
    width: Length::from_mm(100.0),
    height: Length::from_mm(100.0),
    ..Job::default_sheet()
  };
  let path = run(&holes, &params(0.0, RetractMode::Full, 7.0), &job);

  assert_eq!(
    rapid_positions(&path.moves),
    vec![Point3::new(30.0, 60.0, 5.0)],
    "a top-left origin flips document y onto machine y"
  );
  for movement in &path.moves {
    assert_eq!(
      movement.destination().xy(),
      Point2::new(30.0, 60.0),
      "every move of a hole stays on the hole"
    );
  }
}

#[test]
fn a_bank_of_three_drills_picks_the_nearest_index_and_compensates_x() {
  let holes = row(&[0.0, 10.0, 40.0, 100.0]);
  let path = run(&holes, &banked_params(3, 32.0), &job());

  assert_eq!(
    path.order,
    vec!["h0", "h1", "h2", "h3"],
    "the bank must not renegotiate the tour"
  );
  assert_eq!(
    path.drill_index,
    vec![Some(0), Some(0), Some(1), Some(2)],
    "from x = 0, 10, 40 and 100 the nearest head is 0, 0, -24 and -64 mm from the hole"
  );
  assert_eq!(
    rapid_positions(&path.moves),
    vec![
      Point3::new(0.0, 20.0, 5.0),
      Point3::new(10.0, 20.0, 5.0),
      Point3::new(8.0, 20.0, 5.0),
      Point3::new(36.0, 20.0, 5.0),
    ],
    "drill k goes over the hole by parking the head k*pitch to the left of it"
  );

  // The whole hole is cut at the compensated position, so the drill lands on it.
  let plunge_x: Vec<f64> = path
    .moves
    .iter()
    .filter_map(|movement| match movement {
      Move::Feed { to } => Some(to.x.mm()),
      _ => None,
    })
    .collect();
  assert_eq!(plunge_x, vec![0.0, 10.0, 8.0, 36.0]);
  for movement in &path.moves {
    assert!(
      (movement.destination().y.mm() - 20.0).abs() < 1.0e-9,
      "a bank compensates along X only: {movement:?}"
    );
  }

  // Origin -> (0, 20) -> 10 -> 2 -> 28: the head travels 60 mm instead of the 120 mm the holes
  // themselves are apart.
  assert!(
    (path.bank_travel.mm() - 60.0).abs() < 1.0e-9,
    "{}",
    path.bank_travel.mm()
  );
  assert!(
    (path.rapid_length.mm() - 120.0).abs() < 1.0e-9,
    "{}",
    path.rapid_length.mm()
  );
}

#[test]
fn head_travel_stays_within_half_the_bank_width() {
  // Three drills 32 mm apart span (count - 1) * pitch = 64 mm, so lining one up never asks the head
  // to move more than half of that along X between two holes. Holes 40 mm apart would make an
  // uncompensated head take 40 mm steps, i.e. more than the half bank.
  let count = 3;
  let pitch = 32.0;
  let holes = row(&[0.0, 40.0, 80.0]);
  let path = run(&holes, &banked_params(count, pitch), &job());
  let half_bank = f64::from(count - 1) * pitch / 2.0;

  let mut previous = Point2::new(0.0, 0.0);
  for position in rapid_positions(&path.moves) {
    let step = (position.x.mm() - previous.x).abs();
    assert!(
      step <= half_bank + 1.0e-9,
      "the head moved {step} mm along X, more than the {half_bank} mm half bank"
    );
    previous = position.xy();
  }

  // Every compensation is a whole number of pitches below count * pitch, which is what puts a drill
  // over the hole rather than near it.
  for (position, hole_x) in rapid_positions(&path.moves).iter().zip([0.0, 40.0, 80.0]) {
    let rest = hole_x - position.x.mm();
    assert!(
      (-1.0e-9..f64::from(count) * pitch).contains(&rest),
      "hole at {hole_x} has its head at {}, which is not a drill in the bank",
      position.x.mm()
    );
    let pitches = rest / pitch;
    assert!(
      (pitches - pitches.round()).abs() < 1.0e-9,
      "{rest} mm is not a whole number of {pitch} mm pitches"
    );
  }
}

#[test]
fn two_banked_runs_on_the_same_input_are_identical() {
  let holes = row(&[0.0, 10.0, 40.0, 100.0]);
  let request = banked_params(3, 32.0);
  let job = job();
  let first = run(&holes, &request, &job);
  let second = run(&holes, &request, &job);
  assert_eq!(first.moves, second.moves);
  assert_eq!(first.order, second.order);
  assert_eq!(first.drill_index, second.drill_index);
  assert_eq!(first.bank_travel, second.bank_travel);
  assert_eq!(first.rapid_length, second.rapid_length);
}

#[test]
fn a_bank_with_no_drills_is_refused() {
  let error = refuse(&row(&[0.0, 10.0]), &banked_params(0, 32.0));
  assert!(matches!(error, ToolpathError::InvalidBank), "{error:?}");
}

#[test]
fn a_bank_without_a_positive_pitch_is_refused() {
  for pitch in [0.0, -32.0] {
    let error = refuse(&row(&[0.0, 10.0]), &banked_params(3, pitch));
    assert!(
      matches!(error, ToolpathError::InvalidBank),
      "pitch {pitch}: {error:?}"
    );
  }
}

#[test]
fn a_bank_of_one_drill_emits_the_un_banked_program() {
  let holes = row(&[0.0, 10.0, 40.0, 100.0]);
  let plain = run(&holes, &params(0.0, RetractMode::Full, 7.0), &job());

  for pitch in [1.0, 32.0, 250.0] {
    let banked = run(&holes, &banked_params(1, pitch), &job());
    assert_eq!(
      banked.moves, plain.moves,
      "one drill at {pitch} mm pitch cannot move the head"
    );
    assert_eq!(banked.order, plain.order);
    assert_eq!(banked.drill_index, vec![Some(0); 4]);
    assert_eq!(banked.rapid_length, plain.rapid_length);
    assert_eq!(banked.bank_travel, plain.bank_travel);
  }
}

#[test]
fn without_a_bank_no_hole_names_a_drill_and_the_travels_agree() {
  let holes = square();
  let path = run(&holes, &params(0.0, RetractMode::Full, 7.0), &job());

  assert_eq!(path.drill_index, vec![None, None, None, None]);
  assert_eq!(
    path.bank_travel, path.rapid_length,
    "with no compensation the head is the hole, so both travel figures are the same"
  );
}

#[test]
fn a_negative_peck_depth_is_refused() {
  let error = refuse(&square(), &params(-1.0, RetractMode::Full, 7.0));
  assert!(
    matches!(error, ToolpathError::InvalidPeckDepth { peck_depth } if (peck_depth + 1.0).abs() < 1.0e-9),
    "{error:?}"
  );
}

#[test]
fn a_request_without_holes_is_refused() {
  let error = refuse(&[], &params(0.0, RetractMode::Full, 7.0));
  assert!(matches!(error, ToolpathError::NoHoles), "{error:?}");
}

#[test]
fn a_cut_depth_above_the_start_depth_is_refused() {
  let mut request = params(3.0, RetractMode::Full, 6.0);
  request.start_depth = Length::from_mm(8.0);
  let error = refuse(&square(), &request);
  assert!(
    matches!(
      error,
      ToolpathError::InvalidDepthRange {
        cut_depth,
        start_depth
      } if (cut_depth - 6.0).abs() < 1.0e-9 && (start_depth - 8.0).abs() < 1.0e-9
    ),
    "{error:?}"
  );
}

#[test]
fn a_non_positive_tool_diameter_is_refused() {
  let holes = square();
  let request = params(0.0, RetractMode::Full, 7.0);
  let mut bad_tool = tool();
  bad_tool.diameter = Length::ZERO;
  let job = job();
  let error = drill(&DrillRequest {
    holes: &holes,
    params: &request,
    tool: &bad_tool,
    job: &job,
  })
  .expect_err("must refuse");
  assert!(
    matches!(error, ToolpathError::InvalidToolDiameter),
    "{error:?}"
  );
}
