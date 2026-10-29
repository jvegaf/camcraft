//! The egui editor shell: panels, canvas and input wiring.
//!
//! # The rule this crate exists to keep
//!
//! The shell contains no business rules. It turns input into [`UiAction`] values, paints
//! [`cam_view::Scene`], and reports what it displayed. [`handle_action`] is the only place that
//! touches [`AppState`], which is what lets the whole interface be tested headless: a test builds a
//! frame with a synthetic input, inspects the actions and the painted text, then applies the actions
//! and asserts on the resulting state.
//!
//! Arcs are flattened **here**, at half a pixel, and nowhere earlier.
//!
//! Must not contain geometry, toolpath or document rules.

#![warn(missing_docs)]

use cam_app::{AppState, Camera, Command, ScreenPoint, Viewport};
use cam_geom::Point2;
use cam_view::{
  box_select, build_grid, build_scene, choose_snap, pick, SceneItem, SnapKind, Style,
  PICK_TOLERANCE_PX, SNAP_TOLERANCE_PX, TARGET_GRID_PIXELS,
};
use egui::{Context, Pos2, Sense, Stroke};

/// The smallest canvas that auto-framing will act on.
///
/// The first frames of a real window can report a canvas of a few dozen pixels while the panels settle
/// their widths. Fitting that geometry zooms the drawing out by an order of magnitude and looks like a
/// broken viewport, so anything smaller than this is ignored and retried next frame.
pub const MIN_FRAME_CANVAS: f64 = 200.0;

/// Colours, in one place so the picture stays consistent.
mod palette {
  use egui::Color32;

  /// Background of the drawing area.
  pub const CANVAS: Color32 = Color32::from_rgb(24, 26, 30);
  /// Grid lines.
  pub const GRID: Color32 = Color32::from_rgb(44, 48, 56);
  /// Ordinary geometry.
  pub const GEOMETRY: Color32 = Color32::from_rgb(206, 212, 222);
  /// Selected geometry.
  pub const SELECTED: Color32 = Color32::from_rgb(255, 176, 59);
  /// The stock outline.
  pub const JOB: Color32 = Color32::from_rgb(88, 132, 178);
  /// Snap markers and the selection marquee.
  pub const SNAP: Color32 = Color32::from_rgb(120, 220, 160);
}

/// Everything the user can ask for. Every button, click and shortcut becomes one of these.
#[derive(Clone, Debug, PartialEq)]
pub enum UiAction {
  /// Step one command back.
  Undo,
  /// Step one command forward.
  Redo,
  /// Frame the whole drawing.
  ZoomToFit,
  /// Add a rectangle, centred on the current view.
  AddRectangle {
    /// Width in millimetres.
    width: f64,
    /// Height in millimetres.
    height: f64,
  },
  /// Delete the selected geometry.
  DeleteSelection,
  /// Replace the selection.
  SetSelection(Vec<String>),
  /// Show or hide a layer.
  ToggleLayerVisible {
    /// The layer to change.
    layer_id: String,
  },
}

impl UiAction {
  /// A short label for the status bar.
  pub fn label(&self) -> &'static str {
    match self {
      Self::Undo => "undo",
      Self::Redo => "redo",
      Self::ZoomToFit => "zoom to fit",
      Self::AddRectangle { .. } => "add rectangle",
      Self::DeleteSelection => "delete selection",
      Self::SetSelection(_) => "select",
      Self::ToggleLayerVisible { .. } => "layer visibility",
    }
  }
}

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
  /// Lines the property panel displayed.
  pub property_lines: Vec<String>,
  /// The status bar text.
  pub status_line: String,
  /// Scene items painted on the canvas.
  pub painted_items: usize,
  /// Grid lines painted on the canvas.
  pub painted_grid_lines: usize,
}

/// What one frame produced.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShellOutput {
  /// Where the canvas was drawn.
  pub canvas: CanvasRect,
  /// Actions the user triggered this frame; the caller applies them through [`handle_action`].
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

/// The shell's own state: widget values and transient view flags.
#[derive(Clone, Debug)]
pub struct Shell {
  /// Size used by the "Add rectangle" button.
  pub rectangle_size: (f64, f64),
  /// Message shown in the status bar.
  pub status: String,
  /// World position of the cursor, when it is over the canvas.
  pub cursor_world: Option<Point2>,
  /// Which snap kinds are enabled.
  pub snap_kinds: Vec<SnapKind>,
  /// Grid step in millimetres, as reported by the view for snapping.
  pub grid_spacing: f64,
  /// Whether snapping is active.
  pub snapping: bool,
  /// The snap point found for the cursor this frame, if any.
  pub snap_marker: Option<(SnapKind, Point2)>,
  /// Origin of a box selection in progress, in canvas-local pixels.
  drag_box: Option<ScreenPoint>,
  /// Whether the view still belongs to the application.
  ///
  /// Stays true until the user pans or zooms: opening the window, resizing it, or the panels settling
  /// must all end with the drawing framed, and only an explicit view gesture should take that away.
  auto_frame: bool,
  /// Whether a document has been framed at least once.
  ///
  /// A camera centred on the origin with 1 px/mm leaves a 1219 mm job almost entirely off screen, so
  /// the first frame that knows its canvas size frames the drawing. Motion is view state: it is not
  /// an undoable edit.
  framed: bool,
  /// The canvas rectangle from the previous frame.
  ///
  /// Panels are laid out before the central area, so the status bar can only show this frame's cursor
  /// if pointer tracking uses last frame's canvas geometry. That is the standard immediate-mode
  /// approach and it keeps the readout a frame fresh instead of a frame stale.
  last_canvas: CanvasRect,
}

impl Default for Shell {
  fn default() -> Self {
    Self {
      rectangle_size: (100.0, 60.0),
      status: String::new(),
      cursor_world: None,
      snap_kinds: vec![
        SnapKind::Endpoint,
        SnapKind::Midpoint,
        SnapKind::Center,
        SnapKind::Grid,
      ],
      grid_spacing: 0.0,
      snapping: true,
      snap_marker: None,
      drag_box: None,
      auto_frame: true,
      framed: false,
      last_canvas: CanvasRect::default(),
    }
  }
}

impl Shell {
  /// A shell with default widget values.
  pub fn new() -> Self {
    Self::default()
  }

  /// Build one frame.
  ///
  /// Actions are returned rather than applied: one frame of latency is invisible, and a pure shell
  /// is what makes the interface testable.
  pub fn show(&mut self, app: &mut AppState, ctx: &Context) -> ShellOutput {
    // egui 0.37 stacks panels on the root `Ui`; every panel of one frame goes through one borrow.
    ctx.root_ui(|ui| self.show_in(app, ui))
  }

  /// Build one frame into an existing root [`egui::Ui`].
  ///
  /// `eframe` hands the application a root `Ui` directly, and calling [`Self::show`] from inside that
  /// callback would try to borrow the root twice. Hosts that already hold one use this.
  pub fn show_in(&mut self, app: &mut AppState, ui: &mut egui::Ui) -> ShellOutput {
    let mut output = ShellOutput::default();
    let ctx = ui.ctx().clone();
    self.track_pointer(app, &ctx);
    self.command_bar(app, ui, &mut output);
    self.layer_dock(app, ui, &mut output);
    self.properties(app, ui, &mut output);
    self.status_bar(app, ui, &mut output);
    self.canvas(app, ui, &mut output);
    output
  }

  fn command_bar(&mut self, app: &AppState, ui: &mut egui::Ui, output: &mut ShellOutput) {
    egui::Panel::top("command_bar").show(ui, |ui| {
      ui.horizontal(|ui| {
        ui.heading("campilot");
        ui.separator();

        if ui
          .add_enabled(app.can_undo(), egui::Button::new("Undo"))
          .clicked()
        {
          output.actions.push(UiAction::Undo);
        }
        if ui
          .add_enabled(app.can_redo(), egui::Button::new("Redo"))
          .clicked()
        {
          output.actions.push(UiAction::Redo);
        }
        ui.separator();
        if ui.button("Zoom to fit").clicked() {
          output.actions.push(UiAction::ZoomToFit);
        }
        ui.separator();

        ui.label("Rectangle");
        ui.add(
          egui::DragValue::new(&mut self.rectangle_size.0)
            .speed(1.0)
            .suffix(" mm"),
        );
        ui.label("x");
        ui.add(
          egui::DragValue::new(&mut self.rectangle_size.1)
            .speed(1.0)
            .suffix(" mm"),
        );
        if ui.button("Add").clicked() {
          output.actions.push(UiAction::AddRectangle {
            width: self.rectangle_size.0,
            height: self.rectangle_size.1,
          });
        }

        ui.separator();
        if ui
          .add_enabled(
            !app.selection().is_empty(),
            egui::Button::new("Delete selection"),
          )
          .clicked()
        {
          output.actions.push(UiAction::DeleteSelection);
        }
      });
    });
  }

  fn layer_dock(&mut self, app: &AppState, ui: &mut egui::Ui, output: &mut ShellOutput) {
    egui::Panel::left("layer_dock")
      .default_size(220.0)
      .show(ui, |ui| {
        ui.heading("Layers");
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
          for layer in &app.document().layers {
            let mut visible = layer.visible;
            ui.horizontal(|ui| {
              if ui.checkbox(&mut visible, "").changed() {
                output.actions.push(UiAction::ToggleLayerVisible {
                  layer_id: layer.id.clone(),
                });
              }
              ui.label(format!("{} — {}", layer.id, layer.name));
            });
            ui.label(format!("  {} entit(ies)", layer.entities.len()));
            output
              .drawn
              .layer_rows
              .push((layer.id.clone(), layer.entities.len(), layer.visible));
            ui.separator();
          }
        });
      });
  }

  fn properties(&mut self, app: &AppState, ui: &mut egui::Ui, output: &mut ShellOutput) {
    egui::Panel::right("properties")
      .default_size(260.0)
      .show(ui, |ui| {
        ui.heading("Properties");
        ui.separator();
        let job = &app.document().job;
        let mut lines = vec![
          format!(
            "Job: {:.1} x {:.1} x {:.1} mm",
            job.width.mm(),
            job.height.mm(),
            job.thickness.mm()
          ),
          format!("Units: {:?}, origin {:?}", job.units, job.origin),
          format!("Safe Z: {:.1} mm", job.safe_z.mm()),
        ];
        ui.label(&lines[0]);
        ui.label(&lines[1]);
        ui.label(&lines[2]);
        ui.separator();

        let selected: Vec<String> = app.selection().ids().map(str::to_owned).collect();
        if selected.is_empty() {
          lines.push("Selection: none".to_owned());
          ui.label("Selection: none");
        } else {
          for id in &selected {
            let summary = match app
              .document()
              .entity(id)
              .and_then(|entity| entity.kind.as_contour())
            {
              Some(contour) => format!(
                "{id}: contour, {:.1} mm perimeter, area {:.1} mm²",
                contour.perimeter(),
                contour.signed_area().abs()
              ),
              None => format!("{id}: (missing)"),
            };
            ui.label(&summary);
            lines.push(summary);
          }
        }
        ui.separator();
        lines.push(format!("History: {} step(s)", app.history_depth()));
        ui.label(format!("History: {} step(s)", app.history_depth()));
        output.drawn.property_lines = lines;
      });
  }

  fn status_bar(&mut self, app: &AppState, ui: &mut egui::Ui, output: &mut ShellOutput) {
    egui::Panel::bottom("status_bar").show(ui, |ui| {
      ui.horizontal(|ui| {
        let cursor = match self.cursor_world {
          Some(point) => format!("X {:.2}  Y {:.2} mm", point.x, point.y),
          None => "—".to_owned(),
        };
        let dirty = if app.is_dirty() { "modified" } else { "saved" };
        // "undo / redo" are the steps available in each direction; dropped steps are reported
        // separately because they are a warning, not a redo count.
        let dropped = app.history_dropped();
        let dropped = if dropped > 0 {
          format!("   {dropped} step(s) dropped")
        } else {
          String::new()
        };
        let line = format!(
          "{cursor}   zoom {:.2} px/mm   {dirty}   undo {} / redo {}{dropped}   {}",
          app.camera().pixels_per_mm(),
          app.history_depth(),
          app.redo_depth(),
          self.status
        );
        ui.monospace(&line);
        output.drawn.status_line = line;
      });
    });
  }

  fn canvas(&mut self, app: &mut AppState, ui: &mut egui::Ui, output: &mut ShellOutput) {
    egui::CentralPanel::default()
      .frame(egui::Frame::NONE.fill(palette::CANVAS))
      .show(ui, |ui| {
        let available = ui.available_rect_before_wrap();
        let (response, painter) = ui.allocate_painter(available.size(), Sense::click_and_drag());

        let canvas = CanvasRect {
          x: f64::from(response.rect.min.x),
          y: f64::from(response.rect.min.y),
          width: f64::from(response.rect.width()),
          height: f64::from(response.rect.height()),
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

        self.handle_input(app, ui, &response, &canvas, output);
        self.paint(painter, &canvas, app, output);
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

  /// Follow the pointer: cursor world position and the snap point it is near.
  ///
  /// Runs before the panels so the status bar shows this frame's coordinates, using the canvas
  /// rectangle remembered from the last frame.
  fn track_pointer(&mut self, app: &mut AppState, ctx: &Context) {
    if self.last_canvas.width <= 0.0 || self.last_canvas.height <= 0.0 {
      return;
    }
    app.camera_mut().set_viewport(self.last_canvas.viewport());

    let Some(position) = ctx.input(|input| input.pointer.hover_pos()) else {
      self.cursor_world = None;
      self.snap_marker = None;
      return;
    };
    let local = self.last_canvas.to_screen(position);
    if local.x < 0.0
      || local.y < 0.0
      || local.x > self.last_canvas.width
      || local.y > self.last_canvas.height
    {
      self.cursor_world = None;
      self.snap_marker = None;
      return;
    }

    self.cursor_world = Some(app.camera().screen_to_world(local));
    self.snap_marker = None;
    if self.snapping {
      let scene = build_scene(app.document(), app.selection());
      if let Some(snap) = choose_snap(
        &scene,
        app.camera(),
        local,
        SNAP_TOLERANCE_PX,
        &self.snap_kinds,
        self.grid_spacing,
      ) {
        self.snap_marker = Some((snap.kind, snap.point));
        self.cursor_world = Some(snap.point);
      }
    }
  }

  /// Turn pointer, wheel and keyboard input into actions and camera motion.
  ///
  /// Every branch either moves the camera (view state, no undo) or pushes a [`UiAction`] (document
  /// state, undoable). Nothing here mutates the document directly.
  fn handle_input(
    &mut self,
    app: &mut AppState,
    ui: &egui::Ui,
    response: &egui::Response,
    canvas: &CanvasRect,
    output: &mut ShellOutput,
  ) {
    let pointer = response
      .hover_pos()
      .or_else(|| response.interact_pointer_pos());

    // Wheel zooms about the cursor, but only while the canvas has the pointer.
    if response.hovered() {
      let scroll = ui.ctx().input(|input| input.smooth_scroll_delta.y);
      if scroll.abs() > 0.01 {
        self.release_view();
        let anchor = pointer
          .map(|position| canvas.to_screen(position))
          .unwrap_or_else(|| canvas.viewport().center());
        let factor = (1.0 + f64::from(scroll) * 0.002).clamp(0.2, 5.0);
        app.camera_mut().zoom_at(anchor, factor);
      }
    }

    // Middle drag pans; the content follows the cursor.
    if response.dragged_by(egui::PointerButton::Middle) {
      self.release_view();
      let delta = response.drag_delta();
      app
        .camera_mut()
        .pan_by_pixels(f64::from(delta.x), f64::from(delta.y));
    }

    // Primary drag draws a selection marquee.
    if response.drag_started_by(egui::PointerButton::Primary) {
      if let Some(origin) = ui.ctx().input(|input| input.pointer.press_origin()) {
        self.drag_box = Some(canvas.to_screen(origin));
      }
    }
    if response.drag_stopped_by(egui::PointerButton::Primary) {
      if let (Some(start), Some(end)) = (self.drag_box, pointer.map(|p| canvas.to_screen(p))) {
        let scene = build_scene(app.document(), app.selection());
        let hits = box_select(&scene, app.camera(), start, end);
        output.actions.push(UiAction::SetSelection(hits));
      }
      self.drag_box = None;
    }
    if response.dragged_by(egui::PointerButton::Primary) && self.drag_box.is_none() {
      // A drag that started outside the canvas still deserves a marquee.
      self.drag_box = pointer.map(|position| canvas.to_screen(position));
    }

    // A plain click selects what is under the cursor, or clears the selection.
    if response.clicked() {
      if let Some(position) = response.interact_pointer_pos() {
        let scene = build_scene(app.document(), app.selection());
        let hit = pick(
          &scene,
          app.camera(),
          canvas.to_screen(position),
          PICK_TOLERANCE_PX,
        );
        output.actions.push(UiAction::SetSelection(
          hit.map(|hit| vec![hit.entity]).unwrap_or_default(),
        ));
      }
    }

    // Keyboard: undo/redo, delete, fit.
    let (undo, redo, delete, fit) = ui.ctx().input(|input| {
      (
        input.modifiers.command && input.key_pressed(egui::Key::Z) && !input.modifiers.shift,
        input.modifiers.command && input.key_pressed(egui::Key::Z) && input.modifiers.shift,
        input.key_pressed(egui::Key::Delete) || input.key_pressed(egui::Key::Backspace),
        input.key_pressed(egui::Key::F) && !input.modifiers.command,
      )
    });
    if undo {
      output.actions.push(UiAction::Undo);
    }
    if redo {
      output.actions.push(UiAction::Redo);
    }
    if delete {
      output.actions.push(UiAction::DeleteSelection);
    }
    if fit {
      output.actions.push(UiAction::ZoomToFit);
    }
  }

  fn paint(
    &mut self,
    painter: egui::Painter,
    canvas: &CanvasRect,
    app: &AppState,
    output: &mut ShellOutput,
  ) {
    let camera = *app.camera();
    self.grid_spacing = cam_view::nice_spacing(camera.pixels_per_mm(), TARGET_GRID_PIXELS);

    let grid = build_grid(&camera, TARGET_GRID_PIXELS);
    output.drawn.painted_grid_lines = grid.len();
    for item in &grid.items {
      paint_item(&painter, canvas, &camera, item, 1.0);
    }

    // The stock outline, so the user can see where the material ends.
    let job = &app.document().job;
    paint_world_rect(
      &painter,
      canvas,
      &camera,
      Point2::new(0.0, 0.0),
      Point2::new(job.width.mm(), job.height.mm()),
    );

    let scene = build_scene(app.document(), app.selection());
    output.drawn.painted_items = scene.len();
    for item in &scene.items {
      paint_item(&painter, canvas, &camera, item, 1.6);
    }

    if let Some((_, point)) = self.snap_marker {
      let position = canvas.to_window(camera.world_to_screen(point));
      let size = 5.0;
      painter.line_segment(
        [
          Pos2::new(position.x - size, position.y),
          Pos2::new(position.x + size, position.y),
        ],
        Stroke::new(1.5, palette::SNAP),
      );
      painter.line_segment(
        [
          Pos2::new(position.x, position.y - size),
          Pos2::new(position.x, position.y + size),
        ],
        Stroke::new(1.5, palette::SNAP),
      );
    }

    if let Some(start) = self.drag_box {
      if let Some(end) = self.cursor_world.map(|world| camera.world_to_screen(world)) {
        let from = canvas.to_window(start);
        let to = canvas.to_window(end);
        let marquee = egui::Rect::from_two_pos(from, to);
        painter.rect_stroke(
          marquee,
          0.0,
          Stroke::new(1.0, palette::SNAP),
          egui::StrokeKind::Inside,
        );
      }
    }

    painter.text(
      Pos2::new(canvas.x as f32 + 8.0, canvas.y as f32 + 8.0),
      egui::Align2::LEFT_TOP,
      format!("{:.2} px/mm", camera.pixels_per_mm()),
      egui::FontId::monospace(12.0),
      palette::GEOMETRY,
    );
  }
}

/// Paint one scene item, flattening arcs at half a pixel.
fn paint_item(
  painter: &egui::Painter,
  canvas: &CanvasRect,
  camera: &Camera,
  item: &SceneItem,
  width: f32,
) {
  let colour = match item.style() {
    Style::Normal => palette::GEOMETRY,
    Style::Selected => palette::SELECTED,
    Style::Construction => palette::JOB,
    Style::Grid => palette::GRID,
  };
  let points: Vec<Pos2> = match item {
    SceneItem::Polyline { points, .. } => points
      .iter()
      .map(|point| canvas.to_window(camera.world_to_screen(*point)))
      .collect(),
    SceneItem::Arc {
      center,
      radius,
      start_angle,
      sweep,
      ..
    } => {
      let Ok(arc) = cam_geom::Arc::new(*center, *radius, *start_angle, *sweep) else {
        return;
      };
      // Half a pixel of chord error: the flattening happens here, for this painter, and nowhere
      // earlier in the pipeline.
      let tolerance_mm = 0.5 / camera.pixels_per_mm();
      arc
        .tessellate(tolerance_mm)
        .into_iter()
        .map(|point| canvas.to_window(camera.world_to_screen(point)))
        .collect()
    }
  };
  if points.len() >= 2 {
    painter.add(egui::Shape::line(points, Stroke::new(width, colour)));
  }
}

/// Paint a world-space rectangle, used for the stock outline.
fn paint_world_rect(
  painter: &egui::Painter,
  canvas: &CanvasRect,
  camera: &Camera,
  min: Point2,
  max: Point2,
) {
  let corners = [
    Point2::new(min.x, min.y),
    Point2::new(max.x, min.y),
    Point2::new(max.x, max.y),
    Point2::new(min.x, max.y),
    Point2::new(min.x, min.y),
  ];
  let points: Vec<Pos2> = corners
    .iter()
    .map(|point| canvas.to_window(camera.world_to_screen(*point)))
    .collect();
  painter.add(egui::Shape::line(points, Stroke::new(1.0, palette::JOB)));
}

/// Apply one action to the application state.
///
/// The only place the shell touches [`AppState`], so a test can drive the whole interface by feeding
/// actions.
pub fn handle_action(app: &mut AppState, action: &UiAction) -> Result<(), String> {
  match action {
    UiAction::Undo => {
      app.undo();
      Ok(())
    }
    UiAction::Redo => {
      app.redo();
      Ok(())
    }
    UiAction::ZoomToFit => {
      let scene = build_scene(app.document(), app.selection());
      let bounds = scene.bounds();
      if bounds.is_empty() {
        return Err("nothing to frame: the document holds no geometry".to_owned());
      }
      app.fit_camera(bounds.expand(5.0), 24.0);
      Ok(())
    }
    UiAction::AddRectangle { width, height } => {
      let layer_id = app
        .document()
        .layers
        .first()
        .map(|layer| layer.id.clone())
        .ok_or_else(|| "the document has no layer to draw into".to_owned())?;
      let centre = app.camera().center();
      let id = app.next_entity_id();
      app
        .execute(Command::AddRectangle {
          layer_id,
          entity_id: id.clone(),
          at: Point2::new(centre.x - width / 2.0, centre.y - height / 2.0),
          width: *width,
          height: *height,
        })
        .map_err(|error| error.to_string())?;
      app.select(vec![id]);
      Ok(())
    }
    UiAction::DeleteSelection => {
      let selected: Vec<String> = app.selection().ids().map(str::to_owned).collect();
      for id in selected {
        app
          .execute(Command::DeleteEntity { entity_id: id })
          .map_err(|error| error.to_string())?;
      }
      app.clear_selection();
      Ok(())
    }
    UiAction::SetSelection(ids) => {
      app.select(ids.clone());
      Ok(())
    }
    UiAction::ToggleLayerVisible { layer_id } => {
      let layer = app
        .document()
        .layers
        .iter()
        .find(|layer| layer.id == *layer_id)
        .ok_or_else(|| format!("layer {layer_id:?} does not exist"))?;
      let visible = !layer.visible;
      app
        .execute(Command::SetLayerVisible {
          layer_id: layer_id.clone(),
          visible,
        })
        .map_err(|error| error.to_string())
    }
  }
}
