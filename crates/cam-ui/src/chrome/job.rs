use cam_app::AppState;
use egui::{Align, Layout, RichText, Sense};

use crate::icons::Icon;
use crate::theme::{self, Palette, Role};
use crate::{Shell, ShellOutput, UiAction};

use super::paint::{dimension_line, field, section};

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
  /// The job tab: what is being cut, and out of what.
  ///
  /// Fields are labelled above and two to a row, the way the reference app lays out MATERIAL: a
  /// machinist reads "Width X 600" and "Thickness Z 18" as one block, not as a list of sentences.
  ///
  /// The boxes report; the opener above them writes. A job's three measurements are one decision made
  /// once, so they get a dialog with a drawn plan instead of three text fields that can each be
  /// half-edited — which is the shape the reference's `Novo trabalho` frame shows and the row for it in
  /// `design/Screenshots-reference/README.md` settles.
  pub(super) fn job_section(
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
