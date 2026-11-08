use super::support::*;
use cam_ui::view3d::{clock_text, seconds_for};

#[test]
fn a_split_with_no_program_draws_no_overlay_row_and_says_why() {
  let mut harness = Harness::bare();
  harness.shell.set_view_3d(true);
  harness.frame(Vec::new());

  assert!(
    harness.output.drawn.split.is_some(),
    "the split is a mode, so it opens with or without a program"
  );
  assert!(
    harness.output.drawn.layer_chips.is_empty(),
    "no overlay row: a checkbox over nothing is a control that lies"
  );
  assert!(
    harness.output.drawn.playback_hits.is_empty()
      && harness.output.drawn.playback_readout.is_empty(),
    "and no playback bar, for the same reason"
  );
  assert!(
    harness
      .texts
      .iter()
      .any(|text| text.contains("half draws")),
    "the half says what it is for (M13 S3: it never sends the person to another program), painted: {:?}",
    harness.texts
  );
  assert!(
    !harness.texts.iter().any(|text| text.contains("cam-cli")),
    "and it names no other program: {:?}",
    harness.texts
  );
}

// ---- The 3D half simulates the drawing's own program (M13 S3) ------------------------------------

/// The point the empty half drew its `Simulate` chip at, which is the only way a test can press it.
fn simulate_chip(harness: &Harness) -> (f32, f32) {
  harness
    .output
    .drawn
    .simulate_chip
    .expect("the empty half must offer `Simulate`, and report where it drew it")
}

#[test]
fn the_empty_half_offers_simulate_and_pressing_it_draws_the_drawings_own_program() {
  let mut harness = Harness::with_document(document_with_toolpath());
  harness.shell.set_tools(vec![stored_tool()]);
  harness.shell.set_view_3d(true);
  harness.frame(Vec::new());

  assert!(
    harness.shell.preview().is_none(),
    "nothing has been simulated yet, so the half is empty"
  );
  assert_eq!(
    harness.output.drawn.painted_triangles, 0,
    "an empty half paints no picture"
  );
  assert!(
    harness.texts.iter().any(|text| text == "Simulate"),
    "the empty half carries the control that fills it: {:?}",
    harness.texts
  );
  // M13 S3: nothing in the shell may send the person out of it. The relief task's own sentence is chrome.rs'
  // (`chrome.rs:911`, outside this task's surface) and is drawn only while that task is open, which this frame
  // is not — what is asserted is that the half itself names no other program.
  assert!(
    !harness.texts.iter().any(|text| text.contains("cam-cli")),
    "no sentence in this frame sends the person to a terminal: {:?}",
    harness.texts
  );

  let chip = simulate_chip(&harness);
  harness.click(chip);

  assert!(
    harness.shell.preview().is_some(),
    "pressing `Simulate` turns the drawing's own toolpaths into the preview"
  );
  assert!(
    harness.shell.view_3d(),
    "and the picture is the view the person is looking at"
  );
  let expected = format!(
    "simulate — {} of cutting, drawn in 3D",
    clock_text(seconds_for(&document_program()))
  );
  assert_eq!(
    harness.shell.status, expected,
    "the status bar names what was simulated, in the clock's own words"
  );

  harness.frame(Vec::new());
  assert!(
    harness.output.drawn.painted_triangles > 0,
    "the frame after the press draws the document's own program cutting its stock"
  );
}

#[test]
fn simulating_a_drawing_whose_tools_are_not_in_the_shell_refuses_by_name_and_draws_nothing() {
  let mut harness = Harness::with_document(document_with_toolpath());
  // No `set_tools`: the library this window was given is empty, so the toolpath's tool cannot be resolved.
  harness.shell.set_view_3d(true);
  harness.frame(Vec::new());

  let chip = simulate_chip(&harness);
  harness.click(chip);

  assert!(
    harness.shell.status.starts_with("refused:"),
    "a missing tool is a refusal, not a picture: {}",
    harness.shell.status
  );
  assert!(
    harness.shell.status.contains(&format!("T{TOOL_NUMBER}")),
    "the refusal must name the missing tool: {}",
    harness.shell.status
  );
  assert!(
    harness.shell.preview().is_none(),
    "a refused simulation hands the half no preview"
  );

  harness.frame(Vec::new());
  assert_eq!(
    harness.output.drawn.painted_triangles, 0,
    "and the half stays empty: a refusal draws nothing"
  );
  assert!(
    harness.output.drawn.playback_hits.is_empty() && harness.output.drawn.layer_chips.is_empty(),
    "no playback bar and no overlay row over a picture that does not exist"
  );
}

#[test]
fn simulating_a_drawing_with_no_toolpaths_refuses_by_name_and_draws_nothing() {
  let mut harness = Harness::with_document(document());
  harness.shell.set_tools(vec![stored_tool()]);
  harness.shell.set_view_3d(true);
  harness.frame(Vec::new());

  let chip = simulate_chip(&harness);
  harness.click(chip);

  assert!(
    harness.shell.status.starts_with("refused:"),
    "a drawing with no toolpaths has nothing to simulate: {}",
    harness.shell.status
  );
  assert!(
    harness.shell.status.contains("toolpaths"),
    "the refusal must name the missing half: {}",
    harness.shell.status
  );
  assert!(
    harness.shell.preview().is_none(),
    "a refused simulation hands the half no preview"
  );

  harness.frame(Vec::new());
  assert_eq!(
    harness.output.drawn.painted_triangles, 0,
    "and the half stays empty: a refusal draws nothing"
  );
}

#[test]
fn simulate_again_rebuilds_the_picture_from_the_document() {
  let mut harness = Harness::with_document(document_with_toolpath());
  harness.shell.set_tools(vec![stored_tool()]);
  // A picture somebody else produced: the playback fixture, whose clock is its own and not the drawing's.
  harness
    .shell
    .set_preview(untouched(), playback_program(), &job());
  harness.frame(Vec::new());

  let old = harness.preview().duration();
  let drawing = seconds_for(&document_program());
  assert!(
    (old - drawing).abs() > 1.0e-9,
    "the fixture and the drawing must be tellable apart by their clocks: {old} against {drawing}"
  );

  // Play the handed-in program out, so "back at the start" is a fact about the rebuild and not about a
  // playhead that never moved.
  harness.click(hit(&harness.output.drawn.playback_hits, "To the end"));
  assert!(
    (harness.preview().elapsed() - old).abs() < 1.0e-6,
    "the old program has played out: {} of {old}",
    harness.preview().elapsed()
  );

  harness.click(hit(&harness.output.drawn.layer_chips, "Simulate again"));

  assert!(
    harness.shell.status.starts_with("simulate"),
    "the row's press reports itself in the status bar: {}",
    harness.shell.status
  );
  assert!(
    harness.preview().elapsed().abs() < 1.0e-9,
    "the rebuilt picture starts at the beginning of its own program: {}",
    harness.preview().elapsed()
  );
  assert!(
    (harness.preview().duration() - drawing).abs() < 1.0e-9,
    "and it lasts the drawing's own program, not the one that was handed in: {} against {drawing}",
    harness.preview().duration()
  );
  assert_eq!(
    harness.shell.status,
    format!("simulate — {} of cutting, drawn in 3D", clock_text(drawing)),
    "the note names the duration the rebuilt playback bar names"
  );
}
