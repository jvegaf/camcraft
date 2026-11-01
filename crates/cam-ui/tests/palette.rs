//! The palette every stage draws, measured once and then pinned.
//!
//! T8.2 made the palette derive from [`cam_ui::help::Command`], which is right — one source for labels, groups and
//! order — but a test that derives its expectation from the same table cannot notice the table changing. These
//! goldens were measured from the shell before the rewrite and after it, stage by stage, and are written down here so
//! that a future edit to `Command::ALL`, to `in_palette()` or to `Stage::runs()` has to come with a deliberate
//! change to this file.

use cam_app::AppState;
use cam_geom::{Contour, Point2};
use cam_model::{Document, Entity, EntityKind, Job, Layer};
use cam_ui::{Shell, ShellOutput, Stage};

/// A document with one closed rectangle, enough for the shell to draw a palette over something.
fn document_with_rectangle() -> Document {
  let mut document = Document::new(Job::default_sheet());
  document.layers.push(Layer {
    id: "L1".to_owned(),
    name: "Part".to_owned(),
    visible: true,
    entities: vec![Entity {
      id: "E1".to_owned(),
      kind: EntityKind::Contour {
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

/// The chips the palette reported on one stage.
fn palette_of(stage: Stage) -> Vec<String> {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.set_stage(stage);
  let mut captured: Option<ShellOutput> = None;
  let mut input = egui::RawInput {
    screen_rect: Some(egui::Rect::from_min_size(
      egui::Pos2::ZERO,
      egui::vec2(1200.0, 800.0),
    )),
    ..Default::default()
  };
  // A stage switch only reaches the widgets on the frame after it, so the shell runs twice.
  for _ in 0..2 {
    captured = None;
    let full = ctx.run_pass(input.clone(), |ctx| {
      captured = Some(shell.show(&mut app, ctx));
    });
    full.drop_without_applying_deltas();
    input.events.clear();
  }
  captured.expect("the shell ran").drawn.palette
}

fn words(words: &[&str]) -> Vec<String> {
  words.iter().map(|word| (*word).to_owned()).collect()
}

#[test]
fn every_stage_draws_its_goldens_in_order() {
  let expected = [
    (
      Stage::Design,
      words(&[
        "Select",
        "Line",
        "Rectangle",
        "Polyline",
        "Arc",
        "Circle",
        "Ellipse",
        "Polygon",
        "Star",
        "Add text",
        "Grid",
        "Layers",
        "Help",
        "Shortcuts",
      ]),
    ),
    (
      Stage::Edit,
      words(&[
        "Move",
        "Rotate",
        "Scale",
        "Mirror",
        "Array",
        "Offset",
        "Join",
        "Grid",
        "Layers",
        "Help",
        "Shortcuts",
      ]),
    ),
    (
      Stage::Relief,
      words(&["Grid", "Layers", "Help", "Shortcuts"]),
    ),
    (
      Stage::Toolpaths,
      words(&["Grid", "Layers", "Help", "Shortcuts"]),
    ),
    (
      Stage::Prove,
      words(&["Grid", "Layers", "Help", "Shortcuts"]),
    ),
  ];
  for (stage, chips) in expected {
    assert_eq!(palette_of(stage), chips, "the {stage:?} palette");
  }
}

#[test]
fn no_stage_offers_a_history_chip() {
  for stage in [
    Stage::Design,
    Stage::Edit,
    Stage::Relief,
    Stage::Toolpaths,
    Stage::Prove,
  ] {
    let palette = palette_of(stage);
    assert!(
      !palette.iter().any(|chip| chip == "Undo" || chip == "Redo"),
      "history belongs to the app bar, not to the palette: {stage:?} drew {palette:?}"
    );
  }
}
