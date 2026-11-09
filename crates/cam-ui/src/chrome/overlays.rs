use cam_app::AppState;
use egui::{Align, Layout, RichText};

use crate::help::{Command, Group};
use crate::script_panel;
use crate::theme::{self, Role};
use crate::{Shell, ShellOutput};

use super::paint::{chip, section};

/// The import modal's title prefix, and the two words on its footer.
///
/// English, like every other word in the shell: the reference frame is pt-BR and what we take from it is the
/// shape of the dialog, never its strings.
const IMPORT_TITLE: &str = "Import";
/// The sentence under the report, so the dialog says what pressing the button will cost.
const IMPORT_LEDE: &str =
  "The file has only been read. The drawing is replaced when you press Import.";
const IMPORT_CANCEL: &str = "Cancel";
const IMPORT_COMMIT: &str = "Import";

/// The width the import modal draws at.
///
/// Wide enough for the report sentence on one line at [`theme::BODY`], and narrower than the tool library: this
/// dialog holds one sentence and two buttons, so it must not grow into the surface it is interrupting.
const IMPORT_WIDTH: f32 = 460.0;

/// The tolerance field's label, the checkbox under it, and the sentence that says what the checkbox does.
///
/// The options the reference's dialog carries that this build still does not — join open vectors, smooth, and
/// position — are **absent on purpose** rather than drawn dead: [`cam_import`] has no such options (the rest of
/// S3's follow-up), and a control that wrote nothing is the dead button S7 and S9 both forbid.
const IMPORT_TOLERANCE_LABEL: &str = "Flattening tolerance";
const IMPORT_ADJUST_MATERIAL_LABEL: &str = "Adjust material size to drawing";
const IMPORT_ADJUST_MATERIAL_NOTE: &str =
  "Fits the stock to the drawing plus 10 mm on every side. Unchecked, the material you already have is kept.";
/// The bounds the tolerance field accepts, in millimetres.
///
/// The importer falls back to its own default for a distance it cannot use, and the shell refuses one here
/// before the file is read ([`crate::Shell::set_import_tolerance`]): a number the parse cannot honour is not an
/// option, it is a stale report.
const IMPORT_TOLERANCE_RANGE: std::ops::RangeInclusive<f64> = 0.001..=10.0;

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

  /// The import modal (shell-file-operations S3): what the chosen file holds, before any of it is written.
  ///
  /// The reference frame (`Screenshot 2026-11-03 032022.png`) settles the shape: the file's own name as the
  /// title, one block saying what the importer found, the options that report can honour, and a `Cancel`/`Import`
  /// footer.
  ///
  /// Two of the reference's options are here because [`cam_import`] can honour them today: the flattening
  /// tolerance the file is parsed at — changing it re-parses the file, so the report above the field is always
  /// the parse `Import` will write ([`crate::Shell::set_import_tolerance`]) — and whether the import fits the
  /// stock to the drawing. The rest of the reference's column — join open vectors, smooth, position — stays
  /// absent rather than dead: the importer has no such options (the remaining follow-up of S3), and a dialog
  /// must not promise a control that writes nothing.
  pub(crate) fn import_dialog(
    &mut self,
    ctx: &egui::Context,
    app: &mut AppState,
    _output: &mut ShellOutput,
  ) {
    let palette = self.palette();
    let metrics = self.metrics();
    // A flag with no parse has nothing to report, so it is closed instead of drawn empty: the two are set
    // together by [`crate::Shell::open_import`], and this is the honest repair of a half state.
    let Some(parsed) = self.import_parsed.clone() else {
      self.show_import = false;
      return;
    };
    let name = self
      .import_path
      .as_deref()
      .and_then(|path| path.file_name())
      .map(|name| name.to_string_lossy().into_owned())
      .unwrap_or_else(|| self.document_name());
    // One sentence for the whole report, so the numbers a test reads back are the numbers the dialog drew: the
    // same rule the material dialog's plan annotations follow.
    let report = format!(
      "format {} · {} contour(s) found, {} curve(s) flattened, {} skipped",
      parsed.report.format,
      parsed.report.contours,
      parsed.report.flattened_curves,
      parsed.report.skipped_count()
    );
    // Cloned before the closure: the refusal is read here and written by [`crate::Shell::confirm_import`] after
    // the frame, and borrowing `self` twice is what the clone avoids.
    let refusal = self.import_error.clone();
    // The two options' drafts, for the same reason: the closure edits these values and the shell writes back
    // only what it can honour — the flag itself for the checkbox, and an accepted re-parse for the tolerance.
    let mut tolerance = self.import_tolerance;
    let mut adjust_material = self.import_adjust_material;
    let mut retolerance: Option<f64> = None;
    let mut cancel = false;
    let mut commit = false;

    egui::Modal::new(egui::Id::unique("campilot-import"))
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
        ui.set_max_width(IMPORT_WIDTH);
        ui.label(
          RichText::new(format!("{IMPORT_TITLE} {name}"))
            .size(theme::TITLE)
            .color(palette.get(Role::TextHi)),
        );
        ui.add_space(metrics.space(2));
        // The report sits on a raised surface, the way the reference draws it: the reading a person makes before
        // pressing `Import` is a block of its own and not a paragraph between two others.
        egui::Frame::NONE
          .fill(palette.get(Role::Raised))
          .corner_radius(egui::CornerRadius::same(4))
          .inner_margin(egui::Margin::symmetric(
            metrics.space(2) as i8,
            metrics.space(1) as i8,
          ))
          .show(ui, |ui| {
            ui.set_max_width(IMPORT_WIDTH);
            ui.label(
              RichText::new(&report)
                .size(theme::BODY)
                .color(palette.get(Role::Text)),
            );
          });
        ui.add_space(metrics.space(1));
        ui.label(
          RichText::new(IMPORT_LEDE)
            .size(theme::CAPTION)
            .color(palette.get(Role::TextDim)),
        );
        ui.add_space(metrics.space(3));
        // The tolerance: `mm` on the number, the way every measurement in this shell carries its unit, and the
        // change is held until after the frame so the re-parse cannot mutate the shell mid-paint.
        ui.horizontal(|ui| {
          ui.label(
            RichText::new(IMPORT_TOLERANCE_LABEL)
              .size(theme::CAPTION)
              .color(palette.get(Role::TextDim)),
          );
          let changed = ui
            .add(
              egui::DragValue::new(&mut tolerance)
                .speed(0.005)
                .range(IMPORT_TOLERANCE_RANGE)
                .suffix(" mm")
                .fixed_decimals(3),
            )
            .changed();
          if changed {
            retolerance = Some(tolerance);
          }
        });
        ui.add_space(metrics.space(1));
        ui.checkbox(
          &mut adjust_material,
          RichText::new(IMPORT_ADJUST_MATERIAL_LABEL).size(theme::SMALL),
        );
        ui.label(
          RichText::new(IMPORT_ADJUST_MATERIAL_NOTE)
            .size(theme::CAPTION)
            .color(palette.get(Role::TextDim)),
        );
        // A refused `Import` says why here and keeps the dialog open: the file is the operator's choice, and the
        // report above is what they were reading when they made it.
        if let Some(sentence) = &refusal {
          ui.add_space(metrics.space(2));
          ui.label(
            RichText::new(sentence)
              .size(theme::CAPTION)
              .color(palette.get(Role::Danger)),
          );
        }
        ui.add_space(metrics.space(4));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
          // The primary action is the filled one, the way the reference's `Importar` is: the modal exists to be
          // confirmed or left, and two plain buttons would make the two look alike.
          let go = ui.add(
            egui::Button::new(
              RichText::new(IMPORT_COMMIT)
                .size(theme::SMALL)
                .color(palette.get(Role::TextHi)),
            )
            .fill(palette.get(Role::Sel)),
          );
          commit = go.clicked();
          let back = ui.button(RichText::new(IMPORT_CANCEL).size(theme::SMALL));
          cancel = back.clicked();
        });
      });

    // The checkbox is a draft until `Import` writes it, so ticking it needs no report of its own — and it is
    // held for the session, like the tolerance, because a shop that imports onto its own sheets says so once
    // rather than once per file.
    self.import_adjust_material = adjust_material;
    if cancel {
      self.close_import();
    } else if let Some(tolerance) = retolerance {
      // The tolerance is not a draft: the report above it has to describe what `Import` will write, so a
      // changed number re-parses the file here, and a refusal (`Err`) leaves the field on the number that
      // report used. Skipped on the `Cancel` frame, so leaving the modal never reports a re-parse of a file
      // nobody is describing any more.
      let _ = self.set_import_tolerance(tolerance);
    }
    if commit {
      // Through the method a headless test calls, so the button and the test cannot disagree about what
      // `Import` does. A refusal sets `import_error` and leaves the modal drawing it.
      let _ = self.confirm_import(app);
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
