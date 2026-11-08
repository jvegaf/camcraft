use cam_app::AppState;
use egui::RichText;

use crate::chrome;
use crate::theme::{self, Palette, Role};
use crate::{Shell, ShellOutput};

use super::{
  cut_fields, generate, kind_of, refusal, subject, Kind, CANCEL, CUT, GENERATE, NAME, NOTE, QUICK,
};

impl Shell {
  /// The strategy editor (S10): one surface for creating and editing.
  ///
  /// Called from the `TOOLPATHS` task body, which is the surface the editor has in this shell: the header above
  /// it carries [`Draft::header`](crate::strategy::Draft::header), and the current-tool card, the quick block,
  /// the `CUT` section, the note and the footer are drawn here in the spec's order. Every control that cannot
  /// act is replaced by the sentence that says why, and every reported point is a point this frame really drew.
  pub(crate) fn strategy_editor(
    &mut self,
    app: &AppState,
    ui: &mut egui::Ui,
    palette: &Palette,
    output: &mut ShellOutput,
  ) {
    let metrics = self.metrics();
    let subject = subject(app, self.toolpath_selection());
    // The editor is drawn inside the inspector's scroll area, and a row below its viewport is painted by egui and
    // clipped away. Reporting its centre would be a promise about a control nobody can press — the defect L23
    // found in the playback bar — so every point this frame reports is filtered against the clip it drew in.
    let clip = ui.clip_rect();

    // 1. the name. One field in both modes: `cam-app` has `SetToolpathName`, so a name typed here is a name
    //    `Generate toolpath` can write — when creating and when editing. A control that can act is drawn in both.
    ui.label(
      RichText::new(NAME)
        .size(theme::CAPTION)
        .color(palette.get(Role::TextDim)),
    );
    let field = ui.add(
      egui::TextEdit::singleline(&mut self.strategy.name)
        .hint_text("the toolpath's name")
        .desired_width(f32::INFINITY),
    );
    let centre = field.rect.center();
    output
      .drawn
      .strategy_hits
      .push((NAME.to_owned(), (centre.x, centre.y)));
    ui.add_space(metrics.space(2));

    // 2. the current-tool card, with its own opener and this frame's number. While editing, the number is the
    //    document's: the modal writes the document, so a draft copy could only disagree with the card.
    let name = self.strategy.name.clone();
    let (number, estimate) = match self.described_toolpath(app) {
      Some(toolpath) => (Some(toolpath.tool_number), true),
      None => (self.strategy.tool_number, false),
    };
    self.tool_card_for(
      app,
      ui,
      palette,
      output,
      chrome::ToolSubject {
        name: &name,
        number,
        estimate,
      },
    );

    // 3. the quick block: the same numbers the section below holds, one click closer.
    let quick = chrome::section_toggle(ui, palette, QUICK, self.strategy.quick_open);
    let centre = quick.rect.center();
    output
      .drawn
      .strategy_hits
      .push((QUICK.to_owned(), (centre.x, centre.y)));
    if quick.clicked() {
      self.strategy.quick_open = !self.strategy.quick_open;
    }
    output.drawn.strategy_quick_open = self.strategy.quick_open;
    if self.strategy.quick_open {
      self.quick_block(app, ui, palette, output);
    }

    // 4. the strategy's own sections.
    chrome::section(ui, palette, CUT);
    self.kind_cards(ui, palette, &metrics, output);
    self.region_line(app, ui, palette);
    for field in cut_fields(&self.strategy.operation) {
      self.number_row(ui, palette, *field, &mut output.drawn.strategy_fields);
    }
    if kind_of(&self.strategy.operation) == Some(Kind::Pocket) {
      self.clearing_cards(ui, palette, &metrics, output);
    }

    // 5. the note, in the spec's own words.
    ui.add_space(metrics.space(2));
    ui.label(
      RichText::new(NOTE)
        .size(theme::CAPTION)
        .color(palette.get(Role::TextDim)),
    );
    output.drawn.strategy_note = NOTE.to_owned();
    ui.add_space(metrics.space(2));

    // 6. the footer. `Cancel` can always be honoured because nothing here has written anything; `Generate
    //    toolpath` is drawn only when the action it names exists, and the sentence takes its place otherwise.
    let refusal = refusal(app, &self.strategy, &subject);
    if let Some(sentence) = &refusal {
      ui.label(
        RichText::new(sentence)
          .size(theme::CAPTION)
          .color(palette.get(Role::Danger)),
      );
      output.drawn.strategy_refusal = sentence.clone();
    }
    ui.horizontal(|ui| {
      if refusal.is_none() {
        let generate_button = ui.button(RichText::new(GENERATE).size(theme::SMALL));
        let centre = generate_button.rect.center();
        output
          .drawn
          .strategy_hits
          .push((GENERATE.to_owned(), (centre.x, centre.y)));
        if generate_button.clicked() {
          output
            .actions
            .extend(generate(app, &self.strategy, &subject));
        }
      }
      let back = ui.button(RichText::new(CANCEL).size(theme::SMALL));
      let centre = back.rect.center();
      output
        .drawn
        .strategy_hits
        .push((CANCEL.to_owned(), (centre.x, centre.y)));
      if back.clicked() {
        self.close_task(true);
      }
    });

    output
      .drawn
      .strategy_hits
      .retain(|(_, (x, y))| clip.contains(egui::pos2(*x, *y)));
  }

  /// The sentence that says what the strategy's region is, in the reference's own place.
  ///
  /// It is the honest replacement for the reference's frontier row: a strategy's region is its `entity_ids`,
  /// which this shell chooses on the canvas, so the editor says how many vectors it holds instead of offering a
  /// choice between three pictures of one set.
  fn region_line(&self, app: &AppState, ui: &mut egui::Ui, palette: &Palette) {
    let count = match self.described_toolpath(app) {
      Some(toolpath) => toolpath.entity_ids.len(),
      None => app.selection().ids().count(),
    };
    let sentence = format!(
      "{count} vector(s) form this strategy's region — the vectors are chosen on the canvas, so this shell \
       offers no frontier to choose here."
    );
    ui.label(
      RichText::new(&sentence)
        .size(theme::CAPTION)
        .color(palette.get(Role::TextDim)),
    );
  }
}
