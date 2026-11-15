//! One module per command: each owns its argument parsing and execution, and the dispatcher in
//! `crate::run` selects among them. Keeping command implementations here (not in `lib.rs`) is what
//! lets future commands such as `drill` or `cutout` land in their own files without turning
//! `lib.rs` into a merge bottleneck (ola 4 preparation).

pub mod check;
pub mod cutout;
pub mod drill;
pub mod export;
pub mod import;
pub mod inspect;
pub mod postp;
pub mod tools;
pub mod validate;
