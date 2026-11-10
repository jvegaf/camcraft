//! The tool library's modal: a header band, three columns, and a footer (tool-library-editor S1, S2, S8).
//!
//! The header band is reserved before anything is drawn into it and its title is painted **after** the body, so
//! the sentence naming the tool the operator just chose is the tool that frame selected: a title one frame
//! behind the form is a reporting defect, the same one L13 named about the detail side.
//!
//! **A draft belongs to the row it was seeded from.** The detail column edits a [`ToolDraft`] rather than the
//! library record, and the draft is re-seeded whenever the selection names a different tool — or names nothing.
//! That is the whole of the discard rule, and it is deliberate: an edited draft for a row the operator leaves is
//! thrown away, because the alternative is a form that writes one tool's numbers into another tool. `Cancel`
//! therefore has nothing to undo beyond the visit itself, and `Use this tool` is the only press in this modal
//! that writes.

use cam_app::AppState;
use cam_store::tools::{StoredTool, ToolKind, ToolLibrary};
use cam_units::UnitSystem;
use egui::{Align, Layout, RichText, Sense};

use crate::theme::{self, Metrics, Palette, Role};
use crate::{Shell, ShellOutput, UiAction};

use super::draft::ToolDraft;
use super::form::{form_rows, FormField};
use super::library::NO_LIBRARY;
use super::preview::{paint_preview, PREVIEW_HEIGHT, PREVIEW_WIDTH};
use super::shell::ToolLibraryRequest;
use super::tree::{row_matches, tool_title, tool_with, tree_rows, Row};

/// The master column's width: wide enough for a tool title and its `Ø6 · 2 cut(s)` line.
const MASTER_WIDTH: f32 = 300.0;

/// The detail column's width: a label on the left, its box and its unit on the right.
const DETAIL_WIDTH: f32 = 424.0;

/// How tall the master list is, so the modal does not grow and shrink with the tree.
const COLUMN_HEIGHT: f32 = 372.0;

/// The header band's height: the title's own line plus the padding that keeps it off the edges.
const HEADER_HEIGHT: f32 = 34.0;

/// The width of one editable box.
const BOX_WIDTH: f32 = 96.0;

/// The slot the unit suffix occupies, fixed so the boxes line up down the column.
///
/// A fixed slot and not the suffix's own width: `mm`, `rpm` and `mm/min` are three different lengths, and a
/// column whose boxes moved with the word beside them would be a column of ragged numbers.
const UNIT_SLOT: f32 = 44.0;

/// The mini tool glyph a tree row carries.
const ROW_GLYPH: f32 = 20.0;

/// The words the modal says about itself.
///
/// Constants because the same string is both drawn and reported: the title, the sentence that refuses a
/// commit, and the words a headless test quotes back cannot drift apart.
const TITLE: &str = "Choose tool";
const SEARCH_HINT: &str = "Find a tool…";
const NAME_HINT: &str = "Name";
const TYPE_LABEL: &str = "Type";
const USE: &str = "Use this tool";
const CANCEL: &str = "Cancel";
const CLOSE: &str = "Close";
const FOOTER_NOTE: &str =
  "Changes apply to new toolpaths; each toolpath keeps its own copy of the tool.";
const NO_TOOL: &str = "Choose a tool in the tree and the form will describe it.";
const NO_MATCH: &str = "No tool matches that search.";
const EMPTY_LIBRARY: &str = "The library holds no tool, so there is nothing to list or to use.";
const NO_TARGET: &str =
  "This window was opened without a toolpath, so there is no toolpath for a choice to apply to.";

/// The words on the master column's three rows of library actions, in S3's order and grouping.
///
/// Constants because the same word is both painted and reported: the report a test reads and the button the
/// operator presses cannot drift apart.
const NEW: &str = "+ New";
const DUPLICATE: &str = "Duplicate";
const DELETE: &str = "Delete";
const EXPORT: &str = "Export…";
const IMPORT: &str = "Import…";
const RESTORE: &str = "Restore defaults";

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

/// One row of the form: its word at the left, and at the right its box and the unit outside it.
///
/// The text is handed in rather than read here, because the two kinds of row get theirs from different places:
/// an editable row shows what the operator typed, and a computed one shows what the boxes above it add up to.
/// Only the editable row writes back, and only when it changed, so a frame that drew the form cannot rewrite
/// the draft with the text it was given. The caller reports the row, because the words a row draws are the
/// caller's list.
fn field_row(
  ui: &mut egui::Ui,
  palette: &Palette,
  metrics: &Metrics,
  draft: &mut ToolDraft,
  field: FormField,
  text: String,
  units: UnitSystem,
) -> String {
  let mut text = text;
  ui.horizontal(|ui| {
    let gap = ui.spacing().item_spacing.x;
    let label_width = (ui.available_width() - BOX_WIDTH - UNIT_SLOT - 2.0 * gap).max(80.0);
    ui.allocate_ui_with_layout(
      egui::vec2(label_width, metrics.control_height),
      Layout::left_to_right(Align::Center),
      |ui| {
        ui.label(
          RichText::new(field.label())
            .size(theme::SMALL)
            .color(palette.get(Role::Text)),
        );
      },
    );

    if field.editable() {
      let response = ui.add(egui::TextEdit::singleline(&mut text).desired_width(BOX_WIDTH));
      if response.changed() {
        draft.set_text(field, text.clone());
      }
    } else {
      // A reading, not a box: the rect is drawn so the row still lines up with the ones above it, and the text
      // is painted rather than typed into. `Chipload` is the one number on the form that is arithmetic on the
      // others, which is why it is the one drawn in the warning colour (S6).
      let (rect, _) = ui.allocate_exact_size(
        egui::vec2(BOX_WIDTH, metrics.control_height),
        Sense::hover(),
      );
      let radius = egui::CornerRadius::same(4);
      ui.painter()
        .rect_filled(rect, radius, palette.get(Role::Raised));
      ui.painter().rect_stroke(
        rect,
        radius,
        egui::Stroke::new(metrics.hairline, palette.get(Role::Border)),
        egui::StrokeKind::Inside,
      );
      let ink = if field == FormField::Chipload {
        palette.get(Role::Warn)
      } else {
        palette.get(Role::TextHi)
      };
      ui.painter().text(
        rect.right_center() - egui::vec2(8.0, 0.0),
        egui::Align2::RIGHT_CENTER,
        &text,
        egui::FontId::monospace(theme::SMALL),
        ink,
      );
    }

    // The suffix's own slot, outside the box and immediately right of it, so every box in the column ends at the
    // same place whatever unit follows it.
    let (slot, _) = ui.allocate_exact_size(
      egui::vec2(UNIT_SLOT, metrics.control_height),
      Sense::hover(),
    );
    if let Some(unit) = field.unit(units) {
      ui.painter().text(
        slot.left_center() + egui::vec2(6.0, 0.0),
        egui::Align2::LEFT_CENTER,
        unit,
        egui::FontId::proportional(theme::CAPTION),
        palette.get(Role::TextDim),
      );
    }
  });
  text
}

/// What pressing one of the master column's rows asks for (S3).
///
/// The two file rows are apart from [`ToolLibraryRequest`] because their answer is a path only the host's dialog
/// can return: the modal asks, and the window's own write happens when that path comes back as a
/// [`UiAction`].
#[derive(Clone, Copy, Debug, PartialEq)]
enum RowRequest {
  /// A write this window can make itself (S10).
  Library(ToolLibraryRequest),
  /// Ask the host for a path to export the library to (S11).
  Export,
  /// Ask the host for a library file to import (S11).
  Import,
}

/// One row of the master column's library actions (S3).
///
/// Each button is an ordinary [`egui::Button`] at [`theme::SMALL`], laid out left to right inside the master
/// column's [`MASTER_WIDTH`]. A row that cannot be honoured is drawn with [`egui::Ui::add_enabled`] and reports
/// no point: visible, so the operator learns the action exists, and unpressed, because a press would emit
/// nothing (L14). The row is reported as a row of words either way, which is what makes "drawn but dark" a
/// reportable state instead of an absence.
///
/// The request is returned so the caller applies it after the frame: nothing in this modal writes while a
/// widget is being drawn.
fn action_row(
  ui: &mut egui::Ui,
  row: &[(&'static str, bool, RowRequest)],
  spots: &mut Vec<Spot>,
  words: &mut Vec<String>,
  actions: &mut Vec<String>,
) -> Option<RowRequest> {
  let mut pressed = None;
  ui.horizontal(|ui| {
    for (word, enabled, request) in row {
      let button = egui::Button::new(RichText::new(*word).size(theme::SMALL));
      let response = if *enabled {
        ui.add(button)
      } else {
        ui.add_enabled(false, button)
      };
      words.push((*word).to_owned());
      actions.push((*word).to_owned());
      if *enabled {
        spots.push(Spot::in_ui((*word).to_owned(), response.rect, ui));
        if response.clicked() {
          pressed = Some(*request);
        }
      }
    }
  });
  pressed
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

  /// The tool library's modal: the header band, the master tree, the detail form, the preview and the footer.
  ///
  /// One `egui::Modal`, the shape [`crate::chrome`]'s material dialog already uses: the reference draws this
  /// screen over the task panel rather than inside it, and a modal is what tells the operator that the
  /// drawing underneath is not taking clicks.
  ///
  /// **The frame that selects a row is the frame that describes it** (L13). The tree is drawn first and the
  /// draft, the form and the preview are resolved from the selection the tree just made, so a click and its
  /// consequence are in one frame; the header band is reserved above the tree and its title is painted after
  /// the body for the same reason.
  ///
  /// **The draft is seeded from the selected row, and discarded when the selection leaves it.** At the top of
  /// every frame the modal holds one draft; the rule is checked right after the tree, against the row the tree
  /// has just chosen: a draft whose `geometry_id` is that row's is kept — that is the form the operator is
  /// typing into — and a draft for any other row, or no draft at all while a row is chosen, is replaced by a
  /// fresh [`ToolDraft::of`] on that row. A selection that names no row clears the draft with it. So an edit
  /// typed into one tool never reaches another, and leaving a row is the discard.
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
    let mut draft = self.tool_draft.clone();
    let mut closed = self.tool_groups_closed.clone();
    let tree = tree_rows(&self.tools);

    // The three columns and the page margin either side of them. The header band spans all of it, which is why
    // the modal's own frame carries no inner margin (S1).
    let columns = [MASTER_WIDTH, DETAIL_WIDTH, PREVIEW_WIDTH];
    let margin = metrics.space(3);
    let width = columns.iter().sum::<f32>() + 2.0 * metrics.space(3) + 2.0 * margin;

    let mut words: Vec<String> = Vec::new();
    let mut actions: Vec<String> = Vec::new();
    let mut spots: Vec<Spot> = Vec::new();
    let mut families: Vec<(String, usize)> = Vec::new();
    let mut rows: Vec<(String, String, String)> = Vec::new();
    let mut groups: Vec<(String, Vec<(String, String)>)> = Vec::new();
    let mut computed: Vec<(String, String)> = Vec::new();
    let mut callouts: Vec<String> = Vec::new();
    let mut caption: Option<String> = None;
    let mut header = String::new();
    let mut drawing: Option<(egui::Rect, f32)> = None;
    let mut cancel = false;
    let mut commit: Option<StoredTool> = None;
    let mut request: Option<RowRequest> = None;

    egui::Modal::new(egui::Id::unique("campilot-tool-library"))
      .frame(
        egui::Frame::NONE
          .fill(palette.get(Role::Overlay))
          .stroke(egui::Stroke::new(
            metrics.hairline,
            palette.get(Role::Border),
          ))
          .corner_radius(egui::CornerRadius::same(metrics.radius_dialog as u8)),
      )
      .show(ctx, |ui| {
        ui.set_max_width(width);
        // Reserved now and titled at the end: the band's fill and its cross are known before the body is, but
        // the sentence in the middle names the tool this frame chose.
        let (band, _) = ui.allocate_exact_size(egui::vec2(width, HEADER_HEIGHT), Sense::hover());
        let radius = metrics.radius_dialog as u8;
        ui.painter().rect_filled(
          band,
          egui::CornerRadius {
            nw: radius,
            ne: radius,
            sw: 0,
            se: 0,
          },
          palette.get(Role::Raised),
        );
        let cross = egui::Rect::from_center_size(
          egui::pos2(band.right() - 16.0, band.center().y),
          egui::vec2(20.0, 20.0),
        );
        let close = ui.interact(
          cross,
          ui.scope_id().with("campilot-tool-library-close"),
          Sense::click(),
        );
        // The cross is painted and not written as `✕`, for the reason the rail's caret is: a glyph the bundled
        // faces may not carry renders as a box, and a box is not an affordance.
        let ink = if close.hovered() {
          palette.get(Role::TextHi)
        } else {
          palette.get(Role::TextDim)
        };
        let stroke = egui::Stroke::new(1.5, ink);
        let centre = cross.center();
        ui.painter().line_segment(
          [
            egui::pos2(centre.x - 4.0, centre.y - 4.0),
            egui::pos2(centre.x + 4.0, centre.y + 4.0),
          ],
          stroke,
        );
        ui.painter().line_segment(
          [
            egui::pos2(centre.x + 4.0, centre.y - 4.0),
            egui::pos2(centre.x - 4.0, centre.y + 4.0),
          ],
          stroke,
        );
        spots.push(Spot::in_ui(CLOSE.to_owned(), cross, ui));
        if close.clicked() {
          cancel = true;
        }

        egui::Frame::NONE
          .inner_margin(egui::Margin::same(margin as i8))
          .show(ui, |ui| {
            ui.horizontal_top(|ui| {
              // ---- the master side: the search field and the kind tree -------------------------------
              ui.vertical(|ui| {
                ui.set_width(MASTER_WIDTH);
                ui.add(
                  egui::TextEdit::singleline(&mut search)
                    .hint_text(SEARCH_HINT)
                    .desired_width(MASTER_WIDTH),
                );
                ui.add_space(metrics.space(2));
                egui::ScrollArea::vertical()
                  .id_salt("tool_tree")
                  .max_height(COLUMN_HEIGHT)
                  .show(ui, |ui| {
                    // Every row reports the clip of the scroll area it was drawn in (see `Spot`), so a row the
                    // fold hides is drawn by egui and is not reported as pressable.
                    let mut shown = 0;
                    for kind in ToolKind::ALL {
                      let group: Vec<&Row> = tree
                        .iter()
                        .filter(|entry| entry.0 == kind && row_matches(&entry.1, kind, &search))
                        .map(|entry| &entry.1)
                        .collect();
                      if group.is_empty() {
                        continue;
                      }
                      shown += group.len();
                      let heading = format!("{} ({})", kind.label(), group.len());
                      let open = !closed.contains(&kind);
                      let toggle = crate::chrome::section_toggle(ui, &palette, &heading, open);
                      spots.push(Spot::in_ui(heading.clone(), toggle.rect, ui));
                      words.push(heading);
                      families.push((kind.label().to_owned(), group.len()));
                      if toggle.clicked() {
                        if open {
                          closed.push(kind);
                        } else {
                          closed.retain(|folded| *folded != kind);
                        }
                      }
                      // Read again after the click, so folding a heading hides its rows in the same frame.
                      if closed.contains(&kind) {
                        continue;
                      }
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
                          egui::vec2(ROW_GLYPH, ROW_GLYPH),
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
                        rows.push((kind.label().to_owned(), row.title.clone(), row.cut.clone()));
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

                // ---- the library action rows: S3's three groups at the foot of the master column ----
                //
                // The four rows that write a library share one condition — a library file this window may
                // write — and `Duplicate` and `Delete` need a tool to act on besides. The two file dialogs
                // need neither: a dialog returns a path and the shell writes it, so they are always live
                // (S11). A dark row is drawn and reported and is not a hit, which is the only shape the
                // module's first rule allows.
                ui.add_space(metrics.space(2));
                let writes = self.tool_library_path.is_some();
                let has_tool = selected.is_some();
                for row in [
                  vec![
                    (NEW, writes, RowRequest::Library(ToolLibraryRequest::New)),
                    (
                      DUPLICATE,
                      writes && has_tool,
                      RowRequest::Library(ToolLibraryRequest::Duplicate),
                    ),
                    (
                      DELETE,
                      writes && has_tool,
                      RowRequest::Library(ToolLibraryRequest::Delete),
                    ),
                  ],
                  vec![
                    (EXPORT, true, RowRequest::Export),
                    (IMPORT, true, RowRequest::Import),
                  ],
                  vec![(
                    RESTORE,
                    writes,
                    RowRequest::Library(ToolLibraryRequest::RestoreDefaults),
                  )],
                ] {
                  if let Some(pressed) = action_row(ui, &row, &mut spots, &mut words, &mut actions)
                  {
                    request = Some(pressed);
                  }
                }
                if !writes {
                  // The six rows write the application's own library (tool-library-app S1), so the only state
                  // that darkens the four that write is a library the application could not open — and the
                  // sentence is the failure's own, kept by `Shell::open_app_library`, rather than a guess
                  // about where a file should have been.
                  let sentence = self
                    .tool_library_error
                    .clone()
                    .unwrap_or_else(|| NO_LIBRARY.to_owned());
                  ui.label(
                    RichText::new(&sentence)
                      .size(theme::CAPTION)
                      .color(palette.get(Role::TextDim)),
                  );
                  words.push(sentence);
                }
              });

              // Read between the columns: this is the selection the tree has just made, and the draft that
              // belongs to it (see this method's own note on seeding and discarding).
              let chosen = tool_with(&self.tools, selected.as_deref());
              let reseed = match (&draft, chosen) {
                (Some(draft), Some(tool)) => draft.geometry_id != tool.geometry.id,
                (None, Some(_)) | (Some(_), None) => true,
                (None, None) => false,
              };
              if reseed {
                draft = chosen.map(ToolDraft::of);
              }

              ui.add_space(metrics.space(3));
              // ---- the detail side: the name, the kind, and the form's four groups ---------------------
              ui.vertical(|ui| {
                ui.set_width(DETAIL_WIDTH);
                let (Some(draft), Some(tool)) = (draft.as_mut(), chosen) else {
                  ui.label(
                    RichText::new(NO_TOOL)
                      .size(theme::CAPTION)
                      .color(palette.get(Role::TextDim)),
                  );
                  words.push(NO_TOOL.to_owned());
                  return;
                };
                let mut name = draft.name.clone();
                let name_box = ui.add(
                  egui::TextEdit::singleline(&mut name)
                    .hint_text(NAME_HINT)
                    .desired_width(DETAIL_WIDTH),
                );
                if name_box.changed() {
                  draft.name = name;
                }
                words.push(draft.name.clone());

                ui.horizontal(|ui| {
                  ui.label(
                    RichText::new(TYPE_LABEL)
                      .size(theme::SMALL)
                      .color(palette.get(Role::Text)),
                  );
                  words.push(TYPE_LABEL.to_owned());
                  ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let mut kind = draft.kind;
                    egui::ComboBox::from_id_salt("tool_kind")
                      .selected_text(kind.label())
                      .show_ui(ui, |ui| {
                        for option in ToolKind::ALL {
                          ui.selectable_value(&mut kind, option, option.label());
                        }
                      });
                    if kind != draft.kind {
                      draft.set_kind(kind);
                    }
                  });
                });
                words.push(draft.kind.label().to_owned());

                // The rows follow the kind the combo shows, not the one the library still holds: choosing
                // `V-bit` has to reveal the angle box in the same frame, and the kind is what decides which
                // shape rows a tool has (`ShapeField::of`).
                let mut probe = tool.clone();
                probe.geometry.tool_type = draft.kind.raw();
                let units = tool.cutting.rate_units;
                for (group, fields) in form_rows(&probe) {
                  crate::chrome::section(ui, &palette, group.label());
                  words.push(group.label().to_owned());
                  let mut reported: Vec<(String, String)> = Vec::new();
                  for field in fields {
                    // An editable row shows what was typed; a computed one shows what the boxes above it add
                    // up to *now*, which is why it is asked for at the moment the row is drawn.
                    let text = if field.editable() {
                      draft.text(field).to_owned()
                    } else {
                      draft
                        .commit()
                        .ok()
                        .map(|committed| field.value(&committed))
                        .unwrap_or_else(|| field.value(tool))
                    };
                    let text = field_row(ui, &palette, &metrics, draft, field, text, units);
                    words.push(field.label().to_owned());
                    words.push(text.clone());
                    if let Some(unit) = field.unit(units) {
                      words.push(unit.to_owned());
                    }
                    if !field.editable() {
                      computed.push((field.label().to_owned(), text.clone()));
                    }
                    reported.push((field.label().to_owned(), text));
                  }
                  groups.push((group.label().to_owned(), reported));
                }
              });

              ui.add_space(metrics.space(3));
              // ---- the preview: the tool in its card, and the kind's name under it ----------------------
              ui.vertical(|ui| {
                ui.set_width(PREVIEW_WIDTH);
                let Some(draft) = draft.as_ref() else {
                  return;
                };
                // The drawing follows the boxes: the preview is the tool the operator is describing, so it is
                // read from the draft when the draft describes one and from the library record while it does
                // not.
                let Some(fallback) = chosen else {
                  return;
                };
                let committed = draft.commit().ok();
                let shown = committed.as_ref().unwrap_or(fallback);
                let kind = draft.kind;
                let (rect, _) =
                  ui.allocate_exact_size(egui::vec2(PREVIEW_WIDTH, PREVIEW_HEIGHT), Sense::hover());
                let width = paint_preview(
                  ui.painter(),
                  rect,
                  &palette,
                  &metrics,
                  &shown.geometry,
                  kind,
                  &mut callouts,
                );
                drawing = Some((rect, width));
                words.extend(callouts.iter().cloned());
                ui.label(
                  RichText::new(kind.label())
                    .size(theme::CAPTION)
                    .color(palette.get(Role::TextDim)),
                );
                words.push(kind.label().to_owned());
                caption = Some(kind.label().to_owned());
              });
            });
          });

        // ---- the footer: one sentence, then the commit pair -------------------------------------------
        let (rule, _) = ui.allocate_exact_size(egui::vec2(width, metrics.hairline), Sense::hover());
        ui.painter()
          .rect_filled(rule, 0.0, palette.get(Role::Border));
        egui::Frame::NONE
          .inner_margin(egui::Margin::same(margin as i8))
          .show(ui, |ui| {
            ui.label(
              RichText::new(FOOTER_NOTE)
                .size(theme::CAPTION)
                .color(palette.get(Role::TextDim)),
            );
            words.push(FOOTER_NOTE.to_owned());
            ui.add_space(metrics.space(2));
            if let Some(sentence) = &self.tool_error {
              ui.label(
                RichText::new(sentence)
                  .size(theme::CAPTION)
                  .color(palette.get(Role::Danger)),
              );
              words.push(sentence.clone());
            }
            // Asked here, after the tree and the form, so the sentence is about the row this frame chose and
            // the boxes this frame holds. The library's own refusal keeps its order; the draft's sentence is
            // the second question, because a draft that does not commit is a tool the library cannot hold.
            let refusal = self
              .refusal_for(app, tool_with(&self.tools, selected.as_deref()))
              .or_else(|| draft.as_ref().and_then(ToolDraft::problem));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
              // `Cancel` is always drawn, and it is added first so it is the rightmost of the pair: nothing in
              // this modal has written anything before it, so it can always be honoured. `Use this tool` is
              // drawn only when the write it names has a meaning — otherwise the sentence that says why is
              // drawn in its place (L6).
              let back = ui.add(egui::Button::new(RichText::new(CANCEL).size(theme::SMALL)));
              spots.push(Spot::in_ui(CANCEL.to_owned(), back.rect, ui));
              words.push(CANCEL.to_owned());
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
                  let go = ui.add(
                    egui::Button::new(
                      RichText::new(USE)
                        .size(theme::SMALL)
                        .color(palette.get(Role::TextHi)),
                    )
                    .fill(palette.get(Role::Sel)),
                  );
                  spots.push(Spot::in_ui(USE.to_owned(), go.rect, ui));
                  words.push(USE.to_owned());
                  if go.clicked() {
                    commit = draft.as_ref().and_then(|draft| draft.commit().ok());
                  }
                }
              }
            });
          });

        // The title, with the tool this frame selected in it (see this method's own note).
        let title = match tool_with(&self.tools, selected.as_deref()) {
          Some(tool) => format!("{TITLE} — {}", tool_title(tool)),
          None => TITLE.to_owned(),
        };
        ui.painter().text(
          band.center(),
          egui::Align2::CENTER_CENTER,
          &title,
          egui::FontId::proportional(theme::TITLE),
          palette.get(Role::TextHi),
        );
        header = title.clone();
        words.push(title);
      });

    self.tool_search = search;
    self.tool_library_selected = selected;
    self.tool_draft = draft;
    self.tool_groups_closed = closed;
    output.drawn.tool_library = words;
    output.drawn.tool_library_hits = pressable(spots);
    output.drawn.tool_actions = actions;
    output.drawn.tool_families = families;
    output.drawn.tool_rows = rows;
    output.drawn.tool_groups = groups;
    output.drawn.tool_callouts = callouts;
    output.drawn.tool_header = Some(header);
    output.drawn.tool_columns = columns.to_vec();
    output.drawn.tool_preview_caption = caption;
    output.drawn.tool_computed = computed;
    // `Export…` and `Import…` are answered by the host between two frames, so their write is reported here,
    // where a write the modal itself made this frame reports itself too.
    output.drawn.tool_saved = std::mem::take(&mut self.tool_saved_pending);
    if let Some((rect, width)) = drawing {
      output.drawn.tool_drawing = Some(crate::chrome::rect_report(rect));
      output.drawn.tool_width_px = width;
    }
    if cancel {
      self.close_tool_library();
    }
    if let Some(tool) = commit {
      self.commit_tool_draft(tool, output);
    }
    // Applied last, after the frame's own drafts are written back, so an action's own selection and draft
    // are the ones the next frame seeds itself from rather than the ones this frame started with.
    if let Some(request) = request {
      match request {
        RowRequest::Library(request) => self.apply_tool_library_request(request, output),
        RowRequest::Export => output.actions.push(UiAction::RequestExportTools),
        RowRequest::Import => output.actions.push(UiAction::RequestImportTools),
      }
    }
    output.drawn.tool_selected = self.tool_library_selected.clone();
    output.drawn.tool_error = self.tool_error.clone();
  }

  /// Write the draft's tool, then do what the commit means.
  ///
  /// Three writes, in this order, and the order is the point: the library file first, because a tool the file
  /// refused must not be described as one the library holds; then the window's snapshot, by geometry id, and
  /// left alone when the row is not in it — the draft was seeded from a row, so "not there" means the library
  /// was replaced between the seed and the press, and a tool this window no longer lists is not one it should
  /// invent; then the number, which is the only document edit this modal can make.
  ///
  /// A refusal from the library stays in the modal ([`Shell::tool_error`]) and stops the other two: closing on
  /// a failure would lose the boxes the operator was editing, the rule the import modal already follows.
  fn commit_tool_draft(&mut self, tool: StoredTool, output: &mut ShellOutput) {
    let id = tool.geometry.id.clone();
    if let Some(path) = self.tool_library_path.clone() {
      let written = ToolLibrary::open(&path)
        .map_err(|error| error.to_string())
        .and_then(|library| library.save_tool(&tool).map_err(|error| error.to_string()));
      if let Err(sentence) = written {
        self.tool_error = Some(sentence);
        return;
      }
      output.drawn.tool_saved.push(id.clone());
    }
    if let Some(entry) = self.tools.iter_mut().find(|entry| entry.geometry.id == id) {
      *entry = tool.clone();
    }
    match tool.cutting.tool_number {
      // The draft's tool, not the document's: `Generate toolpath` is what writes it (S10).
      Some(number) if self.tool_library_for_draft => self.strategy.tool_number = Some(number),
      Some(number) => {
        if let Some(target) = self.tool_library_target.clone() {
          output.actions.push(UiAction::SetToolpathToolNumber {
            toolpath_id: target,
            tool_number: number,
          });
        }
      }
      None => {}
    }
    self.close_tool_library();
  }
}
