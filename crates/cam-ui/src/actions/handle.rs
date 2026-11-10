use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cam_app::{AppState, Command};
use cam_geom::Point2;
use cam_view::build_scene;

use crate::{ActionReport, UiAction, JOIN_TOLERANCE_MM};

/// What a save with no recorded path answers: the one command that can supply one.
const NO_RECORDED_PATH: &str =
  "this drawing has no file yet — use Save As to name one, and Save writes to it from then on";

/// What a save from an entry point that holds no shell answers.
///
/// The archive's entries belong to the shell, so a write here could only be a document-only archive written
/// over a file that held more: the refusal names what is missing instead of losing it.
const NO_SHELL_FOR_SAVE: &str =
  "a save needs the archive entries the shell is holding, and this entry point has no shell — a write \
   here would drop them; apply the action through Shell::apply_action";

/// What a save to an empty path answers.
///
/// An empty path is not a file name: the writer would compose its temporary sibling in the process's own
/// working directory, so the refusal comes before any filesystem call and the save reaches nothing.
const EMPTY_SAVE_PATH: &str =
  "a save needs a file to write, and this path is empty — use Save As to name one, and no save writes \
   into the directory the window was started from";

/// What a program generation with no post-processor answers.
///
/// The dialect is not a default: two post-processors write two different programs from one toolpath, so a
/// window that picked one for the person would be choosing what their machine runs. The refusal names the one
/// control that supplies it.
pub(crate) const NO_POST: &str =
  "writing a program needs a post-processor, and none is chosen — use Post… in the app bar to name one, and \
   every program this window writes is written in it";

/// What a program generation from a drawing with no toolpaths answers.
///
/// A program is a sequence of moves; a drawing alone is geometry. Naming the missing half is the whole point
/// of M13 S2, which exists because the shell used to point at a command in another window instead.
pub(crate) const NO_TOOLPATHS: &str =
  "this drawing has no toolpaths, so there is no program to write — author one in the Toolpaths task first, \
   and the program follows from it";

/// What a program generation to an empty path answers.
///
/// An empty path is not a file name, so the refusal comes before any filesystem call: the writer would
/// otherwise compose its temporary sibling in the directory the window was started from.
pub(crate) const EMPTY_PROGRAM_PATH: &str =
  "a program needs a file to write, and this path is empty — use the save dialog to name one, and no program \
   is written into the directory the window was started from";

/// What a post-processor choice with an empty path answers.
pub(crate) const EMPTY_POST_PATH: &str =
  "choosing a post-processor needs a file to read, and this path is empty — use the open dialog to name one";

/// What a program or a post-processor answers from an entry point that holds no shell.
///
/// The post-processor travels with the window, like the archive entries a save refuses to write without
/// ([`NO_SHELL_FOR_SAVE`]): a program written from a bare handler would be written in a dialect nobody chose.
const NO_SHELL_FOR_POST: &str =
  "writing a program needs the post-processor the window is holding, and this entry point has no shell — \
   choose one with Post… and apply the action through Shell::apply_action";

/// What an export or an import of the tool library answers from an entry point that holds no shell.
///
/// The library travels with the window for the same reason the post-processor does: a bare handler holds no
/// tools to export and no snapshot an imported file could replace, so it names what is missing instead of
/// writing an empty library over the operator's file.
const NO_SHELL_FOR_TOOLS: &str =
  "a tool library export or import needs the tools the window is holding, and this entry point has no \
   shell — open the library and apply the action through Shell::apply_action";

/// The file the drawing came from, as the shell holds it: the recorded path and the archive's other
/// entries.
///
/// Handed to [`handle_action_with_file`] as an `Option` because the bare [`handle_action`] entry point has
/// no shell and therefore neither: a save there refuses rather than write a file it cannot carry the
/// entries into.
pub(crate) struct DocumentFileState<'a> {
  /// The path the shell recorded, `None` until an open or a Save As succeeds.
  pub(crate) path: &'a mut Option<PathBuf>,
  /// Uninterpreted archive entries, carried from the last load and written back by the next save.
  pub(crate) extras: &'a mut BTreeMap<String, Vec<u8>>,
}

/// Apply one action that needs nothing but the application state.
///
/// **A bare save is refused.** The shell owns the path the drawing came from and the archive entries this
/// build does not interpret, and a write without those entries would drop them from the operator's file —
/// the one damage [`cam_store::document::DocumentFile`] exists to prevent. [`UiAction::Save`] and
/// [`UiAction::SaveFile`] therefore answer `refused: …` here, create no file and modify none, and a host
/// applies every action through [`Shell::apply_action`](crate::Shell::apply_action), which is this handler with
/// the shell's file handed in.
///
/// Reading needs no such state: [`UiAction::OpenFile`] still loads and applies a document here, because an
/// open writes no file — it only leaves the recorded path empty, which is what makes a save refuse after it.
pub fn handle_action(app: &mut AppState, action: &UiAction) -> Result<ActionReport, String> {
  handle_action_with_file(app, action, None)
}

/// Apply one action to the application state, with the file the drawing came from.
///
/// The only place the shell touches [`AppState`], so a test can drive the whole interface by feeding
/// actions. Every authoring arm builds exactly one [`Command`] and runs it through
/// [`apply_command`]; nothing here mutates the document itself. The three file actions take the shell's
/// [`DocumentFileState`]: `Save` and `SaveFile` refuse without it, because there is nothing they could
/// write the archive's other entries from.
pub(crate) fn handle_action_with_file(
  app: &mut AppState,
  action: &UiAction,
  file: Option<&mut DocumentFileState<'_>>,
) -> Result<ActionReport, String> {
  match action {
    UiAction::Undo => {
      app.undo();
      Ok(ActionReport {
        label: action.label(),
        ..ActionReport::default()
      })
    }
    UiAction::Redo => {
      app.redo();
      Ok(ActionReport {
        label: action.label(),
        ..ActionReport::default()
      })
    }
    UiAction::ZoomToFit => {
      let scene = build_scene(app.document(), app.selection());
      let bounds = scene.bounds();
      if bounds.is_empty() {
        return Err("nothing to frame: the document holds no geometry".to_owned());
      }
      app.fit_camera(bounds.expand(5.0), 24.0);
      Ok(ActionReport {
        label: action.label(),
        ..ActionReport::default()
      })
    }
    UiAction::ImportFile { path } => {
      let imported = cam_import::import_path(
        std::path::Path::new(path),
        &cam_import::ImportOptions::default(),
      )
      .map_err(|error| error.to_string())?;
      let document =
        cam_import::into_document(&imported, None).map_err(|error| error.to_string())?;
      let created_entities: Vec<String> = document
        .entities()
        .map(|entity| entity.id.clone())
        .collect();
      let file_name = std::path::Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.clone());
      let note = format!(
        "{} contour(s) from {file_name}, {} curve(s) flattened, {} skipped",
        imported.report.contours,
        imported.report.flattened_curves,
        imported.report.skipped_count()
      );
      app.replace_document_undoable(document);
      app.select(created_entities.clone());
      Ok(ActionReport {
        label: action.label(),
        created: created_entities.len(),
        created_entities,
        note: Some(note),
        ..ActionReport::default()
      })
    }
    UiAction::OpenFile { path: opening } => {
      // A drawing with unsaved changes is refused **before** the file is read. The shell can save now, so
      // silently discarding edits is no longer a service, and refusing first is what makes the refusal total:
      // the document, the history, the recorded path and the name are untouched, instead of an open that half
      // happens and leaves the bar naming a file the drawing did not come from (S4).
      if app.is_dirty() {
        let name = file
          .as_deref()
          .and_then(|file| file.path.as_deref())
          .map(file_label)
          .unwrap_or_else(|| "this drawing".to_owned());
        return Err(format!(
          "{name} has unsaved changes — save or undo them before opening {}",
          opening.display()
        ));
      }
      // The loader `cam-cli open` uses, so the archive scan, the migration and the validation are one
      // rule (S2) — and `DocumentFile` rather than `load_document`, because the entries this build does
      // not interpret are part of the document rather than something to drop on the floor: they are carried
      // on the shell and written back by the next save.
      let opened =
        cam_store::document::DocumentFile::load(opening).map_err(|error| error.to_string())?;
      let cam_store::document::DocumentFile {
        document,
        extras: carried,
      } = opened;
      // The replacement is **not** undoable (S4, the owner's "abrir deja de ser deshacible"): the open
      // replaces the drawing and the recorded path together, so a history entry that crossed it would let an
      // undo step back to a drawing the shell no longer names — and the next Save would then write the wrong
      // document over the file the path still points at, dropping that file's archive entries. The
      // non-undoable replacement clears the history and the selection, which is what keeps "the recorded path
      // describes what is on screen" true by construction. No operator edit is lost by that: an open over
      // unsaved work is refused above, before the file is even read.
      app.replace_document(document);
      // No `mark_saved` follows: `replace_document` already clears the dirty flag, because what was opened is
      // exactly what is in the file.
      // The entries ride on the shell and are written back by the next save. A caller with no shell keeps
      // none — and a save from that state refuses, so nothing is ever written without them (S3).
      if let Some(file) = file {
        *file.extras = carried;
        *file.path = Some(opening.clone());
      }
      Ok(ActionReport {
        label: action.label(),
        note: Some(format!("opened {}", file_label(opening))),
        ..ActionReport::default()
      })
    }
    // An intent the host answers: nothing to apply, and nothing to refuse either, because the shell has
    // already said everything it has to say about it by asking.
    UiAction::RequestOpenFile => Ok(ActionReport {
      label: action.label(),
      ..ActionReport::default()
    }),
    UiAction::NewProject => Ok(ActionReport {
      label: action.label(),
      ..ActionReport::default()
    }),
    UiAction::RequestImportFile => Ok(ActionReport {
      label: action.label(),
      ..ActionReport::default()
    }),
    UiAction::Save => {
      // The shell will not invent a path: a save with nowhere to go would write into whatever directory the
      // window was started from, or pick a name the operator never chose, so it is refused and points at the
      // one command that can ask for a name (S2). A caller with no shell has no recorded path either, so it
      // refuses in the same sentence instead of guessing at a file.
      let Some(file) = file else {
        return Err(NO_RECORDED_PATH.to_owned());
      };
      let Some(target) = file.path.as_deref() else {
        return Err(NO_RECORDED_PATH.to_owned());
      };
      // A recorded path is never empty through this shell, and the rule belongs to saving rather than to
      // Save As: an empty path names no file, so it is refused here too instead of reaching the writer.
      if target.as_os_str().is_empty() {
        return Err(EMPTY_SAVE_PATH.to_owned());
      }
      write_document(app, target, file.extras)?;
      Ok(ActionReport {
        label: action.label(),
        note: Some(format!("wrote {}", file_label(target))),
        ..ActionReport::default()
      })
    }
    UiAction::SaveFile { path: target } => {
      // Refused before anything touches the filesystem when there is no shell to take the entries from: the
      // store would happily write a document-only archive, which is exactly the silent loss S3 exists to
      // prevent, and refusing first is also what leaves no temporary file behind.
      let Some(file) = file else {
        return Err(NO_SHELL_FOR_SAVE.to_owned());
      };
      // Refused before the writer is reached: an empty path names no file, and the writer would compose
      // its temporary sibling in the working directory of the process instead (S2, S5).
      if target.as_os_str().is_empty() {
        return Err(EMPTY_SAVE_PATH.to_owned());
      }
      write_document(app, target, file.extras)?;
      // Save As becomes the file: the bar names what was just written, and the next Save writes there. Set
      // after the write and not before, so a failed save leaves the recorded path as it was (S5).
      *file.path = Some(target.clone());
      Ok(ActionReport {
        label: action.label(),
        note: Some(format!("wrote {}", file_label(target))),
        ..ActionReport::default()
      })
    }
    // The open intent's mirror image: ask the host for a path, and change nothing while asking (S2).
    UiAction::RequestSaveAs => Ok(ActionReport {
      label: action.label(),
      ..ActionReport::default()
    }),
    // The tool library's two file intents are the same question in the library's own format: they ask the host
    // for a path and change nothing while asking (S11).
    UiAction::RequestExportTools | UiAction::RequestImportTools => Ok(ActionReport {
      label: action.label(),
      ..ActionReport::default()
    }),
    // And their answers belong to the window, not to this handler: the library is shell state, so a bare
    // handler has no tools to write out and no snapshot to replace, and says so rather than writing nothing.
    UiAction::ExportTools { .. } | UiAction::ImportTools { .. } => {
      Err(NO_SHELL_FOR_TOOLS.to_owned())
    }
    // The two program intents ask the host for something and change nothing while asking, exactly as the two
    // file intents do.
    UiAction::RequestGenerateGcode | UiAction::RequestChoosePost => Ok(ActionReport {
      label: action.label(),
      ..ActionReport::default()
    }),
    // And their answers belong to the window, not to this handler: the post-processor is held by the shell,
    // so a bare handler has no dialect to write in and says so instead of picking one.
    UiAction::GenerateGcode { .. } | UiAction::ChoosePost { .. } => {
      Err(NO_SHELL_FOR_POST.to_owned())
    }
    UiAction::SetJob {
      width,
      height,
      thickness,
    } => apply_command(
      app,
      Command::SetJob {
        width: *width,
        height: *height,
        thickness: *thickness,
      },
    ),
    UiAction::AddRectangle { width, height } => {
      let layer_id = drawing_layer(app)?;
      let centre = app.camera().center();
      let id = app.next_entity_id();
      let command = Command::AddRectangle {
        layer_id,
        entity_id: id.clone(),
        at: Point2::new(centre.x - width / 2.0, centre.y - height / 2.0),
        width: *width,
        height: *height,
      };
      apply_draw(app, command, &id)
    }
    UiAction::Draw(shape) => {
      let layer_id = drawing_layer(app)?;
      let id = app.next_entity_id();
      apply_draw(app, shape.to_command(&layer_id, &id), &id)
    }
    UiAction::Transform(transform) => {
      let entity_ids = selected_ids(app)?;
      apply_command(
        app,
        Command::TransformEntities {
          entity_ids,
          transform: transform.clone(),
        },
      )
    }
    UiAction::Array {
      rows,
      columns,
      dx,
      dy,
    } => {
      let entity_ids = selected_ids(app)?;
      apply_minting(app, |first_entity_index| Command::ArrayCopy {
        entity_ids: entity_ids.clone(),
        rows: *rows,
        columns: *columns,
        dx: *dx,
        dy: *dy,
        first_entity_index,
      })
    }
    UiAction::Align(mode) => {
      let entity_ids = selected_ids(app)?;
      apply_command(
        app,
        Command::AlignEntities {
          entity_ids,
          mode: *mode,
        },
      )
    }
    UiAction::Offset { distance } => {
      let entity_id = single_selection(app)?;
      apply_minting(app, |first_entity_index| Command::OffsetContour {
        entity_id: entity_id.clone(),
        distance: *distance,
        first_entity_index,
      })
    }
    UiAction::Join => {
      let entity_ids = selected_ids(app)?;
      apply_command(
        app,
        Command::JoinEntities {
          entity_ids,
          tolerance: JOIN_TOLERANCE_MM,
        },
      )
    }
    UiAction::Trim { target, cutter } => {
      apply_minting(app, |first_entity_index| Command::TrimEntity {
        target_id: target.clone(),
        cutter_id: cutter.clone(),
        tolerance: JOIN_TOLERANCE_MM,
        first_entity_index,
      })
    }
    UiAction::EditVertex(edit) => {
      let entity_id = single_selection(app)?;
      apply_command(
        app,
        Command::EditVertex {
          entity_id,
          edit: *edit,
        },
      )
    }
    UiAction::AddText {
      text,
      height,
      origin,
    } => {
      let layer_id = drawing_layer(app)?;
      apply_minting(app, |first_entity_index| Command::AddText {
        layer_id: layer_id.clone(),
        first_entity_index,
        text: text.clone(),
        origin: *origin,
        height: *height,
      })
    }
    UiAction::DeleteSelection => {
      let selected: Vec<String> = app.selection().ids().map(str::to_owned).collect();
      for id in selected {
        app
          .execute(Command::DeleteEntity { entity_id: id })
          .map_err(|error| error.to_string())?;
      }
      app.clear_selection();
      Ok(ActionReport {
        label: action.label(),
        ..ActionReport::default()
      })
    }
    UiAction::SetToolpathToolNumber {
      toolpath_id,
      tool_number,
    } => apply_command(
      app,
      Command::SetToolpathToolNumber {
        toolpath_id: toolpath_id.clone(),
        tool_number: *tool_number,
      },
    ),
    UiAction::AddToolpath {
      toolpath_id,
      name,
      entity_ids,
      island_ids,
      tool_number,
      operation,
    } => apply_command(
      app,
      Command::AddToolpath {
        toolpath_id: toolpath_id.clone(),
        name: name.clone(),
        entity_ids: entity_ids.clone(),
        island_ids: island_ids.clone(),
        tool_number: *tool_number,
        operation: operation.clone(),
      },
    ),
    UiAction::SetToolpathOperation {
      toolpath_id,
      operation,
    } => apply_command(
      app,
      Command::SetToolpathOperation {
        toolpath_id: toolpath_id.clone(),
        operation: operation.clone(),
      },
    ),
    UiAction::SetToolpathName { toolpath_id, name } => apply_command(
      app,
      Command::SetToolpathName {
        toolpath_id: toolpath_id.clone(),
        name: name.clone(),
      },
    ),
    UiAction::SetSelection(ids) => {
      app.select(ids.clone());
      Ok(ActionReport {
        label: action.label(),
        ..ActionReport::default()
      })
    }
    UiAction::ToggleLayerVisible { layer_id } => {
      let layer = app
        .document()
        .layers
        .iter()
        .find(|layer| layer.id == *layer_id)
        .ok_or_else(|| format!("layer {layer_id:?} does not exist"))?;
      let visible = !layer.visible;
      apply_command(
        app,
        Command::SetLayerVisible {
          layer_id: layer_id.clone(),
          visible,
        },
      )
    }
  }
}

/// Write the drawing to `path`, carrying the archive entries the shell is holding.
///
/// [`cam_store::document::DocumentFile::save`] and not `save_document`: the latter builds a file with no
/// extras on purpose, which is right for a CLI document that authors a new file and wrong here, where the
/// entries belong to the file the operator opened. The store validates the document first and writes a
/// sibling temporary file before renaming it into place, so a save that fails leaves the previous bytes on
/// disk untouched and no truncating write ever reaches a drawing (S5).
///
/// The dirty flag clears only after the write returned: an error leaves it, and the recorded path, exactly
/// as they were.
fn write_document(
  app: &mut AppState,
  path: &Path,
  extras: &BTreeMap<String, Vec<u8>>,
) -> Result<(), String> {
  let file = cam_store::document::DocumentFile {
    document: app.document().clone(),
    extras: extras.clone(),
  };
  file.save(path).map_err(|error| error.to_string())?;
  app.mark_saved();
  Ok(())
}

/// The name a status message calls a file by: its own file name, or its whole path when it has none.
pub(crate) fn file_label(path: &Path) -> String {
  path
    .file_name()
    .map(|name| name.to_string_lossy().into_owned())
    .unwrap_or_else(|| path.display().to_string())
}

/// The layer new geometry is drawn into: the first one.
///
/// The shell never asks the user for a layer, so a document with no layer is a refusal the status bar
/// explains instead of a silent no-op.
fn drawing_layer(app: &AppState) -> Result<String, String> {
  app
    .document()
    .layers
    .first()
    .map(|layer| layer.id.clone())
    .ok_or_else(|| "the document has no layer to draw into".to_owned())
}

/// The selection, refusing an empty one.
fn selected_ids(app: &AppState) -> Result<Vec<String>, String> {
  let ids: Vec<String> = app.selection().ids().map(str::to_owned).collect();
  if ids.is_empty() {
    return Err("nothing is selected".to_owned());
  }
  Ok(ids)
}

/// The first selected entity: the operand of every single-target editing command.
///
/// A [`cam_app::Selection`] is ordered by id, so "first" is deterministic rather than pick order.
fn single_selection(app: &AppState) -> Result<String, String> {
  selected_ids(app)?
    .into_iter()
    .next()
    .ok_or_else(|| "nothing is selected".to_owned())
}

/// Apply one command and describe what it produced.
///
/// The command layer already guarantees a refusal leaves the document, the history and the dirty flag
/// untouched; this adds the label, the outcome and the selection of whatever the command created.
fn apply_command(app: &mut AppState, command: Command) -> Result<ActionReport, String> {
  let label = command.label();
  let outcome = app
    .execute_with_outcome(command)
    .map_err(|error| error.to_string())?;
  let report = ActionReport {
    label,
    created: outcome.created_entities.len(),
    created_entities: outcome.created_entities,
    max_radial_error: outcome.max_radial_error,
    lost_arcs: outcome.lost_arcs,
    note: None,
  };
  if !report.created_entities.is_empty() {
    app.select(report.created_entities.clone());
  }
  Ok(report)
}

/// Apply a drawing command that created exactly the one entity it was handed an id for.
///
/// [`cam_app::CommandOutcome::created_entities`] is only filled by commands that mint their own ids, so
/// a single-entity draw reports its count here instead of claiming nothing was created.
fn apply_draw(
  app: &mut AppState,
  command: Command,
  entity_id: &str,
) -> Result<ActionReport, String> {
  let mut report = apply_command(app, command)?;
  report.created = 1;
  report.created_entities = vec![entity_id.to_owned()];
  app.select(vec![entity_id.to_owned()]);
  Ok(report)
}

/// Apply a command that mints its own `E<n>` ids from a first index, keeping the id counter in step.
///
/// Batch commands (`ArrayCopy`, `OffsetContour`, `TrimEntity`, `AddText`) take a numeric first index
/// instead of an id, because how many ids they need is an output of the command. The shell therefore
/// reads the next free number from [`AppState::next_entity_id`] and, once the command has run,
/// consumes the rest of the ids it actually minted. Without that top-up the next draw would mint an id
/// the batch already took and be refused as a duplicate. A refused batch leaves one number unused,
/// which is invisible: ids are opaque strings.
fn apply_minting(
  app: &mut AppState,
  mint: impl FnOnce(u32) -> Command,
) -> Result<ActionReport, String> {
  let first_entity_index = entity_number(&app.next_entity_id())?;
  let report = apply_command(app, mint(first_entity_index))?;
  for _ in 1..report.created {
    app.next_entity_id();
  }
  Ok(report)
}

/// The number in an `E<n>` id, so a batch command can be handed its first index.
fn entity_number(id: &str) -> Result<u32, String> {
  id.strip_prefix('E')
    .and_then(|rest| rest.parse::<u32>().ok())
    .ok_or_else(|| format!("entity id {id:?} is not of the form E<number>"))
}
