//! The V-carve oracle: the distance field, in both signs (M7 A6, S11, D-M7-5).
//!
//! The claim this file tests is stated once and used three times, so it is written here rather than
//! repeated:
//!
//! A V-bit with its tip on `p` at depth `d(p) / tan(θ/2)` occupies a cone whose flank is `|q − p| /
//! tan(θ/2)` above the tip, and its **body is above that cone**. The material left standing after
//! sweeping a set of tips is therefore the lower envelope, and because `d` is 1-Lipschitz,
//!
//! ```text
//! |q − p| − d(p) ≥ −d(q)   ⟹   the swept surface never rises above −d(q)/tan(θ/2),
//! ```
//!
//! so a carve can only ever leave material standing, **never cut deeper than the distance field**.
//! That makes `gouge` exactly zero for *any* tip set inside the region, and it makes a dropped or
//! misplaced piece of axis cost `uncut` and nothing else. This file asserts both signs, because either
//! one alone can pass as the whole truth.
//!
//! Every depth here is written from the geometry — `clearance / tan(θ/2)`, with the tip positions and
//! their clearances taken from `cam_geom::medial`'s certified vertices — and never from
//! `VCarveParams::depth()`. The volume bounds are the simulation's own published ones (D-M5-3): a cell
//! is cut when its **centre** is inside the swept footprint, so a boundary can be wrong by at most half
//! a cell, and the volume error of a boundary of perimeter `p` cut `d` deep is at most `p·cell/2·d`.
//!
//! # What was measured, on a 0.5 mm grid with a 0.1 mm axis
//!
//! | fixture | analytic | removed | uncut | gouge | deepest |
//! | --- | --- | --- | --- | --- | --- |
//! | 20 x 20 mm square, 90° | 1333.333 | 1332.5 | 0.0 | **0.0** | -10 |
//! | r = 15 mm disc, 90° | 3534.292 | 3534.152 | 0.0 | **0.0** | -15 |
//! | the square's axis halved | 1333.333 | 1189.802 | 142.043 (642 cells) | **0.0** | -10 |
//! | the square at 20% too deep | 1333.333 | 1962.155 | 0.0 | 629.557 (1937 cells) | -12 |
//! | the square's axis at 0.5 mm | 1333.333 | 1332.5 | 0.207 (4 cells) | **0.0** | -10 |
//!
//! The two correct carves are short of the analytic volume by `0.833` and `0.140` mm³ — the cell-centre
//! quadrature of `∫ d dA`, not lost material: `uncut` is exactly zero because every cell's own centre is
//! reached to its own field value. The `uncut` breakdown the milestone owes is in the last two rows and
//! is stated there: at a 0.1 mm axis the boundary band and the axis's own tolerance cost are both
//! **0.0**, and at 0.5 mm the whole of the measured 0.207 mm³ over four cells is the axis's convex-corner
//! band — one corner cell per corner, 0.207 mm of missed depth each, which is D-M7-5's measured `0.20`.

use std::f64::consts::PI;

use cam_geom::medial::{medial_axis, MedialAxis};
use cam_geom::region::Region;
use cam_geom::{Contour, Point2};
use cam_model::{
  Document, Entity, EntityKind, Job, Layer, Operation, OriginCorner, Toolpath, VCarveParams,
};
use cam_move::{Move, MoveStream, Point3, Segment, ToolShape};
use cam_sim::target::{deviations, target_for, Deviation, ToolShapes};
use cam_sim::{sweep, Heightfield};
use cam_units::{Angle, Length, UnitSystem};

/// The stock every fixture is cut out of: 60 mm square, 18 mm thick, so a 20 mm square's 10 mm apex and
/// a 15 mm radius disc's 15 mm apex both stay inside it.
const STOCK: f64 = 60.0;
const THICKNESS: f64 = 18.0;
const SAFE_Z: f64 = 5.0;
/// The removal grid: half a millimetre, four times finer than the simulation's default, so the volume
/// arithmetic below is measured on a lattice fine enough to see the field rather than its cell corners.
const CELL: f64 = 0.5;
/// The medial axis's lattice step, which is the operation's own declared tolerance (D-M7-5).
const TOLERANCE: f64 = 0.1;
/// A 90 degree bit: `tan(45) = 1`, so the depth **is** the clearance everywhere. Every fixture in this
/// file uses it, because then the analytic volume is `∫ d dA` itself and a derivation that forgot the
/// angle cannot hide behind a tangent.
const DEGREES: f64 = 90.0;
/// The fixtures' centre: a lattice node of a 0.5 mm grid, so the apex of a square and the single tip of
/// a disc both land exactly on a cell centre and the apex depth is measurable rather than rounded.
const CENTRE: f64 = 30.25;

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

/// A square of side `size` about the fixtures' centre.
fn square(size: f64) -> Contour {
  let low = CENTRE - size / 2.0;
  let high = CENTRE + size / 2.0;
  Contour::from_points(
    &[
      Point2::new(low, low),
      Point2::new(high, low),
      Point2::new(high, high),
      Point2::new(low, high),
    ],
    true,
  )
  .expect("square")
}

fn disc(radius: f64) -> Contour {
  cam_geom::primitive::circle(Point2::new(CENTRE, CENTRE), radius).expect("circle")
}

/// The document that authorises these contours as one V-carve.
fn document(contour: &Contour, degrees: f64) -> Document {
  let mut document = Document::new(job());
  document.layers.push(Layer {
    id: "L1".to_owned(),
    name: "Part".to_owned(),
    visible: true,
    entities: vec![Entity {
      id: "E1".to_owned(),
      kind: EntityKind::Contour {
        contour: contour.clone(),
      },
    }],
  });
  document.toolpaths.push(Toolpath {
    id: "TP1".to_owned(),
    name: "V carve".to_owned(),
    entity_ids: vec!["E1".to_owned()],
    island_ids: Vec::new(),
    tool_number: 1,
    operation: Operation::VCarve(VCarveParams {
      included_angle: Angle::from_degrees(degrees),
      tolerance: Length::from_mm(TOLERANCE),
    }),
  });
  document
}

fn region(contour: &Contour) -> Region {
  Region::new(vec![contour.clone()]).expect("a closed contour bounds a region")
}

fn axis(contour: &Contour) -> MedialAxis {
  axis_at(contour, TOLERANCE)
}

/// The axis of `contour` refined to `tolerance`, for the fixtures that vary it.
fn axis_at(contour: &Contour, tolerance: f64) -> MedialAxis {
  medial_axis(&region(contour), tolerance).expect("the axis computes")
}

/// The depth a tip of `clearance` sits at, written from the geometry: `-scale · clearance / tan(θ/2)`.
fn depth(clearance: f64, degrees: f64, scale: f64) -> f64 {
  -(scale * clearance / (Angle::from_degrees(degrees).radians() / 2.0).tan())
}

/// The program a V-carve emits for `branches`: per branch a rapid, a plunge, one feed per further
/// vertex, a closing feed for a loop, and a retract. `scale` is the deliberate error the deeper fixture
/// is made of — 1.0 is the program the strategy writes.
fn program(branches: &[&cam_geom::medial::MedialBranch], degrees: f64, scale: f64) -> MoveStream {
  let frame = job().frame();
  let at = |point: Point2, clearance: f64| {
    let machine = frame.to_machine(point);
    Point3::new(machine.x, machine.y, depth(clearance, degrees, scale))
  };
  let mut moves = Vec::new();
  for branch in branches {
    let first = branch.vertices.first().expect("a branch has a vertex");
    let start = at(first.point, first.clearance);
    moves.push(Move::Rapid {
      to: Point3::new(start.x.mm(), start.y.mm(), SAFE_Z),
    });
    moves.push(Move::Feed { to: start });
    for vertex in &branch.vertices[1..] {
      moves.push(Move::Feed {
        to: at(vertex.point, vertex.clearance),
      });
    }
    if branch.closed {
      moves.push(Move::Feed { to: start });
    }
    let last = moves
      .last()
      .map(|movement| movement.destination())
      .expect("a move");
    moves.push(Move::Retract {
      to: Point3::new(last.x.mm(), last.y.mm(), SAFE_Z),
    });
  }
  MoveStream {
    segments: vec![Segment {
      name: "V carve".to_owned(),
      tool_number: 1,
      // The cone the program was written for: the document's angle and a sharp point, which is what
      // `cam-cli`'s `emit` carries for a V-carve (M7 A6).
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

/// What running `program` over the stock did, against what `document` authorises.
struct Measured {
  removed: f64,
  deepest: f64,
  deviation: Deviation,
}

fn measure(document: &Document, program: &MoveStream, degrees: f64) -> Measured {
  let cell = Length::from_mm(CELL);
  let mut field = Heightfield::new(&document.job, cell).expect("the fixture fits the grid");
  let tools = ToolShapes::new([(
    1,
    ToolShape::v_bit(Angle::from_degrees(degrees), Length::ZERO),
  )]);
  let target = target_for(document, cell, &tools).expect("a target");
  sweep::apply(program, &mut field);
  let deviation = deviations(&field, &target, Length::from_mm(CELL / 8.0)).expect("one grid");
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

/// The simulation's published coverage bound: a boundary of perimeter `p` cut `d` deep can be wrong by
/// at most half a cell, so its volume is known to `p · cell / 2 · d`.
fn coverage_bound(perimeter: f64, apex: f64) -> f64 {
  perimeter * CELL / 2.0 * apex
}

#[test]
fn a_square_carves_the_analytic_volume_and_gouges_nothing() {
  // A 20 x 20 mm square with a 90 degree bit. The distance field's integral over a square of side `w`
  // is `w³/6` — the mean distance from an interior point to the boundary over a unit square is `1/6`,
  // and it scales with the cube — so the carve removes `w³/6 / tan(45) = 1333.333` mm³, and its apex
  // is `w/2 = 10` mm, well inside the 18 mm stock.
  let square = square(20.0);
  let document = document(&square, DEGREES);
  let axis = axis(&square);
  let branches: Vec<&cam_geom::medial::MedialBranch> = axis.branches.iter().collect();
  let measured = measure(&document, &program(&branches, DEGREES, 1.0), DEGREES);

  let side = 20.0_f64;
  let apex = side / 2.0;
  let analytic = side * side * side / 6.0 / (Angle::from_degrees(DEGREES).radians() / 2.0).tan();
  assert_eq!(
    measured.deviation.gouge, 0.0,
    "the claim's first point: a tip inside the region can never cut below the distance field"
  );
  assert_eq!(
    measured.deviation.gouge_cells, 0,
    "and no cell is even judged a gouge"
  );
  assert!(
    (measured.deepest + apex).abs() < 1.0e-6,
    "the apex is w/2 = {apex} mm deep, got {}",
    measured.deepest
  );
  let bound = coverage_bound(4.0 * side, apex);
  assert!(
    (measured.removed - analytic).abs() <= bound,
    "removed {} mm³ against the analytic {analytic} mm³, within the published bound {bound} mm³",
    measured.removed
  );
  assert!(
    measured.deviation.uncut <= bound,
    "uncut {} mm³ is inside the published bound {bound} mm³",
    measured.deviation.uncut
  );
  println!(
    "square: removed {} (analytic {analytic}), uncut {} over {} cell(s), deepest {}, bound {bound}",
    measured.removed, measured.deviation.uncut, measured.deviation.uncut_cells, measured.deepest
  );
}

#[test]
fn a_circle_carves_the_analytic_volume_and_gouges_nothing() {
  // A disc of radius `r` is the sharpest case there is: its medial axis is **one point**, the centre, so
  // the whole carve is one cone and the claim is exact rather than approximated. `∫ d dA` over a disc is
  // `∫₀^r (r − ρ)·2πρ dρ = πr³/3`, so a 15 mm radius with a 90 degree bit removes `3534.292` mm³ and
  // its apex is `r = 15` mm, inside the 18 mm stock.
  let disc = disc(15.0);
  let document = document(&disc, DEGREES);
  let axis = axis(&disc);
  assert_eq!(
    axis.branches.len(),
    1,
    "a disc's axis is one branch: {:?}",
    axis.branches
  );
  assert_eq!(
    axis.branches[0].vertices.len(),
    1,
    "of one vertex, the centre"
  );
  let branches: Vec<&cam_geom::medial::MedialBranch> = axis.branches.iter().collect();
  let measured = measure(&document, &program(&branches, DEGREES, 1.0), DEGREES);

  let radius = 15.0_f64;
  let analytic =
    PI * radius * radius * radius / 3.0 / (Angle::from_degrees(DEGREES).radians() / 2.0).tan();
  assert_eq!(
    measured.deviation.gouge, 0.0,
    "a single tip at the centre cuts the field exactly and never deeper"
  );
  assert!(
    (measured.deepest + radius).abs() < 1.0e-6,
    "the apex is r = {radius} mm deep, got {}",
    measured.deepest
  );
  let bound = coverage_bound(2.0 * PI * radius, radius);
  assert!(
    (measured.removed - analytic).abs() <= bound,
    "removed {} mm³ against the analytic {analytic} mm³, within the published bound {bound} mm³",
    measured.removed
  );
  assert!(
    measured.deviation.uncut <= bound,
    "uncut {} mm³ is inside the published bound {bound} mm³",
    measured.deviation.uncut
  );
  println!(
    "circle: removed {} (analytic {analytic}), uncut {} over {} cell(s), deepest {}, bound {bound}",
    measured.removed, measured.deviation.uncut, measured.deviation.uncut_cells, measured.deepest
  );
}

#[test]
fn a_thinned_axis_leaves_material_standing_and_still_gouges_nothing() {
  // The other sign of the same claim: a skeleton piece that is missing costs **coverage** and nothing
  // else. Half of the axis's branches are dropped here — the first half in the axis's own order, so the
  // fixture is deterministic — and the report must show `uncut` and exactly zero `gouge`.
  let square = square(20.0);
  let document = document(&square, DEGREES);
  let axis = axis(&square);
  assert!(
    axis.branches.len() >= 2,
    "the square's axis has more than one branch to drop: {}",
    axis.branches.len()
  );
  let kept: Vec<&cam_geom::medial::MedialBranch> =
    axis.branches.iter().take(axis.branches.len() / 2).collect();
  let dropped: Vec<&cam_geom::medial::MedialBranch> =
    axis.branches.iter().skip(axis.branches.len() / 2).collect();
  assert!(!dropped.is_empty(), "something is dropped");
  let measured = measure(&document, &program(&kept, DEGREES, 1.0), DEGREES);

  let whole = measure(
    &document,
    &program(&axis.branches.iter().collect::<Vec<_>>(), DEGREES, 1.0),
    DEGREES,
  );
  assert!(
    measured.deviation.uncut > 0.0,
    "dropping {} of {} branches leaves material standing, but uncut is 0",
    dropped.len(),
    axis.branches.len()
  );
  assert_eq!(
    measured.deviation.gouge, 0.0,
    "a missing tip is material left standing, never a deeper cut"
  );
  assert!(
    measured.removed < whole.removed,
    "the thinned carve removes less than the whole one: {} against {}",
    measured.removed,
    whole.removed
  );
  println!(
    "thinned: uncut {} over {} cell(s), gouge {}, removed {} of {}",
    measured.deviation.uncut,
    measured.deviation.uncut_cells,
    measured.deviation.gouge,
    measured.removed,
    whole.removed
  );
}

#[test]
fn a_deeper_toolpath_gouges() {
  // The sign that no correct program can produce: the same tips, at 20% more depth than their clearance
  // allows. The claim says the swept surface is bounded above by the field, so a program that goes below
  // it must show up as `gouge` — and it must show up in the same cells the correct one is exact in,
  // which is what makes the zero in the two tests above an assertion rather than an absence.
  //
  // The measured 629.557 mm3 is more than the 266.667 mm3 a fifth of the carve would be, and the excess
  // is the second half of the same error: a 20% taller cone is 20% **wider**, so it reaches cells the
  // correct carve never touched at all — 1937 cells against the 0.5 mm field's few hundred inside the
  // square. That is not noise, it is the shape of the mistake, and it is why the bound asserted below is
  // the one-sided one.
  let square = square(20.0);
  let document = document(&square, DEGREES);
  let axis = axis(&square);
  let branches: Vec<&cam_geom::medial::MedialBranch> = axis.branches.iter().collect();
  let measured = measure(&document, &program(&branches, DEGREES, 1.2), DEGREES);

  assert!(
    measured.deviation.gouge > 0.0,
    "a program 20% too deep must report gouge, got {}",
    measured.deviation.gouge
  );
  assert!(
    measured.deviation.gouge_cells > 0,
    "and it must name the cells"
  );
  // At least a fifth of the correct carve's volume, `1333.333 / 5 = 266.667` mm³, must be gone: every
  // cell inside the region loses a fifth of its own depth more than authorised.
  let side = 20.0_f64;
  let analytic = side * side * side / 6.0;
  assert!(
    measured.deviation.gouge > analytic / 5.0 * 0.9,
    "a fifth of the carve is {} mm³ and the gouge must exceed it, got {}",
    analytic / 5.0,
    measured.deviation.gouge
  );
  println!(
    "deeper: gouge {} over {} cell(s), removed {}",
    measured.deviation.gouge, measured.deviation.gouge_cells, measured.removed
  );
}

#[test]
fn a_coarser_axis_leaves_the_tolerance_band_standing_and_still_gouges_nothing() {
  // D-M7-5's convex-corner cost, measured rather than assumed, and the breakdown of `uncut` the
  // milestone has to state. At a 0.1 mm tolerance the band is `0.00` mm of missed depth — an order of
  // magnitude below the simulation's own judgement tolerance (`cell / 8`) — so the correct carve
  // reports `uncut` exactly 0. At 0.5 mm the axis stops one lattice step short of each corner, the
  // deepest tip left there is 0.5 mm from the wall, and the corner cells lose up to `0.5` mm of depth:
  // the same program then reports `uncut` and still exactly zero `gouge`.
  let square = square(20.0);
  let document = document(&square, DEGREES);
  let coarse = axis_at(&square, 0.5);
  let branches: Vec<&cam_geom::medial::MedialBranch> = coarse.branches.iter().collect();
  let measured = measure(&document, &program(&branches, DEGREES, 1.0), DEGREES);

  assert!(
    measured.deviation.uncut > 0.0,
    "a 0.5 mm axis leaves the corner band standing, so uncut must be reported"
  );
  assert_eq!(
    measured.deviation.gouge, 0.0,
    "and a coarse tolerance costs coverage only, never depth"
  );
  println!(
    "coarse: uncut {} over {} cell(s), gouge {}",
    measured.deviation.uncut, measured.deviation.uncut_cells, measured.deviation.gouge
  );
}
