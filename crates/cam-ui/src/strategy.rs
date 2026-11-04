//! A strategy's editor: the same surface for creating and editing one toolpath (camui-prancheta S10).
//!
//! S10 asks for one editor, prefixed [`NEW`] or [`EDIT`] with the strategy's name, holding its name field,
//! the current-tool card with its opener, the collapsible [`QUICK`] block, its own sections, the note
//! [`NOTE`], and the pair [`GENERATE`] / [`CANCEL`]. The reference draws it twice — `Novo percurso · Desbaste
//! 3D` (`Screenshot 2026-11-03 034023.png`) and `Editar · Perfil` (`033350.png`) — and the two frames differ in
//! the header prefix and in what the footer does, which is exactly what this module makes the difference:
//! [`subject`] reads the document, [`subject`]`::New` drafts a toolpath that does not exist yet, and
//! [`subject`]`::Edit` rewrites the operation of the one that does.
//!
//! **One difference the model forces, and it is stated in the frame rather than hidden.** Every number in this
//! editor is a draft, so the name is applied by [`GENERATE`] when the toolpath is created; `cam-app` has
//! `AddToolpath` and `SetToolpathOperation` and **no rename command**, so while editing, the name is drawn as
//! the document's own reading with the sentence [`RENAME`] under it instead of a field that would quietly drop
//! what was typed. A control that cannot act is not drawn (L6, L14, L22) — the rule the tool library's footer
//! and the playback bar were both corrected for.
//!
//! **What the sections hold, and what they refuse to invent.**
//!
//! - The **kind** is the model's own: `Profile` or `Pocket` ([`cam_model::Operation`]). Choosing it replaces
//!   the draft's operation with that kind's defaults, and [`GENERATE`] writes it — a new toolpath starts with
//!   it, an existing one is rewritten by `SetToolpathOperation`. A document whose toolpath is another
//!   operation (`Drilling`, `VCarve`, …) is not one this shell drafts: the sections say so, name the operation,
//!   and the footer says how to replace it rather than pretending to a form it does not have.
//! - The **numbers** are [`CutField`]s: one table, read by both the [`QUICK`] block and the `CUT` section, so
//!   the quick edit duplicates values instead of holding a second copy of them (the same single-source rule
//!   [`crate::params`] keeps for the seven geometry commands).
//! - **Feed, plunge and spindle are not here.** They belong to the tool library record
//!   ([`cam_store::tools::CuttingData`]), the shell holds a read-only snapshot and `cam-store` owns the only
//!   writer, so the quick block draws them as readings from `tool_library::form_fields` — the same rows, in the
//!   same units, that the library's `SPEEDS` group draws (the units finding L25 paid for) — above the sentence
//!   naming where they are changed.
//!
//! **The image cards are drawn, not pasted.** `034023.png` chooses with pictures: a frontier row and a clearing
//! strategy row. This shell has no frontier to choose — a strategy's region is its `entity_ids`, picked on the
//! canvas — so that row is not invented. The choices that *are* shaped — which kind, and a pocket's clearing
//! pattern — are drawn as cards carrying a diagram painted from the operation's own semantics (a contour with
//! the tool on it; a region filled with rings or with raster rows), with two-pixel strokes on the panel's own
//! palette. What a card would need to carry a real thumbnail instead: a rendered preview of the toolpath, which
//! does not exist before [`GENERATE`] runs and which this shell has no panel renderer for — the 3D half renders
//! a *program*, not an authoring draft. A screenshot of the reference app is not product art and is not used.

use cam_app::AppState;
use cam_model::{Operation, PocketParams, PocketStrategy, ProfileParams};
use cam_store::tools::stored_tool_for_number;
use cam_units::{Format, Length};
use egui::{RichText, Sense};

use crate::chrome;
use crate::theme::{self, Metrics, Palette, Role};
use crate::{Shell, ShellOutput, UiAction};

/// The note that closes the editor, verbatim from S10.
pub const NOTE: &str = "The toolpath is calculated when you click \"Generate toolpath\".";

/// The footer's primary action.
pub const GENERATE: &str = "Generate toolpath";

/// The footer's other action.
pub const CANCEL: &str = "Cancel";

/// The collapsible block that duplicates the operation's numbers.
pub const QUICK: &str = "Quick parameter edit";

/// The prefix the header carries while the editor is drafting a new toolpath.
pub const NEW: &str = "New toolpath";

/// The prefix the header carries while the editor is editing the document's toolpath.
pub const EDIT: &str = "Edit";

/// The label over the draft's name.
pub const NAME: &str = "Name";

/// The section that holds the strategy's own numbers.
pub const CUT: &str = "CUT";

/// Why the name is a reading rather than a field while editing.
pub const RENAME: &str =
  "The name is fixed while editing: this shell has no command to rename a toolpath, and \
`Generate toolpath` rewrites the operation instead of quietly dropping a name.";

/// Why the quick block's rates are readings.
pub const RATES: &str =
  "Feed, plunge and spindle are the tool's own numbers, shown in the units its library \
authored them in. They change in the library, not here.";

/// What the footer says when a new strategy has no tool yet.
pub const NO_TOOL: &str =
  "No tool yet — a strategy has to cut with something, so choose one from the tool \
library first.";

/// What the footer says when a new strategy has no region.
pub const NO_REGION: &str =
  "Nothing is selected — select the vector(s) the strategy cuts, and `Generate \
toolpath` has a region to calculate.";

/// What the footer says when the draft has no name.
pub const NO_NAME: &str =
  "Give the toolpath a name — `Generate toolpath` writes the name it is given.";

/// What the card says before a tool has been chosen.
pub const NO_TOOL_YET: &str =
  "No tool chosen yet — `Choose another tool…` opens the library, and the choice \
is part of this draft until `Generate toolpath` writes it.";

/// What the sections say about an operation this shell does not draft.
pub const NOT_DRAFTED: &str =
  "This shell drafts a profile and a pocket; the rest are authored with `cam-cli \
toolpath`. Choosing Profile or Pocket replaces {operation}, and `Generate toolpath` writes that.";

/// The subject the editor is working on: the document decides it, so both modes run the same code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Subject {
  /// The document holds no toolpath, so the editor is drafting the first one.
  New,
  /// The editor is editing the document's first toolpath, by id.
  ///
  /// The **first** toolpath is the subject for the same reason the current-tool card names the first one
  /// (camui-toolpath S2): one rule in one place, and the frame's tool card and its editor cannot describe two
  /// different toolpaths.
  Edit(String),
}

/// Which toolpath the document puts the editor in front of.
pub(crate) fn subject(app: &AppState) -> Subject {
  match app.document().toolpaths.first() {
    Some(toolpath) => Subject::Edit(toolpath.id.clone()),
    None => Subject::New,
  }
}

/// The editor's draft: the name, the tool and the operation `Generate toolpath` writes.
///
/// Public and flat the way every other parameter of this shell is ([`Shell::material_size`]): a number that
/// could not be set from outside a frame is a number only a mouse could test.
#[derive(Clone, Debug)]
pub struct Draft {
  /// The name the toolpath will carry.
  pub name: String,
  /// The tool number a **new** strategy will cut with, once one has been chosen.
  ///
  /// While editing, the tool is the document's own ([`cam_model::Toolpath::tool_number`]) and the card reads it
  /// from there, because the library modal writes the document directly — one source, and a draft copy could
  /// not disagree with it without lying about the tool on screen.
  pub tool_number: Option<u32>,
  /// The operation and its numbers.
  pub operation: Operation,
  /// Whether the [`QUICK`] block is expanded.
  pub quick_open: bool,
  /// The subject this draft was seeded from, so a document that changed underneath it is re-read once.
  seeded: Option<Subject>,
}

impl Default for Draft {
  fn default() -> Self {
    let operation = Operation::Profile(ProfileParams::default());
    Self {
      name: default_name(&operation),
      tool_number: None,
      operation,
      quick_open: true,
      seeded: None,
    }
  }
}

impl Draft {
  /// Re-seed the draft from the document when its subject changed.
  ///
  /// Called once per frame before the header is drawn, so the frame that reports the editor also reports the
  /// subject it is editing (L13). A document that changed *content* — a tool chosen through the library, an
  /// undo — does not re-seed, because a draft that discarded typed numbers whenever the document moved would be
  /// a form that loses work; the two cases that do re-seed are the two the subject names: no toolpath becomes
  /// one, and a toolpath the operator switched away from is no longer the editor's.
  pub(crate) fn sync(&mut self, app: &AppState) {
    let subject = subject(app);
    if self.seeded.as_ref() == Some(&subject) {
      return;
    }
    match (app.document().toolpaths.first(), &subject) {
      (Some(toolpath), Subject::Edit(_)) => {
        self.name = toolpath.name.clone();
        self.tool_number = Some(toolpath.tool_number);
        self.operation = toolpath.operation.clone();
      }
      _ => {
        let operation = Operation::Profile(ProfileParams::default());
        self.name = default_name(&operation);
        self.tool_number = None;
        self.operation = operation;
      }
    }
    self.quick_open = true;
    self.seeded = Some(subject);
  }

  /// Throw the draft away, so the next frame reads the document again.
  ///
  /// This is what `Cancel` (and the header's `✕`) means: the document, the tool and the name stay exactly as
  /// they were, and so does everything that was typed.
  pub(crate) fn discard(&mut self) {
    self.seeded = None;
  }

  /// The header line, `New toolpath · <name>` or `Edit · <name>`.
  pub(crate) fn header(&self, subject: &Subject) -> String {
    let prefix = match subject {
      Subject::New => NEW,
      Subject::Edit(_) => EDIT,
    };
    if self.name.is_empty() {
      prefix.to_owned()
    } else {
      format!("{prefix} · {}", self.name)
    }
  }

  /// Replace the draft's operation with `kind`'s defaults, keeping a name the operator typed.
  ///
  /// The name follows the kind only while it is still the previous kind's default one: a name somebody chose is
  /// theirs, and a `Profile 1` that became a pocket toolpath should say `Pocket 1` rather than name a shape it
  /// no longer cuts. **Except while editing**, where the name is the document's own reading and `Generate`
  /// cannot change it (`cam-app` has no rename), so the draft must not invent one — the editor would show a name
  /// the document does not have (R2-001 in this task's review).
  ///
  /// Pressing the kind the draft already is does nothing at all: re-defaulting on every press silently threw
  /// away the numbers an operator had typed into `CUT` and the quick block, and the frame still reported the
  /// kind as freshly chosen (R3-001).
  pub(crate) fn choose_kind(&mut self, kind: Kind) {
    if kind_of(&self.operation) == Some(kind) {
      return;
    }
    let editing = matches!(self.seeded, Some(Subject::Edit(_)));
    let previous = default_name(&self.operation);
    if !editing && self.name == previous {
      self.name = default_name(&kind.default_operation());
    }
    self.operation = kind.default_operation();
  }
}

/// The default name of a strategy of this operation: the kind's word and `1`.
///
/// The reference names a strategy after what it does (`Desbaste 3D 1`, `Perfil 1`), which is a name an operator
/// can read back without opening it.
fn default_name(operation: &Operation) -> String {
  format!("{} 1", kind_word(operation))
}

/// The word the shell uses for an operation: the model's own variant name, lowercased where it is a sentence.
fn kind_word(operation: &Operation) -> &'static str {
  match operation {
    Operation::Profile(_) => "Profile",
    Operation::Pocket(_) => "Pocket",
    Operation::Drilling(_) => "Drilling",
    Operation::Chamfer(_) => "Chamfer",
    Operation::VCarve(_) => "V-carve",
    Operation::Engrave(_) => "Engrave",
    Operation::Fluting(_) => "Fluting",
    Operation::Texture(_) => "Texture",
    Operation::Inlay(_) => "Inlay",
    Operation::Thread(_) => "Thread",
  }
}

/// The two operations this shell can draft, as the image cards offer them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
  /// One pass along the vector.
  Profile,
  /// A cleared region inside it.
  Pocket,
}

impl Kind {
  /// Both kinds, in the order the cards are drawn.
  pub(crate) const ALL: [Kind; 2] = [Kind::Profile, Kind::Pocket];

  /// The word on the card.
  pub(crate) fn word(self) -> &'static str {
    match self {
      Self::Profile => "Profile",
      Self::Pocket => "Pocket",
    }
  }

  /// The operation a new draft of this kind starts from.
  pub(crate) fn default_operation(self) -> Operation {
    match self {
      Self::Profile => Operation::Profile(ProfileParams::default()),
      Self::Pocket => Operation::Pocket(PocketParams::default()),
    }
  }

  /// The picture the card carries.
  fn art(self) -> Art {
    match self {
      Self::Profile => Art::Profile,
      Self::Pocket => Art::Pocket,
    }
  }
}

/// The kind an operation is, or `None` for one this shell does not draft.
pub(crate) fn kind_of(operation: &Operation) -> Option<Kind> {
  match operation {
    Operation::Profile(_) => Some(Kind::Profile),
    Operation::Pocket(_) => Some(Kind::Pocket),
    _ => None,
  }
}

/// One number the editor edits, in the draft's operation.
///
/// One table and two readers: [`quick_fields`] and [`cut_fields`] are subsets of the same enum, so the quick
/// block cannot show a number the `CUT` section reads differently — the point of a quick edit is that it is the
/// *same* number, one scroll closer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CutField {
  /// Depth of material removed per pass.
  PassDepth,
  /// Material left on the wall.
  Allowance,
  /// Distance between adjacent rings or raster rows, on a pocket.
  Stepover,
  /// Depth at which cutting starts.
  StartDepth,
  /// Total depth below the stock surface.
  CutDepth,
}

impl CutField {
  /// The word the row writes before the number.
  pub(crate) fn label(self) -> &'static str {
    match self {
      Self::PassDepth => "Pass depth",
      Self::Allowance => "Allowance",
      Self::Stepover => "Stepover",
      Self::StartDepth => "Start depth",
      Self::CutDepth => "Cut depth",
    }
  }

  /// This number in `operation`, or `None` when the operation is of a kind that has no such number.
  pub(crate) fn value(self, operation: &Operation) -> Option<Length> {
    match operation {
      Operation::Profile(params) => Some(match self {
        Self::PassDepth => params.pass_depth,
        Self::Allowance => params.allowance,
        Self::StartDepth => params.start_depth,
        Self::CutDepth => params.cut_depth,
        Self::Stepover => return None,
      }),
      Operation::Pocket(params) => Some(match self {
        Self::PassDepth => params.pass_depth,
        Self::Allowance => params.allowance,
        Self::Stepover => params.stepover,
        Self::StartDepth => params.start_depth,
        Self::CutDepth => params.cut_depth,
      }),
      _ => None,
    }
  }

  /// Write this number into `operation`, ignoring a kind that has no such number.
  fn set(self, operation: &mut Operation, value: Length) {
    match operation {
      Operation::Profile(params) => match self {
        Self::PassDepth => params.pass_depth = value,
        Self::Allowance => params.allowance = value,
        Self::StartDepth => params.start_depth = value,
        Self::CutDepth => params.cut_depth = value,
        Self::Stepover => {}
      },
      Operation::Pocket(params) => match self {
        Self::PassDepth => params.pass_depth = value,
        Self::Allowance => params.allowance = value,
        Self::Stepover => params.stepover = value,
        Self::StartDepth => params.start_depth = value,
        Self::CutDepth => params.cut_depth = value,
      },
      _ => {}
    }
  }
}

/// The numbers the `CUT` section draws, in its order.
pub(crate) fn cut_fields(operation: &Operation) -> &'static [CutField] {
  const PROFILE: [CutField; 4] = [
    CutField::CutDepth,
    CutField::PassDepth,
    CutField::StartDepth,
    CutField::Allowance,
  ];
  const POCKET: [CutField; 5] = [
    CutField::CutDepth,
    CutField::PassDepth,
    CutField::Stepover,
    CutField::StartDepth,
    CutField::Allowance,
  ];
  match operation {
    Operation::Profile(_) => &PROFILE,
    Operation::Pocket(_) => &POCKET,
    _ => &[],
  }
}

/// The numbers the quick block duplicates: the ones an operator changes most.
///
/// The reference's `Edição rápida dos parâmetros` holds the pass depth and, on a pocket, the stepover, above the
/// full section that named them first. Both lists read [`CutField`], so the quick block cannot drift from the
/// section it shortcuts.
pub(crate) fn quick_fields(operation: &Operation) -> &'static [CutField] {
  const PROFILE: [CutField; 1] = [CutField::PassDepth];
  const POCKET: [CutField; 2] = [CutField::PassDepth, CutField::Stepover];
  match operation {
    Operation::Profile(_) => &PROFILE,
    Operation::Pocket(_) => &POCKET,
    _ => &[],
  }
}

/// A pocket's clearing pattern, as the cards offer it.
pub(crate) fn clearing(operation: &Operation) -> Option<PocketStrategy> {
  match operation {
    Operation::Pocket(params) => Some(params.strategy),
    _ => None,
  }
}

/// Why [`GENERATE`] cannot be honoured right now, in the operator's order, or `None` when it can.
///
/// The button is not drawn while one is returned (L6, L14): a Generate that emits nothing because the draft is
/// incomplete would be the dead button the tool library's footer refused to be. Each sentence names the thing
/// that is missing, and the last one is the model's: a document whose toolpath is another operation has no form
/// here, so the sentence names the operation and the route that authors it.
pub(crate) fn refusal(app: &AppState, draft: &Draft, subject: &Subject) -> Option<String> {
  if kind_of(&draft.operation).is_none() {
    return Some(NOT_DRAFTED.replace("{operation}", kind_word(&draft.operation)));
  }
  if draft.name.trim().is_empty() {
    return Some(NO_NAME.to_owned());
  }
  if let Subject::New = subject {
    if draft.tool_number.is_none() {
      return Some(NO_TOOL.to_owned());
    }
    if app.selection().is_empty() {
      return Some(NO_REGION.to_owned());
    }
  }
  None
}

/// The action [`GENERATE`] pushes, or `None` when [`refusal`] has something to say instead.
///
/// A new strategy is [`UiAction::AddToolpath`]; an existing one is [`UiAction::SetToolpathOperation`], which
/// rewrites the operation and leaves the geometry, the tool and the name alone. What the shell cannot see from
/// here — an open vector under a cutting operation, say — is the document's own rule, refused by the command
/// with its sentence on the status bar.
pub(crate) fn generate(app: &AppState, draft: &Draft, subject: &Subject) -> Option<UiAction> {
  if refusal(app, draft, subject).is_some() {
    return None;
  }
  match subject {
    Subject::Edit(toolpath_id) => Some(UiAction::SetToolpathOperation {
      toolpath_id: toolpath_id.clone(),
      operation: draft.operation.clone(),
    }),
    Subject::New => Some(UiAction::AddToolpath {
      toolpath_id: free_toolpath_id(app),
      name: draft.name.trim().to_owned(),
      entity_ids: app.selection().ids().map(str::to_owned).collect(),
      island_ids: Vec::new(),
      tool_number: draft.tool_number?,
      operation: draft.operation.clone(),
    }),
  }
}

/// The next free `TPn` id, by the same scan `cam-cli toolpath` performs.
///
/// `cam-app` takes the id from its caller because the document carries no toolpath counter
/// ([`cam_app::Command::AddToolpath`]), and the CLI's rule is `len + 1` upwards until a free one is found. It is
/// repeated here rather than imported because the CLI is a binary crate this one cannot depend on; the rule is
/// the whole of it, and a second spelling would be a second answer to "what is the next id".
fn free_toolpath_id(app: &AppState) -> String {
  let toolpaths = &app.document().toolpaths;
  let mut number = toolpaths.len() + 1;
  loop {
    let id = format!("TP{number}");
    if !toolpaths.iter().any(|toolpath| toolpath.id == id) {
      return id;
    }
    number += 1;
  }
}

/// The picture a card carries.
///
/// Four diagrams, each painted from the operation's own semantics rather than pasted: a contour with the tool
/// beside it, a region filled with rings, and the two clearing patterns a pocket can use. They are the shell's
/// own art the way [`crate::tool_library`]'s scaled tool drawing is — see this module's header for what a real
/// thumbnail would need.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Art {
  /// A vector and the tool running along its outside.
  Profile,
  /// A region cleared with concentric rings.
  Pocket,
  /// The same region, cleared with rings.
  Offset,
  /// The same region, cleared with parallel rows.
  Raster,
}

impl Shell {
  /// The strategy editor (S10): one surface for creating and editing.
  ///
  /// Called from the `TOOLPATHS` task body, which is the surface the editor has in this shell: the header above
  /// it carries [`Draft::header`], and the current-tool card, the quick block, the `CUT` section, the note and
  /// the footer are drawn here in the spec's order. Every control that cannot act is replaced by the sentence
  /// that says why, and every reported point is a point this frame really drew.
  pub(crate) fn strategy_editor(
    &mut self,
    app: &AppState,
    ui: &mut egui::Ui,
    palette: &Palette,
    output: &mut ShellOutput,
  ) {
    let metrics = self.metrics();
    let subject = subject(app);
    // The editor is drawn inside the inspector's scroll area, and a row below its viewport is painted by egui and
    // clipped away. Reporting its centre would be a promise about a control nobody can press — the defect L23
    // found in the playback bar — so every point this frame reports is filtered against the clip it drew in.
    let clip = ui.clip_rect();

    // 1. the name. A field while the toolpath is being created, a reading with its reason while it is being
    //    edited: `cam-app` has no rename command, and a field that dropped what was typed would be a lie.
    ui.label(
      RichText::new(NAME)
        .size(theme::CAPTION)
        .color(palette.get(Role::TextDim)),
    );
    match &subject {
      Subject::New => {
        let field = ui.add(
          egui::TextEdit::singleline(&mut self.strategy.name)
            .hint_text("the toolpath's name")
            .desired_width(f32::INFINITY),
        );
        let centre = field.rect.center();
        output
          .drawn
          .strategy_hits
          .push((NAME.to_owned(), (centre.x, centre.y)));
      }
      Subject::Edit(_) => {
        ui.label(
          RichText::new(&self.strategy.name)
            .size(theme::TITLE)
            .color(palette.get(Role::TextHi)),
        );
        ui.label(
          RichText::new(RENAME)
            .size(theme::CAPTION)
            .color(palette.get(Role::TextDim)),
        );
      }
    }
    ui.add_space(metrics.space(2));

    // 2. the current-tool card, with its own opener and this frame's number. While editing, the number is the
    //    document's: the modal writes the document, so a draft copy could only disagree with the card.
    let name = self.strategy.name.clone();
    let (number, estimate) = match &subject {
      Subject::Edit(_) => (
        app
          .document()
          .toolpaths
          .first()
          .map(|toolpath| toolpath.tool_number),
        true,
      ),
      Subject::New => (self.strategy.tool_number, false),
    };
    self.tool_card_for(app, ui, palette, output, &name, number, estimate);

    // 3. the quick block: the same numbers the section below holds, one click closer.
    let quick = chrome::section_toggle(ui, palette, QUICK, self.strategy.quick_open);
    let centre = quick.rect.center();
    output
      .drawn
      .strategy_hits
      .push((QUICK.to_owned(), (centre.x, centre.y)));
    if quick.clicked() {
      self.strategy.quick_open = !self.strategy.quick_open;
    }
    output.drawn.strategy_quick_open = self.strategy.quick_open;
    if self.strategy.quick_open {
      self.quick_block(app, ui, palette, output);
    }

    // 4. the strategy's own sections.
    chrome::section(ui, palette, CUT);
    self.kind_cards(ui, palette, &metrics, output);
    self.region_line(app, ui, palette);
    for field in cut_fields(&self.strategy.operation) {
      self.number_row(ui, palette, *field, &mut output.drawn.strategy_fields);
    }
    if kind_of(&self.strategy.operation) == Some(Kind::Pocket) {
      self.clearing_cards(ui, palette, &metrics, output);
    }

    // 5. the note, in the spec's own words.
    ui.add_space(metrics.space(2));
    ui.label(
      RichText::new(NOTE)
        .size(theme::CAPTION)
        .color(palette.get(Role::TextDim)),
    );
    output.drawn.strategy_note = NOTE.to_owned();
    ui.add_space(metrics.space(2));

    // 6. the footer. `Cancel` can always be honoured because nothing here has written anything; `Generate
    //    toolpath` is drawn only when the action it names exists, and the sentence takes its place otherwise.
    let refusal = refusal(app, &self.strategy, &subject);
    if let Some(sentence) = &refusal {
      ui.label(
        RichText::new(sentence)
          .size(theme::CAPTION)
          .color(palette.get(Role::Danger)),
      );
      output.drawn.strategy_refusal = sentence.clone();
    }
    ui.horizontal(|ui| {
      if refusal.is_none() {
        let generate_button = ui.button(RichText::new(GENERATE).size(theme::SMALL));
        let centre = generate_button.rect.center();
        output
          .drawn
          .strategy_hits
          .push((GENERATE.to_owned(), (centre.x, centre.y)));
        if generate_button.clicked() {
          if let Some(action) = generate(app, &self.strategy, &subject) {
            output.actions.push(action);
          }
        }
      }
      let back = ui.button(RichText::new(CANCEL).size(theme::SMALL));
      let centre = back.rect.center();
      output
        .drawn
        .strategy_hits
        .push((CANCEL.to_owned(), (centre.x, centre.y)));
      if back.clicked() {
        self.close_task(true);
      }
    });

    output
      .drawn
      .strategy_hits
      .retain(|(_, (x, y))| clip.contains(egui::pos2(*x, *y)));
  }

  /// The quick block's rows: the operation's most-changed numbers, then the tool's rates as readings.
  fn quick_block(
    &mut self,
    app: &AppState,
    ui: &mut egui::Ui,
    palette: &Palette,
    output: &mut ShellOutput,
  ) {
    for field in quick_fields(&self.strategy.operation) {
      self.number_row(ui, palette, *field, &mut output.drawn.strategy_quick_fields);
    }
    // The rates are the tool's, not the operation's: the same rows the library's `SPEEDS` group draws, in the
    // units that library authored them in (L25). Nothing here can change them — `cam-store` owns the writer —
    // so they are drawn as readings above the sentence that says where they live.
    let number = match subject(app) {
      Subject::Edit(_) => app
        .document()
        .toolpaths
        .first()
        .map(|toolpath| toolpath.tool_number),
      Subject::New => self.strategy.tool_number,
    };
    match number.and_then(|number| stored_tool_for_number(&self.tools, number)) {
      Some(tool) => {
        let rates =
          crate::tool_library::form_fields(crate::tool_library::ToolFormGroup::Speeds, tool);
        // One row per rate, in the block's own shape: a word at the left and its value at the right. The
        // library's boxed field widget is not used here because three fixed-width boxes in a wrapping row
        // measure wider than the panel on the frame that has not been laid out yet, and a child wider than its
        // panel drags the whole frame's layout out with it (the panel then reports a content width the shell
        // never intended and the next frame draws the surface off the window).
        for (label, value) in &rates {
          ui.horizontal(|ui| {
            ui.label(
              RichText::new(label)
                .size(theme::CAPTION)
                .color(palette.get(Role::TextDim)),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
              ui.label(
                RichText::new(value)
                  .size(theme::SMALL)
                  .color(palette.get(Role::TextHi)),
              );
            });
          });
        }
        if !rates.is_empty() {
          ui.label(
            RichText::new(RATES)
              .size(theme::CAPTION)
              .color(palette.get(Role::TextDim)),
          );
        }
      }
      // A missing tool is said by the card above, in its own sentence: a second one here would be the same
      // fact twice.
      None => {}
    }
  }

  /// One number row: its word, the number the draft holds, and the unit.
  ///
  /// The widget writes straight into the draft's operation, and the request that draws it also reports the row,
  /// so the `CUT` section and the quick block cannot show two different values for one number.
  fn number_row(
    &mut self,
    ui: &mut egui::Ui,
    palette: &Palette,
    field: CutField,
    report: &mut Vec<(String, String)>,
  ) {
    let Some(value) = field.value(&self.strategy.operation) else {
      return;
    };
    let mut millimetres = value.mm();
    ui.horizontal(|ui| {
      ui.label(
        RichText::new(field.label())
          .size(theme::CAPTION)
          .color(palette.get(Role::TextDim)),
      );
      ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let response = ui.add(
          egui::DragValue::new(&mut millimetres)
            .speed(0.5)
            .suffix(" mm"),
        );
        if response.changed() {
          field.set(&mut self.strategy.operation, Length::from_mm(millimetres));
        }
      });
    });
    report.push((
      field.label().to_owned(),
      format!("{} mm", Format::fixed(2).apply(millimetres)),
    ));
  }

  /// The two kind cards: the strategy's own shape, which is a choice that needs a picture.
  fn kind_cards(
    &mut self,
    ui: &mut egui::Ui,
    palette: &Palette,
    metrics: &Metrics,
    output: &mut ShellOutput,
  ) {
    let current = kind_of(&self.strategy.operation);
    let cards: Vec<(String, Art, bool)> = Kind::ALL
      .iter()
      .map(|kind| (kind.word().to_owned(), kind.art(), current == Some(*kind)))
      .collect();
    let mut drawn: Vec<(String, bool, usize)> = Vec::new();
    let pressed = image_cards(
      ui,
      palette,
      metrics,
      &cards,
      &mut drawn,
      &mut output.drawn.strategy_hits,
    );
    if let Some(index) = pressed {
      self.strategy.choose_kind(Kind::ALL[index]);
    }
    // Reported after the press, so the frame that chooses a kind is the frame that reports it chosen (L13).
    let now = kind_of(&self.strategy.operation);
    for (word, _, strokes) in drawn {
      let kind = Kind::ALL.iter().find(|kind| kind.word() == word).copied();
      output
        .drawn
        .strategy_cards
        .push((word, kind.is_some() && kind == now, strokes));
    }
  }

  /// The clearing pattern's two cards, on a pocket only: the other choice that needs a picture.
  fn clearing_cards(
    &mut self,
    ui: &mut egui::Ui,
    palette: &Palette,
    metrics: &Metrics,
    output: &mut ShellOutput,
  ) {
    let current = clearing(&self.strategy.operation);
    let cards = vec![
      (
        "Offset".to_owned(),
        Art::Offset,
        current == Some(PocketStrategy::Offset),
      ),
      (
        "Raster".to_owned(),
        Art::Raster,
        current == Some(PocketStrategy::Raster),
      ),
    ];
    let mut drawn: Vec<(String, bool, usize)> = Vec::new();
    let pressed = image_cards(
      ui,
      palette,
      metrics,
      &cards,
      &mut drawn,
      &mut output.drawn.strategy_hits,
    );
    if let Some(index) = pressed {
      if let Operation::Pocket(params) = &mut self.strategy.operation {
        params.strategy = if index == 0 {
          PocketStrategy::Offset
        } else {
          PocketStrategy::Raster
        };
      }
    }
    let now = clearing(&self.strategy.operation);
    for (word, _, strokes) in drawn {
      let chosen = match (word.as_str(), now) {
        ("Offset", Some(PocketStrategy::Offset)) => true,
        ("Raster", Some(PocketStrategy::Raster)) => true,
        _ => false,
      };
      output.drawn.strategy_cards.push((word, chosen, strokes));
    }
  }

  /// The sentence that says what the strategy's region is, in the reference's own place.
  ///
  /// It is the honest replacement for the reference's frontier row: a strategy's region is its `entity_ids`,
  /// which this shell chooses on the canvas, so the editor says how many vectors it holds instead of offering a
  /// choice between three pictures of one set.
  fn region_line(&self, app: &AppState, ui: &mut egui::Ui, palette: &Palette) {
    let count = match subject(app) {
      Subject::Edit(_) => app
        .document()
        .toolpaths
        .first()
        .map(|toolpath| toolpath.entity_ids.len())
        .unwrap_or_default(),
      Subject::New => app.selection().ids().count(),
    };
    let sentence = format!(
      "{count} vector(s) form this strategy's region — the vectors are chosen on the canvas, so this shell \
       offers no frontier to choose here."
    );
    ui.label(
      RichText::new(&sentence)
        .size(theme::CAPTION)
        .color(palette.get(Role::TextDim)),
    );
  }
}

/// One row of image cards: the choice a picture makes shorter than a sentence.
///
/// Each card is a picture and its word. The picture is painted from [`Art`], the card is filled and stroked by
/// the palette's own roles, and the number of strokes it painted is reported beside its word so a test can tell
/// a card with a diagram from an empty box (L23: a headless test reads shapes, and a box is a shape).
fn image_cards(
  ui: &mut egui::Ui,
  palette: &Palette,
  metrics: &Metrics,
  cards: &[(String, Art, bool)],
  drawn: &mut Vec<(String, bool, usize)>,
  hits: &mut Vec<(String, (f32, f32))>,
) -> Option<usize> {
  let mut pressed = None;
  let gap = metrics.space(1);
  let width = ((ui.available_width() - gap * (cards.len().saturating_sub(1) as f32))
    / cards.len().max(1) as f32)
    .max(40.0);
  let height = 58.0;
  ui.horizontal(|ui| {
    ui.spacing_mut().item_spacing.x = gap;
    for (index, (word, art, selected)) in cards.iter().enumerate() {
      let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), Sense::click());
      let radius = egui::CornerRadius::same(metrics.radius_control as u8);
      let fill = if *selected {
        palette.get(Role::SelSoft)
      } else if response.hovered() {
        palette.get(Role::Raised)
      } else {
        palette.get(Role::Panel)
      };
      ui.painter().rect_filled(rect, radius, fill);
      ui.painter().rect_stroke(
        rect,
        radius,
        egui::Stroke::new(
          if *selected { 1.5 } else { 1.0 },
          palette.get(if *selected { Role::Sel } else { Role::Border }),
        ),
        egui::StrokeKind::Inside,
      );
      let picture = egui::Rect::from_min_size(
        rect.min + egui::vec2(6.0, 5.0),
        egui::vec2(rect.width() - 12.0, height - 26.0),
      );
      let strokes = paint_art(ui.painter(), picture, palette, *art);
      ui.painter().text(
        egui::pos2(rect.left() + 6.0, rect.bottom() - 5.0),
        egui::Align2::LEFT_BOTTOM,
        word,
        egui::FontId::proportional(theme::SMALL),
        palette.get(if *selected { Role::TextHi } else { Role::Text }),
      );
      let centre = rect.center();
      hits.push((word.clone(), (centre.x, centre.y)));
      if response.clicked() {
        pressed = Some(index);
      }
      drawn.push((word.clone(), *selected, strokes));
    }
  });
  pressed
}

/// Paint one card's diagram and report how many strokes it took.
fn paint_art(painter: &egui::Painter, rect: egui::Rect, palette: &Palette, art: Art) -> usize {
  let ink = palette.get(Role::Text);
  let cut = palette.get(Role::Cut);
  let hair = egui::Stroke::new(1.0, ink);
  let cut_stroke = egui::Stroke::new(1.2, cut);
  let mut strokes = 0;
  let ring = |rect: egui::Rect, stroke: egui::Stroke, strokes: &mut usize| {
    painter.rect_stroke(
      rect,
      egui::CornerRadius::same(2),
      stroke,
      egui::StrokeKind::Inside,
    );
    *strokes += 1;
  };
  match art {
    Art::Profile => {
      // The vector, then the tool's own circle sitting on a pass beside it: a profile cuts along the shape, and
      // the picture says "one line, with the bit on it" rather than "a region".
      let vector = rect.shrink(6.0);
      ring(vector, hair, &mut strokes);
      let pass = vector.expand(4.0);
      ring(pass, cut_stroke, &mut strokes);
      let tip = egui::pos2(pass.left() + pass.width() * 0.25, pass.bottom());
      painter.circle_filled(tip, 3.0, palette.get(Role::TextHi));
    }
    Art::Pocket => {
      // The boundary and the rings a clearing pass leaves inside it.
      let mut inside = rect.shrink(5.0);
      ring(inside, hair, &mut strokes);
      for _ in 0..3 {
        inside = inside.shrink(4.0);
        if inside.width() <= 4.0 || inside.height() <= 4.0 {
          break;
        }
        ring(inside, cut_stroke, &mut strokes);
      }
    }
    Art::Offset => {
      // The same rings, without the pocket's boundary stroke: the pattern itself.
      let mut inside = rect.shrink(3.0);
      for _ in 0..4 {
        ring(inside, cut_stroke, &mut strokes);
        inside = inside.shrink(4.0);
        if inside.width() <= 4.0 || inside.height() <= 4.0 {
          break;
        }
      }
    }
    Art::Raster => {
      // Parallel rows: the other clearing pattern, drawn as the rows it cuts.
      let rows = 4;
      let step = rect.height() / (rows + 1) as f32;
      for row in 1..=rows {
        let y = rect.top() + step * row as f32;
        painter.line_segment(
          [
            egui::pos2(rect.left() + 3.0, y),
            egui::pos2(rect.right() - 3.0, y),
          ],
          cut_stroke,
        );
        strokes += 1;
      }
    }
  }
  strokes
}
