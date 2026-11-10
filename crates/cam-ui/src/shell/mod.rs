use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use cam_app::ScreenPoint;
use cam_geom::Point2;
use cam_model::Job;
use cam_move::MoveStream;
use cam_sim::Heightfield;
use cam_store::tools::{StoredTool, ToolKind};
use cam_view::SnapKind;

use crate::default_material_size;
use crate::script_panel;
use crate::strategy;
use crate::theme::{self, Palette, Theme};
use crate::tool_library::ToolDraft;
use crate::view3d;
use crate::{Category, Preview3d, Tab};

mod action;
mod canvas;
mod frame;
mod input;
mod output;
mod paint;
mod panels;
mod status;
mod tool;

pub use output::{CanvasRect, DrawnState, PixelRect, ShellOutput};
pub use tool::{DrawShape, Tool};

/// The shell's own state: widget values and transient view flags.
///
/// # Where the numeric arguments come from
///
/// Authoring needs numbers the user does not type during a gesture: how many sides a polygon has, how
/// far an array copies, how deep an offset goes, how tall the text is. There is no wizard; each one is
/// an explicit field below with a documented default, editable in the tools area, and every button
/// reads the field into the [`UiAction`](crate::UiAction) it pushes. Tolerances are constants
/// ([`DEFAULT_TOLERANCE_MM`](crate::DEFAULT_TOLERANCE_MM), [`JOIN_TOLERANCE_MM`](crate::JOIN_TOLERANCE_MM))
/// because they are a promise about the result, not a per-shape taste.
#[derive(Clone, Debug)]
pub struct Shell {
  /// The tool armed on the canvas. Default [`Tool::Select`], which keeps M1's click-to-pick gesture.
  pub active_tool: Tool,
  /// Size used by the "Add rectangle" button. Default 100 x 60 mm.
  pub rectangle_size: (f64, f64),
  /// Side count used by the polygon tool. Default 6: a hexagon reads as a regular polygon at a glance.
  pub polygon_sides: usize,
  /// Tip count used by the star tool. Default 5.
  pub star_points: usize,
  /// Inner/outer radius ratio used by the star tool. Default 0.5, the classic five-point star.
  pub star_inner_ratio: f64,
  /// The colour set this shell draws in. [`Theme::Dark`] is the design default: a dark shop, a bright
  /// screen. Widgets never name a colour, so switching sets re-resolves the whole picture.
  pub theme: Theme,
  /// Rows and columns the array command uses. Default 2 x 3.
  pub array_grid: (usize, usize),
  /// Column and row spacing the array command uses, in millimetres. Default 50 x 25 mm.
  pub array_spacing: (f64, f64),
  /// Distance the offset command uses, in millimetres. Default 2 mm.
  pub offset_distance: f64,
  /// Displacement the move command uses, in millimetres. Default 10 x 0 mm.
  pub move_delta: (f64, f64),
  /// Rotation the rotate command uses, in degrees counter-clockwise. Default 15.
  pub rotate_degrees: f64,
  /// Factors the scale command uses. Default 2 x 2, which stays exact on arcs.
  pub scale_factors: (f64, f64),
  /// Vertex index the node-edit commands use. Default 0, the first vertex.
  pub vertex_index: usize,
  /// Segment index the insert-node command uses. Default 0, the first segment.
  pub segment_index: usize,
  /// Position the node-edit commands use, in millimetres. Default the origin.
  pub vertex_position: (f64, f64),
  /// String the text command draws. Default "CAM".
  pub text: String,
  /// Cap height the text command uses, in millimetres. Default 10 mm.
  pub text_height: f64,
  /// Whether the canvas is showing the 3D split (camui-prancheta S8).
  ///
  /// View state, not document state: like the camera, opening the split is not an edit and is not in the undo
  /// history (M5 A12). [`Shell::set_preview`] opens it, the `2D | 3D` toggle at the canvas' top right sets it
  /// ([`chrome`](crate::chrome)'s `view_toggle`), the rail's `3D` command flips it, and
  /// [`Shell::set_view_3d`] is how a host or a test says so directly.
  pub show_3d: bool,
  /// Which of the 3D half's layers its overlay row is showing (camui-prancheta S8).
  ///
  /// View state: hiding the toolpaths from the picture is a decision about what the screen shows, not about
  /// the program, so it is not an edit and not an undo step. Public and flat the way every other switch of
  /// this shell is, because a checkbox only a click can reach is a checkbox no test can preset.
  pub scene_layers: view3d::Layers,
  /// The program preview the 3D panel draws, once a caller has handed one in.
  pub(crate) preview: Option<Preview3d>,
  /// The tool library as a snapshot: every tool this window was given, in the library's own order.
  ///
  /// A snapshot and not a library: [`cam_store::tools::ToolLibrary`] owns a rusqlite connection and is
  /// not `Clone`, so the shell cannot hold one. It is filled from two places — an open resolves the
  /// library its document names, read from beside that document (shell-tools-ref S2, S3), and a host may
  /// hand one in with [`Shell::set_tools`]. It is empty after an open whose document names no library, or
  /// one that cannot be used, and empty is a real answer — the card then says the number it was asked for
  /// is not in the library it was given.
  pub(crate) tools: Vec<StoredTool>,
  /// The file the drawing was opened from, once a host has opened or saved one.
  ///
  /// `None` until [`UiAction::OpenFile`](crate::UiAction::OpenFile) or
  /// [`UiAction::SaveFile`](crate::UiAction::SaveFile) succeeds, which is when the app bar stops
  /// saying `untitled.c2d` and starts naming the file. The shell keeps the path so the bar can name it and
  /// [`UiAction::Save`](crate::UiAction::Save) can write to it: the shell shows no dialog of its own, so this is
  /// a record of what was read or written and not a file handle.
  pub(crate) document_path: Option<PathBuf>,
  /// The archive entries this build does not interpret, carried from the last load.
  ///
  /// [`cam_store::document::DocumentFile`] exists so an entry a later build understands is part of the
  /// document rather than something dropped on the floor, and opening is the moment it is known: the shell
  /// keeps these beside the path and [`UiAction::Save`](crate::UiAction::Save) writes them back under their
  /// own names. They are **file-level, not document-level** — the undo history snapshots bare
  /// [`cam_model::Document`]s, so an undo cannot restore or lose them, and a drawing that was never opened
  /// has no entries to carry.
  document_extras: BTreeMap<String, Vec<u8>>,
  /// The post-processor this window writes programs in, as its file label and its source text.
  ///
  /// A program is only defined together with the dialect it is written in, and the dialect is a choice:
  /// `cam-cli emit` takes it as `--post`. No document can carry that choice in a way a screen could reach —
  /// `Document.postp` names a post-processor but nothing in the workspace writes it (M13 L4) — so the choice
  /// is session state, exactly like [`Self::document_path`]: chosen once with the app bar's `Post…`, applied
  /// to every program this window writes. `None` is not "a default post": with no post the program is
  /// refused by name, because guessing a dialect would be writing somebody else's G-code.
  post: Option<(String, String)>,
  /// When the file behind the drawing was last written, as the filesystem reports it.
  ///
  /// Read once per action that opens or writes a document, never per frame: the brand block's `Saved …` line
  /// (camui-prancheta S4, L7) is a fact about the **file**, so it comes from the file's own mtime rather than
  /// from the moment this process happened to notice the write. `None` covers both "no file yet" and "the
  /// filesystem answered nothing about this one".
  saved_at: Option<SystemTime>,
  /// Message shown in the status bar.
  pub status: String,
  /// World position of the cursor, when it is over the canvas.
  pub cursor_world: Option<Point2>,
  /// Which snap kinds are enabled.
  pub snap_kinds: Vec<SnapKind>,
  /// Grid step in millimetres, as reported by the view for snapping.
  pub grid_spacing: f64,
  /// Whether snapping is active.
  pub snapping: bool,
  /// The snap point found for the cursor this frame, if any.
  pub snap_marker: Option<(SnapKind, Point2)>,
  /// Where a draw gesture started, in world millimetres, while one is in progress.
  pub draw_start: Option<Point2>,
  /// Where a draw gesture's cursor is, in world millimetres, for the preview.
  pub draw_current: Option<Point2>,
  /// Origin of a box selection in progress, in canvas-local pixels.
  drag_box: Option<ScreenPoint>,
  /// Whether the view still belongs to the application.
  ///
  /// Stays true until the user pans or zooms: opening the window, resizing it, or the panels settling
  /// must all end with the drawing framed, and only an explicit view gesture should take that away.
  auto_frame: bool,
  /// Whether a document has been framed at least once.
  ///
  /// A camera centred on the origin with 1 px/mm leaves a 1219 mm job almost entirely off screen, so
  /// the first frame that knows its canvas size frames the drawing. Motion is view state: it is not
  /// an undoable edit.
  framed: bool,
  /// The canvas rectangle from the previous frame.
  ///
  /// Panels are laid out before the central area, so the status bar can only show this frame's cursor
  /// if pointer tracking uses last frame's canvas geometry. That is the standard immediate-mode
  /// approach and it keeps the readout a frame fresh instead of a frame stale.
  last_canvas: CanvasRect,
  /// The set egui was last told about. `theme::apply` clones styles, so it runs when the set changes
  /// rather than once per frame.
  applied_theme: Option<Theme>,
  /// Whether the icon sheet is open. Until the shell has a rail to hang glyphs on, this is the only
  /// way to look at the whole set beside the rest of it.
  show_icons: bool,
  /// Which inspector tab is showing.
  pub(crate) inspector_tab: Tab,
  /// The ribbon category whose task editor is open, if any.
  ///
  /// `None` is not "nothing to show": it is the panel's idle state, which is the selection's numbers. The
  /// reference draws no selection category — selection is what the panel says while no task is open — which
  /// is why this is an `Option` rather than a fourth member of [`Category`].
  pub(crate) task: Option<Category>,
  /// Whether the layers block is expanded.
  ///
  /// View state of its own, and S6 asks for exactly that: `Camadas` collapses on its own, below the task
  /// editor, independently of which task is open.
  pub(crate) layers_open: bool,
  /// Whether the inspector is expanded. Collapsed it leaves a 12 px strip that reopens on hover.
  pub(crate) inspector_open: bool,
  /// The rail groups the operator has collapsed.
  ///
  /// View state, not document state: collapsing a group changes what the frame shows and nothing else, so
  /// it is neither an undoable edit nor a reason for the document to read as modified. Every group starts
  /// expanded, because a rail that hid its own commands on the first frame would teach nothing — and the
  /// reference opens expanded.
  pub(crate) collapsed_groups: Vec<crate::help::Group>,
  /// Whether the shortcuts window is open.
  pub(crate) show_shortcuts: bool,
  /// Whether the `F1` help panel is open.
  ///
  /// View state, like the icon sheet: opening the panel is not an edit and is not in the undo history.
  /// `F1` toggles it and `Esc` closes it, so the operator never has to aim at a close box to get the
  /// drawing back.
  pub show_help: bool,
  /// Whether the material dialog is open.
  ///
  /// View state: opening the dialog and cancelling it out of it are not edits, and only `Continue` writes.
  /// `Esc` closes it without writing, the same rule the help panel follows.
  pub show_material: bool,
  /// The three numbers the material dialog is editing, in millimetres: width X, height Y, thickness Z.
  ///
  /// Drafts, not the document: they are seeded from the job when the dialog opens and written back only
  /// when `Continue` is pressed, so a half-typed size never reaches the document and `Cancel` costs
  /// nothing. Public and flat the way every other parameter of this shell is ([`Self::rectangle_size`]),
  /// because a dialog whose numbers could not be set is a dialog that cannot be tested.
  pub material_size: (f64, f64, f64),
  /// Whether the import modal is open (shell-file-operations S3).
  ///
  /// View state: reading the file and showing what it holds is not an edit, and `Cancel` costs nothing, so
  /// `Esc` closes the modal and leaves the drawing exactly as it was. Only `Import` writes, and it writes the
  /// parse the modal described instead of reading the file a second time.
  pub show_import: bool,
  /// The file the import modal is describing, as the host named it.
  ///
  /// Held beside [`Self::import_parsed`] rather than inside it because the title names the file and
  /// [`Self::confirm_import`] names it in the sentence the status bar shows: the modal reports one file, and
  /// the import is that file's parse.
  pub import_path: Option<PathBuf>,
  /// What [`cam_import::import_path`] found in [`Self::import_path`], kept while the modal is open.
  ///
  /// `Some` is what makes the modal a report rather than a question: the counts it draws are this parse's own,
  /// so `Import` cannot import a file that changed on disk after the operator was shown what it held.
  pub import_parsed: Option<cam_import::Imported>,
  /// Why the last `Import` press could not write the document, while the modal stays open.
  ///
  /// `None` while there is nothing to say. A refusal is drawn inside the modal beside the report it is about,
  /// because closing on a failure would lose the file the operator chose and the counts they were reading.
  pub import_error: Option<String>,
  /// The flattening tolerance the import modal parses at, in millimetres (shell-file-operations S3).
  ///
  /// Seeded with [`cam_import::DEFAULT_TOLERANCE`], which is what [`cam_import::ImportOptions::default`] would
  /// have used, and **sticky for the session** like [`Self::post`]: changing it re-parses the file the modal is
  /// showing ([`Self::set_import_tolerance`]) and the next file opens at the same tolerance, because it is the
  /// operator's reading of how close a flattened curve has to be, not a fact about one file. Only an accepted
  /// re-parse writes it back — a refusal leaves the field on the number the report above it was parsed at.
  ///
  /// Public and flat the way every other parameter of this shell is, because a `DragValue` can only be edited
  /// from inside a frame and a field no test can set is a field nothing pins.
  pub import_tolerance: f64,
  /// Whether `Import` resizes the stock to the drawing it is importing (shell-file-operations S3).
  ///
  /// `true` by default, which is what the importer already did before the option existed: the job becomes the
  /// imported contours' bounding box plus [`cam_import::PLACEMENT_MARGIN`] on every side, the margin the
  /// importer places the geometry at, with the thickness left alone. `false` keeps the material the drawing
  /// being replaced already had — the sheet an operator set up for this job — and leaves the geometry where the
  /// importer put it.
  ///
  /// A draft until `Import` is pressed, like [`Self::material_size`]: ticking it in the modal writes nothing and
  /// `Cancel` costs nothing. Sticky for the session too, so a shop that imports onto its own sheets says so once.
  pub import_adjust_material: bool,
  /// Whether `Import` joins each layer's open contours into one cut path (shell-file-operations S3).
  ///
  /// The join is [`cam_geom::join::join`] with [`Self::import_tolerance`]. A layer whose open contours do not
  /// form exactly one chain — or that holds a closed contour — is imported unchanged: the joiner refuses rather
  /// than guessing, and a guessed chain is a cut path nobody asked for. Opt-in, because joining is a change to
  /// the drawing rather than a reading of it.
  ///
  /// A draft until `Import` is pressed, like [`Self::import_adjust_material`], and sticky for the session.
  pub import_join: bool,
  /// Whether `Import` smooths each contour with Douglas–Peucker at [`Self::import_tolerance`]
  /// (shell-file-operations S3).
  ///
  /// Only polyline contours are simplified: a contour that still carries arcs is kept exactly, because dropping
  /// vertices from it would also drop the arcs, and smoothing must not change the shape it cannot measure.
  /// Opt-in and sticky, for the reason [`Self::import_join`] is.
  pub import_smooth: bool,
  /// Whether `Import` centers the drawing on the material (shell-file-operations S3).
  ///
  /// `true` by default. When the stock is fitted to the drawing this is exactly where the importer already put
  /// it — the stock is the drawing plus [`cam_import::PLACEMENT_MARGIN`] on every side — so the default import
  /// is unchanged. On a sheet the operator already has, it centers the drawing on that sheet instead of leaving
  /// it at the importer's bottom-left margin. Unchecked, the geometry keeps that bottom-left placement.
  ///
  /// A draft until `Import` is pressed, and sticky, like the other options.
  pub import_center: bool,
  /// Whether the tool-library modal is open (camui-prancheta S9).
  ///
  /// View state: opening the modal, searching it and cancelling out of it are not edits, and only
  /// `Use this tool` writes. `Esc` closes it without writing, the rule the material dialog already follows.
  pub show_tool_library: bool,
  /// Whether the script modal is open (M10 T4b, S4).
  ///
  /// View state, with one exception it is worth stating: opening the panel, typing a path into it and
  /// closing it again are not edits, and `Run` edits **the shell's document** in one undo step (M11) — so
  /// `Esc` has nothing of its own to undo, and what a run changed survives closing the window. The panel
  /// itself is [`Self::script`].
  pub show_script: bool,
  /// The scripting panel the Script modal draws: the path, `Run`, and the last report.
  ///
  /// A draft of the frame's own, the rule [`Self::material_size`] follows: the path field and the report
  /// reach nothing else. Reached through [`Self::script_panel`], because a headless test has to be able to
  /// name a script and press `Run`.
  pub(crate) script: script_panel::ScriptPanel,
  /// The text the tool-library modal's search field holds.
  ///
  /// A draft of the frame's own, like [`Self::material_size`]: it filters the tree and reaches nothing else,
  /// so `Cancel` has nothing to undo about it. Public because a search field whose text could not be set
  /// from outside a frame is a filter only a keyboard could test.
  pub tool_search: String,
  /// The toolpath `Use this tool` will retool, by id — this is what opened the modal.
  ///
  /// Captured when the modal opens and re-read from the document on every frame: a toolpath the operator
  /// deleted (or an undo restored) between opening and committing is a state the modal has to answer for,
  /// and it answers by not drawing `Use this tool` and saying why (camui-prancheta S9, L6).
  pub(crate) tool_library_target: Option<String>,
  /// The tool the modal's detail side is showing, by the library's own geometry id.
  ///
  /// A draft too: choosing a row changes what the form and the drawing describe, and nothing else. Only
  /// `Use this tool` turns the choice into an action.
  pub(crate) tool_library_selected: Option<String>,
  /// The draft the modal's detail column is editing, and the text of every box it draws.
  ///
  /// One draft at a time, seeded from the row the tree selected ([`crate::tool_library::dialog`] holds the rule
  /// and the reason a draft for a row the operator leaves is thrown away). Public and flat the way every other
  /// draft of this shell is ([`Self::material_size`]), because a form only a keyboard could fill is a form no
  /// test can pin: `Use this tool` writes the draft and `Cancel` discards it, and nothing else reaches it.
  pub tool_draft: Option<ToolDraft>,
  /// The tree's kind headings the operator has folded shut, by kind.
  ///
  /// View state, like [`Self::collapsed_groups`]: folding a heading changes what the column shows and nothing
  /// else, so it is not an edit and not an undo step. Every heading starts open, and the list is cleared when
  /// the modal closes so the next visit does not inherit a fold nobody made in it.
  pub tool_groups_closed: Vec<ToolKind>,
  /// Whether the library was opened by the strategy editor while it is drafting a **new** toolpath (S10).
  ///
  /// The modal's commit writes one thing: a tool number. With a toolpath on screen that is a document edit
  /// ([`UiAction::SetToolpathToolNumber`](crate::UiAction::SetToolpathToolNumber)); with only a draft it can
  /// only mean "this strategy will cut with this tool", so the number lands in [`Shell::strategy`] and
  /// `Generate toolpath` is what writes it. The flag is what tells the two apart, and it is cleared whenever
  /// the modal closes.
  pub(crate) tool_library_for_draft: bool,
  /// The library file this window reads and writes, when the document named one.
  ///
  /// `None` while the snapshot came from a host ([`Shell::set_tools`]) rather than from a file beside an opened
  /// document: a window that does not know where a library lives cannot write one, so `Use this tool` updates
  /// the snapshot and says nothing about a file. An open that resolves a library fills it
  /// ([`Shell::set_tools_from`]), and [`Shell::set_tools`] clears it, because a snapshot handed in has no file
  /// behind it to write.
  pub(crate) tool_library_path: Option<PathBuf>,
  /// Why the last `Use this tool` could not write the library, while the modal stays open.
  ///
  /// `None` while there is nothing to say. The sentence is drawn inside the modal instead of the status bar,
  /// for the reason [`Self::import_error`] is: closing on a failure would lose the form the operator was
  /// editing, and the next opening clears it.
  pub(crate) tool_error: Option<String>,
  /// The geometry ids a library write recorded since the last frame reported one.
  ///
  /// `Export…` and `Import…` are answered by the host **between** two frames — the dialog returns a path and
  /// [`Shell::apply_action`] writes it, with no [`ShellOutput`] to report into — so the ids wait here and the
  /// next frame that draws the modal moves them into `drawn.tool_saved` and empties this
  /// ([`crate::tool_library::dialog`], where a write the modal made itself reports itself too). A write the
  /// modal's own rows make reports straight into the frame and never touches this.
  pub(crate) tool_saved_pending: Vec<String>,
  /// The toolpath the operator selected in the `TOOLPATHS` surface's list, by id (M13 T3, S4).
  ///
  /// View state, like [`Self::show_3d`] or the collapsed rail groups: choosing which toolpath the card and
  /// the editor describe is a decision about the screen, so it is neither an edit nor an undo step, and the
  /// document is not written. `None` is not "no toolpath" — it is "no choice", which reads as the document's
  /// first toolpath, the answer this shell gave before the list existed. A selection that outlives its
  /// toolpath (a delete, or a document opened over it) reads the same way rather than going stale
  /// ([`Shell::described_toolpath`]).
  pub(crate) selected_toolpath: Option<String>,
  /// The strategy editor's draft: the name, the tool and the operation `Generate toolpath` writes
  /// (camui-prancheta S10).
  ///
  /// Public and flat the way [`Self::material_size`] is, because a draft whose numbers could not be set from
  /// outside a frame is a draft only a mouse could test. It is re-seeded from the document whenever the editor's
  /// subject changes ([`strategy::Draft::sync`]), and `Cancel` throws it away.
  pub strategy: strategy::Draft,
  /// The command the help panel is teaching: whichever row was pressed last, or the first one.
  pub(crate) help_focus: crate::help::Command,
  /// What the canvas context menu is scoped to, as of the last right-click on the canvas.
  ///
  /// View state, like [`Shell::drag_box`]: it is what the menu offers, never what the document holds. The
  /// right-click decides it ([`Shell::handle_input`]) because the selection cannot — a right-click on empty
  /// canvas keeps an existing selection and still means the canvas menu — and it is re-decided on every
  /// right-click, so a menu can never outlive the click that opened it.
  pub(crate) menu_scope: crate::help::MenuScope,
}

impl Default for Shell {
  fn default() -> Self {
    Self {
      active_tool: Tool::Select,
      rectangle_size: (100.0, 60.0),
      polygon_sides: 6,
      star_points: 5,
      star_inner_ratio: 0.5,
      theme: Theme::Dark,
      array_grid: (2, 3),
      array_spacing: (50.0, 25.0),
      offset_distance: 2.0,
      move_delta: (10.0, 0.0),
      rotate_degrees: 15.0,
      scale_factors: (2.0, 2.0),
      vertex_index: 0,
      segment_index: 0,
      vertex_position: (0.0, 0.0),
      text: "CAM".to_owned(),
      text_height: 10.0,
      show_3d: false,
      scene_layers: view3d::Layers::default(),
      preview: None,
      tools: Vec::new(),
      document_path: None,
      document_extras: BTreeMap::new(),
      post: None,
      saved_at: None,
      status: String::new(),
      cursor_world: None,
      snap_kinds: vec![
        SnapKind::Endpoint,
        SnapKind::Midpoint,
        SnapKind::Center,
        SnapKind::Grid,
      ],
      grid_spacing: 0.0,
      snapping: true,
      snap_marker: None,
      draw_start: None,
      draw_current: None,
      drag_box: None,
      auto_frame: true,
      framed: false,
      last_canvas: CanvasRect::default(),
      applied_theme: None,
      show_icons: false,
      inspector_tab: Tab::default(),
      task: None,
      layers_open: true,
      inspector_open: true,
      collapsed_groups: Vec::new(),
      show_shortcuts: false,
      show_help: false,
      // Closed at birth, like every other panel here: a dialog that opens by itself swallows the first
      // click on the canvas, which is what `cam-ui --test input` caught on eighteen tests. The Job tab
      // is how it opens (L108's lesson: a temporary default nobody meant to ship).
      show_material: false,
      material_size: default_material_size(),
      // Closed at birth for the material dialog's reason: a modal that opened by itself would swallow the
      // first click on the canvas, and only an `ImportFile` action opens this one.
      show_import: false,
      import_path: None,
      import_parsed: None,
      import_error: None,
      import_tolerance: cam_import::DEFAULT_TOLERANCE,
      import_adjust_material: true,
      import_join: false,
      import_smooth: false,
      import_center: true,
      show_tool_library: false,
      show_script: false,
      script: script_panel::ScriptPanel::new(),
      tool_search: String::new(),
      tool_library_target: None,
      tool_library_selected: None,
      tool_draft: None,
      tool_groups_closed: Vec::new(),
      tool_library_for_draft: false,
      tool_library_path: None,
      tool_error: None,
      tool_saved_pending: Vec::new(),
      selected_toolpath: None,
      strategy: strategy::Draft::default(),
      help_focus: crate::help::Command::ALL[0],
      menu_scope: crate::help::MenuScope::default(),
    }
  }
}

impl Shell {
  /// A shell with default widget values.
  pub fn new() -> Self {
    Self::default()
  }

  /// The colour set this shell draws in.
  pub fn theme(&self) -> Theme {
    self.theme
  }

  /// Whether the 3D view is the one showing.
  pub fn view_3d(&self) -> bool {
    self.show_3d
  }

  /// The command the help panel is currently teaching.
  ///
  /// The panel opens on the first command in the table and follows the last row the operator pressed,
  /// so a test can state which command's sentences it is about to look for.
  pub fn help_focused(&self) -> crate::help::Command {
    self.help_focus
  }

  /// Show the 3D view (`true`) or the drawing (`false`).
  pub fn set_view_3d(&mut self, showing: bool) {
    self.show_3d = showing;
  }

  /// Switch the colour set. It takes effect on the next frame, because a frame is what publishes it.
  pub fn set_theme(&mut self, theme: Theme) {
    self.theme = theme;
  }

  /// The palette of the current set, resolved once per frame by whoever needs several roles.
  pub fn palette(&self) -> Palette {
    self.theme.palette()
  }

  /// The measurements of the current set.
  pub fn metrics(&self) -> theme::Metrics {
    self.theme.metrics()
  }

  /// The scripting panel the Script modal draws, so a headless test can name a script and read the point
  /// `Run` was drawn at (M11 T2).
  ///
  /// The modal is still the only thing that *paints* it, and the shell still owns the commit: a run
  /// mutates a copy and the copy reaches [`AppState`](cam_app::AppState) only through
  /// [`AppState::replace_document_undoable`](cam_app::AppState::replace_document_undoable).
  /// This exists for the same reason the palette reports its chips' points — a widget a test cannot
  /// press is a widget nothing pins.
  pub fn script_panel(&mut self) -> &mut script_panel::ScriptPanel {
    &mut self.script
  }

  /// The file the drawing was opened from or last written to, once a host has opened or saved one.
  ///
  /// `None` while the frame holds a document that has no file yet, which is the state the app bar still
  /// calls `untitled.c2d` and the state in which [`UiAction::Save`](crate::UiAction::Save) is refused.
  pub fn document_path(&self) -> Option<&Path> {
    self.document_path.as_deref()
  }

  /// The words the brand block shows at the foot of the rail: `Saved 2026-11-03 19:30 UTC`, or
  /// `Unsaved changes`.
  ///
  /// One source for the line the frame paints and the line a test reads, which is the only way the two can be
  /// the same sentence. The rule is L7's: the line stamps the **document**, so it is the file's own last write
  /// ([`Self::saved_at`]) and never a build stamp; a document with no file, or a filesystem that reported no
  /// mtime, has nothing saved to name. A document with unsaved edits reads `Unsaved changes` too — the panel
  /// asks [`AppState::is_dirty`](cam_app::AppState::is_dirty) and draws that word itself, because dirty is the
  /// document's fact and the stamp is the file's.
  pub fn saved_stamp(&self) -> String {
    match self.saved_at {
      Some(at) => format!("Saved {}", crate::stamp::format_stamp(at)),
      None => "Unsaved changes".to_owned(),
    }
  }

  /// The archive entries this build does not interpret, carried from the last load.
  ///
  /// Reported so a host or a test can see what a save will write back without opening the file: the shell
  /// writes them through [`cam_store::document::DocumentFile`] and never inspects them.
  pub fn document_extras(&self) -> &BTreeMap<String, Vec<u8>> {
    &self.document_extras
  }

  /// The name the app bar shows for the drawing.
  ///
  /// The opened or saved file's own name once there is one, and `untitled.c2d` before that: the shell had no
  /// document path of its own until a host could hand it one, which is why the bar named the frame rather
  /// than a file (M0).
  pub fn document_name(&self) -> String {
    self
      .document_path
      .as_ref()
      .and_then(|path| path.file_name())
      .map(|name| name.to_string_lossy().into_owned())
      .unwrap_or_else(|| "untitled.c2d".to_owned())
  }

  /// Hand the 3D panel a program to preview: the stock it cuts, the program itself, and the stock box.
  ///
  /// `field` must be the **untouched** stock — [`Heightfield::new`] for `job`, at the cell size the
  /// caller chose — and `program` the moves to cut it with. The panel plays it (M5 A10): as the
  /// cursor advances, whole moves are applied to the field, so the surface a viewer watches is the
  /// same simulation [`cam_sim::sweep::apply`] gives the report, cell for cell.
  ///
  /// This method reads no document and returns no [`UiAction`](crate::UiAction), which is what keeps the
  /// preview off the document's authoring path (M5 D-M5-9, A12): it takes a program a caller has already built.
  /// `Shell::simulate` in `view3d` is that caller inside the shell (M13 S3) — it generates the open
  /// document's own toolpaths and hands the result here, so a person never leaves the window for a preview,
  /// and the preview still edits nothing.
  ///
  /// The panel opens with the first preview, because a caller that hands one in means to look at it;
  /// [`Shell::show_3d`] closes it again without discarding the preview or its cursor, and
  /// [`Self::clear_preview`] discards both.
  pub fn set_preview(&mut self, field: Heightfield, program: MoveStream, job: &Job) {
    self.preview = Some(Preview3d::new(field, program, job));
    self.show_3d = true;
  }

  /// Drop the preview and close the panel.
  pub fn clear_preview(&mut self) {
    self.preview = None;
    self.show_3d = false;
  }

  /// Hand the shell the tool library as a snapshot, for the current-tool card to resolve numbers with.
  ///
  /// A snapshot rather than a library because [`cam_store::tools::ToolLibrary`] owns a connection: the
  /// host decides where the library comes from and when it is read, and the shell only resolves the
  /// numbers the document's toolpaths name. An open is the one exception, and it is not a second rule:
  /// it resolves the library the loaded document names from beside that document (shell-tools-ref S2)
  /// and calls this with the entries. Replacing the snapshot replaces what the card can describe, and an
  /// empty one is legal — it makes every number unresolved, which is the state the card says out loud
  /// instead of drawing a blank tool.
  pub fn set_tools(&mut self, tools: Vec<StoredTool>) {
    self.tools = tools;
    // A snapshot handed in is not a file: the shell must not go on writing the library the previous drawing
    // named, so the path goes with it (see [`Self::tool_library_path`]).
    self.tool_library_path = None;
  }

  /// Hand the shell the library an opened document named, together with the file it was read from.
  ///
  /// The same snapshot as [`Self::set_tools`] with the one difference a write needs: the path. It is
  /// `pub(crate)` because only the open handler resolves a library from beside a document (shell-tools-ref S2),
  /// and a host that hands in a library of its own has no file for this window to write.
  pub(crate) fn set_tools_from(&mut self, path: PathBuf, tools: Vec<StoredTool>) {
    self.tools = tools;
    self.tool_library_path = Some(path);
  }

  /// The preview the 3D panel draws, when a caller has handed one in.
  pub fn preview(&self) -> Option<&Preview3d> {
    self.preview.as_ref()
  }

  /// The preview, for a caller that frames the camera directly instead of by a gesture.
  pub fn preview_mut(&mut self) -> Option<&mut Preview3d> {
    self.preview.as_mut()
  }
}
