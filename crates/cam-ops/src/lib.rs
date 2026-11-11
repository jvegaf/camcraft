//! Machining operations: profile/follow.
//!
//! This crate rescues the PCB-relevant machining capabilities from `cam-toolpath` so they can
//! evolve independently of the general machining domain.

#![warn(missing_docs)]

pub mod follow;
pub mod shared;

pub use follow::{profile, ProfileRequest};
pub use shared::{ToolSpec, ToolpathError};
