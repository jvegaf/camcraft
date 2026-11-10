//! Writing a tool into a library: what a row has to be, and the writer that enforces it.
//!
//! `cam-store` accepted any row it was handed until this module existed: [`ToolLibrary::put_geometry`] is
//! an upsert and checked nothing. That was defensible while nothing in the app wrote tools — `cam-cli
//! tools add` was the only route and it filled every field itself. An editor changes the actuarial
//! position: the numbers come from a text box, so "a diameter of nothing" stops being a hypothetical.
//!
//! The rule is the one the modal already follows about `Use this tool`: a write that cannot mean anything
//! is refused, and the refusal names the first thing that is wrong, in the order the form reads
//! ([`ToolProblem`]). Validation lives here rather than in the UI for the reason every validation rule
//! does — a second caller must not be able to forget it — and [`ToolLibrary::save_tool`] runs it itself,
//! so no caller can skip it by not asking.
//!
//! [`ToolLibrary::put_geometry`]: crate::tools::ToolLibrary::put_geometry

use cam_units::{Angle, Length};

use crate::tools::{StoredTool, ToolKind, ToolLibrary};
use crate::StoreError;

/// Why a tool cannot be written into a library.
///
/// One variant per refusal, carrying the number that failed, because a sentence that says "invalid
/// diameter" beside a box showing `0` teaches nobody anything. [`ToolProblem::sentence`] is the only place
/// these become words, so the modal and the store cannot describe one failure two ways.
#[derive(Clone, Debug, PartialEq)]
pub enum ToolProblem {
  /// The record carries no cutting diameter, which is the one number every tool has.
  NoDiameter,
  /// The record's diameter is zero or negative.
  DiameterNotPositive(f64),
  /// The record carries no flute count.
  NoFlutes,
  /// The record's flute count is zero, so the tool cannot cut.
  FlutesNotPositive,
  /// The record's shank diameter is zero or negative.
  ShankNotPositive(f64),
  /// An included angle of zero or one at least half a turn describes no cone.
  AngleOutOfRange(f64),
  /// The overall length is shorter than the flute length it has to contain, in millimetres.
  OverallShorterThanFlute {
    /// The whole tool's length.
    overall: f64,
    /// The flute length it was asked to contain.
    flute: f64,
  },
  /// The stepdown is zero or negative, or not a number at all.
  StepdownNotPositive(f64),
  /// The stepover is zero or negative.
  StepoverNotPositive(f64),
  /// The stepover is wider than the tool, so a pass would leave uncut material between passes.
  StepoverWiderThanDiameter {
    /// The stepover the record asks for.
    stepover: f64,
    /// The cutting diameter it has to fit in.
    diameter: f64,
  },
  /// A feed rate is negative.
  FeedNegative(f64),
  /// The plunge rate is negative.
  PlungeNegative(f64),
  /// The spindle speed is zero, so the tool does not turn.
  SpindleNotPositive,
  /// The record carries no tool number, so no program can name it.
  NoToolNumber,
  /// The tool number is zero, which no program uses.
  ToolNumberNotPositive,
}

impl ToolProblem {
  /// The sentence the operator reads, naming the box and the number.
  pub fn sentence(&self) -> String {
    match self {
      ToolProblem::NoDiameter => "A tool needs a diameter before it can be saved.".to_owned(),
      ToolProblem::DiameterNotPositive(diameter) => {
        format!("A diameter has to be greater than zero, and this one is {diameter} mm.")
      }
      ToolProblem::NoFlutes => "A tool needs a number of cuts before it can be saved.".to_owned(),
      ToolProblem::FlutesNotPositive => {
        "A tool needs at least one cut, because a tool with none cannot cut.".to_owned()
      }
      ToolProblem::ShankNotPositive(shank) => {
        format!("A shank diameter has to be greater than zero, and this one is {shank} mm.")
      }
      ToolProblem::AngleOutOfRange(angle) => format!(
        "An included angle has to be greater than 0° and less than 180°, and this one is {angle}°."
      ),
      ToolProblem::OverallShorterThanFlute { overall, flute } => format!(
        "The overall length ({overall} mm) cannot be shorter than the flute length ({flute} mm) it contains."
      ),
      ToolProblem::StepdownNotPositive(stepdown) => {
        format!("A depth per pass has to be greater than zero, and this one is {stepdown} mm.")
      }
      ToolProblem::StepoverNotPositive(stepover) => {
        format!("A stepover has to be greater than zero, and this one is {stepover} mm.")
      }
      ToolProblem::StepoverWiderThanDiameter {
        stepover,
        diameter,
      } => format!(
        "A stepover ({stepover} mm) cannot be wider than the tool ({diameter} mm), or a pass would leave \
         material between passes."
      ),
      ToolProblem::FeedNegative(feed) => {
        format!("A feed rate cannot be negative, and this one is {feed}.")
      }
      ToolProblem::PlungeNegative(plunge) => {
        format!("A plunge rate cannot be negative, and this one is {plunge}.")
      }
      ToolProblem::SpindleNotPositive => {
        "A spindle speed has to be at least 1 rpm, because a tool that does not turn does not cut."
          .to_owned()
      }
      ToolProblem::NoToolNumber => {
        "A tool needs a number before it can be saved, so a program can name it.".to_owned()
      }
      ToolProblem::ToolNumberNotPositive => {
        "A tool number has to be at least 1, because no program uses T0.".to_owned()
      }
    }
  }
}

impl std::fmt::Display for ToolProblem {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.write_str(&self.sentence())
  }
}

impl std::error::Error for ToolProblem {}

/// Everything that can stop a tool from being written.
#[derive(Debug, thiserror::Error)]
pub enum WriteToolError {
  /// The row itself does not describe a tool.
  #[error("{0}")]
  Invalid(#[from] ToolProblem),
  /// The library refused the write.
  #[error("{0}")]
  Store(#[from] StoreError),
}

/// Whether a tool describes something a library may hold.
///
/// The order is the form's: the shape's own numbers first, then the passes they bound, then the rates,
/// then the number the program names it by. The first failure is the answer, because a form that listed
/// five things at once would be a form nobody finishes.
pub fn validate_tool(tool: &StoredTool) -> Result<(), ToolProblem> {
  let geometry = &tool.geometry;
  let cutting = &tool.cutting;

  let diameter = match geometry.diameter {
    Some(diameter) => diameter.mm(),
    None => return Err(ToolProblem::NoDiameter),
  };
  if !positive(diameter) {
    return Err(ToolProblem::DiameterNotPositive(diameter));
  }
  let flutes = match geometry.num_flutes {
    Some(flutes) => flutes,
    None => return Err(ToolProblem::NoFlutes),
  };
  if flutes == 0 {
    return Err(ToolProblem::FlutesNotPositive);
  }
  if let Some(shank) = geometry.shank_diameter {
    if !positive(shank.mm()) {
      return Err(ToolProblem::ShankNotPositive(shank.mm()));
    }
  }
  // A drill and a V both have one; a flat end mill has none, and `None` is not an error — it is a shape
  // that carries no angle.
  if let Some(angle) = geometry.included_angle {
    let degrees = angle.degrees();
    if !(degrees > 0.0 && degrees < 180.0) {
      return Err(ToolProblem::AngleOutOfRange(degrees));
    }
  }
  if let (Some(overall), Some(flute)) = (geometry.overall_length, geometry.flute_length) {
    if overall.mm() < flute.mm() {
      return Err(ToolProblem::OverallShorterThanFlute {
        overall: overall.mm(),
        flute: flute.mm(),
      });
    }
  }
  if let Some(stepdown) = cutting.stepdown {
    // The same door the stepover gets, and it was missing: a negative, zero, NaN or infinite depth of cut
    // is not a pass, and `save_tool` and `from_json` both relied on this gate to keep one out of a library
    // (review R3-stepdown-unvalidated).
    if !positive(stepdown.mm()) {
      return Err(ToolProblem::StepdownNotPositive(stepdown.mm()));
    }
  }
  if let Some(stepover) = cutting.stepover {
    let stepover = stepover.mm();
    if !positive(stepover) {
      return Err(ToolProblem::StepoverNotPositive(stepover));
    }
    if stepover > diameter {
      return Err(ToolProblem::StepoverWiderThanDiameter { stepover, diameter });
    }
  }
  if let Some(feed) = cutting.feed_rate {
    if !(feed.is_finite() && feed >= 0.0) {
      return Err(ToolProblem::FeedNegative(feed));
    }
  }
  if let Some(plunge) = cutting.plunge_rate {
    if !(plunge.is_finite() && plunge >= 0.0) {
      return Err(ToolProblem::PlungeNegative(plunge));
    }
  }
  if let Some(speed) = cutting.spindle_speed {
    if speed == 0 {
      return Err(ToolProblem::SpindleNotPositive);
    }
  }
  match cutting.tool_number {
    None => return Err(ToolProblem::NoToolNumber),
    Some(0) => return Err(ToolProblem::ToolNumberNotPositive),
    Some(_) => {}
  }
  Ok(())
}

/// Whether a length is a number that describes a real distance.
fn positive(millimetres: f64) -> bool {
  millimetres.is_finite() && millimetres > 0.0
}

impl ToolLibrary {
  /// Validate and write one tool, all three of its rows at once.
  ///
  /// The geometry, the cutting data and the link are one transaction because a tool is the three of them:
  /// a geometry written without its cutting data is a tool with no feeds, and an interrupted write that
  /// left that behind would be a row the tree lists and the form cannot describe.
  ///
  /// The link id is derived from the geometry's, so writing the same tool twice updates it instead of
  /// leaving a second link behind that would list the tool twice.
  pub fn save_tool(&self, tool: &StoredTool) -> Result<(), WriteToolError> {
    validate_tool(tool)?;
    let transaction = self.begin("save a tool")?;
    write_tool(&transaction, tool).map_err(|source| WriteToolError::Store(self.failure(source)))?;
    transaction
      .commit()
      .map_err(|source| WriteToolError::Store(self.failure(source)))?;
    Ok(())
  }

  /// Remove one tool by its geometry id, and answer whether anything was there.
  ///
  /// Everything that names the tool goes: the link, the cutting data it pointed at, and the geometry. A
  /// delete that left the cutting data behind would grow the library with rows nothing lists, which is the
  /// kind of debris that only shows up as a mystery in `tools.sqlite` a year later.
  pub fn remove_tool(&self, geometry_id: &str) -> Result<bool, StoreError> {
    let transaction = self.begin("remove a tool")?;
    let mut cutting_ids: Vec<String> = Vec::new();
    {
      let mut statement = transaction
        .prepare("SELECT tool_cutting_data_id FROM tool_entity WHERE tool_geometry_id = ?1")
        .map_err(|source| self.failure(source))?;
      let rows = statement
        .query_map(rusqlite::params![geometry_id], |row| {
          row.get::<_, String>(0)
        })
        .map_err(|source| self.failure(source))?;
      for row in rows {
        cutting_ids.push(row.map_err(|source| self.failure(source))?);
      }
    }
    let mut removed = false;
    for statement in [
      "DELETE FROM tool_entity WHERE tool_geometry_id = ?1",
      "DELETE FROM tool_geometry WHERE id = ?1",
    ] {
      let touched = transaction
        .execute(statement, rusqlite::params![geometry_id])
        .map_err(|source| self.failure(source))?;
      removed = removed || touched > 0;
    }
    for cutting_id in &cutting_ids {
      transaction
        .execute(
          "DELETE FROM tool_cutting_data WHERE id = ?1",
          rusqlite::params![cutting_id],
        )
        .map_err(|source| self.failure(source))?;
    }
    transaction
      .commit()
      .map_err(|source| self.failure(source))?;
    Ok(removed)
  }

  /// Replace the whole library with `tools`, all or nothing.
  ///
  /// This is what `Restore defaults` and an import are: not "add these", but "this is the library now".
  /// Every tool is validated **before** anything is deleted, so a set with one bad tool changes nothing —
  /// the alternative is a library emptied by a typo.
  pub fn replace_tools(&self, tools: &[StoredTool]) -> Result<(), WriteToolError> {
    for tool in tools {
      validate_tool(tool)?;
    }
    let transaction = self.begin("replace the library")?;
    for statement in [
      "DELETE FROM tool_entity",
      "DELETE FROM tool_cutting_data",
      "DELETE FROM tool_geometry",
    ] {
      transaction
        .execute(statement, [])
        .map_err(|source| WriteToolError::Store(self.failure(source)))?;
    }
    for tool in tools {
      write_tool(&transaction, tool)
        .map_err(|source| WriteToolError::Store(self.failure(source)))?;
    }
    transaction
      .commit()
      .map_err(|source| WriteToolError::Store(self.failure(source)))?;
    Ok(())
  }
}

/// The three rows one tool is, written into an open transaction.
fn write_tool(
  transaction: &rusqlite::Transaction<'_>,
  tool: &StoredTool,
) -> Result<(), rusqlite::Error> {
  let geometry = &tool.geometry;
  let cutting = &tool.cutting;
  transaction.execute(
    "INSERT INTO tool_geometry
         (id, name_format, notes, tool_type, units, diameter, included_angle, flat_diameter,
          num_flutes, flute_length, tip_radius, shank_diameter, overall_length, custom_attributes)
       VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
       ON CONFLICT(id) DO UPDATE SET
         name_format = excluded.name_format, tool_type = excluded.tool_type,
         units = excluded.units, diameter = excluded.diameter,
         included_angle = excluded.included_angle, flat_diameter = excluded.flat_diameter,
         num_flutes = excluded.num_flutes, flute_length = excluded.flute_length,
         tip_radius = excluded.tip_radius, shank_diameter = excluded.shank_diameter,
         overall_length = excluded.overall_length, custom_attributes = excluded.custom_attributes",
    rusqlite::params![
      geometry.id,
      geometry.name_format,
      geometry.tool_type,
      crate::tools::encode_units(geometry.units),
      geometry.diameter.map(Length::mm),
      geometry.included_angle.map(Angle::degrees),
      geometry.flat_diameter.map(Length::mm),
      geometry.num_flutes,
      geometry.flute_length.map(Length::mm),
      geometry.tip_radius.map(Length::mm),
      geometry.shank_diameter.map(Length::mm),
      geometry.overall_length.map(Length::mm),
      geometry.custom_attributes,
    ],
  )?;
  transaction.execute(
    "INSERT INTO tool_cutting_data
         (id, rate_units, feed_rate, plunge_rate, spindle_speed, spindle_dir, stepdown,
          stepover, clear_stepover, tool_number, notes)
       VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?7, ?8, ?9, ?10)
       ON CONFLICT(id) DO UPDATE SET
         rate_units = excluded.rate_units, feed_rate = excluded.feed_rate,
         plunge_rate = excluded.plunge_rate, spindle_speed = excluded.spindle_speed,
         stepdown = excluded.stepdown, stepover = excluded.stepover,
         clear_stepover = excluded.clear_stepover, tool_number = excluded.tool_number,
         notes = excluded.notes",
    rusqlite::params![
      cutting.id,
      crate::tools::encode_units(cutting.rate_units),
      cutting.feed_rate,
      cutting.plunge_rate,
      cutting.spindle_speed,
      cutting.stepdown.map(Length::mm),
      cutting.stepover.map(Length::mm),
      cutting.clear_stepover.map(Length::mm),
      cutting.tool_number,
      cutting.notes,
    ],
  )?;
  transaction.execute(
    // The link is upserted by the id this writer derived, and the conflict arm deliberately does **not**
    // touch `material_id` or `machine_id`. A library keeps one row per (tool, material, machine) because the
    // same cutter feeds differently in MDF and in hardwood, and `StoredTool` carries neither: an arm that
    // wrote `excluded.material_id` would set both to NULL and silently discard an association this API
    // cannot even express, while reporting the write as successful (review R4-001).
    "INSERT INTO tool_entity
         (id, material_id, machine_id, tool_geometry_id, tool_cutting_data_id)
       VALUES (?1, NULL, NULL, ?2, ?3)
       ON CONFLICT(id) DO UPDATE SET
         tool_geometry_id = excluded.tool_geometry_id,
         tool_cutting_data_id = excluded.tool_cutting_data_id",
    rusqlite::params![link_id(&geometry.id), geometry.id, cutting.id,],
  )?;
  Ok(())
}

/// The link row's id for a tool: derived, so two writes of one tool keep one row.
pub fn link_id(geometry_id: &str) -> String {
  format!("{geometry_id}-link")
}

/// The tool a fresh `New` starts from: a 6 mm two-flute flat end mill with no cutting data yet.
///
/// A starting point and not a recommendation: every number here is one the operator is expected to change,
/// which is why it carries no feeds — a feed the library invented would be a number nobody measured, the
/// rule the form already follows about a dash.
pub fn new_tool(id: &str, name: &str, tool_number: u32) -> StoredTool {
  StoredTool {
    geometry: cam_store_geometry(id, name, ToolKind::FlatEndMill, Some(Length::from_mm(6.0))),
    cutting: crate::tools::CuttingData {
      id: format!("{id}-cut"),
      rate_units: cam_units::UnitSystem::Mm,
      feed_rate: None,
      plunge_rate: None,
      spindle_speed: None,
      stepdown: None,
      stepover: None,
      clear_stepover: None,
      tool_number: Some(tool_number),
      notes: None,
    },
  }
}

fn round_mm_2(value: f64) -> f64 {
  (value * 100.0).round() / 100.0
}

fn derived_stepdown_mm(diameter_mm: f64) -> f64 {
  round_mm_2((diameter_mm / 2.0).min(3.0))
}

fn derived_stepover_mm(diameter_mm: f64) -> f64 {
  round_mm_2(diameter_mm * 0.4)
}

/// The factory set `Restore defaults` writes, in tool-number order.
///
/// The reference's 22 tools in its six families, with speeds and feeds derived by one rule.
pub fn factory_tools() -> Vec<StoredTool> {
  let mut tools = vec![
    factory(
      "factory-flat-6",
      "Flat end mill · 6 mm",
      ToolKind::FlatEndMill,
      1,
      |geometry| {
        geometry.diameter = Some(Length::from_mm(6.0));
        geometry.num_flutes = Some(2);
      },
    ),
    factory(
      "factory-flat-3-175",
      "Flat end mill · 3.175 mm (1/8\")",
      ToolKind::FlatEndMill,
      2,
      |geometry| {
        geometry.diameter = Some(Length::from_mm(3.175));
        geometry.num_flutes = Some(2);
      },
    ),
    factory(
      "factory-ball-6",
      "Ball nose · 6 mm",
      ToolKind::BallNose,
      3,
      |geometry| {
        geometry.diameter = Some(Length::from_mm(6.0));
        geometry.tip_radius = Some(Length::from_mm(3.0));
        geometry.num_flutes = Some(2);
      },
    ),
    factory(
      "factory-vbit-90-12-7",
      "V-bit · 90° · 12.7 mm",
      ToolKind::VBit,
      4,
      |geometry| {
        geometry.diameter = Some(Length::from_mm(12.7));
        geometry.included_angle = Some(Angle::from_degrees(90.0));
        geometry.flat_diameter = Some(Length::from_mm(0.2));
        geometry.num_flutes = Some(2);
      },
    ),
    factory(
      "factory-vbit-60-12-7",
      "V-bit · 60° · 12.7 mm",
      ToolKind::VBit,
      5,
      |geometry| {
        geometry.diameter = Some(Length::from_mm(12.7));
        geometry.included_angle = Some(Angle::from_degrees(60.0));
        geometry.flat_diameter = Some(Length::from_mm(0.2));
        geometry.num_flutes = Some(2);
      },
    ),
    factory(
      "factory-drill-5",
      "Drill · 5 mm",
      ToolKind::Drill,
      6,
      |geometry| {
        geometry.diameter = Some(Length::from_mm(5.0));
        geometry.included_angle = Some(Angle::from_degrees(118.0));
        geometry.num_flutes = Some(2);
        geometry.shank_diameter = Some(Length::from_mm(5.0));
        geometry.flute_length = Some(Length::from_mm(30.0));
        geometry.overall_length = Some(Length::from_mm(60.0));
      },
    ),
    factory(
      "factory-flat-8",
      "Flat end mill · 8 mm",
      ToolKind::FlatEndMill,
      7,
      |geometry| {
        geometry.diameter = Some(Length::from_mm(8.0));
        geometry.num_flutes = Some(2);
      },
    ),
    factory(
      "factory-flat-12-roughing",
      "Flat end mill · 12 mm (roughing)",
      ToolKind::FlatEndMill,
      8,
      |geometry| {
        geometry.diameter = Some(Length::from_mm(12.0));
        geometry.num_flutes = Some(2);
      },
    ),
    factory(
      "factory-ball-3-175",
      "Ball nose · 3.175 mm",
      ToolKind::BallNose,
      9,
      |geometry| {
        geometry.diameter = Some(Length::from_mm(3.175));
        geometry.tip_radius = Some(Length::from_mm(1.5875));
        geometry.num_flutes = Some(2);
      },
    ),
    factory(
      "factory-ball-1-5-detail",
      "Ball nose · 1.5 mm (detail)",
      ToolKind::BallNose,
      10,
      |geometry| {
        geometry.diameter = Some(Length::from_mm(1.5));
        geometry.tip_radius = Some(Length::from_mm(0.75));
        geometry.num_flutes = Some(2);
      },
    ),
    factory(
      "factory-vbit-30-3-175",
      "V-bit · 30° · 3.175 mm",
      ToolKind::VBit,
      11,
      |geometry| {
        geometry.diameter = Some(Length::from_mm(3.175));
        geometry.included_angle = Some(Angle::from_degrees(30.0));
        geometry.flat_diameter = Some(Length::from_mm(0.1));
        geometry.num_flutes = Some(1);
        geometry.flute_length = Some(Length::from_mm(7.7));
        geometry.overall_length = Some(Length::from_mm(38.0));
      },
    ),
    factory(
      "factory-drill-8",
      "Drill · 8 mm",
      ToolKind::Drill,
      12,
      |geometry| {
        geometry.diameter = Some(Length::from_mm(8.0));
        geometry.included_angle = Some(Angle::from_degrees(118.0));
        geometry.num_flutes = Some(2);
      },
    ),
    factory(
      "factory-flat-3-175-single",
      "Single flute · 3.175 mm",
      ToolKind::FlatEndMill,
      13,
      |geometry| {
        geometry.diameter = Some(Length::from_mm(3.175));
        geometry.num_flutes = Some(1);
      },
    ),
    factory(
      "factory-flat-4-single",
      "Single flute · 4 mm",
      ToolKind::FlatEndMill,
      14,
      |geometry| {
        geometry.diameter = Some(Length::from_mm(4.0));
        geometry.num_flutes = Some(1);
      },
    ),
    factory(
      "factory-flat-6-compression",
      "Compression · 6 mm",
      ToolKind::FlatEndMill,
      15,
      |geometry| {
        geometry.diameter = Some(Length::from_mm(6.0));
        geometry.num_flutes = Some(2);
      },
    ),
    factory(
      "factory-flat-25-surfacing",
      "Surfacing · 25 mm",
      ToolKind::FlatEndMill,
      16,
      |geometry| {
        geometry.diameter = Some(Length::from_mm(25.0));
        geometry.num_flutes = Some(2);
      },
    ),
    factory(
      "factory-engraving-30-0-3",
      "Engraving · 30° · 0.3 mm tip",
      ToolKind::Engraving,
      17,
      |geometry| {
        geometry.diameter = Some(Length::from_mm(3.175));
        geometry.included_angle = Some(Angle::from_degrees(30.0));
        geometry.flat_diameter = Some(Length::from_mm(0.3));
        geometry.num_flutes = Some(1);
      },
    ),
    factory(
      "factory-vbit-120-32",
      "V-bit · 120° · 32 mm",
      ToolKind::VBit,
      18,
      |geometry| {
        geometry.diameter = Some(Length::from_mm(32.0));
        geometry.included_angle = Some(Angle::from_degrees(120.0));
        geometry.flat_diameter = Some(Length::from_mm(0.2));
        geometry.num_flutes = Some(2);
      },
    ),
    factory(
      "factory-tapered-ball-r0-25",
      "Tapered ball · R0.25 · 3.6° per side",
      ToolKind::CornerRadius,
      19,
      |geometry| {
        geometry.diameter = Some(Length::from_mm(3.175));
        geometry.tip_radius = Some(Length::from_mm(0.25));
        geometry.num_flutes = Some(2);
      },
    ),
    factory(
      "factory-tapered-ball-r0-5",
      "Tapered ball · R0.5 · 4.76° per side",
      ToolKind::CornerRadius,
      20,
      |geometry| {
        geometry.diameter = Some(Length::from_mm(6.0));
        geometry.tip_radius = Some(Length::from_mm(0.5));
        geometry.num_flutes = Some(2);
      },
    ),
    factory(
      "factory-tapered-ball-r0-75",
      "Tapered ball · R0.75 · 6.42° per side",
      ToolKind::CornerRadius,
      21,
      |geometry| {
        geometry.diameter = Some(Length::from_mm(6.0));
        geometry.tip_radius = Some(Length::from_mm(0.75));
        geometry.num_flutes = Some(2);
      },
    ),
    factory(
      "factory-flat-1-3-175-shank",
      "Flat end mill · 1 mm · 3.175 shank",
      ToolKind::FlatEndMill,
      22,
      |geometry| {
        geometry.diameter = Some(Length::from_mm(1.0));
        geometry.shank_diameter = Some(Length::from_mm(3.175));
        geometry.num_flutes = Some(1);
      },
    ),
  ];
  tools.sort_by_key(|tool| tool.cutting.tool_number);
  tools
}

/// One factory tool: the shape, the kind it declares and the number it answers to.
fn factory(
  id: &str,
  name: &str,
  kind: ToolKind,
  number: u32,
  shape: impl FnOnce(&mut crate::tools::ToolGeometry),
) -> StoredTool {
  let mut tool = new_tool(id, name, number);
  shape(&mut tool.geometry);
  tool.geometry.tool_type = kind.raw();
  tool.cutting.feed_rate = Some(100.0);
  tool.cutting.plunge_rate = Some(30.0);
  tool.cutting.spindle_speed = Some(18_000);
  if let Some(diameter) = tool.geometry.diameter {
    let diameter_mm = diameter.mm();
    tool.cutting.stepdown = Some(Length::from_mm(derived_stepdown_mm(diameter_mm)));
    tool.cutting.stepover = Some(Length::from_mm(derived_stepover_mm(diameter_mm)));
  }
  tool
}

/// A geometry with the fields every tool shares, and only `name`, `kind` and `diameter` decided.
fn cam_store_geometry(
  id: &str,
  name: &str,
  kind: ToolKind,
  diameter: Option<Length>,
) -> crate::tools::ToolGeometry {
  crate::tools::ToolGeometry {
    id: id.to_owned(),
    name_format: name.to_owned(),
    tool_type: kind.raw(),
    units: cam_units::UnitSystem::Mm,
    diameter,
    included_angle: None,
    flat_diameter: None,
    num_flutes: Some(2),
    flute_length: None,
    tip_radius: None,
    shank_diameter: None,
    overall_length: None,
    custom_attributes: None,
  }
}

/// The next tool number nothing in `tools` uses, counting from 1.
///
/// The lowest gap rather than the highest plus one: a library that lost T2 to a delete should hand the next
/// `New` T2 back, because the numbers are how a person finds a tool in a carousel and a library that only
/// ever counts up ends up with `T47` in a rack of six.
pub fn next_tool_number(tools: &[StoredTool]) -> u32 {
  let taken: Vec<u32> = tools
    .iter()
    .filter_map(|tool| tool.cutting.tool_number)
    .collect();
  let mut candidate = 1;
  while taken.contains(&candidate) {
    candidate += 1;
  }
  candidate
}

/// Copy a tool as a new one: a fresh geometry id and cutting id, the same shape, and a name that says so.
///
/// `Copy` rather than the same id twice, because the library lists tools by geometry id: duplicating one
/// in place would be editing it, and the reference keeps both — that is what the button is for.
pub fn duplicate_tool(tool: &StoredTool, id: &str, tool_number: u32) -> StoredTool {
  let mut copy = tool.clone();
  copy.geometry.id = id.to_owned();
  copy.geometry.name_format = format!("{} (copy)", rendered_name(tool));
  copy.cutting.id = format!("{id}-cut");
  copy.cutting.tool_number = Some(tool_number);
  copy
}

/// A tool's display name: its format rendered when it is a template, verbatim when it is not.
///
/// The editor writes a plain string — the reference's `Name` box holds a name, not a template — and the
/// library has one column for both, so a string with no `{` in it is its own name. That is also why writing
/// over a templated name is not lossy in a way anyone notices: [`render_name`] returns a fieldless template
/// unchanged, so both shapes are the same kind of value.
pub fn rendered_name(tool: &StoredTool) -> String {
  crate::tools::render_name(
    &tool.geometry.name_format,
    ToolKind::of(&tool.geometry).label(),
    &tool.geometry,
  )
  .unwrap_or_else(|_| tool.geometry.name_format.clone())
}
