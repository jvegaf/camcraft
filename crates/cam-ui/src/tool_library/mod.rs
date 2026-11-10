//! The tool library as a master-detail modal (tool-library-editor S1-S14).
//!
//! One screen, in three columns and a band: a header carrying the dialog's title and a close cross; a master
//! column of a search field over a tree grouped by the tool's kind, each heading carrying its count and folding
//! with a caret; a detail column of the editable form; a preview column of the tool drawn to scale inside a dark
//! card; and a footer of one sentence above `Cancel` and `Use this tool` (S1, S2, S8).
//!
//! Four decisions this module makes, each of them because the alternative would be a control that lies
//! (the rule L6, L14 and L22 established and the T6 review enforced, L23):
//!
//! 1. **A row is grouped by the kind the record stores, and drawn by the shape its geometry proves.** The two
//!    vocabularies are deliberately separate: [`cam_store::tools::ToolKind`] is what the operator called the
//!    tool and what the library keeps, and [`ToolFamily`] is what can be painted. Six kinds collapse to four
//!    shapes because two pairs paint identically — an engraving cutter ends flat like an end mill, and a drill
//!    ends in a cone to a point like a V-bit whose flat is zero — and the tree groups by the kind, because the
//!    kind is what the operator typed in the combo and expects to find the tool under.
//! 2. **Every group of the form is drawn, whether or not the record fills it.** That reverses an earlier rule,
//!    and the reversal is the point of this screen: a box is not a claim about a tool, it is a question, and a
//!    tool with no feeds is exactly the tool an operator opens this form to give feeds to.
//! 3. **Every keystroke edits a draft, and only `Use this tool` edits the library.** The boxes pass through
//!    states that are not tools — `6.` on the way to `6.35` — so a form that wrote as it was typed would refuse
//!    a tool mid-word. [`ToolDraft`] holds the text and [`ToolDraft::commit`] turns it back into a `StoredTool`
//!    through the store's own validator, which is why the sentence a disabled button shows and the sentence the
//!    store would refuse with cannot drift apart. A draft belongs to the row it was seeded from, so leaving that
//!    row discards it.
//! 4. **`Use this tool` is drawn only when it can be honoured, and the reason is drawn when it cannot.** The
//!    modal writes one tool and one number: the tool into the **application's** library file, and the number
//!    into the toolpath that opened the dialog (or into the strategy draft, when it was opened for one). A tool
//!    the library gives no number, a toolpath that has left the document, an empty library and a draft whose
//!    boxes do not describe a tool are all states in which that write has no meaning, so the button is replaced
//!    by the sentence that says so. `Cancel` is always honourable for one reason worth writing down: nothing in
//!    this modal writes before it, so there is nothing for it to undo.
//!
//! The master column ends in S3's three rows of library actions — `+ New`, `Duplicate`, `Delete`; `Export…`,
//! `Import…`; `Restore defaults` — and they are the one place in this modal where a control is drawn while it
//! cannot be honoured. The library is the **application's** ([`library::app_data_dir`], tool-library-app S1), so
//! the only state that darkens the four rows which write is a library the application could not open; the rule
//! above is then kept by the report and by one sentence rather than by absence: a dark row reports no point in
//! [`crate::DrawnState::tool_library_hits`], and the sentence under the rows is the failure's own words.
//! `Export…` and `Import…` are never dark, because they need a file dialog and not a library, and their write
//! happens when the dialog's path comes back as a [`crate::UiAction`].
//!
//! **Every library write is the file first and the window's snapshot second.** The two can then never disagree in
//! the direction that matters — a library that refused a tool is never described by a tree that lists it — and a
//! refusal stays in this modal ([`Shell::tool_error`]) with the snapshot untouched, which is what makes
//! `New`, `Duplicate`, `Delete`, `Restore defaults` and `Use this tool` one rule instead of five.
//!
//! The preview is the tool's cutting end drawn vertically inside its own card, and its **horizontal scale is
//! shared by every tool at once** — [`preview::SPAN_MM`] of tool is the card's inner width — so two tools in it
//! are comparable by eye. The length and the diameter are painted as dimension callouts only when the record
//! holds them, because a dimension the library does not have is a dimension the shell invented.

use cam_store::tools::StoredTool;

mod dialog;
mod draft;
mod family;
mod form;
mod library;
mod preview;
mod shell;
mod tree;

pub use draft::ToolDraft;
pub use family::{ShapeField, ToolFamily};
pub use form::{form_rows, FormField, ToolFormGroup};
pub use library::{app_data_dir, library_path_in, LIBRARY_FILE};
pub use tree::{cut_line, tool_title};

/// One group's rows for a tool, as `(label, text)` pairs.
///
/// The reading the strategy editor's quick block takes of the library's own rates, and kept beside
/// [`form_rows`] because the two are the same rows in two shapes: the block wants one group's labels and values
/// and nothing about boxes, while the modal wants every group with the fields it has to draw. Written over
/// [`form_rows`] rather than over the record, so the block and the form cannot list different rows for one tool,
/// and the unit is appended because the block's own note promises the rates are shown in the units the library
/// authored them in.
pub fn form_fields(group: ToolFormGroup, tool: &StoredTool) -> Vec<(String, String)> {
  form_rows(tool)
    .into_iter()
    .filter(|(name, _)| *name == group)
    .flat_map(|(_, fields)| fields)
    .map(|field| {
      let text = field.value(tool);
      let shown = match field.unit(tool.cutting.rate_units) {
        Some(unit) if !text.is_empty() => format!("{text} {unit}"),
        _ => text,
      };
      (field.label().to_owned(), shown)
    })
    .collect()
}
