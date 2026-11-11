//! The start screen: an empty workspace, the three choices, and the list of projects opened before
//! (start-screen S1-S3).

use cam_app::AppState;
use cam_model::{Document, Job};
use cam_ui::start_screen::{
  MAX_RECENTS, NEW_PROJECT, NOTHING_OPEN, NO_RECENTS, OPEN_PROJECT, RECENTS_UNREADABLE, RETRY,
};
use cam_ui::{handle_action, RecentProject, Shell, UiAction};

use super::support::{click_at, frame_with, press, scratch, write_c2d};

fn empty_app() -> AppState {
  AppState::new(Document::new(Job::default_sheet()))
}

#[test]
fn a_window_that_starts_with_no_project_draws_the_start_screen_and_no_stock() {
  let ctx = egui::Context::default();
  let mut shell = Shell::new();
  shell.start_with_no_project();
  let mut app = empty_app();

  let (output, _) = frame_with(&ctx, &mut shell, &mut app);

  assert!(output.drawn.start.iter().any(|word| word == NEW_PROJECT));
  assert!(output.drawn.start.iter().any(|word| word == OPEN_PROJECT));
  assert_eq!(
    output.drawn.painted_items, 0,
    "the empty workspace draws no stock"
  );
  assert!(
    output.drawn.status_line.contains(NOTHING_OPEN),
    "status is {:?}",
    output.drawn.status_line
  );
}

#[test]
fn new_project_from_the_start_screen_opens_one_and_closes_the_modal() {
  let mut shell = Shell::new();
  shell.start_with_no_project();
  let mut app = empty_app();

  shell.apply_action(&mut app, &UiAction::NewProject);

  assert!(shell.project_is_open());
  assert!(!shell.start_screen_shown());
}

#[test]
fn a_recent_row_opens_the_path_it_remembers() {
  let ctx = egui::Context::default();
  let dir = scratch("recent-open");
  let path = write_c2d(&dir, "part.c2d", &Document::new(Job::default_sheet()));
  let mut shell = Shell::new();
  shell.start_with_no_project();
  shell.set_recents(vec![RecentProject::from_path(path.clone())]);
  let mut app = empty_app();

  // Two frames: a floating, auto-sized modal is measured on the frame it first appears and settles on the next one, so a
  // hit read from the first frame names a rectangle the modal has already left. An operator always presses into a
  // settled frame; a test has to ask for one.
  let _ = frame_with(&ctx, &mut shell, &mut app);
  let (output, _) = frame_with(&ctx, &mut shell, &mut app);
  let at = output
    .drawn
    .start_hits
    .iter()
    .find(|(label, _)| label == &path.to_string_lossy())
    .map(|(_, point)| egui::pos2(point.0, point.1))
    .expect("a hit for the recent row's path");

  let (clicked, _) = click_at(&ctx, &mut shell, &mut app, at);

  assert!(
    clicked
      .actions
      .iter()
      .any(|action| matches!(action, UiAction::OpenFile { path: p } if p == &path)),
    "actions: {:?}",
    clicked.actions
  );
}

#[test]
fn escape_leaves_the_empty_workspace_and_the_app_bar_brings_the_list_back() {
  let ctx = egui::Context::default();
  let mut shell = Shell::new();
  shell.start_with_no_project();
  let mut app = empty_app();

  let _ = frame_with(&ctx, &mut shell, &mut app);
  press(&ctx, &mut shell, &mut app, egui::Key::Escape);

  assert!(!shell.start_screen_shown());
  assert!(!shell.project_is_open());

  let (output, _) = frame_with(&ctx, &mut shell, &mut app);
  let at = output
    .drawn
    .app_bar_hits
    .iter()
    .find(|(label, _)| label == cam_ui::start_screen::NO_PROJECT)
    .map(|(_, point)| egui::pos2(point.0, point.1))
    .expect("an app bar hit for the no-project opener");
  click_at(&ctx, &mut shell, &mut app, at);

  assert!(shell.start_screen_shown());
}

#[test]
fn a_command_that_needs_a_project_is_refused_and_says_so() {
  let mut shell = Shell::new();
  shell.start_with_no_project();
  let mut app = empty_app();
  let before = app.history_depth();

  shell.apply_action(&mut app, &UiAction::Undo);

  assert_eq!(app.history_depth(), before);
  assert!(
    shell.status.starts_with("refused:"),
    "status: {:?}",
    shell.status
  );
}

#[test]
fn the_recent_list_is_one_row_per_path_and_is_capped() {
  let mut shell = Shell::new();
  shell.start_with_no_project();
  let mut app = empty_app();

  let path = scratch("recents-dedup").join("same.c2d");
  handle_action(&mut app, &UiAction::NewProject).expect("new project");
  shell.apply_action(&mut app, &UiAction::NewProject);
  // Two successful saves of the same path record one row.
  shell.apply_action(&mut app, &UiAction::SaveFile { path: path.clone() });
  shell.apply_action(&mut app, &UiAction::SaveFile { path: path.clone() });
  assert_eq!(
    shell
      .recents()
      .iter()
      .filter(|entry| entry.path == path)
      .count(),
    1
  );

  for index in 0..(MAX_RECENTS + 5) {
    let p = scratch("recents-cap").join(format!("p{index}.c2d"));
    shell.apply_action(&mut app, &UiAction::SaveFile { path: p });
  }
  assert!(shell.recents().len() <= MAX_RECENTS);
  // and still at most one per path
  let mut seen = std::collections::BTreeSet::new();
  for entry in shell.recents() {
    assert!(
      seen.insert(entry.path.clone()),
      "duplicate {:?}",
      entry.path
    );
  }
}

#[test]
fn two_d_three_d_is_refused_with_no_project() {
  let mut shell = Shell::new();
  shell.start_with_no_project();

  shell.set_view_3d(true);

  assert!(!shell.view_3d());
  assert!(
    shell.status.starts_with("refused:"),
    "status: {:?}",
    shell.status
  );
}

#[test]
fn pressing_new_project_on_the_start_screen_starts_one() {
  let ctx = egui::Context::default();
  let mut shell = Shell::new();
  shell.start_with_no_project();
  let mut app = empty_app();

  // A settled frame reports the rectangle the tile is really in (see `DrawnState::start_hits`).
  let _ = frame_with(&ctx, &mut shell, &mut app);
  let (output, _) = frame_with(&ctx, &mut shell, &mut app);
  let at = output
    .drawn
    .start_hits
    .iter()
    .find(|(label, _)| label == NEW_PROJECT)
    .map(|(_, point)| egui::pos2(point.0, point.1))
    .expect("a hit for the new-project tile");

  let (clicked, _) = click_at(&ctx, &mut shell, &mut app, at);
  assert!(
    clicked
      .actions
      .iter()
      .any(|action| matches!(action, UiAction::NewProject)),
    "actions: {:?}",
    clicked.actions
  );

  // The host applies what the shell reported; the workspace holds a project from that point on.
  for action in clicked.actions {
    shell.apply_action(&mut app, &action);
  }
  assert!(shell.project_is_open());
  assert!(!shell.start_screen_shown());
}

#[test]
fn an_unreadable_list_says_so_instead_of_saying_nothing_was_opened() {
  let ctx = egui::Context::default();
  let mut shell = Shell::new();
  shell.start_with_no_project();
  shell.set_recents_error(Some(RECENTS_UNREADABLE.to_owned()));
  let mut app = empty_app();

  let _ = frame_with(&ctx, &mut shell, &mut app);
  let (output, _) = frame_with(&ctx, &mut shell, &mut app);

  assert!(
    output
      .drawn
      .start
      .iter()
      .any(|word| word == RECENTS_UNREADABLE),
    "words: {:?}",
    output.drawn.start
  );
  assert!(
    !output.drawn.start.iter().any(|word| word == NO_RECENTS),
    "a window with projects must not be told it has none"
  );

  let at = output
    .drawn
    .start_hits
    .iter()
    .find(|(label, _)| label == RETRY)
    .map(|(_, point)| egui::pos2(point.0, point.1))
    .expect("a hit for the retry button");
  let _ = click_at(&ctx, &mut shell, &mut app, at);

  // Reading the file again is the host's, so the button leaves a request and nothing else.
  assert!(shell.take_recents_retry());
}
