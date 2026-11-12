//! Import of foreign vector formats into millimetre geometry.
//!
//! # Contract
//!
//! * Geometry comes out as [`cam_geom::Contour`]s in millimetres: nothing is written to disk here,
//!   and no document is mutated. [`into_document`] assembles a fresh document when the caller wants
//!   one.
//! * Arcs stay arcs: a circular arc in the source file becomes a [`cam_geom::Arc`], never a
//!   polyline. Anything that cannot be an arc is flattened at the caller's tolerance **and counted**
//!   in the [`ImportReport`], because a silent approximation is a wrong part.
//! * Every importer reports what it skipped and why, with the source location when the format has
//!   one. A file that yields nothing is an error, not an empty document.

#![warn(missing_docs)]

pub mod csv;
pub mod dxf;
pub mod svg;

mod document;
mod report;

pub use document::{into_document, PLACEMENT_MARGIN};
pub use report::{ImportReport, Skipped};

use std::path::Path;

use cam_geom::Contour;

/// Default flattening tolerance for imported curves, in millimetres.
///
/// One hundredth of a millimetre: below what any router resolves, and coarse enough that a page of
/// text outlines does not explode into millions of segments.
pub const DEFAULT_TOLERANCE: f64 = 0.01;

/// The formats this crate reads, as the refusal message spells them.
pub const SUPPORTED_EXTENSIONS: &str = "dxf, svg";

/// A source format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
  /// AutoCAD DXF, ASCII form.
  Dxf,
  /// SVG, the subset described in [`svg`].
  Svg,
}

impl Format {
  /// The format's name, as it appears in reports and errors.
  pub fn name(self) -> &'static str {
    match self {
      Self::Dxf => "dxf",
      Self::Svg => "svg",
    }
  }

  /// The canonical file extension for the format.
  pub fn extension(self) -> &'static str {
    match self {
      Self::Dxf => "dxf",
      Self::Svg => "svg",
    }
  }

  fn from_extension(extension: &str) -> Option<Self> {
    match extension {
      "dxf" => Some(Self::Dxf),
      "svg" => Some(Self::Svg),
      _ => None,
    }
  }
}

/// Options every importer honours.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImportOptions {
  /// Flattening tolerance for curves that cannot stay arcs, in millimetres.
  pub tolerance: f64,
}

impl Default for ImportOptions {
  fn default() -> Self {
    Self {
      tolerance: DEFAULT_TOLERANCE,
    }
  }
}

/// One imported layer, before it becomes a document layer.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedLayer {
  /// Name as it appears in the source file; the importers never invent one.
  pub name: String,
  /// Geometry in millimetres.
  pub contours: Vec<Contour>,
}

/// The result of one import.
#[derive(Debug, Clone, PartialEq)]
pub struct Imported {
  /// The detected format.
  pub format: Format,
  /// Geometry grouped by source layer.
  pub layers: Vec<ImportedLayer>,
  /// What the import did, in a form a reviewer can check.
  pub report: ImportReport,
}

/// Everything an import can refuse.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImportError {
  /// The file could not be read.
  #[error("{path}: {message}")]
  Io {
    /// Path as it was given.
    path: String,
    /// The operating system's reason.
    message: String,
  },
  /// Neither the extension nor the content identified a format this crate reads.
  #[error("unsupported format {extension}; supported formats are {supported}")]
  Unsupported {
    /// Extension as found, or `"<none>"`.
    extension: String,
    /// The formats that are supported.
    supported: &'static str,
  },
  /// The file is the right format but not readable as such.
  #[error("{format} at {location}: {message}", location = location.as_deref().unwrap_or("?"))]
  Parse {
    /// Format name.
    format: &'static str,
    /// What is wrong.
    message: String,
    /// Where it is wrong, when the format has a position.
    location: Option<String>,
  },
  /// The file is valid but uses a feature this milestone refuses by name.
  #[error("{format} refused: {reason}")]
  Refused {
    /// Format name.
    format: &'static str,
    /// Why it was refused.
    reason: String,
  },
  /// The file parsed but produced no vector geometry.
  #[error("{format}: nothing to import: {reason}")]
  NothingImported {
    /// Format name.
    format: &'static str,
    /// Why nothing was produced.
    reason: String,
  },
}

impl ImportError {
  /// A parse failure with a location.
  pub fn parse(
    format: &'static str,
    message: impl Into<String>,
    location: impl Into<String>,
  ) -> Self {
    Self::Parse {
      format,
      message: message.into(),
      location: Some(location.into()),
    }
  }

  /// A parse failure without a location.
  pub fn parse_plain(format: &'static str, message: impl Into<String>) -> Self {
    Self::Parse {
      format,
      message: message.into(),
      location: None,
    }
  }

  /// A refusal with a reason.
  pub fn refused(format: &'static str, reason: impl Into<String>) -> Self {
    Self::Refused {
      format,
      reason: reason.into(),
    }
  }
}

/// Decide the format of `bytes`, which came from `path`.
///
/// The extension decides first, because it is what the user chose to name the file; when the
/// extension is unknown, the content signature is tried before refusing.
pub fn detect(path: &Path, bytes: &[u8]) -> Result<Format, ImportError> {
  let extension = path
    .extension()
    .and_then(|extension| extension.to_str())
    .map(|extension| extension.to_ascii_lowercase());

  if let Some(format) = extension.as_deref().and_then(Format::from_extension) {
    return Ok(format);
  }

  if let Some(format) = sniff(bytes) {
    return Ok(format);
  }

  Err(ImportError::Unsupported {
    extension: extension.unwrap_or_else(|| "<none>".to_string()),
    supported: SUPPORTED_EXTENSIONS,
  })
}

/// Identify a format from its content signature.
fn sniff(bytes: &[u8]) -> Option<Format> {
  let head = &bytes[..bytes.len().min(65_536)];
  let text = String::from_utf8_lossy(head);
  let trimmed = text.trim_start_matches(['\u{feff}', ' ', '\t', '\r', '\n']);

  if trimmed.starts_with("<?xml") || trimmed.starts_with("<svg") || text.contains("<svg") {
    return Some(Format::Svg);
  }
  if is_dxf(&text) {
    return Some(Format::Dxf);
  }
  None
}

/// A DXF is a stream of group-code pairs; the first record of a real file is `0 / SECTION`.
fn is_dxf(text: &str) -> bool {
  let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty());
  let starts_with_section =
    matches!(lines.next(), Some("0")) && matches!(lines.next(), Some("SECTION"));
  starts_with_section
    || (text.lines().any(|line| line.trim() == "ENTITIES")
      && text.lines().any(|line| line.trim() == "EOF"))
}

/// Import `bytes`, which came from `source`, without touching the filesystem.
///
/// `source` is only used for the name it carries: format detection and diagnostics.
pub fn import_bytes(
  source: &str,
  bytes: &[u8],
  options: &ImportOptions,
) -> Result<Imported, ImportError> {
  let format = detect(Path::new(source), bytes)?;
  let mut report = ImportReport::new(format.name(), options.tolerance);

  let layers = match format {
    Format::Dxf => dxf::import(bytes, options, &mut report)?,
    Format::Svg => svg::import(bytes, options, &mut report)?,
  };

  report.contours = layers.iter().map(|layer| layer.contours.len()).sum();
  if report.contours == 0 {
    let reason = match report.skipped.first() {
      Some(skipped) => format!(
        "every construct was skipped, starting with {}",
        skipped.construct
      ),
      None => "the file contains no vector geometry".to_string(),
    };
    return Err(ImportError::NothingImported {
      format: format.name(),
      reason,
    });
  }

  Ok(Imported {
    format,
    layers,
    report,
  })
}

/// Read `path` and import it.
pub fn import_path(path: &Path, options: &ImportOptions) -> Result<Imported, ImportError> {
  let bytes = std::fs::read(path).map_err(|error| ImportError::Io {
    path: path.display().to_string(),
    message: error.to_string(),
  })?;
  import_bytes(&path.display().to_string(), &bytes, options)
}
