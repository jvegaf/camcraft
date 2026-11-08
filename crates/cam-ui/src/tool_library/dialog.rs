use cam_app::AppState;
use cam_store::tools::{StoredTool, ToolGeometry};
use egui::{Align, Layout, RichText, Sense};

use crate::theme::{self, Role};
use crate::{Shell, ShellOutput, UiAction};

use super::drawing::{paint_section, DRAWING_HEIGHT};
use super::family::ToolFamily;
use super::form::{form_fields, form_groups};
use super::tree::{row_matches, tool_title, tool_with, tree_rows, Row};

/// The master column's width: wide enough for a tool title and its `Ø6 · 2 cut(s)` line.
const MASTER_WIDTH: f32 = 296.0;

/// The detail column's width: the form's two boxes a row, with the reference's proportions.
const DETAIL_WIDTH: f32 = 460.0;

/// How tall the two columns are, so the modal does not grow and shrink with the form.
const COLUMN_HEIGHT: f32 = 344.0;

/// The words the modal says about itself.
///
/// Constants because the same string is both drawn and reported: the title, the sentence that refuses a
/// commit, and the words a headless test quotes back cannot drift apart.
const TITLE: &str = "Choose tool";
const SEARCH_LABEL: &str = "Find a tool";
const SEARCH_HINT: &str = "name, number, id or shape";
const CLEAR: &str = "Clear";
const USE: &str = "Use this tool";
const CANCEL: &str = "Cancel";
const NO_TOOL: &str = "Choose a tool in the tree and the form will describe it.";
const NO_DIAMETER: &str =
  "This tool carries no diameter, so there is no width to draw. Its numbers are below.";
const NO_MATCH: &str = "No tool matches that search.";
const EMPTY_LIBRARY: &str =
  "The library this window was given holds no tool, so there is nothing to list or to use.";
const NO_TARGET: &str =
  "This window was opened without a toolpath, so there is no toolpath for a choice to apply to.";
/// The footer's action row, and the whole of what this shell can do about the library.
const LIBRARY_ACTIONS: &str = "New, Duplicate, Delete, Import… and Export… are not here: this window holds a \
read-only snapshot of the library, so nothing in it could write one. `cam-cli tools add` is how a library \
is filled today.";

/// A tool the modal can draw and describe, with nothing borrowed.
#[derive(Clone, Debug)]
struct Detail {
  /// The tool's title, the modal's own heading once a tool is chosen.
  title: String,
  /// The number the program names it by, when the library holds one.
  number: Option<u32>,
  /// The shape the drawing is made from.
  geometry: ToolGeometry,
  /// The groups and fields the form draws, in order.
  groups: Vec<(String, Vec<(String, String)>)>,
}

/// Everything the detail side needs about one tool.
fn detail_of(tool: &StoredTool) -> Detail {
  Detail {
    title: tool_title(tool),
    number: tool.cutting.tool_number,
    geometry: tool.geometry.clone(),
    groups: form_groups(tool)
      .into_iter()
      .map(|group| (group.label().to_owned(), form_fields(group, tool)))
      .collect(),
  }
}

/// A pressable thing the modal drew, with the rectangle it occupied and the area it was clipped to.
///
/// The clip is kept per control, and for one reason: a row scrolled out of the master list still produces a
/// shape, and a report that named its centre would be a promise about a control nobody can see. The T6 review
/// found exactly that in the playback bar — invisible, with live hit areas, behind 43 green tests (L23) — so
/// the report drops anything the frame that drew it would not have shown.
struct Spot {
  /// The word on the control.
  word: String,
  /// Where it was drawn.
  rect: egui::Rect,
  /// The area the control was clipped to when it was drawn.
  clip: egui::Rect,
}

impl Spot {
  /// A control drawn inside `ui`'s current clip rectangle.
  fn in_ui(word: String, rect: egui::Rect, ui: &egui::Ui) -> Self {
    Self {
      word,
      rect,
      clip: ui.clip_rect(),
    }
  }
}

/// Drop every spot its own clip cut away, and report the rest as `(word, point)`.
///
/// The point reported is the centre, so the centre is what has to be inside the clip: a row with two pixels
/// showing would otherwise report a point in the fold.
fn pressable(spots: Vec<Spot>) -> Vec<(String, (f32, f32))> {
  spots
    .into_iter()
    .filter(|spot| spot.clip.contains(spot.rect.center()))
    .map(|spot| {
      let centre = spot.rect.center();
      (spot.word, (centre.x, centre.y))
    })
    .collect()
}

impl Shell {
  /// Why `Use this tool` cannot be honoured right now, or `None` when it can.
  ///
  /// The order is the operator's: an empty library first, then the toolpath that opened the dialog, then the
  /// choice itself. Each sentence names the thing that is missing, so a dark button is never a mystery — and
  /// the button is not drawn at all while one is returned (camui-prancheta S9, L6).
  pub fn tool_library_refusal(&self, app: &AppState) -> Option<String> {
    self.refusal_for(
      app,
      tool_with(&self.tools, self.tool_library_selected.as_deref()),
    )
  }

  /// Why `Use this tool` cannot be honoured for one particular choice.
  ///
  /// Split from [`Shell::tool_library_refusal`] because the frame asks it **after** the tree has run: the
  /// answer the operator sees must be about the row this frame selected, not the one the last frame had
  /// (L13's reporting rule, which the T3 task paid for).
  fn refusal_for(&self, app: &AppState, tool: Option<&StoredTool>) -> Option<String> {
    // A draft opened by the strategy editor has no document row to write (S10): the one thing the commit can
    // mean is "this strategy cuts with this tool", so all it needs is a tool with a number.
    if self.tool_library_for_draft {
      let Some(tool) = tool else {
        return Some(NO_TOOL.to_owned());
      };
      if tool.cutting.tool_number.is_none() {
        return Some(format!(
          "{} carries no tool number in the library, so a toolpath cannot name it.",
          tool_title(tool)
        ));
      }
      return None;
    }
    if self.tools.is_empty() {
      return Some(EMPTY_LIBRARY.to_owned());
    }
    let Some(target) = self.tool_library_target.as_deref() else {
      return Some(NO_TARGET.to_owned());
    };
    if !app
      .document()
      .toolpaths
      .iter()
      .any(|toolpath| toolpath.id == target)
    {
      return Some(format!(
        "The toolpath this dialog was opened for ({target}) is no longer in the document, so there is \
         nothing to retool."
      ));
    }
    let Some(tool) = tool else {
      return Some(NO_TOOL.to_owned());
    };
    if tool.cutting.tool_number.is_none() {
      return Some(format!(
        "{} carries no tool number in the library, so a toolpath cannot name it.",
        tool_title(tool)
      ));
    }
    None
  }

  /// The tool library's modal: the master tree, the detail form and the footer.
  ///
  /// One `egui::Modal`, the shape [`crate::chrome`]'s material dialog already uses: the reference draws this
  /// screen over the task panel rather than inside it, and a modal is what tells the operator that the
  /// drawing underneath is not taking clicks.
  ///
  /// **The frame that selects a row is the frame that describes it** (L13). The tree is drawn first and the
  /// detail is resolved from the selection the tree just made, so a click and its consequence are in one
  /// frame; the title is the only thing drawn above the tree, so its text is painted into a rect reserved
  /// before it and filled after the tree has run — a title one frame behind the form would be the same
  /// reporting defect one layer up.
  pub(crate) fn tool_library_dialog(
    &mut self,
    ctx: &egui::Context,
    app: &AppState,
    output: &mut ShellOutput,
  ) {
    let palette = self.palette();
    let metrics = self.metrics();
    // The drafts are copied out so the closure can edit them: the modal writes to `self` once, after the
    // frame, and nothing else about the shell changes while it is open.
    let mut search = self.tool_search.clone();
    let mut selected = self.tool_library_selected.clone();
    // A selection the library no longer holds is no selection: the tools this window was given can be replaced
    // between two frames, and a form describing a tool nobody has would be describing nothing.
    if tool_with(&self.tools, selected.as_deref()).is_none() {
      selected = None;
    }

    let tree = tree_rows(&self.tools);

    let mut words: Vec<String> = Vec::new();
    let mut spots: Vec<Spot> = Vec::new();
    let mut families: Vec<(String, usize)> = Vec::new();
    let mut rows: Vec<(String, String, String)> = Vec::new();
    let mut groups: Vec<(String, Vec<(String, String)>)> = Vec::new();
    let mut callouts: Vec<String> = Vec::new();
    let mut drawing: Option<(egui::Rect, f32)> = None;
    let mut cancel = false;
    let mut commit: Option<u32> = None;

    egui::Modal::new(egui::Id::unique("campilot-tool-library"))
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
        let columns = MASTER_WIDTH + DETAIL_WIDTH + metrics.space(6);
        ui.set_max_width(columns);
        // Reserved now, painted at the end: the title names the tool this frame chose.
        let (title_rect, _) =
          ui.allocate_exact_size(egui::vec2(columns, metrics.space(6)), Sense::hover());
        ui.add_space(metrics.space(2));

        let mut detail: Option<Detail> = None;
        ui.horizontal_top(|ui| {
          // ---- the master side: the search field and the family tree -------------------------------
          ui.vertical(|ui| {
            ui.set_width(MASTER_WIDTH);
            ui.label(
              RichText::new(SEARCH_LABEL)
                .size(theme::CAPTION)
                .color(palette.get(Role::TextDim)),
            );
            ui.horizontal(|ui| {
              ui.add(
                egui::TextEdit::singleline(&mut search)
                  .hint_text(SEARCH_HINT)
                  .desired_width(MASTER_WIDTH - 52.0),
              );
              // A filter with no way out except selecting the text and deleting it is a trap; the chip is the
              // same vocabulary the status bar's snaps use.
              if !search.is_empty() {
                let clear = crate::chrome::chip(ui, &palette, CLEAR, false);
                spots.push(Spot::in_ui(CLEAR.to_owned(), clear.rect, ui));
                if clear.clicked() {
                  search.clear();
                }
              }
            });
            ui.add_space(metrics.space(2));
            egui::ScrollArea::vertical()
              .id_salt("tool_tree")
              .max_height(COLUMN_HEIGHT)
              .show(ui, |ui| {
                // Every row reports the clip of the scroll area it was drawn in (see `Spot`), so a row the
                // fold hides is drawn by egui and is not reported as pressable.
                let mut shown = 0;
                for family in ToolFamily::ALL {
                  let group: Vec<&Row> = tree
                    .iter()
                    .filter(|entry| entry.0 == family && row_matches(&entry.1, family, &search))
                    .map(|entry| &entry.1)
                    .collect();
                  if group.is_empty() {
                    continue;
                  }
                  shown += group.len();
                  let heading = format!("{} ({})", family.label(), group.len());
                  ui.add_space(metrics.space(1));
                  ui.label(
                    RichText::new(&heading)
                      .size(theme::CAPTION)
                      .color(palette.get(Role::TextDim)),
                  );
                  families.push((family.label().to_owned(), group.len()));
                  words.push(heading);
                  for row in group {
                    let on = selected.as_deref() == Some(row.id.as_str());
                    let (rect, response) = ui.allocate_exact_size(
                      egui::vec2(ui.available_width(), metrics.row_height),
                      Sense::click(),
                    );
                    let radius = egui::CornerRadius::same(4);
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
                    // The mini tool glyph: the set's own router bit, at the row's size.
                    let glyph = egui::Rect::from_min_size(
                      egui::pos2(rect.left() + 4.0, rect.top() + 3.0),
                      egui::vec2(20.0, 20.0),
                    );
                    crate::icons::paint(
                      ui.painter(),
                      glyph,
                      crate::icons::Icon::ToolLibrary,
                      palette.get(if on { Role::Sel } else { Role::Text }),
                      theme::ICON_STROKE * glyph.width() / theme::ICON_SIZE,
                    );
                    let text_left = glyph.right() + 6.0;
                    ui.painter().text(
                      egui::pos2(text_left, rect.top() + 3.0),
                      egui::Align2::LEFT_TOP,
                      &row.title,
                      egui::FontId::proportional(theme::SMALL),
                      palette.get(if on { Role::TextHi } else { Role::Text }),
                    );
                    ui.painter().text(
                      egui::pos2(text_left, rect.bottom() - 3.0),
                      egui::Align2::LEFT_BOTTOM,
                      &row.cut,
                      egui::FontId::proportional(theme::CAPTION),
                      palette.get(Role::TextDim),
                    );
                    if response.clicked() {
                      selected = Some(row.id.clone());
                    }
                    spots.push(Spot::in_ui(row.title.clone(), rect, ui));
                    words.push(row.title.clone());
                    words.push(row.cut.clone());
                    rows.push((
                      family.label().to_owned(),
                      row.title.clone(),
                      row.cut.clone(),
                    ));
                  }
                }
                if shown == 0 {
                  let sentence = if self.tools.is_empty() {
                    EMPTY_LIBRARY
                  } else {
                    NO_MATCH
                  };
                  ui.add_space(metrics.space(2));
                  ui.label(
                    RichText::new(sentence)
                      .size(theme::CAPTION)
                      .color(palette.get(Role::TextDim)),
                  );
                  words.push(sentence.to_owned());
                }
                ui.add_space(metrics.space(2));
              });
          });

          // Read here, between the two columns: this is the selection the tree has just made.
          detail = tool_with(&self.tools, selected.as_deref()).map(detail_of);

          ui.add_space(metrics.space(3));
          // ---- the detail side: the scaled drawing and the family's form ----------------------------
          ui.vertical(|ui| {
            ui.set_width(DETAIL_WIDTH);
            egui::ScrollArea::vertical()
              .id_salt("tool_detail")
              .max_height(COLUMN_HEIGHT)
              .show(ui, |ui| match &detail {
                Some(detail) => {
                  if detail.geometry.diameter.is_some() {
                    let (rect, _) = ui.allocate_exact_size(
                      egui::vec2(DETAIL_WIDTH, DRAWING_HEIGHT),
                      Sense::hover(),
                    );
                    let width = paint_section(
                      ui.painter(),
                      rect,
                      &palette,
                      &metrics,
                      &detail.geometry,
                      &mut callouts,
                    );
                    words.extend(callouts.iter().cloned());
                    drawing = Some((rect, width));
                  } else {
                    ui.label(
                      RichText::new(NO_DIAMETER)
                        .size(theme::CAPTION)
                        .color(palette.get(Role::TextDim)),
                    );
                    words.push(NO_DIAMETER.to_owned());
                  }
                  for (label, fields) in &detail.groups {
                    crate::chrome::section(ui, &palette, label);
                    words.push(label.clone());
                    ui.horizontal_wrapped(|ui| {
                      for (field, value) in fields {
                        crate::chrome::field(ui, &palette, field, value);
                        words.push((*field).clone());
                        words.push((*value).clone());
                      }
                    });
                  }
                  groups = detail.groups.clone();
                }
                None => {
                  ui.label(
                    RichText::new(NO_TOOL)
                      .size(theme::CAPTION)
                      .color(palette.get(Role::TextDim)),
                  );
                  words.push(NO_TOOL.to_owned());
                }
              });
          });
        });

        ui.add_space(metrics.space(3));
        // ---- the footer: the library's actions, then the commit pair -------------------------------
        ui.separator();
        ui.horizontal_wrapped(|ui| {
          ui.label(
            RichText::new(LIBRARY_ACTIONS)
              .size(theme::CAPTION)
              .color(palette.get(Role::TextDim)),
          );
        });
        words.push(LIBRARY_ACTIONS.to_owned());
        ui.add_space(metrics.space(2));
        // Asked here, after the tree, so the sentence is about the row this frame chose.
        let refusal = self.refusal_for(app, tool_with(&self.tools, selected.as_deref()));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
          // `Cancel` is always drawn: nothing in this modal has written anything before it, so it can always
          // be honoured. `Use this tool` is drawn only when the write it names has a meaning — otherwise the
          // sentence that says why is drawn in its place (L6).
          let back = ui.button(RichText::new(CANCEL).size(theme::SMALL));
          spots.push(Spot::in_ui(CANCEL.to_owned(), back.rect, ui));
          if back.clicked() {
            cancel = true;
          }
          match &refusal {
            Some(sentence) => {
              ui.label(
                RichText::new(sentence)
                  .size(theme::CAPTION)
                  .color(palette.get(Role::Danger)),
              );
              words.push(sentence.clone());
            }
            None => {
              let go = ui.add_enabled(
                detail.is_some(),
                egui::Button::new(RichText::new(USE).size(theme::SMALL)),
              );
              spots.push(Spot::in_ui(USE.to_owned(), go.rect, ui));
              if go.clicked() {
                commit = detail.as_ref().and_then(|detail| detail.number);
              }
            }
          }
        });

        // The title, with the tool this frame selected in it (see this method's own note).
        let title = match &detail {
          Some(detail) => format!("{TITLE} — {}", detail.title),
          None => TITLE.to_owned(),
        };
        ui.painter().text(
          title_rect.left_top(),
          egui::Align2::LEFT_TOP,
          &title,
          egui::FontId::proportional(theme::TITLE),
          palette.get(Role::TextHi),
        );
        words.push(title);
      });

    self.tool_search = search;
    self.tool_library_selected = selected;
    output.drawn.tool_library = words;
    output.drawn.tool_library_hits = pressable(spots);
    output.drawn.tool_families = families;
    output.drawn.tool_rows = rows;
    output.drawn.tool_selected = self.tool_library_selected.clone();
    output.drawn.tool_groups = groups;
    output.drawn.tool_callouts = callouts;
    if let Some((rect, width)) = drawing {
      output.drawn.tool_drawing = Some(crate::chrome::rect_report(rect));
      output.drawn.tool_width_px = width;
    }
    if cancel {
      self.close_tool_library();
    }
    if let Some(number) = commit {
      if self.tool_library_for_draft {
        // The draft's tool, not the document's: `Generate toolpath` is what writes it (S10).
        self.strategy.tool_number = Some(number);
      } else if let Some(target) = self.tool_library_target.clone() {
        output.actions.push(UiAction::SetToolpathToolNumber {
          toolpath_id: target,
          tool_number: number,
        });
      }
      self.close_tool_library();
    }
  }
}
