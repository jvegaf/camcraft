//! The tool library as a master-detail modal (tool-library-editor S1-S14).
//!
//! The modal is one screen in a header band, three columns and a footer: a title and a close cross; a master
//! column of a search field over a tree grouped by the tool's kind, each heading carrying its count and folding
//! with a caret; a detail column of the editable form; a preview column of the tool drawn to scale inside a dark
//! card; and one sentence above `Cancel` and `Use this tool`.
//!
//! What these tests pin is behaviour, not shapes: the faces are painted, so the shell reports what it drew
//! ([`cam_ui::DrawnState::tool_header`] and its neighbours) and every claim below is read from that report, from
//! what the document holds afterwards, or from what the library file reads back. The rules that matter most are
//! that a control is drawn only when it governs something (L6, L14, L22) and that a headless test reading shapes
//! cannot tell a clipped control from a visible one (L23) — which is why the reported hits are only points the
//! frame really showed, and why `Use this tool` is asserted through the action it emits and the file it writes.
//!
//! One exception is stated where it is made rather than hidden here: the form's boxes are not reported as
//! hits, so the tests that type into one press the box's own pixels — the point its value was painted at — and
//! never a rectangle computed from the column's padding (`support::type_into_box`).
//!
//! Every claim is one test, named after the sentence of the spec it pins, so a spec that changes has exactly
//! one place to change with it.

#[path = "tool_library/actions.rs"]
mod actions;
#[path = "tool_library/card.rs"]
mod card;
#[path = "tool_library/editing.rs"]
mod editing;
#[path = "tool_library/empty.rs"]
mod empty;
#[path = "tool_library/family.rs"]
mod family;
#[path = "tool_library/footer.rs"]
mod footer;
#[path = "tool_library/form.rs"]
mod form;
#[path = "tool_library/header.rs"]
mod header;
#[path = "tool_library/library_file.rs"]
mod library_file;
#[path = "tool_library/preview.rs"]
mod preview;
#[path = "tool_library/speeds.rs"]
mod speeds;
#[path = "tool_library/support.rs"]
mod support;
#[path = "tool_library/tree.rs"]
mod tree;
