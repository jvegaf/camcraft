//! Export of a document back out to an interchange format.
//!
//! # Contract
//!
//! * The document goes out as bytes in millimetres: nothing is written to disk here and the document
//!   is not mutated. [`export_bytes`] hands the caller the file it asked for, and the CLI owns the
//!   path.
//! * Bulges stay bulges. A [`cam_geom::Contour`] leaves as the same vertices carrying the same bulge
//!   on each, so the arc the kernel holds is the arc the file carries and never a flattened
//!   approximation.
//! * The same document always exports to the same bytes: no hash order, no clock, no locale. A diff
//!   between two exports therefore means the document changed, which is what makes a round trip
//!   comparable.
//! * What a format cannot carry is refused by name rather than written as a lossy approximation the
//!   caller never asked for. The losses each format *does* carry are named in its own module.

#![warn(missing_docs)]

pub mod dxf;
pub mod svg;

use cam_model::Document;

/// The formats this crate writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
  /// AutoCAD DXF, ASCII form.
  Dxf,
  /// Scalable Vector Graphics, XML form.
  Svg,
}

impl Format {
  /// The format's name, as it appears in errors and on the command line.
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

  /// The format a file extension names, if this crate writes it.
  pub fn from_extension(extension: &str) -> Option<Self> {
    match extension.to_ascii_lowercase().as_str() {
      "dxf" => Some(Self::Dxf),
      "svg" => Some(Self::Svg),
      _ => None,
    }
  }

  /// The format a `--format` value names, case-insensitively, or `None` when nothing here writes it.
  pub fn from_name(name: &str) -> Option<Self> {
    let wanted = name.trim();
    [Self::Dxf, Self::Svg]
      .into_iter()
      .find(|format| format.name().eq_ignore_ascii_case(wanted))
  }
}

/// Everything an export can refuse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportError {
  /// The document holds something the format cannot carry back unchanged.
  Refused {
    /// Format name.
    format: &'static str,
    /// Why it was refused.
    reason: String,
  },
}

impl ExportError {
  /// A refusal with a reason.
  pub fn refused(format: &'static str, reason: impl Into<String>) -> Self {
    Self::Refused {
      format,
      reason: reason.into(),
    }
  }
}

impl std::fmt::Display for ExportError {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Refused { format, reason } => write!(formatter, "{format} refused: {reason}"),
    }
  }
}

impl std::error::Error for ExportError {}

/// Export `document` in `format`, without touching the filesystem.
pub fn export_bytes(document: &Document, format: Format) -> Result<Vec<u8>, ExportError> {
  match format {
    Format::Dxf => dxf::export(document),
    Format::Svg => svg::export(document),
  }
}

#[cfg(test)]
mod tests {
  use super::Format;

  #[test]
  fn a_format_names_itself_and_its_extension() {
    assert_eq!(Format::Dxf.name(), "dxf");
    assert_eq!(Format::Dxf.extension(), "dxf");
    assert_eq!(Format::Svg.name(), "svg");
    assert_eq!(Format::Svg.extension(), "svg");
  }

  #[test]
  fn an_extension_names_the_format_case_insensitively() {
    assert_eq!(Format::from_extension("DXF"), Some(Format::Dxf));
    assert_eq!(Format::from_extension("dxf"), Some(Format::Dxf));
    assert_eq!(Format::from_extension("SVG"), Some(Format::Svg));
    assert_eq!(Format::from_extension("svg"), Some(Format::Svg));
    assert_eq!(Format::from_extension("step"), None);
    assert_eq!(Format::from_extension(""), None);
  }
}
