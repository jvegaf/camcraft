//! The tool library as a master-detail modal (camui-prancheta T7, spec S9).
//!
//! S9 draws one screen: a search field; a tree grouped by tool family with a count and one row per tool (mini
//! glyph, name, the dim `Ø6 · 2 cut(s)` line); a detail form grouped under `GEOMETRY` / `PASSES` / `SPEEDS` /
//! `MACHINE` whose fields change with the family; the tool drawn to scale with dimension callouts; and a
//! footer of library actions above `Cancel` / `Use this tool`.
//!
//! What these tests pin is behaviour, not shapes: the faces are painted, so the shell reports what it drew
//! ([`cam_ui::DrawnState::tool_library`] and its neighbours) and every claim below is read from that report or
//! from what the document holds afterwards. The two rules that matter most are that a control is drawn only
//! when it governs something (L6, L14, L22) and that a headless test reading shapes cannot tell a clipped
//! control from a visible one (L23) — which is why the reported hits are only points the frame really showed,
//! and why `Use this tool` is asserted through the action it emits and the document it changes.

#[path = "tool_library/card.rs"]
mod card;
#[path = "tool_library/drawing.rs"]
mod drawing;
#[path = "tool_library/family.rs"]
mod family;
#[path = "tool_library/footer.rs"]
mod footer;
#[path = "tool_library/form.rs"]
mod form;
#[path = "tool_library/support.rs"]
mod support;
#[path = "tool_library/tree.rs"]
mod tree;
