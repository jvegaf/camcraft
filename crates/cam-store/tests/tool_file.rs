//! Behaviour tests for the tool-library file (tool-library-editor T5).

use cam_store::tool_edit::factory_tools;
use cam_store::tool_file::{from_json, to_json, ToolFileError, FORMAT, VERSION};
use cam_store::tools::{CuttingData, StoredTool, ToolGeometry, ToolKind};
use cam_units::{Angle, Length, UnitSystem};

fn inch_library_tool() -> StoredTool {
  StoredTool {
    geometry: ToolGeometry {
      id: "G1".to_owned(),
      name_format: "Ball nose · 6 mm".to_owned(),
      tool_type: ToolKind::BallNose.raw(),
      units: UnitSystem::Mm,
      diameter: Some(Length::from_mm(6.0)),
      included_angle: None,
      flat_diameter: None,
      num_flutes: Some(2),
      flute_length: Some(Length::from_mm(20.0)),
      tip_radius: Some(Length::from_mm(3.0)),
      shank_diameter: Some(Length::from_mm(6.0)),
      overall_length: Some(Length::from_mm(60.0)),
      custom_attributes: None,
    },
    cutting: CuttingData {
      id: "G1-cut".to_owned(),
      // Authored in inches: the file has to normalise it, not copy the number across.
      rate_units: UnitSystem::Inch,
      feed_rate: Some(1.0),
      plunge_rate: Some(0.5),
      spindle_speed: Some(12_000),
      stepdown: Some(Length::from_mm(1.5)),
      stepover: Some(Length::from_mm(2.5)),
      clear_stepover: None,
      tool_number: Some(1),
      notes: None,
    },
  }
}

#[test]
fn a_library_round_trips_through_a_file() {
  let tools = factory_tools();
  let text = to_json(&tools).expect("writes");
  let read = from_json(&text).expect("reads");
  assert_eq!(read.len(), tools.len());
  for (before, after) in tools.iter().zip(read.iter()) {
    assert_eq!(after.geometry.id, before.geometry.id);
    assert_eq!(after.geometry.diameter, before.geometry.diameter);
    assert_eq!(after.geometry.tip_radius, before.geometry.tip_radius);
    assert_eq!(
      after.geometry.included_angle,
      before.geometry.included_angle
    );
    assert_eq!(after.cutting.tool_number, before.cutting.tool_number);
    assert_eq!(after.cutting.feed_rate, before.cutting.feed_rate);
    assert_eq!(
      ToolKind::of(&after.geometry),
      ToolKind::of(&before.geometry),
      "a kind survives the round trip, which an integer in a file would not"
    );
  }
}

#[test]
fn a_rate_crosses_the_file_without_a_conversion() {
  let text = to_json(&[inch_library_tool()]).expect("writes");
  // The library keeps millimetres per minute and so does the file, so the number crosses unchanged. The
  // field's own name is what says which unit it is.
  assert!(
    text.contains("\"feed_mm_per_min\": 1.0"),
    "the exported feed is the stored number: {text}"
  );
  let read = from_json(&text).expect("reads");
  assert_eq!(
    read[0].cutting.feed_rate,
    Some(1.0),
    "a feed crosses a file as the same millimetres per minute the library keeps"
  );
  assert_eq!(
    read[0].cutting.rate_units,
    UnitSystem::Mm,
    "the authored unit is a display preference and not part of what a file stores"
  );
}

#[test]
fn a_file_that_is_not_a_library_is_refused_by_name() {
  let text = format!("{{\"format\":\"something-else\",\"version\":{VERSION},\"tools\":[]}}");
  assert!(matches!(
    from_json(&text),
    Err(ToolFileError::NotALibrary { .. })
  ));
}

#[test]
fn a_newer_version_is_refused_rather_than_partly_read() {
  let text = format!("{{\"format\":\"{FORMAT}\",\"version\":99,\"tools\":[]}}");
  match from_json(&text) {
    Err(ToolFileError::UnsupportedVersion { found }) => assert_eq!(found, 99),
    other => panic!("expected a version refusal, got {other:?}"),
  }
}

#[test]
fn a_kind_this_build_does_not_know_is_refused_and_named() {
  let text = format!(
    "{{\"format\":\"{FORMAT}\",\"version\":{VERSION},\"tools\":[{{\"id\":\"G1\",\"name\":\"Mystery\",\
\"kind\":\"laser\",\"diameter_mm\":6.0,\"included_angle_deg\":null,\"flat_diameter_mm\":null,\
\"flutes\":2,\"flute_length_mm\":null,\"tip_radius_mm\":null,\"shank_diameter_mm\":null,\
\"overall_length_mm\":null,\"feed_mm_per_min\":null,\"plunge_mm_per_min\":null,\"spindle_rpm\":null,\
\"stepdown_mm\":null,\"stepover_mm\":null,\"tool_number\":1}}]}}"
  );
  match from_json(&text) {
    Err(ToolFileError::UnknownKind { tool, kind }) => {
      assert_eq!(tool, "Mystery");
      assert_eq!(kind, "laser");
    }
    other => panic!("expected an unknown-kind refusal, got {other:?}"),
  }
}

#[test]
fn a_file_holding_one_impossible_tool_is_refused_whole() {
  let mut good = factory_tools()[0].clone();
  good.cutting.tool_number = Some(1);
  let mut bad = factory_tools()[2].clone();
  bad.cutting.tool_number = Some(2);
  bad.cutting.stepover = Some(Length::from_mm(99.0));
  let text = to_json(&[good, bad]).expect("writes");
  match from_json(&text) {
    Err(ToolFileError::Invalid { tool, problem }) => {
      assert_eq!(tool, "Ball nose · 6 mm");
      assert!(problem.sentence().contains("stepover"), "{problem}");
    }
    other => panic!("expected an invalid-tool refusal, got {other:?}"),
  }
}

#[test]
fn a_drill_and_a_v_bit_stay_apart_through_a_file() {
  let mut drill = factory_tools()[0].clone();
  drill.geometry.tool_type = ToolKind::Drill.raw();
  drill.geometry.included_angle = Some(Angle::from_degrees(118.0));
  let text = to_json(&[drill]).expect("writes");
  let read = from_json(&text).expect("reads");
  assert_eq!(ToolKind::of(&read[0].geometry), ToolKind::Drill);
}

#[test]
fn an_isolation_mill_stays_a_real_type_through_a_file() {
  let mut tool = inch_library_tool();
  tool.geometry.tool_type = ToolKind::IsolationMill.raw();
  tool.geometry.included_angle = Some(Angle::from_degrees(30.0));
  tool.geometry.flat_diameter = Some(Length::from_mm(0.1));
  let text = to_json(&[tool]).expect("writes");
  assert!(
    text.contains("\"kind\": \"isolation-mill\""),
    "the file names the kind by its stable slug: {text}"
  );
  let read = from_json(&text).expect("reads");
  assert_eq!(ToolKind::of(&read[0].geometry), ToolKind::IsolationMill);
  assert_eq!(
    read[0].geometry.included_angle,
    Some(Angle::from_degrees(30.0))
  );
  assert_eq!(read[0].geometry.flat_diameter, Some(Length::from_mm(0.1)));
}
