//! The oracle: for every strategy M4 shipped, the simulated removed volume against the analytic
//! expectation, with the arithmetic written out (M5 A8, S6, D-M5-3, D-M5-11).
//!
//! Every test here is written against the simulation's own published coverage bound (D-M5-3): a cell
//! is cut when its **centre** is inside the swept footprint, so a boundary can be wrong by at most
//! half a cell, and the volume error of a boundary of perimeter `p` cut `d` deep is at most
//! `p * cell / 2 * d`. Each bound below is that product written out; no tolerance is chosen to fit a
//! measurement.
//!
//! The programs are the streams the strategies emit, built here move by move: `cam-sim` must not
//! depend on `cam-toolpath` (D-M5-1), so the kernel test states what the emitter's geometry is — the
//! rings at `radius + allowance` and then one stepover apart, the rows of a raster, the plunge of a
//! drill, the two half turns of a circular profile — and asserts what the simulation does with it.
//! The same oracles are asserted again through the binary in `cam-cli/tests/cli.rs`.

use std::f64::consts::{PI, TAU};

use cam_geom::{Contour, Point2};
use cam_model::{
  Document, DrillParams, Entity, EntityKind, Job, Layer, Operation, OriginCorner, PocketParams,
  PocketStrategy, ProfileParams, ProfileSide, Toolpath,
};
use cam_move::{Move, MoveStream, Point3, Segment, ToolShape};
use cam_sim::target::{deviations, target_for, Deviation, ToolShapes};
use cam_sim::{sweep, Heightfield};
use cam_units::{Length, UnitSystem};

/// The stock the pocket, drilling and refinement fixtures are cut out of: 100 mm square, 18 mm thick.
const STOCK: f64 = 100.0;
const THICKNESS: f64 = 18.0;
/// The clearance height every program in this file retracts to.
const SAFE_Z: f64 = 5.0;
/// The tool the pocket, drilling and profile oracles cut with: 6 mm across, so 3 mm of radius.
const TOOL: f64 = 6.0;
/// The depth every fixture is cut to.
const DEPTH: f64 = 6.0;
/// The cell size of the single-resolution oracles.
const CELL: f64 = 1.0;

fn job(extent: f64) -> Job {
  Job {
    width: Length::from_mm(extent),
    height: Length::from_mm(extent),
    thickness: Length::from_mm(THICKNESS),
    units: UnitSystem::Mm,
    origin: OriginCorner::BottomLeft,
    safe_z: Length::from_mm(SAFE_Z),
  }
}

fn field(extent: f64, cell: f64) -> Heightfield {
  Heightfield::new(&job(extent), Length::from_mm(cell)).expect("the fixture fits the grid")
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

fn pocket(
  id: &str,
  entities: &[&str],
  strategy: PocketStrategy,
  stepover: f64,
  cut: f64,
) -> Toolpath {
  Toolpath {
    id: id.to_owned(),
    name: "Pocket".to_owned(),
    entity_ids: entities.iter().map(|id| (*id).to_owned()).collect(),
    island_ids: Vec::new(),
    tool_number: 1,
    operation: Operation::Pocket(PocketParams {
      strategy,
      stepover: Length::from_mm(stepover),
      cut_depth: Length::from_mm(cut),
      ..PocketParams::default()
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

fn profile(id: &str, entities: &[&str], side: ProfileSide, cut: f64) -> Toolpath {
  Toolpath {
    id: id.to_owned(),
    name: "Profile".to_owned(),
    entity_ids: entities.iter().map(|id| (*id).to_owned()).collect(),
    island_ids: Vec::new(),
    tool_number: 1,
    operation: Operation::Profile(ProfileParams {
      side,
      allowance: Length::ZERO,
      cut_depth: Length::from_mm(cut),
      ..ProfileParams::default()
    }),
  }
}

fn tools(diameter: f64) -> ToolShapes {
  ToolShapes::new([(1, ToolShape::cylinder(Length::from_mm(diameter)))])
}

fn program(diameter: f64, moves: Vec<Move>) -> MoveStream {
  MoveStream {
    segments: vec![Segment {
      name: "program".to_owned(),
      tool_number: 1,
      tool: ToolShape::cylinder(Length::from_mm(diameter)),
      feed_rate: 1000.0,
      plunge_rate: 300.0,
      spindle_speed: 12_000,
      moves,
    }],
    safe_z: Length::from_mm(SAFE_Z),
    home: Point2::new(0.0, 0.0),
  }
}

/// Run a program over a fresh field and hand the field back.
///
/// A crash is asserted away: none of these programs rams the tool, and a test that quietly simulated
/// one would report less material removed for a reason nobody asked about.
fn cut(extent: f64, cell: f64, diameter: f64, moves: Vec<Move>) -> Heightfield {
  let mut field = field(extent, cell);
  let removal = sweep::apply(&program(diameter, moves), &mut field);
  assert!(
    removal.crashes.is_empty(),
    "a fixture program must not ram the stock: {:?}",
    removal.crashes
  );
  field
}

/// The moves the offset strategy emits for one ring of one pass: rapid over it, plunge, four sides,
/// retract.
fn ring_moves(inset: f64, half: f64, depth: f64) -> Vec<Move> {
  let near = inset;
  let far = 2.0 * half - inset;
  let start = Point2::new(near, near);
  let mut moves = vec![
    Move::Rapid {
      to: Point3::new(start.x, start.y, SAFE_Z),
    },
    Move::Feed {
      to: Point3::new(start.x, start.y, -depth),
    },
  ];
  for corner in [
    Point2::new(far, near),
    Point2::new(far, far),
    Point2::new(near, far),
    start,
  ] {
    moves.push(Move::Feed {
      to: Point3::new(corner.x, corner.y, -depth),
    });
  }
  moves.push(Move::Retract {
    to: Point3::new(start.x, start.y, SAFE_Z),
  });
  moves
}

/// The moves of an offset pocket on the 100 mm square: rings at `radius + allowance` (no allowance
/// here, so 3 mm) and then one stepover apart, until the inset passes the point the tool still fits
/// at — `half - radius` — which is where the emitter's own offset collapses.
fn offset_pocket_moves(stepover: f64, depth: f64) -> Vec<Move> {
  let half = STOCK / 2.0;
  let mut moves = Vec::new();
  let mut inset = TOOL / 2.0;
  while inset <= half - TOOL / 2.0 {
    moves.extend(ring_moves(inset, half, depth));
    inset += stepover;
  }
  moves
}

/// The moves of a raster pocket on the 100 mm square: rows across the region inset by the tool's
/// radius, from its low edge to its high edge one stepover apart, each row a straight span.
fn raster_pocket_moves(stepover: f64, depth: f64) -> Vec<Move> {
  let inset = TOOL / 2.0;
  let far = STOCK - inset;
  let mut moves = Vec::new();
  let mut y = inset;
  while y <= far {
    moves.extend([
      Move::Rapid {
        to: Point3::new(inset, y, SAFE_Z),
      },
      Move::Feed {
        to: Point3::new(inset, y, -depth),
      },
      Move::Feed {
        to: Point3::new(far, y, -depth),
      },
      Move::Retract {
        to: Point3::new(far, y, SAFE_Z),
      },
    ]);
    y += stepover;
  }
  moves
}

/// The moves of a drilling program: each hole entered at clearance, plunged once and left at
/// clearance (the fixture's peck depth is zero, so one plunge per hole).
fn drill_moves(centres: &[Point2], depth: f64) -> Vec<Move> {
  let mut moves = Vec::new();
  for centre in centres {
    moves.extend([
      Move::Rapid {
        to: Point3::new(centre.x, centre.y, SAFE_Z),
      },
      Move::Feed {
        to: Point3::new(centre.x, centre.y, -depth),
      },
      Move::Retract {
        to: Point3::new(centre.x, centre.y, SAFE_Z),
      },
    ]);
  }
  moves
}

/// The three holes the drilling oracles use: r3, 6 mm deep, on lattice-friendly centres.
fn hole_centres() -> Vec<Point2> {
  vec![
    Point2::new(25.0, 25.0),
    Point2::new(75.0, 25.0),
    Point2::new(50.0, 75.0),
  ]
}

fn hole_document(cut: f64) -> Document {
  document(
    STOCK,
    &[
      ("h1", circle(Point2::new(25.0, 25.0), 3.0)),
      ("h2", circle(Point2::new(75.0, 25.0), 3.0)),
      ("h3", circle(Point2::new(50.0, 75.0), 3.0)),
    ],
    vec![drilling("Drill", &["h1", "h2", "h3"], cut)],
  )
}

/// Every cell whose centre is within `radius` of one of `centres`: the test's own lattice count.
fn cells_within(field: &Heightfield, centres: &[Point2], radius: f64) -> usize {
  let grid = field.grid();
  let mut count = 0;
  for row in 0..grid.rows {
    for column in 0..grid.columns {
      let centre = field.cell_centre(column, row);
      if centres
        .iter()
        .any(|hole| centre.distance_to(*hole) <= radius)
      {
        count += 1;
      }
    }
  }
  count
}

#[test]
fn an_offset_pocket_removes_its_region_at_its_depth() {
  // A 100 mm square pocket, 6 mm deep, cut with a 6 mm tool at a 4 mm stepover. The emitter insets
  // the tool centre by `radius + allowance` = 3 mm and then steps over 4 mm, so the rings sit at
  // insets 3, 7, ... 47 mm (the next, 51, is past `half - radius` and the offset collapses). The
  // innermost ring is 3 mm from the square's centre — exactly the tool's radius — so the middle is
  // cleared too. Twelve rings of seven moves: rapid, plunge, four sides, retract.
  let moves = offset_pocket_moves(4.0, DEPTH);
  assert_eq!(moves.len(), 12 * 7, "insets 3, 7, ... 47 mm");

  let field = cut(STOCK, CELL, TOOL, moves);

  // The analytic volume is the region's area times the depth: 100 x 100 x 6 = 60000 mm3. The bound is
  // the region's perimeter times half a cell times the depth: 400 x 1 / 2 x 6 = 1200 mm3.
  let analytic = STOCK * STOCK * DEPTH;
  let bound = 4.0 * STOCK * CELL / 2.0 * DEPTH;
  let removed = field.volumes().removed;
  assert!(
    (removed - analytic).abs() <= bound,
    "removed {removed} mm3 against the analytic {analytic} mm3, bound {bound} mm3"
  );
  assert_eq!(
    field.volumes().through_cells,
    0,
    "6 mm of material out of an 18 mm stock goes through nothing"
  );

  // And the volume is the right cells rather than two errors cancelling: the pocket cuts nothing the
  // document did not authorise, and what it leaves standing is the four corner cells a 6 mm disc
  // cannot reach into a square corner. The sliver is `r^2 - pi r^2 / 4` = 1.93 mm2 per corner, which
  // on a 1 mm lattice is the single cell at each corner: 4 x 1 x 6 = 24 mm3 of uncut, gouge zero.
  let document = document(
    STOCK,
    &[("square", square(0.0, STOCK))],
    vec![pocket(
      "Pocket",
      &["square"],
      PocketStrategy::Offset,
      4.0,
      DEPTH,
    )],
  );
  let target = target_for(&document, Length::from_mm(CELL), &tools(TOOL)).expect("a target");
  let deviation = deviations(&field, &target, Length::from_mm(CELL / 8.0)).expect("one grid");
  assert_eq!(deviation.gouge, 0.0, "the pocket never left its region");
  assert_eq!(deviation.gouge_cells, 0);
  assert_eq!(
    deviation.uncut,
    4.0 * CELL * CELL * DEPTH,
    "four corner cells, {CELL} mm2 each, {DEPTH} mm deep"
  );
  assert_eq!(deviation.uncut_cells, 4);
}

#[test]
fn the_raster_pocket_agrees_with_the_offset_one() {
  // The same region, the same 6 mm tool, the same 6 mm depth — cleared the other way. A raster cuts
  // rows across the region inset by the tool's radius (3 mm), from its low edge (3 mm) to its high
  // edge (97 mm), one stepover apart: at a 3 mm stepover the rows are 3, 6, ... 96, and the 6 mm tool
  // reaches 99 mm — a millimetre short of the far edge, because the 94 mm the inset region spans is
  // not a multiple of 3. That strip is the strategy's own row lattice (`raster_rows` steps from the
  // low edge and stops past the high one), so it is measured here rather than fitted away: what the
  // raster leaves standing is inside the published bound, which is what the oracle asserts.
  let raster = cut(STOCK, CELL, TOOL, raster_pocket_moves(3.0, DEPTH));
  let offset = cut(STOCK, CELL, TOOL, offset_pocket_moves(4.0, DEPTH));

  let analytic = STOCK * STOCK * DEPTH;
  let bound = 4.0 * STOCK * CELL / 2.0 * DEPTH;
  for (name, field) in [("raster", &raster), ("offset", &offset)] {
    let removed = field.volumes().removed;
    assert!(
      (removed - analytic).abs() <= bound,
      "{name} removed {removed} mm3 against the analytic {analytic} mm3, bound {bound} mm3"
    );
  }

  // Two strategies, one answer: the two fields differ by far less than the bound they are each held
  // to, because both clear the same region with the same tool.
  let difference = (raster.volumes().removed - offset.volumes().removed).abs();
  assert!(
    difference <= bound,
    "the raster removed {} mm3 and the offset {} mm3: {difference} mm3 apart, bound {bound} mm3",
    raster.volumes().removed,
    offset.volumes().removed
  );
  let document = document(
    STOCK,
    &[("square", square(0.0, STOCK))],
    vec![pocket(
      "Pocket",
      &["square"],
      PocketStrategy::Raster,
      3.0,
      DEPTH,
    )],
  );
  let target = target_for(&document, Length::from_mm(CELL), &tools(TOOL)).expect("a target");
  let tolerance = Length::from_mm(CELL / 8.0);
  let raster = deviations(&raster, &target, tolerance).expect("one grid");
  let offset = deviations(&offset, &target, tolerance).expect("one grid");

  // Neither cut leaves its region, and what each leaves standing is inside the bound too: the
  // offset's is the four corner cells a 6 mm disc cannot reach into a square corner (4 x 1 x 6 = 24
  // mm3), and the raster's is those plus the far strip its row lattice stops short of.
  assert_eq!(raster.gouge, 0.0, "the raster never left its region");
  assert_eq!(offset.gouge, 0.0, "the offset never left its region");
  assert_eq!(offset.uncut, 4.0 * CELL * CELL * DEPTH, "the four corners");
  assert_eq!(offset.uncut_cells, 4);
  assert!(
    raster.uncut <= bound,
    "the raster left {} mm3 standing, over the {bound} mm3 bound",
    raster.uncut
  );
}

#[test]
fn three_drilled_holes_remove_their_cylinders() {
  // Three r3 holes 6 mm deep, drilled with a 6 mm tool: the tool is the hole, so the analytic volume
  // is `pi r^2 depth` per hole. The bound is each hole's perimeter times half a cell times the depth.
  let centres = hole_centres();
  let field = cut(STOCK, CELL, TOOL, drill_moves(&centres, DEPTH));

  let analytic = centres.len() as f64 * PI * 3.0 * 3.0 * DEPTH;
  let bound = centres.len() as f64 * TAU * 3.0 * CELL / 2.0 * DEPTH;
  let removed = field.volumes().removed;
  assert!(
    (removed - analytic).abs() <= bound,
    "removed {removed} mm3 against the analytic {analytic} mm3, bound {bound} mm3"
  );

  // The three cylinders are the whole cut: nothing outside them, and nothing through the 18 mm stock.
  assert_eq!(
    removed,
    cells_within(&field, &centres, 3.0) as f64 * CELL * CELL * DEPTH,
    "the removed volume is the cells the discs cover, at the depth"
  );
  assert_eq!(field.volumes().through_cells, 0);
}

#[test]
fn a_profile_on_a_circle_removes_its_kerf_and_deviates_by_nothing() {
  // A 6 mm tool profiled Outside a 40 mm radius circle, 6 mm deep, no allowance. The emitter puts the
  // tool centre `radius + allowance` = 3 mm outside the vector, so the disc it sweeps spans the band
  // from `allowance` to `2 * radius + allowance` = 0 .. 6 mm outside the vector (D-M5-5): the removed
  // area is `pi ((40 + 6)^2 - 40^2)` = pi * 516 mm2, and the volume is that times 6 mm.
  //
  // The stock is 160 mm square so the band fits inside it, and the circle sits at its centre. The
  // profile is cut as the two half turns `cam_geom::primitive::circle` stores, which is what the
  // emitter's `push_horizontal` hands the machine for an arc.
  let extent = 160.0;
  let radius = 40.0;
  let centre = Point2::new(80.0, 80.0);
  let path = radius + TOOL / 2.0;
  let start = Point2::new(centre.x + path, centre.y);
  let opposite = Point2::new(centre.x - path, centre.y);
  let moves = vec![
    Move::Rapid {
      to: Point3::new(start.x, start.y, SAFE_Z),
    },
    Move::Feed {
      to: Point3::new(start.x, start.y, -DEPTH),
    },
    Move::Arc {
      to: Point3::new(opposite.x, opposite.y, -DEPTH),
      center: centre,
      ccw: true,
    },
    Move::Arc {
      to: Point3::new(start.x, start.y, -DEPTH),
      center: centre,
      ccw: true,
    },
    Move::Retract {
      to: Point3::new(start.x, start.y, SAFE_Z),
    },
  ];
  let field = cut(extent, CELL, TOOL, moves);

  let analytic = PI * ((radius + TOOL) * (radius + TOOL) - radius * radius) * DEPTH;
  // The band has two boundaries, the vector and the outer edge one tool's reach outside it, so the
  // perimeter the bound is written for is `2 pi R + 2 pi (R + 2r)`.
  let bound = TAU * (radius + (radius + TOOL)) * CELL / 2.0 * DEPTH;
  let removed = field.volumes().removed;
  assert!(
    (removed - analytic).abs() <= bound,
    "removed {removed} mm3 against the analytic {analytic} mm3, bound {bound} mm3"
  );

  // A circle is the one case where the statement is exact (D-M5-11): the mitered offset of a circle
  // is a circle, so the band the document authorises and the band the tool sweeps are the same
  // statement about the same cells, and the deviation is exactly zero rather than merely inside the
  // bound. It is asserted through `deviations`, so a field that cut the wrong side of the vector —
  // the half of the kerf that stands or the part that falls — could not pass.
  let document = document(
    extent,
    &[("ring", circle(centre, radius))],
    vec![profile("Profile", &["ring"], ProfileSide::Outside, DEPTH)],
  );
  let target = target_for(&document, Length::from_mm(CELL), &tools(TOOL)).expect("a target");
  let deviation = deviations(&field, &target, Length::from_mm(CELL / 8.0))
    .expect("the field and the target share a grid");
  assert_eq!(
    deviation,
    Deviation::default(),
    "a clean profile on a circle deviates by nothing"
  );
}

#[test]
fn refining_the_cell_never_grows_the_error() {
  // A6's monotone refinement, on the drilled-hole fixture: three r3 holes, 6 mm deep, simulated at
  // 2, 1 and 0.5 mm cells. The analytic volume does not depend on the cell, so what is left is the
  // coverage error the bound is written for, and it must shrink as the cells do.
  let centres = hole_centres();
  let analytic = centres.len() as f64 * PI * 3.0 * 3.0 * DEPTH;
  let errors: Vec<(f64, f64)> = [2.0, 1.0, 0.5]
    .into_iter()
    .map(|cell| {
      let field = cut(STOCK, cell, TOOL, drill_moves(&centres, DEPTH));
      let error = (field.volumes().removed - analytic).abs();
      let bound = centres.len() as f64 * TAU * 3.0 * cell / 2.0 * DEPTH;
      assert!(
        error <= bound,
        "at {cell} mm cells the error {error} mm3 is over the {bound} mm3 bound"
      );
      (cell, error)
    })
    .collect();

  for pair in errors.windows(2) {
    let (coarse_cell, coarse) = pair[0];
    let (fine_cell, fine) = pair[1];
    assert!(
      fine <= coarse,
      "refining from {coarse_cell} to {fine_cell} mm grew the error: {coarse} mm3 -> {fine} mm3"
    );
  }
  assert!(
    errors.windows(2).any(|pair| pair[1].1 < pair[0].1),
    "refinement must actually reduce the error, not merely not grow it: {errors:?}"
  );
}

#[test]
fn one_millimetre_too_shallow_is_uncut_and_one_too_deep_is_gouge() {
  // S6's assertion in both directions on the same region, so a test cannot pass by reporting the
  // wrong sign: the document authorises three r3 holes 6 mm deep, one field is cut at 5 mm and the
  // other at 7 mm. The 1 mm of material each hole's cells lose too little (or too much) is
  // `cells x cell^2 x 1 mm`, and it has to appear as uncut in the first field and as gouge in the
  // second, with the other number exactly zero.
  let document = hole_document(DEPTH);
  let cell = Length::from_mm(CELL);
  let target = target_for(&document, cell, &tools(TOOL)).expect("a target");
  let tolerance = Length::from_mm(CELL / 8.0);
  let centres = hole_centres();

  let shallow = cut(STOCK, CELL, TOOL, drill_moves(&centres, DEPTH - 1.0));
  let deep = cut(STOCK, CELL, TOOL, drill_moves(&centres, DEPTH + 1.0));
  let per_hole = cells_within(&shallow, &centres, 3.0) / centres.len();
  let expected = centres.len() as f64 * per_hole as f64 * CELL * CELL * 1.0;

  let shallow = deviations(&shallow, &target, tolerance).expect("one grid");
  assert_eq!(shallow.gouge, 0.0, "1 mm shallow gouges nothing");
  assert_eq!(
    shallow.uncut, expected,
    "{per_hole} cells per hole, 1 mm short, at {CELL} mm cells"
  );
  assert_eq!(shallow.uncut_cells, per_hole * centres.len());
  assert_eq!(shallow.gouge_cells, 0);

  let deep = deviations(&deep, &target, tolerance).expect("one grid");
  assert_eq!(deep.uncut, 0.0, "1 mm deep leaves nothing uncut");
  assert_eq!(
    deep.gouge, expected,
    "the same volume, the other sign: {per_hole} cells per hole, 1 mm over"
  );
  assert_eq!(deep.gouge_cells, per_hole * centres.len());
  assert_eq!(deep.uncut_cells, 0);
}
