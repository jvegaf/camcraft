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
  /// Floating Studio top bar (task 070): thin frame with product, document, command search, and frame actions.
  pub(crate) fn app_bar(&mut self, app: &AppState, ui: &mut egui::Ui, output: &mut ShellOutput) {
    let palette = self.palette();
    let metrics = self.metrics();
    let height = 52.0_f32.max(metrics.app_bar_height);
    egui::Panel::top("app_bar")
      .exact_size(height)
      .frame(
        egui::Frame::NONE
          .fill(palette.get(Role::App))
          .inner_margin(egui::Margin::symmetric(14, 0)),
      )
      .show(ui, |ui| {
        ui.horizontal_centered(|ui| {
          paint_mark(ui, &palette, 24.0);
          ui.add_space(4.0);
          ui.label(
            RichText::new("Camcraft")
              .size(15.0)
              .strong()
              .color(palette.get(Role::Text)),
          );
          ui.add_space(6.0);
          ui.label(
            RichText::new("|")
              .size(theme::CAPTION)
              .color(palette.get(Role::TextOff)),
          );
          ui.add_space(6.0);

          if self.project_open {
            let dirty = if app.is_dirty() { " · modified" } else { "" };
            ui.label(
              RichText::new(format!("{}{dirty}", self.document_name()))
                .size(theme::BODY)
                .color(palette.get(Role::TextDim)),
            );
            let stamp = if app.is_dirty() {
              "Unsaved changes".to_owned()
            } else {
              self.saved_stamp()
            };
            ui.add_space(8.0);
            ui.label(
              RichText::new(format!("v{} · {stamp}", env!("CARGO_PKG_VERSION")))
                .size(theme::CAPTION)
                .color(palette.get(Role::TextOff)),
            );
          } else {
            let open = ui.button(
              RichText::new(crate::start_screen::NO_PROJECT)
                .size(theme::BODY)
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
            let stamp = if app.is_dirty() {
              "Unsaved changes".to_owned()
            } else {
              self.saved_stamp()
            };
            ui.add_space(8.0);
            ui.label(
              RichText::new(format!("v{} · {stamp}", env!("CARGO_PKG_VERSION")))
                .size(theme::CAPTION)
                .color(palette.get(Role::TextOff)),
            );
          }

          // Central command search placeholder
          let available = ui.available_width();
          let right_reserve = 360.0;
          let search_w = (available - right_reserve).clamp(160.0, 520.0);
          ui.add_space(((available - right_reserve - search_w) / 2.0).max(8.0));
          let (search_rect, search_resp) =
            ui.allocate_exact_size(egui::vec2(search_w, 34.0), egui::Sense::click());
          let search_bg = palette.get(Role::Raised);
          let search_border = palette.get(Role::Border);
          ui.painter()
            .rect_filled(search_rect, egui::CornerRadius::same(10), search_bg);
          ui.painter().rect_stroke(
            search_rect,
            egui::CornerRadius::same(10),
            egui::Stroke::new(1.0, search_border),
            egui::StrokeKind::Inside,
          );
          ui.painter().text(
            search_rect.left_center() + egui::vec2(12.0, 0.0),
            egui::Align2::LEFT_CENTER,
            "Buscar comandos o ir a…",
            egui::FontId::proportional(theme::BODY),
            palette.get(Role::TextOff),
          );
          ui.painter().text(
            search_rect.right_center() - egui::vec2(12.0, 0.0),
            egui::Align2::RIGHT_CENTER,
            "⌘K",
            egui::FontId::proportional(theme::SMALL),
            palette.get(Role::TextOff),
          );
          if search_resp
            .on_hover_text("Help / command search (F1)")
            .clicked()
          {
            self.show_help = !self.show_help;
          }

          ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui
              .button(RichText::new("Settings").size(theme::SMALL))
              .on_hover_text("Settings")
              .clicked()
            {
              self.open_settings();
            }
            let (icon, label) = match self.theme {
              Theme::Dark => (Icon::ThemeDark, "Theme: dark — press for light"),
              Theme::Light => (Icon::ThemeLight, "Theme: light — press for dark"),
            };
            if icon_button(ui, &palette, icon, label, false, true, 18.0).clicked() {
              self.flip_theme();
            }
            let layout_resp = ui.add(egui::Button::new(
              RichText::new("Floating layout").size(theme::SMALL),
            ));
            if layout_resp
              .on_hover_text("Floating Studio layout")
              .clicked()
            {
              self.status = "Floating layout active".to_owned();
            }
            ui.separator();
            let generate = ui
              .button(RichText::new("Generate G-code").size(theme::SMALL))
              .on_hover_text("Write the program the drawing describes to a file you name");
            output.drawn.app_bar_hits.push((
              "Generate G-code".to_owned(),
              (generate.rect.center().x, generate.rect.center().y),
            ));
            let post: Option<String> = None;
            let post_label = match &post {
              Some(name) => format!("Post: {name}"),
              None => "Post…".to_owned(),
            };
            let choose = ui.button(RichText::new(post_label).size(theme::SMALL));
            output.drawn.app_bar_hits.push((
              "Post".to_owned(),
              (choose.rect.center().x, choose.rect.center().y),
            ));
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
