//! The authorised target surface, and the two ways a field deviates from it (M5 A5, D-M5-2,
//! D-M5-5, D-M5-10, A13).
//!
//! Every number is written as arithmetic in the test: the cell edge, the area and the depth are
//! visible in the assertion, so a failure says which of the three moved. The profile band is
//! `allowance … 2·radius + allowance` away from the vector, because the emitter offsets the tool
//! *centre* by `radius + allowance` and the disc of the tool reaches one more radius past it
//! (`crates/cam-toolpath/src/lib.rs:249-258`); the pocket band is the region inset by the allowance
//! (`crates/cam-toolpath/src/pocket.rs:113-115`), because the wall it keeps is asked for.

use std::f64::consts::{PI, TAU};

use cam_geom::{Contour, Point2};
use cam_model::{
  ChamferParams, Document, DrillParams, EngraveParams, Entity, EntityKind, FlutingParams,
  InlayParams, Job, Layer, Operation, OriginCorner, PocketParams, PocketStrategy, ProfileParams,
  ProfileSide, Tab, TextureParams, Toolpath,
};
use cam_move::{Move, MoveStream, Point3, Segment, ToolShape};
use cam_sim::sweep::apply;
use cam_sim::target::{
  deviations, target_for, Deviation, DeviationError, Target, TargetError, ToolShapes,
};
use cam_sim::Heightfield;
use cam_units::{Angle, Length, UnitSystem};

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
  pocket_with(id, entities, islands, start, cut, 0.0)
}

fn pocket_with(
  id: &str,
  entities: &[&str],
  islands: &[&str],
  start: f64,
  cut: f64,
  allowance: f64,
) -> Toolpath {
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
      allowance: Length::from_mm(allowance),
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

/// A chamfer over `entities`: a bevel `width` millimetres wide on a bit of `degrees`, starting at
/// `start` millimetres below the surface. The depth is **not** written here — it is derived.
fn chamfer(id: &str, entities: &[&str], width: f64, degrees: f64, start: f64) -> Toolpath {
  Toolpath {
    id: id.to_owned(),
    name: "Chamfer".to_owned(),
    entity_ids: entities.iter().map(|id| (*id).to_owned()).collect(),
    island_ids: Vec::new(),
    tool_number: 1,
    operation: Operation::Chamfer(ChamferParams {
      chamfer_width: Length::from_mm(width),
      included_angle: Angle::from_degrees(degrees),
      start_depth: Length::from_mm(start),
    }),
  }
}

fn tools(diameter: f64) -> ToolShapes {
  ToolShapes::new([(1, ToolShape::cylinder(Length::from_mm(diameter)))])
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

  // The emitter puts the tool centre radius + allowance = 3 mm outside the vector, and the 6 mm tool
  // reaches one more radius past it, so the kerf spans 0 .. 2 * 3 + 0 = 6 mm outside (D-M5-5): a cell
  // 1.5 mm out and a cell 4.5 mm out are both authorised, and a cell 6.5 mm out is not.
  assert_eq!(target.authorised(111, 60), 6.0, "centre at (111.5, 60.5)");
  assert_eq!(target.authorised(114, 60), 6.0, "centre at (114.5, 60.5)");
  assert_eq!(target.authorised(116, 60), 0.0, "centre at (116.5, 60.5)");
  // The keep side is authorised nothing: the middle of the part stays standing, and so does the half
  // cell just inside the vector at (10.5, 60.5).
  assert_eq!(target.authorised(60, 60), 0.0);
  assert_eq!(target.authorised(10, 60), 0.0, "centre at (10.5, 60.5)");

  // The band is the 6 mm annulus: a 112 x 112 square outside a 100 x 100 one. Both land on cell
  // edges, so the cell count is 112^2 - 100^2 exactly, 1 mm2 each, 6 mm deep.
  assert_eq!(
    authorised_volume(&target),
    (112.0 * 112.0 - 100.0 * 100.0) * 6.0
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

  // The same band mirrored: 0 .. 2 * 3 = 6 mm inside the vector. A cell 1.5 mm inside the edge is
  // authorised, a cell 6.5 mm inside it, the middle of the part and the material outside the vector
  // are not.
  assert_eq!(target.authorised(11, 60), 6.0, "centre at (11.5, 60.5)");
  assert_eq!(target.authorised(15, 60), 6.0, "centre at (15.5, 60.5)");
  assert_eq!(target.authorised(16, 60), 0.0, "centre at (16.5, 60.5)");
  assert_eq!(target.authorised(60, 60), 0.0, "the middle of the part");
  assert_eq!(target.authorised(5, 60), 0.0, "outside the vector");

  // The annulus is a 100 x 100 square less an 88 x 88 one: 100^2 - 88^2 cells at 6 mm deep.
  assert_eq!(
    authorised_volume(&target),
    (100.0 * 100.0 - 88.0 * 88.0) * 6.0
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
  // A cell under that span is authorised 6 - 4 = 2 across the whole 6 mm band, not only hard against
  // the vector: columns 4..=19 (centres 4.5 .. 19.5) of the six band rows 4..=9. The three rows below
  // the vector are the corner block, whose nearest point on the perimeter is the first vertex, and
  // the three rows between are the first 10 mm of the bottom edge itself.
  for row in 4..=9 {
    for column in 4..=19 {
      assert_eq!(
        target.authorised(column, row),
        2.0,
        "tab cell ({column}, {row}) is authorised the full depth"
      );
    }
  }
  // The far edge of the band under the tab keeps its height too: 5.5 mm out on the bottom edge.
  assert_eq!(target.authorised(12, 4), 2.0, "centre at (12.5, 4.5)");
  // Past the tab's 10 mm span on the same edge — at the vector and at the band's outer edge — and on
  // the far side of the square: all cut full.
  assert_eq!(target.authorised(25, 8), 6.0, "centre at (25.5, 8.5)");
  assert_eq!(target.authorised(25, 4), 6.0, "centre at (25.5, 4.5)");
  assert_eq!(target.authorised(60, 111), 6.0, "centre at (60.5, 111.5)");

  // 96 cells keep 4 mm back (16 columns x 6 rows): the 2544 band cells at 6 mm, less 4 mm on each.
  assert_eq!(
    authorised_volume(&target),
    2544.0 * 6.0 - 96.0 * 4.0,
    "112^2 - 100^2 band cells, 16 x 6 of them under the tab"
  );
}

#[test]
fn a_tab_on_a_circular_profile_follows_the_arc_length() {
  // An r10 circle built by `primitive::circle` starts at (60, 50) and runs counter-clockwise, so its
  // 2 pi r = 62.8 mm perimeter maps straight onto the arc angle. A 10 mm tab at position 0.0 covers
  // the first 10 mm of that arc: a cell just outside the start point keeps 6 - 4 = 2, a cell on the
  // opposite side of the circle, at about 31 mm of arc, is cut to the full 6 — 3.5 mm out, which is
  // inside the 0 .. 6 mm band.
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
  assert_eq!(
    target.authorised(36, 50),
    6.0,
    "centre (36.5, 50.5), 3.5 mm out: in the band, past the tab"
  );
  assert_eq!(
    target.authorised(33, 50),
    0.0,
    "centre (33.5, 50.5), 6.5 mm out: past the band"
  );
}

#[test]
fn an_inside_profile_allowance_keeps_its_wall() {
  // The mirror of the outside case, with the emitter's negative offset: the kerf spans 1 .. 7 mm
  // *inside* the vector, so the first millimetre inwards is the wall the allowance keeps.
  let document = document(
    120.0,
    &[("square", square(10.0, 110.0))],
    vec![profile(
      "profile 1",
      &["square"],
      ProfileSide::Inside,
      1.0,
      6.0,
      Vec::new(),
    )],
  );
  let target = target_for(&document, cell(), &tools(6.0)).expect("a target");

  assert_eq!(target.authorised(10, 60), 0.0, "centre at (10.5, 60.5)");
  assert_eq!(target.authorised(11, 60), 6.0, "centre at (11.5, 60.5)");
  assert_eq!(target.authorised(17, 60), 0.0, "centre at (17.5, 60.5)");

  // The band is the 11 .. 109 square between offsets 1 and 7, less the 17 .. 103 it does not claim:
  // 98^2 - 86^2 cells at 6 mm deep.
  assert_eq!(
    authorised_volume(&target),
    (98.0 * 98.0 - 86.0 * 86.0) * 6.0
  );
}

#[test]
fn a_clean_outside_cut_through_the_sweep_leaves_no_gouge_and_no_uncut() {
  // The amendment's whole point (D-M5-5, L6): a profile that cuts its own declared band cleanly must
  // deviate by nothing. A r10 boss profiled Outside with a 6 mm tool has its centre on the r13
  // circle, so the disc the tool drags around covers the annulus 10 .. 16 mm from the centre — which
  // is exactly the band the target authorises. The narrow reading of the band authorised 0 .. 3 mm
  // and reported the outer half of this perfect cut as gouge.
  let document = document(
    100.0,
    &[("boss", circle(Point2::new(50.0, 50.0), 10.0))],
    vec![profile(
      "profile 1",
      &["boss"],
      ProfileSide::Outside,
      0.0,
      6.0,
      Vec::new(),
    )],
  );
  let target = target_for(&document, cell(), &tools(6.0)).expect("a target");

  // The program the emitter would write for that band: rapid to the start above the stock, plunge
  // there, and sweep the circle once at depth.
  let start = Point3::new(63.0, 50.0, -6.0);
  let program = MoveStream {
    segments: vec![Segment {
      name: "profile 1".to_owned(),
      tool_number: 1,
      tool: ToolShape::cylinder(Length::from_mm(6.0)),
      feed_rate: 1000.0,
      plunge_rate: 300.0,
      spindle_speed: 18_000,
      moves: vec![
        Move::Rapid {
          to: Point3::new(63.0, 50.0, 5.0),
        },
        Move::Feed { to: start },
        Move::Arc {
          to: start,
          center: Point2::new(50.0, 50.0),
          ccw: true,
        },
      ],
    }],
    safe_z: Length::from_mm(5.0),
    home: Point2::new(0.0, 0.0),
  };
  let mut field = Heightfield::new(&document.job, cell()).expect("a field");
  let removal = apply(&program, &mut field);
  assert!(
    removal.crashes.is_empty(),
    "a rapid at safe_z is not a crash, got {:?}",
    removal.crashes
  );

  assert_eq!(
    deviations(&field, &target, Length::ZERO).expect("the same grid"),
    Deviation::default()
  );
  assert_eq!(
    field.volumes().removed,
    authorised_volume(&target),
    "the cut removed exactly what the document authorised"
  );
}

#[test]
fn a_profile_allowance_leaves_its_first_millimetre_unauthorised() {
  // Outside with a 1 mm allowance and a 6 mm tool: the tool centre rides 3 + 1 = 4 mm out and the
  // kerf spans 1 .. 2 * 3 + 1 = 7 mm outside the vector. The wall the allowance keeps is material the
  // document asked for, so the target does not authorise it (D-M5-5, D-M5-10).
  let document = document(
    120.0,
    &[("square", square(10.0, 110.0))],
    vec![profile(
      "profile 1",
      &["square"],
      ProfileSide::Outside,
      1.0,
      6.0,
      Vec::new(),
    )],
  );
  let target = target_for(&document, cell(), &tools(6.0)).expect("a target");

  // 0.5 mm out is inside the wall the allowance keeps.
  assert_eq!(target.authorised(9, 60), 0.0, "centre at (9.5, 60.5)");
  // 1.5 mm out is the kerf, at the profile's full depth.
  assert_eq!(target.authorised(8, 60), 6.0, "centre at (8.5, 60.5)");
  // 7.5 mm out is past the kerf's outer edge.
  assert_eq!(target.authorised(2, 60), 0.0, "centre at (2.5, 60.5)");

  // The band is the 114 x 114 square between offsets 7 and 1, less the 102 x 102 it does not claim:
  // 114^2 - 102^2 cells at 6 mm deep.
  assert_eq!(
    authorised_volume(&target),
    (114.0 * 114.0 - 102.0 * 102.0) * 6.0
  );
}

#[test]
fn a_pocket_allowance_keeps_its_wall() {
  // The emitter insets the outermost ring by radius + allowance, so a 1 mm allowance leaves a 1 mm
  // wall standing on purpose: the target is the region inset by the allowance (D-M5-10).
  let document = document(
    100.0,
    &[("square", square(20.0, 40.0))],
    vec![pocket_with("pocket 1", &["square"], &[], 0.0, 6.0, 1.0)],
  );
  let target = target_for(&document, cell(), &tools(6.0)).expect("a target");

  // The wall's first millimetre: the cell 0.5 mm inside the boundary is not authorised.
  assert_eq!(target.authorised(20, 30), 0.0, "centre at (20.5, 30.5)");
  // Past the wall: the full depth.
  assert_eq!(target.authorised(21, 30), 6.0, "centre at (21.5, 30.5)");
  assert_eq!(target.authorised(10, 30), 0.0, "outside the pocket");

  // 18 x 18 cells of 1 mm2 at 6 mm: the 20 x 20 pocket less the 1 mm wall on every side.
  assert_eq!(authorised_volume(&target), 18.0 * 18.0 * 6.0);
}

#[test]
fn a_pocket_allowance_grows_its_islands() {
  // The inset is one parity operation, so the island grows by the allowance while the boundary
  // shrinks: the same wall in both directions (D-M5-10).
  let entities = [
    ("square", square(0.0, 100.0)),
    ("island", circle(Point2::new(50.0, 50.0), 10.0)),
  ];
  let with_allowance = document(
    100.0,
    &entities,
    vec![pocket_with(
      "pocket 1",
      &["square"],
      &["island"],
      0.0,
      6.0,
      1.0,
    )],
  );
  let target = target_for(&with_allowance, cell(), &tools(6.0)).expect("a target");
  let without = document(
    100.0,
    &entities,
    vec![pocket("pocket 1", &["square"], &["island"], 0.0, 6.0)],
  );
  let narrow = target_for(&without, cell(), &tools(6.0)).expect("a target");

  // 10.51 mm from the centre: outside the r10 island, inside the r11 the allowance grows it to. The
  // same cell is authorised the full depth when the allowance is zero.
  assert_eq!(
    target.authorised(60, 50),
    0.0,
    "centre at (60.5, 50.5), 10.51 mm from the centre"
  );
  assert_eq!(
    narrow.authorised(60, 50),
    6.0,
    "the same cell, no allowance"
  );
  // 11.51 mm out: past the grown island, so the full depth.
  assert_eq!(target.authorised(61, 50), 6.0, "centre at (61.5, 50.5)");
  // The wall at the region's own boundary: 0.5 mm in is not authorised, 1.5 mm in is.
  assert_eq!(target.authorised(0, 50), 0.0, "centre at (0.5, 50.5)");
  assert_eq!(target.authorised(1, 50), 6.0, "centre at (1.5, 50.5)");
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
    target_for(&document, cell(), &ToolShapes::default()),
    Err(TargetError::MissingTool {
      toolpath: "profile 1".to_owned(),
      tool_number: 1,
    })
  );
}

/// The tool table for a single V-bit, the way a profile's band asks for it.
fn v_bit_tools(degrees: f64, flat: f64) -> ToolShapes {
  ToolShapes::new([(
    1,
    ToolShape::v_bit(Angle::from_degrees(degrees), Length::from_mm(flat)),
  )])
}

#[test]
fn a_v_bit_on_the_vector_authorises_its_cone_and_not_its_widest_disc() {
  // A 90 degree V-bit engraved on a 20 mm square 6 mm deep. Its footprint where it meets the surface
  // reaches 6 mm either side of the vector, and its floor `x` millimetres away is `6 - x`. On the
  // bottom edge the cell centres 0.5, 3.5 and 6.5 mm from it are authorised 5.5, 2.5 and nothing.
  // Authorising the whole 6 mm across the footprint instead would report a correct V-groove as uncut.
  let document = document(
    60.0,
    &[("square", square(10.0, 30.0))],
    vec![profile(
      "engrave 1",
      &["square"],
      ProfileSide::On,
      0.0,
      6.0,
      Vec::new(),
    )],
  );
  let target = target_for(&document, cell(), &v_bit_tools(90.0, 0.0)).expect("a target");

  assert!(
    (target.authorised(20, 10) - 5.5).abs() < 1e-9,
    "half a millimetre off the vector is authorised 5.5, got {}",
    target.authorised(20, 10)
  );
  assert!(
    (target.authorised(20, 13) - 2.5).abs() < 1e-9,
    "3.5 mm off the vector is authorised 2.5, got {}",
    target.authorised(20, 13)
  );
  assert_eq!(
    target.authorised(20, 16),
    0.0,
    "6.5 mm off the vector is past the cone's reach"
  );
}

#[test]
fn a_v_bit_cutting_one_side_of_the_vector_is_refused_instead_of_approximated() {
  // A cone is graded from the vector, so a band offset to one side of it would be graded from the
  // wrong line and would call a correct cut a deviation. Refused by name (D-M6-3).
  let document = document(
    60.0,
    &[("square", square(10.0, 30.0))],
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
    target_for(&document, cell(), &v_bit_tools(90.0, 0.0)),
    Err(TargetError::ConeOnVectorOnly {
      toolpath: "profile 1".to_owned(),
      side: "outside",
    })
  );
}

#[test]
fn a_clean_cone_cut_leaves_no_gouge_and_uncut() {
  // The D-M5-5 test with a cone instead of a disc: the program the emitter would write for a 6 mm
  // pass on the vector, swept with the same shape the target was derived for. Gouge must be exactly
  // zero — the sweep never removes past its own law — and the only deviation allowed is the sweep's
  // sampling: it walks the path every half cell, so a cell's nearest sample can sit up to a quarter
  // cell further away than the path itself, which leaves that much of the cone's floor standing.
  let document = document(
    60.0,
    &[("square", square(10.0, 30.0))],
    vec![profile(
      "engrave 1",
      &["square"],
      ProfileSide::On,
      0.0,
      6.0,
      Vec::new(),
    )],
  );
  let tool = ToolShape::v_bit(Angle::from_degrees(90.0), Length::ZERO);
  let target = target_for(&document, cell(), &v_bit_tools(90.0, 0.0)).expect("a target");
  let corner = |x: f64, y: f64| Point3::new(x, y, -6.0);
  let program = MoveStream {
    segments: vec![Segment {
      name: "engrave 1".to_owned(),
      tool_number: 1,
      tool,
      feed_rate: 1000.0,
      plunge_rate: 300.0,
      spindle_speed: 18_000,
      moves: vec![
        Move::Rapid {
          to: Point3::new(10.0, 10.0, 5.0),
        },
        Move::Feed {
          to: corner(10.0, 10.0),
        },
        Move::Feed {
          to: corner(30.0, 10.0),
        },
        Move::Feed {
          to: corner(30.0, 30.0),
        },
        Move::Feed {
          to: corner(10.0, 30.0),
        },
        Move::Feed {
          to: corner(10.0, 10.0),
        },
        Move::Retract {
          to: Point3::new(10.0, 10.0, 5.0),
        },
      ],
    }],
    safe_z: Length::from_mm(5.0),
    home: Point2::new(0.0, 0.0),
  };
  let mut field = Heightfield::new(&document.job, cell()).expect("a field");
  let removal = apply(&program, &mut field);
  assert!(removal.crashes.is_empty(), "the retract is at safe Z");

  let exact = deviations(&field, &target, Length::ZERO).expect("the same grid");
  assert_eq!(exact.gouge, 0.0, "a cone never cuts past its own law");
  assert_eq!(exact.gouge_cells, 0, "no cell is gouged");

  let sampled = deviations(&field, &target, Length::from_mm(CELL / 4.0)).expect("the same grid");
  assert_eq!(
    sampled,
    Deviation::default(),
    "everything left standing is within the sweep's own sampling"
  );
}

#[test]
fn a_chamfer_authorises_the_cone_its_derived_depth_reaches() {
  // A 6 mm bevel on a 60 degree bit, tip on a 20 mm square. A 60 degree bit's half angle is 30
  // degrees, so the derived depth is `6 / tan(30)` — 10.392 mm, **not** the 6 mm a width-equals-depth
  // reading would give — and the cone reaches the surface exactly 6 mm either side of the vector.
  // Cell centres 0.5, 3.5 and 6.5 mm from the bottom edge are authorised
  // `depth - 0.5/tan(30)`, `depth - 3.5/tan(30)` and nothing. A target that used the chamfer's width,
  // or the tool's footprint without the grading, would disagree with one of the three.
  let document = document(
    60.0,
    &[("square", square(10.0, 30.0))],
    vec![chamfer("chamfer 1", &["square"], 6.0, 60.0, 0.0)],
  );
  let target = target_for(&document, cell(), &v_bit_tools(60.0, 0.0)).expect("a target");

  let half = Angle::from_degrees(60.0).radians() / 2.0;
  let depth = 6.0 / half.tan();
  let authorised = |offset: f64| depth - offset / half.tan();

  assert!(
    (target.authorised(20, 10) - authorised(0.5)).abs() < 1e-9,
    "half a millimetre off the vector is authorised {}, got {}",
    authorised(0.5),
    target.authorised(20, 10)
  );
  assert!(
    (target.authorised(20, 13) - authorised(3.5)).abs() < 1e-9,
    "3.5 mm off the vector is authorised {}, got {}",
    authorised(3.5),
    target.authorised(20, 13)
  );
  assert_eq!(
    target.authorised(20, 17),
    0.0,
    "7 mm off the vector is past the cone's reach"
  );
}

#[test]
fn a_chamfer_with_a_cylinder_tool_is_refused_by_name() {
  // A chamfer's depth comes from the bit's angle, so a tool the caller described as a cylinder
  // cannot chamfer. Approximating it as a cylinder would authorise a flat-bottomed slot the program
  // never cuts (D-M6-3), and defaulting an angle would cut a different bevel (D-M6-9).
  let document = document(
    60.0,
    &[("square", square(10.0, 30.0))],
    vec![chamfer("chamfer 1", &["square"], 6.0, 60.0, 0.0)],
  );
  assert_eq!(
    target_for(&document, cell(), &tools(6.0)),
    Err(TargetError::ChamferNeedsVBit {
      toolpath: "chamfer 1".to_owned(),
      tool_number: 1,
    })
  );
}

#[test]
fn a_chamfer_with_a_flat_tipped_bit_is_refused_by_name() {
  // D-M6-10: with a flat of diameter `f` the bevel reaches the surface at `f/2 + d*tan(half)`, so the
  // target would authorise a bevel wider than the `chamfer_width` the user typed. The derivation in
  // `ChamferParams` reads only the document (L7), so the honest answer is a refusal naming the tool,
  // not a graded band around a wider groove.
  let document = document(
    60.0,
    &[("square", square(10.0, 30.0))],
    vec![chamfer("chamfer 1", &["square"], 6.0, 60.0, 0.0)],
  );
  assert_eq!(
    target_for(&document, cell(), &v_bit_tools(60.0, 2.0)),
    Err(TargetError::ChamferNeedsSharpBit {
      toolpath: "chamfer 1".to_owned(),
      tool_number: 1,
      flat_diameter: 2.0,
    })
  );
}

/// A quick engrave over `entities` at `cut` millimetres, from `start` (D-M6-4).
fn engrave(id: &str, entities: &[&str], cut: f64, start: f64) -> Toolpath {
  Toolpath {
    id: id.to_owned(),
    name: "Engrave".to_owned(),
    entity_ids: entities.iter().map(|id| (*id).to_owned()).collect(),
    island_ids: Vec::new(),
    tool_number: 1,
    operation: Operation::Engrave(EngraveParams {
      start_depth: Length::from_mm(start),
      cut_depth: Length::from_mm(cut),
    }),
  }
}

#[test]
fn an_engrave_authorises_the_cone_either_side_of_an_open_vector() {
  // An open vector has no interior, so it is not a region at all: the claim is built from the geometry
  // (T3). A 90 degree V-bit 6 mm deep on the line `y = 10` reaches the surface 6 mm either side of it,
  // and a cell `x` mm away is authorised `6 - x`. Cell centres 0.5, 3.5 and 6.5 mm from the line are
  // authorised 5.5, 2.5 and nothing — the same three numbers T1 pinned for a profile `on` the vector,
  // now from a contour the profile would have refused as open.
  let line = Contour::from_points(&[Point2::new(10.0, 10.0), Point2::new(30.0, 10.0)], false)
    .expect("an open vector");
  let document = document(
    60.0,
    &[("line", line)],
    vec![engrave("engrave 1", &["line"], 6.0, 0.0)],
  );
  let target = target_for(&document, cell(), &v_bit_tools(90.0, 0.0)).expect("a target");

  assert!(
    (target.authorised(20, 10) - 5.5).abs() < 1e-9,
    "half a millimetre off the vector is authorised 5.5, got {}",
    target.authorised(20, 10)
  );
  assert!(
    (target.authorised(20, 13) - 2.5).abs() < 1e-9,
    "3.5 mm off the vector is authorised 2.5, got {}",
    target.authorised(20, 13)
  );
  assert_eq!(
    target.authorised(20, 16),
    0.0,
    "6.5 mm off the vector is past the cone's reach"
  );
  // The claim is a capsule around the vector, not a strip across the sheet: a cell half a millimetre
  // past the vector's last vertex is already `sqrt(6.5^2 + 0.5^2)` from it, which is past the cone's
  // reach of 6.
  assert_eq!(
    target.authorised(36, 10),
    0.0,
    "past the end of an open vector nothing is authorised"
  );
}

#[test]
fn an_engrave_with_an_end_mill_authorises_a_uniform_band_and_not_a_region() {
  // A8's fall-out for the third strategy, asserted rather than assumed: a cylinder's `rise` is zero at
  // every radius, so the cone law degenerates to one depth across the footprint the tool sweeps. The
  // footprint is the *band* — engraving does not clear — so the inside of a closed contour stays
  // standing even though the contour is closed, which is where this differs from a pocket.
  let document = document(
    60.0,
    &[("square", square(10.0, 50.0))],
    vec![engrave("engrave 1", &["square"], 6.0, 0.0)],
  );
  let target = target_for(&document, cell(), &tools(6.0)).expect("a target");

  // Cell centres 0.5 and 2.5 mm inside the bottom edge are within the 3 mm radius, at the whole depth
  // and the same number both times: that is what "uniform" means here.
  assert_eq!(target.authorised(30, 10), 6.0, "0.5 mm inside the band");
  assert_eq!(target.authorised(30, 12), 6.0, "2.5 mm inside the band");
  // 3.5 mm away is outside the footprint. The cell window is the contour's bounds widened by the
  // radius, and its corner is further than the radius from the vector, so the window alone cannot be
  // the claim: the distance has to be checked, exactly as the sweep checks it.
  assert_eq!(target.authorised(30, 13), 0.0, "3.5 mm away is outside");
  // The middle of the square is 20 mm from every edge, so an engrave leaves it standing.
  assert_eq!(
    target.authorised(30, 30),
    0.0,
    "a closed engrave grooves the loop and clears nothing inside it"
  );
  // The far edge is claimed by the same law as the near one.
  assert_eq!(target.authorised(30, 49), 6.0, "0.5 mm inside the top edge");
}

// --- fluting (M6 A4, D-M6-5) ------------------------------------------------------------------

/// A flute over `entities` at `cut` millimetres from `start`, ramped in and out over `ramp`.
fn fluting(id: &str, entities: &[&str], cut: f64, start: f64, ramp: f64) -> Toolpath {
  Toolpath {
    id: id.to_owned(),
    name: "Fluting".to_owned(),
    entity_ids: entities.iter().map(|id| (*id).to_owned()).collect(),
    island_ids: Vec::new(),
    tool_number: 1,
    operation: Operation::Fluting(FlutingParams {
      start_depth: Length::from_mm(start),
      cut_depth: Length::from_mm(cut),
      ramp: Length::from_mm(ramp),
    }),
  }
}

#[test]
fn a_ramped_flute_authorises_the_ramp_law_along_the_vector() {
  // A4's claim is a *depth along the path*, not a depth at a distance (D-M6-5), and this is the test
  // that makes the difference visible. A 100 mm vector at `y = 80.5` with `ramp = 10` and a 6 mm deep
  // flute authorises `min(6, 6·s/10)` on the way in and `min(6, 6·(100 − s)/10)` on the way out, where
  // `s` is the arc length of the cell's own point on the vector: half the depth 5 mm along, the full
  // depth at 50 mm, half again at 95 mm. The vector sits on `y = 80.5` so that a row of cell centres
  // lies exactly on it and the arc lengths are whole millimetres.
  //
  // The bit is a 90 degree V-bit on purpose. The law is stated at the tool's tip, and the claim is the
  // deepest depth the tool's *footprint* can reach at a cell — a bit whose flank climbs faster than the
  // ramp does reaches deepest at the tip's own point, so the number below is the law's. An end mill is
  // not like that: its flat bottom is still in the cut 3 mm further along the vector, at a greater
  // depth, which the second half of this test asserts. Both numbers are the same law; the difference is
  // the tool.
  let vector = Contour::from_points(&[Point2::new(10.5, 80.5), Point2::new(110.5, 80.5)], false)
    .expect("an open vector");
  let document = document(
    160.0,
    &[("vector", vector)],
    vec![fluting("fluting 1", &["vector"], 6.0, 0.0, 10.0)],
  );

  let target = target_for(&document, cell(), &v_bit_tools(90.0, 0.0)).expect("a target");
  // Column 15 holds the cell centre (15.5, 80.5): 5 mm along the vector, so half the depth.
  assert!(
    (target.authorised(15, 80) - 3.0).abs() < 1e-9,
    "5 mm along a 10 mm ramp is authorised half of 6 mm, got {}",
    target.authorised(15, 80)
  );
  // Column 60 is 50 mm along: past the entry's ramp, so the whole depth.
  assert_eq!(
    target.authorised(60, 80),
    6.0,
    "50 mm along is past the ramp and authorised the full depth"
  );
  // Column 105 is 95 mm along: 5 mm from the far end, so the exit's ramp authorises half again.
  assert!(
    (target.authorised(105, 80) - 3.0).abs() < 1e-9,
    "95 mm along is authorised half of 6 mm, got {}",
    target.authorised(105, 80)
  );
  // The plateau is flat: the same full depth at 25 mm as at 50.
  assert_eq!(target.authorised(35, 80), 6.0, "the plateau is one depth");
  // The two ends of the vector are where the ramp meets the surface, so they are authorised nothing.
  assert_eq!(
    target.authorised(10, 80),
    0.0,
    "the entry's first millimetre is at the surface"
  );
  assert_eq!(
    target.authorised(110, 80),
    0.0,
    "the exit's last millimetre is back at the surface"
  );
  // Across the groove the cone's own flank grades the claim: a cell 2 mm off the vector is authorised
  // 6 − 2, and one 6 mm off it — the cone's reach at this depth — nothing. The vector lies on a row of
  // cell centres, so the lateral distances on this grid are whole millimetres.
  assert!(
    (target.authorised(60, 82) - 4.0).abs() < 1e-9,
    "2 mm off a 90 degree bit is authorised the flank's depth, got {}",
    target.authorised(60, 82)
  );
  assert_eq!(
    target.authorised(60, 86),
    0.0,
    "6 mm off the vector is past the cone's reach"
  );

  // The same document cut by a 6 mm end mill. The cell 5 mm along is authorised 4.8 and not 3: the
  // tool's flat bottom reaches 3 mm further along the vector, where the ramp law is 3 mm deeper, and
  // that material is genuinely removed. This is `claim_flute`'s footprint, and the reason a correct
  // ramped flute cut by an end mill would otherwise be reported as a gouge along both its ramps.
  let milled = target_for(&document, cell(), &tools(6.0)).expect("a target");
  assert!(
    (milled.authorised(15, 80) - 4.8).abs() < 1e-9,
    "an end mill's footprint reaches 3 mm further along the ramp, got {}",
    milled.authorised(15, 80)
  );
  assert_eq!(
    milled.authorised(60, 80),
    6.0,
    "and the plateau is the same full depth for both tools"
  );
  assert_eq!(
    milled.authorised(60, 84),
    0.0,
    "3.5 mm off a 3 mm radius is outside the footprint, flat bottom or not"
  );
}

#[test]
fn a_flute_with_no_ramp_authorises_a_band_and_not_a_region() {
  // The degenerate ramp D-M6-5 keeps: with `ramp = 0` the whole vector is the plateau, so the claim is
  // one uniform depth across the tool's footprint — a band around the vector, never a filled region,
  // even though the vector is a closed contour. The material inside the loop stands.
  let document = document(
    60.0,
    &[("square", square(10.0, 50.0))],
    vec![fluting("fluting 1", &["square"], 6.0, 0.0, 0.0)],
  );
  let target = target_for(&document, cell(), &tools(6.0)).expect("a target");

  assert_eq!(target.authorised(30, 10), 6.0, "0.5 mm inside the band");
  assert_eq!(target.authorised(30, 12), 6.0, "2.5 mm inside the band");
  assert_eq!(target.authorised(30, 9), 6.0, "0.5 mm outside the band");
  assert_eq!(target.authorised(30, 13), 0.0, "3.5 mm away is outside");
  assert_eq!(
    target.authorised(30, 30),
    0.0,
    "the middle of a closed flute's loop is not cleared"
  );
  assert_eq!(
    target.authorised(30, 50),
    6.0,
    "the far edge is claimed by the same law as the near one"
  );
}

/// The 60 x 50 mm sheet the vertex fixture is cut out of, with one 40 x 30 mm loop at (20, 20)-(60, 50).
///
/// The loop's first vertex is **inside** the stock, which is what makes it the case that matters: a
/// cell just outside that vertex is reached by the tool's flank on the ramp, and the loop's other three
/// corners sit in the plateau, where a claim stated at the corner itself is deep enough by luck.
fn vertex_fixture(ramp: f64) -> (Job, Document) {
  let mut sheet = job(60.0);
  sheet.height = Length::from_mm(50.0);
  let loop_ = Contour::from_points(
    &[
      Point2::new(20.0, 20.0),
      Point2::new(60.0, 20.0),
      Point2::new(60.0, 50.0),
      Point2::new(20.0, 50.0),
    ],
    true,
  )
  .expect("a rectangle");
  let mut document = Document::new(sheet);
  document.layers.push(Layer {
    id: "L1".to_owned(),
    name: "Part".to_owned(),
    visible: true,
    entities: vec![Entity {
      id: "E1".to_owned(),
      kind: EntityKind::Contour { contour: loop_ },
    }],
  });
  document
    .toolpaths
    .push(fluting("fluting 1", &["E1"], 6.0, 0.0, ramp));
  (sheet, document)
}

/// The program a flute emits on the vertex fixture: a plunge at the loop's first vertex, one ramp in
/// over `ramp` millimetres of the bottom edge, the plateau round the other three sides and one ramp out.
fn vertex_moves(ramp: f64) -> Vec<Move> {
  let corner = |x: f64, y: f64, z: f64| Point3::new(x, y, z);
  vec![
    Move::Rapid {
      to: corner(20.0, 20.0, 5.0),
    },
    Move::Feed {
      to: corner(20.0, 20.0, 0.0),
    },
    Move::Feed {
      to: corner(20.0 + ramp, 20.0, -6.0),
    },
    Move::Feed {
      to: corner(60.0, 20.0, -6.0),
    },
    Move::Feed {
      to: corner(60.0, 50.0, -6.0),
    },
    Move::Feed {
      to: corner(20.0, 50.0, -6.0),
    },
    Move::Feed {
      to: corner(20.0, 50.0 - (30.0 - ramp), -6.0),
    },
    Move::Feed {
      to: corner(20.0, 20.0, 0.0),
    },
    Move::Retract {
      to: corner(20.0, 20.0, 5.0),
    },
  ]
}

#[test]
fn a_flutes_claim_reaches_past_its_own_start_vertex_where_the_tool_lands() {
  // The defect this pins: a cell beyond a contour vertex — here (18.5, 18.5), just outside the loop's
  // first corner — is reached by the cutting flank on the entry ramp, but the claim over-authorised
  // nothing there. Its footprint candidate was `±sqrt(reach² − d²)` measured from the cell's *nearest*
  // point on each segment, which is the half-width of a chord centred on a perpendicular foot and is
  // only right while that foot is interior to the segment. Beyond a vertex the foot clamps to the
  // endpoint, the reachable stretch is shorter and one-sided, and the far end of it — where the law is
  // deepest — was not a candidate at all.
  //
  // The exact number: from the corner (20, 20) the bottom edge is reachable for `t` satisfying
  // `1.5² + 2·1.5·t + t² ≤ 3²`, i.e. `t ≤ (−3 + sqrt(27)) / 2 = 1.0981` mm, so the claim there is the
  // entry ramp at that arc length, `6 · 1.0981 / 10 = 0.6588`. The left edge's exit ramp gives the same
  // by symmetry, and the sweep cut the cell 0.625 mm deep.
  let (sheet, document) = vertex_fixture(10.0);
  let target = target_for(&document, cell(), &tools(6.0)).expect("a target");
  let claimed = target.authorised(18, 18);
  assert!(
    (claimed - 6.0 * ((-3.0 + 27.0_f64.sqrt()) / 2.0) / 10.0).abs() < 1e-9,
    "the cell beyond the start vertex is authorised the ramp at the reachable end of the bottom edge: \
     got {claimed}"
  );

  let program = MoveStream {
    segments: vec![Segment {
      name: "fluting 1".to_owned(),
      tool_number: 1,
      tool: ToolShape::cylinder(Length::from_mm(6.0)),
      feed_rate: 100.0,
      plunge_rate: 30.0,
      spindle_speed: 18_000,
      moves: vertex_moves(10.0),
    }],
    safe_z: Length::from_mm(5.0),
    home: Point2::new(0.0, 0.0),
  };
  let mut field = Heightfield::new(&sheet, cell()).expect("the field fits");
  apply(&program, &mut field);
  let deviation = deviations(&field, &target, Length::from_mm(CELL / 8.0)).expect("one grid");
  assert_eq!(
    deviation,
    Deviation::default(),
    "a correct 10 mm-ramped flute deviates by nothing, corners included"
  );
}

#[test]
fn a_steep_flutes_claim_reaches_past_its_start_vertex_too() {
  // The same fixture with a 1 mm ramp: the reachable stretch beyond the vertex is the same geometry, so
  // the same candidate has to reach it. This variant is also the one whose gouge the finer sampling
  // exposed last round — 1.750 mm3 over two cells — so it is the regression that keeps both fixed.
  let (sheet, document) = vertex_fixture(1.0);
  let target = target_for(&document, cell(), &tools(6.0)).expect("a target");
  let mut field = Heightfield::new(&sheet, cell()).expect("the field fits");
  apply(
    &MoveStream {
      segments: vec![Segment {
        name: "fluting 1".to_owned(),
        tool_number: 1,
        tool: ToolShape::cylinder(Length::from_mm(6.0)),
        feed_rate: 100.0,
        plunge_rate: 30.0,
        spindle_speed: 18_000,
        moves: vertex_moves(1.0),
      }],
      safe_z: Length::from_mm(5.0),
      home: Point2::new(0.0, 0.0),
    },
    &mut field,
  );
  let deviation = deviations(&field, &target, Length::from_mm(CELL / 8.0)).expect("one grid");
  assert_eq!(
    deviation,
    Deviation::default(),
    "a correct 1 mm-ramped flute deviates by nothing either"
  );
}

#[test]
fn a_v_bits_flutes_claim_also_reaches_past_the_start_vertex() {
  // The same vertex fixture cut by a 90 degree V-bit. A cone's claim is the law at the sample's own depth
  // less the flank's rise, and its maximum over the reachable stretch can sit *inside* that stretch
  // rather than at its end — which is what the interior candidate is for. This asks the two candidates
  // together the same question the end mill was asked: is a correct V-groove flute clean at a corner?
  let (sheet, document) = vertex_fixture(10.0);
  let target = target_for(&document, cell(), &v_bit_tools(90.0, 0.0)).expect("a target");
  let mut field = Heightfield::new(&sheet, cell()).expect("the field fits");
  apply(
    &MoveStream {
      segments: vec![Segment {
        name: "fluting 1".to_owned(),
        tool_number: 1,
        tool: ToolShape::v_bit(Angle::from_degrees(90.0), Length::ZERO),
        feed_rate: 100.0,
        plunge_rate: 30.0,
        spindle_speed: 18_000,
        moves: vertex_moves(10.0),
      }],
      safe_z: Length::from_mm(5.0),
      home: Point2::new(0.0, 0.0),
    },
    &mut field,
  );
  let deviation = deviations(&field, &target, Length::from_mm(CELL / 8.0)).expect("one grid");
  assert_eq!(
    deviation,
    Deviation::default(),
    "a correct V-groove flute deviates by nothing, its start vertex included"
  );
}

// --- texture (M6 A5, D-M6-6) ------------------------------------------------------------------

/// A texture over `entities` at `cut` millimetres, `spacing` apart at `degrees` (D-M6-6).
fn texture(id: &str, entities: &[&str], cut: f64, spacing: f64, degrees: f64) -> Toolpath {
  Toolpath {
    id: id.to_owned(),
    name: "Texture".to_owned(),
    entity_ids: entities.iter().map(|id| (*id).to_owned()).collect(),
    island_ids: Vec::new(),
    tool_number: 1,
    operation: Operation::Texture(TextureParams {
      start_depth: Length::ZERO,
      cut_depth: Length::from_mm(cut),
      spacing: Length::from_mm(spacing),
      angle: Angle::from_degrees(degrees),
    }),
  }
}

#[test]
fn a_texture_authorises_a_groove_and_not_the_material_between_grooves() {
  // A 60 x 60 mm region textured at 0 degrees with a 12 mm spacing and a 6 mm end mill: the pattern is
  // the rows at y = 6, 18, 30, 42 and 54, and the claim is each groove's footprint band, 3 mm either
  // side of it — `cut_depth` across the band, because a cylinder's rise is zero at every radius (A8).
  // The material between two grooves is what a texture is *for*, so it is authorised nothing at all:
  // that is the difference between this strategy and a pocket with a very wide stepover.
  let document = document(
    60.0,
    &[("square", square(0.0, 60.0))],
    vec![texture("texture 1", &["square"], 6.0, 12.0, 0.0)],
  );
  let target = target_for(&document, cell(), &tools(6.0)).expect("a target");

  // Cell (30, 6) holds the centre (30.5, 6.5): half a millimetre from the groove at y = 6.
  assert_eq!(
    target.authorised(30, 6),
    6.0,
    "a cell on a groove is authorised the whole cut depth"
  );
  // Cell (30, 12) holds (30.5, 12.5): 5.5 mm from the nearest groove, past the 3 mm radius.
  assert_eq!(
    target.authorised(30, 12),
    0.0,
    "a cell between two grooves is authorised nothing: the material stands"
  );
  // The grooves are the pattern's, not the region's bounds: a cell just inside the region's edge, 3.5 mm
  // from the first groove, is not claimed either.
  assert_eq!(
    target.authorised(30, 0),
    0.0,
    "the region's edge is no groove"
  );
  // And every row the pattern names is claimed, including the last one. Cell (30, 54) holds
  // (30.5, 54.5), half a millimetre from the groove at y = 54 and 5.5 mm from the one at 42 — the last
  // groove is the far one, not a dropped remainder.
  assert_eq!(target.authorised(30, 54), 6.0, "the last groove is claimed");
  // Only the bands are claimed, and exactly them: five grooves 60 mm long across a 60 mm square, each
  // claimed 6 mm wide (3 mm of radius either side), is `5 * 60 * 6 = 1800` of the sheet's 3600 cells.
  // The other 1800 are the material between the grooves, which a texture is meant to leave standing —
  // so a claim that treated the region as cleared would report all of them as a gouge.
  assert_eq!(
    claimed_cells(&target),
    1800,
    "a texture claims its grooves by their own length and width, not the region"
  );
}

#[test]
fn a_forty_five_degree_texture_claims_where_the_pattern_says_and_not_an_axis_aligned_band() {
  // The pattern is clipped in the groove frame, so at 45 degrees the grooves are diagonals: `y - x`
  // constant. The same 60 x 60 region at a 12 mm spacing gives rows half a spacing inside the diamond
  // whose extent is `60 * sqrt(2) = 84.85` mm — at `y - x = -17.5736` for the row nearest the bottom
  // right corner and `y - x = -0.603` for the one through the middle.
  //
  // Two cells make the difference observable, and each is the other one's opposite:
  //
  // - (20, 2) — centre (20.5, 2.5) — is 0.3 mm from a diagonal groove and 3.5 mm from every axis-aligned
  //   row the 0 degree pattern has. Authorised the full depth at 45 degrees and nothing at 0.
  // - (30, 6) — centre (30.5, 6.5) — sits on the 0 degree pattern's groove at y = 6, and is 4.5 mm from
  //   the nearest diagonal. Authorised the full depth at 0 degrees and nothing at 45.
  //
  // A claim that rotated the region's *bounds* rather than clipping the region would fail both halves.
  let rotated = document(
    60.0,
    &[("square", square(0.0, 60.0))],
    vec![texture("texture 1", &["square"], 6.0, 12.0, 45.0)],
  );
  let target = target_for(&rotated, cell(), &tools(6.0)).expect("a target");
  assert!(
    (target.authorised(20, 2) - 6.0).abs() < 1e-9,
    "a cell on a 45 degree groove is authorised the cut depth, got {}",
    target.authorised(20, 2)
  );
  assert_eq!(
    target.authorised(30, 6),
    0.0,
    "and a cell on the axis-aligned pattern's groove is not authorised by this one"
  );

  let aligned = document(
    60.0,
    &[("square", square(0.0, 60.0))],
    vec![texture("texture 1", &["square"], 6.0, 12.0, 0.0)],
  );
  let target = target_for(&aligned, cell(), &tools(6.0)).expect("a target");
  assert_eq!(
    target.authorised(20, 2),
    0.0,
    "the 0 degree pattern does not reach the diagonal's cell"
  );
  assert_eq!(
    target.authorised(30, 6),
    6.0,
    "and it claims its own groove at y = 6"
  );
}

#[test]
fn a_texture_without_its_tool_shape_is_refused_by_name() {
  // Like a profile and an engrave, a texture's claim needs the footprint the tool sweeps, and the
  // diameter is in the library, not the document.
  let document = document(
    60.0,
    &[("square", square(0.0, 60.0))],
    vec![texture("texture 1", &["square"], 6.0, 12.0, 0.0)],
  );
  let error = target_for(&document, cell(), &ToolShapes::default()).expect_err("must refuse");
  assert_eq!(
    error,
    TargetError::MissingTool {
      toolpath: "texture 1".to_owned(),
      tool_number: 1,
    }
  );
}

// --- inlay (M6 A6, D-M6-7) -------------------------------------------------------------------

/// The female half of an inlay: the region `entities` grown by its allowance, claimed.
fn inlay_female(id: &str, entities: &[&str], allowance: f64, cut: f64) -> Toolpath {
  Toolpath {
    id: id.to_owned(),
    name: "inlay female".to_owned(),
    entity_ids: entities.iter().map(|id| (*id).to_owned()).collect(),
    island_ids: Vec::new(),
    tool_number: 1,
    operation: Operation::Inlay(InlayParams {
      allowance: Length::from_mm(allowance),
      stepover: Length::from_mm(4.0),
      cut_depth: Length::from_mm(cut),
      ..InlayParams::default()
    }),
  }
}

/// The male half of an inlay: the job rectangle with `islands` standing inside it, claimed.
fn inlay_male(id: &str, islands: &[&str], allowance: f64, cut: f64) -> Toolpath {
  Toolpath {
    id: id.to_owned(),
    name: "inlay male".to_owned(),
    entity_ids: Vec::new(),
    island_ids: islands.iter().map(|id| (*id).to_owned()).collect(),
    tool_number: 1,
    operation: Operation::Inlay(InlayParams {
      allowance: Length::from_mm(allowance),
      stepover: Length::from_mm(4.0),
      cut_depth: Length::from_mm(cut),
      ..InlayParams::default()
    }),
  }
}

/// A 40 x 40 mm region at (20, 20) inside a 100 x 100 mm job, a 2 mm allowance and a 6 mm depth.
fn inlay_fixture(female_cut: f64, male_cut: f64) -> Target {
  let document = document(
    100.0,
    &[("region", square(20.0, 60.0))],
    vec![
      inlay_female("inlay female", &["region"], 2.0, female_cut),
      inlay_male("inlay male", &["region"], 2.0, male_cut),
    ],
  );
  target_for(&document, cell(), &tools(6.0)).expect("a target")
}

#[test]
fn an_inlay_pair_authorises_the_cavity_and_the_plug() {
  // The two halves together claim the whole stock: the female the region grown by the allowance (the
  // cavity, 18..62 mm), the male the job rectangle minus the region (the material around the plug). The
  // only cells either half leaves alone are the ones neither is asked to cut, and here there are none.
  let target = inlay_fixture(6.0, 6.0);

  // A cell inside the region is the cavity: the female authorises it to the full depth.
  assert_eq!(target.authorised(40, 40), 6.0, "centre at (40.5, 40.5)");
  // A cell in the allowance ring — between the region's 20 mm wall and the cavity's 18 mm one — is
  // authorised too: it is clearance, and the female clears it.
  assert_eq!(
    target.authorised(19, 40),
    6.0,
    "centre at (19.5, 40.5), 0.5 mm outside the region"
  );
  assert_eq!(
    target.authorised(18, 40),
    6.0,
    "centre at (18.5, 40.5), 1.5 mm outside the region, still inside the 2 mm allowance"
  );
  // A cell outside the allowance ring is the male's: the plug's wall is at 20 mm and the stock's edge at
  // 0, so everything between is material the male removes.
  assert_eq!(target.authorised(5, 5), 6.0, "centre at (5.5, 5.5)");
  // And the cavity stops where the allowance says: 0.5 mm outside the grown boundary is the male's, but
  // the cavity does not reach past 18 mm — measured here as the *change* the allowance makes, below.

  // Every cell of the 100 x 100 grid is claimed, at 6 mm: the pair clears the stock between them.
  assert_eq!(claimed_cells(&target), 100 * 100);
  assert_eq!(authorised_volume(&target), 100.0 * 100.0 * 6.0);
}

#[test]
fn the_female_authorises_exactly_the_region_grown_by_the_allowance() {
  // The cavity's edge is the region's, grown: a cell whose centre is inside `R + a` is claimed and one
  // outside it is not, and a zero allowance leaves the region's own cells and nothing more (M5's rule for
  // the pocket's allowance, applied to the inlay's).
  let widened = inlay_fixture(6.0, 0.0);
  // Columns 18..61 have centres 18.5..61.5, inside the grown 18..62 boundary.
  assert_eq!(widened.authorised(18, 40), 6.0, "centre at (18.5, 40.5)");
  assert_eq!(widened.authorised(17, 40), 0.0, "centre at (17.5, 40.5)");
  assert_eq!(widened.authorised(61, 40), 6.0, "centre at (61.5, 40.5)");
  assert_eq!(widened.authorised(62, 40), 0.0, "centre at (62.5, 40.5)");
  // 44 x 44 cells of 1 mm2: the 40 x 40 region plus 2 mm on every side.
  assert_eq!(authorised_volume(&widened), 44.0 * 44.0 * 6.0);

  let plain = document(
    100.0,
    &[("region", square(20.0, 60.0))],
    vec![inlay_female("inlay female", &["region"], 0.0, 6.0)],
  );
  let plain = target_for(&plain, cell(), &tools(6.0)).expect("a target");
  assert_eq!(plain.authorised(19, 40), 0.0, "no allowance, no ring");
  assert_eq!(plain.authorised(20, 40), 6.0, "centre at (20.5, 40.5)");
  assert_eq!(authorised_volume(&plain), 40.0 * 40.0 * 6.0);
}

#[test]
fn a_cell_both_halves_claim_keeps_the_deeper_not_the_sum() {
  // The halves' clearings meet in the allowance ring (D-M6-7), so those cells are claimed twice. The rule
  // is the one two pockets already follow (D-M5-2): the deeper claim wins, never the sum. A document
  // whose halves disagree about their depth is what makes the two rules tell each other apart.
  let target = inlay_fixture(6.0, 4.0);

  // In the ring, both halves claim it: 6 from the female, 4 from the male.
  assert_eq!(
    target.authorised(19, 40),
    6.0,
    "centre at (19.5, 40.5): 6, not 10, not 4"
  );
  // Inside the region only the female claims it, so its own depth shows.
  assert_eq!(
    target.authorised(40, 40),
    6.0,
    "the cavity, from the female"
  );
  // Outside the region only the male claims it, so the shallower number shows there.
  assert_eq!(
    target.authorised(5, 5),
    4.0,
    "the plug's surroundings, from the male"
  );
}

#[test]
fn an_inlay_needs_no_tool_shape() {
  // A pocket's claim does not need the tool's geometry and neither does an inlay's, because both clear a
  // region to one depth instead of authorising a footprint: the diameter is only in the library, and a
  // caller who never supplies it still gets the pair.
  let document = document(
    100.0,
    &[("region", square(20.0, 60.0))],
    vec![
      inlay_female("inlay female", &["region"], 2.0, 6.0),
      inlay_male("inlay male", &["region"], 2.0, 6.0),
    ],
  );
  let target =
    target_for(&document, cell(), &ToolShapes::default()).expect("an inlay needs no tool");
  assert_eq!(target.authorised(40, 40), 6.0);
  assert_eq!(target.authorised(5, 5), 6.0);
}
