use std::path::{Path, PathBuf};

use cam_app::AppState;
use cam_geom::{Contour, Point2};
use cam_model::{Document, Entity, EntityKind, Job, Layer, LayerRole, Toolpath};
use cam_store::tools::{CuttingData, StoredTool, ToolGeometry, ToolKind, ToolLibrary};
use cam_ui::chrome::Category;
use cam_ui::{Shell, ShellOutput};
use cam_units::Length;
use cam_units::{Angle, UnitSystem};

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
    role: LayerRole::Geometry,
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
    tool_number,
    operation: cam_model::Operation::Isolation(cam_model::IsolationParams {
      passes: 1,
      overlap: 0.5,
      direction: cam_model::MillingDirection::Climb,
      kind: cam_model::IsolationKind::Full,
      cut_depth: cam_units::Length::from_mm(0.1),
      keep_pours: false,
    }),
  });
  document
}

/// The four numbers a `StoredTool` needs that a family does not change.
pub(crate) fn base(id: &str, number: Option<u32>, diameter: Option<f64>) -> StoredTool {
  StoredTool {
    geometry: ToolGeometry {
      id: id.to_owned(),
      name_format: "{Tool Type} ({Diameter|F}{Units Short})".to_owned(),
      // The code `cam-cli tools add` always wrote: every fixture starts as a row that names no kind, so the
      // shape decides, which is the backwards compatibility rule `ToolKind::of` states.
      tool_type: ToolKind::LEGACY_RAW,
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

/// A tool that names its own kind, which is how `Engraving` and `Drill` are reachable at all: neither shape
/// can be told from the geometry, because an engraving cutter ends flat like an end mill and a drill's cone
/// reads exactly like a V's.
pub(crate) fn named(id: &str, number: Option<u32>, diameter: f64, kind: ToolKind) -> StoredTool {
  let mut tool = base(id, number, Some(diameter));
  tool.geometry.tool_type = kind.raw();
  tool
}

/// An engraving cutter: a flat end with the flat end's own kind stored.
pub(crate) fn engraving(id: &str, number: Option<u32>, diameter: f64) -> StoredTool {
  named(id, number, diameter, ToolKind::Engraving)
}

/// A drill: a pointed tip and the cone's own kind stored.
pub(crate) fn drill(id: &str, number: Option<u32>, diameter: f64, angle: f64) -> StoredTool {
  let mut tool = named(id, number, diameter, ToolKind::Drill);
  tool.geometry.included_angle = Some(Angle::from_degrees(angle));
  tool
}

/// A record a form has to describe completely: every length, every pass, every rate.
pub(crate) fn measured(id: &str, number: Option<u32>) -> StoredTool {
  let mut tool = flat_mill(id, number, 6.0);
  tool.geometry.flute_length = Some(Length::from_mm(18.0));
  tool.geometry.shank_diameter = Some(Length::from_mm(6.0));
  tool.geometry.overall_length = Some(Length::from_mm(38.0));
  tool.cutting.stepdown = Some(Length::from_mm(2.0));
  tool.cutting.stepover = Some(Length::from_mm(3.0));
  tool.cutting.feed_rate = Some(600.0);
  tool.cutting.plunge_rate = Some(200.0);
  tool.cutting.spindle_speed = Some(12_000);
  tool
}

/// A record that holds no cutting data at all, so `PASSES` and `SPEEDS` have nothing to describe.
pub(crate) fn bare(id: &str, number: Option<u32>, diameter: f64) -> StoredTool {
  let mut tool = flat_mill(id, number, diameter);
  tool.cutting.feed_rate = None;
  tool.cutting.plunge_rate = None;
  tool.cutting.spindle_speed = None;
  tool.cutting.stepdown = None;
  tool.cutting.stepover = None;
  tool
}

/// The library these tests work from: one tool of every kind, so the tree's six headings are all drawn.
pub(crate) fn library() -> Vec<StoredTool> {
  vec![
    flat_mill("G1", Some(1), 6.0),
    ball_nose("G2", Some(2), 6.0),
    corner_radius("G3", Some(3), 12.0, 2.0),
    v_bit("G4", Some(4), 12.7, 90.0, 0.2),
    engraving("G5", Some(5), 3.175),
    drill("G6", Some(6), 5.0, 118.0),
  ]
}

/// Where a test writes its files: a fresh directory in the system temp directory, removed when the test ends.
///
/// Outside the worktree on purpose: a test that wrote beside the source would leave a database in `git status`
/// the first time it failed.
pub(crate) struct Scratch {
  path: PathBuf,
}

impl Scratch {
  pub(crate) fn new(name: &str) -> Self {
    let path = std::env::temp_dir().join(format!(
      "campilot-tool-library-{}-{name}",
      std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("scratch directory");
    Self { path }
  }

  pub(crate) fn path(&self) -> &Path {
    &self.path
  }
}

impl Drop for Scratch {
  fn drop(&mut self) {
    let _ = std::fs::remove_dir_all(&self.path);
  }
}

/// Write a real `.c2d` with the store's own writer, so the shell opens a file and not a stub.
pub(crate) fn write_document(directory: &Path, name: &str, document: &Document) -> PathBuf {
  let path = directory.join(name);
  cam_store::document::save_document(&path, document).expect("document written");
  path
}

/// Write a real tool library with the store's own writer, so `Use this tool` has a file to write through.
pub(crate) fn write_library(directory: &Path, name: &str, tools: &[StoredTool]) -> PathBuf {
  let path = directory.join(name);
  let library = ToolLibrary::open(&path).expect("library created");
  for tool in tools {
    library.save_tool(tool).expect("tool written");
  }
  path
}

/// A document and the **application's** library, opened the way the window opens them.
///
/// The library is the application's and not the drawing's (tool-library-app S1): the test writes a real
/// `tools.sqlite` into the scratch directory the window treats as its data directory, opens it through
/// [`Shell::open_library_at`] — the same call the desktop host makes at launch — and only then opens a `.c2d`.
/// The drawing names no library at all, and a drawing that did name one would change nothing here.
pub(crate) fn opened_library_document(
  ctx: &egui::Context,
  scratch: &Scratch,
) -> (AppState, Shell, ShellOutput, PathBuf) {
  let library_path = write_library(scratch.path(), "tools.sqlite", &library());
  let path = write_document(scratch.path(), "part.c2d", &document_with_toolpath(1));

  let mut app = AppState::new(document_with_toolpath(1));
  let mut shell = Shell::new();
  shell
    .open_library_at(scratch.path())
    .expect("the application's library opens");
  shell.apply_action(&mut app, &cam_ui::UiAction::OpenFile { path });
  let opened = open_library(ctx, &mut shell, &mut app);
  (app, shell, opened, library_path)
}

/// The tools one library file holds, read back with the store's own reader.
pub(crate) fn tools_in(path: &Path) -> Vec<StoredTool> {
  ToolLibrary::open_read_only(path)
    .expect("the library is readable")
    .tools()
    .expect("the library lists its tools")
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

fn pass(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
  input: egui::RawInput,
) -> (ShellOutput, Vec<String>, Vec<Run>) {
  let mut captured: Option<ShellOutput> = None;
  let full = ctx.run_pass(input, |ctx| {
    captured = Some(shell.show(app, ctx));
  });
  let texts = painted_text(&full);
  let runs = painted_runs(&full);
  full.drop_without_applying_deltas();
  (captured.expect("the shell ran"), texts, runs)
}

pub(crate) fn frame_with_events(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
  events: Vec<egui::Event>,
) -> (ShellOutput, Vec<String>) {
  let mut input = raw_input();
  input.events = events;
  let (output, texts, _) = pass(ctx, shell, app, input);
  (output, texts)
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
  painted_runs(output)
    .into_iter()
    .map(|(text, _, _)| text)
    .collect()
}

/// One piece of text a frame painted: the words, the point its glyphs start at, and their size.
pub(crate) type Run = (String, egui::Pos2, egui::Vec2);

/// Every piece of text the frame actually drew, with the point it drew it at.
///
/// The words alone answer "was it painted?"; the point answers "and where?", which is the only way a headless
/// test can press a box the report does not place. The modal reports the controls it drew (its tree, its two
/// buttons) but not the form's boxes — S9 makes them editable, and a box is reached the way an operator
/// reaches it: by its own pixels, not by a rectangle a test computed from the column's padding.
pub(crate) fn painted_runs(output: &egui::FullOutput) -> Vec<Run> {
  output
    .shapes
    .iter()
    .filter_map(|clipped| match &clipped.shape {
      egui::Shape::Text(text) => {
        Some((text.galley.text().to_owned(), text.pos, text.galley.size()))
      }
      _ => None,
    })
    .collect()
}

/// One quiet frame that also hands back the points its text was painted at.
pub(crate) fn frame_with_runs(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
) -> (ShellOutput, Vec<Run>) {
  let (output, _, runs) = pass(ctx, shell, app, raw_input());
  (output, runs)
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
    let (output, texts, _) = pass(ctx, shell, app, input);
    last = Some((output, texts));
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

/// The point inside the box on the row labelled `label`, read from the frame that painted it.
///
/// A row draws its label at the left and its box at the right, so the box's own pixels are the first text the
/// frame painted after the label and on the same line. That is the point an operator presses, and taking it
/// from the frame rather than from the column's padding means a test of `S9` is a test of the box, not of the
/// distance a row happens to be indented by.
///
/// The **last** run carrying the word is the modal's, because the modal is a layer over the panel behind it and
/// is painted after it: a panel that grows a row called `Feed` cannot move the point a test presses.
pub(crate) fn box_point(runs: &[Run], label: &str) -> egui::Pos2 {
  let index = runs
    .iter()
    .rposition(|(text, _, _)| text == label)
    .unwrap_or_else(|| {
      panic!(
        "the modal never painted the row {label:?}: {:?}",
        runs
          .iter()
          .map(|(text, _, _)| text.as_str())
          .collect::<Vec<_>>()
      )
    });
  let (_, at, size) = &runs[index];
  runs
    .iter()
    .skip(index + 1)
    .find(|(text, point, _)| {
      !text.is_empty() && point.x > at.x + size.x && (point.y - at.y).abs() < 12.0
    })
    .map(|(_, point, size)| egui::pos2(point.x + 2.0, point.y + size.y / 2.0))
    .unwrap_or_else(|| panic!("the row {label:?} drew no box to press"))
}

/// Type `text` into the box on the row labelled `label`: press the box, select what it holds, then send the
/// word.
///
/// Mirrors the strategy editor's `type_into_name`, because a box that is only reported as a number is not yet
/// a box: this presses the point the frame drew and sends the keystrokes an operator would, so the draft the
/// next frame reports is one a keyboard really reached.
pub(crate) fn type_into_box(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
  runs: &[Run],
  label: &str,
  text: &str,
) -> (ShellOutput, Vec<String>) {
  let at = box_point(runs, label);
  let _ = click_at(ctx, shell, app, at);
  let mut input = raw_input();
  input.events.push(egui::Event::Key {
    key: egui::Key::A,
    physical_key: None,
    pressed: true,
    repeat: false,
    modifiers: egui::Modifiers::COMMAND,
  });
  input.events.push(egui::Event::Text(text.to_owned()));
  let (output, texts, _) = pass(ctx, shell, app, input);
  (output, texts)
}

// ---------------------------------------------------------------------------------------------
// Readings
// ---------------------------------------------------------------------------------------------

/// The fields the form drew under one group, by the group's word.
pub(crate) fn fields_of(drawn: &ShellOutput, group: &str) -> Vec<String> {
  drawn
    .drawn
    .tool_groups
    .iter()
    .find(|(label, _)| label == group)
    .map(|(_, fields)| fields.iter().map(|(label, _)| label.clone()).collect())
    .unwrap_or_else(|| panic!("the form drew no {group} group: {:?}", group_labels(drawn)))
}

/// The value the form drew for one labelled row of one group.
pub(crate) fn value_of(drawn: &ShellOutput, group: &str, field: &str) -> String {
  drawn
    .drawn
    .tool_groups
    .iter()
    .find(|(label, _)| label == group)
    .and_then(|(_, fields)| {
      fields
        .iter()
        .find(|(label, _)| label == field)
        .map(|(_, value)| value.clone())
    })
    .unwrap_or_else(|| {
      panic!(
        "the {group} group drew no {field:?} row: {:?}",
        drawn.drawn.tool_groups
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

/// The words of the controls the modal reports as pressable, in the order it drew them.
pub(crate) fn hits(drawn: &ShellOutput) -> Vec<&str> {
  drawn
    .drawn
    .tool_library_hits
    .iter()
    .map(|(word, _)| word.as_str())
    .collect()
}

/// Where one pressable word sits in the order the modal drew it.
pub(crate) fn hit_position(drawn: &ShellOutput, word: &str) -> Option<usize> {
  hits(drawn).iter().position(|known| *known == word)
}

pub(crate) fn joined(texts: &[String]) -> String {
  texts.join(" | ")
}
