//! Headless tests for the 3D preview panel (M5 A9, A10, A11, A12).
//!
//! No display and no GPU: `egui::Context` runs a full frame on the CPU, the panel reports how many
//! triangles and 3D lines it painted, and the shape list says whether the surface really reached the
//! painter as an `egui::Mesh` (A11). The panel is additive (A12): while it is open the 2D canvas keeps
//! painting its own items, and closing it restores the M1–M3 picture exactly.
//!
//! The playback tests (A10) drive [`cam_ui::Preview3d::advance`] with an injected delta and no clock:
//! the program's own time is a closed form — each move's duration is its length over the segment's
//! feed rate — so the cursor, the fraction of the current move and the removed volume are all
//! asserted against numbers written out by hand, and the field at the end is compared cell for cell
//! with the whole-program [`sweep::apply`] the report uses.
//!
//! The published seconds (`seconds_for`) are held to the same standard: the estimate beside the
//! toolpath card may not be a second time formula, so a test asserts it is exactly the number the
//! playback clock reports for the same program (camui-toolpath S5).

#[path = "view3d/overlay.rs"]
mod overlay;
#[path = "view3d/playback.rs"]
mod playback;
#[path = "view3d/playback_bar.rs"]
mod playback_bar;
#[path = "view3d/scene.rs"]
mod scene;
#[path = "view3d/simulation.rs"]
mod simulation;
#[path = "view3d/split.rs"]
mod split;
#[path = "view3d/support.rs"]
mod support;
