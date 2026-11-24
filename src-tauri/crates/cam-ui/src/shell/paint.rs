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

    if self.copper_mode {
      paint_copper_fills(&painter, canvas, &camera, app);
    }

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
  // Style still decides the drawing intent, but PCB role gives Normal geometry its identity color so
  // the fabrication layers can be told apart at a glance (E6.1).
  let colour = match item.style() {
    Style::Normal => item.role().map_or(palette.get(Role::Text), |role| {
      let [r, g, b] = role.color_rgb();
      egui::Color32::from_rgb(r, g, b)
    }),
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
        .tessellate(cam_geom::TolerancePolicy::standard().with_chord(tolerance_mm))
        .into_iter()
        .map(|point| canvas.to_window(camera.world_to_screen(point)))
        .collect()
    }
  };
  if points.len() >= 2 {
    painter.add(egui::Shape::line(points, Stroke::new(width, colour)));
  }
}

/// Paint filled copper layers when copper mode is active.
fn paint_copper_fills(
  painter: &egui::Painter,
  canvas: &CanvasRect,
  camera: &Camera,
  app: &AppState,
) {
  for layer in &app.document().layers {
    if !layer.visible || !layer.role.is_copper() {
      continue;
    }
    let [r, g, b] = layer.role.color_rgb();
    let fill = egui::Color32::from_rgba_unmultiplied(r, g, b, 110);
    let stroke = egui::Color32::from_rgb(r, g, b);
    for entity in &layer.entities {
      let Some(contour) = entity.kind.as_contour() else {
        continue;
      };
      let points = flatten_contour_to_screen(contour, canvas, camera);
      if points.len() < 3 {
        continue;
      }
      for triangle in earclip(&points) {
        painter.add(egui::Shape::convex_polygon(
          triangle.to_vec(),
          fill,
          egui::Stroke::NONE,
        ));
      }
      if points.len() >= 2 {
        let mut outline = points.clone();
        if outline.first() != outline.last() {
          if let Some(first) = outline.first().copied() {
            outline.push(first);
          }
        }
        painter.add(egui::Shape::line(outline, egui::Stroke::new(1.0, stroke)));
      }
    }
  }
}

/// Screen-space dedupe threshold: 0.5 px, squared to avoid sqrt.
const SCREEN_DEDUPE_DIST_SQ: f32 = 0.25;

/// Above this point count, ear clipping becomes too expensive for per-frame rendering.
/// Fall back to fan triangulation, which is fast and correct for convex contours; concave
/// contours may get imperfect fill, but an approximate fill is better than dropping it.
const EARCLIP_MAX_POINTS: usize = 96;

fn flatten_contour_to_screen(
  contour: &cam_geom::Contour,
  canvas: &CanvasRect,
  camera: &Camera,
) -> Vec<Pos2> {
  let mut points = Vec::new();
  for segment in contour.segments() {
    let seg_points: Vec<Pos2> = match segment {
      cam_geom::Segment::Line(line) => {
        vec![
          canvas.to_window(camera.world_to_screen(line.start)),
          canvas.to_window(camera.world_to_screen(line.end)),
        ]
      }
      cam_geom::Segment::Arc(arc) => {
        let tolerance_mm = 0.5 / camera.pixels_per_mm().max(f64::EPSILON);
        arc
          .tessellate(cam_geom::TolerancePolicy::standard().with_chord(tolerance_mm))
          .into_iter()
          .map(|point| canvas.to_window(camera.world_to_screen(point)))
          .collect()
      }
    };
    for point in seg_points {
      if points
        .last()
        .is_some_and(|last: &Pos2| last.distance_sq(point) < SCREEN_DEDUPE_DIST_SQ)
      {
        continue;
      }
      points.push(point);
    }
  }
  if points.len() > 1
    && points
      .first()
      .zip(points.last())
      .is_some_and(|(a, b)| a.distance_sq(*b) < SCREEN_DEDUPE_DIST_SQ)
  {
    points.pop();
  }
  points
}

fn earclip(points: &[Pos2]) -> Vec<[Pos2; 3]> {
  if points.len() < 3 {
    return Vec::new();
  }
  let area = signed_area(points);
  if area.abs() < 1.0e-6 {
    return Vec::new();
  }
  if points.len() > EARCLIP_MAX_POINTS && is_convex(points) {
    return fan_triangulate(points);
  }
  let mut indices: Vec<usize> = (0..points.len()).collect();
  if area < 0.0 {
    indices.reverse();
  }
  let mut triangles = Vec::new();
  let mut guard = 0usize;
  let max_guard = indices.len().saturating_mul(indices.len()).max(16);
  while indices.len() > 3 && guard < max_guard {
    guard += 1;
    let mut clipped = false;
    let n = indices.len();
    for i in 0..n {
      let prev = indices[(i + n - 1) % n];
      let curr = indices[i];
      let next = indices[(i + 1) % n];
      let a = points[prev];
      let b = points[curr];
      let c = points[next];
      if cross(a, b, c) <= 0.0 {
        continue;
      }
      let mut has_point = false;
      for &other in &indices {
        if other == prev || other == curr || other == next {
          continue;
        }
        if point_in_triangle(points[other], a, b, c) {
          has_point = true;
          break;
        }
      }
      if has_point {
        continue;
      }
      triangles.push([a, b, c]);
      indices.remove(i);
      clipped = true;
      break;
    }
    if !clipped {
      break;
    }
  }
  if indices.len() == 3 {
    triangles.push([points[indices[0]], points[indices[1]], points[indices[2]]]);
  } else if indices.len() > 3 {
    // Best-effort fallback: keep the copper fill visible instead of dropping it silently.
    let remaining: Vec<Pos2> = indices.iter().map(|&i| points[i]).collect();
    triangles.extend(fan_triangulate(&remaining));
  }
  triangles
}

fn fan_triangulate(points: &[Pos2]) -> Vec<[Pos2; 3]> {
  if points.len() < 3 {
    return Vec::new();
  }
  let mut triangles = Vec::with_capacity(points.len().saturating_sub(2));
  for i in 1..(points.len() - 1) {
    triangles.push([points[0], points[i], points[i + 1]]);
  }
  triangles
}

fn signed_area(points: &[Pos2]) -> f32 {
  let mut area = 0.0f32;
  for i in 0..points.len() {
    let a = points[i];
    let b = points[(i + 1) % points.len()];
    area += a.x * b.y - b.x * a.y;
  }
  area * 0.5
}

fn is_convex(points: &[Pos2]) -> bool {
  let mut sign = 0.0f32;
  let n = points.len();
  for i in 0..n {
    let a = points[i];
    let b = points[(i + 1) % n];
    let c = points[(i + 2) % n];
    let turn = (b.x - a.x) * (c.y - b.y) - (b.y - a.y) * (c.x - b.x);
    if turn.abs() <= 1.0e-6 {
      continue;
    }
    if sign == 0.0 {
      sign = turn;
    } else if sign * turn < 0.0 {
      return false;
    }
  }
  true
}

fn cross(a: Pos2, b: Pos2, c: Pos2) -> f32 {
  (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

fn point_in_triangle(p: Pos2, a: Pos2, b: Pos2, c: Pos2) -> bool {
  let c1 = cross(a, b, p);
  let c2 = cross(b, c, p);
  let c3 = cross(c, a, p);
  let has_neg = c1 < -1.0e-5 || c2 < -1.0e-5 || c3 < -1.0e-5;
  let has_pos = c1 > 1.0e-5 || c2 > 1.0e-5 || c3 > 1.0e-5;
  !(has_neg && has_pos)
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
