use cam_app::AppState;
use cam_ui::{handle_action, Shell, ShellOutput, UiAction};

use super::support::*;

// ---------------------------------------------------------------------------------------------
// The footer, and the two ways out
// ---------------------------------------------------------------------------------------------

#[test]
fn the_footer_offers_only_the_library_actions_this_shell_can_perform() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  let opened = open_library(&ctx, &mut shell, &mut app);

  // The shell holds a read-only snapshot: `cam-store` owns the only writer. Five buttons that emitted nothing
  // would be L14's defect five times over, so what the footer draws is the sentence that says so.
  for absent in ["New", "Duplicate", "Delete", "Import…", "Export…"] {
    assert!(
      !opened
        .drawn
        .tool_library_hits
        .iter()
        .any(|(name, _)| name == absent),
      "the footer offered {absent:?}, a library action this window cannot perform: {:?}",
      opened.drawn.tool_library_hits
    );
  }
  let reason = opened
    .drawn
    .tool_library
    .iter()
    .find(|word| word.contains("read-only snapshot"))
    .unwrap_or_else(|| {
      panic!(
        "the footer does not say why the library actions are missing: {:?}",
        opened.drawn.tool_library
      )
    })
    .clone();
  assert!(
    reason.contains("cam-cli tools add"),
    "the sentence must name the route that does work: {reason:?}"
  );
  // And the pair S9 puts under the actions is there, in this order.
  let hits: Vec<&str> = opened
    .drawn
    .tool_library_hits
    .iter()
    .map(|(name, _)| name.as_str())
    .collect();
  let cancel = hits.iter().position(|word| *word == "Cancel");
  let use_it = hits.iter().position(|word| *word == "Use this tool");
  assert!(
    cancel.is_some() && use_it.is_some(),
    "the commit pair is missing: {hits:?}"
  );
  assert!(
    hits.contains(&"T1 - Tool (6mm)"),
    "the tree rows are not pressable: {hits:?}"
  );
}

#[test]
fn use_this_tool_retools_the_toolpath_and_closes_the_library() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  let opened = open_library(&ctx, &mut shell, &mut app);

  // Choose T1 and commit: the one thing this modal writes is the number the toolpath cuts with.
  let (chosen, _) = click_word(&ctx, &mut shell, &mut app, &opened, "T1 - Tool (6mm)");
  assert_eq!(chosen.drawn.tool_selected.as_deref(), Some("G1"));
  let (pressed, _) = click_word(&ctx, &mut shell, &mut app, &chosen, "Use this tool");
  assert_eq!(
    pressed.actions,
    vec![UiAction::SetToolpathToolNumber {
      toolpath_id: "P1".to_owned(),
      tool_number: 1,
    }],
    "`Use this tool` must emit exactly the tool it showed"
  );
  let report = handle_action(&mut app, &pressed.actions[0]).expect("the tool changes");
  assert_eq!(report.label, "set toolpath tool");
  assert_eq!(app.document().toolpaths[0].tool_number, 1);
  assert!(
    !shell.show_tool_library,
    "the library stayed open after committing, so the next press could commit twice"
  );
  let (after, _) = frame_with(&ctx, &mut shell, &mut app);
  assert!(
    after.drawn.tool_library.is_empty() && after.drawn.tool_library_hits.is_empty(),
    "a closed library still reported itself: {:?}",
    after.drawn.tool_library
  );
}

#[test]
fn cancel_leaves_the_toolpath_the_library_and_the_search_exactly_as_they_were() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  let _ = open_library(&ctx, &mut shell, &mut app);
  let before = app.document().clone();

  // Change the draft in both ways the modal offers, then leave.
  shell.tool_search = "flat".to_owned();
  let (filtered, _) = frame_with(&ctx, &mut shell, &mut app);
  let (chosen, _) = click_word(&ctx, &mut shell, &mut app, &filtered, "T1 - Tool (6mm)");
  let (left, _) = click_word(&ctx, &mut shell, &mut app, &chosen, "Cancel");

  assert!(
    left.actions.is_empty(),
    "`Cancel` wrote something: {:?}",
    left.actions
  );
  assert_eq!(
    app.document().toolpaths[0].tool_number,
    before.toolpaths[0].tool_number,
    "the toolpath was retooled by a cancelled choice"
  );
  assert_eq!(
    app.document(),
    &before,
    "the document changed under `Cancel`"
  );
  assert!(!shell.show_tool_library);
  // The drafts go too: a search left behind would filter the next visit for a reason nobody remembers.
  assert_eq!(shell.tool_search, "");
  assert!(shell.tool_library_target().is_none());

  // And a second visit starts where the document is, not where the cancelled choice was.
  let (again, _) = frame_with(&ctx, &mut shell, &mut app);
  assert!(again.drawn.tool_library.is_empty());
  let reopened = open_library(&ctx, &mut shell, &mut app);
  assert_eq!(reopened.drawn.tool_selected.as_deref(), Some("G2"));
}

#[test]
fn a_tool_with_no_number_draws_no_use_this_tool_and_says_why() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  // A tool the library holds without the number a program names it by: the modal cannot honour
  // `Use this tool`, so it must not draw it.
  shell.set_tools(vec![
    flat_mill("G9", None, 4.0),
    flat_mill("G1", Some(1), 6.0),
  ]);
  let (redrawn, _) = {
    shell.open_tool_library(&app);
    let _ = frame_with(&ctx, &mut shell, &mut app);
    frame_with(&ctx, &mut shell, &mut app)
  };
  let (selected, _) = click_word(&ctx, &mut shell, &mut app, &redrawn, "Tool (4mm)");
  assert_eq!(selected.drawn.tool_selected.as_deref(), Some("G9"));
  assert!(
    selected
      .drawn
      .tool_library_hits
      .iter()
      .all(|(name, _)| name != "Use this tool"),
    "the modal drew a `Use this tool` it could not honour: {:?}",
    selected.drawn.tool_library_hits
  );
  let reason = selected
    .drawn
    .tool_library
    .iter()
    .find(|word| word.contains("carries no tool number"))
    .unwrap_or_else(|| {
      panic!(
        "the modal did not say why it cannot use this tool: {:?}",
        selected.drawn.tool_library
      )
    })
    .clone();
  assert!(
    reason.contains("T"),
    "the sentence must name the tool it refuses: {reason:?}"
  );
  // `Cancel` still works: the way out is never the thing that disappears.
  let (left, _) = click_word(&ctx, &mut shell, &mut app, &selected, "Cancel");
  assert!(left.actions.is_empty());
  assert!(!shell.show_tool_library);
}

#[test]
fn an_empty_library_says_so_and_offers_nothing_to_use() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(Vec::new());
  let (output, _texts) = {
    shell.open_tool_library(&app);
    let _ = frame_with(&ctx, &mut shell, &mut app);
    frame_with(&ctx, &mut shell, &mut app)
  };

  assert!(!output.drawn.tool_rows.is_empty());
  let _ = click_word(&ctx, &mut shell, &mut app, &output, "Cancel");
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

  let (output, _) = frame_with(&ctx, &mut shell, &mut app);
  let _ = frame_with(&ctx, &mut shell, &mut app);

  assert_eq!(
    output.drawn.tool_rows.len(),
    4,
    "the tree is not the thing that depends on a target"
  );
  assert!(
    output
      .drawn
      .tool_library_hits
      .iter()
      .all(|(name, _)| name != "Use this tool"),
    "a library with no toolpath to retool offered `Use this tool`: {:?}",
    output.drawn.tool_library_hits
  );
  assert!(
    output
      .drawn
      .tool_library
      .iter()
      .any(|word| word.contains("without a toolpath")),
    "the modal did not say why it cannot use a tool: {:?}",
    output.drawn.tool_library
  );
  assert!(shell.tool_library_target().is_none());
}

#[test]
fn a_row_the_scroll_area_hides_is_not_reported_as_pressable() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(1));
  let mut shell = Shell::new();
  // More tools than the master column has room for: the ones below the fold are drawn by egui and clipped
  // away, and reporting their centres would be a promise about a control nobody can see — the defect the T6
  // review found in an invisible playback bar with live hit areas (L23).
  shell.set_tools(
    (1..=24)
      .map(|index| flat_mill(&format!("G{index}"), Some(index), 6.0))
      .collect(),
  );
  shell.open_tool_library(&app);
  let _ = frame_with(&ctx, &mut shell, &mut app);
  let (output, _) = frame_with(&ctx, &mut shell, &mut app);

  let rows: Vec<&str> = output
    .drawn
    .tool_rows
    .iter()
    .map(|(_, title, _)| title.as_str())
    .collect();
  assert_eq!(rows.len(), 24, "the tree lists every tool it was given");
  let pressable: Vec<&str> = output
    .drawn
    .tool_library_hits
    .iter()
    .map(|(name, _)| name.as_str())
    .filter(|name| rows.contains(name))
    .collect();
  assert!(
    pressable.len() < rows.len(),
    "every row is reported as pressable, so nothing was clipped: {pressable:?}"
  );
  // What it reports is a prefix of the tree — the rows it drew inside the fold, in the tree's own order — and
  // the last tool is not among them.
  assert_eq!(
    pressable,
    rows[..pressable.len()],
    "the reported rows are not the ones at the top of the tree"
  );
  assert!(
    !pressable.contains(&"T24 - Tool (6mm)"),
    "a row below the fold was reported as pressable"
  );
  // And the report is actionable: the last row it names really does select its tool.
  let last = *pressable.last().expect("some row is visible");
  let at = output
    .drawn
    .tool_library_hits
    .iter()
    .find(|(name, _)| name == last)
    .map(|(_, point)| egui::pos2(point.0, point.1))
    .expect("the point it reported");
  click_at(&ctx, &mut shell, &mut app, at);
  assert!(
    shell.tool_library_selection().is_some_and(|id| id != "G1"),
    "the last reported row did not select its tool: {:?}",
    shell.tool_library_selection()
  );
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
  let mut captured: Option<ShellOutput> = None;
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
    .any(|word| word == "Choose tool — T2 - Tool (6mm)"));

  let (left, _) = press(&ctx, &mut shell, &mut app, egui::Key::Escape);
  assert!(
    left.actions.is_empty(),
    "`Esc` wrote something: {:?}",
    left.actions
  );
  assert!(!shell.show_tool_library);
  assert_eq!(app.document().toolpaths[0].tool_number, 2);
}
