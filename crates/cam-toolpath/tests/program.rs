//! T4's seam, pinned in the library: a document's toolpaths become one program, and the tool a
//! toolpath names is resolved against the specs the caller handed over.
//!
//! The program builder used to live in `cam-cli`'s binary, so the only way to reach it was to run the
//! emitter. These tests call [`program_for`] directly, which is what the shell's estimate needs: the
//! moves a document's toolpaths produce, without a process, a post-processor or a library on disk.

use cam_geom::{Contour, Point2};
use cam_model::{
  Document, EngraveParams, Entity, EntityKind, Job, Layer, Operation, OriginCorner, ProfileParams,
  ProfileSide, Toolpath,
};
use cam_move::ToolShape;
use cam_toolpath::{profile, program_for, ProfileRequest, ProgramTool, ToolSpec, ToolpathError};
use cam_units::{Angle, Length};

fn tool(number: u32, diameter: f64) -> ToolSpec {
  ToolSpec {
    number,
    diameter: Length::from_mm(diameter),
    included_angle: None,
    feed_rate: 100.0,
    plunge_rate: 30.0,
    spindle_speed: 18_000,
  }
}

/// A sharp tool: the mapping a caller makes when its library records no tip flat.
fn sharp(spec: ToolSpec) -> ProgramTool {
  ProgramTool {
    spec,
    flat_diameter: Length::ZERO,
  }
}

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

fn profile_params() -> ProfileParams {
  ProfileParams {
    side: ProfileSide::Outside,
    allowance: Length::ZERO,
    pass_depth: Length::from_mm(6.0),
    start_depth: Length::ZERO,
    cut_depth: Length::from_mm(6.0),
    lead_in: Length::ZERO,
    lead_out: Length::ZERO,
    tabs: Vec::new(),
  }
}

fn outside_profile(id: &str, name: &str, tool_number: u32) -> Toolpath {
  Toolpath {
    id: id.to_owned(),
    name: name.to_owned(),
    entity_ids: vec!["E1".to_owned()],
    island_ids: Vec::new(),
    tool_number,
    operation: Operation::Profile(profile_params()),
  }
}

/// One layer holding one rectangle, `E1`, and the toolpaths given.
fn document_with(toolpaths: Vec<Toolpath>) -> (Document, Contour) {
  let contour = rectangle(40.0, 30.0);
  let mut document = Document::new(Job {
    origin: OriginCorner::BottomLeft,
    width: Length::from_mm(100.0),
    height: Length::from_mm(100.0),
    ..Job::default_sheet()
  });
  document.layers.push(Layer {
    id: "L1".to_owned(),
    name: "Part".to_owned(),
    visible: true,
    entities: vec![Entity {
      id: "E1".to_owned(),
      kind: EntityKind::Contour {
        contour: contour.clone(),
      },
    }],
  });
  document.toolpaths = toolpaths;
  (document, contour)
}

#[test]
fn one_toolpath_becomes_the_moves_the_strategy_produces() {
  let (document, contour) = document_with(vec![outside_profile("TP1", "Outside", 1)]);
  let spec = tool(1, 6.0);

  let program = program_for(&document, &[sharp(spec)]).expect("a program");

  // One toolpath, one segment: the program is the document's toolpaths in order, and each segment is
  // the one tool the toolpath named.
  assert_eq!(program.segments.len(), 1);
  let segment = &program.segments[0];
  assert_eq!(segment.name, "Outside");
  assert_eq!(segment.tool_number, 1);
  assert_eq!(segment.tool, ToolShape::cylinder(Length::from_mm(6.0)));

  // The rates come from the caller's spec, not from anywhere else: the builder reads no library.
  assert_eq!(segment.feed_rate, 100.0);
  assert_eq!(segment.plunge_rate, 30.0);
  assert_eq!(segment.spindle_speed, 18_000);

  // The seam itself: the segment's moves are exactly the strategy's, so the program builder adds the
  // loop and the per-tool decisions and nothing about the cutting.
  let expected = profile(&ProfileRequest {
    contour: &contour,
    params: &profile_params(),
    tool: &spec,
    job: &document.job,
  })
  .expect("the strategy's own moves");
  assert!(!expected.is_empty());
  assert_eq!(segment.moves, expected);

  // The machine's clearance reference is the job's, so the emitter and the simulation see one program.
  assert_eq!(program.safe_z, document.job.safe_z);
  assert_eq!(program.move_count(), expected.len());
}

#[test]
fn two_toolpaths_come_back_in_document_order_whatever_the_specs_order() {
  let (document, _) = document_with(vec![
    outside_profile("TP1", "First", 1),
    outside_profile("TP2", "Second", 2),
  ]);

  // The specs are handed over in the other order: a toolpath finds its tool by number, never by
  // position.
  let program =
    program_for(&document, &[sharp(tool(2, 3.0)), sharp(tool(1, 6.0))]).expect("a program");

  assert_eq!(
    program
      .segments
      .iter()
      .map(|segment| (segment.name.as_str(), segment.tool_number))
      .collect::<Vec<_>>(),
    vec![("First", 1), ("Second", 2)]
  );
  assert_eq!(
    program.segments[1].tool,
    ToolShape::cylinder(Length::from_mm(3.0))
  );
}

#[test]
fn a_tool_number_the_specs_do_not_hold_is_refused_by_name() {
  let (document, _) = document_with(vec![outside_profile("TP1", "Outside", 7)]);

  // The library is not the builder's business (S6): the caller decides which tools exist, so the
  // refusal has to say which number the list it was handed did not hold — and which toolpath asked
  // for it — in the sentence `cam-cli` has always printed.
  let error = program_for(&document, &[sharp(tool(1, 6.0))]).expect_err("the tool is missing");

  assert_eq!(
    error,
    ToolpathError::ToolNotInLibrary {
      tool_number: 7,
      toolpath: "TP1".to_owned(),
    }
  );
  assert_eq!(
    error.to_string(),
    "tool T7 used by toolpath \"TP1\" is not in the library"
  );
}

#[test]
fn an_engrave_still_carries_the_bits_real_shape() {
  // Preserved while M8 added moulding to this arm of the shape match: an authored-depth groove says
  // nothing about the cutter, so the program carries the bit the library describes — a V-bit when it
  // records an angle, a cylinder otherwise (M6 A8).
  let (document, _) = document_with(vec![Toolpath {
    id: "TP1".to_owned(),
    name: "Engrave".to_owned(),
    entity_ids: vec!["E1".to_owned()],
    island_ids: Vec::new(),
    tool_number: 3,
    operation: Operation::Engrave(EngraveParams::default()),
  }]);

  let angled = ToolSpec {
    included_angle: Some(Angle::from_degrees(60.0)),
    ..tool(3, 0.2)
  };
  let program = program_for(&document, &[sharp(angled)]).expect("a program");
  assert_eq!(
    program.segments[0].tool,
    ToolShape::v_bit(Angle::from_degrees(60.0), Length::ZERO)
  );

  let program = program_for(&document, &[sharp(tool(3, 3.0))]).expect("a program");
  assert_eq!(
    program.segments[0].tool,
    ToolShape::cylinder(Length::from_mm(3.0))
  );
}
