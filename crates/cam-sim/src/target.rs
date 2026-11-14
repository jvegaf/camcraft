//! Target derivation and deviation reporting for PCB operations.
//!
//! PCB operation target derivation is not implemented yet in this task. Consumers that need a
//! derived target receive an explicit diagnostic naming the operation.

use std::collections::BTreeMap;

use cam_model::{Document, Operation};
use cam_move::ToolShape;
use cam_units::Length;
use thiserror::Error;

use crate::grid::{grid_for, Grid, GridError};
use crate::heightfield::Heightfield;

/// The tools a document's toolpaths refer to, by tool number.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ToolShapes {
  shapes: BTreeMap<u32, ToolShape>,
}

impl ToolShapes {
  /// Build the table from `(tool number, shape)` pairs, later pairs winning.
  pub fn new<I: IntoIterator<Item = (u32, ToolShape)>>(tools: I) -> Self {
    let mut shapes = BTreeMap::new();
    for (number, tool) in tools {
      shapes.insert(number, tool);
    }
    Self { shapes }
  }

  /// The shape of a tool, when the caller supplied one.
  pub fn shape(&self, tool_number: u32) -> Option<ToolShape> {
    self.shapes.get(&tool_number).copied()
  }
}

/// Why a target surface could not be derived.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum TargetError {
  /// The grid the target would live on could not be built.
  #[error("{0}")]
  Grid(#[from] GridError),
  /// A PCB operation's target derivation is not implemented yet.
  #[error(
    "toolpath {toolpath:?} uses operation {operation}, which is not implemented yet for target derivation"
  )]
  OperationNotImplemented {
    /// The toolpath id.
    toolpath: String,
    /// The operation name.
    operation: &'static str,
  },
}

/// Why a field and a target could not be compared.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum DeviationError {
  /// The two were built on different lattices, so comparing them cell by cell is meaningless.
  #[error(
    "the field is {field_columns} x {field_rows} cells of {field_cell} mm and the target is \
     {target_columns} x {target_rows} cells of {target_cell} mm: both must be built for the same \
     stock and resolution"
  )]
  GridMismatch {
    /// The field's columns.
    field_columns: usize,
    /// The field's rows.
    field_rows: usize,
    /// The field's cell size, in millimetres.
    field_cell: f64,
    /// The target's columns.
    target_columns: usize,
    /// The target's rows.
    target_rows: usize,
    /// The target's cell size, in millimetres.
    target_cell: f64,
  },
}

/// The depth each cell is authorised to lose, on the simulation's grid.
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
  grid: Grid,
  authorised: Vec<f64>,
}

impl Target {
  /// The lattice the target lives on.
  pub fn grid(&self) -> Grid {
    self.grid
  }

  /// The depth cell `(column, row)` is authorised to lose, in millimetres.
  pub fn authorised(&self, column: usize, row: usize) -> f64 {
    self.authorised[row * self.grid.columns + column]
  }
}

/// The two ways a field can differ from the target, as volumes and cell counts.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Deviation {
  /// Material removed deeper than authorised, in cubic millimetres.
  pub gouge: f64,
  /// Material left above what the toolpath claimed it would reach, in cubic millimetres.
  pub uncut: f64,
  /// Cells that were gouged.
  pub gouge_cells: usize,
  /// Cells that were left uncut.
  pub uncut_cells: usize,
}

/// Derive the target surface for a whole document, on a grid of `cell` millimetres.
pub fn target_for(
  document: &Document,
  cell: Length,
  _tools: &ToolShapes,
) -> Result<Target, TargetError> {
  let grid = grid_for(&document.board, cell)?;
  if let Some(toolpath) = document.toolpaths.first() {
    let operation = match &toolpath.operation {
      Operation::Isolation(_) => "isolation",
      Operation::NonCopperClear(_) => "non_copper_clear",
      Operation::Cutout(_) => "cutout",
      Operation::Paint(_) => "paint",
      Operation::Panelize(_) => "panelize",
    };
    return Err(TargetError::OperationNotImplemented {
      toolpath: toolpath.id.clone(),
      operation,
    });
  }
  Ok(Target {
    grid,
    authorised: vec![0.0; grid.cells()],
  })
}

/// Compare a field against the target: one subtraction, two signs.
pub fn deviations(
  field: &Heightfield,
  target: &Target,
  tolerance: Length,
) -> Result<Deviation, DeviationError> {
  let grid = field.grid();
  let cell = grid.cell.mm();
  let lattice = target.grid;
  if grid.columns != lattice.columns || grid.rows != lattice.rows || cell != lattice.cell.mm() {
    return Err(DeviationError::GridMismatch {
      field_columns: grid.columns,
      field_rows: grid.rows,
      field_cell: cell,
      target_columns: lattice.columns,
      target_rows: lattice.rows,
      target_cell: lattice.cell.mm(),
    });
  }
  let area = grid.cell_area();
  let tolerance = tolerance.mm();
  let mut deviation = Deviation::default();
  for row in 0..grid.rows {
    for column in 0..grid.columns {
      let removed = -field.surface(column, row);
      let authorised = target.authorised(column, row);
      if removed > authorised + tolerance {
        deviation.gouge += (removed - authorised) * area;
        deviation.gouge_cells += 1;
      } else if authorised > removed + tolerance {
        deviation.uncut += (authorised - removed) * area;
        deviation.uncut_cells += 1;
      }
    }
  }
  Ok(deviation)
}
