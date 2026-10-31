//! Behaviour tests for the V-carve strategy (M7 A1, A3, D-M7-7).
//!
//! The depths here are always written from the geometry — `clearance / tan(theta / 2)`, with the
//! clearance read from `cam_geom::medial`'s own certified vertices — and never from
//! `VCarveParams::depth()`. A test that read the depth back from the function under test would agree
//! with any derivation it was given, including a wrong one.

use cam_geom::medial::{medial_axis, MedialAxis};
use cam_geom::{region::Region, Contour, Point2};
use cam_model::{Job, OriginCorner, VCarveParams};
use cam_move::{Move, Point3};
use cam_toolpath::{vcarve, ToolSpec, ToolpathError, VCarveRequest};
use cam_units::{Angle, Length};

/// The clearance the fixtures are refined to, in millimetres: coarse on purpose, so the program stays
/// small enough to read, and the axis's step is a number the test names.
const TOLERANCE: f64 = 0.5;

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

fn disc(centre: Point2, radius: f64) -> Contour {
  cam_geom::primitive::circle(centre, radius).expect("circle")
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

fn tool(degrees: Option<f64>) -> ToolSpec {
  ToolSpec {
    number: 7,
    diameter: Length::from_mm(12.7),
    included_angle: degrees.map(Angle::from_degrees),
    feed_rate: 100.0,
    plunge_rate: 30.0,
    spindle_speed: 18_000,
  }
}

fn params(degrees: f64, tolerance: f64) -> VCarveParams {
  VCarveParams {
    included_angle: Angle::from_degrees(degrees),
    tolerance: Length::from_mm(tolerance),
  }
}

fn request<'a>(
  contours: &'a [&'a Contour],
  params: &'a VCarveParams,
  tool: &'a ToolSpec,
  flat: f64,
  job: &'a Job,
) -> VCarveRequest<'a> {
  VCarveRequest {
    contours,
    params,
    tool,
    flat_diameter: Length::from_mm(flat),
    job,
  }
}

/// The axis `cam_geom` certifies for these contours at `tolerance`: what the walk is measured against.
fn axis(contours: &[&Contour], tolerance: f64) -> MedialAxis {
  let region = Region::new(contours.iter().map(|contour| (*contour).clone()).collect())
    .expect("closed contours bound a region");
  medial_axis(&region, tolerance).expect("the axis computes")
}

/// The depth a tip of `clearance` sits at, written from the geometry: `-clearance / tan(theta / 2)`.
fn depth(clearance: f64, degrees: f64) -> f64 {
  -(clearance / (Angle::from_degrees(degrees).radians() / 2.0).tan())
}

/// The program the walk is supposed to be, built from the axis and the law rather than from the
/// strategy: per branch a rapid, a plunge, one feed per further vertex, a closing feed for a loop, and a
/// retract.
fn expected_walk(axis: &MedialAxis, degrees: f64, job: &Job) -> Vec<Move> {
  let safe_z = job.safe_z.mm();
  let at = |point: Point2, clearance: f64| {
    let machine = job.frame().to_machine(point);
    Point3::new(machine.x, machine.y, depth(clearance, degrees))
  };
  let mut moves = Vec::new();
  for branch in &axis.branches {
    let first = branch.vertices.first().expect("a branch has a vertex");
    let start = at(first.point, first.clearance);
    moves.push(Move::Rapid {
      to: Point3::new(start.x.mm(), start.y.mm(), safe_z),
    });
    moves.push(Move::Feed { to: start });
    for vertex in &branch.vertices[1..] {
      moves.push(Move::Feed {
        to: at(vertex.point, vertex.clearance),
      });
    }
    if branch.closed {
      moves.push(Move::Feed { to: start });
    }
    let last = moves
      .last()
      .map(|movement| movement.destination())
      .expect("a move");
    moves.push(Move::Retract {
      to: Point3::new(last.x.mm(), last.y.mm(), safe_z),
    });
  }
  moves
}

#[test]
fn the_walk_is_the_medial_axis_at_the_depth_each_vertex_demands() {
  // The whole claim in one assertion: the tips are the axis's certified vertices, in the axis's order,
  // each at `clearance / tan(theta/2)`. A 90 degree bit is not used here — `tan(45) = 1` would let a
  // derivation that ignored the angle agree with the right one, so the fixture is 60 degrees, where the
  // depth is `clearance * sqrt(3)`.
  let square = square(20.0);
  let contours = [&square];
  let keel = params(60.0, TOLERANCE);
  let cutter = tool(Some(60.0));
  let moves = vcarve(&request(&contours, &keel, &cutter, 0.0, &job())).expect("a V-carve");

  let axis = axis(&contours, TOLERANCE);
  assert_eq!(
    moves,
    expected_walk(&axis, 60.0, &job()),
    "every tip is an axis vertex at its own clearance's depth"
  );
  assert!(
    !moves
      .iter()
      .any(|movement| matches!(movement, Move::Arc { .. } | Move::Helix { .. })),
    "no arc and no helix: a V-carve's Z is a function of position, and `Move::Arc` is constant-Z"
  );

  // The apex: the axis's junction is the square's centre, 10 mm from every side, so a 60 degree bit
  // reaches `10 / tan(30) = 10 * sqrt(3)` below the surface there.
  let deepest = moves
    .iter()
    .map(|movement| movement.destination().z.mm())
    .fold(f64::INFINITY, f64::min);
  let apex = depth(10.0, 60.0);
  assert!(
    (deepest - apex).abs() < 1.0e-6,
    "the square's centre is 10 mm from the boundary, so the apex is {apex} mm, got {deepest}"
  );
}

#[test]
fn a_ninety_degree_bit_reaches_the_clearance_itself() {
  // The case a user checks by eye: `tan(45) = 1`, so the apex of a 20 mm square is exactly 10 mm.
  let square = square(20.0);
  let contours = [&square];
  let keel = params(90.0, TOLERANCE);
  let cutter = tool(Some(90.0));
  let moves = vcarve(&request(&contours, &keel, &cutter, 0.0, &job())).expect("a V-carve");
  let deepest = moves
    .iter()
    .map(|movement| movement.destination().z.mm())
    .fold(f64::INFINITY, f64::min);
  assert!(
    (deepest + 10.0).abs() < 1.0e-6,
    "a 90 degree bit on a 20 mm square reaches -10 mm, got {deepest}"
  );
}

#[test]
fn a_discs_axis_is_one_point_so_its_program_is_one_plunge_and_one_retract() {
  // A disc's medial axis is its centre alone (A2): the program is a rapid, a plunge to the radius's
  // depth and a retract. There is nothing else to walk, which is why the disc is the smallest possible
  // V-carve and the fixture the oracle's arithmetic is easiest to state for.
  let disc = disc(Point2::new(50.0, 50.0), 10.0);
  let contours = [&disc];
  let keel = params(90.0, TOLERANCE);
  let cutter = tool(Some(90.0));
  let moves = vcarve(&request(&contours, &keel, &cutter, 0.0, &job())).expect("a V-carve");
  assert_eq!(moves.len(), 3, "a rapid, a plunge and a retract: {moves:?}");
  assert_eq!(
    moves[0],
    Move::Rapid {
      to: Point3::new(50.0, 50.0, 5.0),
    }
  );
  // `tan(45 degrees)` is not exactly one in binary floating point, so the depth is the law's own value
  // and the assertion is the law read from the geometry, to within rounding.
  let deepest = moves[1].destination();
  assert!(
    matches!(moves[1], Move::Feed { .. }),
    "the plunge is a feed: {moves:?}"
  );
  assert_eq!((deepest.x.mm(), deepest.y.mm()), (50.0, 50.0));
  assert!(
    (deepest.z.mm() - depth(10.0, 90.0)).abs() < 1.0e-9,
    "a disc of radius 10 on a 90 degree bit plunges to the radius, got {}",
    deepest.z.mm()
  );
  assert_eq!(
    moves[2],
    Move::Retract {
      to: Point3::new(50.0, 50.0, 5.0),
    }
  );
}

#[test]
fn a_top_left_origin_maps_every_tip_into_the_machine_frame() {
  // The tip is a position like any other, so it goes through the one mapping the model owns (M7 A7):
  // the reflect turns Y into `height - Y` and leaves Z alone, because depth is not a plane.
  let square = square(20.0);
  let contours = [&square];
  let keel = params(90.0, TOLERANCE);
  let cutter = tool(Some(90.0));
  let flipped = Job {
    origin: OriginCorner::TopLeft,
    ..job()
  };
  let moves = vcarve(&request(&contours, &keel, &cutter, 0.0, &flipped)).expect("a V-carve");
  assert_eq!(
    moves,
    expected_walk(&axis(&contours, TOLERANCE), 90.0, &flipped)
  );
  let axis_vertices = axis(&contours, TOLERANCE);
  let first = axis_vertices.branches[0].vertices[0].point;
  assert_eq!(
    moves[0].destination().y.mm(),
    flipped.height.mm() - first.y,
    "the first tip is reflected, not copied"
  );
}

#[test]
fn the_tolerance_is_the_axis_step_and_never_a_depth() {
  // D-M7-5: a finer tolerance buys a finer axis, so a shorter walk with more vertices — and it must
  // not move a single depth off its own vertex's clearance. The coarse program is a subsequence of
  // nothing: both are the walk of their own axis, which is the property that keeps the tolerance a
  // measurement and not a second depth law.
  let square = square(20.0);
  let contours = [&square];
  let cutter = tool(Some(90.0));
  let coarse = params(90.0, 0.5);
  let fine = params(90.0, 0.1);
  let coarse_moves = vcarve(&request(&contours, &coarse, &cutter, 0.0, &job())).expect("a V-carve");
  let fine_moves = vcarve(&request(&contours, &fine, &cutter, 0.0, &job())).expect("a V-carve");
  assert!(
    fine_moves.len() > coarse_moves.len(),
    "a 0.1 mm axis has more vertices than a 0.5 mm one: {} against {}",
    fine_moves.len(),
    coarse_moves.len()
  );
  assert_eq!(
    coarse_moves,
    expected_walk(&axis(&contours, 0.5), 90.0, &job()),
    "the coarse walk is its own axis"
  );
  assert_eq!(
    fine_moves,
    expected_walk(&axis(&contours, 0.1), 90.0, &job()),
    "and the fine walk is its own"
  );
}

#[test]
fn an_open_contour_is_refused() {
  let open = open_polyline();
  let contours = [&open];
  let keel = params(90.0, TOLERANCE);
  let cutter = tool(Some(90.0));
  assert_eq!(
    vcarve(&request(&contours, &keel, &cutter, 0.0, &job())),
    Err(ToolpathError::OpenContour)
  );
}

#[test]
fn an_angle_outside_the_cone_is_refused_naming_the_value() {
  // The document refuses an impossible angle for a stored toolpath (A4), and the strategy refuses it
  // again for a caller that bypassed the document: `tan(0)` and `tan(90 degrees)` are the two ends
  // where the depth law has no depth behind it.
  let square = square(20.0);
  let contours = [&square];
  let cutter = tool(Some(90.0));
  for degrees in [0.0, 180.0] {
    let keel = params(degrees, TOLERANCE);
    assert_eq!(
      vcarve(&request(&contours, &keel, &cutter, 0.0, &job())),
      Err(ToolpathError::InvalidIncludedAngle { degrees })
    );
  }
}

#[test]
fn a_tool_without_an_included_angle_is_refused_naming_the_tool() {
  // D-M6-9: a cylinder has no cone behind it, and defaulting 90 degrees would carve a different shape
  // than the bit in the machine. The refusal names the tool so the operator can fix the library.
  let square = square(20.0);
  let contours = [&square];
  let keel = params(90.0, TOLERANCE);
  let cutter = tool(None);
  let error = vcarve(&request(&contours, &keel, &cutter, 0.0, &job()))
    .expect_err("must refuse a tool with no angle");
  assert_eq!(
    error,
    ToolpathError::VCarveNeedsAngledTool { tool_number: 7 }
  );
  assert!(
    error.to_string().contains("T7"),
    "the refusal names the tool: {error}"
  );
}

#[test]
fn a_flat_tipped_bit_is_refused_naming_the_flat() {
  // With a flat the tip cannot carve to a sharp edge, so the depth law — which reaches zero exactly at
  // the vectors — is the law of a bit that is not in the machine (the chamfer's D-M6-10, reversed).
  let square = square(20.0);
  let contours = [&square];
  let keel = params(90.0, TOLERANCE);
  let cutter = tool(Some(90.0));
  let error =
    vcarve(&request(&contours, &keel, &cutter, 0.4, &job())).expect_err("must refuse a flat tip");
  assert_eq!(
    error,
    ToolpathError::VCarveNeedsSharpBit {
      tool_number: 7,
      flat_diameter: 0.4,
    }
  );
  assert!(
    error.to_string().contains("0.4"),
    "the refusal names the flat: {error}"
  );
}

#[test]
fn a_branch_that_passes_a_junction_is_walked_through_it() {
  // `cam_geom::medial`'s stitch pass joins two arms of one junction into a single walk, so a branch is
  // not "one side of a junction": its clearance can rise to a junction and fall again on the other
  // side. The L is the shape where that happens, and a walk that assumed monotone depth — or that
  // retracted at the junction — would cut the second arm wrong.
  let l = Contour::from_points(
    &[
      Point2::new(0.0, 0.0),
      Point2::new(10.0, 0.0),
      Point2::new(10.0, 4.0),
      Point2::new(4.0, 4.0),
      Point2::new(4.0, 10.0),
      Point2::new(0.0, 10.0),
    ],
    true,
  )
  .expect("L");
  let contours = [&l];
  let keel = params(90.0, 0.25);
  let cutter = tool(Some(90.0));
  let moves = vcarve(&request(&contours, &keel, &cutter, 0.0, &job())).expect("a V-carve");
  let axis = axis(&contours, 0.25);

  assert_eq!(
    moves,
    expected_walk(&axis, 90.0, &job()),
    "every vertex of every branch, in the axis's own order, at its own depth"
  );

  // The junction is a vertex whose clearance is greater than both of its neighbours in the same walk:
  // the reflex corner's 2.281 mm against the arms' 2.0 mm. At least one branch really does pass through
  // it, which is what makes the assertion above more than a restatement.
  let through_a_junction = axis.branches.iter().any(|branch| {
    branch.vertices.windows(3).any(|window| {
      window[1].clearance > window[0].clearance && window[1].clearance > window[2].clearance
    })
  });
  assert!(
    through_a_junction,
    "the fixture must have a branch whose clearance peaks in its interior, or it is not the case"
  );

  // And the walk still retracts exactly once per branch: a junction is not a reason to lift.
  let retracts = moves
    .iter()
    .filter(|movement| matches!(movement, Move::Retract { .. }))
    .count();
  assert_eq!(
    retracts,
    axis.branches.len(),
    "one retract per branch, none at a junction inside a branch"
  );
}
