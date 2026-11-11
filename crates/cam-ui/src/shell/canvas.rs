use cam_app::AppState;
use cam_geom::Point2;
use cam_view::build_scene;
use egui::Sense;

use crate::chrome;
use crate::ruler;
use crate::theme::Role;
use crate::{CanvasRect, Shell, ShellOutput, MIN_FRAME_CANVAS};

impl Shell {
  /// The central panel: the drawing, the `2D | 3D` toggle, and — while 3D is the mode — the split
  /// (camui-prancheta S8).
  ///
  /// # Why the split is two columns
  ///
  /// The reference frame (`design/prancheta-reference/Screenshot 2026-11-03 032943.png`) splits the canvas
  /// left to right, and this shell's own sheet says why that is right: a part is wider than it is tall, so a
  /// horizontal split would take the drawing's long side away from it and leave both halves worse than the
  /// drawing was. Two columns keep each half's shape close to the one the drawing already had, which is also
  /// why the 2D half stays on the left with the rulers S5 gave it.
  ///
  /// # What each half keeps
  ///
  /// The 2D half **is** the canvas: [`ShellOutput::canvas`] is its rectangle, the 2D camera is told about that
  /// rectangle and nothing else, and picking, drawing, panning, zooming and the context menu all stay inside
  /// it. The 3D half owns dragging and the wheel, and its own chrome
  /// ([`view3d`](crate::view3d) — the overlay row, the home button, the triad and the playback bar). The 3D
  /// half inherits the ruler's inset (L20), so both halves' content starts on the drawing's own top line.
  ///
  /// # Why the toggle is allocated first
  ///
  /// The toggle is drawn and pressed before the split is decided, so the frame that presses `3D` is the frame
  /// that splits: `show_3d` is read by the layout below, after the click (L13's rule). It is drawn in the
  /// canvas' top strip — the band the ruler occupies on the drawing half (S5) — and never over the sheet,
  /// because a press on chrome that also dropped a selection on the drawing underneath would be a control that
  /// lies.
  pub(super) fn canvas(
    &mut self,
    app: &mut AppState,
    ui: &mut egui::Ui,
    output: &mut ShellOutput,
    dt: f64,
  ) {
    let palette = self.palette();
    let metrics = self.metrics();
    egui::CentralPanel::default()
      .frame(egui::Frame::NONE.fill(palette.get(Role::Canvas)))
      .show(ui, |ui| {
        let area = ui.available_rect_before_wrap();
        // The whole drawing area: the central area inset by the two bands (S5, L20). The clamp is for a frame
        // whose central area has not been given its room yet — a band must not make the canvas negative.
        let thickness = metrics.ruler_thickness;
        let drawing = egui::Rect::from_min_max(
          egui::pos2(
            (area.min.x + thickness).min(area.max.x),
            (area.min.y + thickness).min(area.max.y),
          ),
          area.max,
        );
        let toggle_paint = self.view_toggle(ui, area, &metrics, output);
        // No project: the drawing area holds one sentence and nothing else (start-screen S2). No stock, no grid, no
        // ruler numbering an origin that is not there, and no `2D | 3D` toggle over a canvas with nothing to show. The
        // camera is still told the rectangle it would be drawn in, so the first project to arrive is framed against
        // the area it will really use instead of against a default nobody can see.
        if !self.project_open {
          let canvas = CanvasRect {
            x: f64::from(drawing.min.x),
            y: f64::from(drawing.min.y),
            width: f64::from(drawing.width()),
            height: f64::from(drawing.height()),
          };
          output.canvas = canvas;
          self.last_canvas = canvas;
          app.camera_mut().set_viewport(canvas.viewport());
          crate::start_screen::paint_no_project(ui.painter(), drawing, &palette, &metrics);
          return;
        }
        let (area_2d, drawing_2d, drawing_3d) = if self.show_3d {
          // Two columns of the drawing area. The left one's own band area is the left half of the central
          // area, whose inset is exactly the drawing half — which is what keeps the rulers and the camera
          // talking about the same rectangle.
          let mid = drawing.min.x + drawing.width() * 0.5;
          (
            egui::Rect::from_min_max(area.min, egui::pos2(mid, area.max.y)),
            egui::Rect::from_min_max(drawing.min, egui::pos2(mid, drawing.max.y)),
            egui::Rect::from_min_max(egui::pos2(mid, drawing.min.y), drawing.max),
          )
        } else {
          (area, drawing, egui::Rect::NOTHING)
        };
        // The bands frame the drawing half. With the split closed that is the whole central area; with it open
        // the band numbers the drawing half only, because numbers over the 3D half would describe a camera that
        // is not there. The toggle floats on the band's right end and its own fill covers what it crosses.
        let band = if self.show_3d { area_2d } else { area };

        let canvas = CanvasRect {
          x: f64::from(drawing_2d.min.x),
          y: f64::from(drawing_2d.min.y),
          width: f64::from(drawing_2d.width()),
          height: f64::from(drawing_2d.height()),
        };
        output.canvas = canvas;
        self.last_canvas = canvas;
        app.camera_mut().set_viewport(canvas.viewport());
        // Fit against the canvas measured this frame. The status bar has already rendered with the
        // previous zoom, so a fit asks for one more frame; egui is lazy and would otherwise leave the
        // readout disagreeing with the canvas until the next mouse move.
        if self.frame_if_needed(app, &canvas) {
          ui.ctx().request_repaint();
        }

        // The bands report what they drew (S5), including whether the pointer is on the sheet: the marker is
        // drawn only while it is, and `cursor_world` is exactly that question already answered.
        output.drawn.rulers = ruler::paint(
          ui.painter(),
          band,
          &canvas,
          app.camera(),
          &palette,
          &metrics,
          self.cursor_world,
        );
        // The band painted after the widget was laid out, and a later painter wins: the toggle is painted a
        // second time here, so the control that enters 3D is never under a ruler number (R2-002 in the T6
        // review). Same function as its first paint, and `show_3d` is already the pressed value, so this frame
        // shows the view the press chose.
        if let Some(control) = toggle_paint.as_ref() {
          chrome::paint_segmented(
            ui.painter(),
            control.rect,
            &control.cell_rects,
            &["2D", "3D"],
            usize::from(self.show_3d),
            &palette,
          );
        }

        let response = ui.allocate_rect(drawing_2d, Sense::click_and_drag());
        // Everything the drawing paints is clipped to its own half: the grid and the geometry must not spill
        // onto a band, print a drawing over a number, or paint over the 3D view beside it.
        let painter = ui.painter().with_clip_rect(drawing_2d);

        self.handle_input(app, ui, &response, &canvas, output);
        // The context menu is drawn after the input that opens it, so a right-click and the rows it scopes
        // arrive in the same frame; the popup's own memory decides whether it is open at all.
        self.canvas_menu(app, &response, output);
        self.paint(painter, &canvas, app, output);

        if self.show_3d {
          output.drawn.split = Some((
            chrome::rect_report(drawing_2d),
            chrome::rect_report(drawing_3d),
          ));
          self.half_3d(app, ui, drawing_3d, output, dt);
        }
      });
  }

  /// Hand the view to the user: auto-framing stops until the shell is asked again.
  ///
  /// Called by pan and zoom gestures, and available to hosts and tests that move the camera directly.
  pub fn release_view(&mut self) {
    self.auto_frame = false;
  }

  /// Frame the document while the view still belongs to the application.
  ///
  /// Refits on every frame the canvas is plausible, which makes the result exact for the canvas that is
  /// really there: a window that opens small and is then resized ends up framed for its final size
  /// instead of for whatever it measured on the first frame. Returns true when the camera moved, so the
  /// caller can ask for one more frame — the status bar has already drawn with the old zoom.
  pub fn frame_if_needed(&mut self, app: &mut AppState, canvas: &CanvasRect) -> bool {
    if !self.auto_frame || canvas.width <= 0.0 || canvas.height <= 0.0 {
      return false;
    }
    if canvas.width < MIN_FRAME_CANVAS || canvas.height < MIN_FRAME_CANVAS {
      // Too small to be a real layout: the panels have not taken their room yet.
      return false;
    }
    let before_zoom = app.camera().pixels_per_mm();
    let before_centre = app.camera().center();
    self.frame_document(app, canvas);
    (app.camera().pixels_per_mm() - before_zoom).abs() > 1.0e-6
      || !app.camera().center().is_near(before_centre, 1.0e-6)
  }

  /// Frame the document for this canvas.
  fn frame_document(&mut self, app: &mut AppState, canvas: &CanvasRect) {
    if canvas.width <= 0.0 || canvas.height <= 0.0 {
      return;
    }
    let scene = build_scene(app.document(), app.selection());
    let mut bounds = scene.bounds();
    if bounds.is_empty() {
      let job = &app.document().job;
      bounds = cam_geom::Bounds::from_points(&[
        Point2::new(0.0, 0.0),
        Point2::new(job.width.mm(), job.height.mm()),
      ]);
    }
    if !bounds.is_empty() {
      app.fit_camera(bounds.expand(10.0), 32.0);
    }
    self.framed = true;
  }
}
