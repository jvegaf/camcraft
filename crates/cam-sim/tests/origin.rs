//! The field and the program must agree about *where* the cut is, at every origin corner (M7 A7).
//!
//! A document's geometry runs from the job's own corner, `(0, 0)` to `(width, height)`, while a
//! program's moves are in machine coordinates: `top_left` reflects Y and `center` translates the whole
//! job, and both maps are exact isometries. M6's recorded defect measured what happens when nobody
//! relates the two — a *correct* program reported as gouge and uncut at every corner but `bottom_left`,
//! for every strategy since M5, because every fixture happened to sit on the one corner where the two
//! frames coincide.
//!
//! The fixture below is deliberately shaped to catch that: the region is a band across the full width,
//! 60 mm up the sheet, so a Y reflection moves it somewhere the target is *not*. The program is built
//! the way the emitter builds one — in machine coordinates, through `Frame::to_machine` — and the
//! report must be the bottom-left report at all three corners.

use cam_geom::{Contour, Point2};
use cam_model::{
  Document, Entity, EntityKind, Frame, Job, Layer, Operation, OriginCorner, PocketParams,
  PocketStrategy, Toolpath,
};
use cam_move::{Move, MoveStream, Point3, Segment, ToolShape};
use cam_sim::target::{deviations, target_for, Deviation, ToolShapes};
use cam_sim::{sweep, Heightfield, Volumes};
use cam_units::Length;

/// The sheet: narrower than it is tall, so a Y reflection is visibly a different place.
const WIDTH: f64 = 80.0;
const HEIGHT: f64 = 120.0;
const THICKNESS: f64 = 18.0;
const SAFE_Z: f64 = 5.0;
/// A 6 mm end mill, so a raster row 6 mm from its neighbour covers the band between them exactly.
const TOOL: f64 = 6.0;
const DEPTH: f64 = 6.0;
const CELL: f64 = 1.0;
/// The band the document pockets: full width, from 60 mm to 90 mm up the sheet.
const BAND_LOW: f64 = 60.0;
const BAND_HIGH: f64 = 90.0;
/// The corners a machine can put its origin on.
const CORNERS: [OriginCorner; 3] = [
  OriginCorner::BottomLeft,
  OriginCorner::TopLeft,
  OriginCorner::Center,
];

fn job(origin: OriginCorner) -> Job {
  Job {
    width: Length::from_mm(WIDTH),
    height: Length::from_mm(HEIGHT),
    thickness: Length::from_mm(THICKNESS),
    units: cam_units::UnitSystem::Mm,
    origin,
    safe_z: Length::from_mm(SAFE_Z),
  }
}

fn band() -> Contour {
  Contour::from_points(
    &[
      Point2::new(0.0, BAND_LOW),
      Point2::new(WIDTH, BAND_LOW),
      Point2::new(WIDTH, BAND_HIGH),
      Point2::new(0.0, BAND_HIGH),
    ],
    true,
  )
  .expect("a rectangle")
}

fn document(origin: OriginCorner) -> Document {
  let mut document = Document::new(job(origin));
  document.layers.push(Layer {
    id: "L1".to_owned(),
    name: "Part".to_owned(),
    visible: true,
    entities: vec![Entity {
      id: "E1".to_owned(),
      kind: EntityKind::Contour { contour: band() },
    }],
  });
  document.toolpaths = vec![Toolpath {
    id: "TP1".to_owned(),
    name: "Pocket".to_owned(),
    entity_ids: vec!["E1".to_owned()],
    island_ids: Vec::new(),
    tool_number: 1,
    operation: Operation::Pocket(PocketParams {
      strategy: PocketStrategy::Raster,
      stepover: Length::from_mm(TOOL),
      cut_depth: Length::from_mm(DEPTH),
      ..PocketParams::default()
    }),
  }];
  document
}

/// The program the emitter would produce: raster rows 6 mm apart across the band, each cut from
/// `x = 3` to `x = 77` so the tool's own radius reaches the band's edges and no further, in
/// **machine** coordinates.
fn program(job: &Job) -> MoveStream {
  let frame = Frame::of(job);
  let safe_z = Length::from_mm(SAFE_Z);
  let depth = -DEPTH;
  let mut moves = Vec::new();
  let mut row = BAND_LOW + TOOL / 2.0;
  while row <= BAND_HIGH - TOOL / 2.0 + 1e-9 {
    let start = frame.to_machine(Point2::new(TOOL / 2.0, row));
    let end = frame.to_machine(Point2::new(WIDTH - TOOL / 2.0, row));
    moves.push(Move::Rapid {
      to: Point3::new(start.x, start.y, safe_z.mm()),
    });
    moves.push(Move::Feed {
      to: Point3::new(start.x, start.y, depth),
    });
    moves.push(Move::Feed {
      to: Point3::new(end.x, end.y, depth),
    });
    moves.push(Move::Retract {
      to: Point3::new(end.x, end.y, safe_z.mm()),
    });
    row += TOOL;
  }
  MoveStream {
    segments: vec![Segment {
      name: "Pocket".to_owned(),
      tool_number: 1,
      tool: ToolShape::Cylinder {
        diameter: Length::from_mm(TOOL),
      },
      feed_rate: 60.0,
      plunge_rate: 20.0,
      spindle_speed: 18_000,
      moves,
    }],
    safe_z,
    home: Point2::new(0.0, 0.0),
  }
}

fn shapes() -> ToolShapes {
  ToolShapes::new([(
    1,
    ToolShape::Cylinder {
      diameter: Length::from_mm(TOOL),
    },
  )])
}

/// Sweep the program at one origin corner and compare the field against the document's target.
fn report(origin: OriginCorner) -> (Volumes, Deviation) {
  let job = job(origin);
  let mut field = Heightfield::new(&job, Length::from_mm(CELL)).expect("the fixture fits the grid");
  sweep::apply(&program(&job), &mut field);
  let target = target_for(&document(origin), Length::from_mm(CELL), &shapes())
    .expect("the fixture derives a target");
  let deviation =
    deviations(&field, &target, Length::from_mm(CELL / 8.0)).expect("one grid serves both");
  (field.volumes(), deviation)
}

#[test]
fn a_correct_program_gouges_nothing_at_any_origin_corner() {
  for corner in CORNERS {
    let (_, deviation) = report(corner);
    assert_eq!(
      (deviation.gouge, deviation.gouge_cells),
      (0.0, 0),
      "a program that never leaves the band gouged at {corner:?}"
    );
  }
}

#[test]
fn a_correct_program_reports_the_same_numbers_at_every_origin_corner() {
  let (left_volumes, left_deviation) = report(OriginCorner::BottomLeft);
  for corner in [OriginCorner::TopLeft, OriginCorner::Center] {
    let (volumes, deviation) = report(corner);
    assert!(
      (volumes.removed - left_volumes.removed).abs() < 1e-9,
      "{corner:?} removed {} mm³ against bottom_left's {} mm³",
      volumes.removed,
      left_volumes.removed
    );
    assert!(
      (deviation.uncut - left_deviation.uncut).abs() < 1e-9,
      "{corner:?} left {} mm³ uncut against bottom_left's {} mm³",
      deviation.uncut,
      left_deviation.uncut
    );
    assert_eq!(
      deviation.uncut_cells, left_deviation.uncut_cells,
      "{corner:?} left a different number of cells uncut than bottom_left"
    );
  }
}
