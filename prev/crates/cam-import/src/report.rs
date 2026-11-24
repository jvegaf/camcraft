//! The report every importer fills in, and the one deterministic way to render it.

use std::collections::BTreeMap;

/// One construct the importer could not turn into geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
  /// What was skipped, as the source format names it (`TEXT`, `<image>`, `Do`).
  pub construct: String,
  /// Why it was skipped.
  pub reason: String,
  /// Where it is in the source, when the format has a position (`line 412`, `object 7`).
  pub location: Option<String>,
}

/// What an import did, in the terms a reviewer checks.
///
/// The counts and lists here are the difference between an importer that loses geometry and one
/// that reports it: nothing may be dropped without appearing in [`ImportReport::skipped`] or
/// [`ImportReport::warnings`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImportReport {
  /// Format name.
  pub format: &'static str,
  /// Flattening tolerance applied to curves that could not stay arcs.
  pub tolerance: f64,
  /// Scale applied to source units to reach millimetres.
  pub unit_scale: f64,
  /// Contours produced.
  pub contours: usize,
  /// Curves flattened to line segments because they could not be exact arcs.
  pub flattened_curves: usize,
  /// How many of each source construct was converted, by construct name.
  pub sources: BTreeMap<String, usize>,
  /// Constructs that produced no geometry, in the order they were met.
  pub skipped: Vec<Skipped>,
  /// Things the caller should know that are not losses, in the order they were met.
  pub warnings: Vec<String>,
}

impl ImportReport {
  /// A report for an import that has not read anything yet.
  pub fn new(format: &'static str, tolerance: f64) -> Self {
    Self {
      format,
      tolerance,
      unit_scale: 1.0,
      contours: 0,
      flattened_curves: 0,
      sources: BTreeMap::new(),
      skipped: Vec::new(),
      warnings: Vec::new(),
    }
  }

  /// Record `count` converted constructs of `construct`.
  pub fn count(&mut self, construct: &str, count: usize) {
    if count == 0 {
      return;
    }
    *self.sources.entry(construct.to_string()).or_insert(0) += count;
  }

  /// Record `count` curves that had to be flattened.
  pub fn flatten(&mut self, count: usize) {
    self.flattened_curves += count;
  }

  /// Record a construct that produced no geometry.
  pub fn skip(
    &mut self,
    construct: impl Into<String>,
    reason: impl Into<String>,
    location: Option<String>,
  ) {
    self.skipped.push(Skipped {
      construct: construct.into(),
      reason: reason.into(),
      location,
    });
  }

  /// Record a warning.
  pub fn warn(&mut self, warning: impl Into<String>) {
    self.warnings.push(warning.into());
  }

  /// How many constructs were skipped.
  pub fn skipped_count(&self) -> usize {
    self.skipped.len()
  }

  /// Render the report as stable text.
  ///
  /// Determinism matters here: `cam-cli import` prints this, and a test asserts it whole.
  pub fn render(&self) -> String {
    let mut out = String::new();
    out.push_str(&format!("format: {}\n", self.format));
    out.push_str(&format!("tolerance: {:.3} mm\n", self.tolerance));
    out.push_str(&format!("unit scale: {:.6}\n", self.unit_scale));
    out.push_str(&format!("contours: {}\n", self.contours));
    out.push_str(&format!("flattened curves: {}\n", self.flattened_curves));
    out.push_str("sources:");
    if self.sources.is_empty() {
      out.push_str(" none");
    }
    for (construct, count) in &self.sources {
      out.push_str(&format!(" {construct}={count}"));
    }
    out.push('\n');
    out.push_str(&format!("skipped: {}\n", self.skipped.len()));
    for skipped in &self.skipped {
      out.push_str(&format!("  - {}", skipped.construct));
      if let Some(location) = &skipped.location {
        out.push_str(&format!(" at {location}"));
      }
      out.push_str(&format!(": {}\n", skipped.reason));
    }
    if !self.warnings.is_empty() {
      out.push_str("warnings:\n");
      for warning in &self.warnings {
        out.push_str(&format!("  - {warning}\n"));
      }
    }
    out
  }
}
