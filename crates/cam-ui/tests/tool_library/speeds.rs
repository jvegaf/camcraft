use cam_app::AppState;
use cam_ui::tool_library::{form_rows, FormField, ToolFormGroup};
use cam_ui::Shell;
use cam_units::UnitSystem;

use super::support::*;

// ---------------------------------------------------------------------------------------------
// S6: `PASSES` and `SPEEDS` are questions, and the chipload is arithmetic
// ---------------------------------------------------------------------------------------------

#[test]
fn passes_and_speeds_are_drawn_for_a_record_that_holds_no_rate_at_all() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(1));
  let mut shell = Shell::new();
  let empty = bare("G1", Some(1), 6.0);
  shell.set_tools(vec![empty.clone()]);
  shell.open_tool_library(&app);
  let _ = frame_with(&ctx, &mut shell, &mut app);
  let (drawn, texts) = frame_with(&ctx, &mut shell, &mut app);

  // The record holds no pass and no rate, and the two groups are still drawn: a box is a question, not a claim
  // about a tool the library measured.
  assert_eq!(
    group_labels(&drawn),
    vec![
      "GEOMETRY".to_owned(),
      "PASSES".to_owned(),
      "SPEEDS".to_owned(),
      "MACHINE".to_owned(),
    ],
    "a group went missing because the record has no number for it"
  );
  assert_eq!(
    fields_of(&drawn, "PASSES"),
    vec!["Depth per pass", "Stepover", "Stepover"],
    "PASSES holds the depth, the stepover in mm and the stepover as a percentage"
  );
  assert_eq!(
    fields_of(&drawn, "SPEEDS"),
    vec!["Spindle", "Feed", "Plunge", "Chipload"],
    "SPEEDS holds the spindle, the feed, the plunge and the computed chipload"
  );
  // Every box is empty rather than seeded with a zero nobody wrote.
  assert_eq!(value_of(&drawn, "SPEEDS", "Feed"), "");
  assert_eq!(value_of(&drawn, "SPEEDS", "Chipload"), "");
  assert!(
    joined(&texts).contains("PASSES") && joined(&texts).contains("SPEEDS"),
    "the two headings were reported and never painted: {}",
    joined(&texts)
  );
  assert_eq!(
    form_rows(&empty)
      .iter()
      .map(|(group, _)| *group)
      .collect::<Vec<_>>(),
    ToolFormGroup::ALL.to_vec()
  );
}

#[test]
fn the_chipload_is_feed_over_spindle_and_cuts_and_is_read_instead_of_typed() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(1));
  let mut shell = Shell::new();
  // The three numbers S6 states the rule over: 600 mm/min, 12000 rpm and two cuts.
  let tool = measured("G1", Some(1));
  shell.set_tools(vec![tool.clone()]);
  shell.open_tool_library(&app);
  let _ = frame_with(&ctx, &mut shell, &mut app);
  let (drawn, texts) = frame_with(&ctx, &mut shell, &mut app);

  let chipload = value_of(&drawn, "SPEEDS", "Chipload");
  assert_eq!(
    chipload,
    "0.025",
    "the chipload is not feed / (spindle x cuts): 600 / (12000 x 2) = {}",
    600.0 / (12_000.0 * 2.0)
  );
  // It is a reading, not a box: the report carries the rows that cannot be typed into, and the chipload is one
  // of them — with its own unit, drawn outside it.
  assert!(
    drawn
      .drawn
      .tool_computed
      .iter()
      .any(|(label, value)| label == "Chipload" && *value == chipload),
    "the chipload is not reported as a computed row: {:?}",
    drawn.drawn.tool_computed
  );
  assert!(joined(&texts).contains("Chipload"));
  assert!(!FormField::Chipload.editable());
  assert_eq!(FormField::Chipload.unit(UnitSystem::Mm), Some("mm"));
  assert_eq!(FormField::Chipload.group(), ToolFormGroup::Speeds);
  assert_eq!(FormField::Chipload.label(), "Chipload");

  // Pressing its box and typing a number changes nothing: there is no box there to type into, and the number
  // the row shows is still the arithmetic over the three rows above it.
  let (_, runs) = frame_with_runs(&ctx, &mut shell, &mut app);
  let (typed, _) = type_into_box(&ctx, &mut shell, &mut app, &runs, "Chipload", "99");
  assert_eq!(
    value_of(&typed, "SPEEDS", "Chipload"),
    "0.025",
    "the chipload took the number it was offered, so it is a fourth number the other three contradict"
  );
  assert_eq!(value_of(&typed, "SPEEDS", "Feed"), "600");
  let draft = shell.tool_draft.as_ref().expect("the draft is still open");
  let committed = draft.commit().expect("the draft still describes a tool");
  assert_eq!(committed.cutting, tool.cutting);
  assert_eq!(committed.geometry.diameter, tool.geometry.diameter);
}
