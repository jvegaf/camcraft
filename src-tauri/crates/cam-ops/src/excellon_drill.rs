//! Drill planning from Excellon tables.
//!
//! Groups Excellon features by tool, orders each group deterministically, and produces drilling
//! programs with hit and tool-change counts (E2.1, E2.2).

use std::collections::BTreeMap;

use cam_excellon::{Excellon, Feature, Tool};
use cam_geom::Point2;
use cam_model::Job;
use cam_move::{Move, Point3};
use cam_units::Length;

use crate::params::{DrillParams, RetractMode};
use crate::peck::{drill, DrillHole, DrillRequest};
use crate::shared::{machine_point, pass_depths, ToolSpec, ToolpathError};

/// How to machine an Excellon slot feature.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SlotMode {
  /// Machine the slot as a point-drilled feature: each slot contributes one hit drilled at its
  /// midpoint (or at its location when zero-length).
  #[default]
  Drill,
  /// Machine the slot as a routed trajectory along its length.
  Trajectory,
}

/// Resolved cutting setup for one Excellon tool number.
#[derive(Clone, Debug, PartialEq)]
pub struct ExcellonToolSetup {
  /// The Excellon tool this setup applies to.
  pub excellon_tool: u32,
  /// Tool used for cutting.
  pub tool: ToolSpec,
  /// Peck/retract/depth parameters for drilling and slot plunges.
  pub params: DrillParams,
}

/// One Excellon tool group's planned drilling result.
#[derive(Clone, Debug, PartialEq)]
pub struct ExcellonToolPlan {
  /// Excellon tool number.
  pub excellon_tool: u32,
  /// Diameter declared in the Excellon table, when present.
  pub diameter: Option<Length>,
  /// Number of Excellon features (hits) in this group.
  pub hits: usize,
  /// Number of hole features in this group.
  pub holes: usize,
  /// Number of slot features in this group.
  pub slots: usize,
  /// Moves produced for this group, in cutting order.
  pub moves: Vec<Move>,
  /// Entity ids/features in drilling order.
  pub order: Vec<String>,
  /// Total XY rapid/travel between features, measured from the machine origin for the first.
  pub rapid_length: Length,
}

/// A complete drilling plan from an Excellon file.
#[derive(Clone, Debug, PartialEq)]
pub struct ExcellonDrillPlan {
  /// Per-tool groups, in deterministic tool order.
  pub groups: Vec<ExcellonToolPlan>,
  /// Total number of features across all groups.
  pub total_hits: usize,
  /// Number of tool changes required to execute the groups in order.
  ///
  /// This is `groups.len().saturating_sub(1)` when every group is emitted in sequence.
  pub tool_changes: usize,
}

/// Resolves the cutting setup for an Excellon tool.
///
/// Implementations may look up tools by diameter or by Excellon tool number.
pub trait ToolResolver {
  /// Return the setup for `tool` from the Excellon table.
  fn resolve(&self, tool: &Tool) -> Result<ExcellonToolSetup, ToolpathError>;
}

/// Plan drilling from an Excellon file.
///
/// Features are grouped by Excellon tool number and processed in ascending tool-number order. Each
/// group's hole features are ordered deterministically (nearest-neighbour + 2-opt). Slots are
/// handled according to `slot_mode`.
pub fn plan_excellon_drill<R: ToolResolver>(
  excellon: &Excellon,
  resolver: &R,
  job: &Job,
  slot_mode: SlotMode,
) -> Result<ExcellonDrillPlan, ToolpathError> {
  let mut grouped: BTreeMap<u32, Vec<(usize, &Feature)>> = BTreeMap::new();
  for (index, feature) in excellon.features.iter().enumerate() {
    let tool = match feature {
      Feature::Hole { tool, .. } | Feature::Slot { tool, .. } => *tool,
    };
    grouped.entry(tool).or_default().push((index, feature));
  }

  let tool_by_number: BTreeMap<u32, &Tool> = excellon
    .tools
    .iter()
    .map(|tool| (tool.number, tool))
    .collect();

  let mut groups = Vec::with_capacity(grouped.len());
  let mut total_hits = 0usize;

  for (tool_number, features) in grouped {
    let tool_entry = tool_by_number.get(&tool_number).copied();
    let setup = if let Some(tool) = tool_entry {
      resolver.resolve(tool)?
    } else {
      return Err(ToolpathError::UnknownExcellonTool { tool: tool_number });
    };
    let diameter = tool_entry.map(|tool| tool.diameter);

    let mut holes_for_tour: Vec<DrillHole> = Vec::new();
    let mut hole_indices: Vec<usize> = Vec::new();
    let mut slot_items: Vec<(usize, SlotItem)> = Vec::new();
    let mut hole_count = 0usize;
    let mut slot_count = 0usize;

    for (index, feature) in features.iter().copied() {
      let name = feature_name(index, feature);
      match feature {
        Feature::Hole { at, .. } => {
          hole_count += 1;
          hole_indices.push(index);
          holes_for_tour.push(DrillHole {
            entity: name,
            centre: *at,
          });
        }
        Feature::Slot { from, to, .. } => {
          slot_count += 1;
          match slot_mode {
            SlotMode::Drill => {
              let centre = slot_centre(*from, *to);
              hole_indices.push(index);
              holes_for_tour.push(DrillHole {
                entity: name,
                centre,
              });
            }
            SlotMode::Trajectory => {
              slot_items.push((
                index,
                SlotItem {
                  entity: name,
                  from: *from,
                  to: *to,
                },
              ));
            }
          }
        }
      }
    }

    let mut moves = Vec::new();
    let mut order = Vec::new();
    let mut rapid_length = 0.0;
    let mut previous = Point2::new(0.0, 0.0);

    if !holes_for_tour.is_empty() {
      let path = drill(&DrillRequest {
        holes: &holes_for_tour,
        params: &setup.params,
        tool: &setup.tool,
        job,
      })?;
      rapid_length += path.rapid_length.mm();
      order.extend(path.order);
      previous = last_xy(&path.moves).unwrap_or(previous);
      moves.extend(path.moves);
    }

    if !slot_items.is_empty() {
      // Sort slot trajectories deterministically by feature index/name to keep output stable.
      slot_items.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.entity.cmp(&b.1.entity)));
      let (slot_moves, slot_order, slot_travel) =
        route_slots(&slot_items, &setup.tool, &setup.params, job, previous)?;
      rapid_length += slot_travel;
      order.extend(slot_order);
      moves.extend(slot_moves);
    }

    let hits = hole_count + slot_count;
    total_hits += hits;
    groups.push(ExcellonToolPlan {
      excellon_tool: tool_number,
      diameter,
      hits,
      holes: hole_count,
      slots: slot_count,
      moves,
      order,
      rapid_length: Length::from_mm(rapid_length),
    });
  }

  let tool_changes = groups.len().saturating_sub(1);
  Ok(ExcellonDrillPlan {
    groups,
    total_hits,
    tool_changes,
  })
}

#[derive(Clone, Debug, PartialEq)]
struct SlotItem {
  entity: String,
  from: Point2,
  to: Point2,
}

fn feature_name(index: usize, feature: &Feature) -> String {
  match feature {
    Feature::Hole { .. } => format!("hole-{index}"),
    Feature::Slot { .. } => format!("slot-{index}"),
  }
}

fn slot_centre(from: Point2, to: Point2) -> Point2 {
  Point2::new((from.x + to.x) * 0.5, (from.y + to.y) * 0.5)
}

fn last_xy(moves: &[Move]) -> Option<Point2> {
  moves.last().map(|movement| movement.destination().xy())
}

fn route_slots(
  slots: &[(usize, SlotItem)],
  tool: &ToolSpec,
  params: &DrillParams,
  job: &Job,
  mut previous: Point2,
) -> Result<(Vec<Move>, Vec<String>, f64), ToolpathError> {
  if tool.diameter.mm() <= 0.0 {
    return Err(ToolpathError::InvalidToolDiameter);
  }
  if params.cut_depth.mm() <= params.start_depth.mm() {
    return Err(ToolpathError::InvalidDepthRange {
      cut_depth: params.cut_depth.mm(),
      start_depth: params.start_depth.mm(),
    });
  }
  let peck = params.peck_depth.mm();
  if peck < 0.0 {
    return Err(ToolpathError::InvalidPeckDepth { peck_depth: peck });
  }
  // The intermediate retract of a pecked slot is `-depth + amount`, so an amount that is not a real
  // lift either leaves a Z the controller cannot read (`NaN`, an infinity) or retracts below the floor
  // the peck just reached (negative). Checked here, where the amount is still one named parameter,
  // rather than downstream where it is only a Z (R3-001).
  if let RetractMode::Amount { amount } = params.retract {
    let amount = amount.mm();
    if !amount.is_finite() || amount < 0.0 {
      return Err(ToolpathError::InvalidRetract { amount });
    }
  }
  let pecks = if peck == 0.0 {
    vec![params.cut_depth.mm()]
  } else {
    pass_depths(params.start_depth.mm(), params.cut_depth.mm(), peck)?
  };

  let safe_z = Length::from_mm(cam_model::DEFAULT_SAFE_Z_MM);
  let mut moves = Vec::new();
  let mut order = Vec::new();
  let mut travel = 0.0;

  for (_, slot) in slots {
    let from = machine_point(job, slot.from);
    let to = machine_point(job, slot.to);
    travel += previous.distance_to(from);
    previous = to;

    let at_from = Point3 {
      x: Length::from_mm(from.x),
      y: Length::from_mm(from.y),
      z: safe_z,
    };
    let at_to_xy = (Length::from_mm(to.x), Length::from_mm(to.y));

    moves.push(Move::Rapid { to: at_from });
    for (position, depth) in pecks.iter().enumerate() {
      // Plunge/feed down at slot start.
      moves.push(Move::Feed {
        to: Point3 {
          z: Length::from_mm(-*depth),
          ..at_from
        },
      });
      // Route along the slot at depth.
      moves.push(Move::Feed {
        to: Point3 {
          x: at_to_xy.0,
          y: at_to_xy.1,
          z: Length::from_mm(-*depth),
        },
      });
      let last = position + 1 == pecks.len();
      let retract_z = if last {
        safe_z
      } else {
        match params.retract {
          RetractMode::Full => safe_z,
          RetractMode::Amount { amount } => Length::from_mm(-depth + amount.mm()),
        }
      };
      // Retract at slot end; for intermediate pecks, reposition to start for next pass.
      moves.push(Move::Retract {
        to: Point3 {
          x: at_to_xy.0,
          y: at_to_xy.1,
          z: retract_z,
        },
      });
      if !last {
        // Back to the slot's start at the height this peck just retracted to. A rapid to `at_from`
        // as it stands would climb to safe Z while it travels in XY, so the cutter would leave the
        // material diagonally at rapid rate instead of lifting out first -- the one move a program
        // must never contain.
        moves.push(Move::Rapid {
          to: Point3 {
            z: retract_z,
            ..at_from
          },
        });
      }
    }
    order.push(slot.entity.clone());
  }

  Ok((moves, order, travel))
}
