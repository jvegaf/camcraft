//! The sweep: what a program removes, and the rapid that rams the stock (M5 A2, A4).

use cam_geom::Point2;
use cam_model::{Job, OriginCorner};
use cam_move::{Move, MoveStream, Point3, Segment};
use cam_sim::{sweep, Heightfield};
use cam_units::{Length, UnitSystem};

fn job(extent: f64, thickness: f64) -> Job {
  Job {
    width: Length::from_mm(extent),
    height: Length::from_mm(extent),
    thickness: Length::from_mm(thickness),
    units: UnitSystem::Mm,
    origin: OriginCorner::BottomLeft,
    safe_z: Length::from_mm(5.0),
  }
}

fn field(extent: f64, cell: f64) -> Heightfield {
  Heightfield::new(&job(extent, 10.0), Length::from_mm(cell)).expect("a small field fits")
}

fn segment(name: &str, diameter: f64, moves: Vec<Move>) -> Segment {
  Segment {
    name: name.to_owned(),
    tool_number: 1,
    tool_diameter: Length::from_mm(diameter),
    feed_rate: 1000.0,
    plunge_rate: 300.0,
    spindle_speed: 12000,
    moves,
  }
}

fn program(segments: Vec<Segment>) -> MoveStream {
  MoveStream {
    segments,
    safe_z: Length::from_mm(5.0),
    home: Point2::new(0.0, 0.0),
  }
}

fn cut_cells(field: &Heightfield) -> usize {
  let grid = field.grid();
  let mut count = 0;
  for row in 0..grid.rows {
    for column in 0..grid.columns {
      if field.surface(column, row) < 0.0 {
        count += 1;
      }
    }
  }
  count
}

/// The distance from a point to a segment, computed the slow obvious way: the test's own opinion.
fn distance_to_segment(point: Point2, from: Point2, to: Point2) -> f64 {
  let dx = to.x - from.x;
  let dy = to.y - from.y;
  let length2 = dx * dx + dy * dy;
  let t = if length2 == 0.0 {
    0.0
  } else {
    (((point.x - from.x) * dx + (point.y - from.y) * dy) / length2).clamp(0.0, 1.0)
  };
  let foot = Point2::new(from.x + dx * t, from.y + dy * t);
  point.distance_to(foot)
}

/// Every cell the exact capsule covers, brute-forced: the sweep must agree with it cell for cell.
fn capsule_cells(field: &Heightfield, from: Point2, to: Point2, radius: f64) -> usize {
  let grid = field.grid();
  let mut count = 0;
  for row in 0..grid.rows {
    for column in 0..grid.columns {
      let centre = field.cell_centre(column, row);
      if distance_to_segment(centre, from, to) <= radius {
        count += 1;
      }
    }
  }
  count
}

#[test]
fn a_straight_feed_cuts_exactly_the_capsule_the_tool_sweeps() {
  let mut field = field(40.0, 1.0);
  let from = Point2::new(5.0, 5.0);
  let to = Point2::new(15.0, 5.0);
  let radius = 3.0;
  let expected = capsule_cells(&field, from, to, radius);

  sweep::apply_move(
    &mut field,
    Point3::new(from.x, from.y, 5.0),
    Move::Feed {
      to: Point3::new(to.x, to.y, -2.0),
    },
    radius,
  );

  assert_eq!(cut_cells(&field), expected, "the sweep's capsule");
  let volumes = field.volumes();
  assert!(
    (volumes.removed - expected as f64 * 2.0).abs() < 1e-9,
    "{} cells at 2 mm deep removed {}",
    expected,
    volumes.removed
  );
}

#[test]
fn a_full_circle_arc_cuts_its_annulus_and_leaves_the_centre_standing() {
  let mut field = field(40.0, 1.0);
  let centre = Point2::new(20.0, 20.0);
  let start = Point2::new(25.0, 20.0);
  sweep::apply_move(
    &mut field,
    Point3::new(start.x, start.y, 5.0),
    Move::Arc {
      to: Point3::new(start.x, start.y, -2.0),
      center: centre,
      ccw: true,
    },
    1.0,
  );

  let middle = field.cell_centre(20, 20);
  assert!(
    (middle.x - 20.5).abs() < 1e-12,
    "the centre cell's centre {middle:?}"
  );
  assert_eq!(
    field.surface(20, 20),
    0.0,
    "a circle of radius 5 cut by a 1 mm tool leaves its middle standing"
  );
  assert!(
    field.surface(25, 20) < 0.0,
    "the cell the arc passes through is cut"
  );
  assert!(
    field.surface(24, 20) < 0.0,
    "the annulus between 4 and 6 mm from the centre is cut: the cell at (24.5, 20.5) is 4.53 mm out"
  );
  assert_eq!(
    field.surface(21, 20),
    0.0,
    "the cell at (21.5, 20.5) is 1.58 mm from the centre, inside the circle the tool never enters"
  );
}

#[test]
fn a_rapid_at_clearance_cuts_nothing_and_is_not_a_crash() {
  let mut field = field(40.0, 1.0);
  let removal = sweep::apply(
    &program(vec![segment(
      "profile",
      6.0,
      vec![Move::Rapid {
        to: Point3::new(20.0, 20.0, 5.0),
      }],
    )]),
    &mut field,
  );
  assert!(removal.crashes.is_empty(), "{:?}", removal.crashes);
  assert_eq!(removal.moves, 1);
  assert_eq!(field.volumes().removed, 0.0);
}

#[test]
fn a_rapid_into_the_material_is_a_crash_and_cuts_nothing() {
  let mut field = field(40.0, 1.0);
  let removal = sweep::apply(
    &program(vec![segment(
      "pocket 1",
      6.0,
      vec![
        Move::Rapid {
          to: Point3::new(20.0, 20.0, 5.0),
        },
        Move::Rapid {
          to: Point3::new(20.5, 20.5, -3.0),
        },
      ],
    )]),
    &mut field,
  );

  assert_eq!(removal.moves, 2);
  assert_eq!(removal.crashes.len(), 1, "{:?}", removal.crashes);
  let crash = &removal.crashes[0];
  assert_eq!(crash.toolpath, "pocket 1");
  assert_eq!(crash.move_index, 1);
  assert_eq!(crash.at.z.mm(), -3.0);
  assert_eq!(
    field.volumes().removed,
    0.0,
    "a crash is reported, not simulated"
  );
}

#[test]
fn a_feed_above_the_surface_cuts_air() {
  let mut field = field(40.0, 1.0);
  let removal = sweep::apply(
    &program(vec![segment(
      "profile",
      6.0,
      vec![Move::Feed {
        to: Point3::new(20.0, 20.0, 2.0),
      }],
    )]),
    &mut field,
  );
  assert!(removal.crashes.is_empty(), "{:?}", removal.crashes);
  assert_eq!(field.volumes().removed, 0.0);
}

#[test]
fn two_runs_over_the_same_program_give_the_same_field() {
  let build = || {
    program(vec![segment(
      "pocket",
      6.0,
      vec![
        Move::Rapid {
          to: Point3::new(5.0, 5.0, 5.0),
        },
        Move::Feed {
          to: Point3::new(5.0, 5.0, -3.0),
        },
        Move::Feed {
          to: Point3::new(30.0, 5.0, -3.0),
        },
        Move::Arc {
          to: Point3::new(30.0, 25.0, -3.0),
          center: Point2::new(30.0, 15.0),
          ccw: false,
        },
      ],
    )])
  };
  let mut first = field(40.0, 1.0);
  let mut second = field(40.0, 1.0);
  let a = sweep::apply(&build(), &mut first);
  let b = sweep::apply(&build(), &mut second);

  assert_eq!(a, b);
  assert_eq!(first.volumes(), second.volumes());
  let grid = first.grid();
  for row in 0..grid.rows {
    for column in 0..grid.columns {
      assert_eq!(
        first.surface(column, row),
        second.surface(column, row),
        "cell ({column}, {row})"
      );
    }
  }
  assert!(
    first.volumes().removed > 0.0,
    "the program did cut something"
  );
}
