use cam_app::AppState;

use cam_model::Toolpath;

use cam_store::tools::{render_name, stored_tool_for_toolpath, StoredTool, ToolGeometry};

use cam_units::Format;

use egui::{Align, Layout, RichText, Sense};

use crate::theme::{self, Palette, Role};

use crate::{Shell, ShellOutput};

use super::paint::section;

use super::{Category, Tab, ToolSubject};

impl Shell {
  /// Which body the inspector is showing.
  pub fn tab(&self) -> Tab {
    self.inspector_tab
  }

  /// Show a tab.
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

  /// The toolpath the `TOOLPATHS` surface describes, by id, or `None` when the operator has chosen none.
  /// The raw selection and not [`Shell::described_toolpath`]: this is the view state itself, which is what a
  /// test and a host set and read, and the two are deliberately not the same question.
  pub fn toolpath_selection(&self) -> Option<&str> {
    self.selected_toolpath.as_deref()
  }

  /// Choose the toolpath the card and the editor describe, or clear the choice with `None`.
  /// **Selecting is not an edit.** It changes what the screen says and nothing the document holds: no command
  /// is run, no undo step is added and the document cannot read as modified. `None` is the answer this shell
  /// gave before the list existed — the document's first toolpath.
  pub fn set_toolpath_selection(&mut self, id: Option<String>) {
    self.selected_toolpath = id;
  }

  /// The toolpath this shell describes: the selection when the document still holds it, and the document's
  /// first toolpath otherwise.
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

  pub(crate) fn toolpath_list(
    &mut self,
    app: &AppState,
    ui: &mut egui::Ui,
    _palette: &Palette,
    output: &mut ShellOutput,
  ) {
    let toolpaths = &app.document().toolpaths;
    output.drawn.toolpath_rows.clear();
    output.drawn.toolpath_hits.clear();
    if toolpaths.is_empty() {
      output.drawn.toolpath_selected = None;
      return;
    }
    let selected = self.described_toolpath(app).map(|t| t.id.clone());
    output.drawn.toolpath_selected = selected.clone();
    for toolpath in toolpaths {
      let label = format!("{} — {}", toolpath.id, toolpath.name);
      let is_selected = selected.as_deref() == Some(toolpath.id.as_str());
      let response = ui.selectable_label(is_selected, &label);
      output
        .drawn
        .toolpath_rows
        .push((label.clone(), is_selected));
      let c = response.rect.center();
      output.drawn.toolpath_hits.push((label, (c.x, c.y)));
      if response.clicked() {
        self.set_toolpath_selection(Some(toolpath.id.clone()));
      }
    }
  }

  /// The task editor's header: the task's name, and the two glyphs that leave it.
  /// `✓` keeps what is typed and `✕` throws away the one draft the panel holds (see [`Shell::close_task`]);
  /// both are painted rather than written as `✓` and `✕`, for the same reason the rail's caret is painted: a
  /// glyph the bundled faces do not carry renders as a box, and a box is not an affordance.
  pub(crate) fn task_header(
    &mut self,

    _app: &AppState,

    ui: &mut egui::Ui,

    palette: &Palette,

    category: Category,

    output: &mut ShellOutput,
  ) {
    let label = category.label().to_owned();

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
  /// The reference opens a 3D machining strategy here. This shell authors no relief — its 3D half plays a
  /// program `cam-cli` produced — so the task says exactly that, opens or closes the split, and names whether
  /// a program is loaded. It is the same refusal the rest of the shell makes: no form for a capability that
  /// does not exist (L13).
  pub(crate) fn relief_task(
    &mut self,

    _app: &AppState,

    ui: &mut egui::Ui,

    palette: &Palette,

    output: &mut ShellOutput,
  ) {
    ui.label(
      RichText::new("This shell authors no relief and no 3D strategy.")
        .size(theme::CAPTION)
        .color(palette.get(Role::TextDim)),
    );

    ui.label(

      RichText::new(

        "The 3D split shows a program loaded for playback when one is available. Nothing here authors a toolpath.",

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
      "No program is loaded for playback."
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
      self.set_view_3d(!self.show_3d);
    }

    output.drawn.task_editor = Category::Relief3d.label().to_owned();
  }

  /// The current-tool card: the tool a toolpath cuts with, in the two lines the reference frame uses.
  /// `03-27-41`'s `Novo percurso — Chanfro` flyout puts a card at the top of a toolpath form: the tool's
  /// number and name, then its type with the shape's numbers and its flute count. **Nothing else is on
  /// it** (S2): the frame shows no material, no machine and no feed, plunge or spindle rate, so the card
  /// does not grow them. The reason is the card's own, not the document's: since M11 a document **does**
  /// carry `job.material`, but [`cam_store::tools::stored_tool_for_number`] still takes no material and no
  /// machine (S3), so the rates this card could print are the tool's own record — which is exactly the pair
  /// a program stops using the moment a material is selected. A rate here would be a number the machine
  /// does not run.
  /// The number resolves through the same function the emitter resolves it with, so the card cannot say
  /// "T7 is missing" beside a program that cuts with T7: the two read one rule (S3). The unresolved case
  /// is a state and not a blank — it names the number and says the library the window was given does not
  /// hold it, because a card that drew nothing would leave the operator guessing whether the tool is
  /// missing or the panel is.
  /// The shell describes the toolpath the operator selected in the `TOOLPATHS` list, and the document's **first**
  /// one when nothing is selected — the answer this card gave before the list existed, so a one-toolpath
  /// document reads exactly as it did ([`Shell::described_toolpath`], M13 T3, S4).
  /// Below the card's own lines sits the **estimated cutting time** — a line of its own, not a fourth
  /// field, because the card's fields are exactly the frame's (S2). Its seconds are
  /// [`seconds_for`] over the program this document's own toolpaths build from the same snapshot, so the
  /// number describes what the machine would cut and is the number the playback clock would report for
  /// it (S5); [`clock_text`] writes it, so the two readouts are formatted by one rule. When that program
  /// cannot be built there is no time to show, and the emitter's sentence says which tool or toolpath
  /// stopped it instead.
  pub(crate) fn tool_card(
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
    let tool_id = toolpath.tool_id.as_deref();
    let number = Some(toolpath.tool_number);

    self.tool_card_for(
      app,
      ui,
      palette,
      output,
      ToolSubject {
        name: &name,
        tool_id,
        number,
        estimate: true,
      },
    );
  }

  /// The card itself, for a tool number the caller owns.
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

    let resolved =
      stored_tool_for_toolpath(&self.tools, subject.tool_id, subject.number.unwrap_or(0));
    match resolved {
      Some(tool) => {
        let number = tool.cutting.tool_number.or(subject.number).unwrap_or(0);
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

      None if subject.tool_id.is_some() || subject.number.is_some() => {
        let label = subject
          .number
          .map(|n| format!("T{n}"))
          .or_else(|| subject.tool_id.map(|id| format!("tool '{id}'")))
          .unwrap_or_else(|| "tool".to_owned());
        let sentence = format!(
          "{label} is not in the tool library this window was given, so this toolpath has no tool to show."
        );

        ui.label(
          RichText::new(&sentence)
            .size(theme::CAPTION)
            .color(palette.get(Role::Danger)),
        );

        output.drawn.tool_card.push(sentence);
      }

      // A draft that has not chosen a tool yet: a sentence, never a blank card, because an operator looking at

      // nothing cannot tell "no tool chosen" from "the panel is broken".
      None => {
        let sentence = "No tool chosen yet.";

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

    ui.add_space(6.0);
  }

}

/// The word every tool display name in this build is rendered with, where the tool's type would go.
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

