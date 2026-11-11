use cam_app::{ScreenPoint, Viewport};
use egui::Pos2;

use crate::{Rulers, UiAction};

/// Where the drawing area ended up on screen, in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasRect {
  /// Left edge, in pixels from the window's left.
  pub x: f64,
  /// Top edge, in pixels from the window's top.
  pub y: f64,
  /// Width in pixels.
  pub width: f64,
  /// Height in pixels.
  pub height: f64,
}

impl CanvasRect {
  /// The camera viewport for this rectangle.
  pub fn viewport(&self) -> Viewport {
    Viewport::new(self.width, self.height)
  }

  /// Convert a window position into camera-local screen space.
  pub fn to_screen(&self, position: Pos2) -> ScreenPoint {
    ScreenPoint::new(
      f64::from(position.x) - self.x,
      f64::from(position.y) - self.y,
    )
  }

  /// Convert camera-local screen space back into a window position.
  pub fn to_window(&self, point: ScreenPoint) -> Pos2 {
    Pos2::new((point.x + self.x) as f32, (point.y + self.y) as f32)
  }
}

/// What the shell displayed, reported so a headless test can assert on content.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DrawnState {
  /// Layer id, entity count and visibility, in the order the dock showed them.
  pub layer_rows: Vec<(String, usize, bool)>,
  /// Whether the layers block was expanded this frame.
  ///
  /// Reported because a folded block draws no row: without this a test could not tell "no layers" from "the
  /// operator folded the block", which are different answers (camui-prancheta S6).
  pub layers_open: bool,
  /// Where the layers block drew its heading: the point that folds it.
  ///
  /// A heading is a control and reports itself the way the rail's headings do (camui-prancheta S4).
  pub layers_heading: Option<(f32, f32)>,
  /// The ribbon's categories, in the order it drew them: glyph-only rows, so these are the words the task
  /// editor will carry once one is opened.
  pub categories: Vec<String>,
  /// Where the ribbon drew each category: its word and the point in the window that presses it.
  pub category_hits: Vec<(String, (f32, f32))>,
  /// The task editor's header while one is open, or empty while the panel is idle.
  pub task_editor: String,
  /// Where the task editor drew `✓` and `✕`: the words `Done` and `Cancel`, and the point that presses them.
  ///
  /// The two are painted glyphs, so the only word a headless test can read is the one this report gives them —
  /// the same reason the app bar used to report its own.
  pub task_editor_hits: Vec<(String, (f32, f32))>,
  /// Lines the property panel displayed.
  pub property_lines: Vec<String>,
  /// The status bar text.
  pub status_line: String,
  /// Scene items painted on the canvas.
  pub painted_items: usize,
  /// Grid lines painted on the canvas.
  pub painted_grid_lines: usize,
  /// Triangles the 3D panel painted, `0` when the panel is closed.
  ///
  /// A triangle counts once it reached the `egui::Mesh`, so a triangle this backend refuses — one
  /// with a vertex behind the camera — is not counted (M5 A9, A11).
  pub painted_triangles: usize,
  /// Every 3D edge the half painted: the sum of the four layer counts beside this one.
  ///
  /// Kept as one number because most questions about the picture are about it as a whole, and the four per-layer
  /// counts ([`Self::painted_material_lines`], [`Self::painted_vector_lines`], [`Self::painted_program_lines`],
  /// [`Self::painted_tool_lines`]) exist because the overlay row hides them one at a time (camui-prancheta S8):
  /// a single total could not say which layer went missing.
  pub painted_3d_lines: usize,
  /// The sentence the status bar paints at its right end: what the last action produced.
  ///
  /// Reported apart from [`Self::status_line`] because the right end is a zone of its own (camui-prancheta
  /// S7): a test that read the whole line could not tell the outcome from the coordinates beside it.
  pub message: String,
  /// Where the status bar drew each snap chip: its word and the point in the window that presses it.
  ///
  /// A chip both arms its kind and, for the grid, names the step it snaps to (camui-prancheta S7), so a test
  /// must press the chip the bar actually drew instead of guessing at the layout — the same reason the
  /// palette reports its own hits.
  pub status_chips: Vec<(String, (f32, f32))>,
  /// The sentence the status bar showed in front of the message: what the current gesture will do.
  pub hint: String,
  /// What the two rulers along the canvas' top and left edges drew (camui-prancheta S5).
  ///
  /// A report and not a picture, for the same reason the palette reports its chips: the bands, the numbers
  /// they read off the camera and the amber marker that tracks the pointer are the only part of S5 a test
  /// cannot see from [`ShellOutput::canvas`], which says where the drawing starts but nothing about what
  /// frames it.
  pub rulers: Rulers,
  /// What the left palette offered, in the order it drew them.
  ///
  /// The palette is the part of the frame whose content changes with the stage, and a headless test cannot
  /// read an icon: this is how "switching stage really changed what the operator can reach" is asserted.
  pub palette: Vec<String>,
  /// Where the palette drew each chip: its word and the point in the window that presses it.
  ///
  /// egui cannot be asked for a widget by its text from outside a frame, so the palette reports the
  /// point it drew each chip at and a click test presses that point instead of guessing at the layout.
  pub palette_hits: Vec<(String, (f32, f32))>,
  /// Where the palette drew each group's heading: its word and the point in the window that folds it.
  ///
  /// A heading is a control now (camui-prancheta S4), so it reports itself the way the chips do: a test
  /// clicks `FILE` and asserts FILE's chips are gone, instead of reaching into the shell's own field to set
  /// a fold the operator would have set by clicking.
  pub palette_headings: Vec<(String, (f32, f32))>,
  /// Where the Tools tab drew each parameterised command's button: its word and the point in the window
  /// that presses it.
  ///
  /// The same surface the palette reports ([`Self::palette_hits`]) and for the same reason, for the one
  /// question only it can answer: the tab and the palette run the same seven payloads, and a test that could
  /// press only one of them could not prove they agree. Only those seven are listed — the tab's other
  /// buttons are not built from [`crate::params`], so nothing needs their points.
  pub authoring_hits: Vec<(String, (f32, f32))>,
  /// The words the app bar's command buttons answer to, in the order it drew them.
  ///
  /// Empty since camui-prancheta T2b, and kept as an empty report on purpose: the bar's file commands moved to
  /// the rail, and a test asserts this list is empty so a button cannot quietly come back. [`Self::app_bar_hits`]
  /// is the same surface for the buttons that remain, which answer to their own labels.
  pub app_bar: Vec<String>,
  /// Where the app bar drew each command button: its word and the point in the window that presses it.
  ///
  /// The same surface the palette reports ([`Self::palette_hits`]) and for the same reason: egui offers no
  /// widget-by-text lookup from outside a frame, so a test that has to press one of the bar's glyphs is
  /// given the point the button was drawn at instead of guessing at the bar's margins.
  pub app_bar_hits: Vec<(String, (f32, f32))>,
  /// What the `F1` help panel drew, in the order it drew it: each group's label, then that group's
  /// command rows, and finally the focused command's `what` and `how`.
  ///
  /// Empty while the panel is closed, because a panel nobody opened drew no row. A headless test reads
  /// this to prove that every command in the table is listed, which is the one claim a screenshot of a
  /// long list of labels cannot make.
  pub help: Vec<String>,
  /// The words the canvas context menu drew, in the order it drew them.
  ///
  /// Empty while no menu is open, because a popup nobody opened drew no row. The menu is the one part of
  /// the frame that only exists while the secondary button has been pressed, so this is how a headless test
  /// sees it at all — and its entries are [`crate::help::Command`] labels, which is what makes "every entry
  /// can be explained" checkable rather than asserted.
  pub menu: Vec<String>,
  /// Where the context menu drew each entry: its word and the point in the window that presses it.
  ///
  /// The same surface the palette reports ([`Self::palette_hits`]) and for the same reason: egui offers no
  /// widget-by-text lookup from outside a frame, so a test that has to press a menu row is given the point
  /// the row was drawn at instead of guessing at the popup's margins.
  pub menu_hits: Vec<(String, (f32, f32))>,
  /// The words the material dialog drew, in the order it drew them: the title, the two cards with their
  /// subtitles, the three measurement labels with their values, the plan's annotations, the caption under
  /// the plan, and the footer.
  ///
  /// Empty while the dialog is closed, because a dialog nobody opened drew nothing. This is how a headless
  /// test reads the one screen in the shell whose content is a picture: the annotated plan is painted with
  /// a painter, and its three dimension labels are the only part of it a test can quote back.
  pub material: Vec<String>,
  /// The words the start screen painted, while it was open (start-screen S3).
  ///
  /// Reported because the modal's contract is what a person reads: the two acts, the two sentences that say what they
  /// give, and whichever of the three list states (returning, first run, unreadable) this frame was in.
  pub start: Vec<String>,
  /// Where the start screen drew each choice: a title, or a recent project's path, and the point that presses it.
  ///
  /// A row is keyed by its path and not by its name, because two projects may share a stem in two folders and the
  /// press has to say which file it opened. Read it from a frame the modal has settled in: a floating, auto-sized area
  /// is measured on the frame it first appears, so the first frame's points name rectangles it has already left (the
  /// material dialog has the same first frame, and the rule is the frame's, not this modal's).
  pub start_hits: Vec<(String, (f32, f32))>,
  /// Where the material path drew each pressable thing: the Job tab's opener while the dialog is closed,
  /// then the dialog's two cards and its two footer buttons while it is open.
  ///
  /// The same surface the palette reports ([`Self::palette_hits`]) and for the same reason: egui offers no
  /// widget-by-text lookup from outside a frame, so a test that has to press a card is given the point the
  /// card was drawn at instead of guessing at the dialog's margins.
  pub material_hits: Vec<(String, (f32, f32))>,
  /// The sentence the material dialog is showing because one of its numbers cannot describe a stock.
  ///
  /// Empty while the numbers are usable and while the dialog is closed. The dialog refuses before it pushes
  /// anything, so a non-empty answer here is also the reason [`ShellOutput::actions`] holds no
  /// [`UiAction::SetJob`] for that frame — and [`cam_app::Command::SetJob`] refuses the same three numbers
  /// by the model's own rule if anything else ever does.
  pub material_refusal: String,
  /// The words the current-tool card drew, in the order it drew them: the section naming the toolpath it
  /// belongs to, then either the tool's two lines — number and name, then type, diameter, angle and flute
  /// count — or the one sentence saying the number is not in the library this window was given.
  ///
  /// Empty while the frame is not on the Toolpaths stage, and empty while the document holds no toolpath:
  /// a card for a toolpath that does not exist would be describing nothing. This is how a headless test
  /// reads the card, for the same reason [`Self::material`] exists — egui offers no widget-by-text lookup
  /// from outside a frame — and it is the only place that pairs a document's `tool_number` with the
  /// library snapshot (camui-toolpath S2).
  pub tool_card: Vec<String>,
  /// How much of the previewed program has played, in `0.0..=1.0`.
  ///
  /// The value the clock itself reports: exactly `0.0` while the preview stands on its first
  /// move and exactly `1.0` once the program has finished (M5 A10). Stays `0.0` when the panel is
  /// closed, because a closed panel is not a clock. Read it as "the program is played out", not as
  /// "the bar is full": a program whose moves all take zero time reports `1.0` here while
  /// [`Self::preview_bar`] stays `0.0`, because there is no time for a bar to fill with.
  pub preview_progress: f64,
  /// How full the 3D panel's progress bar was painted, in `0.0..=1.0`.
  ///
  /// The same number the bar's own `egui::ProgressBar` is given, which is not always
  /// [`Self::preview_progress`]: a program with no total duration paints an empty bar however far it
  /// has played, so the readout never claims a completion the clock could not measure. Stays `0.0`
  /// when the panel is closed or holds no preview.
  pub preview_bar: f32,
  /// The two clocks the progress bar's readout states, as one sentence: `0:42 / 3:07`.
  ///
  /// Painted beside the bar and mirrored here because a headless test can read a string where it
  /// cannot read a painted clock: the formatter is unit-tested, and this field is what proves the
  /// panel writes its output rather than something of its own. Empty when the panel is closed or
  /// holds no preview, because a bar nobody drew has no clock.
  pub preview_clock: String,
  /// The words the `2D | 3D` toggle drew and the point in the window that presses each (camui-prancheta S8).
  ///
  /// Reported for the same reason the palette reports its chips: the toggle is painted, not a widget egui
  /// can be asked for by text from outside a frame, so a test presses the cell the frame really drew. A
  /// frame that drew no canvas reports nothing.
  pub view_toggle: Vec<(String, (f32, f32))>,
  /// The strip the `2D | 3D` toggle occupied, as `(x, y, width, height)` in window pixels.
  ///
  /// Separate from [`Self::view_toggle`] because S8 is a claim about *where* the toggle sits — the top right
  /// of the canvas — and a list of pressable points cannot say where the control as a whole was drawn.
  pub view_toggle_rect: Option<(f32, f32, f32, f32)>,
  /// The canvas' two halves while the 3D split is open: the drawing half, then the 3D half, each as
  /// `(x, y, width, height)` in window pixels (camui-prancheta S8).
  ///
  /// `None` while 3D is off, because a split that is not open has no halves. [`ShellOutput::canvas`] is the
  /// first of the two when the split is open: the rectangle the 2D camera is told about.
  pub split: Option<(PixelRect, PixelRect)>,
  /// The words the 3D half's overlay row drew and the point that presses each.
  ///
  /// Empty while the split is closed, and empty while it is open with no program loaded.
  pub layer_chips: Vec<(String, (f32, f32))>,
  /// Which of the four scene layers the overlay row is showing, in the row's own order (camui-prancheta S8).
  ///
  /// Reported beside the counts below so a test can tell "the layer is hidden" from "the layer had nothing
  /// to paint", which are different answers.
  pub scene_layers: Vec<(String, bool)>,
  /// Where the 3D half drew its home button, in window pixels (camui-prancheta S8).
  pub home_button: Option<(f32, f32)>,
  /// The axis triad's three labels and where each was drawn (camui-prancheta S8): `TOP`, `FRONT`, `RIGHT`.
  ///
  /// The words in the order painted, with the point each label sits at — which is what lets a test prove the
  /// triad is drawn from the camera and not pasted on: orbiting turns it.
  pub triad: Vec<(String, (f32, f32))>,
  /// The playback bar's controls and the point that presses each (camui-prancheta S8): `Play` or `Pause`, the
  /// speed's own word (`60x`), `To the end`, `Step` and `Reset`.
  pub playback_hits: Vec<(String, (f32, f32))>,
  /// The playback bar's readout, exactly as painted: `0:05 / 0:05  T22 · Drilling 1 · Z 5.000`.
  ///
  /// The clocks come from [`view3d::progress_text`](crate::view3d::progress_text) and the move from
  /// [`Preview3d::playhead_text`](crate::view3d::Preview3d::playhead_text), so this field is what proves the
  /// bar writes those two rather than a third opinion of its own.
  pub playback_readout: String,
  /// Where the playback bar drew its scrubber, as `(left, right, y)` in window pixels (camui-prancheta S8).
  ///
  /// The two ends and the row's middle, not the filled part: a test seeks by pressing a fraction of the
  /// way along the track the frame drew instead of guessing at the bar's margins.
  pub scrubber: Option<(f32, f32, f32)>,
  /// Stock-box edges the 3D half painted, `0` when `Material` is hidden or the split is closed.
  pub painted_material_lines: usize,
  /// The document's vectors, projected into the 3D half, `0` when `Vectors` is hidden or the split is closed.
  pub painted_vector_lines: usize,
  /// The program's moves, projected into the 3D half, `0` when `Toolpaths` is hidden or the split is closed.
  pub painted_program_lines: usize,
  /// The tool marker's edges, `0` when `Tool` is hidden, the split is closed, or no program is loaded.
  pub painted_tool_lines: usize,
  /// The modal's header title, exactly as the band across its top paints it.
  ///
  /// `Choose tool` while the tree holds no selection, and `Choose tool — T4 - Flat end mill · 6 mm` once a row
  /// is chosen, so a test reads the sentence the band really shows instead of rebuilding it from the selection.
  /// `None` while the modal is closed, because a band nobody drew has no title.
  pub tool_header: Option<String>,
  /// The three columns' widths, in the order they are drawn: master, detail, preview.
  ///
  /// The numbers the frame laid out with, so S2's three columns are a claim about the picture rather than about
  /// a constant a test would have to import: a column that moved is a difference here.
  pub tool_columns: Vec<f32>,
  /// The word under the preview card: the kind's own name (`Flat end mill`).
  ///
  /// `None` while nothing is drawn, because a caption under no card would be a label for nothing.
  pub tool_preview_caption: Option<String>,
  /// The rows the form draws as readings rather than boxes, in the order drawn: the row's word and its text.
  ///
  /// Empty while the modal is closed. These are the two numbers the operator cannot type — the stepover as a
  /// percentage and the chipload — and the chipload is the one S6 requires to be arithmetic over the rows above
  /// it, so this is how "computed, never typed" is checked rather than asserted.
  pub tool_computed: Vec<(String, String)>,
  /// The library action rows the master column drew, in the order drawn: `+ New`, `Duplicate`, `Delete`,
  /// `Export…`, `Import…`, `Restore defaults` (S3).
  ///
  /// Every row is named here whether or not it is pressable, because a row that cannot be honoured is still
  /// drawn and still teaches what a library file would enable; [`Self::tool_library_hits`] is what says which of
  /// them the frame made pressable. Empty while the modal is closed, because a modal nobody opened drew no row.
  pub tool_actions: Vec<String>,
  /// The geometry ids a library write recorded, in the order it wrote them.
  ///
  /// Empty when nothing was written — no library file is known, the press was refused, or the write failed — so a
  /// test can tell "the library was updated" from "the window's own snapshot was". The modal's own rows report
  /// the frame that wrote them; `Export…` and `Import…` are answered by the host between two frames, so their ids
  /// reach the next frame that draws the modal.
  pub tool_saved: Vec<String>,
  /// The sentence the modal is showing because reading or writing the library failed.
  ///
  /// `None` while there is nothing to say. A refusal — a tool the store would not write, a library file that
  /// could not be opened, a JSON file that is not a library — is drawn inside the modal beside the rows it is
  /// about, because closing on a failure would lose the tree the operator was working in.
  pub tool_error: Option<String>,
  /// The words the tool-library modal drew, in the order it drew them (camui-prancheta S9).
  ///
  /// Empty while the modal is closed, because a modal nobody opened drew nothing. The modal is the one
  /// surface in the shell whose content is a tree of the library and a form that changes with the selected
  /// tool, so this is how a headless test reads it: egui offers no widget-by-text lookup from outside a
  /// frame, and the tree's rows carry a painted name and a painted cut line rather than a label a test can
  /// ask for.
  pub tool_library: Vec<String>,
  /// Where the tool-library modal drew each pressable thing: its word and the point in the window that
  /// presses it (camui-prancheta S9).
  ///
  /// The same surface the palette reports ([`Self::palette_hits`]) and for the same reason. Only points the
  /// operator could actually press are reported: a row scrolled out of the master list is drawn by egui and
  /// clipped away, and reporting its centre would be a promise about a control nobody can see (the defect
  /// the T6 review found in the playback bar, L23).
  pub tool_library_hits: Vec<(String, (f32, f32))>,
  /// The title of the tool-library import question, when the modal is asking (tool-library-app S5).
  ///
  /// `None` while no question is open.
  pub tool_import_question_title: Option<String>,
  /// The sentence the tool-library import question asks with (tool-library-app S5).
  ///
  /// `None` while no question is open.
  pub tool_import_question_sentence: Option<String>,
  /// The buttons the tool-library import question drew, in the order drawn (tool-library-app S5).
  pub tool_import_question_buttons: Vec<String>,
  /// Where the tool-library import question drew each pressable button: its word and the point that presses it.
  pub tool_import_question_hits: Vec<(String, (f32, f32))>,
  /// The note the last answered tool-library import reported (tool-library-app S5).
  ///
  /// `None` until an import is answered in a way that reports a note.
  pub tool_import_note: Option<String>,
  /// The family tree the modal drew: each family's own name and the number of rows drawn under it.
  ///
  /// The count is the number of rows that follow the heading in the tree, not the number of tools the
  /// library holds in that family: a count beside a filtered tree that named tools the operator cannot see
  /// would be a number nobody can check against what is on screen.
  pub tool_families: Vec<(String, usize)>,
  /// One entry per tool row, in the order drawn: the family's name, the tool's title, and its
  /// `Ø6 · 2 cut(s)` line.
  ///
  /// The family is part of the row rather than implied by [`Self::tool_families`] because a tree is a
  /// claim about *which* rows sit together, and a list of counts cannot make it.
  pub tool_rows: Vec<(String, String, String)>,
  /// The tool the modal's detail side is showing, by the library's own geometry id.
  ///
  /// `None` while the modal is closed and while the tree holds no selection. An id and not a row title,
  /// because two tools may carry the same name and the same number, and the operator's choice is a tool.
  pub tool_selected: Option<String>,
  /// The detail form's groups, in the order drawn, each with the fields and values under it
  /// (camui-prancheta S9).
  ///
  /// Empty while the modal is closed. This is the report that makes "the form changes with the family"
  /// answerable: the group's word and its fields are exactly what the form drew, so a field a family does
  /// not have cannot be claimed by a test that only read the painted text.
  pub tool_groups: Vec<(String, Vec<(String, String)>)>,
  /// The dimension callouts painted on the scaled tool drawing, in the order drawn.
  ///
  /// The drawing is painted, so these are the only part of it a test can quote back — the same reason the
  /// material plan reports its three measurements.
  pub tool_callouts: Vec<String>,
  /// Where the scaled tool drawing went, as `(x, y, width, height)` in window pixels.
  ///
  /// `None` while the modal is closed, while nothing is selected, and when the selected tool carries no
  /// diameter: a tool with no width has no width to draw, and the form says so instead of drawing a shape
  /// of its own invention.
  pub tool_drawing: Option<(f32, f32, f32, f32)>,
  /// The width the tool's own diameter was drawn at, in pixels, `0.0` when nothing was drawn.
  ///
  /// Separate from [`Self::tool_drawing`] because "to scale" is a claim about the *number*: the same tool
  /// drawn wider is not the same reading, and a rectangle tells a test nothing about how many millimetres
  /// wide it is.
  pub tool_width_px: f32,
  /// The tree's visible tool rows as geometry: `(row, glyph, title, subtitle)` rectangles in window pixels.
  ///
  /// Reported so the row height, glyph slot and text positions can be pinned without reading the image.
  pub tool_tree_rows: Vec<(PixelRect, PixelRect, PixelRect, PixelRect)>,
  /// Whether the tool tree drew a scrollbar because the list overflowed.
  pub tool_tree_scrollbar: bool,
  /// The preview's dimensions: `(label, line, label box)` rectangles in window pixels, in drawn order.
  ///
  /// Reported so the rule S13 keeps can be asserted rather than admired: a label sits eight pixels clear of its
  /// own line, and touches neither the tool's body nor another label.
  pub tool_preview_dimensions: Vec<(String, PixelRect, PixelRect)>,
  /// Where the tool is drawn inside its card: its shank, its cutting section and its amber tip.
  pub tool_preview_body: Vec<PixelRect>,
  /// A cone's `(top width, tip width, height)` in pixels, when the selected kind draws one.
  pub tool_preview_cone: Option<(f32, f32, f32)>,
  /// The form's box column, per drawn row: `(group, label, left, right)` in window pixels.
  ///
  /// Reported so every row of every kind can be shown to share one box column.
  pub tool_form_box_edges: Vec<(String, String, f32, f32)>,
  /// Where the current-tool card drew `Choose another tool…`: its word and the point that presses it.
  ///
  /// Empty while no card is drawn. The card's own words stay in [`Self::tool_card`] and did not grow this
  /// button, because the card is a reading of the tool a toolpath cuts with and the button is the way to
  /// change it.
  pub tool_card_hits: Vec<(String, (f32, f32))>,
  /// The `TOOLPATHS` surface's list, in the order it drew it: the row's own words and whether it reads as the
  /// selected toolpath (M13 T3, S4).
  ///
  /// Empty while the `TOOLPATHS` task is closed and while the document holds no toolpath, because a list of
  /// nothing is not a list. The words are the row's, so a test reads back exactly what the operator sees.
  pub toolpath_rows: Vec<(String, bool)>,
  /// Where the list drew each row: the row's words and the point in the window that presses it.
  ///
  /// The same reason [`Self::layer_chips`] and [`Self::app_bar_hits`] report their points: egui cannot be
  /// asked for a widget by its text from outside a frame, and a row nobody can press is the `first()` the
  /// list replaced.
  pub toolpath_hits: Vec<(String, (f32, f32))>,
  /// The toolpath the shell is describing, by id: the selection when the document still holds it, and the
  /// document's first toolpath otherwise (M13 T3, S4).
  ///
  /// The **resolved** answer, not the raw selection, so the list, the card and the editor cannot describe two
  /// different toolpaths: a selection that names a toolpath the document no longer holds reads as the first
  /// one rather than as a card nobody can act on.
  pub toolpath_selected: Option<String>,
}

/// A rectangle in window pixels, as `(x, y, width, height)`.
///
/// The shape a drawn surface reports its own geometry in: `chrome::rect_report` turns an `egui::Rect` into one,
/// and [`DrawnState::split`] holds the canvas' two halves as a pair of them.
pub type PixelRect = (f32, f32, f32, f32);

/// What one frame produced.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShellOutput {
  /// Where the canvas was drawn.
  pub canvas: CanvasRect,
  /// Actions the user triggered this frame; the caller applies them through
  /// [`handle_action`](crate::handle_action).
  pub actions: Vec<UiAction>,
  /// What the panels displayed.
  pub drawn: DrawnState,
}

impl Default for CanvasRect {
  fn default() -> Self {
    Self {
      x: 0.0,
      y: 0.0,
      width: 0.0,
      height: 0.0,
    }
  }
}
