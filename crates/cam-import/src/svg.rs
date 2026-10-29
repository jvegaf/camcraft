//! SVG import: the XML subset and the path grammar.
//!
//! Supported elements: `<path>` with the full command set, `<line>`, `<polyline>`, `<polygon>`,
//! `<rect>`, `<circle>`, `<ellipse>`; `transform` inherited through `<g>`; `viewBox` plus
//! `width`/`height` resolved to millimetres (unitless is px at 96 dpi). A circular `A` arc becomes
//! an exact [`cam_geom::Arc`]; every other curve is flattened at the caller's tolerance and counted.

use crate::{ImportError, ImportOptions, ImportReport, ImportedLayer};

/// Import an SVG.
pub fn import(
  bytes: &[u8],
  options: &ImportOptions,
  report: &mut ImportReport,
) -> Result<Vec<ImportedLayer>, ImportError> {
  let _ = (bytes, options, report);
  Err(ImportError::refused(
    "svg",
    "the svg importer is not implemented yet",
  ))
}
