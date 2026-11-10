use cam_app::AppState;
use cam_ui::Shell;

use super::support::*;

// ---------------------------------------------------------------------------------------------
// S13: no fabricated library — an empty one says it is empty
// ---------------------------------------------------------------------------------------------

#[test]
fn an_empty_library_lists_nothing_and_says_so() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(Vec::new());
  shell.open_tool_library(&app);
  let _ = frame_with(&ctx, &mut shell, &mut app);
  let (output, texts) = frame_with(&ctx, &mut shell, &mut app);

  // Not one tool is invented: the modal lists the library it was given, which is none.
  assert!(
    output.drawn.tool_rows.is_empty(),
    "the modal invented tools out of an empty library: {:?}",
    output.drawn.tool_rows
  );
  assert!(output.drawn.tool_families.is_empty());
  assert_eq!(output.drawn.tool_selected, None);
  assert_eq!(output.drawn.tool_header.as_deref(), Some("Choose tool"));

  // And it says so, in the sentence it is given rather than in a blank column.
  let sentence = "The library holds no tool, so there is nothing to list or to use.";
  assert!(
    output
      .drawn
      .tool_library
      .iter()
      .any(|word| word == sentence),
    "an empty library said nothing: {:?}",
    output.drawn.tool_library
  );
  assert!(
    joined(&texts).contains(sentence),
    "the sentence was reported and never painted: {}",
    joined(&texts)
  );

  // There is nothing to describe and nothing to use, so neither the preview nor the commit pair is drawn.
  assert_eq!(output.drawn.tool_drawing, None);
  assert_eq!(output.drawn.tool_preview_caption, None);
  assert!(output.drawn.tool_groups.is_empty());
  assert!(
    !hits(&output).contains(&"Use this tool"),
    "an empty library offered to use a tool: {:?}",
    hits(&output)
  );
  // The way out is not one of the things that disappears.
  assert!(hits(&output).contains(&"Cancel"));
  assert!(hits(&output).contains(&"Close"));
}
