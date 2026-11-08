use cam_app::AppState;
use egui::Sense;

use crate::chrome;
use crate::theme::{self, Palette, Role};
use crate::{Shell, ShellOutput};

use super::kind_word;

impl Shell {
  /// The `TOOLPATHS` surface's list: every toolpath in the document, in document order, one pressable row each
  /// (M13 T3, S4).
  ///
  /// The gap this closes is measured in S4: the surface showed one editor for "the" toolpath and the card
  /// described `toolpaths.first()` unconditionally, so a document with two toolpaths could only ever describe
  /// the first. The row names what identifies its toolpath — the name, the operation and the tool number, the
  /// three things the card and the editor already read, so the list invents no vocabulary of its own — and a
  /// press selects it by id. That selection is view state ([`Shell::set_toolpath_selection`]): no command runs,
  /// no undo step is added, and the document is not written.
  ///
  /// An empty document draws no list at all rather than a heading over nothing: the editor below is already
  /// drafting its first toolpath and says so, and a control that cannot act is not drawn (L6, L14).
  pub(crate) fn toolpath_list(
    &mut self,
    app: &AppState,
    ui: &mut egui::Ui,
    palette: &Palette,
    output: &mut ShellOutput,
  ) {
    if app.document().toolpaths.is_empty() {
      output.drawn.toolpath_rows.clear();
      output.drawn.toolpath_hits.clear();
      output.drawn.toolpath_selected = None;
      return;
    }
    chrome::section(ui, palette, chrome::Category::Toolpaths.label());

    let metrics = self.metrics();
    // Every point this frame reports is filtered against the clip it drew in: the list lives in the inspector's
    // scroll area, and a row painted below its viewport is a promise about a control nobody can press (L23).
    let clip = ui.clip_rect();
    let height = 22.0;
    let radius = egui::CornerRadius::same(metrics.radius_control as u8);
    ui.spacing_mut().item_spacing.y = metrics.space(1);

    // The rows are painted against the answer of the *previous* frame, then a press is applied, then the
    // report is read from the answer that is now true — the rule the kind cards follow, so the frame that
    // selects a toolpath is the frame that reports it selected (L13).
    let selected_before = self
      .described_toolpath(app)
      .map(|toolpath| toolpath.id.clone());
    let mut rows: Vec<(String, String, egui::Rect)> = Vec::new();
    let mut pressed: Option<String> = None;
    for toolpath in &app.document().toolpaths {
      let word = format!(
        "{} · {} · T{}",
        toolpath.name,
        kind_word(&toolpath.operation),
        toolpath.tool_number
      );
      let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), height), Sense::click());
      let selected = selected_before.as_deref() == Some(toolpath.id.as_str());
      let fill = if selected {
        palette.get(Role::SelSoft)
      } else if response.hovered() {
        palette.get(Role::Raised)
      } else {
        palette.get(Role::Panel)
      };
      ui.painter().rect_filled(rect, radius, fill);
      ui.painter().rect_stroke(
        rect,
        radius,
        egui::Stroke::new(
          if selected { 1.5 } else { 1.0 },
          palette.get(if selected { Role::Sel } else { Role::Border }),
        ),
        egui::StrokeKind::Inside,
      );
      ui.painter().text(
        egui::pos2(rect.left() + 6.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        &word,
        egui::FontId::proportional(theme::SMALL),
        palette.get(if selected { Role::TextHi } else { Role::Text }),
      );
      if response.clicked() {
        pressed = Some(toolpath.id.clone());
      }
      rows.push((word, toolpath.id.clone(), rect));
    }

    if let Some(id) = pressed {
      self.set_toolpath_selection(Some(id));
      // The draft is re-seeded here rather than at the next frame's header, so the card and the sections that
      // follow the list in this very frame describe the toolpath the operator just chose.
      let selection = self.toolpath_selection().map(str::to_owned);
      self.strategy.sync(app, selection.as_deref());
    }

    let selected = self
      .described_toolpath(app)
      .map(|toolpath| toolpath.id.clone());
    for (word, id, rect) in rows {
      let chosen = selected.as_deref() == Some(id.as_str());
      output.drawn.toolpath_rows.push((word.clone(), chosen));
      let centre = rect.center();
      if clip.contains(centre) {
        output
          .drawn
          .toolpath_hits
          .push((word, (centre.x, centre.y)));
      }
    }
    output.drawn.toolpath_selected = selected;
    ui.add_space(metrics.space(1));
  }
}
