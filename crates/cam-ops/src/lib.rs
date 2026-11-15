//! Machining operations: profile/follow, drilling/peck, and pocket/rings.
//!
//! This crate rescues the PCB-relevant machining capabilities from `cam-toolpath` so they can
//! evolve independently of the general machining domain.

#![warn(missing_docs)]

pub mod excellon_drill;
pub mod follow;
pub mod isolation;
pub mod params;
pub mod peck;
pub mod penetration;
pub mod rings;
pub mod shared;

pub use excellon_drill::{
  plan_excellon_drill, ExcellonDrillPlan, ExcellonToolPlan, ExcellonToolSetup, SlotMode,
  ToolResolver,
};
pub use follow::{cutout, profile, CutoutRequest, ProfileRequest};
pub use isolation::{isolation_passes, pass_distance, unite_copper, IsolationPass, IsolationPath};
pub use params::{
  DrillBank, DrillParams, PocketParams, PocketStrategy, ProfileParams, ProfileSide, RetractMode,
};
pub use peck::{circle_centre, drill, DrillHole, DrillRequest, Drillpath};
pub use rings::{pocket, PocketRequest, Pocketpath};
pub use shared::{ToolSpec, ToolpathError};
