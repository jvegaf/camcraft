//! The palette the shell draws, measured once and then pinned.
//!
//! T8.2 made the palette derive from [`cam_ui::help::Command`], which is right — one source for labels, groups and
//! order — but a test that derives its expectation from the same table cannot notice the table changing. These
//! goldens were measured from the shell before the rewrite and after it, and are written down here so that a future
//! edit to `Command::ALL` or to `in_palette()` has to come with a deliberate change to this file.
//!
//! One palette, not five. Until camui-prancheta T1 the shell drew a different rail per stage and this file pinned
//! each one; the reference app has no stage bar, so the rail is the whole table and there is one golden to pin.

use cam_app::AppState;
use cam_geom::{Contour, Point2};
use cam_model::{Document, Entity, EntityKind, Job, Layer};
use cam_ui::{Shell, ShellOutput};

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

/// The chips the palette reported.
fn palette() -> Vec<String> {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let mut captured: Option<ShellOutput> = None;
  let mut input = egui::RawInput {
    screen_rect: Some(egui::Rect::from_min_size(
      egui::Pos2::ZERO,
      egui::vec2(1200.0, 800.0),
    )),
    ..Default::default()
  };
  // egui lays a panel out from the space the previous pass ended with, so the shell runs twice and the
  // second pass is the one worth reading.
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
fn the_palette_draws_its_golden_in_order() {
  assert_eq!(
    palette(),
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
    "the palette must be the table's own run, in its order"
  );
}

#[test]
fn the_palette_offers_no_history_chip() {
  let palette = palette();
  assert!(
    !palette.iter().any(|chip| chip == "Undo" || chip == "Redo"),
    "history belongs to the app bar, not to the palette: the palette drew {palette:?}"
  );
}
