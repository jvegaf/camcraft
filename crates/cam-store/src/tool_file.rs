//! The tool library as a file: reading one, and writing one.
//!
//! The library itself is SQLite ([`crate::tools`]), and deliberately so — the analysed product's
//! normalisation is what makes a tool feed differently in MDF and in hardwood. A *file* is a different
//! question: `Export…` has to hand a person something they can keep, diff and mail, and `Import…` has to
//! read one back. JSON, one tool per object, every length in millimetres with the unit in the field's own
//! name (`diameter_mm`), and every rate in millimetres per minute — which is the unit the store keeps too,
//! so a file and a library agree about a feed without a conversion in between.
//!
//! Two rules this format keeps, both of them about not guessing:
//!
//! 1. **The kind is a slug, not an integer and not a label.** The label is prose that may be reworded and
//!    the integer is one schema's storage detail; a file another build reads has to be neither. A slug this
//!    build does not know is a refusal that names it, never a silent fallback to a flat end mill.
//! 2. **Every tool in the file has to validate**, by the same rule the store applies, before any of them is
//!    used. A file is a claim about a library, and an import that installed half of it would be a library
//!    nobody wrote.

use serde::{Deserialize, Serialize};

use crate::tool_edit::{validate_tool, ToolProblem};
use crate::tools::{CuttingData, StoredTool, ToolGeometry, ToolKind};
use cam_units::{Angle, Length, UnitSystem};

/// The `format` field every file carries, so a wrong file is refused by name rather than by parse error.
pub const FORMAT: &str = "campilot-tool-library";

/// The version this build writes, and the only one it reads.
pub const VERSION: u32 = 1;

/// Everything that can stop a tool-library file from being read.
#[derive(Debug, thiserror::Error)]
pub enum ToolFileError {
  /// The bytes are not this format's JSON.
  #[error("{0}")]
  Json(#[from] serde_json::Error),
  /// The file declares a different format.
  #[error("this is not a tool library: its format is {found:?} and not {FORMAT:?}")]
  NotALibrary {
    /// What the file said it was.
    found: String,
  },
  /// The file declares a version this build cannot read.
  #[error("this tool library is version {found} and this build reads version {VERSION}")]
  UnsupportedVersion {
    /// What the file said it was.
    found: u32,
  },
  /// A tool names a kind this build does not know.
  #[error("the tool {tool:?} is a {kind:?}, which this build does not know")]
  UnknownKind {
    /// The tool's own name.
    tool: String,
    /// The slug the file used.
    kind: String,
  },
  /// A tool in the file does not describe a tool.
  #[error("the tool {tool:?} cannot be used: {problem}")]
  Invalid {
    /// The tool's own name.
    tool: String,
    /// What is wrong with it.
    problem: ToolProblem,
  },
}

/// A tool library as a file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolFile {
  /// [`FORMAT`], so a reader can tell one JSON shape from another.
  pub format: String,
  /// [`VERSION`].
  pub version: u32,
  /// The tools, in the library's own order.
  pub tools: Vec<ToolEntry>,
}

/// One tool in a library file.
///
/// Every number optional, because the library's numbers are optional: a tool with no flute length has no
/// flute length, and writing `0` would be a length nobody measured. The unit is in the field's name rather
/// than in a separate units block, so one field cannot be read in the wrong unit on its own.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolEntry {
  /// The library's own id for the tool.
  pub id: String,
  /// The tool's name, as the operator wrote it.
  pub name: String,
  /// The kind, as [`ToolKind::slug`].
  pub kind: String,
  /// Cutting diameter.
  pub diameter_mm: Option<f64>,
  /// A pointed tip's included angle.
  pub included_angle_deg: Option<f64>,
  /// The flat on the end of a cone.
  pub flat_diameter_mm: Option<f64>,
  /// How many cutting edges the tool has.
  pub flutes: Option<u32>,
  /// Usable flute length.
  pub flute_length_mm: Option<f64>,
  /// A bull-nose's corner radius.
  pub tip_radius_mm: Option<f64>,
  /// The shank's diameter.
  pub shank_diameter_mm: Option<f64>,
  /// The whole tool's length.
  pub overall_length_mm: Option<f64>,
  /// Feed rate along the cut.
  pub feed_mm_per_min: Option<f64>,
  /// Plunge rate.
  pub plunge_mm_per_min: Option<f64>,
  /// Spindle speed.
  pub spindle_rpm: Option<u32>,
  /// Depth of cut per pass.
  pub stepdown_mm: Option<f64>,
  /// Stepover between passes.
  pub stepover_mm: Option<f64>,
  /// The number a program names the tool by.
  pub tool_number: Option<u32>,
}

/// Write a library as this format's JSON.
pub fn to_json(tools: &[StoredTool]) -> Result<String, serde_json::Error> {
  let file = ToolFile {
    format: FORMAT.to_owned(),
    version: VERSION,
    tools: tools.iter().map(entry_of).collect(),
  };
  serde_json::to_string_pretty(&file)
}

/// Read a library from this format's JSON.
///
/// Every tool is validated before any is returned, so a caller that installs the result installs all of it
/// or none of it. The tools arrive with fresh cutting-data ids derived from their geometry ids, because a
/// file is not a database and an id copied between libraries would name two rows.
pub fn from_json(text: &str) -> Result<Vec<StoredTool>, ToolFileError> {
  let file: ToolFile = serde_json::from_str(text)?;
  if file.format != FORMAT {
    return Err(ToolFileError::NotALibrary { found: file.format });
  }
  if file.version != VERSION {
    return Err(ToolFileError::UnsupportedVersion {
      found: file.version,
    });
  }
  let mut tools = Vec::with_capacity(file.tools.len());
  for entry in &file.tools {
    let kind = ToolKind::from_slug(&entry.kind).ok_or_else(|| ToolFileError::UnknownKind {
      tool: entry.name.clone(),
      kind: entry.kind.clone(),
    })?;
    let tool = tool_of(entry, kind);
    validate_tool(&tool).map_err(|problem| ToolFileError::Invalid {
      tool: entry.name.clone(),
      problem,
    })?;
    tools.push(tool);
  }
  Ok(tools)
}

/// The file entry one stored tool becomes.
fn entry_of(tool: &StoredTool) -> ToolEntry {
  let geometry = &tool.geometry;
  let cutting = &tool.cutting;
  ToolEntry {
    id: geometry.id.clone(),
    name: crate::tool_edit::rendered_name(tool),
    kind: ToolKind::of(geometry).slug().to_owned(),
    diameter_mm: geometry.diameter.map(|value| value.mm()),
    included_angle_deg: geometry.included_angle.map(|value| value.degrees()),
    flat_diameter_mm: geometry.flat_diameter.map(|value| value.mm()),
    flutes: geometry.num_flutes,
    flute_length_mm: geometry.flute_length.map(|value| value.mm()),
    tip_radius_mm: geometry.tip_radius.map(|value| value.mm()),
    shank_diameter_mm: geometry.shank_diameter.map(|value| value.mm()),
    overall_length_mm: geometry.overall_length.map(|value| value.mm()),
    feed_mm_per_min: cutting.feed_rate,
    plunge_mm_per_min: cutting.plunge_rate,
    spindle_rpm: cutting.spindle_speed,
    stepdown_mm: cutting.stepdown.map(|value| value.mm()),
    stepover_mm: cutting.stepover.map(|value| value.mm()),
    tool_number: cutting.tool_number,
  }
}

/// The stored tool one file entry becomes.
fn tool_of(entry: &ToolEntry, kind: ToolKind) -> StoredTool {
  StoredTool {
    geometry: ToolGeometry {
      id: entry.id.clone(),
      name_format: entry.name.clone(),
      tool_type: kind.raw(),
      units: UnitSystem::Mm,
      diameter: entry.diameter_mm.map(Length::from_mm),
      included_angle: entry.included_angle_deg.map(Angle::from_degrees),
      flat_diameter: entry.flat_diameter_mm.map(Length::from_mm),
      num_flutes: entry.flutes,
      flute_length: entry.flute_length_mm.map(Length::from_mm),
      tip_radius: entry.tip_radius_mm.map(Length::from_mm),
      shank_diameter: entry.shank_diameter_mm.map(Length::from_mm),
      overall_length: entry.overall_length_mm.map(Length::from_mm),
      custom_attributes: None,
    },
    cutting: CuttingData {
      id: format!("{}-cut", entry.id),
      rate_units: UnitSystem::Mm,
      feed_rate: entry.feed_mm_per_min,
      plunge_rate: entry.plunge_mm_per_min,
      spindle_speed: entry.spindle_rpm,
      stepdown: entry.stepdown_mm.map(Length::from_mm),
      stepover: entry.stepover_mm.map(Length::from_mm),
      clear_stepover: None,
      tool_number: entry.tool_number,
      notes: None,
    },
  }
}
