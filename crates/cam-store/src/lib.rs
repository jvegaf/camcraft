//! Persistence for the CAM pipeline.
//!
//! Two stores, both versioned with forward-only migrations:
//!
//! * [`document`] — the `.c2d` file: a ZIP container holding `document.json`.
//! * [`tools`] — the SQLite tool library (`tool_geometry` / `tool_cutting_data` / `tool_entity`),
//!   normalised so the same tool carries different cutting data per material and machine.
//!
//! Must not depend on `cam-post`, `cam-toolpath` or any UI crate.

#![warn(missing_docs)]

pub mod document;

use cam_model::ModelError;

/// Anything that can go wrong reading or writing a store.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
  /// A filesystem operation failed.
  #[error("io error at {path}: {source}")]
  Io {
    /// The path involved.
    path: String,
    /// The underlying error.
    #[source]
    source: std::io::Error,
  },
  /// The archive could not be read as a ZIP container.
  #[error("{path} is not a readable c2d archive: {source}")]
  Archive {
    /// The path involved.
    path: String,
    /// The underlying error.
    #[source]
    source: zip::result::ZipError,
  },
  /// The archive exists but carries no document payload.
  #[error("{path} has no {entry} entry, so it is not a c2d document")]
  MissingEntry {
    /// The path involved.
    path: String,
    /// The entry that was expected.
    entry: String,
  },
  /// The payload is not valid JSON for the document model.
  #[error("{path} is not a valid document: {source}")]
  Json {
    /// The path involved.
    path: String,
    /// The underlying error, which carries the line and column.
    #[source]
    source: serde_json::Error,
  },
  /// The document parsed but violates a model invariant.
  #[error("{path} is not a valid document: {source}")]
  Invalid {
    /// The path involved.
    path: String,
    /// The model error.
    #[source]
    source: ModelError,
  },
  /// The tool library is not in a state this build understands.
  #[error("tool library {path} cannot be used: {reason}")]
  ToolLibrary {
    /// The path involved.
    path: String,
    /// What is wrong with it.
    reason: String,
  },
}
