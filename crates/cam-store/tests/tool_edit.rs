//! Behaviour tests for writing tools into a library (tool-library-editor T1).

use cam_store::tool_edit::{
  duplicate_tool, factory_tools, new_tool, next_tool_number, rendered_name, validate_tool,
  ToolProblem, WriteToolError,
};
use cam_store::tools::{
  head_version, CuttingData, StoredTool, ToolGeometry, ToolKind, ToolLibrary,
};
use cam_units::{Angle, Length, UnitSystem};

/// A tool that passes validation, so each test can break exactly one thing.
fn sound(number: u32) -> StoredTool {
  StoredTool {
    geometry: ToolGeometry {
      id: format!("G{number}"),
      name_format: format!("Flat end mill · {number}"),
      tool_type: ToolKind::FlatEndMill.raw(),
      units: UnitSystem::Mm,
      diameter: Some(Length::from_mm(6.0)),
      included_angle: None,
      flat_diameter: None,
      num_flutes: Some(2),
      flute_length: Some(Length::from_mm(20.0)),
      tip_radius: None,
      shank_diameter: Some(Length::from_mm(6.0)),
      overall_length: Some(Length::from_mm(60.0)),
      custom_attributes: None,
    },
    cutting: CuttingData {
      id: format!("G{number}-cut"),
      rate_units: UnitSystem::Mm,
      feed_rate: Some(100.0),
      plunge_rate: Some(30.0),
      spindle_speed: Some(18_000),
      stepdown: Some(Length::from_mm(1.5)),
      stepover: Some(Length::from_mm(2.5)),
      clear_stepover: None,
      tool_number: Some(number),
      notes: None,
    },
  }
}

#[test]
fn a_fresh_library_carries_the_columns_an_editor_writes() {
  let library = ToolLibrary::open_in_memory().expect("opens");
  assert_eq!(library.schema_version().expect("version"), head_version());
  assert!(
    head_version() >= 3,
    "the shank and overall length are columns, not fields of the name format"
  );
  library.save_tool(&sound(1)).expect("saves");
  let read = library.geometry("G1").expect("reads").expect("present");
  assert_eq!(read.shank_diameter, Some(Length::from_mm(6.0)));
  assert_eq!(read.overall_length, Some(Length::from_mm(60.0)));
}

#[test]
fn a_row_without_a_declared_kind_reads_as_the_shape_it_proves() {
  // What `cam-cli tools add` has always written: a raw zero and a diameter.
  let mut legacy = sound(1);
  legacy.geometry.tool_type = ToolKind::LEGACY_RAW;
  assert_eq!(ToolKind::of(&legacy.geometry), ToolKind::FlatEndMill);

  let mut ball = legacy.clone();
  ball.geometry.tip_radius = Some(Length::from_mm(3.0));
  assert_eq!(ToolKind::of(&ball.geometry), ToolKind::BallNose);

  let mut corner = legacy.clone();
  corner.geometry.tip_radius = Some(Length::from_mm(1.5));
  assert_eq!(ToolKind::of(&corner.geometry), ToolKind::CornerRadius);

  let mut vee = legacy.clone();
  vee.geometry.included_angle = Some(Angle::from_degrees(90.0));
  assert_eq!(ToolKind::of(&vee.geometry), ToolKind::VBit);
}

#[test]
fn a_declared_kind_wins_over_the_shape_because_a_drill_is_not_a_v() {
  // The one case the geometry cannot decide: both carry an included angle.
  let mut drill = sound(1);
  drill.geometry.tool_type = ToolKind::Drill.raw();
  drill.geometry.included_angle = Some(Angle::from_degrees(118.0));
  assert_eq!(ToolKind::of(&drill.geometry), ToolKind::Drill);
  assert_eq!(ToolKind::from_shape(&drill.geometry), ToolKind::VBit);
  assert_eq!(
    ToolKind::from_raw(ToolKind::Drill.raw()),
    Some(ToolKind::Drill)
  );
  assert_eq!(ToolKind::from_raw(ToolKind::LEGACY_RAW), None);
}

#[test]
fn saving_a_tool_writes_the_geometry_the_cutting_data_and_the_link() {
  let library = ToolLibrary::open_in_memory().expect("opens");
  library.save_tool(&sound(1)).expect("saves");
  let tools = library.tools().expect("lists");
  assert_eq!(tools.len(), 1, "one tool, one row in the list");
  assert_eq!(tools[0].geometry.id, "G1");
  assert_eq!(tools[0].cutting.tool_number, Some(1));
  // Writing the same tool twice updates it rather than listing it twice.
  library.save_tool(&sound(1)).expect("saves again");
  assert_eq!(library.tools().expect("lists").len(), 1);
}

#[test]
fn a_tool_with_no_diameter_is_refused_and_nothing_is_written() {
  let library = ToolLibrary::open_in_memory().expect("opens");
  let mut broken = sound(1);
  broken.geometry.diameter = None;
  let refusal = library.save_tool(&broken).expect_err("refuses");
  assert!(matches!(
    refusal,
    WriteToolError::Invalid(ToolProblem::NoDiameter)
  ));
  assert!(library.tools().expect("lists").is_empty());
}

#[test]
fn a_stepover_wider_than_the_tool_is_refused() {
  let library = ToolLibrary::open_in_memory().expect("opens");
  let mut broken = sound(1);
  broken.cutting.stepover = Some(Length::from_mm(9.0));
  match library.save_tool(&broken).expect_err("refuses") {
    WriteToolError::Invalid(ToolProblem::StepoverWiderThanDiameter { stepover, diameter }) => {
      assert_eq!((stepover, diameter), (9.0, 6.0));
    }
    other => panic!("expected a stepover refusal, got {other:?}"),
  }
  assert!(library.tools().expect("lists").is_empty());
}

#[test]
fn a_tool_without_a_number_is_refused_because_no_program_could_name_it() {
  let mut broken = sound(1);
  broken.cutting.tool_number = None;
  assert_eq!(validate_tool(&broken), Err(ToolProblem::NoToolNumber));
  assert_eq!(
    validate_tool(&sound(1)),
    Ok(()),
    "the sound tool is the control"
  );
}

#[test]
fn an_overall_length_shorter_than_the_flutes_is_refused() {
  let mut broken = sound(1);
  broken.geometry.overall_length = Some(Length::from_mm(10.0));
  assert!(matches!(
    validate_tool(&broken),
    Err(ToolProblem::OverallShorterThanFlute { .. })
  ));
}

#[test]
fn removing_a_tool_takes_its_rows_with_it() {
  let library = ToolLibrary::open_in_memory().expect("opens");
  library.save_tool(&sound(1)).expect("saves");
  library.save_tool(&sound(2)).expect("saves");
  assert!(library.remove_tool("G1").expect("removes"));
  let left = library.tools().expect("lists");
  assert_eq!(left.len(), 1);
  assert_eq!(left[0].geometry.id, "G2");
  assert!(
    !library.remove_tool("G1").expect("removes"),
    "the second removal has nothing to remove and says so"
  );
  // A removed tool's ids are free again: writing it back works and lists once.
  library.save_tool(&sound(1)).expect("saves again");
  assert_eq!(library.tools().expect("lists").len(), 2);
}

#[test]
fn replacing_the_library_is_all_or_nothing() {
  let library = ToolLibrary::open_in_memory().expect("opens");
  library.save_tool(&sound(7)).expect("saves");
  let mut broken = sound(8);
  broken.cutting.tool_number = Some(0);
  let refusal = library
    .replace_tools(&[sound(1), broken])
    .expect_err("refuses");
  assert!(matches!(
    refusal,
    WriteToolError::Invalid(ToolProblem::ToolNumberNotPositive)
  ));
  let kept = library.tools().expect("lists");
  assert_eq!(kept.len(), 1, "one bad tool changes nothing at all");
  assert_eq!(kept[0].cutting.tool_number, Some(7));
}

#[test]
fn replacing_the_library_installs_exactly_the_factory_set() {
  let library = ToolLibrary::open_in_memory().expect("opens");
  library.save_tool(&sound(9)).expect("saves");
  library.replace_tools(&factory_tools()).expect("restores");
  let tools = library.tools().expect("lists");
  assert_eq!(tools.len(), factory_tools().len());
  assert!(
    tools.iter().all(|tool| tool.cutting.tool_number != Some(9)),
    "the tool the library held before is gone, because a restore is a replacement"
  );
  for tool in &tools {
    assert_eq!(
      validate_tool(tool),
      Ok(()),
      "{} is a tool the factory set claims is valid",
      tool.geometry.id
    );
  }
}

#[test]
fn the_factory_set_covers_the_shapes_the_drawing_knows() {
  let kinds: Vec<ToolKind> = factory_tools()
    .iter()
    .map(|tool| ToolKind::of(&tool.geometry))
    .collect();
  assert!(kinds.contains(&ToolKind::FlatEndMill));
  assert!(kinds.contains(&ToolKind::BallNose));
  assert!(kinds.contains(&ToolKind::VBit));
}

#[test]
fn a_new_tool_number_fills_the_lowest_gap() {
  let tools = vec![sound(1), sound(3)];
  assert_eq!(next_tool_number(&tools), 2);
  assert_eq!(next_tool_number(&[]), 1);
  assert_eq!(next_tool_number(&[sound(1), sound(2)]), 3);
}

#[test]
fn a_duplicate_keeps_the_shape_and_says_it_is_a_copy() {
  let original = sound(1);
  let copy = duplicate_tool(&original, "G1-copy", 2);
  assert_eq!(copy.geometry.id, "G1-copy");
  assert_eq!(copy.cutting.id, "G1-copy-cut");
  assert_eq!(copy.cutting.tool_number, Some(2));
  assert_eq!(copy.geometry.diameter, original.geometry.diameter);
  assert_eq!(copy.geometry.tip_radius, original.geometry.tip_radius);
  assert_eq!(rendered_name(&copy), "Flat end mill · 1 (copy)");
  assert_eq!(
    rendered_name(&original),
    "Flat end mill · 1",
    "the original is untouched"
  );
}

#[test]
fn a_new_tool_is_valid_and_starts_from_the_form_s_own_defaults() {
  let fresh = new_tool("G-new", "New tool", 4);
  assert_eq!(validate_tool(&fresh), Ok(()));
  assert_eq!(fresh.cutting.tool_number, Some(4));
  assert_eq!(
    fresh.cutting.feed_rate, None,
    "a feed the library invented would be a number nobody measured"
  );
  assert_eq!(ToolKind::of(&fresh.geometry), ToolKind::FlatEndMill);
}

#[test]
fn a_templated_name_still_renders_and_a_plain_one_is_itself() {
  let mut templated = sound(1);
  templated.geometry.name_format = "{Tool Type} ({Diameter|F}{Units Short})".to_owned();
  assert_eq!(rendered_name(&templated), "Flat end mill (6mm)");
  assert_eq!(rendered_name(&sound(2)), "Flat end mill · 2");
}

#[test]
fn saving_a_tool_keeps_the_material_its_link_already_names() {
  // A library keeps one link per (tool, material, machine) — the normalisation is the reason the library is
  // worth having, since the same cutter feeds differently in MDF and in hardwood. `StoredTool` carries
  // neither column, so a writer that overwrote them would discard an association it cannot even express,
  // and would report the write as successful while doing it (review R4-001).
  let library = ToolLibrary::open_in_memory().expect("opens");
  library.save_tool(&sound(1)).expect("saves");
  library
    .link("G1-link", "G1", "G1-cut", Some("MDF"), None)
    .expect("links the tool to a material");
  assert!(
    library
      .cutting_data_for("G1", Some("MDF"), None)
      .expect("reads")
      .is_some(),
    "the fixture's own association has to exist before a save can be blamed for losing it"
  );

  library.save_tool(&sound(1)).expect("saves again");

  assert!(
    library
      .cutting_data_for("G1", Some("MDF"), None)
      .expect("reads")
      .is_some(),
    "saving a tool must not drop the material association its link already named"
  );
}

#[test]
fn a_depth_per_pass_that_is_not_a_pass_is_refused() {
  // `stepover` was checked from the first version and `stepdown` was not, so a negative, zero or NaN depth
  // of cut walked straight through the gate that `save_tool` and `from_json` both rely on (review
  // R3-stepdown-unvalidated).
  for impossible in [-1.0, 0.0, f64::NAN, f64::INFINITY] {
    let mut broken = sound(1);
    broken.cutting.stepdown = Some(Length::from_mm(impossible));
    match validate_tool(&broken) {
      Err(ToolProblem::StepdownNotPositive(_)) => {}
      other => panic!("a stepdown of {impossible} must be refused, got {other:?}"),
    }
  }
  let mut fine = sound(1);
  fine.cutting.stepdown = Some(Length::from_mm(1.5));
  assert_eq!(validate_tool(&fine), Ok(()), "and a real pass still passes");
  fine.cutting.stepdown = None;
  assert_eq!(
    validate_tool(&fine),
    Ok(()),
    "and so does a tool with no pass at all"
  );
}
