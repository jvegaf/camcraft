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

/// A sheet with one rectangle (`E1`) and nothing else.
pub(crate) fn rectangle_document() -> Document {
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
pub(crate) fn document_with(operation: Operation, name: &str) -> Document {
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
pub(crate) fn profile_document() -> Document {
  document_with(Operation::Profile(ProfileParams::default()), "Perfil 01")
}

/// A document whose toolpath is a pocket: the `Desbaste` frame.
pub(crate) fn pocket_document() -> Document {
  document_with(Operation::Pocket(PocketParams::default()), "Desbaste 1")
}

/// One tool in the library snapshot: a Ø6 flat end mill numbered `T2`, with rates the quick block reads.
pub(crate) fn flat_mill() -> StoredTool {
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
pub(crate) const NOTE: &str = "The toolpath is calculated when you click \"Generate toolpath\".";

// ---------------------------------------------------------------------------------------------
// Frames
// ---------------------------------------------------------------------------------------------

/// A window tall enough to hold the whole editor, so every reported point is one the frame drew.
///
/// The inspector's body is a scroll area capped at 55 % of the window, and S10's editor is longer than that at
/// the standard 800 px: a point read from a clipped frame is exactly the defect L23 found in the playback bar,
/// so the tests that press a control read a frame that showed it.
pub(crate) fn tall() -> egui::RawInput {
  egui::RawInput {
    screen_rect: Some(egui::Rect::from_min_size(
      egui::Pos2::ZERO,
      egui::vec2(1200.0, 2000.0),
    )),
    ..Default::default()
  }
}

/// The text one pass painted, in draw order.
pub(crate) fn painted_text(full: &egui::FullOutput) -> Vec<String> {
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
pub(crate) fn frame(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
) -> (ShellOutput, Vec<String>) {
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
pub(crate) fn editor_frame(
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
pub(crate) fn click_at(
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
pub(crate) fn point_of(hits: &[(String, (f32, f32))], word: &str) -> egui::Pos2 {
  hits
    .iter()
    .find(|(name, _)| name == word)
    .map(|(_, point)| egui::pos2(point.0, point.1))
    .unwrap_or_else(|| panic!("nothing called {word:?} in the editor: {hits:?}"))
}

/// Press one of the editor's own controls by the word on it.
pub(crate) fn click_word(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
  drawn: &ShellOutput,
  word: &str,
) -> ShellOutput {
  let at = point_of(&drawn.drawn.strategy_hits, word);
  click_at(ctx, shell, app, at).0
}

/// Type `text` into the editor's name field: press the field, select what it holds, then send the word.
///
/// A field that is only reported as a point is not yet a field: this presses the point the frame drew and
/// sends the keystrokes an operator would, so the draft the frame reports is one a keyboard really reached.
pub(crate) fn type_into_name(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
  drawn: &ShellOutput,
  text: &str,
) -> ShellOutput {
  let at = point_of(&drawn.drawn.strategy_hits, "Name");
  let _ = click_at(ctx, shell, app, at);
  let mut input = tall();
  input.events.push(egui::Event::Key {
    key: egui::Key::A,
    physical_key: None,
    pressed: true,
    repeat: false,
    modifiers: egui::Modifiers::COMMAND,
  });
  input.events.push(egui::Event::Text(text.to_owned()));
  let mut captured: Option<ShellOutput> = None;
  let full = ctx.run_pass(input, |ctx| {
    captured = Some(shell.show(app, ctx));
  });
  full.drop_without_applying_deltas();
  captured.expect("the shell ran")
}

/// The words of the editor's controls, in the order it drew them.
pub(crate) fn controls(drawn: &ShellOutput) -> Vec<String> {
  drawn
    .drawn
    .strategy_hits
    .iter()
    .map(|(word, _)| word.clone())
    .collect()
}
