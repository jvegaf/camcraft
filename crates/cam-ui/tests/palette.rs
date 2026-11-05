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
  palette_output().drawn.palette
}

/// One frame's report, kept whole: the rail's chips, its headings and the app bar's own report are the
/// three surfaces the FILE move touched, and a test that read only the chips could not see the other two.
fn palette_output() -> ShellOutput {
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
  captured.expect("the shell ran")
}

fn words(words: &[&str]) -> Vec<String> {
  words.iter().map(|word| (*word).to_owned()).collect()
}

#[test]
fn the_palette_draws_its_golden_in_order() {
  assert_eq!(
    palette(),
    words(&[
      "Open",
      "Save",
      "Save as",
      "Fit the job",
      "Undo",
      "Redo",
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
      "Offset",
      "Join",
      "Move",
      "Rotate",
      "Scale",
      "Mirror",
      "Array",
      "Grid",
      "Layers",
      "Help",
      "Shortcuts",
      "Script",
    ]),
    "the palette must be the table's own run, in its order, FILE first"
  );
}

#[test]
fn the_rail_headings_are_the_references_five_in_its_order() {
  let headings: Vec<String> = palette_output()
    .drawn
    .palette_headings
    .into_iter()
    .map(|(label, _)| label)
    .collect();
  assert_eq!(
    headings,
    words(&["FILE", "DRAW", "EDIT", "ORGANISE", "SHOW AND SELECT"]),
    "the rail's headings are S4's five groups, in S4's order"
  );
}

#[test]
fn the_app_bar_offers_no_command_of_its_own() {
  // The FILE move is only correct if the command stopped being reachable twice. The bar's report is the
  // proof: a button that came back would have to come back here.
  let drawn = palette_output().drawn;
  assert!(
    drawn.app_bar.is_empty(),
    "the app bar must hold no table command: {:?}",
    drawn.app_bar
  );
}

#[test]
fn folding_a_group_hides_its_chips_and_keeps_the_rest() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let mut input = egui::RawInput {
    screen_rect: Some(egui::Rect::from_min_size(
      egui::Pos2::ZERO,
      egui::vec2(1200.0, 800.0),
    )),
    ..Default::default()
  };
  let mut run = |shell: &mut Shell, app: &mut AppState, events: Vec<egui::Event>| {
    input.events = events;
    let mut captured: Option<ShellOutput> = None;
    let full = ctx.run_pass(input.clone(), |ctx| {
      captured = Some(shell.show(app, ctx));
    });
    full.drop_without_applying_deltas();
    input.events.clear();
    captured.expect("the shell ran")
  };

  // Settle the layout first: egui lays a panel out from the space the previous pass ended with.
  let _ = run(&mut shell, &mut app, Vec::new());
  let settled = run(&mut shell, &mut app, Vec::new());
  let (x, y) = settled
    .drawn
    .palette_headings
    .iter()
    .find(|(label, _)| label == "FILE")
    .map(|(_, point)| *point)
    .expect("FILE heads the rail");

  // A click is two frames: the press, then the release. One pass carrying both is a drag egui never
  // resolves into a click — which is why the shell's own `click_at` helper is written the same way.
  for pressed in [true, false] {
    let _ = run(
      &mut shell,
      &mut app,
      vec![
        egui::Event::PointerMoved(egui::pos2(x, y)),
        egui::Event::PointerButton {
          pos: egui::pos2(x, y),
          button: egui::PointerButton::Primary,
          pressed,
          modifiers: egui::Modifiers::default(),
        },
      ],
    );
  }
  let folded = run(&mut shell, &mut app, Vec::new());

  assert!(
    !folded.drawn.palette.iter().any(|chip| chip == "Open"),
    "FILE's chips must be gone while it is folded: {:?}",
    folded.drawn.palette
  );
  assert!(
    folded.drawn.palette.iter().any(|chip| chip == "Select"),
    "folding one group must not fold another: {:?}",
    folded.drawn.palette
  );
  assert!(
    folded
      .drawn
      .palette_headings
      .iter()
      .any(|(label, _)| label == "FILE"),
    "a folded group keeps its name: {:?}",
    folded.drawn.palette_headings
  );
  assert!(!shell.group_open(cam_ui::help::Group::File));
}
