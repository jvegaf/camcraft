//! The 3D preview panel: a simulated stock, its camera, and the painter's mesh (M5 A9, A11, A12).
//!
//! # What this is, and what it is not
//!
//! The panel **shows** a program; it never authors one and never touches the document (D-M5-9, S3:
//! "La ventana muestra el preview del programa ya autorado por `cam-cli`"). The caller simulates —
//! [`cam_sim::Heightfield`] plus [`cam_sim::sweep::apply`] — and hands the result in through
//! [`Shell::set_preview`](crate::Shell::set_preview) together with the [`Job`] the stock box is built
//! from. The shell's only job is to draw it, so the panel adds no [`UiAction`](crate::UiAction) and
//! no path from the picture back into the geometry.
//!
//! # Why the geometry is not computed here
//!
//! The projection ([`Camera3d`]) and the surface ([`build_surface`]) are data, tested headless in
//! `cam-view` (A9, S2). This module is the painter: it turns that data into [`egui::Mesh`] triangles
//! and lines on the existing [`egui::Painter`], which is why the 3D view needs no new dependency
//! (A11, D-M5-7). A `wgpu` backend would consume the same [`cam_view::SurfaceMesh`] when S12's
//! trigger fires; nothing here would have to change to allow it.

use cam_app::Viewport;
use cam_model::Job;
use cam_move::{Move, MoveStream, Point3};
use cam_sim::Heightfield;
use cam_units::Length;
use cam_view::{build_stock_box, build_surface, Camera3d, Line3, FIELD_OF_VIEW, MIN_DISTANCE};
use egui::{Color32, Pos2, Stroke};

use crate::DrawnState;

/// Height of the 3D region, in pixels.
///
/// Fixed rather than proportional: the panel is a preview the user opens on top of the 2D canvas, and
/// a region that resized with the window would make the camera's framing jump on every resize.
pub const PREVIEW_HEIGHT: f32 = 320.0;

/// How far the camera turns per pixel of drag, in radians.
///
/// A tenth of a radian per hundred pixels: a full turn is about 630 px of dragging, which is the
/// "pick it up and turn it" feel of a modelling view rather than the twitchy one of a trackball.
pub const ORBIT_RADIANS_PER_PIXEL: f64 = 0.01;

/// The largest number of grid lines the surface samples along its longer side.
///
/// The mesh's triangle count is bounded by the decimation stride, not by the job's cell count (S7,
/// D-M5-7): `200` lines give at most 199 x 199 x 2 triangles, which the CPU painter draws in one
/// frame for any stock A6's cell cap admits.
pub const MAX_LATTICE: usize = 200;

/// How much larger than the stock's diagonal the camera's distance is when the panel opens, so the
/// stock sits inside the picture with a margin rather than touching its edges.
const FRAME_MARGIN: f64 = 1.4;

/// How much of a pixel an arc may deviate by when the painter flattens it, in pixels.
///
/// The same half-pixel rule the 2D canvas uses: the flattening belongs to the painter, and the
/// tolerance is expressed in what the user can see, not in millimetres.
const ARC_TOLERANCE_PX: f64 = 0.5;

/// Colours, in one place so the picture stays consistent with the 2D canvas.
mod palette {
  use egui::Color32;

  /// The stock's edges.
  pub const STOCK: Color32 = Color32::from_rgb(88, 132, 178);
  /// Cutting moves of the previewed program.
  pub const CUT: Color32 = Color32::from_rgb(255, 176, 59);
  /// Rapids and retracts: present, but not the thing being shown.
  pub const TRAVEL: Color32 = Color32::from_rgb(96, 104, 116);
  /// The untouched top of the stock, at the `0.0` end of the depth ramp.
  pub const SURFACE_TOP: Color32 = Color32::from_rgb(214, 220, 230);
  /// The stock's bottom, at the `1.0` end of the depth ramp.
  pub const SURFACE_BOTTOM: Color32 = Color32::from_rgb(38, 74, 122);
}

/// One program's preview: the simulated stock, the camera that looks at it, and where playback stands.
///
/// Built by [`Preview3d::new`] and handed to the shell by
/// [`Shell::set_preview`](crate::Shell::set_preview). The field is the caller's simulation, not the
/// panel's: nothing in this module lowers a cell.
#[derive(Clone, Debug, PartialEq)]
pub struct Preview3d {
  /// The simulated stock: the heights the surface is drawn from.
  field: Heightfield,
  /// The program the surface came from, drawn as 3D lines.
  program: MoveStream,
  /// Where the eye is.
  camera: Camera3d,
  /// The stock's twelve edges, built once from the job so the panel does not need it later.
  stock: Vec<Line3>,
  /// Decimation factor for the surface mesh.
  stride: usize,
  /// V3's playback cursor: how many moves of [`Preview3d::program`] have been applied.
  ///
  /// It is here, at `0`, because V3 needs it to live with the rest of the preview's state, and it is
  /// **not advanced** by anything in this module. When V3 lands, the play/pause/step controls move
  /// this cursor and re-run the sweep over a copy of the untouched field; the panel then redraws
  /// without knowing any of that happened, because it only ever reads
  /// [`Preview3d::field`]. Deliberately not a "fake" timer: a cursor nothing can move is honest
  /// state, an animation that stutters against no clock is not.
  cursor: usize,
}

impl Preview3d {
  /// A preview of `field` and `program` over the stock `job` describes.
  ///
  /// `field` must already be the simulation of `program` — the caller owns the simulation (D-M5-9) —
  /// and `job` supplies only the stock box's extent. The camera is framed on the stock with a margin
  /// and the stride is chosen so the mesh's triangle count is bounded (see [`MAX_LATTICE`]).
  pub fn new(field: Heightfield, program: MoveStream, job: &Job) -> Self {
    let stride = stride_for(&field);
    let stock = build_stock_box(job);
    let camera = frame_camera(job);
    Self {
      field,
      program,
      camera,
      stock,
      stride,
      cursor: 0,
    }
  }

  /// The simulated stock this preview draws.
  pub fn field(&self) -> &Heightfield {
    &self.field
  }

  /// The program this preview draws.
  pub fn program(&self) -> &MoveStream {
    &self.program
  }

  /// The camera, for a caller that wants to frame or test the view.
  pub fn camera(&self) -> &Camera3d {
    &self.camera
  }

  /// The camera, for a caller that sets a field directly instead of going through a gesture.
  pub fn camera_mut(&mut self) -> &mut Camera3d {
    &mut self.camera
  }

  /// The stock's edges, in machine space.
  pub fn stock(&self) -> &[Line3] {
    &self.stock
  }

  /// The surface mesh's decimation factor; `1` samples every cell.
  pub fn stride(&self) -> usize {
    self.stride
  }

  /// Set the surface mesh's decimation factor. `0` is read as `1`, never as a divisor.
  pub fn set_stride(&mut self, stride: usize) {
    self.stride = stride.max(1);
  }

  /// V3's playback cursor, in moves. Always `0` until V3 advances it.
  pub fn cursor(&self) -> usize {
    self.cursor
  }

  /// Paint the preview into a region of `ui`, and take the orbit and zoom gestures it owns.
  pub fn show(&mut self, ui: &mut egui::Ui, drawn: &mut DrawnState) {
    // A panel squeezed narrower than nothing paints nothing rather than allocating a negative
    // region: `egui` asserts on that, and a window that small is a layout in progress.
    let size = egui::vec2(ui.available_width().max(0.0), PREVIEW_HEIGHT);
    let (response, painter) = ui.allocate_painter(size, egui::Sense::click_and_drag());
    let rect = response.rect;
    // The camera projects into its own viewport, so it learns this frame's size before anything is
    // drawn; `cam-view` owns the arithmetic and knows nothing about egui's rectangles.
    self.camera.viewport = Viewport::new(f64::from(rect.width()), f64::from(rect.height()));
    self.take_input(ui, &response);
    self.paint(&painter, rect, drawn);
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

  /// Paint the surface, then the stock's edges, then the program.
  ///
  /// That order is the only depth ordering `egui::Mesh` can be given: it has no depth buffer, so the
  /// surface's own triangles arrive back to front from [`build_surface`] and the lines are drawn on
  /// top of a picture that is already correct.
  fn paint(&self, painter: &egui::Painter, rect: egui::Rect, drawn: &mut DrawnState) {
    self.paint_surface(painter, rect, drawn);
    self.paint_stock(painter, rect, drawn);
    self.paint_program(painter, rect, drawn);
  }

  /// Convert the surface mesh into one [`egui::Mesh`], shaded by each vertex's depth (A11).
  ///
  /// A triangle with a vertex the camera cannot project is skipped, not clamped: drawing it would
  /// mean inventing a screen position, and the surface would then say something the field does not.
  fn paint_surface(&self, painter: &egui::Painter, rect: egui::Rect, drawn: &mut DrawnState) {
    let surface = build_surface(&self.field, &self.camera, self.stride);
    if surface.is_empty() {
      return;
    }
    let mut mesh = egui::Mesh::default();
    for triangle in &surface.triangles {
      let mut corners = [Pos2::ZERO; 3];
      let mut colours = [Color32::BLACK; 3];
      let mut visible = true;
      for (slot, index) in triangle.iter().enumerate() {
        let vertex = surface.vertices[*index];
        match self.screen(rect, vertex.position) {
          Some(position) => {
            corners[slot] = position;
            colours[slot] = depth_colour(vertex.depth);
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
          color: colours[corner],
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
  fn paint_stock(&self, painter: &egui::Painter, rect: egui::Rect, drawn: &mut DrawnState) {
    for line in &self.stock {
      let (Some(start), Some(end)) = (self.screen(rect, line.start), self.screen(rect, line.end))
      else {
        continue;
      };
      painter.line_segment([start, end], Stroke::new(1.0, palette::STOCK));
      drawn.painted_3d_lines += 1;
    }
  }

  /// Paint the program's moves as projected 3D polylines.
  ///
  /// Arcs are flattened here, at half a pixel, for the same reason the 2D canvas flattens them: the
  /// painter owns the flattening, and every earlier stage keeps the arc as an arc. A movement with an
  /// end the camera cannot project is dropped rather than drawn through a made-up point. The
  /// segment's first move starts a polyline instead of being joined to the previous toolpath: the
  /// stream carries no start position, and inventing one would draw a line the program never made.
  fn paint_program(&self, painter: &egui::Painter, rect: egui::Rect, drawn: &mut DrawnState) {
    let tolerance = self.flatten_tolerance_mm();
    for segment in &self.program.segments {
      let mut previous: Option<Point3> = None;
      for movement in &segment.moves {
        let to = movement.destination();
        let stroke = match movement {
          Move::Feed { .. } | Move::Arc { .. } => Stroke::new(1.6, palette::CUT),
          Move::Rapid { .. } | Move::Retract { .. } => Stroke::new(1.0, palette::TRAVEL),
        };
        if let Some(from) = previous {
          for pair in path_points(from, *movement, tolerance) {
            let (Some(start), Some(end)) = (self.screen(rect, pair.0), self.screen(rect, pair.1))
            else {
              continue;
            };
            painter.line_segment([start, end], stroke);
            drawn.painted_3d_lines += 1;
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

/// The decimation factor that keeps the mesh's lattice within [`MAX_LATTICE`] lines per side.
///
/// A grid smaller than that is sampled cell for cell: decimating a mesh already inside the budget
/// would only lose detail.
fn stride_for(field: &Heightfield) -> usize {
  let grid = field.grid();
  grid.columns.max(grid.rows).div_ceil(MAX_LATTICE).max(1)
}

/// A camera framed on the stock `job` describes, at the default three-quarter viewpoint.
///
/// The target is the stock's centre, half way down its thickness, so orbiting turns the material
/// rather than swinging it around the table. A degenerate job — no width, or a value that is not a
/// number — lands on [`MIN_DISTANCE`], where the camera's own projection rule reports that nothing
/// can be drawn instead of dividing by zero.
fn frame_camera(job: &Job) -> Camera3d {
  let (width, height, thickness) = (job.width.mm(), job.height.mm(), job.thickness.mm());
  let target = Point3::new(width / 2.0, height / 2.0, -thickness / 2.0);
  let mut camera = Camera3d::new(target, Viewport::new(0.0, 0.0));
  let diagonal = (width * width + height * height + thickness * thickness).sqrt();
  let distance = if diagonal.is_finite() {
    (diagonal * FRAME_MARGIN).max(MIN_DISTANCE.mm())
  } else {
    MIN_DISTANCE.mm()
  };
  camera.distance = Length::from_mm(distance);
  camera
}

/// The straight edges a movement contributes, in machine space.
///
/// A feed, rapid or retract is one edge. An arc is flattened into the chords that stay within
/// `tolerance` of it, with its Z interpolated along the way — the arc model is planar in XY, so the
/// height comes from the movement, not from the circle. A degenerate arc falls back to its chord
/// rather than disappearing: showing where the tool went is more useful than showing nothing.
fn path_points(from: Point3, movement: Move, tolerance: f64) -> Vec<(Point3, Point3)> {
  let to = movement.destination();
  let Move::Arc { center, ccw, .. } = movement else {
    return vec![(from, to)];
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
fn arc_sweep(
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

/// The colour a vertex with this depth is painted with.
///
/// `depth` is the mesh's own ramp coordinate, `0.0` at the untouched top and `1.0` at the stock's
/// bottom, so the same palette says the same thing about an 18 mm sheet and a 6 mm one. A value that
/// is not a number reads as the top rather than as a colour a shader would have to defend against.
fn depth_colour(depth: f32) -> Color32 {
  let t = if depth.is_finite() {
    depth.clamp(0.0, 1.0)
  } else {
    0.0
  };
  let mix = |top: u8, bottom: u8| -> u8 {
    let top = f32::from(top);
    let bottom = f32::from(bottom);
    (top + (bottom - top) * t).round().clamp(0.0, 255.0) as u8
  };
  Color32::from_rgb(
    mix(palette::SURFACE_TOP.r(), palette::SURFACE_BOTTOM.r()),
    mix(palette::SURFACE_TOP.g(), palette::SURFACE_BOTTOM.g()),
    mix(palette::SURFACE_TOP.b(), palette::SURFACE_BOTTOM.b()),
  )
}
