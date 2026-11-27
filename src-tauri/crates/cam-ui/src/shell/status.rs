use cam_app::AppState;
use cam_view::SnapKind;
use egui::RichText;

use crate::chrome;
use crate::theme::{self, Role};
use crate::{Shell, ShellOutput};

impl Shell {
  /// The status bar: the reference's three zones (camui-prancheta S7).
  ///
  /// Left: where the pointer is, with the two facts nothing else on the frame reports — the zoom this drawing
  /// is shown at and the steps available in each direction. Middle: the snaps, where a chip arms its kind and
  /// the grid chip names the step it snaps to. Right: what the last action produced, with the gesture's own
  /// sentence in front of it, where the reference keeps it beside the chips.
  ///
  /// The save state is **not** here: the rail's brand block names the moment the document was written
  /// (`Saved 2026-11-03 19:30 UTC` or `Unsaved changes`), and a bar saying `saved` beside a block saying when
  /// would be the same fact twice, the second time without the moment (T2b-ii, L16).
  pub(super) fn status_bar(&mut self, app: &AppState, ui: &mut egui::Ui, output: &mut ShellOutput) {
    let palette = self.palette();
    let cursor = match self.cursor_world {
      Some(point) => format!("X {:.2}   Y {:.2}", point.x, point.y),
      None => "X —   Y —".to_owned(),
    };
    // "undo / redo" are the steps available in each direction; dropped steps are reported separately
    // because they are a warning, not a redo count.
    let dropped = app.history_dropped();
    let dropped = if dropped > 0 {
      format!("   {dropped} step(s) dropped")
    } else {
      String::new()
    };
    let facts = format!(
      "zoom {:.2} px/mm   undo {} / redo {}{dropped}",
      app.camera().pixels_per_mm(),
      app.history_depth(),
      app.redo_depth()
    );
    let grid = grid_chip(self.grid_spacing);
    let message = self.status.clone();
    let hint = self.hint();
    let mut chips: Vec<(String, (f32, f32))> = Vec::new();
    egui::Panel::bottom("status_bar").show(ui, |ui| {
      ui.horizontal(|ui| {
        // With no project open there is no pointer position to report and no snap a person could arm: the bar says
        // what is true — that nothing is open — and names the two commands that end the state (start-screen S2).
        if !self.project_open {
          ui.label(
            RichText::new(crate::start_screen::NOTHING_OPEN)
              .size(theme::CAPTION)
              .color(palette.get(Role::TextDim)),
          );
          ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(
              egui::Label::new(
                RichText::new(&message)
                  .size(theme::CAPTION)
                  .color(palette.get(Role::TextDim)),
              )
              .truncate(),
            );
            ui.add(
              egui::Label::new(
                RichText::new(crate::start_screen::NO_PROJECT_HINT)
                  .size(theme::CAPTION)
                  .color(palette.get(Role::TextDim)),
              )
              .truncate(),
            );
          });
          return;
        }
        ui.label(
          RichText::new(&cursor)
            .monospace()
            .size(theme::CAPTION)
            .color(palette.get(Role::Text)),
        );
        ui.label(
          RichText::new("mm")
            .size(theme::CAPTION)
            .color(palette.get(Role::TextDim)),
        );
        ui.separator();
        ui.label(
          RichText::new(&facts)
            .monospace()
            .size(theme::CAPTION)
            .color(palette.get(Role::TextDim)),
        );
        ui.separator();
        // The reference's `Encaixes | Ponto | Meio | Centro | Grid 10` strip: what the pointer will stick to is
        // a thing an operator arms and disarms while working, not a setting in a dialog. Each click is applied
        // in the frame it happens, so the frame that arms a chip is the frame that says it is armed (L13).
        let response = chrome::chip(ui, &palette, "Snaps", self.snapping);
        chips.push((
          "Snaps".to_owned(),
          (response.rect.center().x, response.rect.center().y),
        ));
        if response.clicked() {
          self.snapping = !self.snapping;
        }
        for (kind, label) in [
          (SnapKind::Endpoint, "Point"),
          (SnapKind::Midpoint, "Mid"),
          (SnapKind::Center, "Centre"),
        ] {
          let response = chrome::chip(ui, &palette, label, self.snap_armed(kind));
          chips.push((
            label.to_owned(),
            (response.rect.center().x, response.rect.center().y),
          ));
          if response.clicked() {
            self.toggle_snap(kind);
          }
        }
        let response = chrome::chip(ui, &palette, &grid, self.snap_armed(SnapKind::Grid));
        chips.push((
          grid.clone(),
          (response.rect.center().x, response.rect.center().y),
        ));
        if response.clicked() {
          self.toggle_snap(SnapKind::Grid);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
          // The right end belongs to one sentence: what the action just produced. The gesture's own sentence
          // sits in front of it — the reference keeps it beside the chips — and yields the width, because
          // "what just happened" is the answer an operator looks for when a command answers back.
          ui.add(
            egui::Label::new(
              RichText::new(&message)
                .size(theme::CAPTION)
                .color(palette.get(Role::TextDim)),
            )
            .truncate(),
          );
          ui.add(
            egui::Label::new(
              RichText::new(&hint)
                .size(theme::CAPTION)
                .color(palette.get(Role::TextDim)),
            )
            .truncate(),
          );
        });
      });
    });
    if self.project_open {
      output.drawn.status_line = format!("{cursor} mm   {facts}   {message}");
      output.drawn.message = message;
      output.drawn.status_chips = chips;
      output.drawn.hint = hint;
    } else {
      output.drawn.status_line = format!("{}   {message}", crate::start_screen::NOTHING_OPEN);
      output.drawn.message = message;
      output.drawn.hint = crate::start_screen::NO_PROJECT_HINT.to_owned();
    }
  }
}

/// The grid chip's own words: the step it snaps to, in millimetres (`Grid 10`, `Grid 2.5`).
///
/// The reference writes `Grid 10`, and a chip that said only `Grid` would hide the one number an operator
/// needs to know what the snap will do. No step yet — the first frame, before the canvas has measured the
/// camera — is `Grid —`: zero is not a step, and `Grid 0` would name a snap that can never fire, because the
/// kernel arms no grid candidate at zero (`cam_view::pick`).
fn grid_chip(spacing: f64) -> String {
  if !spacing.is_finite() || spacing <= 0.0 {
    return "Grid —".to_owned();
  }
  let step = if spacing.fract().abs() < 1e-9 {
    format!("{spacing:.0}")
  } else {
    format!("{spacing:.2}")
      .trim_end_matches('0')
      .trim_end_matches('.')
      .to_owned()
  };
  format!("Grid {step}")
}
