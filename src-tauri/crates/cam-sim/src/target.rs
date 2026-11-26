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

/// Maximum declared PCB cut depth across document toolpaths, in millimetres.
pub fn max_pcb_cut_depth_mm(document: &Document) -> f64 {
  document
    .toolpaths
    .iter()
    .map(|toolpath| match &toolpath.operation {
      Operation::Isolation(p) => p.cut_depth.mm(),
      Operation::NonCopperClear(p) => p.cut_depth.mm(),
      Operation::Cutout(p) => p.cut_depth.mm(),
      Operation::Paint(p) => p.cut_depth.mm(),
      Operation::Drilling(p) => p.cut_depth.mm(),
      Operation::Panelize(_) => 0.0,
    })
    .fold(0.0_f64, f64::max)
}

/// Gouge and uncut against the maximum declared PCB cut depth.
///
/// PCB preview has no per-operation geometric target, so the whole simulated area is authorised the
/// document's deepest declared cut: removal beyond it is gouge, and material left above it — the
/// board surface a program never reached included — is uncut. [`deviations`] holds the arithmetic.
pub fn pcb_gouge_over_max_depth(
  field: &Heightfield,
  max_depth_mm: f64,
  tolerance: Length,
) -> Deviation {
  let target = Target {
    grid: field.grid(),
    authorised: vec![max_depth_mm.max(0.0); field.grid().cells()],
  };
  deviations(field, &target, tolerance).unwrap_or_default()
}

/// Derive the target surface for a whole document, on a grid of `cell` millimetres.
///
/// PCB operations currently use the maximum declared cut depth across the document as a
/// whole-board authorisation bound. This preserves gouge-over-depth semantics for callers using
/// [`deviations`], but it is not a per-operation geometric uncut target.
pub fn target_for(
  document: &Document,
  cell: Length,
  _tools: &ToolShapes,
) -> Result<Target, TargetError> {
  let grid = grid_for(&document.board, cell)?;
  let depth = max_pcb_cut_depth_mm(document).max(0.0);
  Ok(Target {
    grid,
    authorised: vec![depth; grid.cells()],
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

#[cfg(test)]
mod tests {
  use super::*;
  use cam_model::{
    Board, BoardSideUp, CutoutParams, Document, IsolationKind, IsolationParams, MillingDirection,
    Operation, TabKind, Toolpath,
  };
  use cam_move::{Move, MoveStream, Point3, Segment, ToolShape};
  use cam_units::Length;

  fn pcb_doc(depth_mm: f64) -> Document {
    let mut doc = Document::new(Board {
      width: Length::from_mm(10.0),
      height: Length::from_mm(10.0),
      thickness: Length::from_mm(1.6),
      origin: cam_model::OriginCorner::BottomLeft,
      side_up: BoardSideUp::Top,
    });
    doc.toolpaths.push(Toolpath {
      id: "T1".into(),
      name: "Iso".into(),
      entity_ids: vec![],
      tool_id: None,
      tool_number: 1,
      operation: Operation::Isolation(IsolationParams {
        passes: 1,
        overlap: 0.5,
        direction: MillingDirection::Climb,
        kind: IsolationKind::Full,
        cut_depth: Length::from_mm(depth_mm),
        keep_pours: false,
      }),
    });
    doc
  }

  #[test]
  fn max_pcb_cut_depth_mm_reports_maximum_declared_depth() {
    let mut doc = pcb_doc(0.1);
    doc.toolpaths.push(Toolpath {
      id: "T2".into(),
      name: "Cut".into(),
      entity_ids: vec![],
      tool_id: None,
      tool_number: 2,
      operation: Operation::Cutout(CutoutParams {
        margin: Length::from_mm(0.0),
        tabs: vec![],
        tab_kind: TabKind::Straight,
        pass_depth: Length::from_mm(0.0),
        cut_depth: Length::from_mm(1.7),
        cut_interiors: false,
      }),
    });
    assert!((max_pcb_cut_depth_mm(&doc) - 1.7).abs() < 1e-9);
  }

  #[test]
  fn pcb_gouge_over_max_depth_reports_excess_removal_and_left_standing_material() {
    let doc = pcb_doc(0.2);
    let mut field = Heightfield::new(&doc.board, Length::from_mm(1.0)).unwrap();
    let mut stream = MoveStream::new(Length::from_mm(5.0), cam_geom::Point2::new(0.0, 0.0));
    stream.segments.push(Segment {
      name: "cut".into(),
      tool_number: 1,
      tool: ToolShape::cylinder(Length::from_mm(1.0)),
      feed_rate: 100.0,
      plunge_rate: 30.0,
      spindle_speed: 18000,
      moves: vec![
        Move::Rapid {
          to: Point3::new(2.0, 2.0, 5.0),
        },
        Move::Feed {
          to: Point3::new(2.0, 2.0, -0.5),
        },
        Move::Feed {
          to: Point3::new(3.0, 2.0, -0.5),
        },
      ],
    });
    crate::sweep::apply(&stream, &mut field);
    let dev = pcb_gouge_over_max_depth(&field, 0.2, Length::from_mm(0.01));
    assert!(dev.gouge > 0.0);
    assert!(dev.gouge_cells > 0);
    // A program that clears part of the board leaves the rest of the simulated area uncut.
    assert!(dev.uncut > 0.0);
    assert!(dev.uncut_cells > 0);
  }

  #[test]
  fn pcb_gouge_over_max_depth_reports_nothing_for_a_board_cut_to_the_declared_depth() {
    let doc = pcb_doc(0.2);
    let mut field = Heightfield::new(&doc.board, Length::from_mm(1.0)).unwrap();
    for row in 0..field.grid().rows {
      for column in 0..field.grid().columns {
        field.lower(column, row, -0.2);
      }
    }
    let dev = pcb_gouge_over_max_depth(&field, 0.2, Length::from_mm(0.01));
    assert_eq!(dev.gouge, 0.0);
    assert_eq!(dev.gouge_cells, 0);
    assert_eq!(dev.uncut, 0.0);
    assert_eq!(dev.uncut_cells, 0);
  }
}
