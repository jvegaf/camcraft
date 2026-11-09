use cam_app::AppState;
use cam_geom::{Contour, Point2};
use cam_model::{Document, Entity, EntityKind, Job, Layer, ProfileParams, Toolpath};
use cam_store::tools::{CuttingData, StoredTool, ToolGeometry};
use cam_ui::chrome::Category;
use cam_ui::{Shell, ShellOutput};
use cam_units::{Angle, Length, UnitSystem};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

/// A document with one rectangle and one profile toolpath cutting with `tool_number`.
pub(crate) fn document_with_toolpath(tool_number: u32) -> Document {
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
pub(crate) fn base(id: &str, number: Option<u32>, diameter: Option<f64>) -> StoredTool {
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
      shank_diameter: None,
      overall_length: None,
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

/// A flat end mill: a diameter and nothing that shapes its tip.
pub(crate) fn flat_mill(id: &str, number: Option<u32>, diameter: f64) -> StoredTool {
  base(id, number, Some(diameter))
}

/// A ball nose: a tip radius of exactly half the diameter.
pub(crate) fn ball_nose(id: &str, number: Option<u32>, diameter: f64) -> StoredTool {
  let mut tool = base(id, number, Some(diameter));
  tool.geometry.tip_radius = Some(Length::from_mm(diameter / 2.0));
  tool
}

/// A corner radius: a tip radius that is not half the diameter.
pub(crate) fn corner_radius(
  id: &str,
  number: Option<u32>,
  diameter: f64,
  radius: f64,
) -> StoredTool {
  let mut tool = base(id, number, Some(diameter));
  tool.geometry.tip_radius = Some(Length::from_mm(radius));
  tool
}

/// A V-bit: an included angle, and the flat at the bottom of the cone.
pub(crate) fn v_bit(
  id: &str,
  number: Option<u32>,
  diameter: f64,
  angle: f64,
  flat: f64,
) -> StoredTool {
  let mut tool = base(id, number, Some(diameter));
  tool.geometry.included_angle = Some(Angle::from_degrees(angle));
  tool.geometry.flat_diameter = Some(Length::from_mm(flat));
  tool
}

/// The library these tests work from: the four families, two of them flat.
pub(crate) fn library() -> Vec<StoredTool> {
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

pub(crate) fn raw_input() -> egui::RawInput {
  egui::RawInput {
    screen_rect: Some(egui::Rect::from_min_size(
      egui::Pos2::ZERO,
      egui::vec2(1200.0, 800.0),
    )),
    ..Default::default()
  }
}

pub(crate) fn frame_with_events(
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

pub(crate) fn frame_with(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
) -> (ShellOutput, Vec<String>) {
  frame_with_events(ctx, shell, app, Vec::new())
}

/// Every piece of text the frame actually drew.
pub(crate) fn painted_text(output: &egui::FullOutput) -> Vec<String> {
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
pub(crate) fn click_at(
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
pub(crate) fn press(
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
pub(crate) fn open_library(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
) -> ShellOutput {
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
pub(crate) fn click_word(
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
pub(crate) fn fields_of(drawn: &ShellOutput, group: &str) -> Vec<String> {
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
pub(crate) fn group_labels(drawn: &ShellOutput) -> Vec<String> {
  drawn
    .drawn
    .tool_groups
    .iter()
    .map(|(label, _)| label.clone())
    .collect()
}

pub(crate) fn joined(texts: &[String]) -> String {
  texts.join(" | ")
}
