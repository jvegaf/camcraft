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
  ChamferParams, Document, DrillParams, EngraveParams, Entity, EntityKind, FlutingParams,
  InlayParams, Job, Layer, Operation, OriginCorner, PocketParams, PocketStrategy, ProfileParams,
  ProfileSide, TextureParams, Toolpath,
};
use cam_move::{Move, MoveStream, Point3, Segment, ToolShape};
use cam_sim::target::{deviations, target_for, Deviation, Target, ToolShapes};
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
    material: None,
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

// --- quick engrave (M6 A3, A9, D-M6-4) -------------------------------------------------------

/// A quick engrave along `entities` at `cut` millimetres, starting at `start` (D-M6-4: one pass, no
/// pass law, so there is no step to write here).
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

/// The moves an engrave emits on one open vector: rapid over its first vertex, one plunge to the
/// pass's own depth, the vector straight through, and a retract to clearance. No lead-in, no lead-out,
/// no tab lift — that is the whole of the strategy (D-M6-4).
fn engrave_moves(from: Point2, to: Point2, depth: f64) -> Vec<Move> {
  vec![
    Move::Rapid {
      to: Point3::new(from.x, from.y, SAFE_Z),
    },
    Move::Feed {
      to: Point3::new(from.x, from.y, -depth),
    },
    Move::Feed {
      to: Point3::new(to.x, to.y, -depth),
    },
    Move::Retract {
      to: Point3::new(to.x, to.y, SAFE_Z),
    },
  ]
}

/// The stock the engrave oracles are cut out of: 160 mm square, so a groove 3.5 mm either side of a
/// line through its middle stays well inside it.
const ENGRAVE_STOCK: f64 = 160.0;
/// The depth every engrave fixture is cut to.
const ENGRAVE_DEPTH: f64 = 6.0;
/// A 60 degree bit on purpose, as for the chamfer: its half angle is 30 degrees, so a depth that
/// ignored `tan(half angle)` would disagree with the right one instead of coinciding with it.
const ENGRAVE_DEGREES: f64 = 60.0;

/// The engrave fixture's vector: one straight line through the middle of the sheet, running 20 mm past
/// it at **both** ends.
///
/// The overhang is the point. A cone swept along a segment removes the wedge of its interior plus two
/// end caps whose volume is `pi * d^3 * tan^2(half) / 6` each — real geometry nobody asked this test to
/// subtract. Running the ends off the sheet puts both caps where neither the field nor the target has
/// a cell, so the grid clips them at exactly the same place and the arithmetic below stays the
/// interior's. (The chamfer oracle reached the same conclusion from the other side: it used a circle so
/// that a closed path had no ends at all.)
fn engrave_vector() -> Contour {
  Contour::from_points(
    &[
      Point2::new(-20.0, ENGRAVE_STOCK / 2.0),
      Point2::new(ENGRAVE_STOCK + 20.0, ENGRAVE_STOCK / 2.0),
    ],
    false,
  )
  .expect("an open vector")
}

fn engrave_fixture(depth: f64) -> Document {
  document(
    ENGRAVE_STOCK,
    &[("vector", engrave_vector())],
    vec![engrave("engrave 1", &["vector"], depth, 0.0)],
  )
}

#[test]
fn an_engrave_with_a_v_bit_removes_the_cone_it_sweeps() {
  // A9's arithmetic for quick engrave, first sign. A 60 degree bit at depth `d` cutting a straight
  // vector of length `L` removes `L * d^2 * tan(30)`: the groove's cross-section is the isosceles
  // triangle whose height is `d` on the vector and whose base is `2 * d * tan(30)` at the surface, so
  // its area is `d^2 * tan(30)` and the volume is that times the length.
  //
  // `L` is the sheet's width, not the vector's length: the vector overhangs both ends so the two end
  // caps fall outside the grid (see `engrave_vector`). The removed volume is therefore
  // `160 * 6^2 * tan(30) = 3325.6` mm3.
  let depth = ENGRAVE_DEPTH;
  let half = Angle::from_degrees(ENGRAVE_DEGREES).radians() / 2.0;
  let tool = ToolShape::v_bit(Angle::from_degrees(ENGRAVE_DEGREES), Length::ZERO);
  let vector = engrave_vector();
  let moves = engrave_moves(
    vector.vertices()[0].point,
    vector.vertices()[1].point,
    depth,
  );
  let mut field = field(ENGRAVE_STOCK, CELL);
  let removal = sweep::apply(&v_bit_program(ENGRAVE_DEGREES, moves), &mut field);
  assert!(removal.crashes.is_empty(), "the retract is at safe Z");

  let analytic = ENGRAVE_STOCK * depth * depth * half.tan();
  // The groove has two graded boundaries at `d * tan(half)` either side of the vector, each 160 mm
  // long. D-M5-3's bound is half a cell of coverage, and on a graded cut a boundary cell can be wrong
  // by the depth the cone gains over the half cell plus the quarter cell the sweep's sampling can add
  // — `rise(0.75 * cell)` — so the bound is `2 * L * cell * rise(0.75 * cell)`.
  let bound = 2.0 * ENGRAVE_STOCK * CELL * tool.rise_at_radius(0.75 * CELL);
  let removed = field.volumes().removed;
  assert!(
    (removed - analytic).abs() <= bound,
    "removed {removed} mm3 against the analytic {analytic} mm3, bound {bound} mm3"
  );
  assert_eq!(
    field.volumes().through_cells,
    0,
    "{depth} mm out of an 18 mm stock goes through nothing"
  );

  // And it is the right cells, not two errors cancelling: the engrave's own target authorises exactly
  // what the program cut, at the tolerance the graded cut is judged at (the same one the chamfer
  // oracle uses, `rise(cell / 4)`).
  let document = engrave_fixture(depth);
  let target = target_for(
    &document,
    Length::from_mm(CELL),
    &v_bit_tools(ENGRAVE_DEGREES),
  )
  .expect("a target");
  let deviation = deviations(&field, &target, cone_tolerance(tool)).expect("the same grid");
  assert_eq!(
    deviation,
    Deviation::default(),
    "a clean engrave deviates by nothing: the target grades the claim with the same cone"
  );
}

#[test]
fn an_engrave_with_an_end_mill_removes_a_uniform_band() {
  // A9's arithmetic for quick engrave, second sign — and A8's fall-out stated rather than assumed: a
  // cylinder's `rise` is zero at every radius, so the cone law degenerates to one depth across the
  // tool's whole footprint. A 6 mm end mill at depth 3 on a straight vector of length 160 removes
  // `L * D * d = 160 * 6 * 3 = 2880` mm3: a rectangle's worth of material, not a V-groove's.
  let depth = 3.0;
  let diameter = 6.0;
  let from = engrave_vector().vertices()[0].point;
  let to = engrave_vector().vertices()[1].point;
  let field = cut(
    ENGRAVE_STOCK,
    CELL,
    diameter,
    engrave_moves(from, to, depth),
  );

  let analytic = ENGRAVE_STOCK * diameter * depth;
  // One boundary on each side of the vector, 160 mm long, half a cell of coverage (D-M5-3).
  let bound = 2.0 * ENGRAVE_STOCK * CELL / 2.0 * depth;
  let removed = field.volumes().removed;
  assert!(
    (removed - analytic).abs() <= bound,
    "removed {removed} mm3 against the analytic {analytic} mm3, bound {bound} mm3"
  );

  // The target claims the same rectangle: the engrave's band is uniform, not graded.
  let document = engrave_fixture(depth);
  let target = target_for(&document, Length::from_mm(CELL), &tools(diameter)).expect("a target");
  let deviation = deviations(&field, &target, Length::from_mm(CELL / 4.0)).expect("the same grid");
  assert_eq!(
    deviation,
    Deviation::default(),
    "a clean engrave with an end mill deviates by nothing"
  );
}

#[test]
fn an_engrave_one_millimetre_shallow_is_uncut_and_one_millimetre_deep_is_gouge() {
  // A9's second sign, on the engrave: the document asks for a 6 mm groove with a 60 degree bit, and
  // the same vector is cut 1 mm shallow in one field and 1 mm deep in the other. The shallow cut must
  // be reported as uncut and must gouge nothing; the deep one gouges and leaves nothing uncut. A
  // target that claimed a flat band, or the wrong depth, would report the *correct* groove as a
  // deviation instead, which is what this test exists to catch.
  let document = engrave_fixture(ENGRAVE_DEPTH);
  let target = target_for(
    &document,
    Length::from_mm(CELL),
    &v_bit_tools(ENGRAVE_DEGREES),
  )
  .expect("a target");
  let tool = ToolShape::v_bit(Angle::from_degrees(ENGRAVE_DEGREES), Length::ZERO);
  let tolerance = cone_tolerance(tool);
  let from = engrave_vector().vertices()[0].point;
  let to = engrave_vector().vertices()[1].point;

  let mut shallow = field(ENGRAVE_STOCK, CELL);
  let removal = sweep::apply(
    &v_bit_program(
      ENGRAVE_DEGREES,
      engrave_moves(from, to, ENGRAVE_DEPTH - 1.0),
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

  let mut deep = field(ENGRAVE_STOCK, CELL);
  let removal = sweep::apply(
    &v_bit_program(
      ENGRAVE_DEGREES,
      engrave_moves(from, to, ENGRAVE_DEPTH + 1.0),
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

// --- fluting (M6 A4, A9, D-M6-5) --------------------------------------------------------------

/// A flute along `entities` at `cut` millimetres, ramped in and out over `ramp` (D-M6-5).
fn fluting(id: &str, entities: &[&str], cut: f64, ramp: f64) -> Toolpath {
  Toolpath {
    id: id.to_owned(),
    name: "Fluting".to_owned(),
    entity_ids: entities.iter().map(|id| (*id).to_owned()).collect(),
    island_ids: Vec::new(),
    tool_number: 1,
    operation: Operation::Fluting(FlutingParams {
      start_depth: Length::ZERO,
      cut_depth: Length::from_mm(cut),
      ramp: Length::from_mm(ramp),
    }),
  }
}

/// The moves a ramped flute emits on one straight vector: rapid over its first point, a plunge to the
/// surface, **one** move in over `ramp` millimetres, the plateau, **one** move out, and a retract.
///
/// The fixture states the strategy's geometry, and since D-M6-12 that geometry is a single linear move
/// per end: `cam-sim` interpolates `z` along a cutting move, so the floor of the ramp is graded by the
/// move itself instead of by a staircase of short ones.
fn flute_moves(from: Point2, to: Point2, depth: f64, ramp: f64) -> Vec<Move> {
  let length = from.distance_to(to);
  let along = |s: f64| from.lerp(to, s / length);
  let ramp_in = along(ramp);
  let ramp_out = along(length - ramp);
  let end = along(length);
  vec![
    Move::Rapid {
      to: Point3::new(from.x, from.y, SAFE_Z),
    },
    Move::Feed {
      to: Point3::new(from.x, from.y, 0.0),
    },
    Move::Feed {
      to: Point3::new(ramp_in.x, ramp_in.y, -depth),
    },
    Move::Feed {
      to: Point3::new(ramp_out.x, ramp_out.y, -depth),
    },
    Move::Feed {
      to: Point3::new(end.x, end.y, 0.0),
    },
    Move::Retract {
      to: Point3::new(end.x, end.y, SAFE_Z),
    },
  ]
}

/// The stock the flute oracles are cut out of: 160 mm square.
const FLUTE_STOCK: f64 = 160.0;
/// The depth every flute fixture is cut to.
const FLUTE_DEPTH: f64 = 6.0;
/// The ramp every flute fixture enters and leaves over.
const FLUTE_RAMP: f64 = 10.0;

/// The flute fixture's vector: one straight line through the middle of the sheet, from its low edge to
/// its high edge.
///
/// The ends sit **on** the sheet's edges rather than past them, unlike the engrave fixture: the ramps
/// live at the vector's ends, so pushing the ends off the sheet would push the ramps off it too and
/// leave the arithmetic below asserting the plateau alone. The two end caps are therefore inside the
/// grid, but they are cut at a depth of one step — a tenth of a millimetre where the entry ramp meets
/// the surface, nothing at all where the exit ramp does — which the bound below covers with room to
/// spare.
fn flute_vector() -> Contour {
  Contour::from_points(
    &[
      Point2::new(0.0, FLUTE_STOCK / 2.0),
      Point2::new(FLUTE_STOCK, FLUTE_STOCK / 2.0),
    ],
    false,
  )
  .expect("an open vector")
}

fn flute_fixture(depth: f64) -> Document {
  document(
    FLUTE_STOCK,
    &[("vector", flute_vector())],
    vec![fluting("fluting 1", &["vector"], depth, FLUTE_RAMP)],
  )
}

#[test]
fn a_ramped_flute_with_an_end_mill_removes_two_radius_times_depth_times_length_minus_ramp() {
  // A9's arithmetic for fluting, first sign, and D-M6-5's closed form. A cylinder of radius `R`
  // cutting a straight vector of length `L` to depth `d` removes the band within `R` of the vector,
  // so its volume is `2·R` times the depth *integrated along the path*. The depth is `d·s/ramp` over
  // the first `ramp` millimetres and `d·(L − s)/ramp` over the last, each integral `d·ramp/2`, and `d`
  // in between over `L − 2·ramp`:
  //
  //   ∫ depth ds = d·ramp/2 + d·(L − 2·ramp) + d·ramp/2 = d·(L − ramp)
  //
  // so the volume is `2·R·d·(L − ramp)`. With `R = 3`, `d = 6`, `L = 160` and `ramp = 10` that is
  // `2 · 3 · 6 · 150 = 5400` mm3 — the material of the plateau plus both ramps, which is exactly what
  // a linear ramp removes and what a uniform band at `d` over the whole vector would overstate by
  // `2·R·d·ramp = 360` mm3.
  //
  // The measurement is 5548.5 mm3 rather than 5400. The 148.5 mm3 between them is not slack: it is most
  // of `π · R² · d = 169.6` mm3 of *toes* — the material a tool's footprint adds where each ramp meets
  // the surface, because the bottom of a 6 mm end mill is still in the cut 3 mm past its tip, so the
  // groove is longer than the tip's own travel by a radius at each end. The rest is the cell grid's own
  // account of the same toes. The closed form has not moved through any of this, and neither has the
  // published bound: the staircase measured 5549.2 mm3 on this field, the half-cell sampling 5539.2, the
  // first derived sampling 5545.5 and this one 5548.5 — four samples of one geometry, converging on the
  // toes, all inside `2 · L · cell/2 · d = 960` mm3. Those toes are exactly the material a claim stated at
  // the tip alone reports as a gouge, which is why `cam_sim::target`'s flute claim looks along the path
  // (see `claim_flute`).
  //
  // The graded sampling matters here and the numbers say how much: the deepest residual on this field is
  // 0.0623 mm, half of the `cell / 8 = 0.125` the judgement uses — where the half-cell sampling left
  // 0.275 mm and the judgement reported 7.695 mm3 of this *correct* flute as uncut.
  let depth = FLUTE_DEPTH;
  let diameter = TOOL;
  let from = flute_vector().vertices()[0].point;
  let to = flute_vector().vertices()[1].point;
  let field = cut(
    FLUTE_STOCK,
    CELL,
    diameter,
    flute_moves(from, to, depth, FLUTE_RAMP),
  );

  let radius = diameter / 2.0;
  let analytic = 2.0 * radius * depth * (FLUTE_STOCK - FLUTE_RAMP);
  // The groove has one straight boundary on each side of the vector, 160 mm long, cut `depth` deep,
  // and D-M5-3's coverage bound is half a cell: `2 · L · cell/2 · d`.
  let bound = 2.0 * FLUTE_STOCK * CELL / 2.0 * depth;
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

  // And the target claims the same groove, cell for cell: the document's flute authorises the ramp
  // law along the path, so a correct ramped flute deviates by nothing at all. Authorising a uniform
  // band instead would report both ramps as uncut, which is the failure this law exists to avoid.
  let document = flute_fixture(depth);
  let target = target_for(&document, Length::from_mm(CELL), &tools(diameter)).expect("a target");
  // The judgement is M5's, unchanged: an eighth of a cell. The *sampling* is what pays for the ramp —
  // a cutting move takes enough samples that its depth residual is at most `cell / 16`
  // (`cam_sim::sweep`'s step-count derivation), so a graded floor is resolved to the tolerance it is
  // judged at and nothing has to be added to the tolerance itself.
  let tolerance = Length::from_mm(CELL / 8.0);
  let deviation = deviations(&field, &target, tolerance).expect("the same grid");
  assert_eq!(
    deviation,
    Deviation::default(),
    "a clean ramped flute deviates by nothing"
  );
}

#[test]
fn a_ramped_flute_one_millimetre_shallow_is_uncut_and_one_millimetre_deep_is_gouge() {
  // A9's second sign on the ramp law: the document asks for a 6 mm flute ramped over 10 mm, and the
  // same vector is cut 1 mm shallow in one field and 1 mm deep in the other. The shallow one must be
  // uncut and gouge nothing; the deep one must gouge and leave nothing uncut. A target that claimed
  // the full depth at the ends — a uniform band — would report the *correct* flute as uncut along both
  // ramps, which is why a test with the right sign has to be paired with a wrong one on each side.
  let document = flute_fixture(FLUTE_DEPTH);
  // The same judgement a clean flute gets: an eighth of a cell, with no sampling term to add.
  let tolerance = Length::from_mm(CELL / 8.0);
  let target = target_for(&document, Length::from_mm(CELL), &tools(TOOL)).expect("a target");
  let from = flute_vector().vertices()[0].point;
  let to = flute_vector().vertices()[1].point;

  let shallow = cut(
    FLUTE_STOCK,
    CELL,
    TOOL,
    flute_moves(from, to, FLUTE_DEPTH - 1.0, FLUTE_RAMP),
  );
  let shallow = deviations(&shallow, &target, tolerance).expect("one grid");
  assert_eq!(shallow.gouge, 0.0, "1 mm shallow gouges nothing");
  assert_eq!(shallow.gouge_cells, 0);
  assert!(
    shallow.uncut > 0.0 && shallow.uncut_cells > 0,
    "1 mm shallow is uncut: {shallow:?}"
  );

  let deep = cut(
    FLUTE_STOCK,
    CELL,
    TOOL,
    flute_moves(from, to, FLUTE_DEPTH + 1.0, FLUTE_RAMP),
  );
  let deep = deviations(&deep, &target, tolerance).expect("one grid");
  assert_eq!(deep.uncut, 0.0, "1 mm deep leaves nothing uncut");
  assert_eq!(deep.uncut_cells, 0);
  assert!(
    deep.gouge > 0.0 && deep.gouge_cells > 0,
    "1 mm deep is a gouge: {deep:?}"
  );
}

/// The same 160 mm vector fluted 6 mm deep, but ramped in and out over **1 mm** instead of 10.
///
/// A grade of 6 mm of Z per millimetre of travel: the steepest ramp the strategy allows on a 140 mm
/// loop, and the case that made a document-global tolerance keyed on the steepest grade unworkable —
/// `cell/8 + grade·cell/2` would have licensed 3.125 mm of deviation anywhere in this document.
fn steep_flute_fixture(depth: f64) -> Document {
  document(
    FLUTE_STOCK,
    &[("vector", flute_vector())],
    vec![fluting("fluting 1", &["vector"], depth, 1.0)],
  )
}

#[test]
fn a_six_millimetre_flute_over_a_one_millimetre_ramp_is_clean_at_cell_eighths_and_a_shallow_one_is_not(
) {
  // The steepest ramp a flute of this size can carry is judged at M5's eighth of a cell like any other,
  // and the residual there measures zero — not because the sampling carries it, but because a 6 mm
  // cutter's footprint spans a 1 mm ramp whole and reaches the plateau beyond it, so every cell near the
  // ramp is cut by a sample at full depth. What this test pins is the *judgement*: the shallow half of it
  // is a millimetre of missing material in a document whose steepest grade is 6, and a document-global
  // tolerance keyed on that grade (`cell/8 + grade·cell/2` = 3.125 mm) would have reported this defect as
  // nothing at all.
  let document = steep_flute_fixture(FLUTE_DEPTH);
  let target = target_for(&document, Length::from_mm(CELL), &tools(TOOL)).expect("a target");
  let tolerance = Length::from_mm(CELL / 8.0);
  let from = flute_vector().vertices()[0].point;
  let to = flute_vector().vertices()[1].point;

  let correct = cut(
    FLUTE_STOCK,
    CELL,
    TOOL,
    flute_moves(from, to, FLUTE_DEPTH, 1.0),
  );
  let clean = deviations(&correct, &target, tolerance).expect("one grid");
  assert_eq!(
    clean,
    Deviation::default(),
    "a clean 6 mm flute over a 1 mm ramp deviates by nothing at an eighth of a cell"
  );

  let shallow = cut(
    FLUTE_STOCK,
    CELL,
    TOOL,
    flute_moves(from, to, FLUTE_DEPTH - 1.0, 1.0),
  );
  let shallow = deviations(&shallow, &target, tolerance).expect("one grid");
  assert_eq!(shallow.gouge, 0.0, "1 mm shallow gouges nothing");
  assert!(
    shallow.uncut > 0.0 && shallow.uncut_cells > 0,
    "1 mm shallow is uncut even on a 1 mm ramp: {shallow:?}"
  );
}

// --- texture (M6 A5, A9, D-M6-6) --------------------------------------------------------------

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

/// The moves a texture emits for one groove: a rapid over its first vertex at safe Z, a plunge in place,
/// **one** feed along the groove, and a retract (D-M6-6).
fn groove_moves(from: Point2, to: Point2, depth: f64) -> Vec<Move> {
  vec![
    Move::Rapid {
      to: Point3::new(from.x, from.y, SAFE_Z),
    },
    Move::Feed {
      to: Point3::new(from.x, from.y, -depth),
    },
    Move::Feed {
      to: Point3::new(to.x, to.y, -depth),
    },
    Move::Retract {
      to: Point3::new(to.x, to.y, SAFE_Z),
    },
  ]
}

/// The stock the texture oracles are cut out of: 40 mm square, so the region is the whole sheet and
/// each groove's ends sit **on** the sheet's edges.
///
/// That is the engrave fixture's device (T2's L9): a groove's band is a stadium, and a stadium whose
/// ends are inside the sheet adds two half-disc toes the closed form below does not count. Clipping the
/// region to the sheet's own extent removes them, because only the rectangle of the stadium lies inside
/// the strip — so the arithmetic is the closed form and not a tolerance over the caps.
const TEXTURE_STOCK: f64 = 40.0;
/// The depth every texture fixture is cut to.
const TEXTURE_DEPTH: f64 = 6.0;
/// The distance between the texture fixtures' grooves. Wider than the 6 mm tool, so the bands do not
/// overlap and the removed volume is the sum of the grooves' own.
const TEXTURE_SPACING: f64 = 8.0;

fn texture_fixture(depth: f64) -> Document {
  document(
    TEXTURE_STOCK,
    &[("sheet", square(0.0, TEXTURE_STOCK))],
    vec![texture(
      "texture 1",
      &["sheet"],
      depth,
      TEXTURE_SPACING,
      0.0,
    )],
  )
}

/// The document's own pattern, as the strategy cuts it: `cam-model` derives it once and both consumers
/// read that one derivation (D-M6-6).
fn texture_pattern(document: &Document) -> Vec<Vec<Point2>> {
  let Operation::Texture(params) = &document.toolpaths[0].operation else {
    panic!("expected a texture toolpath");
  };
  let contours: Vec<&Contour> = document.toolpaths[0]
    .entity_ids
    .iter()
    .map(|id| {
      document
        .entity(id)
        .and_then(|entity| entity.kind.as_contour())
        .expect("a contour")
    })
    .collect();
  params.grooves(&contours)
}

#[test]
fn a_texture_with_an_end_mill_removes_two_radius_times_depth_per_millimetre_of_groove() {
  // A9's arithmetic for a texture, first sign. A cylinder of radius `R` cutting a groove of length `L`
  // to depth `d` removes the band within `R` of the groove, and on a straight groove that band is
  // `2·R` wide, so the groove removes `2·R·d` per millimetre of its own length:
  //
  //   removed = 2·R·d·Σ length(groove)
  //
  // With `R = 3`, `d = 6`, and a 40 mm square textured at 0 degrees with an 8 mm spacing, the pattern is
  // the rows at y = 4, 12, 20, 28 and 36, so `Σ length = 5 · 40 = 200` mm and the analytic volume is
  // `6 · 6 · 200 = 7200` mm3. The sum is taken from the **document's own pattern** — not from a second
  // derivation written here — so the number below is the pattern the program and the target share.
  //
  // The measured volume is 7200 exactly, and that is the fixture and not luck: the grooves end on the
  // sheet's edges, so no toe is inside it, and every band is `2·R` wide with its edges on cell
  // boundaries (`y = 1 … 7`, `9 … 15`, …), so each groove's six rows of forty cells are all in and the
  // row beyond is all out. `2·ΣL·cell/2·d = 1200` mm3 is the published coverage bound; it is written out
  // rather than needed.
  let depth = TEXTURE_DEPTH;
  let diameter = TOOL;
  let document = texture_fixture(depth);
  let pattern = texture_pattern(&document);
  assert_eq!(pattern.len(), 5, "five rows a spacing apart: {pattern:?}");

  let mut moves = Vec::new();
  for groove in &pattern {
    moves.extend(groove_moves(groove[0], groove[1], depth));
  }
  let field = cut(TEXTURE_STOCK, CELL, diameter, moves);

  let radius = diameter / 2.0;
  let total_length: f64 = pattern
    .iter()
    .map(|groove| groove[0].distance_to(groove[1]))
    .sum();
  assert!(
    (total_length - 200.0).abs() < 1e-9,
    "the pattern's own length sum: {total_length}"
  );
  let analytic = 2.0 * radius * depth * total_length;
  let bound = 2.0 * total_length * CELL / 2.0 * depth;
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

  // And the target claims the same grooves, cell for cell: the document's pattern is what it
  // authorises, so a correct texture deviates by nothing at all.
  let target = target_for(&document, Length::from_mm(CELL), &tools(diameter)).expect("a target");
  let deviation = deviations(&field, &target, Length::from_mm(CELL / 8.0)).expect("one grid");
  assert_eq!(
    deviation,
    Deviation::default(),
    "a clean texture deviates by nothing"
  );
}

#[test]
fn a_texture_one_millimetre_shallow_is_uncut_and_one_millimetre_deep_is_gouge() {
  // A9's second sign, on the pattern rather than on a single groove: the document asks for five grooves
  // 6 mm deep, and the same pattern is cut 1 mm shallow in one field and 1 mm deep in the other. The
  // shallow one must be uncut and gouge nothing; the deep one must gouge and leave nothing uncut. A
  // target that claimed the region instead of the grooves would fail the clean case next door; a target
  // that claimed the wrong angle would fail both signs at once.
  let document = texture_fixture(TEXTURE_DEPTH);
  let pattern = texture_pattern(&document);
  let tolerance = Length::from_mm(CELL / 8.0);
  let target = target_for(&document, Length::from_mm(CELL), &tools(TOOL)).expect("a target");

  let program = |depth: f64| {
    let mut moves = Vec::new();
    for groove in &pattern {
      moves.extend(groove_moves(groove[0], groove[1], depth));
    }
    moves
  };

  let shallow = cut(TEXTURE_STOCK, CELL, TOOL, program(TEXTURE_DEPTH - 1.0));
  let shallow = deviations(&shallow, &target, tolerance).expect("one grid");
  assert_eq!(shallow.gouge, 0.0, "1 mm shallow gouges nothing");
  assert_eq!(shallow.gouge_cells, 0);
  assert!(
    shallow.uncut > 0.0 && shallow.uncut_cells > 0,
    "1 mm shallow is uncut: {shallow:?}"
  );

  let deep = cut(TEXTURE_STOCK, CELL, TOOL, program(TEXTURE_DEPTH + 1.0));
  let deep = deviations(&deep, &target, tolerance).expect("one grid");
  assert_eq!(deep.uncut, 0.0, "1 mm deep leaves nothing uncut");
  assert_eq!(deep.uncut_cells, 0);
  assert!(
    deep.gouge > 0.0 && deep.gouge_cells > 0,
    "1 mm deep is a gouge: {deep:?}"
  );
}

// --- inlay (M6 A6, A9, D-M6-7) ----------------------------------------------------------------

/// The region every inlay oracle is cut from: a 38 mm square at (21, 21), so the stock has 21 mm of
/// material beside it on every side.
///
/// The numbers are chosen so the hand-built programs below are the emitter's own, move for move: the
/// male's row lattice starts at the tool radius (3 mm) and steps by [`INLAY_STEPOVER`], so it lands on
/// **every** 3 + 2k row and never on the region's grown edges (18 and 62 mm) — where the emitter itself
/// retries a row a hair to one side, which a hand-built fixture cannot copy without copying that retry.
const INLAY_MIN: f64 = 21.0;
const INLAY_SIDE: f64 = 38.0;
/// The clearance between the male plug and the female cavity, and the stepover both halves clear at.
/// Two millimetres under a 6 mm tool overlaps by 4 mm, so both lattices cover what they claim, and the
/// row lattice reaches the stock's far edge exactly (3 + 47 x 2 = 97).
const INLAY_ALLOWANCE: f64 = 2.0;
const INLAY_STEPOVER: f64 = 2.0;

/// One closed rectangular ring cut in one pass: rapid over it, plunge, four sides, retract.
fn rectangle_pass(min: Point2, max: Point2, depth: f64) -> Vec<Move> {
  let start = min;
  let mut moves = vec![
    Move::Rapid {
      to: Point3::new(start.x, start.y, SAFE_Z),
    },
    Move::Feed {
      to: Point3::new(start.x, start.y, -depth),
    },
  ];
  for corner in [
    Point2::new(max.x, min.y),
    Point2::new(max.x, max.y),
    Point2::new(min.x, max.y),
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

/// The moves the **female** half emits on the fixture's region.
///
/// The cavity is the region grown by the allowance, which the pocket's ring machinery cuts as rings
/// `radius - allowance` inside the region's own boundary (1 mm here), one stepover apart, until the
/// inset reaches half the region — the point the offset collapses at.
fn inlay_female_moves(allowance: f64, stepover: f64, depth: f64) -> Vec<Move> {
  let half = INLAY_SIDE / 2.0;
  let mut moves = Vec::new();
  let mut inset = TOOL / 2.0 - allowance;
  while inset < half {
    moves.extend(rectangle_pass(
      Point2::new(INLAY_MIN + inset, INLAY_MIN + inset),
      Point2::new(
        INLAY_MIN + INLAY_SIDE - inset,
        INLAY_MIN + INLAY_SIDE - inset,
      ),
      depth,
    ));
    inset += stepover;
  }
  moves
}

/// The moves the **male** half emits: the job rectangle with the region standing as an island.
///
/// The male clears with the pocket's **raster** pattern, and this fixture is why: the material around a
/// region is a frame, the offset ring lattice converges on the frame's medial axis and stops there, and
/// the four diagonal corners between the last two rings are never cut — 2808 of the frame's 8400 mm2 on a
/// 40 mm region in the same stock at a 4 mm stepover, which is a plug still attached to the stock at four
/// points. Rows have no such blind spot: each is clipped to the frame by the kernel's raster spans, so
/// every row reaches both walls.
///
/// The rows are the inset frame's own: the stock inset by the tool radius (3 mm) crossed `stepover` at a
/// time to the far edge, and a row that crosses the region is cut as **two** spans, never one (A7's
/// parity). The region's grown bound is 18..62 mm here and the lattice never lands on it, so this is the
/// emitter's program move for move.
fn inlay_male_moves(stepover: f64, depth: f64) -> Vec<Move> {
  let near = TOOL / 2.0;
  let far = STOCK - TOOL / 2.0;
  let island_near = INLAY_MIN - TOOL / 2.0;
  let island_far = INLAY_MIN + INLAY_SIDE + TOOL / 2.0;
  let mut moves = Vec::new();
  let mut y = near;
  while y <= far {
    let crossing = y > island_near && y < island_far;
    let spans = if crossing {
      vec![(near, island_near), (island_far, far)]
    } else {
      vec![(near, far)]
    };
    for (left, right) in spans {
      moves.extend([
        Move::Rapid {
          to: Point3::new(left, y, SAFE_Z),
        },
        Move::Feed {
          to: Point3::new(left, y, -depth),
        },
        Move::Feed {
          to: Point3::new(right, y, -depth),
        },
        Move::Retract {
          to: Point3::new(right, y, SAFE_Z),
        },
      ]);
    }
    y += stepover;
  }
  moves
}

/// The document one inlay half describes, as `toolpath --kind inlay` writes it.
///
/// One half at a time, so each half's program is judged against the half's own claim: a target built
/// from the whole pair would report the other half's cells as uncut and drown the number the test is
/// about.
fn inlay_document(female: bool, cut: f64) -> Document {
  let region = square(INLAY_MIN, INLAY_MIN + INLAY_SIDE);
  let mut document = document(STOCK, &[("region", region)], Vec::new());
  let params = InlayParams {
    allowance: Length::from_mm(INLAY_ALLOWANCE),
    stepover: Length::from_mm(INLAY_STEPOVER),
    cut_depth: Length::from_mm(cut),
    ..InlayParams::default()
  };
  document.toolpaths = vec![if female {
    Toolpath {
      id: "inlay female".to_owned(),
      name: "inlay female".to_owned(),
      entity_ids: vec!["region".to_owned()],
      island_ids: Vec::new(),
      tool_number: 1,
      operation: Operation::Inlay(params),
    }
  } else {
    Toolpath {
      id: "inlay male".to_owned(),
      name: "inlay male".to_owned(),
      entity_ids: Vec::new(),
      island_ids: vec!["region".to_owned()],
      tool_number: 1,
      operation: Operation::Inlay(params),
    }
  }];
  document
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

#[test]
fn an_inlay_removes_both_halves_analytically() {
  // A9 for the pair, in the M5 oracle's own shape: the removed volume of each half against the analytic
  // volume of the material it is meant to clear, summed from the document's own geometry.
  //
  // The cavity is the region grown by the allowance. The grown region is **mitered**, so its area is
  // exact rather than approximate: a square grown by `a` is `(side + 2a)²`, because each of the four
  // corners picks up `a²` and no rounding arc — `Region::offset` joins mitre, and D-M5-11's precedent (a
  // tolerance is not widened over a shape the kernel describes exactly) is why this is arithmetic and not
  // a bound. The male's is the stock minus the region, a rectangle difference and exact for the same
  // reason.
  let tolerance = Length::from_mm(CELL / 8.0);

  let grown_side = INLAY_SIDE + 2.0 * INLAY_ALLOWANCE;
  let female_target = target_for(
    &inlay_document(true, DEPTH),
    Length::from_mm(CELL),
    &tools(TOOL),
  )
  .expect("a target");
  let female = cut(
    STOCK,
    CELL,
    TOOL,
    inlay_female_moves(INLAY_ALLOWANCE, INLAY_STEPOVER, DEPTH),
  );
  let removed = female.volumes().removed;
  let analytic = grown_side * grown_side * DEPTH;
  // The coverage bound this file already uses: the region's perimeter times half a cell, times the depth.
  let bound = 4.0 * grown_side * CELL / 2.0 * DEPTH;
  assert!(
    (removed - analytic).abs() <= bound,
    "the female removed {removed} mm3 against the analytic {analytic} mm3, bound {bound} mm3"
  );
  assert_eq!(
    female.volumes().through_cells,
    0,
    "6 mm of material out of an 18 mm stock goes through nothing"
  );

  // And it cuts what the cavity authorises and nothing else: the four cells a 6 mm disc cannot reach into
  // the grown square's mitered corners stand — `4 x 1 x 6 = 24 mm3`, the pocket oracle's own sliver.
  let female = deviations(&female, &female_target, tolerance).expect("one grid");
  assert_eq!(female.gouge, 0.0, "the female never left the cavity");
  assert_eq!(female.gouge_cells, 0);
  assert_eq!(
    female.uncut,
    4.0 * CELL * CELL * DEPTH,
    "four corner cells of the grown region"
  );
  assert_eq!(female.uncut_cells, 4);

  let male_target = target_for(
    &inlay_document(false, DEPTH),
    Length::from_mm(CELL),
    &tools(TOOL),
  )
  .expect("a target");
  let male = cut(STOCK, CELL, TOOL, inlay_male_moves(INLAY_STEPOVER, DEPTH));
  let removed = male.volumes().removed;
  let analytic = (STOCK * STOCK - INLAY_SIDE * INLAY_SIDE) * DEPTH;
  let bound = (4.0 * STOCK + 4.0 * INLAY_SIDE) * CELL / 2.0 * DEPTH;
  assert!(
    (removed - analytic).abs() <= bound,
    "the male removed {removed} mm3 against the analytic {analytic} mm3, bound {bound} mm3"
  );

  // The male's residue is the pocket's published coverage bound and not nothing: its row lattice leaves
  // the stock's four corner cells **and** a rind of up to one stepover along the two walls of the region
  // that run parallel to the rows. Measured here: 480 mm3 over 80 cells — 76 of them the 1 mm strip along
  // the region's low and high walls, whose nearest row is at `wall - radius - 1` rather than at
  // `wall - radius`. The same frame cleared with the offset pattern instead leaves 2300 mm² (13800 mm3) at
  // the same stepover, in the four diagonal corners the ring lattice converges on and stops short of, so
  // the rows are the smaller — and the only — defect the pocket offers here. What the inlay needs is that
  // the plug be *separated* from the stock, which the rows do everywhere except those corner cells, and
  // that is the honest statement of this pair's finish.
  let male = deviations(&male, &male_target, tolerance).expect("one grid");
  assert_eq!(male.gouge, 0.0, "the male never crossed into the plug");
  assert_eq!(male.gouge_cells, 0);
  assert!(
    male.uncut <= bound,
    "the male's residue is inside the bound"
  );
  assert_eq!(
    male.uncut, 480.0,
    "80 cells: the corner four and the two rinds"
  );
  assert_eq!(male.uncut_cells, 80);
}

#[test]
fn an_inlay_one_millimetre_shallow_is_uncut_and_one_deep_is_gouge() {
  // A9's other sign, on both halves: a program that cuts one millimetre shallower than the document says
  // is reported as uncut, and one a millimetre deeper as gouge, with the other number exactly zero. Both
  // halves are cut at both wrong depths, so neither half can pass by reporting the other's law.
  //
  // The arithmetic is the claimed cells times the millimetre: every cell the document authorises loses one
  // millimetre less (or more) than the number it names, and each half's own residue from its clean cut is
  // unchanged in the deep one — the corner cells a disc cannot reach stay uncut either way.
  let tolerance = Length::from_mm(CELL / 8.0);
  let halves: [(&str, bool); 2] = [("female", true), ("male", false)];
  for (name, female) in halves {
    let target = target_for(
      &inlay_document(female, DEPTH),
      Length::from_mm(CELL),
      &tools(TOOL),
    )
    .expect("a target");
    let claimed = authorised_volume(&target) / DEPTH;
    let moves = |depth: f64| {
      if female {
        inlay_female_moves(INLAY_ALLOWANCE, INLAY_STEPOVER, depth)
      } else {
        inlay_male_moves(INLAY_STEPOVER, depth)
      }
    };

    let clean = cut(STOCK, CELL, TOOL, moves(DEPTH));
    let clean = deviations(&clean, &target, tolerance).expect("one grid");

    let shallow = cut(STOCK, CELL, TOOL, moves(DEPTH - 1.0));
    let shallow = deviations(&shallow, &target, tolerance).expect("one grid");
    assert_eq!(shallow.gouge, 0.0, "{name}: 1 mm shallow gouges nothing");
    assert!(
      shallow.uncut >= claimed,
      "{name}: every one of the {claimed} claimed cells is at least 1 mm short: {shallow:?}"
    );

    let deep = cut(STOCK, CELL, TOOL, moves(DEPTH + 1.0));
    let deep = deviations(&deep, &target, tolerance).expect("one grid");
    assert_eq!(
      deep.uncut, clean.uncut,
      "{name}: 1 mm deep leaves no more standing than the clean cut's own residue"
    );
    // Every claimed cell but the clean cut's own residue is a millimetre too deep: the residue cells are
    // removed by nothing at all, so they are the only ones that cannot be too deep.
    assert!(
      deep.gouge >= claimed - clean.uncut_cells as f64,
      "{name}: every claimed cell but the residue is at least 1 mm too deep: {deep:?}"
    );
  }
}

// --- thread milling (M6 A7, A9, D-M6-8) -------------------------------------------------------

/// The moves one thread emits, stated here move by move: the orbit's `+x` point at safe Z, a feed down to
/// the surface in place, one **helical** move per whole revolution of `pitch`, one carrying the fraction
/// of a turn that reaches the depth, and a retract (M6 A7).
///
/// The fixture is the strategy's own geometry, and it is the reason this file needs a helical move at
/// all: a thread is one continuous descent around a circle, so it cannot be built out of constant-Z arcs.
fn thread_moves(centre: Point2, orbit: f64, pitch: f64, depth: f64) -> Vec<Move> {
  let start = Point2::new(centre.x + orbit, centre.y);
  let mut moves = vec![
    Move::Rapid {
      to: Point3::new(start.x, start.y, SAFE_Z),
    },
    Move::Feed {
      to: Point3::new(start.x, start.y, 0.0),
    },
  ];
  let turns = depth / pitch;
  let whole = turns.floor();
  for turn in 0..whole as usize {
    moves.push(Move::Helix {
      to: Point3::new(start.x, start.y, -pitch * (turn + 1) as f64),
      center: centre,
      ccw: true,
    });
  }
  let remainder = depth - pitch * whole;
  if remainder > 1.0e-9 {
    let angle = (remainder / pitch) * TAU;
    moves.push(Move::Helix {
      to: Point3::new(
        centre.x + orbit * angle.cos(),
        centre.y + orbit * angle.sin(),
        -depth,
      ),
      center: centre,
      ccw: true,
    });
  }
  let last = moves.last().expect("a move").destination();
  moves.push(Move::Retract {
    to: Point3::new(last.x.mm(), last.y.mm(), SAFE_Z),
  });
  moves
}

/// The thread fixture's hole: 12 mm across, centred clear of the sheet's edges.
const THREAD_HOLE: f64 = 12.0;
/// The tool the thread oracles mill with.
const THREAD_TOOL: f64 = 4.0;
/// The depth every thread fixture is cut to.
const THREAD_DEPTH: f64 = 6.0;
/// The pitch every thread fixture descends by.
const THREAD_PITCH: f64 = 1.5;

fn thread_fixture(pitch: f64, direction: cam_model::ThreadDirection, depth: f64) -> Document {
  document(
    STOCK,
    &[("hole", circle(Point2::new(50.0, 50.0), THREAD_HOLE / 2.0))],
    vec![Toolpath {
      id: "thread 1".to_owned(),
      name: "Thread".to_owned(),
      entity_ids: vec!["hole".to_owned()],
      island_ids: Vec::new(),
      tool_number: 1,
      operation: Operation::Thread(cam_model::ThreadParams {
        pitch: Length::from_mm(pitch),
        direction,
        start_depth: Length::ZERO,
        cut_depth: Length::from_mm(depth),
      }),
    }],
  )
}

/// The orbit a thread runs on: `(D − t)/2` inside the circle, `(D + t)/2` outside it.
fn thread_orbit(direction: cam_model::ThreadDirection) -> f64 {
  match direction {
    cam_model::ThreadDirection::Internal => (THREAD_HOLE - THREAD_TOOL) / 2.0,
    cam_model::ThreadDirection::External => (THREAD_HOLE + THREAD_TOOL) / 2.0,
  }
}

#[test]
fn an_internal_thread_removes_the_annulus_between_d_minus_t_and_d() {
  // A9's arithmetic for thread milling, from M6 A7's geometry: the tool orbits at `(D − t)/2` with a
  // radius of `t/2`, so its disc sweeps the annulus between `D/2 − t = 2` and `D/2 = 6` mm, and the
  // analytic volume is that annulus times the depth:
  //
  //   π · ((D/2)² − (D/2 − t)²) · d = π · t · (D − t) · d = π · 4 · 8 · 6 = 603.186 mm3
  //
  // The published bound is the annulus's perimeter times half a cell times the depth, both circles
  // included: `(2π·6 + 2π·2) · 1/2 · 6 = 150.796` mm3.
  //
  // The measurement is 541.000 mm3 rather than 603.186, and the 62.186 mm3 between them is not slack: it
  // is the **crest** a helical descent leaves. The floor of the swept annulus is a spiral — one pitch per
  // revolution — so the material a thread leaves standing between two revolutions is real, and the
  // analytic above is the *full annulus at `cut_depth`*, which no helix can remove. Three numbers state
  // the whole of it, and none of them is a tolerance:
  //
  // - `603.186` — the analytic: the annulus at one depth, i.e. what a flat-bottomed cut would remove.
  // - `542.288` — what the claim authorises, measured by summing it over the grid: the spiral, which
  //   averages `cut_depth − pitch/2 = 5.25` mm deep (`527.788` mm3), deepened by the angular window of
  //   the tool's own footprint — a cell is reached from angles up to `α` past its own, and the later pass
  //   is the deeper one — and quantised onto 1 mm cells.
  // - `541.000` — what the program removes: the same spiral, sampled. The `1.288` mm3 between this and
  //   the claim is the sampling (`cell²/(32·r)` per cell, the published coverage bound), and it is why
  //   the claim is taken as the maximum over the footprint rather than at the tip: the tip's law alone
  //   would authorise *less* than the program removes and report a correct thread as a gouge.
  let direction = cam_model::ThreadDirection::Internal;
  let moves = thread_moves(
    Point2::new(50.0, 50.0),
    thread_orbit(direction),
    THREAD_PITCH,
    THREAD_DEPTH,
  );
  assert_eq!(
    moves.len(),
    2 + 4 + 1,
    "two entries, four whole revolutions at a 1.5 mm pitch over 6 mm, and a retract"
  );
  let field = cut(STOCK, CELL, THREAD_TOOL, moves);

  let analytic = PI * THREAD_TOOL * (THREAD_HOLE - THREAD_TOOL) * THREAD_DEPTH;
  let bound = (2.0 * PI * THREAD_HOLE / 2.0 + 2.0 * PI * (THREAD_HOLE / 2.0 - THREAD_TOOL)) * CELL
    / 2.0
    * THREAD_DEPTH;
  let removed = field.volumes().removed;
  assert!(
    (removed - analytic).abs() <= bound,
    "removed {removed} mm3 against the analytic {analytic} mm3, bound {bound} mm3"
  );

  // And the target authorises exactly the spiral the helix leaves, per cell, so a correct **coarse**
  // thread is clean on both signs — the acceptance this claim exists for. The claim is the maximum the
  // law allows anywhere the tool could have removed the cell, so it can only be tighter than the sweep by
  // one sampling interval of angle, well inside the judgement's eighth of a cell.
  let document = thread_fixture(THREAD_PITCH, direction, THREAD_DEPTH);
  let target = target_for(&document, Length::from_mm(CELL), &tools(THREAD_TOOL))
    .expect("the spiral's claim derives");
  let deviation = deviations(&field, &target, Length::from_mm(CELL / 8.0)).expect("one grid");
  assert_eq!(
    deviation,
    Deviation::default(),
    "a 1.5 mm pitch on a 1 mm cell is the spacing a real thread uses, and it is clean: {deviation:?}"
  );
}

#[test]
fn an_external_thread_removes_the_annulus_between_d_and_d_plus_t() {
  // The same arithmetic outside the circle: the orbit is `(D + t)/2 = 8`, the annulus runs from
  // `D/2 = 6` to `D/2 + t = 10`, and the analytic volume is `π · t · (D + t) · d = π · 4 · 16 · 6 =
  // 1206.372` mm3. The bound is `(2π·10 + 2π·6) · 1/2 · 6 = 301.593` mm3.
  let direction = cam_model::ThreadDirection::External;
  let moves = thread_moves(
    Point2::new(50.0, 50.0),
    thread_orbit(direction),
    THREAD_PITCH,
    THREAD_DEPTH,
  );
  let field = cut(STOCK, CELL, THREAD_TOOL, moves);

  let analytic = PI * THREAD_TOOL * (THREAD_HOLE + THREAD_TOOL) * THREAD_DEPTH;
  let bound = (2.0 * PI * (THREAD_HOLE / 2.0 + THREAD_TOOL) + 2.0 * PI * THREAD_HOLE / 2.0) * CELL
    / 2.0
    * THREAD_DEPTH;
  let removed = field.volumes().removed;
  assert!(
    (removed - analytic).abs() <= bound,
    "removed {removed} mm3 against the analytic {analytic} mm3, bound {bound} mm3"
  );

  let document = thread_fixture(THREAD_PITCH, direction, THREAD_DEPTH);
  let target = target_for(&document, Length::from_mm(CELL), &tools(THREAD_TOOL))
    .expect("the spiral's claim derives");
  let deviation = deviations(&field, &target, Length::from_mm(CELL / 8.0)).expect("one grid");
  assert_eq!(
    deviation,
    Deviation::default(),
    "a clean external thread is clean on both signs, as an internal one is: {deviation:?}"
  );
}

#[test]
fn a_thread_cut_a_millimetre_shallow_is_uncut_and_a_millimetre_deep_is_a_gouge() {
  // A9's second sign, both ways, at the fixture's own coarse pitch: a descent that stops a millimetre
  // short leaves the spiral shallow everywhere — uncut, never gouge — and one that runs a millimetre past
  // cuts deeper than the spiral everywhere, because the exact claim tracks the crest instead of levelling
  // it at the depth. (The pitch no longer has to be squeezed under the tolerance for the deep case to read
  // as a gouge at every cell; the claim's own one-pitch generosity is gone.)
  let pitch = THREAD_PITCH;
  let direction = cam_model::ThreadDirection::Internal;
  let centre = Point2::new(50.0, 50.0);
  let orbit = thread_orbit(direction);
  let target = target_for(
    &thread_fixture(pitch, direction, THREAD_DEPTH),
    Length::from_mm(CELL),
    &tools(THREAD_TOOL),
  )
  .expect("the annulus claim derives");
  let tolerance = Length::from_mm(CELL / 8.0);

  let shallow = cut(
    STOCK,
    CELL,
    THREAD_TOOL,
    thread_moves(centre, orbit, pitch, THREAD_DEPTH - 1.0),
  );
  let shallow = deviations(&shallow, &target, tolerance).expect("one grid");
  assert_eq!(shallow.gouge, 0.0, "a millimetre shallow gouges nothing");
  assert_eq!(shallow.gouge_cells, 0);
  assert!(
    shallow.uncut > 0.0 && shallow.uncut_cells > 0,
    "a millimetre shallow is uncut: {shallow:?}"
  );

  let deep = cut(
    STOCK,
    CELL,
    THREAD_TOOL,
    thread_moves(centre, orbit, pitch, THREAD_DEPTH + 1.0),
  );
  let deep = deviations(&deep, &target, tolerance).expect("one grid");
  assert_eq!(deep.uncut, 0.0, "a millimetre deep leaves nothing uncut");
  assert_eq!(deep.uncut_cells, 0);
  assert!(
    deep.gouge > 0.0 && deep.gouge_cells > 0,
    "a millimetre deep is a gouge: {deep:?}"
  );
}

#[test]
fn a_coarse_thread_is_clean_and_a_fine_one_is_too() {
  // The claim is exact for every pitch, so the sub-tolerance fixture the upper-bound claim needed is
  // gone: what used to be "clean only when the pitch is inside the judgement" is now "clean at a real
  // pitch", and a pitch of a twentieth of a millimetre is clean for the same reason. Two pitches, four
  // times apart, no deviation on either sign.
  for pitch in [THREAD_PITCH, 0.05] {
    let direction = cam_model::ThreadDirection::Internal;
    let target = target_for(
      &thread_fixture(pitch, direction, THREAD_DEPTH),
      Length::from_mm(CELL),
      &tools(THREAD_TOOL),
    )
    .expect("the spiral's claim derives");
    let field = cut(
      STOCK,
      CELL,
      THREAD_TOOL,
      thread_moves(
        Point2::new(50.0, 50.0),
        thread_orbit(direction),
        pitch,
        THREAD_DEPTH,
      ),
    );
    let deviation = deviations(&field, &target, Length::from_mm(CELL / 8.0)).expect("one grid");
    assert_eq!(
      deviation,
      Deviation::default(),
      "a {pitch} mm pitch leaves nothing to report"
    );
  }
}
