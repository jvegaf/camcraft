//! The moulding oracle: a cross-section swept along a rail, and the two signs of its deviation
//! (M8 T5; S3, S6).
//!
//! The claim this file tests is stated once and used three times, so it is written here rather than
//! repeated:
//!
//! A moulding's rail is an XY path, its profile is authored data — a cross-section of
//! `(lateral, depth)` points — and its stepover spaces the overlapped lateral passes (S3). Each pass
//! is the rail walked at one lateral offset and cut at the depth the cross-section authors there, so a
//! cell is authorised the depth of the pass whose path it lies under, and nothing where no pass
//! reaches. The target is read from the **document** — the rail entity, the profile and the stepover —
//! and never from the emitted moves.
//!
//! # Why `gouge` is zero by construction
//!
//! Every depth here is written as arithmetic from the authored profile, never read back through
//! `CrossSection::depth_at`, so a law that interpolated differently would be visible. A correct
//! program cuts, at each of its own samples, the pass's own depth within the tool's own footprint, and
//! the claim authorises that depth over the **whole** continuous offset path, with the tool's `rise`
//! taken at a distance that is never larger. So the claim's footprint contains the program's and its
//! depth there is never shallower: material removed can never exceed material authorised, which is why
//! `gouge` is exactly zero for a correct moulding whatever the bit's shape, and why a dropped pass or
//! a too-shallow cross-section costs `uncut` and nothing else. This is a different argument from the
//! V-carve's (M7 A1: a Lipschitz distance field) — here it is coverage plus a monotone tool law — and
//! it is stated as a both-signs pair below rather than assumed.
//!
//! # The straight-rail fixture, and why its numbers are exact
//!
//! The rail runs along `y = 50.5`, from `x = 20` to `x = 80`, so a pass at a **whole** lateral
//! distance lies on the centre line of a row of cells (`y = 50.5 − lateral` is the centre of row
//! `50 − lateral`), and a 0.5 mm bit's footprint (radius 0.25) stays inside that row. The profile is
//! `−1 mm` at lateral 0, `−4 mm` at lateral 4 and `−1 mm` at lateral 8, and the stepover is 2 mm, so
//! the passes are at laterals 0, 2, 4, 6 and 8 and their depths are `1, 2.5, 4, 2.5, 1` mm — all exact
//! in binary, because each one is a midpoint of authored points that are themselves whole numbers. So
//! the arithmetic below is an equality and not a bound: 60 columns x 5 rows, `60 · 11 = 660 mm³`.

use cam_geom::moulding::{Rail, DEFAULT_CHORD_TOLERANCE};
use cam_geom::{Contour, Point2};
use cam_model::{
  Document, Entity, EntityKind, Job, Layer, MouldingParams, MouldingProfilePoint, Operation,
  OriginCorner, Toolpath,
};
use cam_move::{Move, MoveStream, Point3, Segment, ToolShape};
use cam_sim::sweep;
use cam_sim::target::{deviations, target_for, Deviation, Target, TargetError, ToolShapes};
use cam_sim::Heightfield;
use cam_units::{Length, UnitSystem};

/// The stock every fixture is cut out of: 120 mm square, 18 mm thick, so a 22 mm ring around a 20 mm
/// rail stays inside it.
const STOCK: f64 = 120.0;
const THICKNESS: f64 = 18.0;
const SAFE_Z: f64 = 5.0;
/// One millimetre cells: the resolution the tree's pocket and profile arithmetic is written for.
const CELL: f64 = 1.0;
/// The judgement `cam-cli` applies: an eighth of a cell. A correct moulding's numbers here are exact,
/// so the tolerance only has to absorb floating point, never a modelling difference.
const TOLERANCE_PER_CELL: f64 = 1.0 / 8.0;
/// The rail's line, its ends and the bit that cuts it.
const RAIL_Y: f64 = 50.5;
const RAIL_FROM: f64 = 20.0;
const RAIL_TO: f64 = 80.0;
const TOOL_DIAMETER: f64 = 0.5;

fn job() -> Job {
  Job {
    width: Length::from_mm(STOCK),
    height: Length::from_mm(STOCK),
    thickness: Length::from_mm(THICKNESS),
    units: UnitSystem::Mm,
    origin: OriginCorner::BottomLeft,
    safe_z: Length::from_mm(SAFE_Z),
    material: None,
  }
}

/// A straight open rail along +X at `y = 50.5`: lateral is to the right of travel, towards -Y.
fn rail() -> Contour {
  Contour::from_points(
    &[Point2::new(RAIL_FROM, RAIL_Y), Point2::new(RAIL_TO, RAIL_Y)],
    false,
  )
  .expect("a straight rail")
}

/// The authored cross-section every fixture sweeps: `-1 mm` at lateral 0, `-4 mm` at lateral 4 and
/// `-1 mm` at lateral 8, with a 2 mm stepover between the passes.
fn params() -> MouldingParams {
  MouldingParams {
    profile: vec![
      MouldingProfilePoint {
        lateral: Length::from_mm(0.0),
        depth: Length::from_mm(-1.0),
      },
      MouldingProfilePoint {
        lateral: Length::from_mm(4.0),
        depth: Length::from_mm(-4.0),
      },
      MouldingProfilePoint {
        lateral: Length::from_mm(8.0),
        depth: Length::from_mm(-1.0),
      },
    ],
    stepover: Length::from_mm(2.0),
  }
}

/// The lateral offsets the stepover law places the passes at, written from the authored span: start at
/// the profile's own minimum, step by the stepover, and make the maximum a pass.
const PASSES: [f64; 5] = [0.0, 2.0, 4.0, 6.0, 8.0];

/// The depth below the surface the cross-section authors at each of [`PASSES`], from the authored
/// points alone: `1`, `2.5`, `4`, `2.5`, `1` mm. Never read through `CrossSection::depth_at`.
const PASS_DEPTHS: [f64; 5] = [1.0, 2.5, 4.0, 2.5, 1.0];

fn document(entities: &[(&str, Contour)], toolpaths: Vec<Toolpath>) -> Document {
  let mut document = Document::new(job());
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

/// A moulding over the entities it names, with the fixture's cross-section.
fn moulding(id: &str, entities: &[&str]) -> Toolpath {
  Toolpath {
    id: id.to_owned(),
    name: "Moulding".to_owned(),
    entity_ids: entities.iter().map(|id| (*id).to_owned()).collect(),
    island_ids: Vec::new(),
    tool_number: 1,
    operation: Operation::Moulding(params()),
  }
}

#[test]
fn two_mouldings_over_the_same_rail_keep_the_deeper_claim_and_never_the_sum() {
  // D-M5-2 across toolpaths: two cross-sections swept along the same rail claim the same cells, and a
  // cell keeps the **deeper** of them. The second profile is the first one millimetre deeper
  // everywhere, so the volume is 60 x (2 + 3.5 + 5 + 3.5 + 2) = 960 mm3 — the deeper of 660 and 960 —
  // and never 660 + 960.
  let document = document(
    &[("rail", rail())],
    vec![
      moulding("moulding 1", &["rail"]),
      Toolpath {
        operation: Operation::Moulding(deeper_params()),
        ..moulding("moulding 2", &["rail"])
      },
    ],
  );
  let target = target_of(&document);
  assert_eq!(
    target.authorised(30, 46),
    5.0,
    "4 mm and 5 mm is 5 mm, not 9 mm"
  );
  assert_eq!(authorised_volume(&target), 960.0);
}

#[test]
fn a_cone_authorises_the_flank_it_cuts_and_not_a_flat_band() {
  // A V-bit's radius grows with depth, so its footprint is a cone and the depth it reaches falls away
  // from the pass (M6 A8). With a 90 degree bit `rise(d) = d`, so a pass at 4 mm authorises `4 − d` at
  // a cell `d` millimetres from it: a flat band at 4 mm would report a correct bevel as uncut.
  //
  // The profile is flat and 0.5 mm wide with a 10 mm stepover, so the stepover law places **one** pass,
  // centred on the profile: the line `y = 50.25`. Rows 50, 49 and 46 are 0.25, 0.75 and 3.75 mm from
  // it; row 45 is 4.25 mm away, past the cone's own 4 mm reach.
  let flat = MouldingParams {
    profile: vec![
      MouldingProfilePoint {
        lateral: Length::from_mm(0.0),
        depth: Length::from_mm(-4.0),
      },
      MouldingProfilePoint {
        lateral: Length::from_mm(0.5),
        depth: Length::from_mm(-4.0),
      },
    ],
    stepover: Length::from_mm(10.0),
  };
  let document = document(
    &[("rail", rail())],
    vec![Toolpath {
      operation: Operation::Moulding(flat),
      ..moulding("moulding 1", &["rail"])
    }],
  );
  let target = target_for(
    &document,
    cell(),
    &ToolShapes::new([(
      1,
      ToolShape::v_bit(cam_units::Angle::from_degrees(90.0), Length::ZERO),
    )]),
  )
  .expect("a target");
  // `tan(π/4)` is not exactly 1 in binary, so the cone's own `rise` carries a unit in the last place:
  // these four are the law to a nanometre, while the cylinder fixtures above are exact equalities.
  let flank = |row: usize, distance: f64| {
    let authorised = target.authorised(30, row);
    assert!(
      (authorised - (4.0 - distance)).abs() < 1.0e-9,
      "row {row} is {distance} mm from the pass, so it is authorised 4 - {distance} mm, got {authorised}"
    );
  };
  flank(50, 0.25);
  flank(49, 0.75);
  flank(46, 3.75);
  assert_eq!(
    target.authorised(30, 45),
    0.0,
    "4.25 mm away, past the 4 mm cone's own reach"
  );
}

/// The fixture's cross-section one millimetre deeper everywhere: `-2`, `-5`, `-2` mm.
fn deeper_params() -> MouldingParams {
  MouldingParams {
    profile: vec![
      MouldingProfilePoint {
        lateral: Length::from_mm(0.0),
        depth: Length::from_mm(-2.0),
      },
      MouldingProfilePoint {
        lateral: Length::from_mm(4.0),
        depth: Length::from_mm(-5.0),
      },
      MouldingProfilePoint {
        lateral: Length::from_mm(8.0),
        depth: Length::from_mm(-2.0),
      },
    ],
    stepover: Length::from_mm(2.0),
  }
}

/// The tool table for the fixture's single cylindrical bit.
fn tools() -> ToolShapes {
  ToolShapes::new([(1, ToolShape::cylinder(Length::from_mm(TOOL_DIAMETER)))])
}

fn target_of(document: &Document) -> Target {
  target_for(document, Length::from_mm(CELL), &tools()).expect("a target")
}

fn cell() -> Length {
  Length::from_mm(CELL)
}

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

/// The program a moulding emits, written from this file's own numbers and never from
/// `cam-toolpath`: per pass a rapid to the pass's first point at the job's safe Z, a plunge there to
/// `-depth`, one feed to the pass's last point at that same Z, and a retract (S3, D-M4-6).
///
/// `depths` is what the program actually cuts at each pass, which is how the two deviation signs are
/// produced: the fixture's own `PASS_DEPTHS` is the correct program, one millimetre more per pass is a
/// program that cuts too deep, and one millimetre less is one that leaves material standing.
fn program(depths: [f64; 5]) -> MoveStream {
  let frame = job().frame();
  let at = |lateral: f64, x: f64, depth: f64| {
    let point = frame.to_machine(Point2::new(x, RAIL_Y - lateral));
    Point3::new(point.x, point.y, -depth)
  };
  let mut moves = Vec::new();
  for (lateral, depth) in PASSES.iter().zip(depths) {
    let start = at(*lateral, RAIL_FROM, depth);
    moves.push(Move::Rapid {
      to: Point3::new(start.x.mm(), start.y.mm(), SAFE_Z),
    });
    moves.push(Move::Feed { to: start });
    moves.push(Move::Feed {
      to: at(*lateral, RAIL_TO, depth),
    });
    let last = moves.last().expect("a move").destination();
    moves.push(Move::Retract {
      to: Point3::new(last.x.mm(), last.y.mm(), SAFE_Z),
    });
  }
  MoveStream {
    segments: vec![Segment {
      name: "Moulding".to_owned(),
      tool_number: 1,
      tool: ToolShape::cylinder(Length::from_mm(TOOL_DIAMETER)),
      feed_rate: 1000.0,
      plunge_rate: 300.0,
      spindle_speed: 18_000,
      moves,
    }],
    safe_z: Length::from_mm(SAFE_Z),
    home: Point2::new(0.0, 0.0),
  }
}

/// What running `program` over the stock did, against what `document` authorises.
struct Measured {
  removed: f64,
  deepest: f64,
  deviation: Deviation,
}

fn measure(document: &Document, program: &MoveStream) -> Measured {
  measure_against(document, &target_of(document), program)
}

/// The same measurement against a target this file built itself — the V-bit fixtures own their tool
/// table, so they cannot use the shared cylinder's.
fn measure_against(document: &Document, target: &Target, program: &MoveStream) -> Measured {
  let mut field = Heightfield::new(&document.job, cell()).expect("the fixture fits the grid");
  let removal = sweep::apply(program, &mut field);
  assert!(
    removal.crashes.is_empty(),
    "no move rams the stock: {:?}",
    removal.crashes
  );
  let deviation =
    deviations(&field, target, Length::from_mm(CELL * TOLERANCE_PER_CELL)).expect("one grid");
  let grid = field.grid();
  let mut deepest = 0.0_f64;
  for row in 0..grid.rows {
    for column in 0..grid.columns {
      deepest = deepest.min(field.surface(column, row));
    }
  }
  Measured {
    removed: field.volumes().removed,
    deepest,
    deviation,
  }
}

fn straight_fixture() -> Document {
  document(&[("rail", rail())], vec![moulding("moulding 1", &["rail"])])
}

#[test]
fn a_straight_rail_authorises_the_cross_sections_analytic_volume() {
  // The claim read from the document alone: each pass is the rail's offset family at its own lateral
  // distance, cut at the depth the authored profile gives there. The rail sits at `y = 50.5`, so the
  // pass at lateral `L` lies on the centre line of row `50 - L` — the passes are rows 50, 48, 46, 44
  // and 42, at 1, 2.5, 4, 2.5 and 1 mm.
  let target = target_of(&straight_fixture());
  assert_eq!(
    target.authorised(30, 50),
    1.0,
    "lateral 0, centre (30.5, 50.5)"
  );
  assert_eq!(
    target.authorised(30, 48),
    2.5,
    "lateral 2, centre (30.5, 48.5)"
  );
  assert_eq!(
    target.authorised(30, 46),
    4.0,
    "lateral 4, centre (30.5, 46.5)"
  );
  assert_eq!(
    target.authorised(30, 44),
    2.5,
    "lateral 6, centre (30.5, 44.5)"
  );
  assert_eq!(
    target.authorised(30, 42),
    1.0,
    "lateral 8, centre (30.5, 42.5)"
  );

  // The ridges between two passes stand: a 0.5 mm bit reaches 0.25 mm either side of its own pass, and
  // the passes are 2 mm apart, so the rows between them are authorised nothing. That is the authored
  // stepover's own gap, not a lost claim.
  for row in [49, 47, 45, 43] {
    assert_eq!(target.authorised(30, row), 0.0, "the ridge at row {row}");
  }
  // Nothing is authorised past the rail's ends either: the pass is the rail, not the rail's line.
  assert_eq!(
    target.authorised(19, 46),
    0.0,
    "centre (19.5, 46.5) is 0.5 mm past the rail's own start"
  );
  assert_eq!(target.authorised(20, 46), 4.0, "centre (20.5, 46.5)");
  assert_eq!(target.authorised(79, 46), 4.0, "centre (79.5, 46.5)");
  assert_eq!(
    target.authorised(80, 46),
    0.0,
    "centre (80.5, 46.5) is 0.5 mm past the rail's own end"
  );

  // 60 columns of cells under each pass, and 1 + 2.5 + 4 + 2.5 + 1 = 11 mm of depth across the five:
  // 660 mm3.
  assert_eq!(claimed_cells(&target), 300, "5 passes x 60 columns");
  assert_eq!(authorised_volume(&target), 660.0);
}

#[test]
fn a_correct_moulding_removes_the_authorised_volume_and_gouges_nothing() {
  let document = straight_fixture();
  let measured = measure(&document, &program(PASS_DEPTHS));
  assert_eq!(
    measured.removed, 660.0,
    "the program cuts the cross-section exactly"
  );
  assert_eq!(
    measured.deepest, -4.0,
    "the deepest pass is the profile's own -4 mm"
  );
  // Both signs, on the correct program: the claim covers every cell the program cuts, at the depth it
  // cuts there, so there is nothing left to report in either direction.
  assert_eq!(
    measured.deviation.gouge, 0.0,
    "a correct moulding never cuts below its own cross-section"
  );
  assert_eq!(measured.deviation.gouge_cells, 0);
  assert_eq!(
    measured.deviation.uncut, 0.0,
    "and leaves no authorised cell standing: {:?}",
    measured.deviation
  );
  assert_eq!(measured.deviation.uncut_cells, 0);
}

#[test]
fn a_moulding_a_millimetre_shallow_leaves_material_standing() {
  // One sign of the same claim: a program cutting 1 mm less than the authored profile at every pass
  // removes 60 x (0 + 1.5 + 3 + 1.5 + 0) = 360 mm3 against the authorised 660, and every one of the
  // 300 claimed cells is exactly 1 mm short. `gouge` stays zero: too shallow is never too deep.
  let document = straight_fixture();
  let shallow = PASS_DEPTHS.map(|depth| (depth - 1.0).max(0.0));
  let measured = measure(&document, &program(shallow));
  assert_eq!(measured.removed, 360.0);
  assert!(
    (measured.deviation.uncut - 300.0).abs() < 1.0e-9,
    "300 cells 1 mm short is 300 mm3, got {}",
    measured.deviation.uncut
  );
  assert_eq!(
    measured.deviation.uncut_cells, 300,
    "every claimed cell, and only those"
  );
  assert_eq!(
    measured.deviation.gouge, 0.0,
    "a shallow pass leaves material standing, never a deeper cut"
  );
}

#[test]
fn a_moulding_a_millimetre_deep_gouges() {
  // The sign no correct program can produce: the same passes, 1 mm too deep at every one of them. The
  // claim says the passes are the authored cross-section, so a program below it must show up as
  // `gouge` in exactly the cells the correct one is exact in: 300 cells 1 mm too deep is 300 mm3.
  let document = straight_fixture();
  let deep = PASS_DEPTHS.map(|depth| depth + 1.0);
  let measured = measure(&document, &program(deep));
  assert_eq!(measured.removed, 960.0);
  assert!(
    (measured.deviation.gouge - 300.0).abs() < 1.0e-9,
    "300 cells 1 mm too deep is 300 mm3, got {}",
    measured.deviation.gouge
  );
  assert_eq!(measured.deviation.gouge_cells, 300);
  assert_eq!(
    measured.deviation.uncut, 0.0,
    "a deeper pass cuts more than authorised, never less"
  );
  assert_eq!(measured.deepest, -5.0, "the deepest pass is 1 mm below -4");
}

#[test]
fn a_closed_rail_authorises_both_sides_of_its_cross_section() {
  // S3's sign decision, pinned on a loop: lateral is positive to the **right** of travel, and a
  // counter-clockwise rail's right is outside the loop — so a profile spanning negative to positive
  // laterals sweeps a frame that authorises material inside the rail and outside it, and nothing in
  // the middle of the loop. A claim that lost the sign would authorise nothing on one side.
  //
  // A 20 mm rail, a cross-section of `-3` at laterals -2 and +2 and `-5` on the rail, a 2 mm stepover
  // and a 2 mm bit: the passes are the circles of radius 18, 20 and 22 mm, and the bit reaches 1 mm
  // either side of each. On the row through the centre, every expected radius below is at least 0.5 mm
  // inside or outside a pass's band, so the raster's own chord error (0.001 mm) cannot move a cell's
  // answer.
  let ring = cam_geom::primitive::circle(Point2::new(60.0, 60.0), 20.0).expect("a circle");
  let document = document(
    &[("rail", ring)],
    vec![Toolpath {
      operation: Operation::Moulding(ring_params()),
      ..moulding("moulding 1", &["rail"])
    }],
  );
  let target = target_for(&document, cell(), &two_mm_tools()).expect("a target");

  // Row 60 is the row whose centres sit at y = 60.5, so the radii below are read along it.
  assert_eq!(
    target.authorised(60, 60),
    0.0,
    "the middle of the loop: r = 0.71"
  );
  assert_eq!(
    target.authorised(76, 60),
    0.0,
    "r = 16.5, inside the innermost pass's band"
  );
  assert_eq!(
    target.authorised(77, 60),
    3.0,
    "r = 17.5: the pass at lateral -2, on the loop's inside"
  );
  assert_eq!(
    target.authorised(79, 60),
    5.0,
    "r = 19.5: the pass on the rail itself"
  );
  assert_eq!(
    target.authorised(81, 60),
    3.0,
    "r = 21.5: the pass at lateral +2, on the loop's outside"
  );
  assert_eq!(
    target.authorised(83, 60),
    0.0,
    "r = 23.5, outside the outermost pass's band"
  );
}

#[test]
fn a_moulding_whose_rail_is_not_a_contour_in_the_document_is_refused_by_name() {
  // The claim needs the rail's geometry, and a document that names an entity it does not hold (or one
  // that is not a contour) has none. Refused by name, with the id, rather than authorising nothing and
  // reporting the whole cut as gouge.
  let document = document(&[("rail", rail())], vec![moulding("moulding 1", &["gone"])]);
  let error = target_for(&document, cell(), &tools()).expect_err("a missing rail must refuse");
  assert_eq!(
    error,
    TargetError::Geometry {
      toolpath: "moulding 1".to_owned(),
      reason: "entity \"gone\" is not a contour in this document".to_owned(),
    }
  );
}

#[test]
fn a_moulding_that_names_no_rail_is_refused_instead_of_authorising_nothing() {
  // The other half of the same refusal: an empty selection is not a sweep. Authorising nothing would
  // turn every cell the program cuts into a gouge, which is a report about the claim and not about the
  // cut.
  let document = document(&[("rail", rail())], vec![moulding("moulding 1", &[])]);
  let error = target_for(&document, cell(), &tools()).expect_err("an empty selection must refuse");
  assert_eq!(
    error,
    TargetError::Geometry {
      toolpath: "moulding 1".to_owned(),
      reason: "it names no rail, and a moulding sweeps a cross-section along one".to_owned(),
    }
  );
}

#[test]
fn a_rail_off_the_stock_authorises_nothing() {
  // The claim's window is the rail's own bounds, clamped to the grid. A rail entirely off the stock
  // therefore authorises nothing at all — an empty claim, not an error and not a wrapped one: a
  // programme cutting there removes nothing, and a clamp that wrapped to the other edge would report
  // a gouge nobody cut.
  let off_stock =
    Contour::from_points(&[Point2::new(20.0, -50.0), Point2::new(80.0, -50.0)], false)
      .expect("a rail below the stock");
  let document = document(
    &[("rail", off_stock)],
    vec![moulding("moulding 1", &["rail"])],
  );
  let target = target_of(&document);
  assert_eq!(claimed_cells(&target), 0);
  assert_eq!(authorised_volume(&target), 0.0);
}

/// The closed rail's cross-section: `-3 mm` at lateral -2 (inside a counter-clockwise loop), `-5 mm`
/// on the rail, `-3 mm` at lateral +2 (outside it), stepped over by 2 mm so the three passes are the
/// rail's own circle and the circles 2 mm either side of it.
fn ring_params() -> MouldingParams {
  MouldingParams {
    profile: vec![
      MouldingProfilePoint {
        lateral: Length::from_mm(-2.0),
        depth: Length::from_mm(-3.0),
      },
      MouldingProfilePoint {
        lateral: Length::from_mm(0.0),
        depth: Length::from_mm(-5.0),
      },
      MouldingProfilePoint {
        lateral: Length::from_mm(2.0),
        depth: Length::from_mm(-3.0),
      },
    ],
    stepover: Length::from_mm(2.0),
  }
}

/// The tool table for the closed-rail fixture's single 2 mm bit: a moulding's profile is authored, so
/// the diameter moves no depth — it only widens the band each pass's own depth is claimed over.
fn two_mm_tools() -> ToolShapes {
  ToolShapes::new([(1, ToolShape::cylinder(Length::from_mm(2.0)))])
}

// --- M8 T7: the frame rail's oracles, and the V-bit's program ----------------------------------
//
// T5's straight-rail oracles above assert the two signs of a *rail* that is one straight edge. This
// block adds the two fixtures T7 owes: a **closed** rail, whose walk turns corners and joins itself,
// and a **V-bit**, whose footprint is a cone rather than a disc. Both are real swept fields: a
// `Heightfield` swept by a program written here, measured against the claim `cam-sim` derives from the
// document alone.

/// The frame's rail, and the four numbers that place it: a 40 x 20 mm rectangle whose lines sit on
/// half-millimetre coordinates, so a pass at a whole lateral distance runs along a row of cell centres
/// and a 0.5 mm bit's footprint (radius 0.25) stays inside that row.
const FRAME_LEFT: f64 = 30.5;
const FRAME_RIGHT: f64 = 70.5;
const FRAME_BOTTOM: f64 = 40.5;
const FRAME_TOP: f64 = 60.5;
/// The middle of the bottom edge, which is where this frame's vertex list **starts**.
const FRAME_SEAM: f64 = 50.5;

/// A closed frame: a 40 x 20 mm rectangle travelled counter-clockwise.
///
/// The vertex list starts in the **middle** of the bottom edge, and that is deliberate. A closed rail's
/// walk carries one sample per segment end, so the seam is the one place where the walk's first and
/// last samples are the same *point* with two different normals: in the middle of an edge the two
/// tangents are collinear and the offset walk joins itself, at a corner they are not and the walk
/// carries the corner's diagonal as one more edge — the pass's real polyline, which
/// [`Rail::offset_walk`] closes and [`claim_moulding`] authorises against. The corner case is measured,
/// not assumed, in [`a_closed_rail_whose_start_vertex_is_a_corner_claims_its_seam_diagonal`].
fn frame_rail() -> Contour {
  Contour::from_points(
    &[
      Point2::new(FRAME_SEAM, FRAME_BOTTOM),
      Point2::new(FRAME_RIGHT, FRAME_BOTTOM),
      Point2::new(FRAME_RIGHT, FRAME_TOP),
      Point2::new(FRAME_LEFT, FRAME_TOP),
      Point2::new(FRAME_LEFT, FRAME_BOTTOM),
    ],
    true,
  )
  .expect("a closed frame")
}

/// The same rectangle whose vertex list starts at the bottom-left **corner**: the seam the closing-edge
/// test measures. Everything else is identical, so the two fixtures differ in one number of the drawing.
fn corner_seam_frame_rail() -> Contour {
  Contour::from_points(
    &[
      Point2::new(FRAME_LEFT, FRAME_BOTTOM),
      Point2::new(FRAME_RIGHT, FRAME_BOTTOM),
      Point2::new(FRAME_RIGHT, FRAME_TOP),
      Point2::new(FRAME_LEFT, FRAME_TOP),
    ],
    true,
  )
  .expect("a closed frame")
}

/// The frame's cross-section: flat and 2 mm wide at 2 mm below the surface, with a 2 mm stepover. The
/// stepover equals the profile's width, so the law admits **two** passes — laterals 0 and 2 — and both
/// of them are at the same depth, which is what makes the frame's volume arithmetic one multiplication.
fn frame_params() -> MouldingParams {
  MouldingParams {
    profile: vec![
      MouldingProfilePoint {
        lateral: Length::from_mm(0.0),
        depth: Length::from_mm(-2.0),
      },
      MouldingProfilePoint {
        lateral: Length::from_mm(2.0),
        depth: Length::from_mm(-2.0),
      },
    ],
    stepover: Length::from_mm(2.0),
  }
}

/// A moulding of the frame with the fixture's cross-section.
fn frame_document(rail: Contour) -> Document {
  document(
    &[("rail", rail)],
    vec![Toolpath {
      operation: Operation::Moulding(frame_params()),
      ..moulding("moulding 1", &["rail"])
    }],
  )
}

/// The program a frame moulding emits, written here from the fixture's own laterals and depths.
///
/// It walks the rail with the geometry kernel's own sample walk ([`Rail::samples`], the walk the claim
/// and the strategy both use, so a program that disagreed with it would not be the program under
/// test), and writes each pass the way the strategy's contract states: a rapid to the pass's first
/// point at safe Z, a plunge there, one feed per further sample, the closing feed where the walk does
/// not end on its own first point, and a retract. The **laterals and the depths are this file's**, not
/// `cam-toolpath`'s: `laterals` is the authored span stepped over by the authored stepover, and `depths`
/// is what the program actually cuts at each pass — which is how the two deviation signs are produced.
fn frame_program(rail: &Contour, laterals: &[f64], depths: &[f64]) -> MoveStream {
  let frame = job().frame();
  let samples = Rail::new(rail)
    .expect("a rail")
    .samples(DEFAULT_CHORD_TOLERANCE)
    .expect("a walk");
  let at = |point: Point2, depth: f64| {
    let point = frame.to_machine(point);
    Point3::new(point.x, point.y, -depth)
  };
  let mut moves = Vec::new();
  for (lateral, depth) in laterals.iter().zip(depths) {
    let start = at(samples[0].offset(*lateral), *depth);
    moves.push(Move::Rapid {
      to: Point3::new(start.x.mm(), start.y.mm(), SAFE_Z),
    });
    moves.push(Move::Feed { to: start });
    for sample in &samples[1..] {
      moves.push(Move::Feed {
        to: at(sample.offset(*lateral), *depth),
      });
    }
    // A closed rail's walk ends where it closes; a walk that ends on a *different* offset point (a
    // corner seam) needs the closing feed, which is the edge the closing-edge test below measures.
    let last = moves.last().expect("a move").destination();
    if last.xy().distance_to(start.xy()) > 1.0e-9 {
      moves.push(Move::Feed { to: start });
    }
    moves.push(Move::Retract {
      to: Point3::new(last.x.mm(), last.y.mm(), SAFE_Z),
    });
  }
  MoveStream {
    segments: vec![Segment {
      name: "Moulding".to_owned(),
      tool_number: 1,
      tool: ToolShape::cylinder(Length::from_mm(TOOL_DIAMETER)),
      feed_rate: 1000.0,
      plunge_rate: 300.0,
      spindle_speed: 18_000,
      moves,
    }],
    safe_z: Length::from_mm(SAFE_Z),
    home: Point2::new(0.0, 0.0),
  }
}

#[test]
fn a_frame_rail_claims_its_cross_section_on_both_sides_of_its_seam() {
  // The claim read from the document alone, on a rail that closes: the two passes are the frame's own
  // rectangle (lateral 0) and the rectangle 2 mm outside it (lateral 2), each claimed over the 0.5 mm
  // footprint of the bit at its own depth of 2 mm.
  //
  // The rail's lines are on half-millimetre coordinates, so a pass at a whole lateral runs along a row
  // of cell centres: the bottom edge at `y = 40.5` is row 40, and the pass 2 mm outside it at
  // `y = 38.5` is row 38. The ridge between them (row 39) is 1 mm from either pass, past the 0.25 mm
  // footprint, and stands — the authored stepover's own gap, not a lost claim.
  let target = target_of(&frame_document(frame_rail()));
  assert_eq!(
    target.authorised(50, 40),
    2.0,
    "the pass on the frame's own bottom edge"
  );
  assert_eq!(target.authorised(50, 38), 2.0, "the pass 2 mm outside it");
  assert_eq!(target.authorised(50, 39), 0.0, "the ridge between the two");
  assert_eq!(
    target.authorised(50, 60),
    2.0,
    "and the same on the top edge"
  );
  assert_eq!(target.authorised(50, 62), 2.0, "and outside it");
  // Nothing in the middle of the frame: a moulding cuts where its passes are, and the passes are the
  // rail's own offset family, never the region the rail encloses.
  assert_eq!(target.authorised(50, 50), 0.0, "the middle of the frame");
  // Each corner turns through a 45 degree diagonal of the offset walk, and that diagonal's own cell is
  // claimed: the walk joins (30.5, 38.5) to (28.5, 40.5) at the bottom-left corner, and the cell whose
  // centre is on that line is `(29, 39)`.
  assert_eq!(
    target.authorised(29, 39),
    2.0,
    "the offset walk's corner diagonal"
  );
  assert_eq!(
    target.authorised(29, 41),
    0.0,
    "one cell inside the corner diagonal, past the footprint"
  );

  // The pass on the rail claims the 120 cell centres of a 40 x 20 mm rectangle's perimeter; the pass
  // one lateral out claims 128 (the four corner diagonals add one cell each). 248 cells at 2 mm is
  // 496 mm3.
  assert_eq!(claimed_cells(&target), 248, "120 + 128 cell centres");
  assert_eq!(authorised_volume(&target), 496.0);
}

#[test]
fn a_correct_frame_rail_gouges_nothing_and_leaves_nothing_standing() {
  // The fixture's two passes at the fixture's own 2 mm depth. The claim covers every cell the program
  // cuts, at the depth it cuts there, so both signs are exactly zero — on a rail that turns four
  // corners and closes on itself, which is the part a straight rail cannot test.
  let document = frame_document(frame_rail());
  let measured = measure(
    &document,
    &frame_program(&frame_rail(), &[0.0, 2.0], &[2.0, 2.0]),
  );
  assert_eq!(
    measured.removed, 496.0,
    "the program cuts the cross-section exactly"
  );
  assert_eq!(measured.deepest, -2.0, "the profile's own -2 mm");
  assert_eq!(
    measured.deviation.gouge, 0.0,
    "a correct frame moulding never cuts below its own cross-section"
  );
  assert_eq!(measured.deviation.gouge_cells, 0);
  assert_eq!(
    measured.deviation.uncut, 0.0,
    "and leaves no claimed cell standing: {:?}",
    measured.deviation
  );
  assert_eq!(measured.deviation.uncut_cells, 0);
}

#[test]
fn a_frame_rail_a_millimetre_shallow_leaves_material_standing() {
  // One sign of the same claim on the closed rail: a program cutting 1 mm less than the authored
  // profile at both passes removes 248 mm3 against the claimed 496, and every one of the 248 claimed
  // cells is exactly 1 mm short. `gouge` stays zero: too shallow is never too deep.
  let document = frame_document(frame_rail());
  let measured = measure(
    &document,
    &frame_program(&frame_rail(), &[0.0, 2.0], &[1.0, 1.0]),
  );
  assert_eq!(measured.removed, 248.0);
  assert_eq!(
    measured.deviation.uncut, 248.0,
    "248 cells 1 mm short is 248 mm3: {:?}",
    measured.deviation
  );
  assert_eq!(
    measured.deviation.uncut_cells, 248,
    "every claimed cell, and only those"
  );
  assert_eq!(
    measured.deviation.gouge, 0.0,
    "a shallow pass leaves material standing, never a deeper cut"
  );
}

#[test]
fn a_frame_rail_a_millimetre_deep_gouges() {
  // The sign no correct program can produce, on the closed rail: the same two passes, 1 mm too deep at
  // both of them. 248 cells 1 mm too deep is 248 mm3 of gouge, in exactly the cells the correct one is
  // exact in, and nothing uncut.
  let document = frame_document(frame_rail());
  let measured = measure(
    &document,
    &frame_program(&frame_rail(), &[0.0, 2.0], &[3.0, 3.0]),
  );
  assert_eq!(measured.removed, 744.0);
  assert_eq!(
    measured.deviation.gouge, 248.0,
    "248 cells 1 mm too deep is 248 mm3: {:?}",
    measured.deviation
  );
  assert_eq!(measured.deviation.gouge_cells, 248);
  assert_eq!(
    measured.deviation.uncut, 0.0,
    "a deeper pass cuts more than claimed, never less"
  );
  assert_eq!(measured.deepest, -3.0, "the deepest pass is 1 mm below -2");
}

#[test]
fn a_closed_rail_whose_start_vertex_is_a_corner_claims_its_seam_diagonal() {
  // The same frame, drawn with its vertex list starting at a corner instead of in the middle of an
  // edge. A closed rail's walk carries one sample per segment end, so the last sample of the walk and
  // the first are the same *point* carrying **two different normals** — and the strategy bridges them
  // with one closing feed across the corner. [`Rail::offset_walk`] is that polyline, the strategy's and
  // the claim's alike, so the diagonal is authorised and the program that cuts it is exact.
  //
  // This fixture is where M8 recorded a **known limit**: while the claim measured
  // `samples.windows(2)` alone, the seam's diagonal was an edge of the program that nothing
  // authorised, and this test asserted `gouge 2.000 mm³ over 1 cell` on a program that was otherwise
  // exact — one cell, `(29, 39)`, the centre of the bottom-left seam's diagonal. Pinning the wrong
  // number was the point: a fix had to come with an assertion, and this is the assertion.
  //
  // A start vertex in the middle of an edge — the fixture every other test in this block uses — has
  // collinear tangents at the seam, so its walk closes with a zero-length edge and its own numbers are
  // unchanged; that is what makes this a fix to one case and not a renumbering of the frame.
  let rail = corner_seam_frame_rail();
  let document = frame_document(rail.clone());
  let measured = measure(&document, &frame_program(&rail, &[0.0, 2.0], &[2.0, 2.0]));
  assert_eq!(
    measured.removed, 496.0,
    "the program still cuts both passes in full"
  );
  assert_eq!(
    measured.deviation.gouge, 0.0,
    "a correct frame moulding never cuts below its own cross-section, corner seam included: {:?}",
    measured.deviation
  );
  assert_eq!(measured.deviation.gouge_cells, 0);
  assert_eq!(
    measured.deviation.uncut, 0.0,
    "and the seam diagonal is claimed, so nothing the program cuts stands unclaimed: {:?}",
    measured.deviation
  );
  assert_eq!(measured.deviation.uncut_cells, 0);
  assert_eq!(
    claimed_cells(&target_of(&document)),
    248,
    "the seam-smooth frame's own 248 cell centres, because the seam diagonal is in the claim too"
  );
}

/// The V-bit fixture's cross-section: flat and 0.5 mm wide at 4 mm below the surface, with a 10 mm
/// stepover. A stepover wider than the profile is **one** pass, centred on the profile — lateral 0.25 —
/// so the whole carve is a single walk along the line `y = 50.25`.
fn cone_params() -> MouldingParams {
  MouldingParams {
    profile: vec![
      MouldingProfilePoint {
        lateral: Length::from_mm(0.0),
        depth: Length::from_mm(-4.0),
      },
      MouldingProfilePoint {
        lateral: Length::from_mm(0.5),
        depth: Length::from_mm(-4.0),
      },
    ],
    stepover: Length::from_mm(10.0),
  }
}

/// A 90 degree V-bit with a sharp point: `rise(d) = d`, so a pass at 4 mm reaches nothing 4 mm away.
fn cone_tool() -> ToolShape {
  ToolShape::v_bit(cam_units::Angle::from_degrees(90.0), Length::ZERO)
}

fn cone_document() -> Document {
  document(
    &[("rail", rail())],
    vec![Toolpath {
      operation: Operation::Moulding(cone_params()),
      ..moulding("moulding 1", &["rail"])
    }],
  )
}

/// Run a program of one pass along the straight rail at `depth` with the 90 degree cone.
fn cone_program(depth: f64) -> MoveStream {
  let frame = job().frame();
  let at = |x: f64| {
    let point = frame.to_machine(Point2::new(x, RAIL_Y - 0.25));
    Point3::new(point.x, point.y, -depth)
  };
  let start = at(RAIL_FROM);
  MoveStream {
    segments: vec![Segment {
      name: "Moulding".to_owned(),
      tool_number: 1,
      tool: cone_tool(),
      feed_rate: 1000.0,
      plunge_rate: 300.0,
      spindle_speed: 18_000,
      moves: vec![
        Move::Rapid {
          to: Point3::new(start.x.mm(), start.y.mm(), SAFE_Z),
        },
        Move::Feed { to: start },
        Move::Feed { to: at(RAIL_TO) },
        Move::Retract {
          to: Point3::new(at(RAIL_TO).x.mm(), at(RAIL_TO).y.mm(), SAFE_Z),
        },
      ],
    }],
    safe_z: Length::from_mm(SAFE_Z),
    home: Point2::new(0.0, 0.0),
  }
}

fn cone_target(document: &Document) -> Target {
  target_for(document, cell(), &ToolShapes::new([(1, cone_tool())])).expect("a target")
}

#[test]
fn a_v_bit_moulding_removes_its_cones_own_volume_and_gouges_nothing() {
  // The cone's field and the cone's program, on the same 60 mm pass: a 90 degree bit at 4 mm reaches
  // `4 - d` at a cell `d` millimetres from the pass, so the swept solid is the triangle's prism —
  // `2 x (1/2 x 4 x 4) x 60 = 960 mm3` — plus the two half-disc caps a cone swept along a *segment*
  // leaves at its ends, which is why the measured volume is larger than the prism alone.
  //
  // The assertion that matters is the pair of zeros: a cone's footprint grows with depth
  // (`rise_at_radius`), so a claim that assumed a cylinder's flat band would report a correct bevel as
  // `uncut` and a program that reached one cell further out as `gouge`.
  let document = cone_document();
  let target = cone_target(&document);
  let measured = measure_against(&document, &target, &cone_program(4.0));
  assert!(
    (measured.removed - authorised_volume(&target)).abs() < 1.0e-6,
    "the cone's program removes exactly the cone's claim: {} vs {}",
    measured.removed,
    authorised_volume(&target)
  );
  assert!(
    measured.removed > 960.0,
    "the prism alone is 960 mm3, and the two end caps add to it: {}",
    measured.removed
  );
  // The tip is at -4, and the nearest cell centre is 0.25 mm away — a 90 degree cone's flank there is
  // 4 - 0.25 below the surface, so the field's deepest cell is that, never the tip's own number.
  assert_eq!(
    measured.deepest, -3.75,
    "4 mm less the quarter millimetre to the nearest centre"
  );
  assert_eq!(
    measured.deviation.gouge, 0.0,
    "a correct V-bit moulding never cuts below its own cone: {:?}",
    measured.deviation
  );
  assert_eq!(measured.deviation.gouge_cells, 0);
  assert_eq!(
    measured.deviation.uncut, 0.0,
    "and leaves no claimed cell standing: {:?}",
    measured.deviation
  );
}

#[test]
fn a_v_bit_moulding_a_millimetre_deep_gouges() {
  // The other sign, on the cone: the same pass 1 mm deeper. A 90 degree bit's footprint widens by
  // 1 mm for every millimetre it descends, so the gouge is larger at the edge than at the pass: the
  // measured 603.671 mm3 over 676 cells against the correct program's zero.
  let document = cone_document();
  let measured = measure_against(&document, &cone_target(&document), &cone_program(5.0));
  assert_eq!(
    measured.deepest, -4.75,
    "5 mm less the quarter millimetre to the nearest centre"
  );
  assert!(
    (measured.deviation.gouge - 603.67099).abs() < 1.0e-4,
    "676 cells below the cone is 603.671 mm3, got {} over {}",
    measured.deviation.gouge,
    measured.deviation.gouge_cells
  );
  assert_eq!(measured.deviation.gouge_cells, 676);
  assert_eq!(
    measured.deviation.uncut, 0.0,
    "a deeper cone cuts more than claimed, never less"
  );
}
