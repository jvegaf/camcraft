//! A strategy's editor: the same surface for creating and editing one toolpath (camui-prancheta S10).
//!
//! S10 asks for one editor, prefixed [`NEW`] or [`EDIT`] with the strategy's name, holding its name field,
//! the current-tool card with its opener, the collapsible [`QUICK`] block, its own sections, the note
//! [`NOTE`], and the pair [`GENERATE`] / [`CANCEL`]. The reference draws it twice — `Novo percurso · Desbaste
//! 3D` (`Screenshot 2026-11-03 034023.png`) and `Editar · Perfil` (`033350.png`) — and the two frames differ in
//! the header prefix and in what the footer does, which is exactly what this module makes the difference:
//! [`subject`] reads the document, [`subject`]`::New` drafts a toolpath that does not exist yet, and
//! [`subject`]`::Edit` rewrites the operation of the one that does — and renames it when the name field holds
//! something the document does not.
//!
//! **The name is a field in both modes.** It is the draft's, and [`GENERATE`] writes it: a new toolpath is
//! created with it, and an existing one is renamed by [`cam_app::Command::SetToolpathName`] before its operation
//! is rewritten. It is written **only when the field holds a name the document does not**, because a second undo
//! step for a name nobody touched would be a lie about what the last action was. A control that cannot act is not
//! drawn (L6, L14, L22) — the rule the tool library's footer and the playback bar were both corrected for, and the
//! reason the sentence that explained a fixed name went with the limitation it described.
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
use cam_units::Length;

use crate::UiAction;

mod cards;
mod editor;
mod list;
mod quick;

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
  /// The toolpath the editor is editing, by id.
  ///
  /// The selection names it. When the operator has selected nothing — or the selection names a toolpath the
  /// document no longer holds, which is the same answer — it is the document's **first** toolpath, the rule the
  /// current-tool card already read (camui-toolpath S2, M13 T3/S4): one rule in one place, and the frame's tool
  /// card and its editor cannot describe two different toolpaths.
  Edit(String),
}

/// Which toolpath the surface puts the editor in front of: the selected one, or the document's first when the
/// selection names none the document holds.
///
/// The fallback is [`crate::resolve_toolpath`]'s, not this function's, because the card reads the same rule.
pub(crate) fn subject(app: &AppState, selection: Option<&str>) -> Subject {
  match crate::resolve_toolpath(app, selection) {
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
  /// one, and a toolpath the operator switched away from — selected another row of the list, or had the
  /// document move under it — is no longer the editor's (M13 T3, S4).
  pub(crate) fn sync(&mut self, app: &AppState, selection: Option<&str>) {
    let subject = subject(app, selection);
    if self.seeded.as_ref() == Some(&subject) {
      return;
    }
    match (crate::resolve_toolpath(app, selection), &subject) {
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
  /// This is what `Cancel` (and the header's `✕`) means: the document, its toolpath and that toolpath's name stay
  /// exactly as they were, and the draft — every number typed into it, the name field included — is thrown away, so
  /// the next frame reads the document again.
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
  /// no longer cuts. **Except while editing**, where the draft's name is the document's own: choosing a kind must
  /// not quietly rewrite a name the document keeps, because a kind is not a rename — the operator can type a new
  /// name in the field, and `Generate toolpath` is what writes it (R2-001 in this task's review).
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
    Operation::Moulding(_) => "Moulding",
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

/// The actions [`GENERATE`] pushes, in the order it pushes them, or none when [`refusal`] has something to say
/// instead.
///
/// A new strategy is [`UiAction::AddToolpath`], which carries the name with everything else it creates. An
/// existing one is renamed — only when the field holds a name the document does not — and then has its operation
/// rewritten by [`UiAction::SetToolpathOperation`], which leaves the geometry and the tool alone. **The rename is
/// omitted when the name did not change**: `Generate` over an untouched name emits exactly the operation, so the
/// history gains no step for a name nobody touched. A step that undid nothing would make undo describe an action
/// that never happened.
///
/// Two actions rather than one command because a rename and an operation change are two edits, `cam-app` has one
/// command for each, and each is its own step in the history — the same shape `Trim` already has, where one
/// gesture emits the edges it leaves behind. What the shell cannot see from here — an open vector under a cutting
/// operation, say — is the document's own rule, refused by the command with its sentence on the status bar.
pub(crate) fn generate(app: &AppState, draft: &Draft, subject: &Subject) -> Vec<UiAction> {
  if refusal(app, draft, subject).is_some() {
    return Vec::new();
  }
  match subject {
    Subject::Edit(toolpath_id) => {
      // The operation goes first, the rename second, and the order is a decision: the kernel can refuse an
      // operation, and a refused operation must leave the document exactly as it was. With the rename first, a
      // refusal would already have written a name the operator then could not explain from the failure message
      // (each command's own sentence stays true, but the pair would not). Pushed this way, the only reachable
      // failure changes nothing; a rename that fails after the operation landed cannot happen in one frame,
      // because `refusal` already refused a blank name before either action was built.
      let mut actions = vec![UiAction::SetToolpathOperation {
        toolpath_id: toolpath_id.clone(),
        operation: draft.operation.clone(),
      }];
      let name = draft.name.trim();
      let stored = app
        .document()
        .toolpaths
        .iter()
        .find(|toolpath| toolpath.id == *toolpath_id)
        .map(|toolpath| toolpath.name.as_str());
      if stored != Some(name) {
        actions.push(UiAction::SetToolpathName {
          toolpath_id: toolpath_id.clone(),
          name: name.to_owned(),
        });
      }
      actions
    }
    Subject::New => {
      // `refusal` already refused a draft with no tool, so this arm is unreachable today; it answers with no
      // action rather than with a default number, because a toolpath written with a tool the operator never
      // chose is a worse answer than none.
      let Some(tool_number) = draft.tool_number else {
        return Vec::new();
      };
      vec![UiAction::AddToolpath {
        toolpath_id: free_toolpath_id(app),
        name: draft.name.trim().to_owned(),
        entity_ids: app.selection().ids().map(str::to_owned).collect(),
        island_ids: Vec::new(),
        tool_number,
        operation: draft.operation.clone(),
      }]
    }
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
