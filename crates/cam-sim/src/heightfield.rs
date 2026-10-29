//! The stock as a field of remaining surface heights, and the volumes it reports (M5 A1, A2, A3).
//!
//! The surface is held in millimetres in machine Z: the stock's top is `0`, a cut cell is negative,
//! and the bottom is `-thickness` (S13). A cell is never raised and never pushed below the bottom, so
//! every value in the field is a height a real tool could leave behind.

use cam_geom::Point2;
use cam_model::Job;
use cam_units::Length;

use crate::grid::{grid_for, Grid, GridError};

/// What the field reports about itself: the arithmetic `cam-cli simulate` prints.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Volumes {
  /// Material removed so far, in cubic millimetres.
  pub removed: f64,
  /// Material still standing, in cubic millimetres.
  pub remaining: f64,
  /// Cells that reached the stock bottom, and so will drop out of the part.
  pub through_cells: usize,
}

/// The remaining stock, one surface height per cell.
#[derive(Clone, Debug, PartialEq)]
pub struct Heightfield {
  grid: Grid,
  thickness: Length,
  surface: Vec<f64>,
}

impl Heightfield {
  /// A field of untouched stock for `job`, on a grid of `cell` millimetres.
  pub fn new(job: &Job, cell: Length) -> Result<Self, GridError> {
    let grid = grid_for(job, cell)?;
    Ok(Self {
      grid,
      thickness: job.thickness,
      surface: vec![0.0; grid.cells()],
    })
  }

  /// The lattice this field lives on.
  pub fn grid(&self) -> Grid {
    self.grid
  }

  /// The stock thickness, which bounds every cut.
  pub fn thickness(&self) -> Length {
    self.thickness
  }

  /// The remaining surface height at a cell, in millimetres.
  ///
  /// # Panics
  ///
  /// Panics when the cell is outside the grid; callers iterate the grid they were given.
  pub fn surface(&self, column: usize, row: usize) -> f64 {
    self.surface[row * self.grid.columns + column]
  }

  /// The remaining surface height at a cell, or `None` when the cell is outside the grid.
  pub fn surface_at(&self, column: usize, row: usize) -> Option<f64> {
    if column >= self.grid.columns || row >= self.grid.rows {
      return None;
    }
    Some(self.surface[row * self.grid.columns + column])
  }

  /// The remaining surface height at the cell containing `point`, or `None` outside the grid.
  pub fn surface_under(&self, point: Point2) -> Option<f64> {
    let cell = self.grid.cell.mm();
    let column = ((point.x - self.grid.origin.x) / cell).floor();
    let row = ((point.y - self.grid.origin.y) / cell).floor();
    if !column.is_finite() || !row.is_finite() || column < 0.0 || row < 0.0 {
      return None;
    }
    let (column, row) = (column as usize, row as usize);
    if column >= self.grid.columns || row >= self.grid.rows {
      return None;
    }
    Some(self.surface[row * self.grid.columns + column])
  }

  /// The centre of a cell in world millimetres.
  pub fn cell_centre(&self, column: usize, row: usize) -> Point2 {
    self.grid.centre(column, row)
  }

  /// Lower a cell to `z`, keeping whichever is deeper, and never going below the stock bottom.
  ///
  /// # Panics
  ///
  /// Panics when the cell is outside the grid.
  pub fn lower(&mut self, column: usize, row: usize, z: f64) {
    assert!(
      column < self.grid.columns && row < self.grid.rows,
      "cell ({column}, {row}) is outside a {} x {} grid",
      self.grid.columns,
      self.grid.rows
    );
    let index = row * self.grid.columns + column;
    let floor = -self.thickness.mm();
    let target = z.clamp(floor, 0.0);
    if target < self.surface[index] {
      self.surface[index] = target;
    }
  }

  /// The removed, remaining and through-cut totals, summed in row-major order (S9).
  pub fn volumes(&self) -> Volumes {
    let area = self.grid.cell_area();
    let floor = -self.thickness.mm();
    let mut removed = 0.0;
    let mut through_cells = 0;
    for height in &self.surface {
      let depth = -height;
      if depth > 0.0 {
        removed += depth * area;
      }
      if *height <= floor {
        through_cells += 1;
      }
    }
    Volumes {
      removed,
      remaining: self.grid.volume(self.thickness) - removed,
      through_cells,
    }
  }
}
