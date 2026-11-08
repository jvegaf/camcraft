//! The shell's frame: the app bar, the rail, the canvas and the inspector.
//!
//! `design/prancheta-reference/` is the visual spec: fifteen full-resolution frames of the reference app,
//! each annotated in that directory's `README.md` with the pattern it settles. What this module implements
//! is the *structure* those frames argue for, mapped onto the widgets the crate already has rather than
//! copied pixel for pixel:
//!
//! ```text
//! ┌───────────────────────────────────────────────────────────┐  app bar    40 px
//! │ mark  campilot   ↶ ↷   untitled      mm ☾ ? fit [G-code]  │
//! ├────┬───────────────────────────────────────┬──────────────┤
//! │ 52 │                                       │ inspector    │
//! │ px │              canvas                   │ 288 px       │
//! │    │                                       │ (collapsible)│
//! ├────┴───────────────────────────────────────┴──────────────┤  status bar 26 px
//! ```
//!
//! **There is no stage bar, and removing it was a correction, not a preference.** An earlier version of this
//! frame carried a `Design › Edit › Relief › Toolpaths › Prove` strip, drawn from eight frames of the
//! reference app that had been photographed off two videos. The fifteen full-resolution frames show no such
//! strip in any state: the reference carries its workflow in the rail and in whatever task the right panel has
//! open. The strip is gone, and with it the stage enum that fed it (camui-prancheta S2).
//!
//! **The rail is not stage-scoped.** Sixty-odd commands do not fit on one screen and never did, but the
//! reference's answer is not to hide them behind a mode: it labels them, groups them, and lets the operator
//! collapse a group. What a command can do right now is decided by the document — the rule [`Command`] already
//! applies when it greys a chip — and never by a mode the operator has to remember they are in.
//!
//! **Counts are counted, never invented.** The frame states a number only when the document can prove it:
//! vectors on the sheet, toolpaths in the document. A section that would otherwise invent a count shows the
//! sentence naming the step that produces it instead — never `0 problems` for a validation nobody runs.

use crate::icons::Icon;

mod app_bar;
mod job;
mod layers;
mod overlays;
mod paint;
mod palette;
mod ribbon;
mod status;

pub(crate) use self::paint::{
  chip, dimension_line, field, paint_segmented, rect_report, section, section_toggle,
};
pub use self::palette::palette_tooltip;
pub(crate) use self::palette::trim_action;
pub(crate) use self::ribbon::tool_name;

/// Which inspector tab is showing.
///
/// Three tabs, and not the artboard's three: our document has no toolpath *parameters* to show yet (a
/// toolpath is authored by `cam-cli` and arrives finished), so the middle tab holds the authoring
/// commands that produce the numbers the Selection tab then reports.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
  /// The commands that make and change geometry.
  #[default]
  Tools,
  /// What the current selection is, in numbers.
  Selection,
  /// Stock, thickness, zero.
  Job,
}

impl Tab {
  /// Every tab, left to right.
  pub const ALL: [Tab; 3] = [Tab::Tools, Tab::Selection, Tab::Job];

  /// The word on the tab.
  pub fn label(self) -> &'static str {
    match self {
      Self::Tools => "Tools",
      Self::Selection => "Selection",
      Self::Job => "Job",
    }
  }

  /// The ribbon category that opens this tab's content, if any.
  ///
  /// The right panel is not a tabbed inspector any more (camui-prancheta S6): it is a ribbon of three
  /// semantic categories, and opening one puts its task editor over the panel. `Selection` is the odd one
  /// out on purpose — the reference draws no selection category, because the selection's numbers are the
  /// panel's *idle* state rather than a task anybody opens — so it maps to **no** category and is drawn when
  /// no task is open.
  pub fn category(self) -> Option<Category> {
    match self {
      Self::Tools => Some(Category::Toolpaths),
      Self::Selection => None,
      Self::Job => Some(Category::MachineOutput),
    }
  }
}

/// A category of the right panel's ribbon: the three the reference draws, in its order (camui-prancheta S6).
///
/// Glyph only, one row per category. Opening one puts that category's **task editor** over the panel's body,
/// which is what replaced the three named tabs: the words moved into the task editor's header, where they name
/// the task instead of a tab nobody could tell apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
  /// What makes and describes toolpaths: the authoring commands, the tool card, the estimate.
  Toolpaths,
  /// The 3D half of the surface: whether it is split, and what it is playing.
  Relief3d,
  /// Stock, material, zero, and the program that leaves the machine.
  MachineOutput,
}

impl Category {
  /// Every category, top to bottom — the reference's order.
  pub const ALL: [Category; 3] = [
    Category::Toolpaths,
    Category::Relief3d,
    Category::MachineOutput,
  ];

  /// The words the task editor's header carries while this category is open.
  ///
  /// English, like every other word in the shell (S3): the reference is pt-BR and what we take from it is
  /// structure, never its strings.
  pub fn label(self) -> &'static str {
    match self {
      Self::Toolpaths => "TOOLPATHS",
      Self::Relief3d => "RELIEF AND 3D PART",
      Self::MachineOutput => "MACHINE AND OUTPUT",
    }
  }

  /// The glyph the ribbon draws for this category.
  ///
  /// Three glyphs the set already had: the profile toolpath, the relief, and the post-processor. No new
  /// symbol was drawn for this, because a glyph nobody has seen before is a legend, not an icon.
  pub fn icon(self) -> Icon {
    match self {
      Self::Toolpaths => Icon::ToolpathProfile,
      Self::Relief3d => Icon::Relief,
      Self::MachineOutput => Icon::PostProcessor,
    }
  }
}

/// The subject a tool card reads: the word its heading carries, the number the library is asked for, and
/// whether a program exists to measure.
///
/// One argument instead of three because the card's own doc already treats them as one subject: the name in
/// the heading, the tool that name belongs to, and whether the estimate below the card can be measured at all.
pub(crate) struct ToolSubject<'a> {
  /// The word the heading carries: the document toolpath's name, or the draft's.
  pub name: &'a str,
  /// The tool number to read out of the library, when the subject has one.
  pub number: Option<u32>,
  /// Whether a program exists to measure the cutting time over. A draft has none.
  pub estimate: bool,
}

#[cfg(test)]
mod tests {
  use crate::help::Command;

  use super::paint::command_icon;
  use super::palette_tooltip;

  #[test]
  fn shortcuts_has_a_glyph_and_a_word_of_its_own() {
    assert_ne!(
      command_icon(Command::Shortcuts),
      command_icon(Command::AddText),
      "Shortcuts and Add text draw the same glyph, so the two chips are the same picture"
    );
    assert!(
      palette_tooltip(Command::Shortcuts).contains(Command::Shortcuts.label()),
      "the Shortcuts chip's tooltip is {:?}, which never says what it is",
      palette_tooltip(Command::Shortcuts)
    );
  }
}
