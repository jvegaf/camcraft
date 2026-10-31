//! A7, as widened by M7's L6: the frame is honoured at an arc's **handedness**, not only at its points.
//!
//! `Job::origin` decides where the machine's XY origin sits, and two of the three corners move the
//! document at all: `center` **translates** it, and `top_left` **reflects** it. A reflection reverses
//! orientation, so a curve the document traverses counter-clockwise is traversed clockwise in the
//! machine's own numbering — an arc emitted with only its points mapped states the *complementary* arc,
//! which is a wrong cut on real hardware rather than a reporting error. These tests are about the
//! program, so they read the direction the strategies actually emit.

use cam_geom::{Contour, Point2};
use cam_model::{
  FlutingParams, Frame, Job, OriginCorner, ProfileParams, ProfileSide, ThreadDirection,
  ThreadParams,
};
use cam_move::{Move, Point3};
use cam_toolpath::{
  fluting, profile, thread, FlutingRequest, ProfileRequest, ThreadRequest, ToolSpec,
};
use cam_units::Length;

fn job(corner: OriginCorner) -> Job {
  Job {
    origin: corner,
    width: Length::from_mm(200.0),
    height: Length::from_mm(100.0),
    ..Job::default_sheet()
  }
}

/// A counter-clockwise circle, which is what the kernel makes of every closed contour it is handed.
fn circle(radius: f64) -> Contour {
  cam_geom::primitive::circle(Point2::new(100.0, 50.0), radius).expect("a circle")
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

fn profile_moves(corner: OriginCorner) -> Vec<Move> {
  profile(&ProfileRequest {
    contour: &circle(15.0),
    params: &ProfileParams {
      side: ProfileSide::Outside,
      allowance: Length::ZERO,
      pass_depth: Length::from_mm(6.0),
      start_depth: Length::ZERO,
      cut_depth: Length::from_mm(6.0),
      lead_in: Length::ZERO,
      lead_out: Length::ZERO,
      tabs: Vec::new(),
    },
    tool: &tool(),
    job: &job(corner),
  })
  .expect("the profile computes")
}

fn fluting_moves(corner: OriginCorner) -> Vec<Move> {
  fluting(&FlutingRequest {
    contour: &circle(15.0),
    params: &FlutingParams {
      start_depth: Length::ZERO,
      cut_depth: Length::from_mm(6.0),
      ramp: Length::from_mm(10.0),
    },
    tool: &tool(),
    job: &job(corner),
  })
  .expect("the flute computes")
}

/// The direction of every arc in a program, in order.
fn arc_directions(moves: &[Move]) -> Vec<bool> {
  moves
    .iter()
    .filter_map(|movement| match movement {
      Move::Arc { ccw, .. } => Some(*ccw),
      _ => None,
    })
    .collect()
}

/// The direction of every helix in a program, in order.
fn helix_directions(moves: &[Move]) -> Vec<bool> {
  moves
    .iter()
    .filter_map(|movement| match movement {
      Move::Helix { ccw, .. } => Some(*ccw),
      _ => None,
    })
    .collect()
}

fn flipped(directions: &[bool]) -> Vec<bool> {
  directions.iter().map(|direction| !direction).collect()
}

/// A thread on a circle of diameter 30: 5 mm deep at a 1.5 mm pitch, so the descent is three whole
/// turns and a third, and the last move ends somewhere other than the orbit's `+x` point.
fn thread_moves(corner: OriginCorner) -> Vec<Move> {
  thread(&ThreadRequest {
    entity: "hole",
    contour: &circle(15.0),
    params: &ThreadParams {
      pitch: Length::from_mm(1.5),
      direction: ThreadDirection::Internal,
      start_depth: Length::ZERO,
      cut_depth: Length::from_mm(5.0),
    },
    tool: &tool(),
    job: &job(corner),
  })
  .expect("the thread computes")
}

/// What a move is, without its geometry: the read-back comparison below is about the program's shape
/// as well as its numbers, and a rapid that became a feed is a different program.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
  Rapid,
  Feed,
  Arc,
  Helix,
  Retract,
}

/// One move read back into the **document** frame: a machine move expressed where the document drew
/// it. A reflection reverses a turn as well as the sign of its Y, so reading a program back flips the
/// sense of every `ccw` when the frame reflects — the relation `cam_sim::sweep::apply_move` applies.
#[derive(Clone, Copy, Debug)]
struct ReadBack {
  kind: Kind,
  to: [f64; 3],
  center: Option<[f64; 2]>,
  ccw: Option<bool>,
}

fn read_back(moves: &[Move], frame: Frame) -> Vec<ReadBack> {
  let point = |at: Point3| {
    let document = frame.from_machine(Point2::new(at.x.mm(), at.y.mm()));
    [document.x, document.y, at.z.mm()]
  };
  let centre = |at: Point2| {
    let document = frame.from_machine(at);
    [document.x, document.y]
  };
  let sense = |ccw: bool| ccw != frame.is_reflection();
  moves
    .iter()
    .map(|movement| match *movement {
      Move::Rapid { to } => ReadBack {
        kind: Kind::Rapid,
        to: point(to),
        center: None,
        ccw: None,
      },
      Move::Feed { to } => ReadBack {
        kind: Kind::Feed,
        to: point(to),
        center: None,
        ccw: None,
      },
      Move::Arc { to, center, ccw } => ReadBack {
        kind: Kind::Arc,
        to: point(to),
        center: Some(centre(center)),
        ccw: Some(sense(ccw)),
      },
      Move::Helix { to, center, ccw } => ReadBack {
        kind: Kind::Helix,
        to: point(to),
        center: Some(centre(center)),
        ccw: Some(sense(ccw)),
      },
      Move::Retract { to } => ReadBack {
        kind: Kind::Retract,
        to: point(to),
        center: None,
        ccw: None,
      },
    })
    .collect()
}

/// The numbers a read-back move states, in order: its destination, then its centre when it has one.
fn numbers(movement: &ReadBack) -> Vec<f64> {
  let mut values = movement.to.to_vec();
  if let Some(center) = movement.center {
    values.extend_from_slice(&center);
  }
  values
}

/// The reflection is its own inverse but not bit-exact, so equality of coordinates is asserted to
/// `1e-9` mm. It is a tolerance on a double's own rounding: the disagreement this test guards against
/// is tens of millimetres, not nanometres.
fn assert_reads_the_same(
  name: &str,
  corner: OriginCorner,
  read: &[ReadBack],
  reference: &[ReadBack],
) {
  assert_eq!(
    read.len(),
    reference.len(),
    "{name} at {corner:?}: read back {} moves where bottom-left emits {}",
    read.len(),
    reference.len()
  );
  for (index, (actual, expected)) in read.iter().zip(reference).enumerate() {
    assert_eq!(
      actual.kind, expected.kind,
      "{name} at {corner:?}: move {index} is the wrong kind"
    );
    let actual_numbers = numbers(actual);
    let expected_numbers = numbers(expected);
    assert_eq!(
      actual.ccw, expected.ccw,
      "{name} at {corner:?}: move {index} turns the other way: reads back {:?}, bottom-left {:?}",
      actual_numbers, expected_numbers
    );
    assert_eq!(
      actual_numbers.len(),
      expected_numbers.len(),
      "{name} at {corner:?}: move {index} has a centre where bottom-left has none, or the reverse"
    );
    for (axis, (actual, expected)) in actual_numbers.iter().zip(&expected_numbers).enumerate() {
      assert!(
        (actual - expected).abs() <= 1.0e-9,
        "{name} at {corner:?}: move {index} coordinate {axis} reads back {actual} where \
         bottom-left emits {expected}"
      );
    }
  }
}

#[test]
fn a_reflected_origin_emits_the_complementary_arc_and_a_translated_one_does_not() {
  let bottom_left = arc_directions(&profile_moves(OriginCorner::BottomLeft));
  assert!(
    !bottom_left.is_empty(),
    "the profile of a circle is cut as arcs"
  );
  assert!(
    bottom_left.iter().all(|direction| *direction),
    "the document's own circle runs counter-clockwise, so bottom-left emits G03: {bottom_left:?}"
  );

  let top_left = arc_directions(&profile_moves(OriginCorner::TopLeft));
  assert_eq!(
    top_left,
    flipped(&bottom_left),
    "a reflection reverses handedness, so the machine's arc is the complementary one"
  );

  let centre = arc_directions(&profile_moves(OriginCorner::Center));
  assert_eq!(
    centre, bottom_left,
    "centring the job translates it, so every arc keeps the direction the document gives it"
  );
}

#[test]
fn a_reflected_origin_emits_the_complementary_helix_and_a_translated_one_does_not() {
  let bottom_left = helix_directions(&fluting_moves(OriginCorner::BottomLeft));
  assert_eq!(
    bottom_left.len(),
    2,
    "a ramp on a closed circle is a helix at each end: {bottom_left:?}"
  );
  assert!(
    bottom_left.iter().all(|direction| *direction),
    "the ramps ride the document's counter-clockwise circle: {bottom_left:?}"
  );

  assert_eq!(
    helix_directions(&fluting_moves(OriginCorner::TopLeft)),
    flipped(&bottom_left),
    "a descending move on an arc is reversed by the same mirror that reverses the arc"
  );
  assert_eq!(
    helix_directions(&fluting_moves(OriginCorner::Center)),
    bottom_left,
    "a translated job keeps the document's handedness"
  );
}

#[test]
fn every_origin_corner_emits_the_program_bottom_left_emits_once_read_back() {
  // A7's rule, stated without mentioning the flag that carries it: whatever corner the job is set to,
  // the machine is told a program that means what bottom-left's means. Reading a program back maps
  // every point into the document frame with `Frame::from_machine`, and a reflection also reverses
  // each turn — the same relation the sweep applies, so a program that only satisfies one of the two
  // is the wrong cut this test exists to catch.
  struct Fixture {
    name: &'static str,
    program: fn(OriginCorner) -> Vec<Move>,
    /// The kind of move this fixture exists to witness, so the rule cannot pass vacuously.
    witness: Kind,
  }
  let fixtures = [
    Fixture {
      name: "a circle profile's arcs",
      program: profile_moves,
      witness: Kind::Arc,
    },
    Fixture {
      name: "a fluting ramp",
      program: fluting_moves,
      witness: Kind::Helix,
    },
    Fixture {
      name: "a thread",
      program: thread_moves,
      witness: Kind::Helix,
    },
  ];

  for Fixture {
    name,
    program,
    witness,
  } in fixtures
  {
    let reference = read_back(
      &program(OriginCorner::BottomLeft),
      job(OriginCorner::BottomLeft).frame(),
    );
    assert!(
      reference.iter().any(|movement| movement.kind == witness),
      "{name}: the fixture's program holds a {witness:?} move, or it cannot witness the rule"
    );
    for corner in [OriginCorner::TopLeft, OriginCorner::Center] {
      let read = read_back(&program(corner), job(corner).frame());
      assert_reads_the_same(name, corner, &read, &reference);
    }
  }
}
