//! Behaviour tests for the data-driven G-code emitter (A5, A6, A9).

use cam_geom::Point2;
use cam_move::{Move, MoveStream, Point3, Segment, ToolShape};
use cam_post::{emit, parse_post, EmitError, ProgramMeta};
use cam_units::Length;

const POST_MM: &str = include_str!("fixtures/post_mm.pp");
const POST_INCH: &str = include_str!("fixtures/post_inch.pp");

fn meta() -> ProgramMeta {
  ProgramMeta {
    file_name: "part.c2d".to_owned(),
    file_path: "/jobs/part.c2d".to_owned(),
  }
}

fn profile_stream() -> MoveStream {
  let mut stream = MoveStream::new(Length::from_mm(5.0), Point2::new(0.0, 0.0));
  stream.segments.push(Segment {
    name: "Outside profile".to_owned(),
    tool_number: 1,
    tool: ToolShape::cylinder(Length::from_mm(6.0)),
    feed_rate: 100.0,
    plunge_rate: 30.0,
    spindle_speed: 18_000,
    moves: vec![
      Move::Rapid {
        to: Point3::new(10.0, 10.0, 5.0),
      },
      Move::Feed {
        to: Point3::new(10.0, 10.0, -1.0),
      },
      Move::Feed {
        to: Point3::new(30.0, 10.0, -1.0),
      },
      Move::Feed {
        to: Point3::new(30.0, 20.0, -1.0),
      },
      Move::Retract {
        to: Point3::new(30.0, 20.0, 5.0),
      },
    ],
  });
  stream
}

fn arc_stream() -> MoveStream {
  let mut stream = MoveStream::new(Length::from_mm(5.0), Point2::new(0.0, 0.0));
  stream.segments.push(Segment {
    name: "Arc segment".to_owned(),
    tool_number: 1,
    tool: ToolShape::cylinder(Length::from_mm(6.0)),
    feed_rate: 100.0,
    plunge_rate: 30.0,
    spindle_speed: 18_000,
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

fn lines(text: &str) -> Vec<&str> {
  text.split("\r\n").filter(|line| !line.is_empty()).collect()
}

#[test]
fn emits_a_profile_program_byte_for_byte() {
  let post = parse_post(POST_MM).expect("fixture post parses");
  let emitted = emit(&post, &profile_stream(), &meta()).expect("emits");
  let expected = [
    "( Outside profile )",
    "( CamPilot fixture Mach3 Arcs (mm) (*.nc) )",
    "N0G00G17G90G21",
    "N10T1M06",
    "N20S18000M03",
    "N30G00X10.0000Y10.0000",
    "N40G01Z-1.0000F100.0",
    "N50G01X30.0000",
    "N60G01Y20.0000",
    "N70G00Z5.0000",
    "N80G00Z5.0000",
    "N90G00X0.0000Y0.0000",
    "N100M05",
    "N110M30",
  ];
  assert_eq!(lines(&emitted.text), expected);
  assert_eq!(emitted.lines, expected.len());
}

#[test]
fn a_conditional_variable_is_emitted_once_per_change() {
  let post = parse_post(POST_MM).expect("fixture post parses");
  let emitted = emit(&post, &profile_stream(), &meta()).expect("emits");
  let feed_mentions = lines(&emitted.text)
    .iter()
    .filter(|line| line.contains('F'))
    .count();
  // The profile changes depth once, so the feed word appears on the plunge only: the following
  // feed moves repeat no rate.
  assert_eq!(feed_mentions, 1, "{}", emitted.text);
}

#[test]
fn an_arc_emits_g3_with_incremental_centre_offsets() {
  let post = parse_post(POST_MM).expect("fixture post parses");
  let emitted = emit(&post, &arc_stream(), &meta()).expect("emits");
  let arc_line = lines(&emitted.text)
    .into_iter()
    .find(|line| line.contains("G03"))
    .expect("an arc line");
  assert_eq!(arc_line, "N50G03X10.0000I5.0000J0.0000", "{}", emitted.text);
}

#[test]
fn inches_convert_coordinates_at_the_emitter_boundary() {
  let post = parse_post(POST_INCH).expect("fixture post parses");
  let emitted = emit(&post, &profile_stream(), &meta()).expect("emits");
  let text = emitted.text;
  // 10 mm is 0.3937 in, 5 mm is 0.1969 in: the conversion happens here, not in the geometry.
  assert!(text.contains("X0.3937Y0.3937"), "{text}");
  assert!(text.contains("Z0.1969"), "{text}");
}

#[test]
fn a_scale_factor_is_applied_before_formatting() {
  let post = parse_post(
    "POST_NAME = \"scaled\"\nVAR FEED_RATE = [FC|A||1.2|0.5]\nbegin FIRST_FEED_MOVE\n\"[FC]\"\n",
  )
  .expect("parses");
  let emitted = emit(&post, &profile_stream(), &meta()).expect("emits");
  assert!(
    emitted.text.contains("50.00"),
    "100 mm/s scaled by 0.5 at two decimals: {}",
    emitted.text
  );
}

#[test]
fn a_post_that_cannot_express_arcs_refuses_rather_than_approximating() {
  let post = parse_post(
    "POST_NAME = \"lines only\"\nVAR X_POSITION = [X|C|X|1.4]\nbegin FEED_MOVE\n\"G01[X]\"\n",
  )
  .expect("parses");
  let error = emit(&post, &arc_stream(), &meta()).expect_err("must refuse");
  assert!(
    matches!(error, EmitError::ArcsUnsupported { .. }),
    "{error:?}"
  );
  assert!(error.to_string().contains("cannot express"), "{error}");
}

#[test]
fn emitting_the_same_stream_twice_is_byte_identical() {
  let post = parse_post(POST_MM).expect("fixture post parses");
  let first = emit(&post, &profile_stream(), &meta()).expect("emits");
  let second = emit(&post, &profile_stream(), &meta()).expect("emits");
  assert_eq!(first.text, second.text);
}

#[test]
fn line_numbers_wrap_at_the_maximum() {
  let post = parse_post(
    "POST_NAME = \"wrap\"\nLINE_NUMBER_START = 0\nLINE_NUMBER_INCREMENT = 10\nLINE_NUMBER_MAXIMUM = 20\n\
     VAR LINE_NUMBER = [N|A|N|1.0]\nbegin HEADER\n\"[N]A\"\n\"[N]B\"\n\"[N]C\"\n\"[N]D\"\n",
  )
  .expect("parses");
  let mut stream = MoveStream::new(Length::from_mm(5.0), Point2::new(0.0, 0.0));
  stream.segments.push(Segment {
    name: "empty".to_owned(),
    tool_number: 1,
    tool: ToolShape::cylinder(Length::from_mm(6.0)),
    feed_rate: 1.0,
    plunge_rate: 1.0,
    spindle_speed: 1,
    moves: Vec::new(),
  });
  let emitted = emit(&post, &stream, &meta()).expect("emits");
  assert_eq!(lines(&emitted.text), ["N0A", "N10B", "N20C", "N0D"]);
}

#[test]
fn a_tool_change_between_segments_emits_toolchange_then_new_segment() {
  let post = parse_post(POST_MM).expect("fixture post parses");
  let mut stream = profile_stream();
  let mut second = stream.segments[0].clone();
  second.name = "Second".to_owned();
  second.tool_number = 2;
  stream.segments.push(second);

  let emitted = emit(&post, &stream, &meta()).expect("emits");
  let text = emitted.text;
  let toolchange = text.find("T2M06").expect("tool change emitted");
  let new_segment = text
    .rfind("G00Z5.0000")
    .expect("retract before new segment");
  assert!(
    toolchange < new_segment,
    "tool change precedes the new segment"
  );
}

#[test]
fn an_unknown_variable_is_an_error_not_an_empty_string() {
  let post = parse_post("begin HEADER\n\"G00[BOGUS]\"\n").expect("parses");
  let error = emit(&post, &profile_stream(), &meta()).expect_err("must reject");
  assert!(
    matches!(error, EmitError::UnknownVariable { .. }),
    "{error:?}"
  );
  assert!(error.to_string().contains("BOGUS"), "{error}");
}

#[test]
fn an_unterminated_reference_is_an_error() {
  let post = parse_post("begin HEADER\n\"G00[X\"\n").expect("parses");
  let error = emit(&post, &profile_stream(), &meta()).expect_err("must reject");
  assert!(
    matches!(error, EmitError::UnterminatedReference { .. }),
    "{error:?}"
  );
}

#[test]
fn a_post_without_a_footer_block_still_emits_cleanly() {
  let post = parse_post(
    "POST_NAME = \"minimal\"\nVAR X_POSITION = [X|A|X|1.0]\nbegin INITIAL_RAPID_MOVE\n\"G00[X]\"\n",
  )
  .expect("parses");
  let emitted = emit(&post, &profile_stream(), &meta()).expect("emits");
  assert_eq!(lines(&emitted.text), ["G00X10"]);
}

#[test]
fn blank_expansions_do_not_produce_empty_lines() {
  let post = parse_post(POST_MM).expect("fixture post parses");
  let emitted = emit(&post, &profile_stream(), &meta()).expect("emits");
  assert!(
    !emitted.text.contains("\r\n\r\n"),
    "no blank lines allowed: {:?}",
    emitted.text
  );
}
