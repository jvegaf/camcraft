//! EPS import: the PostScript path subset.
//!
//! Path operators `newpath`, `moveto`, `lineto`, `curveto`, `rlineto`, `rcurveto`, `closepath`,
//! `arc`, `arcn` are interpreted with `translate`, `scale`, `rotate`, `concat` and `setmatrix`
//! tracked through a transformation stack; `stroke`, `fill`, `eofill` and `clip` close the path.
//! `%%BoundingBox` places the drawing.

use crate::{ImportError, ImportOptions, ImportReport, ImportedLayer};

/// Import an EPS.
pub fn import(
  bytes: &[u8],
  options: &ImportOptions,
  report: &mut ImportReport,
) -> Result<Vec<ImportedLayer>, ImportError> {
  let _ = (bytes, options, report);
  Err(ImportError::refused(
    "eps",
    "the eps importer is not implemented yet",
  ))
}
