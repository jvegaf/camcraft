//! The app bar's `Generate G-code`, end to end on the shell's side of the host dialog (113 T3, S3, S5, S6, S8).
//!
//! The save dialog cannot exist in a headless test, so these tests pin the two halves of the seam the other file
//! actions already use: the press emits the *intent* (`UiAction::RequestGenerateProgram`), and the host's answer
//! (`UiAction::GenerateProgram { path }`) is what builds the stream and writes the program through the one shared
//! emission path. Every refusal writes nothing, which is asserted against the directory itself.

use super::support::*;
use cam_app::AppState;
use cam_ui::{Shell, UiAction};

/// The app bar's reported hit for the generate button.
fn generate_hit(output: &cam_ui::ShellOutput) -> egui::Pos2 {
  output
    .drawn
    .app_bar_hits
    .iter()
    .find(|(label, _)| label == "Generate G-code")
    .map(|(_, point)| egui::pos2(point.0, point.1))
    .unwrap_or_else(|| {
      panic!(
        "no Generate G-code hit in the app bar: {:?}",
        output.drawn.app_bar_hits
      )
    })
}

/// A window that can write a program: the drawing has a toolpath, the tool it names is in the snapshot and a
/// post-processor has been chosen.
fn window_that_can_generate() -> (AppState, Shell) {
  let app = AppState::new(document_with_toolpath(1, 7));
  let mut shell = Shell::new();
  shell.set_tools(vec![stored_tool(7)]);
  shell.set_post("grbl.pp", cam_post::GRBL_POST);
  (app, shell)
}

#[test]
fn pressing_generate_asks_the_host_for_a_path_and_writes_nothing_itself() {
  let directory = scratch("generate-asked");
  let ctx = egui::Context::default();
  let (mut app, mut shell) = window_that_can_generate();

  let (output, _) = frame_with(&ctx, &mut shell, &mut app);
  let at = generate_hit(&output);
  let (clicked, _) = click_at(&ctx, &mut shell, &mut app, at);

  assert!(
    clicked
      .actions
      .iter()
      .any(|action| matches!(action, UiAction::RequestGenerateProgram)),
    "the press must ask the host for an output path: {:?}",
    clicked.actions
  );
  assert!(
    listing(&directory).is_empty(),
    "asking is not writing: {:?}",
    listing(&directory)
  );
  assert!(!app.is_dirty(), "asking is not an edit");
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_path_from_the_host_is_written_as_a_program_and_the_same_drawing_twice_is_identical() {
  let directory = scratch("generate-written");
  let (mut app, mut shell) = window_that_can_generate();
  let first = directory.join("board.nc");
  let second = directory.join("board-again.nc");

  // The press is the intent; the scratch path below is the host's answer to it.
  let ctx = egui::Context::default();
  let (output, _) = frame_with(&ctx, &mut shell, &mut app);
  let at = generate_hit(&output);
  let (pressed, _) = click_at(&ctx, &mut shell, &mut app, at);
  assert!(
    pressed
      .actions
      .iter()
      .any(|action| matches!(action, UiAction::RequestGenerateProgram)),
    "the press must ask the host for an output path: {:?}",
    pressed.actions
  );

  shell.apply_action(
    &mut app,
    &UiAction::GenerateProgram {
      path: first.clone(),
    },
  );
  assert!(
    first.exists(),
    "the program file must exist after the answer, status: {:?}",
    shell.status
  );
  assert_eq!(
    shell.status, "generate program — wrote board.nc",
    "the status bar names the file written"
  );
  let first_text = std::fs::read_to_string(&first).expect("the program reads");
  assert!(
    first_text.contains("G1"),
    "the program carries a cutting move:\n{first_text}"
  );
  assert!(!app.is_dirty(), "writing a program is not an edit (S6)");

  shell.apply_action(
    &mut app,
    &UiAction::GenerateProgram {
      path: second.clone(),
    },
  );
  let second_text = std::fs::read_to_string(&second).expect("the second program reads");
  assert_eq!(
    first_text, second_text,
    "the same drawing and post must emit byte for byte the same program"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn with_no_post_chosen_the_answer_is_refused_by_name_and_nothing_is_written() {
  let directory = scratch("generate-no-post");
  let mut app = AppState::new(document_with_toolpath(1, 7));
  let mut shell = Shell::new();
  shell.set_tools(vec![stored_tool(7)]);
  let out = directory.join("board.nc");

  shell.apply_action(&mut app, &UiAction::GenerateProgram { path: out.clone() });

  assert!(
    shell.status.starts_with("refused: "),
    "no post chosen must refuse, got {:?}",
    shell.status
  );
  assert!(
    shell.status.contains("post-processor"),
    "the refusal names what is missing, got {:?}",
    shell.status
  );
  assert!(
    shell.status.contains("Post…"),
    "the refusal names the way to choose one, got {:?}",
    shell.status
  );
  assert!(
    listing(&directory).is_empty(),
    "a refusal writes no file: {:?}",
    listing(&directory)
  );
  assert!(!app.is_dirty(), "a refusal changes no document");
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn with_no_project_open_the_answer_is_refused_and_nothing_is_written() {
  let directory = scratch("generate-no-document");
  let (mut app, mut shell) = window_that_can_generate();
  shell.start_with_no_project();
  let out = directory.join("board.nc");

  shell.apply_action(&mut app, &UiAction::GenerateProgram { path: out.clone() });

  assert!(
    shell.status.starts_with("refused: "),
    "no document open must refuse, got {:?}",
    shell.status
  );
  assert_eq!(
    shell.status,
    format!("refused: {}", cam_ui::start_screen::NO_PROJECT_REFUSAL),
    "the refusal is the start screen's own sentence"
  );
  assert!(
    listing(&directory).is_empty(),
    "a refusal writes no file: {:?}",
    listing(&directory)
  );
  assert!(!app.is_dirty(), "a refusal changes no document");
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn an_empty_output_path_is_refused_and_nothing_is_written() {
  let directory = scratch("generate-empty-path");
  let (mut app, mut shell) = window_that_can_generate();

  shell.apply_action(
    &mut app,
    &UiAction::GenerateProgram {
      path: std::path::PathBuf::new(),
    },
  );

  assert!(
    shell.status.starts_with("refused: "),
    "an empty path must refuse, got {:?}",
    shell.status
  );
  assert!(
    shell.status.contains("empty"),
    "the refusal names the empty path, got {:?}",
    shell.status
  );
  assert!(
    listing(&directory).is_empty(),
    "a refusal writes no file: {:?}",
    listing(&directory)
  );
  assert!(!app.is_dirty(), "a refusal changes no document");
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn with_no_stream_to_emit_the_builders_own_message_is_reported_and_nothing_is_written() {
  let directory = scratch("generate-no-stream");
  // The drawing names tool 7, but the window's snapshot is empty: the builder says which toolpath it skipped,
  // and that sentence is what the status bar has to show rather than one of its own.
  let mut app = AppState::new(document_with_toolpath(1, 7));
  let mut shell = Shell::new();
  shell.set_post("grbl.pp", cam_post::GRBL_POST);
  let out = directory.join("board.nc");

  shell.apply_action(&mut app, &UiAction::GenerateProgram { path: out.clone() });

  assert!(
    shell.status.starts_with("refused: "),
    "no stream must refuse, got {:?}",
    shell.status
  );
  assert!(
    shell.status.contains("missing tool 7"),
    "the builder's own message must reach the status bar, got {:?}",
    shell.status
  );
  assert!(
    listing(&directory).is_empty(),
    "a refusal writes no file: {:?}",
    listing(&directory)
  );
  assert!(!app.is_dirty(), "a refusal changes no document");
  std::fs::remove_dir_all(&directory).ok();
}
