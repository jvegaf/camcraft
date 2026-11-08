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
//! tool at once** — `SPAN_MM` of tool is the box's width — so two tools in the same box are comparable by
//! eye and a test can check the ratio. The vertical extent is the flute length when the record holds one; when
//! it does not, the shape is drawn to the box's own height and no flute callout is painted, because the
//! library carries no length to point at and a guessed one would be a dimension the shell does not have (the
//! same rule `view3d::paint_tool` follows about the spindle).

mod dialog;
mod drawing;
mod family;
mod form;
mod shell;
mod tree;

pub use family::{ShapeField, ToolFamily};
pub use form::{form_fields, form_groups, ToolFormGroup};
pub use tree::{cut_line, tool_title};
