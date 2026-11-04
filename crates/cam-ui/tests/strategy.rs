//! A strategy's editor: one surface for creating and editing (camui-prancheta T8, spec S10).
//!
//! S10 draws one editor twice: `New toolpath · <name>` and `Edit · <name>`, holding the name field, the
//! current-tool card with `Choose another tool…`, the collapsible `Quick parameter edit`, the strategy's own
//! sections, the note `The toolpath is calculated when you click "Generate toolpath".` and the pair
//! `Generate toolpath` / `Cancel`. The reference frames are `Screenshot 2026-11-03 034023.png` (creating a
//! `Desbaste 3D`) and `033350.png` (editing a `Perfil`).
//!
//! What these tests pin is what the operator can act on: the header the frame drew
//! ([`cam_ui::DrawnState::task_editor`]), the points the frame really showed
//! ([`cam_ui::DrawnState::strategy_hits`]), the words and pictures of its cards
//! ([`cam_ui::DrawnState::strategy_cards`]) and the state after each footer button. Three rules from this
//! feature's own log are applied rather than paid for again: a control is drawn only when it governs something
//! (L6, L14, L22), a headless test reads shapes and therefore needs a point the frame actually showed (L23),
//! and one number is one number — the quick block duplicates the section's values, never a second copy of them.

use cam_app::AppState;
use cam_geom::{Contour, Point2};
use cam_model::{
  Document, DrillParams, Entity, EntityKind, Job, Layer, Operation, PocketParams, PocketStrategy,
  ProfileParams, Toolpath,
};
use cam_store::tools::{CuttingData, StoredTool, ToolGeometry};
use cam_ui::strategy::{self, Draft};
use cam_ui::{Category, Shell, ShellOutput, UiAction};
use cam_units::{Length, UnitSystem};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

/// A sheet with one rectangle (`E1`) and nothing else.
fn rectangle_document() -> Document {
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

/// The same sheet with one toolpath on it, so the editor has something to edit.
fn document_with(operation: Operation, name: &str) -> Document {
  let mut document = rectangle_document();
  document.toolpaths.push(Toolpath {
    id: "TP9".to_owned(),
    name: name.to_owned(),
    entity_ids: vec!["E1".to_owned()],
    island_ids: Vec::new(),
    tool_number: 2,
    operation,
  });
  document
}

/// A document whose toolpath is a profile: the `Editar · Perfil` frame.
fn profile_document() -> Document {
  document_with(Operation::Profile(ProfileParams::default()), "Perfil 01")
}

/// A document whose toolpath is a pocket: the `Desbaste` frame.
fn pocket_document() -> Document {
  document_with(Operation::Pocket(PocketParams::default()), "Desbaste 1")
}

/// One tool in the library snapshot: a Ø6 flat end mill numbered `T2`, with rates the quick block reads.
fn flat_mill() -> StoredTool {
  StoredTool {
    geometry: ToolGeometry {
      id: "G2".to_owned(),
      name_format: "{Tool Type} ({Diameter|F}{Units Short})".to_owned(),
      tool_type: 0,
      units: UnitSystem::Mm,
      diameter: Some(Length::from_mm(6.0)),
      included_angle: None,
      flat_diameter: None,
      num_flutes: Some(2),
      flute_length: None,
      tip_radius: None,
      custom_attributes: None,
    },
    cutting: CuttingData {
      id: "G2-cut".to_owned(),
      rate_units: UnitSystem::Mm,
      feed_rate: Some(100.0),
      plunge_rate: Some(30.0),
      spindle_speed: Some(18_000),
      stepdown: None,
      stepover: None,
      clear_stepover: None,
      tool_number: Some(2),
      notes: None,
    },
  }
}

/// The editor's own sentence: S10 gives it verbatim, so every test quotes it from one place.
const NOTE: &str = "The toolpath is calculated when you click \"Generate toolpath\".";

// ---------------------------------------------------------------------------------------------
// Frames
// ---------------------------------------------------------------------------------------------

/// A window tall enough to hold the whole editor, so every reported point is one the frame drew.
///
/// The inspector's body is a scroll area capped at 55 % of the window, and S10's editor is longer than that at
/// the standard 800 px: a point read from a clipped frame is exactly the defect L23 found in the playback bar,
/// so the tests that press a control read a frame that showed it.
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

/// Open the `TOOLPATHS` task — the surface S10's editor lives on — and report a settled frame.
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
    last = captured.map(|output| (output, texts));
  }
  last.expect("a click is two frames")
}

/// Where the editor drew the control that answers to `word`, or a panic naming what it drew instead.
fn point_of(hits: &[(String, (f32, f32))], word: &str) -> egui::Pos2 {
  hits
    .iter()
    .find(|(name, _)| name == word)
    .map(|(_, point)| egui::pos2(point.0, point.1))
    .unwrap_or_else(|| panic!("nothing called {word:?} in the editor: {hits:?}"))
}

/// Press one of the editor's own controls by the word on it.
fn click_word(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
  drawn: &ShellOutput,
  word: &str,
) -> ShellOutput {
  let at = point_of(&drawn.drawn.strategy_hits, word);
  click_at(ctx, shell, app, at).0
}

/// The words of the editor's controls, in the order it drew them.
fn controls(drawn: &ShellOutput) -> Vec<String> {
  drawn
    .drawn
    .strategy_hits
    .iter()
    .map(|(word, _)| word.clone())
    .collect()
}

// ---------------------------------------------------------------------------------------------
// One surface for both modes
// ---------------------------------------------------------------------------------------------

#[test]
fn creating_and_editing_are_one_surface_and_only_the_header_prefix_changes() {
  // S10's first sentence. The two modes run the same code path; the subject the document puts in front of it
  // decides the prefix, and nothing else about the surface.
  let ctx = egui::Context::default();

  // Creating: the document holds no toolpath, so the draft is the editor's subject.
  let mut app = AppState::new(rectangle_document());
  app.select(vec!["E1".to_owned()]);
  let mut shell = Shell::new();
  shell.set_tools(vec![flat_mill()]);
  // The editor seeds its draft from the document, so the tests that set a draft open the editor first and
  // then set it — the order a host follows too.
  let _ = editor_frame(&ctx, &mut shell, &mut app);
  shell.strategy.tool_number = Some(2);
  let (creating, _) = editor_frame(&ctx, &mut shell, &mut app);
  assert_eq!(
    creating.drawn.task_editor, "New toolpath · Profile 1",
    "the creating mode's header names the draft"
  );

  // Editing: the document holds one toolpath, and the editor edits it.
  let mut app = AppState::new(profile_document());
  let mut shell = Shell::new();
  shell.set_tools(vec![flat_mill()]);
  let (editing, _) = editor_frame(&ctx, &mut shell, &mut app);
  assert_eq!(
    editing.drawn.task_editor, "Edit · Perfil 01",
    "the editing mode's header names the document's toolpath"
  );

  // The same surface: every control the one mode drew, the other drew too, in the same order. Two differences
  // exist and both are named here rather than left to be discovered:
  //
  // - the header prefix, above;
  // - the name. `cam-app` carries `AddToolpath` and `SetToolpathOperation` and **no rename command**, so while
  //   editing, the name is drawn as the document's own reading with the sentence saying why — a field that
  //   would drop what was typed is the L6 dead control this feature keeps correcting.
  let mut creating_controls = controls(&creating);
  creating_controls.retain(|word| word != "Name");
  assert_eq!(
    creating_controls,
    controls(&editing),
    "creating and editing must offer the same editor"
  );
  assert!(
    controls(&creating).iter().any(|word| word == "Name"),
    "creating a toolpath offers its name field: {:?}",
    creating.drawn.strategy_hits
  );
  assert!(
    !controls(&editing).iter().any(|word| word == "Name"),
    "editing draws the name as a reading, not as a field: {:?}",
    editing.drawn.strategy_hits
  );
  let (_, editing_text) = editor_frame(&ctx, &mut shell, &mut app);
  assert!(
    editing_text
      .iter()
      .any(|text| text.starts_with("The name is fixed while editing")),
    "and says why the name cannot be typed: {editing_text:?}"
  );
  assert_eq!(
    creating.drawn.strategy_note, editing.drawn.strategy_note,
    "the note is the same sentence in both modes"
  );
  assert_eq!(
    creating.drawn.strategy_cards, editing.drawn.strategy_cards,
    "the strategy's picture cards are the same choice in both modes"
  );
}

#[test]
fn the_editor_holds_its_parts_in_the_specs_order() {
  // S10's order: name, current-tool card with `Choose another tool…`, the collapsible `Quick parameter edit`,
  // the sections, the note, then the footer pair. Read from the painted text, which is drawn in that order,
  // so a part that moved is a part whose index moved.
  let ctx = egui::Context::default();
  let mut app = AppState::new(profile_document());
  let mut shell = Shell::new();
  shell.set_tools(vec![flat_mill()]);

  let (output, painted) = editor_frame(&ctx, &mut shell, &mut app);
  let index = |word: &str| {
    painted
      .iter()
      .position(|text| text == word)
      .unwrap_or_else(|| panic!("the editor did not draw {word:?}: {painted:?}"))
  };
  let order = [
    "Name",
    "TOOL — Perfil 01",
    "Choose another tool…",
    "Quick parameter edit",
    "CUT",
    NOTE,
    "Generate toolpath",
    "Cancel",
  ];
  let mut last = 0;
  for word in order {
    let at = index(word);
    assert!(
      at >= last,
      "S10 puts {word:?} after the part before it, and the frame drew it before: {painted:?}"
    );
    last = at;
  }
  // The card's own opener is a point the frame really showed, not a word painted in passing.
  assert!(
    output
      .drawn
      .tool_card_hits
      .iter()
      .any(|(word, _)| word == "Choose another tool…"),
    "the card's opener must be pressable: {:?}",
    output.drawn.tool_card_hits
  );
}

#[test]
fn the_note_is_the_specs_sentence_verbatim() {
  // S10 quotes the sentence, and the reference frame prints it under the sections. One string, drawn and
  // reported, so the frame and this test cannot read two spellings of it.
  let ctx = egui::Context::default();
  let mut app = AppState::new(profile_document());
  let mut shell = Shell::new();
  shell.set_tools(vec![flat_mill()]);

  let (output, painted) = editor_frame(&ctx, &mut shell, &mut app);

  assert_eq!(output.drawn.strategy_note, NOTE);
  assert!(
    painted.iter().any(|text| text == NOTE),
    "the note must be on the frame, not only in the report: {painted:?}"
  );
}

// ---------------------------------------------------------------------------------------------
// The footer: Generate toolpath and Cancel
// ---------------------------------------------------------------------------------------------

#[test]
fn generating_a_new_strategy_emits_the_action_and_cancel_leaves_the_document_as_it_was() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(rectangle_document());
  app.select(vec!["E1".to_owned()]);
  let mut shell = Shell::new();
  shell.set_tools(vec![flat_mill()]);
  let _ = editor_frame(&ctx, &mut shell, &mut app);
  shell.strategy.tool_number = Some(2);
  shell.strategy.name = "Profile 1".to_owned();
  shell.strategy.operation = Operation::Profile(ProfileParams {
    pass_depth: Length::from_mm(1.5),
    ..ProfileParams::default()
  });

  let (drawn, _) = editor_frame(&ctx, &mut shell, &mut app);
  assert!(
    drawn.drawn.strategy_refusal.is_empty(),
    "a complete draft must not be refused: {:?}",
    drawn.drawn.strategy_refusal
  );
  let pressed = click_word(&ctx, &mut shell, &mut app, &drawn, "Generate toolpath");

  // The button emits an authoring action, and emitting it is not applying it: the document is the host's.
  assert_eq!(pressed.actions.len(), 1, "{:?}", pressed.actions);
  let action = pressed.actions[0].clone();
  match &action {
    UiAction::AddToolpath {
      name,
      entity_ids,
      tool_number,
      operation,
      ..
    } => {
      assert_eq!(
        name, "Profile 1",
        "the name the editor holds is the name written"
      );
      assert_eq!(entity_ids, &vec!["E1".to_owned()]);
      assert_eq!(*tool_number, 2, "the chosen tool is the tool written");
      assert_eq!(*operation, shell.strategy.operation);
    }
    other => panic!("`Generate toolpath` must add a toolpath, got {other:?}"),
  }
  assert!(
    app.document().toolpaths.is_empty(),
    "the frame may not write the document behind the host's back"
  );

  // Applied the way a host applies it, the strategy exists, and the editor now edits it.
  shell.apply_action(&mut app, &action);
  assert_eq!(app.document().toolpaths.len(), 1);
  assert_eq!(app.document().toolpaths[0].name, "Profile 1");
  let (after, _) = editor_frame(&ctx, &mut shell, &mut app);
  assert_eq!(
    after.drawn.task_editor, "Edit · Profile 1",
    "the editor that created a strategy edits it on the next frame"
  );

  // Cancel: the document, the tool and the name stay exactly as they were, and the task leaves with the draft.
  let before = app.document().clone();
  let (drawn, _) = editor_frame(&ctx, &mut shell, &mut app);
  let cancelled = click_word(&ctx, &mut shell, &mut app, &drawn, "Cancel");
  assert!(
    cancelled.actions.is_empty(),
    "Cancel must not emit anything: {:?}",
    cancelled.actions
  );
  assert_eq!(
    app.document(),
    &before,
    "Cancel leaves the document as it was"
  );
  assert_eq!(shell.task(), None, "Cancel leaves the task");
  let (reopened, _) = editor_frame(&ctx, &mut shell, &mut app);
  assert_eq!(
    reopened.drawn.task_editor, "Edit · Profile 1",
    "and re-opening reads the document's own name, not a discarded draft"
  );
}

#[test]
fn generating_an_existing_strategy_rewrites_its_operation_and_leaves_the_rest_alone() {
  // Editing is not re-creating: `Generate toolpath` writes the operation, and the geometry, the tool and the
  // name stay the document's.
  let ctx = egui::Context::default();
  let mut app = AppState::new(profile_document());
  let mut shell = Shell::new();
  shell.set_tools(vec![flat_mill()]);
  let (drawn, _) = editor_frame(&ctx, &mut shell, &mut app);
  assert!(
    drawn
      .drawn
      .strategy_fields
      .iter()
      .any(|(label, value)| label == "Pass depth" && value == "6.00 mm"),
    "the section must read the document's own number: {:?}",
    drawn.drawn.strategy_fields
  );

  // The number the editor holds is the one it writes.
  shell.strategy.operation = Operation::Profile(ProfileParams {
    pass_depth: Length::from_mm(2.25),
    ..ProfileParams::default()
  });
  let (drawn, _) = editor_frame(&ctx, &mut shell, &mut app);
  let pressed = click_word(&ctx, &mut shell, &mut app, &drawn, "Generate toolpath");
  let action = pressed.actions[0].clone();
  match &action {
    UiAction::SetToolpathOperation {
      toolpath_id,
      operation,
    } => {
      assert_eq!(toolpath_id, "TP9");
      match operation {
        Operation::Profile(params) => assert_eq!(params.pass_depth, Length::from_mm(2.25)),
        other => panic!("the draft was a profile, got {other:?}"),
      }
    }
    other => panic!("editing a strategy must rewrite its operation, got {other:?}"),
  }
  shell.apply_action(&mut app, &action);
  assert_eq!(
    app.document().toolpaths[0].name,
    "Perfil 01",
    "a rewrite keeps the name"
  );
  assert_eq!(
    app.document().toolpaths[0].tool_number,
    2,
    "a rewrite keeps the tool"
  );
  assert_eq!(
    app.document().toolpaths[0].entity_ids,
    vec!["E1".to_owned()]
  );
}

#[test]
fn generate_is_drawn_only_when_it_can_act_and_the_footer_says_why_when_it_cannot() {
  // L6/L14: a button that emits nothing is the dead control this feature keeps correcting. Each state below is
  // one the sentence names, and the sentence is what replaces the button.
  let ctx = egui::Context::default();
  let cases: Vec<(&str, Document, Option<u32>, bool, &str)> = vec![
    (
      "no tool chosen",
      rectangle_document(),
      None,
      true,
      "No tool yet",
    ),
    (
      "nothing selected",
      rectangle_document(),
      Some(2),
      false,
      "Nothing is selected",
    ),
  ];
  for (label, document, tool, select, expected) in cases {
    let mut app = AppState::new(document);
    if select {
      app.select(vec!["E1".to_owned()]);
    }
    let mut shell = Shell::new();
    shell.set_tools(vec![flat_mill()]);
    let _ = editor_frame(&ctx, &mut shell, &mut app);
    shell.strategy.tool_number = tool;
    let (drawn, _) = editor_frame(&ctx, &mut shell, &mut app);
    assert!(
      !controls(&drawn)
        .iter()
        .any(|word| word == "Generate toolpath"),
      "{label}: Generate must not be drawn while it cannot act: {:?}",
      drawn.drawn.strategy_hits
    );
    assert!(
      drawn.drawn.strategy_refusal.contains(expected),
      "{label}: the footer must say why, got {:?}",
      drawn.drawn.strategy_refusal
    );
    assert!(
      controls(&drawn).iter().any(|word| word == "Cancel"),
      "{label}: Cancel can always be honoured, so it is always drawn"
    );
  }

  // A strategy whose operation this shell does not draft: no form, no button, and the sentence names both the
  // operation and the route that does author it.
  let mut app = AppState::new(document_with(
    Operation::Drilling(DrillParams::default()),
    "Drill 1",
  ));
  let mut shell = Shell::new();
  shell.set_tools(vec![flat_mill()]);
  let (drawn, _) = editor_frame(&ctx, &mut shell, &mut app);
  assert!(
    !controls(&drawn)
      .iter()
      .any(|word| word == "Generate toolpath"),
    "an operation with no editor here must not offer Generate: {:?}",
    drawn.drawn.strategy_hits
  );
  assert!(
    drawn.drawn.strategy_refusal.contains("Drilling")
      && drawn.drawn.strategy_refusal.contains("cam-cli"),
    "the refusal must name the operation and the route: {:?}",
    drawn.drawn.strategy_refusal
  );
  // Choosing a kind this shell does draft replaces it, and the footer comes back: the refusal is a state, not
  // a dead end.
  let (drawn, _) = editor_frame(&ctx, &mut shell, &mut app);
  let after = click_word(&ctx, &mut shell, &mut app, &drawn, "Pocket");
  assert!(
    controls(&after)
      .iter()
      .any(|word| word == "Generate toolpath"),
    "choosing a drafted kind must restore the footer: {:?}",
    after.drawn.strategy_hits
  );
}

// ---------------------------------------------------------------------------------------------
// The quick parameter edit
// ---------------------------------------------------------------------------------------------

#[test]
fn the_quick_edit_duplicates_the_sections_numbers_and_folds_on_its_own() {
  // The reference's `Edição rápida dos parâmetros` repeats the numbers you actually change above the full
  // section. It repeats the *values*: one number, two widgets, never two copies.
  let ctx = egui::Context::default();
  let mut app = AppState::new(pocket_document());
  let mut shell = Shell::new();
  shell.set_tools(vec![flat_mill()]);
  let _ = editor_frame(&ctx, &mut shell, &mut app);
  shell.strategy.operation = Operation::Pocket(PocketParams {
    pass_depth: Length::from_mm(1.25),
    stepover: Length::from_mm(2.5),
    ..PocketParams::default()
  });

  let (drawn, _) = editor_frame(&ctx, &mut shell, &mut app);
  assert!(drawn.drawn.strategy_quick_open, "the block starts expanded");
  let quick = drawn.drawn.strategy_quick_fields.clone();
  assert_eq!(
    quick,
    vec![
      ("Pass depth".to_owned(), "1.25 mm".to_owned()),
      ("Stepover".to_owned(), "2.50 mm".to_owned()),
    ],
    "the quick block holds the numbers the operator changes most"
  );
  for (label, value) in &quick {
    assert!(
      drawn
        .drawn
        .strategy_fields
        .iter()
        .any(|(section_label, section_value)| section_label == label && section_value == value),
      "the quick block's {label:?} must be the section's own number, got {value:?} against {:?}",
      drawn.drawn.strategy_fields
    );
  }

  // Folding it draws no row and leaves the section exactly where it was.
  let folded = click_word(&ctx, &mut shell, &mut app, &drawn, "Quick parameter edit");
  assert!(
    !folded.drawn.strategy_quick_open,
    "the heading folds the block"
  );
  assert!(
    folded.drawn.strategy_quick_fields.is_empty(),
    "a folded block draws no number: {:?}",
    folded.drawn.strategy_quick_fields
  );
  assert_eq!(
    folded.drawn.strategy_fields.len(),
    drawn.drawn.strategy_fields.len(),
    "the section behind it keeps every number"
  );

  // Unfolding brings the same numbers back, and the tool's own rates are readings with their units that say
  // where they live: the library owns them.
  let unfolded = click_word(&ctx, &mut shell, &mut app, &folded, "Quick parameter edit");
  assert!(
    unfolded.drawn.strategy_quick_open,
    "pressing the heading again expands it"
  );
  let _ = frame(&ctx, &mut shell, &mut app);
  let (_again, painted) = frame(&ctx, &mut shell, &mut app);
  assert_eq!(
    _again.drawn.strategy_quick_fields, quick,
    "and the same numbers come back"
  );
  for word in ["Feed", "100.00 mm/s", "Plunge", "Spindle"] {
    assert!(
      painted.iter().any(|text| text == word),
      "the quick block must show the tool's {word:?}: {painted:?}"
    );
  }
  assert!(
    painted
      .iter()
      .any(|text| text.starts_with("Feed, plunge and spindle")),
    "and it must say where they are changed: {painted:?}"
  );
}

// ---------------------------------------------------------------------------------------------
// A choice that needs a picture
// ---------------------------------------------------------------------------------------------

#[test]
fn a_choice_that_needs_a_picture_is_a_card_with_a_picture_and_the_choice_reaches_the_action() {
  // S10's last sentence. The card carries a diagram painted from the operation's own semantics — never an empty
  // box and never a screenshot of the reference app — and pressing it changes the strategy the footer writes.
  let ctx = egui::Context::default();
  let mut app = AppState::new(rectangle_document());
  app.select(vec!["E1".to_owned()]);
  let mut shell = Shell::new();
  shell.set_tools(vec![flat_mill()]);
  let _ = editor_frame(&ctx, &mut shell, &mut app);
  shell.strategy.tool_number = Some(2);

  let (drawn, _) = editor_frame(&ctx, &mut shell, &mut app);
  let words: Vec<(String, bool)> = drawn
    .drawn
    .strategy_cards
    .iter()
    .map(|(word, chosen, _)| (word.clone(), *chosen))
    .collect();
  assert_eq!(
    words,
    vec![("Profile".to_owned(), true), ("Pocket".to_owned(), false)],
    "the draft starts as a profile and the card says so"
  );
  for (word, _, strokes) in &drawn.drawn.strategy_cards {
    assert!(
      *strokes >= 2,
      "the {word:?} card must carry a picture, and an empty box is what L23 taught us to check for: {strokes} \
       strokes"
    );
  }

  // Pressing `Pocket` chooses it in the same frame: the cards, the sections and the footer all follow.
  let chosen = click_word(&ctx, &mut shell, &mut app, &drawn, "Pocket");
  assert!(
    chosen
      .drawn
      .strategy_cards
      .iter()
      .any(|(word, picked, _)| word == "Pocket" && *picked),
    "the frame that presses the card is the frame that reports it chosen: {:?}",
    chosen.drawn.strategy_cards
  );
  assert!(
    chosen
      .drawn
      .strategy_fields
      .iter()
      .any(|(label, _)| label == "Stepover"),
    "a pocket's section holds the numbers a pocket has: {:?}",
    chosen.drawn.strategy_fields
  );
  // The clearing pattern is the second choice shaped by a picture, and it appears with the pocket.
  assert!(
    chosen
      .drawn
      .strategy_cards
      .iter()
      .any(|(word, picked, strokes)| word == "Offset" && *picked && *strokes >= 2),
    "a pocket's clearing pattern is drawn as cards too: {:?}",
    chosen.drawn.strategy_cards
  );
  // The frame that applies a choice reports the layout it drew, which is the one before the choice grew the
  // sections above it. A second press reads the frame the operator would be looking at.
  let _ = frame(&ctx, &mut shell, &mut app);
  let (settled, _) = frame(&ctx, &mut shell, &mut app);
  let raster = click_word(&ctx, &mut shell, &mut app, &settled, "Raster");
  assert!(
    raster
      .drawn
      .strategy_cards
      .iter()
      .any(|(word, picked, _)| word == "Raster" && *picked),
    "pressing the raster card chooses it: {:?}",
    raster.drawn.strategy_cards
  );

  let pressed = click_word(&ctx, &mut shell, &mut app, &raster, "Generate toolpath");
  match &pressed.actions[0] {
    UiAction::AddToolpath { operation, .. } => match operation {
      Operation::Pocket(params) => {
        assert_eq!(params.strategy, PocketStrategy::Raster);
      }
      other => panic!("the card chose a pocket, got {other:?}"),
    },
    other => panic!("expected a toolpath to be added, got {other:?}"),
  }
}

// ---------------------------------------------------------------------------------------------
// The tool: the card's opener, and the library that fills the draft
// ---------------------------------------------------------------------------------------------

#[test]
fn choosing_a_tool_for_a_new_strategy_writes_the_draft_and_generate_writes_the_document() {
  // The card's opener is the same row S9 built. What changes is the subject: with no toolpath on screen the
  // choice is the draft's tool, and `Generate toolpath` is what writes it.
  let ctx = egui::Context::default();
  let mut app = AppState::new(rectangle_document());
  app.select(vec!["E1".to_owned()]);
  let mut shell = Shell::new();
  shell.set_tools(vec![flat_mill()]);

  let (drawn, _) = editor_frame(&ctx, &mut shell, &mut app);
  assert!(
    drawn
      .drawn
      .tool_card
      .iter()
      .any(|line| line.starts_with("No tool chosen yet")),
    "a card with no tool says so instead of drawing nothing: {:?}",
    drawn.drawn.tool_card
  );
  assert!(
    !controls(&drawn)
      .iter()
      .any(|word| word == "Generate toolpath"),
    "and the footer says why Generate is absent"
  );

  let at = point_of(&drawn.drawn.tool_card_hits, "Choose another tool…");
  let (_opened, _) = click_at(&ctx, &mut shell, &mut app, at);
  assert!(shell.show_tool_library, "the opener must open the library");
  // The rows take a pass to settle, the way every modal in this shell does.
  let _ = frame(&ctx, &mut shell, &mut app);
  let mut latest = frame(&ctx, &mut shell, &mut app).0;
  let row = latest
    .drawn
    .tool_library_hits
    .iter()
    .find(|(word, _)| word.starts_with("T2"))
    .map(|(_, point)| egui::pos2(point.0, point.1))
    .unwrap_or_else(|| {
      panic!(
        "the library drew no T2 row: {:?}",
        latest.drawn.tool_library_hits
      )
    });
  let _ = click_at(&ctx, &mut shell, &mut app, row);
  latest = frame(&ctx, &mut shell, &mut app).0;
  assert!(
    latest
      .drawn
      .tool_library_hits
      .iter()
      .any(|(word, _)| word == "Use this tool"),
    "a draft with a chosen tool can use it: {:?}",
    latest.drawn.tool_library_hits
  );
  let (committed, _) = click_at(
    &ctx,
    &mut shell,
    &mut app,
    point_of(&latest.drawn.tool_library_hits, "Use this tool"),
  );
  assert_eq!(
    shell.strategy.tool_number,
    Some(2),
    "`Use this tool` writes the draft's tool"
  );
  assert!(
    committed.actions.is_empty(),
    "and writes no document: Generate is what writes it: {:?}",
    committed.actions
  );
  assert_eq!(
    app.document().toolpaths.len(),
    0,
    "choosing a tool for a draft must not create a toolpath"
  );

  let (drawn, _) = editor_frame(&ctx, &mut shell, &mut app);
  assert!(
    drawn
      .drawn
      .tool_card
      .iter()
      .any(|line| line.starts_with("T2 - ")),
    "the card reads the chosen tool back: {:?}",
    drawn.drawn.tool_card
  );
  assert!(
    controls(&drawn)
      .iter()
      .any(|word| word == "Generate toolpath"),
    "with a tool and a region, the footer is back: {:?}",
    drawn.drawn.strategy_hits
  );
}

// ---------------------------------------------------------------------------------------------
// Preserved: the editor is the toolpath task's, and no other panel's
// ---------------------------------------------------------------------------------------------

#[test]
fn the_editor_is_the_toolpath_task_and_the_other_tasks_are_untouched() {
  // The editor replaces the panel's body only while `TOOLPATHS` is open. The other two tasks keep their own
  // header and draw none of S10's surface, which is what keeps the inspector's tabs working.
  let ctx = egui::Context::default();
  let mut app = AppState::new(profile_document());
  let mut shell = Shell::new();
  shell.set_tools(vec![flat_mill()]);

  // Idle: no task, no editor.
  let (idle, _) = {
    let _ = frame(&ctx, &mut shell, &mut app);
    frame(&ctx, &mut shell, &mut app)
  };
  assert!(idle.drawn.task_editor.is_empty(), "idle draws no editor");
  assert!(
    idle.drawn.strategy_note.is_empty() && idle.drawn.strategy_hits.is_empty(),
    "idle draws none of S10's surface: {:?}",
    idle.drawn.strategy_hits
  );

  for (category, label) in [
    (Category::Relief3d, "RELIEF AND 3D PART"),
    (Category::MachineOutput, "MACHINE AND OUTPUT"),
  ] {
    shell.open_task(category);
    let _ = frame(&ctx, &mut shell, &mut app);
    let (output, _) = frame(&ctx, &mut shell, &mut app);
    assert_eq!(
      output.drawn.task_editor, label,
      "{label} keeps its own header"
    );
    assert!(
      output.drawn.strategy_note.is_empty(),
      "{label} draws no strategy note"
    );
  }
}

#[test]
fn the_draft_seeds_from_the_document_and_cancel_puts_it_back() {
  // The draft is a draft: it seeds from the toolpath it edits, it can be typed into, and `Cancel` throws the
  // typed numbers away so the next opening reads the document again.
  let ctx = egui::Context::default();
  let mut app = AppState::new(profile_document());
  let mut shell = Shell::new();
  shell.set_tools(vec![flat_mill()]);

  let (_drawn, _) = editor_frame(&ctx, &mut shell, &mut app);
  assert_eq!(
    shell.strategy.name, "Perfil 01",
    "the draft is seeded with the document's name"
  );
  assert_eq!(
    shell.strategy.operation,
    Operation::Profile(ProfileParams::default()),
    "and with the document's operation"
  );

  shell.strategy.operation = Operation::Profile(ProfileParams {
    cut_depth: Length::from_mm(3.0),
    ..ProfileParams::default()
  });
  let (drawn, _) = editor_frame(&ctx, &mut shell, &mut app);
  let cancelled = click_word(&ctx, &mut shell, &mut app, &drawn, "Cancel");
  assert!(cancelled.actions.is_empty());
  assert_eq!(shell.task(), None);

  let (reopened, _) = editor_frame(&ctx, &mut shell, &mut app);
  assert_eq!(
    reopened
      .drawn
      .strategy_fields
      .iter()
      .find(|(label, _)| label == "Cut depth")
      .map(|(_, value)| value.clone()),
    Some("18.00 mm".to_owned()),
    "Cancel discards the typed number: the section reads the document again"
  );
  assert_eq!(
    shell.strategy.operation,
    Operation::Profile(ProfileParams::default())
  );
}

#[test]
fn the_draft_is_a_public_parameter_so_a_host_or_a_test_can_set_it() {
  // The same rule `Shell::material_size` follows: a draft that could not be set from outside a frame is a
  // draft only a mouse could test. This also pins the defaults the editor opens with.
  let draft = Draft::default();
  assert_eq!(draft.name, "Profile 1");
  assert_eq!(draft.tool_number, None);
  assert!(draft.quick_open);
  assert_eq!(
    draft.operation,
    Operation::Profile(ProfileParams::default())
  );
  assert_eq!(strategy::NOTE, NOTE);
}

#[test]
fn a_control_the_scroll_area_clips_is_not_reported_as_pressable() {
  // L23, applied before a reviewer has to find it: a clipped control is still a shape, so a reported point is
  // only a promise while the frame really showed it. At 800 px the inspector's body is shorter than the editor,
  // and the footer sits below its clip — an operator cannot press what is not on screen.
  let ctx = egui::Context::default();
  let mut app = AppState::new(profile_document());
  let mut shell = Shell::new();
  shell.set_tools(vec![flat_mill()]);
  shell.open_task(Category::Toolpaths);

  let short = || egui::RawInput {
    screen_rect: Some(egui::Rect::from_min_size(
      egui::Pos2::ZERO,
      egui::vec2(1200.0, 800.0),
    )),
    ..Default::default()
  };
  let mut last = None;
  for _ in 0..2 {
    let mut captured: Option<ShellOutput> = None;
    let full = ctx.run_pass(short(), |ctx| {
      captured = Some(shell.show(&mut app, ctx));
    });
    full.drop_without_applying_deltas();
    last = captured;
  }
  let output = last.expect("the shell ran");

  // The frame still draws the editor — this is not "nothing was drawn" — and it reports the parts it showed.
  assert_eq!(output.drawn.strategy_note, NOTE);
  assert!(
    output
      .drawn
      .strategy_hits
      .iter()
      .any(|(word, _)| word == "Quick parameter edit"),
    "the parts the frame showed are pressable: {:?}",
    output.drawn.strategy_hits
  );
  for word in ["Generate toolpath", "Cancel"] {
    assert!(
      !output
        .drawn
        .strategy_hits
        .iter()
        .any(|(name, _)| name == word),
      "{word} is below the scroll area's clip at 800 px and must not be reported as pressable: {:?}",
      output.drawn.strategy_hits
    );
  }
}
