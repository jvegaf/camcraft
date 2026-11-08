use cam_app::{Selection, Viewport};
use cam_move::{Move, Point3, ToolShape};
use cam_view::{build_scene, build_surface, Line3, SceneItem, FIELD_OF_VIEW};
use egui::{Pos2, Stroke};

use crate::theme::{Palette, Role};
use crate::DrawnState;

use super::{Layers, Preview3d, ORBIT_RADIANS_PER_PIXEL};

/// How much of a pixel an arc may deviate by when the painter flattens it, in pixels.
///
/// The same half-pixel rule the 2D canvas uses: the flattening belongs to the painter, and the
/// tolerance is expressed in what the user can see, not in millimetres.
const ARC_TOLERANCE_PX: f64 = 0.5;

/// How long the tool marker's axis is, in millimetres, above the playhead's own point.
///
/// A marker's length and not the tool's: the program carries where the tip is and how wide the cutter is, and
/// no tool length at all, so this is the one dimension of the marker that is the shell's own. Twelve
/// millimetres reads as a tool beside a 6 mm cutter on an 18 mm sheet without pretending to be the spindle.
const TOOL_MARKER_MM: f64 = 12.0;

/// How far apart a vector is allowed to be from its arc when the 3D half projects one, in millimetres.
///
/// A fixed millimetre rather than a pixel, unlike the program's own flattening: a vector is a light overlay on
/// a 3D face, and a tolerance that followed the camera would rebuild the whole document's edges every time the
/// wheel turned.
const VECTOR_TOLERANCE_MM: f64 = 0.1;

impl Preview3d {
  /// Paint the preview into the region the frame hands it, and take the orbit and zoom gestures it owns.
  ///
  /// A8 makes the 3D view a **half** of the canvas rather than the whole of it (camui-prancheta S8), so the
  /// region is an argument: the caller decides where the half is, and the camera learns that region's size
  /// every frame. The camera therefore learns a new viewport on resize, which is what a filling view does.
  /// `palette` is the current theme's colours: the half takes them as an argument instead of holding any of
  /// its own, so a theme flip repaints the stock without the picture knowing a theme exists.
  ///
  /// `vectors` are the document's own edges ([`vector_lines`]) and `layers` is the overlay row's answer to
  /// which of the four things the half draws (S8). Each layer is painted only when it is showing, and the
  /// report counts what reached the painter per layer, so "hidden" and "had nothing to draw" stay different
  /// answers rather than both reading zero in the same field.
  /// Take this frame's orbit and zoom gestures, and learn the region's size.
  ///
  /// Allocation and painting are separated on purpose: the chrome of the half has to be *registered* after this
  /// response (so a press on a chip is not a press on the picture) and *painted* after the picture (so it sits on
  /// top of it). `rect` is the picture's region, which is the camera's viewport for this frame.
  pub fn take_gestures(&mut self, ui: &egui::Ui, response: &egui::Response, rect: egui::Rect) {
    self.camera.viewport = Viewport::new(f64::from(rect.width()), f64::from(rect.height()));
    self.take_input(ui, response);
  }

  /// Paint the picture and report what each of S8's four layers put on the painter.
  ///
  /// `vectors` are the document's own edges ([`vector_lines`]) and `layers` is the overlay row's answer to which
  /// of the four things the half draws. Each layer is painted only when it is showing, and the report counts
  /// what reached the painter per layer, so "hidden" and "had nothing to draw" stay different answers instead of
  /// both reading zero in one field.
  pub fn paint_scene(
    &self,
    painter: &egui::Painter,
    rect: egui::Rect,
    drawn: &mut DrawnState,
    palette: &Palette,
    vectors: &[Line3],
    layers: Layers,
  ) {
    drawn.painted_material_lines = 0;
    drawn.painted_vector_lines = 0;
    drawn.painted_program_lines = 0;
    drawn.painted_tool_lines = 0;
    if layers.material {
      self.paint_surface(painter, rect, drawn, palette);
      self.paint_stock(painter, rect, drawn, palette);
    }
    if layers.vectors {
      self.paint_vectors(painter, rect, drawn, palette, vectors);
    }
    if layers.toolpaths {
      self.paint_program(painter, rect, drawn, palette);
    }
    if layers.tool {
      self.paint_tool(painter, rect, drawn, palette);
    }
    drawn.painted_3d_lines = drawn.painted_material_lines
      + drawn.painted_vector_lines
      + drawn.painted_program_lines
      + drawn.painted_tool_lines;
  }

  /// Turn the camera, and zoom it about the target.
  ///
  /// Both gestures go through [`Camera3d::orbit`] and [`Camera3d::zoom`] rather than assigning the
  /// fields: the pitch clamp and the distance clamp are the camera's rules, and a gesture is not
  /// allowed a second opinion about them.
  fn take_input(&mut self, ui: &egui::Ui, response: &egui::Response) {
    let delta = response.drag_delta();
    if delta.x != 0.0 || delta.y != 0.0 {
      // Dragging right turns the stock to the right and dragging down lowers the eye: the picture
      // follows the hand, the way a pan does on the 2D canvas.
      self.camera.orbit(
        -f64::from(delta.x) * ORBIT_RADIANS_PER_PIXEL,
        -f64::from(delta.y) * ORBIT_RADIANS_PER_PIXEL,
      );
    }
    if response.hovered() {
      let scroll = ui.ctx().input(|input| input.smooth_scroll_delta.y);
      if scroll.abs() > 0.01 {
        let factor = (1.0 + f64::from(scroll) * 0.002).clamp(0.2, 5.0);
        self.camera.zoom(factor);
      }
    }
  }

  /// Paint the document's vectors on the stock's top face (S8's `Vectors` layer).
  ///
  /// The role is the one the 2D canvas paints ordinary geometry in ([`Role::Text`]), because these are the
  /// same vectors: the document's own drawing, not the program and not a signal about either.
  fn paint_vectors(
    &self,
    painter: &egui::Painter,
    rect: egui::Rect,
    drawn: &mut DrawnState,
    palette: &Palette,
    vectors: &[Line3],
  ) {
    let stroke = Stroke::new(1.2, palette.get(Role::Text));
    for line in vectors {
      let (Some(start), Some(end)) = (self.screen(rect, line.start), self.screen(rect, line.end))
      else {
        continue;
      };
      painter.line_segment([start, end], stroke);
      drawn.painted_vector_lines += 1;
    }
  }

  /// Paint the tool at the playhead (S8's `Tool` layer): its axis and its diameter.
  ///
  /// Two edges, and no more, because the picture may not invent a fact the program does not carry: a spindle
  /// drawn to a guessed length would be exactly that. What the program does hold is where the tip is and how
  /// wide the tool is, and those are the two edges here — a marker of the tool, not a model of the machine.
  fn paint_tool(
    &self,
    painter: &egui::Painter,
    rect: egui::Rect,
    drawn: &mut DrawnState,
    palette: &Palette,
  ) {
    let Some(step) = self.step_under_playhead() else {
      return;
    };
    let tip = step.movement.destination();
    let radius = tool_radius(step.tool);
    let stroke = Stroke::new(1.6, palette.get(Role::TextHi));
    let axis = [
      self.screen(rect, tip),
      self.screen(
        rect,
        Point3::new(tip.x.mm(), tip.y.mm(), tip.z.mm() + TOOL_MARKER_MM),
      ),
    ];
    if let [Some(start), Some(end)] = axis {
      painter.line_segment([start, end], stroke);
      drawn.painted_tool_lines += 1;
    }
    let across = [
      self.screen(
        rect,
        Point3::new(tip.x.mm() - radius, tip.y.mm(), tip.z.mm()),
      ),
      self.screen(
        rect,
        Point3::new(tip.x.mm() + radius, tip.y.mm(), tip.z.mm()),
      ),
    ];
    if let [Some(start), Some(end)] = across {
      painter.line_segment([start, end], stroke);
      drawn.painted_tool_lines += 1;
    }
  }

  /// Convert the surface mesh into one [`egui::Mesh`], shaded by each vertex's depth (A11).
  ///
  /// A triangle with a vertex the camera cannot project is skipped, not clamped: drawing it would
  /// mean inventing a screen position, and the surface would then say something the field does not.
  fn paint_surface(
    &self,
    painter: &egui::Painter,
    rect: egui::Rect,
    drawn: &mut DrawnState,
    palette: &Palette,
  ) {
    let surface = build_surface(&self.field, &self.camera, self.stride);
    if surface.is_empty() {
      return;
    }
    let mut mesh = egui::Mesh::default();
    for triangle in &surface.triangles {
      let mut corners = [Pos2::ZERO; 3];
      let mut depths = [0.0_f32; 3];
      let mut visible = true;
      for (slot, index) in triangle.iter().enumerate() {
        let vertex = surface.vertices[*index];
        match self.screen(rect, vertex.position) {
          Some(position) => {
            corners[slot] = position;
            depths[slot] = vertex.depth;
          }
          None => {
            visible = false;
            break;
          }
        }
      }
      if !visible {
        continue;
      }
      let base = mesh.vertices.len() as u32;
      for corner in 0..3 {
        mesh.vertices.push(egui::epaint::Vertex {
          pos: corners[corner],
          uv: egui::epaint::WHITE_UV,
          // Depth becomes value through the theme's ramp: the same field says the same thing about
          // an 18 mm sheet and a 6 mm one, and both ends are roles like every other colour here.
          color: palette.stock_shade(depths[corner]),
        });
      }
      mesh.indices.extend([base, base + 1, base + 2]);
      drawn.painted_triangles += 1;
    }
    if !mesh.indices.is_empty() {
      painter.add(egui::Shape::mesh(mesh));
    }
  }

  /// Paint the stock's twelve edges.
  fn paint_stock(
    &self,
    painter: &egui::Painter,
    rect: egui::Rect,
    drawn: &mut DrawnState,
    palette: &Palette,
  ) {
    for line in &self.stock {
      let (Some(start), Some(end)) = (self.screen(rect, line.start), self.screen(rect, line.end))
      else {
        continue;
      };
      // The silhouette of the material. Not `Border`: a control's hairline divides two surfaces inside
      // one panel, and at 1.8:1 against the canvas it let a dark block's lower edges dissolve into the
      // background. This role is held to 3:1 against both the canvas and the face it bounds.
      // 1.6, the icon set's own line weight, not a hairline: measured on the running app, a 1.0 stroke is
      // antialiased over about two pixels and reaches 2.32:1 against the stock's face where the token
      // promises 4.90:1. A silhouette that carries an object's shape has to survive being rendered.
      painter.line_segment([start, end], Stroke::new(1.6, palette.get(Role::StockEdge)));
      drawn.painted_material_lines += 1;
    }
  }

  /// Paint the program's moves as projected 3D polylines.
  ///
  /// Arcs are flattened here, at half a pixel, for the same reason the 2D canvas flattens them: the
  /// painter owns the flattening, and every earlier stage keeps the arc as an arc. A movement with an
  /// end the camera cannot project is dropped rather than drawn through a made-up point. The
  /// segment's first move starts a polyline instead of being joined to the previous toolpath: the
  /// stream carries no start position, and inventing one would draw a line the program never made.
  fn paint_program(
    &self,
    painter: &egui::Painter,
    rect: egui::Rect,
    drawn: &mut DrawnState,
    palette: &Palette,
  ) {
    let tolerance = self.flatten_tolerance_mm();
    for segment in &self.program.segments {
      let mut previous: Option<Point3> = None;
      for movement in &segment.moves {
        let to = movement.destination();
        // Amber is the cut signal; a rapid's line exists only so the eye can find the cut between
        // them, so it is the secondary role rather than a signal of its own.
        let stroke = match movement {
          Move::Feed { .. } | Move::Arc { .. } | Move::Helix { .. } => {
            Stroke::new(1.6, palette.get(Role::Cut))
          }
          Move::Rapid { .. } | Move::Retract { .. } => Stroke::new(1.0, palette.get(Role::TextDim)),
        };
        if let Some(from) = previous {
          for pair in path_points(from, *movement, tolerance) {
            let (Some(start), Some(end)) = (self.screen(rect, pair.0), self.screen(rect, pair.1))
            else {
              continue;
            };
            painter.line_segment([start, end], stroke);
            drawn.painted_program_lines += 1;
          }
        }
        previous = Some(to);
      }
    }
  }

  /// Half a pixel, expressed in millimetres at the camera's own scale.
  fn flatten_tolerance_mm(&self) -> f64 {
    let distance = self.camera.distance.mm();
    let height = self.camera.viewport.height;
    if !distance.is_finite() || distance <= 0.0 || height <= 0.0 {
      return ARC_TOLERANCE_PX;
    }
    let pixels_per_mm = height / (2.0 * distance * (FIELD_OF_VIEW / 2.0).tan());
    if pixels_per_mm <= 0.0 || !pixels_per_mm.is_finite() {
      return ARC_TOLERANCE_PX;
    }
    ARC_TOLERANCE_PX / pixels_per_mm
  }

  /// Project a machine-space point into window coordinates, or `None` when it cannot be drawn.
  fn screen(&self, rect: egui::Rect, point: Point3) -> Option<Pos2> {
    self.camera.project(point).map(|screen| {
      Pos2::new(
        (f64::from(rect.min.x) + screen.x) as f32,
        (f64::from(rect.min.y) + screen.y) as f32,
      )
    })
  }
}

/// The document's vectors as 3D edges laid on the stock's top face, for S8's `Vectors` layer.
///
/// The projection is the same [`build_scene`] the 2D canvas paints, read as edges: a vector on a hidden layer
/// is hidden here too, so the two pictures cannot disagree about what the document holds. `z = 0` is the
/// stock's top face, which is where a vector is in machine space; arcs are flattened at
/// [`VECTOR_TOLERANCE_MM`] rather than at the painter's pixel tolerance, because this list is built before
/// there is a camera to express a pixel in.
pub fn vector_lines(document: &cam_model::Document) -> Vec<Line3> {
  let scene = build_scene(document, &Selection::default());
  let mut lines = Vec::new();
  for item in &scene.items {
    let points: Vec<cam_geom::Point2> = match item {
      SceneItem::Polyline { points, .. } => points.clone(),
      SceneItem::Arc {
        center,
        radius,
        start_angle,
        sweep,
        ..
      } => match cam_geom::Arc::new(*center, *radius, *start_angle, *sweep) {
        Ok(arc) => arc.tessellate(VECTOR_TOLERANCE_MM),
        // An arc the kernel will not build is dropped rather than replaced with an invented chord: a vector
        // drawn as a line it is not would be a picture of a document that does not exist.
        Err(_) => continue,
      },
    };
    for pair in points.windows(2) {
      lines.push(Line3::new(
        Point3::new(pair[0].x, pair[0].y, 0.0),
        Point3::new(pair[1].x, pair[1].y, 0.0),
      ));
    }
  }
  lines
}

/// Half the width a tool cuts across at its tip, from the shape the program carries.
///
/// A V-bit is measured at its flat: the cone above it widens with depth, and the marker is drawn at the one
/// depth the tool's own numbers describe — its tip.
fn tool_radius(shape: ToolShape) -> f64 {
  match shape {
    ToolShape::Cylinder { diameter } => diameter.mm() / 2.0,
    ToolShape::VBit { flat_diameter, .. } => flat_diameter.mm() / 2.0,
  }
}

/// The straight edges a movement contributes, in machine space.
///
/// A feed, rapid or retract is one edge. An arc or a helix is flattened into the chords that stay within
/// `tolerance` of its circle, with its Z interpolated along the way — the arc model is planar in XY, so
/// the height comes from the movement, not from the circle, which is what draws a helix as the descending
/// spiral it is. A degenerate arc falls back to its chord rather than disappearing: showing where the
/// tool went is more useful than showing nothing.
fn path_points(from: Point3, movement: Move, tolerance: f64) -> Vec<(Point3, Point3)> {
  let to = movement.destination();
  let (center, ccw) = match movement {
    Move::Arc { center, ccw, .. } | Move::Helix { center, ccw, .. } => (center, ccw),
    _ => return vec![(from, to)],
  };
  let radius = center.distance_to(to.xy());
  if !radius.is_finite() || radius <= 0.0 {
    return vec![(from, to)];
  }
  let start_angle = (from.y.mm() - center.y).atan2(from.x.mm() - center.x);
  let sweep = arc_sweep(from.xy(), to.xy(), center, ccw);
  let Ok(arc) = cam_geom::Arc::new(center, radius, start_angle, sweep) else {
    return vec![(from, to)];
  };
  let tolerance = if tolerance.is_finite() && tolerance > 0.0 {
    tolerance
  } else {
    ARC_TOLERANCE_PX
  };
  let points = arc.tessellate(tolerance);
  let (start_z, end_z) = (from.z.mm(), to.z.mm());
  let last = points.len().saturating_sub(1).max(1) as f64;
  let mut edges = Vec::with_capacity(points.len());
  let mut previous: Option<Point3> = None;
  for (index, point) in points.into_iter().enumerate() {
    let t = index as f64 / last;
    let position = Point3::new(point.x, point.y, start_z + (end_z - start_z) * t);
    if let Some(from) = previous {
      edges.push((from, position));
    }
    previous = Some(position);
  }
  edges
}

/// The signed sweep of an arc from `from` to `to` about `center`, in radians.
///
/// The direction comes from the movement's own `ccw` flag, never from the smaller of the two
/// rotations: an arc that goes the long way round must be drawn the long way round, or the picture
/// shows a cut the machine did not make.
pub(super) fn arc_sweep(
  from: cam_geom::Point2,
  to: cam_geom::Point2,
  center: cam_geom::Point2,
  ccw: bool,
) -> f64 {
  let start = (from.y - center.y).atan2(from.x - center.x);
  let end = (to.y - center.y).atan2(to.x - center.x);
  let mut sweep = end - start;
  if ccw {
    while sweep <= 0.0 {
      sweep += std::f64::consts::TAU;
    }
  } else {
    while sweep >= 0.0 {
      sweep -= std::f64::consts::TAU;
    }
  }
  sweep
}
