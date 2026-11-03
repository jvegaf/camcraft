//! The shell's frame: the app bar, the rail, the canvas and the inspector.
//!
//! `design/prancheta-reference/` is the visual spec: fifteen full-resolution frames of the reference app,
//! each annotated in that directory's `README.md` with the pattern it settles. What this module implements
//! is the *structure* those frames argue for, mapped onto the widgets the crate already has rather than
//! copied pixel for pixel:
//!
//! ```text
//! ┌───────────────────────────────────────────────────────────┐  app bar    40 px
//! │ mark  campilot   ↶ ↷   untitled      mm ☾ ? fit [G-code]  │
//! ├────┬───────────────────────────────────────┬──────────────┤
//! │ 52 │                                       │ inspector    │
//! │ px │              canvas                   │ 288 px       │
//! │    │                                       │ (collapsible)│
//! ├────┴───────────────────────────────────────┴──────────────┤  status bar 26 px
//! ```
//!
//! **There is no stage bar, and removing it was a correction, not a preference.** An earlier version of this
//! frame carried a `Design › Edit › Relief › Toolpaths › Prove` strip, drawn from eight frames of the
//! reference app that had been photographed off two videos. The fifteen full-resolution frames show no such
//! strip in any state: the reference carries its workflow in the rail and in whatever task the right panel has
//! open. The strip is gone, and with it the stage enum that fed it (camui-prancheta S2).
//!
//! **The rail is not stage-scoped.** Sixty-odd commands do not fit on one screen and never did, but the
//! reference's answer is not to hide them behind a mode: it labels them, groups them, and lets the operator
//! collapse a group. What a command can do right now is decided by the document — the rule [`Command`] already
//! applies when it greys a chip — and never by a mode the operator has to remember they are in.
//!
//! **Counts are counted, never invented.** The frame states a number only when the document can prove it:
//! vectors on the sheet, toolpaths in the document. A section that would otherwise invent a count shows the
//! sentence naming the step that produces it instead — never `0 problems` for a validation nobody runs.

use cam_app::{AppState, VertexEdit};
use cam_geom::Point2;
use cam_program::program_for_snapshot;
use cam_store::tools::{render_name, stored_tool_for_number, StoredTool, ToolGeometry};
use cam_units::Format;
use egui::{Align, Layout, RichText, Sense};

use crate::help::{Command, Group, MenuScope};
use crate::icons::Icon;
use crate::theme::{self, Metrics, Palette, Role, Theme};
use crate::view3d::{clock_text, seconds_for};
use crate::{Shell, ShellOutput, Tool, UiAction};

/// One palette chip: the glyph and the command it runs.
type PaletteChip = (Icon, Command);

/// The height the rail reserves for its foot: three lines of text and a hairline.
///
/// A number and not a measurement, because the block is a bottom panel and egui needs its size before the
/// content exists. It is generous by a few pixels on purpose: a block clipped by its own reserve would cut the
/// stamp in half, which is the one line in the frame that has to be readable at a glance.
const BRAND_BLOCK_HEIGHT: f32 = 62.0;

/// A labelled run of palette chips: the group's name and the chips under it.
type PaletteRun = (Group, Vec<PaletteChip>);

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

  /// The ribbon category that opens this tab's content, if any.
  ///
  /// The right panel is not a tabbed inspector any more (camui-prancheta S6): it is a ribbon of three
  /// semantic categories, and opening one puts its task editor over the panel. `Selection` is the odd one
  /// out on purpose — the reference draws no selection category, because the selection's numbers are the
  /// panel's *idle* state rather than a task anybody opens — so it maps to **no** category and is drawn when
  /// no task is open.
  pub fn category(self) -> Option<Category> {
    match self {
      Self::Tools => Some(Category::Toolpaths),
      Self::Selection => None,
      Self::Job => Some(Category::MachineOutput),
    }
  }
}

/// A category of the right panel's ribbon: the three the reference draws, in its order (camui-prancheta S6).
///
/// Glyph only, one row per category. Opening one puts that category's **task editor** over the panel's body,
/// which is what replaced the three named tabs: the words moved into the task editor's header, where they name
/// the task instead of a tab nobody could tell apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
  /// What makes and describes toolpaths: the authoring commands, the tool card, the estimate.
  Toolpaths,
  /// The 3D half of the surface: whether it is split, and what it is playing.
  Relief3d,
  /// Stock, material, zero, and the program that leaves the machine.
  MachineOutput,
}

impl Category {
  /// Every category, top to bottom — the reference's order.
  pub const ALL: [Category; 3] = [
    Category::Toolpaths,
    Category::Relief3d,
    Category::MachineOutput,
  ];

  /// The words the task editor's header carries while this category is open.
  ///
  /// English, like every other word in the shell (S3): the reference is pt-BR and what we take from it is
  /// structure, never its strings.
  pub fn label(self) -> &'static str {
    match self {
      Self::Toolpaths => "TOOLPATHS",
      Self::Relief3d => "RELIEF AND 3D PART",
      Self::MachineOutput => "MACHINE AND OUTPUT",
    }
  }

  /// The glyph the ribbon draws for this category.
  ///
  /// Three glyphs the set already had: the profile toolpath, the relief, and the post-processor. No new
  /// symbol was drawn for this, because a glyph nobody has seen before is a legend, not an icon.
  pub fn icon(self) -> Icon {
    match self {
      Self::Toolpaths => Icon::ToolpathProfile,
      Self::Relief3d => Icon::Relief,
      Self::MachineOutput => Icon::PostProcessor,
    }
  }
}

/// The step the document is missing, in the shell's own voice, or nothing when it is missing none.
///
/// Two facts, and only facts: a drawing with no entities, and a document with no toolpaths. Everything else the
/// shell cannot see, and it says nothing rather than certifying a gate it cannot check.
fn next_step(app: &AppState) -> Option<&'static str> {
  let vectors: usize = app
    .document()
    .layers
    .iter()
    .map(|layer| layer.entities.len())
    .sum();
  if vectors == 0 {
    return Some(
      "Nothing drawn yet — 0 vectors on the sheet. Draw or import the shape you want to cut.",
    );
  }
  if app.document().toolpaths.is_empty() {
    return Some(
      "Nothing to cut yet — 0 toolpaths in the document. A toolpath comes from `cam-cli toolpath` and \
       lands here finished.",
    );
  }
  None
}

impl Shell {
  /// Which body the inspector is showing.
  pub fn tab(&self) -> Tab {
    self.inspector_tab
  }

  /// Show a tab.
  ///
  /// A tab is now *derived* from what the panel is showing: the ribbon opens a task, and the tab names the
  /// body under it. `Selection` has no category, so setting it closes the task editor and lets the panel fall
  /// back to the selection's numbers — the same state as pressing `✕`.
  pub fn set_tab(&mut self, tab: Tab) {
    self.inspector_tab = tab;
    self.task = tab.category();
  }

  /// The ribbon category whose task editor is open, or `None` for the panel's idle state.
  pub fn task(&self) -> Option<Category> {
    self.task
  }

  /// Open one category's task editor.
  pub fn open_task(&mut self, category: Category) {
    self.task = Some(category);
    self.inspector_tab = match category {
      // `RELIEF AND 3D PART` opens the 3D half of the frame, and the authoring body is what the panel shows
      // behind it: this shell authors no relief, so the task says what it does instead of pretending to a
      // form it does not have (L13).
      Category::Toolpaths | Category::Relief3d => Tab::Tools,
      Category::MachineOutput => Tab::Job,
    };
  }

  /// Close the task editor, and say whether its draft numbers went with it.
  ///
  /// `discard` is the only real difference between the header's `✕` and its `✓`: **every number in this
  /// panel's authoring body applies as it is typed**, so `✓` has nothing to commit and `✕` has nothing to undo
  /// — except the material's drafts, which sit in [`Shell::material_size`] until the dialog's own apply is
  /// pressed. `✕` puts them back to the document's stock, which is what "cancel this task" means when the
  /// task holds an unapplied edit.
  pub fn close_task(&mut self, discard: bool) {
    if discard {
      self.material_size = crate::default_material_size();
      self.show_material = false;
    }
    self.task = None;
    self.inspector_tab = Tab::Selection;
  }

  /// Whether the layers block is expanded.
  pub fn layers_open(&self) -> bool {
    self.layers_open
  }

  /// Expand or collapse the layers block.
  pub fn set_layers_open(&mut self, open: bool) {
    self.layers_open = open;
  }

  /// Collapse or expand the layers block, from its own heading.
  pub fn toggle_layers(&mut self) {
    self.layers_open = !self.layers_open;
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

  /// Whether the rail draws one group's chips.
  ///
  /// The heading is drawn either way. Collapsing a group hides its chips and nothing else: the word stays,
  /// so the rail is a table the operator folds, not a memory test (camui-prancheta S4).
  pub fn group_open(&self, group: Group) -> bool {
    !self.collapsed_groups.contains(&group)
  }

  /// Collapse or expand one rail group.
  pub fn toggle_group(&mut self, group: Group) {
    match self.collapsed_groups.iter().position(|held| *held == group) {
      Some(index) => {
        self.collapsed_groups.remove(index);
      }
      None => self.collapsed_groups.push(group),
    }
  }

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
  pub(crate) fn app_bar(&mut self, app: &AppState, ui: &mut egui::Ui, _output: &mut ShellOutput) {
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
          // opens one there is no file to name, which is why the bar said `untitled.c2d` for so long.
          let dirty = if app.is_dirty() { " · modified" } else { "" };
          ui.label(
            RichText::new(format!("{}{dirty}", self.document_name()))
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
      self.show_3d = pick == 1;
    }
    output.drawn.view_toggle = control.cells.clone();
    output.drawn.view_toggle_rect = Some(rect_report(control.rect));
    // Returned rather than only painted: the caller paints it again after the ruler band, which is drawn later
    // in the frame and would otherwise cover it (R2-002). The press above is already applied, so the frame that
    // presses `3D` is still the frame that splits (L13).
    Some(control)
  }

  /// The left palette: the commands, grouped, labelled, and foldable.
  ///
  /// The reference app settles a question this shell got wrong first. A rail of unlabelled glyphs stops
  /// working the moment there are more than a dozen of them — which is why the seven Edit commands had
  /// to hide behind icons nobody could name. Twenty commands in labelled groups stay findable, and the
  /// label is the word a machinist already uses.
  ///
  /// Every word, run, glyph and command here comes from [`crate::help`]: the palette holds no list of
  /// its own, which is what closed the two-lists-one-command debt T8 inherited (L14).
  ///
  /// **A heading folds its group** (camui-prancheta S4). Clicking `FILE` hides FILE's chips and leaves the
  /// word; clicking it again brings them back. The heading is a control and not a decoration, which is why it
  /// reports where it was drawn the way the chips do: a test clicks the word, not a coordinate it guessed.
  pub(crate) fn palette_panel(
    &mut self,
    app: &AppState,
    ui: &mut egui::Ui,
    output: &mut ShellOutput,
  ) {
    let palette = self.palette();
    let metrics = self.metrics();
    // Computed before the panel borrows the shell: the brand block is read-only, and the rail's chips are
    // not — a `&mut self` closure that had to read `self` inside would not compile, which is the borrow
    // checker saying the same thing the split of concerns says.
    let brand = self.brand_lines(app);
    egui::Panel::left("palette")
      .exact_size(metrics.palette_width)
      .show(ui, |ui| {
        // The brand block is a bottom panel of the rail and not a row inside the scrolling one: a stamp that
        // scrolled away with the chips would be a footnote, and the rail is the part of the frame the operator
        // glances at to know what they are looking at. Reserving its height first is also what keeps the
        // chips' coordinates stable between frames — a layout that grew or shrank as the block measured
        // itself would report a point the next frame had already moved.
        egui::Panel::bottom("brand_block")
          .exact_size(BRAND_BLOCK_HEIGHT)
          .show(ui, |ui| {
            brand_block(ui, &palette, &brand);
          });
        egui::ScrollArea::vertical()
          .id_salt("palette")
          .show(ui, |ui| {
            let mut offered: Vec<String> = Vec::new();
            let mut hits: Vec<(String, (f32, f32))> = Vec::new();
            let mut headings: Vec<(String, (f32, f32))> = Vec::new();

            // The runs arrive in the table's own order: FILE, DRAW, EDIT, ORGANISE and SHOW AND SELECT.
            for (group, chips) in self.palette_runs() {
              if group == Group::ShowAndSelect {
                ui.add_space(metrics.space(3));
              }
              let open = self.group_open(group);
              let heading = section_toggle(ui, &palette, group.label(), open);
              let centre = heading.rect.center();
              headings.push((group.label().to_owned(), (centre.x, centre.y)));
              if heading.clicked() {
                self.toggle_group(group);
              }
              if !open {
                continue;
              }
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
            output.drawn.palette = offered;
            output.drawn.palette_hits = hits;
            output.drawn.palette_headings = headings;
          });
      });
  }

  /// The three lines the rail's foot carries: the product, the crate version, and the document's own stamp.
  ///
  /// The stamp is L7's answer and not a build stamp: a `build.rs` timestamp in the binary would break
  /// reproducible builds and would be worse than the version string above it, which is comparable where a
  /// timestamp is not. `Unsaved changes` is the document's fact (it has edits nobody wrote) and the stamp is
  /// the file's (when the last write happened), so the two are asked in that order.
  fn brand_lines(&self, app: &AppState) -> Vec<String> {
    vec![
      "campilot".to_owned(),
      format!("v{}", env!("CARGO_PKG_VERSION")),
      if app.is_dirty() {
        "Unsaved changes".to_owned()
      } else {
        self.saved_stamp()
      },
    ]
  }

  /// The palette's runs, derived from the command table.
  ///
  /// One source: [`Command::ALL`] decides the words ([`Command::label`]), the runs ([`Command::group`]), the
  /// glyphs and the commands themselves. The table's order is the order the runs and the chips are drawn in,
  /// so a command that moves in the table moves in the palette — which is why `Command::ALL` keeps each group
  /// contiguous, in [`Group::ALL`]'s order.
  ///
  /// [`Command::in_palette`] is the only filter: it leaves the Tools tab's own commands and the `F2` sheet
  /// out, and it lets the whole FILE group in (camui-prancheta T2b).
  fn palette_runs(&self) -> Vec<PaletteRun> {
    let mut runs: Vec<PaletteRun> = Vec::new();
    for command in Command::ALL
      .iter()
      .copied()
      .filter(|command| command.in_palette())
    {
      let group = command.group();
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
  /// The seven parameterised commands come from [`crate::params`], which is also what the Tools tab's
  /// buttons call: the chip, the menu row and the tab read the same fields *and* build the same payload, so
  /// a number typed once is the number all three run. `Node edit` is the Tools tab's *Move node*, because a
  /// menu row is one command and moving the node at the index the tab holds is the one of the three that
  /// reads a single index.
  fn command_action(&self, command: Command, app: &AppState) -> Option<UiAction> {
    let centre = app.camera().center();
    match command {
      Command::NodeEdit => Some(UiAction::EditVertex(VertexEdit::Move {
        index: self.vertex_index,
        to: Point2::new(self.vertex_position.0, self.vertex_position.1),
      })),
      Command::Trim => trim_action(app),
      _ => crate::params::action(command, self, centre).or_else(|| command.action()),
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

  /// The right panel: a ribbon of three categories, the task editor they open, and the layers block under it.
  ///
  /// **This is not a tabbed inspector any more** (camui-prancheta S6). The reference draws `PERCURSOS`,
  /// `RELEVO E PEÇA 3D` and `MÁQUINA E SAÍDA` as glyph rows, and opening one puts a *task editor* over the
  /// panel's body: a header carrying the task's name with `✓` and `✕`, and under it the task's own content.
  /// Which task is open is [`Shell::task`]; the words left the old tab row and became the task's name, which
  /// is the difference between a tab and a task — a tab names a shelf, a task names what you are doing.
  ///
  /// **The idle state is the selection's numbers.** The reference has no selection category, because there is
  /// no selection *task*: it is what the panel says while nothing is open. So `None` is a state and not an
  /// absence, and every surface the three tabs used to reach is still one click from here — `TOOLPATHS` for the
  /// authoring body, `MACHINE AND OUTPUT` for the job's, and the selection's readout for the idle one.
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

        // The ribbon: one row per category, glyph only, in the reference's order. The row is what replaces the
        // three named tabs, and the active one says so the way a rail chip does.
        for category in Category::ALL {
          let response = icon_button(
            ui,
            &palette,
            category.icon(),
            &ribbon_tooltip(category),
            self.task == Some(category),
            true,
            20.0,
          );
          let centre = response.rect.center();
          output.drawn.categories.push(category.label().to_owned());
          output
            .drawn
            .category_hits
            .push((category.label().to_owned(), (centre.x, centre.y)));
          if response.clicked() {
            // A second press closes it: the same gesture that opened the task is the one that leaves it, which
            // is what a ribbon of exclusive categories means.
            match self.task {
              Some(open) if open == category => self.close_task(false),
              _ => self.open_task(category),
            }
          }
        }
        ui.separator();

        let body = (ui.available_height() * 0.55).max(120.0);
        // The header is drawn first and the body is chosen **after** it, because the header's `✓` and `✕` can
        // close the task: a frame that drew the header and then the body from the same answer would report a
        // task it had just closed, and the report is what a test and the operator both read.
        if let Some(category) = self.task {
          self.task_header(ui, &palette, category, output);
          ui.add_space(metrics.space(2));
        }
        match self.task {
          Some(category) => {
            egui::ScrollArea::vertical()
              .id_salt("inspector_body")
              .max_height(body)
              .show(ui, |ui| match category {
                Category::Toolpaths => {
                  // The card belongs to a toolpath, so it is drawn only while the document holds one: a card
                  // for a toolpath that does not exist would be describing nothing.
                  if !app.document().toolpaths.is_empty() {
                    self.tool_card(app, ui, &palette, output);
                  }
                  self.authoring(app, ui, output);
                }
                Category::Relief3d => self.relief_task(app, ui, &palette, output),
                Category::MachineOutput => self.job_section(app, ui, &palette, output),
              });
          }
          // Idle: what the panel says while no task is open — the selection's numbers. This is also the frame
          // in which a `✓` or `✕` just closed a task, which is why the answer is read from `self.task` again
          // instead of from the `category` above.
          None => {
            output.drawn.task_editor.clear();
            egui::ScrollArea::vertical()
              .id_salt("inspector_body")
              .max_height(body)
              .show(ui, |ui| {
                ui.add_space(metrics.space(2));
                if !app.document().toolpaths.is_empty() {
                  self.tool_card(app, ui, &palette, output);
                }
                self.properties(app, ui, output);
              });
          }
        }

        ui.separator();
        // `Layers` is its own block below the task editor and folds on its own (S6): a document fact, not a
        // task's opinion, so it stays put while tasks open and close above it.
        let heading = section_toggle(ui, &palette, "LAYERS", self.layers_open);
        let centre = heading.rect.center();
        output.drawn.layers_heading = Some((centre.x, centre.y));
        if heading.clicked() {
          self.toggle_layers();
        }
        // Read after the click: the folded block draws no row in the frame that folds it, not one frame later.
        output.drawn.layers_open = self.layers_open;
        if self.layers_open {
          egui::ScrollArea::vertical()
            .id_salt("inspector_layers")
            .show(ui, |ui| self.layer_dock(app, ui, output));
        }
      });
  }

  /// The task editor's header: the task's name, and the two glyphs that leave it.
  ///
  /// `✓` keeps what is typed and `✕` throws away the one draft the panel holds (see [`Shell::close_task`]);
  /// both are painted rather than written as `✓` and `✕`, for the same reason the rail's caret is painted: a
  /// glyph the bundled faces do not carry renders as a box, and a box is not an affordance.
  fn task_header(
    &mut self,
    ui: &mut egui::Ui,
    palette: &Palette,
    category: Category,
    output: &mut ShellOutput,
  ) {
    ui.horizontal(|ui| {
      ui.label(
        RichText::new(category.label())
          .size(theme::CAPTION)
          .color(palette.get(Role::TextHi)),
      );
      output.drawn.task_editor = category.label().to_owned();
      ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        let cancel = glyph_button(
          ui,
          palette,
          Glyph::Cross,
          "Cancel — close this task and discard the numbers you have not applied",
        );
        let done = glyph_button(
          ui,
          palette,
          Glyph::Check,
          "Done — every number here applies as you type, so closing keeps them",
        );
        for (response, word) in [(&done, "Done"), (&cancel, "Cancel")] {
          let centre = response.rect.center();
          output
            .drawn
            .task_editor_hits
            .push((word.to_owned(), (centre.x, centre.y)));
        }
        if cancel.clicked() {
          self.close_task(true);
        } else if done.clicked() {
          self.close_task(false);
        }
      });
    });
  }

  /// `RELIEF AND 3D PART`, which in this shell is a *state* and not a form.
  ///
  /// The reference opens a 3D machining strategy here. This shell authors no relief — its 3D half plays a
  /// program `cam-cli` produced — so the task says exactly that, opens or closes the split, and names whether
  /// a program is loaded. It is the same refusal the rest of the shell makes: no form for a capability that
  /// does not exist (L13).
  fn relief_task(
    &mut self,
    app: &AppState,
    ui: &mut egui::Ui,
    palette: &Palette,
    output: &mut ShellOutput,
  ) {
    let _ = app;
    ui.label(
      RichText::new("This shell authors no relief and no 3D strategy.")
        .size(theme::CAPTION)
        .color(palette.get(Role::TextDim)),
    );
    ui.label(
      RichText::new(
        "3D plays a program that `cam-cli` produced and a host handed in. The split's overlay row, its 
         axis triad and its playback bar are the 3D half's own chrome (S8); nothing here authors a 
         toolpath.",
      )
      .size(theme::CAPTION)
      .color(palette.get(Role::TextDim)),
    );
    ui.add_space(4.0);
    let state = if self.show_3d {
      "The 3D split is open."
    } else {
      "The 3D split is closed."
    };
    ui.label(
      RichText::new(state)
        .size(theme::CAPTION)
        .color(palette.get(Role::TextHi)),
    );
    let loaded = if self.preview.is_some() {
      "A program is loaded for playback."
    } else {
      "No program loaded — simulate one with `cam-cli simulate` and hand it in."
    };
    ui.label(
      RichText::new(loaded)
        .size(theme::CAPTION)
        .color(palette.get(Role::TextDim)),
    );
    ui.add_space(4.0);
    let label = if self.show_3d {
      "Close the 3D split"
    } else {
      "Open the 3D split"
    };
    if ui.button(label).clicked() {
      self.show_3d = !self.show_3d;
    }
    output.drawn.task_editor = Category::Relief3d.label().to_owned();
  }

  /// The current-tool card: the tool a toolpath cuts with, in the two lines the reference frame uses.
  ///
  /// `03-27-41`'s `Novo percurso — Chanfro` flyout puts a card at the top of a toolpath form: the tool's
  /// number and name, then its type with the shape's numbers and its flute count. **Nothing else is on
  /// it** (S2): the frame shows no material, no machine and no feed, plunge or spindle rate, so the card
  /// does not grow them. That is also what makes the card honest —
  /// [`cam_store::tools::stored_tool_for_number`] takes no material and no machine (S3), so a rate on
  /// this card would have to claim a material the document does not carry.
  ///
  /// The number resolves through the same function the emitter resolves it with, so the card cannot say
  /// "T7 is missing" beside a program that cuts with T7: the two read one rule (S3). The unresolved case
  /// is a state and not a blank — it names the number and says the library the window was given does not
  /// hold it, because a card that drew nothing would leave the operator guessing whether the tool is
  /// missing or the panel is.
  ///
  /// The shell has no toolpath selection yet, so the card describes the document's **first** toolpath and
  /// names it in the heading. When authoring lands (T3) the selection is what the heading reads from.
  ///
  /// Below the card's own lines sits the **estimated cutting time** — a line of its own, not a fourth
  /// field, because the card's fields are exactly the frame's (S2). Its seconds are
  /// [`seconds_for`] over the program this document's own toolpaths build from the same snapshot, so the
  /// number describes what the machine would cut and is the number the playback clock would report for
  /// it (S5); [`clock_text`] writes it, so the two readouts are formatted by one rule. When that program
  /// cannot be built there is no time to show, and the emitter's sentence says which tool or toolpath
  /// stopped it instead.
  fn tool_card(
    &mut self,
    app: &AppState,
    ui: &mut egui::Ui,
    palette: &Palette,
    output: &mut ShellOutput,
  ) {
    let Some(toolpath) = app.document().toolpaths.first() else {
      return;
    };
    let heading = format!("TOOL — {}", toolpath.name);
    section(ui, palette, &heading);
    output.drawn.tool_card.push(heading);

    let number = toolpath.tool_number;
    match stored_tool_for_number(&self.tools, number) {
      Some(tool) => {
        let title = format!("T{number} - {}", tool_name(&tool.geometry));
        let shape = tool_shape_line(tool);
        ui.label(
          RichText::new(&title)
            .size(theme::TITLE)
            .color(palette.get(Role::TextHi)),
        );
        ui.label(
          RichText::new(&shape)
            .size(theme::CAPTION)
            .color(palette.get(Role::TextDim)),
        );
        output.drawn.tool_card.push(title);
        output.drawn.tool_card.push(shape);
      }
      None => {
        let sentence = format!(
          "T{number} is not in the tool library this window was given, so this toolpath has no tool \
           to show."
        );
        ui.label(
          RichText::new(&sentence)
            .size(theme::CAPTION)
            .color(palette.get(Role::Danger)),
        );
        output.drawn.tool_card.push(sentence);
      }
    }

    // A line of its own rather than a field of the card (S2), measured over the document's own program
    // and written by the playback clock's formatter (S5). No second time formula, no second clock.
    let estimate = match program_for_snapshot(app.document(), &self.tools) {
      Ok(program) => format!(
        "Estimated cutting time · {}",
        clock_text(seconds_for(&program))
      ),
      // No program, no time: naming what stopped it is more use than a `0:00` beside a card that has
      // just said the tool is missing.
      Err(error) => format!("No estimated cutting time — {error}"),
    };
    ui.label(
      RichText::new(&estimate)
        .size(theme::CAPTION)
        .color(palette.get(Role::TextDim)),
    );
    output.drawn.cutting_time = estimate;
    ui.add_space(6.0);
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
  ///
  /// The boxes report; the opener above them writes. A job's three measurements are one decision made
  /// once, so they get a dialog with a drawn plan instead of three text fields that can each be
  /// half-edited — which is the shape the reference's `Novo trabalho` frame shows and the row for it in
  /// `design/Screenshots-reference/README.md` settles.
  fn job_section(
    &mut self,
    app: &AppState,
    ui: &mut egui::Ui,
    palette: &Palette,
    output: &mut ShellOutput,
  ) {
    let job = &app.document().job;
    section(ui, palette, "MATERIAL");
    // The opener is the only path to the dialog, so it is also the only thing in this tab that writes.
    // Reported only while the dialog is closed: with the modal up this point is behind its backdrop, and
    // a hit list that named an unreachable point would be lying about what can be pressed.
    let opener = ui.button(RichText::new(MATERIAL_OPENER).size(theme::SMALL));
    if !self.show_material {
      let centre = opener.rect.center();
      output
        .drawn
        .material_hits
        .push((MATERIAL_OPENER.to_owned(), (centre.x, centre.y)));
    }
    if opener.clicked() {
      self.open_material(app);
    }
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
    // The section reports the document, so the one sentence it can add is the step the document is missing.
    // A document that has everything the shell can see owes nothing here, and printing another section's
    // sentence would be the one lie this section could tell.
    if let Some(step) = next_step(app) {
      ui.add_space(6.0);
      note(ui, palette, step);
    }
  }

  /// The material dialog: the first thing a job defines, and the shell's first document-level edit.
  ///
  /// The `03-26-19` row of `design/Screenshots-reference/README.md` is the pattern — two choice cards
  /// over a live annotated key plan, one caption saying what the origin marker means, and a
  /// `Cancel`/`Continue` footer — and three decisions are stated here rather than hidden:
  ///
  /// **The rotary card is drawn and cannot be chosen.** The document has no axis mode: `Job` is a width,
  /// a height, a thickness, a unit system, an origin corner and a safe Z, and `cam-model` plus the
  /// `c2d/document@2` format are out of scope for this slice (S1). A card that wrote nothing would be a
  /// dead control, which S7 and S9 both forbid, so the card carries its reason on its own face and is not
  /// clickable. The operator reads why it is dark instead of pressing a button that does nothing.
  ///
  /// **The plan is drawn, never shipped.** S6 forbids a new dependency and an image asset is not a token,
  /// so the stock box is geometry: an axonometric box built from the three numbers being edited, three
  /// dimension lines carrying those same numbers, and an origin marker whose meaning is in the caption
  /// rather than in its colour alone (S7). Every colour is a [`Role`], every surface a token.
  ///
  /// **The refusal is two-layered and single-sourced.** This dialog refuses a non-positive number in the
  /// sentence it shows under the plan, and [`cam_app::Command::SetJob`] refuses the same three numbers with
  /// the model's own `ModelError::NonPositiveJobDimension`. The dialog's sentence is a reading of the
  /// invariant and keeps `Continue` dark beside the number that caused it; the command is the invariant,
  /// and it is the layer that guarantees `Job::outline` is never reached with a zero.
  pub(crate) fn material_dialog(
    &mut self,
    ctx: &egui::Context,
    app: &AppState,
    output: &mut ShellOutput,
  ) {
    let _ = app;
    let palette = self.palette();
    let metrics = self.metrics();
    // The drafts are copied out of `self` so the closure can edit them: the dialog writes to the
    // document once, through an action, and nothing else about the shell changes while it is open.
    let mut width = self.material_size.0;
    let mut height = self.material_size.1;
    let mut thickness = self.material_size.2;

    let field_width = 232.0;
    let plan_width = 420.0;
    let plan_height = 220.0;
    let card_width = (field_width + plan_width) / 2.0;

    let mut words: Vec<String> = Vec::new();
    let mut hits: Vec<(String, (f32, f32))> = Vec::new();
    let mut cancel = false;
    let mut commit = false;

    egui::Modal::new(egui::Id::unique("campilot-material"))
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
        ui.set_max_width(field_width + plan_width + metrics.space(4));
        ui.label(
          RichText::new(MATERIAL_TITLE)
            .size(theme::TITLE)
            .color(palette.get(Role::TextHi)),
        );
        words.push(MATERIAL_TITLE.to_owned());
        ui.add_space(metrics.space(1));
        ui.label(
          RichText::new(MATERIAL_LEDE)
            .size(theme::BODY)
            .color(palette.get(Role::Text)),
        );
        words.push(MATERIAL_LEDE.to_owned());
        ui.add_space(metrics.space(4));

        // The two cards. The sheet is the one this document can hold, so it is drawn chosen and does not
        // need pressing; the rotary card names its own absence instead of pretending to be a choice.
        ui.horizontal(|ui| {
          for (glyph, title, sentence, chosen, available) in [
            (Icon::StockSheet, SHEET_TITLE, SHEET_SENTENCE, true, None),
            (
              Icon::StockRotary,
              ROTARY_TITLE,
              ROTARY_REASON,
              false,
              Some(()),
            ),
          ] {
            let response = choice_card(
              ui, &palette, &metrics, glyph, card_width, title, sentence, chosen, available,
            );
            let centre = response.rect.center();
            hits.push((title.to_owned(), (centre.x, centre.y)));
            words.push(title.to_owned());
            words.push(sentence.to_owned());
          }
        });
        ui.add_space(metrics.space(4));

        // Measurements on the left, the live plan on the right, the way the reference frame lays it out:
        // the numbers and the picture they describe are read together.
        ui.horizontal(|ui| {
          ui.vertical(|ui| {
            ui.set_width(field_width);
            for (label, value) in [
              ("Width X", &mut width),
              ("Height Y", &mut height),
              ("Thickness Z", &mut thickness),
            ] {
              measurement(ui, &palette, &metrics, label, value);
              words.push(label.to_owned());
            }
          });
          ui.add_space(metrics.space(4));
          ui.vertical(|ui| {
            let (rect, _) =
              ui.allocate_exact_size(egui::vec2(plan_width, plan_height), Sense::hover());
            // One set of strings for the drawing and the report: the numbers the plan annotates are the
            // numbers a test reads back, so the report cannot describe a plan the painter did not draw.
            let dims = [
              format!("{width:.0} mm"),
              format!("{height:.0} mm"),
              format!("{thickness:.0} mm"),
            ];
            paint_material_plan(
              ui.painter(),
              rect,
              &palette,
              &metrics,
              (width, height, thickness),
              [&dims[0], &dims[1], &dims[2]],
            );
            words.extend(dims.iter().cloned());
            ui.add_space(metrics.space(2));
            ui.label(
              RichText::new(PLAN_CAPTION)
                .size(theme::CAPTION)
                .color(palette.get(Role::TextDim)),
            );
            words.push(PLAN_CAPTION.to_owned());
          });
        });

        // The refusal is a sentence under the plan and not a red box: it names the field and the number,
        // so the operator can fix the number without being told the rule in the abstract (S4).
        let refusal = material_refusal_of(width, height, thickness);
        if let Some(sentence) = &refusal {
          ui.add_space(metrics.space(2));
          ui.label(
            RichText::new(sentence)
              .size(theme::CAPTION)
              .color(palette.get(Role::Danger)),
          );
          words.push(sentence.clone());
        }

        ui.add_space(metrics.space(4));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
          // Dark while the numbers describe no sheet: the sentence above is the whole reason, and a button
          // that could be pressed into a refusal would be a control that answers with a lie.
          let go = ui.add_enabled(
            refusal.is_none(),
            egui::Button::new(RichText::new(CONTINUE).size(theme::SMALL)),
          );
          let centre = go.rect.center();
          hits.push((CONTINUE.to_owned(), (centre.x, centre.y)));
          commit = go.clicked();
          let back = ui.button(RichText::new(CANCEL).size(theme::SMALL));
          let centre = back.rect.center();
          hits.push((CANCEL.to_owned(), (centre.x, centre.y)));
          cancel = back.clicked();
        });
        // A press on the backdrop does not close the dialog: the three numbers are typed work, and losing
        // them to a stray click on the darkening is the one thing a modal must not do.
      });

    self.material_size = (width, height, thickness);
    output.drawn.material = words;
    output.drawn.material_hits = hits;
    output.drawn.material_refusal = self.material_refusal().unwrap_or_default();
    if cancel {
      self.show_material = false;
    }
    if commit {
      self.show_material = false;
      output.actions.push(UiAction::SetJob {
        width,
        height,
        thickness,
      });
    }
  }

  /// Open the material dialog, seeded with the document's own job.
  ///
  /// Public because starting a new job is the other thing that opens this, and because a dialog whose
  /// opening cannot be driven from outside a frame is a dialog only a mouse could test.
  pub fn open_material(&mut self, app: &AppState) {
    let job = &app.document().job;
    self.material_size = (job.width.mm(), job.height.mm(), job.thickness.mm());
    self.show_material = true;
  }

  /// The sentence the material dialog is showing, or nothing when its three numbers describe a sheet.
  ///
  /// Derived from the drafts rather than stored, so the sentence cannot outlive the number it complains
  /// about. [`cam_app::Command::SetJob`] refuses the same three numbers by the model's own rule.
  pub fn material_refusal(&self) -> Option<String> {
    material_refusal_of(
      self.material_size.0,
      self.material_size.1,
      self.material_size.2,
    )
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
      // Both halves are on screen at once (S8), so the sentence names both gestures: the drawing half
      // still picks and draws, and the 3D half orbits.
      return "3D half — drag to orbit, wheel to zoom; the drawing half still picks and draws".to_owned();
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
    Command::Open => Icon::Open,
    Command::Save => Icon::Save,
    Command::SaveAs => Icon::SaveAs,
    Command::FitJob => Icon::ZoomFit,
    // The rest are not drawn as glyphs anywhere in the frame: they are labelled buttons in the Tools tab
    // or reached by a key, so this arm is the honest "no symbol of its own" answer rather than a new
    // symbol nobody has ever seen.
    _ => Icon::Select,
  }
}

/// Record one app-bar button: the word the command table gives it, and the point that presses it.
///
/// Kept for the buttons the bar still owns. Since camui-prancheta T2b no bar button runs a table command —
/// FILE moved to the rail — so [`crate::DrawnState::app_bar`] stays empty and a test asserts it, which is how a
/// duplicate button cannot come back unnoticed.
#[allow(dead_code)]
fn report_app_bar(output: &mut ShellOutput, command: Command, rect: egui::Rect) {
  let centre = rect.center();
  output.drawn.app_bar.push(command.label().to_owned());
  output
    .drawn
    .app_bar_hits
    .push((command.label().to_owned(), (centre.x, centre.y)));
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

/// One window rectangle as the four numbers [`DrawnState`] reports its regions with.
pub(crate) fn rect_report(rect: egui::Rect) -> (f32, f32, f32, f32) {
  (rect.min.x, rect.min.y, rect.width(), rect.height())
}

/// What a segmented control drew: the cell that was pressed, where each cell was drawn, and the strip.
///
/// The cells are reported for the same reason the palette reports its chips: egui offers no
/// widget-by-text lookup from outside a frame, so a test that has to press `3D` is given the point the
/// cell was drawn at instead of guessing at the control's margins (camui-prancheta S8).
pub(crate) struct Segmented {
  /// The index the operator picked this frame, if any.
  pub picked: Option<usize>,
  /// Each cell's word and the point in the window that presses it, in the order drawn.
  pub cells: Vec<(String, (f32, f32))>,
  /// The strip the whole control occupied.
  pub rect: egui::Rect,
  /// Each cell's rectangle, in the order drawn.
  ///
  /// Kept so the control can be painted a second time later in the same frame: the canvas' `2D | 3D` sits on
  /// the ruler band and has to be painted over it (camui-prancheta S8, R2-002).
  pub cell_rects: Vec<egui::Rect>,
}

/// A segmented control: mutually exclusive choices in one joined strip.
///
/// The reference uses these for every small decision (`Material | Último vetor`, `Não | De 90° | Livre`,
/// `2D | 3D`) and it is the right shape: the alternatives are visible, so choosing is a comparison instead
/// of a search. Returns the index the operator picked and the geometry it drew at.
pub(crate) fn segmented(
  ui: &mut egui::Ui,
  palette: &Palette,
  labels: &[&str],
  active: usize,
) -> Segmented {
  let control = segmented_cells(ui, palette, labels);
  paint_segmented(
    ui.painter(),
    control.rect,
    &control.cell_rects,
    labels,
    active,
    palette,
  );
  control
}

/// Lay a segmented control out and report it, without painting it.
///
/// The canvas' `2D | 3D` needs this: it is laid out before the frame knows whether it splits (L13's rule) but it
/// has to be *painted* after the ruler band it sits on (R2-002), so the two halves of the widget are separate
/// calls and [`paint_segmented`] is the only painter for both.
fn segmented_cells(ui: &mut egui::Ui, palette: &Palette, labels: &[&str]) -> Segmented {
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
  let mut left = rect.left();
  let mut cell_rects = Vec::with_capacity(labels.len());
  for width in &widths {
    cell_rects.push(egui::Rect::from_min_size(
      egui::Pos2::new(left, rect.top()),
      egui::vec2(*width, rect.height()),
    ));
    left += *width;
  }
  let mut clicked = None;
  let mut cells = Vec::with_capacity(labels.len());
  for (index, label) in labels.iter().enumerate() {
    let cell = cell_rects[index];
    if response.clicked()
      && ui
        .ctx()
        .pointer_hover_pos()
        .is_some_and(|p| cell.contains(p))
    {
      clicked = Some(index);
    }
    let centre = cell.center();
    cells.push(((*label).to_owned(), (centre.x, centre.y)));
  }
  Segmented {
    picked: clicked,
    cells,
    rect,
    cell_rects,
  }
}

/// Paint a segmented control: the strip, the chosen cell and every word.
///
/// Separate from [`segmented`] so it can be called a **second** time, later in the frame. The canvas'
/// `2D | 3D` sits on the ruler band; the band is painted after the widget is laid out, and a later painter
/// wins — which is how the control ended up under the band's own fill and numbers, invisible in 2D mode
/// (R2-002 in the T6 review). One function for both paints, so the two cannot drift apart.
pub(crate) fn paint_segmented(
  painter: &egui::Painter,
  rect: egui::Rect,
  cell_rects: &[egui::Rect],
  labels: &[&str],
  active: usize,
  palette: &Palette,
) {
  let radius = egui::CornerRadius::same(11);
  painter.rect_filled(rect, radius, palette.get(Role::Raised));
  painter.rect_stroke(
    rect,
    radius,
    egui::Stroke::new(1.0, palette.get(Role::Border)),
    egui::StrokeKind::Inside,
  );
  let font = egui::FontId::proportional(theme::CAPTION);
  for (index, label) in labels.iter().enumerate() {
    let Some(cell) = cell_rects.get(index).copied() else {
      continue;
    };
    let on = index == active;
    if on {
      painter.rect_filled(cell, radius, palette.get(Role::SelSoft));
      painter.rect_stroke(
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
    painter.text(
      cell.center(),
      egui::Align2::CENTER_CENTER,
      *label,
      font.clone(),
      ink,
    );
  }
}

/// One labelled value box: the label above, the number inside a bordered field.
///
/// Read-only on purpose, and that is the one thing T12 changed about it: a job's three measurements are
/// defined once, in [`Shell::material_dialog`], and a box in a 288 px inspector is where they are
/// *reported* afterwards. Typing a size into two of these in place would let a job exist in a state
/// nobody drew a plan for, so the dialog is the only writer and this box is the receipt.
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

/// The word every tool display name in this build is rendered with, where the tool's type would go.
///
/// The library keeps a tool's type as a raw integer and says in its own header that the numeric mapping
/// is not established from the artifact (`cam-store::tools`), so this build has no vocabulary to put
/// there — and inventing one, a table of codes to "end mill" and "V-bit", would be inventing a fact
/// about a file nobody has read. `cam-cli tools list` renders the same templates with this same word,
/// which is the point: one library read by two programs may not name one tool two different things.
const TOOL_TYPE_LABEL: &str = "Tool";

/// A tool's display name: its `name_format` template rendered, or its id when the template names a field
/// the tool does not set — the same fallback `cam-cli tools list` prints for the same failure.
fn tool_name(geometry: &ToolGeometry) -> String {
  render_name(&geometry.name_format, TOOL_TYPE_LABEL, geometry)
    .unwrap_or_else(|_| geometry.id.clone())
}

/// The card's second line: the tool's type code, then the three numbers S2 puts beside it.
///
/// The type is the library's raw code, marked as one, because no build of this shell has a word for it
/// (see [`TOOL_TYPE_LABEL`]). A dash stands where the library holds no number: that is a reading of what
/// the library has, where a `0` would be a number the tool does not have.
fn tool_shape_line(tool: &StoredTool) -> String {
  let diameter = tool
    .geometry
    .diameter
    .map(|diameter| format!("Ø {} mm", Format::fixed(2).apply(diameter.mm())))
    .unwrap_or_else(|| "Ø —".to_owned());
  let angle = tool
    .geometry
    .included_angle
    .map(|angle| format!("{}°", Format::fixed(1).apply(angle.degrees())))
    .unwrap_or_else(|| "—".to_owned());
  let flutes = tool
    .geometry
    .num_flutes
    .map(|flutes| format!("{flutes} flutes"))
    .unwrap_or_else(|| "—".to_owned());
  format!(
    "code {} · {diameter} · {angle} · {flutes}",
    tool.geometry.tool_type
  )
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

/// The rail's foot: the product, the crate version, and the document's own stamp.
///
/// The block is drawn in a bottom-up layout, so the lines are painted from the bottom of the rail upwards:
/// the stamp sits lowest, the version above it, and the product name on top of the three. It is the only part
/// of the rail that is not a command, which is why it is separated by a hairline rather than by a heading — a
/// heading promises chips under it.
fn brand_block(ui: &mut egui::Ui, palette: &Palette, lines: &[String]) {
  ui.add_space(4.0);
  let width = ui.available_width();
  let (hairline, _) = ui.allocate_exact_size(egui::vec2(width, 1.0), Sense::hover());
  ui.painter()
    .rect_filled(hairline, 0.0, palette.get(Role::Hairline));
  ui.add_space(2.0);
  // The last line goes to the bottom of the block, so the list is walked backwards. The first entry is the
  // product name and gets the size a title gets; the two below it are the version and the stamp, which are
  // secondary information and say so in colour.
  for (index, line) in lines.iter().enumerate().rev() {
    let (size, role) = if index == 0 {
      (theme::TITLE, Role::TextHi)
    } else {
      (theme::CAPTION, Role::TextDim)
    };
    ui.label(RichText::new(line).size(size).color(palette.get(role)));
  }
}

/// A palette heading that folds its group.
///
/// The same word, weight, colour and hairline as [`section`], plus a caret and a click target: this is the
/// reference's collapsible group, and the whole row is the control rather than a triangle the operator has to
/// hit. The caret is painted as a triangle instead of written as a glyph because `▾` is a font bet — the
/// bundled faces may or may not carry it — and a group whose affordance renders as a box is worse than no
/// affordance at all.
/// The two glyphs the task editor's header paints: a check and a cross.
///
/// Painted and not written for the same reason the rail's caret is painted: `✓` and `✕` are a font bet, and an
/// affordance that renders as a box is worse than a drawn mark nobody has to trust the font for.
#[derive(Clone, Copy)]
enum Glyph {
  /// `Done`.
  Check,
  /// `Cancel`.
  Cross,
}

/// A small square button holding a painted glyph, with its teaching on hover.
fn glyph_button(
  ui: &mut egui::Ui,
  palette: &Palette,
  glyph: Glyph,
  tooltip: &str,
) -> egui::Response {
  let size = egui::vec2(20.0, 18.0);
  let (rect, response) = ui.allocate_exact_size(size, Sense::click());
  let centre = rect.center();
  let colour = if response.hovered() {
    palette.get(Role::TextHi)
  } else {
    palette.get(Role::TextDim)
  };
  let stroke = egui::Stroke::new(1.5, colour);
  match glyph {
    Glyph::Check => {
      ui.painter().line_segment(
        [
          egui::pos2(centre.x - 4.0, centre.y),
          egui::pos2(centre.x - 1.0, centre.y + 3.5),
        ],
        stroke,
      );
      ui.painter().line_segment(
        [
          egui::pos2(centre.x - 1.0, centre.y + 3.5),
          egui::pos2(centre.x + 4.5, centre.y - 3.5),
        ],
        stroke,
      );
    }
    Glyph::Cross => {
      ui.painter().line_segment(
        [
          egui::pos2(centre.x - 3.5, centre.y - 3.5),
          egui::pos2(centre.x + 3.5, centre.y + 3.5),
        ],
        stroke,
      );
      ui.painter().line_segment(
        [
          egui::pos2(centre.x + 3.5, centre.y - 3.5),
          egui::pos2(centre.x - 3.5, centre.y + 3.5),
        ],
        stroke,
      );
    }
  }
  response.on_hover_text(tooltip)
}

/// What a ribbon icon says on hover: the category's name first, then what opening it does.
fn ribbon_tooltip(category: Category) -> String {
  match category {
    Category::Toolpaths => {
      "TOOLPATHS — open the toolpath task: authoring, the tool card, the estimated time".to_owned()
    }
    Category::Relief3d => {
      "RELIEF AND 3D PART — open the 3D task: the split, and what it is playing".to_owned()
    }
    Category::MachineOutput => {
      "MACHINE AND OUTPUT — open the machine task: stock, material, zero".to_owned()
    }
  }
}

fn section_toggle(ui: &mut egui::Ui, palette: &Palette, label: &str, open: bool) -> egui::Response {
  ui.add_space(4.0);
  // The row is allocated as a widget before anything is painted into it, because that is what makes it
  // clickable: a response reused from a child layout answers no click, and this heading is the one control
  // in the rail that is drawn without a widget of its own.
  let width = ui.available_width();
  let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 14.0), Sense::click());
  let colour = palette.get(Role::TextDim);
  let (cx, cy) = (rect.left() + 5.0, rect.center().y);
  let points = if open {
    vec![
      egui::pos2(cx - 3.0, cy - 2.0),
      egui::pos2(cx + 3.0, cy - 2.0),
      egui::pos2(cx, cy + 3.0),
    ]
  } else {
    vec![
      egui::pos2(cx - 2.0, cy - 3.0),
      egui::pos2(cx - 2.0, cy + 3.0),
      egui::pos2(cx + 3.0, cy),
    ]
  };
  ui.painter().add(egui::Shape::convex_polygon(
    points,
    colour,
    egui::Stroke::NONE,
  ));
  ui.painter().text(
    egui::pos2(cx + 8.0, cy),
    egui::Align2::LEFT_CENTER,
    label,
    egui::FontId::proportional(theme::CAPTION),
    colour,
  );
  let width = ui.available_width();
  let (hairline, _) = ui.allocate_exact_size(egui::vec2(width, 1.0), Sense::hover());
  ui.painter()
    .rect_filled(hairline, 0.0, palette.get(Role::Hairline));
  response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A sentence in the shell's teaching voice, used where a stage has nothing to show yet.
fn note(ui: &mut egui::Ui, palette: &Palette, text: &str) {
  ui.label(
    RichText::new(text)
      .size(theme::CAPTION)
      .color(palette.get(Role::TextDim)),
  );
}

/// The words the material dialog says about itself.
///
/// Constants because the same string is both drawn and reported: the card the operator reads, the sentence
/// that refuses a number, and the words a headless test quotes back cannot drift apart.
const MATERIAL_TITLE: &str = "New job — define the material";
const MATERIAL_LEDE: &str =
  "Choose what the stock is, then set its three measurements. You can change them later in the Job tab.";
const MATERIAL_OPENER: &str = "Define the material…";
const SHEET_TITLE: &str = "Flat sheet";
const SHEET_SENTENCE: &str = "A panel laid flat: cut and carved in three axes.";
const ROTARY_TITLE: &str = "Rotary";
const ROTARY_REASON: &str =
  "Not available: every job in this document is a flat sheet, so there is no \
axis to turn and this card would write nothing.";
const PLAN_CAPTION: &str =
  "The dot is X0 Y0, where the drawing's coordinates start. Z0 is the sheet's top \
face, so a cut depth measures down from it.";
const CANCEL: &str = "Cancel";
const CONTINUE: &str = "Continue";

/// The sentence naming the first of three job measurements that cannot describe a stock.
///
/// The rule is the model's own — `Document::validate` refuses a non-positive dimension with
/// `ModelError::NonPositiveJobDimension` — and this is that rule said the way the shell says things: the
/// field, the number the operator typed, and what a sheet needs.
fn material_refusal_of(width: f64, height: f64, thickness: f64) -> Option<String> {
  for (field, value) in [
    ("Width X", width),
    ("Height Y", height),
    ("Thickness Z", thickness),
  ] {
    if !value.is_finite() || value <= 0.0 {
      return Some(format!(
        "{field} is {value:.0} mm. A sheet needs a width, a height and a thickness, all above 0 mm."
      ));
    }
  }
  None
}

/// One measurement in the material dialog: the label above, a unit-bearing number below.
///
/// A `DragValue` rather than the [`field`] box, because this one is editable, and it carries its `mm`
/// suffix so no number in the dialog is printed without its unit (S7).
fn measurement(
  ui: &mut egui::Ui,
  palette: &Palette,
  metrics: &theme::Metrics,
  label: &str,
  value: &mut f64,
) {
  ui.label(
    RichText::new(label)
      .size(theme::CAPTION)
      .color(palette.get(Role::TextDim)),
  );
  ui.add_space(metrics.space(1));
  ui.add(
    egui::DragValue::new(value)
      .speed(1.0)
      .range(0.0..=100_000.0)
      .suffix(" mm")
      .fixed_decimals(0),
  );
  ui.add_space(metrics.space(2));
}

/// One choice card: a glyph, a title and a sentence, with a tick when it is the chosen one.
///
/// Painted rather than built from a `selectable_label`, because the shape is the point: the reference's two
/// cards are compared at a glance, and the drawing on the left of each is what tells them apart. A card is
/// never clickable — the sheet is already the document's shape, and the rotary card's `available` is the
/// reason it cannot be, drawn on its own face instead of promised in a tooltip nobody opens.
#[allow(clippy::too_many_arguments)]
fn choice_card(
  ui: &mut egui::Ui,
  palette: &Palette,
  metrics: &theme::Metrics,
  glyph: Icon,
  width: f32,
  title: &str,
  sentence: &str,
  chosen: bool,
  unavailable: Option<()>,
) -> egui::Response {
  let pad = metrics.space(3);
  let glyph_size = 30.0;
  let text_width = width - pad * 3.0 - glyph_size;
  let ink_title = palette.get(Role::TextHi);
  // An unavailable card is not greyed out wholesale: its reason is the one thing it has to say, so it is
  // drawn in the dim ink a person can still read, and only the glyph is dark (S7: never colour-only).
  let ink_sentence = if unavailable.is_some() {
    palette.get(Role::TextDim)
  } else {
    palette.get(Role::Text)
  };
  let title_galley = ui.painter().layout(
    title.to_owned(),
    egui::FontId::proportional(theme::SMALL),
    ink_title,
    text_width,
  );
  let sentence_galley = ui.painter().layout(
    sentence.to_owned(),
    egui::FontId::proportional(theme::CAPTION),
    ink_sentence,
    text_width,
  );
  let text_height = title_galley.size().y + metrics.space(1) + sentence_galley.size().y;
  let height = (text_height + pad * 2.0).max(metrics.control_height * 2.0);
  let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), Sense::hover());
  let radius = egui::CornerRadius::same(metrics.radius_panel as u8);
  ui.painter().rect_filled(
    rect,
    radius,
    palette.get(if chosen { Role::SelSoft } else { Role::Raised }),
  );
  ui.painter().rect_stroke(
    rect,
    radius,
    egui::Stroke::new(
      metrics.hairline,
      palette.get(if chosen { Role::Sel } else { Role::Border }),
    ),
    egui::StrokeKind::Inside,
  );

  let glyph_rect = egui::Rect::from_min_size(
    egui::pos2(rect.left() + pad, rect.top() + pad),
    egui::vec2(glyph_size, glyph_size),
  );
  crate::icons::paint(
    ui.painter(),
    glyph_rect,
    glyph,
    palette.get(if chosen { Role::Sel } else { Role::TextOff }),
    metrics.icon_stroke * glyph_size / metrics.icon_size,
  );

  let text_left = glyph_rect.right() + pad;
  let mut cursor = rect.top() + pad;
  ui.painter().galley(
    egui::pos2(text_left, cursor),
    title_galley.clone(),
    ink_title,
  );
  cursor += title_galley.size().y + metrics.space(1);
  ui.painter().galley(
    egui::pos2(text_left, cursor),
    sentence_galley.clone(),
    ink_sentence,
  );

  if chosen {
    // A tick, not only a colour: the chosen card has to say so in a shape as well as a hue (S7).
    let centre = egui::pos2(
      rect.right() - pad - metrics.space(2),
      rect.top() + pad + metrics.space(1),
    );
    let ink = palette.get(Role::Sel);
    let stroke = egui::Stroke::new(metrics.icon_stroke, ink);
    ui.painter()
      .line_segment([centre + egui::vec2(-4.0, 0.0), centre], stroke);
    ui.painter()
      .line_segment([centre, centre + egui::vec2(4.0, -5.0)], stroke);
  }
  response
}

/// One dimension line: its two end ticks, the line between them, and its number.
fn dimension_line(
  painter: &egui::Painter,
  palette: &Palette,
  from: egui::Pos2,
  to: egui::Pos2,
  label: &str,
  label_offset: egui::Vec2,
  align: egui::Align2,
) {
  let stroke = egui::Stroke::new(1.0, palette.get(Role::TextDim));
  painter.line_segment([from, to], stroke);
  let tick = (to - from).normalized().rot90() * 3.5;
  painter.line_segment([from - tick, from + tick], stroke);
  painter.line_segment([to - tick, to + tick], stroke);
  let mid = from + (to - from) / 2.0 + label_offset;
  painter.text(
    mid,
    align,
    label,
    egui::FontId::monospace(theme::CAPTION),
    palette.get(Role::TextHi),
  );
}

/// The key plan the material dialog draws: the stock box from the three numbers, annotated.
///
/// An axonometric box, not a picture: S6 forbids a new dependency and an image asset is not a token, so the
/// drawing is a projection and a handful of lines. The two
/// ratios below are its only invention — they are what makes a box read as a box — and everything else is
/// derived from `width`, `height` and `thickness`, which is what makes the plan respond to the numbers as
/// they are typed instead of illustrating a shape chosen in advance.
///
/// Colours are the T9 material roles and nothing else: [`Role::StockTop`] for the face the tool touches,
/// [`Role::StockBottom`] for the sides, [`Role::StockEdge`] for the silhouette at the icon set's own stroke.
/// The origin marker is the selection signal, because it *is* the selection's frame rather than a verdict.
fn paint_material_plan(
  painter: &egui::Painter,
  rect: egui::Rect,
  palette: &Palette,
  metrics: &theme::Metrics,
  dims: (f64, f64, f64),
  labels: [&str; 3],
) {
  const DEPTH_X: f32 = 0.62;
  const DEPTH_Y: f32 = 0.45;
  let (width, height, thickness) = dims;
  // A stock with a dimension of zero has a dimension of zero: the drawing collapses to a line rather than
  // inventing a shape for it, and the refusal under the plan says why. Only the *fit* is floored, so the
  // arithmetic never divides by one.
  let w = width.max(0.0) as f32;
  let h = height.max(0.0) as f32;
  let t = thickness.max(0.0) as f32;

  let plan = rect.shrink2(egui::vec2(metrics.space(2), metrics.space(2)));
  // The band the width dimension line and its number live in, below the box.
  let below = metrics.space(5);
  // Room for the three annotations *outside* the box: the thickness line and its number on the left, the
  // depth line and its number on the right, the width line under it. A number drawn over the stock's own
  // light face is unreadable, and the first screenshot of this plan caught exactly that — the depth label
  // sat on the top face, and the thickness label was clipped at the left edge — so these insets are the
  // fix and the reason.
  // The thickness number is the one annotation whose width the drawing does not control — it is as wide as
  // "18 mm" or "100 mm" happens to be — so its inset is *measured* rather than guessed at, which is what the
  // first screenshot of this plan showed: the label was clipped to its last two characters at the left edge.
  let thickness_room = metrics.space(3)
    + metrics.space(2)
    + painter
      .layout_no_wrap(
        labels[2].to_owned(),
        egui::FontId::monospace(theme::CAPTION),
        palette.get(Role::TextHi),
      )
      .size()
      .x
    + metrics.space(1);
  let inner = egui::Rect::from_min_max(
    egui::pos2(
      (plan.left() + thickness_room).min(plan.right() - 1.0),
      plan.top() + metrics.space(2),
    ),
    egui::pos2(
      plan.right() - metrics.space(6) - metrics.space(4),
      (plan.bottom() - below - metrics.space(2)).max(plan.top() + 1.0),
    ),
  );
  let extent_x = (w + DEPTH_X * h).max(1.0);
  let extent_y = (DEPTH_Y * h + t).max(1.0);
  let scale = (inner.width() / extent_x).min(inner.height() / extent_y);
  let origin = egui::pos2(
    inner.center().x - extent_x * scale / 2.0,
    inner.center().y - extent_y * scale / 2.0 + DEPTH_Y * h * scale,
  );
  let at = |x: f32, y: f32, z: f32| -> egui::Pos2 {
    egui::pos2(
      origin.x + (x + DEPTH_X * y) * scale,
      origin.y - (DEPTH_Y * y + z) * scale,
    )
  };

  // The three visible faces: top, front (y = 0) and right (x = width).
  let face = |points: [egui::Pos2; 4], role: Role| {
    painter.add(egui::Shape::convex_polygon(
      points.to_vec(),
      palette.get(role),
      egui::Stroke::NONE,
    ));
  };
  face(
    [
      at(0.0, 0.0, 0.0),
      at(w, 0.0, 0.0),
      at(w, h, 0.0),
      at(0.0, h, 0.0),
    ],
    Role::StockTop,
  );
  face(
    [
      at(0.0, 0.0, 0.0),
      at(w, 0.0, 0.0),
      at(w, 0.0, -t),
      at(0.0, 0.0, -t),
    ],
    Role::StockBottom,
  );
  face(
    [at(w, 0.0, 0.0), at(w, h, 0.0), at(w, h, -t), at(w, 0.0, -t)],
    Role::StockBottom,
  );
  // The silhouette, at the icon set's line weight: a 1 px stroke is antialiased over two and stops reading
  // as an edge (the measurement T9 made when the stock's rim went to 1.6).
  let edge = egui::Stroke::new(metrics.icon_stroke, palette.get(Role::StockEdge));
  for (from, to) in [
    (at(0.0, 0.0, 0.0), at(w, 0.0, 0.0)),
    (at(w, 0.0, 0.0), at(w, h, 0.0)),
    (at(w, h, 0.0), at(0.0, h, 0.0)),
    (at(0.0, h, 0.0), at(0.0, 0.0, 0.0)),
    (at(0.0, 0.0, 0.0), at(0.0, 0.0, -t)),
    (at(w, 0.0, 0.0), at(w, 0.0, -t)),
    (at(w, h, 0.0), at(w, h, -t)),
    (at(0.0, h, 0.0), at(0.0, h, -t)),
    (at(0.0, 0.0, -t), at(w, 0.0, -t)),
    (at(w, 0.0, -t), at(w, h, -t)),
  ] {
    painter.line_segment([from, to], edge);
  }

  // X along the front bottom edge, Y along the right bottom edge, Z up the front left corner.
  let gap = metrics.space(3);
  dimension_line(
    painter,
    palette,
    at(0.0, 0.0, -t) + egui::vec2(0.0, below * 0.55),
    at(w, 0.0, -t) + egui::vec2(0.0, below * 0.55),
    labels[0],
    egui::vec2(0.0, metrics.space(1)),
    egui::Align2::CENTER_TOP,
  );
  // The depth edge runs up and to the right, and `rot90` of it points up and to the left — into the sheet.
  // The outward side is the negation, and getting it wrong prints the depth number across the top face.
  let normal = -egui::vec2(DEPTH_X, -DEPTH_Y).normalized().rot90();
  dimension_line(
    painter,
    palette,
    at(w, 0.0, 0.0) + normal * gap,
    at(w, h, 0.0) + normal * gap,
    labels[1],
    normal * metrics.space(2),
    egui::Align2::LEFT_CENTER,
  );
  dimension_line(
    painter,
    palette,
    at(0.0, 0.0, 0.0) + egui::vec2(-gap, 0.0),
    at(0.0, 0.0, -t) + egui::vec2(-gap, 0.0),
    labels[2],
    egui::vec2(-metrics.space(2), 0.0),
    egui::Align2::RIGHT_CENTER,
  );

  // The origin marker the caption explains, and the two plan axes it belongs to.
  let origin_at = at(0.0, 0.0, 0.0);
  let ink = palette.get(Role::Sel);
  let stroke = egui::Stroke::new(metrics.icon_stroke, ink);
  let along_x = axis_tip(origin_at, egui::vec2(1.0, 0.0), metrics.space(4));
  let along_y = axis_tip(
    origin_at,
    egui::vec2(DEPTH_X, -DEPTH_Y).normalized(),
    metrics.space(4),
  );
  for (tip, direction) in [along_x, along_y] {
    painter.line_segment([origin_at, tip], stroke);
    let back = tip - direction * metrics.space(2);
    let wing = direction.rot90() * metrics.space(1);
    painter.line_segment([tip, back + wing], stroke);
    painter.line_segment([tip, back - wing], stroke);
  }
  painter.circle_filled(origin_at, metrics.space(1) + 1.0, ink);
}

/// The far end of a direction of `length` from `from`, and that direction again so a caller can draw a head.
fn axis_tip(from: egui::Pos2, direction: egui::Vec2, length: f32) -> (egui::Pos2, egui::Vec2) {
  (from + direction * length, direction)
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
