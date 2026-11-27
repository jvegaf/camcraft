//! The grid a stock is simulated on: its extent, its cell size and its hard cap (M5 A1, A6).
//!
//! The grid covers the stock in whole cells, so its area can exceed the job's by up to one cell in
//! each direction. That difference is part of the simulation's published coverage bound (D-M5-3);
//! every volume the crate reports is measured on the grid, never on the nominal job rectangle, so
//! `removed + remaining` is exactly the grid's volume.

use cam_geom::Point2;
use cam_model::Job;
use cam_units::Length;
use thiserror::Error;

/// The cell size used when a caller does not choose one, in millimetres.
///
/// One millimetre keeps the default 1219 x 2438 mm sheet under the cap at that sheet's own
/// resolution, and it is the resolution the M5 oracle's error bound is written for.
pub const DEFAULT_CELL: f64 = 1.0;

/// The most cells a single simulation may allocate.
///
/// Four million cells is 32 MB of `f64` surface heights: enough for the default sheet at one
/// millimetre, small enough that a mistyped resolution is refused instead of attempted (A6).
pub const MAX_CELLS: usize = 4_000_000;

/// The cell lattice covering a stock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
  /// The world position of the grid's corner: the stock's own origin.
  pub origin: Point2,
  /// Edge length of one square cell.
  pub cell: Length,
  /// Cells along X.
  pub columns: usize,
  /// Cells along Y.
  pub rows: usize,
}

impl Grid {
  /// The number of cells in the grid.
  pub fn cells(&self) -> usize {
    self.columns * self.rows
  }

  /// The area of one cell in square millimetres.
  pub fn cell_area(&self) -> f64 {
    self.cell.mm() * self.cell.mm()
  }

  /// The volume the grid encloses for a stock of `thickness`.
  pub fn volume(&self, thickness: Length) -> f64 {
    self.cells() as f64 * self.cell_area() * thickness.mm()
  }

  /// The centre of a cell, in world millimetres.
  pub fn centre(&self, column: usize, row: usize) -> Point2 {
    let x = self.origin.x + (column as f64 + 0.5) * self.cell.mm();
    let y = self.origin.y + (row as f64 + 0.5) * self.cell.mm();
    Point2::new(x, y)
  }
}

/// Why a grid could not be built.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum GridError {
  /// The requested cell is so small that the grid would exceed [`MAX_CELLS`].
  #[error(
    "a {cell} mm cell over this stock is {columns} x {rows} = {} cells, over the {cap} cell cap; \
     use a coarser --resolution",
    (*columns as u128) * (*rows as u128)
  )]
  TooFine {
    /// The requested cell size in millimetres.
    cell: f64,
    /// Columns the request implies.
    columns: usize,
    /// Rows the request implies.
    rows: usize,
    /// The cap it exceeded.
    cap: usize,
  },
  /// The stock or the cell size is not a positive, finite number, so there is nothing to simulate.
  #[error("cannot simulate a stock {reason}")]
  Degenerate {
    /// What is wrong, in words.
    reason: String,
  },
}

/// Build the grid that covers `job` in cells of `cell` millimetres.
///
/// Fails, never panics and never allocates, when the request is degenerate or over [`MAX_CELLS`].
pub fn grid_for(job: &Job, cell: Length) -> Result<Grid, GridError> {
  let size = cell.mm();
  if !size.is_finite() || size <= 0.0 {
    return Err(GridError::Degenerate {
      reason: format!("with a {size} mm cell"),
    });
  }
  let width = job.width.mm();
  let height = job.height.mm();
  if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
    return Err(GridError::Degenerate {
      reason: format!("of {width} x {height} mm"),
    });
  }

  let columns = (width / size).ceil();
  let rows = (height / size).ceil();
  let cap = MAX_CELLS as f64;
  if columns > cap || rows > cap || columns * rows > cap {
    return Err(GridError::TooFine {
      cell: size,
      columns: columns as usize,
      rows: rows as usize,
      cap: MAX_CELLS,
    });
  }

  Ok(Grid {
    origin: Point2::new(0.0, 0.0),
    cell: Length::from_mm(size),
    columns: columns as usize,
    rows: rows as usize,
  })
}
