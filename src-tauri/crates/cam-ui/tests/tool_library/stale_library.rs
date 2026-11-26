//! What the modal says when the library it was opened for cannot be read (R3-002/R4-001, batch 16b).
//!
//! `open_tool_library` binds the modal to the live SQLite file, not to the snapshot the window happens to
//! hold (tool-library-app E6.4). When that read fails — a file another process has locked, or one that is
//! no longer a database — the failure is a fact about this visit and must reach the operator: a modal that
//! kept a previous snapshot on screen, and offered `Use this tool` for a tool it could not re-read, would be
//! presenting last visit's tools as live ones.

use cam_app::AppState;
use cam_ui::Shell;

use super::support::*;

/// A library the window read once, then made unreadable: the file is replaced by bytes no SQLite can open.
fn a_library_that_was_read_once(scratch: &Scratch, shell: &mut Shell) -> std::path::PathBuf {
  let path = shell
    .open_library_at(scratch.path())
    .expect("the library opens");
  assert!(
    !shell.tools().is_empty(),
    "the fixture must leave the window a snapshot to go stale"
  );
  std::fs::write(
    &path,
    b"not a SQLite database, whatever it used to be",
  )
  .expect("the file is replaced");
  path
}

#[test]
fn a_library_that_cannot_be_read_says_so_in_the_modal_and_draws_no_use_this_tool() {
  let scratch = Scratch::new("unreadable");
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(1));
  let mut shell = Shell::new();
  let path = a_library_that_was_read_once(&scratch, &mut shell);

  shell.open_tool_library(&app);
  assert!(shell.show_tool_library, "the failure still opens the modal");
  let _ = frame_with(&ctx, &mut shell, &mut app);
  let (drawn, texts) = frame_with(&ctx, &mut shell, &mut app);

  let sentence = drawn
    .drawn
    .tool_library
    .iter()
    .find(|word| word.contains("could not") && word.contains("tools.sqlite"))
    .unwrap_or_else(|| {
      panic!(
        "the modal did not say that {} could not be read: {:?}",
        path.display(),
        drawn.drawn.tool_library
      )
    })
    .clone();
  assert!(
    joined(&texts).contains(&sentence),
    "the sentence was reported and never painted: {sentence:?}"
  );

  // The one press this modal exists for is the one the failure must take away: a tool from a snapshot the
  // file no longer confirms cannot be bound to a toolpath.
  assert!(
    !hits(&drawn).contains(&"Use this tool"),
    "the modal offered `Use this tool` against a snapshot it could not re-read: {:?}",
    hits(&drawn)
  );
  // And nothing else in the modal writes a library it could not read either: `New`, `Duplicate`, `Delete` and
  // `Restore defaults` all write the file behind the snapshot, so the same failure darkens all four.
  for row in ["+ New", "Duplicate", "Delete", "Restore defaults"] {
    assert!(
      !hits(&drawn).contains(&row),
      "the modal offered {row:?} for a library it could not re-read: {:?}",
      hits(&drawn)
    );
  }
  assert!(
    drawn.actions.is_empty(),
    "opening an unreadable library wrote something: {:?}",
    drawn.actions
  );
}
