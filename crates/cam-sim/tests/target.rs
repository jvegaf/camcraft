//! The authorised target surface, and the two ways a field deviates from it (M5 A5, D-M5-2,
//! D-M5-5, A13).
//!
//! Every number is written as arithmetic in the test: the cell edge, the area and the depth are
//! visible in the assertion, so a failure says which of the three moved.

use std::f64::consts::{PI, TAU};

use cam_geom::{Contour, Point2};
use cam_model::{
  Document, DrillParams, Entity, EntityKind, Job, Layer, Operation, OriginCorner, PocketParams,
  PocketStrategy, ProfileParams, ProfileSide, Tab, Toolpath,
};
use cam_sim::target::{
  deviations, target_for, Deviation, DeviationError, Target, TargetError, ToolDiameters,
};
use cam_sim::Heightfield;
use cam_units::{Length, UnitSystem};

/// The resolution every test in this file runs at, and the one the tree's arithmetic assumes.
const CELL: f64 = 1.0;

fn job(extent: f64) -> Job {
  Job {
    width: Length::from_mm(extent),
    height: Length::from_mm(extent),
    thickness: Length::from_mm(18.0),
    units: UnitSystem::Mm,
    origin: OriginCorner::BottomLeft,
    safe_z: Length::from_mm(5.0),
  }
}

fn cell() -> Length {
  Length::from_mm(CELL)
}

fn document(extent: f64, entities: &[(&str, Contour)], toolpaths: Vec<Toolpath>) -> Document {
  let mut document = Document::new(job(extent));
  document.layers.push(Layer {
    id: "L1".to_owned(),
    name: "Part".to_owned(),
    visible: true,
    entities: entities
      .iter()
      .map(|(id, contour)| Entity {
        id: (*id).to_owned(),
        kind: EntityKind::Contour {
          contour: contour.clone(),
        },
      })
      .collect(),
  });
  document.toolpaths = toolpaths;
  document
}

/// A counter-clockwise square whose first vertex is `(min, min)`.
fn square(min: f64, max: f64) -> Contour {
  Contour::from_points(
    &[
      Point2::new(min, min),
      Point2::new(max, min),
      Point2::new(max, max),
      Point2::new(min, max),
    ],
    true,
  )
  .expect("a square")
}

fn circle(center: Point2, radius: f64) -> Contour {
  cam_geom::primitive::circle(center, radius).expect("a circle")
}

fn pocket(id: &str, entities: &[&str], islands: &[&str], start: f64, cut: f64) -> Toolpath {
  Toolpath {
    id: id.to_owned(),
    name: "Pocket".to_owned(),
    entity_ids: entities.iter().map(|id| (*id).to_owned()).collect(),
    island_ids: islands.iter().map(|id| (*id).to_owned()).collect(),
    tool_number: 1,
    operation: Operation::Pocket(PocketParams {
      strategy: PocketStrategy::Offset,
      start_depth: Length::from_mm(start),
      cut_depth: Length::from_mm(cut),
      ..PocketParams::default()
    }),
  }
}

fn profile(
  id: &str,
  entities: &[&str],
  side: ProfileSide,
  allowance: f64,
  cut: f64,
  tabs: Vec<Tab>,
) -> Toolpath {
  Toolpath {
    id: id.to_owned(),
    name: "Profile".to_owned(),
    entity_ids: entities.iter().map(|id| (*id).to_owned()).collect(),
    island_ids: Vec::new(),
    tool_number: 1,
    operation: Operation::Profile(ProfileParams {
      side,
      allowance: Length::from_mm(allowance),
      cut_depth: Length::from_mm(cut),
      tabs,
      ..ProfileParams::default()
    }),
  }
}

fn drilling(id: &str, entities: &[&str], cut: f64) -> Toolpath {
  Toolpath {
    id: id.to_owned(),
    name: "Drilling".to_owned(),
    entity_ids: entities.iter().map(|id| (*id).to_owned()).collect(),
    island_ids: Vec::new(),
    tool_number: 1,
    operation: Operation::Drilling(DrillParams {
      cut_depth: Length::from_mm(cut),
      ..DrillParams::default()
    }),
  }
}

fn tools(diameter: f64) -> ToolDiameters {
  ToolDiameters::new([(1, Length::from_mm(diameter))])
}

/// The whole target surface's volume, summed in row-major order: the test's own arithmetic.
fn authorised_volume(target: &Target) -> f64 {
  let grid = target.grid();
  let mut volume = 0.0;
  for row in 0..grid.rows {
    for column in 0..grid.columns {
      volume += target.authorised(column, row) * grid.cell_area();
    }
  }
  volume
}

fn claimed_cells(target: &Target) -> usize {
  let grid = target.grid();
  let mut claimed = 0;
  for row in 0..grid.rows {
    for column in 0..grid.columns {
      if target.authorised(column, row) > 0.0 {
        claimed += 1;
      }
    }
  }
  claimed
}

/// A field cut to the target, with one cell at a time adjusted by `depth_of`.
fn field_cut_to(
  job: &Job,
  target: &Target,
  depth_of: impl Fn(usize, usize, f64) -> f64,
) -> Heightfield {
  let mut field = Heightfield::new(job, cell()).expect("the field fits on the grid");
  let grid = target.grid();
  for row in 0..grid.rows {
    for column in 0..grid.columns {
      let depth = depth_of(column, row, target.authorised(column, row));
      if depth > 0.0 {
        field.lower(column, row, -depth);
      }
    }
  }
  field
}

/// A 20 x 20 mm pocket 6 mm deep in a 100 x 100 mm job: 400 claimed cells of 1 mm2 each.
fn pocket_fixture() -> (Job, Target) {
  let document = document(
    100.0,
    &[("square", square(20.0, 40.0))],
    vec![pocket("pocket 1", &["square"], &[], 0.0, 6.0)],
  );
  let target = target_for(&document, cell(), &tools(6.0)).expect("a target on a 100 x 100 grid");
  (job(100.0), target)
}

#[test]
fn a_pocket_authorises_its_region_and_leaves_its_island_standing() {
  // 100 x 100 mm stock at 1 mm cells: 100 x 100 cells, centres at 0.5 .. 99.5.
  let document = document(
    100.0,
    &[
      ("square", square(0.0, 100.0)),
      ("island", circle(Point2::new(50.0, 50.0), 10.0)),
    ],
    vec![pocket("pocket 1", &["square"], &["island"], 0.0, 6.0)],
  );
  let target = target_for(&document, cell(), &tools(6.0)).expect("a target");

  // Every cell whose centre falls inside the r10 island is authorised to lose nothing. dx and dy
  // are odd multiples of 0.5, so no centre lands on the circle (a^2 + b^2 = 400 has no odd solution).
  let grid = target.grid();
  let mut island_cells = 0;
  for row in 0..grid.rows {
    for column in 0..grid.columns {
      let dx = column as f64 + 0.5 - 50.0;
      let dy = row as f64 + 0.5 - 50.0;
      let in_island = dx * dx + dy * dy < 10.0 * 10.0;
      if in_island {
        assert_eq!(
          target.authorised(column, row),
          0.0,
          "island cell ({column}, {row}) is authorised to lose material"
        );
        island_cells += 1;
      }
    }
  }
  assert_eq!(
    claimed_cells(&target),
    100 * 100 - island_cells,
    "the pocket claims the square minus exactly the island"
  );
  assert!(
    (300..=330).contains(&island_cells),
    "pi r^2 is about 314 cells at 1 mm, got {island_cells}"
  );

  // A cell well inside the region reaches the whole final depth.
  assert_eq!(target.authorised(10, 10), 6.0);

  // The volume is the square minus the disc at 6 mm deep, within the coverage bound the grid
  // publishes (D-M5-3): the region's perimeter times half a cell, times the depth.
  let expected = (100.0 * 100.0 - PI * 10.0 * 10.0) * 6.0;
  let bound = (4.0 * 100.0 + TAU * 10.0) * CELL / 2.0 * 6.0;
  let volume = authorised_volume(&target);
  assert!(
    (volume - expected).abs() <= bound,
    "authorised {volume} mm3, expected {expected} +/- {bound}"
  );
}

#[test]
fn a_profile_authorises_the_kerf_band_outside_the_contour() {
  // A 100 mm square inset 10 mm into a 120 mm stock, so the band it declares is on the grid.
  let document = document(
    120.0,
    &[("square", square(10.0, 110.0))],
    vec![profile(
      "profile 1",
      &["square"],
      ProfileSide::Outside,
      0.0,
      6.0,
      Vec::new(),
    )],
  );
  let target = target_for(&document, cell(), &tools(6.0)).expect("a target");

  // The tool centre rides radius + allowance = 3 mm outside the vector, and that is the band
  // (D-M5-5): a cell 1.5 mm outside the edge is authorised, a cell 4.5 mm out is not.
  assert_eq!(target.authorised(111, 60), 6.0, "centre at (111.5, 60.5)");
  assert_eq!(target.authorised(114, 60), 0.0, "centre at (114.5, 60.5)");
  // The keep side is authorised nothing: the middle of the part stays standing.
  assert_eq!(target.authorised(60, 60), 0.0);

  // The band is the 3 mm annulus: a 106 x 106 square outside a 100 x 100 one. Both land on cell
  // edges, so the cell count is 106^2 - 100^2 exactly, 1 mm2 each, 6 mm deep.
  assert_eq!(
    authorised_volume(&target),
    (106.0 * 106.0 - 100.0 * 100.0) * 6.0
  );
}

#[test]
fn an_inside_profile_authorises_the_band_inside_the_contour() {
  let document = document(
    120.0,
    &[("square", square(10.0, 110.0))],
    vec![profile(
      "profile 1",
      &["square"],
      ProfileSide::Inside,
      0.0,
      6.0,
      Vec::new(),
    )],
  );
  let target = target_for(&document, cell(), &tools(6.0)).expect("a target");

  // The band is inside the vector: a cell 1.5 mm inside the edge is authorised, the middle of the
  // part and the material outside the vector are not.
  assert_eq!(target.authorised(11, 60), 6.0, "centre at (11.5, 60.5)");
  assert_eq!(target.authorised(60, 60), 0.0, "the middle of the part");
  assert_eq!(target.authorised(5, 60), 0.0, "outside the vector");

  // The annulus is a 100 x 100 square less a 94 x 94 one: 100^2 - 94^2 cells at 6 mm deep.
  assert_eq!(
    authorised_volume(&target),
    (100.0 * 100.0 - 94.0 * 94.0) * 6.0
  );
}

#[test]
fn a_tab_keeps_its_height_back_across_the_whole_band() {
  let tab = Tab {
    position: 0.0,
    width: Length::from_mm(10.0),
    height: Length::from_mm(4.0),
  };
  let document = document(
    120.0,
    &[("square", square(10.0, 110.0))],
    vec![profile(
      "profile 1",
      &["square"],
      ProfileSide::Outside,
      0.0,
      6.0,
      vec![tab],
    )],
  );
  let target = target_for(&document, cell(), &tools(6.0)).expect("a target");

  // The contour's first vertex is (10, 10), so position 0.0 is the first 10 mm of the bottom edge.
  // A cell under that span is authorised 6 - 4 = 2 across the whole 3 mm band, not only hard against
  // the vector: columns 10..=19 (centres 10.5 .. 19.5, arc length 0.5 .. 9.5) of the three band rows,
  // plus the 3 x 3 block at the corner, whose nearest point on the perimeter is the first vertex.
  for row in 7..=9 {
    for column in 7..=19 {
      assert_eq!(
        target.authorised(column, row),
        2.0,
        "tab cell ({column}, {row}) is authorised the full depth"
      );
    }
  }
  // Past the tab's 10 mm span on the same edge, and on the far side of the square: both cut full.
  assert_eq!(target.authorised(25, 8), 6.0, "centres at (25.5, 8.5)");
  assert_eq!(target.authorised(60, 111), 6.0, "centre at (60.5, 111.5)");

  // 39 cells keep 4 mm back (13 columns x 3 rows): the 1236 band cells at 6 mm, less 4 mm on each.
  assert_eq!(authorised_volume(&target), 1236.0 * 6.0 - 39.0 * 4.0);
}

#[test]
fn a_tab_on_a_circular_profile_follows_the_arc_length() {
  // An r10 circle built by `primitive::circle` starts at (60, 50) and runs counter-clockwise, so its
  // 2 pi r = 62.8 mm perimeter maps straight onto the arc angle. A 10 mm tab at position 0.0 covers
  // the first 10 mm of that arc: a cell just outside the start point keeps 6 - 4 = 2, a cell on the
  // opposite side of the circle, at about 31 mm of arc, is cut to the full 6.
  let tab = Tab {
    position: 0.0,
    width: Length::from_mm(10.0),
    height: Length::from_mm(4.0),
  };
  let document = document(
    100.0,
    &[("hole", circle(Point2::new(50.0, 50.0), 10.0))],
    vec![profile(
      "profile 1",
      &["hole"],
      ProfileSide::Outside,
      0.0,
      6.0,
      vec![tab],
    )],
  );
  let target = target_for(&document, cell(), &tools(6.0)).expect("a target");

  assert_eq!(
    target.authorised(61, 50),
    2.0,
    "centre (61.5, 50.5), 0.5 mm of arc"
  );
  assert_eq!(
    target.authorised(38, 50),
    6.0,
    "centre (38.5, 50.5), 31 mm of arc"
  );
}

#[test]
fn a_drilling_toolpath_authorises_each_hole_and_nothing_else() {
  let document = document(
    100.0,
    &[("hole", circle(Point2::new(50.0, 50.0), 5.0))],
    vec![drilling("drill 1", &["hole"], 6.0)],
  );
  let target = target_for(&document, cell(), &tools(6.0)).expect("a target");

  let grid = target.grid();
  let mut inside = 0;
  for row in 0..grid.rows {
    for column in 0..grid.columns {
      let dx = column as f64 + 0.5 - 50.0;
      let dy = row as f64 + 0.5 - 50.0;
      let in_hole = dx * dx + dy * dy < 5.0 * 5.0;
      let expected = if in_hole { 6.0 } else { 0.0 };
      assert_eq!(
        target.authorised(column, row),
        expected,
        "cell ({column}, {row}) is at ({}, {})",
        column as f64 + 0.5,
        row as f64 + 0.5
      );
      if in_hole {
        inside += 1;
      }
    }
  }

  // pi r^2 depth = 471.2 mm3 of a 78-cell disc, within the perimeter x cell / 2 bound.
  let volume = authorised_volume(&target);
  assert_eq!(volume, inside as f64 * 6.0);
  let expected = PI * 5.0 * 5.0 * 6.0;
  let bound = TAU * 5.0 * CELL / 2.0 * 6.0;
  assert!(
    (volume - expected).abs() <= bound,
    "authorised {volume} mm3, expected {expected} +/- {bound}"
  );
}

#[test]
fn two_pockets_authorise_the_deeper_one_not_the_sum() {
  let document = document(
    100.0,
    &[
      ("shallow", square(10.0, 60.0)),
      ("deep", square(40.0, 90.0)),
    ],
    vec![
      pocket("pocket 1", &["shallow"], &[], 0.0, 3.0),
      pocket("pocket 2", &["deep"], &[], 0.0, 6.0),
    ],
  );
  let target = target_for(&document, cell(), &tools(6.0)).expect("a target");

  assert_eq!(target.authorised(50, 50), 6.0, "in both: 6, not 9, not 3");
  assert_eq!(target.authorised(20, 20), 3.0, "only in the shallow pocket");
  assert_eq!(target.authorised(80, 80), 6.0, "only in the deep pocket");

  // The 20 x 20 mm overlap (400 cells) counts once, at 6 mm: 2100 x 3 + 2500 x 6.
  assert_eq!(
    authorised_volume(&target),
    (2500.0 - 400.0) * 3.0 + 2500.0 * 6.0
  );
}

#[test]
fn a_cell_no_toolpath_claims_is_authorised_nothing() {
  let (_, target) = pocket_fixture();
  // A 20 x 20 mm pocket is 400 cells; every other cell of the 100 x 100 grid is a zero.
  assert_eq!(claimed_cells(&target), 400);
  assert_eq!(target.authorised(60, 60), 0.0);
  assert_eq!(authorised_volume(&target), 400.0 * 6.0);
}

#[test]
fn a_field_cut_exactly_to_the_target_deviates_by_zero() {
  let (job, target) = pocket_fixture();
  let field = field_cut_to(&job, &target, |_, _, depth| depth);
  assert_eq!(field.volumes().removed, 400.0 * 6.0);
  assert_eq!(
    deviations(&field, &target, Length::ZERO).expect("the same grid"),
    Deviation::default()
  );
}

#[test]
fn a_gouge_and_an_uncut_are_one_difference_with_two_signs() {
  let (job, target) = pocket_fixture();
  // The block of cells in columns and rows 22..=27: 36 cells at 1 mm2, all of them inside the
  // pocket, so all of them authorised 6 mm.
  let in_block =
    |column: usize, row: usize| (22..=27).contains(&column) && (22..=27).contains(&row);

  let deep = field_cut_to(&job, &target, |column, row, depth| {
    if in_block(column, row) {
      depth + 1.0
    } else {
      depth
    }
  });
  let deviation = deviations(&deep, &target, Length::ZERO).expect("the same grid");
  assert_eq!(deviation.gouge, 36.0 * 1.0 * 1.0, "36 cells, 1 mm each");
  assert_eq!(deviation.gouge_cells, 36);
  assert_eq!(deviation.uncut, 0.0);

  let shallow = field_cut_to(&job, &target, |column, row, depth| {
    if in_block(column, row) {
      depth - 1.0
    } else {
      depth
    }
  });
  let deviation = deviations(&shallow, &target, Length::ZERO).expect("the same grid");
  assert_eq!(deviation.uncut, 36.0 * 1.0 * 1.0, "the same volume, uncut");
  assert_eq!(deviation.uncut_cells, 36);
  assert_eq!(deviation.gouge, 0.0);
}

#[test]
fn the_tolerance_decides_which_side_a_difference_falls_on() {
  let (job, target) = pocket_fixture();
  // One cell cut 0.5 mm past the 6 mm it was authorised: half of one cubic millimetre.
  let field = field_cut_to(&job, &target, |column, row, depth| {
    if column == 22 && row == 22 {
      depth + 0.5
    } else {
      depth
    }
  });

  let tight = deviations(&field, &target, Length::from_mm(0.25)).expect("the same grid");
  assert_eq!(tight.gouge, 0.5 * 1.0 * 1.0);
  assert_eq!(tight.gouge_cells, 1);
  assert_eq!(tight.uncut, 0.0);

  let loose = deviations(&field, &target, Length::from_mm(1.0)).expect("the same grid");
  assert_eq!(
    loose,
    Deviation::default(),
    "0.5 mm is inside a 1 mm tolerance"
  );
}

#[test]
fn a_field_and_a_target_on_different_grids_are_refused_by_name() {
  let document = document(
    100.0,
    &[("square", square(20.0, 40.0))],
    vec![pocket("pocket 1", &["square"], &[], 0.0, 6.0)],
  );
  let target =
    target_for(&document, Length::from_mm(2.0), &tools(6.0)).expect("a target on a 2 mm grid");
  let field = Heightfield::new(&document.job, cell()).expect("a field on a 1 mm grid");

  assert_eq!(
    deviations(&field, &target, Length::ZERO),
    Err(DeviationError::GridMismatch {
      field_columns: 100,
      field_rows: 100,
      field_cell: 1.0,
      target_columns: 50,
      target_rows: 50,
      target_cell: 2.0,
    })
  );
}

#[test]
fn the_authorised_depth_is_the_depth_the_last_pass_reaches() {
  // `start_depth` is where a pass sequence begins, not material added on top of the cut:
  // `cam-toolpath` walks `pass_depths(start_depth, cut_depth, ..)` and lands the last pass exactly
  // on `cut_depth` (`crates/cam-toolpath/src/lib.rs:264`, `docs/m4-toolpaths.md:60`). Authorising
  // `start_depth + cut_depth` would claim 2 mm the emitter never cuts, and report it as uncut.
  let document = document(
    100.0,
    &[("square", square(20.0, 40.0))],
    vec![pocket("pocket 1", &["square"], &[], 2.0, 6.0)],
  );
  let target = target_for(&document, cell(), &tools(6.0)).expect("a target");
  assert_eq!(target.authorised(30, 30), 6.0);
}

#[test]
fn an_on_profile_ignores_the_allowance_the_emitter_ignores() {
  // `lateral_offset` in `cam-toolpath` runs the tool centre on the vector whatever the allowance
  // says (`ProfileSide::On => Length::ZERO`), so the band is the tool's own width either side.
  let document = document(
    120.0,
    &[("square", square(10.0, 110.0))],
    vec![profile(
      "profile 1",
      &["square"],
      ProfileSide::On,
      1.0,
      6.0,
      Vec::new(),
    )],
  );
  let target = target_for(&document, cell(), &tools(6.0)).expect("a target");

  assert_eq!(target.authorised(112, 60), 6.0, "2.5 mm outside the edge");
  assert_eq!(
    target.authorised(114, 60),
    0.0,
    "4.5 mm outside, past the radius"
  );
  assert_eq!(target.authorised(50, 60), 0.0, "the middle of the part");

  // The band is 3 mm either side of the vector, not 3 + 1: 106^2 - 94^2 cells at 6 mm deep.
  assert_eq!(
    authorised_volume(&target),
    (106.0 * 106.0 - 94.0 * 94.0) * 6.0
  );
}

#[test]
fn a_profile_without_its_tool_diameter_is_refused_by_name() {
  let document = document(
    120.0,
    &[("square", square(10.0, 110.0))],
    vec![profile(
      "profile 1",
      &["square"],
      ProfileSide::Outside,
      0.0,
      6.0,
      Vec::new(),
    )],
  );
  assert_eq!(
    target_for(&document, cell(), &ToolDiameters::default()),
    Err(TargetError::MissingTool {
      toolpath: "profile 1".to_owned(),
      tool_number: 1,
    })
  );
}
