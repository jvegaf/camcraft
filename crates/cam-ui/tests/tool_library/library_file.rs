use cam_app::AppState;
use cam_store::tools::ToolLibrary;
use cam_ui::Shell;

use super::support::*;

// ---------------------------------------------------------------------------------------------
// S10: `Use this tool` writes through the library file beside the document
// ---------------------------------------------------------------------------------------------

#[test]
fn use_this_tool_writes_the_edited_tool_through_the_library_file() {
  let scratch = Scratch::new("write");
  let ctx = egui::Context::default();
  let (mut app, mut shell, opened, library_path) = opened_library_document(&ctx, &scratch);
  // The modal opens on the tool the opened document's toolpath cuts with, out of the file it named.
  assert_eq!(opened.drawn.tool_selected.as_deref(), Some("G1"));
  assert_eq!(opened.drawn.tool_rows.len(), library().len());

  // Edit the number the program names the tool by, and commit it.
  let (_, runs) = frame_with_runs(&ctx, &mut shell, &mut app);
  let (typed, _) = type_into_box(&ctx, &mut shell, &mut app, &runs, "Tool number", "7");
  assert_eq!(value_of(&typed, "MACHINE", "Tool number"), "7");
  assert!(
    typed.drawn.tool_saved.is_empty(),
    "a keystroke wrote the library: {:?}",
    typed.drawn.tool_saved
  );

  let (wrote, _) = click_word(&ctx, &mut shell, &mut app, &typed, "Use this tool");
  assert_eq!(
    wrote.drawn.tool_saved,
    vec!["G1".to_owned()],
    "`Use this tool` did not write the edited tool through the library file"
  );
  assert_eq!(wrote.drawn.tool_error, None);
  assert_eq!(
    wrote.actions,
    vec![cam_ui::UiAction::SetToolpathToolNumber {
      toolpath_id: "P1".to_owned(),
      tool_number: 7,
    }],
    "the commit wrote the tool and did not retool the toolpath it was opened for"
  );

  // The file, not the window's snapshot, is what holds the edit.
  let reopened = ToolLibrary::open_read_only(&library_path).expect("the library is readable");
  let tools = reopened.tools().expect("the library lists its tools");
  let written = tools
    .iter()
    .find(|tool| tool.geometry.id == "G1")
    .expect("the library still holds G1");
  assert_eq!(
    written.cutting.tool_number,
    Some(7),
    "the file still holds the number the tool had before the edit"
  );
  assert_eq!(
    tools.len(),
    library().len(),
    "the write added or dropped a tool: {tools:?}"
  );
}

#[test]
fn a_library_that_cannot_be_opened_is_a_sentence_that_writes_nothing() {
  let scratch = Scratch::new("unwritable");
  let ctx = egui::Context::default();
  let library_path = write_library(scratch.path(), "tools.sqlite", &library());
  let path = write_document(scratch.path(), "part.c2d", &document_with_toolpath(1));

  let mut app = AppState::new(document_with_toolpath(1));
  let mut shell = Shell::new();
  shell
    .open_library_at(scratch.path())
    .expect("the application's library opens");
  shell.apply_action(&mut app, &cam_ui::UiAction::OpenFile { path });
  let opened = open_library(&ctx, &mut shell, &mut app);

  // The document was read with its library and the library then stopped being a library — a file another
  // process is writing, a disk that went away. The press is the moment the failure is discovered.
  std::fs::write(&library_path, b"this is not a database").expect("the library is corrupted");
  let (pressed, _) = click_word(&ctx, &mut shell, &mut app, &opened, "Use this tool");

  assert_eq!(
    pressed.drawn.tool_saved,
    Vec::<String>::new(),
    "a refused write reported itself as a write: {:?}",
    pressed.drawn.tool_saved
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
    pressed.actions.is_empty(),
    "a refused write still retooled the toolpath: {:?}",
    pressed.actions
  );
  assert!(
    shell.show_tool_library,
    "a refused write closed the modal, which loses the boxes the operator was editing"
  );

  // The sentence is drawn beside the form it is about, on the frame after the press that discovered it.
  let (after, texts) = frame_with(&ctx, &mut shell, &mut app);
  assert_eq!(after.drawn.tool_error.as_deref(), Some(sentence.as_str()));
  assert!(
    joined(&texts).contains(&sentence),
    "the refusal was reported and never painted: {}",
    joined(&texts)
  );
}
