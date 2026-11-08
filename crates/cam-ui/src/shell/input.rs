use cam_app::{AppState, ScreenPoint};
use cam_geom::Point2;
use cam_view::{
  box_select, build_scene, choose_snap, pick, Snap, PICK_TOLERANCE_PX, SNAP_TOLERANCE_PX,
};
use egui::{Context, Pos2};

use crate::{
  CanvasRect, DrawShape, Shell, ShellOutput, Tool, UiAction, DEFAULT_TOLERANCE_MM, MIN_DRAW_DRAG_PX,
};

impl Shell {
  /// Follow the pointer: cursor world position and the snap point it is near.
  ///
  /// Runs before the panels so the status bar shows this frame's coordinates, using the canvas
  /// rectangle remembered from the last frame.
  pub(super) fn track_pointer(&mut self, app: &mut AppState, ctx: &Context) {
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
    self.snap_marker = self
      .snap_point(app, local)
      .map(|snap| (snap.kind, snap.point));
    if let Some((_, point)) = self.snap_marker {
      self.cursor_world = Some(point);
    }
  }

  /// The snap point within range of a canvas-local position, when snapping is on.
  ///
  /// One implementation for the hover readout and for the draw gesture: a gesture that snapped
  /// differently from the marker the user is looking at would be a lie.
  fn snap_point(&self, app: &AppState, local: ScreenPoint) -> Option<Snap> {
    if !self.snapping {
      return None;
    }
    let scene = build_scene(app.document(), app.selection());
    choose_snap(
      &scene,
      app.camera(),
      local,
      SNAP_TOLERANCE_PX,
      &self.snap_kinds,
      self.grid_spacing,
    )
  }

  /// The world position of a window position, snapped when a snap point is in range.
  fn world_at(&self, app: &AppState, canvas: &CanvasRect, position: Pos2) -> Point2 {
    let local = canvas.to_screen(position);
    let world = app.camera().screen_to_world(local);
    self
      .snap_point(app, local)
      .map(|snap| snap.point)
      .unwrap_or(world)
  }

  /// Turn a primary press, drag and release into exactly one drawing command.
  ///
  /// The gesture is in world millimetres: the press point is one corner and the release point the
  /// opposite one. Nothing is submitted until the release, and a gesture shorter than
  /// [`MIN_DRAW_DRAG_PX`] is a cancel — a zero-size shape is not something the user can see, select
  /// or undo on purpose, so it never becomes a command.
  fn draw_gesture(
    &mut self,
    app: &AppState,
    ui: &egui::Ui,
    response: &egui::Response,
    canvas: &CanvasRect,
    output: &mut ShellOutput,
  ) {
    let pointer = response
      .hover_pos()
      .or_else(|| response.interact_pointer_pos());

    if response.drag_started_by(egui::PointerButton::Primary) {
      self.draw_start = ui
        .ctx()
        .input(|input| input.pointer.press_origin())
        .map(|origin| self.world_at(app, canvas, origin));
    }
    if response.dragged_by(egui::PointerButton::Primary) {
      if self.draw_start.is_none() {
        // A drag that began outside the canvas still has a current point.
        self.draw_start = pointer.map(|position| self.world_at(app, canvas, position));
      }
      self.draw_current = pointer.map(|position| self.world_at(app, canvas, position));
    }
    if response.drag_stopped_by(egui::PointerButton::Primary) {
      let start = self.draw_start.take();
      let end = pointer
        .map(|position| self.world_at(app, canvas, position))
        .or(self.draw_current);
      self.draw_current = None;
      if let (Some(start), Some(end)) = (start, end) {
        let dragged_px = start.distance_to(end) * app.camera().pixels_per_mm();
        if dragged_px >= MIN_DRAW_DRAG_PX {
          match self.draw_shape(start, end) {
            Some(shape) => output.actions.push(UiAction::Draw(shape)),
            // A drag along one axis only: a rectangle, ellipse, arc or star with no height is a
            // zero-size shape, so it is cancelled and the status bar says why.
            None => {
              self.status = format!(
                "{}: drag in both directions to size the shape",
                self.active_tool.label()
              )
            }
          }
        }
      }
    }
    // A plain click with a drawing tool armed draws nothing and selects nothing: press and release in
    // the same place is a cancel.
    if response.clicked() {
      self.draw_start = None;
      self.draw_current = None;
    }
  }

  /// The shape a drag from `start` to `end` describes, or `None` when the gesture has no size.
  ///
  /// The two points are opposite corners of the gesture's box. Rectangles and polylines use them
  /// directly; circles, ellipses, polygons and stars are inscribed in that box, so the centre is the
  /// box centre and the size comes from the smaller side (circle, polygon, star) or from both sides
  /// (ellipse). An arc runs from the start corner to the end corner through the box corner
  /// `(start.x, end.y)`, which is the only third point a two-point drag names unambiguously.
  fn draw_shape(&self, start: Point2, end: Point2) -> Option<DrawShape> {
    let min_x = start.x.min(end.x);
    let max_x = start.x.max(end.x);
    let min_y = start.y.min(end.y);
    let max_y = start.y.max(end.y);
    let width = max_x - min_x;
    let height = max_y - min_y;
    let centre = Point2::new((min_x + max_x) * 0.5, (min_y + max_y) * 0.5);
    let half_smaller = width.min(height) * 0.5;

    match self.active_tool {
      Tool::Select => None,
      // A straight line only needs a direction, so a horizontal or vertical drag is a line.
      Tool::Line => Some(DrawShape::Line {
        from: start,
        to: end,
      }),
      Tool::Polyline => Some(DrawShape::Polyline {
        points: vec![start, end],
        closed: false,
      }),
      Tool::Rectangle => (width > 0.0 && height > 0.0).then_some(DrawShape::Rectangle {
        at: Point2::new(min_x, min_y),
        width,
        height,
      }),
      Tool::Arc => (width > 0.0 && height > 0.0).then_some(DrawShape::Arc {
        start,
        end,
        through: Point2::new(start.x, end.y),
      }),
      Tool::Circle => (half_smaller > 0.0).then_some(DrawShape::Circle {
        at: centre,
        radius: half_smaller,
      }),
      Tool::Ellipse => (width > 0.0 && height > 0.0).then_some(DrawShape::Ellipse {
        at: centre,
        rx: width * 0.5,
        ry: height * 0.5,
        tolerance: DEFAULT_TOLERANCE_MM,
      }),
      Tool::Polygon => (half_smaller > 0.0).then_some(DrawShape::Polygon {
        at: centre,
        radius: half_smaller,
        sides: self.polygon_sides,
        rotation: 0.0,
      }),
      Tool::Star => (half_smaller > 0.0).then_some(DrawShape::Star {
        at: centre,
        outer_radius: half_smaller,
        inner_radius: half_smaller * self.star_inner_ratio,
        points: self.star_points,
        rotation: 0.0,
      }),
    }
  }

  /// Turn pointer, wheel and keyboard input into actions and camera motion.
  ///
  /// Every branch either moves the camera (view state, no undo) or pushes a [`UiAction`] (document
  /// state, undoable). Nothing here mutates the document directly.
  pub(super) fn handle_input(
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

    if self.active_tool.draws() {
      // A drawing tool owns the primary button: press, move and release become one command, and the
      // selection gestures stay out of the way so a draw never clears the selection it just made.
      self.draw_gesture(app, ui, response, canvas, output);
    } else {
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
    }

    // The secondary button scopes the canvas context menu, and that is all it does. Pointing at an entity
    // selects it first, so the command picked from the menu acts on what the operator pointed at; a pointer
    // over empty canvas offers the canvas' own commands instead and leaves the selection alone.
    if response.secondary_clicked() {
      if let Some(position) = response.interact_pointer_pos() {
        let scene = build_scene(app.document(), app.selection());
        let hit = pick(
          &scene,
          app.camera(),
          canvas.to_screen(position),
          PICK_TOLERANCE_PX,
        );
        self.menu_scope = match hit {
          Some(_) => crate::help::MenuScope::Object,
          None => crate::help::MenuScope::Canvas,
        };
        if let Some(hit) = hit {
          if !app.selection().contains(&hit.entity) {
            output
              .actions
              .push(UiAction::SetSelection(vec![hit.entity]));
          }
        }
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
}
