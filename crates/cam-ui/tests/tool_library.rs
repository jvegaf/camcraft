//! The tool library as a master-detail modal (camui-prancheta T7, spec S9).
//!
//! S9 draws one screen: a search field; a tree grouped by tool family with a count and one row per tool (mini
//! glyph, name, the dim `Ø6 · 2 cut(s)` line); a detail form grouped under `GEOMETRY` / `PASSES` / `SPEEDS` /
//! `MACHINE` whose fields change with the family; the tool drawn to scale with dimension callouts; and a
//! footer of library actions above `Cancel` / `Use this tool`.
//!
//! What these tests pin is behaviour, not shapes: the faces are painted, so the shell reports what it drew
//! ([`cam_ui::DrawnState::tool_library`] and its neighbours) and every claim below is read from that report or
//! from what the document holds afterwards. The two rules that matter most are that a control is drawn only
//! when it governs something (L6, L14, L22) and that a headless test reading shapes cannot tell a clipped
//! control from a visible one (L23) — which is why the reported hits are only points the frame really showed,
//! and why `Use this tool` is asserted through the action it emits and the document it changes.

use cam_app::AppState;
use cam_geom::{Contour, Point2};
use cam_model::{Document, Entity, EntityKind, Job, Layer, ProfileParams, Toolpath};
use cam_store::tools::{CuttingData, StoredTool, ToolGeometry};
use cam_ui::chrome::Category;
use cam_ui::tool_library::{
  cut_line, form_fields, form_groups, tool_title, ToolFamily, ToolFormGroup,
};
use cam_ui::{handle_action, Shell, ShellOutput, UiAction};
use cam_units::{Angle, Length, UnitSystem};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

/// A document with one rectangle and one profile toolpath cutting with `tool_number`.
fn document_with_toolpath(tool_number: u32) -> Document {
  let mut document = Document::new(Job::default_sheet());
  let contour = Contour::from_points(
    &[
      Point2::new(0.0, 0.0),
      Point2::new(100.0, 0.0),
      Point2::new(100.0, 60.0),
      Point2::new(0.0, 60.0),
    ],
    true,
  )
  .expect("rectangle");
  document.layers.push(Layer {
    id: "L1".to_owned(),
    name: "Part".to_owned(),
    visible: true,
    entities: vec![Entity {
      id: "E1".to_owned(),
      kind: EntityKind::Contour { contour },
    }],
  });
  document.toolpaths.push(Toolpath {
    id: "P1".to_owned(),
    name: "Perfil 01".to_owned(),
    entity_ids: vec!["E1".to_owned()],
    island_ids: Vec::new(),
    tool_number,
    operation: cam_model::Operation::Profile(ProfileParams::default()),
  });
  document
}

/// The four numbers a `StoredTool` needs that a family does not change.
fn base(id: &str, number: Option<u32>, diameter: Option<f64>) -> StoredTool {
  StoredTool {
    geometry: ToolGeometry {
      id: id.to_owned(),
      name_format: "{Tool Type} ({Diameter|F}{Units Short})".to_owned(),
      tool_type: 0,
      units: UnitSystem::Mm,
      diameter: diameter.map(Length::from_mm),
      included_angle: None,
      flat_diameter: None,
      num_flutes: Some(2),
      flute_length: None,
      tip_radius: None,
      custom_attributes: None,
    },
    cutting: CuttingData {
      id: format!("{id}-cut"),
      rate_units: UnitSystem::Mm,
      feed_rate: Some(100.0),
      plunge_rate: Some(30.0),
      spindle_speed: Some(18_000),
      stepdown: None,
      stepover: None,
      clear_stepover: None,
      tool_number: number,
      notes: None,
    },
  }
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
  assert_eq!(feed, "3.94 in/s", "an inch library's feed reads in its own units");
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

/// A flat end mill: a diameter and nothing that shapes its tip.
fn flat_mill(id: &str, number: Option<u32>, diameter: f64) -> StoredTool {
  base(id, number, Some(diameter))
}

/// A ball nose: a tip radius of exactly half the diameter.
fn ball_nose(id: &str, number: Option<u32>, diameter: f64) -> StoredTool {
  let mut tool = base(id, number, Some(diameter));
  tool.geometry.tip_radius = Some(Length::from_mm(diameter / 2.0));
  tool
}

/// A corner radius: a tip radius that is not half the diameter.
fn corner_radius(id: &str, number: Option<u32>, diameter: f64, radius: f64) -> StoredTool {
  let mut tool = base(id, number, Some(diameter));
  tool.geometry.tip_radius = Some(Length::from_mm(radius));
  tool
}

/// A V-bit: an included angle, and the flat at the bottom of the cone.
fn v_bit(id: &str, number: Option<u32>, diameter: f64, angle: f64, flat: f64) -> StoredTool {
  let mut tool = base(id, number, Some(diameter));
  tool.geometry.included_angle = Some(Angle::from_degrees(angle));
  tool.geometry.flat_diameter = Some(Length::from_mm(flat));
  tool
}

/// The library these tests work from: the four families, two of them flat.
fn library() -> Vec<StoredTool> {
  vec![
    flat_mill("G1", Some(1), 6.0),
    flat_mill("G5", Some(5), 3.175),
    ball_nose("G2", Some(2), 6.0),
    v_bit("G3", Some(3), 12.7, 90.0, 0.2),
  ]
}

// ---------------------------------------------------------------------------------------------
// Frames
// ---------------------------------------------------------------------------------------------

fn raw_input() -> egui::RawInput {
  egui::RawInput {
    screen_rect: Some(egui::Rect::from_min_size(
      egui::Pos2::ZERO,
      egui::vec2(1200.0, 800.0),
    )),
    ..Default::default()
  }
}

fn frame_with_events(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
  events: Vec<egui::Event>,
) -> (ShellOutput, Vec<String>) {
  let mut captured: Option<ShellOutput> = None;
  let mut input = raw_input();
  input.events = events;
  let full = ctx.run_pass(input, |ctx| {
    captured = Some(shell.show(app, ctx));
  });
  let texts = painted_text(&full);
  full.drop_without_applying_deltas();
  (captured.expect("the shell ran"), texts)
}

fn frame_with(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
) -> (ShellOutput, Vec<String>) {
  frame_with_events(ctx, shell, app, Vec::new())
}

/// Every piece of text the frame actually drew.
fn painted_text(output: &egui::FullOutput) -> Vec<String> {
  output
    .shapes
    .iter()
    .filter_map(|clipped| match &clipped.shape {
      egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
      _ => None,
    })
    .collect()
}

/// Press the pointer at one window point: move there, press, then release.
fn click_at(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
  at: egui::Pos2,
) -> (ShellOutput, Vec<String>) {
  let mut last: Option<(ShellOutput, Vec<String>)> = None;
  for pressed in [true, false] {
    let mut input = raw_input();
    input.events.push(egui::Event::PointerMoved(at));
    input.events.push(egui::Event::PointerButton {
      pos: at,
      button: egui::PointerButton::Primary,
      pressed,
      modifiers: egui::Modifiers::default(),
    });
    let mut captured: Option<ShellOutput> = None;
    let full = ctx.run_pass(input, |ctx| {
      captured = Some(shell.show(app, ctx));
    });
    let texts = painted_text(&full);
    full.drop_without_applying_deltas();
    last = Some((captured.expect("the shell ran"), texts));
  }
  last.expect("a click is two frames")
}

/// Press one key and report the frame the press was read in.
fn press(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
  key: egui::Key,
) -> (ShellOutput, Vec<String>) {
  let key_event = |pressed: bool| egui::Event::Key {
    key,
    physical_key: None,
    pressed,
    repeat: false,
    modifiers: egui::Modifiers::default(),
  };
  let frame = frame_with_events(ctx, shell, app, vec![key_event(true)]);
  let _ = frame_with_events(ctx, shell, app, vec![key_event(false)]);
  frame
}

/// Open the library the way an operator does: from the current-tool card's own row.
///
/// Two quiet frames after the press, because an egui area settles on a sizing pass and the points it reports in
/// the frame it first exists are not the ones it settles at — the same reason the material dialog's opener
/// does.
fn open_library(ctx: &egui::Context, shell: &mut Shell, app: &mut AppState) -> ShellOutput {
  shell.open_task(Category::Toolpaths);
  let (output, _) = frame_with(ctx, shell, app);
  let at = output
    .drawn
    .tool_card_hits
    .iter()
    .find(|(word, _)| word.starts_with("Choose another tool"))
    .map(|(_, point)| egui::pos2(point.0, point.1))
    .unwrap_or_else(|| {
      panic!(
        "the current-tool card offers no way to choose another tool: {:?}",
        output.drawn.tool_card_hits
      )
    });
  click_at(ctx, shell, app, at);
  let _ = frame_with(ctx, shell, app);
  frame_with(ctx, shell, app).0
}

/// Press one thing the modal drew, by the word on it.
fn click_word(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
  drawn: &ShellOutput,
  word: &str,
) -> (ShellOutput, Vec<String>) {
  let at = drawn
    .drawn
    .tool_library_hits
    .iter()
    .find(|(name, _)| name == word)
    .map(|(_, point)| egui::pos2(point.0, point.1))
    .unwrap_or_else(|| {
      panic!(
        "nothing called {word:?} in the tool library: {:?}",
        drawn.drawn.tool_library_hits
      )
    });
  click_at(ctx, shell, app, at)
}

/// The fields the form drew under one group, by the group's word.
fn fields_of(drawn: &ShellOutput, group: &str) -> Vec<String> {
  drawn
    .drawn
    .tool_groups
    .iter()
    .find(|(label, _)| label == group)
    .map(|(_, fields)| fields.iter().map(|(label, _)| label.clone()).collect())
    .unwrap_or_else(|| {
      panic!(
        "the form drew no {group} group: {:?}",
        drawn
          .drawn
          .tool_groups
          .iter()
          .map(|(label, _)| label.clone())
          .collect::<Vec<_>>()
      )
    })
}

/// The groups the form drew, top to bottom.
fn group_labels(drawn: &ShellOutput) -> Vec<String> {
  drawn
    .drawn
    .tool_groups
    .iter()
    .map(|(label, _)| label.clone())
    .collect()
}

fn joined(texts: &[String]) -> String {
  texts.join(" | ")
}

// ---------------------------------------------------------------------------------------------
// The family the library's own geometry proves
// ---------------------------------------------------------------------------------------------

#[test]
fn the_family_is_the_shape_the_record_proves_not_the_raw_type_code() {
  // Every fixture below carries `tool_type: 0`, the same opaque code — so if the family came from that number
  // the tree would be one group. The shape is what separates them, and the code names nothing (cam-store says
  // so itself).
  assert_eq!(
    ToolFamily::of(&flat_mill("a", Some(1), 6.0).geometry),
    ToolFamily::Flat
  );
  assert_eq!(
    ToolFamily::of(&ball_nose("b", Some(2), 6.0).geometry),
    ToolFamily::BallNose
  );
  assert_eq!(
    ToolFamily::of(&corner_radius("c", Some(3), 12.0, 2.0).geometry),
    ToolFamily::CornerRadius
  );
  assert_eq!(
    ToolFamily::of(&v_bit("d", Some(4), 12.7, 90.0, 0.2).geometry),
    ToolFamily::VBit
  );
  // A V is decided by its angle even when it also carries a tip radius, and a record with no diameter has no
  // radius to prove a ball: both are readings of the rule, not of a list.
  let mut both = v_bit("e", Some(5), 12.7, 60.0, 0.0);
  both.geometry.tip_radius = Some(Length::from_mm(6.0));
  assert_eq!(ToolFamily::of(&both.geometry), ToolFamily::VBit);
  assert_eq!(
    ToolFamily::of(&base("f", Some(6), None).geometry),
    ToolFamily::Flat
  );
}

#[test]
fn a_row_names_the_tool_and_carries_its_own_diameter_and_cut_count() {
  // The reference's second line, in its own form: the diameter without a unit and the number of cuts. The
  // zeroes a reader does not need are trimmed, so a Ø12.7 tool does not read as `Ø12.700`.
  assert_eq!(cut_line(&flat_mill("G1", Some(1), 6.0)), "Ø6 · 2 cut(s)");
  assert_eq!(
    cut_line(&flat_mill("G5", Some(5), 3.175)),
    "Ø3.175 · 2 cut(s)"
  );
  assert_eq!(
    cut_line(&v_bit("G3", Some(3), 12.7, 90.0, 0.2)),
    "Ø12.7 · 2 cut(s)"
  );
  // A number the library does not hold is a dash: `Ø0 · 0 cut(s)` would be two numbers nobody wrote.
  let mut half_described = base("G9", None, None);
  half_described.geometry.num_flutes = None;
  assert_eq!(cut_line(&half_described), "Ø— · — cut(s)");
  // The title is the card's own spelling, so the row and the card name one tool one way.
  assert_eq!(
    tool_title(&flat_mill("G1", Some(1), 6.0)),
    "T1 - Tool (6mm)"
  );
  assert_eq!(tool_title(&base("G8", None, Some(6.0))), "Tool (6mm)");
}

// ---------------------------------------------------------------------------------------------
// The master side: the opener, the search field and the tree
// ---------------------------------------------------------------------------------------------

#[test]
fn the_cards_own_row_opens_the_library_on_the_tool_the_toolpath_cuts_with() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());

  let opened = open_library(&ctx, &mut shell, &mut app);

  assert!(
    shell.show_tool_library,
    "the card offered a row that did not open the library"
  );
  // The selection is the tool the toolpath already cuts with: the first thing an operator wants from
  // `Choose another tool…` is to see what the tool is now.
  assert_eq!(opened.drawn.tool_selected.as_deref(), Some("G2"));
  let words = opened.drawn.tool_library.join(" | ");
  assert!(
    words.contains("Choose tool — T2 - Tool (6mm)"),
    "the dialog's title does not name the chosen tool: {words}"
  );
  // Every tool in the library is a row, and the commit pair is there because this state can be honoured.
  assert_eq!(
    opened.drawn.tool_rows.len(),
    4,
    "{:?}",
    opened.drawn.tool_rows
  );
  for word in ["Cancel", "Use this tool"] {
    assert!(
      opened
        .drawn
        .tool_library_hits
        .iter()
        .any(|(name, _)| name == word),
      "nothing to press called {word:?}: {:?}",
      opened.drawn.tool_library_hits
    );
  }
}

#[test]
fn the_tree_groups_the_library_by_family_and_counts_what_it_drew() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());

  let opened = open_library(&ctx, &mut shell, &mut app);

  assert_eq!(
    opened.drawn.tool_families,
    vec![
      ("Flat end mill".to_owned(), 2),
      ("Ball nose".to_owned(), 1),
      ("V-bit".to_owned(), 1),
    ],
    "the tree must group by family, in one order, and skip a family with no tool"
  );
  // A tree is a claim about *which* rows sit together, which a list of counts cannot make: the two flat mills
  // are the first two rows and they say so.
  let families: Vec<&str> = opened
    .drawn
    .tool_rows
    .iter()
    .map(|(family, _, _)| family.as_str())
    .collect();
  assert_eq!(
    families,
    vec!["Flat end mill", "Flat end mill", "Ball nose", "V-bit"]
  );
  let words = opened.drawn.tool_library.join(" | ");
  for heading in [
    "Flat end mill (2)",
    "Ball nose (1)",
    "V-bit (1)",
    "Ø6 · 2 cut(s)",
  ] {
    assert!(
      words.contains(heading),
      "the tree never drew {heading:?}: {words}"
    );
  }
}

#[test]
fn the_search_field_filters_the_tree_and_the_counts_name_what_is_left() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  let _ = open_library(&ctx, &mut shell, &mut app);

  // A family's own word: an operator who reads `Ball nose (1)` and types `ball` is asking for its rows.
  shell.tool_search = "ball".to_owned();
  let (filtered, _) = frame_with(&ctx, &mut shell, &mut app);
  assert_eq!(
    filtered.drawn.tool_rows.len(),
    1,
    "the search field filtered nothing: {:?}",
    filtered.drawn.tool_rows
  );
  assert_eq!(filtered.drawn.tool_selected.as_deref(), Some("G2"));
  // The count is the number of rows drawn, not the number the library holds: a heading that said `(2)` over
  // one visible row would be a number nobody can check against the screen.
  assert_eq!(
    filtered.drawn.tool_families,
    vec![("Ball nose".to_owned(), 1)]
  );

  // A diameter on the cut line, which is how a machinist looks a tool up.
  shell.tool_search = "Ø12.7".to_owned();
  let (by_width, _) = frame_with(&ctx, &mut shell, &mut app);
  assert_eq!(
    by_width.drawn.tool_rows,
    vec![(
      "V-bit".to_owned(),
      "T3 - Tool (12.7mm)".to_owned(),
      "Ø12.7 · 2 cut(s)".to_owned()
    )],
    "the search never looked at the row's own cut line"
  );

  // And a search that matches nothing says so instead of drawing an empty tree.
  shell.tool_search = "spiral flute".to_owned();
  let (none, texts) = frame_with(&ctx, &mut shell, &mut app);
  assert!(none.drawn.tool_rows.is_empty());
  assert!(
    none
      .drawn
      .tool_library
      .iter()
      .any(|word| word.contains("No tool matches")),
    "an empty tree said nothing: {:?}",
    none.drawn.tool_library
  );
  assert!(joined(&texts).contains("No tool matches"));

  // The chip the field grows while it holds text is how the filter is cleared — a filter with no way out but
  // selecting the text is a trap.
  assert!(
    none
      .drawn
      .tool_library_hits
      .iter()
      .any(|(name, _)| name == "Clear"),
    "the search field offers no way back: {:?}",
    none.drawn.tool_library_hits
  );
  let (cleared, _) = click_word(&ctx, &mut shell, &mut app, &none, "Clear");
  assert_eq!(shell.tool_search, "");
  assert_eq!(cleared.drawn.tool_rows.len(), 4);
}

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

// ---------------------------------------------------------------------------------------------
// The drawing: to scale, with its own dimensions called out
// ---------------------------------------------------------------------------------------------

#[test]
fn the_drawing_is_one_shared_scale_so_two_tools_are_comparable() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  let opened = open_library(&ctx, &mut shell, &mut app);

  // Ø6.35 and Ø12.7 in the same box: a picture that drew every tool to the frame's width would give the same
  // number twice, which is the bug this pins.
  let (small, small_texts) = click_word(&ctx, &mut shell, &mut app, &opened, "T1 - Tool (6mm)");
  let small_width = small.drawn.tool_width_px;
  let small_box = small
    .drawn
    .tool_drawing
    .expect("the flat mill was not drawn");
  assert!(
    small_width > 0.0,
    "the drawing reported no width at all, so `to scale` is unanswerable"
  );

  let (large, large_texts) = click_word(&ctx, &mut shell, &mut app, &small, "T3 - Tool (12.7mm)");
  let large_width = large.drawn.tool_width_px;
  assert_eq!(
    large.drawn.tool_drawing.map(|rect| rect.2),
    Some(small_box.2),
    "the two tools were drawn in boxes of different widths, so their widths are not comparable"
  );
  let ratio = large_width / small_width;
  assert!(
    (ratio - 12.7 / 6.0).abs() < 0.02,
    "the drawing is not to one shared scale: Ø12.7 came out {ratio:.3} times Ø6, not {:.3}",
    12.7 / 6.0
  );

  // The callouts are the tool's own numbers, painted and reported, and a tool has only the ones it holds.
  assert!(
    large
      .drawn
      .tool_callouts
      .iter()
      .any(|callout| callout == "Ø 12.70 mm"),
    "the width was not called out: {:?}",
    large.drawn.tool_callouts
  );
  assert!(
    large
      .drawn
      .tool_callouts
      .iter()
      .any(|callout| callout.contains("Included angle 90.0°")),
    "the V-bit's angle was not called out: {:?}",
    large.drawn.tool_callouts
  );
  assert!(
    !small
      .drawn
      .tool_callouts
      .iter()
      .any(|callout| callout.contains("Included angle")),
    "a flat end mill was given a V's dimension: {:?}",
    small.drawn.tool_callouts
  );
  for callout in &large.drawn.tool_callouts {
    assert!(
      joined(&large_texts).contains(callout),
      "the callout {callout:?} was reported and never painted: {}",
      joined(&large_texts)
    );
  }
  assert!(joined(&small_texts).contains("Ø 6.00 mm"));
}

// ---------------------------------------------------------------------------------------------
// The footer, and the two ways out
// ---------------------------------------------------------------------------------------------

#[test]
fn the_footer_offers_only_the_library_actions_this_shell_can_perform() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  let opened = open_library(&ctx, &mut shell, &mut app);

  // The shell holds a read-only snapshot: `cam-store` owns the only writer. Five buttons that emitted nothing
  // would be L14's defect five times over, so what the footer draws is the sentence that says so.
  for absent in ["New", "Duplicate", "Delete", "Import…", "Export…"] {
    assert!(
      !opened
        .drawn
        .tool_library_hits
        .iter()
        .any(|(name, _)| name == absent),
      "the footer offered {absent:?}, a library action this window cannot perform: {:?}",
      opened.drawn.tool_library_hits
    );
  }
  let reason = opened
    .drawn
    .tool_library
    .iter()
    .find(|word| word.contains("read-only snapshot"))
    .unwrap_or_else(|| {
      panic!(
        "the footer does not say why the library actions are missing: {:?}",
        opened.drawn.tool_library
      )
    })
    .clone();
  assert!(
    reason.contains("cam-cli tools add"),
    "the sentence must name the route that does work: {reason:?}"
  );
  // And the pair S9 puts under the actions is there, in this order.
  let hits: Vec<&str> = opened
    .drawn
    .tool_library_hits
    .iter()
    .map(|(name, _)| name.as_str())
    .collect();
  let cancel = hits.iter().position(|word| *word == "Cancel");
  let use_it = hits.iter().position(|word| *word == "Use this tool");
  assert!(
    cancel.is_some() && use_it.is_some(),
    "the commit pair is missing: {hits:?}"
  );
  assert!(
    hits.iter().any(|word| *word == "T1 - Tool (6mm)"),
    "the tree rows are not pressable: {hits:?}"
  );
}

#[test]
fn use_this_tool_retools_the_toolpath_and_closes_the_library() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  let opened = open_library(&ctx, &mut shell, &mut app);

  // Choose T1 and commit: the one thing this modal writes is the number the toolpath cuts with.
  let (chosen, _) = click_word(&ctx, &mut shell, &mut app, &opened, "T1 - Tool (6mm)");
  assert_eq!(chosen.drawn.tool_selected.as_deref(), Some("G1"));
  let (pressed, _) = click_word(&ctx, &mut shell, &mut app, &chosen, "Use this tool");
  assert_eq!(
    pressed.actions,
    vec![UiAction::SetToolpathToolNumber {
      toolpath_id: "P1".to_owned(),
      tool_number: 1,
    }],
    "`Use this tool` must emit exactly the tool it showed"
  );
  let report = handle_action(&mut app, &pressed.actions[0]).expect("the tool changes");
  assert_eq!(report.label, "set toolpath tool");
  assert_eq!(app.document().toolpaths[0].tool_number, 1);
  assert!(
    !shell.show_tool_library,
    "the library stayed open after committing, so the next press could commit twice"
  );
  let (after, _) = frame_with(&ctx, &mut shell, &mut app);
  assert!(
    after.drawn.tool_library.is_empty() && after.drawn.tool_library_hits.is_empty(),
    "a closed library still reported itself: {:?}",
    after.drawn.tool_library
  );
}

#[test]
fn cancel_leaves_the_toolpath_the_library_and_the_search_exactly_as_they_were() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  let _ = open_library(&ctx, &mut shell, &mut app);
  let before = app.document().clone();

  // Change the draft in both ways the modal offers, then leave.
  shell.tool_search = "flat".to_owned();
  let (filtered, _) = frame_with(&ctx, &mut shell, &mut app);
  let (chosen, _) = click_word(&ctx, &mut shell, &mut app, &filtered, "T1 - Tool (6mm)");
  let (left, _) = click_word(&ctx, &mut shell, &mut app, &chosen, "Cancel");

  assert!(
    left.actions.is_empty(),
    "`Cancel` wrote something: {:?}",
    left.actions
  );
  assert_eq!(
    app.document().toolpaths[0].tool_number,
    before.toolpaths[0].tool_number,
    "the toolpath was retooled by a cancelled choice"
  );
  assert_eq!(
    app.document(),
    &before,
    "the document changed under `Cancel`"
  );
  assert!(!shell.show_tool_library);
  // The drafts go too: a search left behind would filter the next visit for a reason nobody remembers.
  assert_eq!(shell.tool_search, "");
  assert!(shell.tool_library_target().is_none());

  // And a second visit starts where the document is, not where the cancelled choice was.
  let (again, _) = frame_with(&ctx, &mut shell, &mut app);
  assert!(again.drawn.tool_library.is_empty());
  let reopened = open_library(&ctx, &mut shell, &mut app);
  assert_eq!(reopened.drawn.tool_selected.as_deref(), Some("G2"));
}

#[test]
fn a_tool_with_no_number_draws_no_use_this_tool_and_says_why() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  // A tool the library holds without the number a program names it by: the modal cannot honour
  // `Use this tool`, so it must not draw it.
  shell.set_tools(vec![
    flat_mill("G9", None, 4.0),
    flat_mill("G1", Some(1), 6.0),
  ]);
  let (redrawn, _) = {
    shell.open_tool_library(&app);
    let _ = frame_with(&ctx, &mut shell, &mut app);
    frame_with(&ctx, &mut shell, &mut app)
  };
  let (selected, _) = click_word(&ctx, &mut shell, &mut app, &redrawn, "Tool (4mm)");
  assert_eq!(selected.drawn.tool_selected.as_deref(), Some("G9"));
  assert!(
    selected
      .drawn
      .tool_library_hits
      .iter()
      .all(|(name, _)| name != "Use this tool"),
    "the modal drew a `Use this tool` it could not honour: {:?}",
    selected.drawn.tool_library_hits
  );
  let reason = selected
    .drawn
    .tool_library
    .iter()
    .find(|word| word.contains("carries no tool number"))
    .unwrap_or_else(|| {
      panic!(
        "the modal did not say why it cannot use this tool: {:?}",
        selected.drawn.tool_library
      )
    })
    .clone();
  assert!(
    reason.contains("T"),
    "the sentence must name the tool it refuses: {reason:?}"
  );
  // `Cancel` still works: the way out is never the thing that disappears.
  let (left, _) = click_word(&ctx, &mut shell, &mut app, &selected, "Cancel");
  assert!(left.actions.is_empty());
  assert!(!shell.show_tool_library);
}

#[test]
fn an_empty_library_says_so_and_offers_nothing_to_use() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  // A window that was given no library at all: `set_tools(vec![])` is the state an open of a document that
  // names none leaves behind.
  shell.set_tools(Vec::new());
  let (output, texts) = {
    shell.open_tool_library(&app);
    let _ = frame_with(&ctx, &mut shell, &mut app);
    frame_with(&ctx, &mut shell, &mut app)
  };

  assert!(output.drawn.tool_rows.is_empty());
  assert!(output.drawn.tool_families.is_empty());
  assert!(
    output
      .drawn
      .tool_library_hits
      .iter()
      .all(|(name, _)| name != "Use this tool"),
    "an empty library offered a tool to use: {:?}",
    output.drawn.tool_library_hits
  );
  assert!(
    output
      .drawn
      .tool_library
      .iter()
      .any(|word| word.contains("holds no tool")),
    "an empty library said nothing: {:?}",
    output.drawn.tool_library
  );
  assert!(joined(&texts).contains("holds no tool"));
}

#[test]
fn a_library_opened_without_a_toolpath_lists_the_tools_and_draws_no_use_this_tool() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  // A host that opens the library with nothing to retool: the tree is still worth showing — reading the
  // library is a reason of its own — but `Use this tool` has no document fact to write, so it is not drawn.
  shell.show_tool_library = true;

  let (output, _) = frame_with(&ctx, &mut shell, &mut app);
  let _ = frame_with(&ctx, &mut shell, &mut app);

  assert_eq!(
    output.drawn.tool_rows.len(),
    4,
    "the tree is not the thing that depends on a target"
  );
  assert!(
    output
      .drawn
      .tool_library_hits
      .iter()
      .all(|(name, _)| name != "Use this tool"),
    "a library with no toolpath to retool offered `Use this tool`: {:?}",
    output.drawn.tool_library_hits
  );
  assert!(
    output
      .drawn
      .tool_library
      .iter()
      .any(|word| word.contains("without a toolpath")),
    "the modal did not say why it cannot use a tool: {:?}",
    output.drawn.tool_library
  );
  assert!(shell.tool_library_target().is_none());
}

#[test]
fn a_row_the_scroll_area_hides_is_not_reported_as_pressable() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(1));
  let mut shell = Shell::new();
  // More tools than the master column has room for: the ones below the fold are drawn by egui and clipped
  // away, and reporting their centres would be a promise about a control nobody can see — the defect the T6
  // review found in an invisible playback bar with live hit areas (L23).
  shell.set_tools(
    (1..=24)
      .map(|index| flat_mill(&format!("G{index}"), Some(index), 6.0))
      .collect(),
  );
  shell.open_tool_library(&app);
  let _ = frame_with(&ctx, &mut shell, &mut app);
  let (output, _) = frame_with(&ctx, &mut shell, &mut app);

  let rows: Vec<&str> = output
    .drawn
    .tool_rows
    .iter()
    .map(|(_, title, _)| title.as_str())
    .collect();
  assert_eq!(rows.len(), 24, "the tree lists every tool it was given");
  let pressable: Vec<&str> = output
    .drawn
    .tool_library_hits
    .iter()
    .map(|(name, _)| name.as_str())
    .filter(|name| rows.contains(name))
    .collect();
  assert!(
    pressable.len() < rows.len(),
    "every row is reported as pressable, so nothing was clipped: {pressable:?}"
  );
  // What it reports is a prefix of the tree — the rows it drew inside the fold, in the tree's own order — and
  // the last tool is not among them.
  assert_eq!(
    pressable,
    rows[..pressable.len()],
    "the reported rows are not the ones at the top of the tree"
  );
  assert!(
    !pressable.contains(&"T24 - Tool (6mm)"),
    "a row below the fold was reported as pressable"
  );
  // And the report is actionable: the last row it names really does select its tool.
  let last = *pressable.last().expect("some row is visible");
  let at = output
    .drawn
    .tool_library_hits
    .iter()
    .find(|(name, _)| name == last)
    .map(|(_, point)| egui::pos2(point.0, point.1))
    .expect("the point it reported");
  click_at(&ctx, &mut shell, &mut app, at);
  assert!(
    shell.tool_library_selection().is_some_and(|id| id != "G1"),
    "the last reported row did not select its tool: {:?}",
    shell.tool_library_selection()
  );
}

#[test]
fn escape_leaves_the_library_without_writing_and_the_modal_owns_the_pointer() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  let _ = open_library(&ctx, &mut shell, &mut app);

  // A modal owns the pointer: the canvas under the backdrop reports no world position and finds no snap
  // target, because a status bar naming a point nobody can aim at would be describing a canvas that is not
  // being aimed at.
  let over_canvas = egui::pos2(600.0, 400.0);
  let mut input = raw_input();
  input.events.push(egui::Event::PointerMoved(over_canvas));
  let mut captured: Option<ShellOutput> = None;
  let full = ctx.run_pass(input, |ctx| {
    captured = Some(shell.show(&mut app, ctx));
  });
  full.drop_without_applying_deltas();
  let drawn = captured.expect("the shell ran");
  assert!(
    shell.cursor_world.is_none(),
    "the canvas tracked the pointer under an open modal: {:?}",
    shell.cursor_world
  );
  assert!(drawn
    .drawn
    .tool_library
    .iter()
    .any(|word| word == "Choose tool — T2 - Tool (6mm)"));

  let (left, _) = press(&ctx, &mut shell, &mut app, egui::Key::Escape);
  assert!(
    left.actions.is_empty(),
    "`Esc` wrote something: {:?}",
    left.actions
  );
  assert!(!shell.show_tool_library);
  assert_eq!(app.document().toolpaths[0].tool_number, 2);
}

// ---------------------------------------------------------------------------------------------
// Preserved: the card the opener lives on
// ---------------------------------------------------------------------------------------------

#[test]
fn the_tool_card_still_reads_out_its_own_three_lines_while_it_offers_the_opener() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  shell.open_task(Category::Toolpaths);

  let (output, _) = frame_with(&ctx, &mut shell, &mut app);

  // The card's fields are exactly the ones the reference frame shows (camui-toolpath S2): the control that
  // opens the library is a row of its own and did not become a fourth field.
  assert_eq!(
    output.drawn.tool_card,
    vec![
      "TOOL — Perfil 01".to_owned(),
      "T2 - Tool (6mm)".to_owned(),
      "code 0 · Ø 6.00 mm · — · 2 flutes".to_owned(),
    ]
  );
  assert!(
    output
      .drawn
      .tool_card_hits
      .iter()
      .any(|(word, _)| word == "Choose another tool…"),
    "the card no longer offers the way to change its tool: {:?}",
    output.drawn.tool_card_hits
  );
}
