//! PDF import: vector content streams only.
//!
//! The object scan is tolerant of a damaged xref, `FlateDecode` streams are inflated, and the path
//! operators are interpreted with the `cm` transform and the `MediaBox` origin honoured. Text,
//! images, shadings and Type3 glyph procedures are counted as skipped (D-M3-5).

use crate::{ImportError, ImportOptions, ImportReport, ImportedLayer};

/// Import the vector paths of every page.
pub fn import(
  bytes: &[u8],
  options: &ImportOptions,
  report: &mut ImportReport,
) -> Result<Vec<ImportedLayer>, ImportError> {
  let _ = (bytes, options, report);
  Err(ImportError::refused(
    "pdf",
    "the pdf importer is not implemented yet",
  ))
}
