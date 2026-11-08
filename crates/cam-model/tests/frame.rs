//! The document frame's relation to the machine frame, at every origin corner (M7 A7).
//!
//! A document's geometry runs from the job's own corner, `(0, 0)` to `(width, height)`, while a
//! program's moves are in machine coordinates. [`Frame`] is the one place those two are related, so it
//! is pinned here on its own: the corner map, its exactness in both directions, and which corners make
//! it something other than the identity.

use cam_geom::Point2;
use cam_model::{Frame, Job, OriginCorner};
use cam_units::{Length, UnitSystem};

/// A sheet that is narrower than it is tall, so a Y reflection is visibly a different place.
const WIDTH: f64 = 80.0;
const HEIGHT: f64 = 120.0;

fn job(origin: OriginCorner) -> Job {
  Job {
    width: Length::from_mm(WIDTH),
    height: Length::from_mm(HEIGHT),
    thickness: Length::from_mm(18.0),
    units: UnitSystem::Mm,
    origin,
    safe_z: Length::from_mm(5.0),
    material: None,
  }
}

fn point(x: f64, y: f64) -> Point2 {
  Point2::new(x, y)
}

/// One corner pair: the document corner `(dx, dy)` and the machine corner `(mx, my)` it must map to.
type CornerMap = (f64, f64, f64, f64);

#[test]
fn each_origin_corner_maps_the_jobs_own_corners_to_the_machine_corners_it_names() {
  // Document corner, then the machine corner the job's origin selector puts it on. `bottom_left` is the
  // identity, `top_left` reflects Y so the document's `(0, 0)` is the machine's `(0, height)`, and
  // `center` translates so the document's middle is the machine's origin.
  let cases: [(OriginCorner, &[CornerMap]); 3] = [
    (
      OriginCorner::BottomLeft,
      &[
        (0.0, 0.0, 0.0, 0.0),
        (WIDTH, 0.0, WIDTH, 0.0),
        (WIDTH, HEIGHT, WIDTH, HEIGHT),
        (0.0, HEIGHT, 0.0, HEIGHT),
      ],
    ),
    (
      OriginCorner::TopLeft,
      &[
        (0.0, 0.0, 0.0, HEIGHT),
        (WIDTH, 0.0, WIDTH, HEIGHT),
        (WIDTH, HEIGHT, WIDTH, 0.0),
        (0.0, HEIGHT, 0.0, 0.0),
      ],
    ),
    (
      OriginCorner::Center,
      &[
        (0.0, 0.0, -WIDTH / 2.0, -HEIGHT / 2.0),
        (WIDTH, 0.0, WIDTH / 2.0, -HEIGHT / 2.0),
        (WIDTH, HEIGHT, WIDTH / 2.0, HEIGHT / 2.0),
        (0.0, HEIGHT, -WIDTH / 2.0, HEIGHT / 2.0),
        // The middle of the sheet is the machine's own origin.
        (WIDTH / 2.0, HEIGHT / 2.0, 0.0, 0.0),
      ],
    ),
  ];

  for (corner, expected) in cases {
    let frame = Frame::of(&job(corner));
    assert_eq!(frame.corner(), corner, "a frame names its own corner");
    assert_eq!(frame, job(corner).frame(), "one definition, two doors");
    for (document_x, document_y, machine_x, machine_y) in expected {
      let mapped = frame.to_machine(point(*document_x, *document_y));
      assert_eq!(
        (mapped.x, mapped.y),
        (*machine_x, *machine_y),
        "{corner:?}: document ({document_x}, {document_y}) must be machine \
         ({machine_x}, {machine_y})"
      );
    }
  }
}

#[test]
fn reading_a_machine_point_back_gives_the_document_point_it_came_from() {
  // The sweep depends on this being an exact inverse rather than an approximate one: a program is
  // mapped into the field's frame once, and every depth it removes is measured at the cell that lands
  // under the move. The points are all exact binary fractions, so "exact" is asserted, not tolerated.
  let points = [
    (0.0, 0.0),
    (WIDTH, 0.0),
    (WIDTH, HEIGHT),
    (0.0, HEIGHT),
    (WIDTH / 2.0, HEIGHT / 2.0),
    (13.25, 47.5),
    (0.125, 119.875),
  ];

  for corner in [
    OriginCorner::BottomLeft,
    OriginCorner::TopLeft,
    OriginCorner::Center,
  ] {
    let frame = Frame::of(&job(corner));
    for (x, y) in points {
      let document = point(x, y);
      let round_tripped = frame.from_machine(frame.to_machine(document));
      assert_eq!(
        (round_tripped.x, round_tripped.y),
        (document.x, document.y),
        "{corner:?}: ({x}, {y}) did not survive the round trip"
      );
      let machine = frame.to_machine(frame.from_machine(document));
      assert_eq!(
        (machine.x, machine.y),
        (document.x, document.y),
        "{corner:?}: ({x}, {y}) did not survive the other way round"
      );
    }
  }
}

#[test]
fn only_bottom_left_is_the_identity() {
  // `is_identity` is what lets the sweep be provably neutral for the corner every existing fixture
  // sits on: when it holds, every map above is the point itself.
  for corner in [
    OriginCorner::BottomLeft,
    OriginCorner::TopLeft,
    OriginCorner::Center,
  ] {
    let frame = Frame::of(&job(corner));
    assert_eq!(
      frame.is_identity(),
      corner == OriginCorner::BottomLeft,
      "{corner:?}"
    );
    let probe = point(13.25, 47.5);
    let mapped = frame.to_machine(probe);
    assert_eq!(
      mapped == probe,
      frame.is_identity(),
      "{corner:?}: the identity is exactly the frame that moves nothing"
    );
  }
}

#[test]
fn only_top_left_mirrors_handedness() {
  // A reflection reverses the sense of every arc, so an arc read back into the document frame has to
  // have its direction flipped with its positions. A translation does not: `center` keeps handedness.
  for corner in [
    OriginCorner::BottomLeft,
    OriginCorner::TopLeft,
    OriginCorner::Center,
  ] {
    assert_eq!(
      Frame::of(&job(corner)).is_reflection(),
      corner == OriginCorner::TopLeft,
      "{corner:?}"
    );
  }
}
