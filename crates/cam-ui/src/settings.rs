//! The settings modal (camcraft-workbench-v2 04-settings): preferences the operator can inspect and change.
//!
//! The layout follows the SolidCraft-family artboard: a page list on the left, the selected page on the right,
//! a search field in the title bar, and a footer with Restore/Cancel/Apply. Controls are wired to real shell
//! state where it exists; other pages show honest, domain-relevant placeholders.

use egui::{RichText, Sense, Stroke};

use crate::theme::{self, Role, Theme};
use crate::{Shell, ShellOutput};

/// Which page is visible in the settings dialog.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SettingsPage {
  /// Startup, recents, close behavior, language.
  #[default]
  General,
  /// Theme and visual presentation.
  Appearance,
  /// Workspace layout.
  Layout,
  /// Canvas/grid/snap behavior.
  Canvas,
  /// Manufacturing defaults.
  Manufacturing,
  /// Tool library access.
  ToolLibrary,
  /// Import defaults.
  Import,
}

impl SettingsPage {
  /// Pages in display order.
  pub const ALL: [SettingsPage; 7] = [
    Self::General,
    Self::Appearance,
    Self::Layout,
    Self::Canvas,
    Self::Manufacturing,
    Self::ToolLibrary,
    Self::Import,
  ];

  /// Display label.
  pub fn label(self) -> &'static str {
    match self {
      Self::General => "General",
      Self::Appearance => "Apariencia",
      Self::Layout => "Layout",
      Self::Canvas => "Canvas",
      Self::Manufacturing => "Manufactura",
      Self::ToolLibrary => "Librería de herramientas",
      Self::Import => "Importar",
    }
  }

  /// Group heading shown above this page in the nav, if it starts a group.
  pub fn group(self) -> Option<&'static str> {
    match self {
      Self::General => Some("General"),
      Self::Layout => Some("Workspace"),
      Self::Manufacturing => Some("CAM"),
      _ => None,
    }
  }
}

/// Editable settings snapshot. Apply copies it into Shell; Cancel discards it.
#[derive(Clone, Debug)]
pub(crate) struct SettingsDraft {
  pub page: SettingsPage,
  pub search: String,
  pub startup_start_screen: bool,
  pub recents_count: usize,
  pub reopen_last_project: bool,
  pub confirm_before_close: bool,
  pub theme: Theme,
  pub snapping: bool,
}

impl SettingsDraft {
  pub fn from_shell(shell: &Shell) -> Self {
    Self {
      page: shell.settings_page,
      search: shell.settings_search.clone(),
      startup_start_screen: shell.settings_startup_start_screen,
      recents_count: shell.settings_recents_count,
      reopen_last_project: shell.settings_reopen_last_project,
      confirm_before_close: shell.settings_confirm_before_close,
      theme: shell.theme,
      snapping: shell.snapping,
    }
  }

  pub fn apply_to(self, shell: &mut Shell) {
    shell.settings_page = self.page;
    shell.settings_search = self.search;
    shell.settings_startup_start_screen = self.startup_start_screen;
    shell.settings_recents_count = self.recents_count;
    shell.settings_reopen_last_project = self.reopen_last_project;
    shell.settings_confirm_before_close = self.confirm_before_close;
    shell.snapping = self.snapping;
    if shell.theme != self.theme {
      shell.theme = self.theme;
      shell.applied_theme = None;
    }
  }
}

const TITLE: &str = "Preferencias";
const SEARCH_HINT: &str = "Buscar configuración…";
const RESTORE: &str = "Restaurar valores";
const CANCEL: &str = "Cancelar";
const APPLY: &str = "Aplicar";

impl Shell {
  /// Open the settings dialog.
  pub fn open_settings(&mut self) {
    self.show_settings = true;
    self.settings_draft = Some(SettingsDraft::from_shell(self));
  }

  /// Close settings without applying.
  pub fn close_settings(&mut self) {
    self.show_settings = false;
    self.settings_draft = None;
  }

  fn apply_settings(&mut self) {
    if let Some(draft) = self.settings_draft.take() {
      draft.apply_to(self);
    }
    self.show_settings = false;
  }

  fn restore_settings_defaults(&mut self) {
    if let Some(draft) = &mut self.settings_draft {
      let page = draft.page;
      let search = draft.search.clone();
      *draft = SettingsDraft {
        page,
        search,
        startup_start_screen: true,
        recents_count: 12,
        reopen_last_project: false,
        confirm_before_close: true,
        theme: Theme::Dark,
        snapping: true,
      };
    }
  }

  pub(crate) fn settings_dialog(
    &mut self,
    ctx: &egui::Context,
    _app: &cam_app::AppState,
    _output: &mut ShellOutput,
  ) {
    let palette = self.palette();
    let metrics = self.metrics();
    let Some(mut draft) = self.settings_draft.clone() else {
      self.show_settings = false;
      return;
    };

    let mut close = false;
    let mut apply = false;
    let mut restore = false;

    egui::Modal::new(egui::Id::unique("camcraft-settings"))
      .frame(
        egui::Frame::NONE
          .fill(palette.get(Role::Panel))
          .stroke(Stroke::new(metrics.hairline, palette.get(Role::Border)))
          .corner_radius(egui::CornerRadius::same(metrics.radius_dialog as u8))
          .inner_margin(egui::Margin::ZERO),
      )
      .show(ctx, |ui| {
        ui.set_width(920.0);
        ui.set_height(680.0);

        // Title bar
        egui::Frame::NONE
          .inner_margin(egui::Margin {
            left: 16,
            right: 16,
            top: 11,
            bottom: 11,
          })
          .show(ui, |ui| {
            ui.horizontal(|ui| {
              ui.label(
                RichText::new(TITLE)
                  .size(theme::TITLE)
                  .color(palette.get(Role::TextHi))
                  .strong(),
              );
              ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                  .add(egui::Button::new(RichText::new("✕").size(theme::BODY)).frame(false))
                  .on_hover_text("Cerrar")
                  .clicked()
                {
                  close = true;
                }
                ui.add_space(8.0);
                ui.add(
                  egui::TextEdit::singleline(&mut draft.search)
                    .hint_text(SEARCH_HINT)
                    .desired_width(260.0),
                );
              });
            });
          });
        let rect = ui.max_rect();
        ui.painter().line_segment(
          [
            egui::pos2(rect.min.x, rect.min.y + 52.0),
            egui::pos2(rect.max.x, rect.min.y + 52.0),
          ],
          Stroke::new(metrics.hairline, palette.get(Role::Hairline)),
        );

        // Content
        let content_h = 680.0 - 52.0 - 56.0;
        ui.allocate_ui_with_layout(
          egui::vec2(920.0, content_h),
          egui::Layout::left_to_right(egui::Align::Min),
          |ui| {
            // Nav
            egui::Frame::NONE
              .fill(palette.get(Role::App))
              .inner_margin(egui::Margin {
                left: 8,
                right: 8,
                top: 12,
                bottom: 12,
              })
              .show(ui, |ui| {
                ui.set_width(204.0);
                ui.set_min_height(content_h);
                egui::ScrollArea::vertical()
                  .id_salt("settings-nav")
                  .show(ui, |ui| {
                    for page in SettingsPage::ALL {
                      if let Some(group) = page.group() {
                        ui.add_space(6.0);
                        ui.label(
                          RichText::new(group.to_uppercase())
                            .size(theme::CAPTION)
                            .color(palette.get(Role::TextOff))
                            .strong(),
                        );
                        ui.add_space(2.0);
                      }
                      let selected = draft.page == page;
                      let (row_rect, resp) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 34.0),
                        Sense::click(),
                      );
                      if resp.hovered() || selected {
                        let fill = if selected {
                          palette.get(Role::SelSoft)
                        } else {
                          palette.get(Role::Raised)
                        };
                        ui.painter()
                          .rect_filled(row_rect, egui::CornerRadius::same(6), fill);
                        if selected {
                          ui.painter().rect_stroke(
                            row_rect,
                            egui::CornerRadius::same(6),
                            Stroke::new(1.0, palette.get(Role::Sel).gamma_multiply(0.25)),
                            egui::StrokeKind::Inside,
                          );
                        }
                      }
                      let icon_color = if selected {
                        palette.get(Role::Sel)
                      } else {
                        palette.get(Role::TextDim)
                      };
                      let text_color = if selected {
                        palette.get(Role::TextHi)
                      } else {
                        palette.get(Role::Text)
                      };
                      paint_nav_icon(
                        ui.painter(),
                        row_rect.min + egui::vec2(10.0, 9.0),
                        page,
                        icon_color,
                      );
                      ui.painter().text(
                        row_rect.min + egui::vec2(36.0, 17.0),
                        egui::Align2::LEFT_CENTER,
                        page.label(),
                        egui::FontId::proportional(theme::BODY),
                        text_color,
                      );
                      if resp.clicked() {
                        draft.page = page;
                      }
                    }
                  });
              });

            // Separator
            let sep_x = ui.min_rect().max.x;
            ui.painter().line_segment(
              [
                egui::pos2(sep_x, ui.min_rect().min.y),
                egui::pos2(sep_x, ui.min_rect().min.y + content_h),
              ],
              Stroke::new(metrics.hairline, palette.get(Role::Hairline)),
            );

            // Main
            egui::Frame::NONE
              .inner_margin(egui::Margin {
                left: 28,
                right: 28,
                top: 24,
                bottom: 24,
              })
              .show(ui, |ui| {
                ui.set_min_width(920.0 - 221.0 - 56.0);
                ui.set_min_height(content_h);
                egui::ScrollArea::vertical()
                  .id_salt("settings-main")
                  .show(ui, |ui| {
                    paint_settings_page(ui, &palette, &mut draft);
                  });
              });
          },
        );

        // Footer separator
        let footer_y = rect.min.y + 680.0 - 56.0;
        ui.painter().line_segment(
          [
            egui::pos2(rect.min.x, footer_y),
            egui::pos2(rect.max.x, footer_y),
          ],
          Stroke::new(metrics.hairline, palette.get(Role::Hairline)),
        );
        egui::Frame::NONE
          .inner_margin(egui::Margin {
            left: 16,
            right: 16,
            top: 12,
            bottom: 12,
          })
          .show(ui, |ui| {
            ui.horizontal(|ui| {
              if ui
                .button(
                  RichText::new(RESTORE)
                    .size(theme::BODY)
                    .color(palette.get(Role::TextDim)),
                )
                .clicked()
              {
                restore = true;
              }
              ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let apply_btn = egui::Button::new(
                  RichText::new(APPLY)
                    .size(theme::BODY)
                    .color(palette.get(Role::Cut)),
                )
                .fill(palette.get(Role::CutSoft))
                .stroke(Stroke::new(
                  1.0,
                  palette.get(Role::Cut).gamma_multiply(0.35),
                ));
                if ui.add(apply_btn).clicked() {
                  apply = true;
                }
                ui.add_space(8.0);
                if ui.button(RichText::new(CANCEL).size(theme::BODY)).clicked() {
                  close = true;
                }
              });
            });
          });
      });

    if restore {
      self.restore_settings_defaults();
      if let Some(d) = self.settings_draft.clone() {
        draft = d;
      }
      self.settings_draft = Some(draft);
      return;
    }
    if close {
      self.close_settings();
      return;
    }
    if apply {
      self.settings_draft = Some(draft);
      self.apply_settings();
      return;
    }
    self.settings_draft = Some(draft);
  }
}

fn paint_nav_icon(
  painter: &egui::Painter,
  pos: egui::Pos2,
  page: SettingsPage,
  color: egui::Color32,
) {
  let r = egui::Rect::from_min_size(pos, egui::vec2(16.0, 16.0));
  let c = r.center();
  let s = Stroke::new(1.5, color);
  match page {
    SettingsPage::General => {
      painter.circle_stroke(c, 3.0, s);
      painter.circle_stroke(c, 6.0, Stroke::new(1.0, color));
    }
    SettingsPage::Appearance => {
      painter.rect_stroke(
        egui::Rect::from_min_size(r.min + egui::vec2(1.0, 2.0), egui::vec2(14.0, 10.0)),
        2.0,
        s,
        egui::StrokeKind::Inside,
      );
      painter.line_segment(
        [egui::pos2(c.x, r.max.y - 2.0), egui::pos2(c.x, r.max.y)],
        s,
      );
    }
    SettingsPage::Layout => {
      painter.rect_stroke(
        egui::Rect::from_min_size(r.min + egui::vec2(1.0, 1.0), egui::vec2(6.0, 6.0)),
        1.0,
        s,
        egui::StrokeKind::Inside,
      );
      painter.rect_stroke(
        egui::Rect::from_min_size(r.min + egui::vec2(9.0, 1.0), egui::vec2(6.0, 6.0)),
        1.0,
        s,
        egui::StrokeKind::Inside,
      );
      painter.rect_stroke(
        egui::Rect::from_min_size(r.min + egui::vec2(1.0, 9.0), egui::vec2(6.0, 6.0)),
        1.0,
        s,
        egui::StrokeKind::Inside,
      );
    }
    SettingsPage::Canvas => {
      painter.line_segment(
        [
          egui::pos2(r.min.x + 2.0, r.max.y - 1.0),
          egui::pos2(r.min.x + 2.0, r.min.y + 2.0),
        ],
        s,
      );
      painter.line_segment(
        [
          egui::pos2(r.min.x + 2.0, r.max.y - 1.0),
          egui::pos2(r.max.x - 1.0, r.max.y - 1.0),
        ],
        s,
      );
      painter.line_segment(
        [
          egui::pos2(r.min.x + 4.0, r.max.y - 4.0),
          egui::pos2(r.min.x + 7.0, r.min.y + 6.0),
        ],
        s,
      );
      painter.line_segment(
        [
          egui::pos2(r.min.x + 7.0, r.min.y + 6.0),
          egui::pos2(r.min.x + 10.0, r.min.y + 9.0),
        ],
        s,
      );
    }
    SettingsPage::Manufacturing => {
      painter.circle_stroke(c, 5.5, s);
      painter.line_segment(
        [
          egui::pos2(c.x, r.min.y + 1.0),
          egui::pos2(c.x, r.min.y + 4.0),
        ],
        s,
      );
      painter.line_segment(
        [
          egui::pos2(c.x, r.max.y - 4.0),
          egui::pos2(c.x, r.max.y - 1.0),
        ],
        s,
      );
    }
    SettingsPage::ToolLibrary => {
      painter.line_segment(
        [
          egui::pos2(c.x - 3.0, r.min.y + 2.0),
          egui::pos2(c.x + 3.0, r.min.y + 2.0),
        ],
        s,
      );
      painter.line_segment(
        [
          egui::pos2(c.x, r.min.y + 2.0),
          egui::pos2(c.x, r.max.y - 5.0),
        ],
        s,
      );
      painter.line_segment(
        [
          egui::pos2(c.x - 3.0, r.max.y - 2.0),
          egui::pos2(c.x + 3.0, r.max.y - 2.0),
        ],
        s,
      );
    }
    SettingsPage::Import => {
      painter.line_segment(
        [
          egui::pos2(c.x, r.min.y + 2.0),
          egui::pos2(c.x, r.max.y - 4.0),
        ],
        s,
      );
      painter.line_segment(
        [
          egui::pos2(c.x - 3.0, r.max.y - 7.0),
          egui::pos2(c.x, r.max.y - 4.0),
        ],
        s,
      );
      painter.line_segment(
        [
          egui::pos2(c.x + 3.0, r.max.y - 7.0),
          egui::pos2(c.x, r.max.y - 4.0),
        ],
        s,
      );
      painter.line_segment(
        [
          egui::pos2(r.min.x + 2.0, r.max.y - 2.0),
          egui::pos2(r.max.x - 2.0, r.max.y - 2.0),
        ],
        s,
      );
    }
  }
}

fn paint_settings_page(ui: &mut egui::Ui, palette: &theme::Palette, draft: &mut SettingsDraft) {
  match draft.page {
    SettingsPage::General => paint_general(ui, palette, draft),
    SettingsPage::Appearance => paint_appearance(ui, palette, draft),
    SettingsPage::Layout => paint_layout(ui, palette),
    SettingsPage::Canvas => paint_canvas(ui, palette, draft),
    SettingsPage::Manufacturing => paint_manufacturing(ui, palette),
    SettingsPage::ToolLibrary => paint_tool_library(ui, palette),
    SettingsPage::Import => paint_import(ui, palette),
  }
}

fn page_title(ui: &mut egui::Ui, palette: &theme::Palette, title: &str, desc: &str) {
  ui.label(
    RichText::new(title)
      .size(theme::DIALOG)
      .color(palette.get(Role::TextHi))
      .strong(),
  );
  ui.add_space(4.0);
  ui.label(
    RichText::new(desc)
      .size(theme::BODY)
      .color(palette.get(Role::TextDim)),
  );
  ui.add_space(16.0);
}

fn section_heading(ui: &mut egui::Ui, palette: &theme::Palette, title: &str) {
  ui.add_space(8.0);
  ui.separator();
  ui.add_space(8.0);
  ui.label(
    RichText::new(title.to_uppercase())
      .size(theme::CAPTION)
      .color(palette.get(Role::TextOff))
      .strong(),
  );
  ui.add_space(4.0);
}

fn setting_row(
  ui: &mut egui::Ui,
  palette: &theme::Palette,
  label: &str,
  hint: &str,
  add_control: impl FnOnce(&mut egui::Ui),
) {
  ui.add_space(6.0);
  ui.horizontal(|ui| {
    ui.allocate_ui_with_layout(
      egui::vec2(240.0, 0.0),
      egui::Layout::top_down(egui::Align::Min),
      |ui| {
        ui.label(
          RichText::new(label)
            .size(theme::BODY)
            .color(palette.get(Role::TextHi))
            .strong(),
        );
        ui.label(
          RichText::new(hint)
            .size(theme::SMALL)
            .color(palette.get(Role::TextDim)),
        );
      },
    );
    ui.add_space(16.0);
    ui.vertical(|ui| {
      add_control(ui);
    });
  });
  ui.add_space(6.0);
}

fn paint_general(ui: &mut egui::Ui, palette: &theme::Palette, draft: &mut SettingsDraft) {
  page_title(ui, palette, "General", "Comportamiento base de la app. Estas opciones afectan cómo arranca Camcraft y cómo maneja archivos recientes.");

  section_heading(ui, palette, "Inicio");
  setting_row(
    ui,
    palette,
    "Al abrir Camcraft",
    "Qué mostrar cuando no hay un proyecto reabierto por el sistema.",
    |ui| {
      ui.horizontal(|ui| {
        let start = draft.startup_start_screen;
        if ui.selectable_label(start, "Start screen").clicked() {
          draft.startup_start_screen = true;
        }
        if ui.selectable_label(!start, "Proyecto vacío").clicked() {
          draft.startup_start_screen = false;
        }
      });
    },
  );
  setting_row(
    ui,
    palette,
    "Recientes",
    "Cantidad de proyectos mostrados en la start screen.",
    |ui| {
      egui::ComboBox::from_id_salt("settings-recents-count")
        .selected_text(format!("{} proyectos", draft.recents_count))
        .show_ui(ui, |ui| {
          for n in [8usize, 12, 20] {
            ui.selectable_value(&mut draft.recents_count, n, format!("{n} proyectos"));
          }
        });
    },
  );

  section_heading(ui, palette, "Archivos");
  setting_row(
    ui,
    palette,
    "Reabrir último proyecto",
    "Al iniciar, abrir el último proyecto si estaba cerrado normalmente.",
    |ui| {
      ui.checkbox(&mut draft.reopen_last_project, "Activar");
    },
  );
  setting_row(
    ui,
    palette,
    "Confirmar antes de cerrar",
    "Pedir confirmación si hay cambios sin guardar.",
    |ui| {
      ui.checkbox(&mut draft.confirm_before_close, "Activar");
    },
  );

  section_heading(ui, palette, "Idioma");
  setting_row(ui, palette, "Idioma de la interfaz", "Camcraft v2 usa inglés en la UI del shell; el contenido y documentación pueden localizarse por proyecto.", |ui| {
    ui.label(RichText::new("English").size(theme::BODY).color(palette.get(Role::Text)));
  });
}

fn paint_appearance(ui: &mut egui::Ui, palette: &theme::Palette, draft: &mut SettingsDraft) {
  page_title(
    ui,
    palette,
    "Apariencia",
    "Tema y presentación visual del shell.",
  );
  section_heading(ui, palette, "Tema");
  setting_row(
    ui,
    palette,
    "Modo de color",
    "Selecciona el tema oscuro o claro del workbench.",
    |ui| {
      ui.horizontal(|ui| {
        if ui
          .selectable_label(draft.theme == Theme::Dark, "Dark")
          .clicked()
        {
          draft.theme = Theme::Dark;
        }
        if ui
          .selectable_label(draft.theme == Theme::Light, "Light")
          .clicked()
        {
          draft.theme = Theme::Light;
        }
      });
    },
  );
}

fn paint_layout(ui: &mut egui::Ui, palette: &theme::Palette) {
  page_title(
    ui,
    palette,
    "Layout",
    "Organización del workspace. Floating Studio es el layout activo en esta versión.",
  );
  section_heading(ui, palette, "Workspace");
  setting_row(
    ui,
    palette,
    "Layout actual",
    "El shell usa paneles flotantes sobre un canvas dominante.",
    |ui| {
      ui.label(
        RichText::new("Floating Studio")
          .size(theme::BODY)
          .color(palette.get(Role::Text)),
      );
    },
  );
}

fn paint_canvas(ui: &mut egui::Ui, palette: &theme::Palette, draft: &mut SettingsDraft) {
  page_title(
    ui,
    palette,
    "Canvas",
    "Comportamiento del viewport y ayudas de dibujo.",
  );
  section_heading(ui, palette, "Snapping");
  setting_row(
    ui,
    palette,
    "Snapping",
    "Activa o desactiva el ajuste a puntos relevantes durante el dibujo.",
    |ui| {
      ui.checkbox(&mut draft.snapping, "Activar");
    },
  );
}

fn paint_manufacturing(ui: &mut egui::Ui, palette: &theme::Palette) {
  page_title(ui, palette, "Manufactura", "Valores por defecto para operaciones CAM. Se configurarán a medida que el flujo de manufactura se expanda.");
  section_heading(ui, palette, "Estado");
  setting_row(
    ui,
    palette,
    "Configuración disponible",
    "Las opciones de manufactura se exponen desde los paneles de operación y el material del job.",
    |ui| {
      ui.label(
        RichText::new("Ver paneles de Toolpaths y Job")
          .size(theme::BODY)
          .color(palette.get(Role::TextDim)),
      );
    },
  );
}

fn paint_tool_library(ui: &mut egui::Ui, palette: &theme::Palette) {
  page_title(
    ui,
    palette,
    "Librería de herramientas",
    "Gestión de herramientas de corte. La librería completa se abre desde el flujo de toolpaths.",
  );
  section_heading(ui, palette, "Acceso");
  setting_row(
    ui,
    palette,
    "Librería de herramientas",
    "Abre la librería para crear, editar, importar o exportar herramientas.",
    |ui| {
      ui.label(
        RichText::new("Disponible en el flujo de Toolpaths")
          .size(theme::BODY)
          .color(palette.get(Role::TextDim)),
      );
    },
  );
}

fn paint_import(ui: &mut egui::Ui, palette: &theme::Palette) {
  page_title(ui, palette, "Importar", "Valores por defecto de importación. Los ajustes actuales se aplican en el diálogo de importación.");
  section_heading(ui, palette, "Estado");
  setting_row(ui, palette, "Configuración de importación", "Tolerancia, unión, suavizado, centrado y ajuste de material se configuran al importar un archivo.", |ui| {
    ui.label(RichText::new("Disponible en el diálogo Importar").size(theme::BODY).color(palette.get(Role::TextDim)));
  });
}
