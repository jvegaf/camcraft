//! Headless behaviour tests for the egui shell (A7, A8, A9).
//!
//! No display and no GPU: `egui::Context` runs a full frame on the CPU, the shell reports what it
//! displayed, and the painted text is read back out of the produced shapes. Behaviour — what a user
//! action does to the document — is asserted by applying the shell's actions through `handle_action`.

#[path = "shell/authoring.rs"]
mod authoring;
#[path = "shell/document.rs"]
mod document;
#[path = "shell/files.rs"]
mod files;
#[path = "shell/help.rs"]
mod help;
#[path = "shell/layout.rs"]
mod layout;
#[path = "shell/material.rs"]
mod material;
#[path = "shell/palette.rs"]
mod palette;
#[path = "shell/panels.rs"]
mod panels;
#[path = "shell/program.rs"]
mod program;
#[path = "shell/rulers.rs"]
mod rulers;
#[path = "shell/save.rs"]
mod save;
#[path = "shell/script.rs"]
mod script;
#[path = "shell/start.rs"]
mod start;
#[path = "shell/status.rs"]
mod status;
#[path = "shell/support.rs"]
mod support;
#[path = "shell/tool_library.rs"]
mod tool_library;
#[path = "shell/view.rs"]
mod view;
