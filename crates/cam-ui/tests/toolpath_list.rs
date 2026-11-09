//! The `TOOLPATHS` surface's list, and the selection that decides what the shell describes (M13 T3, S4).
//!
//! S4 measures the gap: the surface listed no toolpath, the current-tool card described
//! `app.document().toolpaths.first()` unconditionally, and a document with two toolpaths could only ever
//! describe the first. What these tests pin is the rule that replaces it — one row per toolpath in document
//! order, one of them selectable by press, the selection held as **view state**, and the card plus the
//! strategy editor describing the selected toolpath rather than the first.
//!
//! Two conventions from the house idiom are used rather than paid for again: the list reports where it drew
//! every row ([`cam_ui::DrawnState::toolpath_hits`]) because egui cannot be asked for a widget by its text
//! from outside a frame, and the frame is tall enough to show the whole surface so a pressed point is a point
//! the frame really drew (L23).

use cam_app::AppState;
use cam_geom::{Contour, Point2};
use cam_model::{
  Document, Entity, EntityKind, Job, Layer, Operation, PocketParams, ProfileParams, Toolpath,
};
use cam_store::tools::{CuttingData, StoredTool, ToolGeometry};
use cam_ui::{Category, Shell, ShellOutput};
use cam_units::{Length, UnitSystem};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

/// A sheet with one rectangle (`E1`), the document every fixture below starts from.
fn sheet() -> Document {
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
  document
}

/// One toolpath over `E1`, named, tooled and operated as the caller says.
fn toolpath(id: &str, name: &str, tool_number: u32, operation: Operation) -> Toolpath {
  Toolpath {
    id: id.to_owned(),
    name: name.to_owned(),
    entity_ids: vec!["E1".to_owned()],
    island_ids: Vec::new(),
    tool_number,
    operation,
  }
}

/// A profile with its own pass depth, so two toolpaths of one kind are still distinguishable by their numbers.
fn profile(pass_depth: f64) -> Operation {
  Operation::Profile(ProfileParams {
    pass_depth: Length::from_mm(pass_depth),
    ..ProfileParams::default()
  })
}

/// A pocket with its own pass depth: the other kind the editor drafts.
fn pocket(pass_depth: f64) -> Operation {
  Operation::Pocket(PocketParams {
    pass_depth: Length::from_mm(pass_depth),
    ..PocketParams::default()
  })
}

/// Three toolpaths in document order: a profile, a pocket, and another profile, each with its own tool.
fn three_toolpaths() -> Document {
  let mut document = sheet();
  document
    .toolpaths
    .push(toolpath("TP1", "Perfil 01", 2, profile(1.0)));
  document
    .toolpaths
    .push(toolpath("TP2", "Desbaste 1", 4, pocket(2.5)));
  document
    .toolpaths
    .push(toolpath("TP3", "Perfil 02", 6, profile(1.5)));
  document
}

/// Two toolpaths, the pair M13 S4 measured: the card could only ever describe the first of them.
fn two_toolpaths() -> Document {
  let mut document = sheet();
  document
    .toolpaths
    .push(toolpath("TP1", "Perfil 01", 2, profile(1.0)));
  document
    .toolpaths
    .push(toolpath("TP2", "Desbaste 1", 4, pocket(2.5)));
  document
}

/// One flat end mill, numbered `number` and `diameter` wide: the tool the card reads out of the library.
fn stored_tool(number: u32, diameter: f64) -> StoredTool {
  StoredTool {
    geometry: ToolGeometry {
      id: format!("G{number}"),
      name_format: "{Tool Type} ({Diameter|F}{Units Short})".to_owned(),
      tool_type: 0,
      units: UnitSystem::Mm,
      diameter: Some(Length::from_mm(diameter)),
      included_angle: None,
      flat_diameter: None,
      num_flutes: Some(2),
      flute_length: None,
      tip_radius: None,
      shank_diameter: None,
      overall_length: None,
      custom_attributes: None,
    },
    cutting: CuttingData {
      id: format!("G{number}-cut"),
      rate_units: UnitSystem::Mm,
      feed_rate: Some(100.0),
      plunge_rate: Some(30.0),
      spindle_speed: Some(18_000),
      stepdown: None,
      stepover: None,
      clear_stepover: None,
      tool_number: Some(number),
      notes: None,
    },
  }
}

/// A library holding the three tools the fixtures name.
fn library() -> Vec<StoredTool> {
  vec![
    stored_tool(2, 6.0),
    stored_tool(4, 3.0),
    stored_tool(6, 4.0),
  ]
}

/// The word the shell writes for an operation, so a row can be asserted to name the one it cuts with.
fn operation_word(operation: &Operation) -> &'static str {
  match operation {
    Operation::Profile(_) => "Profile",
    Operation::Pocket(_) => "Pocket",
    other => panic!("no word for {other:?} in this fixture"),
  }
}

// ---------------------------------------------------------------------------------------------
// Frames
// ---------------------------------------------------------------------------------------------

/// A window tall enough to hold the whole `TOOLPATHS` surface, so every reported point is one the frame drew.
///
/// The inspector's body is a scroll area capped at 55 % of the window: at the standard 800 px the list and the
/// editor below it do not both fit, and a point read from a clipped frame is the defect L23 found in the
/// playback bar.
fn tall() -> egui::RawInput {
  egui::RawInput {
    screen_rect: Some(egui::Rect::from_min_size(
      egui::Pos2::ZERO,
      egui::vec2(1200.0, 2000.0),
    )),
    ..Default::default()
  }
}

/// The text one pass painted, in draw order.
fn painted_text(full: &egui::FullOutput) -> Vec<String> {
  full
    .shapes
    .iter()
    .filter_map(|clipped| match &clipped.shape {
      egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
      _ => None,
    })
    .collect()
}

/// Run one frame and report what the shell produced, with its painted text in draw order.
fn frame(ctx: &egui::Context, shell: &mut Shell, app: &mut AppState) -> (ShellOutput, Vec<String>) {
  let mut captured: Option<ShellOutput> = None;
  let full = ctx.run_pass(tall(), |ctx| {
    captured = Some(shell.show(app, ctx));
  });
  let texts = painted_text(&full);
  // The texture deltas belong to a renderer we do not have; dropping them silently is what epaint forbids.
  full.drop_without_applying_deltas();
  (captured.expect("the shell ran"), texts)
}

/// Open the `TOOLPATHS` task — the surface the list lives on — and report a settled frame.
fn editor_frame(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
) -> (ShellOutput, Vec<String>) {
  shell.open_task(Category::Toolpaths);
  // Two passes: egui lays a panel out from the space it ended the previous pass with, so the first frame is
  // the one that measures and the second is the one worth reading.
  let _ = frame(ctx, shell, app);
  frame(ctx, shell, app)
}

/// Press the pointer at one window point: move there, press, then release.
fn click_at(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
  at: egui::Pos2,
) -> (ShellOutput, Vec<String>) {
  let mut last = None;
  for pressed in [true, false] {
    let mut input = tall();
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

/// The row that names `name`, or a panic naming every row the surface drew instead.
fn row_naming(rows: &[(String, bool)], name: &str) -> String {
  let matching: Vec<&String> = rows
    .iter()
    .filter(|(word, _)| word.contains(name))
    .map(|(word, _)| word)
    .collect();
  assert_eq!(
    matching.len(),
    1,
    "exactly one row names {name:?}: {rows:?}"
  );
  matching[0].clone()
}

/// Where the list drew the row whose word is `word`, or a panic naming what it drew instead.
fn point_of(hits: &[(String, (f32, f32))], word: &str) -> egui::Pos2 {
  hits
    .iter()
    .find(|(name, _)| name == word)
    .map(|(_, point)| egui::pos2(point.0, point.1))
    .unwrap_or_else(|| panic!("nothing called {word:?} in the list: {hits:?}"))
}

/// Press the row that names `name`, and report the frame the press was read in.
fn click_row(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
  drawn: &ShellOutput,
  name: &str,
) -> (ShellOutput, Vec<String>) {
  let word = row_naming(&drawn.drawn.toolpath_rows, name);
  let at = point_of(&drawn.drawn.toolpath_hits, &word);
  click_at(ctx, shell, app, at)
}

// ---------------------------------------------------------------------------------------------
// The list
// ---------------------------------------------------------------------------------------------

#[test]
fn the_list_draws_one_row_per_toolpath_in_document_order() {
  // S4's first half: the surface lists the document's toolpaths instead of one editor for "the" toolpath.
  let ctx = egui::Context::default();
  let mut app = AppState::new(three_toolpaths());
  let mut shell = Shell::new();
  shell.set_tools(library());

  let (output, painted) = editor_frame(&ctx, &mut shell, &mut app);

  let rows = &output.drawn.toolpath_rows;
  assert_eq!(rows.len(), 3, "one row per toolpath: {rows:?}");
  // Document order, and every row names what identifies its toolpath: its name, its operation and its tool.
  for (index, toolpath) in app.document().toolpaths.iter().enumerate() {
    let (word, _) = rows
      .get(index)
      .unwrap_or_else(|| panic!("no row {index}: {rows:?}"));
    assert!(
      word.contains(&toolpath.name),
      "row {index} must name {:?}: {word:?}",
      toolpath.name
    );
    assert!(
      word.contains(operation_word(&toolpath.operation)),
      "row {index} must name the operation {:?}: {word:?}",
      operation_word(&toolpath.operation)
    );
    assert!(
      word.contains(&format!("T{}", toolpath.tool_number)),
      "row {index} must name the tool T{}: {word:?}",
      toolpath.tool_number
    );
  }
  // The rows are drawn, not only reported: the words the report carries are on the frame.
  for (word, _) in rows {
    assert!(
      painted.iter().any(|text| text == word),
      "the frame must draw the row {word:?}: {painted:?}"
    );
  }
  // And each row reports where it was drawn, because a row nobody can press is the `first()` this replaced.
  assert_eq!(
    output
      .drawn
      .toolpath_hits
      .iter()
      .map(|(word, _)| word.clone())
      .collect::<Vec<_>>(),
    rows
      .iter()
      .map(|(word, _)| word.clone())
      .collect::<Vec<_>>(),
    "every row reports the point that presses it, in the order it was drawn"
  );
}

// ---------------------------------------------------------------------------------------------
// The selection drives the card and the editor
// ---------------------------------------------------------------------------------------------

#[test]
fn pressing_a_row_selects_it_and_the_card_and_the_editor_describe_that_toolpath() {
  // S4's second half, and the whole of the gap: with two toolpaths the card used to describe the first one
  // whatever the operator did. Selecting is view state, and the selection is what the surface reads.
  let ctx = egui::Context::default();
  let mut app = AppState::new(two_toolpaths());
  let mut shell = Shell::new();
  shell.set_tools(library());

  let (before, _) = editor_frame(&ctx, &mut shell, &mut app);
  assert_eq!(
    shell.toolpath_selection(),
    None,
    "nothing is selected before a press"
  );
  assert_eq!(
    before.drawn.task_editor, "Edit · Perfil 01",
    "with no selection the editor still describes the document's first toolpath"
  );
  assert_eq!(before.drawn.tool_card[0], "TOOL — Perfil 01");

  let (pressed, _) = click_row(&ctx, &mut shell, &mut app, &before, "Desbaste 1");
  assert!(
    pressed.actions.is_empty(),
    "selecting is not an edit, so the frame emits no action: {:?}",
    pressed.actions
  );

  let (output, _) = frame(&ctx, &mut shell, &mut app);
  assert_eq!(
    shell.toolpath_selection(),
    Some("TP2"),
    "the press selects the row's toolpath, by id"
  );
  assert_eq!(
    output.drawn.toolpath_selected.as_deref(),
    Some("TP2"),
    "the shell reports which toolpath it is describing"
  );

  // The card describes the selected toolpath, not the first.
  assert_eq!(output.drawn.tool_card[0], "TOOL — Desbaste 1");
  assert!(
    output
      .drawn
      .tool_card
      .iter()
      .any(|line| line.starts_with("T4 - ")),
    "the card reads the selected toolpath's tool: {:?}",
    output.drawn.tool_card
  );
  assert!(
    !output
      .drawn
      .tool_card
      .iter()
      .any(|line| line.starts_with("T2 - ")),
    "the first toolpath's tool is not what the card describes: {:?}",
    output.drawn.tool_card
  );

  // The editor opened for it: its header names that toolpath and its sections carry that toolpath's numbers.
  assert_eq!(
    output.drawn.task_editor, "Edit · Desbaste 1",
    "the editor edits the selected toolpath"
  );
  assert!(
    output
      .drawn
      .strategy_fields
      .iter()
      .any(|(label, value)| label == "Pass depth" && value == "2.50 mm"),
    "the sections read the selected toolpath's operation: {:?}",
    output.drawn.strategy_fields
  );
  assert!(
    output
      .drawn
      .strategy_cards
      .iter()
      .any(|(word, chosen, _)| word == "Pocket" && *chosen),
    "the kind card reports the selected toolpath's kind: {:?}",
    output.drawn.strategy_cards
  );
  assert!(
    !output
      .drawn
      .strategy_cards
      .iter()
      .any(|(word, chosen, _)| word == "Profile" && *chosen),
    "and not the first toolpath's kind: {:?}",
    output.drawn.strategy_cards
  );

  // The idle panel's own card is the same reading: the selection drives the card wherever it is drawn.
  shell.close_task(false);
  let (idle, _) = frame(&ctx, &mut shell, &mut app);
  assert_eq!(idle.drawn.tool_card[0], "TOOL — Desbaste 1");
}

#[test]
fn selecting_changes_no_document_no_dirty_flag_and_no_undo_step() {
  // A selection is view state, the rule the camera, the 3D split and the collapsed rail groups already keep:
  // it is neither an edit nor a reason for the document to read as modified.
  let ctx = egui::Context::default();
  let mut app = AppState::new(two_toolpaths());
  let mut shell = Shell::new();
  shell.set_tools(library());

  let (drawn, _) = editor_frame(&ctx, &mut shell, &mut app);
  let document = app.document().clone();
  let depth = app.history_depth();
  assert!(!app.is_dirty(), "the fixture starts clean");
  assert!(!app.can_undo(), "the fixture starts with nothing to undo");

  let (pressed, _) = click_row(&ctx, &mut shell, &mut app, &drawn, "Desbaste 1");
  assert!(
    pressed.actions.is_empty(),
    "selecting emits no action: {:?}",
    pressed.actions
  );
  // One more frame, because the editor re-seeds the draft for the new subject: a re-seed that wrote the
  // document would show up here.
  let _ = frame(&ctx, &mut shell, &mut app);

  assert_eq!(
    app.document(),
    &document,
    "a selection may not change the document"
  );
  assert!(
    !app.is_dirty(),
    "a selection is not an edit, so the document cannot read as modified"
  );
  assert_eq!(
    app.history_depth(),
    depth,
    "a selection may not add an undo step"
  );
  assert!(!app.can_undo(), "and there is still nothing to undo");
}

// ---------------------------------------------------------------------------------------------
// PRESERVE: a one-toolpath document reads as it did
// ---------------------------------------------------------------------------------------------

#[test]
fn a_one_toolpath_document_reads_exactly_as_before() {
  // The fallback the whole change rests on: with no selection, `first()` is still the answer, so a document
  // whose only toolpath is the first one describes it exactly as it did before the list existed.
  let ctx = egui::Context::default();
  let mut document = sheet();
  document
    .toolpaths
    .push(toolpath("TP1", "Perfil 01", 2, profile(1.0)));
  let mut app = AppState::new(document);
  let mut shell = Shell::new();
  shell.set_tools(library());

  let (output, _) = editor_frame(&ctx, &mut shell, &mut app);

  assert_eq!(
    shell.toolpath_selection(),
    None,
    "a document nobody has chosen in leaves the selection empty"
  );
  assert_eq!(
    output.drawn.task_editor, "Edit · Perfil 01",
    "the header is the first toolpath's, as it was"
  );
  assert_eq!(
    output.drawn.tool_card,
    vec![
      "TOOL — Perfil 01".to_owned(),
      "T2 - Tool (6mm)".to_owned(),
      "code 0 · Ø 6.00 mm · — · 2 flutes".to_owned(),
    ],
    "the card is the same three lines it drew before the list existed"
  );
  // The list itself is one row, and it reads as the selected one: a single-toolpath document is a list of one,
  // and the row the card describes has to be the row that reads as chosen.
  assert_eq!(output.drawn.toolpath_rows.len(), 1);
  assert!(
    output.drawn.toolpath_rows[0].1,
    "the only row reads as selected: {:?}",
    output.drawn.toolpath_rows
  );
  assert_eq!(output.drawn.toolpath_selected.as_deref(), Some("TP1"));
}

// ---------------------------------------------------------------------------------------------
// A stale selection falls back to `first()`
// ---------------------------------------------------------------------------------------------

#[test]
fn a_selection_naming_a_toolpath_the_document_does_not_hold_falls_back_to_the_first() {
  // Two ways a selection outlives its toolpath: the toolpath is deleted, or a document is opened over it.
  // Neither is an error state — it is the same answer as no selection at all, and the card must not go stale.
  let ctx = egui::Context::default();
  let mut app = AppState::new(two_toolpaths());
  let mut shell = Shell::new();
  shell.set_tools(library());

  shell.set_toolpath_selection(Some("TP-gone".to_owned()));
  let (stale, _) = editor_frame(&ctx, &mut shell, &mut app);
  assert_eq!(
    stale.drawn.task_editor, "Edit · Perfil 01",
    "a selection the document does not hold falls back to the first toolpath"
  );
  assert_eq!(
    stale.drawn.tool_card[0], "TOOL — Perfil 01",
    "the card describes the first toolpath rather than the one that is gone"
  );
  assert_eq!(
    stale.drawn.toolpath_selected.as_deref(),
    Some("TP1"),
    "the shell reports the toolpath it really describes"
  );
  assert_eq!(
    stale
      .drawn
      .toolpath_rows
      .iter()
      .map(|(_, selected)| *selected)
      .collect::<Vec<_>>(),
    vec![true, false],
    "and the first row is the one that reads as selected: {:?}",
    stale.drawn.toolpath_rows
  );

  // The other shape: a document opened over the selection, whose toolpaths have ids the selection never saw.
  shell.set_toolpath_selection(Some("TP2".to_owned()));
  let mut opened = sheet();
  opened
    .toolpaths
    .push(toolpath("TP8", "Opened 1", 4, pocket(3.0)));
  app.replace_document(opened);
  let (output, _) = frame(&ctx, &mut shell, &mut app);
  assert_eq!(
    output.drawn.task_editor, "Edit · Opened 1",
    "a document opened over the selection falls back to its own first toolpath"
  );
  assert_eq!(output.drawn.tool_card[0], "TOOL — Opened 1");
  assert_eq!(output.drawn.toolpath_selected.as_deref(), Some("TP8"));
}

#[test]
fn the_tool_library_retools_the_toolpath_the_card_describes() {
  // The one door beside the card: `Choose another tool…` opens the library aimed at the described toolpath. It
  // used to aim at `toolpaths.first()` (crates/cam-ui/src/tool_library.rs:538), which would have retooled a
  // different toolpath than the one the card named — the same `first()` defect S4 removes from the reading
  // path, one press away from writing it.
  let ctx = egui::Context::default();
  let mut app = AppState::new(two_toolpaths());
  let mut shell = Shell::new();
  shell.set_tools(library());

  let (before, _) = editor_frame(&ctx, &mut shell, &mut app);
  shell.set_toolpath_selection(Some("TP2".to_owned()));
  let (pressed, _) = click_row(&ctx, &mut shell, &mut app, &before, "Desbaste 1");
  assert!(pressed.actions.is_empty(), "selecting is not an edit");

  shell.open_tool_library(&app);
  assert_eq!(
    shell.tool_library_target(),
    Some("TP2"),
    "the library must retool the toolpath the card describes, not the document's first"
  );

  // With no selection the card describes the first toolpath, and the library must agree with it.
  shell.close_tool_library();
  shell.set_toolpath_selection(None);
  shell.open_tool_library(&app);
  assert_eq!(
    shell.tool_library_target(),
    Some("TP1"),
    "with nothing selected the dialog retools the first toolpath, exactly as the card reads"
  );
}
