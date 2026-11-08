use super::support::*;
use cam_app::AppState;
use cam_ui::help::Command as HelpCommand;
use cam_ui::{Shell, Tool};

#[test]
fn the_shell_lays_out_a_canvas_between_the_panels() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let (output, _) = frame(&mut shell, &mut app);

  assert!(output.canvas.width > 0.0 && output.canvas.height > 0.0);
  assert!(
    output.canvas.width < 1200.0,
    "the side panels must take room from the canvas, got {}",
    output.canvas.width
  );
  // The camera follows the canvas, which is what makes picking and painting agree.
  assert_eq!(
    app.camera().viewport().width,
    output.canvas.width,
    "the camera viewport must match the canvas"
  );
}

#[test]
fn the_frame_draws_no_stage_bar_and_the_rail_offers_every_group_at_once() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let (output, texts) = frame(&mut shell, &mut app);
  let all = joined(&texts);
  // The three words only the strip ever drew. camui-prancheta S2: the reference has no stage bar, so
  // neither does this frame.
  for gone in ["Design", "Relief", "Prove"] {
    assert!(
      !all.contains(gone),
      "the stage bar is gone, so {gone:?} must not be drawn: {all}"
    );
  }
  // And the rail is the whole table: a drawing command and an edit command in the same frame, with no
  // switch in between.
  for expected in ["Line", "Offset", "Grid"] {
    assert!(
      output.drawn.palette.iter().any(|entry| entry == expected),
      "the rail must offer {expected:?} without a stage switch: {:?}",
      output.drawn.palette
    );
  }
}

#[test]
fn collapsing_the_inspector_widens_the_canvas_and_the_strip_opens_it_again() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let ctx = egui::Context::default();
  let (open, _) = frame_with(&ctx, &mut shell, &mut app);

  shell.set_inspector_open(false);
  let (closed, texts) = frame_with(&ctx, &mut shell, &mut app);
  assert!(
    !joined(&texts).contains("Inspector"),
    "the panel must be gone"
  );
  assert!(
    closed.canvas.width > open.canvas.width,
    "the canvas must take the width back: {} vs {}",
    closed.canvas.width,
    open.canvas.width
  );

  // Touching the strip brings it back: that is what makes collapsing usable rather than modal.
  let strip = egui::pos2(
    (closed.canvas.x + closed.canvas.width) as f32 + 6.0,
    (closed.canvas.y + 40.0) as f32,
  );
  // Two passes with the pointer there: egui decides what is hovered from the interaction state the
  // previous pass ended with, so one pass with a moved pointer reports hover on the next frame.
  for _ in 0..2 {
    let mut input = raw_input();
    input.events.push(egui::Event::PointerMoved(strip));
    let full = ctx.run_pass(input, |ctx| {
      shell.show(&mut app, ctx);
    });
    full.drop_without_applying_deltas();
  }
  assert!(shell.inspector_open(), "hovering the strip must reopen it");
}

#[test]
fn the_app_bar_offers_the_document_the_units_and_the_primary_action() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let (output, texts) = frame(&mut shell, &mut app);
  let all = joined(&texts);
  for label in ["campilot", "untitled.c2d", "mm", "Generate G-code"] {
    assert!(all.contains(label), "missing {label:?} in {all}");
  }
  // The bar's file command moved to the rail (camui-prancheta T2b, S4), and a bar that could not ask for a
  // document would leave the operator with a name and no way to change it — so the rail must offer it.
  assert!(
    output
      .drawn
      .palette
      .contains(&HelpCommand::Open.label().to_owned()),
    "the rail must offer Open: {:?}",
    output.drawn.palette
  );
  // The rail is where the gestures live now, and it says so in its own report rather than in pixels.
  for tool in Tool::ALL {
    assert!(
      output
        .drawn
        .palette
        .iter()
        .any(|entry| entry == tool.label()),
      "{:?} must be on the rail: {:?}",
      tool,
      output.drawn.palette
    );
  }
}

#[test]
fn the_rail_foot_names_the_product_and_says_the_work_is_unsaved() {
  // S4's brand block, on the state every shell starts in: a document with no file has nothing saved that a
  // stamp could name, and saying so is the honest answer rather than a blank line.
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let (output, texts) = frame(&mut shell, &mut app);
  let all = joined(&texts);
  for label in ["campilot", "Unsaved changes"] {
    assert!(all.contains(label), "missing {label:?} in {all}");
  }
  assert_eq!(
    shell.saved_stamp(),
    "Unsaved changes",
    "a document with no file has no last write to name"
  );
  assert!(
    all.contains(&format!("v{}", env!("CARGO_PKG_VERSION"))),
    "the block carries the crate version: {all}"
  );
  // The block is at the foot of the rail and the rail is a panel, not a canvas overlay: what it must never be
  // is a command, so none of the three lines is a chip the palette reports.
  for line in ["campilot", "Unsaved changes"] {
    assert!(
      !output.drawn.palette.iter().any(|chip| chip == line),
      "{line:?} must not be a rail command: {:?}",
      output.drawn.palette
    );
  }
}

#[test]
fn a_frame_without_input_asks_for_nothing() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let (output, _) = frame(&mut shell, &mut app);
  assert!(
    output.actions.is_empty(),
    "no input must produce no actions, got {:?}",
    output.actions
  );
}
