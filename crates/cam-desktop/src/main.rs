//! Native window binary: opens the editor with no project.
//!
//! The window starts on the start screen (start-screen S1, S2, S3): no sample document, no generated
//! preview and no playing 3D half. A project exists only once the operator starts one, opens one from
//! disk, or resumes one from the list of projects opened before.
//!
//! The window itself needs a display, so it is verified by building rather than by automation; the
//! behaviour it exposes lives in `cam-app`, `cam-view` and `cam-ui`, all tested headless.

use std::path::Path;

use cam_app::AppState;
use cam_model::{Document, Job};
use cam_ui::start_screen::MAX_RECENTS;
use cam_ui::{stamp, tool_library, RecentProject, Shell, UiAction};
use serde::{Deserialize, Serialize};

fn main() -> eframe::Result<()> {
  let options = eframe::NativeOptions {
    viewport: eframe::egui::ViewportBuilder::default()
      .with_inner_size([1440.0, 900.0])
      .with_min_inner_size([800.0, 600.0])
      .with_title("campilot"),
    ..Default::default()
  };
  eframe::run_native(
    "campilot",
    options,
    Box::new(|creation| {
      let mut editor = Editor::new();
      // Publish the tokens before the first frame: this host hands the shell a `Ui` it built itself,
      // so the only moment the theme can reach every widget of frame one is here, on the context.
      editor.shell.publish_theme(&creation.egui_ctx);
      Ok(Box::new(editor))
    }),
  )
}

/// The editor: application state plus the shell, wired together once per frame.
struct Editor {
  app: AppState,
  shell: Shell,
}

impl Editor {
  fn new() -> Self {
    let directory = tool_library::app_data_dir();
    Self::with_library(directory.as_deref())
  }

  /// The editor over one application data directory, which is [`Self::new`]'s only variable.
  ///
  /// Split out so the one startup failure this window has to report — a data directory that cannot hold a tool
  /// library — can be produced by a test instead of needing a broken home directory to observe it.
  fn with_library(directory: Option<&Path>) -> Self {
    let mut shell = Shell::new();
    // No project at launch: the start screen is the first thing the operator sees (start-screen S1, S2).
    let app = AppState::new(Document::new(Job::default_sheet()));
    shell.start_with_no_project();
    // The tool library belongs to the application, not to a drawing (tool-library-app S1). Open it at
    // launch so the modal and the current-tool card read the application's own file, but do not open the
    // library modal itself: two modals cannot own the first frame (start-screen L5).
    //
    // A failure stays in the status bar. This window used to clear that slot unconditionally a few lines lower,
    // which was written when a sample document still left a sentence to drop: `Shell::open_app_library` writes no
    // success line of its own, so once the sample was gone the clear could only ever erase this refusal — an
    // operator whose library could not be read got a window that said nothing at all.
    let opened = match directory {
      Some(directory) => shell.open_library_at(directory),
      None => shell.open_app_library(),
    };
    if let Err(sentence) = opened {
      shell.status = sentence;
    }
    // Projects opened before live in the application's data directory. A missing file is an empty list; a
    // file that cannot be read or parsed is reported by name and never turned into an empty list.
    match read_recents() {
      RecentsRead::List(recents) => shell.set_recents(recents),
      RecentsRead::Error(sentence) => shell.set_recents_error(Some(sentence)),
    }
    Self { app, shell }
  }
}

impl eframe::App for Editor {
  fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
    let output = self.shell.show_in(&mut self.app, ui);
    // The shell never mutates the document itself; its actions are applied here, and a refusal
    // becomes a status-bar message instead of a silent no-op.
    for action in output.actions {
      // A file dialog is the host's: the shell emits the intent, the window asks the platform, and the
      // answer re-enters the shell as a path. Keeping the load behind `apply_action` is what keeps the
      // refusal sentence, the undo step and the post-open bookkeeping in one place (shell-open-document
      // S2, S3) — the dialog only decides *which* path, never what happens to it.
      let action = match action {
        UiAction::RequestOpenFile => match rfd::FileDialog::new()
          .add_filter("campilot document", &["c2d"])
          .pick_file()
        {
          Some(path) => UiAction::OpenFile { path },
          // Cancelled: no action, no message, nothing touched.
          None => continue,
        },
        UiAction::RequestImportFile => match rfd::FileDialog::new()
          .add_filter("vector or image", &["dxf", "svg", "pdf", "eps", "png"])
          .pick_file()
        {
          Some(path) => UiAction::ImportFile {
            path: path.to_string_lossy().into_owned(),
          },
          None => continue,
        },
        // Save As asks the same question in the other direction: the shell names the file it currently has so
        // the dialog opens near it, and the answer re-enters the shell as the path to write (shell-save-document
        // S2). Everything after the path — the extras, the dirty flag, the recorded name — stays in
        // `apply_action`, where a refusal is a sentence instead of a silent no-op.
        UiAction::RequestSaveAs => match rfd::FileDialog::new()
          .add_filter("campilot document", &["c2d"])
          .set_file_name(self.shell.document_name())
          .save_file()
        {
          Some(path) => UiAction::SaveFile { path },
          None => continue,
        },
        // The program asks the same question as Save As, in the extension the machine reads, and starts from
        // the document's own name so a folder of jobs is recognisable (M13 S1). Nothing after the path is the
        // host's business: the posting, the write and the sentence all happen in `apply_action`, where a
        // refusal is a message rather than a silent no-op.
        UiAction::RequestGenerateGcode => match rfd::FileDialog::new()
          .add_filter("G-code program", &["nc", "gcode", "tap"])
          .set_file_name(program_file_name(&self.shell))
          .save_file()
        {
          Some(path) => UiAction::GenerateGcode { path },
          None => continue,
        },
        // The post-processor is a choice of file, so it is the open dialog and not the save one (M13 S1).
        UiAction::RequestChoosePost => match rfd::FileDialog::new()
          .add_filter("post-processor", &["pp"])
          .pick_file()
        {
          Some(path) => UiAction::ChoosePost { path },
          None => continue,
        },
        // The tool library's export is a save dialog in the format the store writes, and it starts from
        // `tools.json` because that is the name a library file keeps wherever it is put down. Everything after
        // the path — the JSON, the refusal, the report — stays in `apply_action` (tool-library-editor S11).
        UiAction::RequestExportTools => match rfd::FileDialog::new()
          .add_filter("tool library", &["json"])
          .set_file_name("tools.json")
          .save_file()
        {
          Some(path) => UiAction::ExportTools { path },
          None => continue,
        },
        // And the import is the same question in the other direction, starting from this drawing's own name so a
        // library exported beside it is the file the dialog opens on.
        UiAction::RequestImportTools => match rfd::FileDialog::new()
          .add_filter("tool library", &["json"])
          .set_file_name(library_file_name(&self.shell))
          .pick_file()
        {
          Some(path) => UiAction::ImportTools { path },
          None => continue,
        },
        other => other,
      };
      self.shell.apply_action(&mut self.app, &action);
    }

    // The list of projects opened before is a file the host keeps (start-screen S3). The shell reports that it
    // changed and that a person asked to read it again; the host stamps the labels and writes the file.
    if self.shell.take_recents_retry() {
      match read_recents() {
        RecentsRead::List(recents) => self.shell.set_recents(recents),
        RecentsRead::Error(sentence) => self.shell.set_recents_error(Some(sentence)),
      }
    }
    if self.shell.take_recents_changed() {
      // The shell records an entry with an empty label because it reads no wall clock, so the label is stamped here and
      // the list handed back through the setter — which is the call that also clears the flag a second time. A clone
      // rather than a mutable borrow of the shell: the list a host writes is a value it owns, not a window into state
      // a frame could be drawing from.
      let mut recents = self.shell.recents().to_vec();
      stamp_recents(&mut recents);
      self.shell.set_recents(recents);
      if let Err(sentence) = write_recents(self.shell.recents()) {
        self.shell.status = sentence;
      }
    }
  }
}

/// The name the program's save dialog starts from: the document's own name with a `.nc` extension.
///
/// `untitled.c2d` becomes `untitled.nc`, so the program written beside a drawing is recognisable as that
/// drawing's program rather than as a second copy of the document (M13 S1). The extension is the dialog's
/// suggestion and not a rule: what is written is whatever name the person accepts.
fn program_file_name(shell: &Shell) -> String {
  std::path::Path::new(&shell.document_name())
    .with_extension("nc")
    .to_string_lossy()
    .into_owned()
}

/// The name the tool library's import dialog starts from: the document's own name, in the library's extension.
///
/// The same suggestion `program_file_name` makes and for the same reason: a library exported beside a drawing is
/// found by that drawing's name, and `part.c2d` therefore opens on `part.json`. The name is the dialog's
/// suggestion and not a rule — `tools.json` in the same folder is accepted just as readily (S11).
fn library_file_name(shell: &Shell) -> String {
  std::path::Path::new(&shell.document_name())
    .with_extension("json")
    .to_string_lossy()
    .into_owned()
}

/// One entry as it is kept in `recents.json`.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct RecentEntry {
  name: String,
  path: String,
  label: String,
}

impl From<&RecentProject> for RecentEntry {
  fn from(project: &RecentProject) -> Self {
    Self {
      name: project.name.clone(),
      path: project.path.to_string_lossy().into_owned(),
      label: project.label.clone(),
    }
  }
}

impl From<RecentEntry> for RecentProject {
  fn from(entry: RecentEntry) -> Self {
    Self {
      name: entry.name,
      path: std::path::PathBuf::from(entry.path),
      label: entry.label,
    }
  }
}

/// The result of reading the recents file: the list, or the reason it could not be read.
enum RecentsRead {
  List(Vec<RecentProject>),
  Error(String),
}

/// The file that holds the list of projects opened before, in the application's data directory.
fn recents_path() -> Option<std::path::PathBuf> {
  tool_library::app_data_dir().map(|dir| dir.join("recents.json"))
}

/// Read the list of projects opened before.
///
/// A missing file is an empty list. A file that cannot be read or parsed is an error named by path, never an
/// empty list and never a panic: a person with projects and an unreadable file must not be told they have none.
fn read_recents() -> RecentsRead {
  match recents_path() {
    Some(path) => read_recents_at(&path),
    None => RecentsRead::List(Vec::new()),
  }
}

/// Read one recents file, at the path the caller names.
///
/// Split from [`read_recents`] so the rule that matters can be tested against a scratch file: a missing file is an
/// empty list, and a file that cannot be read or parsed is an error naming it. They are different answers, and only the
/// first one means "no project was ever opened here".
fn read_recents_at(path: &Path) -> RecentsRead {
  let text = match std::fs::read_to_string(path) {
    Ok(text) => text,
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
      return RecentsRead::List(Vec::new());
    }
    Err(error) => {
      return RecentsRead::Error(format!("{}: {error}", path.display()));
    }
  };
  match serde_json::from_str::<Vec<RecentEntry>>(&text) {
    Ok(entries) => {
      let mut recents: Vec<RecentProject> = entries.into_iter().map(RecentProject::from).collect();
      recents.truncate(MAX_RECENTS);
      RecentsRead::List(recents)
    }
    Err(error) => RecentsRead::Error(format!("{}: {error}", path.display())),
  }
}

/// Stamp every entry that has no label yet with the current civil time.
///
/// The shell records an entry with an empty label because it reads no wall clock; the host stamps it when it
/// writes the file, using the same formatter the brand block's `Saved …` line uses.
fn stamp_recents(recents: &mut [RecentProject]) {
  let now = std::time::SystemTime::now();
  let label = stamp::format_stamp(now);
  for entry in recents {
    if entry.label.is_empty() {
      entry.label = label.clone();
    }
  }
}

/// Write the list of projects opened before, atomically.
///
/// A sibling temporary file is written and then renamed into place, so a crash mid-write never leaves a file
/// that reads as "nothing was opened". A failed write is reported as a sentence, not a crash.
fn write_recents(recents: &[RecentProject]) -> Result<(), String> {
  match recents_path() {
    Some(path) => write_recents_at(&path, recents),
    None => Ok(()),
  }
}

/// Write one recents file, at the path the caller names.
fn write_recents_at(path: &Path, recents: &[RecentProject]) -> Result<(), String> {
  if let Some(dir) = path.parent() {
    std::fs::create_dir_all(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
  }
  let entries: Vec<RecentEntry> = recents
    .iter()
    .take(MAX_RECENTS)
    .map(RecentEntry::from)
    .collect();
  let text = serde_json::to_string_pretty(&entries)
    .map_err(|error| format!("{}: {error}", path.display()))?;
  let tmp = tmp_path(path);
  std::fs::write(&tmp, text).map_err(|error| format!("{}: {error}", tmp.display()))?;
  std::fs::rename(&tmp, path).map_err(|error| {
    let _ = std::fs::remove_file(&tmp);
    format!("{}: {error}", path.display())
  })?;
  Ok(())
}

/// A sibling path for the temporary file the atomic write goes through.
fn tmp_path(path: &Path) -> std::path::PathBuf {
  let name = path
    .file_name()
    .map(|name| name.to_string_lossy().into_owned())
    .unwrap_or_else(|| "recents.json".to_owned());
  path.with_file_name(format!("{name}.tmp"))
}

#[cfg(test)]
mod tests {
  use std::path::PathBuf;

  use cam_ui::start_screen::MAX_RECENTS;
  use cam_ui::RecentProject;

  use super::{read_recents_at, write_recents_at, Editor, RecentsRead};

  /// A scratch directory under `/var/tmp`: `/tmp` is a full tmpfs on this machine, and the name is one path segment
  /// so the deletion below can never aim at somebody else's tree.
  fn scratch(name: &str) -> PathBuf {
    assert!(!name.contains('/') && !name.contains(".."));
    let directory =
      PathBuf::from("/var/tmp").join(format!("campilot-desktop-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("scratch directory");
    directory
  }

  fn entries(count: usize) -> Vec<RecentProject> {
    (0..count)
      .map(|index| {
        RecentProject::new(
          format!("part {index}"),
          PathBuf::from(format!("/shop/part-{index}.c2d")),
          String::new(),
        )
      })
      .collect()
  }

  #[test]
  fn the_window_starts_with_no_project_and_no_preview() {
    let editor = Editor::new();
    assert!(!editor.shell.project_is_open());
    assert!(editor.shell.start_screen_shown());
    assert!(!editor.shell.view_3d());
  }

  #[test]
  fn a_missing_recents_file_is_an_empty_list() {
    let directory = scratch("missing");
    match read_recents_at(&directory.join("recents.json")) {
      RecentsRead::List(list) => assert!(list.is_empty()),
      RecentsRead::Error(sentence) => panic!("a missing file is not an error: {sentence}"),
    }
  }

  #[test]
  fn a_malformed_recents_file_is_an_error_and_not_an_empty_list() {
    let directory = scratch("malformed");
    let path = directory.join("recents.json");
    std::fs::write(&path, "{not a list").expect("the broken file");

    match read_recents_at(&path) {
      RecentsRead::Error(sentence) => assert!(
        sentence.contains("recents.json"),
        "the sentence names the file: {sentence}"
      ),
      RecentsRead::List(list) => panic!("a broken file must not read as nothing opened: {list:?}"),
    }
  }

  #[test]
  fn recents_round_trip_and_keep_the_newest_twelve() {
    let directory = scratch("round-trip");
    let path = directory.join("recents.json");

    write_recents_at(&path, &entries(MAX_RECENTS + 3)).expect("the write");

    match read_recents_at(&path) {
      RecentsRead::List(list) => {
        assert_eq!(list.len(), MAX_RECENTS);
        assert_eq!(list[0].path, PathBuf::from("/shop/part-0.c2d"));
      }
      RecentsRead::Error(sentence) => panic!("the file just written could not be read: {sentence}"),
    }
    assert!(
      !path.with_file_name("recents.json.tmp").exists(),
      "the temporary file is renamed into place, not left behind"
    );
  }

  #[test]
  fn a_library_that_cannot_be_opened_leaves_its_sentence_in_the_status_bar() {
    let directory = scratch("library-broken");
    // A directory standing where the library file goes: SQLite cannot open it, which is the answer a read-only or
    // damaged data directory gives a person.
    std::fs::create_dir_all(directory.join("tools.sqlite"))
      .expect("a directory where the file goes");

    let editor = Editor::with_library(Some(&directory));

    assert!(
      !editor.shell.status.is_empty(),
      "a library that cannot be read is the one thing a fresh window has to say"
    );
  }

  #[test]
  fn a_library_that_opens_leaves_the_fresh_window_with_nothing_to_report() {
    let directory = scratch("library-ok");

    let editor = Editor::with_library(Some(&directory));

    assert!(
      editor.shell.status.is_empty(),
      "status: {:?}",
      editor.shell.status
    );
    assert!(
      !editor.shell.tools().is_empty(),
      "a fresh data directory gets the factory catalogue, so the open really happened"
    );
  }
}
