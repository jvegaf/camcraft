use cam_app::AppState;
use cam_store::tools::stored_tool_for_number;
use cam_units::{Format, Length};
use egui::RichText;

use crate::theme::{self, Palette, Role};
use crate::{Shell, ShellOutput};

use super::{quick_fields, CutField, RATES};

impl Shell {
  /// The quick block's rows: the operation's most-changed numbers, then the tool's rates as readings.
  pub(super) fn quick_block(
    &mut self,
    app: &AppState,
    ui: &mut egui::Ui,
    palette: &Palette,
    output: &mut ShellOutput,
  ) {
    for field in quick_fields(&self.strategy.operation) {
      self.number_row(ui, palette, *field, &mut output.drawn.strategy_quick_fields);
    }
    // The rates are the tool's, not the operation's: the same rows the library's `SPEEDS` group draws, in the
    // units that library authored them in (L25). Nothing here can change them — `cam-store` owns the writer —
    // so they are drawn as readings above the sentence that says where they live.
    let number = match self.described_toolpath(app) {
      Some(toolpath) => Some(toolpath.tool_number),
      None => self.strategy.tool_number,
    };
    // A missing tool is said by the card above, in its own sentence: a second one here would be the same
    // fact twice, so this `if let` has no `else`.
    let tool = number.and_then(|number| stored_tool_for_number(&self.tools, number));
    if let Some(tool) = tool {
      let rates =
        crate::tool_library::form_fields(crate::tool_library::ToolFormGroup::Speeds, tool);
      // One row per rate, in the block's own shape: a word at the left and its value at the right. The
      // library's boxed field widget is not used here because three fixed-width boxes in a wrapping row
      // measure wider than the panel on the frame that has not been laid out yet, and a child wider than its
      // panel drags the whole frame's layout out with it (the panel then reports a content width the shell
      // never intended and the next frame draws the surface off the window).
      for (label, value) in &rates {
        ui.horizontal(|ui| {
          ui.label(
            RichText::new(label)
              .size(theme::CAPTION)
              .color(palette.get(Role::TextDim)),
          );
          ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
              RichText::new(value)
                .size(theme::SMALL)
                .color(palette.get(Role::TextHi)),
            );
          });
        });
      }
      if !rates.is_empty() {
        ui.label(
          RichText::new(RATES)
            .size(theme::CAPTION)
            .color(palette.get(Role::TextDim)),
        );
      }
    }
  }

  /// One number row: its word, the number the draft holds, and the unit.
  ///
  /// The widget writes straight into the draft's operation, and the request that draws it also reports the row,
  /// so the `CUT` section and the quick block cannot show two different values for one number.
  pub(super) fn number_row(
    &mut self,
    ui: &mut egui::Ui,
    palette: &Palette,
    field: CutField,
    report: &mut Vec<(String, String)>,
  ) {
    let Some(value) = field.value(&self.strategy.operation) else {
      return;
    };
    let mut millimetres = value.mm();
    ui.horizontal(|ui| {
      ui.label(
        RichText::new(field.label())
          .size(theme::CAPTION)
          .color(palette.get(Role::TextDim)),
      );
      ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let response = ui.add(
          egui::DragValue::new(&mut millimetres)
            .speed(0.5)
            .suffix(" mm"),
        );
        if response.changed() {
          field.set(&mut self.strategy.operation, Length::from_mm(millimetres));
        }
      });
    });
    report.push((
      field.label().to_owned(),
      format!("{} mm", Format::fixed(2).apply(millimetres)),
    ));
  }
}
