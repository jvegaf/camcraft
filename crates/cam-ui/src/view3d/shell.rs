use cam_sim::Heightfield;
use cam_units::Length;

use crate::theme::{self, Palette, Role};
use crate::Shell;

use super::chrome::{
  chip_size, paint_home, paint_layer_chip, paint_plain_chip, paint_scrubber, paint_transport,
};
use super::{
  bar_fill, clock_text, next_speed, progress_text, speed_text, vector_lines, Layer, Preview3d,
  SPEEDS,
};

/// How many lines the overlay row may wrap to before it stops.
///
/// Two, because that is what the reference frame uses: the four checkboxes and `Centre` sit on the first line and
/// the `?` drops to the second when the half is narrow. A row that wrapped for ever would walk down the picture.
const OVERLAY_ROWS: usize = 2;

/// How tall one line of the overlay row is, in points.
const OVERLAY_LINE_HEIGHT: f32 = 18.0;

/// What a simulation of a drawing with no toolpaths answers.
///
/// A preview is a program cutting a stock, and a drawing alone is geometry, so there is nothing to put in the
/// half until the document names a path. The refusal names the missing half and the control that produces it,
/// because M13 S3 exists precisely because this half used to point at a command in another window.
const NO_TOOLPATHS_TO_SIMULATE: &str =
  "this drawing has no toolpaths, so there is nothing to simulate — author one in the Toolpaths task first, \
   and press Simulate";

/// How tall the 3D half's playback bar is, in points: one row of controls and the readout's own line.
const PLAYBACK_BAR_HEIGHT: f32 = 46.0;

/// How wide the square at the 3D half's top right is: the home button above, the axis triad below.
const CORNER_WIDTH: f32 = 86.0;

/// How tall that square is.
const CORNER_HEIGHT: f32 = 68.0;

/// Where one frame puts the 3D half's chrome (camui-prancheta S8).
///
/// The picture is the half less the playback bar's strip, and two strips float over it: the overlay row at its
/// top left, and the square at its top right that holds the home button and the axis triad. The playback bar
/// is reserved rather than floated, because a bar lying over the model would swallow the drags that orbit it.
#[derive(Clone, Copy, Debug)]
struct Half {
  /// The picture: the region the scene is painted and clipped into, and the one the orbit and wheel own.
  picture: egui::Rect,
  /// The overlay row's strip, at the picture's top left.
  overlay: egui::Rect,
  /// The square at the picture's top right: the home button above, the axis triad below.
  corner: egui::Rect,
  /// The playback bar's strip, along the half's bottom.
  bar: egui::Rect,
}

/// Where the chrome sits in one 3D half.
fn half_layout(rect: egui::Rect, metrics: &theme::Metrics) -> Half {
  let pad = metrics.space(3);
  let bar = egui::Rect::from_min_max(
    egui::pos2(rect.min.x + pad, rect.max.y - PLAYBACK_BAR_HEIGHT),
    egui::pos2(rect.max.x - pad, rect.max.y),
  );
  let picture = egui::Rect::from_min_max(
    rect.min,
    egui::pos2(rect.max.x, (bar.min.y - pad).max(rect.min.y)),
  );
  // The row spans the half and may use two lines, and the corner sits *below* it: that is the reference's own
  // arrangement, and it is what keeps the six words of S8's row readable in a half that is only half a screen.
  let overlay = egui::Rect::from_min_size(
    egui::pos2(picture.min.x + pad, picture.min.y + pad),
    egui::vec2(
      (picture.width() - 2.0 * pad).max(0.0),
      OVERLAY_LINE_HEIGHT * OVERLAY_ROWS as f32 + metrics.space(1),
    ),
  );
  let corner = egui::Rect::from_min_size(
    egui::pos2(picture.max.x - CORNER_WIDTH - pad, overlay.max.y + pad),
    egui::vec2(CORNER_WIDTH, CORNER_HEIGHT),
  );
  Half {
    picture,
    overlay,
    corner,
    bar,
  }
}

/// What the overlay row has to paint once the picture is under it: each chip's word, where it goes, and
/// whether its box is ticked (camui-prancheta S8).
type RowSlots = Vec<(String, egui::Rect, bool)>;

/// What the playback bar has to paint once the picture is under it.
///
/// The same split as [`RowSlots`], and for the same reason: every control in the bar has to be registered
/// before the picture's gesture region (so a press on it is not a press that orbits) and painted after the
/// picture (so it is not painted over). What is left when both are done is this: where everything goes, and
/// the state the presses above left behind.
#[derive(Clone, Debug)]
struct BarPaint {
  /// The transport button's rectangle.
  transport: egui::Rect,
  /// Whether the program is playing *after* this frame's presses.
  playing: bool,
  /// The speed pill's rectangle and the word it names.
  speed: (egui::Rect, String),
  /// `To the end`, `Step` and `Reset`: their words and rectangles.
  plain: Vec<(String, egui::Rect)>,
  /// The scrubber's track, when the row had any width for it.
  scrubber: Option<egui::Rect>,
  /// How full the track's fill is, from the clock's own two numbers.
  fill: f32,
  /// Every pressable control and the point that presses it, in the order drawn.
  hits: Vec<(String, (f32, f32))>,
}

impl Default for BarPaint {
  fn default() -> Self {
    Self {
      // A bar nobody drew has no rectangle to paint in; `Rect::NOTHING` is egui's own way of saying so.
      transport: egui::Rect::NOTHING,
      playing: false,
      speed: (egui::Rect::NOTHING, String::new()),
      plain: Vec::new(),
      scrubber: None,
      fill: 0.0,
      hits: Vec::new(),
    }
  }
}

impl Shell {
  /// The 3D half of the split (camui-prancheta S8): the picture, its overlay row, the home button with the
  /// axis triad, and the playback bar.
  ///
  /// # The order of the four steps, and why it is that order
  ///
  /// 1. the picture's gesture region is allocated **first**, so every control below is registered after it and
  ///    a press on a chip is a press on chrome rather than one that orbits the stock;
  /// 2. the chrome is allocated and each press applied where it happens, so the frame that hides the tool is
  ///    the frame whose picture has no tool in it (L13);
  /// 3. the picture is painted, from the state those presses left behind;
  /// 4. the chrome is painted over it.
  ///
  /// # What it refuses to draw
  ///
  /// With no program loaded the half says what it is for and offers the one control that fills it — the
  /// `Simulate` chip (M13 S3) — and draws **no** overlay row and **no** playback bar. Every control in those two
  /// strips governs something in the picture or in the program, and a checkbox over an empty scene is the same
  /// lie as a count nobody measured (L6): there is nothing to hide and nothing to play, so there is nothing to
  /// draw. The chip is the exception that proves the rule: it governs the picture's *absence*, and its own
  /// press is what gives the two strips something to govern.
  pub(crate) fn half_3d(
    &mut self,
    app: &cam_app::AppState,
    ui: &mut egui::Ui,
    rect: egui::Rect,
    output: &mut crate::ShellOutput,
    dt: f64,
  ) {
    if rect.width() <= 1.0 || rect.height() <= 1.0 {
      return;
    }
    let palette = self.palette();
    let metrics = self.metrics();
    let layout = half_layout(rect, &metrics);
    if self.preview.is_none() {
      let painter = ui.painter().with_clip_rect(layout.picture);
      let origin = egui::pos2(
        layout.picture.min.x + metrics.space(4),
        layout.picture.min.y + metrics.space(4),
      );
      // What the half is *for*, never which other program to run: M13 S3 removed the sentence that sent the
      // person to a terminal, and a control replaced it.
      painter.text(
        origin,
        egui::Align2::LEFT_TOP,
        "This half draws the drawing's own program cutting its stock.",
        egui::FontId::proportional(theme::CAPTION),
        palette.get(Role::TextDim),
      );
      let word = "Simulate";
      let size = chip_size(&painter, &palette, word, false);
      let rect = egui::Rect::from_min_size(
        egui::pos2(origin.x, origin.y + theme::CAPTION + metrics.space(3)),
        size,
      );
      let response = ui.allocate_rect(rect, egui::Sense::click());
      if response.clicked() {
        self.simulate_pressed(app);
      }
      paint_plain_chip(&painter, rect, &palette, word);
      let centre = rect.center();
      output.drawn.simulate_chip = Some((centre.x, centre.y));
      return;
    }

    let response = ui.allocate_rect(layout.picture, egui::Sense::click_and_drag());
    if let Some(preview) = self.preview.as_mut() {
      preview.take_gestures(ui, &response, layout.picture);
      // Playback advances only while the split is open: a hidden half is not a clock, and the 2D path pays
      // nothing for a preview nobody is looking at (A12).
      preview.advance(dt);
      if preview.playing() {
        // Without this the window would repaint only on input, and a program that plays to its end would
        // appear to play only when the mouse moves.
        ui.ctx().request_repaint();
      }
    }

    let row = self.overlay_row(app, ui, &layout, &palette, output);
    let home = self.home_button(ui, &layout, output);
    let bar = self.playback_controls(ui, &layout, &palette, output);

    // The vectors are the document's, and the layers are the row's answer taken *after* its presses: neither is
    // part of the preview, so both are handed in, and a hidden layer costs no projection at all.
    let layers = self.scene_layers;
    let vectors = if layers.vectors {
      vector_lines(app.document())
    } else {
      Vec::new()
    };
    let painter = ui.painter().with_clip_rect(layout.picture);
    if let Some(preview) = self.preview.as_ref() {
      preview.paint_scene(
        &painter,
        layout.picture,
        &mut output.drawn,
        &palette,
        &vectors,
        layers,
      );
      output.drawn.triad = preview.triad(&painter, layout.corner, &palette);
    }
    for (word, rect, on) in &row {
      if Layer::ALL.iter().any(|layer| layer.label() == word) {
        paint_layer_chip(&painter, *rect, &palette, word, *on);
      } else {
        paint_plain_chip(&painter, *rect, &palette, word);
      }
    }
    paint_home(&painter, home, &palette);
    // The bar sits *below* the picture, so it needs a painter that is allowed to be there: a painter clipped to
    // `layout.picture` erased every rectangle of the bar while its hit areas stayed live — an invisible control
    // that still takes clicks (R2-001, R3-001 in the T6 review). Its own strip is the honest clip.
    let bar_painter = ui.painter().with_clip_rect(layout.bar);
    self.paint_playback(&bar_painter, &layout, &palette, output, &bar);
  }

  /// Simulate the open document's own toolpaths and hand the picture to the half (M13 S3).
  ///
  /// This is the one place the 3D half stops showing a program somebody else produced. The document's
  /// toolpaths become a program through [`cam_program::program_for_snapshot`], a pristine
  /// [`Heightfield::new`] stock is built at [`cam_sim::DEFAULT_CELL`], and [`Shell::set_preview`] takes both —
  /// which also opens the split, because a press that produced a picture means to look at it. The note names
  /// the duration the playback bar names, because "how long is this job" is the question the picture answers.
  ///
  /// # Refusals, by name
  ///
  /// A drawing with no toolpaths has nothing to simulate ([`NO_TOOLPATHS_TO_SIMULATE`]), and a toolpath whose
  /// tool this window's library cannot resolve has no program the builder can make — the builder's own sentence
  /// names the tool and the toolpath. Both are errors here rather than an empty picture: a half that drew a
  /// program the document does not have would be worse than one that says what is missing. Nothing is written
  /// and no document is touched, because a simulation moves a cursor and never the geometry.
  ///
  /// # Why the material is not passed on
  ///
  /// A snapshot is a list of tools with their own records, and a material is a selector *into a library*, so
  /// `program_for_snapshot` refuses one by name; passing the document's own would make every drawing that names
  /// one refuse to simulate. `None` is what the snapshot can honestly mean, which is the same reason
  /// `Shell::generate_gcode` passes it and the same recorded divergence the cut-time estimate carries.
  fn simulate(&mut self, app: &cam_app::AppState) -> Result<String, String> {
    if app.document().toolpaths.is_empty() {
      return Err(NO_TOOLPATHS_TO_SIMULATE.to_owned());
    }
    let program = cam_program::program_for_snapshot(app.document(), &self.tools, None)
      .map_err(|error| error.to_string())?;
    let stock = Heightfield::new(&app.document().job, Length::from_mm(cam_sim::DEFAULT_CELL))
      .map_err(|error| error.to_string())?;
    self.set_preview(stock, program, &app.document().job);
    let duration = self.preview().map_or(0.0, Preview3d::duration);
    Ok(format!(
      "simulate — {} of cutting, drawn in 3D",
      clock_text(duration)
    ))
  }

  /// Run [`Shell::simulate`] from a press and put its answer in the status bar.
  ///
  /// The two controls that simulate — the empty half's `Simulate` and the overlay row's `Simulate again` —
  /// report through this one function, so the note and the refusal cannot drift between them.
  fn simulate_pressed(&mut self, app: &cam_app::AppState) {
    self.status = match self.simulate(app) {
      Ok(note) => note,
      Err(message) => format!("refused: {message}"),
    };
  }

  /// The overlay row: the four layer checkboxes, `Simulate again`, `Centre` and `?` (camui-prancheta S8, M13 S3).
  ///
  /// Each press is applied where it happens, so the frame that hides the tool is also the frame that reports it
  /// hidden and the frame whose picture has no tool in it. `Simulate again` rebuilds the picture from the open
  /// document, `Centre` re-frames the stock at the angle the eye is at, and `?` opens the `F1` panel — the
  /// shell's one place that teaches every command — rather than a help window invented for this row. Returns what
  /// is left to paint.
  fn overlay_row(
    &mut self,
    app: &cam_app::AppState,
    ui: &mut egui::Ui,
    layout: &Half,
    palette: &Palette,
    output: &mut crate::ShellOutput,
  ) -> RowSlots {
    let metrics = self.metrics();
    let painter = ui.painter().clone();
    let row = layout.overlay;
    let mut items: Vec<(String, bool)> = Layer::ALL
      .iter()
      .map(|layer| (layer.label().to_owned(), true))
      .collect();
    items.push(("Simulate again".to_owned(), false));
    items.push(("Centre".to_owned(), false));
    items.push(("?".to_owned(), false));
    let mut slots: RowSlots = Vec::new();
    let mut left = row.min.x;
    let mut line = 0;
    for (word, checkbox) in items {
      let size = chip_size(&painter, palette, &word, checkbox);
      if left + size.x > row.max.x {
        // The row wraps, like the reference's own: `?` is on a second line there, and a chip drawn past the
        // half's edge would be over the panel next door while one clipped in half is not pressable at all.
        line += 1;
        left = row.min.x;
        if line >= OVERLAY_ROWS {
          break;
        }
      }
      let rect = egui::Rect::from_min_size(
        egui::pos2(
          left,
          row.min.y + line as f32 * (OVERLAY_LINE_HEIGHT + metrics.space(1)),
        ),
        size,
      );
      left += size.x + metrics.space(1);
      slots.push((word, rect, false));
    }
    for (word, rect, on) in &mut slots {
      let response = ui.allocate_rect(*rect, egui::Sense::click());
      if !response.clicked() {
        if let Some(layer) = Layer::ALL.iter().find(|layer| layer.label() == word) {
          *on = self.scene_layers.get(*layer);
        }
        continue;
      }
      match word.as_str() {
        "Simulate again" => self.simulate_pressed(app),
        "Centre" => {
          if let Some(preview) = self.preview.as_mut() {
            preview.recentre();
          }
        }
        "?" => self.show_help = true,
        _ => {
          if let Some(layer) = Layer::ALL.iter().find(|layer| layer.label() == word) {
            let showing = self.scene_layers.get(*layer);
            self.scene_layers.set(*layer, !showing);
            *on = !showing;
          }
        }
      }
    }
    output.drawn.layer_chips = slots
      .iter()
      .map(|(word, rect, _)| (word.clone(), (rect.center().x, rect.center().y)))
      .collect();
    output.drawn.scene_layers = self.scene_layers.words();
    slots
  }

  /// The home button: allocated and applied here, painted by the caller after the picture.
  fn home_button(
    &mut self,
    ui: &mut egui::Ui,
    layout: &Half,
    output: &mut crate::ShellOutput,
  ) -> egui::Rect {
    let size = 22.0;
    let rect = egui::Rect::from_min_size(
      egui::pos2(layout.corner.center().x - size * 0.5, layout.corner.min.y),
      egui::vec2(size, size),
    );
    let response = ui.allocate_rect(rect, egui::Sense::click());
    if response.clicked() {
      if let Some(preview) = self.preview.as_mut() {
        preview.home();
      }
    }
    let centre = rect.center();
    output.drawn.home_button = Some((centre.x, centre.y));
    rect
  }

  /// The playback bar's controls: the transport, the speed, `To the end`, `Step`, `Reset` and the scrubber
  /// (camui-prancheta S8).
  ///
  /// `Step` and `Reset` are here as well. They are not in S8's list and they stay because they are behaviour this
  /// shell already had and already tests (M5 A10, `the_panel_offers_play_step_and_reset`): deleting a working
  /// control to match a list is a regression, and both move the same playhead the rest of the bar moves.
  fn playback_controls(
    &mut self,
    ui: &mut egui::Ui,
    layout: &Half,
    palette: &Palette,
    output: &mut crate::ShellOutput,
  ) -> BarPaint {
    let metrics = self.metrics();
    let painter = ui.painter().clone();
    let bar = layout.bar;
    let row = egui::Rect::from_min_size(
      egui::pos2(bar.min.x + 8.0, bar.min.y + 4.0),
      egui::vec2((bar.width() - 16.0).max(0.0), 20.0),
    );
    let Some(preview) = self.preview.as_mut() else {
      return BarPaint::default();
    };
    let mut paint = BarPaint::default();
    let mut left = row.min.x;
    let middle = row.center().y;

    // The transport: one button whose mark and word say what pressing it does now.
    paint.transport =
      egui::Rect::from_min_size(egui::pos2(left, middle - 9.0), egui::vec2(26.0, 18.0));
    left += 26.0 + metrics.space(1);
    let response = ui.allocate_rect(paint.transport, egui::Sense::click());
    if response.clicked() {
      preview.toggle_playing();
    }
    paint.playing = preview.playing();
    let centre = paint.transport.center();
    paint.hits.push((
      if paint.playing { "Pause" } else { "Play" }.to_owned(),
      (centre.x, centre.y),
    ));

    // The speed: a pill naming the stop it is set to, stepping to the next one on a press. Its width is the
    // widest word the ladder can produce, so pressing it never shifts the scrubber beside it.
    let size = chip_size(
      &painter,
      palette,
      &speed_text(SPEEDS[SPEEDS.len() - 1]),
      false,
    );
    let speed = egui::Rect::from_min_size(egui::pos2(left, middle - 9.0), size);
    left += size.x + metrics.space(1);
    let response = ui.allocate_rect(speed, egui::Sense::click());
    if response.clicked() {
      preview.set_speed(next_speed(preview.speed()));
    }
    paint.speed = (speed, speed_text(preview.speed()));
    let centre = speed.center();
    paint
      .hits
      .push((speed_text(preview.speed()), (centre.x, centre.y)));

    // `To the end` shares the control line with the transport and the speed; `Step` and `Reset` live on the
    // readout's line, at its right end, where a second pair of pacing controls does not crowd the scrubber.
    {
      let word = "To the end";
      let size = chip_size(&painter, palette, word, false);
      let rect = egui::Rect::from_min_size(egui::pos2(left, middle - 9.0), size);
      left += size.x + metrics.space(1);
      let response = ui.allocate_rect(rect, egui::Sense::click());
      if response.clicked() {
        preview.to_the_end();
      }
      let centre = rect.center();
      paint.hits.push((word.to_owned(), (centre.x, centre.y)));
      paint.plain.push((word.to_owned(), rect));
    }
    let mut right = row.max.x;
    for word in ["Reset", "Step"] {
      let size = chip_size(&painter, palette, word, false);
      right -= size.x;
      let rect = egui::Rect::from_min_size(egui::pos2(right, bar.max.y - 6.0 - size.y), size);
      right -= metrics.space(1);
      let response = ui.allocate_rect(rect, egui::Sense::click());
      if response.clicked() {
        match word {
          "Step" => preview.step(),
          _ => preview.reset(),
        }
      }
      let centre = rect.center();
      paint.hits.push((word.to_owned(), (centre.x, centre.y)));
      paint.plain.push((word.to_owned(), rect));
    }

    // The scrubber takes the row's remaining width: how far the program has played is the one thing in the bar
    // that is a measurement rather than a control, so it is the part that stretches.
    let track = egui::Rect::from_min_size(
      egui::pos2(left + metrics.space(1), middle - 4.0),
      egui::vec2((row.max.x - left - metrics.space(2)).max(0.0), 8.0),
    );
    if track.width() > 0.0 {
      let response = ui.allocate_rect(track, egui::Sense::click_and_drag());
      if response.clicked() || response.dragged() {
        if let Some(at) = response.interact_pointer_pos() {
          let fraction = f64::from(((at.x - track.min.x) / track.width()).clamp(0.0, 1.0));
          preview.seek(fraction);
        }
      }
      paint.fill = bar_fill(preview.progress(), preview.duration());
      paint.scrubber = Some(track);
    }
    output.drawn.playback_hits = paint.hits.clone();
    paint
  }

  /// Paint the playback bar over the picture, and report what it reads.
  ///
  /// The readout is written **here**, after every press above has reached the preview, so the clock in the bar
  /// and the clock in the report are the same moment (`tests/view3d.rs` reads both).
  fn paint_playback(
    &mut self,
    painter: &egui::Painter,
    layout: &Half,
    palette: &Palette,
    output: &mut crate::ShellOutput,
    bar: &BarPaint,
  ) {
    let strip = layout.bar;
    let radius = egui::CornerRadius::same(8);
    painter.rect_filled(strip, radius, palette.get(Role::Overlay));
    painter.rect_stroke(
      strip,
      radius,
      egui::Stroke::new(1.0, palette.get(Role::Border)),
      egui::StrokeKind::Inside,
    );
    paint_transport(painter, bar.transport, bar.playing, palette);
    paint_plain_chip(painter, bar.speed.0, palette, &bar.speed.1);
    for (word, rect) in &bar.plain {
      paint_plain_chip(painter, *rect, palette, word);
    }
    if let Some(track) = bar.scrubber {
      paint_scrubber(painter, track, bar.fill, palette);
    }
    let Some(preview) = self.preview.as_ref() else {
      return;
    };
    output.drawn.preview_progress = preview.progress();
    output.drawn.preview_bar = bar_fill(preview.progress(), preview.duration());
    output.drawn.preview_clock = progress_text(preview.elapsed(), preview.duration());
    output.drawn.playback_readout = format!(
      "{}  {}",
      output.drawn.preview_clock,
      preview.playhead_text()
    );
    painter.text(
      egui::pos2(strip.min.x + 10.0, strip.max.y - 6.0),
      egui::Align2::LEFT_BOTTOM,
      &output.drawn.playback_readout,
      egui::FontId::monospace(theme::CAPTION),
      palette.get(Role::TextHi),
    );
    output.drawn.scrubber = bar
      .scrubber
      .map(|track| (track.min.x, track.max.x, track.center().y));
  }
}
