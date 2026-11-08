use super::support::*;
use cam_app::AppState;
use cam_store::document::DocumentFile;
use cam_ui::help::Command as HelpCommand;
use cam_ui::{handle_action, Shell, UiAction};

#[test]
fn save_with_no_recorded_path_is_refused_and_points_at_save_as() {
  let directory = scratch("save-unnamed");
  let mut app = AppState::new(document_with_rectangle());
  let before = app.document_json();
  let mut shell = Shell::new();

  shell.apply_action(&mut app, &UiAction::Save);

  assert!(
    shell.status.starts_with("refused: "),
    "a save with nowhere to write must be refused, got {:?}",
    shell.status
  );
  assert!(
    shell.status.contains("Save As"),
    "the refusal must name the one command that can ask for a path, got {:?}",
    shell.status
  );
  assert_eq!(shell.document_path(), None, "a refusal names no file");
  assert_eq!(
    app.document_json(),
    before,
    "a refused save touches no document"
  );
  assert_eq!(
    std::fs::read_dir(&directory)
      .expect("the scratch directory")
      .count(),
    0,
    "a refused save must not write anything, not even a temporary file"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn save_as_with_an_empty_path_is_refused_and_writes_nothing_into_the_working_directory() {
  let directory = scratch("save-empty-path");
  let opened = write_c2d(&directory, "bracket.c2d", &document_with_squares(1));
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(
    &mut app,
    &UiAction::OpenFile {
      path: opened.clone(),
    },
  );
  // An empty path names the process's own working directory, so the writer would compose its temporary
  // sibling there and leave it behind if the write failed: the one place a save must never reach, which
  // is why the refusal comes before anything touches the filesystem.
  let working_directory = std::env::current_dir().expect("the working directory");
  let before = listing(&working_directory);

  shell.apply_action(
    &mut app,
    &UiAction::SaveFile {
      path: std::path::PathBuf::from(""),
    },
  );

  assert!(
    shell.status.starts_with("refused: "),
    "an empty path must be refused, got {:?}",
    shell.status
  );
  assert!(
    shell.status.contains("empty"),
    "the refusal must say what is missing, got {:?}",
    shell.status
  );
  assert_eq!(
    shell.document_path(),
    Some(opened.as_path()),
    "a refused save leaves the recorded path as it was"
  );
  assert_eq!(
    listing(&working_directory),
    before,
    "a refused save must create nothing in the working directory, not even a temporary file"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn save_writes_the_opened_document_back_and_clears_the_dirty_flag() {
  let directory = scratch("save-current");
  let path = write_c2d(&directory, "bracket.c2d", &document_with_squares(2));
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(&mut app, &UiAction::OpenFile { path: path.clone() });
  assert!(
    !app.is_dirty(),
    "what was opened is exactly what is in the file"
  );
  // An edit, so the save has something of its own to write.
  handle_action(
    &mut app,
    &UiAction::AddRectangle {
      width: 10.0,
      height: 10.0,
    },
  )
  .expect("the rectangle is added");
  assert!(app.is_dirty(), "an edit is unsaved work");

  shell.apply_action(&mut app, &UiAction::Save);

  assert_eq!(shell.status, "save document — wrote bracket.c2d");
  assert!(!app.is_dirty(), "a successful save clears the dirty flag");
  assert_eq!(
    shell.document_path(),
    Some(path.as_path()),
    "a save keeps the file it wrote"
  );
  let reloaded = DocumentFile::load(&path).expect("the file the save wrote loads again");
  assert_eq!(
    &reloaded.document,
    app.document(),
    "what is on disk is what is on screen"
  );
  assert_eq!(
    app.document().entities().count(),
    3,
    "the edit the save wrote is the one on screen"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn save_as_writes_the_new_path_and_the_bar_names_it_from_then_on() {
  let directory = scratch("save-as");
  let opened = write_c2d(&directory, "bracket.c2d", &document_with_squares(2));
  let target = directory.join("copy.c2d");
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(
    &mut app,
    &UiAction::OpenFile {
      path: opened.clone(),
    },
  );

  shell.apply_action(
    &mut app,
    &UiAction::SaveFile {
      path: target.clone(),
    },
  );

  assert_eq!(shell.status, "save document as — wrote copy.c2d");
  assert_eq!(
    shell.document_path(),
    Some(target.as_path()),
    "Save As becomes the file the shell writes"
  );
  assert_eq!(shell.document_name(), "copy.c2d");
  assert!(!app.is_dirty(), "a successful save clears the dirty flag");
  let reloaded = DocumentFile::load(&target).expect("the file Save As wrote loads again");
  assert_eq!(&reloaded.document, app.document());
  let (_, texts) = frame(&mut shell, &mut app);
  let all = joined(&texts);
  assert!(
    all.contains("copy.c2d"),
    "the bar must name the file Save As wrote: {all}"
  );
  assert!(
    !all.contains("bracket.c2d"),
    "the bar must stop naming the file it was opened from: {all}"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn an_archive_entry_this_build_does_not_interpret_survives_a_save() {
  let directory = scratch("save-extras");
  let path = write_c2d_with_extras(&directory, "with-asset.c2d", &document_with_squares(1));
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(&mut app, &UiAction::OpenFile { path: path.clone() });
  assert_eq!(
    shell.document_extras().get("assets/logo.png"),
    Some(&vec![1, 2, 3, 4]),
    "opening must carry the entries this build does not read"
  );
  // An edit, so the save really rewrites the archive rather than leaving the old bytes in place.
  handle_action(
    &mut app,
    &UiAction::AddRectangle {
      width: 10.0,
      height: 10.0,
    },
  )
  .expect("the rectangle is added");

  shell.apply_action(&mut app, &UiAction::Save);

  let reloaded = DocumentFile::load(&path).expect("the saved file loads again");
  assert_eq!(
    reloaded.extras.get("assets/logo.png"),
    Some(&vec![1, 2, 3, 4]),
    "an entry a later build understands must not be dropped on the floor by a save"
  );
  assert_eq!(&reloaded.document, app.document());
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_save_of_a_document_that_does_not_validate_writes_nothing() {
  let directory = scratch("save-invalid-document");
  let path = write_c2d(&directory, "bracket.c2d", &document_with_squares(1));
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(&mut app, &UiAction::OpenFile { path: path.clone() });
  let before_bytes = std::fs::read(&path).expect("the file the drawing came from");
  let before_listing = listing(&directory);
  // A schema tag this build neither writes nor reads: the store's validation refuses the document, so the
  // save must refuse too, before any byte of it reaches the filesystem (S5).
  let mut unsupported = document_with_squares(1);
  unsupported.schema = "c2d/document@3".to_owned();
  app.replace_document_undoable(unsupported);

  shell.apply_action(&mut app, &UiAction::Save);

  assert!(
    shell.status.starts_with("refused: "),
    "an invalid document must not be written, got {:?}",
    shell.status
  );
  assert!(
    shell.status.contains("unsupported document schema"),
    "the refusal must carry the kernel's own message, got {:?}",
    shell.status
  );
  assert_eq!(
    shell.document_path(),
    Some(path.as_path()),
    "a refused save leaves the recorded path as it was"
  );
  assert_eq!(
    std::fs::read(&path).expect("the file still reads"),
    before_bytes,
    "a refused save leaves the file's bytes as they were"
  );
  assert_eq!(
    listing(&directory),
    before_listing,
    "a refused save leaves no temporary file behind either"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn every_uninterpreted_entry_survives_a_save_including_one_with_no_bytes() {
  let directory = scratch("save-every-entry");
  let path = write_c2d_with_extras(&directory, "with-entries.c2d", &document_with_squares(1));
  // Two more entries beside the asset: a text note and one that holds no bytes at all, which is the one
  // entry a map that conflated "empty" with "absent" would lose without saying so.
  let mut file = DocumentFile::load(&path).expect("the archive loads again");
  file
    .extras
    .insert("meta/notes.txt".to_owned(), b"kept".to_vec());
  file.extras.insert("empty.bin".to_owned(), Vec::new());
  file
    .save(&path)
    .expect("the archive with all three entries");
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(&mut app, &UiAction::OpenFile { path: path.clone() });
  assert_eq!(
    shell.document_extras().len(),
    3,
    "opening carries every entry this build does not read, got {:?}",
    shell.document_extras().keys().collect::<Vec<_>>()
  );
  // An edit, so the save really rewrites the archive rather than leaving the old bytes in place.
  handle_action(
    &mut app,
    &UiAction::AddRectangle {
      width: 10.0,
      height: 10.0,
    },
  )
  .expect("the rectangle is added");

  shell.apply_action(&mut app, &UiAction::Save);

  let reloaded = DocumentFile::load(&path).expect("the saved file loads again");
  assert_eq!(
    &reloaded.extras,
    shell.document_extras(),
    "every entry this build does not interpret must come back unchanged"
  );
  assert_eq!(
    reloaded.extras.get("meta/notes.txt"),
    Some(&b"kept".to_vec()),
    "an entry's bytes must come back as they went in"
  );
  assert_eq!(
    reloaded.extras.get("empty.bin"),
    Some(&Vec::new()),
    "an entry with no bytes must come back as an empty entry, not as a missing one"
  );
  assert_eq!(&reloaded.document, app.document());
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn the_archive_entries_belong_to_the_file_and_not_to_the_history() {
  let directory = scratch("save-extras-undo");
  let path = write_c2d_with_extras(&directory, "with-asset.c2d", &document_with_squares(1));
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(&mut app, &UiAction::OpenFile { path: path.clone() });
  handle_action(
    &mut app,
    &UiAction::AddRectangle {
      width: 10.0,
      height: 10.0,
    },
  )
  .expect("the rectangle is added");
  // History snapshots the bare document, so an undo steps the drawing back without touching what the file
  // carries beside it (S7).
  shell.apply_action(&mut app, &UiAction::Undo);
  assert_eq!(
    app.document().entities().count(),
    1,
    "the undo stepped the drawing back to what was opened"
  );
  assert_eq!(
    shell.document_extras().get("assets/logo.png"),
    Some(&vec![1, 2, 3, 4]),
    "an undo leaves the entries the file carries where they were"
  );

  shell.apply_action(&mut app, &UiAction::Save);

  assert_eq!(shell.status, "save document — wrote with-asset.c2d");
  let reloaded = DocumentFile::load(&path).expect("the saved file loads again");
  assert_eq!(
    reloaded.extras.get("assets/logo.png"),
    Some(&vec![1, 2, 3, 4]),
    "the entries belong to the file, so a save after an undo writes them back too"
  );
  assert_eq!(
    &reloaded.document,
    app.document(),
    "the document on disk is the one on screen"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_failed_save_changes_neither_the_dirty_flag_nor_the_recorded_path() {
  let directory = scratch("save-failed");
  let opened = write_c2d(&directory, "bracket.c2d", &document_with_squares(1));
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(
    &mut app,
    &UiAction::OpenFile {
      path: opened.clone(),
    },
  );
  handle_action(
    &mut app,
    &UiAction::AddRectangle {
      width: 10.0,
      height: 10.0,
    },
  )
  .expect("the rectangle is added");
  let before = app.document_json();
  // A directory is the one target the atomic writer cannot replace with a file: the sibling temporary file
  // is written and the rename onto it fails, which is the error path a save has to survive.
  let blocked = directory.join("locked.c2d");
  std::fs::create_dir(&blocked).expect("a directory where a document should be");

  shell.apply_action(
    &mut app,
    &UiAction::SaveFile {
      path: blocked.clone(),
    },
  );

  assert!(
    shell.status.starts_with("refused: "),
    "a failed save must be refused, got {:?}",
    shell.status
  );
  assert!(
    shell.status.contains("locked.c2d"),
    "the refusal must name the file it could not write, got {:?}",
    shell.status
  );
  assert!(
    app.is_dirty(),
    "a failed save leaves the dirty flag as it was"
  );
  assert_eq!(
    shell.document_path(),
    Some(opened.as_path()),
    "a failed save leaves the recorded path as it was"
  );
  assert_eq!(app.document_json(), before, "a failed save edits nothing");
  let reloaded = DocumentFile::load(&opened).expect("the file the drawing came from still loads");
  assert_eq!(
    reloaded.document,
    document_with_squares(1),
    "the atomic writer left the previous bytes in place"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn opening_a_dirty_document_is_refused_and_a_clean_one_still_opens() {
  let directory = scratch("open-dirty");
  let first = write_c2d(&directory, "first.c2d", &document_with_squares(1));
  let second = write_c2d(&directory, "second.c2d", &document_with_squares(3));
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(
    &mut app,
    &UiAction::OpenFile {
      path: first.clone(),
    },
  );
  assert!(
    !app.is_dirty(),
    "a document freshly opened is not dirty, and the bar must stop saying modified"
  );
  handle_action(
    &mut app,
    &UiAction::AddRectangle {
      width: 10.0,
      height: 10.0,
    },
  )
  .expect("the rectangle is added");
  let before = app.document_json();
  let history = app.history_depth();

  shell.apply_action(
    &mut app,
    &UiAction::OpenFile {
      path: second.clone(),
    },
  );

  assert!(
    shell.status.starts_with("refused: "),
    "an open over unsaved work must be refused, got {:?}",
    shell.status
  );
  assert!(
    shell.status.contains("save or undo"),
    "the refusal must tell the operator what to do first, got {:?}",
    shell.status
  );
  assert_eq!(
    app.document_json(),
    before,
    "a refused open leaves the drawing alone"
  );
  assert_eq!(
    app.history_depth(),
    history,
    "a refused open records no step"
  );
  assert!(app.is_dirty(), "a refused open leaves the dirty flag alone");
  assert_eq!(
    shell.document_path(),
    Some(first.as_path()),
    "a refused open leaves the file the bar names alone"
  );
  assert_eq!(shell.document_name(), "first.c2d");

  // Saving is the way through, and then the open the operator asked for happens.
  shell.apply_action(&mut app, &UiAction::Save);
  assert!(!app.is_dirty());
  shell.apply_action(
    &mut app,
    &UiAction::OpenFile {
      path: second.clone(),
    },
  );
  assert_eq!(app.document().entities().count(), 3);
  assert_eq!(shell.document_name(), "second.c2d");
  assert!(
    !app.is_dirty(),
    "the drawing the clean open brought is not modified"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn the_rails_save_chips_emit_their_actions() {
  let mut app = AppState::new(document_with_rectangle());
  let before = app.document_json();
  let mut shell = Shell::new();
  let ctx = egui::Context::default();
  let _ = frame_with(&ctx, &mut shell, &mut app);

  let (output, _) = click_palette_chip(&ctx, &mut shell, &mut app, HelpCommand::Save.label());
  assert_eq!(
    output.actions,
    vec![UiAction::Save],
    "the Save chip writes the file the shell already has"
  );

  let (output, _) = click_palette_chip(&ctx, &mut shell, &mut app, HelpCommand::SaveAs.label());
  assert_eq!(
    output.actions,
    vec![UiAction::RequestSaveAs],
    "Save As must ask the host, exactly as Open does"
  );
  assert_eq!(app.document_json(), before, "an intent touches no document");
  assert_eq!(shell.document_path(), None, "an intent writes no file");
}

#[test]
fn a_bare_handle_action_refuses_to_save_because_the_shell_holds_the_entries() {
  let directory = scratch("save-bare");
  let target = directory.join("nowhere.c2d");
  let mut app = AppState::new(document_with_rectangle());
  let before = app.document_json();

  let error = handle_action(
    &mut app,
    &UiAction::SaveFile {
      path: target.clone(),
    },
  )
  .expect_err("a write with no archive entries to carry must be refused");

  assert!(
    error.contains("shell") && error.contains("entries"),
    "the refusal must say what is missing, got {error:?}"
  );
  assert!(
    !target.exists(),
    "a refused save must not create the file it was given: {}",
    target.display()
  );
  assert_eq!(
    std::fs::read_dir(&directory)
      .expect("the scratch directory")
      .count(),
    0,
    "a refused save must leave no file and no temporary sibling behind"
  );
  assert_eq!(
    app.document_json(),
    before,
    "a refused save touches no document"
  );
  // A bare `Save` refuses for the same rule: with no shell there is no recorded path, so it names Save As
  // rather than pick a file.
  let error =
    handle_action(&mut app, &UiAction::Save).expect_err("a bare save has no file to write");
  assert!(error.contains("Save As"), "got {error:?}");
  assert_eq!(
    std::fs::read_dir(&directory)
      .expect("the scratch directory")
      .count(),
    0,
    "a refused Save must create nothing either"
  );

  // The same action through the shell writes, and keeps the entries the shell is holding: the rule is about
  // who holds them, not about saving being refused.
  let opened = write_c2d_with_extras(&directory, "with-asset.c2d", &document_with_squares(1));
  let mut shell = Shell::new();
  shell.apply_action(&mut app, &UiAction::OpenFile { path: opened });
  shell.apply_action(
    &mut app,
    &UiAction::SaveFile {
      path: target.clone(),
    },
  );
  assert!(
    target.is_file(),
    "the shell writes the file the bare path refused"
  );
  let reloaded = DocumentFile::load(&target).expect("the shell's save loads again");
  assert_eq!(
    reloaded.extras.get("assets/logo.png"),
    Some(&vec![1, 2, 3, 4]),
    "the entries the shell holds are the ones written"
  );
  std::fs::remove_dir_all(&directory).ok();
}

// ---- T8.1: the command table and the F1 help panel --------------------------------------------
