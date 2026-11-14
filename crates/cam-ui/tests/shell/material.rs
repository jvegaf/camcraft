use super::support::*;
use cam_app::AppState;
use cam_ui::{handle_action, Shell, ShellOutput, UiAction};

/// Press one thing the material path drew, by the word on it.
fn click_material(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
  drawn: &ShellOutput,
  word: &str,
) -> (ShellOutput, Vec<String>) {
  let at = drawn
    .drawn
    .material_hits
    .iter()
    .find(|(name, _)| name == word)
    .map(|(_, point)| egui::pos2(point.0, point.1))
    .unwrap_or_else(|| {
      panic!(
        "nothing called {word:?} in the material path: {:?}",
        drawn.drawn.material_hits
      )
    });
  click_at(ctx, shell, app, at)
}

#[test]
fn the_job_tab_opens_the_material_dialog_seeded_with_the_documents_job() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();

  let settled = open_material_dialog(&ctx, &mut shell, &mut app);

  let words = settled.drawn.material.join(" | ");
  for drawn in [
    "New job — define the material",
    "Flat sheet",
    "Rotary",
    "Width X",
    "Height Y",
    "Thickness Z",
  ] {
    assert!(
      words.contains(drawn),
      "the dialog never drew {drawn:?}: {words}"
    );
  }
  // The drafts are the document's own numbers, not a default of their own: opening the dialog on a
  // 1219 x 2438 x 18 job must offer to change that job, not to replace it with a sample.
  assert_eq!(shell.material_size, (1219.0, 2438.0, 18.0));
  for pressable in ["Rotary", "Cancel", "Continue"] {
    assert!(
      settled
        .drawn
        .material_hits
        .iter()
        .any(|(name, _)| name == pressable),
      "nothing to press called {pressable:?}: {:?}",
      settled.drawn.material_hits
    );
  }
  // The opener is behind the modal now, so it is no longer offered as a target.
  assert!(
    !settled
      .drawn
      .material_hits
      .iter()
      .any(|(name, _)| name == MATERIAL_OPENER),
    "the dialog is open and the tab's opener is still reported as pressable"
  );
}

#[test]
fn the_material_dialog_shows_the_three_numbers_it_writes() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let _ = open_material_dialog(&ctx, &mut shell, &mut app);

  shell.material_size = (600.0, 400.0, 18.0);
  let (output, texts) = frame_with(&ctx, &mut shell, &mut app);
  let words = output.drawn.material.join(" | ");
  let painted = joined(&texts);
  for value in ["600 mm", "400 mm", "18 mm"] {
    assert!(
      output.drawn.material.iter().any(|word| word == value),
      "the plan does not annotate {value:?}: {words}"
    );
    assert!(
      painted.contains(value),
      "no {value:?} reached the frame, so the number was reported and never drawn: {painted}"
    );
  }

  let (pressed, _) = click_material(&ctx, &mut shell, &mut app, &output, "Continue");
  assert_eq!(
    pressed.actions,
    vec![UiAction::SetJob {
      width: 600.0,
      height: 400.0,
      thickness: 18.0,
    }],
    "Continue must emit exactly the job it showed"
  );
  let report = handle_action(&mut app, &pressed.actions[0]).expect("the job applies");
  assert_eq!(report.label, "set job size");
  assert_eq!(app.document().board.width.mm(), 600.0);
  assert_eq!(app.document().board.height.mm(), 400.0);
  assert_eq!(app.document().board.thickness.mm(), 18.0);

  // And the dialog is gone, so the next `Continue` cannot write twice.
  let (after, _) = frame_with(&ctx, &mut shell, &mut app);
  assert!(after.drawn.material.is_empty(), "the dialog stayed open");
}

#[test]
fn a_refused_measurement_names_the_number_and_leaves_the_job_alone() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let _ = open_material_dialog(&ctx, &mut shell, &mut app);

  let before = app.document().board.clone();
  shell.material_size = (0.0, 400.0, 18.0);
  let (output, texts) = frame_with(&ctx, &mut shell, &mut app);
  let sentence = output.drawn.material_refusal.clone();
  assert!(
    sentence.contains("Width X"),
    "the refusal must name the field: {sentence:?}"
  );
  assert!(
    sentence.contains("0 mm"),
    "the refusal must name the number: {sentence:?}"
  );
  assert!(
    joined(&texts).contains(&sentence),
    "the sentence was reported but never drawn: {}",
    joined(&texts)
  );

  let (pressed, _) = click_material(&ctx, &mut shell, &mut app, &output, "Continue");
  assert!(
    pressed.actions.is_empty(),
    "a zero width was written through: {:?}",
    pressed.actions
  );
  assert_eq!(
    app.document().board,
    before,
    "a refused measurement changed the job"
  );
  // The dialog is still open with the sentence still on it, which is what makes the refusal fixable
  // instead of a status line that disappears.
  let (after, _) = frame_with(&ctx, &mut shell, &mut app);
  assert!(!after.drawn.material.is_empty(), "the dialog closed itself");
  assert_eq!(after.drawn.material_refusal, sentence);
}

#[test]
fn the_rotary_card_says_why_it_cannot_be_chosen_and_writes_nothing() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let settled = open_material_dialog(&ctx, &mut shell, &mut app);

  let reason = settled
    .drawn
    .material
    .iter()
    .find(|word| word.contains("no axis to turn"))
    .unwrap_or_else(|| {
      panic!(
        "the rotary card does not say why it is dark: {:?}",
        settled.drawn.material
      )
    })
    .clone();
  assert!(
    reason.contains("flat sheet"),
    "the reason must name what this document can hold: {reason:?}"
  );

  let before = app.document().board.clone();
  let (pressed, _) = click_material(&ctx, &mut shell, &mut app, &settled, "Rotary");
  assert!(
    pressed.actions.is_empty(),
    "the rotary card wrote something: {:?}",
    pressed.actions
  );
  assert_eq!(app.document().board, before);
  let (after, _) = frame_with(&ctx, &mut shell, &mut app);
  assert!(
    !after.drawn.material.is_empty(),
    "a card that cannot be chosen closed the dialog"
  );
}

#[test]
fn the_plans_annotations_follow_the_numbers_as_they_are_edited() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let settled = open_material_dialog(&ctx, &mut shell, &mut app);
  assert!(
    settled.drawn.material.iter().any(|word| word == "1219 mm"),
    "the plan does not annotate the document's own width: {:?}",
    settled.drawn.material
  );

  shell.material_size = (1000.0, 800.0, 25.0);
  let (output, texts) = frame_with(&ctx, &mut shell, &mut app);
  let painted = joined(&texts);
  for value in ["1000 mm", "800 mm", "25 mm"] {
    assert!(
      output.drawn.material.iter().any(|word| word == value),
      "the plan kept its old annotation instead of {value:?}: {:?}",
      output.drawn.material
    );
    assert!(painted.contains(value), "no {value:?} was drawn: {painted}");
  }
  assert!(
    !output.drawn.material.iter().any(|word| word == "1219 mm"),
    "the plan still annotates the retired width: {:?}",
    output.drawn.material
  );
}

#[test]
fn the_plan_is_a_drawing_and_not_a_caption() {
  // The one thing about the annotated key plan a headless test can see: the stock is painted as three filled
  // faces rather than described in words. egui reports a filled quad as a closed, filled `PathShape`, and
  // nothing else in a frame of labels, buttons and panels draws a four-point one — the icons are strokes.
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let closed = count_filled_quads(&ctx, &mut shell, &mut app);
  let _ = open_material_dialog(&ctx, &mut shell, &mut app);
  let open = count_filled_quads(&ctx, &mut shell, &mut app);
  assert_eq!(
    closed, 0,
    "something else in the frame paints a filled quad"
  );
  assert_eq!(
    open, 3,
    "the plan must paint the stock's top and its two visible sides, and it painted {open}"
  );
}

/// How many closed, filled, four-point paths one frame painted.
fn count_filled_quads(ctx: &egui::Context, shell: &mut Shell, app: &mut AppState) -> usize {
  let mut captured: Option<ShellOutput> = None;
  let full = ctx.run_pass(raw_input(), |ctx| {
    captured = Some(shell.show(app, ctx));
  });
  let _ = captured;
  let quads = full
    .shapes
    .iter()
    .filter(|clipped| match &clipped.shape {
      egui::Shape::Path(path) => {
        path.closed && path.points.len() == 4 && path.fill != egui::Color32::TRANSPARENT
      }
      _ => false,
    })
    .count();
  full.drop_without_applying_deltas();
  quads
}

#[test]
fn cancelling_the_material_dialog_writes_nothing() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let settled = open_material_dialog(&ctx, &mut shell, &mut app);

  let before = app.document().board.clone();
  let (pressed, _) = click_material(&ctx, &mut shell, &mut app, &settled, "Cancel");
  assert!(
    pressed.actions.is_empty(),
    "Cancel wrote something: {:?}",
    pressed.actions
  );
  let (after, _) = frame_with(&ctx, &mut shell, &mut app);
  assert!(after.drawn.material.is_empty(), "Cancel left the dialog up");
  assert_eq!(app.document().board, before, "Cancel changed the job");
}

#[test]
fn a_set_job_action_with_a_zero_is_refused_by_the_shell_path_too() {
  // No RED for this one, and it is worth saying why: `AppState::execute_with_outcome` already validated a
  // zero away before this slice, so the shell's path was safe by accident while `Command::apply` was not.
  // It is pinned here because the dialog is the first thing that can *type* a zero, and the two layers must
  // keep answering the same. The RED that mattered is `set_job_refuses_a_dimension_a_stock_cannot_have`
  // in `cam-app`, where the command answered `Ok(())`.
  let mut app = AppState::new(document_with_rectangle());
  let before = app.document().board.clone();
  let error = handle_action(
    &mut app,
    &UiAction::SetJob {
      width: 0.0,
      height: 400.0,
      thickness: 18.0,
    },
  )
  .expect_err("a zero width is not a stock size");
  assert!(error.contains("width"), "{error}");
  assert!(error.contains('0'), "the refusal names no number: {error}");
  assert_eq!(app.document().board, before);
}

// Covers shell-tools-ref S3: the window creates nothing outside the document's directory, even for a
// library whose header does not declare WAL but which carries a -wal beside it.
