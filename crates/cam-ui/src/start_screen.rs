//! The start screen: the one modal the first frame shows (start-screen S2, S3).
//!
//! The window used to open on a demonstrator — a rectangle, a circle and a generated pocket preview playing in the
//! 3D half — which meant the first thing an operator saw was someone else's drawing, and every command they pressed
//! edited a document they never asked for. The start screen is the opposite statement: **the shell holds no project
//! until one is started or opened**, and this modal is where that choice is made.
//!
//! # Three answers, and only three
//!
//! The modal offers what S3 names and nothing else: **New project**, **Open project…**, and the projects opened last.
//! It adds no settings, no recent-file management and no "what's new": a start screen that grows into a launcher is a
//! second app in front of the first one. `New project` and `Open project…` emit the actions the shell already had
//! ([`UiAction::NewProject`], [`UiAction::RequestOpenFile`]) and a recent row emits [`UiAction::OpenFile`] with the
//! path it remembers, so this module decides *which* command and never what a command does.
//!
//! # The list belongs to the window, the time belongs to the host
//!
//! A [`RecentProject`] carries a name, a path and a label. The label is opaque text — `3 Nov 2026 18:04` — written by
//! whoever persists the list, because this shell reads exactly one clock ([`crate::Shell`]'s playback delta) and a
//! second one here would put a wall clock inside a pure frame. An entry the shell records while opening a project
//! therefore arrives with an empty label and the host stamps it when it writes the file; nothing is invented and
//! nothing in a frame needs `SystemTime`.
//!
//! # What the screen does not do by itself
//!
//! The list is state and the file is not: reading `recents.json` and writing it back is the host's, for the reason
//! the file dialogs are (shell-open-document S2). The shell reports that the list changed
//! ([`Shell::take_recents_changed`]) and that a person asked to read it again ([`Shell::take_recents_retry`]), and
//! stops there.

use std::path::PathBuf;

use egui::{Align, Layout, RichText, Sense, Stroke};

use crate::icons::{self, Icon};
use crate::theme::{self, Metrics, Palette, Role};
use crate::{Shell, ShellOutput, UiAction};

/// How many projects the start screen keeps.
///
/// Twelve fills the list without scrolling at every window height the frame promises (800 × 600 and up), which is the
/// only reason a number is here: a list longer than the modal is a list with a scrollbar nobody asked for.
pub const MAX_RECENTS: usize = 12;

/// The modal's floor, in dp.
///
/// Two columns beside each other read as a dialog only while there is a body under them: with three recent projects the
/// content measured under 240 and the frame collapsed into a strip with the two acts in its corner. The floor is the
/// side column's own stack — the wordmark, the `START` heading, the two acts and the two footer lines — with the air
/// the list wants under its last row, so a short list lengthens the dialog instead of flattening it.
///
/// Declared here rather than in [`crate::theme::Metrics`] because only this modal has a floor: every other surface of
/// the shell is sized by what it holds.
pub const MIN_HEIGHT: f32 = 360.0;

/// The modal's own words. Constants rather than string literals because a test asserting on a sentence the operator
/// reads should break when the sentence changes.
/// The side column's section heading.
pub const START: &str = "Start";
/// The action that creates an empty project.
pub const NEW_PROJECT: &str = "New project";
/// The sentence under the new-project action.
pub const NEW_PROJECT_SENTENCE: &str = "An empty stock, nothing drawn";
/// The action that opens a file dialog.
pub const OPEN_PROJECT: &str = "Open project…";
/// The sentence under the open-project action.
pub const OPEN_PROJECT_SENTENCE: &str = "Choose a .c2d file";
/// The heading above the list of projects opened before.
pub const RECENTS_TITLE: &str = "Recent projects";
/// The empty-list sentence on a first run.
pub const NO_RECENTS: &str = "No projects opened on this machine yet.";
/// The hint that follows the first-run sentence.
pub const NO_RECENTS_HINT: &str =
  "New project starts an empty stock. Open project… finds a .c2d file anywhere on disk.";
/// The sentence shown when the list could not be read.
pub const RECENTS_UNREADABLE: &str = "The recent list could not be read, so it is not shown.";
/// The hint that follows an unreadable-list sentence.
pub const RECENTS_UNREADABLE_HINT: &str =
  "Your projects are still on disk. Open project… finds one.";
/// The button that asks the host to read the list again.
pub const RETRY: &str = "Try again";
/// The footer line that names the file format.
pub const FOOTER_PATH: &str = "Projects are .c2d files on this machine.";
/// The footer line that names the Escape behaviour.
pub const FOOTER_ESCAPE: &str = "Esc starts with nothing open.";
/// The tooltip on the modal's close mark.
pub const CLOSE_HINT: &str = "Close and start with nothing open";

/// What the app bar says when no project is open, and the choice that reopens this modal.
pub const NO_PROJECT: &str = "No project open";
/// What the status bar's left zone says when no project is open.
pub const NOTHING_OPEN: &str = "Nothing open";
/// The status bar's right zone: the two commands that change the fact.
pub const NO_PROJECT_HINT: &str = "New project, or open a .c2d file";
/// The sentence a command gets when it needed a project and there was none.
///
/// It is a refusal and not a silence for the reason every other refusal in this shell is a sentence: a command that
/// does nothing and says nothing reads as a broken window.
pub const NO_PROJECT_REFUSAL: &str = "no project open: start a new project or open one first";
/// The canvas' empty state: what the drawing area holds until a project exists.
pub const NO_PROJECT_SENTENCE: &str =
  "The drawing area stays empty until a .c2d project is started or opened.";

/// One project the operator opened before (start-screen S3).
///
/// A name, a path and a label, and nothing else: no handle, no parsed document, no preview. Opening a row is
/// [`UiAction::OpenFile`] with this path, which is the same path the file dialog would have answered, so a project
/// opened from this list goes through the load, the refusal and the bookkeeping a project opened from disk goes
/// through — one entry to the document, not two.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecentProject {
  /// What the row says first: the file's stem, `Bracket v3`.
  pub name: String,
  /// The file it opens.
  pub path: PathBuf,
  /// When it was last opened, as the host wrote it. Empty until the host stamps it.
  pub label: String,
}

impl RecentProject {
  /// An entry from its three parts.
  pub fn new(name: impl Into<String>, path: impl Into<PathBuf>, label: impl Into<String>) -> Self {
    Self {
      name: name.into(),
      path: path.into(),
      label: label.into(),
    }
  }

  /// The entry a path implies: the file's stem as the name, and no label yet.
  ///
  /// A path with no stem — a name that is only an extension, or a root — keeps its whole text as the name, because a
  /// row with an empty first line would be a row a person cannot recognise.
  pub fn from_path(path: impl Into<PathBuf>) -> Self {
    let path = path.into();
    let name = path
      .file_stem()
      .map(|stem| stem.to_string_lossy().into_owned())
      .filter(|stem| !stem.is_empty())
      .unwrap_or_else(|| path.to_string_lossy().into_owned());
    Self {
      name,
      path,
      label: String::new(),
    }
  }

  /// The path as the row shows it: the whole path, because a start screen cannot guess which part matters.
  pub fn display_path(&self) -> String {
    self.path.to_string_lossy().into_owned()
  }
}

impl Shell {
  /// Whether a project is open at all, which every command that edits one asks first.
  pub fn project_is_open(&self) -> bool {
    self.project_open
  }

  /// Declare that this window holds no project, and show the start screen (start-screen S2, S3).
  ///
  /// This is the launch state, and it is the host's to say: the shell's default is the older answer — a drawing was
  /// handed to it, so a project is open — because the default is what every headless test and every `cam-cli` flow
  /// already assumes. The window calls this before its first frame, which is the one moment the difference matters.
  pub fn start_with_no_project(&mut self) {
    self.project_open = false;
    self.show_start = true;
  }

  /// Show the start screen without changing whether a project is open.
  ///
  /// The app bar's `No project open` is this: after `Esc` leaves the empty workspace, the modal is still one press
  /// away, and the list of recent projects is not stranded behind a launch that already happened.
  pub fn show_start_screen(&mut self) {
    self.show_start = true;
  }

  /// Whether the start screen is showing.
  pub fn start_screen_shown(&self) -> bool {
    self.show_start
  }

  /// The projects the start screen lists, most recent first.
  pub fn recents(&self) -> &[RecentProject] {
    &self.recents
  }

  /// Replace the list, as a host does after reading its own file.
  pub fn set_recents(&mut self, recents: Vec<RecentProject>) {
    let mut recents = recents;
    recents.truncate(MAX_RECENTS);
    self.recents = recents;
    self.recents_error = None;
    self.recents_changed = false;
  }

  /// Say that the list could not be read. `None` clears the sentence.
  pub fn set_recents_error(&mut self, sentence: Option<String>) {
    self.recents_error = sentence;
  }

  /// Take the flag that says the list changed and a host should write it.
  pub fn take_recents_changed(&mut self) -> bool {
    std::mem::take(&mut self.recents_changed)
  }

  /// Take the flag that says a person pressed `Try again` on an unreadable list.
  pub fn take_recents_retry(&mut self) -> bool {
    std::mem::take(&mut self.recents_retry)
  }

  /// Record a project the operator just opened or wrote, at the front of the list.
  ///
  /// By path and not by name: the same file opened twice is one row, and a file that moved is a new one — the only
  /// identity a list of paths can honestly offer.
  pub(crate) fn record_recent(&mut self, path: &std::path::Path) {
    let mut recents = std::mem::take(&mut self.recents);
    recents.retain(|entry| entry.path != path);
    recents.insert(0, RecentProject::from_path(path));
    recents.truncate(MAX_RECENTS);
    self.recents = recents;
    self.recents_changed = true;
  }

  /// The start screen itself: a side column of the two acts and the list of what was opened last.
  ///
  /// The layout is `design/start-screen/03-split-workbench.html`, mapped onto this shell's tokens: the modal is
  /// [`Role::Overlay`] with a [`Role::Border`] edge and `radius_dialog`, the side column is [`Role::Panel`] behind a
  /// hairline, the primary act carries the cutting signal ([`Role::CutSoft`] wash, [`Role::Cut`] edge), and a row
  /// answers the pointer with [`Role::SelSoft`] the way a selected row does everywhere else in the frame.
  pub(crate) fn start_screen(&mut self, ctx: &egui::Context, output: &mut ShellOutput) {
    let palette = self.palette();
    let metrics = self.metrics();
    let side_width = 232.0;
    let list_width = 456.0;
    let mut words: Vec<String> = Vec::new();
    let mut hits: Vec<(String, (f32, f32))> = Vec::new();
    let mut pressed: Option<UiAction> = None;
    let mut close = false;
    let mut retry = false;
    // Copied out of `self` so the closure can draw while the shell stays borrowed once: the modal writes nothing of
    // its own, and every decision it makes leaves as a flag or an action.
    let recents = self.recents.clone();
    let unreadable = self.recents_error.clone();
    let modal_id = egui::Id::unique("campilot-start");
    egui::Modal::new(modal_id)
      .frame(
        egui::Frame::NONE
          .fill(palette.get(Role::Overlay))
          .stroke(Stroke::new(metrics.hairline, palette.get(Role::Border)))
          .corner_radius(egui::CornerRadius::same(metrics.radius_dialog as u8))
          .inner_margin(egui::Margin::same(metrics.space(4) as i8)),
      )
      .show(ctx, |ui| {
        ui.set_min_height(MIN_HEIGHT);
        ui.horizontal_top(|ui| {
          // ---- the side column: what a person can start ----
          ui.vertical(|ui| {
            ui.set_width(side_width);
            ui.horizontal(|ui| {
              glyph(ui, &palette, &metrics, Icon::View3d, 20.0, Role::Sel);
              ui.label(
                RichText::new("campilot")
                  .size(theme::TITLE)
                  .color(palette.get(Role::TextHi)),
              );
            });
            words.push("campilot".to_owned());
            ui.add_space(metrics.space(4));

            heading(ui, &palette, START);
            words.push(START.to_owned());
            ui.add_space(metrics.space(1));

            // The two acts are separate decisions and not a list, so they get more air between them than one line and
            // its neighbour: 16 dp of `item_spacing` instead of the style's 8. The scope keeps that spacing off the
            // heading above and the footer below, which are read as one block with the column.
            ui.scope(|ui| {
              ui.spacing_mut().item_spacing.y = metrics.space(5);
              for (icon, title, sentence, primary) in [
                (Icon::New, NEW_PROJECT, NEW_PROJECT_SENTENCE, true),
                (Icon::Open, OPEN_PROJECT, OPEN_PROJECT_SENTENCE, false),
              ] {
                let response = action_tile(
                  ui,
                  &palette,
                  &metrics,
                  Tile {
                    icon,
                    title,
                    sentence,
                    primary,
                  },
                  side_width,
                );
                let centre = response.rect.center();
                hits.push((title.to_owned(), (centre.x, centre.y)));
                words.push(title.to_owned());
                words.push(sentence.to_owned());
                if response.clicked() {
                  pressed = Some(if primary {
                    UiAction::NewProject
                  } else {
                    UiAction::RequestOpenFile
                  });
                }
              }
            });

            ui.add_space(metrics.space(4));
            ui.label(
              RichText::new(FOOTER_PATH)
                .size(theme::SMALL)
                .color(palette.get(Role::TextDim)),
            );
            words.push(FOOTER_PATH.to_owned());
            ui.label(
              RichText::new(FOOTER_ESCAPE)
                .size(theme::SMALL)
                .color(palette.get(Role::TextDim)),
            );
            words.push(FOOTER_ESCAPE.to_owned());
          });

          ui.add_space(metrics.space(2));
          ui.separator();
          ui.add_space(metrics.space(2));

          // ---- the list: what was open before ----
          ui.vertical(|ui| {
            ui.set_width(list_width);
            ui.horizontal(|ui| {
              heading(ui, &palette, RECENTS_TITLE);
              words.push(RECENTS_TITLE.to_owned());
              ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let response = close_mark(ui, &palette, &metrics);
                if response.clicked() {
                  close = true;
                }
              });
            });
            ui.add_space(metrics.space(1));

            if let Some(sentence) = &unreadable {
              // The list could not be read, which is not the same answer as "nothing was opened yet" and must not be
              // drawn as one: a person with twelve projects and an unreadable file would be told they have none.
              ui.label(
                RichText::new(sentence)
                  .size(theme::BODY)
                  .color(palette.get(Role::Text)),
              );
              words.push(sentence.clone());
              ui.label(
                RichText::new(RECENTS_UNREADABLE_HINT)
                  .size(theme::SMALL)
                  .color(palette.get(Role::TextDim)),
              );
              words.push(RECENTS_UNREADABLE_HINT.to_owned());
              ui.add_space(metrics.space(1));
              let response = ui.button(RichText::new(RETRY).size(theme::SMALL));
              let centre = response.rect.center();
              hits.push((RETRY.to_owned(), (centre.x, centre.y)));
              words.push(RETRY.to_owned());
              retry = response.clicked();
            } else if recents.is_empty() {
              ui.label(
                RichText::new(NO_RECENTS)
                  .size(theme::BODY)
                  .color(palette.get(Role::Text)),
              );
              words.push(NO_RECENTS.to_owned());
              ui.label(
                RichText::new(NO_RECENTS_HINT)
                  .size(theme::SMALL)
                  .color(palette.get(Role::TextDim)),
              );
              words.push(NO_RECENTS_HINT.to_owned());
            } else {
              for entry in &recents {
                let response = recent_row(ui, &palette, &metrics, entry, list_width);
                let centre = response.rect.center();
                words.push(entry.name.clone());
                words.push(entry.display_path());
                hits.push((entry.display_path(), (centre.x, centre.y)));
                let action = UiAction::OpenFile {
                  path: entry.path.clone(),
                };
                if response.clicked() {
                  pressed = Some(action);
                }
              }
            }
          });
        });
      });

    output.drawn.start = words;
    output.drawn.start_hits = hits;
    if let Some(action) = pressed {
      output.actions.push(action);
    }
    if close {
      self.show_start = false;
    }
    if retry {
      self.recents_retry = true;
    }
  }
}

/// One of the two acts, as data: a glyph, a name, the sentence that says what pressing it gives, and whether it is the
/// primary one.
///
/// A struct rather than four more positional arguments to the tile painter: the two acts differ by two of these values
/// and agree on the rest, and a call with eight positional values is a call nobody can read at the site.
struct Tile<'a> {
  icon: Icon,
  title: &'a str,
  sentence: &'a str,
  primary: bool,
}

/// One of the two acts: a glyph, a name and the sentence that says what pressing it gives.
///
/// Drawn by allocating the rectangle first and painting into it rather than as a `Button`, for two reasons: a button in
/// this theme is one line of text and these are two, and the rectangle the pointer answers to has to be the rectangle
/// that was drawn. [`crate::chrome`]'s chip takes the same route for the second reason.
fn action_tile(
  ui: &mut egui::Ui,
  palette: &Palette,
  metrics: &Metrics,
  tile: Tile<'_>,
  width: f32,
) -> egui::Response {
  let Tile {
    icon,
    title,
    sentence,
    primary,
  } = tile;
  let fill = if primary { Role::CutSoft } else { Role::Panel };
  let edge = if primary { Role::Cut } else { Role::Border };
  // Two lines of text inside a 24 dp of padding either side: the act is a card a person aims at, and at 41 dp it read
  // as a row of the list beside it.
  let height = metrics.space(6) + theme::BODY + theme::SMALL;
  let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), Sense::click());
  let radius = egui::CornerRadius::same(metrics.radius_panel as u8);
  let painter = ui.painter();
  painter.rect(
    rect,
    radius,
    palette.get(fill),
    Stroke::new(metrics.hairline, palette.get(edge)),
    egui::StrokeKind::Inside,
  );
  if response.hovered() || response.has_focus() {
    // Hover and focus are the same 1 px step in the selection signal, drawn inside the tile so nothing moves: the
    // frame's own 2 px focus ring is the outer statement, and a tile that grew on hover would shift the two acts apart
    // while a person aims at them.
    painter.rect_stroke(
      rect,
      radius,
      Stroke::new(metrics.hairline, palette.get(Role::Sel)),
      egui::StrokeKind::Inside,
    );
  }
  let inner = metrics.space(2);
  let glyph_size = 18.0;
  let glyph_rect = egui::Rect::from_min_size(
    egui::pos2(rect.left() + inner, rect.center().y - glyph_size * 0.5),
    egui::vec2(glyph_size, glyph_size),
  );
  // The glyph carries the same signal as the edge, so the primary act is legible in a monochrome print too.
  icons::paint(
    painter,
    glyph_rect,
    icon,
    palette.get(if primary { Role::Cut } else { Role::Text }),
    metrics.icon_stroke * glyph_size / metrics.icon_size,
  );
  // The two lines hang off the tile's middle instead of a stack of measured heights: the name ends on the line above
  // the centre and the sentence starts on the line below it, so the pair stays centred when a sentence is longer.
  let text_left = glyph_rect.right() + metrics.space(2);
  let step = metrics.space(0);
  painter.text(
    egui::pos2(text_left, rect.center().y - step),
    egui::Align2::LEFT_BOTTOM,
    title,
    egui::FontId::proportional(theme::BODY),
    palette.get(Role::TextHi),
  );
  painter.text(
    egui::pos2(text_left, rect.center().y + step),
    egui::Align2::LEFT_TOP,
    sentence,
    egui::FontId::proportional(theme::SMALL),
    palette.get(Role::TextDim),
  );
  response
}

/// One project: its name, the path it lives at, and when it was last opened.
///
/// The same allocate-then-paint shape as [`action_tile`], and one row of a list instead of a card: the hairline under it
/// is the boundary, so the whole row — and not a button inside it — is what a press lands on.
fn recent_row(
  ui: &mut egui::Ui,
  palette: &Palette,
  metrics: &Metrics,
  entry: &RecentProject,
  width: f32,
) -> egui::Response {
  let height = metrics.space(5) + theme::BODY + theme::SMALL;
  let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), Sense::click());
  let painter = ui.painter();
  if response.hovered() || response.has_focus() {
    painter.rect_filled(
      rect,
      egui::CornerRadius::same(metrics.radius_control as u8),
      palette.get(Role::SelSoft),
    );
  }
  let inner = metrics.space(2);
  let text_left = rect.left() + inner;
  let step = metrics.space(0);
  painter.text(
    egui::pos2(text_left, rect.center().y - step),
    egui::Align2::LEFT_BOTTOM,
    &entry.name,
    egui::FontId::proportional(theme::BODY),
    palette.get(Role::TextHi),
  );
  painter.text(
    egui::pos2(text_left, rect.center().y + step),
    egui::Align2::LEFT_TOP,
    entry.display_path(),
    egui::FontId::monospace(theme::SMALL),
    palette.get(Role::TextDim),
  );
  // The stamp is right-aligned and is the one column a list of paths can line up, so it carries the row's only tabular
  // figure: a person scanning for "the one from yesterday" reads one column and not twelve sentences.
  painter.text(
    egui::pos2(rect.right() - inner, rect.center().y),
    egui::Align2::RIGHT_CENTER,
    &entry.label,
    egui::FontId::proportional(theme::SMALL),
    palette.get(Role::TextDim),
  );
  painter.hline(
    rect.x_range(),
    rect.max.y,
    Stroke::new(metrics.hairline, palette.get(Role::Hairline)),
  );
  response
}

/// A section heading: the word in the caption style, in the ink this system reserves for headings.
fn heading(ui: &mut egui::Ui, palette: &Palette, text: &str) {
  ui.label(
    RichText::new(text.to_uppercase())
      .size(theme::CAPTION)
      .color(palette.get(Role::TextOff)),
  );
}

/// A glyph from the icon set, in one role and at one size (the box is always 24).
fn glyph(
  ui: &mut egui::Ui,
  palette: &Palette,
  metrics: &Metrics,
  icon: Icon,
  size: f32,
  role: Role,
) {
  let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), Sense::hover());
  icons::paint(
    ui.painter(),
    rect,
    icon,
    palette.get(role),
    metrics.icon_stroke * size / metrics.icon_size,
  );
}

/// The list's close mark: two strokes, drawn rather than typed, because a font glyph for "close" is a promise about a
/// font this system does not fetch.
fn close_mark(ui: &mut egui::Ui, palette: &Palette, metrics: &Metrics) -> egui::Response {
  let (rect, response) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), Sense::click());
  let response = response.on_hover_text(CLOSE_HINT);
  let centre = rect.center();
  let reach = 5.0;
  let ink = if response.hovered() {
    palette.get(Role::Text)
  } else {
    palette.get(Role::TextDim)
  };
  let stroke = Stroke::new(metrics.icon_stroke, ink);
  ui.painter().line_segment(
    [
      egui::pos2(centre.x - reach, centre.y - reach),
      egui::pos2(centre.x + reach, centre.y + reach),
    ],
    stroke,
  );
  ui.painter().line_segment(
    [
      egui::pos2(centre.x + reach, centre.y - reach),
      egui::pos2(centre.x - reach, centre.y + reach),
    ],
    stroke,
  );
  response
}

/// The drawing area's empty state: the two lines `design/start-screen/03-split-workbench.html` puts where a stock
/// would be.
///
/// Painted by the canvas rather than by this module because the canvas is what owns the rectangle; the words live here
/// so the sentence a person reads and the sentence a test asserts on are one constant.
pub(crate) fn paint_no_project(
  painter: &egui::Painter,
  rect: egui::Rect,
  palette: &Palette,
  metrics: &Metrics,
) {
  if !rect.is_positive() {
    return;
  }
  let title = painter.layout_no_wrap(
    NO_PROJECT.to_owned(),
    egui::FontId::proportional(theme::TITLE),
    palette.get(Role::TextDim),
  );
  let sentence = painter.layout_no_wrap(
    NO_PROJECT_SENTENCE.to_owned(),
    egui::FontId::proportional(theme::SMALL),
    palette.get(Role::TextOff),
  );
  let gap = metrics.space(2);
  let height = title.size().y + gap + sentence.size().y;
  let top = rect.center().y - height * 0.5;
  let title_at = egui::pos2(rect.center().x - title.size().x * 0.5, top);
  let sentence_at = egui::pos2(
    rect.center().x - sentence.size().x * 0.5,
    top + title.size().y + gap,
  );
  painter.galley(title_at, title, palette.get(Role::TextDim));
  painter.galley(sentence_at, sentence, palette.get(Role::TextOff));
}
