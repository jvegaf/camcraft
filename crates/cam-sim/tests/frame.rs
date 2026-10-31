//! A program is in machine coordinates and the field is in the document frame, so every move is read
//! back through the job's frame exactly once, at the field's boundary (M7 A7).
//!
//! Positions are the obvious half of that map. The other half is the one a fixture of straight moves
//! cannot show: a reflection **reverses handedness**, so an arc or a helix read back into the document
//! frame has to have its direction flipped as well, or the sweep cuts the mirror image of the curve the
//! document asked for. A crash's position goes the other way — it is reported where the program put it,
//! because it is read against the program the user can open.

use cam_geom::Point2;
use cam_model::{Job, OriginCorner};
use cam_move::{Move, MoveStream, Point3, Segment, ToolShape};
use cam_sim::{sweep, Heightfield};
use cam_units::{Length, UnitSystem};

/// A sheet that is narrower than it is tall, so a Y reflection is visibly a different place.
const WIDTH: f64 = 80.0;
const HEIGHT: f64 = 120.0;
const THICKNESS: f64 = 10.0;
const SAFE_Z: f64 = 5.0;
const TOOL: f64 = 6.0;
const CELL: f64 = 1.0;
const DEPTH: f64 = 4.0;

/// The arc the document asks for, in **document** coordinates: half a turn about `(40, 40)` from
/// `(30, 40)` to `(50, 40)`, counter-clockwise, so it bulges through `y = 30`. It is deliberately off
/// the sheet's mid-line, which a reflection would leave in place.
const ARC_START: (f64, f64) = (30.0, 40.0);
const ARC_END: (f64, f64) = (50.0, 40.0);
const ARC_CENTRE: (f64, f64) = (40.0, 40.0);

fn job(origin: OriginCorner) -> Job {
  Job {
    width: Length::from_mm(WIDTH),
    height: Length::from_mm(HEIGHT),
    thickness: Length::from_mm(THICKNESS),
    units: UnitSystem::Mm,
    origin,
    safe_z: Length::from_mm(SAFE_Z),
  }
}

fn field(origin: OriginCorner) -> Heightfield {
  Heightfield::new(&job(origin), Length::from_mm(CELL)).expect("the fixture fits the grid")
}

fn tool() -> ToolShape {
  ToolShape::cylinder(Length::from_mm(TOOL))
}

fn program(moves: Vec<Move>) -> MoveStream {
  MoveStream {
    segments: vec![Segment {
      name: "Arc".to_owned(),
      tool_number: 1,
      tool: tool(),
      feed_rate: 60.0,
      plunge_rate: 20.0,
      spindle_speed: 18_000,
      moves,
    }],
    safe_z: Length::from_mm(SAFE_Z),
    home: Point2::new(0.0, 0.0),
  }
}

fn point3(x: f64, y: f64, z: f64) -> Point3 {
  Point3::new(x, y, z)
}

/// The document's arc, cut by a plunge at its start and then one constant-Z arc move.
fn document_arc() -> Vec<Move> {
  vec![
    Move::Feed {
      to: point3(ARC_START.0, ARC_START.1, -DEPTH),
    },
    Move::Arc {
      to: point3(ARC_END.0, ARC_END.1, -DEPTH),
      center: Point2::new(ARC_CENTRE.0, ARC_CENTRE.1),
      ccw: true,
    },
  ]
}

/// The same arc, in the document frame, cut by a plunge and then one descending helical move.
fn document_helix() -> Vec<Move> {
  vec![
    Move::Feed {
      to: point3(ARC_START.0, ARC_START.1, -1.0),
    },
    Move::Helix {
      to: point3(ARC_END.0, ARC_END.1, -DEPTH),
      center: Point2::new(ARC_CENTRE.0, ARC_CENTRE.1),
      ccw: true,
    },
  ]
}

/// The machine-coordinate program the emitter would write for the same arc, at one origin corner.
///
/// Written out by hand rather than derived from the frame, because this is the emitter's contract
/// stated independently: `top_left` puts the document's `y` at `120 - y` and therefore has to reverse
/// the arc's direction, while `center` shifts the whole arc by `(-40, -60)` and leaves its direction
/// alone.
fn machine_arc(corner: OriginCorner) -> Vec<Move> {
  match corner {
    OriginCorner::BottomLeft => document_arc(),
    OriginCorner::TopLeft => vec![
      Move::Feed {
        to: point3(30.0, 80.0, -DEPTH),
      },
      Move::Arc {
        to: point3(50.0, 80.0, -DEPTH),
        center: Point2::new(40.0, 80.0),
        ccw: false,
      },
    ],
    OriginCorner::Center => vec![
      Move::Feed {
        to: point3(-10.0, -20.0, -DEPTH),
      },
      Move::Arc {
        to: point3(10.0, -20.0, -DEPTH),
        center: Point2::new(0.0, -20.0),
        ccw: true,
      },
    ],
  }
}

/// The same arc as a helix, at one origin corner, with the same rule for its direction.
fn machine_helix(corner: OriginCorner) -> Vec<Move> {
  match corner {
    OriginCorner::BottomLeft => document_helix(),
    OriginCorner::TopLeft => vec![
      Move::Feed {
        to: point3(30.0, 80.0, -1.0),
      },
      Move::Helix {
        to: point3(50.0, 80.0, -DEPTH),
        center: Point2::new(40.0, 80.0),
        ccw: false,
      },
    ],
    OriginCorner::Center => vec![
      Move::Feed {
        to: point3(-10.0, -20.0, -1.0),
      },
      Move::Helix {
        to: point3(10.0, -20.0, -DEPTH),
        center: Point2::new(0.0, -20.0),
        ccw: true,
      },
    ],
  }
}

/// Sweep `moves` over a field at `origin` and return it.
fn cut(origin: OriginCorner, moves: Vec<Move>) -> Heightfield {
  let mut field = field(origin);
  sweep::apply(&program(moves), &mut field);
  field
}

/// Every cell of `actual` must be the depth `expected` reached, to rounding.
///
/// The two programs are the same curve sampled from the same end, so the samples coincide up to the
/// floating-point rounding of the two trigonometric expressions — never up to a cell.
fn assert_same_field(actual: &Heightfield, expected: &Heightfield, how: &str) {
  let (grid, other) = (actual.grid(), expected.grid());
  assert_eq!(
    (grid.columns, grid.rows),
    (other.columns, other.rows),
    "{how}: the two fields must cover the same stock"
  );
  for row in 0..grid.rows {
    for column in 0..grid.columns {
      let (left, right) = (actual.surface(column, row), expected.surface(column, row));
      assert!(
        (left - right).abs() < 1e-9,
        "{how}: cell ({column}, {row}) reads {left} mm where the document's own arc reads {right} mm"
      );
    }
  }
}

#[test]
fn an_arc_emitted_at_another_origin_cuts_where_the_document_puts_it() {
  let document = cut(OriginCorner::BottomLeft, document_arc());
  for corner in [OriginCorner::TopLeft, OriginCorner::Center] {
    let machine = cut(corner, machine_arc(corner));
    assert_same_field(&machine, &document, &format!("arc at {corner:?}"));
  }
}

#[test]
fn a_helix_emitted_at_another_origin_cuts_where_the_document_puts_it() {
  // A helix is an arc that descends, so it is read back by the same map and has the same handedness.
  let document = cut(OriginCorner::BottomLeft, document_helix());
  for corner in [OriginCorner::TopLeft, OriginCorner::Center] {
    let machine = cut(corner, machine_helix(corner));
    assert_same_field(&machine, &document, &format!("helix at {corner:?}"));
  }
}

#[test]
fn a_crash_is_reported_where_the_program_put_it() {
  // The field is in the document frame, but a crash's position is read against the program the user
  // opens, so it is reported in machine coordinates — the frame it was reported in before the field
  // learned about frames at all.
  for (corner, machine_x, machine_y) in [
    (OriginCorner::TopLeft, 10.0, 80.0),
    (OriginCorner::Center, -20.0, -30.0),
  ] {
    let mut field = field(corner);
    let from = point3(machine_x, machine_y, SAFE_Z);
    let crash = sweep::apply_move(
      &mut field,
      from,
      Move::Rapid {
        to: point3(machine_x, machine_y, -2.0),
      },
      tool(),
    )
    .expect("a rapid into the stock is a crash");

    assert_eq!(
      (crash.at.x.mm(), crash.at.y.mm(), crash.at.z.mm()),
      (machine_x, machine_y, -2.0),
      "{corner:?}: the crash is at the program's own point, not the field's"
    );
  }
}
