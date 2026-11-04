//! The tool library as a master-detail modal (camui-prancheta S9).
//!
//! S9 asks for one screen: a search field; a tree grouped by tool family with a count and one row per tool
//! (mini tool glyph, name, the dim `Ø6 · 2 cut(s)` line); a detail form whose fields change with the family,
//! grouped under `GEOMETRY` / `PASSES` / `SPEEDS` / `MACHINE`; the tool drawn to scale with dimension
//! callouts; and a footer of library actions above `Cancel` / `Use this tool`.
//!
//! Four decisions this module makes, each of them because the alternative would be a control that lies
//! (the rule L6, L14 and L22 established and the T6 review enforced, L23):
//!
//! 1. **The family is derived from the geometry, never from `tool_type`.** The library keeps the tool's type
//!    as a raw integer and says in its own header that the numeric mapping is not established from the
//!    artifact ([`cam_store::tools`]), which is why the current-tool card already refuses to name it and
//!    writes `Tool` where its type would go (the crate's `TOOL_TYPE_LABEL`). A tree grouped by a code nobody
//!    can read would be a heading over a number. What *is*
//!    proved by the record is its shape — an included angle makes a V, a tip radius half the diameter makes a
//!    ball, any other tip radius makes a corner radius, and no radius at all makes a flat end mill — so that
//!    is the family, and [`ToolFamily::of`] is the one rule that decides it.
//! 2. **A group is drawn only when the record can fill it.** `PASSES` is the three numbers that describe a
//!    pass and `SPEEDS` the three that describe a rate; a tool whose cutting data holds neither has no pass
//!    and no speed to report, and a heading over nothing is the L6 lie. `GEOMETRY` is always drawn — the
//!    shape is what a tool is, and a record that has none is a fact the form has to say out loud — and so is
//!    `MACHINE`, because every record carries the units its rates were authored in.
//! 3. **The library actions in the footer are the ones this shell can really perform.** It can perform
//!    none: the shell holds a read-only snapshot of the library ([`Shell::set_tools`]), `cam-store` owns the
//!    only writer, and `cam-cli tools add` is the route that exists today. So the footer's action row carries
//!    one sentence naming the five the reference draws and why they are not here, instead of five buttons
//!    that would emit nothing — L14's correction, applied before it could be found again.
//! 4. **`Use this tool` is drawn only when it can be honoured, and the reason is drawn when it cannot.** The
//!    modal writes one thing: the number the toolpath that opened it cuts with
//!    ([`crate::UiAction::SetToolpathToolNumber`]). A tool the library gives no number, a toolpath that has
//!    left the document, and an empty library are all states in which that write has no meaning, so the
//!    button is replaced by the sentence that says so. `Cancel` is always honourable for one reason worth
//!    writing down: nothing in this modal writes before it, so there is nothing for it to undo.
//!
//! The drawing is a section of the tool's cutting end, in a box whose **horizontal scale is shared by every
//! tool at once** — [`SPAN_MM`] of tool is the box's width — so two tools in the same box are comparable by
//! eye and a test can check the ratio. The vertical extent is the flute length when the record holds one; when
//! it does not, the shape is drawn to the box's own height and no flute callout is painted, because the
//! library carries no length to point at and a guessed one would be a dimension the shell does not have (the
//! same rule `view3d::paint_tool` follows about the spindle).

use cam_app::AppState;
use cam_store::tools::{stored_tool_for_number, CuttingData, StoredTool, ToolGeometry};
use cam_units::{Format, Length};
use egui::{Align, Layout, RichText, Sense};

use crate::theme::{self, Metrics, Palette, Role};
use crate::{Shell, ShellOutput, UiAction};

/// The width of tool, in millimetres, the drawing's box represents.
///
/// One number for every tool, which is what "to scale" means: a Ø6 tool is drawn at half the width of a Ø12
/// one in the same box. A tool wider than this gets a scale of its own (see [`paint_section`]) so it stays
/// inside the frame, and the callout is what says how wide it really is — the picture is never the only place
/// a number is written.
const SPAN_MM: f32 = 40.0;

/// The master column's width: wide enough for a tool title and its `Ø6 · 2 cut(s)` line.
const MASTER_WIDTH: f32 = 296.0;

/// The detail column's width: the form's two boxes a row, with the reference's proportions.
const DETAIL_WIDTH: f32 = 460.0;

/// How tall the two columns are, so the modal does not grow and shrink with the form.
const COLUMN_HEIGHT: f32 = 344.0;

/// How tall the drawing's box is: the section, its `Ø` dimension line, and the callouts under them.
const DRAWING_HEIGHT: f32 = 186.0;

/// The words the modal says about itself.
///
/// Constants because the same string is both drawn and reported: the title, the sentence that refuses a
/// commit, and the words a headless test quotes back cannot drift apart.
const TITLE: &str = "Choose tool";
const SEARCH_LABEL: &str = "Find a tool";
const SEARCH_HINT: &str = "name, number, id or shape";
const CLEAR: &str = "Clear";
const USE: &str = "Use this tool";
const CANCEL: &str = "Cancel";
const NO_TOOL: &str = "Choose a tool in the tree and the form will describe it.";
const NO_DIAMETER: &str =
  "This tool carries no diameter, so there is no width to draw. Its numbers are below.";
const NO_MATCH: &str = "No tool matches that search.";
const EMPTY_LIBRARY: &str =
  "The library this window was given holds no tool, so there is nothing to list or to use.";
const NO_TARGET: &str =
  "This window was opened without a toolpath, so there is no toolpath for a choice to apply to.";
/// The footer's action row, and the whole of what this shell can do about the library.
const LIBRARY_ACTIONS: &str = "New, Duplicate, Delete, Import… and Export… are not here: this window holds a \
read-only snapshot of the library, so nothing in it could write one. `cam-cli tools add` is how a library \
is filled today.";

/// A tool family: the shape a tool's own geometry proves.
///
/// Derived, never declared (see this module's header): `tool_type` is a raw code the library does not map, so
/// two tools that carry the same shape fields are the same family whatever their code says.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ToolFamily {
  /// A straight cutter with a square end: a diameter and nothing that shapes its tip.
  Flat,
  /// A hemisphere for a tip: a tip radius of exactly half the diameter.
  BallNose,
  /// A square corner with a radius on it: a tip radius that is not half the diameter.
  CornerRadius,
  /// A cone ending in a flat: an included angle.
  VBit,
}

impl ToolFamily {
  /// Every family, in the order the tree groups them: the plainest shape first, the V last.
  pub const ALL: [ToolFamily; 4] = [
    ToolFamily::Flat,
    ToolFamily::BallNose,
    ToolFamily::CornerRadius,
    ToolFamily::VBit,
  ];

  /// The family a tool's own geometry proves.
  ///
  /// An included angle decides first, because a V is the only shape with one. Then the tip radius: half the
  /// diameter is a hemisphere, anything else is a corner that was rounded. A tool with neither is straight,
  /// which is also the answer for a record whose diameter is missing — there is no radius to prove otherwise.
  pub fn of(geometry: &ToolGeometry) -> Self {
    if geometry.included_angle.is_some() {
      return Self::VBit;
    }
    match (geometry.diameter, geometry.tip_radius) {
      (Some(diameter), Some(radius)) if (radius.mm() * 2.0 - diameter.mm()).abs() <= 1.0e-6 => {
        Self::BallNose
      }
      (_, Some(_)) => Self::CornerRadius,
      _ => Self::Flat,
    }
  }

  /// The word the tree's heading carries.
  pub fn label(self) -> &'static str {
    match self {
      Self::Flat => "Flat end mill",
      Self::BallNose => "Ball nose",
      Self::CornerRadius => "Corner radius",
      Self::VBit => "V-bit",
    }
  }

  /// The `GEOMETRY` fields this family has, in the order the form draws them.
  pub fn fields(self) -> &'static [ShapeField] {
    match self {
      Self::Flat => &[
        ShapeField::Diameter,
        ShapeField::FluteLength,
        ShapeField::Flutes,
      ],
      Self::BallNose => &[
        ShapeField::Diameter,
        ShapeField::TipRadius,
        ShapeField::FluteLength,
        ShapeField::Flutes,
      ],
      Self::CornerRadius => &[
        ShapeField::Diameter,
        ShapeField::CornerRadius,
        ShapeField::FluteLength,
        ShapeField::Flutes,
      ],
      Self::VBit => &[
        ShapeField::Diameter,
        ShapeField::IncludedAngle,
        ShapeField::FlatTip,
        ShapeField::FluteLength,
        ShapeField::Flutes,
      ],
    }
  }
}

/// One field of the `GEOMETRY` group.
///
/// The family decides which of these a tool has ([`ToolFamily::fields`]), which is the whole of "the fields
/// change with the family": a flat end mill has no included angle to show, and showing one as `—` would
/// describe a shape it is not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeField {
  /// The cutting diameter.
  Diameter,
  /// A V's included angle.
  IncludedAngle,
  /// The flat at the bottom of a V.
  FlatTip,
  /// A ball's radius.
  TipRadius,
  /// A square corner's radius.
  CornerRadius,
  /// The usable length of the flutes.
  FluteLength,
  /// How many cutting edges the tool has.
  Flutes,
}

impl ShapeField {
  /// The label the form writes above the value.
  pub fn label(self) -> &'static str {
    match self {
      Self::Diameter => "Diameter",
      Self::IncludedAngle => "Included angle",
      Self::FlatTip => "Flat tip",
      Self::TipRadius => "Ball radius",
      Self::CornerRadius => "Corner radius",
      Self::FluteLength => "Flute length",
      Self::Flutes => "Flutes",
    }
  }

  /// The value the record holds, or a dash for a number the library does not have.
  ///
  /// A dash and never a zero: a tool with no flute length has no length, and printing `0.00 mm` would be a
  /// number nobody wrote — the same rule [`crate::chrome`]'s shape line follows.
  pub fn value(self, geometry: &ToolGeometry) -> String {
    let length = |value: Option<Length>| match value {
      Some(length) => format!("{} mm", Format::fixed(2).apply(length.mm())),
      None => "—".to_owned(),
    };
    match self {
      Self::Diameter => match geometry.diameter {
        Some(diameter) => format!("Ø {}", length(Some(diameter))),
        None => "—".to_owned(),
      },
      Self::IncludedAngle => match geometry.included_angle {
        Some(angle) => format!("{}°", Format::fixed(1).apply(angle.degrees())),
        None => "—".to_owned(),
      },
      Self::FlatTip => length(geometry.flat_diameter),
      Self::TipRadius | Self::CornerRadius => match geometry.tip_radius {
        Some(radius) => format!("R {}", length(Some(radius))),
        None => "—".to_owned(),
      },
      Self::FluteLength => length(geometry.flute_length),
      Self::Flutes => match geometry.num_flutes {
        Some(flutes) => flutes.to_string(),
        None => "—".to_owned(),
      },
    }
  }
}

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

/// A tool's title: `T4 - Tool (12.7mm)`, or the name alone when the record has no number.
///
/// The card's own format ([`crate::chrome`]`::tool_card`), because a tree row and the card that opens it are
/// naming the same tool and a second spelling would read as a second tool.
pub fn tool_title(tool: &StoredTool) -> String {
  let name = crate::chrome::tool_name(&tool.geometry);
  match tool.cutting.tool_number {
    Some(number) => format!("T{number} - {name}"),
    None => name,
  }
}

/// The row's dim second line: `Ø6 · 2 cut(s)`.
///
/// The two numbers the reference's row carries, and nothing else: the diameter without its unit, because the
/// row is a list and the form below it is where a number is written with its unit, and the flute count, which
/// the reference calls the number of cuts. A number the library does not hold is a dash, never a zero.
pub fn cut_line(tool: &StoredTool) -> String {
  let diameter = match tool.geometry.diameter {
    Some(diameter) => trim_mm(diameter.mm()),
    None => "—".to_owned(),
  };
  let cuts = match tool.geometry.num_flutes {
    Some(flutes) => flutes.to_string(),
    None => "—".to_owned(),
  };
  format!("Ø{diameter} · {cuts} cut(s)")
}

/// A millimetre value with the zeroes a reader does not need: `6`, `12.7`, `3.175`.
fn trim_mm(millimetres: f64) -> String {
  let text = Format::fixed(3).apply(millimetres);
  let trimmed = text.trim_end_matches('0').trim_end_matches('.');
  if trimmed.is_empty() {
    "0".to_owned()
  } else {
    trimmed.to_owned()
  }
}

/// One row of the master tree, with nothing borrowed: the paint needs no access to the shell's library.
#[derive(Clone, Debug)]
struct Row {
  /// The row's title, as reported and painted.
  title: String,
  /// The dim second line.
  cut: String,
  /// The library's own geometry id: what a selection names.
  id: String,
}

/// The tree's rows for a library snapshot, in the library's own order.
fn tree_rows(tools: &[StoredTool]) -> Vec<(ToolFamily, Row)> {
  tools
    .iter()
    .map(|tool| {
      (
        ToolFamily::of(&tool.geometry),
        Row {
          title: tool_title(tool),
          cut: cut_line(tool),
          id: tool.geometry.id.clone(),
        },
      )
    })
    .collect()
}

/// The tool one selection id names, when the library this window was given still holds it.
fn tool_with<'a>(tools: &'a [StoredTool], id: Option<&str>) -> Option<&'a StoredTool> {
  let id = id?;
  tools.iter().find(|tool| tool.geometry.id == id)
}

/// Whether one row survives the search field.
///
/// The title, the id, the family's own word and the cut line are all searched, because they are all on
/// screen: an operator who reads `Ball nose (2)` and types `ball` is asking for the rows under it, and one who
/// types `Ø6` is asking for the tools that width. Case is folded, because nobody types a capital to find one.
fn row_matches(row: &Row, family: ToolFamily, needle: &str) -> bool {
  let needle = needle.trim().to_lowercase();
  if needle.is_empty() {
    return true;
  }
  row.title.to_lowercase().contains(&needle)
    || row.id.to_lowercase().contains(&needle)
    || row.cut.to_lowercase().contains(&needle)
    || family.label().to_lowercase().contains(&needle)
}

/// A tool the modal can draw and describe, with nothing borrowed.
#[derive(Clone, Debug)]
struct Detail {
  /// The tool's title, the modal's own heading once a tool is chosen.
  title: String,
  /// The number the program names it by, when the library holds one.
  number: Option<u32>,
  /// The shape the drawing is made from.
  geometry: ToolGeometry,
  /// The groups and fields the form draws, in order.
  groups: Vec<(String, Vec<(String, String)>)>,
}

/// Everything the detail side needs about one tool.
fn detail_of(tool: &StoredTool) -> Detail {
  Detail {
    title: tool_title(tool),
    number: tool.cutting.tool_number,
    geometry: tool.geometry.clone(),
    groups: form_groups(tool)
      .into_iter()
      .map(|group| (group.label().to_owned(), form_fields(group, tool)))
      .collect(),
  }
}

/// A pressable thing the modal drew, with the rectangle it occupied and the area it was clipped to.
///
/// The clip is kept per control, and for one reason: a row scrolled out of the master list still produces a
/// shape, and a report that named its centre would be a promise about a control nobody can see. The T6 review
/// found exactly that in the playback bar — invisible, with live hit areas, behind 43 green tests (L23) — so
/// the report drops anything the frame that drew it would not have shown.
struct Spot {
  /// The word on the control.
  word: String,
  /// Where it was drawn.
  rect: egui::Rect,
  /// The area the control was clipped to when it was drawn.
  clip: egui::Rect,
}

impl Spot {
  /// A control drawn inside `ui`'s current clip rectangle.
  fn in_ui(word: String, rect: egui::Rect, ui: &egui::Ui) -> Self {
    Self {
      word,
      rect,
      clip: ui.clip_rect(),
    }
  }
}

/// Drop every spot its own clip cut away, and report the rest as `(word, point)`.
///
/// The point reported is the centre, so the centre is what has to be inside the clip: a row with two pixels
/// showing would otherwise report a point in the fold.
fn pressable(spots: Vec<Spot>) -> Vec<(String, (f32, f32))> {
  spots
    .into_iter()
    .filter(|spot| spot.clip.contains(spot.rect.center()))
    .map(|spot| {
      let centre = spot.rect.center();
      (spot.word, (centre.x, centre.y))
    })
    .collect()
}

impl Shell {
  /// Open the tool library for the toolpath the current-tool card names.
  ///
  /// The card describes the document's **first** toolpath, so that is what this dialog retools — one rule in
  /// one place. The selection starts on the tool that toolpath already cuts with, because the first thing an
  /// operator wants from `Choose another tool…` is to see what the tool is now.
  ///
  /// Public for the reason [`Shell::open_material`] is: a modal whose opening cannot be driven from outside a
  /// frame is a modal only a mouse could test.
  pub fn open_tool_library(&mut self, app: &AppState) {
    let target = app.document().toolpaths.first().map(|toolpath| {
      (
        toolpath.id.clone(),
        stored_tool_for_number(&self.tools, toolpath.tool_number)
          .map(|tool| tool.geometry.id.clone()),
      )
    });
    self.tool_library_target = target.as_ref().map(|(id, _)| id.clone());
    self.tool_library_selected = target.and_then(|(_, selected)| selected);
    self.tool_search.clear();
    self.show_tool_library = true;
  }

  /// Leave the library without writing anything.
  ///
  /// The drafts go with it: a search left behind would filter the next opening for a reason the operator has
  /// already forgotten, and a selection left behind would be a choice nobody made in this visit.
  pub fn close_tool_library(&mut self) {
    self.show_tool_library = false;
    self.tool_library_target = None;
    self.tool_library_selected = None;
    self.tool_library_for_draft = false;
    self.tool_search.clear();
  }

  /// Open the library to choose the tool a **new** strategy will cut with (camui-prancheta S10).
  ///
  /// The same modal and the same commit, with the one difference the subject forces: the strategy editor is
  /// drafting a toolpath that does not exist yet, so `Use this tool` has no document row to write and writes the
  /// draft's tool number instead. Opening it from the card keeps the modal's own promise — the selection starts
  /// on the tool the draft already names, or on nothing when it names none.
  pub(crate) fn open_tool_library_for_draft(&mut self) {
    self.tool_library_target = None;
    self.tool_library_selected = self.strategy.tool_number.and_then(|number| {
      stored_tool_for_number(&self.tools, number).map(|tool| tool.geometry.id.clone())
    });
    self.tool_library_for_draft = true;
    self.tool_search.clear();
    self.show_tool_library = true;
  }

  /// The toolpath `Use this tool` would retool, by id, while the modal is open.
  pub fn tool_library_target(&self) -> Option<&str> {
    self.tool_library_target.as_deref()
  }

  /// The tool the detail side is showing, by the library's own geometry id.
  pub fn tool_library_selection(&self) -> Option<&str> {
    self.tool_library_selected.as_deref()
  }

  /// Why `Use this tool` cannot be honoured right now, or `None` when it can.
  ///
  /// The order is the operator's: an empty library first, then the toolpath that opened the dialog, then the
  /// choice itself. Each sentence names the thing that is missing, so a dark button is never a mystery — and
  /// the button is not drawn at all while one is returned (camui-prancheta S9, L6).
  pub fn tool_library_refusal(&self, app: &AppState) -> Option<String> {
    self.refusal_for(
      app,
      tool_with(&self.tools, self.tool_library_selected.as_deref()),
    )
  }

  /// Why `Use this tool` cannot be honoured for one particular choice.
  ///
  /// Split from [`Shell::tool_library_refusal`] because the frame asks it **after** the tree has run: the
  /// answer the operator sees must be about the row this frame selected, not the one the last frame had
  /// (L13's reporting rule, which the T3 task paid for).
  fn refusal_for(&self, app: &AppState, tool: Option<&StoredTool>) -> Option<String> {
    // A draft opened by the strategy editor has no document row to write (S10): the one thing the commit can
    // mean is "this strategy cuts with this tool", so all it needs is a tool with a number.
    if self.tool_library_for_draft {
      let Some(tool) = tool else {
        return Some(NO_TOOL.to_owned());
      };
      if tool.cutting.tool_number.is_none() {
        return Some(format!(
          "{} carries no tool number in the library, so a toolpath cannot name it.",
          tool_title(tool)
        ));
      }
      return None;
    }
    if self.tools.is_empty() {
      return Some(EMPTY_LIBRARY.to_owned());
    }
    let Some(target) = self.tool_library_target.as_deref() else {
      return Some(NO_TARGET.to_owned());
    };
    if !app
      .document()
      .toolpaths
      .iter()
      .any(|toolpath| toolpath.id == target)
    {
      return Some(format!(
        "The toolpath this dialog was opened for ({target}) is no longer in the document, so there is \
         nothing to retool."
      ));
    }
    let Some(tool) = tool else {
      return Some(NO_TOOL.to_owned());
    };
    if tool.cutting.tool_number.is_none() {
      return Some(format!(
        "{} carries no tool number in the library, so a toolpath cannot name it.",
        tool_title(tool)
      ));
    }
    None
  }

  /// The tool library's modal: the master tree, the detail form and the footer.
  ///
  /// One `egui::Modal`, the shape [`crate::chrome`]'s material dialog already uses: the reference draws this
  /// screen over the task panel rather than inside it, and a modal is what tells the operator that the
  /// drawing underneath is not taking clicks.
  ///
  /// **The frame that selects a row is the frame that describes it** (L13). The tree is drawn first and the
  /// detail is resolved from the selection the tree just made, so a click and its consequence are in one
  /// frame; the title is the only thing drawn above the tree, so its text is painted into a rect reserved
  /// before it and filled after the tree has run — a title one frame behind the form would be the same
  /// reporting defect one layer up.
  pub(crate) fn tool_library_dialog(
    &mut self,
    ctx: &egui::Context,
    app: &AppState,
    output: &mut ShellOutput,
  ) {
    let palette = self.palette();
    let metrics = self.metrics();
    // The drafts are copied out so the closure can edit them: the modal writes to `self` once, after the
    // frame, and nothing else about the shell changes while it is open.
    let mut search = self.tool_search.clone();
    let mut selected = self.tool_library_selected.clone();
    // A selection the library no longer holds is no selection: the tools this window was given can be replaced
    // between two frames, and a form describing a tool nobody has would be describing nothing.
    if tool_with(&self.tools, selected.as_deref()).is_none() {
      selected = None;
    }

    let tree = tree_rows(&self.tools);

    let mut words: Vec<String> = Vec::new();
    let mut spots: Vec<Spot> = Vec::new();
    let mut families: Vec<(String, usize)> = Vec::new();
    let mut rows: Vec<(String, String, String)> = Vec::new();
    let mut groups: Vec<(String, Vec<(String, String)>)> = Vec::new();
    let mut callouts: Vec<String> = Vec::new();
    let mut drawing: Option<(egui::Rect, f32)> = None;
    let mut cancel = false;
    let mut commit: Option<u32> = None;

    egui::Modal::new(egui::Id::unique("campilot-tool-library"))
      .frame(
        egui::Frame::NONE
          .fill(palette.get(Role::Overlay))
          .stroke(egui::Stroke::new(
            metrics.hairline,
            palette.get(Role::Border),
          ))
          .corner_radius(egui::CornerRadius::same(metrics.radius_dialog as u8))
          .inner_margin(egui::Margin::same(metrics.space(5) as i8)),
      )
      .show(ctx, |ui| {
        let columns = MASTER_WIDTH + DETAIL_WIDTH + metrics.space(6);
        ui.set_max_width(columns);
        // Reserved now, painted at the end: the title names the tool this frame chose.
        let (title_rect, _) =
          ui.allocate_exact_size(egui::vec2(columns, metrics.space(6)), Sense::hover());
        ui.add_space(metrics.space(2));

        let mut detail: Option<Detail> = None;
        ui.horizontal_top(|ui| {
          // ---- the master side: the search field and the family tree -------------------------------
          ui.vertical(|ui| {
            ui.set_width(MASTER_WIDTH);
            ui.label(
              RichText::new(SEARCH_LABEL)
                .size(theme::CAPTION)
                .color(palette.get(Role::TextDim)),
            );
            ui.horizontal(|ui| {
              ui.add(
                egui::TextEdit::singleline(&mut search)
                  .hint_text(SEARCH_HINT)
                  .desired_width(MASTER_WIDTH - 52.0),
              );
              // A filter with no way out except selecting the text and deleting it is a trap; the chip is the
              // same vocabulary the status bar's snaps use.
              if !search.is_empty() {
                let clear = crate::chrome::chip(ui, &palette, CLEAR, false);
                spots.push(Spot::in_ui(CLEAR.to_owned(), clear.rect, ui));
                if clear.clicked() {
                  search.clear();
                }
              }
            });
            ui.add_space(metrics.space(2));
            egui::ScrollArea::vertical()
              .id_salt("tool_tree")
              .max_height(COLUMN_HEIGHT)
              .show(ui, |ui| {
                // Every row reports the clip of the scroll area it was drawn in (see `Spot`), so a row the
                // fold hides is drawn by egui and is not reported as pressable.
                let mut shown = 0;
                for family in ToolFamily::ALL {
                  let group: Vec<&Row> = tree
                    .iter()
                    .filter(|entry| entry.0 == family && row_matches(&entry.1, family, &search))
                    .map(|entry| &entry.1)
                    .collect();
                  if group.is_empty() {
                    continue;
                  }
                  shown += group.len();
                  let heading = format!("{} ({})", family.label(), group.len());
                  ui.add_space(metrics.space(1));
                  ui.label(
                    RichText::new(&heading)
                      .size(theme::CAPTION)
                      .color(palette.get(Role::TextDim)),
                  );
                  families.push((family.label().to_owned(), group.len()));
                  words.push(heading);
                  for row in group {
                    let on = selected.as_deref() == Some(row.id.as_str());
                    let (rect, response) = ui.allocate_exact_size(
                      egui::vec2(ui.available_width(), metrics.row_height),
                      Sense::click(),
                    );
                    let radius = egui::CornerRadius::same(4);
                    if on {
                      ui.painter()
                        .rect_filled(rect, radius, palette.get(Role::SelSoft));
                      ui.painter().rect_stroke(
                        rect,
                        radius,
                        egui::Stroke::new(1.0, palette.get(Role::Sel)),
                        egui::StrokeKind::Inside,
                      );
                    } else if response.hovered() {
                      ui.painter()
                        .rect_filled(rect, radius, palette.get(Role::Raised));
                    }
                    // The mini tool glyph: the set's own router bit, at the row's size.
                    let glyph = egui::Rect::from_min_size(
                      egui::pos2(rect.left() + 4.0, rect.top() + 3.0),
                      egui::vec2(20.0, 20.0),
                    );
                    crate::icons::paint(
                      ui.painter(),
                      glyph,
                      crate::icons::Icon::ToolLibrary,
                      palette.get(if on { Role::Sel } else { Role::Text }),
                      theme::ICON_STROKE * glyph.width() / theme::ICON_SIZE,
                    );
                    let text_left = glyph.right() + 6.0;
                    ui.painter().text(
                      egui::pos2(text_left, rect.top() + 3.0),
                      egui::Align2::LEFT_TOP,
                      &row.title,
                      egui::FontId::proportional(theme::SMALL),
                      palette.get(if on { Role::TextHi } else { Role::Text }),
                    );
                    ui.painter().text(
                      egui::pos2(text_left, rect.bottom() - 3.0),
                      egui::Align2::LEFT_BOTTOM,
                      &row.cut,
                      egui::FontId::proportional(theme::CAPTION),
                      palette.get(Role::TextDim),
                    );
                    if response.clicked() {
                      selected = Some(row.id.clone());
                    }
                    spots.push(Spot::in_ui(row.title.clone(), rect, ui));
                    words.push(row.title.clone());
                    words.push(row.cut.clone());
                    rows.push((
                      family.label().to_owned(),
                      row.title.clone(),
                      row.cut.clone(),
                    ));
                  }
                }
                if shown == 0 {
                  let sentence = if self.tools.is_empty() {
                    EMPTY_LIBRARY
                  } else {
                    NO_MATCH
                  };
                  ui.add_space(metrics.space(2));
                  ui.label(
                    RichText::new(sentence)
                      .size(theme::CAPTION)
                      .color(palette.get(Role::TextDim)),
                  );
                  words.push(sentence.to_owned());
                }
                ui.add_space(metrics.space(2));
              });
          });

          // Read here, between the two columns: this is the selection the tree has just made.
          detail = tool_with(&self.tools, selected.as_deref()).map(detail_of);

          ui.add_space(metrics.space(3));
          // ---- the detail side: the scaled drawing and the family's form ----------------------------
          ui.vertical(|ui| {
            ui.set_width(DETAIL_WIDTH);
            egui::ScrollArea::vertical()
              .id_salt("tool_detail")
              .max_height(COLUMN_HEIGHT)
              .show(ui, |ui| match &detail {
                Some(detail) => {
                  if detail.geometry.diameter.is_some() {
                    let (rect, _) = ui.allocate_exact_size(
                      egui::vec2(DETAIL_WIDTH, DRAWING_HEIGHT),
                      Sense::hover(),
                    );
                    let width = paint_section(
                      ui.painter(),
                      rect,
                      &palette,
                      &metrics,
                      &detail.geometry,
                      &mut callouts,
                    );
                    words.extend(callouts.iter().cloned());
                    drawing = Some((rect, width));
                  } else {
                    ui.label(
                      RichText::new(NO_DIAMETER)
                        .size(theme::CAPTION)
                        .color(palette.get(Role::TextDim)),
                    );
                    words.push(NO_DIAMETER.to_owned());
                  }
                  for (label, fields) in &detail.groups {
                    crate::chrome::section(ui, &palette, label);
                    words.push(label.clone());
                    ui.horizontal_wrapped(|ui| {
                      for (field, value) in fields {
                        crate::chrome::field(ui, &palette, field, value);
                        words.push((*field).clone());
                        words.push((*value).clone());
                      }
                    });
                  }
                  groups = detail.groups.clone();
                }
                None => {
                  ui.label(
                    RichText::new(NO_TOOL)
                      .size(theme::CAPTION)
                      .color(palette.get(Role::TextDim)),
                  );
                  words.push(NO_TOOL.to_owned());
                }
              });
          });
        });

        ui.add_space(metrics.space(3));
        // ---- the footer: the library's actions, then the commit pair -------------------------------
        ui.separator();
        ui.horizontal_wrapped(|ui| {
          ui.label(
            RichText::new(LIBRARY_ACTIONS)
              .size(theme::CAPTION)
              .color(palette.get(Role::TextDim)),
          );
        });
        words.push(LIBRARY_ACTIONS.to_owned());
        ui.add_space(metrics.space(2));
        // Asked here, after the tree, so the sentence is about the row this frame chose.
        let refusal = self.refusal_for(app, tool_with(&self.tools, selected.as_deref()));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
          // `Cancel` is always drawn: nothing in this modal has written anything before it, so it can always
          // be honoured. `Use this tool` is drawn only when the write it names has a meaning — otherwise the
          // sentence that says why is drawn in its place (L6).
          let back = ui.button(RichText::new(CANCEL).size(theme::SMALL));
          spots.push(Spot::in_ui(CANCEL.to_owned(), back.rect, ui));
          if back.clicked() {
            cancel = true;
          }
          match &refusal {
            Some(sentence) => {
              ui.label(
                RichText::new(sentence)
                  .size(theme::CAPTION)
                  .color(palette.get(Role::Danger)),
              );
              words.push(sentence.clone());
            }
            None => {
              let go = ui.add_enabled(
                detail.is_some(),
                egui::Button::new(RichText::new(USE).size(theme::SMALL)),
              );
              spots.push(Spot::in_ui(USE.to_owned(), go.rect, ui));
              if go.clicked() {
                commit = detail.as_ref().and_then(|detail| detail.number);
              }
            }
          }
        });

        // The title, with the tool this frame selected in it (see this method's own note).
        let title = match &detail {
          Some(detail) => format!("{TITLE} — {}", detail.title),
          None => TITLE.to_owned(),
        };
        ui.painter().text(
          title_rect.left_top(),
          egui::Align2::LEFT_TOP,
          &title,
          egui::FontId::proportional(theme::TITLE),
          palette.get(Role::TextHi),
        );
        words.push(title);
      });

    self.tool_search = search;
    self.tool_library_selected = selected;
    output.drawn.tool_library = words;
    output.drawn.tool_library_hits = pressable(spots);
    output.drawn.tool_families = families;
    output.drawn.tool_rows = rows;
    output.drawn.tool_selected = self.tool_library_selected.clone();
    output.drawn.tool_groups = groups;
    output.drawn.tool_callouts = callouts;
    if let Some((rect, width)) = drawing {
      output.drawn.tool_drawing = Some(crate::chrome::rect_report(rect));
      output.drawn.tool_width_px = width;
    }
    if cancel {
      self.close_tool_library();
    }
    if let Some(number) = commit {
      if self.tool_library_for_draft {
        // The draft's tool, not the document's: `Generate toolpath` is what writes it (S10).
        self.strategy.tool_number = Some(number);
      } else if let Some(target) = self.tool_library_target.clone() {
        output.actions.push(UiAction::SetToolpathToolNumber {
          toolpath_id: target,
          tool_number: number,
        });
      }
      self.close_tool_library();
    }
  }

  /// The current-tool card's `Choose another tool…` row: the one affordance that opens the library.
  ///
  /// Its words are reported apart from [`crate::DrawnState::tool_card`] on purpose: the card is a reading of
  /// the tool a toolpath cuts with, and it grew a control, not a fourth field.
  pub(crate) fn choose_tool_row(
    &mut self,
    app: &AppState,
    ui: &mut egui::Ui,
    output: &mut ShellOutput,
  ) {
    let response = ui.button(RichText::new(CHOOSE).size(theme::SMALL));
    let centre = response.rect.center();
    output
      .drawn
      .tool_card_hits
      .push((CHOOSE.to_owned(), (centre.x, centre.y)));
    if response.clicked() {
      // The subject decides what the choice can write (S10): a toolpath on screen is retooled, a draft is
      // filled in for `Generate toolpath` to write.
      if app.document().toolpaths.is_empty() {
        self.open_tool_library_for_draft();
      } else {
        self.open_tool_library(app);
      }
    }
  }
}

/// The word on the card's opener, spelled the way the reference frame spells it.
pub(crate) const CHOOSE: &str = "Choose another tool…";

/// Draw one tool's cutting end to scale, and its dimension callouts.
///
/// The section is built from the tool's own numbers and nothing else: the width is its diameter, the tip is
/// the profile its family describes — a square end, a hemisphere, a rounded corner, or a cone ending in a
/// flat — and every callout points at a dimension the record holds. A number the library does not have is
/// not drawn at all, which is why a tool with no flute length gets no length callout.
///
/// The **horizontal scale is shared** ([`SPAN_MM`] across the box), so two tools drawn in it are comparable:
/// that is what "to scale" means here, and it is why the scale is not computed from the tool alone, which
/// would draw a Ø3 and a Ø12 tool the same width. A tool wider than the span gets the box's width and the
/// callout says how wide it really is, because a picture that overflowed its frame would be unreadable.
fn paint_section(
  painter: &egui::Painter,
  rect: egui::Rect,
  palette: &Palette,
  metrics: &Metrics,
  geometry: &ToolGeometry,
  callouts: &mut Vec<String>,
) -> f32 {
  let Some(diameter) = geometry.diameter else {
    return 0.0;
  };
  let diameter = diameter.mm();
  if !diameter.is_finite() || diameter <= 0.0 {
    return 0.0;
  }
  let span = SPAN_MM.max(diameter as f32);
  let scale = rect.width() / span;
  let width = (diameter as f32 * scale).max(1.0);
  let half = width / 2.0;
  let family = ToolFamily::of(geometry);

  let centre = rect.center().x;
  // The shape's own band: the diameter line and the callouts live under it, so the section is never crossed
  // by a number that makes it unreadable — the lesson the material plan's first screenshot taught.
  let top = rect.top() + metrics.space(2);
  let bottom = rect.top() + DRAWING_HEIGHT * 0.62;
  let tip_height = (tip_height_mm(geometry) as f32 * scale).min(bottom - top - 1.0);
  let shoulder = bottom - tip_height;

  let ink = egui::Stroke::new(1.6, palette.get(Role::TextHi));
  let flank = |painter: &egui::Painter, y: f32| {
    painter.line_segment(
      [egui::pos2(centre - half, top), egui::pos2(centre - half, y)],
      ink,
    );
    painter.line_segment(
      [egui::pos2(centre + half, top), egui::pos2(centre + half, y)],
      ink,
    );
  };
  // The shank's end is the frame's own edge: the library holds no overall length, so the top edge is where the
  // box ends and no callout points at it (`view3d::paint_tool`'s rule about the spindle).
  painter.line_segment(
    [
      egui::pos2(centre - half, top),
      egui::pos2(centre + half, top),
    ],
    ink,
  );

  match family {
    ToolFamily::Flat => {
      flank(painter, bottom);
      painter.line_segment(
        [
          egui::pos2(centre - half, bottom),
          egui::pos2(centre + half, bottom),
        ],
        ink,
      );
    }
    ToolFamily::BallNose => {
      let radius = half;
      let centre_y = bottom - radius;
      flank(painter, centre_y);
      // The hemisphere, as eleven points on the arc: egui has no arc primitive and a circle would bulge the
      // wrong way.
      let points: Vec<egui::Pos2> = (0..=10)
        .map(|step| {
          let angle = std::f32::consts::PI * (1.0 - step as f32 / 10.0);
          egui::pos2(
            centre + radius * angle.cos(),
            centre_y + radius * angle.sin(),
          )
        })
        .collect();
      painter.add(egui::Shape::line(points, ink));
    }
    ToolFamily::CornerRadius => {
      let radius = (geometry.tip_radius.map(|r| r.mm()).unwrap_or(0.0) as f32 * scale)
        .min(half)
        .min(bottom - top);
      flank(painter, bottom - radius);
      let points = vec![
        egui::pos2(centre - half, bottom - radius),
        egui::pos2(centre - half + radius, bottom),
        egui::pos2(centre + half - radius, bottom),
        egui::pos2(centre + half, bottom - radius),
      ];
      painter.add(egui::Shape::line(points, ink));
    }
    ToolFamily::VBit => {
      let flat = geometry
        .flat_diameter
        .map(|flat| (flat.mm() as f32 * scale) / 2.0)
        .unwrap_or(0.0)
        .min(half);
      flank(painter, shoulder);
      let points = vec![
        egui::pos2(centre - half, shoulder),
        egui::pos2(centre - flat, bottom),
        egui::pos2(centre + flat, bottom),
        egui::pos2(centre + half, shoulder),
      ];
      painter.add(egui::Shape::line(points, ink));
    }
  }

  // The width's dimension line, under the section: the one number every family has.
  let line_y = bottom + metrics.space(3);
  let width_label = format!("Ø {} mm", Format::fixed(2).apply(diameter));
  crate::chrome::dimension_line(
    painter,
    palette,
    egui::pos2(centre - half, line_y),
    egui::pos2(centre + half, line_y),
    &width_label,
    egui::vec2(0.0, 2.0),
    egui::Align2::CENTER_TOP,
  );
  callouts.push(width_label);

  // The family's own dimension, when the record holds the number that describes it.
  let family_callout = match family {
    ToolFamily::Flat => None,
    ToolFamily::BallNose => geometry
      .tip_radius
      .map(|radius| format!("Ball radius R {} mm", Format::fixed(2).apply(radius.mm()))),
    ToolFamily::CornerRadius => geometry
      .tip_radius
      .map(|radius| format!("Corner radius R {} mm", Format::fixed(2).apply(radius.mm()))),
    ToolFamily::VBit => match (geometry.included_angle, geometry.flat_diameter) {
      (Some(angle), flat) => Some(format!(
        "Included angle {}° · flat {} mm",
        Format::fixed(1).apply(angle.degrees()),
        match flat {
          Some(flat) => Format::fixed(2).apply(flat.mm()),
          None => "—".to_owned(),
        }
      )),
      (None, Some(flat)) => Some(format!("Flat {} mm", Format::fixed(2).apply(flat.mm()))),
      (None, None) => None,
    },
  };
  if let Some(callout) = family_callout {
    callouts.push(callout);
  }
  if let Some(length) = geometry.flute_length {
    callouts.push(format!(
      "Flute length {} mm",
      Format::fixed(2).apply(length.mm())
    ));
  }

  // The legend: one line per callout that is not the width, which the dimension line already carries. It
  // starts a whole step under that line so the `Ø` number and the first legend line never cross.
  let mut cursor = bottom + metrics.space(6);
  for callout in callouts.iter().skip(1) {
    painter.text(
      egui::pos2(rect.left(), cursor),
      egui::Align2::LEFT_TOP,
      callout,
      egui::FontId::monospace(theme::CAPTION),
      palette.get(Role::TextDim),
    );
    cursor += metrics.space(4);
  }

  width
}

/// How tall the tool's tip profile is, in millimetres, for the families whose geometry sets one.
///
/// A flat end mill has no profile: its tip is the end of the body, so its height is `0`. A ball nose's is its
/// radius, a corner radius's is its radius, and a V's is how far the cone rises from its flat to its full
/// diameter — the one length that is arithmetic on the tool's own angle rather than a number it carries.
fn tip_height_mm(geometry: &ToolGeometry) -> f64 {
  match ToolFamily::of(geometry) {
    ToolFamily::Flat => 0.0,
    ToolFamily::BallNose => geometry
      .diameter
      .map(|diameter| diameter.mm() / 2.0)
      .unwrap_or(0.0),
    ToolFamily::CornerRadius => geometry.tip_radius.map(|radius| radius.mm()).unwrap_or(0.0),
    ToolFamily::VBit => {
      let Some(diameter) = geometry.diameter else {
        return 0.0;
      };
      let flat = geometry.flat_diameter.map(|flat| flat.mm()).unwrap_or(0.0);
      let Some(angle) = geometry.included_angle else {
        return 0.0;
      };
      let half_angle = (angle.degrees() / 2.0).to_radians();
      if half_angle <= 0.0 || !half_angle.is_finite() {
        return 0.0;
      }
      ((diameter.mm() - flat) / 2.0) / half_angle.tan()
    }
  }
}
