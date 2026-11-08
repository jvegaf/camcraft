use cam_app::{AppState, VertexEdit};
use cam_geom::Point2;
use egui::{RichText, Sense};

use crate::help::{Command, Group, MenuScope};
use crate::icons::Icon;
use crate::theme::{self, Palette, Role, Theme};
use crate::{Shell, ShellOutput, UiAction};

use super::paint::{command_icon, icon_button, section_toggle};

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

impl Shell {
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
      Command::Script => self.show_script = !self.show_script,
      Command::View3d => self.show_3d = !self.show_3d,
      Command::Theme => self.flip_theme(),
      _ => output.actions.extend(self.command_action(command, app)),
    }
  }

  /// Switch the colour set, from the app bar's button or from the canvas menu's `Theme` row.
  pub(super) fn flip_theme(&mut self) {
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
    Command::Script => format!("{label} — run a command script and read its report"),
    _ => label.to_owned(),
  }
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
