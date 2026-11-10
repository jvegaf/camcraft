use cam_app::AppState;
use cam_ui::Shell;

use super::support::*;

// ---------------------------------------------------------------------------------------------
// S3: the three rows of library actions at the foot of the master column
// ---------------------------------------------------------------------------------------------

/// The six words the rows carry, in the order and grouping S3 names them.
const ROWS: [&str; 6] = [
  "+ New",
  "Duplicate",
  "Delete",
  "Export…",
  "Import…",
  "Restore defaults",
];

/// Where one pressable word was drawn, in the order the modal reported its hits.
fn point_of(drawn: &cam_ui::ShellOutput, word: &str) -> (f32, f32) {
  drawn
    .drawn
    .tool_library_hits
    .iter()
    .find(|(name, _)| name == word)
    .map(|(_, point)| *point)
    .unwrap_or_else(|| {
      panic!(
        "nothing called {word:?} is pressable in the modal: {:?}",
        drawn.drawn.tool_library_hits
      )
    })
}

#[test]
fn the_master_column_ends_in_three_rows_of_library_actions() {
  let scratch = Scratch::new("rows");
  let ctx = egui::Context::default();
  let (_app, _shell, opened, _library_path) = opened_library_document(&ctx, &scratch);

  // S3's order, exactly: three rows, and no seventh word among them.
  assert_eq!(
    opened.drawn.tool_actions, ROWS,
    "the master column's action rows are not S3's three groups in S3's order"
  );

  // Every one of them is a control the operator can press: a library is on disk and a tool is selected,
  // which is the state in which all six can do what they say.
  let pressable = hits(&opened);
  for word in ROWS {
    assert!(
      pressable.contains(&word),
      "{word:?} was drawn and cannot be pressed, though this library can honour it: {pressable:?}"
    );
  }

  // And the grouping is a claim about the picture, not about the list's order: the first three share one
  // row, the two file dialogs share the next, and `Restore defaults` stands alone below them.
  let new = point_of(&opened, "+ New");
  let duplicate = point_of(&opened, "Duplicate");
  let delete = point_of(&opened, "Delete");
  let export = point_of(&opened, "Export…");
  let import = point_of(&opened, "Import…");
  let restore = point_of(&opened, "Restore defaults");
  assert_eq!(
    [new.1, duplicate.1, delete.1],
    [new.1; 3],
    "`+ New`, `Duplicate` and `Delete` are not on one row: {new:?} {duplicate:?} {delete:?}"
  );
  assert_eq!(
    export.1, import.1,
    "`Export…` and `Import…` are not on one row: {export:?} {import:?}"
  );
  assert!(
    new.0 < duplicate.0 && duplicate.0 < delete.0,
    "`+ New`, `Duplicate` and `Delete` are not left to right: {new:?} {duplicate:?} {delete:?}"
  );
  assert!(
    export.0 < import.0,
    "`Export…` is not left of `Import…`: {export:?} {import:?}"
  );
  assert!(
    export.1 > delete.1 && restore.1 > import.1,
    "the three rows are not stacked top to bottom: {delete:?} {export:?} {restore:?}"
  );
}

#[test]
fn with_no_library_at_all_the_four_writers_are_disabled_and_one_sentence_says_why() {
  let scratch = Scratch::new("rows-no-library");
  let ctx = egui::Context::default();
  // A window that was handed a snapshot and never opened a library: the application's library is the only one
  // the six rows can write (tool-library-app S1), so a bare snapshot leaves them nothing to write through. The
  // rows are drawn — they are what teaches the operator the action exists — and four of them are dark.
  let path = write_document(scratch.path(), "part.c2d", &document_with_toolpath(1));
  let mut app = AppState::new(document_with_toolpath(1));
  let mut shell = Shell::new();
  shell.apply_action(&mut app, &cam_ui::UiAction::OpenFile { path });
  // The snapshot still lists the six tools, so the tree has a selection: the reason the four are dark is the
  // missing library and nothing else (S10).
  shell.set_tools(library());
  let opened = open_library(&ctx, &mut shell, &mut app);

  assert_eq!(
    opened.drawn.tool_actions, ROWS,
    "the actions are not named when they cannot be pressed, so nothing teaches what a library would enable"
  );
  let pressable = hits(&opened);
  for word in ["+ New", "Duplicate", "Delete", "Restore defaults"] {
    assert!(
      !pressable.contains(&word),
      "{word:?} is pressable with no library file, so it claims a write it cannot do: {pressable:?}"
    );
  }
  // `Export…` and `Import…` need a file dialog, not a library, so they stay pressable always (S11).
  for word in ["Export…", "Import…"] {
    assert!(
      pressable.contains(&word),
      "{word:?} was disabled, though it needs a dialog and not a library: {pressable:?}"
    );
  }

  // One dim sentence, under the rows: the application's own words about a library it could not open. No
  // directory is named, because no drawing owns the library any more, so there is no drawing's directory to name.
  let sentence = opened
    .drawn
    .tool_library
    .iter()
    .find(|word| word.contains("could not open its tool library"))
    .unwrap_or_else(|| {
      panic!(
        "nothing explains why the four rows are dark: {:?}",
        opened.drawn.tool_library
      )
    })
    .clone();
  let (_, texts) = frame_with(&ctx, &mut shell, &mut app);
  assert!(
    joined(&texts).contains(&sentence),
    "the refusal was reported and never painted: {}",
    joined(&texts)
  );
}

// ---------------------------------------------------------------------------------------------
// S10: the four rows that write the library
// ---------------------------------------------------------------------------------------------

/// The geometry ids one set of tools carries, in the order it holds them.
fn ids(tools: &[cam_store::tools::StoredTool]) -> Vec<String> {
  tools.iter().map(|tool| tool.geometry.id.clone()).collect()
}

#[test]
fn new_appends_a_tool_with_the_lowest_free_number_and_writes_it() {
  let scratch = Scratch::new("new");
  let ctx = egui::Context::default();
  // A library that already holds a tool called `tool-1`, so "the lowest number nothing uses" is a claim about
  // the ids on hand and not about counting to one.
  let mut tools = library();
  tools.push(flat_mill("tool-1", Some(9), 3.0));
  let library_path = write_library(scratch.path(), "tools.sqlite", &tools);
  let path = write_document(scratch.path(), "part.c2d", &document_with_toolpath(1));
  let mut app = AppState::new(document_with_toolpath(1));
  let mut shell = Shell::new();
  shell
    .open_library_at(scratch.path())
    .expect("the application's library opens");
  shell.apply_action(&mut app, &cam_ui::UiAction::OpenFile { path });
  let opened = open_library(&ctx, &mut shell, &mut app);
  assert_eq!(opened.drawn.tool_selected.as_deref(), Some("G1"));
  assert_eq!(opened.drawn.tool_rows.len(), tools.len());

  let (pressed, _) = click_word(&ctx, &mut shell, &mut app, &opened, "+ New");
  assert_eq!(
    pressed.drawn.tool_saved,
    vec!["tool-2".to_owned()],
    "`+ New` did not write the fresh tool: it took an id `tool-1` already answers to"
  );
  assert_eq!(pressed.drawn.tool_error, None);
  assert!(
    shell.show_tool_library,
    "`+ New` closed the modal, so the tool it made cannot be edited"
  );

  // The new row is the selection, so the form describes what was just made.
  let (after, _) = frame_with(&ctx, &mut shell, &mut app);
  assert_eq!(after.drawn.tool_selected.as_deref(), Some("tool-2"));
  assert_eq!(after.drawn.tool_rows.len(), tools.len() + 1);
  assert!(
    after
      .drawn
      .tool_rows
      .iter()
      .any(|(_, title, _)| title == "T7 - New tool"),
    "the new tool is not in the tree under its lowest free number: {:?}",
    after.drawn.tool_rows
  );

  // And the file, not only the window, holds it.
  let written = tools_in(&library_path);
  assert_eq!(written.len(), tools.len() + 1);
  let fresh = written
    .iter()
    .find(|tool| tool.geometry.id == "tool-2")
    .expect("the library file holds no tool called tool-2");
  assert_eq!(fresh.cutting.tool_number, Some(7));
  assert_eq!(cam_store::tool_edit::rendered_name(fresh), "New tool");
}

#[test]
fn duplicate_copies_the_shape_and_names_the_copy() {
  let scratch = Scratch::new("duplicate");
  let ctx = egui::Context::default();
  let (mut app, mut shell, opened, library_path) = opened_library_document(&ctx, &scratch);
  assert_eq!(opened.drawn.tool_selected.as_deref(), Some("G1"));

  let (pressed, _) = click_word(&ctx, &mut shell, &mut app, &opened, "Duplicate");
  assert_eq!(
    pressed.drawn.tool_saved,
    vec!["tool-1".to_owned()],
    "`Duplicate` did not write the copy"
  );
  assert_eq!(pressed.drawn.tool_error, None);

  let (after, _) = frame_with(&ctx, &mut shell, &mut app);
  assert_eq!(
    after.drawn.tool_selected.as_deref(),
    Some("tool-1"),
    "the copy was written and not selected, so the operator edits the original"
  );
  assert!(
    after
      .drawn
      .tool_rows
      .iter()
      .any(|(_, title, _)| title.contains("(copy)")),
    "the copy is not in the tree: {:?}",
    after.drawn.tool_rows
  );

  let original = library()
    .into_iter()
    .find(|tool| tool.geometry.id == "G1")
    .expect("the fixture holds G1");
  let copy = tools_in(&library_path)
    .into_iter()
    .find(|tool| tool.geometry.id == "tool-1")
    .expect("the library file holds no copy");
  assert_eq!(
    cam_store::tool_edit::rendered_name(&copy),
    format!("{} (copy)", cam_store::tool_edit::rendered_name(&original)),
    "the copy is not named after the tool it copies"
  );
  assert_eq!(copy.geometry.diameter, original.geometry.diameter);
  assert_eq!(copy.geometry.num_flutes, original.geometry.num_flutes);
  assert_eq!(copy.geometry.tool_type, original.geometry.tool_type);
  assert_ne!(
    copy.cutting.id, original.cutting.id,
    "the copy shares the original's cutting data, so editing one edits both"
  );
  assert_eq!(copy.cutting.tool_number, Some(7));
}

#[test]
fn delete_removes_the_selected_tool_and_leaves_no_row_behind() {
  let scratch = Scratch::new("delete");
  let ctx = egui::Context::default();
  let (mut app, mut shell, opened, library_path) = opened_library_document(&ctx, &scratch);
  let rows = opened.drawn.tool_rows.clone();
  let doomed = rows
    .iter()
    .find(|(_, title, _)| title.starts_with("T1 - "))
    .expect("the tree lists G1")
    .clone();

  let (pressed, _) = click_word(&ctx, &mut shell, &mut app, &opened, "Delete");
  assert_eq!(
    pressed.drawn.tool_saved,
    vec!["G1".to_owned()],
    "`Delete` did not write the removal"
  );
  assert_eq!(pressed.drawn.tool_error, None);
  assert!(shell.show_tool_library, "`Delete` closed the modal");

  let (after, _) = frame_with(&ctx, &mut shell, &mut app);
  assert_eq!(
    after.drawn.tool_selected, None,
    "the deleted tool is still the selection"
  );
  assert_eq!(after.drawn.tool_rows.len(), rows.len() - 1);
  assert!(
    !after.drawn.tool_rows.contains(&doomed),
    "the tree still lists {doomed:?}: {:?}",
    after.drawn.tool_rows
  );
  let written = tools_in(&library_path);
  assert_eq!(written.len(), rows.len() - 1);
  assert!(
    !written.iter().any(|tool| tool.geometry.id == "G1"),
    "the library file still holds G1"
  );
}

#[test]
fn restore_defaults_installs_exactly_the_factory_set() {
  let scratch = Scratch::new("restore");
  let ctx = egui::Context::default();
  let (mut app, mut shell, opened, library_path) = opened_library_document(&ctx, &scratch);
  assert_eq!(opened.drawn.tool_rows.len(), library().len());

  let factory = cam_store::tool_edit::factory_tools();
  let (pressed, _) = click_word(&ctx, &mut shell, &mut app, &opened, "Restore defaults");
  assert_eq!(
    pressed.drawn.tool_saved,
    ids(&factory),
    "`Restore defaults` did not report the set it wrote"
  );
  assert_eq!(pressed.drawn.tool_error, None);

  let (after, _) = frame_with(&ctx, &mut shell, &mut app);
  assert_eq!(after.drawn.tool_rows.len(), factory.len());
  assert_eq!(
    after.drawn.tool_selected, None,
    "a tool from the library that was replaced is still the selection"
  );

  let written = tools_in(&library_path);
  let mut got = ids(&written);
  got.sort();
  let mut want = ids(&factory);
  want.sort();
  assert_eq!(
    got, want,
    "the file holds a different set than the factory's"
  );
  for tool in &factory {
    let stored = written
      .iter()
      .find(|stored| stored.geometry.id == tool.geometry.id)
      .expect("a factory tool is missing from the file");
    assert_eq!(stored.cutting.tool_number, tool.cutting.tool_number);
    assert_eq!(
      cam_store::tool_edit::rendered_name(stored),
      cam_store::tool_edit::rendered_name(tool)
    );
  }
}

#[test]
fn a_write_that_cannot_open_the_library_changes_nothing() {
  let scratch = Scratch::new("delete-unwritable");
  let ctx = egui::Context::default();
  let (mut app, mut shell, opened, library_path) = opened_library_document(&ctx, &scratch);
  let before = opened.drawn.tool_rows.clone();
  // The library stopped being a library between the open and the press — a file another process is writing.
  std::fs::write(&library_path, b"this is not a database").expect("the library is corrupted");

  let (pressed, _) = click_word(&ctx, &mut shell, &mut app, &opened, "+ New");
  assert_eq!(
    pressed.drawn.tool_saved,
    Vec::<String>::new(),
    "a refused write reported itself as a write"
  );
  let sentence = pressed
    .drawn
    .tool_error
    .clone()
    .expect("a refused write says why");
  assert!(
    sentence.contains("cannot be used"),
    "the sentence must name what could not be used: {sentence:?}"
  );
  assert!(
    shell.show_tool_library,
    "a refused write closed the modal, which loses the tree the operator was working in"
  );

  // The snapshot is untouched, which is the half that a write-then-mutate would have got wrong.
  let (after, texts) = frame_with(&ctx, &mut shell, &mut app);
  assert_eq!(
    after.drawn.tool_rows, before,
    "the snapshot gained a tool the library refused"
  );
  assert_eq!(after.drawn.tool_selected.as_deref(), Some("G1"));
  assert!(
    joined(&texts).contains(&sentence),
    "the refusal was reported and never painted: {}",
    joined(&texts)
  );
}

// ---------------------------------------------------------------------------------------------
// S11: `Export…` and `Import…`, through the host's file dialogs
// ---------------------------------------------------------------------------------------------

#[test]
fn export_writes_a_file_the_store_reads_back() {
  let scratch = Scratch::new("export");
  let ctx = egui::Context::default();
  let (mut app, mut shell, opened, _library_path) = opened_library_document(&ctx, &scratch);
  let export_path = scratch.path().join("tools.json");

  // The row asks the host for a path and writes nothing itself: `cam-ui` has no file dialog.
  let (asked, _) = click_word(&ctx, &mut shell, &mut app, &opened, "Export…");
  assert_eq!(asked.actions, vec![cam_ui::UiAction::RequestExportTools]);
  assert!(
    !export_path.exists(),
    "the press wrote a file the operator never named"
  );

  shell.apply_action(
    &mut app,
    &cam_ui::UiAction::ExportTools {
      path: export_path.clone(),
    },
  );
  let text = std::fs::read_to_string(&export_path).expect("the export wrote no file");
  let read = cam_store::tool_file::from_json(&text).expect("the store cannot read its own file");
  assert_eq!(ids(&read), ids(&library()));
  for tool in &library() {
    let stored = read
      .iter()
      .find(|stored| stored.geometry.id == tool.geometry.id)
      .expect("a tool went missing from the export");
    assert_eq!(stored.cutting.tool_number, tool.cutting.tool_number);
    assert_eq!(
      cam_store::tool_edit::rendered_name(stored),
      cam_store::tool_edit::rendered_name(tool)
    );
  }

  // The write reaches the report on the next frame, so an export is visible to a test that reads the modal.
  let (after, _) = frame_with(&ctx, &mut shell, &mut app);
  assert_eq!(after.drawn.tool_saved, ids(&library()));
  assert_eq!(after.drawn.tool_error, None);
}

#[test]
fn import_asks_and_replace_all_replaces_the_library() {
  let scratch = Scratch::new("import-replace");
  let ctx = egui::Context::default();
  let (mut app, mut shell, opened, library_path) = opened_library_document(&ctx, &scratch);
  assert_eq!(opened.drawn.tool_rows.len(), library().len());
  let incoming = vec![flat_mill("X1", Some(1), 3.0), measured("X2", Some(2))];
  let file = scratch.path().join("incoming.json");
  let json = cam_store::tool_file::to_json(&incoming).expect("the fixture is a library");
  std::fs::write(&file, &json).expect("the file is written");
  let before = ids(&tools_in(&library_path));

  shell.apply_action(
    &mut app,
    &cam_ui::UiAction::ImportTools { path: file.clone() },
  );

  // A valid file stages the question and changes nothing yet (tool-library-app S5).
  let _ = frame_with(&ctx, &mut shell, &mut app);
  let (settled, _) = frame_with(&ctx, &mut shell, &mut app);
  assert_eq!(ids(&tools_in(&library_path)), before);
  assert_eq!(
    settled.drawn.tool_import_question_title.as_deref(),
    Some("Import tool library")
  );
  let at = settled
    .drawn
    .tool_import_question_hits
    .iter()
    .find(|(word, _)| word == "Replace all")
    .map(|(_, point)| egui::pos2(point.0, point.1))
    .expect("Replace all is pressable");
  click_at(&ctx, &mut shell, &mut app, at);

  // The library file beside the document is what changed, and the whole of it.
  let written = tools_in(&library_path);
  let mut got = ids(&written);
  got.sort();
  let mut want = ids(&incoming);
  want.sort();
  assert_eq!(
    got, want,
    "the library file does not hold the imported tools"
  );

  // And the window's own library is the one that was just read.
  let (after, _) = frame_with(&ctx, &mut shell, &mut app);
  assert_eq!(after.drawn.tool_rows.len(), incoming.len());
  assert_eq!(
    after.drawn.tool_selected, None,
    "a tool from the library that was replaced is still the selection"
  );
  assert_eq!(after.drawn.tool_error, None);
  assert_eq!(after.drawn.tool_saved, ids(&incoming));
}

#[test]
fn an_import_that_cannot_be_read_changes_nothing() {
  let scratch = Scratch::new("import-refused");
  let ctx = egui::Context::default();
  let (mut app, mut shell, _opened, library_path) = opened_library_document(&ctx, &scratch);
  let before = ids(&tools_in(&library_path));
  let file = scratch.path().join("not-a-library.json");
  std::fs::write(
    &file,
    "{\"format\": \"something else\", \"version\": 1, \"tools\": []}",
  )
  .expect("the file is written");

  shell.apply_action(&mut app, &cam_ui::UiAction::ImportTools { path: file });

  let (after, texts) = frame_with(&ctx, &mut shell, &mut app);
  let sentence = after
    .drawn
    .tool_error
    .clone()
    .expect("a refused import says why");
  assert!(
    sentence.contains("not a tool library"),
    "the sentence must say what the file is not: {sentence:?}"
  );
  assert!(
    after.drawn.tool_saved.is_empty(),
    "a refused import reported itself as a write"
  );
  assert_eq!(
    after.drawn.tool_rows.len(),
    before.len(),
    "the snapshot lost tools to an import that failed"
  );
  assert_eq!(ids(&tools_in(&library_path)), before);
  assert!(
    joined(&texts).contains(&sentence),
    "the refusal was reported and never painted: {}",
    joined(&texts)
  );
}
