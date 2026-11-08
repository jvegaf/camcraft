//! The current-tool card: the tool a toolpath cuts with, read from a library snapshot (camui-toolpath
//! T1, specs S2/S3/S4).
//!
//! The card is drawn in the inspector and reported in [`cam_ui::DrawnState::tool_card`], for the reason
//! every other mirror in this crate exists: egui cannot be asked for a widget by its text from outside a
//! frame, so the shell reports the words it drew and the test quotes them back.

use cam_app::AppState;
use cam_geom::{Contour, Point2};
use cam_model::{Document, Entity, EntityKind, Job, Layer, ProfileParams, Toolpath};
use cam_store::tools::{CuttingData, StoredTool, ToolGeometry};
use cam_ui::view3d::{clock_text, seconds_for};
use cam_ui::{Shell, ShellOutput, Tab};
use cam_units::{Angle, Length, UnitSystem};

/// A document with one rectangle and one profile toolpath that cuts with `tool_number`.
fn document_with_toolpath(tool_number: u32) -> Document {
  document_with_toolpaths(tool_number, 1)
}

/// The same document with `count` profile toolpaths, all cutting with `tool_number`.
fn document_with_toolpaths(tool_number: u32, count: usize) -> Document {
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
  for index in 0..count {
    document.toolpaths.push(Toolpath {
      id: format!("P{}", index + 1),
      name: format!("Perfil {:02}", index + 1),
      entity_ids: vec!["E1".to_owned()],
      island_ids: Vec::new(),
      tool_number,
      operation: cam_model::Operation::Profile(ProfileParams::default()),
    });
  }
  document
}

/// One tool as a library snapshot holds it: geometry joined to its cutting data.
fn stored_tool(
  id: &str,
  number: u32,
  diameter: Option<f64>,
  included_angle: Option<f64>,
  num_flutes: Option<u32>,
) -> StoredTool {
  StoredTool {
    geometry: ToolGeometry {
      id: id.to_owned(),
      name_format: "{Tool Type} ({Diameter|F}{Units Short})".to_owned(),
      tool_type: 0,
      units: UnitSystem::Mm,
      diameter: diameter.map(Length::from_mm),
      included_angle: included_angle.map(Angle::from_degrees),
      flat_diameter: None,
      num_flutes,
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
      tool_number: Some(number),
      notes: None,
    },
  }
}

/// Run one frame and report what the shell displayed and painted.
fn toolpaths_frame(shell: &mut Shell, app: &mut AppState) -> (ShellOutput, Vec<String>) {
  let ctx = egui::Context::default();
  // Two passes: egui lays a panel out from the space it ended the previous pass with, so the first frame
  // is the one that measures and the second is the one worth reading.
  let mut last = None;
  for _ in 0..2 {
    let input = egui::RawInput {
      screen_rect: Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(1200.0, 800.0),
      )),
      ..Default::default()
    };
    let mut captured: Option<ShellOutput> = None;
    let full = ctx.run_pass(input, |ctx| {
      captured = Some(shell.show(app, ctx));
    });
    let painted = full
      .shapes
      .iter()
      .filter_map(|clipped| match &clipped.shape {
        egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
        _ => None,
      })
      .collect();
    full.drop_without_applying_deltas();
    last = Some((captured.expect("the shell ran"), painted));
  }
  last.expect("the shell ran")
}

/// One frame of the Toolpaths stage, as the card alone reports itself.
fn card_of(shell: &mut Shell, app: &mut AppState) -> Vec<String> {
  toolpaths_frame(shell, app).0.drawn.tool_card
}

#[test]
fn the_card_names_the_tool_the_toolpath_cuts_with() {
  let mut app = AppState::new(document_with_toolpath(4));
  let mut shell = Shell::new();
  shell.set_tools(vec![stored_tool("G4", 4, Some(12.7), Some(90.0), Some(2))]);

  // The frame's own two lines (S2): the number and the name, then the type with the shape's numbers.
  // Every field the card promises is in them, and nothing the frame does not show — no material, no
  // machine, no feed, plunge or spindle rate — is.
  assert_eq!(
    card_of(&mut shell, &mut app),
    vec![
      "TOOL — Perfil 01".to_owned(),
      "T4 - Tool (12.7mm)".to_owned(),
      "code 0 · Ø 12.70 mm · 90.0° · 2 flutes".to_owned(),
    ],
  );
}

#[test]
fn a_number_the_snapshot_does_not_hold_says_so_and_names_the_number() {
  let mut app = AppState::new(document_with_toolpath(7));
  let mut shell = Shell::new();
  // The snapshot holds T4, and the document cuts with T7 — the state that would otherwise be a blank card.
  shell.set_tools(vec![stored_tool("G4", 4, Some(12.7), Some(90.0), Some(2))]);

  let (output, _) = toolpaths_frame(&mut shell, &mut app);

  assert_eq!(
    output.drawn.tool_card,
    vec![
      "TOOL — Perfil 01".to_owned(),
      "T7 is not in the tool library this window was given, so this toolpath has no tool to show."
        .to_owned(),
    ],
  );
  // No tool, no program, no time: the estimate says why instead of printing `0:00` beside a card that
  // has just said the tool is missing.
  assert!(
    output.drawn.cutting_time.contains("T7") && !output.drawn.cutting_time.contains(':'),
    "the estimate must name the missing tool rather than measure nothing: {:?}",
    output.drawn.cutting_time
  );
}

#[test]
fn a_tool_the_library_only_half_describes_shows_a_dash_for_what_it_lacks() {
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  // An end mill: a diameter and two flutes, and no included angle at all.
  shell.set_tools(vec![stored_tool("G2", 2, Some(6.0), None, Some(2))]);

  assert_eq!(
    card_of(&mut shell, &mut app),
    vec![
      "TOOL — Perfil 01".to_owned(),
      "T2 - Tool (6mm)".to_owned(),
      "code 0 · Ø 6.00 mm · — · 2 flutes".to_owned(),
    ],
    "a number the library does not hold is a dash, never a zero the tool does not have"
  );
}

#[test]
fn a_document_with_no_toolpath_draws_no_card() {
  let mut document = document_with_toolpath(4);
  document.toolpaths.clear();
  let mut app = AppState::new(document);
  let mut shell = Shell::new();
  shell.set_tools(vec![stored_tool("G4", 4, Some(12.7), Some(90.0), Some(2))]);

  let card = card_of(&mut shell, &mut app);

  assert!(
    card.is_empty(),
    "a card for a toolpath the document does not hold would be describing nothing: {card:?}"
  );
}

#[test]
fn the_estimate_beside_the_card_is_the_documents_own_cutting_time() {
  let mut app = AppState::new(document_with_toolpath(4));
  let mut shell = Shell::new();
  let tools = vec![stored_tool("G4", 4, Some(12.7), Some(90.0), Some(2))];
  shell.set_tools(tools.clone());

  let (output, _) = toolpaths_frame(&mut shell, &mut app);

  // The card is exactly the three lines S2 allows: the estimate did not become a fourth field.
  assert_eq!(
    output.drawn.tool_card.len(),
    3,
    "{:?}",
    output.drawn.tool_card
  );

  // The time is this document's own program — the shell builds it from the document's toolpaths and
  // the snapshot — and it is written by the playback clock's formatter, not by one of its own.
  let program = cam_program::program_for_snapshot(app.document(), &tools, None).expect("a program");
  assert_eq!(
    output.drawn.cutting_time,
    format!(
      "Estimated cutting time · {}",
      clock_text(seconds_for(&program))
    )
  );
  // And not an empty program wearing a clock: the fixture's own profile is 13 whole seconds of
  // cutting at 100 mm/s, written the way the playback bar writes a moment.
  assert_eq!(output.drawn.cutting_time, "Estimated cutting time · 0:13");
}

#[test]
fn the_estimate_moves_when_the_document_does() {
  // A second toolpath of the same shape is more path to cut, so the estimate grows: this is what makes
  // it the document's time rather than a constant drawn beside the card.
  let mut app = AppState::new(document_with_toolpaths(4, 2));
  let mut shell = Shell::new();
  shell.set_tools(vec![stored_tool("G4", 4, Some(12.7), Some(90.0), Some(2))]);
  let (two, _) = toolpaths_frame(&mut shell, &mut app);

  let mut single = AppState::new(document_with_toolpaths(4, 1));
  let mut other_shell = Shell::new();
  other_shell.set_tools(vec![stored_tool("G4", 4, Some(12.7), Some(90.0), Some(2))]);
  let (one, _) = toolpaths_frame(&mut other_shell, &mut single);

  assert_ne!(
    two.drawn.cutting_time, one.drawn.cutting_time,
    "two passes take longer than one: {:?} vs {:?}",
    two.drawn.cutting_time, one.drawn.cutting_time
  );
}

#[test]
fn the_job_tab_still_reports_the_job_it_reported_before_the_card() {
  // The card shares the inspector with the Job tab. Adding it must not take the numbers that tab already
  // showed away from the operator, and since camui-prancheta T1 the tab is asked for rather than implied
  // by a stage.
  let mut app = AppState::new(document_with_toolpath(4));
  let mut shell = Shell::new();
  shell.set_tab(Tab::Job);

  let (_, painted) = toolpaths_frame(&mut shell, &mut app);

  for word in ["MATERIAL", "DOCUMENT", "Width X", "Toolpaths"] {
    assert!(
      painted.iter().any(|text| text == word),
      "the Job tab must still draw {word:?}: {painted:?}"
    );
  }
}
