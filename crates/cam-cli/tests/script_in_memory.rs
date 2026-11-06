//! Tests for running a script against a document the caller owns (M10's residual, closed in M11 T1).
//!
//! These call [`cam_cli::script::run_script_on`] directly, not the binary: the contract is that the
//! document the caller holds is the one the script mutates, that the whole run is the transaction, and
//! that a command which would read or write a file for that document is refused instead of silently
//! reaching the disk. `crates/cam-cli/tests/script.rs` keeps pinning the binary's own path.

use std::path::{Path, PathBuf};

use cam_cli::script::run_script_on;
use cam_model::{Document, Job, OriginCorner};
use cam_units::{Length, UnitSystem};

/// A scratch directory under `/var/tmp`: `/tmp` is a full tmpfs here, and the name must be one path
/// segment because this fixture removes what it created.
fn scratch(name: &str) -> PathBuf {
  assert!(
    !name.contains('/') && !name.contains('\\') && !name.contains(".."),
    "a scratch name is one path segment, got {name:?}"
  );
  let directory =
    PathBuf::from("/var/tmp").join(format!("campilot-in-memory-{}-{name}", std::process::id()));
  let _ = std::fs::remove_dir_all(&directory);
  std::fs::create_dir_all(&directory).expect("scratch directory");
  directory
}

/// An untitled document: a job and nothing else.
fn document() -> Document {
  Document::new(Job {
    width: Length::from_mm(200.0),
    height: Length::from_mm(100.0),
    thickness: Length::from_mm(18.0),
    units: UnitSystem::Mm,
    origin: OriginCorner::BottomLeft,
    safe_z: Length::from_mm(5.0),
  })
}

/// Write `text` as the script at `directory/script.c2ds` and run it against `document`.
///
/// Returns the two sinks and the refusal's message, so a caller can assert the report, the notice and
/// the diagnostic without a second helper.
fn run(
  directory: &Path,
  text: &str,
  document: &mut Document,
  document_path: Option<&Path>,
) -> (String, String, Result<(), String>) {
  let script = directory.join("script.c2ds");
  std::fs::write(&script, text).expect("script file");
  let mut reports = Vec::new();
  let mut notices = Vec::new();
  let outcome = run_script_on(&script, document, document_path, &mut reports, &mut notices)
    .map_err(|failure| failure.to_string());
  (
    String::from_utf8(reports).expect("utf-8 reports"),
    String::from_utf8(notices).expect("utf-8 notices"),
    outcome,
  )
}

#[test]
fn the_run_mutates_the_document_the_caller_owns() {
  let directory = scratch("mutates");
  let mut document = document();
  assert!(document.nesting.is_none(), "the fixture starts un-nested");

  let (reports, notices, outcome) = run(&directory, "sheet 300 200\n", &mut document, None);

  assert_eq!(outcome, Ok(()), "a script of one accepted command runs");
  let nesting = document
    .nesting
    .as_ref()
    .expect("the sheet the script authored is in the caller's document");
  assert_eq!(nesting.sheets.len(), 1, "exactly the one sheet authored");
  assert_eq!(nesting.sheets[0].width, Length::from_mm(300.0));
  assert_eq!(nesting.sheets[0].height, Length::from_mm(200.0));
  assert_eq!(reports, "", "`sheet` reports nothing");
  assert_eq!(notices, "", "and writes no notice");
}

#[test]
fn a_refused_line_leaves_the_document_byte_identical() {
  let directory = scratch("transactional");
  let mut document = document();

  // The first line is accepted and the second is not: a run is one transaction, so the accepted line
  // must not survive the refusal that follows it.
  let (_, _, outcome) = run(&directory, "sheet 300 200\nnope\n", &mut document, None);

  let message = outcome.expect_err("an unknown command refuses the run");
  assert!(
    message.starts_with("line 2:"),
    "the refusal names the line that caused it, got {message:?}"
  );
  assert!(
    message.contains("unknown script command \"nope\""),
    "and the command it could not place, got {message:?}"
  );
  assert!(
    document.nesting.is_none(),
    "the accepted line before the refusal is not kept: a run is one transaction"
  );
}

#[test]
fn save_is_refused_when_the_document_belongs_to_the_caller() {
  let directory = scratch("save");
  let mut document = document();

  let (_, _, outcome) = run(&directory, "save out.c2d\n", &mut document, None);

  let message = outcome.expect_err("a script run against a caller's document writes no file");
  assert!(
    message.starts_with("line 1:1:"),
    "located at the command word, got {message:?}"
  );
  assert!(
    message.contains("writes no file"),
    "and it says what the rule is, got {message:?}"
  );
  assert!(
    !directory.join("out.c2d").exists(),
    "nothing reached the disk"
  );
}

#[test]
fn open_is_refused_when_the_document_belongs_to_the_caller() {
  let directory = scratch("open");
  let mut document = document();

  let (_, _, outcome) = run(&directory, "open other.c2d\n", &mut document, None);

  let message =
    outcome.expect_err("a script against the shell's document cannot open a second one");
  assert!(
    message.starts_with("line 1:1:"),
    "located at the command word, got {message:?}"
  );
  assert!(
    message.contains("already has a document open"),
    "and it says why, got {message:?}"
  );
}

#[test]
fn a_command_that_needs_a_file_beside_the_document_says_the_document_has_no_path() {
  let directory = scratch("no-path");
  let mut document = document();

  // `emit` resolves the post-processor relative to the document, so an untitled document cannot run
  // it — and the refusal must say that, not claim no document is open.
  let (_, _, outcome) = run(&directory, "emit --post post_mm.pp\n", &mut document, None);

  let message = outcome.expect_err("an untitled document has no path to resolve beside");
  assert!(message.starts_with("line 1:"), "located, got {message:?}");
  assert!(
    message.contains("has no path yet"),
    "names the missing path rather than a missing document, got {message:?}"
  );
  assert!(
    !message.contains("no document is open"),
    "and never pretends there is no document, got {message:?}"
  );
}
