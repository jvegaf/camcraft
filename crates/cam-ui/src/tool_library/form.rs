use cam_store::tools::{CuttingData, StoredTool};
use cam_units::{Format, Length};

use super::family::ToolFamily;

/// One group of the detail form, in S9's own order and words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolFormGroup {
  /// The tool's shape: the family's own fields.
  Geometry,
  /// The numbers that describe a pass over the material.
  Passes,
  /// The numbers that describe how fast it moves.
  Speeds,
  /// The number the program names the tool by, and the units its rates are written in.
  Machine,
}

impl ToolFormGroup {
  /// Every group, top to bottom.
  pub const ALL: [ToolFormGroup; 4] = [
    ToolFormGroup::Geometry,
    ToolFormGroup::Passes,
    ToolFormGroup::Speeds,
    ToolFormGroup::Machine,
  ];

  /// The heading the form writes.
  pub fn label(self) -> &'static str {
    match self {
      Self::Geometry => "GEOMETRY",
      Self::Passes => "PASSES",
      Self::Speeds => "SPEEDS",
      Self::Machine => "MACHINE",
    }
  }
}

/// The groups the form draws for one tool, in order — the ones the record can fill.
///
/// `GEOMETRY` and `MACHINE` are always here; `PASSES` and `SPEEDS` are here only when the cutting data holds
/// at least one of their numbers, because a heading over nothing is a claim about a tool nobody measured.
pub fn form_groups(tool: &StoredTool) -> Vec<ToolFormGroup> {
  let cutting = &tool.cutting;
  let passes =
    cutting.stepdown.is_some() || cutting.stepover.is_some() || cutting.clear_stepover.is_some();
  let speeds =
    cutting.feed_rate.is_some() || cutting.plunge_rate.is_some() || cutting.spindle_speed.is_some();
  ToolFormGroup::ALL
    .iter()
    .copied()
    .filter(|group| match group {
      ToolFormGroup::Geometry | ToolFormGroup::Machine => true,
      ToolFormGroup::Passes => passes,
      ToolFormGroup::Speeds => speeds,
    })
    .collect()
}

/// The fields and values one group draws for one tool.
pub fn form_fields(group: ToolFormGroup, tool: &StoredTool) -> Vec<(String, String)> {
  let geometry = &tool.geometry;
  let cutting = &tool.cutting;
  let length = |value: Option<Length>| match value {
    Some(length) => format!("{} mm", Format::fixed(2).apply(length.mm())),
    None => "—".to_owned(),
  };
  // Stored rates are millimetres per second — `cam-store` documents that — but the library records the units
  // its author typed them in, and `MACHINE` names that unit in its `Rates` row. Printing the stored number under
  // `mm/s` beside a `Rates in/s` row gave one number two contradictory units, which is R3-001 in this task's
  // review: an inch-library tool read `Feed 100.00 mm/s` and `Rates in/s` at once. The number is converted to the
  // authored unit and labelled with it, so both rows say the same thing and it is the one the operator entered.
  let rate = |value: Option<f64>| match value {
    Some(rate) => {
      let shown = match cutting.rate_units {
        cam_units::UnitSystem::Mm => rate,
        cam_units::UnitSystem::Inch => rate / cam_units::MM_PER_INCH,
      };
      format!("{} {}", Format::fixed(2).apply(shown), units_word(cutting))
    }
    None => "—".to_owned(),
  };
  let rows: Vec<(&str, String)> = match group {
    ToolFormGroup::Geometry => ToolFamily::of(geometry)
      .fields()
      .iter()
      .map(|field| (field.label(), field.value(geometry)))
      .collect(),
    ToolFormGroup::Passes => vec![
      ("Stepdown", length(cutting.stepdown)),
      ("Stepover", length(cutting.stepover)),
      ("Clearing stepover", length(cutting.clear_stepover)),
    ],
    ToolFormGroup::Speeds => vec![
      ("Feed", rate(cutting.feed_rate)),
      ("Plunge", rate(cutting.plunge_rate)),
      (
        "Spindle",
        match cutting.spindle_speed {
          Some(speed) => format!("{speed} rpm"),
          None => "—".to_owned(),
        },
      ),
    ],
    ToolFormGroup::Machine => vec![
      (
        "Tool number",
        match cutting.tool_number {
          Some(number) => format!("T{number}"),
          None => "—".to_owned(),
        },
      ),
      ("Rates", units_word(cutting)),
    ],
  };
  rows
    .into_iter()
    .map(|(label, value)| (label.to_owned(), value))
    .collect()
}

/// The units word the `MACHINE` group writes for a tool's rates.
fn units_word(cutting: &CuttingData) -> String {
  match cutting.rate_units {
    cam_units::UnitSystem::Mm => "mm/s".to_owned(),
    cam_units::UnitSystem::Inch => "in/s".to_owned(),
  }
}
