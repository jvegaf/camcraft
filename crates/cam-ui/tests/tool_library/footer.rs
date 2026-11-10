use cam_app::AppState;
use cam_ui::Shell;

use super::support::*;

// ---------------------------------------------------------------------------------------------
// S8: the footer — one sentence, and the pair under it
// ---------------------------------------------------------------------------------------------

#[test]
fn the_footer_paints_its_sentence_and_puts_cancel_before_use_this_tool() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  let opened = open_library(&ctx, &mut shell, &mut app);
  let (_, texts) = frame_with(&ctx, &mut shell, &mut app);

  let sentence = "Changes apply to new toolpaths; each toolpath keeps its own copy of the tool.";
  assert!(
    opened
      .drawn
      .tool_library
      .iter()
      .any(|word| word == sentence),
    "the footer's sentence is not the one S8 writes down: {:?}",
    opened.drawn.tool_library
  );
  assert!(
    joined(&texts).contains(sentence),
    "the sentence was reported and never painted: {}",
    joined(&texts)
  );

  // The row is laid out right to left, so the order the hits were reported in *is* the order on the screen:
  // `Cancel` first, then the primary `Use this tool`.
  let cancel = hit_position(&opened, "Cancel").expect("the footer drew no Cancel");
  let use_it = hit_position(&opened, "Use this tool").expect("the footer drew no Use this tool");
  assert!(
    cancel < use_it,
    "`Cancel` must be drawn before `Use this tool`: {:?}",
    hits(&opened)
  );
  // And `Cancel` really is the rightmost of the pair, which is what right-aligned means.
  let x = |word: &str| {
    opened
      .drawn
      .tool_library_hits
      .iter()
      .find(|(name, _)| name == word)
      .map(|(_, point)| point.0)
      .expect("the word the previous assertion found")
  };
  assert!(
    x("Cancel") > x("Use this tool"),
    "`Cancel` is drawn left of `Use this tool`, so the pair is not right-aligned"
  );
}

#[test]
fn a_tool_with_no_number_draws_no_use_this_tool_and_says_why() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  // A tool the library holds without the number a program names it by: the modal cannot honour
  // `Use this tool`, so it must not draw it.
  shell.set_tools(vec![
    flat_mill("G9", None, 4.0),
    flat_mill("G1", Some(1), 6.0),
  ]);
  shell.open_tool_library(&app);
  let _ = frame_with(&ctx, &mut shell, &mut app);
  let (drawn, _) = frame_with(&ctx, &mut shell, &mut app);
  let (selected, _) = click_word(&ctx, &mut shell, &mut app, &drawn, "Flat end mill (4mm)");
  assert_eq!(selected.drawn.tool_selected.as_deref(), Some("G9"));
  // One quiet frame: choosing a row gives the detail column a form where it had one sentence, and egui lays the
  // taller modal out on the frame after the press that grew it.
  let (settled, _) = frame_with(&ctx, &mut shell, &mut app);
  assert!(
    !hits(&settled).contains(&"Use this tool"),
    "the modal drew a `Use this tool` it could not honour: {:?}",
    hits(&settled)
  );
  let reason = settled
    .drawn
    .tool_library
    .iter()
    .find(|word| word.contains("carries no tool number"))
    .unwrap_or_else(|| {
      panic!(
        "the modal did not say why it cannot use this tool: {:?}",
        settled.drawn.tool_library
      )
    })
    .clone();
  assert!(
    reason.contains("Flat end mill (4mm)"),
    "the sentence must name the tool it refuses: {reason:?}"
  );
  // `Cancel` still works: the way out is never the thing that disappears.
  let (left, _) = click_word(&ctx, &mut shell, &mut app, &settled, "Cancel");
  assert!(left.actions.is_empty());
  assert!(!shell.show_tool_library);
}

#[test]
fn a_library_opened_without_a_toolpath_lists_the_tools_and_draws_no_use_this_tool() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  // A host that opens the library with nothing to retool: the tree is still worth showing — reading the
  // library is a reason of its own — but `Use this tool` has no document fact to write, so it is not drawn.
  shell.show_tool_library = true;

  let _ = frame_with(&ctx, &mut shell, &mut app);
  let (output, _) = frame_with(&ctx, &mut shell, &mut app);

  assert_eq!(
    output.drawn.tool_rows.len(),
    6,
    "the tree is not the thing that depends on a target"
  );
  assert!(
    !hits(&output).contains(&"Use this tool"),
    "a library with no toolpath to retool offered `Use this tool`: {:?}",
    hits(&output)
  );
  assert!(output
    .drawn
    .tool_library
    .iter()
    .any(|word| word.contains("without a toolpath")));
  assert!(shell.tool_library_target().is_none());
}

#[test]
fn escape_leaves_the_library_without_writing_and_the_modal_owns_the_pointer() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  let _ = open_library(&ctx, &mut shell, &mut app);

  // A modal owns the pointer: the canvas under the backdrop reports no world position and finds no snap
  // target, because a status bar naming a point nobody can aim at would be describing a canvas that is not
  // being aimed at.
  let over_canvas = egui::pos2(600.0, 400.0);
  let mut input = raw_input();
  input.events.push(egui::Event::PointerMoved(over_canvas));
  let mut captured: Option<cam_ui::ShellOutput> = None;
  let full = ctx.run_pass(input, |ctx| {
    captured = Some(shell.show(&mut app, ctx));
  });
  full.drop_without_applying_deltas();
  let drawn = captured.expect("the shell ran");
  assert!(
    shell.cursor_world.is_none(),
    "the canvas tracked the pointer under an open modal: {:?}",
    shell.cursor_world
  );
  assert!(drawn
    .drawn
    .tool_library
    .iter()
    .any(|word| word == "Choose tool — T2 - Ball nose (6mm)"));

  let (left, _) = press(&ctx, &mut shell, &mut app, egui::Key::Escape);
  assert!(
    left.actions.is_empty(),
    "`Esc` wrote something: {:?}",
    left.actions
  );
  assert!(!shell.show_tool_library);
  assert_eq!(app.document().toolpaths[0].tool_number, 2);
}
