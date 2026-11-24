//! The stock as 3D geometry: a decimated surface mesh, its depth ramp, and the stock box (M5 A9).
//!
//! The mesh is **data, not paint** (D-M5-7). The shell hands [`SurfaceMesh`] to `egui::Mesh`; a
//! `wgpu` backend would consume the same vertices and triangles when S12's trigger fires. Kept here,
//! the decimation, the shading ramp and the stock's edges are all testable without a GPU.
//!
//! # Decimation
//!
//! [`build_surface`] samples every `stride`-th grid line, and always samples the last one, so the
//! triangle count is bounded by `stride` rather than by the job's cell count (S7, D-M5-7) while the
//! surface still spans the whole stock. For `columns` and `rows` cells the sampled lattice is
//! `ceil((columns - 1) / stride) + 1` by `ceil((rows - 1) / stride) + 1` vertices, which is exactly
//! the field when `stride == 1`, and the triangles are two per sampled cell pair.
//!
//! # Determinism
//!
//! Nothing here reads a clock, a random source or a hash map: the same field, camera and stride give
//! the same vertices, in the same order (S9).

use cam_model::Job;
use cam_move::Point3;
use cam_sim::Heightfield;

use crate::camera3d::Camera3d;

/// A straight edge in machine space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line3 {
  /// Where the edge starts.
  pub start: Point3,
  /// Where the edge ends.
  pub end: Point3,
}

impl Line3 {
  /// An edge from `start` to `end`.
  pub const fn new(start: Point3, end: Point3) -> Self {
    Self { start, end }
  }
}

/// One vertex of the stock's surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceVertex {
  /// The cell's centre in machine space, at the remaining surface height.
  pub position: Point3,
  /// How deep into the stock this vertex is, from `0` at the untouched top to `1` at the bottom.
  ///
  /// This is the colour ramp's coordinate, not a distance in millimetres: a cell cut half way
  /// through an 18 mm sheet and one cut half way through a 6 mm sheet both read `0.5`, so the same
  /// palette says the same thing about the shape.
  pub depth: f32,
}

/// The stock's surface as triangles, ready to be painted.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SurfaceMesh {
  /// The vertices, in row-major order over the sampled lattice.
  pub vertices: Vec<SurfaceVertex>,
  /// Triangles as indices into [`SurfaceMesh::vertices`], farthest from the camera first.
  pub triangles: Vec<[usize; 3]>,
}

impl SurfaceMesh {
  /// How many triangles the mesh holds.
  pub fn triangle_count(&self) -> usize {
    self.triangles.len()
  }

  /// True when there is nothing to paint.
  pub fn is_empty(&self) -> bool {
    self.triangles.is_empty()
  }
}

/// Build the stock's surface from a simulated field.
///
/// `stride` decimates the grid: `1` samples every cell, `4` every fourth, and a value of `0` is
/// treated as `1` (a stride is a decimation factor, never a divisor). The far edge of the stock is
/// always sampled, so a coarse mesh still reaches the stock's boundary.
///
/// Triangles are emitted back to front for the camera, because `egui::Mesh` has no depth buffer; the
/// cameras' own depth decides that order, and a stable sort keeps ties in lattice order so two builds
/// agree.
pub fn build_surface(field: &Heightfield, camera: &Camera3d, stride: usize) -> SurfaceMesh {
  let stride = stride.max(1);
  let grid = field.grid();
  let columns = sampled(grid.columns, stride);
  let rows = sampled(grid.rows, stride);
  let thickness = field.thickness().mm();

  let mut vertices = Vec::with_capacity(columns.len() * rows.len());
  for &row in &rows {
    for &column in &columns {
      let centre = grid.centre(column, row);
      let height = field.surface(column, row);
      vertices.push(SurfaceVertex {
        position: Point3::new(centre.x, centre.y, height),
        depth: depth_fraction(height, thickness),
      });
    }
  }

  let mut triangles = Vec::new();
  let width = columns.len();
  for row in 0..rows.len().saturating_sub(1) {
    for column in 0..width.saturating_sub(1) {
      let lower_left = row * width + column;
      let lower_right = lower_left + 1;
      let upper_left = lower_left + width;
      let upper_right = upper_left + 1;
      // Counter-clockwise seen from above (world +Z): the winding the stock's top face has, so a
      // painter that ever culls back faces culls the underside and not the part.
      triangles.push([lower_left, lower_right, upper_right]);
      triangles.push([lower_left, upper_right, upper_left]);
    }
  }

  let mut ordered: Vec<(f64, [usize; 3])> = Vec::with_capacity(triangles.len());
  for triangle in triangles {
    let depth = triangle
      .iter()
      .map(|index| {
        camera
          .project(vertices[*index].position)
          .map_or(f64::NEG_INFINITY, |screen| screen.depth)
      })
      .sum::<f64>()
      / 3.0;
    ordered.push((depth, triangle));
  }
  ordered.sort_by(|left, right| left.0.total_cmp(&right.0));

  SurfaceMesh {
    vertices,
    triangles: ordered.into_iter().map(|(_, triangle)| triangle).collect(),
  }
}

/// The stock's twelve edges, as 3D lines.
///
/// The stock's top is `z = 0` and its bottom `z = -thickness` (S13), because the strategies emit
/// cutting moves as negative Z and the raw stock is what `cam-sim` starts from. The bottom face is
/// listed first, then the top, then the four verticals; corners run counter-clockwise from the
/// origin, so the order is the same on every call.
pub fn build_stock_box(job: &Job) -> Vec<Line3> {
  let (width, height) = (job.width.mm(), job.height.mm());
  let bottom = -job.thickness.mm();
  let corners = |z: f64| {
    [
      Point3::new(0.0, 0.0, z),
      Point3::new(width, 0.0, z),
      Point3::new(width, height, z),
      Point3::new(0.0, height, z),
    ]
  };
  let floor = corners(bottom);
  let ceiling = corners(0.0);

  let mut edges = Vec::with_capacity(12);
  for index in 0..4 {
    let next = (index + 1) % 4;
    edges.push(Line3::new(floor[index], floor[next]));
  }
  for index in 0..4 {
    let next = (index + 1) % 4;
    edges.push(Line3::new(ceiling[index], ceiling[next]));
  }
  for index in 0..4 {
    edges.push(Line3::new(floor[index], ceiling[index]));
  }
  edges
}

/// The grid lines to sample for a lattice of `count` cells at `stride`.
///
/// Always includes the first and the last line, so a coarse mesh still covers the stock's full extent
/// instead of stopping up to `stride - 1` cells short of the far edge.
fn sampled(count: usize, stride: usize) -> Vec<usize> {
  if count < 2 {
    return (0..count).collect();
  }
  let mut lines: Vec<usize> = (0..count).step_by(stride).collect();
  if lines.last().copied() != Some(count - 1) {
    lines.push(count - 1);
  }
  lines
}

/// How far into the stock a surface height sits, as the colour ramp's `0.0..=1.0` coordinate.
///
/// `0.0` is the untouched top and `1.0` the bottom, exactly at both ends: the palette's two ends are
/// the two surfaces an operator cares about. A stock of no thickness, or a height that is not a
/// number, reads as the top rather than as a NaN the painter would have to defend against.
fn depth_fraction(height: f64, thickness: f64) -> f32 {
  if !height.is_finite() || !thickness.is_finite() || thickness <= 0.0 {
    return 0.0;
  }
  let fraction = (-height / thickness).clamp(0.0, 1.0);
  if fraction > 0.0 {
    fraction as f32
  } else {
    // `-0.0` compares equal to `0.0` but is the wrong end of the ramp to hand a shader.
    0.0
  }
}
