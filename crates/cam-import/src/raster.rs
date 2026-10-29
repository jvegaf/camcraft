//! Raster tracing: PNG in, contours out.
//!
//! The decoder is hand-written (8-bit greyscale, RGB, RGBA and palette, non-interlaced) and inflated
//! with `flate2`. Luminance → global threshold → contour extraction; `--min-area` and `--simplify`
//! are applied and reported, so the same input and options produce the same output (D-M3-8).

use crate::{ImportError, ImportOptions, ImportReport, ImportedLayer};

/// Trace a PNG into closed contours in millimetres.
pub fn import(
  bytes: &[u8],
  options: &ImportOptions,
  report: &mut ImportReport,
) -> Result<Vec<ImportedLayer>, ImportError> {
  let _ = (bytes, options, report);
  Err(ImportError::refused(
    "png",
    "the raster tracer is not implemented yet",
  ))
}
