use cam_store::tools::{CuttingData, StoredTool, ToolKind};
use cam_units::{Format, Length, UnitSystem, MM_PER_INCH};

use super::family::ShapeField;

/// One group of the detail form, in the reference screen's own order and words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolFormGroup {
  /// The tool's shape.
  Geometry,
  /// The numbers that describe a pass over the material.
  Passes,
  /// The numbers that describe how fast it moves.
  Speeds,
  /// The number the program names the tool by.
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

/// One editable row of the detail form.
///
/// Every group is now drawn whether or not the record fills it, and that reverses a rule this module used
/// to keep. The old rule was right for a *report*: a heading over nothing claimed a tool had been measured.
/// A box is not a claim — it is a question — and the reference asks all of them, which is how an operator
/// gives a tool the feeds it did not have.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormField {
  /// One row of the `GEOMETRY` group, decided by the tool's kind.
  Shape(ShapeField),
  /// Depth of cut per pass.
  Stepdown,
  /// The stepover between passes, in millimetres.
  Stepover,
  /// The same stepover as a percentage of the diameter — derived, and edited through the millimetres.
  StepoverPercent,
  /// Spindle speed, in rpm.
  Spindle,
  /// Feed rate along the cut, in the rate unit's per-minute form.
  Feed,
  /// Plunge rate.
  Plunge,
  /// How much one cutting edge takes per revolution — computed, never typed.
  Chipload,
  /// The number the program names the tool by.
  ToolNumber,
}

impl FormField {
  /// The box's label, spelled the way the reference screen spells it.
  pub fn label(self) -> &'static str {
    match self {
      Self::Shape(field) => field.label(),
      Self::Stepdown => "Depth per pass",
      Self::Stepover => "Stepover",
      Self::StepoverPercent => "Stepover",
      Self::Spindle => "Spindle",
      Self::Feed => "Feed",
      Self::Plunge => "Plunge",
      Self::Chipload => "Chipload",
      Self::ToolNumber => "Tool number",
    }
  }

  /// The unit the suffix shows outside the box, or `None` for a count.
  pub fn unit(self, units: UnitSystem) -> Option<&'static str> {
    match self {
      Self::Shape(field) => field.unit(),
      Self::Stepdown | Self::Stepover => Some("mm"),
      Self::StepoverPercent => Some("%"),
      Self::Spindle => Some("rpm"),
      Self::Feed | Self::Plunge => Some(match units {
        UnitSystem::Mm => "mm/min",
        UnitSystem::Inch => "in/min",
      }),
      Self::Chipload => Some("mm"),
      Self::ToolNumber => None,
    }
  }

  /// The group this row belongs to.
  pub fn group(self) -> ToolFormGroup {
    match self {
      Self::Shape(_) => ToolFormGroup::Geometry,
      Self::Stepdown | Self::Stepover | Self::StepoverPercent => ToolFormGroup::Passes,
      Self::Spindle | Self::Feed | Self::Plunge | Self::Chipload => ToolFormGroup::Speeds,
      Self::ToolNumber => ToolFormGroup::Machine,
    }
  }

  /// Whether the box can be typed into.
  ///
  /// Two rows cannot, and for two different reasons. `Chipload` is arithmetic on the feed, the speed and the
  /// cuts, so a box of its own would be a fourth number the other three contradict. `Stepover` as a
  /// percentage is the same measurement as the millimetres one row above it, and one measurement gets one
  /// box — the reference draws the percentage as a box of its own, and follows the millimetres one when
  /// either is typed into.
  pub fn editable(self) -> bool {
    !matches!(self, Self::Chipload | Self::StepoverPercent)
  }

  /// The text the box starts with: the record's number, or nothing at all.
  ///
  /// Nothing and never a zero: a tool with no flute length has no length, and seeding `0` would be a number
  /// nobody wrote. An empty box is the honest answer to "what does the library hold here?".
  pub fn value(self, tool: &StoredTool) -> String {
    let geometry = &tool.geometry;
    let cutting = &tool.cutting;
    let millimetres = |value: Option<Length>| value.map(decimal).unwrap_or_default();
    match self {
      Self::Shape(field) => field.value(geometry).unwrap_or_default(),
      Self::Stepdown => millimetres(cutting.stepdown),
      Self::Stepover => millimetres(cutting.stepover),
      Self::StepoverPercent => percent_of(cutting, geometry),
      Self::Spindle => cutting
        .spindle_speed
        .map(|speed| speed.to_string())
        .unwrap_or_default(),
      Self::Feed => cutting
        .feed_rate
        .map(|rate| decimal(Length::from_mm(shown(rate, cutting.rate_units))))
        .unwrap_or_default(),
      Self::Plunge => cutting
        .plunge_rate
        .map(|rate| decimal(Length::from_mm(shown(rate, cutting.rate_units))))
        .unwrap_or_default(),
      Self::Chipload => chipload(tool)
        .map(|value| decimal(Length::from_mm(value)))
        .unwrap_or_default(),
      Self::ToolNumber => cutting
        .tool_number
        .map(|number| number.to_string())
        .unwrap_or_default(),
    }
  }
}

/// The `PASSES` and `SPEEDS` rows for a tool, in the reference's order.
fn rows_after_geometry() -> Vec<FormField> {
  vec![
    FormField::Stepdown,
    FormField::Stepover,
    FormField::StepoverPercent,
    FormField::Spindle,
    FormField::Feed,
    FormField::Plunge,
    FormField::Chipload,
    FormField::ToolNumber,
  ]
}

/// Every row the form draws for one tool, grouped, in order.
///
/// `GEOMETRY` is the one group whose membership is not fixed: [`ShapeField::of`] decides it from the kind,
/// because a row for a number the shape does not have would describe a tool nobody has.
pub fn form_rows(tool: &StoredTool) -> Vec<(ToolFormGroup, Vec<FormField>)> {
  let kind = ToolKind::of(&tool.geometry);
  let mut geometry: Vec<FormField> = ShapeField::of(kind)
    .iter()
    .map(|field| FormField::Shape(*field))
    .collect();
  let mut groups = vec![(ToolFormGroup::Geometry, Vec::new())];
  groups[0].1.append(&mut geometry);
  for field in rows_after_geometry() {
    let group = field.group();
    match groups.iter_mut().find(|(name, _)| *name == group) {
      Some((_, rows)) => rows.push(field),
      None => groups.push((group, vec![field])),
    }
  }
  groups
}

/// The tool's chipload: how much one cutting edge takes per revolution, in millimetres.
///
/// `feed ÷ (spindle × cuts)`, on the numbers the record holds: millimetres per minute over revolutions per
/// minute is millimetres per revolution, and over the cuts it is millimetres per tooth. It is `None` rather than zero when any of the three is missing: a chipload computed from a
/// number nobody entered would be arithmetic over a guess, and this row is drawn in the warning colour
/// precisely because it is the one number on the form the operator does not type.
pub fn chipload(tool: &StoredTool) -> Option<f64> {
  let feed = tool.cutting.feed_rate?;
  let spindle = tool.cutting.spindle_speed?;
  let flutes = tool.geometry.num_flutes?;
  if spindle == 0 || flutes == 0 || !feed.is_finite() {
    return None;
  }
  Some(shown(feed, tool.cutting.rate_units) / (f64::from(spindle) * f64::from(flutes)))
}

/// A stepover as a percentage of the cutting diameter, as the form's third `PASSES` box shows it.
fn percent_of(cutting: &CuttingData, geometry: &cam_store::tools::ToolGeometry) -> String {
  let (Some(stepover), Some(diameter)) = (cutting.stepover, geometry.diameter) else {
    return String::new();
  };
  if diameter.mm() <= 0.0 {
    return String::new();
  }
  Format::fixed(0).apply(stepover.mm() / diameter.mm() * 100.0)
}

/// A stored rate as its box shows it.
///
/// The library keeps millimetres per minute and the form writes millimetres per minute, so there is no
/// per-time conversion here at all: the only thing `rate_units` changes is the unit the operator typed,
/// and a number quoted in inches is shown in inches. A conversion by sixty used to live here and it was
/// wrong twice over — it inflated every feed the form showed and every chipload it computed.
pub fn shown(millimetres_per_minute: f64, units: UnitSystem) -> f64 {
  match units {
    UnitSystem::Mm => millimetres_per_minute,
    UnitSystem::Inch => millimetres_per_minute / MM_PER_INCH,
  }
}

/// The stored form of a rate the operator typed into a box: the inverse of [`shown`].
pub fn stored(shown: f64, units: UnitSystem) -> f64 {
  match units {
    UnitSystem::Mm => shown,
    UnitSystem::Inch => shown * MM_PER_INCH,
  }
}

/// A millimetre value with at most three decimals and no trailing zeroes.
pub fn decimal(length: Length) -> String {
  let text = Format::fixed(3).apply(length.mm());
  let trimmed = text.trim_end_matches('0').trim_end_matches('.');
  if trimmed.is_empty() {
    "0".to_owned()
  } else {
    trimmed.to_owned()
  }
}
