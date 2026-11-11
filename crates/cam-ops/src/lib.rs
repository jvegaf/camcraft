//! Machining operations: profile/follow and pocket/rings.
//!
//! This crate rescues the PCB-relevant machining capabilities from `cam-toolpath` so they can
//! evolve independently of the general machining domain.

#![warn(missing_docs)]

pub mod follow;
pub mod rings;
pub mod shared;

pub use follow::{profile, ProfileRequest};
pub use rings::{pocket, PocketRequest, Pocketpath};
pub use shared::{ToolSpec, ToolpathError};
