//! The modal's editing draft: the boxes' text, and the tool it turns back into.
//!
//! A draft rather than an immediate write, for the reason the modal already gave about its selection: every
//! keystroke is a decision about a tool, and only `Use this tool` makes it one about the library. Typing
//! `6.` on the way to `6.35` passes through a state that parses to nothing, and a form that wrote on every
//! keystroke would refuse a tool mid-word.
//!
//! The numbers come back out of the boxes through [`ToolDraft::commit`], which builds a [`StoredTool`] and
//! hands it to [`cam_store::tool_edit::validate_tool`]. That is the *same* rule the library enforces, so the
//! sentence a disabled button shows and the sentence the store would refuse with cannot drift apart.
//!
//! The kind is authoritative over the fields it implies. Choosing `Ball nose` sets the tip radius to half
//! the diameter, `V-bit` fills in a 90° angle if the box is empty, and a flat end mill drops both — because
//! a tool whose stored kind says one shape and whose stored radius says another is a record two readers
//! would describe differently.

use cam_store::tool_edit::{validate_tool, ToolProblem};
use cam_store::tools::{CuttingData, StoredTool, ToolGeometry, ToolKind};
use cam_units::{Angle, Length};

use super::family::ShapeField;
use super::form::{stored, FormField};

/// Why the draft cannot be turned into a tool yet.
#[derive(Clone, Debug, PartialEq)]
pub enum DraftProblem {
  /// A box holds text that is not a number.
  NotANumber {
    /// The row's label, so the sentence can name the box.
    label: &'static str,
    /// What the box actually holds.
    text: String,
  },
  /// The numbers parse, and together they do not describe a tool.
  Invalid(ToolProblem),
}

impl DraftProblem {
  /// The sentence the footer shows instead of the commit button.
  pub fn sentence(&self) -> String {
    match self {
      DraftProblem::NotANumber { label, text } => {
        format!("{label} has to be a number, and \"{text}\" is not one.")
      }
      DraftProblem::Invalid(problem) => problem.sentence(),
    }
  }
}

/// One tool being edited, and the text of every box the form drew for it.
#[derive(Clone, Debug)]
pub struct ToolDraft {
  /// The geometry id this draft edits: what tells a draft for one row from a draft for another.
  pub geometry_id: String,
  /// The name box's text.
  pub name: String,
  /// The kind the combo shows.
  pub kind: ToolKind,
  /// The text of each editable box, keyed by the row it belongs to.
  texts: Vec<(FormField, String)>,
  /// The record the draft started from, which is where everything the form does not draw comes from.
  original: StoredTool,
}

impl ToolDraft {
  /// Seed a draft from the record the tree selected.
  pub fn of(tool: &StoredTool) -> Self {
    let kind = ToolKind::of(&tool.geometry);
    let mut draft = ToolDraft {
      geometry_id: tool.geometry.id.clone(),
      name: cam_store::tool_edit::rendered_name(tool),
      kind,
      texts: Vec::new(),
      original: tool.clone(),
    };
    draft.reseed();
    draft
  }

  /// The text one box holds.
  pub fn text(&self, field: FormField) -> &str {
    self
      .texts
      .iter()
      .find(|(known, _)| *known == field)
      .map(|(_, text)| text.as_str())
      .unwrap_or("")
  }

  /// Replace one box's text.
  pub fn set_text(&mut self, field: FormField, text: String) {
    match self.texts.iter_mut().find(|(known, _)| *known == field) {
      Some((_, held)) => *held = text,
      None => self.texts.push((field, text)),
    }
  }

  /// Choose a new kind, and reshape the boxes the new kind draws.
  ///
  /// The numbers that survive the change are kept: switching a 6 mm flat end mill to a ball nose must not
  /// lose the 6, only gain the radius half of it implies.
  pub fn set_kind(&mut self, kind: ToolKind) {
    if kind == self.kind {
      return;
    }
    self.kind = kind;
    self.reseed();
  }

  /// Re-read every shape box from the record, keeping the ones the new kind still draws.
  fn reseed(&mut self) {
    let mut shaped = self.shaped_geometry();
    self.shape_geometry(&mut shaped);
    // A box the old kind drew and the new one does not is a number about a shape that is gone.
    let kept: Vec<ShapeField> = ShapeField::of(self.kind).to_vec();
    self.texts.retain(|(field, _)| match field {
      FormField::Shape(shape) => kept.contains(shape),
      _ => true,
    });
    for field in ShapeField::of(self.kind) {
      let field = FormField::Shape(*field);
      let text = field.value(&self.snapshot(&shaped));
      match self.texts.iter_mut().find(|(known, _)| *known == field) {
        Some((_, held)) => *held = text,
        None => self.texts.push((field, text)),
      }
    }
    // The rows the kind does not decide are seeded once, from the record, and left alone afterwards: a seed
    // that ran on every call would undo an edit the moment the operator picked a different kind.
    for field in [
      FormField::Stepdown,
      FormField::Stepover,
      FormField::Spindle,
      FormField::Feed,
      FormField::Plunge,
      FormField::ToolNumber,
    ] {
      if !self.texts.iter().any(|(known, _)| *known == field) {
        let text = field.value(&self.original);
        self.set_text(field, text);
      }
    }
  }

  /// The geometry the boxes currently describe, before the kind has had its say.
  fn shaped_geometry(&self) -> ToolGeometry {
    let mut geometry = self.original.geometry.clone();
    geometry.tool_type = self.kind.raw();
    for field in ShapeField::of(self.kind) {
      let text = self.text(FormField::Shape(*field));
      let Ok(number) = text.trim().parse::<f64>() else {
        continue;
      };
      match field {
        ShapeField::Diameter => geometry.diameter = Some(Length::from_mm(number)),
        ShapeField::IncludedAngle => geometry.included_angle = Some(Angle::from_degrees(number)),
        ShapeField::CornerRadius => geometry.tip_radius = Some(Length::from_mm(number)),
        ShapeField::FluteLength => geometry.flute_length = Some(Length::from_mm(number)),
        ShapeField::ShankDiameter => geometry.shank_diameter = Some(Length::from_mm(number)),
        ShapeField::OverallLength => geometry.overall_length = Some(Length::from_mm(number)),
        ShapeField::Flutes => geometry.num_flutes = Some(number.max(0.0) as u32),
      }
    }
    geometry
  }

  /// Make the geometry agree with the kind it declares.
  fn shape_geometry(&self, geometry: &mut ToolGeometry) {
    geometry.tool_type = self.kind.raw();
    let half = geometry.diameter.map(|diameter| diameter.mm() / 2.0);
    match self.kind {
      ToolKind::FlatEndMill | ToolKind::Engraving => {
        geometry.tip_radius = None;
        geometry.included_angle = None;
        geometry.flat_diameter = None;
      }
      ToolKind::BallNose => {
        geometry.tip_radius = half.map(Length::from_mm);
        geometry.included_angle = None;
        geometry.flat_diameter = None;
      }
      ToolKind::CornerRadius => {
        geometry.included_angle = None;
        geometry.flat_diameter = None;
        if geometry.tip_radius.is_none() {
          geometry.tip_radius = half.map(|half| Length::from_mm(half / 2.0));
        }
      }
      ToolKind::VBit => {
        if geometry.included_angle.is_none() {
          geometry.included_angle = Some(Angle::from_degrees(90.0));
        }
      }
      ToolKind::Drill => {
        if geometry.included_angle.is_none() {
          geometry.included_angle = Some(Angle::from_degrees(118.0));
        }
        geometry.flat_diameter = None;
      }
    }
  }

  /// A record whose geometry is `geometry` and whose cutting data is the draft's.
  fn snapshot(&self, geometry: &ToolGeometry) -> StoredTool {
    StoredTool {
      geometry: geometry.clone(),
      cutting: self.original.cutting.clone(),
    }
  }

  /// The tool the boxes describe, or the reason they do not describe one.
  pub fn commit(&self) -> Result<StoredTool, DraftProblem> {
    let mut geometry = self.shaped_geometry();
    self.shape_geometry(&mut geometry);
    geometry.name_format = self.name.trim().to_owned();

    let cutting = &self.original.cutting;
    let mut next = CuttingData {
      id: cutting.id.clone(),
      rate_units: cutting.rate_units,
      ..cutting.clone()
    };
    let length = |field: FormField| -> Result<Option<Length>, DraftProblem> {
      let text = self.text(field).trim();
      if text.is_empty() {
        return Ok(None);
      }
      text
        .parse::<f64>()
        .map(|value| Some(Length::from_mm(value)))
        .map_err(|_| DraftProblem::NotANumber {
          label: field.label(),
          text: text.to_owned(),
        })
    };
    let number = |field: FormField| -> Result<Option<f64>, DraftProblem> {
      let text = self.text(field).trim();
      if text.is_empty() {
        return Ok(None);
      }
      text
        .parse::<f64>()
        .map(Some)
        .map_err(|_| DraftProblem::NotANumber {
          label: field.label(),
          text: text.to_owned(),
        })
    };
    next.stepdown = length(FormField::Stepdown)?;
    next.stepover = length(FormField::Stepover)?;
    next.feed_rate = number(FormField::Feed)?.map(|rate| stored(rate, cutting.rate_units));
    next.plunge_rate = number(FormField::Plunge)?.map(|rate| stored(rate, cutting.rate_units));
    next.spindle_speed = number(FormField::Spindle)?.map(|speed| speed.max(0.0) as u32);
    next.tool_number = number(FormField::ToolNumber)?.map(|number| number.max(0.0) as u32);

    let tool = StoredTool {
      geometry,
      cutting: next,
    };
    validate_tool(&tool).map_err(DraftProblem::Invalid)?;
    Ok(tool)
  }

  /// The sentence the footer shows, or `None` when the draft can be committed.
  pub fn problem(&self) -> Option<String> {
    self.commit().err().map(|problem| problem.sentence())
  }

  /// Whether the draft says the same thing the record does.
  ///
  /// Used to decide whether a commit has anything to write, so pressing `Use this tool` twice does not
  /// rewrite a library for nothing.
  pub fn changed(&self) -> bool {
    match self.commit() {
      Ok(tool) => tool != self.original,
      Err(_) => true,
    }
  }
}
