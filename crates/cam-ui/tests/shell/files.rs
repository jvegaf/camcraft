use super::support::*;
use cam_app::AppState;
use cam_geom::Point2;
use cam_model::Job;
use cam_move::MoveStream;
use cam_sim::Heightfield;
use cam_store::document::DocumentFile;
use cam_ui::help::Command as HelpCommand;
use cam_ui::{handle_action, Shell, UiAction};
use cam_units::Length;

#[test]
fn opening_and_saving_stamp_the_brand_block_with_the_files_own_write() {
  // L7's rule: the line stamps the document. The stamp therefore comes from the file the action read, which
  // is the file's mtime — not from `now`, and not from the binary.
  let directory = std::env::temp_dir().join(format!("campilot-stamp-{}", std::process::id()));
  std::fs::remove_dir_all(&directory).ok();
  std::fs::create_dir_all(&directory).expect("scratch directory");
  let path = write_c2d(&directory, "stamped.c2d", &document_with_rectangle());
  let opened_at = std::fs::metadata(&path)
    .and_then(|metadata| metadata.modified())
    .expect("the file has a modification time");

  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(&mut app, &UiAction::OpenFile { path: path.clone() });

  let stamp = cam_ui::stamp::format_stamp(opened_at);
  assert_eq!(shell.saved_stamp(), format!("Saved {stamp}"));
  let (_, texts) = frame(&mut shell, &mut app);
  assert!(
    joined(&texts).contains(&format!("Saved {stamp}")),
    "the rail must paint the file's own stamp: {}",
    joined(&texts)
  );

  // An edit makes the document unsaved without changing what the file last was: the block says the first,
  // because that is the question an operator asks before closing.
  shell.apply_action(
    &mut app,
    &UiAction::AddRectangle {
      width: 10.0,
      height: 10.0,
    },
  );
  assert!(app.is_dirty(), "the rectangle is an unsaved edit");
  let (_, texts) = frame(&mut shell, &mut app);
  assert!(
    joined(&texts).contains("Unsaved changes"),
    "an edited drawing reads unsaved: {}",
    joined(&texts)
  );

  // Saving it stamps it again, from the write that just happened.
  shell.apply_action(&mut app, &UiAction::Save);
  assert!(!app.is_dirty(), "a save writes the drawing");
  let written_at = std::fs::metadata(&path)
    .and_then(|metadata| metadata.modified())
    .expect("the file has a modification time");
  assert_eq!(
    shell.saved_stamp(),
    format!("Saved {}", cam_ui::stamp::format_stamp(written_at)),
    "the stamp follows the write"
  );
  std::fs::remove_dir_all(&directory).ok();
}

/// A two-line DXF, written to a scratch directory, so the shell test drives a real importer.
fn write_dxf(directory: &std::path::Path) -> std::path::PathBuf {
  let dxf = "0\nSECTION\n2\nENTITIES\n\
0\nLINE\n8\n0\n10\n0.0\n20\n0.0\n11\n100.0\n21\n0.0\n\
0\nLINE\n8\n0\n10\n100.0\n20\n0.0\n11\n100.0\n21\n60.0\n\
0\nENDSEC\n0\nEOF\n";
  let path = directory.join("part.dxf");
  std::fs::write(&path, dxf).expect("dxf written");
  path
}

#[test]
fn importing_a_file_replaces_the_drawing_and_says_what_arrived() {
  let directory = scratch("import");
  let path = write_dxf(&directory);
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();

  shell.apply_action(
    &mut app,
    &UiAction::ImportFile {
      path: path.display().to_string(),
    },
  );

  assert_eq!(app.document().entities().count(), 2);
  assert_eq!(
    shell.status,
    "import file — 2 entities created, 2 contour(s) from part.dxf, 0 curve(s) flattened, 0 skipped"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_failed_import_leaves_the_drawing_untouched() {
  let mut app = AppState::new(document_with_rectangle());
  let before = app.document_json();
  let mut shell = Shell::new();

  shell.apply_action(
    &mut app,
    &UiAction::ImportFile {
      path: "/nonexistent/part.dxf".to_owned(),
    },
  );

  assert_eq!(app.document_json(), before);
  assert_eq!(app.history_depth(), 0);
  assert!(!app.is_dirty());
  assert!(shell.status.starts_with("refused: "), "{}", shell.status);
  assert!(
    shell.status.contains("/nonexistent/part.dxf"),
    "{}",
    shell.status
  );
}

#[test]
fn one_undo_after_an_import_brings_the_previous_drawing_back() {
  let directory = scratch("import-undo");
  let path = write_dxf(&directory);
  let mut app = AppState::new(document_with_rectangle());
  let original = app.document_json();
  let mut shell = Shell::new();

  shell.apply_action(
    &mut app,
    &UiAction::ImportFile {
      path: path.display().to_string(),
    },
  );
  assert_eq!(app.document().entities().count(), 2);

  shell.apply_action(&mut app, &UiAction::Undo);

  assert_eq!(app.document_json(), original);
  std::fs::remove_dir_all(&directory).ok();
}

// ---- Opening a document chosen at runtime ------------------------------------------------------

#[test]
fn opening_a_file_shows_its_document_and_its_name_in_the_app_bar() {
  let directory = scratch("open");
  let path = write_c2d(&directory, "bracket.c2d", &document_with_squares(2));
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  assert_eq!(shell.document_name(), "untitled.c2d");

  shell.apply_action(&mut app, &UiAction::OpenFile { path: path.clone() });

  assert_eq!(
    app.document().entities().count(),
    2,
    "the opened drawing is the one showing"
  );
  assert_eq!(shell.status, "open file — opened bracket.c2d");
  assert_eq!(shell.document_path(), Some(path.as_path()));
  assert_eq!(shell.document_name(), "bracket.c2d");

  let (_, texts) = frame(&mut shell, &mut app);
  let all = joined(&texts);
  assert!(
    all.contains("bracket.c2d"),
    "the bar must name the opened file: {all}"
  );
  assert!(
    !all.contains("untitled.c2d"),
    "the bar must stop saying untitled: {all}"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_refused_open_changes_nothing_and_says_why() {
  let directory = scratch("open-refused");
  let opened = write_c2d(&directory, "bracket.c2d", &document_with_squares(2));
  let missing = directory.join("gone.c2d");
  let not_an_archive = directory.join("notes.c2d");
  std::fs::write(&not_an_archive, b"not a zip archive, just text").expect("text written");
  let invalid = directory.join("broken.c2d");
  let mut broken = document_with_squares(1);
  let repeated = broken.layers[0].clone();
  // Two layers with one id: a document the model refuses, so the file loads and the validation fails.
  broken.layers.push(repeated);
  std::fs::write(
    &invalid,
    DocumentFile {
      document: broken,
      extras: Default::default(),
    }
    .to_bytes()
    .expect("archive bytes"),
  )
  .expect("archive written");

  for (case, path) in [
    ("missing", missing),
    ("not an archive", not_an_archive),
    ("invalid document", invalid),
  ] {
    let mut app = AppState::new(document_with_rectangle());
    let mut shell = Shell::new();
    // One good open first, so "the name did not change" is a claim about a name that exists, and so the
    // history holds a step a refused open must not add to.
    shell.apply_action(
      &mut app,
      &UiAction::OpenFile {
        path: opened.clone(),
      },
    );
    let before = app.document_json();
    let history = app.history_depth();
    let dirty = app.is_dirty();

    shell.apply_action(&mut app, &UiAction::OpenFile { path: path.clone() });

    assert!(
      shell.status.starts_with("refused: "),
      "{case}: the status must refuse, got {:?}",
      shell.status
    );
    assert!(
      shell.status.contains(&path.display().to_string()),
      "{case}: the refusal must name the file, got {:?}",
      shell.status
    );
    assert_eq!(
      app.document_json(),
      before,
      "{case}: a refused open must leave the drawing alone"
    );
    assert_eq!(
      app.history_depth(),
      history,
      "{case}: a refused open must not record a step"
    );
    assert_eq!(
      app.is_dirty(),
      dirty,
      "{case}: a refused open must not change the dirty flag"
    );
    assert_eq!(
      shell.document_name(),
      "bracket.c2d",
      "{case}: a refused open must not change the name in the bar"
    );
  }
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn opening_a_file_clears_the_preview_the_tool_snapshot_and_the_material_drafts() {
  let directory = scratch("open-stale");
  let path = write_c2d(&directory, "cut.c2d", &document_with_toolpath(2, 4));
  let mut app = AppState::new(document_with_toolpath(1, 4));
  let mut shell = Shell::new();
  shell.set_tools(vec![stored_tool(4)]);
  let job = Job::default_sheet();
  shell.set_preview(
    Heightfield::new(&job, Length::from_mm(20.0)).expect("a field for the default sheet"),
    MoveStream::new(Length::from_mm(6.0), Point2::new(0.0, 0.0)),
    &job,
  );
  shell.material_size = (300.0, 200.0, 25.0);
  // The card resolves T4 while the snapshot is the one this window was given.
  let (output, _) = frame(&mut shell, &mut app);
  assert!(
    output
      .drawn
      .tool_card
      .iter()
      .any(|line| line.starts_with("T4 - ")),
    "the fixture must start with a resolvable tool: {:?}",
    output.drawn.tool_card
  );

  shell.apply_action(&mut app, &UiAction::OpenFile { path });

  assert_eq!(app.document().entities().count(), 2);
  assert!(
    shell.preview().is_none(),
    "the 3D panel must not keep playing the previous drawing's program"
  );
  assert_eq!(
    shell.material_size,
    (
      Job::default_sheet().width.mm(),
      Job::default_sheet().height.mm(),
      Job::default_sheet().thickness.mm(),
    ),
    "the drafts were seeded for a document nobody is looking at any more"
  );
  let (output, _) = frame(&mut shell, &mut app);
  assert!(
    output
      .drawn
      .tool_card
      .iter()
      .any(|line| line.contains("T4 is not in the tool library")),
    "the tool snapshot belonged to the previous window: {:?}",
    output.drawn.tool_card
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn an_open_is_not_undoable_and_undo_leaves_the_opened_document_alone() {
  let directory = scratch("open-undo");
  let path = write_c2d(&directory, "bracket.c2d", &document_with_squares(3));
  let mut app = AppState::new(document_with_rectangle());
  let original = app.document_json();
  let mut shell = Shell::new();

  shell.apply_action(&mut app, &UiAction::OpenFile { path: path.clone() });

  assert_eq!(
    app.history_depth(),
    0,
    "an open records no undo step: it replaces the drawing and the recorded path together"
  );
  let opened = app.document_json();

  shell.apply_action(&mut app, &UiAction::Undo);

  assert_eq!(
    app.document_json(),
    opened,
    "an undo after an open leaves the opened drawing on screen"
  );
  assert_ne!(
    app.document_json(),
    original,
    "an undo must not reach back across the open to the drawing it replaced"
  );
  assert_eq!(
    shell.document_path(),
    Some(path.as_path()),
    "the recorded path still names the drawing on screen"
  );
  assert_eq!(
    app.history_depth(),
    0,
    "an undo with nothing to step back to records no step of its own"
  );
  std::fs::remove_dir_all(&directory).ok();
}

/// The class the verifier reproduced: an undo that crosses a document boundary lets a save write the wrong
/// drawing over the file the bar still names, dropping that file's archive entries.
///
/// Nothing in this shell can make that undo: an open records no history entry, so `Undo` after the second
/// open has nothing to step back to. The assertions below are one-sided on purpose — they fail if the open
/// is ever made undoable again, because the save would then write the first document, and its bytes and
/// archive entries, over the second file.
#[test]
fn an_undo_between_two_opens_cannot_make_a_save_overwrite_the_wrong_document() {
  let directory = scratch("open-two-undo-save");
  let first = write_c2d_with_extras(&directory, "first.c2d", &document_with_squares(3));
  let second = write_c2d_with_extras(&directory, "second.c2d", &document_with_squares(1));
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(&mut app, &UiAction::OpenFile { path: first });
  shell.apply_action(
    &mut app,
    &UiAction::OpenFile {
      path: second.clone(),
    },
  );
  let second_document = app.document_json();
  let before_bytes = std::fs::read(&second).expect("the second file as it was before the save");
  let before_extras = DocumentFile::load(&second)
    .expect("the second file loads before the save")
    .extras;

  shell.apply_action(&mut app, &UiAction::Undo);

  assert_eq!(
    app.document_json(),
    second_document,
    "an undo after opens must not step back across a document boundary"
  );
  shell.apply_action(&mut app, &UiAction::Save);

  assert_eq!(shell.status, "save document — wrote second.c2d");
  assert_eq!(
    shell.document_path(),
    Some(second.as_path()),
    "the recorded path still names the second file"
  );
  assert_eq!(
    std::fs::read(&second).expect("the second file after the save"),
    before_bytes,
    "the file the bar names must come out of an undo and a save byte for byte as it went in"
  );
  let after = DocumentFile::load(&second).expect("the second file loads after the save");
  assert_eq!(
    after.extras, before_extras,
    "the second file's archive entries must survive the save"
  );
  assert_eq!(
    &after.document,
    app.document(),
    "the drawing on disk is the one on screen"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn an_open_frames_the_new_drawing_even_when_the_view_was_the_users() {
  let directory = scratch("open-frame");
  let path = write_c2d(&directory, "wide.c2d", &document_with_squares(2));
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let _ = frame(&mut shell, &mut app);
  // The user takes the view: panning and zooming stops the shell framing the drawing.
  shell.release_view();
  app
    .camera_mut()
    .zoom_at(cam_app::ScreenPoint::new(10.0, 10.0), 4.0);

  shell.apply_action(&mut app, &UiAction::OpenFile { path });
  let _ = frame(&mut shell, &mut app);

  let centre = app.camera().center();
  assert!(
    centre.is_near(Point2::new(135.0, 45.0), 5.0),
    "the opened drawing must be framed, not left at the old drawing's zoom: {centre:?}"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn the_rails_open_chip_asks_the_host_for_a_file() {
  let mut app = AppState::new(document_with_rectangle());
  let before = app.document_json();
  let mut shell = Shell::new();
  let ctx = egui::Context::default();
  let _ = frame_with(&ctx, &mut shell, &mut app);

  let (output, _) = click_palette_chip(&ctx, &mut shell, &mut app, HelpCommand::Open.label());

  assert_eq!(
    output.actions,
    vec![UiAction::RequestOpenFile],
    "the chip must ask the host, not open a file of its own"
  );
  assert_eq!(app.document_json(), before, "an intent touches no document");
  assert_eq!(
    shell.document_name(),
    "untitled.c2d",
    "an intent names no file"
  );
}

#[test]
fn the_rails_file_chips_still_emit_their_commands() {
  // PRESERVE (camui-prancheta T2b): these seven buttons lived in the app bar and emitted exactly these
  // actions. They emit the same actions from the rail, which is the move's whole claim — one surface, and
  // the command it runs unchanged.
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let ctx = egui::Context::default();
  let _ = frame_with(&ctx, &mut shell, &mut app);

  let (output, _) = click_palette_chip(&ctx, &mut shell, &mut app, HelpCommand::FitJob.label());
  assert_eq!(
    output.actions,
    vec![UiAction::ZoomToFit],
    "Fit the job still frames the drawing"
  );

  handle_action(
    &mut app,
    &UiAction::AddRectangle {
      width: 10.0,
      height: 10.0,
    },
  )
  .expect("the rectangle is added");
  let (output, _) = click_palette_chip(&ctx, &mut shell, &mut app, HelpCommand::Undo.label());
  assert_eq!(
    output.actions,
    vec![UiAction::Undo],
    "Undo still steps back"
  );
  shell.apply_action(&mut app, &UiAction::Undo);

  let (output, _) = click_palette_chip(&ctx, &mut shell, &mut app, HelpCommand::Redo.label());
  assert_eq!(
    output.actions,
    vec![UiAction::Redo],
    "Redo still steps forward"
  );
}

// ---- Saving the document the window opened (S2, S3, S4, S5) -----------------------------------
