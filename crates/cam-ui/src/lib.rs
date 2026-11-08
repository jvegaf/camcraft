//! The egui editor shell: panels, canvas and input wiring.
//!
//! # The rule this crate exists to keep
//!
//! The shell contains no business rules. It turns input into [`UiAction`] values, paints
//! [`cam_view::Scene`], and reports what it displayed. [`handle_action`] is the only place that
//! touches [`AppState`], which is what lets the whole interface be tested headless: a test builds a
//! frame with a synthetic input, inspects the actions and the painted text, then applies the actions
//! and asserts on the resulting state.
//!
//! # Authoring (M2)
//!
//! Every M2 command is one [`UiAction`] handled by [`handle_action`]: [`UiAction::Draw`] carries the
//! shape a canvas drag described, and [`UiAction::Transform`], [`UiAction::Array`],
//! [`UiAction::Align`], [`UiAction::Offset`], [`UiAction::Join`], [`UiAction::Trim`],
//! [`UiAction::EditVertex`] and [`UiAction::AddText`] carry the edit. The editing groups read their
//! operands from the selection, so the selection *is* the operand of every edit and no command needs
//! a wizard. Numbers the user does not type during a gesture come from [`Shell`]'s documented fields
//! and from [`DEFAULT_TOLERANCE_MM`] / [`JOIN_TOLERANCE_MM`]; [`Shell::apply_action`] then puts what
//! the command produced — entity count, ellipse error, lost arcs, or the kernel's refusal — in the
//! status bar.
//!
//! Arcs are flattened **here**, at half a pixel, and nowhere earlier.
//!
//! # The 3D half (M5 V2, V3; camui-prancheta S8)
//!
//! [`view3d`] adds the stock's 3D picture to the shell: it paints a simulated [`cam_sim::Heightfield`]
//! through `egui::Mesh`, takes orbit and wheel-zoom on the same painter, and draws the program's moves as
//! projected 3D lines. Since camui-prancheta T6 it is a **half** of the canvas rather than the whole of it:
//! `3D` is a mode that splits the drawing area into the 2D half and the 3D half, and the 3D half carries the
//! overlay row, the home button, the axis triad and the playback bar S8 asks for. It stays **additive**: the
//! 2D half keeps painting, picking and drawing beside it (A12), and the half never authors geometry or touches
//! the document (D-M5-9) — the caller hands in the untouched stock and the program, and the half plays it
//! (A10).
//!
//! Playback is the one thing in this crate that needs a clock, and [`Shell::show_in`] is the only
//! place that reads one: `ctx.input(|input| input.stable_dt)` becomes the delta
//! [`Preview3d::advance`] consumes, so the arithmetic itself stays a pure function a test drives.
//!
//! Must not contain geometry, toolpath or document rules.

#![warn(missing_docs)]

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use cam_app::{AlignMode, AppState, Camera, Command, ScreenPoint, Transform, VertexEdit, Viewport};
use cam_geom::Point2;
use cam_model::{Job, Operation};
use cam_move::MoveStream;
use cam_sim::Heightfield;
use cam_store::tools::{StoredTool, ToolLibrary};
use cam_view::{
  box_select, build_grid, build_scene, choose_snap, pick, SceneItem, Snap, SnapKind, Style,
  PICK_TOLERANCE_PX, SNAP_TOLERANCE_PX, TARGET_GRID_PIXELS,
};
use egui::{Context, Pos2, RichText, Sense, Stroke};

use crate::icons::{Family, Icon};
use crate::theme::{Palette, Role, Theme};

pub mod chrome;
/// The command table and its teaching voice.
pub mod help;
pub mod icons;
/// The seven commands whose numbers travel with their builders: `Add text`, `Move`, `Rotate`, `Scale`,
/// `Mirror`, `Array` and `Offset`.
pub mod params;
/// The two millimetre rulers along the canvas' top and left edges (camui-prancheta S5).
pub mod ruler;
/// The scripting panel: one script path, one `Run`, one output area (M10 T4b, S4).
///
/// It runs the script by calling `cam-cli` as a library — never by spawning the binary — and its
/// running half needs no egui at all, so a caller without a window can still run a script and read what
/// it produced.
pub mod script_panel;
/// The brand block's `Saved …` line: a civil date and time from a [`std::time::SystemTime`].
pub mod stamp;
/// A strategy's editor: the same surface for creating and editing one toolpath (camui-prancheta S10).
pub mod strategy;
pub mod theme;
/// The tool library as a master-detail modal (camui-prancheta S9).
pub mod tool_library;

pub use chrome::{Category, Tab};
pub use ruler::{RulerMarker, RulerTick, Rulers};
pub use tool_library::{ToolFamily, ToolFormGroup};
pub mod view3d;

pub use view3d::Preview3d;

/// The smallest canvas that auto-framing will act on.
///
/// The first frames of a real window can report a canvas of a few dozen pixels while the panels settle
/// their widths. Fitting that geometry zooms the drawing out by an order of magnitude and looks like a
/// broken viewport, so anything smaller than this is ignored and retried next frame.
pub const MIN_FRAME_CANVAS: f64 = 200.0;

/// The largest radial error the shell asks the kernel for when it approximates an arc.
///
/// Two M2 features approximate: an ellipse (A3) and a non-uniform scale over contours that hold arcs
/// (A5, decision D-M2-2). The kernel takes the tolerance as a parameter and has no default of its own,
/// so the number the user gets is chosen here, once, instead of being sprinkled through the callers.
/// Both callers report what the approximation actually cost, so this is a request, not a promise.
pub const DEFAULT_TOLERANCE_MM: f64 = 0.01;

/// How close two endpoints must be for the shell's join and trim commands to count them as touching.
///
/// This is the kernel's own chaining tolerance ([`cam_geom::DEFAULT_TOLERANCE`], one micrometre)
/// rather than a second number invented here: whether two ends meet is a tolerance problem, and the
/// kernel already owns the answer.
pub const JOIN_TOLERANCE_MM: f64 = cam_geom::DEFAULT_TOLERANCE;

/// How far a drag must travel before it is a shape rather than a click, in pixels.
///
/// A press and release in the same place is a cancel: a zero-size rectangle or a zero-length line is
/// not something the user can see, select or undo on purpose, so it is never submitted.
pub const MIN_DRAW_DRAG_PX: f64 = 3.0;

/// The tool armed on the canvas.
///
/// [`Tool::Select`] keeps M1's behaviour: a click picks, a drag marquees. Every other tool turns a
/// primary press-drag-release into exactly one drawing command, and is therefore also the shell's
/// "active tool" state: there is no modal command loop, only which gesture the canvas interprets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
  /// Click to pick, drag to marquee. Draws nothing.
  Select,
  /// Drag a line from the press point to the release point.
  Line,
  /// Drag two opposite corners of a rectangle.
  Rectangle,
  /// Drag one straight segment, stored as an open polyline.
  Polyline,
  /// Drag a chord; the arc passes through the gesture box corner `(start.x, end.y)`.
  Arc,
  /// Drag the bounding box of a circle.
  Circle,
  /// Drag the bounding box of an ellipse.
  Ellipse,
  /// Drag the bounding box of a regular polygon.
  Polygon,
  /// Drag the bounding box of a star.
  Star,
}

impl Tool {
  /// Every tool, in the order the tool list shows them.
  pub const ALL: [Tool; 9] = [
    Tool::Select,
    Tool::Line,
    Tool::Rectangle,
    Tool::Polyline,
    Tool::Arc,
    Tool::Circle,
    Tool::Ellipse,
    Tool::Polygon,
    Tool::Star,
  ];

  /// The label on the tool button.
  pub fn label(self) -> &'static str {
    match self {
      Self::Select => "Select",
      Self::Line => "Line",
      Self::Rectangle => "Rectangle",
      Self::Polyline => "Polyline",
      Self::Arc => "Arc",
      Self::Circle => "Circle",
      Self::Ellipse => "Ellipse",
      Self::Polygon => "Polygon",
      Self::Star => "Star",
    }
  }

  /// Whether this tool draws on a canvas drag instead of selecting.
  pub fn draws(self) -> bool {
    self != Tool::Select
  }
}

/// One drawing command, with every number the kernel needs, in world millimetres.
///
/// The shell builds these from a canvas drag, and [`handle_action`] turns one into exactly one
/// [`Command`]. Carrying the geometry in the action is what lets a test drive a drag and then assert
/// on the command it produced without a display, and it is why a shape is committed once, on release,
/// rather than once per mouse move.
#[derive(Clone, Debug, PartialEq)]
pub enum DrawShape {
  /// A rectangle with its lower-left corner at `at`.
  Rectangle {
    /// Lower-left corner, in millimetres.
    at: Point2,
    /// Width in millimetres; it must be positive.
    width: f64,
    /// Height in millimetres; it must be positive.
    height: f64,
  },
  /// A straight line, stored as an open contour.
  Line {
    /// Start point, in millimetres.
    from: Point2,
    /// End point, in millimetres.
    to: Point2,
  },
  /// A chain of straight segments.
  Polyline {
    /// Points to pass through, in millimetres.
    points: Vec<Point2>,
    /// Whether the last point joins the first.
    closed: bool,
  },
  /// A single arc through three ordered points.
  Arc {
    /// Start point, in millimetres.
    start: Point2,
    /// A point the arc passes through, in millimetres.
    through: Point2,
    /// End point, in millimetres.
    end: Point2,
  },
  /// An ellipse approximated by arcs within `tolerance`.
  Ellipse {
    /// Centre, in millimetres.
    at: Point2,
    /// Radius along X, in millimetres.
    rx: f64,
    /// Radius along Y, in millimetres.
    ry: f64,
    /// Largest radial error the caller accepts, in millimetres.
    tolerance: f64,
  },
  /// A circle, stored as two half-turn arcs.
  Circle {
    /// Centre, in millimetres.
    at: Point2,
    /// Radius in millimetres.
    radius: f64,
  },
  /// A regular polygon.
  Polygon {
    /// Centre, in millimetres.
    at: Point2,
    /// Circumscribed radius, in millimetres.
    radius: f64,
    /// Number of sides, at least three.
    sides: usize,
    /// Angle of the first vertex, in radians from the positive X axis.
    rotation: f64,
  },
  /// A star alternating `points` tips and valleys.
  Star {
    /// Centre, in millimetres.
    at: Point2,
    /// Radius of the outer tips, in millimetres.
    outer_radius: f64,
    /// Radius of the inner valleys, in millimetres.
    inner_radius: f64,
    /// Number of tips, at least three.
    points: usize,
    /// Angle of the first tip, in radians from the positive X axis.
    rotation: f64,
  },
}

impl DrawShape {
  /// The status-bar label for this shape.
  pub fn label(&self) -> &'static str {
    match self {
      Self::Rectangle { .. } => "add rectangle",
      Self::Line { .. } => "add line",
      Self::Polyline { .. } => "add polyline",
      Self::Arc { .. } => "add arc",
      Self::Ellipse { .. } => "add ellipse",
      Self::Circle { .. } => "add circle",
      Self::Polygon { .. } => "add polygon",
      Self::Star { .. } => "add star",
    }
  }

  /// The one command this shape becomes on `layer_id` as `entity_id`.
  pub fn to_command(&self, layer_id: &str, entity_id: &str) -> Command {
    let layer_id = layer_id.to_owned();
    let entity_id = entity_id.to_owned();
    match self {
      Self::Rectangle { at, width, height } => Command::AddRectangle {
        layer_id,
        entity_id,
        at: *at,
        width: *width,
        height: *height,
      },
      Self::Line { from, to } => Command::AddLine {
        layer_id,
        entity_id,
        from: *from,
        to: *to,
      },
      Self::Polyline { points, closed } => Command::AddPolyline {
        layer_id,
        entity_id,
        points: points.clone(),
        closed: *closed,
      },
      Self::Arc {
        start,
        through,
        end,
      } => Command::AddArc {
        layer_id,
        entity_id,
        start: *start,
        through: *through,
        end: *end,
      },
      Self::Ellipse {
        at,
        rx,
        ry,
        tolerance,
      } => Command::AddEllipse {
        layer_id,
        entity_id,
        at: *at,
        rx: *rx,
        ry: *ry,
        tolerance: *tolerance,
      },
      Self::Circle { at, radius } => Command::AddCircle {
        layer_id,
        entity_id,
        at: *at,
        radius: *radius,
      },
      Self::Polygon {
        at,
        radius,
        sides,
        rotation,
      } => Command::AddPolygon {
        layer_id,
        entity_id,
        at: *at,
        radius: *radius,
        sides: *sides,
        rotation: *rotation,
      },
      Self::Star {
        at,
        outer_radius,
        inner_radius,
        points,
        rotation,
      } => Command::AddStar {
        layer_id,
        entity_id,
        at: *at,
        outer_radius: *outer_radius,
        inner_radius: *inner_radius,
        points: *points,
        rotation: *rotation,
      },
    }
  }
}

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
  /// Write the program the drawing describes to `path`, in the post-processor this shell holds.
  ///
  /// This is the answer to [`UiAction::RequestGenerateGcode`], and it is the primary action of the whole
  /// window: the drawing, its toolpaths and their tools become the file the machine reads. The write is the
  /// shell's, exactly as [`UiAction::SaveFile`]'s is, so the sentence the person reads and the tests that pin
  /// it live in one place (M13 S7).
  ///
  /// Two things are refused by name rather than by a greyed-out button (M13 S2): a drawing with no toolpaths
  /// has no program to write, and a window with no post-processor holds no dialect to write it in. Either way
  /// nothing is created and the status bar says which of the two is missing.
  GenerateGcode {
    /// Where to write the program.
    path: PathBuf,
  },
  /// Ask the host for a path to write the program to; the host shows its save dialog and applies
  /// [`UiAction::GenerateGcode`].
  ///
  /// An intent and nothing more: the shell opens no dialog and writes nothing when it receives this.
  RequestGenerateGcode,
  /// Hold the post-processor in `path` and write every program with it from now on.
  ///
  /// This is the answer to [`UiAction::RequestChoosePost`]. The file is read and parsed **before** it is
  /// held, so a file that is not a post-processor is refused and the post-processor already in hand stays:
  /// a choice that could not write a program is not a choice, and losing the working one to it would be a
  /// second failure on top of the first.
  ChoosePost {
    /// The `.pp` file to read.
    path: PathBuf,
  },
  /// Ask the host for a post-processor; the host shows its open dialog and applies [`UiAction::ChoosePost`].
  ///
  /// An intent and nothing more, for the reason [`UiAction::RequestOpenFile`] is one.
  RequestChoosePost,
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
  /// Add a toolpath, binding the strategy editor's draft to geometry and a tool (camui-prancheta S10).
  ///
  /// The strategy editor's `Generate toolpath` is what pushes this, and it carries the whole draft: the name,
  /// the vectors the strategy cuts, the tool number a chosen tool carries and the operation with its numbers.
  /// [`cam_app::Command::AddToolpath`] is the rule; the id is minted by the shell because the document carries
  /// no toolpath counter, by the same `TPn` scan `cam-cli toolpath` performs.
  AddToolpath {
    /// Identifier for the new toolpath; it must be free.
    toolpath_id: String,
    /// Display name, as the toolpath list shows it.
    name: String,
    /// Entities the toolpath is computed from; at least one.
    entity_ids: Vec<String>,
    /// Closed contours inside the region that must be left standing; empty for none.
    island_ids: Vec<String>,
    /// Tool number, matched against the tool library at emission time.
    tool_number: u32,
    /// The operation and its parameters.
    operation: Operation,
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
      Self::ImportFile { .. } => "import file",
      Self::OpenFile { .. } | Self::RequestOpenFile => "open file",
      Self::Save => "save document",
      Self::SaveFile { .. } | Self::RequestSaveAs => "save document as",
      Self::GenerateGcode { .. } | Self::RequestGenerateGcode => "generate g-code",
      Self::ChoosePost { .. } | Self::RequestChoosePost => "choose post-processor",
      Self::SetToolpathToolNumber { .. } => "set toolpath tool",
      Self::AddToolpath { .. } => "add toolpath",
      Self::SetToolpathOperation { .. } => "set toolpath operation",
      Self::SetToolpathName { .. } => "set toolpath name",
    }
  }
}

/// Where the drawing area ended up on screen, in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasRect {
  /// Left edge, in pixels from the window's left.
  pub x: f64,
  /// Top edge, in pixels from the window's top.
  pub y: f64,
  /// Width in pixels.
  pub width: f64,
  /// Height in pixels.
  pub height: f64,
}

impl CanvasRect {
  /// The camera viewport for this rectangle.
  pub fn viewport(&self) -> Viewport {
    Viewport::new(self.width, self.height)
  }

  /// Convert a window position into camera-local screen space.
  pub fn to_screen(&self, position: Pos2) -> ScreenPoint {
    ScreenPoint::new(
      f64::from(position.x) - self.x,
      f64::from(position.y) - self.y,
    )
  }

  /// Convert camera-local screen space back into a window position.
  pub fn to_window(&self, point: ScreenPoint) -> Pos2 {
    Pos2::new((point.x + self.x) as f32, (point.y + self.y) as f32)
  }
}

/// What the shell displayed, reported so a headless test can assert on content.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DrawnState {
  /// Layer id, entity count and visibility, in the order the dock showed them.
  pub layer_rows: Vec<(String, usize, bool)>,
  /// Whether the layers block was expanded this frame.
  ///
  /// Reported because a folded block draws no row: without this a test could not tell "no layers" from "the
  /// operator folded the block", which are different answers (camui-prancheta S6).
  pub layers_open: bool,
  /// Where the layers block drew its heading: the point that folds it.
  ///
  /// A heading is a control and reports itself the way the rail's headings do (camui-prancheta S4).
  pub layers_heading: Option<(f32, f32)>,
  /// The ribbon's categories, in the order it drew them: glyph-only rows, so these are the words the task
  /// editor will carry once one is opened.
  pub categories: Vec<String>,
  /// Where the ribbon drew each category: its word and the point in the window that presses it.
  pub category_hits: Vec<(String, (f32, f32))>,
  /// The task editor's header while one is open, or empty while the panel is idle.
  pub task_editor: String,
  /// Where the task editor drew `✓` and `✕`: the words `Done` and `Cancel`, and the point that presses them.
  ///
  /// The two are painted glyphs, so the only word a headless test can read is the one this report gives them —
  /// the same reason the app bar used to report its own.
  pub task_editor_hits: Vec<(String, (f32, f32))>,
  /// Lines the property panel displayed.
  pub property_lines: Vec<String>,
  /// The status bar text.
  pub status_line: String,
  /// Scene items painted on the canvas.
  pub painted_items: usize,
  /// Grid lines painted on the canvas.
  pub painted_grid_lines: usize,
  /// Triangles the 3D panel painted, `0` when the panel is closed.
  ///
  /// A triangle counts once it reached the `egui::Mesh`, so a triangle this backend refuses — one
  /// with a vertex behind the camera — is not counted (M5 A9, A11).
  pub painted_triangles: usize,
  /// Every 3D edge the half painted: the sum of the four layer counts beside this one.
  ///
  /// Kept as one number because most questions about the picture are about it as a whole, and the four per-layer
  /// counts ([`Self::painted_material_lines`], [`Self::painted_vector_lines`], [`Self::painted_program_lines`],
  /// [`Self::painted_tool_lines`]) exist because the overlay row hides them one at a time (camui-prancheta S8):
  /// a single total could not say which layer went missing.
  pub painted_3d_lines: usize,
  /// The sentence the status bar paints at its right end: what the last action produced.
  ///
  /// Reported apart from [`Self::status_line`] because the right end is a zone of its own (camui-prancheta
  /// S7): a test that read the whole line could not tell the outcome from the coordinates beside it.
  pub message: String,
  /// Where the status bar drew each snap chip: its word and the point in the window that presses it.
  ///
  /// A chip both arms its kind and, for the grid, names the step it snaps to (camui-prancheta S7), so a test
  /// must press the chip the bar actually drew instead of guessing at the layout — the same reason the
  /// palette reports its own hits.
  pub status_chips: Vec<(String, (f32, f32))>,
  /// The sentence the status bar showed in front of the message: what the current gesture will do.
  pub hint: String,
  /// What the two rulers along the canvas' top and left edges drew (camui-prancheta S5).
  ///
  /// A report and not a picture, for the same reason the palette reports its chips: the bands, the numbers
  /// they read off the camera and the amber marker that tracks the pointer are the only part of S5 a test
  /// cannot see from [`ShellOutput::canvas`], which says where the drawing starts but nothing about what
  /// frames it.
  pub rulers: Rulers,
  /// What the left palette offered, in the order it drew them.
  ///
  /// The palette is the part of the frame whose content changes with the stage, and a headless test cannot
  /// read an icon: this is how "switching stage really changed what the operator can reach" is asserted.
  pub palette: Vec<String>,
  /// Where the palette drew each chip: its word and the point in the window that presses it.
  ///
  /// egui cannot be asked for a widget by its text from outside a frame, so the palette reports the
  /// point it drew each chip at and a click test presses that point instead of guessing at the layout.
  pub palette_hits: Vec<(String, (f32, f32))>,
  /// Where the palette drew each group's heading: its word and the point in the window that folds it.
  ///
  /// A heading is a control now (camui-prancheta S4), so it reports itself the way the chips do: a test
  /// clicks `FILE` and asserts FILE's chips are gone, instead of reaching into the shell's own field to set
  /// a fold the operator would have set by clicking.
  pub palette_headings: Vec<(String, (f32, f32))>,
  /// Where the Tools tab drew each parameterised command's button: its word and the point in the window
  /// that presses it.
  ///
  /// The same surface the palette reports ([`Self::palette_hits`]) and for the same reason, for the one
  /// question only it can answer: the tab and the palette run the same seven payloads, and a test that could
  /// press only one of them could not prove they agree. Only those seven are listed — the tab's other
  /// buttons are not built from [`crate::params`], so nothing needs their points.
  pub authoring_hits: Vec<(String, (f32, f32))>,
  /// The words the app bar's command buttons answer to, in the order it drew them.
  ///
  /// Empty since camui-prancheta T2b, and kept as an empty report on purpose: the bar's file commands moved to
  /// the rail, and a test asserts this list is empty so a button cannot quietly come back. [`Self::app_bar_hits`]
  /// is the same surface for the buttons that remain, which answer to their own labels.
  pub app_bar: Vec<String>,
  /// Where the app bar drew each command button: its word and the point in the window that presses it.
  ///
  /// The same surface the palette reports ([`Self::palette_hits`]) and for the same reason: egui offers no
  /// widget-by-text lookup from outside a frame, so a test that has to press one of the bar's glyphs is
  /// given the point the button was drawn at instead of guessing at the bar's margins.
  pub app_bar_hits: Vec<(String, (f32, f32))>,
  /// Where the 3D half's empty state drew its `Simulate` chip, in window pixels (M13 S3).
  ///
  /// `None` while the half has a picture to draw, because a control is reported only in the frame that drew it:
  /// the chip lives in the one state that has no overlay row and no playback bar to carry it, so this is the
  /// only way a headless test can press it instead of guessing at the half's margins.
  pub simulate_chip: Option<(f32, f32)>,
  /// What the `F1` help panel drew, in the order it drew it: each group's label, then that group's
  /// command rows, and finally the focused command's `what` and `how`.
  ///
  /// Empty while the panel is closed, because a panel nobody opened drew no row. A headless test reads
  /// this to prove that every command in the table is listed, which is the one claim a screenshot of a
  /// long list of labels cannot make.
  pub help: Vec<String>,
  /// The words the canvas context menu drew, in the order it drew them.
  ///
  /// Empty while no menu is open, because a popup nobody opened drew no row. The menu is the one part of
  /// the frame that only exists while the secondary button has been pressed, so this is how a headless test
  /// sees it at all — and its entries are [`crate::help::Command`] labels, which is what makes "every entry
  /// can be explained" checkable rather than asserted.
  pub menu: Vec<String>,
  /// Where the context menu drew each entry: its word and the point in the window that presses it.
  ///
  /// The same surface the palette reports ([`Self::palette_hits`]) and for the same reason: egui offers no
  /// widget-by-text lookup from outside a frame, so a test that has to press a menu row is given the point
  /// the row was drawn at instead of guessing at the popup's margins.
  pub menu_hits: Vec<(String, (f32, f32))>,
  /// The words the material dialog drew, in the order it drew them: the title, the two cards with their
  /// subtitles, the three measurement labels with their values, the plan's annotations, the caption under
  /// the plan, and the footer.
  ///
  /// Empty while the dialog is closed, because a dialog nobody opened drew nothing. This is how a headless
  /// test reads the one screen in the shell whose content is a picture: the annotated plan is painted with
  /// a painter, and its three dimension labels are the only part of it a test can quote back.
  pub material: Vec<String>,
  /// Where the material path drew each pressable thing: the Job tab's opener while the dialog is closed,
  /// then the dialog's two cards and its two footer buttons while it is open.
  ///
  /// The same surface the palette reports ([`Self::palette_hits`]) and for the same reason: egui offers no
  /// widget-by-text lookup from outside a frame, so a test that has to press a card is given the point the
  /// card was drawn at instead of guessing at the dialog's margins.
  pub material_hits: Vec<(String, (f32, f32))>,
  /// The sentence the material dialog is showing because one of its numbers cannot describe a stock.
  ///
  /// Empty while the numbers are usable and while the dialog is closed. The dialog refuses before it pushes
  /// anything, so a non-empty answer here is also the reason [`ShellOutput::actions`] holds no
  /// [`UiAction::SetJob`] for that frame — and [`cam_app::Command::SetJob`] refuses the same three numbers
  /// by the model's own rule if anything else ever does.
  pub material_refusal: String,
  /// The words the current-tool card drew, in the order it drew them: the section naming the toolpath it
  /// belongs to, then either the tool's two lines — number and name, then type, diameter, angle and flute
  /// count — or the one sentence saying the number is not in the library this window was given.
  ///
  /// Empty while the frame is not on the Toolpaths stage, and empty while the document holds no toolpath:
  /// a card for a toolpath that does not exist would be describing nothing. This is how a headless test
  /// reads the card, for the same reason [`Self::material`] exists — egui offers no widget-by-text lookup
  /// from outside a frame — and it is the only place that pairs a document's `tool_number` with the
  /// library snapshot (camui-toolpath S2).
  pub tool_card: Vec<String>,
  /// The estimated cutting time drawn beside the current-tool card, as one line of text.
  ///
  /// A line of its own and not a fourth card field, because the card's fields are exactly the ones the
  /// reference frame shows and must not grow (camui-toolpath S2): the time is about the document's
  /// program, not about the tool. The seconds are the published law
  /// ([`view3d::seconds_for`](crate::view3d::seconds_for)) and the text is
  /// [`view3d::clock_text`](crate::view3d::clock_text)'s, so this reads what the playback clock reads.
  ///
  /// Empty while the frame is not on the Toolpaths stage and while the document holds no toolpath, for
  /// the same reason [`Self::tool_card`] is; when the document's program cannot be built it holds the
  /// emitter's own sentence instead of a duration nobody can measure.
  pub cutting_time: String,
  /// How much of the previewed program has played, in `0.0..=1.0`.
  ///
  /// The value the clock itself reports: exactly `0.0` while the preview stands on its first
  /// move and exactly `1.0` once the program has finished (M5 A10). Stays `0.0` when the panel is
  /// closed, because a closed panel is not a clock. Read it as "the program is played out", not as
  /// "the bar is full": a program whose moves all take zero time reports `1.0` here while
  /// [`Self::preview_bar`] stays `0.0`, because there is no time for a bar to fill with.
  pub preview_progress: f64,
  /// How full the 3D panel's progress bar was painted, in `0.0..=1.0`.
  ///
  /// The same number the bar's own `egui::ProgressBar` is given, which is not always
  /// [`Self::preview_progress`]: a program with no total duration paints an empty bar however far it
  /// has played, so the readout never claims a completion the clock could not measure. Stays `0.0`
  /// when the panel is closed or holds no preview.
  pub preview_bar: f32,
  /// The two clocks the progress bar's readout states, as one sentence: `0:42 / 3:07`.
  ///
  /// Painted beside the bar and mirrored here because a headless test can read a string where it
  /// cannot read a painted clock: the formatter is unit-tested, and this field is what proves the
  /// panel writes its output rather than something of its own. Empty when the panel is closed or
  /// holds no preview, because a bar nobody drew has no clock.
  pub preview_clock: String,
  /// The words the `2D | 3D` toggle drew and the point in the window that presses each (camui-prancheta S8).
  ///
  /// Reported for the same reason the palette reports its chips: the toggle is painted, not a widget egui
  /// can be asked for by text from outside a frame, so a test presses the cell the frame really drew. A
  /// frame that drew no canvas reports nothing.
  pub view_toggle: Vec<(String, (f32, f32))>,
  /// The strip the `2D | 3D` toggle occupied, as `(x, y, width, height)` in window pixels.
  ///
  /// Separate from [`Self::view_toggle`] because S8 is a claim about *where* the toggle sits — the top right
  /// of the canvas — and a list of pressable points cannot say where the control as a whole was drawn.
  pub view_toggle_rect: Option<(f32, f32, f32, f32)>,
  /// The canvas' two halves while the 3D split is open: the drawing half, then the 3D half, each as
  /// `(x, y, width, height)` in window pixels (camui-prancheta S8).
  ///
  /// `None` while 3D is off, because a split that is not open has no halves. [`ShellOutput::canvas`] is the
  /// first of the two when the split is open: the rectangle the 2D camera is told about.
  pub split: Option<(PixelRect, PixelRect)>,
  /// The words the 3D half's overlay row drew — `Material`, `Vectors`, `Toolpaths`, `Tool`, `Simulate again`,
  /// `Centre`, `?` — and the point that presses each (camui-prancheta S8, M13 S3).
  ///
  /// Empty while the split is closed, and empty while it is open with no program loaded: the row is drawn
  /// only when it has something to govern, because a checkbox over nothing is a control that lies. The empty
  /// half's `Simulate` chip is not in this list — it is reported where it was drawn
  /// ([`Self::simulate_chip`]) because it is the one control that lives outside the row.
  pub layer_chips: Vec<(String, (f32, f32))>,
  /// Which of the four scene layers the overlay row is showing, in the row's own order (camui-prancheta S8).
  ///
  /// Reported beside the counts below so a test can tell "the layer is hidden" from "the layer had nothing
  /// to paint", which are different answers.
  pub scene_layers: Vec<(String, bool)>,
  /// Where the 3D half drew its home button, in window pixels (camui-prancheta S8).
  pub home_button: Option<(f32, f32)>,
  /// The axis triad's three labels and where each was drawn (camui-prancheta S8): `TOP`, `FRONT`, `RIGHT`.
  ///
  /// The words in the order painted, with the point each label sits at — which is what lets a test prove the
  /// triad is drawn from the camera and not pasted on: orbiting turns it.
  pub triad: Vec<(String, (f32, f32))>,
  /// The playback bar's controls and the point that presses each (camui-prancheta S8): `Play` or `Pause`, the
  /// speed's own word (`60x`), `To the end`, `Step` and `Reset`.
  pub playback_hits: Vec<(String, (f32, f32))>,
  /// The playback bar's readout, exactly as painted: `0:05 / 0:05  T22 · Drilling 1 · Z 5.000`.
  ///
  /// The clocks come from [`view3d::progress_text`](crate::view3d::progress_text) and the move from
  /// [`Preview3d::playhead_text`](crate::view3d::Preview3d::playhead_text), so this field is what proves the
  /// bar writes those two rather than a third opinion of its own.
  pub playback_readout: String,
  /// Where the playback bar drew its scrubber, as `(left, right, y)` in window pixels (camui-prancheta S8).
  ///
  /// The two ends and the row's middle, not the filled part: a test seeks by pressing a fraction of the
  /// way along the track the frame drew instead of guessing at the bar's margins.
  pub scrubber: Option<(f32, f32, f32)>,
  /// Stock-box edges the 3D half painted, `0` when `Material` is hidden or the split is closed.
  pub painted_material_lines: usize,
  /// The document's vectors, projected into the 3D half, `0` when `Vectors` is hidden or the split is closed.
  pub painted_vector_lines: usize,
  /// The program's moves, projected into the 3D half, `0` when `Toolpaths` is hidden or the split is closed.
  pub painted_program_lines: usize,
  /// The tool marker's edges, `0` when `Tool` is hidden, the split is closed, or no program is loaded.
  pub painted_tool_lines: usize,
  /// The words the tool-library modal drew, in the order it drew them (camui-prancheta S9).
  ///
  /// Empty while the modal is closed, because a modal nobody opened drew nothing. The modal is the one
  /// surface in the shell whose content is a tree of the library and a form that changes with the selected
  /// tool, so this is how a headless test reads it: egui offers no widget-by-text lookup from outside a
  /// frame, and the tree's rows carry a painted name and a painted cut line rather than a label a test can
  /// ask for.
  pub tool_library: Vec<String>,
  /// Where the tool-library modal drew each pressable thing: its word and the point in the window that
  /// presses it (camui-prancheta S9).
  ///
  /// The same surface the palette reports ([`Self::palette_hits`]) and for the same reason. Only points the
  /// operator could actually press are reported: a row scrolled out of the master list is drawn by egui and
  /// clipped away, and reporting its centre would be a promise about a control nobody can see (the defect
  /// the T6 review found in the playback bar, L23).
  pub tool_library_hits: Vec<(String, (f32, f32))>,
  /// The family tree the modal drew: each family's own name and the number of rows drawn under it.
  ///
  /// The count is the number of rows that follow the heading in the tree, not the number of tools the
  /// library holds in that family: a count beside a filtered tree that named tools the operator cannot see
  /// would be a number nobody can check against what is on screen.
  pub tool_families: Vec<(String, usize)>,
  /// One entry per tool row, in the order drawn: the family's name, the tool's title, and its
  /// `Ø6 · 2 cut(s)` line.
  ///
  /// The family is part of the row rather than implied by [`Self::tool_families`] because a tree is a
  /// claim about *which* rows sit together, and a list of counts cannot make it.
  pub tool_rows: Vec<(String, String, String)>,
  /// The tool the modal's detail side is showing, by the library's own geometry id.
  ///
  /// `None` while the modal is closed and while the tree holds no selection. An id and not a row title,
  /// because two tools may carry the same name and the same number, and the operator's choice is a tool.
  pub tool_selected: Option<String>,
  /// The detail form's groups, in the order drawn, each with the fields and values under it
  /// (camui-prancheta S9).
  ///
  /// Empty while the modal is closed. This is the report that makes "the form changes with the family"
  /// answerable: the group's word and its fields are exactly what the form drew, so a field a family does
  /// not have cannot be claimed by a test that only read the painted text.
  pub tool_groups: Vec<(String, Vec<(String, String)>)>,
  /// The dimension callouts painted on the scaled tool drawing, in the order drawn.
  ///
  /// The drawing is painted, so these are the only part of it a test can quote back — the same reason the
  /// material plan reports its three measurements.
  pub tool_callouts: Vec<String>,
  /// Where the scaled tool drawing went, as `(x, y, width, height)` in window pixels.
  ///
  /// `None` while the modal is closed, while nothing is selected, and when the selected tool carries no
  /// diameter: a tool with no width has no width to draw, and the form says so instead of drawing a shape
  /// of its own invention.
  pub tool_drawing: Option<(f32, f32, f32, f32)>,
  /// The width the tool's own diameter was drawn at, in pixels, `0.0` when nothing was drawn.
  ///
  /// Separate from [`Self::tool_drawing`] because "to scale" is a claim about the *number*: the same tool
  /// drawn wider is not the same reading, and a rectangle tells a test nothing about how many millimetres
  /// wide it is.
  pub tool_width_px: f32,
  /// Where the current-tool card drew `Choose another tool…`: its word and the point that presses it.
  ///
  /// Empty while no card is drawn. The card's own words stay in [`Self::tool_card`] and did not grow this
  /// button, because the card is a reading of the tool a toolpath cuts with and the button is the way to
  /// change it.
  pub tool_card_hits: Vec<(String, (f32, f32))>,
  /// The note the strategy editor drew under its sections, verbatim (camui-prancheta S10).
  ///
  /// Empty while the `TOOLPATHS` task is closed, because an editor nobody opened drew no note. The note is the
  /// spec's own sentence, so it is reported as one string rather than left to a test that would have to find it
  /// among the panel's painted text.
  pub strategy_note: String,
  /// The sentence drawn in place of `Generate toolpath` because the draft's action does not exist yet.
  ///
  /// Empty whenever `Generate toolpath` was drawn — the two are exclusive by design (L6, L14): the footer says
  /// why instead of offering a button that would emit nothing.
  pub strategy_refusal: String,
  /// Every pressable thing the strategy editor drew: its word and the point in the window that presses it.
  ///
  /// The same surface the palette reports ([`Self::palette_hits`]) and for the same reason. Only controls this
  /// frame really drew are listed: `Generate toolpath` is absent while [`Self::strategy_refusal`] holds the
  /// reason, and the name is absent while the name is a reading (camui-prancheta S10).
  pub strategy_hits: Vec<(String, (f32, f32))>,
  /// The image cards the editor drew, in the order drawn: the card's word, whether it is the chosen one, and
  /// how many strokes its diagram painted.
  ///
  /// The stroke count is the report that makes "a card carries a picture, not an empty box" answerable: a
  /// headless test reads shapes, and a shape with no strokes in it is the empty box L23 taught us to look for
  /// even when the frame is green.
  pub strategy_cards: Vec<(String, bool, usize)>,
  /// Whether the collapsible `Quick parameter edit` block was expanded this frame.
  ///
  /// Reported because a folded block draws no row, and without this a test could not tell "the quick edit holds
  /// no number" from "the operator folded it" (the same reason [`Self::layers_open`] exists).
  pub strategy_quick_open: bool,
  /// The numbers the `Quick parameter edit` block drew, in order: the row's word and its value.
  ///
  /// The same numbers [`Self::strategy_fields`] holds for the full section, drawn one click closer: the quick
  /// edit duplicates values, never a second copy of them.
  pub strategy_quick_fields: Vec<(String, String)>,
  /// The numbers the strategy's `CUT` section drew, in order: the row's word and its value.
  pub strategy_fields: Vec<(String, String)>,
}

/// A rectangle in window pixels, as `(x, y, width, height)`.
///
/// The shape a drawn surface reports its own geometry in: `chrome::rect_report` turns an `egui::Rect` into one,
/// and [`DrawnState::split`] holds the canvas' two halves as a pair of them.
pub type PixelRect = (f32, f32, f32, f32);

/// What one frame produced.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShellOutput {
  /// Where the canvas was drawn.
  pub canvas: CanvasRect,
  /// Actions the user triggered this frame; the caller applies them through [`handle_action`].
  pub actions: Vec<UiAction>,
  /// What the panels displayed.
  pub drawn: DrawnState,
}

impl Default for CanvasRect {
  fn default() -> Self {
    Self {
      x: 0.0,
      y: 0.0,
      width: 0.0,
      height: 0.0,
    }
  }
}

/// The shell's own state: widget values and transient view flags.
///
/// # Where the numeric arguments come from
///
/// Authoring needs numbers the user does not type during a gesture: how many sides a polygon has, how
/// far an array copies, how deep an offset goes, how tall the text is. There is no wizard; each one is
/// an explicit field below with a documented default, editable in the tools area, and every button
/// reads the field into the [`UiAction`] it pushes. Tolerances are constants
/// ([`DEFAULT_TOLERANCE_MM`], [`JOIN_TOLERANCE_MM`]) because they are a promise about the result, not
/// a per-shape taste.
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
  /// ([`chrome`]'s `view_toggle`), the rail's `3D` command flips it, and [`Shell::set_view_3d`] is how a host
  /// or a test says so directly.
  pub show_3d: bool,
  /// Which of the 3D half's layers its overlay row is showing (camui-prancheta S8).
  ///
  /// View state: hiding the toolpaths from the picture is a decision about what the screen shows, not about
  /// the program, so it is not an edit and not an undo step. Public and flat the way every other switch of
  /// this shell is, because a checkbox only a click can reach is a checkbox no test can preset.
  pub scene_layers: view3d::Layers,
  /// The program preview the 3D panel draws, once a caller has handed one in.
  preview: Option<Preview3d>,
  /// The tool library as a snapshot: every tool this window was given, in the library's own order.
  ///
  /// A snapshot and not a library: [`cam_store::tools::ToolLibrary`] owns a rusqlite connection and is
  /// not `Clone`, so the shell cannot hold one. It is filled from two places — an open resolves the
  /// library its document names, read from beside that document (shell-tools-ref S2, S3), and a host may
  /// hand one in with [`Shell::set_tools`]. It is empty after an open whose document names no library, or
  /// one that cannot be used, and empty is a real answer — the card then says the number it was asked for
  /// is not in the library it was given.
  tools: Vec<StoredTool>,
  /// The file the drawing was opened from, once a host has opened or saved one.
  ///
  /// `None` until [`UiAction::OpenFile`] or [`UiAction::SaveFile`] succeeds, which is when the app bar stops
  /// saying `untitled.c2d` and starts naming the file. The shell keeps the path so the bar can name it and
  /// [`UiAction::Save`] can write to it: the shell shows no dialog of its own, so this is a record of what
  /// was read or written and not a file handle.
  document_path: Option<PathBuf>,
  /// The archive entries this build does not interpret, carried from the last load.
  ///
  /// [`cam_store::document::DocumentFile`] exists so an entry a later build understands is part of the
  /// document rather than something dropped on the floor, and opening is the moment it is known: the shell
  /// keeps these beside the path and [`UiAction::Save`] writes them back under their own names. They are
  /// **file-level, not document-level** — the undo history snapshots bare [`cam_model::Document`]s, so an
  /// undo cannot restore or lose them, and a drawing that was never opened has no entries to carry.
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
  inspector_tab: Tab,
  /// The ribbon category whose task editor is open, if any.
  ///
  /// `None` is not "nothing to show": it is the panel's idle state, which is the selection's numbers. The
  /// reference draws no selection category — selection is what the panel says while no task is open — which
  /// is why this is an `Option` rather than a fourth member of [`Category`].
  task: Option<Category>,
  /// Whether the layers block is expanded.
  ///
  /// View state of its own, and S6 asks for exactly that: `Camadas` collapses on its own, below the task
  /// editor, independently of which task is open.
  layers_open: bool,
  /// Whether the inspector is expanded. Collapsed it leaves a 12 px strip that reopens on hover.
  inspector_open: bool,
  /// The rail groups the operator has collapsed.
  ///
  /// View state, not document state: collapsing a group changes what the frame shows and nothing else, so
  /// it is neither an undoable edit nor a reason for the document to read as modified. Every group starts
  /// expanded, because a rail that hid its own commands on the first frame would teach nothing — and the
  /// reference opens expanded.
  collapsed_groups: Vec<crate::help::Group>,
  /// Whether the shortcuts window is open.
  show_shortcuts: bool,
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
  script: script_panel::ScriptPanel,
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
  tool_library_target: Option<String>,
  /// The tool the modal's detail side is showing, by the library's own geometry id.
  ///
  /// A draft too: choosing a row changes what the form and the drawing describe, and nothing else. Only
  /// `Use this tool` turns the choice into an action.
  tool_library_selected: Option<String>,
  /// Whether the library was opened by the strategy editor while it is drafting a **new** toolpath (S10).
  ///
  /// The modal's commit writes one thing: a tool number. With a toolpath on screen that is a document edit
  /// ([`UiAction::SetToolpathToolNumber`]); with only a draft it can only mean "this strategy will cut with
  /// this tool", so the number lands in [`Shell::strategy`] and `Generate toolpath` is what writes it. The flag
  /// is what tells the two apart, and it is cleared whenever the modal closes.
  pub(crate) tool_library_for_draft: bool,
  /// The strategy editor's draft: the name, the tool and the operation `Generate toolpath` writes
  /// (camui-prancheta S10).
  ///
  /// Public and flat the way [`Self::material_size`] is, because a draft whose numbers could not be set from
  /// outside a frame is a draft only a mouse could test. It is re-seeded from the document whenever the editor's
  /// subject changes ([`strategy::Draft::sync`]), and `Cancel` throws it away.
  pub strategy: strategy::Draft,
  /// The command the help panel is teaching: whichever row was pressed last, or the first one.
  help_focus: crate::help::Command,
  /// What the canvas context menu is scoped to, as of the last right-click on the canvas.
  ///
  /// View state, like [`Shell::drag_box`]: it is what the menu offers, never what the document holds. The
  /// right-click decides it ([`Shell::handle_input`]) because the selection cannot — a right-click on empty
  /// canvas keeps an existing selection and still means the canvas menu — and it is re-decided on every
  /// right-click, so a menu can never outlive the click that opened it.
  menu_scope: crate::help::MenuScope,
}

/// The three numbers the material dialog starts with: the default sheet, in millimetres.
///
/// One function and two callers — the shell's own default and an open that has to drop drafts seeded
/// from a document nobody is looking at any more — so the seeds cannot drift apart.
fn default_material_size() -> (f64, f64, f64) {
  let sheet = Job::default_sheet();
  (sheet.width.mm(), sheet.height.mm(), sheet.thickness.mm())
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
      show_tool_library: false,
      show_script: false,
      script: script_panel::ScriptPanel::new(),
      tool_search: String::new(),
      tool_library_target: None,
      tool_library_selected: None,
      tool_library_for_draft: false,
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
  /// mutates a copy and the copy reaches [`AppState`] only through [`AppState::replace_document_undoable`].
  /// This exists for the same reason the palette reports its chips' points — a widget a test cannot
  /// press is a widget nothing pins.
  pub fn script_panel(&mut self) -> &mut script_panel::ScriptPanel {
    &mut self.script
  }

  /// Apply one action and show what it produced in the status bar.
  ///
  /// This is what a host calls for every action [`Self::show`] returned. It keeps the one-command
  /// rule — the work itself happens in the action handler, which is [`handle_action`] with this shell's file
  /// handed in — and turns the outcome the command layer reports (entity count, ellipse radial error, arcs
  /// lost to tessellation) and any refusal, with the kernel's own message, into the sentence the user reads.
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
      UiAction::ChoosePost { path } => self.choose_post(path),
      UiAction::GenerateGcode { path } => self.generate_gcode(app, path),
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

  /// The file the drawing was opened from or last written to, once a host has opened or saved one.
  ///
  /// `None` while the frame holds a document that has no file yet, which is the state the app bar still
  /// calls `untitled.c2d` and the state in which [`UiAction::Save`] is refused.
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
  /// asks [`AppState::is_dirty`] and draws that word itself, because dirty is the document's fact and the
  /// stamp is the file's.
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
  /// This method reads no document and returns no [`UiAction`], which is what keeps the preview off
  /// the document's authoring path (M5 D-M5-9, A12): it takes a program a caller has already built.
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
  }

  /// The preview the 3D panel draws, when a caller has handed one in.
  pub fn preview(&self) -> Option<&Preview3d> {
    self.preview.as_ref()
  }

  /// The preview, for a caller that frames the camera directly instead of by a gesture.
  pub fn preview_mut(&mut self) -> Option<&mut Preview3d> {
    self.preview.as_mut()
  }

  /// Build one frame.
  ///
  /// Actions are returned rather than applied: one frame of latency is invisible, and a pure shell
  /// is what makes the interface testable.
  pub fn show(&mut self, app: &mut AppState, ctx: &Context) -> ShellOutput {
    // Before the root `Ui` exists, because a `Ui` captures its style when it is created: publishing the
    // tokens from inside the frame would reach the next frame's widgets and not this one's.
    self.publish_theme(ctx);
    // egui 0.37 stacks panels on the root `Ui`; every panel of one frame goes through one borrow.
    ctx.root_ui(|ui| self.show_in(app, ui))
  }

  /// Publish the tokens into a context, once per change.
  ///
  /// Public because a host that owns its own root `Ui` (eframe hands one in) has to call this before it
  /// builds that `Ui`; [`Self::show`] calls it for hosts that do not.
  pub fn publish_theme(&mut self, ctx: &Context) {
    // The cache is only valid for the context it was written into: a host may replace the context, and
    // a fresh one has none of the named styles the shell asks for. The marker in the style itself is
    // therefore the authority, and `applied_theme` is only the fast path.
    let styled = ctx
      .style_of(self.theme.egui())
      .text_styles
      .contains_key(&theme::title());
    if !styled || self.applied_theme != Some(self.theme) {
      theme::apply(ctx, self.theme);
      self.applied_theme = Some(self.theme);
    }
  }

  /// The whole glyph set, on one window.
  ///
  /// A glyph is only checkable against its neighbours: one drawn at a 2.4 stroke looks fine alone and
  /// wrong beside sixty-five drawn at 1.6. The window is deliberately plain — a grid and names —
  /// because it is an instrument, not a screen the operator ever sees.
  fn icon_sheet(&mut self, ctx: &Context) {
    let palette = self.palette();
    let metrics = self.metrics();
    let mut open = self.show_icons;
    egui::Window::new("Icons")
      .open(&mut open)
      .resizable(true)
      .default_width(760.0)
      .show(ctx, |ui| {
        ui.label(
          egui::RichText::new(format!(
            "{} glyphs · stroke {} in a {} box · F2 closes",
            Icon::ALL.len(),
            metrics.icon_stroke,
            metrics.icon_size
          ))
          .size(theme::CAPTION)
          .color(palette.get(Role::TextDim)),
        );
        egui::ScrollArea::vertical()
          .id_salt("icon-sheet")
          .show(ui, |ui| {
            // The artboard's structure: six families, each with its count, so seventy glyphs read as six
            // arguments instead of one wall. `min_col_width` keeps a long name like `toolpath-profile`
            // inside its cell instead of printing over its neighbour.
            for family in Family::ALL {
              ui.add_space(metrics.space(3));
              ui.label(
                egui::RichText::new(format!(
                  "{}  ·  {}",
                  family.label(),
                  Icon::in_family(family).count()
                ))
                .size(theme::SMALL)
                .color(palette.get(Role::TextHi)),
              );
              egui::Grid::new(family.label())
                .num_columns(8)
                .min_col_width(84.0)
                .spacing([metrics.space(4), metrics.space(3)])
                .show(ui, |ui| {
                  for (index, icon) in Icon::in_family(family).enumerate() {
                    ui.vertical(|ui| {
                      ui.spacing_mut().item_spacing.y = metrics.space(0);
                      let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(28.0, 28.0), Sense::hover());
                      let scale = rect.width() / metrics.icon_size;
                      icons::paint(
                        ui.painter(),
                        rect,
                        icon,
                        palette.get(Role::Text),
                        metrics.icon_stroke * scale,
                      );
                      ui.label(
                        egui::RichText::new(icon.name())
                          .size(theme::CAPTION)
                          .color(palette.get(Role::TextDim)),
                      );
                    });
                    if index % 8 == 7 {
                      ui.end_row();
                    }
                  }
                });
            }
          });
      });
    self.show_icons = open;
  }

  /// Build one frame into an existing root [`egui::Ui`].
  ///
  /// `eframe` hands the application a root `Ui` directly, and calling [`Self::show`] from inside that
  /// callback would try to borrow the root twice. Hosts that already hold one use this.
  pub fn show_in(&mut self, app: &mut AppState, ui: &mut egui::Ui) -> ShellOutput {
    let mut output = ShellOutput::default();
    let ctx = ui.ctx().clone();
    // A host may hand in a `Ui` it built before calling us; the styles are still worth publishing for
    // whatever this frame creates from here on, and `publish_theme` is idempotent.
    self.publish_theme(&ctx);
    // F2 opens the glyph set: the vocabulary is part of the design, so it has to be inspectable from
    // inside the app that uses it rather than only from the artboard.
    if ctx.input(|input| input.key_pressed(egui::Key::F2)) {
      self.show_icons = !self.show_icons;
    }
    if self.show_icons {
      self.icon_sheet(&ctx);
    }
    // `F1` is the same idea one level up: `F2` shows the vocabulary of shapes, `F1` shows the vocabulary
    // of commands with the two sentences each one teaches (T8, S2, S4).
    if ctx.input(|input| input.key_pressed(egui::Key::F1)) {
      self.show_help = !self.show_help;
    }
    // `Esc` closes the panel and touches nothing else, so reading help can never cancel a gesture the
    // operator did not mean to cancel.
    if self.show_help && ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
      self.show_help = false;
    }
    if self.show_help {
      self.help_window(&ctx, &mut output);
    }
    // The material dialog is the shell's one modal: `Esc` leaves it without writing, the rule the help panel
    // already follows, and `Continue` is the only thing in the dialog that touches the document.
    if self.show_material && ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
      self.show_material = false;
    }
    if self.show_material {
      self.material_dialog(&ctx, app, &mut output);
    }
    // The tool library is the shell's second modal (camui-prancheta S9), and `Esc` leaves it without writing
    // for the reason the material dialog's does: the choice is a draft until `Use this tool` says otherwise.
    if self.show_tool_library && ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
      self.close_tool_library();
    }
    if self.show_tool_library {
      self.tool_library_dialog(&ctx, app, &mut output);
    }
    // The script panel is the shell's third modal (M10 T4b, S4), and `Esc` closes it for the reason the
    // other two close: the panel holds no edit of its own, so leaving it has nothing to undo — what `Run`
    // changed is in the document, one undo step at a time (M11 T2).
    if self.show_script && ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
      self.show_script = false;
    }
    if self.show_script {
      self.script_dialog(&ctx, app);
    }
    // The crate's only clock read (A10). Everything that needs time — playback, and nothing else —
    // receives this delta as an argument, so the arithmetic stays a pure function a test can drive
    // frame by frame. `stable_dt` rather than `unstable_dt`: a frame that stalled (a resize, a
    // breakpoint) must not jump the animation forward by the length of the stall.
    let dt = f64::from(ctx.input(|input| input.stable_dt));
    // A modal owns the pointer, so the canvas reports no cursor and finds no snap target while one is open:
    // a status bar naming a world position under the backdrop would be describing a canvas nobody can aim at.
    if !self.show_material && !self.show_tool_library && !self.show_script {
      self.track_pointer(app, &ctx);
    }
    // Bars first, then the sides, then the canvas: egui lays panels out in the order they are added,
    // and a central panel must come last or it would claim the space the sides need.
    self.app_bar(app, ui, &mut output);
    self.status_bar(app, ui, &mut output);
    self.palette_panel(app, ui, &mut output);
    self.inspector(app, ui, &mut output);
    // One central area, and the mode decides how it is divided: the drawing alone, or the drawing and the 3D
    // half side by side (camui-prancheta S8).
    self.canvas(app, ui, &mut output, dt);
    if self.show_shortcuts {
      self.shortcuts_window(&ctx);
    }
    output
  }

  fn layer_dock(&mut self, app: &AppState, ui: &mut egui::Ui, output: &mut ShellOutput) {
    {
      egui::ScrollArea::vertical().show(ui, |ui| {
        for layer in &app.document().layers {
          let mut visible = layer.visible;
          ui.horizontal(|ui| {
            if ui.checkbox(&mut visible, "").changed() {
              output.actions.push(UiAction::ToggleLayerVisible {
                layer_id: layer.id.clone(),
              });
            }
            ui.label(format!("{} — {}", layer.id, layer.name));
          });
          ui.label(format!("  {} entit(ies)", layer.entities.len()));
          output
            .drawn
            .layer_rows
            .push((layer.id.clone(), layer.entities.len(), layer.visible));
          ui.separator();
        }
      });
    }
  }

  fn properties(&mut self, app: &AppState, ui: &mut egui::Ui, output: &mut ShellOutput) {
    {
      ui.heading("Properties");
      ui.separator();
      if ui
        .add_enabled(
          !app.selection().is_empty(),
          egui::Button::new("Delete selection"),
        )
        .clicked()
      {
        output.actions.push(UiAction::DeleteSelection);
      }
      let job = &app.document().job;
      let mut lines = vec![
        format!(
          "Job: {:.1} x {:.1} x {:.1} mm",
          job.width.mm(),
          job.height.mm(),
          job.thickness.mm()
        ),
        format!("Units: {:?}, origin {:?}", job.units, job.origin),
        format!("Safe Z: {:.1} mm", job.safe_z.mm()),
      ];
      ui.label(&lines[0]);
      ui.label(&lines[1]);
      ui.label(&lines[2]);
      ui.separator();

      let selected: Vec<String> = app.selection().ids().map(str::to_owned).collect();
      if selected.is_empty() {
        lines.push("Selection: none".to_owned());
        ui.label("Selection: none");
      } else {
        for id in &selected {
          let summary = match app
            .document()
            .entity(id)
            .and_then(|entity| entity.kind.as_contour())
          {
            Some(contour) => format!(
              "{id}: contour, {:.1} mm perimeter, area {:.1} mm²",
              contour.perimeter(),
              contour.signed_area().abs()
            ),
            None => format!("{id}: (missing)"),
          };
          ui.label(&summary);
          lines.push(summary);
        }
      }
      ui.separator();
      lines.push(format!("History: {} step(s)", app.history_depth()));
      ui.label(format!("History: {} step(s)", app.history_depth()));
      output.drawn.property_lines = lines;
    }
  }

  /// The authoring dock: which tool is armed, and the numbers the commands use.
  ///
  /// Every button here reads a field of [`Shell`] into one [`UiAction`]; none of them talks to
  /// [`AppState`]. It is its own dock rather than a corner of the properties panel because a reviewer
  /// has to be able to see every command at once: egui culls widgets that fall outside their panel, so
  /// a panel that cannot fit them all would hide half of them.
  ///
  /// The seven parameterised commands are drawn and built from [`crate::params`] — the same module the
  /// palette's chips go through — so this tab owns neither their numbers nor their payloads any more. What
  /// stays here is the tab's own furniture: the tool list, the typed rectangle, the polygon and star counts,
  /// Align, Join, Trim and the three node edits, none of which a palette chip reaches.
  fn authoring(&mut self, app: &AppState, ui: &mut egui::Ui, output: &mut ShellOutput) {
    let centre = app.camera().center();
    // The trim this selection asks for, built by the same rule the canvas menu's row uses: None while fewer
    // than two vectors are selected, which is what leaves the button dark.
    let trim = chrome::trim_action(app);

    {
      ui.heading("Authoring");
      ui.separator();
      ui.horizontal_wrapped(|ui| {
        for tool in Tool::ALL {
          if ui
            .selectable_label(self.active_tool == tool, tool.label())
            .clicked()
          {
            self.active_tool = tool;
          }
        }
        ui.label(if self.active_tool.draws() {
          "drag on the canvas to draw"
        } else {
          "click to select, drag to marquee"
        });
      });

      // A rectangle by its numbers, next to the tool that draws one by hand: the same command, one
      // gesture or two typed values.
      ui.horizontal_wrapped(|ui| {
        ui.add(
          egui::DragValue::new(&mut self.rectangle_size.0)
            .speed(1.0)
            .suffix(" mm"),
        );
        ui.label("x");
        ui.add(
          egui::DragValue::new(&mut self.rectangle_size.1)
            .speed(1.0)
            .suffix(" mm"),
        );
        if ui.button("Add rectangle").clicked() {
          output.actions.push(UiAction::AddRectangle {
            width: self.rectangle_size.0,
            height: self.rectangle_size.1,
          });
        }
      });

      ui.horizontal_wrapped(|ui| {
        ui.label("Polygon sides");
        ui.add(egui::DragValue::new(&mut self.polygon_sides).range(3..=64));
        ui.label("Star tips");
        ui.add(egui::DragValue::new(&mut self.star_points).range(3..=64));
        ui.label("inner ratio");
        ui.add(
          egui::DragValue::new(&mut self.star_inner_ratio)
            .speed(0.05)
            .range(0.05..=0.95),
        );
        ui.separator();
        ui.label("Text");
        ui.add(egui::TextEdit::singleline(&mut self.text).desired_width(70.0));
        // The seven parameterised commands draw their numbers and push their payloads from the same module
        // (`crate::params`), so a chip in the palette and this button cannot build two different actions.
        params::edit(ui, self, crate::help::Command::AddText);
        if authoring_button(ui, output, crate::help::Command::AddText) {
          output.actions.push(params::add_text(self, centre));
        }
      });

      ui.separator();
      ui.label(RichText::new("Transform").size(theme::SMALL));
      ui.horizontal_wrapped(|ui| {
        params::edit(ui, self, crate::help::Command::Move);
        if authoring_button(ui, output, crate::help::Command::Move) {
          output.actions.push(params::translate(self, centre));
        }
        ui.separator();
        params::edit(ui, self, crate::help::Command::Rotate);
        if authoring_button(ui, output, crate::help::Command::Rotate) {
          output.actions.push(params::rotate(self, centre));
        }
        ui.separator();
        params::edit(ui, self, crate::help::Command::Scale);
        if authoring_button(ui, output, crate::help::Command::Scale) {
          output.actions.push(params::scale(self, centre));
        }
        if authoring_button(ui, output, crate::help::Command::Mirror) {
          output.actions.push(params::mirror(self, centre));
        }
      });

      ui.separator();
      ui.label(RichText::new("Repeat and align").size(theme::SMALL));
      ui.horizontal_wrapped(|ui| {
        params::edit(ui, self, crate::help::Command::Array);
        if authoring_button(ui, output, crate::help::Command::Array) {
          output.actions.push(params::array(self, centre));
        }
        ui.separator();
        ui.label("Align");
        for (label, mode) in [
          ("Left", AlignMode::Left),
          ("Right", AlignMode::Right),
          ("Top", AlignMode::Top),
          ("Bottom", AlignMode::Bottom),
          ("Centre H", AlignMode::HorizontalCentre),
          ("Centre V", AlignMode::VerticalCentre),
        ] {
          if ui.button(label).clicked() {
            output.actions.push(UiAction::Align(mode));
          }
        }
      });

      ui.separator();
      ui.label(RichText::new("Vectors").size(theme::SMALL));
      ui.horizontal_wrapped(|ui| {
        params::edit(ui, self, crate::help::Command::Offset);
        if authoring_button(ui, output, crate::help::Command::Offset) {
          output.actions.push(params::offset(self, centre));
        }
        if ui.button("Join").clicked() {
          output.actions.push(UiAction::Join);
        }
        // Trim needs to know which of the two selected entities is the target and which the cutter; the
        // pairing lives in `chrome::trim_action`, so the button and the context menu's row cannot disagree.
        if ui
          .add_enabled(trim.is_some(), egui::Button::new("Trim"))
          .clicked()
        {
          output.actions.extend(trim.clone());
        }
      });
      ui.label(RichText::new("Nodes").size(theme::SMALL));
      ui.horizontal_wrapped(|ui| {
        ui.label("Node edit");
        ui.add(egui::DragValue::new(&mut self.vertex_index));
        ui.add(egui::DragValue::new(&mut self.segment_index));
        ui.add(
          egui::DragValue::new(&mut self.vertex_position.0)
            .speed(1.0)
            .suffix(" mm"),
        );
        ui.add(
          egui::DragValue::new(&mut self.vertex_position.1)
            .speed(1.0)
            .suffix(" mm"),
        );
        let vertex_at = Point2::new(self.vertex_position.0, self.vertex_position.1);
        if ui.button("Move node").clicked() {
          output.actions.push(UiAction::EditVertex(VertexEdit::Move {
            index: self.vertex_index,
            to: vertex_at,
          }));
        }
        if ui.button("Insert node").clicked() {
          output
            .actions
            .push(UiAction::EditVertex(VertexEdit::Insert {
              segment_index: self.segment_index,
              at: vertex_at,
            }));
        }
        if ui.button("Delete node").clicked() {
          output
            .actions
            .push(UiAction::EditVertex(VertexEdit::Delete {
              index: self.vertex_index,
            }));
        }
      });
    }
  }

  /// The status bar: the reference's three zones (camui-prancheta S7).
  ///
  /// Left: where the pointer is, with the two facts nothing else on the frame reports — the zoom this drawing
  /// is shown at and the steps available in each direction. Middle: the snaps, where a chip arms its kind and
  /// the grid chip names the step it snaps to. Right: what the last action produced, with the gesture's own
  /// sentence in front of it, where the reference keeps it beside the chips.
  ///
  /// The save state is **not** here: the rail's brand block names the moment the document was written
  /// (`Saved 2026-11-03 19:30 UTC` or `Unsaved changes`), and a bar saying `saved` beside a block saying when
  /// would be the same fact twice, the second time without the moment (T2b-ii, L16).
  fn status_bar(&mut self, app: &AppState, ui: &mut egui::Ui, output: &mut ShellOutput) {
    let palette = self.palette();
    let cursor = match self.cursor_world {
      Some(point) => format!("X {:.2}   Y {:.2}", point.x, point.y),
      None => "X —   Y —".to_owned(),
    };
    // "undo / redo" are the steps available in each direction; dropped steps are reported separately
    // because they are a warning, not a redo count.
    let dropped = app.history_dropped();
    let dropped = if dropped > 0 {
      format!("   {dropped} step(s) dropped")
    } else {
      String::new()
    };
    let facts = format!(
      "zoom {:.2} px/mm   undo {} / redo {}{dropped}",
      app.camera().pixels_per_mm(),
      app.history_depth(),
      app.redo_depth()
    );
    let grid = grid_chip(self.grid_spacing);
    let message = self.status.clone();
    let hint = self.hint();
    let mut chips: Vec<(String, (f32, f32))> = Vec::new();
    egui::Panel::bottom("status_bar").show(ui, |ui| {
      ui.horizontal(|ui| {
        ui.label(
          RichText::new(&cursor)
            .monospace()
            .size(theme::CAPTION)
            .color(palette.get(Role::Text)),
        );
        ui.label(
          RichText::new("mm")
            .size(theme::CAPTION)
            .color(palette.get(Role::TextDim)),
        );
        ui.separator();
        ui.label(
          RichText::new(&facts)
            .monospace()
            .size(theme::CAPTION)
            .color(palette.get(Role::TextDim)),
        );
        ui.separator();
        // The reference's `Encaixes | Ponto | Meio | Centro | Grid 10` strip: what the pointer will stick to is
        // a thing an operator arms and disarms while working, not a setting in a dialog. Each click is applied
        // in the frame it happens, so the frame that arms a chip is the frame that says it is armed (L13).
        let response = chrome::chip(ui, &palette, "Snaps", self.snapping);
        chips.push((
          "Snaps".to_owned(),
          (response.rect.center().x, response.rect.center().y),
        ));
        if response.clicked() {
          self.snapping = !self.snapping;
        }
        for (kind, label) in [
          (SnapKind::Endpoint, "Point"),
          (SnapKind::Midpoint, "Mid"),
          (SnapKind::Center, "Centre"),
        ] {
          let response = chrome::chip(ui, &palette, label, self.snap_armed(kind));
          chips.push((
            label.to_owned(),
            (response.rect.center().x, response.rect.center().y),
          ));
          if response.clicked() {
            self.toggle_snap(kind);
          }
        }
        let response = chrome::chip(ui, &palette, &grid, self.snap_armed(SnapKind::Grid));
        chips.push((
          grid.clone(),
          (response.rect.center().x, response.rect.center().y),
        ));
        if response.clicked() {
          self.toggle_snap(SnapKind::Grid);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
          // The right end belongs to one sentence: what the action just produced. The gesture's own sentence
          // sits in front of it — the reference keeps it beside the chips — and yields the width, because
          // "what just happened" is the answer an operator looks for when a command answers back.
          ui.add(
            egui::Label::new(
              RichText::new(&message)
                .size(theme::CAPTION)
                .color(palette.get(Role::TextDim)),
            )
            .truncate(),
          );
          ui.add(
            egui::Label::new(
              RichText::new(&hint)
                .size(theme::CAPTION)
                .color(palette.get(Role::TextDim)),
            )
            .truncate(),
          );
        });
      });
    });
    output.drawn.status_line = format!("{cursor} mm   {facts}   {message}");
    output.drawn.message = message;
    output.drawn.status_chips = chips;
    output.drawn.hint = hint;
  }

  /// The central panel: the drawing, the `2D | 3D` toggle, and — while 3D is the mode — the split
  /// (camui-prancheta S8).
  ///
  /// # Why the split is two columns
  ///
  /// The reference frame (`design/prancheta-reference/Screenshot 2026-11-03 032943.png`) splits the canvas
  /// left to right, and this shell's own sheet says why that is right: a part is wider than it is tall, so a
  /// horizontal split would take the drawing's long side away from it and leave both halves worse than the
  /// drawing was. Two columns keep each half's shape close to the one the drawing already had, which is also
  /// why the 2D half stays on the left with the rulers S5 gave it.
  ///
  /// # What each half keeps
  ///
  /// The 2D half **is** the canvas: [`ShellOutput::canvas`] is its rectangle, the 2D camera is told about that
  /// rectangle and nothing else, and picking, drawing, panning, zooming and the context menu all stay inside
  /// it. The 3D half owns dragging and the wheel, and its own chrome
  /// ([`view3d`](crate::view3d) — the overlay row, the home button, the triad and the playback bar). The 3D
  /// half inherits the ruler's inset (L20), so both halves' content starts on the drawing's own top line.
  ///
  /// # Why the toggle is allocated first
  ///
  /// The toggle is drawn and pressed before the split is decided, so the frame that presses `3D` is the frame
  /// that splits: `show_3d` is read by the layout below, after the click (L13's rule). It is drawn in the
  /// canvas' top strip — the band the ruler occupies on the drawing half (S5) — and never over the sheet,
  /// because a press on chrome that also dropped a selection on the drawing underneath would be a control that
  /// lies.
  fn canvas(&mut self, app: &mut AppState, ui: &mut egui::Ui, output: &mut ShellOutput, dt: f64) {
    let palette = self.palette();
    let metrics = self.metrics();
    egui::CentralPanel::default()
      .frame(egui::Frame::NONE.fill(palette.get(Role::Canvas)))
      .show(ui, |ui| {
        let area = ui.available_rect_before_wrap();
        // The whole drawing area: the central area inset by the two bands (S5, L20). The clamp is for a frame
        // whose central area has not been given its room yet — a band must not make the canvas negative.
        let thickness = metrics.ruler_thickness;
        let drawing = egui::Rect::from_min_max(
          egui::pos2(
            (area.min.x + thickness).min(area.max.x),
            (area.min.y + thickness).min(area.max.y),
          ),
          area.max,
        );
        let toggle_paint = self.view_toggle(ui, area, &metrics, output);
        let (area_2d, drawing_2d, drawing_3d) = if self.show_3d {
          // Two columns of the drawing area. The left one's own band area is the left half of the central
          // area, whose inset is exactly the drawing half — which is what keeps the rulers and the camera
          // talking about the same rectangle.
          let mid = drawing.min.x + drawing.width() * 0.5;
          (
            egui::Rect::from_min_max(area.min, egui::pos2(mid, area.max.y)),
            egui::Rect::from_min_max(drawing.min, egui::pos2(mid, drawing.max.y)),
            egui::Rect::from_min_max(egui::pos2(mid, drawing.min.y), drawing.max),
          )
        } else {
          (area, drawing, egui::Rect::NOTHING)
        };
        // The bands frame the drawing half. With the split closed that is the whole central area; with it open
        // the band numbers the drawing half only, because numbers over the 3D half would describe a camera that
        // is not there. The toggle floats on the band's right end and its own fill covers what it crosses.
        let band = if self.show_3d { area_2d } else { area };

        let canvas = CanvasRect {
          x: f64::from(drawing_2d.min.x),
          y: f64::from(drawing_2d.min.y),
          width: f64::from(drawing_2d.width()),
          height: f64::from(drawing_2d.height()),
        };
        output.canvas = canvas;
        self.last_canvas = canvas;
        app.camera_mut().set_viewport(canvas.viewport());
        // Fit against the canvas measured this frame. The status bar has already rendered with the
        // previous zoom, so a fit asks for one more frame; egui is lazy and would otherwise leave the
        // readout disagreeing with the canvas until the next mouse move.
        if self.frame_if_needed(app, &canvas) {
          ui.ctx().request_repaint();
        }

        // The bands report what they drew (S5), including whether the pointer is on the sheet: the marker is
        // drawn only while it is, and `cursor_world` is exactly that question already answered.
        output.drawn.rulers = ruler::paint(
          ui.painter(),
          band,
          &canvas,
          app.camera(),
          &palette,
          &metrics,
          self.cursor_world,
        );
        // The band painted after the widget was laid out, and a later painter wins: the toggle is painted a
        // second time here, so the control that enters 3D is never under a ruler number (R2-002 in the T6
        // review). Same function as its first paint, and `show_3d` is already the pressed value, so this frame
        // shows the view the press chose.
        if let Some(control) = toggle_paint.as_ref() {
          chrome::paint_segmented(
            ui.painter(),
            control.rect,
            &control.cell_rects,
            &["2D", "3D"],
            usize::from(self.show_3d),
            &palette,
          );
        }

        let response = ui.allocate_rect(drawing_2d, Sense::click_and_drag());
        // Everything the drawing paints is clipped to its own half: the grid and the geometry must not spill
        // onto a band, print a drawing over a number, or paint over the 3D view beside it.
        let painter = ui.painter().with_clip_rect(drawing_2d);

        self.handle_input(app, ui, &response, &canvas, output);
        // The context menu is drawn after the input that opens it, so a right-click and the rows it scopes
        // arrive in the same frame; the popup's own memory decides whether it is open at all.
        self.canvas_menu(app, &response, output);
        self.paint(painter, &canvas, app, output);

        if self.show_3d {
          output.drawn.split = Some((
            chrome::rect_report(drawing_2d),
            chrome::rect_report(drawing_3d),
          ));
          self.half_3d(app, ui, drawing_3d, output, dt);
        }
      });
  }

  /// Hand the view to the user: auto-framing stops until the shell is asked again.
  ///
  /// Called by pan and zoom gestures, and available to hosts and tests that move the camera directly.
  pub fn release_view(&mut self) {
    self.auto_frame = false;
  }

  /// Frame the document while the view still belongs to the application.
  ///
  /// Refits on every frame the canvas is plausible, which makes the result exact for the canvas that is
  /// really there: a window that opens small and is then resized ends up framed for its final size
  /// instead of for whatever it measured on the first frame. Returns true when the camera moved, so the
  /// caller can ask for one more frame — the status bar has already drawn with the old zoom.
  pub fn frame_if_needed(&mut self, app: &mut AppState, canvas: &CanvasRect) -> bool {
    if !self.auto_frame || canvas.width <= 0.0 || canvas.height <= 0.0 {
      return false;
    }
    if canvas.width < MIN_FRAME_CANVAS || canvas.height < MIN_FRAME_CANVAS {
      // Too small to be a real layout: the panels have not taken their room yet.
      return false;
    }
    let before_zoom = app.camera().pixels_per_mm();
    let before_centre = app.camera().center();
    self.frame_document(app, canvas);
    (app.camera().pixels_per_mm() - before_zoom).abs() > 1.0e-6
      || !app.camera().center().is_near(before_centre, 1.0e-6)
  }

  /// Frame the document for this canvas.
  fn frame_document(&mut self, app: &mut AppState, canvas: &CanvasRect) {
    if canvas.width <= 0.0 || canvas.height <= 0.0 {
      return;
    }
    let scene = build_scene(app.document(), app.selection());
    let mut bounds = scene.bounds();
    if bounds.is_empty() {
      let job = &app.document().job;
      bounds = cam_geom::Bounds::from_points(&[
        Point2::new(0.0, 0.0),
        Point2::new(job.width.mm(), job.height.mm()),
      ]);
    }
    if !bounds.is_empty() {
      app.fit_camera(bounds.expand(10.0), 32.0);
    }
    self.framed = true;
  }

  /// Follow the pointer: cursor world position and the snap point it is near.
  ///
  /// Runs before the panels so the status bar shows this frame's coordinates, using the canvas
  /// rectangle remembered from the last frame.
  fn track_pointer(&mut self, app: &mut AppState, ctx: &Context) {
    if self.last_canvas.width <= 0.0 || self.last_canvas.height <= 0.0 {
      return;
    }
    app.camera_mut().set_viewport(self.last_canvas.viewport());

    let Some(position) = ctx.input(|input| input.pointer.hover_pos()) else {
      self.cursor_world = None;
      self.snap_marker = None;
      return;
    };
    let local = self.last_canvas.to_screen(position);
    if local.x < 0.0
      || local.y < 0.0
      || local.x > self.last_canvas.width
      || local.y > self.last_canvas.height
    {
      self.cursor_world = None;
      self.snap_marker = None;
      return;
    }

    self.cursor_world = Some(app.camera().screen_to_world(local));
    self.snap_marker = self
      .snap_point(app, local)
      .map(|snap| (snap.kind, snap.point));
    if let Some((_, point)) = self.snap_marker {
      self.cursor_world = Some(point);
    }
  }

  /// The snap point within range of a canvas-local position, when snapping is on.
  ///
  /// One implementation for the hover readout and for the draw gesture: a gesture that snapped
  /// differently from the marker the user is looking at would be a lie.
  fn snap_point(&self, app: &AppState, local: ScreenPoint) -> Option<Snap> {
    if !self.snapping {
      return None;
    }
    let scene = build_scene(app.document(), app.selection());
    choose_snap(
      &scene,
      app.camera(),
      local,
      SNAP_TOLERANCE_PX,
      &self.snap_kinds,
      self.grid_spacing,
    )
  }

  /// The world position of a window position, snapped when a snap point is in range.
  fn world_at(&self, app: &AppState, canvas: &CanvasRect, position: Pos2) -> Point2 {
    let local = canvas.to_screen(position);
    let world = app.camera().screen_to_world(local);
    self
      .snap_point(app, local)
      .map(|snap| snap.point)
      .unwrap_or(world)
  }

  /// Turn a primary press, drag and release into exactly one drawing command.
  ///
  /// The gesture is in world millimetres: the press point is one corner and the release point the
  /// opposite one. Nothing is submitted until the release, and a gesture shorter than
  /// [`MIN_DRAW_DRAG_PX`] is a cancel — a zero-size shape is not something the user can see, select
  /// or undo on purpose, so it never becomes a command.
  fn draw_gesture(
    &mut self,
    app: &AppState,
    ui: &egui::Ui,
    response: &egui::Response,
    canvas: &CanvasRect,
    output: &mut ShellOutput,
  ) {
    let pointer = response
      .hover_pos()
      .or_else(|| response.interact_pointer_pos());

    if response.drag_started_by(egui::PointerButton::Primary) {
      self.draw_start = ui
        .ctx()
        .input(|input| input.pointer.press_origin())
        .map(|origin| self.world_at(app, canvas, origin));
    }
    if response.dragged_by(egui::PointerButton::Primary) {
      if self.draw_start.is_none() {
        // A drag that began outside the canvas still has a current point.
        self.draw_start = pointer.map(|position| self.world_at(app, canvas, position));
      }
      self.draw_current = pointer.map(|position| self.world_at(app, canvas, position));
    }
    if response.drag_stopped_by(egui::PointerButton::Primary) {
      let start = self.draw_start.take();
      let end = pointer
        .map(|position| self.world_at(app, canvas, position))
        .or(self.draw_current);
      self.draw_current = None;
      if let (Some(start), Some(end)) = (start, end) {
        let dragged_px = start.distance_to(end) * app.camera().pixels_per_mm();
        if dragged_px >= MIN_DRAW_DRAG_PX {
          match self.draw_shape(start, end) {
            Some(shape) => output.actions.push(UiAction::Draw(shape)),
            // A drag along one axis only: a rectangle, ellipse, arc or star with no height is a
            // zero-size shape, so it is cancelled and the status bar says why.
            None => {
              self.status = format!(
                "{}: drag in both directions to size the shape",
                self.active_tool.label()
              )
            }
          }
        }
      }
    }
    // A plain click with a drawing tool armed draws nothing and selects nothing: press and release in
    // the same place is a cancel.
    if response.clicked() {
      self.draw_start = None;
      self.draw_current = None;
    }
  }

  /// The shape a drag from `start` to `end` describes, or `None` when the gesture has no size.
  ///
  /// The two points are opposite corners of the gesture's box. Rectangles and polylines use them
  /// directly; circles, ellipses, polygons and stars are inscribed in that box, so the centre is the
  /// box centre and the size comes from the smaller side (circle, polygon, star) or from both sides
  /// (ellipse). An arc runs from the start corner to the end corner through the box corner
  /// `(start.x, end.y)`, which is the only third point a two-point drag names unambiguously.
  fn draw_shape(&self, start: Point2, end: Point2) -> Option<DrawShape> {
    let min_x = start.x.min(end.x);
    let max_x = start.x.max(end.x);
    let min_y = start.y.min(end.y);
    let max_y = start.y.max(end.y);
    let width = max_x - min_x;
    let height = max_y - min_y;
    let centre = Point2::new((min_x + max_x) * 0.5, (min_y + max_y) * 0.5);
    let half_smaller = width.min(height) * 0.5;

    match self.active_tool {
      Tool::Select => None,
      // A straight line only needs a direction, so a horizontal or vertical drag is a line.
      Tool::Line => Some(DrawShape::Line {
        from: start,
        to: end,
      }),
      Tool::Polyline => Some(DrawShape::Polyline {
        points: vec![start, end],
        closed: false,
      }),
      Tool::Rectangle => (width > 0.0 && height > 0.0).then_some(DrawShape::Rectangle {
        at: Point2::new(min_x, min_y),
        width,
        height,
      }),
      Tool::Arc => (width > 0.0 && height > 0.0).then_some(DrawShape::Arc {
        start,
        end,
        through: Point2::new(start.x, end.y),
      }),
      Tool::Circle => (half_smaller > 0.0).then_some(DrawShape::Circle {
        at: centre,
        radius: half_smaller,
      }),
      Tool::Ellipse => (width > 0.0 && height > 0.0).then_some(DrawShape::Ellipse {
        at: centre,
        rx: width * 0.5,
        ry: height * 0.5,
        tolerance: DEFAULT_TOLERANCE_MM,
      }),
      Tool::Polygon => (half_smaller > 0.0).then_some(DrawShape::Polygon {
        at: centre,
        radius: half_smaller,
        sides: self.polygon_sides,
        rotation: 0.0,
      }),
      Tool::Star => (half_smaller > 0.0).then_some(DrawShape::Star {
        at: centre,
        outer_radius: half_smaller,
        inner_radius: half_smaller * self.star_inner_ratio,
        points: self.star_points,
        rotation: 0.0,
      }),
    }
  }

  /// Turn pointer, wheel and keyboard input into actions and camera motion.
  ///
  /// Every branch either moves the camera (view state, no undo) or pushes a [`UiAction`] (document
  /// state, undoable). Nothing here mutates the document directly.
  fn handle_input(
    &mut self,
    app: &mut AppState,
    ui: &egui::Ui,
    response: &egui::Response,
    canvas: &CanvasRect,
    output: &mut ShellOutput,
  ) {
    let pointer = response
      .hover_pos()
      .or_else(|| response.interact_pointer_pos());

    // Wheel zooms about the cursor, but only while the canvas has the pointer.
    if response.hovered() {
      let scroll = ui.ctx().input(|input| input.smooth_scroll_delta.y);
      if scroll.abs() > 0.01 {
        self.release_view();
        let anchor = pointer
          .map(|position| canvas.to_screen(position))
          .unwrap_or_else(|| canvas.viewport().center());
        let factor = (1.0 + f64::from(scroll) * 0.002).clamp(0.2, 5.0);
        app.camera_mut().zoom_at(anchor, factor);
      }
    }

    // Middle drag pans; the content follows the cursor.
    if response.dragged_by(egui::PointerButton::Middle) {
      self.release_view();
      let delta = response.drag_delta();
      app
        .camera_mut()
        .pan_by_pixels(f64::from(delta.x), f64::from(delta.y));
    }

    if self.active_tool.draws() {
      // A drawing tool owns the primary button: press, move and release become one command, and the
      // selection gestures stay out of the way so a draw never clears the selection it just made.
      self.draw_gesture(app, ui, response, canvas, output);
    } else {
      // Primary drag draws a selection marquee.
      if response.drag_started_by(egui::PointerButton::Primary) {
        if let Some(origin) = ui.ctx().input(|input| input.pointer.press_origin()) {
          self.drag_box = Some(canvas.to_screen(origin));
        }
      }
      if response.drag_stopped_by(egui::PointerButton::Primary) {
        if let (Some(start), Some(end)) = (self.drag_box, pointer.map(|p| canvas.to_screen(p))) {
          let scene = build_scene(app.document(), app.selection());
          let hits = box_select(&scene, app.camera(), start, end);
          output.actions.push(UiAction::SetSelection(hits));
        }
        self.drag_box = None;
      }
      if response.dragged_by(egui::PointerButton::Primary) && self.drag_box.is_none() {
        // A drag that started outside the canvas still deserves a marquee.
        self.drag_box = pointer.map(|position| canvas.to_screen(position));
      }

      // A plain click selects what is under the cursor, or clears the selection.
      if response.clicked() {
        if let Some(position) = response.interact_pointer_pos() {
          let scene = build_scene(app.document(), app.selection());
          let hit = pick(
            &scene,
            app.camera(),
            canvas.to_screen(position),
            PICK_TOLERANCE_PX,
          );
          output.actions.push(UiAction::SetSelection(
            hit.map(|hit| vec![hit.entity]).unwrap_or_default(),
          ));
        }
      }
    }

    // The secondary button scopes the canvas context menu, and that is all it does. Pointing at an entity
    // selects it first, so the command picked from the menu acts on what the operator pointed at; a pointer
    // over empty canvas offers the canvas' own commands instead and leaves the selection alone.
    if response.secondary_clicked() {
      if let Some(position) = response.interact_pointer_pos() {
        let scene = build_scene(app.document(), app.selection());
        let hit = pick(
          &scene,
          app.camera(),
          canvas.to_screen(position),
          PICK_TOLERANCE_PX,
        );
        self.menu_scope = match hit {
          Some(_) => crate::help::MenuScope::Object,
          None => crate::help::MenuScope::Canvas,
        };
        if let Some(hit) = hit {
          if !app.selection().contains(&hit.entity) {
            output
              .actions
              .push(UiAction::SetSelection(vec![hit.entity]));
          }
        }
      }
    }

    // Keyboard: undo/redo, delete, fit.
    let (undo, redo, delete, fit) = ui.ctx().input(|input| {
      (
        input.modifiers.command && input.key_pressed(egui::Key::Z) && !input.modifiers.shift,
        input.modifiers.command && input.key_pressed(egui::Key::Z) && input.modifiers.shift,
        input.key_pressed(egui::Key::Delete) || input.key_pressed(egui::Key::Backspace),
        input.key_pressed(egui::Key::F) && !input.modifiers.command,
      )
    });
    if undo {
      output.actions.push(UiAction::Undo);
    }
    if redo {
      output.actions.push(UiAction::Redo);
    }
    if delete {
      output.actions.push(UiAction::DeleteSelection);
    }
    if fit {
      output.actions.push(UiAction::ZoomToFit);
    }
  }

  fn paint(
    &mut self,
    painter: egui::Painter,
    canvas: &CanvasRect,
    app: &AppState,
    output: &mut ShellOutput,
  ) {
    let palette = self.palette();
    let camera = *app.camera();
    self.grid_spacing = cam_view::nice_spacing(camera.pixels_per_mm(), TARGET_GRID_PIXELS);

    let grid = build_grid(&camera, TARGET_GRID_PIXELS);
    output.drawn.painted_grid_lines = grid.len();
    for item in &grid.items {
      paint_item(&painter, canvas, &camera, item, 1.0, &palette);
    }

    // The stock outline, so the user can see where the material ends.
    let job = &app.document().job;
    paint_world_rect(
      &painter,
      canvas,
      &camera,
      Point2::new(0.0, 0.0),
      Point2::new(job.width.mm(), job.height.mm()),
      &palette,
    );

    let scene = build_scene(app.document(), app.selection());
    output.drawn.painted_items = scene.len();
    for item in &scene.items {
      paint_item(&painter, canvas, &camera, item, 1.6, &palette);
    }

    if let Some((_, point)) = self.snap_marker {
      let position = canvas.to_window(camera.world_to_screen(point));
      let size = 5.0;
      painter.line_segment(
        [
          Pos2::new(position.x - size, position.y),
          Pos2::new(position.x + size, position.y),
        ],
        Stroke::new(1.5, palette.get(Role::Sel)),
      );
      painter.line_segment(
        [
          Pos2::new(position.x, position.y - size),
          Pos2::new(position.x, position.y + size),
        ],
        Stroke::new(1.5, palette.get(Role::Sel)),
      );
    }

    if let Some(start) = self.drag_box {
      if let Some(end) = self.cursor_world.map(|world| camera.world_to_screen(world)) {
        let from = canvas.to_window(start);
        let to = canvas.to_window(end);
        let marquee = egui::Rect::from_two_pos(from, to);
        painter.rect_stroke(
          marquee,
          0.0,
          Stroke::new(1.0, palette.get(Role::Sel)),
          egui::StrokeKind::Inside,
        );
      }
    }

    // The draw gesture's preview: the segment for the line-like tools, the gesture box for the rest.
    // It is the gesture's bounds, not the shape, because the shape itself is built by the command
    // layer on release and the shell must not own geometry rules.
    if let (Some(start), Some(current)) = (self.draw_start, self.draw_current) {
      let from = canvas.to_window(camera.world_to_screen(start));
      let to = canvas.to_window(camera.world_to_screen(current));
      if matches!(self.active_tool, Tool::Line | Tool::Polyline) {
        painter.line_segment([from, to], Stroke::new(1.5, palette.get(Role::Sel)));
      } else {
        painter.rect_stroke(
          egui::Rect::from_two_pos(from, to),
          0.0,
          Stroke::new(1.5, palette.get(Role::Sel)),
          egui::StrokeKind::Inside,
        );
      }
    }

    painter.text(
      Pos2::new(canvas.x as f32 + 8.0, canvas.y as f32 + 8.0),
      egui::Align2::LEFT_TOP,
      format!("{:.2} px/mm", camera.pixels_per_mm()),
      egui::FontId::monospace(12.0),
      palette.get(Role::TextDim),
    );
  }
}

/// Paint one scene item, flattening arcs at half a pixel.
fn paint_item(
  painter: &egui::Painter,
  canvas: &CanvasRect,
  camera: &Camera,
  item: &SceneItem,
  width: f32,
  palette: &Palette,
) {
  // Four roles carry the whole scene vocabulary: what is drawn, what is selected, what is only
  // construction, and the grid. A fifth would be a fifth kind of geometry.
  let colour = match item.style() {
    Style::Normal => palette.get(Role::Text),
    Style::Selected => palette.get(Role::Sel),
    Style::Construction => palette.get(Role::TextDim),
    Style::Grid => palette.get(Role::Hairline),
  };
  let points: Vec<Pos2> = match item {
    SceneItem::Polyline { points, .. } => points
      .iter()
      .map(|point| canvas.to_window(camera.world_to_screen(*point)))
      .collect(),
    SceneItem::Arc {
      center,
      radius,
      start_angle,
      sweep,
      ..
    } => {
      let Ok(arc) = cam_geom::Arc::new(*center, *radius, *start_angle, *sweep) else {
        return;
      };
      // Half a pixel of chord error: the flattening happens here, for this painter, and nowhere
      // earlier in the pipeline.
      let tolerance_mm = 0.5 / camera.pixels_per_mm();
      arc
        .tessellate(tolerance_mm)
        .into_iter()
        .map(|point| canvas.to_window(camera.world_to_screen(point)))
        .collect()
    }
  };
  if points.len() >= 2 {
    painter.add(egui::Shape::line(points, Stroke::new(width, colour)));
  }
}

/// Draw one Tools-tab command button, report where it landed, and say whether it was pressed.
///
/// The palette's chips report their points for the same reason ([`DrawnState::palette_hits`]): egui offers no
/// widget-by-text lookup from outside a frame, so a test that wants to press *this* button — the second of
/// the two routes a parameterised command is reachable by — is handed the point instead of guessing at the
/// panel's margins.
fn authoring_button(
  ui: &mut egui::Ui,
  output: &mut ShellOutput,
  command: crate::help::Command,
) -> bool {
  let response = ui.button(command.label());
  let centre = response.rect.center();
  output
    .drawn
    .authoring_hits
    .push((command.label().to_owned(), (centre.x, centre.y)));
  response.clicked()
}

/// Paint a world-space rectangle, used for the stock outline.
fn paint_world_rect(
  painter: &egui::Painter,
  canvas: &CanvasRect,
  camera: &Camera,
  min: Point2,
  max: Point2,
  palette: &Palette,
) {
  let corners = [
    Point2::new(min.x, min.y),
    Point2::new(max.x, min.y),
    Point2::new(max.x, max.y),
    Point2::new(min.x, max.y),
    Point2::new(min.x, min.y),
  ];
  let points: Vec<Pos2> = corners
    .iter()
    .map(|point| canvas.to_window(camera.world_to_screen(*point)))
    .collect();
  painter.add(egui::Shape::line(
    points,
    Stroke::new(1.0, palette.get(Role::Border)),
  ));
}

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
const NO_POST: &str =
  "writing a program needs a post-processor, and none is chosen — use Post… in the app bar to name one, and \
   every program this window writes is written in it";

/// What a program generation from a drawing with no toolpaths answers.
///
/// A program is a sequence of moves; a drawing alone is geometry. Naming the missing half is the whole point
/// of M13 S2, which exists because the shell used to point at a command in another window instead.
const NO_TOOLPATHS: &str =
  "this drawing has no toolpaths, so there is no program to write — author one in the Toolpaths task first, \
   and the program follows from it";

/// What a program generation to an empty path answers.
///
/// An empty path is not a file name, so the refusal comes before any filesystem call: the writer would
/// otherwise compose its temporary sibling in the directory the window was started from.
const EMPTY_PROGRAM_PATH: &str =
  "a program needs a file to write, and this path is empty — use the save dialog to name one, and no program \
   is written into the directory the window was started from";

/// What a post-processor choice with an empty path answers.
const EMPTY_POST_PATH: &str =
  "choosing a post-processor needs a file to read, and this path is empty — use the open dialog to name one";

/// What a program or a post-processor answers from an entry point that holds no shell.
///
/// The post-processor travels with the window, like the archive entries a save refuses to write without
/// ([`NO_SHELL_FOR_SAVE`]): a program written from a bare handler would be written in a dialect nobody chose.
const NO_SHELL_FOR_POST: &str =
  "writing a program needs the post-processor the window is holding, and this entry point has no shell — \
   choose one with Post… and apply the action through Shell::apply_action";

/// The file the drawing came from, as the shell holds it: the recorded path and the archive's other
/// entries.
///
/// Handed to [`handle_action_with_file`] as an `Option` because the bare [`handle_action`] entry point has
/// no shell and therefore neither: a save there refuses rather than write a file it cannot carry the
/// entries into.
struct DocumentFileState<'a> {
  /// The path the shell recorded, `None` until an open or a Save As succeeds.
  path: &'a mut Option<PathBuf>,
  /// Uninterpreted archive entries, carried from the last load and written back by the next save.
  extras: &'a mut BTreeMap<String, Vec<u8>>,
}

/// Apply one action that needs nothing but the application state.
///
/// **A bare save is refused.** The shell owns the path the drawing came from and the archive entries this
/// build does not interpret, and a write without those entries would drop them from the operator's file —
/// the one damage [`cam_store::document::DocumentFile`] exists to prevent. [`UiAction::Save`] and
/// [`UiAction::SaveFile`] therefore answer `refused: …` here, create no file and modify none, and a host
/// applies every action through [`Shell::apply_action`], which is this handler with the shell's file handed
/// in.
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
fn handle_action_with_file(
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
fn file_label(path: &Path) -> String {
  path
    .file_name()
    .map(|name| name.to_string_lossy().into_owned())
    .unwrap_or_else(|| path.display().to_string())
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

/// The grid chip's own words: the step it snaps to, in millimetres (`Grid 10`, `Grid 2.5`).
///
/// The reference writes `Grid 10`, and a chip that said only `Grid` would hide the one number an operator
/// needs to know what the snap will do. No step yet — the first frame, before the canvas has measured the
/// camera — is `Grid —`: zero is not a step, and `Grid 0` would name a snap that can never fire, because the
/// kernel arms no grid candidate at zero (`cam_view::pick`).
fn grid_chip(spacing: f64) -> String {
  if !spacing.is_finite() || spacing <= 0.0 {
    return "Grid —".to_owned();
  }
  let step = if spacing.fract().abs() < 1e-9 {
    format!("{spacing:.0}")
  } else {
    format!("{spacing:.2}")
      .trim_end_matches('0')
      .trim_end_matches('.')
      .to_owned()
  };
  format!("Grid {step}")
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
