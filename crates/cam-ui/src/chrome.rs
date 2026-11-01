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

use cam_app::{AppState, Transform};
use cam_geom::Vec2;
use egui::{Align, Layout, RichText, Sense};

use crate::icons::Icon;
use crate::theme::{self, Palette, Role, Theme};
use crate::{Shell, ShellOutput, Tool, UiAction, DEFAULT_TOLERANCE_MM};

/// One palette entry: the glyph, the word it answers to, and the command it runs.
type PaletteEntry = (Icon, &'static str, UiAction);

/// A labelled run of palette entries: the group's name and the buttons under it.
type PaletteRun = (&'static str, Vec<PaletteEntry>);

/// A rail command: which group it belongs to, its glyph, its word, and the command.
type GroupedCommand = (&'static str, Icon, &'static str, UiAction);

/// The nine canvas gestures, as a `'static` slice so the rail can borrow it.
static DESIGN_TOOLS: [Tool; 9] = Tool::ALL;

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

  /// The armed tools this stage offers, in rail order.
  ///
  /// Only Design has armed canvas gestures today; the other stages offer commands
  /// ([`Shell::rail_commands`]) or nothing at all, and the difference is visible in the rail rather
  /// than papered over.
  fn tools(self) -> &'static [Tool] {
    match self {
      Self::Design => &DESIGN_TOOLS,
      _ => &[],
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

          if icon_button(ui, &palette, Icon::Undo, "Undo — Ctrl+Z", false, app.can_undo(), 18.0)
            .clicked()
          {
            output.actions.push(UiAction::Undo);
          }
          if icon_button(ui, &palette, Icon::Redo, "Redo — Ctrl+Y", false, app.can_redo(), 18.0)
            .clicked()
          {
            output.actions.push(UiAction::Redo);
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
            if icon_button(ui, &palette, Icon::Help, "Shortcuts and help", false, true, 18.0)
              .clicked()
            {
              self.show_shortcuts = !self.show_shortcuts;
            }
            let (icon, label) = match self.theme {
              Theme::Dark => (Icon::ThemeDark, "Theme: dark — press for light"),
              Theme::Light => (Icon::ThemeLight, "Theme: light — press for dark"),
            };
            if icon_button(ui, &palette, icon, label, false, true, 18.0).clicked() {
              self.theme = match self.theme {
                Theme::Dark => Theme::Light,
                Theme::Light => Theme::Dark,
              };
            }
            // The view toggle the artboard floats over the canvas. It lives in the app bar instead:
            // it is view state for the whole frame, and a floating control over the drawing surface
            // would be one more thing between the pointer and the geometry.
            if ui.selectable_label(self.show_3d, "3D").clicked() {
              self.show_3d = true;
            }
            if ui.selectable_label(!self.show_3d, "2D").clicked() {
              self.show_3d = false;
            }
            ui.separator();
            ui.label(
              RichText::new("mm")
                .size(theme::CAPTION)
                .color(palette.get(Role::TextDim)),
            )
            .on_hover_text("Working units. Every length in the shell is millimetres.");
            ui.separator();
            if icon_button(ui, &palette, Icon::ZoomFit, "Fit the job — F", false, true, 18.0)
              .clicked()
            {
              output.actions.push(UiAction::ZoomToFit);
            }
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
            let centre = app.camera().center();

            match self.stage {
              Stage::Design => {
                section(ui, &palette, "DRAW");
                ui.horizontal_wrapped(|ui| {
                  for tool in self.stage.tools() {
                    if icon_button(
                      ui,
                      &palette,
                      tool_icon(*tool),
                      tool.label(),
                      self.active_tool == *tool,
                      true,
                      20.0,
                    )
                    .clicked()
                    {
                      self.active_tool = *tool;
                    }
                    offered.push(tool.label().to_owned());
                  }
                  // Text is the one "tool" that needs a value before it acts, so it reads the number from
                  // the Tools tab and lands in the middle of the view.
                  if icon_button(ui, &palette, Icon::Text, "Add text", false, true, 20.0).clicked()
                  {
                    output.actions.push(UiAction::AddText {
                      text: self.text.clone(),
                      height: self.text_height,
                      origin: centre,
                    });
                  }
                  offered.push("Add text".to_owned());
                });
              }
              Stage::Edit => {
                // Grouped by runs, so a command list with three groups needs no second table to say
                // which group a command is in.
                let mut runs: Vec<PaletteRun> = Vec::new();
                for (group, icon, label, action) in self.rail_commands(app) {
                  match runs.last_mut() {
                    Some((name, entries)) if *name == group => entries.push((icon, label, action)),
                    _ => runs.push((group, vec![(icon, label, action)])),
                  }
                }
                for (group, entries) in runs {
                  section(ui, &palette, group);
                  ui.horizontal_wrapped(|ui| {
                    for (icon, label, action) in entries {
                      if icon_button(ui, &palette, icon, label, false, true, 20.0).clicked() {
                        output.actions.push(action);
                      }
                      offered.push(label.to_owned());
                    }
                  });
                }
              }
              Stage::Relief | Stage::Toolpaths | Stage::Prove => {
                let title = self.stage.label().to_uppercase();
                section(ui, &palette, &title);
                note(ui, &palette, self.stage.note());
              }
            }

            ui.add_space(metrics.space(3));
            section(ui, &palette, "VIEW");
            ui.horizontal_wrapped(|ui| {
              if icon_button(
                ui,
                &palette,
                Icon::Grid,
                "Grid and snapping",
                self.snapping,
                true,
                20.0,
              )
              .clicked()
              {
                self.snapping = !self.snapping;
              }
              offered.push("Grid".to_owned());
              if icon_button(
                ui,
                &palette,
                Icon::Layers,
                "Inspector — collapse or expand",
                self.inspector_open,
                true,
                20.0,
              )
              .clicked()
              {
                self.inspector_open = !self.inspector_open;
              }
              offered.push("Layers".to_owned());
              if icon_button(
                ui,
                &palette,
                Icon::Help,
                "Shortcuts and help",
                false,
                true,
                20.0,
              )
              .clicked()
              {
                self.show_shortcuts = !self.show_shortcuts;
              }
              offered.push("Help".to_owned());
            });

            // What the palette offered: `DrawnState` is the shell's report of what it drew, and the
            // palette is the part of the frame that carries no text of its own.
            output.drawn.stage = self.stage;
            output.drawn.palette = offered;
          });
      });
  }

  /// The commands the current stage runs from the rail, built from the numbers in the Tools tab.
  ///
  /// These are the same constructions the Tools tab draws as labelled buttons. Keeping one
  /// implementation would mean extracting them into a shared command module — worth doing, and this
  /// note is the reminder that leaving two is a debt rather than a decision.
  fn rail_commands(&self, app: &AppState) -> Vec<GroupedCommand> {
    let centre = app.camera().center();
    match self.stage {
      Stage::Edit => vec![
        (
          "TRANSFORM",
          Icon::Move,
          "Move",
          UiAction::Transform(Transform::Translate {
            delta: Vec2::new(self.move_delta.0, self.move_delta.1),
          }),
        ),
        (
          "TRANSFORM",
          Icon::Rotate,
          "Rotate",
          UiAction::Transform(Transform::Rotate {
            radians: self.rotate_degrees.to_radians(),
            about: centre,
          }),
        ),
        (
          "TRANSFORM",
          Icon::Scale,
          "Scale",
          UiAction::Transform(Transform::Scale {
            sx: self.scale_factors.0,
            sy: self.scale_factors.1,
            about: centre,
            tolerance: DEFAULT_TOLERANCE_MM,
          }),
        ),
        (
          "TRANSFORM",
          Icon::MirrorH,
          "Mirror",
          UiAction::Transform(Transform::Mirror {
            point: centre,
            direction: Vec2::Y_AXIS,
          }),
        ),
        (
          "TRANSFORM",
          Icon::ArrayGrid,
          "Array",
          UiAction::Array {
            rows: self.array_grid.0,
            columns: self.array_grid.1,
            dx: self.array_spacing.0,
            dy: self.array_spacing.1,
          },
        ),
        (
          "VECTOR EDIT",
          Icon::Offset,
          "Offset",
          UiAction::Offset {
            distance: self.offset_distance,
          },
        ),
        ("VECTOR EDIT", Icon::Union, "Join", UiAction::Join),
      ],
      Stage::Design | Stage::Relief | Stage::Toolpaths | Stage::Prove => Vec::new(),
    }
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
                if self.stage.tools().is_empty() && self.rail_commands(app).is_empty() {
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
