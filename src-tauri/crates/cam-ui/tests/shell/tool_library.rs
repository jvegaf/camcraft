//! The tool library is the **application's**, not a document's (tool-library-app S1).
//!
//! Four states a window can be in about it, and the one refusal that is left: a fresh application with no
//! library file yet, an application whose library is already there and must survive a document opening over it,
//! a library a person emptied and expects to stay empty, and a directory that cannot hold a library at all.
//!
//! What the previous rule pinned — a `tools_ref` resolved from beside the drawing, and the whole containment,
//! symlink and WAL ladder that came with it — went with the rule: opening a document no longer reads a library,
//! so there is nothing there to resolve. `cam-cli` keeps its own resolver and its own tests for `tools_ref`, and
//! the document model still carries the field.

use super::support::*;
use cam_app::AppState;
use cam_store::tool_edit::factory_tools;
use cam_store::tools::ToolLibrary;
use cam_ui::tool_library::library_path_in;
use cam_ui::{Shell, UiAction};

/// A real library file holding one tool nothing else in the test owns, written the way `cam-cli` writes one.
fn write_foreign_library(path: &std::path::Path, tool_number: u32) {
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

fn ids(shell: &Shell) -> Vec<String> {
  shell
    .tools()
    .iter()
    .map(|tool| tool.geometry.id.clone())
    .collect()
}

#[test]
fn a_fresh_application_writes_the_factory_catalogue_into_its_data_directory() {
  let directory = scratch("app-library-fresh");
  let mut shell = Shell::new();

  let path = shell
    .open_library_at(&directory)
    .expect("the library opens");

  assert_eq!(path, library_path_in(&directory));
  assert!(path.exists(), "the first launch creates the library file");
  let mut expected: Vec<String> = factory_tools()
    .iter()
    .map(|tool| tool.geometry.id.clone())
    .collect();
  let mut seeded = ids(&shell);
  // The library answers in its own order and the catalogue in tool-number order, so the claim is about the
  // set: one tool per factory tool, and nothing else.
  expected.sort();
  seeded.sort();
  assert_eq!(
    seeded, expected,
    "a fresh application starts with the factory catalogue"
  );
  assert_eq!(
    shell.tool_library_path(),
    Some(path.as_path()),
    "the window knows the file it just created, so the six action rows can write through it"
  );
}

#[test]
fn a_library_that_is_already_there_is_read_and_not_reseeded() {
  let directory = scratch("app-library-existing");
  let path = library_path_in(&directory);
  write_foreign_library(&path, 6);
  let mut shell = Shell::new();

  shell
    .open_library_at(&directory)
    .expect("the library opens");

  assert_eq!(
    ids(&shell),
    vec!["G6".to_owned()],
    "an application with a library reads it as it is: seeding is a first-launch answer"
  );
}

#[test]
fn opening_a_document_leaves_the_applications_library_alone() {
  let directory = scratch("app-library-survives-open");
  let mut shell = Shell::new();
  shell
    .open_library_at(&directory)
    .expect("the library opens");
  let before = ids(&shell);

  // A drawing that names a library of its own, holding a tool the application's does not: the field is still
  // in the document model, so the test proves the open ignores it rather than that nobody could write it.
  let foreign = directory.join("cut-tools.sqlite");
  write_foreign_library(&foreign, 99);
  let mut document = document_with_toolpath(2, 99);
  document.tools_ref = Some("cut-tools.sqlite".to_owned());
  let path = write_c2d(&directory, "cut.cmc", &document);
  let mut app = AppState::new(document_with_toolpath(2, 99));

  shell.apply_action(&mut app, &UiAction::OpenFile { path });

  assert_eq!(
    ids(&shell),
    before,
    "a document neither replaces the library nor adds its own tools to it"
  );
  let expected_path = library_path_in(&directory);
  assert_eq!(
    shell.tool_library_path(),
    Some(expected_path.as_path()),
    "the window still writes the application's file, not the one the drawing named"
  );
  let (_output, words) = frame(&mut shell, &mut app);
  let card = words.join("\n");
  assert!(
    !card.contains(&stored_tool(99).geometry.name_format) || card.contains("not in"),
    "the drawing's library is not read: {card}"
  );
}

#[test]
fn a_library_a_person_emptied_is_not_reseeded() {
  let directory = scratch("app-library-emptied");
  let mut shell = Shell::new();
  let path = shell
    .open_library_at(&directory)
    .expect("the library opens");
  ToolLibrary::open(&path)
    .expect("the file opens")
    .replace_tools(&[])
    .expect("every tool removed");

  shell
    .open_library_at(&directory)
    .expect("the library opens");

  assert!(
    shell.tools().is_empty(),
    "a library a person emptied stays empty: restoring the defaults is one press, not a surprise"
  );
}

#[test]
fn a_data_directory_that_cannot_hold_a_library_is_a_sentence_and_not_a_panic() {
  let directory = scratch("app-library-unwritable");
  let occupied = directory.join("not-a-directory");
  std::fs::write(&occupied, b"a file, not a directory").expect("the file is written");
  let mut shell = Shell::new();

  let refused = shell.open_library_at(&occupied);

  let sentence = refused.expect_err("a path that is a file cannot hold a library");
  assert!(
    !sentence.is_empty(),
    "a refusal is a sentence the modal can draw, not a silent no-op"
  );
  assert!(
    shell.tools().is_empty(),
    "a library that could not be opened leaves no snapshot behind"
  );
}
