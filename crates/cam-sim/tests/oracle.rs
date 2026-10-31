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
  ChamferParams, Document, DrillParams, Entity, EntityKind, Job, Layer, Operation, OriginCorner,
  PocketParams, PocketStrategy, ProfileParams, ProfileSide, Toolpath,
};
use cam_move::{Move, MoveStream, Point3, Segment, ToolShape};
use cam_sim::target::{deviations, target_for, Deviation, ToolShapes};
use cam_sim::{sweep, Heightfield};
use cam_units::{Angle, Length, UnitSystem};

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

/// A chamfer's bevel: `width` millimetres wide, cut with a bit of `degrees`, `start_depth` millimetres
/// down an already-machined surface.
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

/// The tool table for a single V-bit of `degrees` and no flat, which is the shape a chamfer's band
/// and its sweep both use.
fn v_bit_tools(degrees: f64) -> ToolShapes {
  ToolShapes::new([(
    1,
    ToolShape::v_bit(Angle::from_degrees(degrees), Length::ZERO),
  )])
}

/// The moves a chamfer emits on a circular edge: rapid over the start vertex, plunge to `depth`, the
/// two half turns the circle primitive stores, and a retract to clearance.
fn chamfer_moves(centre: Point2, radius: f64, depth: f64) -> Vec<Move> {
  let start = Point2::new(centre.x + radius, centre.y);
  let opposite = Point2::new(centre.x - radius, centre.y);
  vec![
    Move::Rapid {
      to: Point3::new(start.x, start.y, SAFE_Z),
    },
    Move::Feed {
      to: Point3::new(start.x, start.y, -depth),
    },
    Move::Arc {
      to: Point3::new(opposite.x, opposite.y, -depth),
      center: centre,
      ccw: true,
    },
    Move::Arc {
      to: Point3::new(start.x, start.y, -depth),
      center: centre,
      ccw: true,
    },
    Move::Retract {
      to: Point3::new(start.x, start.y, SAFE_Z),
    },
  ]
}

/// A program that cuts `moves` with a V-bit of `degrees`: what a chamfer's segment carries (A8).
fn v_bit_program(degrees: f64, moves: Vec<Move>) -> MoveStream {
  MoveStream {
    segments: vec![Segment {
      name: "chamfer".to_owned(),
      tool_number: 1,
      tool: ToolShape::v_bit(Angle::from_degrees(degrees), Length::ZERO),
      feed_rate: 1000.0,
      plunge_rate: 300.0,
      spindle_speed: 18_000,
      moves,
    }],
    safe_z: Length::from_mm(SAFE_Z),
    home: Point2::new(0.0, 0.0),
  }
}

/// The stock the chamfer oracles are cut out of: 160 mm square, so a 40 mm radius ring and the 6 mm
/// of bevel either side of it both stay inside it.
const CHAMFER_STOCK: f64 = 160.0;
const CHAMFER_RADIUS: f64 = 40.0;
const CHAMFER_WIDTH: f64 = 6.0;
/// A 60 degree bit on purpose: its half angle is 30 degrees, so the derived depth is
/// `width / tan(30) = width * sqrt(3)`. A 90 degree bit would make `tan(45) = 1` and a derivation that
/// ignored the angle would agree with the right one, which is exactly the bug these oracles must not
/// be blind to.
const CHAMFER_DEGREES: f64 = 60.0;

/// The depth tolerance a cone's cut is judged at.
///
/// D-M5-3's coverage bound is "a cell's centre can be half a cell from the path", and the sweep's own
/// sampling can place a cell's nearest sample a quarter cell further away than the path itself. For a
/// cylinder that costs a quarter cell of *depth*; for a graded cut it costs the depth the cone gains
/// over that distance, `rise(cell / 4)` — 0.43 mm on a 60 degree bit, well over the cylinder's 0.25.
/// Judging a chamfer at the cylinder's tolerance would report a correct bevel as uncut on its steeper
/// flank, which is a tolerance chosen wrong rather than a cut made wrong.
fn cone_tolerance(tool: ToolShape) -> Length {
  Length::from_mm(tool.rise_at_radius(CELL / 4.0))
}

fn chamfer_depth(width: f64, degrees: f64, start: f64) -> f64 {
  start + width / (Angle::from_degrees(degrees).radians() / 2.0).tan()
}

fn chamfer_fixture(width: f64, degrees: f64, start: f64) -> Document {
  document(
    CHAMFER_STOCK,
    &[(
      "ring",
      circle(
        Point2::new(CHAMFER_STOCK / 2.0, CHAMFER_STOCK / 2.0),
        CHAMFER_RADIUS,
      ),
    )],
    vec![chamfer("chamfer 1", &["ring"], width, degrees, start)],
  )
}

#[test]
fn a_chamfer_around_a_circle_removes_both_flanks_of_the_bevel() {
  // A 6 mm bevel on a 60 degree bit, tip on a 40 mm radius ring. A 60 degree bit's half angle is 30
  // degrees, so the derived depth is `6 / tan(30) = 10.392` mm and the cone meets the surface 6 mm
  // either side of the vector: the groove's cross-section is a triangle of base 12 and height 10.392,
  // which is `w * d`.
  //
  // The removed volume is **both** flanks. One flank is the right triangle `w * d / 2` — the 30 degree
  // face that is 6 mm wide at the surface and 10.392 mm deep on the vector — and the bevel is
  // symmetric about the vector (D-M6-2 as amended in L7), so the cross-section is
  // `2 * (w * d / 2) = w * d` and the volume is `L * w * d`. That is the arithmetic this test asserts;
  // a formula written for one flank only would be half the groove.
  //
  // A circle is used because that volume is exact: with the depth graded radially by `g(R - r)`, the
  // `+u` and `-u` terms of the annulus cancel and the volume is `2 pi R` times the cross-section. A
  // rectangle would need the four corner overlaps subtracted, which would bury the real arithmetic.
  let centre = Point2::new(CHAMFER_STOCK / 2.0, CHAMFER_STOCK / 2.0);
  let depth = chamfer_depth(CHAMFER_WIDTH, CHAMFER_DEGREES, 0.0);
  let program = v_bit_program(
    CHAMFER_DEGREES,
    chamfer_moves(centre, CHAMFER_RADIUS, depth),
  );

  let mut field = field(CHAMFER_STOCK, CELL);
  let removal = sweep::apply(&program, &mut field);
  assert!(removal.crashes.is_empty(), "the retract is at safe Z");

  let analytic = TAU * CHAMFER_RADIUS * CHAMFER_WIDTH * depth;
  // The two graded boundaries are the rings at `R - w` and `R + w`, so their total perimeter is
  // `4 pi R` and the coverage bound of D-M5-3 is `perimeter * cell / 2 * depth`.
  let bound = 4.0 * PI * CHAMFER_RADIUS * CELL / 2.0 * depth;
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

  // The volume is the right cells rather than two errors cancelling: a chamfer cuts exactly what the
  // document authorised and nothing else, at the tolerance the sweep's own sampling stays inside.
  let document = chamfer_fixture(CHAMFER_WIDTH, CHAMFER_DEGREES, 0.0);
  let target = target_for(
    &document,
    Length::from_mm(CELL),
    &v_bit_tools(CHAMFER_DEGREES),
  )
  .expect("a target");
  let tool = ToolShape::v_bit(Angle::from_degrees(CHAMFER_DEGREES), Length::ZERO);
  let deviation = deviations(&field, &target, cone_tolerance(tool)).expect("the same grid");
  assert_eq!(
    deviation,
    Deviation::default(),
    "a clean chamfer deviates by nothing: the target derives its depth from the same parameters"
  );
}

#[test]
fn a_chamfer_one_millimetre_shallow_is_uncut_and_one_millimetre_deep_is_gouge() {
  // A9's second sign, on the chamfer: the document derives `6 / tan(30) = 10.392` mm from the 6 mm
  // bevel and the 60 degree bit, and the same ring is cut 1 mm shallow in one field and 1 mm deep in
  // the other. The
  // shallow cut must be reported as uncut and must gouge nothing; the deep one gouges and leaves
  // nothing uncut. A target that derived a different depth — or that ignored the grading — would
  // report the *correct* cut as a deviation instead, which is what this test exists to catch.
  let document = chamfer_fixture(CHAMFER_WIDTH, CHAMFER_DEGREES, 0.0);
  let target = target_for(
    &document,
    Length::from_mm(CELL),
    &v_bit_tools(CHAMFER_DEGREES),
  )
  .expect("a target");
  let tolerance = cone_tolerance(ToolShape::v_bit(
    Angle::from_degrees(CHAMFER_DEGREES),
    Length::ZERO,
  ));
  let centre = Point2::new(CHAMFER_STOCK / 2.0, CHAMFER_STOCK / 2.0);
  let depth = chamfer_depth(CHAMFER_WIDTH, CHAMFER_DEGREES, 0.0);

  let mut shallow = field(CHAMFER_STOCK, CELL);
  let removal = sweep::apply(
    &v_bit_program(
      CHAMFER_DEGREES,
      chamfer_moves(centre, CHAMFER_RADIUS, depth - 1.0),
    ),
    &mut shallow,
  );
  assert!(removal.crashes.is_empty(), "the retract is at safe Z");
  let shallow = deviations(&shallow, &target, tolerance).expect("one grid");
  assert_eq!(shallow.gouge, 0.0, "1 mm shallow gouges nothing");
  assert_eq!(shallow.gouge_cells, 0);
  assert!(
    shallow.uncut > 0.0 && shallow.uncut_cells > 0,
    "1 mm shallow is uncut: {shallow:?}"
  );

  let mut deep = field(CHAMFER_STOCK, CELL);
  let removal = sweep::apply(
    &v_bit_program(
      CHAMFER_DEGREES,
      chamfer_moves(centre, CHAMFER_RADIUS, depth + 1.0),
    ),
    &mut deep,
  );
  assert!(removal.crashes.is_empty(), "the retract is at safe Z");
  let deep = deviations(&deep, &target, tolerance).expect("one grid");
  assert_eq!(deep.uncut, 0.0, "1 mm deep leaves nothing uncut");
  assert_eq!(deep.uncut_cells, 0);
  assert!(
    deep.gouge > 0.0 && deep.gouge_cells > 0,
    "1 mm deep is a gouge: {deep:?}"
  );
}

/// A closed rectangle with explicit corners, so a fixture can put one edge exactly on the stock's
/// boundary instead of approximating it.
fn rectangle(left: f64, bottom: f64, right: f64, top: f64) -> Contour {
  Contour::from_points(
    &[
      Point2::new(left, bottom),
      Point2::new(right, bottom),
      Point2::new(right, top),
      Point2::new(left, top),
    ],
    true,
  )
  .expect("a rectangle")
}

#[test]
fn a_chamfer_on_the_stock_edge_removes_one_flank() {
  // A9's second arithmetic case (L9): when the vector lies on the stock's edge there is no material
  // on the other side of it, so only one flank is cut and the volume is `L * w * d / 2` — half the
  // both-sides case the circular fixture asserts.
  //
  // The fixture is the stock's own edge, not a clip of the test's making: the closed vector is a
  // rectangle whose top edge runs along `y = 0`, the stock's low edge, and whose other three edges sit
  // outside the sheet (`y = -20`, `x = -20`, `x = STOCK + 20`), where neither the field nor the target
  // has a cell to disagree about. The grid clips both at exactly the same cells, which is why the
  // assertion is about the model and not about a cut-off the test invented.
  let width = CHAMFER_WIDTH;
  let degrees = CHAMFER_DEGREES;
  let depth = chamfer_depth(width, degrees, 0.0);
  let tool = ToolShape::v_bit(Angle::from_degrees(degrees), Length::ZERO);
  let corner = |x: f64, y: f64| Point3::new(x, y, -depth);
  let (left, right, off) = (-20.0, CHAMFER_STOCK + 20.0, -20.0);
  let moves = vec![
    Move::Rapid {
      to: Point3::new(left, off, SAFE_Z),
    },
    Move::Feed {
      to: corner(left, off),
    },
    Move::Feed {
      to: corner(right, off),
    },
    Move::Feed {
      to: corner(right, 0.0),
    },
    Move::Feed {
      to: corner(left, 0.0),
    },
    Move::Feed {
      to: corner(left, off),
    },
    Move::Retract {
      to: Point3::new(left, off, SAFE_Z),
    },
  ];

  let mut field = field(CHAMFER_STOCK, CELL);
  let removal = sweep::apply(&v_bit_program(degrees, moves), &mut field);
  assert!(removal.crashes.is_empty(), "the retract is at safe Z");

  // One flank's cross-section is the right triangle `w * d / 2`; over the sheet's length `L` that is
  // `L * w * d / 2`. Doubling it is the both-sides case, and the bound below (one graded boundary of
  // length `L`, half a cell of coverage) is far too tight for that to hide inside.
  let analytic = CHAMFER_STOCK * width * depth / 2.0;
  let bound = CHAMFER_STOCK * CELL / 2.0 * depth;
  let removed = field.volumes().removed;
  assert!(
    (removed - analytic).abs() <= bound,
    "removed {removed} mm3 against the analytic {analytic} mm3, bound {bound} mm3"
  );
  assert_eq!(
    field.volumes().through_cells,
    0,
    "{depth} mm of material out of an 18 mm stock goes through nothing"
  );

  // And the target claims the same half: the document's chamfer over the same vector authorises what
  // the program cut, cell for cell, so the one-sided case is not merely a volume that cancels out.
  let document = document(
    CHAMFER_STOCK,
    &[("edge", rectangle(left, off, right, 0.0))],
    vec![chamfer("chamfer 1", &["edge"], width, degrees, 0.0)],
  );
  let target =
    target_for(&document, Length::from_mm(CELL), &v_bit_tools(degrees)).expect("a target");
  let deviation = deviations(&field, &target, cone_tolerance(tool)).expect("the same grid");
  assert_eq!(
    deviation,
    Deviation::default(),
    "a chamfer on the stock's edge deviates by nothing"
  );
}
