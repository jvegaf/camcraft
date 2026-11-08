use cam_app::AppState;
use cam_model::Toolpath;
use cam_program::program_for_snapshot;
use cam_store::tools::{render_name, stored_tool_for_number, StoredTool, ToolGeometry};
use cam_units::Format;
use egui::{Align, Layout, RichText, Sense};

use crate::theme::{self, Palette, Role};
use crate::view3d::{clock_text, seconds_for};
use crate::{Shell, ShellOutput};

use super::paint::{icon_button, section, section_toggle};
use super::{Category, Tab, ToolSubject};

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
      // The strategy editor's draft goes with the task (camui-prancheta S10): `Cancel` leaves the document, the
      // tool and the name exactly as they were, and the next opening re-reads them.
      self.strategy.discard();
    }
    self.task = None;
    self.inspector_tab = Tab::Selection;
  }

  /// The toolpath the `TOOLPATHS` surface describes, by id, or `None` when the operator has chosen none.
  ///
  /// The raw selection and not [`Shell::described_toolpath`]: this is the view state itself, which is what a
  /// test and a host set and read, and the two are deliberately not the same question.
  pub fn toolpath_selection(&self) -> Option<&str> {
    self.selected_toolpath.as_deref()
  }

  /// Choose the toolpath the card and the editor describe, or clear the choice with `None`.
  ///
  /// **Selecting is not an edit.** It changes what the screen says and nothing the document holds: no command
  /// is run, no undo step is added and the document cannot read as modified. `None` is the answer this shell
  /// gave before the list existed — the document's first toolpath.
  pub fn set_toolpath_selection(&mut self, id: Option<String>) {
    self.selected_toolpath = id;
  }

  /// The toolpath this shell describes: the selection when the document still holds it, and the document's
  /// first toolpath otherwise.
  ///
  /// One answer for the list, the current-tool card and the editor, so a frame cannot show a card for one
  /// toolpath and a form for another. A selection that outlives its toolpath — the toolpath was deleted, or a
  /// document was opened over it — is not an error: it reads as no selection, which is the first toolpath.
  pub fn described_toolpath<'a>(&self, app: &'a AppState) -> Option<&'a Toolpath> {
    crate::resolve_toolpath(app, self.toolpath_selection())
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
          // The strategy editor's draft is re-read from the document *before* the header is drawn, so the frame
          // that shows a header also shows the subject it names (L13): a header computed from last frame's seed
          // would name a toolpath the panel had already stopped editing.
          if category == Category::Toolpaths {
            // The editor's subject is the selected toolpath when there is one, and the document's first
            // otherwise; the selection is view state, so this reads it and writes nothing.
            let selection = self.toolpath_selection().map(str::to_owned);
            self.strategy.sync(app, selection.as_deref());
          }
          self.task_header(app, ui, &palette, category, output);
          ui.add_space(metrics.space(2));
        }
        match self.task {
          Some(category) => {
            egui::ScrollArea::vertical()
              .id_salt("inspector_body")
              .max_height(body)
              .show(ui, |ui| match category {
                Category::Toolpaths => {
                  // The list the reference has and this shell did not (M13 S4): one row per toolpath, one of
                  // them selectable, and the selection — not `first()` — is what the editor below and the card
                  // inside it describe.
                  self.toolpath_list(app, ui, &palette, output);
                  // S10's editor is the card, the quick block and the sections, in one surface for creating and
                  // editing; the tool card is drawn **inside** it, from the draft while creating and from the
                  // document while editing, so there is one card and not two.
                  self.strategy_editor(app, ui, &palette, output);
                  // The geometry authoring body S6 put behind this category is not part of a strategy, and it is
                  // the only surface that reaches those commands: it stays below the editor, unchanged.
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
    app: &AppState,
    ui: &mut egui::Ui,
    palette: &Palette,
    category: Category,
    output: &mut ShellOutput,
  ) {
    // The header names the task. For `TOOLPATHS` the task *is* a strategy, and S10 gives it its own word:
    // `New toolpath · <name>` while the editor drafts one and `Edit · <name>` while it edits the document's.
    let label = match category {
      Category::Toolpaths => self
        .strategy
        .header(&crate::strategy::subject(app, self.toolpath_selection())),
      other => other.label().to_owned(),
    };
    ui.horizontal(|ui| {
      ui.label(
        RichText::new(&label)
          .size(theme::CAPTION)
          .color(palette.get(Role::TextHi)),
      );
      output.drawn.task_editor = label;
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
        "3D simulates the drawing's own toolpaths when you press Simulate in the picture. The split's overlay row, its 
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
      "Nothing is simulated yet — press Simulate in the picture."
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
  /// does not grow them. The reason is the card's own, not the document's: since M11 a document **does**
  /// carry `job.material`, but [`cam_store::tools::stored_tool_for_number`] still takes no material and no
  /// machine (S3), so the rates this card could print are the tool's own record — which is exactly the pair
  /// a program stops using the moment a material is selected. A rate here would be a number the machine
  /// does not run.
  ///
  /// The number resolves through the same function the emitter resolves it with, so the card cannot say
  /// "T7 is missing" beside a program that cuts with T7: the two read one rule (S3). The unresolved case
  /// is a state and not a blank — it names the number and says the library the window was given does not
  /// hold it, because a card that drew nothing would leave the operator guessing whether the tool is
  /// missing or the panel is.
  ///
  /// The shell describes the toolpath the operator selected in the `TOOLPATHS` list, and the document's **first**
  /// one when nothing is selected — the answer this card gave before the list existed, so a one-toolpath
  /// document reads exactly as it did ([`Shell::described_toolpath`], M13 T3, S4).
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
    let Some(toolpath) = self.described_toolpath(app) else {
      return;
    };
    let name = toolpath.name.clone();
    let number = Some(toolpath.tool_number);
    self.tool_card_for(
      app,
      ui,
      palette,
      output,
      ToolSubject {
        name: &name,
        number,
        estimate: true,
      },
    );
  }

  /// The card itself, for a tool number the caller owns.
  ///
  /// Split from [`Shell::tool_card`] because the strategy editor draws the same card for two subjects
  /// (camui-prancheta S10): the document's toolpath while editing, and its own draft while creating one. The
  /// words, the opener and the estimate are one implementation, so the create and edit surfaces cannot show two
  /// different cards; `estimate` is the one part that needs a document to measure, and a draft has none.
  pub(crate) fn tool_card_for(
    &mut self,
    app: &AppState,
    ui: &mut egui::Ui,
    palette: &Palette,
    output: &mut ShellOutput,
    subject: ToolSubject<'_>,
  ) {
    let heading = format!("TOOL — {}", subject.name);
    section(ui, palette, &heading);
    output.drawn.tool_card.push(heading);

    match subject.number {
      Some(number) => match stored_tool_for_number(&self.tools, number) {
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
      },
      // A draft that has not chosen a tool yet: a sentence, never a blank card, because an operator looking at
      // nothing cannot tell "no tool chosen" from "the panel is broken".
      None => {
        let sentence = crate::strategy::NO_TOOL_YET;
        ui.label(
          RichText::new(sentence)
            .size(theme::CAPTION)
            .color(palette.get(Role::Danger)),
        );
        output.drawn.tool_card.push(sentence.to_owned());
      }
    }

    // The one control on the card: the way to change the tool it reads out (camui-prancheta S9). It is drawn
    // whatever the card said above — a toolpath whose tool the library does not hold is exactly the case in
    // which an operator wants to choose one — and its words are reported apart from the card's own lines,
    // because the card is a reading of the tool and this is the way to replace it.
    self.choose_tool_row(app, ui, output);

    if !subject.estimate {
      return;
    }
    // A line of its own rather than a field of the card (S2), measured over the document's own program
    // and written by the playback clock's formatter (S5). No second time formula, no second clock.
    //
    // No material is selected yet — the shell has no control that names one — and a snapshot holds no
    // material key to select from anyway (M11 T2), so this is the program the document's own toolpaths
    // and the snapshot's own records describe.
    let estimate = match program_for_snapshot(app.document(), &self.tools, None) {
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
pub(crate) fn tool_name(geometry: &ToolGeometry) -> String {
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
