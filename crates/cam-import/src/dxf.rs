//! DXF import: group-code stream, layers, and the exact entity types we can cut.
//!
//! Supported entities: `LINE`, `LWPOLYLINE` (with bulges), `POLYLINE`/`VERTEX`/`SEQEND` (with
//! bulges), `ARC`, `CIRCLE`, `ELLIPSE`. A bulge becomes a [`cam_geom::Arc`], never a polyline.
//! Unknown entity types are skipped and reported; a malformed group code is a located parse error.

use crate::{ImportError, ImportOptions, ImportReport, ImportedLayer};

/// Import an ASCII DXF.
pub fn import(
  bytes: &[u8],
  options: &ImportOptions,
  report: &mut ImportReport,
) -> Result<Vec<ImportedLayer>, ImportError> {
  let _ = (bytes, options, report);
  Err(ImportError::refused(
    "dxf",
    "the dxf importer is not implemented yet",
  ))
}
