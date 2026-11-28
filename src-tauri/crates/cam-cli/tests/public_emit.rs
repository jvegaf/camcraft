//! The document→program emission path as a public library API (S4).
//!
//! `cam-cli program` and, later, `cam-ui` must emit through the *same* function, so it has to be
//! reachable from outside the crate as `cam_cli::emit_program`. These tests call it there, never
//! through `cam_cli::commands::…`, and assert the observable contract the promoted function keeps:
//! the file it writes, its determinism, and the exact refusals it returns.

use std::path::{Path, PathBuf};

use cam_geom::Point2;
use cam_move::{Move, MoveStream, Point3, Segment, ToolShape};
use cam_units::Length;

const POST: &str = include_str!("../../cam-post/tests/fixtures/post_mm.pp");

fn scratch(name: &str) -> PathBuf {
  let directory = std::env::temp_dir().join(format!("camcli-pubemit-{}-{name}", std::process::id()));
  let _ = std::fs::remove_dir_all(&directory);
  std::fs::create_dir_all(&directory).expect("scratch directory");
  directory
}

/// One cut: a rapid to clearance, a plunge and a feed along X.
fn stream() -> MoveStream {
  let mut stream = MoveStream::new(Length::from_mm(5.0), Point2::new(0.0, 0.0));
  stream.segments.push(Segment {
    name: "Cut".to_owned(),
    tool_number: 1,
    tool: ToolShape::cylinder(Length::from_mm(2.0)),
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
      Move::Feed {
        to: Point3::new(10.0, 0.0, -1.0),
      },
      Move::Retract {
        to: Point3::new(10.0, 0.0, 5.0),
      },
    ],
  });
  stream
}

fn write_post(directory: &Path, name: &str, text: &str) -> PathBuf {
  let path = directory.join(name);
  std::fs::write(&path, text).expect("post writes");
  path
}

fn document_path(directory: &Path) -> PathBuf {
  directory.join("board.cmc")
}

#[test]
fn the_public_function_writes_the_program_deterministically() {
  let directory = scratch("determinism");
  let post = write_post(&directory, "post.pp", POST);
  let document = document_path(&directory);
  let stream = stream();

  let first = directory.join("first.nc");
  let second = directory.join("second.nc");
  cam_cli::emit_program(&stream, &post, Some(&first), &document).expect("first run emits");
  cam_cli::emit_program(&stream, &post, Some(&second), &document).expect("second run emits");

  let first_text = std::fs::read_to_string(&first).expect("first program file");
  let second_text = std::fs::read_to_string(&second).expect("second program file");
  assert!(
    first_text.contains("G1"),
    "the program carries the cutting move:\n{first_text}"
  );
  assert_eq!(
    first_text, second_text,
    "the same stream and post must emit byte for byte the same program"
  );
  assert!(
    !directory.join("first.nc.tmp").exists(),
    "the temporary file is renamed away, not left behind"
  );
}

#[test]
fn a_post_that_cannot_be_read_is_refused_with_invalid_input() {
  let directory = scratch("missing-post");
  let missing = directory.join("absent.pp");
  let out = directory.join("board.nc");

  let failure = cam_cli::emit_program(&stream(), &missing, Some(&out), &document_path(&directory))
    .expect_err("a missing post is refused");

  assert_eq!(failure.code(), 2, "{}", failure.message);
  assert!(
    failure.message.contains(&missing.display().to_string()),
    "the refusal names the post file: {}",
    failure.message
  );
  assert!(!out.exists(), "a refused emission writes no file");
}

#[test]
fn a_post_that_does_not_parse_is_a_post_error() {
  let directory = scratch("bad-post");
  let post = write_post(&directory, "broken.pp", "begin not_a_block\n");
  let out = directory.join("board.nc");

  let failure = cam_cli::emit_program(&stream(), &post, Some(&out), &document_path(&directory))
    .expect_err("an unparsable post is refused");

  assert_eq!(failure.code(), 4, "{}", failure.message);
  assert!(
    failure.message.contains(&post.display().to_string()),
    "the refusal names the post file: {}",
    failure.message
  );
  assert!(!out.exists(), "a refused emission writes no file");
}
