//! Behaviour tests for the built-in Repetier post-processor (E5.1).

use cam_geom::Point2;
use cam_move::{Move, MoveStream, Point3, Segment, ToolShape};
use cam_post::{emit, parse_post, ProgramMeta, REPETIER_POST};
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

#[test]
fn repetier_post_parses() {
  let post = parse_post(REPETIER_POST).expect("built-in Repetier post parses");
  assert_eq!(post.file_extension, "gcode");
}

#[test]
fn profile_emits_byte_for_byte() {
  let post = parse_post(REPETIER_POST).expect("built-in Repetier post parses");
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
    "G0 X0.000 Y0.000\n",
    "M30\n",
  );
  assert_eq!(emitted.text, expected);
}

#[test]
fn emitting_the_same_stream_twice_is_byte_identical() {
  let post = parse_post(REPETIER_POST).expect("built-in Repetier post parses");
  let first = emit(&post, &profile_stream(), &meta()).expect("emits");
  let second = emit(&post, &profile_stream(), &meta()).expect("emits");
  assert_eq!(first.text, second.text);
}
