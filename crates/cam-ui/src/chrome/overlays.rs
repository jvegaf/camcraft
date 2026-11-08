use cam_app::AppState;
use egui::{Align, Layout, RichText};

use crate::help::{Command, Group};
use crate::script_panel;
use crate::theme::{self, Role};
use crate::{Shell, ShellOutput};

use super::paint::{chip, section};

impl Shell {
  /// The script modal (M10 T4b, S4): the panel's own row and its report, over the canvas.
  ///
  /// It is a modal for the reason the tool library is one (camui-prancheta S9): the panel's path field is
  /// 380 points wide and the right dock is 288, so no zone of the frame has room for it. A window adds the
  /// surface without moving the drawing, and `Esc` closes it. What `Run` does to the document is the
  /// shell's own edit path: the run mutates a copy and the copy replaces the document only when every
  /// line was accepted.
  pub(crate) fn script_dialog(&mut self, ctx: &egui::Context, app: &mut AppState) {
    let palette = self.palette();
    let metrics = self.metrics();
    // Cloned before the panel's own borrow is taken: the run reads this path and never writes it.
    let document_path = self.document_path.clone();
    // The panel is the only thing the closure touches: it holds the path field, `Run` and the last report,
    // and none of them is shell state a frame could read while the modal is open.
    let script = &mut self.script;
    let mut committed = None;
    egui::Modal::new(egui::Id::unique("campilot-script-panel"))
      .frame(
        egui::Frame::NONE
          .fill(palette.get(Role::Overlay))
          .stroke(egui::Stroke::new(
            metrics.hairline,
            palette.get(Role::Border),
          ))
          .corner_radius(egui::CornerRadius::same(metrics.radius_dialog as u8))
          .inner_margin(egui::Margin::same(metrics.space(5) as i8)),
      )
      .show(ctx, |ui| {
        // The panel's own row is a label, a 380-point field and `Run`, so the window is sized for those
        // three and their gaps: a field narrower than `PATH_FIELD_WIDTH` would scroll the path the
        // operator is typing.
        ui.set_max_width(script_panel::PATH_FIELD_WIDTH + metrics.space(7) * 3.0);
        committed = script.ui(ui, app.document(), document_path.as_deref());
      });
    // A run the engine accepted replaces the shell's document in one step, through the same history and
    // the same modified state every other edit uses: the panel writes no file, so the document it
    // mutated is only real once the shell holds it (M11 T2). A run that produced the document it was
    // given is not an edit at all — it records no undo step and leaves the modified state alone, so an
    // accepted script that changed nothing does not turn a saved document into an unsaved one (R4-001).
    if let Some(document) = committed {
      if &document != app.document() {
        app.replace_document_undoable(document);
      }
    }
  }

  /// The `F1` help panel: the whole command table, in the design's teaching voice.
  ///
  /// `design/camui-shell/02-stages.html` §04 draws the shape, and this is it: a header with the command's
  /// own name and the key that opened it, then one strong paragraph of what the command gives or costs,
  /// then how to run it, then the neighbours usually wanted next, then the table itself grouped the way
  /// the palette groups commands. The panel only teaches — a row here focuses help, it never runs a
  /// command — which is what lets the operator read about a destructive command safely.
  ///
  /// Sizes and colours come from [`crate::theme`] and nowhere else: the sentence an operator reads is
  /// [`theme::BODY`], the table is [`theme::SMALL`], and the group labels are the palette's own
  /// [`section`] run, so help looks like the frame it explains.
  pub(crate) fn help_window(&mut self, ctx: &egui::Context, output: &mut ShellOutput) {
    let palette = self.palette();
    let metrics = self.metrics();
    let mut open = self.show_help;
    egui::Window::new("Help")
      .open(&mut open)
      .resizable(true)
      // As wide as the two docked columns together: the frame's own measurement for "a panel of text".
      .default_width(metrics.palette_width + metrics.dock_width)
      .show(ctx, |ui| {
        let help = self.help_focus.help();
        ui.horizontal(|ui| {
          ui.label(RichText::new(self.help_focus.label()).size(theme::TITLE));
          ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            chip(ui, &palette, "F1", true);
          });
        });
        ui.add_space(metrics.space(1));
        // Consequence before mechanics: the `what` is the one sentence the operator came for, so it is
        // the only text in the panel drawn in the bright ink.
        ui.label(
          RichText::new(help.what)
            .size(theme::BODY)
            .color(palette.get(Role::TextHi)),
        );
        ui.label(
          RichText::new(help.how)
            .size(theme::BODY)
            .color(palette.get(Role::Text)),
        );
        if !help.related.is_empty() {
          let related: Vec<&str> = help.related.iter().map(|command| command.label()).collect();
          ui.label(
            RichText::new(format!("Related: {}", related.join(" · ")))
              .size(theme::CAPTION)
              .color(palette.get(Role::TextDim)),
          );
        }
        ui.add_space(metrics.space(2));
        // The table, in the palette's own order and with the palette's own group labels: a command is
        // learned where it is found, and that grouping is the one thing the frame already teaches.
        for group in Group::ALL {
          section(ui, &palette, group.label());
          output.drawn.help.push(group.label().to_owned());
          ui.horizontal_wrapped(|ui| {
            for command in Command::ALL
              .iter()
              .filter(|command| command.group() == group)
            {
              let focused = self.help_focus == *command;
              if ui
                .selectable_label(focused, RichText::new(command.label()).size(theme::SMALL))
                .clicked()
              {
                self.help_focus = *command;
              }
              output.drawn.help.push(command.label().to_owned());
            }
          });
        }
        // The two sentences the panel drew, reported last so a test can read them back without
        // walking the whole table looking for them.
        output.drawn.help.push(help.what.to_owned());
        output.drawn.help.push(help.how.to_owned());
      });
    self.show_help = open;
  }

  /// The shortcuts that exist today, in one window.
  ///
  /// Help listing keys nobody implemented would be a promise the shell does not keep, so the list is
  /// short on purpose.
  pub(crate) fn shortcuts_window(&mut self, ctx: &egui::Context) {
    let palette = self.palette();
    let mut open = self.show_shortcuts;
    egui::Window::new("Shortcuts")
      .open(&mut open)
      .resizable(false)
      .show(ctx, |ui| {
        for (keys, meaning) in [
          ("Ctrl+Z", "Undo"),
          ("Ctrl+Y", "Redo"),
          ("Delete", "Delete the selection"),
          ("F", "Fit the job in the window"),
          ("F2", "The icon set"),
          ("Wheel", "Zoom about the pointer"),
          ("Drag", "Pan; with a tool armed, draw"),
        ] {
          ui.horizontal(|ui| {
            ui.label(RichText::new(keys).text_style(egui::TextStyle::Monospace));
            ui.label(
              RichText::new(meaning)
                .size(theme::CAPTION)
                .color(palette.get(Role::TextDim)),
            );
          });
        }
      });
    self.show_shortcuts = open;
  }
}
