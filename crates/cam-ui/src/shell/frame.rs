use cam_app::AppState;
use egui::{Context, Sense};

use crate::icons::{self, Family, Icon};
use crate::theme::{self, Role};
use crate::{Shell, ShellOutput};

impl Shell {
  /// Build one frame.
  ///
  /// Actions are returned rather than applied: one frame of latency is invisible, and a pure shell
  /// is what makes the interface testable.
  pub fn show(&mut self, app: &mut AppState, ctx: &Context) -> ShellOutput {
    // Before the root `Ui` exists, because a `Ui` captures its style when it is created: publishing the
    // tokens from inside the frame would reach the next frame's widgets and not this one's.
    self.publish_theme(ctx);
    // egui 0.37 stacks panels on the root `Ui`; every panel of one frame goes through one borrow.
    ctx.root_ui(|ui| self.show_in(app, ui))
  }

  /// Publish the tokens into a context, once per change.
  ///
  /// Public because a host that owns its own root `Ui` (eframe hands one in) has to call this before it
  /// builds that `Ui`; [`Self::show`] calls it for hosts that do not.
  pub fn publish_theme(&mut self, ctx: &Context) {
    // The cache is only valid for the context it was written into: a host may replace the context, and
    // a fresh one has none of the named styles the shell asks for. The marker in the style itself is
    // therefore the authority, and `applied_theme` is only the fast path.
    let styled = ctx
      .style_of(self.theme.egui())
      .text_styles
      .contains_key(&theme::title());
    if !styled || self.applied_theme != Some(self.theme) {
      theme::apply(ctx, self.theme);
      self.applied_theme = Some(self.theme);
    }
  }

  /// The whole glyph set, on one window.
  ///
  /// A glyph is only checkable against its neighbours: one drawn at a 2.4 stroke looks fine alone and
  /// wrong beside sixty-five drawn at 1.6. The window is deliberately plain — a grid and names —
  /// because it is an instrument, not a screen the operator ever sees.
  fn icon_sheet(&mut self, ctx: &Context) {
    let palette = self.palette();
    let metrics = self.metrics();
    let mut open = self.show_icons;
    egui::Window::new("Icons")
      .open(&mut open)
      .resizable(true)
      .default_width(760.0)
      .show(ctx, |ui| {
        ui.label(
          egui::RichText::new(format!(
            "{} glyphs · stroke {} in a {} box · F2 closes",
            Icon::ALL.len(),
            metrics.icon_stroke,
            metrics.icon_size
          ))
          .size(theme::CAPTION)
          .color(palette.get(Role::TextDim)),
        );
        egui::ScrollArea::vertical()
          .id_salt("icon-sheet")
          .show(ui, |ui| {
            // The artboard's structure: six families, each with its count, so seventy glyphs read as six
            // arguments instead of one wall. `min_col_width` keeps a long name like `toolpath-profile`
            // inside its cell instead of printing over its neighbour.
            for family in Family::ALL {
              ui.add_space(metrics.space(3));
              ui.label(
                egui::RichText::new(format!(
                  "{}  ·  {}",
                  family.label(),
                  Icon::in_family(family).count()
                ))
                .size(theme::SMALL)
                .color(palette.get(Role::TextHi)),
              );
              egui::Grid::new(family.label())
                .num_columns(8)
                .min_col_width(84.0)
                .spacing([metrics.space(4), metrics.space(3)])
                .show(ui, |ui| {
                  for (index, icon) in Icon::in_family(family).enumerate() {
                    ui.vertical(|ui| {
                      ui.spacing_mut().item_spacing.y = metrics.space(0);
                      let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(28.0, 28.0), Sense::hover());
                      let scale = rect.width() / metrics.icon_size;
                      icons::paint(
                        ui.painter(),
                        rect,
                        icon,
                        palette.get(Role::Text),
                        metrics.icon_stroke * scale,
                      );
                      ui.label(
                        egui::RichText::new(icon.name())
                          .size(theme::CAPTION)
                          .color(palette.get(Role::TextDim)),
                      );
                    });
                    if index % 8 == 7 {
                      ui.end_row();
                    }
                  }
                });
            }
          });
      });
    self.show_icons = open;
  }

  /// Build one frame into an existing root [`egui::Ui`].
  ///
  /// `eframe` hands the application a root `Ui` directly, and calling [`Self::show`] from inside that
  /// callback would try to borrow the root twice. Hosts that already hold one use this.
  pub fn show_in(&mut self, app: &mut AppState, ui: &mut egui::Ui) -> ShellOutput {
    let mut output = ShellOutput::default();
    let ctx = ui.ctx().clone();
    // A host may hand in a `Ui` it built before calling us; the styles are still worth publishing for
    // whatever this frame creates from here on, and `publish_theme` is idempotent.
    self.publish_theme(&ctx);
    // F2 opens the glyph set: the vocabulary is part of the design, so it has to be inspectable from
    // inside the app that uses it rather than only from the artboard.
    if ctx.input(|input| input.key_pressed(egui::Key::F2)) {
      self.show_icons = !self.show_icons;
    }
    if self.show_icons {
      self.icon_sheet(&ctx);
    }
    // `F1` is the same idea one level up: `F2` shows the vocabulary of shapes, `F1` shows the vocabulary
    // of commands with the two sentences each one teaches (T8, S2, S4).
    if ctx.input(|input| input.key_pressed(egui::Key::F1)) {
      self.show_help = !self.show_help;
    }
    // `Esc` closes the panel and touches nothing else, so reading help can never cancel a gesture the
    // operator did not mean to cancel.
    if self.show_help && ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
      self.show_help = false;
    }
    if self.show_help {
      self.help_window(&ctx, &mut output);
    }
    // The material dialog is the shell's one modal: `Esc` leaves it without writing, the rule the help panel
    // already follows, and `Continue` is the only thing in the dialog that touches the document.
    if self.show_material && ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
      self.show_material = false;
    }
    if self.show_material {
      self.material_dialog(&ctx, app, &mut output);
    }
    // The tool library is the shell's second modal (camui-prancheta S9), and `Esc` leaves it without writing
    // for the reason the material dialog's does: the choice is a draft until `Use this tool` says otherwise.
    // An import question laid over it answers `Esc` itself (tool-library-app S5): the question closes and the
    // library behind it stays open.
    let import_question_open = self.pending_tool_import.is_some();
    if import_question_open && ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
      self.cancel_tool_import();
    } else if self.show_tool_library && ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
      self.close_tool_library();
    }
    if self.pending_tool_import.is_some() {
      self.tool_import_question_dialog(&ctx, &mut output);
      if self.pending_tool_import.is_none() && self.show_tool_library {
        self.tool_library_dialog(&ctx, app, &mut output);
      }
    } else if self.show_tool_library {
      self.tool_library_dialog(&ctx, app, &mut output);
    }
    // The script panel is the shell's third modal (M10 T4b, S4), and `Esc` closes it for the reason the
    // other two close: the panel holds no edit of its own, so leaving it has nothing to undo — what `Run`
    // changed is in the document, one undo step at a time (M11 T2).
    if self.show_script && ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
      self.show_script = false;
    }
    if self.show_script {
      self.script_dialog(&ctx, app);
    }
    // The import modal is the shell's fourth (shell-file-operations S3), and `Esc` leaves it without writing for
    // the reason the other three close: the file has only been read, so cancelling costs the parse and nothing
    // else — and the `Import` button is the only thing that replaces the drawing.
    if self.show_import && ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
      self.close_import();
    }
    if self.show_import {
      self.import_dialog(&ctx, app, &mut output);
    }
    // The crate's only clock read (A10). Everything that needs time — playback, and nothing else —
    // receives this delta as an argument, so the arithmetic stays a pure function a test can drive
    // frame by frame. `stable_dt` rather than `unstable_dt`: a frame that stalled (a resize, a
    // breakpoint) must not jump the animation forward by the length of the stall.
    let dt = f64::from(ctx.input(|input| input.stable_dt));
    // A modal owns the pointer, so the canvas reports no cursor and finds no snap target while one is open:
    // a status bar naming a world position under the backdrop would be describing a canvas nobody can aim at.
    if !self.show_material
      && !self.show_tool_library
      && !self.show_script
      && !self.show_import
      && self.pending_tool_import.is_none()
    {
      self.track_pointer(app, &ctx);
    }
    // Bars first, then the sides, then the canvas: egui lays panels out in the order they are added,
    // and a central panel must come last or it would claim the space the sides need.
    self.app_bar(app, ui, &mut output);
    self.status_bar(app, ui, &mut output);
    self.palette_panel(app, ui, &mut output);
    self.inspector(app, ui, &mut output);
    // One central area, and the mode decides how it is divided: the drawing alone, or the drawing and the 3D
    // half side by side (camui-prancheta S8).
    self.canvas(app, ui, &mut output, dt);
    if self.show_shortcuts {
      self.shortcuts_window(&ctx);
    }
    output
  }
}
