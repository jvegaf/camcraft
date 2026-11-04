//! Behaviour tests for the moulding strategy (M8 T4; S3, S5).
//!
//! Every expected depth is written from the cross-section the test authored, never from
//! `CrossSection::depth_at` or from the strategy: a test that read the depth back through the same
//! law would agree with any law, including a wrong one. The straight rail's moves are pinned as whole
//! blocks so the pass order, the plunge, the walk and the retract are one assertion each; the curved
//! rail's moves are checked against the circle's own equation, which a chord-based or mis-signed offset
//! would fail.

use cam_geom::{Contour, Point2, Vertex};
use cam_model::{
  Document, Entity, EntityKind, Job, Layer, MouldingParams, MouldingProfilePoint, Operation,
  OriginCorner, Toolpath,
};
use cam_move::{Move, Point3, ToolShape};
use cam_toolpath::{moulding, program_for, MouldingRequest, ProgramTool, ToolSpec, ToolpathError};
use cam_units::{Angle, Length};

/// The job's clearance height, so each block's rapid and retract can be read.
const SAFE_Z: f64 = 5.0;

/// A straight open rail along +X at y = 50: lateral is to the right of travel, towards -Y.
fn straight_rail() -> Contour {
  Contour::from_points(&[Point2::new(0.0, 50.0), Point2::new(100.0, 50.0)], false).expect("rail")
}

/// A closed circular rail of `radius` about `centre`, travelled counter-clockwise.
fn circular_rail(centre: Point2, radius: f64) -> Contour {
  Contour::new(
    vec![
      Vertex::new(Point2::new(centre.x + radius, centre.y), 1.0),
      Vertex::new(Point2::new(centre.x - radius, centre.y), 1.0),
    ],
    true,
  )
  .expect("circle from two half arcs")
}

/// The 12 mm wide cross-section the task's own example uses: zero at both edges, 4 mm deep in the
/// middle.
fn params(stepover: f64) -> MouldingParams {
  MouldingParams {
    profile: vec![
      MouldingProfilePoint {
        lateral: Length::from_mm(0.0),
        depth: Length::ZERO,
      },
      MouldingProfilePoint {
        lateral: Length::from_mm(6.0),
        depth: Length::from_mm(-4.0),
      },
      MouldingProfilePoint {
        lateral: Length::from_mm(12.0),
        depth: Length::ZERO,
      },
    ],
    stepover: Length::from_mm(stepover),
  }
}

fn job() -> Job {
  Job {
    origin: OriginCorner::BottomLeft,
    width: Length::from_mm(200.0),
    height: Length::from_mm(200.0),
    ..Job::default_sheet()
  }
}

fn tool(diameter: f64, angle: Option<f64>) -> ToolSpec {
  ToolSpec {
    number: 3,
    diameter: Length::from_mm(diameter),
    included_angle: angle.map(Angle::from_degrees),
    feed_rate: 120.0,
    plunge_rate: 30.0,
    spindle_speed: 18_000,
  }
}

fn request<'a>(
  rail: &'a Contour,
  params: &'a MouldingParams,
  tool: &'a ToolSpec,
  job: &'a Job,
) -> MouldingRequest<'a> {
  MouldingRequest {
    rail,
    params,
    tool,
    job,
  }
}

/// One lateral pass as the strategy is meant to emit it: a rapid to the pass's first point at safe Z,
/// a plunge there to `z`, one feed to the pass's last point at that same `z`, and a retract.
fn expected_pass(first: (f64, f64), last: (f64, f64), z: f64) -> Vec<Move> {
  vec![
    Move::Rapid {
      to: Point3::new(first.0, first.1, SAFE_Z),
    },
    Move::Feed {
      to: Point3::new(first.0, first.1, z),
    },
    Move::Feed {
      to: Point3::new(last.0, last.1, z),
    },
    Move::Retract {
      to: Point3::new(last.0, last.1, SAFE_Z),
    },
  ]
}

fn document_with(rail: Contour, operation: Operation, tool_number: u32) -> Document {
  let mut document = Document::new(job());
  document.layers.push(Layer {
    id: "L1".to_owned(),
    name: "Part".to_owned(),
    visible: true,
    entities: vec![Entity {
      id: "E1".to_owned(),
      kind: EntityKind::Contour { contour: rail },
    }],
  });
  document.toolpaths = vec![Toolpath {
    id: "TP1".to_owned(),
    name: "Moulding".to_owned(),
    entity_ids: vec!["E1".to_owned()],
    island_ids: Vec::new(),
    tool_number,
    operation,
  }];
  document
}

fn program_tool(spec: ToolSpec) -> ProgramTool {
  ProgramTool {
    spec,
    flat_diameter: Length::ZERO,
  }
}

#[test]
fn the_walk_is_the_cross_sections_depth_at_each_lateral_pass() {
  // Three passes at 6 mm: lateral 0, 6 and 12, whose authored depths are 0, -4 and 0. The straight
  // rail answers every one of them exactly, so the whole program is arithmetic the test wrote down.
  let rail = straight_rail();
  let keel = params(6.0);
  let cutter = tool(6.0, None);
  let moves = moulding(&request(&rail, &keel, &cutter, &job())).expect("a moulding");

  let mut expected = Vec::new();
  expected.extend(expected_pass((0.0, 50.0), (100.0, 50.0), 0.0));
  expected.extend(expected_pass((0.0, 44.0), (100.0, 44.0), -4.0));
  expected.extend(expected_pass((0.0, 38.0), (100.0, 38.0), 0.0));
  assert_eq!(moves, expected);
  assert!(
    !moves
      .iter()
      .any(|movement| matches!(movement, Move::Arc { .. } | Move::Helix { .. })),
    "a moulding's Z is a function of the lateral offset, and an arc is constant-Z by contract"
  );
}

#[test]
fn the_tools_diameter_moves_nothing_because_the_profile_is_authored() {
  // The cross-section is data, not a tool shape: a 40 mm bit cuts the same path a 1 mm bit does, and
  // the strategy must not rescale the passes by the diameter.
  let rail = straight_rail();
  let keel = params(6.0);
  let narrow = moulding(&request(&rail, &keel, &tool(1.0, None), &job())).expect("a moulding");
  let wide = moulding(&request(&rail, &keel, &tool(40.0, None), &job())).expect("a moulding");
  assert_eq!(narrow, wide);
}

#[test]
fn a_stepover_wider_than_the_profile_cuts_one_centred_pass() {
  // T3's decision, seen from the program: the same cross-section at a 20 mm stepover is one pass,
  // centred at lateral 6 mm, at the -4 mm the cross-section authors there.
  let rail = straight_rail();
  let keel = params(20.0);
  let cutter = tool(6.0, None);
  let moves = moulding(&request(&rail, &keel, &cutter, &job())).expect("a moulding");
  assert_eq!(
    moves,
    expected_pass((0.0, 44.0), (100.0, 44.0), -4.0),
    "one pass, centred on the profile, at the profile's own middle depth"
  );
}

#[test]
fn linking_between_passes_retracts_to_safe_z() {
  // D-M4-6's rule, inherited: a pass that merely looks adjacent can be on the far side of a wall, so
  // every pass ends with a retract to the job's clearance and the next begins with a rapid there.
  let rail = straight_rail();
  let keel = params(3.0);
  let cutter = tool(6.0, None);
  let moves = moulding(&request(&rail, &keel, &cutter, &job())).expect("a moulding");

  let mut blocks = 0;
  let mut index = 0;
  while index < moves.len() {
    match &moves[index] {
      Move::Rapid { to } => {
        assert_eq!(to.z.mm(), SAFE_Z, "a pass starts with a rapid at safe Z");
        // The block runs to the retract that closes it, and every cutting move stays at one Z.
        let mut end = index + 1;
        let z = match &moves[end] {
          Move::Feed { to } => {
            assert_eq!(
              (to.x.mm(), to.y.mm()),
              (
                moves[index].destination().x.mm(),
                moves[index].destination().y.mm()
              ),
              "the plunge happens over the pass's first point"
            );
            to.z.mm()
          }
          other => panic!("a rapid is followed by the plunge, got {other:?}"),
        };
        while let Some(Move::Feed { to }) = moves.get(end + 1) {
          assert_eq!(to.z.mm(), z, "a pass is cut at one Z");
          end += 1;
        }
        match &moves[end + 1] {
          Move::Retract { to } => {
            assert_eq!(to.z.mm(), SAFE_Z, "a pass ends at safe Z");
            assert_eq!(
              (to.x.mm(), to.y.mm()),
              (
                moves[end].destination().x.mm(),
                moves[end].destination().y.mm()
              ),
              "the retract leaves from where the pass stopped"
            );
          }
          other => panic!("a pass is closed by a retract, got {other:?}"),
        }
        index = end + 2;
        blocks += 1;
      }
      other => panic!("a program starts with a rapid, got {other:?}"),
    }
  }
  assert_eq!(
    blocks, 5,
    "a 12 mm profile at a 3 mm stepover is five passes"
  );
}

#[test]
fn a_curved_rail_is_walked_as_feeds_that_stay_on_the_offset_circle() {
  // The offset of a circular rail at lateral `l` is a circle of radius `r + l`: a wrong normal sign, a
  // chord-based point or a pass that ignored the cross-section would all leave this circle. No arc move
  // is emitted even though the rail is arcs, because the walk's Z is the cross-section's, not the arc's.
  let centre = Point2::new(60.0, 60.0);
  let radius = 20.0;
  let rail = circular_rail(centre, radius);
  let keel = MouldingParams {
    profile: vec![
      MouldingProfilePoint {
        lateral: Length::ZERO,
        depth: Length::ZERO,
      },
      MouldingProfilePoint {
        lateral: Length::from_mm(1.0),
        depth: Length::from_mm(-1.0),
      },
      MouldingProfilePoint {
        lateral: Length::from_mm(2.0),
        depth: Length::from_mm(-2.0),
      },
    ],
    stepover: Length::from_mm(1.0),
  };
  let cutter = tool(6.0, None);
  let moves = moulding(&request(&rail, &keel, &cutter, &job())).expect("a moulding");

  assert!(
    !moves
      .iter()
      .any(|movement| matches!(movement, Move::Arc { .. } | Move::Helix { .. })),
    "the walk is feeds: {moves:?}"
  );
  let mut zs = Vec::new();
  for movement in &moves {
    let to = movement.destination();
    if to.z.mm() == SAFE_Z {
      continue;
    }
    let expected_radius = radius + -to.z.mm();
    let actual = to.xy().distance_to(centre);
    assert!(
      (actual - expected_radius).abs() < 1.0e-9,
      "a move at z {} should be on radius {expected_radius}, it is at {actual}",
      to.z.mm()
    );
    zs.push(to.z.mm());
  }
  zs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
  zs.dedup();
  assert_eq!(zs, vec![-2.0, -1.0, 0.0], "one Z per lateral pass");
}

#[test]
fn a_closed_rail_closes_its_pass() {
  // A closed rail's walk ends where it started, and the pass must cut the edge between the last
  // sample and the first instead of leaving a gap at the seam.
  let rail = circular_rail(Point2::new(60.0, 60.0), 20.0);
  let keel = params(6.0);
  let cutter = tool(6.0, None);
  let moves = moulding(&request(&rail, &keel, &cutter, &job())).expect("a moulding");

  let first = moves[0].destination();
  let closing = moves
    .iter()
    .take_while(|movement| !matches!(movement, Move::Retract { .. }))
    .last()
    .expect("a cutting move")
    .destination();
  assert!(
    closing.xy().distance_to(first.xy()) < 1.0e-9,
    "the pass stops at {:?}, it starts at {:?}",
    closing.xy(),
    first.xy()
  );
}

#[test]
fn the_program_is_identical_twice() {
  // S5: the strategy has no map to iterate and no clock to read, so the same document is the same
  // program byte for byte.
  let rail = circular_rail(Point2::new(60.0, 60.0), 20.0);
  let keel = params(2.0);
  let cutter = tool(6.0, Some(60.0));
  let first = moulding(&request(&rail, &keel, &cutter, &job())).expect("a moulding");
  let second = moulding(&request(&rail, &keel, &cutter, &job())).expect("a moulding");
  assert_eq!(first, second);

  let document = document_with(rail, Operation::Moulding(keel), 3);
  let library = [program_tool(cutter)];
  let first_program = program_for(&document, &library).expect("a program");
  let second_program = program_for(&document, &library).expect("a program");
  assert_eq!(first_program, second_program);
}

#[test]
fn program_for_sends_a_moulding_to_the_sweep_with_the_tools_real_shape() {
  // The dispatch arm is the strategy, not a refusal: the program's moves are the sweep's own. The
  // segment carries the tool's real shape — a V-bit when the library records an angle — because the
  // authored cross-section says nothing about the cutter that sweeps it (M6 A8).
  let rail = straight_rail();
  let keel = params(6.0);
  let cutter = tool(6.0, Some(60.0));
  let document = document_with(rail.clone(), Operation::Moulding(keel.clone()), 3);
  let library = [program_tool(cutter)];
  let program = program_for(&document, &library).expect("a program");
  assert_eq!(program.segments.len(), 1);
  let segment = &program.segments[0];
  assert_eq!(
    segment.moves,
    moulding(&request(&rail, &keel, &cutter, &job())).expect("a moulding")
  );
  assert_eq!(
    segment.tool,
    ToolShape::v_bit(Angle::from_degrees(60.0), Length::ZERO)
  );
}

#[test]
fn a_profile_with_fewer_than_two_points_is_refused_by_name() {
  let rail = straight_rail();
  let keel = MouldingParams {
    profile: vec![MouldingProfilePoint {
      lateral: Length::ZERO,
      depth: Length::ZERO,
    }],
    stepover: Length::from_mm(2.0),
  };
  let cutter = tool(6.0, None);
  let error =
    moulding(&request(&rail, &keel, &cutter, &job())).expect_err("one point is not a profile");
  assert_eq!(
    error,
    ToolpathError::MouldingProfileTooShort { points: 1 },
    "the refusal names how many points there were"
  );
}

#[test]
fn a_profile_whose_laterals_do_not_increase_is_refused_by_name() {
  let rail = straight_rail();
  let keel = MouldingParams {
    profile: vec![
      MouldingProfilePoint {
        lateral: Length::ZERO,
        depth: Length::ZERO,
      },
      MouldingProfilePoint {
        lateral: Length::from_mm(6.0),
        depth: Length::from_mm(-4.0),
      },
      MouldingProfilePoint {
        lateral: Length::from_mm(6.0),
        depth: Length::from_mm(-5.0),
      },
    ],
    stepover: Length::from_mm(2.0),
  };
  let cutter = tool(6.0, None);
  let error = moulding(&request(&rail, &keel, &cutter, &job()))
    .expect_err("a doubled lateral is no function");
  assert_eq!(
    error,
    ToolpathError::MouldingProfileLateral {
      index: 2,
      previous: 6.0,
      lateral: 6.0,
    }
  );
}

#[test]
fn a_zero_stepover_is_refused_by_name() {
  // The document refuses it as it loads, but a caller that hands the strategy its parameters reaches
  // it without passing through there, so the strategy refuses it too.
  let rail = straight_rail();
  let keel = params(0.0);
  let cutter = tool(6.0, None);
  let error = moulding(&request(&rail, &keel, &cutter, &job())).expect_err("no spacing, no passes");
  assert_eq!(error, ToolpathError::InvalidStepover { stepover: 0.0 });
}

#[test]
fn a_tool_without_a_diameter_is_refused_by_name() {
  let rail = straight_rail();
  let keel = params(6.0);
  let error =
    moulding(&request(&rail, &keel, &tool(0.0, None), &job())).expect_err("no tool, no cut");
  assert_eq!(error, ToolpathError::InvalidToolDiameter);
}

#[test]
fn a_rail_that_cannot_be_walked_is_refused_by_name() {
  // An empty contour has no length to sweep along; the kernel's refusal is kept, with its own words,
  // instead of a program of no moves that would look like a strategy that found nothing to cut.
  let rail = Contour::default();
  let keel = params(6.0);
  let cutter = tool(6.0, None);
  let error = moulding(&request(&rail, &keel, &cutter, &job())).expect_err("no rail, no sweep");
  match error {
    ToolpathError::MouldingGeometry { reason } => {
      assert!(
        reason.contains("a moulding rail needs a length"),
        "{reason}"
      );
    }
    other => panic!("{other:?}"),
  }
}
