//! Material-removal simulation: the stock as a heightfield, the program's sweep over it, and what the
//! document authorised each cell to lose.
//!
//! The simulation's input is the [`cam_move::MoveStream`] the emitter would run, never a strategy's
//! intentions (D-M5-1): an oracle that re-reads the intent it is meant to check is not one.
//!
//! Must not depend on `cam-toolpath`, `cam-post`, `cam-store` or any UI crate.

#![warn(missing_docs)]

pub mod grid;
pub mod heightfield;
pub mod sweep;
pub mod target;

pub use grid::{grid_for, Grid, GridError, DEFAULT_CELL, MAX_CELLS};
pub use heightfield::{Heightfield, Volumes};
pub use sweep::{apply, apply_move, Crash, Removal};
