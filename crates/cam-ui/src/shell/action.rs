use std::path::{Component, Path, PathBuf};

use cam_app::AppState;
use cam_model::Job;
use cam_store::tools::{StoredTool, ToolLibrary};
use cam_units::Length;

use crate::actions::{
  file_label, handle_action_with_file, DocumentFileState, EMPTY_POST_PATH, EMPTY_PROGRAM_PATH,
  NO_POST, NO_TOOLPATHS,
};
use crate::{ActionReport, Shell, UiAction};

use super::default_material_size;

impl Shell {
  /// Apply one action and show what it produced in the status bar.
  ///
  /// This is what a host calls for every action [`Self::show`] returned. It keeps the one-command
  /// rule — the work itself happens in the action handler, which is
  /// [`handle_action`](crate::handle_action) with this shell's file handed in — and turns the outcome the
  /// command layer reports (entity count, ellipse radial error, arcs lost to tessellation) and any refusal,
  /// with the kernel's own message, into the sentence the user reads.
  pub fn apply_action(&mut self, app: &mut AppState, action: &UiAction) {
    // The file behind the drawing is the shell's, not the application's: the path it was read from and the
    // archive entries this build does not interpret travel with the window, so they are handed to the one
    // handler that touches `AppState` instead of being patched back in here after the fact. That is what
    // lets an open carry its extras and a save write them back without a second copy of either.
    // The two program actions are the shell's own rather than the shared handler's, for the reason
    // `NO_SHELL_FOR_SAVE` gives in the other direction: the post-processor is window state, like the path the
    // drawing came from, and `handle_action_with_file` is reachable without a window at all. Intercepting them
    // here is what keeps one place holding the dialect and one place writing the file, without widening a
    // handler whose signature every file action already shares.
    let applied = match action {
      UiAction::NewProject => {
        app.replace_document(cam_model::Document::new(cam_model::Job::default_sheet()));
        self.open_material(app);
        self.material_size = default_material_size();
        self.clear_preview();
        self.auto_frame = true;
        Ok(ActionReport {
          label: action.label(),
          ..ActionReport::default()
        })
      }
      UiAction::ChoosePost { path } => self.choose_post(path),
      UiAction::GenerateGcode { path } => self.generate_gcode(app, path),
      // `ImportFile` no longer imports: it parses the file and opens the modal that reports what it found
      // (shell-file-operations S3), so nothing replaces the drawing until the operator presses `Import`.
      // `handle_action_with_file` keeps the immediate import for callers with no window at all, which is why
      // this interception lives in the shell and not in the shared handler.
      UiAction::ImportFile { path } => self.open_import(path),
      _ => handle_action_with_file(
        app,
        action,
        Some(&mut DocumentFileState {
          path: &mut self.document_path,
          extras: &mut self.document_extras,
        }),
      ),
    };
    // The brand block's stamp is read from the file the action just read or wrote, and only when the action
    // was one of those: a `stat` per frame would be a syscall in the paint loop, and a stamp cached from an
    // earlier action would describe a file that has since changed. `modified` answers `Err` on a filesystem
    // that does not keep mtimes, which is an honest `None` — the block then says the work is unsaved.
    if applied.is_ok()
      && matches!(
        action,
        UiAction::OpenFile { .. } | UiAction::SaveFile { .. } | UiAction::Save
      )
    {
      self.saved_at = self
        .document_path
        .as_deref()
        .and_then(|path| std::fs::metadata(path).ok())
        .and_then(|metadata| metadata.modified().ok());
    }
    // An open replaces the document, so everything the shell derived from the previous one goes with it.
    // None of these three is in the undo history — the preview is a picture of a program, the tool
    // snapshot belongs to the drawing that was showing, and the drafts are numbers the dialog has not
    // written yet — so a bare swap would leave the 3D panel playing the old drawing's program and the
    // dialog holding the old stock. The snapshot is not simply emptied: the opened document names the
    // library it was written against, so that library is resolved from beside it (S2), and a library that
    // is missing or unusable costs the drawing nothing (S4) — the open's sentence stays and says why the
    // library could not be used. The view is handed back to the shell for the same reason: the new drawing
    // is framed, because the old drawing's zoom was never a choice about this one.
    let mut library_note: Option<String> = None;
    if applied.is_ok() && matches!(action, UiAction::OpenFile { .. }) {
      self.clear_preview();
      let resolved = self.document_path.as_deref().and_then(|document_path| {
        app
          .document()
          .tools_ref
          .as_deref()
          .map(|reference| tools_beside_document(document_path, reference))
      });
      match resolved {
        Some(Ok(tools)) => self.set_tools(tools),
        Some(Err(reason)) => {
          self.set_tools(Vec::new());
          library_note = Some(reason);
        }
        // No `tools_ref`: today's empty snapshot and today's sentence, unchanged (S2).
        None => self.set_tools(Vec::new()),
      }
      self.material_size = default_material_size();
      self.auto_frame = true;
    }
    self.status = match applied {
      Ok(report) => report.message(),
      Err(message) => format!("refused: {message}"),
    };
    // Added after the open's own sentence rather than folded into it, so a document with no `tools_ref`
    // prints exactly what it printed before this resolution existed.
    if let Some(note) = library_note {
      self.status.push_str(&format!(" — {note}"));
    }
  }

  /// The modal's `Import`: write the parse the modal reported into the document (shell-file-operations S3).
  ///
  /// It is public because a modal is painted inside a frame and a headless test has no frame to press: without
  /// this, the only way to complete an import in a test would be to click a point, and the point would be a
  /// promise about the dialog's layout rather than about the import.
  ///
  /// The document comes from [`Self::import_parsed`], never from a second read of the file: the counts the
  /// modal drew and the geometry that replaces the drawing are then the same parse by construction. A refusal
  /// (`into_document` failing on a parse that produced no bounds) stays in the modal — [`Self::import_error`]
  /// carries the sentence — because closing it would lose the file and the report the operator was reading.
  pub fn confirm_import(&mut self, app: &mut AppState) -> Result<ActionReport, String> {
    let Some(parsed) = self.import_parsed.clone() else {
      return Err(NO_IMPORT_PENDING.to_owned());
    };
    let adjust_material = self.import_adjust_material;
    match import_into_document(app, &parsed, self.import_path.as_deref(), adjust_material) {
      Ok(report) => {
        self.status = report.message();
        self.close_import();
        Ok(report)
      }
      Err(message) => {
        self.status = format!("refused: {message}");
        self.import_error = Some(message.clone());
        Err(message)
      }
    }
  }

  /// Re-parse the file the import modal is showing at a new flattening tolerance (shell-file-operations S3).
  ///
  /// This is the modal's tolerance field's change handler, and it is public for the reason
  /// [`Self::confirm_import`] is: a `DragValue` is edited inside a frame, and without this the only way to
  /// prove the field re-parses would be to guess at the dialog's layout and drag a point.
  ///
  /// The parse is the same file read again with `ImportOptions { tolerance, ..default }`, so the report above
  /// the field, the geometry `Import` will write and the number beside it are one parse by construction. A
  /// refusal — a number that is not a usable distance, or a file that has gone away since the modal opened —
  /// leaves [`Self::import_tolerance`] and [`Self::import_parsed`] exactly as they were, so the modal goes on
  /// describing something `Import` can still write, and [`Self::import_error`] carries the sentence.
  pub fn set_import_tolerance(&mut self, tolerance: f64) -> Result<ActionReport, String> {
    if !(tolerance.is_finite() && tolerance > 0.0) {
      return Err(self.import_refused(format!(
        "flattening tolerance must be a positive number of millimetres, got {tolerance} mm"
      )));
    }
    let Some(path) = self.import_path.clone() else {
      return Err(self.import_refused(NO_IMPORT_PENDING.to_owned()));
    };
    let options = cam_import::ImportOptions {
      tolerance,
      ..cam_import::ImportOptions::default()
    };
    let parsed = cam_import::import_path(&path, &options)
      .map_err(|error| self.import_refused(error.to_string()))?;
    self.import_tolerance = tolerance;
    self.import_parsed = Some(parsed.clone());
    self.import_error = None;
    let report = ActionReport {
      label: "import file",
      note: Some(format!(
        "tolerance {tolerance} mm · {} contour(s) found, {} curve(s) flattened, {} skipped",
        parsed.report.contours,
        parsed.report.flattened_curves,
        parsed.report.skipped_count()
      )),
      ..ActionReport::default()
    };
    self.status = report.message();
    Ok(report)
  }

  /// Record a refusal of one of the modal's options, and keep the report it is showing.
  ///
  /// Returns the sentence so the caller can hand it to its own `Err`: the status bar and the dialog must say
  /// the same words ([`Self::set_import_tolerance`] is the only caller, and its `Err` is that sentence).
  fn import_refused(&mut self, message: String) -> String {
    self.status = format!("refused: {message}");
    self.import_error = Some(message.clone());
    message
  }

  /// The modal's `Cancel`, and the `Esc` that means the same thing: forget the parse and write nothing.
  ///
  /// Public for the reason [`Self::confirm_import`] is: the state a cancelled modal leaves behind — no open
  /// flag, no path, no parse and no refusal — is a claim a headless test has to be able to make.
  pub fn close_import(&mut self) {
    self.show_import = false;
    self.import_path = None;
    self.import_parsed = None;
    self.import_error = None;
  }

  /// Parse `path` and open the import modal, replacing nothing yet (shell-file-operations S3).
  ///
  /// A parse that fails is refused here and never reaches the modal: a dialog whose only content is the reason
  /// a file could not be read would ask the operator to press `Import` on nothing. The status bar carries the
  /// refusal, the same way it carries a refused open, and the drawing is untouched because nothing has been read
  /// into it.
  fn open_import(&mut self, path: &str) -> Result<ActionReport, String> {
    let path = PathBuf::from(path);
    let options = cam_import::ImportOptions {
      tolerance: self.import_tolerance,
      ..cam_import::ImportOptions::default()
    };
    let parsed = cam_import::import_path(&path, &options).map_err(|error| error.to_string())?;
    self.import_path = Some(path);
    self.import_parsed = Some(parsed);
    self.import_error = None;
    self.show_import = true;
    Ok(ActionReport {
      label: "import file",
      note: Some("import pending".to_owned()),
      ..ActionReport::default()
    })
  }

  /// Hand the window the post-processor it writes programs in, as the file's label and its source text.
  ///
  /// The label is what the app bar shows and what a refusal names, so it is the file's own name rather than a
  /// path: a person reading `Post: grbl.pp` is reading the choice they made. Held as source text rather than as
  /// a parsed processor because a session can outlive the file it came from — a post-processor edited on disk
  /// mid-session is a different dialect, and re-parsing at use is what makes the program match the text in hand.
  pub fn set_post(&mut self, name: impl Into<String>, source: impl Into<String>) {
    self.post = Some((name.into(), source.into()));
  }

  /// Forget the held post-processor, so the next program refuses until another is chosen.
  pub fn clear_post(&mut self) {
    self.post = None;
  }

  /// The post-processor's file label, when the window holds one.
  pub fn post_name(&self) -> Option<&str> {
    self.post.as_ref().map(|(name, _)| name.as_str())
  }

  /// Hold the post-processor in `path`, refusing a file that is not one.
  ///
  /// The parse is the acceptance test and it happens here rather than at the first program: a file that does
  /// not parse would otherwise be accepted now and fail once the person believes their program exists. The
  /// previous post-processor stays on every refusal, so choosing a wrong file costs the choice and nothing
  /// else.
  fn choose_post(&mut self, path: &Path) -> Result<ActionReport, String> {
    if path.as_os_str().is_empty() {
      return Err(EMPTY_POST_PATH.to_owned());
    }
    let name = file_label(path);
    let source = std::fs::read_to_string(path).map_err(|error| format!("{name}: {error}"))?;
    cam_post::parse_post(&source).map_err(|error| format!("{name}: {error}"))?;
    self.post = Some((name.clone(), source));
    Ok(ActionReport {
      label: "choose post-processor",
      note: Some(format!("using {name}")),
      ..ActionReport::default()
    })
  }

  /// Post the open document and write the program to `target` (M13 S1, S2).
  ///
  /// The program is [`cam_program::program_for_snapshot`]'s, the same authoring the cut-time estimate in the
  /// Toolpaths task already uses, so the sentence about how long the job takes and the file the machine runs
  /// cannot disagree about what the toolpaths are.
  ///
  /// # Why the material is not passed on
  ///
  /// A snapshot is a list of tools with their own records, and a material is a selector *into a library* —
  /// `program_for_snapshot` cannot answer one and refuses it by name (`crates/cam-program/src/lib.rs:95-113`),
  /// so passing the document's material would make every drawing that names one refuse to produce a program.
  /// `None` is what the snapshot can honestly mean, and the program therefore cuts at the tools' own rates;
  /// the divergence from `cam-cli emit --material` is the same recorded limit the shell's estimate carries
  /// (M11's follow-up), and it is named here rather than discovered by a person comparing two files.
  fn generate_gcode(&self, app: &AppState, target: &Path) -> Result<ActionReport, String> {
    if target.as_os_str().is_empty() {
      return Err(EMPTY_PROGRAM_PATH.to_owned());
    }
    let Some((name, source)) = self.post.as_ref() else {
      return Err(NO_POST.to_owned());
    };
    if app.document().toolpaths.is_empty() {
      return Err(NO_TOOLPATHS.to_owned());
    }
    let post = cam_post::parse_post(source).map_err(|error| format!("{name}: {error}"))?;
    let document = app.document();
    let stream = cam_program::program_for_snapshot(document, &self.tools, None)
      .map_err(|error| error.to_string())?;
    let meta = cam_post::ProgramMeta {
      file_name: target
        .file_name()
        .map(|file| file.to_string_lossy().into_owned())
        .unwrap_or_default(),
      file_path: target
        .parent()
        .map(|dir| dir.to_string_lossy().into_owned())
        .unwrap_or_default(),
    };
    let emitted =
      cam_post::emit(&post, &stream, &meta).map_err(|error| format!("{name}: {error}"))?;
    std::fs::write(target, &emitted.text)
      .map_err(|error| format!("{}: {error}", file_label(target)))?;
    Ok(ActionReport {
      label: "generate g-code",
      note: Some(format!(
        "wrote {} line(s) to {}",
        emitted.lines,
        file_label(target)
      )),
      ..ActionReport::default()
    })
  }
}

/// The sentence a shell with no parse to import answers with.
///
/// `confirm_import` is public so a headless test can finish an import, and a test — or a host that missed the
/// flag — asking it to write nothing gets a refusal that names the missing step instead of a silent `Ok`.
const NO_IMPORT_PENDING: &str = "no import is pending: choose a file to import first";

/// Write one parse into `app` as an undoable replacement, and report what arrived.
///
/// This is [`handle_action`](crate::handle_action)'s `ImportFile` branch without the read: the command layer
/// takes a path and imports it immediately, which is what a caller with no window needs, and the modal has
/// already paid for the parse it showed. The note is the same sentence, word for word, so the status bar cannot
/// describe the same file two ways depending on which route imported it.
///
/// `adjust_material` is the modal's "Adjust material size to drawing" option, and it is the one thing this
/// function does beyond `cam_import::into_document`: checked, the stock is re-derived as the imported contours'
/// bounding box plus [`cam_import::PLACEMENT_MARGIN`] on every side, which is where the importer placed the
/// geometry, so it lands with that margin on all four sides and the thickness is left alone. Unchecked, the
/// stock is the material the drawing being replaced already had — the sheet this operator set up — and the
/// geometry keeps the placement the importer gave it.
fn import_into_document(
  app: &mut AppState,
  parsed: &cam_import::Imported,
  path: Option<&Path>,
  adjust_material: bool,
) -> Result<ActionReport, String> {
  let previous_job = app.document().job.clone();
  let mut document = cam_import::into_document(parsed, None).map_err(|error| error.to_string())?;
  document.job = if adjust_material {
    stock_for_drawing(&document)
  } else {
    previous_job
  };
  let created_entities: Vec<String> = document
    .entities()
    .map(|entity| entity.id.clone())
    .collect();
  let file_name = path
    .and_then(|path| path.file_name())
    .map(|name| name.to_string_lossy().into_owned())
    .or_else(|| path.map(|path| path.display().to_string()))
    .unwrap_or_default();
  let note = format!(
    "{} contour(s) from {file_name}, {} curve(s) flattened, {} skipped",
    parsed.report.contours,
    parsed.report.flattened_curves,
    parsed.report.skipped_count()
  );
  // One undo step, the same one the immediate import records: an import replaces the drawing, and one undo
  // brings back the drawing it replaced.
  app.replace_document_undoable(document);
  app.select(created_entities.clone());
  Ok(ActionReport {
    label: "import file",
    created: created_entities.len(),
    created_entities,
    note: Some(note),
    ..ActionReport::default()
  })
}

/// The stock an import writes when it is asked to fit the material to the drawing.
///
/// The job is the imported contours' bounding box plus [`cam_import::PLACEMENT_MARGIN`] on every side, which is
/// the margin the importer placed the geometry at — so the drawing lands in the stock with that margin on all
/// four sides. Everything else about the job is kept: the thickness is how thick the material is, not a fact
/// about the drawing, and the origin is the imported document's own bottom-left corner, measured the same way
/// the geometry was placed.
fn stock_for_drawing(document: &cam_model::Document) -> Job {
  let margin = cam_import::PLACEMENT_MARGIN;
  let mut bounds = cam_geom::Bounds::empty();
  for entity in document.entities() {
    if let Some(contour) = entity.kind.as_contour() {
      bounds = bounds.union(contour.bounds());
    }
  }
  let mut job = document.job.clone();
  job.width = Length::from_mm(bounds.width() + 2.0 * margin);
  job.height = Length::from_mm(bounds.height() + 2.0 * margin);
  job
}

/// The tools an opened document names, read from the directory that document was opened from.
///
/// `tools_ref` is relative to the document's own directory (`docs/format.md`), which is the whole of the
/// rule the join below implements: an absolute reference, or one that leaves the directory through `..`,
/// is **refused before the filesystem is touched**, because it is not a reference relative to that
/// directory at all. This is the one place the window is stricter than `cam-cli` (S3, S7), and the reason
/// is the write half of the same rule: a document names a file the window reads, so the window opens it
/// with [`ToolLibrary::open_read_only`], which never creates a file, never creates its directory and never
/// migrates — a hostile document therefore has nothing to point the window's writer at.
///
/// The rule is checked on the reference rather than decided by canonicalising both sides. `..` is refused
/// by name wherever it appears, even in `sub/../tools.sqlite`, which would land back inside the
/// directory: a canonicalised comparison needs both ends to exist, so it would turn "the library is
/// missing" — the ordinary, soft failure S4 is about — into a containment verdict, and answering a
/// question about the path would read the filesystem before the guard did. A path with a `..` in it is
/// not a path relative to the document's directory, and refusing it costs the user one field.
///
/// A reference that stays inside the directory and names nothing is not a refused open: the drawing is
/// the user's and the library is an accessory, so the answer is the empty snapshot plus the sentence that
/// says which library could not be used and why (S4).
fn tools_beside_document(document_path: &Path, reference: &str) -> Result<Vec<StoredTool>, String> {
  let named = Path::new(reference);
  // Every component must be a plain name. Naming what is *allowed* rather than what is refused is what
  // keeps this rule total: refusing `..` and anything `is_absolute()` missed the two Windows shapes that
  // are neither — a root-relative `\drv\file` (a `RootDir` with no prefix) and a drive-relative
  // `C:file` (a `Prefix` with no root) — and `join` resolves both outside the document's directory. A
  // list of refusals is a list to forget something on; this one has nothing left to forget.
  if named
    .components()
    .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
  {
    return Err(format!(
      "the tool library {reference} could not be used: it is not inside the document's directory"
    ));
  }
  let directory = document_path.parent().unwrap_or_else(|| Path::new("."));
  let path = directory.join(named);
  // Checked before the library is opened, because the sentence a missing library deserves is the one that
  // names the reference the user wrote and the directory that holds no such file, rather than SQLite's
  // description of a path it could not open (S4). The read-only open below creates nothing either way, so
  // this is the message and not the write surface. `try_exists` rather than `exists` so a directory the
  // process may not read is reported as the error it is instead of being told the file is missing.
  let present = path
    .try_exists()
    .map_err(|error| format!("the tool library {reference} could not be used: {error}"))?;
  if !present {
    return Err(format!(
      "the tool library {reference} could not be used: the document's directory holds no such file"
    ));
  }
  // Read-only, so the window reads a library and can never write one — not even through a symlink beside
  // the document that points at a file outside it (S3). A library this build cannot read without writing
  // (a file that was never initialised, or one left at an older schema version) is refused with the
  // store's own reason, and that refusal is soft (S4).
  let library = ToolLibrary::open_read_only(&path).map_err(|error| error.to_string())?;
  library.tools().map_err(|error| error.to_string())
}
