use cam_app::AppState;
use cam_store::tools::{StoredTool, ToolKind};
use cam_ui::tool_library::{FormField, ShapeField, ToolFormGroup};
use cam_ui::Shell;

use super::support::*;

// ---------------------------------------------------------------------------------------------
// S4, S5 and S7: the name, the kind, the geometry rows and the machine number
// ---------------------------------------------------------------------------------------------

#[test]
fn the_type_combo_offers_the_six_kinds_in_s4s_order_and_a_choice_reaches_the_draft() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  let opened = open_library(&ctx, &mut shell, &mut app);

  // S4's list, read from the one place it is written down.
  let labels: Vec<&str> = ToolKind::ALL.iter().map(|kind| kind.label()).collect();
  assert_eq!(
    labels,
    vec![
      "Flat end mill",
      "Ball nose",
      "Corner radius",
      "V-bit",
      "Engraving",
      "Drill",
    ],
    "S4 fixes these six kinds in this order, and ToolKind::ALL is what the combo iterates"
  );

  // The combo is a row labelled `Type` whose box opens the list, so the list that is offered is read from the
  // frame that drew it, under that row and inside the detail column.
  let (_, runs) = frame_with_runs(&ctx, &mut shell, &mut app);
  let at = box_point(&runs, "Type");
  let card = opened
    .drawn
    .tool_drawing
    .expect("the preview drew no card")
    .0;
  let (_, _) = click_at(&ctx, &mut shell, &mut app, at);
  let (_, runs) = frame_with_runs(&ctx, &mut shell, &mut app);
  let offered: Vec<String> = runs
    .iter()
    .filter(|(text, point, _)| point.x < card && point.y > at.y && labels.contains(&text.as_str()))
    .map(|(text, _, _)| text.clone())
    .collect();
  assert_eq!(
    offered, labels,
    "the combo offers {:?} rather than S4's six kinds in S4's order",
    offered
  );

  // Choosing one is a choice about the draft, and the preview's caption follows it in the same frame.
  let drill = runs
    .iter()
    .find(|(text, point, _)| text == "Drill" && point.x < card && point.y > at.y)
    .map(|(_, point, size)| egui::pos2(point.x + size.x / 2.0, point.y + size.y / 2.0))
    .expect("the combo drew no Drill");
  let (chosen, texts) = click_at(&ctx, &mut shell, &mut app, drill);
  let draft = shell
    .tool_draft
    .as_ref()
    .expect("the draft survives the choice");
  assert_eq!(
    draft.kind,
    ToolKind::Drill,
    "choosing Drill left the draft describing a {}",
    draft.kind.label()
  );
  assert_eq!(
    chosen.drawn.tool_preview_caption.as_deref(),
    Some("Drill"),
    "the preview is captioned with the kind the form no longer shows"
  );
  assert!(
    joined(&texts).contains("Drill"),
    "the caption was reported and never painted: {}",
    joined(&texts)
  );
  // A drill's tip is a cone, so the angle row appears with the choice (S5).
  assert!(fields_of(&chosen, "GEOMETRY").contains(&"Included angle".to_owned()));
}

#[test]
fn the_geometry_rows_follow_the_kind_and_keep_s5s_order() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  let opened = open_library(&ctx, &mut shell, &mut app);

  // A flat end mill: S5's five rows, and no angle to describe a square end.
  let (flat, flat_texts) = click_word(
    &ctx,
    &mut shell,
    &mut app,
    &opened,
    "T1 - Flat end mill (6mm)",
  );
  assert_eq!(flat.drawn.tool_selected.as_deref(), Some("G1"));
  assert_eq!(
    fields_of(&flat, "GEOMETRY"),
    vec![
      "Diameter",
      "Flute length",
      "Shank diameter",
      "Overall length",
      "Number of cuts",
    ],
    "the flat end mill's GEOMETRY is not S5's rows in S5's order"
  );
  assert!(
    !joined(&flat_texts).contains("Included angle"),
    "a flat end mill drew an included angle: {}",
    joined(&flat_texts)
  );
  assert!(
    !fields_of(&flat, "GEOMETRY").contains(&"Corner radius".to_owned()),
    "a flat end mill drew a corner radius"
  );

  // A V-bit: the angle sits where S5 puts it, second, and carries the tool's own number.
  let (vbit, vbit_texts) = click_word(&ctx, &mut shell, &mut app, &flat, "T4 - V-bit (12.7mm)");
  assert_eq!(
    fields_of(&vbit, "GEOMETRY"),
    vec![
      "Diameter",
      "Included angle",
      "Flute length",
      "Shank diameter",
      "Overall length",
      "Number of cuts",
    ]
  );
  assert_eq!(value_of(&vbit, "GEOMETRY", "Included angle"), "90");
  assert_eq!(value_of(&vbit, "GEOMETRY", "Diameter"), "12.7");
  assert!(
    joined(&vbit_texts).contains("Included angle"),
    "the V-bit did not paint its own angle: {}",
    joined(&vbit_texts)
  );

  // A corner radius: the radius its name comes from, in the angle's own place (the deviation L5 records).
  let (corner, _) = click_word(
    &ctx,
    &mut shell,
    &mut app,
    &vbit,
    "T3 - Corner radius (12mm)",
  );
  assert_eq!(
    fields_of(&corner, "GEOMETRY"),
    vec![
      "Diameter",
      "Corner radius",
      "Flute length",
      "Shank diameter",
      "Overall length",
      "Number of cuts",
    ]
  );
  assert_eq!(value_of(&corner, "GEOMETRY", "Corner radius"), "2");
  assert!(!fields_of(&corner, "GEOMETRY").contains(&"Included angle".to_owned()));
}

#[test]
fn the_machine_group_carries_the_tool_number() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  let opened = open_library(&ctx, &mut shell, &mut app);
  let (chosen, texts) = click_word(&ctx, &mut shell, &mut app, &opened, "T4 - V-bit (12.7mm)");

  assert_eq!(fields_of(&chosen, "MACHINE"), vec!["Tool number"]);
  assert_eq!(value_of(&chosen, "MACHINE", "Tool number"), "4");
  assert!(joined(&texts).contains("Tool number"));
  // S7 draws it as `T` followed by the number's box, and the label the form uses is the group's own.
  assert_eq!(FormField::ToolNumber.label(), "Tool number");
  assert_eq!(FormField::ToolNumber.group(), ToolFormGroup::Machine);
  assert!(FormField::ToolNumber.editable());
  assert_eq!(FormField::ToolNumber.unit(cam_units::UnitSystem::Mm), None);
}

#[test]
fn the_form_rows_of_a_record_are_the_kinds_rows_and_then_the_three_fixed_groups() {
  // The API the form and the strategy editor's quick block share: one tool's rows, grouped, in draw order.
  let tool: StoredTool = flat_mill("G1", Some(1), 6.0);
  let rows = cam_ui::tool_library::form_rows(&tool);
  assert_eq!(
    rows.iter().map(|(group, _)| *group).collect::<Vec<_>>(),
    vec![
      ToolFormGroup::Geometry,
      ToolFormGroup::Passes,
      ToolFormGroup::Speeds,
      ToolFormGroup::Machine,
    ]
  );
  assert_eq!(
    rows[0].1,
    ShapeField::of(ToolKind::FlatEndMill)
      .iter()
      .map(|field| FormField::Shape(*field))
      .collect::<Vec<_>>(),
    "the GEOMETRY rows are not the kind's own shape rows"
  );
  assert_eq!(
    rows[1].1,
    vec![
      FormField::Stepdown,
      FormField::Stepover,
      FormField::StepoverPercent,
    ]
  );
  assert_eq!(
    rows[2].1,
    vec![
      FormField::Spindle,
      FormField::Feed,
      FormField::Plunge,
      FormField::Chipload,
    ]
  );
  assert_eq!(rows[3].1, vec![FormField::ToolNumber]);
}
