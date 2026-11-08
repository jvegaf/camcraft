use super::support::*;
use cam_app::AppState;
use cam_ui::{handle_action, Shell, ShellOutput, UiAction};

/// A post-processor that parses and emits, small enough to live in the test.
///
/// The variables and the six blocks are the ones a linear program needs; declaring `LINE_NUMBER` is what
/// makes `[N]` a known variable rather than a name the emitter has to refuse. A real `.pp` is a file the
/// person chooses, so every test that needs one writes this to a scratch directory and hands the shell the
/// path — nothing reads a fixture out of another crate's tests.
const MINIMAL_POST: &str = r#"POST_NAME = "Minimal (mm)"
FILE_EXTENSION = "nc"
UNITS = "MM"
LINE_ENDING = "[13][10]"
VAR LINE_NUMBER = [N|A|N|1.0]
VAR FEED_RATE = [F|C|F|1.1]
VAR X_POSITION = [X|C|X|1.4]
VAR Y_POSITION = [Y|C|Y|1.4]
VAR Z_POSITION = [Z|C|Z|1.4]

begin HEADER
"( [TOOLPATH_NAME] )"
"[N]G00G17G90G21"

begin RAPID_MOVE
"[N]G00[X][Y]"

begin PLUNGE_MOVE
"[N]G01[Z][F]"

begin FEED_MOVE
"[N]G01[X][Y][Z]"

begin RETRACT_MOVE
"[N]G00[Z]"

begin FOOTER
"[N]M30"
"#;

/// Write a post-processor into the scratch directory and return where it went.
fn write_post(directory: &std::path::Path, name: &str, source: &str) -> std::path::PathBuf {
  let path = directory.join(name);
  std::fs::write(&path, source).expect("the test writes its own post-processor");
  path
}

/// Click one app-bar button by the word it reports.
///
/// The bar reports the point each of its two presses sits at ([`cam_ui::DrawnState::app_bar_hits`]) for the
/// same reason the palette does: egui cannot be asked for a widget by its text from outside a frame, and a
/// test that guessed at the layout would break the day a margin moved.
fn click_app_bar(
  ctx: &egui::Context,
  shell: &mut Shell,
  app: &mut AppState,
  word: &str,
) -> (ShellOutput, Vec<String>) {
  let (output, _) = frame_with(ctx, shell, app);
  let at = output
    .drawn
    .app_bar_hits
    .iter()
    .find(|(label, _)| label == word)
    .map(|(_, point)| egui::pos2(point.0, point.1))
    .unwrap_or_else(|| panic!("no {word:?} button: {:?}", output.drawn.app_bar_hits));
  click_at(ctx, shell, app, at)
}

#[test]
fn the_app_bar_asks_the_host_for_a_program_and_for_a_post_processor() {
  // M13 S2: the primary action is pressable and asks the host for a path instead of being greyed out with a
  // hover excuse, and the dialect it is written in has a control of its own.
  let mut app = AppState::new(document_with_toolpath(1, 4));
  let before = app.document_json();
  let mut shell = Shell::new();
  let ctx = egui::Context::default();
  let (_output, texts) = frame_with(&ctx, &mut shell, &mut app);
  let all = joined(&texts);
  assert!(
    all.contains("Generate G-code"),
    "the primary action must be painted: {all}"
  );
  assert!(
    all.contains("Post…"),
    "with no post-processor chosen the bar must say so on the control: {all}"
  );

  let (output, _) = click_app_bar(&ctx, &mut shell, &mut app, "Generate G-code");
  assert_eq!(
    output.actions,
    vec![UiAction::RequestGenerateGcode],
    "the primary action asks the host for a path"
  );
  assert_eq!(app.document_json(), before, "an intent touches no document");
  assert_eq!(shell.document_path(), None, "an intent writes no file");

  let (output, _) = click_app_bar(&ctx, &mut shell, &mut app, "Post");
  assert_eq!(
    output.actions,
    vec![UiAction::RequestChoosePost],
    "choosing the dialect asks the host for a file"
  );
  assert_eq!(app.document_json(), before, "an intent touches no document");
  assert_eq!(shell.post_name(), None, "an intent holds no post-processor");
}

#[test]
fn choosing_a_post_processor_holds_it_and_refuses_a_file_that_is_not_one() {
  let directory = scratch("post-choose");
  let good = write_post(&directory, "minimal.pp", MINIMAL_POST);
  let bad = write_post(
    &directory,
    "not-a-post.pp",
    "this file is not a post-processor\n",
  );
  let mut app = AppState::new(document_with_toolpath(1, 4));
  let mut shell = Shell::new();

  shell.apply_action(&mut app, &UiAction::ChoosePost { path: good });
  assert_eq!(shell.post_name(), Some("minimal.pp"));
  assert!(
    shell.status.contains("minimal.pp"),
    "the status bar must name the choice: {}",
    shell.status
  );

  // A file that does not parse is refused **and** the working one stays: a choice that could not write a
  // program is not a choice, and losing the post in hand to it would be a second failure on top of the first.
  shell.apply_action(&mut app, &UiAction::ChoosePost { path: bad });
  assert!(
    shell.status.starts_with("refused:"),
    "an unparsable post-processor must be refused: {}",
    shell.status
  );
  assert_eq!(
    shell.post_name(),
    Some("minimal.pp"),
    "the refusal must leave the held post-processor alone"
  );

  // The empty path names no file, so it reaches no filesystem call and costs the choice nothing either.
  shell.apply_action(
    &mut app,
    &UiAction::ChoosePost {
      path: Default::default(),
    },
  );
  assert!(shell.status.starts_with("refused:"));
  assert_eq!(shell.post_name(), Some("minimal.pp"));

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn generating_gcode_writes_the_program_and_names_it() {
  let directory = scratch("post-generate");
  let post = write_post(&directory, "minimal.pp", MINIMAL_POST);
  let target = directory.join("part.nc");
  let mut app = AppState::new(document_with_toolpath(1, 4));
  let before = app.document_json();
  let mut shell = Shell::new();
  shell.set_tools(vec![stored_tool(4)]);

  shell.apply_action(&mut app, &UiAction::ChoosePost { path: post });
  shell.apply_action(
    &mut app,
    &UiAction::GenerateGcode {
      path: target.clone(),
    },
  );

  let written = std::fs::read_to_string(&target).expect("the program is written");
  assert!(
    written.contains("G01"),
    "the program must carry the cutting move: {written}"
  );
  assert!(
    written.trim_end().ends_with("M30"),
    "the program must end with the post-processor's footer: {written}"
  );
  assert!(
    shell.status.starts_with("generate g-code"),
    "the status bar must say what was written: {}",
    shell.status
  );
  assert!(
    shell.status.contains("line(s) to part.nc"),
    "the status bar must name the file and the size: {}",
    shell.status
  );
  assert_eq!(
    app.document_json(),
    before,
    "writing a program is not an edit of the drawing"
  );

  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_drawing_that_names_a_material_still_produces_a_program() {
  // The trap this test exists for: a snapshot is a list of tools, a material is a selector *into a library*,
  // and `program_for_snapshot` refuses a material by name. Passing the document's own material would make
  // every drawing that names one refuse to write a program — the exact opposite of M13 S2 — so the program
  // is written at the tools' own rates and the shell's estimate and its program agree about the same job.
  let directory = scratch("post-material");
  let post = write_post(&directory, "minimal.pp", MINIMAL_POST);
  let target = directory.join("part.nc");
  let mut document = document_with_toolpath(1, 4);
  document.job.material = Some("Plywood".to_owned());
  let mut app = AppState::new(document);
  let mut shell = Shell::new();
  shell.set_tools(vec![stored_tool(4)]);
  shell.apply_action(&mut app, &UiAction::ChoosePost { path: post });

  shell.apply_action(
    &mut app,
    &UiAction::GenerateGcode {
      path: target.clone(),
    },
  );

  assert!(
    shell.status.starts_with("generate g-code"),
    "a named material must not stop the program: {}",
    shell.status
  );
  assert!(target.exists(), "the program is written");
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn generating_gcode_without_a_post_processor_writes_nothing() {
  let directory = scratch("post-missing");
  let target = directory.join("part.nc");
  let mut app = AppState::new(document_with_toolpath(1, 4));
  let mut shell = Shell::new();
  shell.set_tools(vec![stored_tool(4)]);

  shell.apply_action(
    &mut app,
    &UiAction::GenerateGcode {
      path: target.clone(),
    },
  );

  assert!(
    shell.status.starts_with("refused:"),
    "no post-processor must refuse by name: {}",
    shell.status
  );
  assert!(
    shell.status.contains("post-processor"),
    "the refusal must name what is missing: {}",
    shell.status
  );
  assert!(
    shell.status.contains("Post…"),
    "the refusal must name the control that supplies it: {}",
    shell.status
  );
  assert!(!target.exists(), "a refused program writes no file");
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn generating_gcode_from_a_drawing_with_no_toolpaths_writes_nothing() {
  let directory = scratch("post-notoolpath");
  let post = write_post(&directory, "minimal.pp", MINIMAL_POST);
  let target = directory.join("part.nc");
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.set_tools(vec![stored_tool(4)]);
  shell.apply_action(&mut app, &UiAction::ChoosePost { path: post });

  shell.apply_action(
    &mut app,
    &UiAction::GenerateGcode {
      path: target.clone(),
    },
  );

  assert!(
    shell.status.starts_with("refused:"),
    "a drawing with no toolpaths has no program: {}",
    shell.status
  );
  assert!(
    shell.status.contains("no toolpaths"),
    "the refusal must name the missing half: {}",
    shell.status
  );
  assert!(!target.exists(), "a refused program writes no file");
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn generating_gcode_to_an_empty_path_writes_nothing() {
  let directory = scratch("post-emptypath");
  let post = write_post(&directory, "minimal.pp", MINIMAL_POST);
  let mut app = AppState::new(document_with_toolpath(1, 4));
  let mut shell = Shell::new();
  shell.set_tools(vec![stored_tool(4)]);
  shell.apply_action(&mut app, &UiAction::ChoosePost { path: post });

  shell.apply_action(
    &mut app,
    &UiAction::GenerateGcode {
      path: Default::default(),
    },
  );

  assert!(
    shell.status.starts_with("refused:"),
    "an empty path names no file: {}",
    shell.status
  );
  assert!(
    shell.status.contains("empty"),
    "the refusal must say why: {}",
    shell.status
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_bare_handle_action_refuses_a_program_because_the_window_holds_the_post_processor() {
  // The post-processor is window state, exactly like the archive entries a save refuses to write without: a
  // handler with no window has no dialect to write in, and picking one for the person is what M13 S2 forbids.
  let directory = scratch("post-bare");
  let mut app = AppState::new(document_with_toolpath(1, 4));
  let target = directory.join("part.nc");

  let error = handle_action(
    &mut app,
    &UiAction::GenerateGcode {
      path: target.clone(),
    },
  )
  .expect_err("a bare handler holds no post-processor");
  assert!(
    error.contains("post-processor"),
    "the refusal must name what is missing: {error}"
  );
  assert!(!target.exists(), "a refused program writes no file");

  let error = handle_action(
    &mut app,
    &UiAction::ChoosePost {
      path: write_post(&directory, "minimal.pp", MINIMAL_POST),
    },
  )
  .expect_err("a bare handler has nowhere to hold a post-processor");
  assert!(error.contains("post-processor"));

  // The two intents ask the host for something and are never refusals.
  handle_action(&mut app, &UiAction::RequestGenerateGcode).expect("an intent is applied trivially");
  handle_action(&mut app, &UiAction::RequestChoosePost).expect("an intent is applied trivially");

  std::fs::remove_dir_all(&directory).ok();
}
