//! The app bar's `Post…` picker, end to end on the shell's side of the host dialog (113 T2, S3).
//!
//! The dialog itself cannot exist in a headless test, so these tests pin the two halves of the seam the
//! other file actions already use: the press emits the *intent* (`UiAction::RequestPost`), and the host's
//! answer (`UiAction::SetPost { path }`) is what reads the file, validates it and makes it the window's post.

use super::support::*;
use cam_app::AppState;
use cam_ui::{Shell, UiAction};

/// Write a post-processor file and return its path.
fn write_post(directory: &std::path::Path, name: &str, source: &str) -> std::path::PathBuf {
  let path = directory.join(name);
  std::fs::write(&path, source).expect("the post-processor file is written");
  path
}

/// The app bar's reported hit for the post-processor button.
fn post_hit(output: &cam_ui::ShellOutput) -> egui::Pos2 {
  output
    .drawn
    .app_bar_hits
    .iter()
    .find(|(label, _)| label == "Post")
    .map(|(_, point)| egui::pos2(point.0, point.1))
    .unwrap_or_else(|| {
      panic!(
        "no Post hit in the app bar: {:?}",
        output.drawn.app_bar_hits
      )
    })
}

#[test]
fn pressing_post_asks_the_host_for_a_file_and_chooses_nothing_itself() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();

  let (output, _) = frame_with(&ctx, &mut shell, &mut app);
  let at = post_hit(&output);
  let (clicked, _) = click_at(&ctx, &mut shell, &mut app, at);

  assert!(
    clicked
      .actions
      .iter()
      .any(|action| matches!(action, UiAction::RequestPost)),
    "the press must ask the host for a post-processor file: {:?}",
    clicked.actions
  );
  assert_eq!(
    shell.post_name(),
    None,
    "asking is not choosing: the post in hand is untouched"
  );
}

#[test]
fn a_chosen_post_becomes_the_windows_post_and_the_bar_names_it() {
  let directory = scratch("post-chosen");
  let path = write_post(&directory, "grbl.pp", cam_post::GRBL_POST);
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();

  shell.apply_action(&mut app, &UiAction::SetPost { path: path.clone() });

  assert_eq!(
    shell.status, "choose post-processor — grbl.pp",
    "the status bar names the file the operator chose"
  );
  assert_eq!(
    shell.post_name(),
    Some("grbl.pp"),
    "the file's own label is what the app bar shows"
  );
  let (_, texts) = frame(&mut shell, &mut app);
  assert!(
    joined(&texts).contains("Post: grbl.pp"),
    "the app bar must name the chosen post: {}",
    joined(&texts)
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_file_that_does_not_parse_as_a_post_is_refused_by_name_and_changes_nothing() {
  let directory = scratch("post-refused");
  let good = write_post(&directory, "grbl.pp", cam_post::GRBL_POST);
  let bad = write_post(
    &directory,
    "bogus.pp",
    "this line is not post-processor dialect\n",
  );
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();

  // With no post chosen, a refusal leaves none: the shell does not fall back to a default dialect.
  shell.apply_action(&mut app, &UiAction::SetPost { path: bad.clone() });

  assert!(
    shell.status.starts_with("refused: "),
    "an unparseable file must be refused, got {:?}",
    shell.status
  );
  assert!(
    shell.status.contains("bogus.pp"),
    "the refusal must name the file, got {:?}",
    shell.status
  );
  assert_eq!(shell.post_name(), None, "a refusal chooses no post");

  // And with one chosen, a refusal leaves exactly that one: no partial state.
  shell.apply_action(&mut app, &UiAction::SetPost { path: good });
  shell.apply_action(&mut app, &UiAction::SetPost { path: bad });

  assert_eq!(
    shell.post_name(),
    Some("grbl.pp"),
    "a refused choice leaves the post that was already held"
  );
  assert!(
    shell.status.contains("bogus.pp"),
    "the second refusal names its own file, got {:?}",
    shell.status
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_post_chosen_for_one_document_does_not_apply_to_the_next_one() {
  let directory = scratch("post-per-document");
  let first = write_c2d(&directory, "first.cmc", &document_with_rectangle());
  let second = write_c2d(&directory, "second.cmc", &document_with_squares(1));
  let path = write_post(&directory, "grbl.pp", cam_post::GRBL_POST);
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();

  shell.apply_action(&mut app, &UiAction::OpenFile { path: first });
  shell.apply_action(&mut app, &UiAction::SetPost { path: path.clone() });
  assert_eq!(
    shell.post_name(),
    Some("grbl.pp"),
    "the first document's post"
  );

  // Replacing the document replaces everything the window derived from it, the post included: a dialect
  // chosen for one drawing is a choice about that drawing, not a default for the next one.
  shell.apply_action(&mut app, &UiAction::OpenFile { path: second });
  assert_eq!(
    shell.post_name(),
    None,
    "an open must not carry the previous document's post"
  );

  shell.apply_action(&mut app, &UiAction::SetPost { path });
  assert_eq!(shell.post_name(), Some("grbl.pp"));
  shell.apply_action(&mut app, &UiAction::NewProject);
  assert_eq!(
    shell.post_name(),
    None,
    "a new project must not carry the previous document's post"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_bare_handler_refuses_a_post_choice_instead_of_holding_nothing() {
  // The post is window state, so an entry point with no window has nowhere to put it; refusing says so
  // rather than silently dropping the choice the operator made.
  let mut app = AppState::new(document_with_rectangle());

  let error = cam_ui::handle_action(
    &mut app,
    &UiAction::SetPost {
      path: std::path::PathBuf::from("grbl.pp"),
    },
  )
  .expect_err("a bare handler cannot hold a post");

  assert!(
    error.contains("post-processor"),
    "the refusal must name what is missing, got {error:?}"
  );
}

#[test]
fn an_empty_path_is_refused_before_anything_is_read() {
  // An empty path names no file, so the refusal comes before the read: nothing is opened and the post the
  // window holds is untouched.
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();

  shell.apply_action(
    &mut app,
    &UiAction::SetPost {
      path: std::path::PathBuf::from(""),
    },
  );

  assert!(
    shell.status.starts_with("refused: ") && shell.status.contains("empty"),
    "an empty path must be refused as such, got {:?}",
    shell.status
  );
  assert_eq!(shell.post_name(), None, "a refusal chooses no post");
}
