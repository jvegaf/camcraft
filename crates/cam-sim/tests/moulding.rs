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
  let mut field = Heightfield::new(&document.job, cell()).expect("the fixture fits the grid");
  let target = target_of(document);
  let removal = sweep::apply(program, &mut field);
  assert!(
    removal.crashes.is_empty(),
    "no move rams the stock: {:?}",
    removal.crashes
  );
  let deviation =
    deviations(&field, &target, Length::from_mm(CELL * TOLERANCE_PER_CELL)).expect("one grid");
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
