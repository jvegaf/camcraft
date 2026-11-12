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
//! [`UiAction::Align`], [`UiAction::Offset`], [`UiAction::Join`], [`UiAction::Trim`] and
//! [`UiAction::EditVertex`] carry the edit. The editing groups read their operands from the selection,
//! so the selection *is* the operand of every edit and no command needs a wizard. Numbers the user does
//! not type during a gesture come from [`Shell`]'s documented fields and from
//! [`DEFAULT_TOLERANCE_MM`] / [`JOIN_TOLERANCE_MM`]; [`Shell::apply_action`] then puts what the command
//! produced — entity count, ellipse error, lost arcs, or the kernel's refusal — in the status bar.
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

mod actions;
pub mod chrome;
/// The command table and its teaching voice.
pub mod help;
pub mod icons;
/// The six commands whose numbers travel with their builders: `Move`, `Rotate`, `Scale`, `Mirror`,
/// `Array` and `Offset`.
pub mod params;
/// The two millimetre rulers along the canvas' top and left edges (camui-prancheta S5).
pub mod ruler;
/// The scripting panel: one script path, one `Run`, one output area (M10 T4b, S4).
///
/// It runs the script by calling `cam-cli` as a library — never by spawning the binary — and its
/// running half needs no egui at all, so a caller without a window can still run a script and read what
/// it produced.
pub mod script_panel;
mod shell;
/// The brand block's `Saved …` line: a civil date and time from a [`std::time::SystemTime`].
pub mod stamp;
/// The start screen: the modal the first frame shows when the window holds no project (start-screen S2, S3).
pub mod start_screen;
pub mod theme;
/// The tool library as a master-detail modal (camui-prancheta S9).
pub mod tool_library;
pub mod view3d;

use cam_app::AppState;
use cam_model::{Job, Toolpath};

pub use actions::{handle_action, ActionReport, UiAction};
pub use chrome::{Category, Tab};
pub use ruler::{RulerMarker, RulerTick, Rulers};
pub use shell::{CanvasRect, DrawShape, DrawnState, PixelRect, Shell, ShellOutput, Tool};
pub use start_screen::RecentProject;
pub use tool_library::{ToolFamily, ToolFormGroup};
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

/// The toolpath a selection names, or the document's first toolpath when it names one the document does not
/// hold (M13 T3, S4).
///
/// One function and every reader that needs the described toolpath, so the fallback rule has exactly one
/// spelling: a stale selection is not a state of its own, it is the same answer as no selection at all.
pub(crate) fn resolve_toolpath<'a>(
  app: &'a AppState,
  selection: Option<&str>,
) -> Option<&'a Toolpath> {
  let toolpaths = &app.document().toolpaths;
  selection
    .and_then(|id| toolpaths.iter().find(|toolpath| toolpath.id == id))
    .or_else(|| toolpaths.first())
}

/// The three numbers the material dialog starts with: the default sheet, in millimetres.
///
/// One function and two callers — the shell's own default and an open that has to drop drafts seeded
/// from a document nobody is looking at any more — so the seeds cannot drift apart.
fn default_material_size() -> (f64, f64, f64) {
  let sheet = Job::default_sheet();
  (sheet.width.mm(), sheet.height.mm(), sheet.thickness.mm())
}
