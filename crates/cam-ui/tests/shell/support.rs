use cam_app::AppState;
use cam_geom::{Contour, Point2};
use cam_model::{
  Document, IsolationKind, IsolationParams, Job, Layer, LayerRole, MillingDirection, Operation,
  Toolpath,
};
use cam_store::document::{save_document, DocumentFile};
use cam_store::tools::{CuttingData, StoredTool, ToolGeometry};
use cam_ui::{Shell, ShellOutput, Tab};
use cam_units::{Length, UnitSystem};

pub(crate) fn document_with_rectangle() -> Document {
  let mut document = Document::new(Job::default_sheet());
  document.layers.push(Layer {
    id: "L1".to_owned(),
    name: "Part".to_owned(),
    role: LayerRole::Geometry,
    visible: true,
    entities: vec![cam_model::Entity {
      id: "E1".to_owned(),
      kind: cam_model::EntityKind::Contour {
        contour: Contour::from_points(
          &[
            Point2::new(0.0, 0.0),
            Point2::new(100.0, 0.0),
            Point2::new(100.0, 60.0),
            Point2::new(0.0, 60.0),
          ],
          true,
        )
        .expect("rectangle"),
      },
    }],
  });
  document
}

pub(crate) fn raw_input() -> egui::RawInput {
  egui::RawInput {
    screen_rect: Some(egui::Rect::from_min_size(
      egui::Pos2::ZERO,
      egui::vec2(1200.0, 800.0),
    )),
    ..Default::default()
  }
}

/// Run one frame and return what the shell produced plus the text it painted.
pub(crate) fn frame_with(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
) -> (ShellOutput, Vec<String>) {
  frame_with_events(ctx, shell, app, Vec::new())
}

/// Run one frame with explicit events, and return what the shell produced plus its painted text.
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
  // The texture deltas belong to a renderer we do not have in a headless test; dropping them
  // silently is exactly what epaint forbids, so say so explicitly.
  full.drop_without_applying_deltas();
  (captured.expect("the shell ran"), texts)
}

/// Press one key and report the frame the press was read in.
///
/// A press is two frames — down then up — the way a keyboard reports it. The second frame is what
/// makes the next press a press instead of a held key, and it is dropped: what a test asserts on is
/// the frame the shell acted in.
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

pub(crate) fn frame(shell: &mut Shell, app: &mut AppState) -> (ShellOutput, Vec<String>) {
  let ctx = egui::Context::default();
  frame_with(&ctx, shell, app)
}

/// Press the pointer at one window point: move there, press, then release.
///
/// A click is two frames, the way a mouse reports it, and the frame that reports the click is the one
/// the release lands in. The move is repeated in both passes because egui decides what is under the
/// pointer from the input it ended the previous pass with.
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

/// Click one status-bar chip by the beginning of its word.
///
/// The bar reports the point it drew each chip at ([`cam_ui::DrawnState::status_chips`]) for the same reason
/// the palette does: egui cannot be asked for a widget by its text from outside a frame. A prefix instead of
/// the whole word because the grid chip names the step it snaps to as well (`Grid 10`), which is the change
/// under test here.
pub(crate) fn click_status_chip(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
  prefix: &str,
) -> String {
  let (output, _) = frame_with(ctx, shell, app);
  let (label, point) = output
    .drawn
    .status_chips
    .iter()
    .find(|(label, _)| label.starts_with(prefix))
    .map(|(label, point)| (label.clone(), egui::pos2(point.0, point.1)))
    .unwrap_or_else(|| panic!("no {prefix:?} chip: {:?}", output.drawn.status_chips));
  click_at(ctx, shell, app, point);
  label
}

/// Click one palette chip by the word on it.
///
/// The palette reports the point it drew each chip at ([`cam_ui::DrawnState::palette_hits`]), because
/// egui cannot be asked for a widget by its text from outside a frame and a test that guessed at the
/// layout would break the day a margin moved.
pub(crate) fn click_palette_chip(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
  label: &str,
) -> (ShellOutput, Vec<String>) {
  let (output, _) = frame_with(ctx, shell, app);
  let at = output
    .drawn
    .palette_hits
    .iter()
    .find(|(word, _)| word == label)
    .map(|(_, point)| egui::pos2(point.0, point.1))
    .unwrap_or_else(|| panic!("no {label:?} chip: {:?}", output.drawn.palette));
  click_at(ctx, shell, app, at)
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

/// A window tall enough to lay the whole Tools tab out, so every one of its buttons is pressable.
///
/// The inspector's body is a scroll area capped at 55 % of the window height, so at the standard 800 px the
/// lower half of the tab falls outside the clip rect and a click at a reported point lands on nothing above
/// it. Reporting a point is only useful while the widget it came from is still on screen.
///
/// authoring body, so the tab is taller than it was and the window has to keep the promise this helper makes
/// instead of the promise it used to be able to keep at 2 000 px.
pub(crate) fn tall_input() -> egui::RawInput {
  let mut input = raw_input();
  input.screen_rect = Some(egui::Rect::from_min_size(
    egui::Pos2::ZERO,
    egui::vec2(1200.0, 2600.0),
  ));
  input
}

/// Run one frame of the tall window, with events, and report what the shell produced.
pub(crate) fn tall_frame(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
  events: Vec<egui::Event>,
) -> ShellOutput {
  let mut captured: Option<ShellOutput> = None;
  let mut input = tall_input();
  input.events = events;
  let full = ctx.run_pass(input, |ctx| {
    captured = Some(shell.show(app, ctx));
  });
  full.drop_without_applying_deltas();
  captured.expect("the shell ran")
}

/// Click one Tools-tab button by the word on it.
///
/// The tab reports where it drew each parameterised command's button ([`cam_ui::DrawnState::authoring_hits`])
/// for the same reason the palette does: egui cannot be asked for a widget by its text from outside a frame.
pub(crate) fn click_authoring_button(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
  label: &str,
) -> ShellOutput {
  let output = tall_frame(ctx, shell, app, Vec::new());
  let at = output
    .drawn
    .authoring_hits
    .iter()
    .find(|(word, _)| word == label)
    .map(|(_, point)| egui::pos2(point.0, point.1))
    .unwrap_or_else(|| panic!("no {label:?} button in the Tools tab"));
  let mut last = None;
  for pressed in [true, false] {
    // The move is repeated in both passes because egui decides what is under the pointer from the input it
    // ended the previous pass with.
    last = Some(tall_frame(
      ctx,
      shell,
      app,
      vec![
        egui::Event::PointerMoved(at),
        egui::Event::PointerButton {
          pos: at,
          button: egui::PointerButton::Primary,
          pressed,
          modifiers: egui::Modifiers::default(),
        },
      ],
    ));
  }
  last.expect("a click is two frames")
}

/// Read a scrollable panel all the way down.
///
/// egui paints only what falls inside a scroll area's clip rect, so one frame shows the top of a long
/// tab and nothing below it. This walks the pointer into the panel and turns the wheel until the
/// painted text stops growing — which is what an operator does to read the same list.
pub(crate) fn texts_scrolled_to_the_end(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
  at: egui::Pos2,
) -> Vec<String> {
  let mut seen: Vec<String> = Vec::new();
  let mut input = raw_input();
  input.events.push(egui::Event::PointerMoved(at));
  for _ in 0..16 {
    input.events.push(egui::Event::MouseWheel {
      unit: egui::MouseWheelUnit::Point,
      delta: egui::vec2(0.0, -160.0),
      phase: egui::TouchPhase::Move,
      source: egui::MouseWheelSource::Unknown,
      modifiers: egui::Modifiers::default(),
    });
    let mut captured: Option<ShellOutput> = None;
    let full = ctx.run_pass(input.clone(), |ctx| {
      captured = Some(shell.show(app, ctx));
    });
    let texts = painted_text(&full);
    full.drop_without_applying_deltas();
    let before = seen.len();
    for text in texts {
      if !seen.contains(&text) {
        seen.push(text);
      }
    }
    let actions = captured.expect("the shell ran").actions;
    for action in actions {
      shell.apply_action(app, &action);
    }
    if seen.len() == before {
      break;
    }
  }
  seen
}

pub(crate) fn joined(texts: &[String]) -> String {
  texts.join(" | ")
}

pub(crate) fn scratch(name: &str) -> std::path::PathBuf {
  let directory = std::env::temp_dir().join(format!("campilot-ui-{}-{name}", std::process::id()));
  let _ = std::fs::remove_dir_all(&directory);
  std::fs::create_dir_all(&directory).expect("scratch directory");
  directory
}

/// Write a real `.c2d` with the store's own writer, so the shell test opens a file and not a stub.
pub(crate) fn write_c2d(
  directory: &std::path::Path,
  name: &str,
  document: &Document,
) -> std::path::PathBuf {
  let path = directory.join(name);
  save_document(&path, document).expect("document written");
  path
}

/// A document holding `count` closed 120 x 90 mm squares in a row, so a test can tell one file's drawing
/// from another's by how much of the sheet it covers.
pub(crate) fn document_with_squares(count: usize) -> Document {
  let entities = (0..count)
    .map(|index| {
      let x = index as f64 * 150.0;
      cam_model::Entity {
        id: format!("S{index}"),
        kind: cam_model::EntityKind::Contour {
          contour: Contour::from_points(
            &[
              Point2::new(x, 0.0),
              Point2::new(x + 120.0, 0.0),
              Point2::new(x + 120.0, 90.0),
              Point2::new(x, 90.0),
            ],
            true,
          )
          .expect("square"),
        },
      }
    })
    .collect();
  let mut document = Document::new(Job::default_sheet());
  document.layers.push(Layer {
    id: "L1".to_owned(),
    name: "Part".to_owned(),
    role: LayerRole::Geometry,
    visible: true,
    entities,
  });
  document
}

/// The same drawing plus one profile toolpath cutting with `tool_number`.
///
/// The toolpath is what makes the current-tool card report something, which is how a test reads the tool
/// snapshot the shell was handed without reaching into the shell's own fields.
pub(crate) fn document_with_toolpath(count: usize, tool_number: u32) -> Document {
  let mut document = document_with_squares(count);
  document.toolpaths.push(Toolpath {
    id: "P1".to_owned(),
    name: "Profile 01".to_owned(),
    entity_ids: vec!["S0".to_owned()],
    island_ids: Vec::new(),
    tool_number,
    operation: Operation::Isolation(IsolationParams {
      passes: 1,
      overlap: 0.5,
      direction: MillingDirection::Climb,
      kind: IsolationKind::Full,
      cut_depth: Length::from_mm(0.1),
      keep_pours: false,
    }),
  });
  document
}

/// One tool as a library snapshot holds it, so the card can resolve `tool_number` with it.
pub(crate) fn stored_tool(tool_number: u32) -> StoredTool {
  StoredTool {
    geometry: ToolGeometry {
      id: format!("G{tool_number}"),
      name_format: "{Tool Type} ({Diameter|F}{Units Short})".to_owned(),
      tool_type: 0,
      units: UnitSystem::Mm,
      diameter: Some(Length::from_mm(6.0)),
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
      id: format!("G{tool_number}-cut"),
      rate_units: UnitSystem::Mm,
      feed_rate: Some(100.0),
      plunge_rate: Some(30.0),
      spindle_speed: Some(18_000),
      stepdown: None,
      stepover: None,
      clear_stepover: None,
      tool_number: Some(tool_number),
      notes: None,
    },
  }
}

/// Write a real `.c2d` that also holds one archive entry this build does not interpret.
///
/// The shape the store's own round-trip test builds (`crates/cam-store/tests/document.rs`): a document
/// written by the store, then given an extra entry and saved back through [`DocumentFile`], which is what
/// a newer build adding an asset to the format would produce.
pub(crate) fn write_c2d_with_extras(
  directory: &std::path::Path,
  name: &str,
  document: &Document,
) -> std::path::PathBuf {
  let path = write_c2d(directory, name, document);
  let mut file = DocumentFile::load(&path).expect("the document loads again");
  file
    .extras
    .insert("assets/logo.png".to_owned(), vec![1, 2, 3, 4]);
  file.save(&path).expect("the archive with its entry");
  path
}

/// The names in `directory`, sorted, so a test can assert a directory is exactly as it found it.
pub(crate) fn listing(directory: &std::path::Path) -> Vec<String> {
  let mut names: Vec<String> = std::fs::read_dir(directory)
    .expect("the directory lists")
    .filter_map(|entry| entry.ok())
    .map(|entry| entry.file_name().to_string_lossy().into_owned())
    .collect();
  names.sort();
  names
}

/// The word on the Job tab's opener, spelled the way the operator reads it.
pub(crate) const MATERIAL_OPENER: &str = "Define the material…";

/// Open the material dialog from the Job tab and settle it.
///
/// Two quiet frames after the press: an egui area positions itself on a sizing pass, so the points it
/// reports in the frame it opens are not the ones it settles at — the same reason the context menu needs
/// one quiet frame before its rows can be pressed.
pub(crate) fn open_material_dialog(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
) -> ShellOutput {
  shell.set_tab(Tab::Job);
  let (output, _) = frame_with(ctx, shell, app);
  let at = output
    .drawn
    .material_hits
    .iter()
    .find(|(word, _)| word == MATERIAL_OPENER)
    .map(|(_, point)| egui::pos2(point.0, point.1))
    .unwrap_or_else(|| {
      panic!(
        "the Job tab offers no material opener: {:?}",
        output.drawn.material_hits
      )
    });
  click_at(ctx, shell, app, at);
  let _ = frame_with(ctx, shell, app);
  frame_with(ctx, shell, app).0
}
