//! Machining operations: profile/follow, drilling/peck, and pocket/rings.
//!
//! This crate rescues the PCB-relevant machining capabilities from `cam-toolpath` so they can
//! evolve independently of the general machining domain.

#![warn(missing_docs)]

pub mod excellon_drill;
pub mod follow;
pub mod isolation;
pub mod ncc;
pub mod ncc_index;
pub mod params;
pub mod peck;
pub mod penetration;
pub mod rings;
pub mod shared;

pub use excellon_drill::{
  plan_excellon_drill, ExcellonDrillPlan, ExcellonToolPlan, ExcellonToolSetup, SlotMode,
  ToolResolver,
};
pub use follow::{cutout, mouse_bite_drills, profile, CutoutRequest, ProfileRequest};
pub use isolation::{
  check_isolation_anti_gouge, isolation_moves, isolation_passes, isolation_passes_with_kind,
  pass_distance, unite_copper, vbit_isolation_depth_mm, IsolationPass, IsolationPath,
};
pub use ncc::{
  ncc_fill_lines, ncc_fill_rings, ncc_fill_seed, ncc_moves, ncc_rest_region, non_copper_envelope,
  swept_band_region, NccFill,
};
pub use params::{
  DrillBank, DrillParams, PocketParams, PocketStrategy, ProfileParams, ProfileSide, RetractMode,
};
pub use peck::{circle_centre, drill, DrillHole, DrillRequest, Drillpath};
pub use rings::{pocket, PocketRequest, Pocketpath};
pub use shared::{ToolSpec, ToolpathError};
