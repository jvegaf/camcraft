//! Behaviour tests for the built-in GRBL post-processor (W4b T3).

use cam_geom::Point2;
use cam_move::{Move, MoveStream, Point3, Segment, ToolShape};
use cam_post::{emit, parse_post, ProgramMeta, GRBL_POST};
use cam_units::Length;

fn meta() -> ProgramMeta {
  ProgramMeta {
    file_name: "part.cmc".to_owned(),
    file_path: "/jobs/part.cmc".to_owned(),
  }
}

fn profile_stream() -> MoveStream {
  let mut stream = MoveStream::new(Length::from_mm(5.0), Point2::new(0.0, 0.0));
  stream.segments.push(Segment {
    name: "Profile".to_owned(),
    tool_number: 1,
    tool: ToolShape::cylinder(Length::from_mm(3.0)),
    feed_rate: 100.0,
    plunge_rate: 30.0,
    spindle_speed: 12_000,
    moves: vec![
      Move::Rapid {
        to: Point3::new(10.0, 0.0, 5.0),
      },
      Move::Feed {
        to: Point3::new(10.0, 0.0, -1.0),
      },
      Move::Feed {
        to: Point3::new(20.0, 0.0, -1.0),
      },
      Move::Feed {
        to: Point3::new(20.0, 10.0, -1.0),
      },
      Move::Retract {
        to: Point3::new(20.0, 10.0, 5.0),
      },
    ],
  });
  stream
}

fn arc_stream() -> MoveStream {
  let mut stream = MoveStream::new(Length::from_mm(5.0), Point2::new(0.0, 0.0));
  stream.segments.push(Segment {
    name: "Arc".to_owned(),
    tool_number: 1,
    tool: ToolShape::cylinder(Length::from_mm(3.0)),
    feed_rate: 100.0,
    plunge_rate: 30.0,
    spindle_speed: 12_000,
    moves: vec![
      Move::Rapid {
        to: Point3::new(0.0, 0.0, 5.0),
      },
      Move::Feed {
        to: Point3::new(0.0, 0.0, -1.0),
      },
      Move::Arc {
        to: Point3::new(10.0, 0.0, -1.0),
        center: Point2::new(5.0, 0.0),
        ccw: true,
      },
      Move::Retract {
        to: Point3::new(10.0, 0.0, 5.0),
      },
    ],
  });
  stream
}

#[test]
fn grbl_post_parses() {
  let post = parse_post(GRBL_POST).expect("built-in GRBL post parses");
  assert_eq!(post.file_extension, "nc");
}

#[test]
fn profile_emits_byte_for_byte() {
  let post = parse_post(GRBL_POST).expect("built-in GRBL post parses");
  let emitted = emit(&post, &profile_stream(), &meta()).expect("emits");
  let expected = concat!(
    "G21 G90 G94 G17\n",
    "M3 S12000\n",
    "G0X10.000Y0.000Z5.000\n",
    "G1Z-1.000F30.0\n",
    "G1X20.000F100.0\n",
    "G1Y10.000\n",
    "G0Z5.000\n",
    "M5\n",
    "M30\n",
  );
  assert_eq!(emitted.text, expected);
}

#[test]
fn emitting_the_same_stream_twice_is_byte_identical() {
  let post = parse_post(GRBL_POST).expect("built-in GRBL post parses");
  let first = emit(&post, &profile_stream(), &meta()).expect("emits");
  let second = emit(&post, &profile_stream(), &meta()).expect("emits");
  assert_eq!(first.text, second.text);
}

#[test]
fn arc_emits_g2_g3_with_i_j() {
  let post = parse_post(GRBL_POST).expect("built-in GRBL post parses");
  let emitted = emit(&post, &arc_stream(), &meta()).expect("emits");
  let arc_line = emitted
    .text
    .lines()
    .find(|line| {
      (line.starts_with("G2") || line.starts_with("G3"))
        && line.as_bytes().get(2).is_none_or(|b| !b.is_ascii_digit())
    })
    .expect("an arc line with G2/G3");
  assert!(arc_line.contains("G3"), "counter-clockwise arc: {arc_line}");
  assert!(arc_line.contains('I'), "arc carries I: {arc_line}");
  assert!(arc_line.contains('J'), "arc carries J: {arc_line}");
}

#[test]
fn a_tool_change_stops_and_restarts_the_spindle() {
  // GRBL has no automatic tool changer, so the program must stop the spindle, tell the operator
  // which tool is wanted, and start it again: a change that only emits `M5` leaves the second
  // segment cutting with the spindle off.
  let post = parse_post(GRBL_POST).expect("built-in GRBL post parses");
  let mut stream = profile_stream();
  let mut second = stream.segments[0].clone();
  second.name = "Second".to_owned();
  second.tool_number = 2;
  second.spindle_speed = 9_000;
  stream.segments.push(second);

  let emitted = emit(&post, &stream, &meta()).expect("emits");
  let lines: Vec<&str> = emitted.text.lines().collect();
  let change = lines
    .iter()
    .position(|line| line.contains("(tool change)"))
    .expect("tool change comment");
  assert_eq!(
    lines[change - 1],
    "M5",
    "the spindle stops before the change: {:?}",
    &lines[change - 1..=change + 1]
  );
  assert_eq!(
    lines[change + 1],
    "M3 S9000",
    "the spindle restarts at the new tool's speed: {:?}",
    &lines[change - 1..=change + 1]
  );
}
