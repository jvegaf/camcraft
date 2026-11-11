use cam_app::AppState;
use egui::{Align, Layout, RichText};

use crate::help::Command;
use crate::icons::Icon;
use crate::theme::{self, Metrics, Role, Theme};
use crate::{Shell, ShellOutput};

use super::paint::{
  command_icon, icon_button, paint_mark, rect_report, segmented_cells, Segmented,
};

impl Shell {
  /// The app bar: what this document is and the switches the frame keeps up here.
  ///
  /// **The bar holds no file command any more.** `Open`, `Save`, `Save as`, `Fit the job`, `Undo` and `Redo`
  /// moved to the rail's FILE group in camui-prancheta T2b (S4): the reference gives the command table to the
  /// rail, and a bar and a rail offering the same seven commands is two vocabularies for one act. What is
  /// left is what belongs to the frame rather than to the document — its name, the units, the theme and the
  /// help panel — plus the one action that costs material.
  ///
  /// **The `2D | 3D` switch is not here.** S8 puts it at the top right of the canvas, where the reference
  /// floats it, and a segmented control in the bar and another over the drawing would be two of one control.
  /// Its one home is [`crate::Shell::view_toggle`].
  pub(crate) fn app_bar(&mut self, app: &AppState, ui: &mut egui::Ui, output: &mut ShellOutput) {
    let palette = self.palette();
    let metrics = self.metrics();
    egui::Panel::top("app_bar")
      .exact_size(metrics.app_bar_height)
      .show(ui, |ui| {
        ui.horizontal_centered(|ui| {
          paint_mark(ui, &palette, metrics.icon_size);
          ui.label(RichText::new("campilot").size(theme::TITLE));
          ui.separator();

          // The bar names the file the drawing came from, and says whether it is modified. Until a host
          // opens one there is no file to name, which is why the bar said `untitled.cmc` for so long.
          if self.project_open {
            let dirty = if app.is_dirty() { " · modified" } else { "" };
            ui.label(
              RichText::new(format!("{}{dirty}", self.document_name()))
                .size(theme::CAPTION)
                .color(palette.get(Role::TextDim)),
            );
          } else {
            // No project: the bar names the absence, and the name is the way back to the start screen — otherwise
            // `Esc` would hide the list of recent projects until the window was launched again (start-screen S3).
            let open = ui.button(
              RichText::new(crate::start_screen::NO_PROJECT)
                .size(theme::CAPTION)
                .color(palette.get(Role::TextDim)),
            );
            let centre = open.rect.center();
            output.drawn.app_bar_hits.push((
              crate::start_screen::NO_PROJECT.to_owned(),
              (centre.x, centre.y),
            ));
            if open.clicked() {
              self.show_start = true;
            }
          }

          ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            // The primary action writes the program the drawing describes, and it is **not** disabled
            // (M13 S2): it stays pressable and refuses by name from `apply_action` when it cannot — a
            // greyed button whose reason lives in a hover is the thing this milestone removed, because a
            // reason nobody hovers over reads as a button that does nothing.
            let generate = ui
              .button("Generate G-code")
              .on_hover_text(
                "Write the program the drawing describes to a file you name, in the post-processor this \
                 window holds",
              );
            let _ = generate.clicked();
            output.drawn.app_bar_hits.push((
              "Generate G-code".to_owned(),
              (generate.rect.center().x, generate.rect.center().y),
            ));
            // The dialect is a choice, not a default: `cam-cli emit` asks for it as `--post`, and a window
            // that picked one for the person would be choosing what their machine runs. The label names the
            // choice once it exists, so "is a program possible yet" is answered without a hover.
            let post: Option<String> = None;
            let post_label = match &post {
              Some(name) => format!("Post: {name}"),
              None => "Post…".to_owned(),
            };
            let choose = ui.button(post_label).on_hover_text(match &post {
              Some(name) => format!("Every program is written in {name} — press to choose another"),
              None => {
                "No post-processor is chosen, so no program can be written — press to choose a `.pp`"
                  .to_owned()
              }
            });
            let _ = choose.clicked();
            output.drawn.app_bar_hits.push((
              "Post".to_owned(),
              (choose.rect.center().x, choose.rect.center().y),
            ));
            // Help is the `F1` panel now: the panel teaches every command, so a help button that opened
            // a list of chords instead would answer a smaller question than the one asked.
            if icon_button(
              ui,
              &palette,
              command_icon(Command::Help),
              "Help — F1",
              false,
              true,
              18.0,
            )
            .clicked()
            {
              self.show_help = !self.show_help;
            }
            let (icon, label) = match self.theme {
              Theme::Dark => (Icon::ThemeDark, "Theme: dark — press for light"),
              Theme::Light => (Icon::ThemeLight, "Theme: light — press for dark"),
            };
            if icon_button(ui, &palette, icon, label, false, true, 18.0).clicked() {
              self.flip_theme();
            }
            ui.separator();
            ui.label(
              RichText::new("mm")
                .size(theme::CAPTION)
                .color(palette.get(Role::TextDim)),
            )
            .on_hover_text("Working units. Every length in the shell is millimetres.");
          });
        });
      });
  }

  /// The `2D | 3D` toggle, at the top right of the canvas (camui-prancheta S8).
  ///
  /// A segmented control, because the choice is exclusive: two alternatives, visible at once, so choosing is a
  /// comparison and not a memory test. It lives in the canvas' **top strip** — the band the ruler occupies on
  /// the drawing half (S5) — and never over the sheet, because a press on chrome that also dropped a selection
  /// on the drawing underneath would be a control that lies.
  ///
  /// It is pressed and applied here, before the caller decides whether the canvas splits, so the frame that
  /// presses `3D` is the frame that shows two halves (L13's rule), and it is painted over the ruler's band rather
  /// than beside it: the control's own fill covers what it crosses, so no number ends up printed under a control.
  pub(crate) fn view_toggle(
    &mut self,
    ui: &mut egui::Ui,
    area: egui::Rect,
    metrics: &Metrics,
    output: &mut ShellOutput,
  ) -> Option<Segmented> {
    let palette = self.palette();
    let strip = egui::Rect::from_min_max(
      area.min,
      egui::pos2(area.max.x, area.min.y + metrics.ruler_thickness),
    );
    let mut control = None;
    ui.scope_builder(egui::UiBuilder::new().max_rect(strip), |ui| {
      ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.add_space(metrics.space(3));
        control = Some(segmented_cells(ui, &palette, &["2D", "3D"]));
      });
    });
    let control = control?;
    if let Some(pick) = control.picked {
      self.set_view_3d(pick == 1);
    }
    output.drawn.view_toggle = control.cells.clone();
    output.drawn.view_toggle_rect = Some(rect_report(control.rect));
    // Returned rather than only painted: the caller paints it again after the ruler band, which is drawn later
    // in the frame and would otherwise cover it (R2-002). The press above is already applied, so the frame that
    // presses `3D` is still the frame that splits (L13).
    Some(control)
  }
}
