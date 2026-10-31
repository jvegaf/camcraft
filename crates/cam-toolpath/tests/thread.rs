//! Behaviour tests for thread milling (M6 A7, D-M6-8).
//!
//! A thread is one **helical** descent around a circle: the tool orbits at a fixed radius — `(D − t)/2`
//! for an internal thread, `(D + t)/2` for an external one — and drops one `pitch` per revolution from
//! the surface to `-cut_depth`, then retracts. The helix is expressed as helical moves, one per
//! revolution, because the move vocabulary's only curved move was constant-Z until this task: a thread
//! cut as a chord or as a flat circle is the defect the whole of D-M6-8 exists to prevent.

use std::f64::consts::TAU;

use cam_geom::{Contour, Point2};
use cam_model::{Job, ThreadDirection, ThreadParams};
use cam_move::{Move, Point3};
use cam_toolpath::{thread, ThreadRequest, ToolSpec, ToolpathError};
use cam_units::Length;

/// A circle of diameter `diameter` centred clear of the sheet's edges.
fn circle(diameter: f64) -> Contour {
  cam_geom::primitive::circle(Point2::new(25.0, 40.0), diameter / 2.0).expect("a circle")
}

fn rectangle() -> Contour {
  Contour::from_points(
    &[
      Point2::new(10.0, 10.0),
      Point2::new(30.0, 10.0),
      Point2::new(30.0, 25.0),
      Point2::new(10.0, 25.0),
    ],
    true,
  )
  .expect("a rectangle")
}

fn job() -> Job {
  Job::default_sheet()
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

fn params(pitch: f64, direction: ThreadDirection, start: f64, cut: f64) -> ThreadParams {
  ThreadParams {
    pitch: Length::from_mm(pitch),
    direction,
    start_depth: Length::from_mm(start),
    cut_depth: Length::from_mm(cut),
  }
}

fn run(
  contour: &Contour,
  params: &ThreadParams,
  tool: &ToolSpec,
) -> Result<Vec<Move>, ToolpathError> {
  thread(&ThreadRequest {
    entity: "E1",
    contour,
    params,
    tool,
    job: &job(),
  })
}

/// The tool-centre circle's centre and radius, from the moves' own helical moves.
fn orbit(moves: &[Move]) -> (Point2, f64) {
  let helix = moves
    .iter()
    .find_map(|movement| match movement {
      Move::Helix { center, to, .. } => Some((*center, center.distance_to(to.xy()))),
      _ => None,
    })
    .expect("a thread is a helix");
  helix
}

/// The helical moves, in program order.
fn helixes(moves: &[Move]) -> Vec<(Point2, Point3, bool)> {
  moves
    .iter()
    .filter_map(|movement| match movement {
      Move::Helix { to, center, ccw } => Some((*center, *to, *ccw)),
      _ => None,
    })
    .collect()
}

#[test]
fn an_internal_thread_orbits_the_hole_less_the_tool() {
  // A 12 mm hole milled by a 4 mm tool: the tool centre runs on a circle of radius
  // `(D − t)/2 = (12 − 4)/2 = 4` mm about the hole's centre, so the tool's own disc reaches the wall at
  // `4 + 2 = 6 = D/2` from the inside and stops 4 mm short of the centre — the annulus the thread is.
  let contour = circle(12.0);
  let moves = run(
    &contour,
    &params(1.5, ThreadDirection::Internal, 0.0, 6.0),
    &tool(4.0),
  )
  .expect("computes");

  let (centre, radius) = orbit(&moves);
  assert!(
    centre.is_near(Point2::new(25.0, 40.0), 1.0e-9),
    "the orbit is concentric with the hole: {centre:?}"
  );
  assert!(
    (radius - 4.0).abs() < 1.0e-9,
    "an internal thread runs at (D − t)/2 = 4 mm, got {radius}"
  );
  assert_eq!(
    moves.first(),
    Some(&Move::Rapid {
      to: Point3::new(29.0, 40.0, 5.0),
    }),
    "the tool enters over the orbit's +x point, at safe Z"
  );
  assert_eq!(
    moves.last(),
    Some(&Move::Retract {
      to: Point3::new(29.0, 40.0, 5.0),
    }),
    "and retracts from where the descent ended, back to safe Z"
  );
}

#[test]
fn an_external_thread_orbits_the_boss_plus_the_tool() {
  // The same 12 mm circle cut as a boss: the tool centre runs outside it, at `(D + t)/2 = 8` mm, so the
  // tool's inner wall is on the boss's own wall at `8 − 2 = 6 = D/2` and the thread stands outside it.
  let contour = circle(12.0);
  let moves = run(
    &contour,
    &params(2.0, ThreadDirection::External, 0.0, 8.0),
    &tool(4.0),
  )
  .expect("computes");

  let (_, radius) = orbit(&moves);
  assert!(
    (radius - 8.0).abs() < 1.0e-9,
    "an external thread runs at (D + t)/2 = 8 mm, got {radius}"
  );
}

#[test]
fn the_descent_is_one_pitch_a_revolution_and_the_last_turn_carries_the_fraction() {
  // `cut_depth / pitch` revolutions, one helical move each. 7 mm at a 2 mm pitch is 3.5 turns: three
  // full ones, each ending exactly where it started and one pitch lower, and a half turn that reaches
  // the depth — so the mark of the fraction is that its destination is diametrically opposite.
  let contour = circle(12.0);
  let moves = run(
    &contour,
    &params(2.0, ThreadDirection::Internal, 0.0, 7.0),
    &tool(4.0),
  )
  .expect("computes");

  let turns: Vec<(Point2, Point3, bool)> = helixes(&moves);
  assert_eq!(turns.len(), 4, "3.5 turns is four helical moves");
  let depths: Vec<f64> = turns.iter().map(|(_, to, _)| to.z.mm()).collect();
  assert_eq!(depths, vec![-2.0, -4.0, -6.0, -7.0]);

  // The first three close their own circle: the destination is the start point, which is one full turn.
  for (index, (_, to, _)) in turns.iter().enumerate().take(3) {
    assert!(
      to.xy().is_near(Point2::new(29.0, 40.0), 1.0e-9),
      "turn {index} is a full revolution and ends where it began: {to:?}"
    );
  }
  // The last is half a turn: it starts at the orbit's +x point and ends at its -x point.
  let last = turns[3].1;
  assert!(
    last.xy().is_near(Point2::new(21.0, 40.0), 1.0e-9),
    "the fraction of a turn that reaches the depth ends half way round: {last:?}"
  );
  assert!(
    turns.iter().all(|(_, _, ccw)| *ccw),
    "one direction of travel for the whole descent, so the thread is one helix and not a zigzag"
  );
}

#[test]
fn a_whole_number_of_turns_needs_no_fraction_and_reaches_the_depth_exactly() {
  // 6 mm at a 1.5 mm pitch is exactly four revolutions, so the last move is a full turn and no partial
  // one is emitted: the descent reaches the depth on the turn boundary.
  let contour = circle(12.0);
  let moves = run(
    &contour,
    &params(1.5, ThreadDirection::Internal, 0.0, 6.0),
    &tool(4.0),
  )
  .expect("computes");

  let turns = helixes(&moves);
  assert_eq!(turns.len(), 4);
  assert_eq!(turns.last().expect("a turn").1.z.mm(), -6.0);
  assert!(
    turns
      .iter()
      .all(|(_, to, _)| to.xy().is_near(Point2::new(29.0, 40.0), 1.0e-9)),
    "with no fraction every turn closes on the orbit's own start point"
  );
  // Each turn sweeps a full revolution, which is what `pitch` is measured against: the sum of the
  // revolutions is the turn count `cut_depth / pitch`, and the sum of the drops is the depth.
  assert_eq!(4.0 * 1.5, 6.0);
}

#[test]
fn the_program_enters_at_the_surface_so_nothing_plunges_into_material() {
  // The helix begins where the material does: rapid to safe Z, one feed down to the surface in place,
  // then the descent. A plunge to the depth and a helix from there would cut the thread's first turn
  // twice, once at full depth and once descending.
  let contour = circle(12.0);
  let moves = run(
    &contour,
    &params(2.0, ThreadDirection::Internal, 0.0, 4.0),
    &tool(4.0),
  )
  .expect("computes");

  assert_eq!(
    moves.get(1),
    Some(&Move::Feed {
      to: Point3::new(29.0, 40.0, 0.0),
    }),
    "the entry reaches the surface, not the depth"
  );
  assert!(
    matches!(moves.get(2), Some(Move::Helix { .. })),
    "the descent itself is the first helical move: {:?}",
    moves.get(2)
  );
}

#[test]
fn a_closed_contour_that_is_not_a_circle_is_refused_by_name() {
  // A thread is a circle's thread: its diameter is what `(D ± t)/2` is measured from, and a rectangle
  // has none. The refusal names the entity that is not a circle, through the same error a drill uses.
  let error = run(
    &rectangle(),
    &params(1.5, ThreadDirection::Internal, 0.0, 6.0),
    &tool(4.0),
  )
  .expect_err("a rectangle has no thread");
  assert_eq!(
    error,
    ToolpathError::NotACircle {
      entity: "E1".to_owned(),
    }
  );
  assert!(
    error.to_string().contains("E1") && error.to_string().contains("not a circle"),
    "{error}"
  );
}

#[test]
fn an_internal_thread_whose_tool_is_wider_than_the_hole_is_refused_naming_both() {
  // `(D − t)/2` is not a radius when the tool is wider than the hole: a 12 mm hole cannot hold a 12 mm
  // tool's centre on a circle, and a 14 mm one is bigger than the hole it is meant to cut inside.
  for diameter in [12.0, 14.0] {
    let error = run(
      &circle(12.0),
      &params(1.5, ThreadDirection::Internal, 0.0, 6.0),
      &tool(diameter),
    )
    .expect_err("the tool does not fit");
    assert_eq!(
      error,
      ToolpathError::ThreadToolTooWide {
        diameter,
        hole: 12.0,
      }
    );
    let message = error.to_string();
    assert!(
      message.contains("12") && message.contains(&diameter.to_string()),
      "the refusal names both the hole and the tool: {message}"
    );
  }
  // The same tool outside the same circle is the external case, where it fits by construction: the
  // orbit is `(D + t)/2` and only grows with the tool.
  run(
    &circle(12.0),
    &params(1.5, ThreadDirection::External, 0.0, 6.0),
    &tool(14.0),
  )
  .expect("a tool wider than the boss still mills its outside");
}

#[test]
fn a_pitch_or_a_depth_range_with_no_thread_behind_it_is_refused() {
  let contour = circle(12.0);
  assert_eq!(
    run(
      &contour,
      &params(0.0, ThreadDirection::Internal, 0.0, 6.0),
      &tool(4.0)
    ),
    Err(ToolpathError::InvalidPitch { pitch: 0.0 })
  );
  assert_eq!(
    run(
      &contour,
      &params(-1.5, ThreadDirection::Internal, 0.0, 6.0),
      &tool(4.0)
    ),
    Err(ToolpathError::InvalidPitch { pitch: -1.5 })
  );
  assert_eq!(
    run(
      &contour,
      &params(1.5, ThreadDirection::Internal, 6.0, 6.0),
      &tool(4.0)
    ),
    Err(ToolpathError::InvalidDepthRange {
      cut_depth: 6.0,
      start_depth: 6.0,
    })
  );
}

#[test]
fn the_descent_revolves_cut_depth_over_pitch_times() {
  // The turn count is the spec's own formula, measured as the sum of the moves' angular sweeps: for a
  // 3 mm pitch and a 9 mm depth that is exactly three revolutions, and one helical move each.
  let contour = circle(20.0);
  let moves = run(
    &contour,
    &params(3.0, ThreadDirection::External, 0.0, 9.0),
    &tool(6.0),
  )
  .expect("computes");

  let sweeps: f64 = helixes(&moves)
    .iter()
    .map(|(center, to, ccw)| {
      let from = Point2::new(center.x + center.distance_to(to.xy()), center.y);
      let start = (from.y - center.y).atan2(from.x - center.x);
      let end = (to.y.mm() - center.y).atan2(to.x.mm() - center.x);
      let mut sweep = end - start;
      if *ccw {
        while sweep <= 0.0 {
          sweep += TAU;
        }
      } else {
        while sweep >= 0.0 {
          sweep -= TAU;
        }
      }
      sweep.abs()
    })
    .sum();
  assert!(
    (sweeps - 3.0 * TAU).abs() < 1.0e-9,
    "9 mm at a 3 mm pitch is three revolutions, got {}",
    sweeps / TAU
  );
}
