//! The egui editor shell: panels, canvas and input wiring, all expressed as actions on `cam-app`.
//!
//! Must not contain business rules: it turns input into [`UiAction`] values and paints
//! `cam_view::Scene`.
