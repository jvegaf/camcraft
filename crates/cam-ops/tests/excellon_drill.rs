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
    .join("../../fixtures/corpus")
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
