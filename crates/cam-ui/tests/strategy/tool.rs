use super::support::*;
use cam_app::AppState;
use cam_ui::Shell;

// ---------------------------------------------------------------------------------------------
// The tool: the card's opener, and the library that fills the draft
// ---------------------------------------------------------------------------------------------

#[test]
fn choosing_a_tool_for_a_new_strategy_writes_the_draft_and_generate_writes_the_document() {
  // The card's opener is the same row S9 built. What changes is the subject: with no toolpath on screen the
  // choice is the draft's tool, and `Generate toolpath` is what writes it.
  let ctx = egui::Context::default();
  let mut app = AppState::new(rectangle_document());
  app.select(vec!["E1".to_owned()]);
  let mut shell = Shell::new();
  shell.set_tools(vec![flat_mill()]);

  let (drawn, _) = editor_frame(&ctx, &mut shell, &mut app);
  assert!(
    drawn
      .drawn
      .tool_card
      .iter()
      .any(|line| line.starts_with("No tool chosen yet")),
    "a card with no tool says so instead of drawing nothing: {:?}",
    drawn.drawn.tool_card
  );
  assert!(
    !controls(&drawn)
      .iter()
      .any(|word| word == "Generate toolpath"),
    "and the footer says why Generate is absent"
  );

  let at = point_of(&drawn.drawn.tool_card_hits, "Choose another tool…");
  let (_opened, _) = click_at(&ctx, &mut shell, &mut app, at);
  assert!(shell.show_tool_library, "the opener must open the library");
  // The rows take a pass to settle, the way every modal in this shell does.
  let _ = frame(&ctx, &mut shell, &mut app);
  let mut latest = frame(&ctx, &mut shell, &mut app).0;
  let row = latest
    .drawn
    .tool_library_hits
    .iter()
    .find(|(word, _)| word.starts_with("T2"))
    .map(|(_, point)| egui::pos2(point.0, point.1))
    .unwrap_or_else(|| {
      panic!(
        "the library drew no T2 row: {:?}",
        latest.drawn.tool_library_hits
      )
    });
  let _ = click_at(&ctx, &mut shell, &mut app, row);
  latest = frame(&ctx, &mut shell, &mut app).0;
  assert!(
    latest
      .drawn
      .tool_library_hits
      .iter()
      .any(|(word, _)| word == "Use this tool"),
    "a draft with a chosen tool can use it: {:?}",
    latest.drawn.tool_library_hits
  );
  let (committed, _) = click_at(
    &ctx,
    &mut shell,
    &mut app,
    point_of(&latest.drawn.tool_library_hits, "Use this tool"),
  );
  assert_eq!(
    shell.strategy.tool_number,
    Some(2),
    "`Use this tool` writes the draft's tool"
  );
  assert!(
    committed.actions.is_empty(),
    "and writes no document: Generate is what writes it: {:?}",
    committed.actions
  );
  assert_eq!(
    app.document().toolpaths.len(),
    0,
    "choosing a tool for a draft must not create a toolpath"
  );

  let (drawn, _) = editor_frame(&ctx, &mut shell, &mut app);
  assert!(
    drawn
      .drawn
      .tool_card
      .iter()
      .any(|line| line.starts_with("T2 - ")),
    "the card reads the chosen tool back: {:?}",
    drawn.drawn.tool_card
  );
  assert!(
    controls(&drawn)
      .iter()
      .any(|word| word == "Generate toolpath"),
    "with a tool and a region, the footer is back: {:?}",
    drawn.drawn.strategy_hits
  );
}
