use super::support::*;
use cam_app::AppState;
use cam_model::{Operation, PocketParams};
use cam_ui::Shell;
use cam_units::Length;

// ---------------------------------------------------------------------------------------------
// The quick parameter edit
// ---------------------------------------------------------------------------------------------

#[test]
fn the_quick_edit_duplicates_the_sections_numbers_and_folds_on_its_own() {
  // The reference's `Edição rápida dos parâmetros` repeats the numbers you actually change above the full
  // section. It repeats the *values*: one number, two widgets, never two copies.
  let ctx = egui::Context::default();
  let mut app = AppState::new(pocket_document());
  let mut shell = Shell::new();
  shell.set_tools(vec![flat_mill()]);
  let _ = editor_frame(&ctx, &mut shell, &mut app);
  shell.strategy.operation = Operation::Pocket(PocketParams {
    pass_depth: Length::from_mm(1.25),
    stepover: Length::from_mm(2.5),
    ..PocketParams::default()
  });

  let (drawn, _) = editor_frame(&ctx, &mut shell, &mut app);
  assert!(drawn.drawn.strategy_quick_open, "the block starts expanded");
  let quick = drawn.drawn.strategy_quick_fields.clone();
  assert_eq!(
    quick,
    vec![
      ("Pass depth".to_owned(), "1.25 mm".to_owned()),
      ("Stepover".to_owned(), "2.50 mm".to_owned()),
    ],
    "the quick block holds the numbers the operator changes most"
  );
  for (label, value) in &quick {
    assert!(
      drawn
        .drawn
        .strategy_fields
        .iter()
        .any(|(section_label, section_value)| section_label == label && section_value == value),
      "the quick block's {label:?} must be the section's own number, got {value:?} against {:?}",
      drawn.drawn.strategy_fields
    );
  }

  // Folding it draws no row and leaves the section exactly where it was.
  let folded = click_word(&ctx, &mut shell, &mut app, &drawn, "Quick parameter edit");
  assert!(
    !folded.drawn.strategy_quick_open,
    "the heading folds the block"
  );
  assert!(
    folded.drawn.strategy_quick_fields.is_empty(),
    "a folded block draws no number: {:?}",
    folded.drawn.strategy_quick_fields
  );
  assert_eq!(
    folded.drawn.strategy_fields.len(),
    drawn.drawn.strategy_fields.len(),
    "the section behind it keeps every number"
  );

  // Unfolding brings the same numbers back, and the tool's own rates are readings with their units that say
  // where they live: the library owns them.
  let unfolded = click_word(&ctx, &mut shell, &mut app, &folded, "Quick parameter edit");
  assert!(
    unfolded.drawn.strategy_quick_open,
    "pressing the heading again expands it"
  );
  let _ = frame(&ctx, &mut shell, &mut app);
  let (_again, painted) = frame(&ctx, &mut shell, &mut app);
  assert_eq!(
    _again.drawn.strategy_quick_fields, quick,
    "and the same numbers come back"
  );
  for word in ["Feed", "100 mm/min", "Plunge", "30 mm/min", "Spindle"] {
    assert!(
      painted.iter().any(|text| text == word),
      "the quick block must show the tool's {word:?}: {painted:?}"
    );
  }
  assert!(
    painted
      .iter()
      .any(|text| text.starts_with("Feed, plunge and spindle")),
    "and it must say where they are changed: {painted:?}"
  );
}
