//! The scripting panel: one script path, one `Run`, one output area (M10 T4b, spec S4).
//!
//! S4 answered "¿M10 toca la UI?" with **"CLI más panel mínimo"**, and this module is the whole of the
//! "panel mínimo" part: it is not an editor, not a console and not a debugging surface. It runs **one
//! document's script** and shows what happened — the report the script wrote, or the located
//! `line:column` diagnostic that stopped it.
//!
//! # The adapter boundary
//!
//! [`ScriptPanel::run`](crate::script_panel::ScriptPanel::run) touches no egui at all: it reads the script,
//! runs it against the document the caller hands it through `cam_cli::script::run_script_on` and keeps the
//! text. [`ScriptPanel::ui`](crate::script_panel::ScriptPanel::ui) is the thin egui surface on top, and it is
//! the only function in this file that needs a frame. A caller without egui — the binary, a test, or a
//! different shell — can therefore run the same script through the same engine and read
//! [`ScriptPanel::output`](crate::script_panel::ScriptPanel::output) and
//! [`ScriptPanel::error`](crate::script_panel::ScriptPanel::error) without ever building a widget.
//!
//! Three rules decide what this module is allowed to be:
//!
//! 1. **No process spawn.** The panel calls `cam_cli` as a library. It never runs the `cam-cli` binary,
//!    so there is no path handling, no environment and no second document on the other side of a pipe
//!    (M10 invariant 1, D-M10-1).
//! 2. **Synchronous, deliberately.** [`ScriptPanel::run`](crate::script_panel::ScriptPanel::run) reads and
//!    runs the script on the frame that pressed `Run`. That blocks the painter for the length of one script
//!    over one document: no
//!    geometry is generated and nothing is emitted, and every command is a document edit the engine
//!    already performs in a test's microseconds. A worker thread would buy a queue, a progress state and
//!    a race for a span a frame cannot show, so the cost is paid where it is understood.
//! 3. **Errors are shown, not swallowed.** A failed run keeps the last output visible — a failure never
//!    clears what the previous run produced — and paints the engine's own located diagnostic beside it,
//!    `line {line}:{column}: {message}`, which is the shape every scripted refusal already has.
//!
//! # One output area, two streams
//!
//! The engine separates a command's **reports** (the cut list, `nest: …`, `valid: …`) from its
//! **notices** (`save`'s `wrote …` line), because the binary sends the first to stdout and the second to
//! stderr. The panel has one area, so it hands the engine two `Capture` handles onto one buffer: what
//! the operator reads is the whole run, in the order the script wrote it.

use std::cell::RefCell;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use cam_model::Document;
use egui::{RichText, Ui};

use crate::theme::{self, Palette, Role, Theme};

/// How wide the script field is, so a long path scrolls inside the field instead of pushing `Run` off
/// the panel.
pub const PATH_FIELD_WIDTH: f32 = 380.0;

/// The most height the output area may take, in points.
///
/// The panel has to live inside something, and an area that grows with its text cannot: a script that
/// prints for hundreds of lines would push a window past any screen. The body scrolls instead, inside
/// this ceiling.
pub const BODY_HEIGHT: f32 = 240.0;

/// The sentence above the field.
pub const PATH_LABEL: &str = "Script file";

/// The word on the one button this panel has.
pub const RUN: &str = "Run";

/// The panel's title.
pub const TITLE: &str = "Script";

/// One `Write` two hands can hold: the buffer behind both of a run's streams.
///
/// `run_script_to` takes two writers and the panel shows one area, so the report sink and the notice
/// sink are two handles onto this buffer. Cloning a `Capture` does not copy the text — it points at the
/// same buffer — which is what keeps the two streams interleaved in the order the script wrote them.
#[derive(Clone, Default)]
struct Capture(Rc<RefCell<Vec<u8>>>);

impl Capture {
  /// Everything written through every handle so far.
  fn text(&self) -> String {
    String::from_utf8_lossy(&self.0.borrow()).into_owned()
  }
}

impl Write for Capture {
  fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
    self.0.borrow_mut().extend_from_slice(bytes);
    Ok(bytes.len())
  }

  fn flush(&mut self) -> io::Result<()> {
    Ok(())
  }
}

/// The scripting panel's state: which script, what the last run produced, and where the frame drew
/// `Run`.
///
/// The state is plain data on purpose: a test, or a caller with no window, sets the path, calls
/// [`ScriptPanel::run`] and reads the two answers, and only then decides whether painting it is worth a
/// frame.
#[derive(Clone, Debug)]
pub struct ScriptPanel {
  /// The path as typed, kept verbatim so the field shows what the operator wrote.
  path: String,
  /// The text the last run captured: reports and notices, in the script's own order.
  output: String,
  /// The located diagnostic of the last run, while it failed.
  error: Option<String>,
  /// Where this frame drew the `Run` button: the point that presses it.
  run_button: Option<(f32, f32)>,
  /// The size the output area took this frame, in points.
  ///
  /// Reported for the reason [`Self::run_button`] is: a surface that has to live inside a window needs
  /// to know how tall its body grew, and a headless test cannot measure a widget any other way.
  body_size: Option<(f32, f32)>,
  /// The set the panel paints its words in.
  theme: Theme,
}

impl ScriptPanel {
  /// An empty panel, in the shell's dark set, with no script named yet.
  pub fn new() -> Self {
    Self {
      path: String::new(),
      output: String::new(),
      error: None,
      run_button: None,
      body_size: None,
      theme: Theme::Dark,
    }
  }

  /// The same panel with `path` already in the field, so a caller that knows its script need not type
  /// it.
  pub fn with_path(path: impl Into<String>) -> Self {
    Self {
      path: path.into(),
      ..Self::new()
    }
  }

  /// The path the field holds.
  pub fn path(&self) -> &str {
    &self.path
  }

  /// Put `path` in the field. Returns nothing: typing a path does not run it.
  pub fn set_path(&mut self, path: impl Into<String>) {
    self.path = path.into();
  }

  /// The text the last run captured, or empty while no run has produced any.
  pub fn output(&self) -> &str {
    &self.output
  }

  /// The located diagnostic of the last run, or `None` while it succeeded.
  pub fn error(&self) -> Option<&str> {
    self.error.as_deref()
  }

  /// Where the last frame drew `Run`: the point that presses it.
  ///
  /// A headless test cannot ask egui for a widget by its word, so the panel reports the point it drew,
  /// the way the palette and the status bar report theirs.
  pub fn run_button(&self) -> Option<(f32, f32)> {
    self.run_button
  }

  /// The size the output area took in the last frame, or `None` while no frame has run.
  pub fn body_size(&self) -> Option<(f32, f32)> {
    self.body_size
  }

  /// Run the named script against `document`, which the caller owns.
  ///
  /// Deliberately synchronous (see this module's rules): the engine mutates the document in memory, so
  /// the work one script costs is bounded by the document, not by a cut. `document_path` is the path
  /// that document was loaded from, or `None` while it is untitled: the scripted commands that resolve a
  /// file beside the document need it, exactly as they do from a command line. The whole run is one
  /// transaction — a refusal leaves `document` as it was — and **returns whether it succeeded**, which
  /// is what tells a caller to keep the document it passed in.
  ///
  /// A failure leaves [`Self::output`] exactly as the previous run left it — a failed run says why
  /// **beside** the last result instead of erasing it — and stores the engine's own
  /// `line {line}:{column}: {message}` diagnostic.
  pub fn run(&mut self, document: &mut Document, document_path: Option<&Path>) -> bool {
    let path = PathBuf::from(self.path.trim());
    let capture = Capture::default();
    let mut reports = capture.clone();
    let mut notices = capture.clone();
    let result =
      cam_cli::script::run_script_on(&path, document, document_path, &mut reports, &mut notices);

    // The **result** decides, never whether the buffer happens to be empty. A script that succeeds
    // without printing anything has still answered, so its empty report replaces the area — keying off
    // emptiness would leave the previous run's text on screen with the error cleared, which reads as
    // "this is what just ran". A run that failed keeps the previous result visible and shows the
    // diagnostic beside it, even when it managed to print something before failing.
    match result {
      Ok(()) => {
        self.output = capture.text();
        self.error = None;
        true
      }
      Err(failure) => {
        self.error = Some(failure.to_string());
        false
      }
    }
  }

  /// Paint the panel: the title, the path field, `Run`, and the output area under them.
  ///
  /// The script runs **inside** the frame that pressed the button and before the output area is
  /// painted, so the press is answered on the frame that made it rather than a frame later. `document`
  /// is the document the caller owns and never changes: the run mutates a copy, and the copy comes back
  /// as this call's return value when — and only when — every line was accepted, which is how the
  /// caller learns to keep it.
  pub fn ui(
    &mut self,
    ui: &mut Ui,
    document: &Document,
    document_path: Option<&Path>,
  ) -> Option<Document> {
    let palette = self.theme.palette();
    ui.heading(
      RichText::new(TITLE)
        .size(theme::TITLE)
        .color(palette.get(Role::TextHi)),
    );
    ui.separator();

    let mut run = false;
    let mut committed = None;
    ui.horizontal(|ui| {
      ui.label(
        RichText::new(PATH_LABEL)
          .size(theme::CAPTION)
          .color(palette.get(Role::TextDim)),
      );
      ui.add(
        egui::TextEdit::singleline(&mut self.path)
          .hint_text("the script to run")
          .desired_width(PATH_FIELD_WIDTH),
      );
      let button = ui.add(egui::Button::new(
        RichText::new(RUN)
          .size(theme::BODY)
          .color(palette.get(Role::Cut)),
      ));
      let centre = button.rect.center();
      self.run_button = Some((centre.x, centre.y));
      run = button.clicked();
    });
    if run {
      // The run mutates a copy: the caller's document is replaced only when the run says every line was
      // accepted, so a refusal cannot leave half a script in the shell's document.
      let mut candidate = document.clone();
      if self.run(&mut candidate, document_path) {
        committed = Some(candidate);
      }
    }

    ui.separator();
    self.body_size = None;
    // Bounded and scrollable, because the panel is drawn inside a window now: an area that grows with
    // its own text would push that window past the screen. The rect reported is the viewport's, so a
    // test can say the body stayed inside `BODY_HEIGHT` instead of trusting the call.
    let body = egui::ScrollArea::vertical()
      .max_height(BODY_HEIGHT)
      .auto_shrink([false, false])
      .show(ui, |ui| self.paint_text(ui, &palette));
    self.body_size = Some((body.inner_rect.width(), body.inner_rect.height()));
    committed
  }

  /// The output area: the failure first, then what the run captured, one line per row.
  ///
  /// Line by line rather than as one label, because a report is a list of lines and a test — or an
  /// operator — reads one of them at a time.
  fn paint_text(&self, ui: &mut Ui, palette: &Palette) {
    if let Some(error) = &self.error {
      for line in error.lines() {
        ui.label(
          RichText::new(line)
            .size(theme::BODY)
            .color(palette.get(Role::Danger)),
        );
      }
    }
    for line in self.output.lines() {
      ui.label(
        RichText::new(line)
          .size(theme::SMALL)
          .color(palette.get(Role::Text))
          .monospace(),
      );
    }
  }
}

impl Default for ScriptPanel {
  fn default() -> Self {
    Self::new()
  }
}
