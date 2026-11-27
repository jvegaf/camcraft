use std::collections::BTreeMap;
use std::path::Path;

use cam_excellon::{parse, Feature, Tool};
use cam_model::Job;
use cam_ops::excellon_drill::{plan_excellon_drill, ExcellonToolSetup, SlotMode, ToolResolver};
use cam_ops::{DrillParams, RetractMode, ToolSpec, ToolpathError};
use cam_units::Length;

struct DiameterResolver {
  peck_depth: f64,
  cut_depth: f64,
}

impl ToolResolver for DiameterResolver {
  fn resolve(&self, tool: &Tool) -> Result<ExcellonToolSetup, ToolpathError> {
    Ok(ExcellonToolSetup {
      excellon_tool: tool.number,
      tool: ToolSpec {
        number: tool.number,
        diameter: tool.diameter,
        included_angle: None,
        feed_rate: 100.0,
        plunge_rate: 30.0,
        spindle_speed: 18_000,
      },
      params: DrillParams {
        peck_depth: Length::from_mm(self.peck_depth),
        retract: RetractMode::Full,
        start_depth: Length::ZERO,
        cut_depth: Length::from_mm(self.cut_depth),
        bank: None,
      },
    })
  }
}

fn corpus_path(slug: &str) -> std::path::PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../../fixtures/corpus")
    .join(slug)
    .join("drill.drl")
}

fn parse_corpus(slug: &str) -> cam_excellon::Excellon {
  let text = std::fs::read_to_string(corpus_path(slug)).unwrap();
  parse(&text).unwrap()
}

fn plan_excellon(
  excellon: &cam_excellon::Excellon,
  slot_mode: SlotMode,
) -> cam_ops::excellon_drill::ExcellonDrillPlan {
  let job = Job::default_sheet();
  let resolver = DiameterResolver {
    peck_depth: 0.0,
    cut_depth: 2.0,
  };
  plan_excellon_drill(excellon, &resolver, &job, slot_mode).unwrap()
}

fn feature_counts_by_tool(excellon: &cam_excellon::Excellon) -> BTreeMap<u32, usize> {
  let mut counts = BTreeMap::new();
  for feature in &excellon.features {
    let tool = match feature {
      Feature::Hole { tool, .. } | Feature::Slot { tool, .. } => *tool,
    };
    *counts.entry(tool).or_insert(0usize) += 1;
  }
  counts
}

#[test]
fn corpus_feature_counts_and_tool_groups_match_excellon() {
  let cases = [
    ("kicad-2l-simple", 3usize, 88usize),
    ("kicad-4l-pours", 6, 692),
    ("kicad-large-pours", 13, 251),
    ("substitute-altium", 10, 526),
    ("substitute-eagle", 12, 445),
    ("substitute-easyeda", 7, 165),
  ];
  for (slug, tools, features) in cases {
    let excellon = parse_corpus(slug);
    let per_tool = feature_counts_by_tool(&excellon);
    for mode in [SlotMode::Drill, SlotMode::Trajectory] {
      let plan = plan_excellon(&excellon, mode);
      assert_eq!(plan.groups.len(), tools, "{slug} {mode:?}: tool groups");
      assert_eq!(plan.total_hits, features, "{slug} {mode:?}: total hits");
      let group_hits: usize = plan.groups.iter().map(|g| g.hits).sum();
      assert_eq!(group_hits, features, "{slug} {mode:?}: summed hits");
      assert_eq!(
        plan.tool_changes,
        tools.saturating_sub(1),
        "{slug} {mode:?}: tool changes"
      );
      for group in &plan.groups {
        let expected = *per_tool.get(&group.excellon_tool).unwrap_or(&0);
        assert_eq!(
          group.hits, expected,
          "{slug} {mode:?}: hits for T{}",
          group.excellon_tool
        );
      }
    }
  }
}

#[test]
fn planning_is_deterministic_for_all_corpus_plates() {
  for slug in [
    "kicad-2l-simple",
    "kicad-4l-pours",
    "kicad-large-pours",
    "substitute-altium",
    "substitute-eagle",
    "substitute-easyeda",
  ] {
    let excellon = parse_corpus(slug);
    for mode in [SlotMode::Drill, SlotMode::Trajectory] {
      let first = plan_excellon(&excellon, mode);
      let second = plan_excellon(&excellon, mode);
      assert_eq!(first, second, "{slug} {mode:?}: deterministic plan");
    }
  }
}

/// A resolver that retracts only by an amount, which is what slot pecking has to survive.
struct AmountResolver {
  peck_depth: f64,
  cut_depth: f64,
  amount: f64,
}

impl ToolResolver for AmountResolver {
  fn resolve(&self, tool: &Tool) -> Result<ExcellonToolSetup, ToolpathError> {
    Ok(ExcellonToolSetup {
      excellon_tool: tool.number,
      tool: ToolSpec {
        number: tool.number,
        diameter: tool.diameter,
        included_angle: None,
        feed_rate: 100.0,
        plunge_rate: 30.0,
        spindle_speed: 18_000,
      },
      params: DrillParams {
        peck_depth: Length::from_mm(self.peck_depth),
        retract: RetractMode::Amount {
          amount: Length::from_mm(self.amount),
        },
        start_depth: Length::ZERO,
        cut_depth: Length::from_mm(self.cut_depth),
        bank: None,
      },
    })
  }
}

/// One slot on a one-tool board, which is what both slot rules below need to have in hand.
fn one_slot() -> cam_excellon::Excellon {
  cam_excellon::Excellon {
    units: cam_excellon::Units::Metric,
    tools: vec![Tool {
      number: 1,
      diameter: Length::from_mm(1.0),
    }],
    features: vec![Feature::Slot {
      tool: 1,
      from: cam_geom::Point2::new(10.0, 10.0),
      to: cam_geom::Point2::new(30.0, 10.0),
    }],
  }
}

/// A retract amount with no number behind it, or one that lifts below the depth just cut, is refused
/// at authoring (R3-001).
///
/// A slot pecked more than once retracts to `-depth + amount`, so `NaN` puts a Z no controller can
/// read into the program and a negative amount retracts *downwards*, below the floor the peck just
/// reached: a "retract" that plunges deeper. Neither is clamped or defaulted — a silently corrected
/// amount changes how far the tool leaves the material between pecks, which is the number the caller
/// asked for. The refusal is named, so the caller learns which parameter is not a real lift.
#[test]
fn a_pecked_slot_refuses_a_retract_amount_that_is_not_a_real_lift() {
  for amount in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.5] {
    let resolver = AmountResolver {
      peck_depth: 1.0,
      cut_depth: 2.0,
      amount,
    };
    let error = plan_excellon_drill(
      &one_slot(),
      &resolver,
      &Job::default_sheet(),
      SlotMode::Trajectory,
    )
    .unwrap_err();
    assert!(
      matches!(error, ToolpathError::InvalidRetract { .. }),
      "a retract of {amount} mm is not a lift: {error:?}"
    );
    assert!(
      error.to_string().contains("retract amount"),
      "the refusal names the parameter: {error}"
    );
  }
}

/// One slot drilled in several pecks must never rapid while it changes Z.
///
/// A `RetractMode::Amount` retract stops above the cut floor but still below the surface, so a
/// following rapid that also climbed to safe Z would move in XY and Z at once and drag the cutter
/// out of the material diagonally. The machine's own rule is simpler: a rapid is a move at one
/// height, and every change of height is a retract or a plunge.
#[test]
fn a_pecked_slot_never_rapids_while_it_changes_height() {
  let excellon = one_slot();
  let resolver = AmountResolver {
    peck_depth: 1.0,
    cut_depth: 2.0,
    amount: 0.5,
  };
  let plan = plan_excellon_drill(
    &excellon,
    &resolver,
    &Job::default_sheet(),
    SlotMode::Trajectory,
  )
  .expect("a slot plans");

  let moves = &plan.groups[0].moves;
  // The program starts parked at clearance over the machine origin, which is where the first rapid
  // begins: a rapid that lifts from Z0 would be this first move, not a reposition between pecks.
  let mut previous = cam_move::Point3 {
    x: Length::ZERO,
    y: Length::ZERO,
    z: Length::from_mm(cam_model::DEFAULT_SAFE_Z_MM),
  };
  for movement in moves {
    let to = movement.destination();
    if matches!(movement, cam_move::Move::Rapid { .. }) {
      assert!(
        (to.z.mm() - previous.z.mm()).abs() < 1.0e-9,
        "a rapid changed height from {} to {}: {moves:?}",
        previous.z.mm(),
        to.z.mm()
      );
    }
    previous = to;
  }
  // And it did peck more than once, so the rule above had a reposition to judge.
  let rapids = moves
    .iter()
    .filter(|movement| matches!(movement, cam_move::Move::Rapid { .. }))
    .count();
  assert!(rapids >= 2, "the slot was pecked: {moves:?}");
}
