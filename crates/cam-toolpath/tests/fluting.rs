//! Behaviour tests for fluting (M6 A4, D-M6-5).
//!
//! A flute is one pass along a vector at one authored depth with the tool's centre **on** the vector,
//! and the ramp is a *length*: with `ramp > 0` the pass enters over the first `ramp` millimetres of the
//! vector and leaves over the last, so the tool never plunges at full depth (A4). The entry and the
//! exit are real moves, because the machine has to traverse them; a ramp longer than half the vector
//! cannot be walked without shortening it, so it is refused by name rather than clipped.

use cam_geom::{Contour, Point2};
use cam_model::{FlutingParams, Job};
use cam_move::{Move, Point3};
use cam_toolpath::{fluting, FlutingRequest, ToolSpec, ToolpathError};
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

/// The commonest flute of all: one straight vector, drawn as a line.
fn open_vector(length: f64) -> Contour {
  Contour::from_points(&[Point2::new(0.0, 0.0), Point2::new(length, 0.0)], false)
    .expect("an open vector")
}

/// A closed contour of two half-turn arcs, centred clear of the sheet's edges.
fn circle(radius: f64) -> Contour {
  cam_geom::primitive::circle(Point2::new(25.0, 40.0), radius).expect("a circle")
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

fn params(cut_depth: f64, start_depth: f64, ramp: f64) -> FlutingParams {
  FlutingParams {
    start_depth: Length::from_mm(start_depth),
    cut_depth: Length::from_mm(cut_depth),
    ramp: Length::from_mm(ramp),
  }
}

fn request<'a>(
  contour: &'a Contour,
  params: &'a FlutingParams,
  tool: &'a ToolSpec,
  job: &'a Job,
) -> FlutingRequest<'a> {
  FlutingRequest {
    contour,
    params,
    tool,
    job,
  }
}

fn run(contour: &Contour, params: &FlutingParams, tool: &ToolSpec) -> Vec<Move> {
  fluting(&request(contour, params, tool, &job())).expect("toolpath computes")
}

/// The XY distance one movement travels.
fn travel(moves: &[Move], index: usize) -> f64 {
  let from = if index == 0 {
    Point3::ORIGIN
  } else {
    moves[index - 1].destination()
  };
  from.xy().distance_to(moves[index].destination().xy())
}

/// The XY travel spent before the tool first reaches `z`, and the travel spent after it last leaves
/// that depth. A flute's ramp is exactly the travel that takes it from the surface to the depth.
fn travel_to_reach(moves: &[Move], z: f64) -> f64 {
  let mut walked = 0.0;
  for (index, movement) in moves.iter().enumerate() {
    walked += travel(moves, index);
    if movement.destination().z.mm() <= z + 1.0e-9 {
      return walked;
    }
  }
  f64::NAN
}

fn travel_after_leaving(moves: &[Move], z: f64) -> f64 {
  let mut last = 0;
  for (index, movement) in moves.iter().enumerate() {
    if movement.destination().z.mm() <= z + 1.0e-9 {
      last = index;
    }
  }
  (last + 1..moves.len())
    .map(|index| travel(moves, index))
    .sum()
}

/// Every destination Z, in program order.
fn depths(moves: &[Move]) -> Vec<f64> {
  moves
    .iter()
    .map(|movement| movement.destination().z.mm())
    .collect()
}

/// True when the Z sequence only ever descends and then only ever ascends: one pass, one trough.
fn one_trough(moves: &[Move]) -> bool {
  let zs = depths(moves);
  let trough = zs
    .iter()
    .enumerate()
    .min_by(|a, b| a.1.partial_cmp(b.1).expect("finite depths"))
    .map(|(index, _)| index)
    .expect("a move");
  zs[..=trough]
    .windows(2)
    .all(|pair| pair[0] >= pair[1] - 1.0e-9)
    && zs[trough..]
      .windows(2)
      .all(|pair| pair[0] <= pair[1] + 1.0e-9)
}

#[test]
fn the_entry_descends_over_the_first_ramp_millimetres_and_the_exit_ascends_over_the_last() {
  // A4's own words, measured as travel: on a 100 mm vector with a 10 mm ramp and a 3 mm flute, the
  // tool spends exactly 10 mm of XY travel getting from the surface to `-3`, and exactly 10 mm
  // getting back. Those are real moves, not a command the post is trusted to interpret.
  let contour = open_vector(100.0);
  let moves = run(&contour, &params(3.0, 0.0, 10.0), &tool(6.0));

  let entry = travel_to_reach(&moves, -3.0);
  assert!(
    (entry - 10.0).abs() < 1.0e-9,
    "the entry ramps down over exactly 10 mm of travel, got {entry}"
  );
  let exit = travel_after_leaving(&moves, -3.0);
  assert!(
    (exit - 10.0).abs() < 1.0e-9,
    "the exit ramps up over exactly 10 mm of travel, got {exit}"
  );
  assert!(
    one_trough(&moves),
    "one pass: the Z sequence descends once and ascends once, got {:?}",
    depths(&moves)
  );
  assert!(
    depths(&moves).iter().any(|z| (z + 3.0).abs() < 1.0e-9),
    "the pass reaches the authored depth"
  );

  // "Never plunges at full depth": a movement with no XY travel may not go below the surface, so the
  // only vertical descent in the program is the one to `Z0`, onto the stock's face.
  for (index, movement) in moves.iter().enumerate() {
    if !matches!(movement, Move::Feed { .. }) {
      continue;
    }
    if travel(&moves, index) <= 1.0e-9 {
      assert!(
        movement.destination().z.mm() >= -1.0e-9,
        "a ramped flute plunges nowhere: move {index} descends to {} without travelling",
        movement.destination().z.mm()
      );
    }
  }
}

#[test]
fn the_ramp_in_is_one_linear_move_of_the_ramp_length_and_so_is_the_ramp_out() {
  // D-M6-12: the ramp is **one move**, not a staircase of Z steps. A real controller interpolates a
  // linear move, and `cam-sim` does now too, so the program says what the machine does: one feed of
  // exactly `ramp` millimetres of travel whose Z goes from the surface to the cut depth, and the mirror
  // of it coming out. The count of cutting moves is therefore the same however deep the flute is — a
  // pass law would add moves, and a staircase would add them with the depth.
  let contour = open_vector(100.0);
  for depth in [1.0, 12.0] {
    let moves = run(&contour, &params(depth, 0.0, 10.0), &tool(6.0));
    let cutting: Vec<&Move> = moves
      .iter()
      .filter(|movement| matches!(movement, Move::Feed { .. } | Move::Arc { .. }))
      .collect();
    assert_eq!(
      cutting.len(),
      4,
      "a {depth} mm flute is the plunge to the surface, one ramp in, the plateau and one ramp out"
    );
    assert_eq!(
      cutting[0],
      &Move::Feed {
        to: Point3::new(0.0, 0.0, 0.0),
      },
      "the plunge only reaches the surface: the ramp does the descending"
    );
    assert_eq!(
      cutting[1],
      &Move::Feed {
        to: Point3::new(10.0, 0.0, -depth),
      },
      "the entry is one move of 10 mm of travel, from the surface to -{depth}"
    );
    assert_eq!(
      cutting[2],
      &Move::Feed {
        to: Point3::new(90.0, 0.0, -depth),
      },
      "then the plateau, at the one depth"
    );
    assert_eq!(
      cutting[3],
      &Move::Feed {
        to: Point3::new(100.0, 0.0, 0.0),
      },
      "and the exit is one mirror move back to the surface"
    );
  }
}

#[test]
fn a_zero_ramp_plunges_at_the_start_and_retracts_at_the_end() {
  // The plain groove D-M6-5 keeps as the degenerate case: no ramp, one plunge in place where the
  // vector starts, the vector cut at one depth, and a retract where it ends.
  let contour = open_vector(100.0);
  let moves = run(&contour, &params(3.0, 0.0, 0.0), &tool(6.0));

  assert_eq!(
    moves.first(),
    Some(&Move::Rapid {
      to: Point3::new(0.0, 0.0, 5.0),
    }),
    "a rapid over the vector's first point at safe Z"
  );
  assert_eq!(
    moves.get(1),
    Some(&Move::Feed {
      to: Point3::new(0.0, 0.0, -3.0),
    }),
    "then a plunge in place, at the start"
  );
  assert_eq!(
    moves.last(),
    Some(&Move::Retract {
      to: Point3::new(100.0, 0.0, 5.0),
    }),
    "and a retract in place at the far end"
  );
  assert_eq!(
    moves.len(),
    4,
    "rapid, plunge, the vector, retract: nothing else belongs to one pass"
  );
}

#[test]
fn a_flute_is_one_pass_at_the_depth_however_deep_the_cut_is() {
  // D-M6-5's pass law is no pass law: a second pass would cut a second groove into the first. So the
  // depth changes the Z of one pass and nothing about how many the program runs.
  for depth in [1.0, 12.0] {
    let contour = open_vector(100.0);
    let plain = run(&contour, &params(depth, 0.0, 0.0), &tool(6.0));
    assert_eq!(
      plain.len(),
      4,
      "a {depth} mm flute without a ramp is still rapid, plunge, cut, retract"
    );
    assert_eq!(
      depths(&plain).iter().cloned().fold(0.0, f64::min),
      -depth,
      "and it reaches the authored depth"
    );

    let ramped = run(&contour, &params(depth, 0.0, 10.0), &tool(6.0));
    assert!(
      one_trough(&ramped),
      "a {depth} mm ramped flute descends once and ascends once: {:?}",
      depths(&ramped)
    );
    assert!(
      (depths(&ramped).iter().cloned().fold(0.0, f64::min) + depth).abs() < 1.0e-9,
      "and reaches -{depth} exactly once, never below it"
    );
    let plateaus = depths(&ramped)
      .iter()
      .filter(|z| (**z + depth).abs() < 1.0e-9)
      .count();
    assert!(
      plateaus <= 2,
      "the depth is reached by the entry and left by the exit, not cut twice: {plateaus} moves at -{depth}"
    );
  }
}

#[test]
fn a_ramp_longer_than_half_the_vector_is_refused_naming_both_numbers() {
  // A4: "a `ramp` longer than half the vector is an input error, not a silently shortened ramp". Half
  // is legal — the entry and the exit meet exactly — and anything past it is refused with the two
  // numbers that cannot both be true.
  let contour = open_vector(100.0);
  assert!(
    fluting(&request(
      &contour,
      &params(3.0, 0.0, 50.0),
      &tool(6.0),
      &job()
    ))
    .is_ok(),
    "a ramp of exactly half the vector fits: the two ramps meet in the middle"
  );

  let refused = fluting(&request(
    &contour,
    &params(3.0, 0.0, 51.0),
    &tool(6.0),
    &job(),
  ));
  assert_eq!(
    refused,
    Err(ToolpathError::RampTooLong {
      ramp: 51.0,
      length: 100.0,
    }),
    "a 51 mm ramp on a 100 mm vector is refused"
  );
  let message = refused.expect_err("refused").to_string();
  assert!(
    message.contains("51") && message.contains("100"),
    "the refusal names both numbers: {message}"
  );

  // A closed contour measures the same way, along its perimeter: 60 mm of rectangle fits a 30 mm ramp
  // and refuses a 31 mm one.
  let loop_ = rectangle(20.0, 10.0);
  assert_eq!(
    fluting(&request(
      &loop_,
      &params(3.0, 0.0, 31.0),
      &tool(6.0),
      &job()
    )),
    Err(ToolpathError::RampTooLong {
      ramp: 31.0,
      length: 60.0,
    })
  );
}

#[test]
fn a_closed_contour_is_fluted_as_one_pass_and_returns_to_its_start() {
  // A flute is one groove per vector, and a closed vector is one groove that happens to close: the
  // pass starts at the first vertex, ramps out over the last millimetres of the perimeter and lands
  // back on the vertex it started from.
  let contour = rectangle(20.0, 10.0);
  let moves = run(&contour, &params(2.0, 0.0, 5.0), &tool(6.0));

  let last_cut = moves
    .iter()
    .rev()
    .find(|movement| matches!(movement, Move::Feed { .. } | Move::Arc { .. }))
    .map(|movement| movement.destination())
    .expect("a cutting move");
  assert!(
    last_cut.xy().is_near(Point2::new(0.0, 0.0), 1.0e-9),
    "the loop closes where it opened, at {last_cut:?}"
  );
  assert_eq!(
    last_cut.z.mm(),
    0.0,
    "and the exit ramp ends at the surface, not below it"
  );
  assert!(
    one_trough(&moves),
    "a closed flute is one pass: {:?}",
    depths(&moves)
  );
  // Nothing leaves the rectangle: the flute grooves the loop and clears nothing inside it.
  for movement in &moves {
    let point = movement.destination().xy();
    assert!(
      point.x >= -1.0e-9
        && point.x <= 20.0 + 1.0e-9
        && point.y >= -1.0e-9
        && point.y <= 10.0 + 1.0e-9,
      "the cut stays on the contour, got {point:?}"
    );
  }
}

#[test]
fn a_negative_ramp_is_refused_by_name() {
  // The document model cannot derive a meaning for a negative length here — it would enter upwards —
  // so the strategy refuses it rather than treating it as zero.
  let contour = open_vector(100.0);
  assert_eq!(
    fluting(&request(
      &contour,
      &params(3.0, 0.0, -1.0),
      &tool(6.0),
      &job()
    )),
    Err(ToolpathError::InvalidRamp { ramp: -1.0 })
  );
}

#[test]
fn a_cut_depth_at_or_below_the_start_depth_is_refused() {
  // The same rule an engrave inherits from the profile (D-M6-4): the pass lands on `cut_depth`, so a
  // cut at or above the start has no pass behind it.
  let contour = open_vector(100.0);
  for (cut_depth, start_depth) in [(3.0, 3.0), (2.0, 3.0)] {
    assert_eq!(
      fluting(&request(
        &contour,
        &params(cut_depth, start_depth, 10.0),
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

#[test]
fn a_ramped_arc_is_cut_as_a_helix_along_the_arc() {
  // T4 refused this: a ramped move cannot ride on `Move::Arc`, which is constant-Z, so the ramp would
  // have been cut as a straight chord — `ramp² / (8·r) = 10² / (8 · 25) = 0.5` mm inside the vector on
  // this 25 mm circle, a gouge on one side of the ramp and uncut material on the other. T7's helical
  // move lifts the refusal: the ramp now rides on the contour's own arc and descends as it goes, so its
  // XY radius is the contour's radius and never a chord's.
  let contour = circle(25.0);
  let moves = run(&contour, &params(6.0, 0.0, 10.0), &tool(6.0));

  let helixes: Vec<&Move> = moves
    .iter()
    .filter(|movement| matches!(movement, Move::Helix { .. }))
    .collect();
  assert_eq!(
    helixes.len(),
    2,
    "the entry and the exit are helixes, and nothing else is: {moves:?}"
  );
  for movement in &helixes {
    let Move::Helix { center, to, .. } = movement else {
      unreachable!("filtered")
    };
    assert!(
      (center.distance_to(to.xy()) - 25.0).abs() < 1.0e-9,
      "each ramp rides the vector's own circle rather than its chord: {movement:?}"
    );
    assert!(
      center.is_near(Point2::new(25.0, 40.0), 1.0e-9),
      "about the circle's own centre: {movement:?}"
    );
  }
  // Ten millimetres of arc on a 25 mm circle is `10 / 25` radians, which is where the entry ends and the
  // plateau begins: the ramp is measured along the vector, exactly as it is on a line.
  let Move::Helix { to: entered, .. } = *helixes[0] else {
    unreachable!("filtered")
  };
  let angle: f64 = 10.0 / 25.0;
  assert!(
    entered.xy().is_near(
      Point2::new(25.0 + 25.0 * angle.cos(), 40.0 + 25.0 * angle.sin()),
      1.0e-9
    ),
    "the entry covers exactly the ramp's arc length: {entered:?}"
  );
  assert_eq!(entered.z.mm(), -6.0, "and reaches the depth at its end");

  // The plateau between the two ramps is still ordinary constant-Z arcs, and the program is still one
  // pass: the Z sequence descends once and ascends once.
  let arcs = moves
    .iter()
    .filter(|movement| matches!(movement, Move::Arc { .. }))
    .count();
  assert_eq!(arcs, 2, "the plateau keeps the circle's arcs: {moves:?}");
  assert!(
    one_trough(&moves),
    "one pass: the Z sequence descends once and ascends once, got {:?}",
    depths(&moves)
  );
  let Move::Helix { to: left, .. } = *helixes[1] else {
    unreachable!("filtered")
  };
  assert_eq!(left.z.mm(), 0.0, "the exit ramp ends at the surface");
  assert!(
    left.xy().is_near(Point2::new(50.0, 40.0), 1.0e-9),
    "and lands back on the vertex the loop opened at: {left:?}"
  );
}

#[test]
fn a_flute_without_a_ramp_still_follows_an_arc_as_an_arc() {
  // Refusing the ramp must not become refusing the contour: a constant-depth flute along an arc is
  // exactly what `Move::Arc` is for, so `ramp = 0` keeps working and the circle's own arcs survive into
  // the program rather than being flattened into chords.
  let contour = circle(25.0);
  let moves = run(&contour, &params(6.0, 0.0, 0.0), &tool(6.0));
  let arcs = moves
    .iter()
    .filter(|movement| matches!(movement, Move::Arc { .. }))
    .count();
  assert_eq!(
    arcs, 2,
    "the circle arrives as two half turns and leaves as two arcs: {moves:?}"
  );
  assert_eq!(
    moves.first(),
    Some(&Move::Rapid {
      to: Point3::new(50.0, 40.0, 5.0),
    }),
    "a rapid over the circle's first vertex, which is its centre plus the radius along +x"
  );
  assert_eq!(
    moves.get(1),
    Some(&Move::Feed {
      to: Point3::new(50.0, 40.0, -6.0),
    }),
    "one plunge in place, at the one depth"
  );
  let last = moves.last().expect("a move");
  assert!(
    matches!(last, Move::Retract { .. }),
    "and a retract, where the loop closed: {last:?}"
  );
}
