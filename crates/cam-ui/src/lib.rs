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
//! # The 3D preview (M5 V2, V3)
//!
//! [`view3d`] adds the stock's 3D picture to the shell: a top panel that paints a simulated
//! [`cam_sim::Heightfield`] through `egui::Mesh`, orbit and wheel-zoom on the same painter, and the
//! program's moves as projected 3D lines. It is **additive**: [`Shell::show`] keeps its 2D path when
//! the panel is closed (A12), and the panel never authors geometry or touches the document
//! (D-M5-9) — the caller hands in the untouched stock and the program, and the panel plays it (A10).
//!
//! Playback is the one thing in this crate that needs a clock, and [`Shell::show_in`] is the only
//! place that reads one: `ctx.input(|input| input.stable_dt)` becomes the delta
//! [`Preview3d::advance`] consumes, so the arithmetic itself stays a pure function a test drives.
//!
//! Must not contain geometry, toolpath or document rules.

#![warn(missing_docs)]

use cam_app::{AlignMode, AppState, Camera, Command, ScreenPoint, Transform, VertexEdit, Viewport};
use cam_geom::{Point2, Vec2};
use cam_model::Job;
use cam_move::MoveStream;
use cam_sim::Heightfield;
use cam_view::{
  box_select, build_grid, build_scene, choose_snap, pick, SceneItem, Snap, SnapKind, Style,
  PICK_TOLERANCE_PX, SNAP_TOLERANCE_PX, TARGET_GRID_PIXELS,
};
use egui::{Context, Pos2, RichText, Sense, Stroke};

use crate::icons::{Family, Icon};
use crate::theme::{Palette, Role, Theme};

pub mod chrome;
pub mod icons;
pub mod theme;

pub use chrome::{Stage, Tab};
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
}

impl UiAction {
  /// A short label for the status bar.
  pub fn label(&self) -> &'static str {
    match self {
      Self::Undo => "undo",
      Self::Redo => "redo",
      Self::ZoomToFit => "zoom to fit",
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
  /// Stock edges and toolpath edges the 3D panel painted, `0` when the panel is closed.
  pub painted_3d_lines: usize,
  /// The stage the frame drew, and the rail entries it offered in order.
  ///
  pub stage: Stage,
  /// What the left palette offered, in the order it drew them.
  ///
  /// The palette is the part of the frame whose content changes with the stage, and a headless test cannot
  /// read an icon: this is how "switching stage really changed what the operator can reach" is asserted.
  pub palette: Vec<String>,
  /// How much of the previewed program has played, in `0.0..=1.0`.
  ///
  /// The value the panel's own readout ends on: exactly `0.0` while the preview stands on its first
  /// move and exactly `1.0` once the program has finished (M5 A10). Stays `0.0` when the panel is
  /// closed, because a closed panel is not a clock.
  pub preview_progress: f64,
}

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
  /// Whether the 3D preview panel is open.
  ///
  /// View state, not document state: like the camera, opening the panel is not an edit and is not
  /// in the undo history (M5 A12). [`Shell::set_preview`] opens it, and the command bar's button
  /// toggles it.
  pub show_3d: bool,
  /// The program preview the 3D panel draws, once a caller has handed one in.
  preview: Option<Preview3d>,
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
  /// Which stage of the job the frame is showing. View state: switching it pushes no command.
  stage: Stage,
  /// Which inspector tab is showing.
  inspector_tab: Tab,
  /// Whether the inspector is expanded. Collapsed it leaves a 12 px strip that reopens on hover.
  inspector_open: bool,
  /// Whether the shortcuts window is open.
  show_shortcuts: bool,
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
      preview: None,
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
      stage: Stage::default(),
      inspector_tab: Tab::default(),
      inspector_open: true,
      show_shortcuts: false,
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

  /// Apply one action and show what it produced in the status bar.
  ///
  /// This is what a host calls for every action [`Self::show`] returned. It keeps the one-command
  /// rule — the work itself happens in [`handle_action`] — and turns the outcome the command layer
  /// reports (entity count, ellipse radial error, arcs lost to tessellation) and any refusal, with the
  /// kernel's own message, into the sentence the user reads.
  pub fn apply_action(&mut self, app: &mut AppState, action: &UiAction) {
    self.status = match handle_action(app, action) {
      Ok(report) => report.message(),
      Err(message) => format!("refused: {message}"),
    };
  }

  /// Hand the 3D panel a program to preview: the stock it cuts, the program itself, and the stock box.
  ///
  /// `field` must be the **untouched** stock — [`Heightfield::new`] for `job`, at the cell size the
  /// caller chose — and `program` the moves to cut it with. The panel plays it (M5 A10): as the
  /// cursor advances, whole moves are applied to the field, so the surface a viewer watches is the
  /// same simulation [`cam_sim::sweep::apply`] gives the report, cell for cell.
  ///
  /// This method reads no document and returns no [`UiAction`], which is what keeps the preview off
  /// the document's authoring path (M5 D-M5-9, A12): the window shows the preview of a program
  /// authored through the CLI, it never authors one.
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
    // The crate's only clock read (A10). Everything that needs time — playback, and nothing else —
    // receives this delta as an argument, so the arithmetic stays a pure function a test can drive
    // frame by frame. `stable_dt` rather than `unstable_dt`: a frame that stalled (a resize, a
    // breakpoint) must not jump the animation forward by the length of the stall.
    let dt = f64::from(ctx.input(|input| input.stable_dt));
    self.track_pointer(app, &ctx);
    // Bars first, then the sides, then the canvas: egui lays panels out in the order they are added,
    // and a central panel must come last or it would claim the space the sides need.
    self.app_bar(app, ui, &mut output);
    self.stage_bar(app, ui);
    self.status_bar(app, ui, &mut output);
    self.palette_panel(app, ui, &mut output);
    self.inspector(app, ui, &mut output);
    // One view or the other, never both: the toggle is a mode, and the mode owns the central area.
    if self.show_3d {
      self.view3d(ui, &mut output, dt);
    } else {
      self.canvas(app, ui, &mut output);
    }
    if self.show_shortcuts {
      self.shortcuts_window(&ctx);
    }
    output
  }

  /// The 3D view, drawn where the canvas would be.
  ///
  /// A mode, not a panel stacked over the drawing: the preview used to take a slice of the top and leave
  /// the canvas the rest, which gave both views half a screen and neither a usable one. Two views that are
  /// never on screen together also means the hidden one costs nothing — no playback clock, no mesh, no
  /// region. The surface fills whatever the frame gives it.
  fn view3d(&mut self, ui: &mut egui::Ui, output: &mut ShellOutput, dt: f64) {
    let palette = self.palette();
    egui::CentralPanel::default().show(ui, |ui| {
      ui.horizontal(|ui| {
        ui.label(egui::RichText::new("3D").size(theme::TITLE));
        ui.separator();
        if let Some(preview) = self.preview.as_mut() {
          // Playback advances only while this view is the one showing: a hidden view is not a clock, and
          // the 2D path pays nothing for a preview nobody is looking at (A12).
          preview.advance(dt);
          if preview.playing() {
            // Without this the window would repaint only on input, and a program that plays to its end
            // would appear to play only when the mouse moves.
            ui.ctx().request_repaint();
          }
          preview.controls(ui);
          ui.separator();
          ui.label(
            egui::RichText::new(format!(
              "{} move(s), {} × {} cells, stride {}",
              preview.program().move_count(),
              preview.field().grid().columns,
              preview.field().grid().rows,
              preview.stride()
            ))
            .size(theme::CAPTION)
            .color(palette.get(Role::TextDim)),
          );
          output.drawn.preview_progress = preview.progress();
        } else {
          ui.label(
            egui::RichText::new(
              "No program to preview. Simulate one with `cam-cli simulate` and hand it in.",
            )
            .size(theme::CAPTION)
            .color(palette.get(Role::TextDim)),
          );
        }
      });
      ui.separator();
      if let Some(preview) = self.preview.as_mut() {
        preview.show(ui, &mut output.drawn);
      } else {
        // An empty 3D view still holds its region: a collapsing view would hand the click to whatever is
        // underneath and read as a bug.
        ui.allocate_space(ui.available_size());
      }
    });
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
  fn authoring(&mut self, app: &AppState, ui: &mut egui::Ui, output: &mut ShellOutput) {
    let centre = app.camera().center();
    let selected: Vec<String> = app.selection().ids().map(str::to_owned).collect();

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
        ui.add(
          egui::DragValue::new(&mut self.text_height)
            .speed(1.0)
            .suffix(" mm"),
        );
        if ui.button("Add text").clicked() {
          output.actions.push(UiAction::AddText {
            text: self.text.clone(),
            height: self.text_height,
            origin: centre,
          });
        }
      });

      ui.separator();
      ui.label(RichText::new("Transform").size(theme::SMALL));
      ui.horizontal_wrapped(|ui| {
        ui.label("Move");
        ui.add(
          egui::DragValue::new(&mut self.move_delta.0)
            .speed(1.0)
            .suffix(" mm"),
        );
        ui.add(
          egui::DragValue::new(&mut self.move_delta.1)
            .speed(1.0)
            .suffix(" mm"),
        );
        if ui.button("Move").clicked() {
          output
            .actions
            .push(UiAction::Transform(Transform::Translate {
              delta: Vec2::new(self.move_delta.0, self.move_delta.1),
            }));
        }
        ui.separator();
        ui.label("Rotate");
        ui.add(
          egui::DragValue::new(&mut self.rotate_degrees)
            .speed(1.0)
            .suffix("°"),
        );
        if ui.button("Rotate").clicked() {
          output.actions.push(UiAction::Transform(Transform::Rotate {
            radians: self.rotate_degrees.to_radians(),
            about: centre,
          }));
        }
        ui.separator();
        ui.label("Scale");
        ui.add(egui::DragValue::new(&mut self.scale_factors.0).speed(0.05));
        ui.add(egui::DragValue::new(&mut self.scale_factors.1).speed(0.05));
        if ui.button("Scale").clicked() {
          output.actions.push(UiAction::Transform(Transform::Scale {
            sx: self.scale_factors.0,
            sy: self.scale_factors.1,
            about: centre,
            tolerance: DEFAULT_TOLERANCE_MM,
          }));
        }
        if ui.button("Mirror").clicked() {
          output.actions.push(UiAction::Transform(Transform::Mirror {
            point: centre,
            direction: Vec2::Y_AXIS,
          }));
        }
      });

      ui.separator();
      ui.label(RichText::new("Repeat and align").size(theme::SMALL));
      ui.horizontal_wrapped(|ui| {
        ui.label("Array");
        ui.add(egui::DragValue::new(&mut self.array_grid.0).range(1..=64));
        ui.add(egui::DragValue::new(&mut self.array_grid.1).range(1..=64));
        ui.add(
          egui::DragValue::new(&mut self.array_spacing.0)
            .speed(1.0)
            .suffix(" mm"),
        );
        ui.add(
          egui::DragValue::new(&mut self.array_spacing.1)
            .speed(1.0)
            .suffix(" mm"),
        );
        if ui.button("Array").clicked() {
          output.actions.push(UiAction::Array {
            rows: self.array_grid.0,
            columns: self.array_grid.1,
            dx: self.array_spacing.0,
            dy: self.array_spacing.1,
          });
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
        ui.label("Offset");
        ui.add(
          egui::DragValue::new(&mut self.offset_distance)
            .speed(0.5)
            .suffix(" mm"),
        );
        if ui.button("Offset").clicked() {
          output.actions.push(UiAction::Offset {
            distance: self.offset_distance,
          });
        }
        if ui.button("Join").clicked() {
          output.actions.push(UiAction::Join);
        }
        // Trim needs to know which of the two selected entities is the target and which the cutter;
        // the selection is ordered by id, so the first id is the target and the second the cutter.
        if ui
          .add_enabled(selected.len() >= 2, egui::Button::new("Trim"))
          .clicked()
        {
          if let [target, cutter, ..] = selected.as_slice() {
            output.actions.push(UiAction::Trim {
              target: target.clone(),
              cutter: cutter.clone(),
            });
          }
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

  fn status_bar(&mut self, app: &AppState, ui: &mut egui::Ui, output: &mut ShellOutput) {
    let palette = self.palette();
    egui::Panel::bottom("status_bar").show(ui, |ui| {
      ui.horizontal(|ui| {
        for (label, value) in [
          ("units", "mm".to_owned()),
          ("snap", format!("{:.2} mm", self.grid_spacing)),
          (
            "snapping",
            if self.snapping { "on" } else { "off" }.to_owned(),
          ),
        ] {
          ui.label(
            RichText::new(format!("{label} {value}"))
              .size(theme::CAPTION)
              .color(palette.get(Role::TextDim)),
          );
        }
        ui.separator();
        let cursor = match self.cursor_world {
          Some(point) => format!("X {:.2}  Y {:.2} mm", point.x, point.y),
          None => "—".to_owned(),
        };
        let dirty = if app.is_dirty() { "modified" } else { "saved" };
        // "undo / redo" are the steps available in each direction; dropped steps are reported
        // separately because they are a warning, not a redo count.
        let dropped = app.history_dropped();
        let dropped = if dropped > 0 {
          format!("   {dropped} step(s) dropped")
        } else {
          String::new()
        };
        let line = format!(
          "{cursor}   zoom {:.2} px/mm   {dirty}   undo {} / redo {}{dropped}   {}",
          app.camera().pixels_per_mm(),
          app.history_depth(),
          app.redo_depth(),
          self.status
        );
        ui.monospace(&line);
        output.drawn.status_line = line;
      });
    });
  }

  fn canvas(&mut self, app: &mut AppState, ui: &mut egui::Ui, output: &mut ShellOutput) {
    let palette = self.palette();
    egui::CentralPanel::default()
      .frame(egui::Frame::NONE.fill(palette.get(Role::Canvas)))
      .show(ui, |ui| {
        let available = ui.available_rect_before_wrap();
        let (response, painter) = ui.allocate_painter(available.size(), Sense::click_and_drag());

        let canvas = CanvasRect {
          x: f64::from(response.rect.min.x),
          y: f64::from(response.rect.min.y),
          width: f64::from(response.rect.width()),
          height: f64::from(response.rect.height()),
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

        self.handle_input(app, ui, &response, &canvas, output);
        self.paint(painter, &canvas, app, output);
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

/// Apply one action to the application state.
///
/// The only place the shell touches [`AppState`], so a test can drive the whole interface by feeding
/// actions. Every authoring arm builds exactly one [`Command`] and runs it through
/// [`apply_command`]; nothing here mutates the document itself.
pub fn handle_action(app: &mut AppState, action: &UiAction) -> Result<ActionReport, String> {
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
