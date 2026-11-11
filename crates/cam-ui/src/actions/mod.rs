use std::path::PathBuf;

use cam_app::{AlignMode, Transform, VertexEdit};
use cam_geom::Point2;
use cam_model::Operation;

use crate::DrawShape;

mod handle;

pub use handle::handle_action;
pub(crate) use handle::{file_label, handle_action_with_file, DocumentFileState};

/// What one action produced, as the status bar reports it.
///
/// The command layer hands back a [`cam_app::CommandOutcome`]; this is that outcome, labelled and
/// ready to read. It exists so a refusal and a successful-but-lossy command can both reach the user
/// instead of being swallowed by the shell.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ActionReport {
  /// The command's own label, for example "add rectangle".
  pub label: &'static str,
  /// How many entities the action created.
  pub created: usize,
  /// The ids of the entities it created, in creation order.
  pub created_entities: Vec<String>,
  /// The largest radial error of an arc approximation, when one was made.
  pub max_radial_error: Option<f64>,
  /// How many arcs the action lost to tessellation (M2 A5).
  pub lost_arcs: usize,
  /// What the action did beyond its command outcome, for actions that are not one command.
  ///
  /// An import reports its own counts here (`12 contours from part.dxf, 0 curves flattened,
  /// 1 skipped`); every other action leaves it empty, so their sentences stay exactly as they were.
  pub note: Option<String>,
}

impl ActionReport {
  /// The sentence the status bar shows.
  pub fn message(&self) -> String {
    let mut details: Vec<String> = Vec::new();
    if self.created > 0 {
      details.push(match self.created {
        1 => "1 entity created".to_owned(),
        count => format!("{count} entities created"),
      });
    }
    if let Some(error) = self.max_radial_error {
      details.push(format!("radial error {error:.4} mm"));
    }
    if self.lost_arcs > 0 {
      details.push(format!("{} arc(s) lost to tessellation", self.lost_arcs));
    }
    if let Some(note) = &self.note {
      details.push(note.clone());
    }
    if details.is_empty() {
      self.label.to_owned()
    } else {
      format!("{} — {}", self.label, details.join(", "))
    }
  }
}

/// Everything the user can ask for. Every button, click and shortcut becomes one of these.
#[derive(Clone, Debug, PartialEq)]
pub enum UiAction {
  /// Step one command back.
  Undo,
  /// Step one command forward.
  Redo,
  /// Frame the whole drawing.
  ZoomToFit,
  /// Replace the stock size. Values are millimetres and must be positive.
  ///
  /// The material dialog is the only thing that pushes this today, and it is the shell's first
  /// *document-level* edit: everything else here changes geometry inside a job whose size was taken as
  /// given. The three positivity rules travel with [`cam_app::Command::SetJob`] rather than with this
  /// action, so a caller feeding it a zero is refused in the same words wherever it came from.
  SetJob {
    /// Stock size along X, in millimetres.
    width: f64,
    /// Stock size along Y, in millimetres.
    height: f64,
    /// Stock thickness, in millimetres.
    thickness: f64,
  },
  /// Add a rectangle of an explicit size, centred on the current view.
  ///
  /// This is the parameter-driven path M1 shipped and the command bar still offers; the canvas
  /// gesture path is [`UiAction::Draw`].
  AddRectangle {
    /// Width in millimetres.
    width: f64,
    /// Height in millimetres.
    height: f64,
  },
  /// Draw one shape, in world millimetres.
  Draw(DrawShape),
  /// Apply one transform to the selection.
  Transform(Transform),
  /// Copy the selection into a grid of duplicates.
  Array {
    /// Number of rows, at least one.
    rows: usize,
    /// Number of columns, at least one.
    columns: usize,
    /// Spacing between columns, in millimetres along X.
    dx: f64,
    /// Spacing between rows, in millimetres along Y.
    dy: f64,
  },
  /// Align the selection on an edge or centre line of its own bounds.
  Align(AlignMode),
  /// Offset the first selected entity by `distance` and store the result as a new entity.
  Offset {
    /// Distance in millimetres; positive grows a counter-clockwise contour.
    distance: f64,
  },
  /// Join the selection into one contour.
  Join,
  /// Trim `target` against `cutter`, keeping the piece the kernel returns.
  Trim {
    /// Entity to trim.
    target: String,
    /// The cutting contour.
    cutter: String,
  },
  /// Apply one node edit to the first selected entity.
  EditVertex(VertexEdit),
  /// Draw `text` as one entity per glyph stroke.
  AddText {
    /// The string to lay out.
    text: String,
    /// Cap height in millimetres.
    height: f64,
    /// Baseline start of the first line, in millimetres.
    origin: Point2,
  },
  /// Delete the selected geometry.
  DeleteSelection,
  /// Replace the selection.
  SetSelection(Vec<String>),
  /// Show or hide a layer.
  ToggleLayerVisible {
    /// The layer to change.
    layer_id: String,
  },
  /// Import a file and replace the drawing with it, undoably (M3 A17).
  ///
  /// The path comes from the caller: there is no file dialog in this milestone (M3 D-M3-6), which is
  /// what keeps the shell testable without a display.
  ImportFile {
    /// The file to read; its extension decides the format.
    path: String,
  },
  /// Open a `.c2d` document and replace the drawing with it.
  ///
  /// The path comes from the caller — the shell has no file dialog and no dialog dependency, so a host
  /// answers [`UiAction::RequestOpenFile`] with the platform's picker and applies this action with what
  /// it returned. The load is [`cam_store::document::DocumentFile::load`], the same archive scan,
  /// migration and validation the CLI's private loader performs.
  ///
  /// The replacement is **not undoable**, and the reason is an invariant rather than a taste for fewer
  /// history entries: the open replaces the drawing and the file the shell records in one step, so an undo
  /// that crossed that boundary would leave the recorded path naming a file whose drawing is no longer on
  /// screen — and the next [`UiAction::Save`] would overwrite that file with the wrong document, losing its
  /// archive entries. Clearing the history is what makes "the recorded path describes what is on screen"
  /// true by construction. The edits an undo step used to protect are protected instead by the refusal: an
  /// open over unsaved work is refused before anything is read.
  OpenFile {
    /// The `.c2d` to load.
    path: PathBuf,
  },
  /// Ask the host for a document to open; the host shows its file dialog and applies [`UiAction::OpenFile`].
  ///
  /// An intent and nothing more: the shell does not open a dialog, read a path or touch the document when
  /// it receives this, which is what keeps `cam-ui` free of a dialog crate and testable without a display.
  RequestOpenFile,

  /// Start a fresh document and open the material dialog, so the first thing defined is the stock.
  ///
  /// An in-shell action: no host dialog is involved, because the dialog the reference shows is the shell's
  /// own (`Novo trabalho — defina o material`), not the platform's file picker.
  NewProject,
  /// Ask the host for a file to import (DXF, SVG, PDF, EPS or PNG); the host shows its picker and applies
  /// [`UiAction::ImportFile`] with the path it returned.
  ///
  /// An intent and nothing more, for the reason [`UiAction::RequestOpenFile`] is one.
  RequestImportFile,
  /// Write the drawing back to the file the shell recorded.
  ///
  /// The write is [`cam_store::document::DocumentFile::save`], carrying the archive entries this build does
  /// not interpret, so a load/save cycle never drops an entry a later build understands. With no recorded
  /// path there is nowhere to write and the action is **refused** with a sentence pointing at
  /// [`UiAction::RequestSaveAs`]; on success the dirty flag clears; on any failure the dirty flag and the
  /// recorded path stay exactly as they were and the status bar says `refused: …`.
  Save,
  /// Write the drawing to `path` and make it the file the app bar names.
  ///
  /// This is the answer to [`UiAction::RequestSaveAs`]: the host asks the platform's save dialog and applies
  /// this action with the path it returned, exactly as [`UiAction::OpenFile`] answers
  /// [`UiAction::RequestOpenFile`]. The shell writes nothing when the dialog is cancelled, because a
  /// cancelled dialog pushes no action.
  SaveFile {
    /// Where to write the archive.
    path: PathBuf,
  },
  /// Ask the host for a path to save under; the host shows its save dialog and applies [`UiAction::SaveFile`].
  ///
  /// An intent and nothing more, for the reason [`UiAction::RequestOpenFile`] is one: the shell shows no
  /// dialog, writes no file and touches no document when it receives this.
  RequestSaveAs,
  /// Give an existing toolpath a different tool number, leaving its geometry and operation alone.
  ///
  /// The tool library's `Use this tool` is what pushes this (camui-prancheta S9): the modal chooses a tool
  /// from the library snapshot, and the one thing the choice can change about the document is the number the
  /// toolpath cuts with. [`cam_app::Command::SetToolpathToolNumber`] is the rule, and it takes the number as
  /// it stands — a number the library does not hold is refused at emission time, where the library is
  /// actually read, not here.
  SetToolpathToolNumber {
    /// Identifier of the toolpath to retool.
    toolpath_id: String,
    /// The new tool number.
    tool_number: u32,
  },
  /// Replace one toolpath's operation, with its parameters, and leave the rest of it alone.
  ///
  /// This is `Generate toolpath` while the editor is editing an existing strategy (camui-prancheta S10): the
  /// geometry, the tool and the name are untouched, and the document's own validation refuses an operation that
  /// cannot cut the toolpath's selection.
  SetToolpathOperation {
    /// Identifier of the toolpath to change.
    toolpath_id: String,
    /// The new operation and its parameters.
    operation: Operation,
  },
  /// Rename one toolpath, leaving its geometry, its tool and its operation alone.
  ///
  /// This is the strategy editor's `Generate toolpath` while editing, and it is pushed only when the name field
  /// holds something other than the name the document already has: the editor is where a strategy is named, and
  /// the document is where the name lives. [`cam_app::Command::SetToolpathName`] is the rule, including its
  /// refusal of a name that is empty or only whitespace.
  SetToolpathName {
    /// Identifier of the toolpath to rename.
    toolpath_id: String,
    /// The new display name.
    name: String,
  },
  /// Write the tool library to `path` as the file format's JSON (tool-library-editor S11).
  ///
  /// This is the answer to [`UiAction::RequestExportTools`], exactly as [`UiAction::SaveFile`] answers
  /// [`UiAction::RequestSaveAs`]: the host asks the platform's save dialog and applies this with the path it
  /// returned. The window's own library is what is written — the tools the tree lists — and nothing else is
  /// read or written, so a cancelled dialog writes no file at all.
  ExportTools {
    /// Where to write the library file.
    path: PathBuf,
  },
  /// Ask the host for a path to export the tool library to; the host shows its save dialog and applies
  /// [`UiAction::ExportTools`].
  ///
  /// An intent and nothing more, for the reason [`UiAction::RequestOpenFile`] is one.
  RequestExportTools,
  /// Read the tool library in `path` and replace this window's library with it (tool-library-editor S11).
  ///
  /// This is the answer to [`UiAction::RequestImportTools`]. The file is validated before anything is written,
  /// so a file this build cannot use changes neither the library file beside the document nor the window's
  /// snapshot; on success the library file is replaced and the snapshot follows it.
  ImportTools {
    /// The library file to read.
    path: PathBuf,
  },
  /// Ask the host for a tool library file to import; the host shows its open dialog and applies
  /// [`UiAction::ImportTools`].
  ///
  /// An intent and nothing more, for the reason [`UiAction::RequestOpenFile`] is one.
  RequestImportTools,
}

impl UiAction {
  /// A short label for the status bar.
  pub fn label(&self) -> &'static str {
    match self {
      Self::Undo => "undo",
      Self::Redo => "redo",
      Self::ZoomToFit => "zoom to fit",
      Self::SetJob { .. } => "set job size",
      Self::AddRectangle { .. } => "add rectangle",
      Self::Draw(shape) => shape.label(),
      Self::Transform(transform) => match transform {
        Transform::Translate { .. } => "move entities",
        Transform::Rotate { .. } => "rotate entities",
        Transform::Scale { .. } => "scale entities",
        Transform::Mirror { .. } => "mirror entities",
      },
      Self::Array { .. } => "array copy",
      Self::Align(_) => "align entities",
      Self::Offset { .. } => "offset contour",
      Self::Join => "join entities",
      Self::Trim { .. } => "trim entity",
      Self::EditVertex(edit) => match edit {
        VertexEdit::Move { .. } => "move vertex",
        VertexEdit::Insert { .. } => "insert vertex",
        VertexEdit::Delete { .. } => "delete vertex",
      },
      Self::AddText { .. } => "add text",
      Self::DeleteSelection => "delete selection",
      Self::SetSelection(_) => "select",
      Self::ToggleLayerVisible { .. } => "layer visibility",
      Self::ImportFile { .. } | Self::RequestImportFile => "import file",
      Self::OpenFile { .. } | Self::RequestOpenFile => "open file",
      Self::NewProject => "new project",
      Self::Save => "save document",
      Self::SaveFile { .. } | Self::RequestSaveAs => "save document as",
      Self::SetToolpathToolNumber { .. } => "set toolpath tool",
      Self::SetToolpathOperation { .. } => "set toolpath operation",
      Self::SetToolpathName { .. } => "set toolpath name",
      Self::ExportTools { .. } | Self::RequestExportTools => "export tools",
      Self::ImportTools { .. } | Self::RequestImportTools => "import tools",
    }
  }
}
