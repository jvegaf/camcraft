//! What a program does to the stock: the swept removal of every move, and the rapid that rams it
//! (M5 A2, A4).
//!
//! The sweep is deliberately geometric rather than physical: a cutting move removes material down to
//! its destination `z` over the footprint the tool sweeps in XY, and the vertical interpolation between
//! the previous `z` and this one is not modelled. That is conservative in the only direction that
//! matters — no material is left standing that the tool did not leave standing (D-M5-3).
//!
//! A `Rapid` never cuts. A rapid that ends below the surface is a crash, reported and not simulated
//! (D-M5-4): a program that rams the tool has a defect, and hiding it inside a plausible volume number
//! is the one thing this module must not do.

use std::f64::consts::TAU;

use cam_geom::Point2;
use cam_move::{Move, MoveStream, Point3};

use crate::heightfield::Heightfield;

/// One movement that ends below the material it was meant to clear.
#[derive(Clone, Debug, PartialEq)]
pub struct Crash {
  /// The toolpath (segment) the movement belongs to.
  pub toolpath: String,
  /// The movement's index within that toolpath.
  pub move_index: usize,
  /// Where the rapid ended.
  pub at: Point3,
}

/// What running a program over a field did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Removal {
  /// Movements walked, cutting and not.
  pub moves: usize,
  /// Every rapid that ended below the surface, in program order.
  pub crashes: Vec<Crash>,
}

/// Apply a whole program to a field, one segment at a time, with each segment's own tool diameter.
pub fn apply(program: &MoveStream, field: &mut Heightfield) -> Removal {
  let mut removal = Removal::default();
  for segment in &program.segments {
    let radius = segment.tool_diameter.mm() / 2.0;
    let mut from: Option<Point3> = None;
    for (index, movement) in segment.moves.iter().enumerate() {
      // The stream carries no explicit start position: a segment's first move starts where it ends,
      // which turns a plunge into a single disc instead of inventing an approach it did not declare.
      let origin = from.unwrap_or(movement.destination());
      if let Some(mut crash) = apply_move(field, origin, *movement, radius) {
        crash.toolpath = segment.name.clone();
        crash.move_index = index;
        removal.crashes.push(crash);
      }
      removal.moves += 1;
      from = Some(movement.destination());
    }
  }
  removal
}

/// Apply one movement that starts at `from`, with a tool of `tool_radius`.
///
/// Returns the crash a rapid represents, when it ends below the surface at its destination.
pub fn apply_move(
  field: &mut Heightfield,
  from: Point3,
  movement: Move,
  tool_radius: f64,
) -> Option<Crash> {
  match movement {
    Move::Feed { to } => {
      sweep_line(field, from.xy(), to.xy(), to.z.mm(), tool_radius);
      None
    }
    Move::Arc { to, center, ccw } => {
      sweep_arc(
        field,
        from.xy(),
        to.xy(),
        center,
        ccw,
        to.z.mm(),
        tool_radius,
      );
      None
    }
    Move::Rapid { to } | Move::Retract { to } => {
      let surface = field.surface_under(to.xy())?;
      if to.z.mm() < surface {
        Some(Crash {
          toolpath: String::new(),
          move_index: 0,
          at: to,
        })
      } else {
        None
      }
    }
  }
}

/// Lower every cell whose centre the tool covers at a point.
fn sweep_disc(field: &mut Heightfield, centre: Point2, z: f64, radius: f64) {
  let grid = field.grid();
  let cell = grid.cell.mm();
  let radius = radius.max(0.0);
  let low = |value: f64, origin: f64, count: usize| -> usize {
    let raw = ((value - origin) / cell).floor();
    if raw <= 0.0 {
      0
    } else if raw >= count as f64 {
      count
    } else {
      raw as usize
    }
  };
  let first_column = low(centre.x - radius, grid.origin.x, grid.columns);
  let last_column = low(centre.x + radius, grid.origin.x, grid.columns);
  let first_row = low(centre.y - radius, grid.origin.y, grid.rows);
  let last_row = low(centre.y + radius, grid.origin.y, grid.rows);
  for row in first_row..=last_row.min(grid.rows.saturating_sub(1)) {
    for column in first_column..=last_column.min(grid.columns.saturating_sub(1)) {
      let distance = field.cell_centre(column, row).distance_to(centre);
      if distance <= radius {
        field.lower(column, row, z);
      }
    }
  }
}

/// Sweep a straight move, sampled at half a cell so the union of discs covers the capsule.
fn sweep_line(field: &mut Heightfield, from: Point2, to: Point2, z: f64, radius: f64) {
  let cell = field.grid().cell.mm();
  let length = from.distance_to(to);
  let steps = ((length / (cell / 2.0)).ceil() as usize).max(1);
  for step in 0..=steps {
    let t = step as f64 / steps as f64;
    let point = Point2::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t);
    sweep_disc(field, point, z, radius);
  }
}

/// Sweep a circular move along its own circle, in angular steps of half a cell of arc.
fn sweep_arc(
  field: &mut Heightfield,
  from: Point2,
  to: Point2,
  center: Point2,
  ccw: bool,
  z: f64,
  radius: f64,
) {
  let cell = field.grid().cell.mm();
  let path_radius = center.distance_to(to);
  if path_radius <= 0.0 {
    sweep_disc(field, to, z, radius);
    return;
  }
  let start = (from.y - center.y).atan2(from.x - center.x);
  let end = (to.y - center.y).atan2(to.x - center.x);
  let mut delta = end - start;
  if ccw {
    while delta <= 0.0 {
      delta += TAU;
    }
  } else {
    while delta >= 0.0 {
      delta -= TAU;
    }
  }
  let step_angle = cell / (2.0 * path_radius);
  let steps = ((delta.abs() / step_angle).ceil() as usize).max(1);
  for step in 0..=steps {
    let angle = start + delta * (step as f64 / steps as f64);
    let point = Point2::new(
      center.x + path_radius * angle.cos(),
      center.y + path_radius * angle.sin(),
    );
    sweep_disc(field, point, z, radius);
  }
}
