//! End-to-end tests for `cam-cli drill` (E2.1, E2.2).
//!
//! These run the real binary, so they assert the observable contract: exit code, exact stdout and
//! stderr, and the files left on disk.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const POST: &str = include_str!("../../cam-post/tests/fixtures/post_mm.pp");

fn binary() -> &'static str {
  env!("CARGO_BIN_EXE_cam-cli")
}

fn scratch(name: &str) -> PathBuf {
  let directory = std::env::temp_dir().join(format!("camdrill-{}-{name}", std::process::id()));
  let _ = std::fs::remove_dir_all(&directory);
  std::fs::create_dir_all(&directory).expect("scratch directory");
  directory
}

/// One tool and two hits at the places these tests look for in the program.
fn write_drill(directory: &Path) -> PathBuf {
  let path = directory.join("board.drl");
  let text = "\
M48
FMAT,2
METRIC
T1C0.800
%
G90
T1
X10.0Y10.0
X20.0Y10.0
";
  std::fs::write(&path, text).expect("drill file writes");
  path
}

fn write_post(directory: &Path) -> PathBuf {
  let path = directory.join("post.pp");
  std::fs::write(&path, POST).expect("post writes");
  path
}

fn run(arguments: &[&str]) -> Output {
  Command::new(binary())
    .args(arguments)
    .output()
    .expect("cam-cli runs")
}

fn stdout(output: &Output) -> String {
  String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
  String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The temporary files a run left in `directory`, whatever they are called.
fn leftovers(directory: &Path) -> Vec<String> {
  std::fs::read_dir(directory)
    .expect("the scratch directory")
    .filter_map(|entry| entry.ok())
    .map(|entry| entry.file_name().to_string_lossy().into_owned())
    .filter(|name| name.contains("tmp"))
    .collect()
}

/// A depth flag that is not a real number is refused by name before it can reach the program.
///
/// `parse_number` handed `NaN` and `±inf` straight to the drilling parameters. Every guard between
/// there and the emitter compares numbers, and every comparison against `NaN` is false, so the guards
/// let it through and the program carried `ZNaN`: a controller stops on a word it cannot read at the
/// one moment the tool is in the material. A value with no number behind it is an input error, and it
/// is refused saying which flag carried it (R1-drill-nan-depth-validation, R3-003).
#[test]
fn drill_refuses_a_non_finite_parameter_before_it_reaches_the_program() {
  let directory = scratch("non-finite");
  let drill = write_drill(&directory);
  let post = write_post(&directory);
  let out = directory.join("program.nc");
  // An earlier run's program, so a refusal that already opened the file shows up as changed bytes.
  std::fs::write(&out, "an earlier program\n").expect("the earlier program writes");

  for (flag, value) in [
    ("--cut-depth", "NaN"),
    ("--peck-depth", "NaN"),
    ("--start-depth", "NaN"),
    ("--retract", "NaN"),
    ("--cut-depth", "inf"),
    ("--peck-depth", "-inf"),
    ("--start-depth", "inf"),
  ] {
    let arguments = non_finite_arguments(&drill, &post, &out, flag, value);
    let borrowed: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let output = run(&borrowed);
    assert_eq!(output.status.code(), Some(2), "{flag} {value}: {}", stderr(&output));
    assert!(
      stdout(&output).is_empty(),
      "{flag} {value}: {}",
      stdout(&output)
    );
    assert_eq!(
      stderr(&output),
      format!("error: {flag} expects a finite number, got {value:?}\n"),
      "{flag} {value}"
    );
    assert_eq!(
      std::fs::read_to_string(&out).expect("the earlier program survives"),
      "an earlier program\n",
      "{flag} {value}: a refused run must not touch the file it was asked to write"
    );
  }
  assert!(
    leftovers(&directory).is_empty(),
    "no temporary is left behind: {:?}",
    leftovers(&directory)
  );
  std::fs::remove_dir_all(&directory).ok();
}

/// A complete `drill` argument list in which `flag` carries `value`.
///
/// The depth the command requires stays finite unless it is the flag under test, so the only reason
/// the run can be refused is the value this test put there.
fn non_finite_arguments(
  drill: &Path,
  post: &Path,
  out: &Path,
  flag: &str,
  value: &str,
) -> Vec<String> {
  let mut arguments: Vec<String> = vec![
    "drill".to_owned(),
    drill.to_string_lossy().into_owned(),
    "--post".to_owned(),
    post.to_string_lossy().into_owned(),
    "--out".to_owned(),
    out.to_string_lossy().into_owned(),
  ];
  if flag != "--cut-depth" {
    arguments.push("--cut-depth".to_owned());
    arguments.push("1.6".to_owned());
  }
  arguments.push(flag.to_owned());
  arguments.push(value.to_owned());
  arguments
}

/// `--out` puts the program in place by renaming a temporary over it, so it never writes *into* the
/// path it was given (R1-drill-non-atomic-write, R3-002, R4-drill-non-atomic-output-write).
///
/// A direct `std::fs::write` truncates first and follows a symlink, so a program that fails half way
/// leaves a `.nc` a controller will happily run short — with no footer, no `M5`, no `M30` — and a
/// link at the destination silently redirects the program onto another file. Renaming a completed
/// temporary closes both: the destination is either the previous file or the whole new one, and the
/// link is replaced rather than followed.
#[cfg(unix)]
#[test]
fn drill_out_replaces_the_program_file_instead_of_writing_into_it() {
  let directory = scratch("atomic");
  let drill = write_drill(&directory);
  let post = write_post(&directory);
  let out = directory.join("program.nc");
  let victim = directory.join("victim.txt");
  std::fs::write(&victim, "somebody else's file\n").expect("the victim writes");
  std::os::unix::fs::symlink(&victim, &out).expect("the destination is a symlink");

  let output = run(&[
    "drill",
    drill.to_string_lossy().as_ref(),
    "--post",
    post.to_string_lossy().as_ref(),
    "--cut-depth",
    "1.6",
    "--out",
    out.to_string_lossy().as_ref(),
  ]);
  assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
  assert!(stdout(&output).is_empty(), "{}", stdout(&output));
  assert!(stderr(&output).contains("wrote"), "{}", stderr(&output));

  assert_eq!(
    std::fs::read_to_string(&victim).expect("the linked file survives"),
    "somebody else's file\n",
    "the program must not be written through the destination's symlink"
  );
  let metadata = std::fs::symlink_metadata(&out).expect("the program is there");
  assert!(
    metadata.file_type().is_file(),
    "the destination is the program itself, not the link: {metadata:?}"
  );
  let program = std::fs::read_to_string(&out).expect("the program is a file");
  assert!(program.contains("X10.0000Y10.0000"), "{program}");
  assert!(
    leftovers(&directory).is_empty(),
    "no temporary is left behind: {:?}",
    leftovers(&directory)
  );
  std::fs::remove_dir_all(&directory).ok();
}

/// The drilling flags still reach a program: finite depths, a finite `--retract` amount, and a
/// program either on disk or on stdout.
#[test]
fn drill_still_writes_a_program_for_finite_parameters() {
  let directory = scratch("finite");
  let drill = write_drill(&directory);
  let post = write_post(&directory);
  let out = directory.join("program.nc");

  let to_file = run(&[
    "drill",
    drill.to_string_lossy().as_ref(),
    "--post",
    post.to_string_lossy().as_ref(),
    "--cut-depth",
    "1.6",
    "--peck-depth",
    "0.4",
    "--start-depth",
    "0",
    "--retract",
    "0.5",
    "--out",
    out.to_string_lossy().as_ref(),
  ]);
  assert_eq!(to_file.status.code(), Some(0), "{}", stderr(&to_file));
  assert!(stdout(&to_file).is_empty(), "{}", stdout(&to_file));
  let report = stderr(&to_file);
  assert!(report.contains("drill: 2 hit(s), 1 tool(s)"), "{report}");
  let program = std::fs::read_to_string(&out).expect("the program is written");
  assert!(program.contains("X10.0000Y10.0000"), "{program}");
  assert!(program.contains("X20.0000"), "{program}");
  assert!(
    leftovers(&directory).is_empty(),
    "no temporary is left behind: {:?}",
    leftovers(&directory)
  );

  // Without `--out` the program is the machine output on stdout and the report stays on stderr.
  let to_stdout = run(&[
    "drill",
    drill.to_string_lossy().as_ref(),
    "--post",
    post.to_string_lossy().as_ref(),
    "--cut-depth",
    "1.6",
  ]);
  assert_eq!(to_stdout.status.code(), Some(0), "{}", stderr(&to_stdout));
  assert!(
    stdout(&to_stdout).contains("X10.0000Y10.0000"),
    "{}",
    stdout(&to_stdout)
  );
  std::fs::remove_dir_all(&directory).ok();
}
