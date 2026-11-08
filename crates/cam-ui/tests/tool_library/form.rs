use cam_app::AppState;
use cam_ui::tool_library::{form_fields, form_groups, ToolFormGroup};
use cam_ui::Shell;
use cam_units::{Length, UnitSystem};

use super::support::*;

// ---------------------------------------------------------------------------------------------
// The detail side: the fields the family has, and the groups the tool has
// ---------------------------------------------------------------------------------------------

#[test]
fn choosing_a_row_describes_that_tool_and_its_family_alone() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  let opened = open_library(&ctx, &mut shell, &mut app);

  // The flat end mill has three shape fields, and no included angle to show: showing one as `—` would
  // describe a shape it is not.
  let (flat, flat_texts) = click_word(&ctx, &mut shell, &mut app, &opened, "T1 - Tool (6mm)");
  assert_eq!(flat.drawn.tool_selected.as_deref(), Some("G1"));
  assert_eq!(
    fields_of(&flat, "GEOMETRY"),
    vec!["Diameter", "Flute length", "Flutes"],
    "the flat end mill's GEOMETRY is not the flat end mill's"
  );
  assert!(
    !joined(&flat_texts).contains("Included angle"),
    "a flat end mill drew an included angle: {}",
    joined(&flat_texts)
  );

  // The V-bit has five, and the two the cone adds are on its own face with their own numbers.
  let (vbit, vbit_texts) = click_word(&ctx, &mut shell, &mut app, &flat, "T3 - Tool (12.7mm)");
  assert_eq!(vbit.drawn.tool_selected.as_deref(), Some("G3"));
  assert_eq!(
    fields_of(&vbit, "GEOMETRY"),
    vec![
      "Diameter",
      "Included angle",
      "Flat tip",
      "Flute length",
      "Flutes"
    ]
  );
  assert!(
    joined(&vbit_texts).contains("Included angle") && joined(&vbit_texts).contains("90.0°"),
    "the V-bit did not draw its own angle: {}",
    joined(&vbit_texts)
  );
  assert!(
    joined(&vbit_texts).contains("Ø 12.70 mm"),
    "a value is written without its unit: {}",
    joined(&vbit_texts)
  );

  // A corner radius keeps its radius and calls it what it is.
  let mut with_corner = Shell::new();
  with_corner.set_tools(vec![corner_radius("G4", Some(2), 12.0, 2.0)]);
  with_corner.open_tool_library(&app);
  let (cornered, _) = frame_with(&ctx, &mut with_corner, &mut app);
  assert_eq!(
    fields_of(&cornered, "GEOMETRY"),
    vec!["Diameter", "Corner radius", "Flute length", "Flutes"]
  );
  // And the same radius on a ball nose is a ball radius, because half the diameter is what makes a ball.
  let mut with_ball = Shell::new();
  with_ball.set_tools(vec![ball_nose("G2", Some(2), 6.0)]);
  with_ball.open_tool_library(&app);
  let (balled, _) = frame_with(&ctx, &mut with_ball, &mut app);
  assert!(fields_of(&balled, "GEOMETRY").contains(&"Ball radius".to_owned()));
  assert!(!fields_of(&balled, "GEOMETRY").contains(&"Corner radius".to_owned()));
}

#[test]
fn the_form_draws_only_the_groups_the_tool_has_numbers_for() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(7));
  let mut shell = Shell::new();
  // A tool the library only half describes: no pass and no rate.
  let mut bare = flat_mill("G7", Some(7), 6.0);
  bare.cutting.feed_rate = None;
  bare.cutting.plunge_rate = None;
  bare.cutting.spindle_speed = None;
  shell.set_tools(vec![bare]);
  shell.open_tool_library(&app);

  let (drawn, texts) = frame_with(&ctx, &mut shell, &mut app);
  assert_eq!(
    group_labels(&drawn),
    vec!["GEOMETRY".to_owned(), "MACHINE".to_owned()],
    "a heading over nothing is a claim about a tool nobody measured"
  );
  for absent in ["PASSES", "SPEEDS", "Stepdown", "Feed"] {
    assert!(
      !joined(&texts).contains(absent),
      "the form drew {absent:?} for a tool that has no such number"
    );
  }
  // `MACHINE` is always there: every record carries the units its rates were written in, and the number the
  // program names the tool by when there is one.
  assert_eq!(
    drawn
      .drawn
      .tool_groups
      .iter()
      .find(|(label, _)| label == "MACHINE")
      .map(|(_, fields)| fields.clone()),
    Some(vec![
      ("Tool number".to_owned(), "T7".to_owned()),
      ("Rates".to_owned(), "mm/s".to_owned()),
    ])
  );

  // A tool with all six numbers draws all four groups, in S9's own order.
  let mut full = flat_mill("G8", Some(7), 6.0);
  full.cutting.stepdown = Some(Length::from_mm(2.0));
  full.cutting.stepover = Some(Length::from_mm(3.0));
  full.cutting.clear_stepover = Some(Length::from_mm(5.0));
  let mut other = Shell::new();
  other.set_tools(vec![full]);
  other.open_tool_library(&app);
  let (whole, _) = frame_with(&ctx, &mut other, &mut app);
  assert_eq!(
    group_labels(&whole),
    vec![
      "GEOMETRY".to_owned(),
      "PASSES".to_owned(),
      "SPEEDS".to_owned(),
      "MACHINE".to_owned()
    ],
    "the four groups S9 names must come in S9's order"
  );
  assert_eq!(
    form_groups(&flat_mill("G1", Some(1), 6.0)),
    vec![
      ToolFormGroup::Geometry,
      ToolFormGroup::Speeds,
      ToolFormGroup::Machine
    ]
  );
}

#[test]
fn a_rate_is_shown_in_the_units_the_library_authored_it_in() {
  // R3-001 in this task's review: stored rates are millimetres per second, but `rate_units` records the units
  // the library's author typed them in and `MACHINE` names that unit — so one number was carrying two units at
  // once (`Feed 100.00 mm/s` beside `Rates in/s`). Converted to the authored unit, both rows agree.
  let mut inch = flat_mill("T1", Some(1), 6.0);
  inch.cutting.rate_units = UnitSystem::Inch;
  let feed = form_fields(ToolFormGroup::Speeds, &inch)
    .into_iter()
    .find(|(label, _)| label == "Feed")
    .expect("the speeds group offers a feed")
    .1;
  assert_eq!(
    feed, "3.94 in/s",
    "an inch library's feed reads in its own units"
  );
  let rates = form_fields(ToolFormGroup::Machine, &inch)
    .into_iter()
    .find(|(label, _)| label == "Rates")
    .expect("the machine group names the rates")
    .1;
  assert_eq!(rates, "in/s", "and the machine row it is compared against");

  // The millimetre path is what it was: this fix must not move the number a mm library reads.
  let mm = flat_mill("T2", Some(2), 6.0);
  let feed = form_fields(ToolFormGroup::Speeds, &mm)
    .into_iter()
    .find(|(label, _)| label == "Feed")
    .expect("the speeds group offers a feed")
    .1;
  assert_eq!(feed, "100.00 mm/s");
}
