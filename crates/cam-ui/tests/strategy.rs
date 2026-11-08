//! A strategy's editor: one surface for creating and editing (camui-prancheta T8, spec S10).
//!
//! S10 draws one editor twice: `New toolpath · <name>` and `Edit · <name>`, holding the name field, the
//! current-tool card with `Choose another tool…`, the collapsible `Quick parameter edit`, the strategy's own
//! sections, the note `The toolpath is calculated when you click "Generate toolpath".` and the pair
//! `Generate toolpath` / `Cancel`. The reference frames are `Screenshot 2026-11-03 034023.png` (creating a
//! `Desbaste 3D`) and `033350.png` (editing a `Perfil`).
//!
//! What these tests pin is what the operator can act on: the header the frame drew
//! ([`cam_ui::DrawnState::task_editor`]), the points the frame really showed
//! ([`cam_ui::DrawnState::strategy_hits`]), the words and pictures of its cards
//! ([`cam_ui::DrawnState::strategy_cards`]) and the state after each footer button. Three rules from this
//! feature's own log are applied rather than paid for again: a control is drawn only when it governs something
//! (L6, L14, L22), a headless test reads shapes and therefore needs a point the frame actually showed (L23),
//! and one number is one number — the quick block duplicates the section's values, never a second copy of them.

#[path = "strategy/cards.rs"]
mod cards;
#[path = "strategy/footer.rs"]
mod footer;
#[path = "strategy/preserved.rs"]
mod preserved;
#[path = "strategy/quick.rs"]
mod quick;
#[path = "strategy/support.rs"]
mod support;
#[path = "strategy/surface.rs"]
mod surface;
#[path = "strategy/tool.rs"]
mod tool;
