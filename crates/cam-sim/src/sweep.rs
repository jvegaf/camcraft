//! What a program does to the stock: the swept removal of every move, and the rapid that rams it
//! (M5 A2, A4).
//!
//! The sweep is geometric rather than physical, and it interpolates: a cutting move removes, at each
//! point it samples along the move, the depth **interpolated** between the `z` it started at and the `z`
//! it ends at, over the footprint the tool sweeps there. A constant-depth move therefore removes one
//! depth over its whole travel, exactly as M5's destination-`z` rule did, and a ramping move removes a
//! *graded* floor instead of a step (D-M6-12, which amends D-M5-3).
//!
//! A `Rapid` never cuts. A rapid that ends below the surface is a crash, reported and not simulated
//! (D-M5-4): a program that rams the tool has a defect, and hiding it inside a plausible volume number
//! is the one thing this module must not do.
//!
//! How finely a cutting move is sampled is part of the same contract, so it is derived rather than
//! chosen: the samples cover the travel at half a cell, *and* they resolve the `z` the move covers — see
//! [`sample_steps`]. A constant-depth move therefore takes exactly the number of samples it always
//! took, and a ramp takes the one it needs to leave no depth residual behind.

use std::f64::consts::TAU;

use cam_geom::Point2;
use cam_move::{Move, MoveStream, Point3, ToolShape};

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

/// Apply a whole program to a field, one segment at a time, with each segment's own tool shape.
pub fn apply(program: &MoveStream, field: &mut Heightfield) -> Removal {
  let mut removal = Removal::default();
  for segment in &program.segments {
    let tool = segment.tool;
    let mut from: Option<Point3> = None;
    for (index, movement) in segment.moves.iter().enumerate() {
      // The stream carries no explicit start position: a segment's first move starts where it ends,
      // which turns a plunge into a single disc instead of inventing an approach it did not declare.
      let origin = from.unwrap_or(movement.destination());
      if let Some(mut crash) = apply_move(field, origin, *movement, tool) {
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

/// The depth residual one sample interval may leave, as a divisor of a cell.
///
/// A **sixteenth** of a cell, which is half the tolerance `cam-cli` judges a simulation at (an eighth,
/// `TOLERANCE_PER_CELL`). The factor of two is the headroom: the residual is bounded by
/// `|Δz| / steps ≤ cell / 16`, so a correct graded cut lands at most half a tolerance short of its
/// target instead of grazing the judgement. It is not free — it doubles the samples of every graded
/// move, from `8·|Δz| / cell` to `16·|Δz| / cell`, which is still bounded by the cut depth and still
/// zero for a constant-depth program — and it is spent because a bound met by two ten-thousandths of a
/// millimetre is a test waiting to fail on another platform's floating point: sampling a 6 mm ramp at
/// `8·|Δz| / cell` left 0.1248 mm against a 0.125 mm judgement.
const RESIDUAL_PER_CELL: f64 = 16.0;

/// The number of samples a cutting move takes: `horizontal` is the count its travel needs at half a
/// cell, and the result is never fewer.
///
/// # Why the count also follows `z`
///
/// A cell's recorded removal is the depth of the deepest **sample** inside its footprint, while the
/// ramp it stands for is continuous, so the two differ by up to one sample interval's worth of `z`. Two
/// samples `step` apart on a move of grade `grade = |Δz| / travel` differ in depth by `grade · step`,
/// and a cell's footprint reaches a little past the last sample inside it — for a monotone ramp the
/// deepest reachable depth is at the footprint's far edge, and the last sample before that edge can be
/// a whole interval short of it. So the residual is `grade · step`, and requiring it to be no more than
/// `cell / RESIDUAL_PER_CELL` gives
///
/// - **line**: `step = travel / steps`, so `grade · step = |Δz| / steps`
///   ⟹ `steps ≥ RESIDUAL_PER_CELL · |Δz| / cell`;
/// - **arc**: the same, because the angular sampling measures the path by arc length —
///   `step = |Δθ| · r / steps` and `grade = |Δz| / (|Δθ| · r)` — so the arc length cancels again and the
///   count is `RESIDUAL_PER_CELL · |Δz| / cell` as well.
///
/// (The half-interval form `grade · step / 2` is the error of a *midpoint* estimate of the ramp. The
/// sweep takes a maximum over samples and the ramp's supremum is at the footprint's edge, so the gap is
/// a full interval — which is what the fluting residual test measures: sampling a 6 mm ramp every half
/// cell leaves 0.275 mm, and `8·|Δz| / cell` still leaves 0.125 mm, one judgement exactly. Hence
/// [`RESIDUAL_PER_CELL`] is 16 and not 8: the interval is halved again to leave the headroom that
/// paragraph describes.)
///
/// The count therefore scales with how much `z` the move covers, which is bounded by the cut depth, and
/// not with how steep it is, which is unbounded as a ramp shortens. The half-cell travel requirement
/// stays as the floor, so horizontal resolution never gets coarser; and a move whose `z` does not change
/// — every cutting move of every strategy before M6's flutes, and every plunge, whose travel is zero —
/// takes exactly the count it took before this rule existed.
fn sample_steps(horizontal: usize, travel: f64, delta_z: f64, cell: f64) -> usize {
  let horizontal = horizontal.max(1);
  if travel <= 0.0 || cell <= 0.0 {
    // A vertical move sweeps one disc at the depth it reaches, so there is no interval to resolve.
    return horizontal;
  }
  let needed = (delta_z.abs() * RESIDUAL_PER_CELL / cell).ceil();
  if !needed.is_finite() || needed < 1.0 {
    return horizontal;
  }
  horizontal.max(needed as usize)
}

/// The samples one straight cutting move takes: the half-cell count its travel needs, or the count its
/// `z` needs, whichever is finer.
fn line_sample_steps(travel: f64, delta_z: f64, cell: f64) -> usize {
  let horizontal = ((travel / (cell / 2.0)).ceil() as usize).max(1);
  sample_steps(horizontal, travel, delta_z, cell)
}

/// The samples one circular cutting move takes.
///
/// The angular step of `cell / (2 · r)` radians is half a cell of *arc*, so the travel term is the same
/// rule as a line's; and the depth term is the same rule too, because `step = |Δθ| · r / steps` and
/// `grade = |Δz| / (|Δθ| · r)` cancel the arc length. The radius therefore moves the angular step and
/// never the count a given `Δz` asks for.
fn arc_sample_steps(delta: f64, path_radius: f64, delta_z: f64, cell: f64) -> usize {
  let step_angle = cell / (2.0 * path_radius);
  let horizontal = ((delta.abs() / step_angle).ceil() as usize).max(1);
  sample_steps(horizontal, delta.abs() * path_radius, delta_z, cell)
}

/// Apply one movement that starts at `from`, cut by `tool`.
///
/// Returns the crash a rapid represents, when it ends below the surface at its destination.
pub fn apply_move(
  field: &mut Heightfield,
  from: Point3,
  movement: Move,
  tool: ToolShape,
) -> Option<Crash> {
  match movement {
    Move::Feed { to } => {
      sweep_line(field, from, to, tool);
      None
    }
    Move::Arc { to, center, ccw } => {
      sweep_arc(field, from, to, center, ccw, tool);
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

/// Lower every cell the tool covers at one sampled point, to the depth its own shape reaches there.
///
/// A cylinder removes the depth the move has reached at this point across its footprint. A cone does
/// not: at horizontal distance `x` from the tip its flank is `rise(x)` above the tip, so the material
/// there is lowered to `−(depth − rise(x))`, and the footprint is the widest disc the cone has at this
/// point's own depth. This is the difference between a V-groove and a flat-bottomed slot, and cutting
/// the flat slot would remove material the machine never touched (M6 A8).
fn sweep_disc(field: &mut Heightfield, centre: Point2, z: f64, tool: ToolShape) {
  let grid = field.grid();
  let cell = grid.cell.mm();
  let depth = -z;
  let radius = tool.radius_at(depth).max(0.0);
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
      if distance > radius {
        continue;
      }
      let removed = (depth - tool.rise_at_radius(distance)).max(0.0);
      if removed > 0.0 {
        field.lower(column, row, -removed);
      }
    }
  }
}

/// Sweep a straight move, sampled at half a cell so the union of discs covers the capsule.
///
/// The depth is interpolated between `from_z` and `to_z` at the same fraction as the position, so a
/// ramp's floor is graded along its travel (D-M6-12). A constant-depth move interpolates to that
/// constant, bit for bit: the difference is zero and multiplying it by the fraction changes nothing.
fn sweep_line(field: &mut Heightfield, from: Point3, to: Point3, tool: ToolShape) {
  let cell = field.grid().cell.mm();
  let (start, end) = (from.xy(), to.xy());
  let (from_z, to_z) = (from.z.mm(), to.z.mm());
  let steps = line_sample_steps(start.distance_to(end), to_z - from_z, cell);
  for step in 0..=steps {
    let t = step as f64 / steps as f64;
    let point = Point2::new(
      start.x + (end.x - start.x) * t,
      start.y + (end.y - start.y) * t,
    );
    sweep_disc(field, point, interpolate(from_z, to_z, t), tool);
  }
}

/// The `z` a ramping move has reached `t` of the way along it.
///
/// `from + (to − from) · t` rather than `from · (1 − t) + to · t`, so a constant move returns its own
/// value exactly instead of an expression that can round away from it.
fn interpolate(from: f64, to: f64, t: f64) -> f64 {
  from + (to - from) * t
}

/// Sweep a circular move along its own circle, in angular steps of half a cell of arc.
///
/// The depth is interpolated by the *angle*, which is the arc's own fraction of travel — the same
/// fraction the position is sampled at (D-M6-12).
fn sweep_arc(
  field: &mut Heightfield,
  from: Point3,
  to: Point3,
  center: Point2,
  ccw: bool,
  tool: ToolShape,
) {
  let cell = field.grid().cell.mm();
  let (start_point, end_point) = (from.xy(), to.xy());
  let (from_z, to_z) = (from.z.mm(), to.z.mm());
  let path_radius = center.distance_to(end_point);
  if path_radius <= 0.0 {
    sweep_disc(field, end_point, to_z, tool);
    return;
  }
  let start = (start_point.y - center.y).atan2(start_point.x - center.x);
  let end = (end_point.y - center.y).atan2(end_point.x - center.x);
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
  let steps = arc_sample_steps(delta, path_radius, to_z - from_z, cell);
  for step in 0..=steps {
    let t = step as f64 / steps as f64;
    let angle = start + delta * t;
    let point = Point2::new(
      center.x + path_radius * angle.cos(),
      center.y + path_radius * angle.sin(),
    );
    sweep_disc(field, point, interpolate(from_z, to_z, t), tool);
  }
}

#[cfg(test)]
mod sampling {
  //! The step-count arithmetic, pinned directly.
  //!
  //! A unit test rather than an integration one because the count is not observable from outside: the
  //! sweep's output is a field, and a field can only show that a residual is small, not that the count
  //! is the smallest one that keeps it inside the bound.

  use super::{arc_sample_steps, line_sample_steps};

  /// The residual a move leaves: the depth gap between two neighbouring samples, `|Δz| / steps`.
  fn residual(delta_z: f64, steps: usize) -> f64 {
    delta_z.abs() / steps as f64
  }

  #[test]
  fn a_constant_depth_move_takes_exactly_the_count_its_travel_needs() {
    // Neutrality is structural, not hoped for: with `Δz = 0` the depth term is zero, so the count is
    // the half-cell count each move has always taken, for every travel and every cell size.
    for travel in [0.25_f64, 1.0, 7.0, 10.0, 140.0] {
      for cell in [0.5_f64, 1.0, 2.0] {
        let half_cell = ((travel / (cell / 2.0)).ceil() as usize).max(1);
        assert_eq!(
          line_sample_steps(travel, 0.0, cell),
          half_cell,
          "a {travel} mm move at {cell} mm cells"
        );
        assert_eq!(
          line_sample_steps(travel, -0.0, cell),
          half_cell,
          "and a negative zero changes nothing"
        );
      }
    }
  }

  #[test]
  fn a_vertical_move_takes_one_step_however_far_its_z_travels() {
    // A plunge sweeps one disc at the depth it reaches: there is no interval along the path to resolve,
    // so the depth term must not inflate it into a stack of identical discs.
    assert_eq!(line_sample_steps(0.0, 8.0, 1.0), 1);
    assert_eq!(line_sample_steps(0.0, 12.0, 0.5), 1);
  }

  #[test]
  fn a_graded_move_takes_the_smallest_count_that_keeps_its_residual_inside_a_sixteenth_of_a_cell() {
    for (delta_z, cell) in [(6.0_f64, 1.0_f64), (6.0, 0.5), (1.0, 2.0)] {
      let steps = line_sample_steps(1.0, delta_z, cell);
      assert!(
        residual(delta_z, steps) <= cell / 16.0,
        "{delta_z} mm over a {cell} mm cell: {} samples leave {} mm",
        steps,
        residual(delta_z, steps)
      );
      assert!(
        residual(delta_z, steps - 1) > cell / 16.0,
        "one sample fewer would already break the bound: {} samples of a {delta_z} mm move leave {} mm",
        steps,
        residual(delta_z, steps - 1)
      );
    }
    // A shallow grade does not ask for anything the travel term does not already provide, and the
    // count never falls below it.
    assert_eq!(line_sample_steps(10.0, 0.5, 1.0), 20);
  }

  #[test]
  fn an_arc_takes_the_count_its_arc_length_and_its_z_ask_for_whatever_its_radius() {
    // Two arcs of the same arc length — 6 mm — and the same 4 mm of Z, at radii 3 and 6: the angular
    // steps differ (twice the angle at half the radius) and the counts are equal.
    let small_radius = arc_sample_steps(2.0, 3.0, 4.0, 1.0);
    let large_radius = arc_sample_steps(1.0, 6.0, 4.0, 1.0);
    assert_eq!(
      small_radius, large_radius,
      "the arc length cancels out of the count"
    );
    // Sixty-four for 4 mm of Z at a 1 mm cell, exactly as the same travel on a line would ask.
    assert_eq!(small_radius, 64);
    assert_eq!(line_sample_steps(6.0, 4.0, 1.0), 64);
    assert!(residual(4.0, small_radius) <= 1.0 / 16.0);
  }
}
