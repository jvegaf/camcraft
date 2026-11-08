use super::support::*;
use cam_app::AppState;
use cam_store::document::save_document;
use cam_store::tools::ToolLibrary;
use cam_ui::{Shell, ShellOutput, UiAction};

/// A real tool library holding `tool_number`'s tool, written beside the document that names it.
///
/// The current-tool card is how a test reads the snapshot an open handed the shell, so the library has to
/// be a real file the store can open: `put_geometry` plus `put_cutting_data` plus the `link` between them
/// is what makes `ToolLibrary::tools` return a record at all.
fn write_library(path: &std::path::Path, tool_number: u32) {
  let library = ToolLibrary::open(path).expect("library");
  let tool = stored_tool(tool_number);
  library.put_geometry(&tool.geometry).expect("geometry");
  library
    .put_cutting_data(&tool.cutting)
    .expect("cutting data");
  library
    .link("E1", &tool.geometry.id, &tool.cutting.id, None, None)
    .expect("link");
}

/// One frame of the shell, which is the frame that draws the current-tool card and the cutting-time line
/// beneath it.
fn toolpaths_frame(shell: &mut Shell, app: &mut AppState) -> (ShellOutput, Vec<String>) {
  frame(shell, app)
}

#[test]
fn an_opened_document_resolves_the_tool_library_beside_it() {
  let directory = scratch("open-tools-ref");
  write_library(&directory.join("cut-tools.sqlite"), 4);
  let mut document = document_with_toolpath(2, 4);
  document.tools_ref = Some("cut-tools.sqlite".to_owned());
  let path = write_c2d(&directory, "cut.c2d", &document);
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();

  shell.apply_action(&mut app, &UiAction::OpenFile { path: path.clone() });

  assert_eq!(
    shell.status, "open file — opened cut.c2d",
    "a library that loads adds nothing to the open's own sentence"
  );
  let (output, _) = toolpaths_frame(&mut shell, &mut app);
  assert!(
    output
      .drawn
      .tool_card
      .iter()
      .any(|line| line.starts_with("T4 - ")),
    "the card must resolve the tool the opened document's toolpath names: {:?}",
    output.drawn.tool_card
  );
  assert!(
    output
      .drawn
      .tool_card
      .iter()
      .any(|line| line.contains("Ø 6.00 mm")),
    "the card must show the diameter the library holds: {:?}",
    output.drawn.tool_card
  );
  assert!(
    output
      .drawn
      .cutting_time
      .starts_with("Estimated cutting time"),
    "the cutting-time line must stop saying the drawing has no tool: {:?}",
    output.drawn.cutting_time
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_missing_tool_library_does_not_cost_the_user_the_drawing() {
  let directory = scratch("open-tools-missing");
  let mut document = document_with_toolpath(2, 4);
  document.tools_ref = Some("gone.sqlite".to_owned());
  let path = write_c2d(&directory, "cut.c2d", &document);
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();

  shell.apply_action(&mut app, &UiAction::OpenFile { path: path.clone() });

  assert_eq!(
    app.document().entities().count(),
    2,
    "the drawing opened anyway: a library it cannot use is not a reason to refuse it"
  );
  assert_eq!(
    shell.document_path(),
    Some(path.as_path()),
    "the opened file is still the one the bar names"
  );
  assert!(
    shell.document_extras().is_empty(),
    "the opened archive carries no extras, and looking for a library must not invent any: {:?}",
    shell.document_extras()
  );
  assert_eq!(
    app.document().tools_ref.as_deref(),
    Some("gone.sqlite"),
    "the document is what the file holds, and the window writes nothing into it"
  );
  assert!(
    !directory.join("gone.sqlite").exists(),
    "looking at a library must never create one: ToolLibrary::open creates whatever it is pointed at"
  );
  assert!(
    shell.status.starts_with("open file — opened cut.c2d"),
    "the open's own sentence stays and the library's reason is added to it, got {:?}",
    shell.status
  );
  assert!(
    shell.status.contains("gone.sqlite") && shell.status.contains("no such file"),
    "the status must name the library and the reason, got {:?}",
    shell.status
  );
  let (output, _) = toolpaths_frame(&mut shell, &mut app);
  assert!(
    output
      .drawn
      .tool_card
      .iter()
      .any(|line| line.contains("T4 is not in the tool library")),
    "the snapshot stays empty, so the card keeps its own sentence: {:?}",
    output.drawn.tool_card
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_file_that_is_not_a_tool_library_does_not_cost_the_user_the_drawing() {
  let directory = scratch("open-tools-not-a-library");
  let notes = "not a database, just a note";
  std::fs::write(directory.join("notes.sqlite"), notes.as_bytes()).expect("text written");
  let mut document = document_with_toolpath(2, 4);
  document.tools_ref = Some("notes.sqlite".to_owned());
  let path = write_c2d(&directory, "cut.c2d", &document);
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();

  shell.apply_action(&mut app, &UiAction::OpenFile { path: path.clone() });

  assert_eq!(app.document().entities().count(), 2);
  assert!(
    !shell.status.starts_with("refused: "),
    "a file that is not a library is soft: the drawing opens, got {:?}",
    shell.status
  );
  assert!(
    shell.status.starts_with("open file — opened cut.c2d"),
    "the open's own sentence stays and the library's reason is added to it, got {:?}",
    shell.status
  );
  assert!(
    shell.status.contains("notes.sqlite") && shell.status.contains("cannot be used"),
    "the status must name the library and the reason, got {:?}",
    shell.status
  );
  assert_eq!(
    std::fs::read(directory.join("notes.sqlite")).expect("the file is still there"),
    notes.as_bytes(),
    "a file that is not a library is read, never rewritten"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn an_absolute_tools_ref_is_refused_and_creates_nothing() {
  let base = scratch("open-tools-absolute");
  let directory = base.join("drawings");
  std::fs::create_dir_all(&directory).expect("document directory");
  let outside = base.join("outside.sqlite");
  let mut document = document_with_toolpath(2, 4);
  document.tools_ref = Some(outside.display().to_string());
  let path = write_c2d(&directory, "cut.c2d", &document);
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();

  shell.apply_action(&mut app, &UiAction::OpenFile { path: path.clone() });

  assert!(
    shell.status.starts_with("open file — opened cut.c2d"),
    "a hostile reference is not a refused open: the drawing is still the user's, got {:?}",
    shell.status
  );
  assert!(
    shell.status.contains("outside.sqlite")
      && shell.status.contains("not inside the document's directory"),
    "the status must name the library and the rule that refused it, got {:?}",
    shell.status
  );
  assert!(
    !outside.exists(),
    "an absolute reference must be read nowhere, and a path nothing reads must not be created"
  );
  std::fs::remove_dir_all(&base).ok();
}

#[test]
fn a_tools_ref_that_climbs_out_of_the_directory_is_refused_and_creates_nothing() {
  let base = scratch("open-tools-parent");
  let directory = base.join("drawings");
  std::fs::create_dir_all(&directory).expect("document directory");
  let outside = base.join("outside.sqlite");
  let mut document = document_with_toolpath(2, 4);
  document.tools_ref = Some("../outside.sqlite".to_owned());
  let path = write_c2d(&directory, "cut.c2d", &document);
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();

  shell.apply_action(&mut app, &UiAction::OpenFile { path: path.clone() });

  assert!(
    shell.status.starts_with("open file — opened cut.c2d"),
    "a hostile reference is not a refused open: the drawing is still the user's, got {:?}",
    shell.status
  );
  assert!(
    shell.status.contains("../outside.sqlite")
      && shell.status.contains("not inside the document's directory"),
    "the status must name the library and the rule that refused it, got {:?}",
    shell.status
  );
  assert!(
    !outside.exists(),
    "a reference that leaves the directory must be read nowhere, and must create nothing there"
  );
  std::fs::remove_dir_all(&base).ok();
}

#[test]
fn a_symlink_to_an_empty_file_outside_is_not_written_through() {
  let base = scratch("open-tools-symlink");
  let directory = base.join("drawings");
  std::fs::create_dir_all(&directory).expect("document directory");
  // The file outside the document's directory already exists and is empty: the link resolves, so the
  // existence guard sees a library, and what happens to those zero bytes is the whole of this test.
  let outside = base.join("outside.sqlite");
  std::fs::write(&outside, b"").expect("empty file outside the directory");
  std::os::unix::fs::symlink(&outside, directory.join("linked.sqlite"))
    .expect("symlink beside the document");
  let mut document = document_with_toolpath(2, 4);
  document.tools_ref = Some("linked.sqlite".to_owned());
  let path = write_c2d(&directory, "cut.c2d", &document);
  let before = listing(&directory);
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();

  shell.apply_action(&mut app, &UiAction::OpenFile { path });

  assert_eq!(
    app.document().entities().count(),
    2,
    "the drawing opens: a library it cannot use costs it nothing"
  );
  assert_eq!(
    std::fs::metadata(&outside)
      .expect("the outside file is still there")
      .len(),
    0,
    "reading a tool library must never write one: a symlink beside the document may be followed for \
     reading, and the window must not bring the file outside it into being as a library"
  );
  assert_eq!(
    listing(&directory),
    before,
    "looking at a library must create nothing beside the document either"
  );
  assert!(
    shell.status.starts_with("open file — opened cut.c2d"),
    "the open's own sentence stays and the library's reason is added to it, got {:?}",
    shell.status
  );
  let (output, _) = toolpaths_frame(&mut shell, &mut app);
  assert!(
    output
      .drawn
      .tool_card
      .iter()
      .any(|line| line.contains("T4 is not in the tool library")),
    "the empty file holds no tool, so the card names the number it cannot resolve: {:?}",
    output.drawn.tool_card
  );
  std::fs::remove_dir_all(&base).ok();
}

// Covers shell-tools-ref S2: a reference into a subdirectory is resolved against the document's own
// directory, so a nested layout — the shape a shop actually uses — is not mistaken for a broken one.

#[test]
fn a_tools_ref_in_a_subdirectory_resolves_beside_the_document() {
  let directory = scratch("open-tools-nested");
  std::fs::create_dir_all(directory.join("parts/tools")).expect("nested directories");
  write_library(&directory.join("parts/tools/lib.sqlite"), 4);
  let mut document = document_with_toolpath(2, 4);
  document.tools_ref = Some("parts/tools/lib.sqlite".to_owned());
  let path = write_c2d(&directory, "cut.c2d", &document);
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();

  shell.apply_action(&mut app, &UiAction::OpenFile { path });

  assert_eq!(
    shell.status, "open file — opened cut.c2d",
    "a nested reference resolves against the document's own directory: {:?}",
    shell.status
  );
  std::fs::remove_dir_all(&directory).ok();
}

// Covers shell-tools-ref S3: a directory symlink beside the document is followed for reading — the
// shared-library arrangement L3 kept — and reading through it creates nothing outside, not even for a
// library that is not there.

#[test]
fn a_directory_symlink_beside_the_document_is_read_and_creates_nothing_outside() {
  let base = scratch("open-tools-dir-symlink");
  let outside = base.join("outside");
  std::fs::create_dir_all(&outside).expect("outside directory");
  let library = outside.join("tools.sqlite");
  write_library(&library, 4);
  let bytes = std::fs::read(&library).expect("read");
  let before = listing(&outside);
  let directory = base.join("drawings");
  std::fs::create_dir_all(&directory).expect("drawings");
  std::os::unix::fs::symlink(&outside, directory.join("sub")).expect("directory symlink");
  let mut document = document_with_toolpath(2, 4);
  document.tools_ref = Some("sub/tools.sqlite".to_owned());
  let path = write_c2d(&directory, "cut.c2d", &document);
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();

  shell.apply_action(&mut app, &UiAction::OpenFile { path });

  assert_eq!(
    std::fs::read(&library).expect("re-read"),
    bytes,
    "the library is read, never written"
  );
  assert_eq!(
    listing(&outside),
    before,
    "reading through the link creates nothing outside"
  );
  assert_eq!(
    shell.status, "open file — opened cut.c2d",
    "the lexical rule keeps the shared-library symlink working"
  );

  let mut missing = document_with_toolpath(2, 4);
  missing.tools_ref = Some("sub/new-lib.sqlite".to_owned());
  let absent = write_c2d(&directory, "missing.c2d", &missing);
  shell.apply_action(&mut app, &UiAction::OpenFile { path: absent });
  assert_eq!(
    listing(&outside),
    before,
    "a library that is not there is not created through the link"
  );
  assert!(
    shell.status.contains("holds no such file"),
    "got {:?}",
    shell.status
  );
  std::fs::remove_dir_all(&base).ok();
}

#[test]
fn a_reference_that_climbs_back_into_the_directory_is_still_refused() {
  let directory = scratch("open-tools-climb-back");
  std::fs::create_dir_all(directory.join("sub")).expect("subdirectory");
  let library = directory.join("tools.sqlite");
  write_library(&library, 4);
  let bytes = std::fs::read(&library).expect("the library is there");
  let mut document = document_with_toolpath(2, 4);
  document.tools_ref = Some("sub/../tools.sqlite".to_owned());
  let path = write_c2d(&directory, "cut.c2d", &document);
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();

  shell.apply_action(&mut app, &UiAction::OpenFile { path });

  assert!(
    shell.status.starts_with("open file — opened cut.c2d"),
    "a reference that lands back inside is soft, not a refused open, got {:?}",
    shell.status
  );
  assert!(
    shell.status.contains("sub/../tools.sqlite")
      && shell.status.contains("not inside the document's directory"),
    "the status must name the reference and the rule that refused it, got {:?}",
    shell.status
  );
  assert_eq!(
    std::fs::read(&library).expect("the library is still there"),
    bytes,
    "a refused reference is read nowhere, so the library it lands on is untouched"
  );
  let (output, _) = toolpaths_frame(&mut shell, &mut app);
  assert!(
    !output
      .drawn
      .tool_card
      .iter()
      .any(|line| line.starts_with("T4 - ")),
    "a refused reference is not read, so the card must not resolve the tool it names: {:?}",
    output.drawn.tool_card
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn an_unusable_tools_ref_is_soft_and_creates_nothing() {
  let directory = scratch("open-tools-unusable");
  let locked = directory.join("locked.sqlite");
  write_library(&locked, 4);
  let mut permissions = std::fs::metadata(&locked)
    .expect("the library is there")
    .permissions();
  std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o000);
  std::fs::set_permissions(&locked, permissions).expect("the library is locked");
  let mut document = document_with_toolpath(2, 4);
  let path = write_c2d(&directory, "cut.c2d", &document);

  // The reason differs by case and must: a reference that names nothing is reported by the guard that
  // names the reference, and one that names something this build cannot read by the store's own sentence.
  for (reference, refusal) in [
    ("locked.sqlite", "cannot be used"),
    ("", "cannot be used"),
    (".", "cannot be used"),
    ("missing.sqlite", "holds no such file"),
  ] {
    let before = listing(&directory);
    document.tools_ref = Some(reference.to_owned());
    save_document(&path, &document).expect("document written");
    let mut app = AppState::new(document_with_rectangle());
    let mut shell = Shell::new();

    shell.apply_action(&mut app, &UiAction::OpenFile { path: path.clone() });

    assert_eq!(
      app.document().entities().count(),
      2,
      "{reference:?}: the drawing opens whatever the library does"
    );
    assert!(
      !shell.status.starts_with("refused: "),
      "{reference:?}: a library that cannot be used is soft, got {:?}",
      shell.status
    );
    assert!(
      shell.status.starts_with("open file — opened cut.c2d — "),
      "{reference:?}: the open's own sentence stays and the library's reason is added to it, got {:?}",
      shell.status
    );
    assert!(
      shell.status.contains("tool library") && shell.status.contains(refusal),
      "{reference:?}: the status must name the library and the reason, got {:?}",
      shell.status
    );
    let reason = shell
      .status
      .split_once(" used: ")
      .map(|(_, reason)| reason)
      .unwrap_or_default();
    assert!(
      !reason.trim().is_empty(),
      "{reference:?}: the status must say why, got {:?}",
      shell.status
    );
    assert_eq!(
      listing(&directory),
      before,
      "{reference:?}: looking at a library must create nothing beside the document"
    );
    let (output, _) = toolpaths_frame(&mut shell, &mut app);
    assert!(
      !output
        .drawn
        .tool_card
        .iter()
        .any(|line| line.starts_with("T4 - ")),
      "{reference:?}: an unusable library leaves the snapshot empty: {:?}",
      output.drawn.tool_card
    );
  }
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn an_open_after_a_resolved_library_leaves_no_card_from_the_previous_document() {
  let directory = scratch("open-tools-stale-card");
  write_library(&directory.join("tools.sqlite"), 4);
  let mut resolved = document_with_toolpath(2, 4);
  resolved.tools_ref = Some("tools.sqlite".to_owned());
  let a = write_c2d(&directory, "a.c2d", &resolved);
  let mut unresolved = document_with_toolpath(1, 4);
  unresolved.tools_ref = Some("gone.sqlite".to_owned());
  let b = write_c2d(&directory, "b.c2d", &unresolved);
  let c = write_c2d(&directory, "c.c2d", &document_with_toolpath(1, 4));
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();

  shell.apply_action(&mut app, &UiAction::OpenFile { path: a });
  let (output, _) = toolpaths_frame(&mut shell, &mut app);
  assert!(
    output
      .drawn
      .tool_card
      .iter()
      .any(|line| line.starts_with("T4 - ")),
    "the fixture must start with a resolved tool: {:?}",
    output.drawn.tool_card
  );

  for path in [b, c] {
    shell.apply_action(&mut app, &UiAction::OpenFile { path });
    let (output, _) = toolpaths_frame(&mut shell, &mut app);
    assert!(
      !output
        .drawn
        .tool_card
        .iter()
        .any(|line| line.starts_with("T4 - ")),
      "the previous document's tool must not survive into this one: {:?}",
      output.drawn.tool_card
    );
    assert!(
      output
        .drawn
        .tool_card
        .iter()
        .any(|line| line.contains("T4 is not in the tool library")),
      "the card names the number it cannot resolve: {:?}",
      output.drawn.tool_card
    );
  }
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_document_with_no_tools_ref_keeps_todays_open_sentence_and_an_empty_snapshot() {
  let directory = scratch("open-tools-none");
  let path = write_c2d(&directory, "cut.c2d", &document_with_toolpath(2, 4));
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  // A snapshot this window was handed before the open, so "empty" is a claim about the open itself.
  shell.set_tools(vec![stored_tool(4)]);

  shell.apply_action(&mut app, &UiAction::OpenFile { path: path.clone() });

  assert_eq!(
    shell.status, "open file — opened cut.c2d",
    "a document with no tools_ref opens in exactly the sentence it printed before the field was \
     resolved at all"
  );
  let (output, _) = toolpaths_frame(&mut shell, &mut app);
  assert!(
    output
      .drawn
      .tool_card
      .iter()
      .any(|line| line.contains("T4 is not in the tool library")),
    "the snapshot is empty, so the card names the number it cannot resolve: {:?}",
    output.drawn.tool_card
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_header_that_does_not_declare_wal_still_creates_nothing_through_a_directory_symlink() {
  let base = scratch("open-tools-header-bypass");
  let outside = base.join("outside");
  std::fs::create_dir_all(&outside).expect("outside directory");
  let library = outside.join("crafted.sqlite");
  write_library(&library, 4);
  {
    let mut bytes = std::fs::read(&library).expect("read");
    bytes[18] = 1;
    bytes[19] = 1;
    std::fs::write(&library, &bytes).expect("header rewritten");
  }
  std::fs::write(outside.join("crafted.sqlite-wal"), [0xABu8; 4096]).expect("log");
  let before = listing(&outside);
  let directory = base.join("drawings");
  std::fs::create_dir_all(&directory).expect("drawings");
  std::os::unix::fs::symlink(&outside, directory.join("sub")).expect("directory symlink");
  let mut document = document_with_toolpath(2, 4);
  document.tools_ref = Some("sub/crafted.sqlite".to_owned());
  let path = write_c2d(&directory, "cut.c2d", &document);
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();

  shell.apply_action(&mut app, &UiAction::OpenFile { path });

  assert_eq!(
    listing(&outside),
    before,
    "a header the guard does not flag must not make the window create anything outside"
  );
  std::fs::remove_dir_all(&base).ok();
}

// Covers shell-tools-ref S3: the guard asks about the file SQLite will open, not the name the
// document wrote. A file symlink beside the document is followed by SQLite, so a non-empty -wal
// beside the *target* is what decides whether reading would create a companion — and it must not.

#[test]
fn a_file_symlink_beside_the_document_still_creates_nothing_outside() {
  let base = scratch("open-tools-file-symlink");
  let outside = base.join("outside");
  std::fs::create_dir_all(&outside).expect("outside directory");
  let real = outside.join("real.sqlite");
  write_library(&real, 4);
  {
    let mut bytes = std::fs::read(&real).expect("read");
    bytes[18] = 1;
    bytes[19] = 1;
    std::fs::write(&real, &bytes).expect("header rewritten");
  }
  std::fs::write(outside.join("real.sqlite-wal"), [0xABu8; 4096]).expect("log");
  let before = listing(&outside);
  let directory = base.join("drawings");
  std::fs::create_dir_all(&directory).expect("drawings");
  std::os::unix::fs::symlink(&real, directory.join("link.sqlite")).expect("file symlink");
  let mut document = document_with_toolpath(2, 4);
  document.tools_ref = Some("link.sqlite".to_owned());
  let path = write_c2d(&directory, "cut.c2d", &document);
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();

  shell.apply_action(&mut app, &UiAction::OpenFile { path });

  assert_eq!(
    listing(&outside),
    before,
    "nothing is created outside the document's directory"
  );
  std::fs::remove_dir_all(&base).ok();
}

// ---- The right panel as a ribbon and a task editor (camui-prancheta S6, T3) ---------------------
