use cam_app::AppState;
use cam_ui::Shell;

use super::support::*;

// ---------------------------------------------------------------------------------------------
// The master side: the opener, the search field and the tree
// ---------------------------------------------------------------------------------------------

#[test]
fn the_cards_own_row_opens_the_library_on_the_tool_the_toolpath_cuts_with() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());

  let opened = open_library(&ctx, &mut shell, &mut app);

  assert!(
    shell.show_tool_library,
    "the card offered a row that did not open the library"
  );
  // The selection is the tool the toolpath already cuts with: the first thing an operator wants from
  // `Choose another tool…` is to see what the tool is now.
  assert_eq!(opened.drawn.tool_selected.as_deref(), Some("G2"));
  let words = opened.drawn.tool_library.join(" | ");
  assert!(
    words.contains("Choose tool — T2 - Tool (6mm)"),
    "the dialog's title does not name the chosen tool: {words}"
  );
  // Every tool in the library is a row, and the commit pair is there because this state can be honoured.
  assert_eq!(
    opened.drawn.tool_rows.len(),
    4,
    "{:?}",
    opened.drawn.tool_rows
  );
  for word in ["Cancel", "Use this tool"] {
    assert!(
      opened
        .drawn
        .tool_library_hits
        .iter()
        .any(|(name, _)| name == word),
      "nothing to press called {word:?}: {:?}",
      opened.drawn.tool_library_hits
    );
  }
}

#[test]
fn the_tree_groups_the_library_by_family_and_counts_what_it_drew() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());

  let opened = open_library(&ctx, &mut shell, &mut app);

  assert_eq!(
    opened.drawn.tool_families,
    vec![
      ("Flat end mill".to_owned(), 2),
      ("Ball nose".to_owned(), 1),
      ("V-bit".to_owned(), 1),
    ],
    "the tree must group by family, in one order, and skip a family with no tool"
  );
  // A tree is a claim about *which* rows sit together, which a list of counts cannot make: the two flat mills
  // are the first two rows and they say so.
  let families: Vec<&str> = opened
    .drawn
    .tool_rows
    .iter()
    .map(|(family, _, _)| family.as_str())
    .collect();
  assert_eq!(
    families,
    vec!["Flat end mill", "Flat end mill", "Ball nose", "V-bit"]
  );
  let words = opened.drawn.tool_library.join(" | ");
  for heading in [
    "Flat end mill (2)",
    "Ball nose (1)",
    "V-bit (1)",
    "Ø6 · 2 cut(s)",
  ] {
    assert!(
      words.contains(heading),
      "the tree never drew {heading:?}: {words}"
    );
  }
}

#[test]
fn the_search_field_filters_the_tree_and_the_counts_name_what_is_left() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  let _ = open_library(&ctx, &mut shell, &mut app);

  // A family's own word: an operator who reads `Ball nose (1)` and types `ball` is asking for its rows.
  shell.tool_search = "ball".to_owned();
  let (filtered, _) = frame_with(&ctx, &mut shell, &mut app);
  assert_eq!(
    filtered.drawn.tool_rows.len(),
    1,
    "the search field filtered nothing: {:?}",
    filtered.drawn.tool_rows
  );
  assert_eq!(filtered.drawn.tool_selected.as_deref(), Some("G2"));
  // The count is the number of rows drawn, not the number the library holds: a heading that said `(2)` over
  // one visible row would be a number nobody can check against the screen.
  assert_eq!(
    filtered.drawn.tool_families,
    vec![("Ball nose".to_owned(), 1)]
  );

  // A diameter on the cut line, which is how a machinist looks a tool up.
  shell.tool_search = "Ø12.7".to_owned();
  let (by_width, _) = frame_with(&ctx, &mut shell, &mut app);
  assert_eq!(
    by_width.drawn.tool_rows,
    vec![(
      "V-bit".to_owned(),
      "T3 - Tool (12.7mm)".to_owned(),
      "Ø12.7 · 2 cut(s)".to_owned()
    )],
    "the search never looked at the row's own cut line"
  );

  // And a search that matches nothing says so instead of drawing an empty tree.
  shell.tool_search = "spiral flute".to_owned();
  let (none, texts) = frame_with(&ctx, &mut shell, &mut app);
  assert!(none.drawn.tool_rows.is_empty());
  assert!(
    none
      .drawn
      .tool_library
      .iter()
      .any(|word| word.contains("No tool matches")),
    "an empty tree said nothing: {:?}",
    none.drawn.tool_library
  );
  assert!(joined(&texts).contains("No tool matches"));

  // The chip the field grows while it holds text is how the filter is cleared — a filter with no way out but
  // selecting the text is a trap.
  assert!(
    none
      .drawn
      .tool_library_hits
      .iter()
      .any(|(name, _)| name == "Clear"),
    "the search field offers no way back: {:?}",
    none.drawn.tool_library_hits
  );
  let (cleared, _) = click_word(&ctx, &mut shell, &mut app, &none, "Clear");
  assert_eq!(shell.tool_search, "");
  assert_eq!(cleared.drawn.tool_rows.len(), 4);
}
