use cam_app::{AppState, Camera};
use cam_geom::Point2;
use cam_view::{build_grid, build_scene, SceneItem, Style, TARGET_GRID_PIXELS};
use egui::{Pos2, Stroke};

use crate::theme::{Palette, Role};
use crate::{CanvasRect, Shell, ShellOutput, Tool};

impl Shell {
  pub(super) fn paint(
    &mut self,
    painter: egui::Painter,
    canvas: &CanvasRect,
    app: &AppState,
    output: &mut ShellOutput,
  ) {
    let palette = self.palette();
    let camera = *app.camera();
    self.grid_spacing = cam_view::nice_spacing(camera.pixels_per_mm(), TARGET_GRID_PIXELS);

    let grid = build_grid(&camera, TARGET_GRID_PIXELS);
    output.drawn.painted_grid_lines = grid.len();
    for item in &grid.items {
      paint_item(&painter, canvas, &camera, item, 1.0, &palette);
    }

    // The stock outline, so the user can see where the material ends.
    let job = &app.document().board;
    paint_world_rect(
      &painter,
      canvas,
      &camera,
      Point2::new(0.0, 0.0),
      Point2::new(job.width.mm(), job.height.mm()),
      &palette,
    );

    let scene = build_scene(app.document(), app.selection());
    output.drawn.painted_items = scene.len();
    for item in &scene.items {
      paint_item(&painter, canvas, &camera, item, 1.6, &palette);
    }

    if let Some((_, point)) = self.snap_marker {
      let position = canvas.to_window(camera.world_to_screen(point));
      let size = 5.0;
      painter.line_segment(
        [
          Pos2::new(position.x - size, position.y),
          Pos2::new(position.x + size, position.y),
        ],
        Stroke::new(1.5, palette.get(Role::Sel)),
      );
      painter.line_segment(
        [
          Pos2::new(position.x, position.y - size),
          Pos2::new(position.x, position.y + size),
        ],
        Stroke::new(1.5, palette.get(Role::Sel)),
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
          Stroke::new(1.0, palette.get(Role::Sel)),
          egui::StrokeKind::Inside,
        );
      }
    }

    // The draw gesture's preview: the segment for the line-like tools, the gesture box for the rest.
    // It is the gesture's bounds, not the shape, because the shape itself is built by the command
    // layer on release and the shell must not own geometry rules.
    if let (Some(start), Some(current)) = (self.draw_start, self.draw_current) {
      let from = canvas.to_window(camera.world_to_screen(start));
      let to = canvas.to_window(camera.world_to_screen(current));
      if matches!(self.active_tool, Tool::Line | Tool::Polyline) {
        painter.line_segment([from, to], Stroke::new(1.5, palette.get(Role::Sel)));
      } else {
        painter.rect_stroke(
          egui::Rect::from_two_pos(from, to),
          0.0,
          Stroke::new(1.5, palette.get(Role::Sel)),
          egui::StrokeKind::Inside,
        );
      }
    }

    painter.text(
      Pos2::new(canvas.x as f32 + 8.0, canvas.y as f32 + 8.0),
      egui::Align2::LEFT_TOP,
      format!("{:.2} px/mm", camera.pixels_per_mm()),
      egui::FontId::monospace(12.0),
      palette.get(Role::TextDim),
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
  palette: &Palette,
) {
  // Four roles carry the whole scene vocabulary: what is drawn, what is selected, what is only
  // construction, and the grid. A fifth would be a fifth kind of geometry.
  let colour = match item.style() {
    Style::Normal => palette.get(Role::Text),
    Style::Selected => palette.get(Role::Sel),
    Style::Construction => palette.get(Role::TextDim),
    Style::Grid => palette.get(Role::Hairline),
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
  palette: &Palette,
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
  painter.add(egui::Shape::line(
    points,
    Stroke::new(1.0, palette.get(Role::Border)),
  ));
}
