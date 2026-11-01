//! The shell's frame: the app bar, the stage bar, the rail and the inspector.
//!
//! `design/camui-shell/01-shell-2d.html` is the visual spec. What this module implements is the
//! *structure* that artboard argues for, mapped onto the widgets the crate already has rather than
//! copied pixel for pixel:
//!
//! ```text
//! ┌───────────────────────────────────────────────────────────┐  app bar    40 px
//! │ mark  campilot   ↶ ↷   untitled      mm ☾ ? fit [G-code]  │
//! ├───────────────────────────────────────────────────────────┤  stage bar  32 px
//! │ 1 Design 4 › 2 Edit › 3 Relief › 4 Toolpaths 3 › 5 Prove │
//! ├────┬───────────────────────────────────────┬──────────────┤
//! │ 52 │                                       │ inspector    │
//! │ px │              canvas                   │ 288 px       │
//! │    │                                       │ (collapsible)│
//! ├────┴───────────────────────────────────────┴──────────────┤  status bar 26 px
//! ```
//!
//! Three decisions are worth stating, because each is a place a shell usually lies.
//!
//! **The stage is view state, not a command.** Switching stage changes what the rail offers and which
//! inspector tab opens first; it pushes no [`UiAction`] and never touches the document. That is what
//! lets the stage bar be clickable before any geometry exists.
//!
//! **Counts are counted, never invented.** A stage shows a number only when the document can prove it:
//! vectors for Design, toolpaths for Toolpaths. The rest show a dash and the inspector says in words
//! where those commands live today (`cam-cli`). A stage bar claiming `0 problems` for a validation
//! nobody runs is worse than one that shows nothing.
//!
//! **The rail is stage-scoped, and that is the point of it.** Sixty-odd commands do not fit on one
//! screen and never did; the rail shows the current stage's tools, so "what can I do right now" is one
//! glance instead of one search. The cost — an expert pays a stage switch — is the trade the artboard
//! names.

use cam_app::{AppState, Transform, VertexEdit};
use cam_geom::{Point2, Vec2};
use egui::{Align, Layout, RichText, Sense};

use crate::help::{Command, Group, MenuScope};
use crate::icons::Icon;
use crate::theme::{self, Palette, Role, Theme};
use crate::{Shell, ShellOutput, Tool, UiAction, DEFAULT_TOLERANCE_MM};

/// One palette chip: the glyph and the command it runs.
type PaletteChip = (Icon, Command);

/// A labelled run of palette chips: the group's name and the chips under it.
type PaletteRun = (Group, Vec<PaletteChip>);

/// The stages a job walks through, in the order the artboard draws them.
///
/// [`Stage::Design`] comes first because it is the only stage the shell can complete end to end today;
/// the rest name work that lives in `cam-cli` (M4–M7) until the shell grows the panels for it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Stage {
  /// Vectors: draw, import, trace.
  #[default]
  Design,
  /// Vector surgery: nodes, trim, fillet, offsets, transforms.
  Edit,
  /// Relief and 3D work.
  Relief,
  /// Strategies: profile, pocket, drill, V-carve.
  Toolpaths,
  /// Simulate, verify, emit.
  Prove,
}

/// Which inspector tab is showing.
///
/// Three tabs, and not the artboard's three: our document has no toolpath *parameters* to show yet (a
/// toolpath is authored by `cam-cli` and arrives finished), so the middle tab holds the authoring
/// commands that produce the numbers the Selection tab then reports.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
  /// The commands that make and change geometry.
  #[default]
  Tools,
  /// What the current selection is, in numbers.
  Selection,
  /// Stock, thickness, zero.
  Job,
}

impl Stage {
  /// Every stage, in the order the bar draws them.
  pub const ALL: [Stage; 5] = [
    Stage::Design,
    Stage::Edit,
    Stage::Relief,
    Stage::Toolpaths,
    Stage::Prove,
  ];

  /// The word on the stage chip.
  pub fn label(self) -> &'static str {
    match self {
      Self::Design => "Design",
      Self::Edit => "Edit",
      Self::Relief => "Relief",
      Self::Toolpaths => "Toolpaths",
      Self::Prove => "Prove",
    }
  }

  /// What this stage asks of the operator, in the shell's own voice.
  ///
  /// The inspector shows this when a stage offers no tools of its own, so an empty rail is an
  /// explanation rather than a dead end.
  pub fn note(self) -> &'static str {
    match self {
      Self::Design => {
        "Draw or import vectors. Arm a tool and drag on the canvas; Select picks and marquees."
      }
      Self::Edit => {
        "Work the vectors: nodes, offsets, transforms. The numbers live in Tools; the rail runs them."
      }
      Self::Relief => {
        "Reliefs are authored with `cam-cli` today: build one from a model or a vector profile there, \
         then simulate it."
      }
      Self::Toolpaths => {
        "Toolpaths are authored with `cam-cli toolpath` today. They arrive in the document finished, \
         and are counted above."
      }
      Self::Prove => {
        "Simulation and G-code live in `cam-cli simulate` and `cam-cli emit` until the shell grows the \
         panels for them."
      }
    }
  }

  /// The number this stage can prove from the document, if any.
  ///
  /// A dash means "nothing to count yet", never "zero". The distinction matters: an empty sheet really
  /// does hold zero vectors, while a count of zero problems would be a claim about a check nobody ran.
  fn count(self, app: &AppState) -> String {
    match self {
      Self::Design => app
        .document()
        .layers
        .iter()
        .map(|layer| layer.entities.len())
        .sum::<usize>()
        .to_string(),
      Self::Toolpaths => app.document().toolpaths.len().to_string(),
      Self::Edit | Self::Relief | Self::Prove => "—".to_owned(),
    }
  }

  /// The palette runs this stage adds, in the order the table lists them.
  ///
  /// Only Design has armed canvas gestures today; Edit offers the commands that work on what already
  /// exists, and the other stages offer nothing but VIEW, which every stage shows. The difference is
  /// visible in the palette rather than papered over.
  fn runs(self) -> &'static [Group] {
    match self {
      Self::Design => &[Group::Draw],
      Self::Edit => &[Group::Transform, Group::VectorEdit],
      Self::Relief | Self::Toolpaths | Self::Prove => &[],
    }
  }
}

impl Tab {
  /// Every tab, left to right.
  pub const ALL: [Tab; 3] = [Tab::Tools, Tab::Selection, Tab::Job];

  /// The word on the tab.
  pub fn label(self) -> &'static str {
    match self {
      Self::Tools => "Tools",
      Self::Selection => "Selection",
      Self::Job => "Job",
    }
  }
}

impl Shell {
  /// The stage the shell is working in.
  pub fn stage(&self) -> Stage {
    self.stage
  }

  /// Switch stage. View state: no command is pushed and the document is not touched.
  pub fn set_stage(&mut self, stage: Stage) {
    self.stage = stage;
    // A stage brings its own tools, so the tab that holds them is the useful one to reveal. The
    // operator can still pick another tab; this only decides what the switch shows.
    self.inspector_tab = match stage {
      Stage::Design | Stage::Edit => Tab::Tools,
      Stage::Relief | Stage::Toolpaths | Stage::Prove => Tab::Job,
    };
  }

  /// Which inspector tab is showing.
  pub fn tab(&self) -> Tab {
    self.inspector_tab
  }

  /// Show a tab.
  pub fn set_tab(&mut self, tab: Tab) {
    self.inspector_tab = tab;
  }

  /// Whether the inspector is expanded.
  pub fn inspector_open(&self) -> bool {
    self.inspector_open
  }

  /// Expand or collapse the inspector. Collapsed it leaves a 12 px strip, and touching that strip
  /// opens it again — the canvas gets the width without the panel becoming unreachable.
  pub fn set_inspector_open(&mut self, open: bool) {
    self.inspector_open = open;
  }

  /// The app bar: what this document is, what can be undone, and the one action that costs material.
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

          // Undo and Redo are the table's HISTORY run: their words, glyphs and actions come from the
          // command that owns them, so the app bar cannot answer to a different name than the help
          // panel teaches. Only the chord is the bar's own, because the keys window lists it too.
          for (command, enabled, keys) in [
            (Command::Undo, app.can_undo(), "Ctrl+Z"),
            (Command::Redo, app.can_redo(), "Ctrl+Y"),
          ] {
            let tooltip = format!("{} — {keys}", command.label());
            if icon_button(
              ui,
              &palette,
              command_icon(command),
              &tooltip,
              false,
              enabled,
              18.0,
            )
            .clicked()
            {
              output.actions.extend(command.action());
            }
            output.drawn.app_bar.push(command.label().to_owned());
          }
          ui.separator();

          // The document has no file name yet: `cam-cli` owns document paths (M0), and the shell only
          // knows whether the frame it holds is modified.
          let dirty = if app.is_dirty() { " · modified" } else { "" };
          ui.label(
            RichText::new(format!("untitled.c2d{dirty}"))
              .size(theme::CAPTION)
              .color(palette.get(Role::TextDim)),
          );

          ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            // The primary action, disabled on purpose: there is nothing to emit until the shell can
            // author a toolpath, and a button that silently does nothing would be worse than one that
            // says why.
            ui.add_enabled(false, egui::Button::new("Generate G-code"))
              .on_disabled_hover_text(
                "Toolpaths are authored with `cam-cli toolpath` today; this button writes the program \
                 once the shell can author them.",
              );
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
            // The view toggle the artboard floats over the canvas. It lives in the app bar instead, and it
            // is a segmented control because the choice is exclusive: two alternatives, visible at once, so
            // choosing is a comparison and not a memory test.
            if let Some(pick) = segmented(ui, &palette, &["2D", "3D"], usize::from(self.show_3d)) {
              self.show_3d = pick == 1;
            }
            ui.separator();
            ui.label(
              RichText::new("mm")
                .size(theme::CAPTION)
                .color(palette.get(Role::TextDim)),
            )
            .on_hover_text("Working units. Every length in the shell is millimetres.");
            ui.separator();
            if icon_button(
              ui,
              &palette,
              command_icon(Command::FitJob),
              &format!("{} — F", Command::FitJob.label()),
              false,
              true,
              18.0,
            )
            .clicked()
            {
              output.actions.extend(Command::FitJob.action());
            }
            output.drawn.app_bar.push(Command::FitJob.label().to_owned());
          });
        });
      });
  }

  /// The stage bar: where the job stands, and what each stage can prove.
  pub(crate) fn stage_bar(&mut self, app: &AppState, ui: &mut egui::Ui) {
    let palette = self.palette();
    egui::Panel::top("stage_bar").show(ui, |ui| {
      ui.horizontal(|ui| {
        for (index, stage) in Stage::ALL.iter().enumerate() {
          if index > 0 {
            ui.label(
              RichText::new("›")
                .size(theme::CAPTION)
                .color(palette.get(Role::TextOff)),
            );
          }
          let text =
            RichText::new(format!("{}  {}", stage.label(), stage.count(app))).size(theme::CAPTION);
          if ui
            .selectable_label(*stage == self.stage, text)
            .on_hover_text(stage.note())
            .clicked()
          {
            self.set_stage(*stage);
          }
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
          let job = &app.document().job;
          ui.label(
            RichText::new(format!(
              "{:.0} × {:.0} × {:.0} mm",
              job.width.mm(),
              job.height.mm(),
              job.thickness.mm()
            ))
            .size(theme::CAPTION)
            .color(palette.get(Role::TextDim)),
          )
          .on_hover_text("Stock: the sheet this job cuts out of.");
        });
      });
    });
  }

  /// The left palette: the current stage's commands, grouped and labelled.
  ///
  /// The reference app settles a question this shell got wrong first. A rail of unlabelled glyphs stops
  /// working the moment there are more than a dozen of them — which is why the seven Edit commands had
  /// to hide behind icons nobody could name. Twenty commands in labelled groups stay findable, and the
  /// label is the word a machinist already uses.
  ///
  /// Every word, run, glyph and command here comes from [`crate::help`]: the palette holds no list of
  /// its own, which is what closed the two-lists-one-command debt T8 inherited (L14).
  pub(crate) fn palette_panel(
    &mut self,
    app: &AppState,
    ui: &mut egui::Ui,
    output: &mut ShellOutput,
  ) {
    let palette = self.palette();
    let metrics = self.metrics();
    egui::Panel::left("palette")
      .exact_size(metrics.palette_width)
      .show(ui, |ui| {
        egui::ScrollArea::vertical()
          .id_salt("palette")
          .show(ui, |ui| {
            let mut offered: Vec<String> = Vec::new();
            let mut hits: Vec<(String, (f32, f32))> = Vec::new();

            // A stage that brings no run of its own explains itself, so an empty rail is an answer and
            // not a dead end.
            if self.stage.runs().is_empty() {
              let title = self.stage.label().to_uppercase();
              section(ui, &palette, &title);
              note(ui, &palette, self.stage.note());
            }

            // The runs arrive in the table's own order: DRAW, TRANSFORM, VECTOR EDIT and VIEW.
            for (group, chips) in self.palette_runs() {
              if group == Group::View {
                ui.add_space(metrics.space(3));
              }
              section(ui, &palette, group.label());
              ui.horizontal_wrapped(|ui| {
                for (icon, command) in chips {
                  let response = icon_button(
                    ui,
                    &palette,
                    icon,
                    &palette_tooltip(command),
                    self.palette_active(command),
                    true,
                    20.0,
                  );
                  if response.clicked() {
                    self.run_command(command, app, output);
                  }
                  let centre = response.rect.center();
                  offered.push(command.label().to_owned());
                  hits.push((command.label().to_owned(), (centre.x, centre.y)));
                }
              });
            }

            // What the palette offered: `DrawnState` is the shell's report of what it drew, and the
            // palette is the part of the frame that carries no text of its own.
            output.drawn.stage = self.stage;
            output.drawn.palette = offered;
            output.drawn.palette_hits = hits;
          });
      });
  }

  /// The palette's runs for the current stage, derived from the command table.
  ///
  /// One source: [`Command::ALL`] decides the words ([`Command::label`]), the runs ([`Command::group`]),
  /// the glyphs and the commands themselves; the stage decides which runs it shows, and VIEW is every
  /// stage's. The table's order is the order the runs and the chips are drawn in, so a command that moves
  /// in the table moves in the palette.
  ///
  /// HISTORY cannot reach the palette even by accident: [`Command::in_palette`] leaves its two commands
  /// to the app bar, and no stage claims that run.
  fn palette_runs(&self) -> Vec<PaletteRun> {
    let mut runs: Vec<PaletteRun> = Vec::new();
    for command in Command::ALL
      .iter()
      .copied()
      .filter(|command| command.in_palette())
    {
      let group = command.group();
      if group != Group::View && !self.stage.runs().contains(&group) {
        continue;
      }
      let chip = (command_icon(command), command);
      match runs.last_mut() {
        Some((name, chips)) if *name == group => chips.push(chip),
        _ => runs.push((group, vec![chip])),
      }
    }
    runs
  }

  /// Run one command: arm its gesture, flip the view flag it owns, or push its action.
  ///
  /// The cases are the table's own distinction — [`Command::tool`] for a gesture, the view commands that
  /// only change what the frame shows, and [`Command::action`] for everything that reaches the document —
  /// plus the commands whose action carries a number the Tools tab holds.
  ///
  /// Both mice meet here. A chip in the palette and a row in the canvas context menu run a command the same
  /// way, which is what lets the menu offer the table's commands without owning a second copy of any of
  /// them; the view arms below that no palette chip reaches (`3D`, `Theme`) are the canvas menu's, because a
  /// toggle is the one thing a menu row can be that a command's own action cannot express.
  fn run_command(&mut self, command: Command, app: &AppState, output: &mut ShellOutput) {
    if let Some(tool) = command.tool() {
      self.active_tool = tool;
      return;
    }
    match command {
      Command::Grid => self.snapping = !self.snapping,
      Command::Layers => self.inspector_open = !self.inspector_open,
      Command::Help => self.show_help = !self.show_help,
      Command::Shortcuts => self.show_shortcuts = !self.show_shortcuts,
      Command::View3d => self.show_3d = !self.show_3d,
      Command::Theme => self.flip_theme(),
      _ => output.actions.extend(self.command_action(command, app)),
    }
  }

  /// Switch the colour set, from the app bar's button or from the canvas menu's `Theme` row.
  fn flip_theme(&mut self) {
    self.theme = match self.theme {
      Theme::Dark => Theme::Light,
      Theme::Light => Theme::Dark,
    };
  }

  /// The action a command pushes, built from the numbers the Tools tab holds.
  ///
  /// These are the same constructions the Tools tab draws as labelled buttons, and this is the one place
  /// they are built now: the rail, the menu and the tab read the same fields, so a number typed once is the
  /// number all three run. `Node edit` is the Tools tab's *Move node*, because a menu row is one command and
  /// moving the node at the index the tab holds is the one of the three that reads a single index.
  fn command_action(&self, command: Command, app: &AppState) -> Option<UiAction> {
    let centre = app.camera().center();
    match command {
      Command::AddText => Some(UiAction::AddText {
        text: self.text.clone(),
        height: self.text_height,
        origin: centre,
      }),
      Command::Move => Some(UiAction::Transform(Transform::Translate {
        delta: Vec2::new(self.move_delta.0, self.move_delta.1),
      })),
      Command::Rotate => Some(UiAction::Transform(Transform::Rotate {
        radians: self.rotate_degrees.to_radians(),
        about: centre,
      })),
      Command::Scale => Some(UiAction::Transform(Transform::Scale {
        sx: self.scale_factors.0,
        sy: self.scale_factors.1,
        about: centre,
        tolerance: DEFAULT_TOLERANCE_MM,
      })),
      Command::Mirror => Some(UiAction::Transform(Transform::Mirror {
        point: centre,
        direction: Vec2::Y_AXIS,
      })),
      Command::Array => Some(UiAction::Array {
        rows: self.array_grid.0,
        columns: self.array_grid.1,
        dx: self.array_spacing.0,
        dy: self.array_spacing.1,
      }),
      Command::Offset => Some(UiAction::Offset {
        distance: self.offset_distance,
      }),
      Command::NodeEdit => Some(UiAction::EditVertex(VertexEdit::Move {
        index: self.vertex_index,
        to: Point2::new(self.vertex_position.0, self.vertex_position.1),
      })),
      Command::Trim => trim_action(app),
      _ => command.action(),
    }
  }

  /// Whether a chip shows its command as on: the armed gesture, snapping, or the open inspector.
  fn palette_active(&self, command: Command) -> bool {
    match command {
      Command::Grid => self.snapping,
      Command::Layers => self.inspector_open,
      _ => command.tool().is_some_and(|tool| tool == self.active_tool),
    }
  }

  /// The canvas context menu: the commands that act on what the pointer was over.
  ///
  /// This is the shell's first secondary-button gesture. egui's [`egui::Response::context_menu`] opens the
  /// popup at the pointer on a right-click and closes it on the next click, so nothing here tracks open or
  /// closed: the popup's own memory does, and this only fills it with the table's rows. What it offers comes
  /// from [`MenuScope`] — decided by the pointer's right-click in `Shell::handle_input`, because the
  /// selection cannot answer it (a right-click on empty canvas keeps an existing selection and still means
  /// the canvas menu).
  ///
  /// A row is a [`Command`], so its word, its teaching and the rule that runs it are already the shell's;
  /// a disabled row is a command that would refuse, and its `how` says why. The two report fields —
  /// [`crate::DrawnState::menu`] and [`crate::DrawnState::menu_hits`] — exist for the same reason the
  /// palette's do: egui cannot be asked for a widget by its text from outside a frame.
  pub(crate) fn canvas_menu(
    &mut self,
    app: &AppState,
    response: &egui::Response,
    output: &mut ShellOutput,
  ) {
    let scope: MenuScope = self.menu_scope;
    response.context_menu(|ui| {
      for command in scope.commands() {
        let label = command.label();
        let enabled = menu_enabled(command, app);
        let entry = ui.add_enabled(
          enabled,
          egui::Button::new(RichText::new(label).size(theme::SMALL)),
        );
        // The row teaches what the command gives and costs, in the words the `F1` panel uses for it: one
        // sentence of consequence when it can run, and the instruction that names the missing operand when
        // it cannot.
        let entry = if enabled {
          entry.on_hover_text(command.help().what)
        } else {
          entry.on_disabled_hover_text(command.help().how)
        };
        let centre = entry.rect.center();
        output.drawn.menu.push(label.to_owned());
        output
          .drawn
          .menu_hits
          .push((label.to_owned(), (centre.x, centre.y)));
        if entry.clicked() {
          self.run_command(command, app, output);
          // A row runs one command and the menu is done. egui's menu popup would also close on this click,
          // but saying it here means the row does not depend on that default staying what it is.
          ui.close();
        }
      }
    });
  }

  /// The right inspector: the stage's commands, the selection's numbers, the job — and the layers,
  /// which stay put because they are a document fact rather than a stage's opinion.
  pub(crate) fn inspector(&mut self, app: &AppState, ui: &mut egui::Ui, output: &mut ShellOutput) {
    if !self.inspector_open {
      self.inspector_strip(ui);
      return;
    }
    let palette = self.palette();
    let metrics = self.metrics();
    egui::Panel::right("inspector")
      .exact_size(metrics.dock_width)
      .show(ui, |ui| {
        ui.horizontal(|ui| {
          ui.label(RichText::new("Inspector").size(theme::TITLE));
          ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if collapse_button(ui, &palette).clicked() {
              self.inspector_open = false;
            }
          });
        });
        ui.separator();

        ui.horizontal(|ui| {
          for tab in Tab::ALL {
            if ui
              .selectable_label(
                self.inspector_tab == tab,
                RichText::new(tab.label()).size(theme::CAPTION),
              )
              .clicked()
            {
              self.inspector_tab = tab;
            }
          }
        });

        let body = (ui.available_height() * 0.55).max(120.0);
        egui::ScrollArea::vertical()
          .id_salt("inspector_body")
          .max_height(body)
          .show(ui, |ui| {
            ui.add_space(metrics.space(2));
            match self.inspector_tab {
              Tab::Tools => {
                if self.stage.runs().is_empty() {
                  note(ui, &palette, self.stage.note());
                } else {
                  self.authoring(app, ui, output);
                }
              }
              Tab::Selection => self.properties(app, ui, output),
              Tab::Job => self.job_section(app, ui, &palette),
            }
          });

        ui.separator();
        ui.label(RichText::new("Layers").size(theme::TITLE));
        egui::ScrollArea::vertical()
          .id_salt("inspector_layers")
          .show(ui, |ui| self.layer_dock(app, ui, output));
      });
  }

  /// The 12 px strip the inspector leaves behind when it is collapsed.
  fn inspector_strip(&mut self, ui: &mut egui::Ui) {
    let palette = self.palette();
    egui::Panel::right("inspector_strip")
      .exact_size(12.0)
      .frame(
        egui::Frame::NONE
          .fill(palette.get(Role::Panel))
          .stroke(egui::Stroke::new(1.0, palette.get(Role::Hairline))),
      )
      .show(ui, |ui| {
        // The panel's own rect, not `available_size`: a panel adds a margin, and a 12 px strip minus
        // that margin rounds down to nothing, which is a hover target nobody can hit.
        //
        // The pointer is read straight from the frame's input rather than from a widget's hover state,
        // because egui resolves hover from the interaction the previous pass ended with: a strip that
        // only wakes on the third frame of hovering is a strip that feels broken.
        let rect = ui.max_rect();
        let hovering = ui
          .ctx()
          .pointer_hover_pos()
          .is_some_and(|position| rect.contains(position));
        let ink = if hovering {
          palette.get(Role::Sel)
        } else {
          palette.get(Role::TextOff)
        };
        if hovering {
          // Opening on hover is what makes collapsing usable instead of modal: the panel comes back
          // when the pointer touches the edge, with no click and no chord to remember.
          self.inspector_open = true;
        }
        let centre = rect.center();
        for (from, to) in [
          (egui::vec2(-2.0, -3.0), egui::vec2(2.0, 0.0)),
          (egui::vec2(2.0, 0.0), egui::vec2(-2.0, 3.0)),
        ] {
          ui.painter()
            .line_segment([centre + from, centre + to], egui::Stroke::new(1.6, ink));
        }
      });
  }

  /// The job tab: what is being cut, and out of what.
  ///
  /// Fields are labelled above and two to a row, the way the reference app lays out MATERIAL: a
  /// machinist reads "Width X 600" and "Thickness Z 18" as one block, not as a list of sentences.
  fn job_section(&self, app: &AppState, ui: &mut egui::Ui, palette: &Palette) {
    let job = &app.document().job;
    section(ui, palette, "MATERIAL");
    egui::Grid::new("job-material")
      .num_columns(2)
      .spacing([8.0, 6.0])
      .show(ui, |ui| {
        field(ui, palette, "Width X", &format!("{:.0} mm", job.width.mm()));
        field(
          ui,
          palette,
          "Height Y",
          &format!("{:.0} mm", job.height.mm()),
        );
        ui.end_row();
        field(
          ui,
          palette,
          "Thickness Z",
          &format!("{:.0} mm", job.thickness.mm()),
        );
        field(ui, palette, "Grid", &format!("{:.2} mm", self.grid_spacing));
        ui.end_row();
      });

    section(ui, palette, "DOCUMENT");
    egui::Grid::new("job-document")
      .num_columns(2)
      .spacing([8.0, 6.0])
      .show(ui, |ui| {
        field(
          ui,
          palette,
          "Toolpaths",
          &app.document().toolpaths.len().to_string(),
        );
        field(
          ui,
          palette,
          "Layers",
          &app.document().layers.len().to_string(),
        );
        ui.end_row();
      });
    ui.add_space(6.0);
    note(ui, palette, Stage::Toolpaths.note());
  }

  /// Toggle one kind of snap target.
  ///
  /// The chips in the status bar call this and nothing else, so the rule lives in one place: at least one
  /// kind stays armed, because a snapping strip with everything off is a magnet with no field.
  pub fn toggle_snap(&mut self, kind: cam_view::SnapKind) {
    match self.snap_kinds.iter().position(|armed| *armed == kind) {
      Some(index) if self.snap_kinds.len() > 1 => {
        self.snap_kinds.remove(index);
      }
      Some(_) => {}
      None => self.snap_kinds.push(kind),
    }
  }

  /// Whether a snap kind is armed.
  pub fn snap_armed(&self, kind: cam_view::SnapKind) -> bool {
    self.snap_kinds.contains(&kind)
  }

  /// The line the status bar shows at its right end: what the current gesture will do.
  ///
  /// The reference app keeps a sentence like this next to its snap chips ("Clique para selecionar · arraste
  /// da esquerda p/ direita"), and it is the cheapest teaching in the whole interface: it answers "what
  /// happens if I drag now" without a manual, and it changes with the tool instead of lying about it.
  pub fn hint(&self) -> String {
    if self.show_3d {
      return "3D — drag to orbit, wheel to zoom; press 2D for the drawing".to_owned();
    }
    if self.active_tool.draws() {
      return format!(
        "{} — drag on the sheet to place it; Esc cancels",
        self.active_tool.label()
      );
    }
    "click to pick · drag left→right keeps what is fully inside · right→left takes what it crosses"
      .to_owned()
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

/// Whether one canvas-menu row can run.
///
/// Only a trim refuses on the strength of the selection alone: it needs a target and a cutter, so over one
/// object the row is drawn dark instead of promising something the command layer would refuse. Every other
/// row in either scope either needs nothing selected or is only offered over an object.
fn menu_enabled(command: Command, app: &AppState) -> bool {
  match command {
    Command::Trim => app.selection().len() >= 2,
    _ => true,
  }
}

/// The trim this selection asks for, if it asks for one.
///
/// A trim needs two operands and the selection has to name which is which: it is ordered by id, so the
/// first id is the target and the second the cutter. This is the one rule both the Tools tab's button and
/// the canvas menu's row use — a trim that cut the other way from one place than the other would be two
/// commands wearing one word.
pub(crate) fn trim_action(app: &AppState) -> Option<UiAction> {
  let selected: Vec<String> = app.selection().ids().map(str::to_owned).collect();
  match selected.as_slice() {
    [target, cutter, ..] => Some(UiAction::Trim {
      target: target.clone(),
      cutter: cutter.clone(),
    }),
    _ => None,
  }
}

/// Which glyph a canvas gesture is drawn with.
fn tool_icon(tool: Tool) -> Icon {
  match tool {
    Tool::Select => Icon::Select,
    Tool::Line => Icon::Line,
    Tool::Rectangle => Icon::Rectangle,
    Tool::Polyline => Icon::Polyline,
    Tool::Arc => Icon::Arc,
    Tool::Circle => Icon::Circle,
    Tool::Ellipse => Icon::Ellipse,
    Tool::Polygon => Icon::Polygon,
    Tool::Star => Icon::Star,
  }
}

/// Which glyph a command is drawn with, wherever the frame draws it as an icon.
///
/// A gesture keeps the glyph its tool has always had ([`tool_icon`]); the rest are the symbols the palette
/// and the app bar already used for them. Each one has to be its own picture: `Shortcuts` drew with
/// [`Icon::Text`] until T8.2 shipped it two runs below the `Add text` chip, and two chips with one glyph
/// are two chips an operator cannot tell apart.
fn command_icon(command: Command) -> Icon {
  if let Some(tool) = command.tool() {
    return tool_icon(tool);
  }
  match command {
    Command::AddText => Icon::Text,
    Command::Move => Icon::Move,
    Command::Rotate => Icon::Rotate,
    Command::Scale => Icon::Scale,
    Command::Mirror => Icon::MirrorH,
    Command::Array => Icon::ArrayGrid,
    Command::Offset => Icon::Offset,
    Command::Join => Icon::Union,
    Command::Grid => Icon::Grid,
    Command::Layers => Icon::Layers,
    Command::Help => Icon::Help,
    Command::Shortcuts => Icon::Shortcuts,
    Command::Undo => Icon::Undo,
    Command::Redo => Icon::Redo,
    Command::FitJob => Icon::ZoomFit,
    // The rest are not drawn as glyphs anywhere in the frame: they are labelled buttons in the Tools tab
    // or reached by a key, so this arm is the honest "no symbol of its own" answer rather than a new
    // symbol nobody has ever seen.
    _ => Icon::Select,
  }
}

/// The words a palette chip shows on hover.
///
/// Every chip is named by its own hover text: the label always leads, and the four that can say more than
/// a word say it after the label, because "Grid" alone does not tell an operator that the chip is a
/// switch. `Shortcuts` shipped as "The keys the shell answers to" — a true sentence that never said
/// *Shortcuts* — so the label leads here instead of being replaced by prose at the call site.
///
/// Public so a headless test can read the words a chip shows on hover: `DrawnState` reports what the
/// palette drew, and the one thing it cannot report is a card that only appears under the pointer.
pub fn palette_tooltip(command: Command) -> String {
  let label = command.label();
  match command {
    Command::Grid => format!("{label} and snapping"),
    Command::Layers => format!("{label} — the inspector, collapse or expand"),
    Command::Help => format!("{label} — F1"),
    Command::Shortcuts => format!("{label} — the keys the shell answers to"),
    _ => label.to_owned(),
  }
}

/// One icon-only control: a square target, a hover wash, and the glyph at the set's own weight.
///
/// The glyph keeps its stroke in every state — pressed, hovered, active — because a rail of twenty
/// icons that thicken as the pointer moves reads as flicker, not as feedback.
fn icon_button(
  ui: &mut egui::Ui,
  palette: &Palette,
  icon: Icon,
  tooltip: &str,
  active: bool,
  enabled: bool,
  glyph: f32,
) -> egui::Response {
  let target = glyph + 12.0;
  let sense = if enabled {
    Sense::click()
  } else {
    Sense::hover()
  };
  let (rect, response) = ui.allocate_exact_size(egui::vec2(target, target), sense);
  let ink = if !enabled {
    palette.get(Role::TextOff)
  } else if active {
    palette.get(Role::Sel)
  } else if response.hovered() {
    palette.get(Role::TextHi)
  } else {
    palette.get(Role::Text)
  };
  let painter = ui.painter();
  let radius = egui::CornerRadius::same(4);
  if active {
    painter.rect_filled(rect, radius, palette.get(Role::SelSoft));
    painter.rect_stroke(
      rect,
      radius,
      egui::Stroke::new(1.0, palette.get(Role::Sel)),
      egui::StrokeKind::Inside,
    );
  } else if response.hovered() && enabled {
    painter.rect_filled(rect, radius, palette.get(Role::Raised));
  }
  let box_size = glyph.min(rect.width() - 4.0);
  let box_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(box_size, box_size));
  crate::icons::paint(
    painter,
    box_rect,
    icon,
    ink,
    theme::ICON_STROKE * box_size / theme::ICON_SIZE,
  );
  if enabled {
    response.on_hover_text(tooltip)
  } else {
    response
  }
}

/// The inspector's collapse affordance: a chevron pointing at the edge the panel retreats to.
fn collapse_button(ui: &mut egui::Ui, palette: &Palette) -> egui::Response {
  let (rect, response) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), Sense::click());
  let ink = if response.hovered() {
    palette.get(Role::TextHi)
  } else {
    palette.get(Role::TextDim)
  };
  let centre = rect.center();
  for (from, to) in [
    (egui::vec2(-2.0, -4.0), egui::vec2(2.0, 0.0)),
    (egui::vec2(2.0, 0.0), egui::vec2(-2.0, 4.0)),
  ] {
    ui.painter()
      .line_segment([centre + from, centre + to], egui::Stroke::new(1.6, ink));
  }
  response.on_hover_text("Collapse the inspector")
}

/// The product mark: a cube in the selection colour, drawn rather than imported.
fn paint_mark(ui: &mut egui::Ui, palette: &Palette, size: f32) {
  let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), Sense::hover());
  crate::icons::paint(
    ui.painter(),
    rect,
    Icon::View3d,
    palette.get(Role::Sel),
    theme::ICON_STROKE * size / theme::ICON_SIZE,
  );
}

/// A chip: a small pill that is either on or off, in the reference's `Ponta | Meio | Centro` strip.
pub(crate) fn chip(ui: &mut egui::Ui, palette: &Palette, label: &str, on: bool) -> egui::Response {
  let font = egui::FontId::proportional(theme::CAPTION);
  let galley = ui
    .painter()
    .layout_no_wrap(label.to_owned(), font.clone(), palette.get(Role::Text));
  let size = egui::vec2(galley.size().x + 14.0, 18.0);
  let (rect, response) = ui.allocate_exact_size(size, Sense::click());
  let radius = egui::CornerRadius::same(9);
  if on {
    ui.painter()
      .rect_filled(rect, radius, palette.get(Role::SelSoft));
    ui.painter().rect_stroke(
      rect,
      radius,
      egui::Stroke::new(1.0, palette.get(Role::Sel)),
      egui::StrokeKind::Inside,
    );
  } else if response.hovered() {
    ui.painter()
      .rect_filled(rect, radius, palette.get(Role::Raised));
  }
  let ink = if on {
    palette.get(Role::Sel)
  } else {
    palette.get(Role::TextDim)
  };
  ui.painter()
    .text(rect.center(), egui::Align2::CENTER_CENTER, label, font, ink);
  response
}

/// A segmented control: mutually exclusive choices in one joined strip.
///
/// The reference uses these for every small decision (`Material | Último vetor`, `Não | De 90° | Livre`,
/// `2D | 3D`) and it is the right shape: the alternatives are visible, so choosing is a comparison instead
/// of a search. Returns the index the operator picked, if any.
pub(crate) fn segmented(
  ui: &mut egui::Ui,
  palette: &Palette,
  labels: &[&str],
  active: usize,
) -> Option<usize> {
  let font = egui::FontId::proportional(theme::CAPTION);
  let mut widths = Vec::with_capacity(labels.len());
  for label in labels {
    let galley =
      ui.painter()
        .layout_no_wrap((*label).to_owned(), font.clone(), palette.get(Role::Text));
    widths.push(galley.size().x + 18.0);
  }
  let total: f32 = widths.iter().sum();
  let (rect, response) = ui.allocate_exact_size(egui::vec2(total, 22.0), Sense::click());
  let radius = egui::CornerRadius::same(11);
  ui.painter()
    .rect_filled(rect, radius, palette.get(Role::Raised));
  ui.painter().rect_stroke(
    rect,
    radius,
    egui::Stroke::new(1.0, palette.get(Role::Border)),
    egui::StrokeKind::Inside,
  );
  let mut left = rect.left();
  let mut clicked = None;
  for (index, label) in labels.iter().enumerate() {
    let cell = egui::Rect::from_min_size(
      egui::Pos2::new(left, rect.top()),
      egui::vec2(widths[index], rect.height()),
    );
    let on = index == active;
    if on {
      ui.painter()
        .rect_filled(cell, radius, palette.get(Role::SelSoft));
      ui.painter().rect_stroke(
        cell,
        radius,
        egui::Stroke::new(1.0, palette.get(Role::Sel)),
        egui::StrokeKind::Inside,
      );
    }
    let ink = if on {
      palette.get(Role::Sel)
    } else {
      palette.get(Role::TextDim)
    };
    ui.painter().text(
      cell.center(),
      egui::Align2::CENTER_CENTER,
      *label,
      font.clone(),
      ink,
    );
    if response.clicked()
      && ui
        .ctx()
        .pointer_hover_pos()
        .is_some_and(|p| cell.contains(p))
    {
      clicked = Some(index);
    }
    left += widths[index];
  }
  clicked
}

/// One labelled value box: the label above, the number inside a bordered field.
///
/// Read-only for now, because nothing in the document's job is editable from the shell yet; the shape is
/// what matters, and it is the shape the reference app uses for every material measurement.
fn field(ui: &mut egui::Ui, palette: &Palette, label: &str, value: &str) {
  ui.vertical(|ui| {
    ui.label(
      RichText::new(label)
        .size(theme::CAPTION)
        .color(palette.get(Role::TextDim)),
    );
    let (rect, _) = ui.allocate_exact_size(egui::vec2(122.0, 24.0), Sense::hover());
    let radius = egui::CornerRadius::same(4);
    ui.painter()
      .rect_filled(rect, radius, palette.get(Role::Raised));
    ui.painter().rect_stroke(
      rect,
      radius,
      egui::Stroke::new(1.0, palette.get(Role::Border)),
      egui::StrokeKind::Inside,
    );
    ui.painter().text(
      rect.right_center() - egui::vec2(8.0, 0.0),
      egui::Align2::RIGHT_CENTER,
      value,
      egui::FontId::monospace(theme::SMALL),
      palette.get(Role::TextHi),
    );
  });
}

/// A palette or inspector section label: small, uppercase, dim, with a hairline under it.
///
/// The reference app labels every group this way and it is the single cheapest thing that made a wall of
/// commands readable: the word is already in the operator's vocabulary, and it costs one line.
fn section(ui: &mut egui::Ui, palette: &Palette, label: &str) {
  ui.add_space(4.0);
  ui.label(
    RichText::new(label)
      .size(theme::CAPTION)
      .color(palette.get(Role::TextDim)),
  );
  let width = ui.available_width();
  let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 1.0), Sense::hover());
  ui.painter()
    .rect_filled(rect, 0.0, palette.get(Role::Hairline));
}

/// A sentence in the shell's teaching voice, used where a stage has nothing to show yet.
fn note(ui: &mut egui::Ui, palette: &Palette, text: &str) {
  ui.label(
    RichText::new(text)
      .size(theme::CAPTION)
      .color(palette.get(Role::TextDim)),
  );
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn shortcuts_has_a_glyph_and_a_word_of_its_own() {
    assert_ne!(
      command_icon(Command::Shortcuts),
      command_icon(Command::AddText),
      "Shortcuts and Add text draw the same glyph, so the two chips are the same picture"
    );
    assert!(
      palette_tooltip(Command::Shortcuts).contains(Command::Shortcuts.label()),
      "the Shortcuts chip's tooltip is {:?}, which never says what it is",
      palette_tooltip(Command::Shortcuts)
    );
  }
}
