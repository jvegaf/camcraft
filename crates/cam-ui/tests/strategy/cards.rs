use super::support::*;
use cam_app::AppState;
use cam_model::{Operation, PocketStrategy};
use cam_ui::{Shell, UiAction};

// ---------------------------------------------------------------------------------------------
// A choice that needs a picture
// ---------------------------------------------------------------------------------------------

#[test]
fn a_choice_that_needs_a_picture_is_a_card_with_a_picture_and_the_choice_reaches_the_action() {
  // S10's last sentence. The card carries a diagram painted from the operation's own semantics — never an empty
  // box and never a screenshot of the reference app — and pressing it changes the strategy the footer writes.
  let ctx = egui::Context::default();
  let mut app = AppState::new(rectangle_document());
  app.select(vec!["E1".to_owned()]);
  let mut shell = Shell::new();
  shell.set_tools(vec![flat_mill()]);
  let _ = editor_frame(&ctx, &mut shell, &mut app);
  shell.strategy.tool_number = Some(2);

  let (drawn, _) = editor_frame(&ctx, &mut shell, &mut app);
  let words: Vec<(String, bool)> = drawn
    .drawn
    .strategy_cards
    .iter()
    .map(|(word, chosen, _)| (word.clone(), *chosen))
    .collect();
  assert_eq!(
    words,
    vec![("Profile".to_owned(), true), ("Pocket".to_owned(), false)],
    "the draft starts as a profile and the card says so"
  );
  for (word, _, strokes) in &drawn.drawn.strategy_cards {
    assert!(
      *strokes >= 2,
      "the {word:?} card must carry a picture, and an empty box is what L23 taught us to check for: {strokes} \
       strokes"
    );
  }

  // Pressing `Pocket` chooses it in the same frame: the cards, the sections and the footer all follow.
  let chosen = click_word(&ctx, &mut shell, &mut app, &drawn, "Pocket");
  assert!(
    chosen
      .drawn
      .strategy_cards
      .iter()
      .any(|(word, picked, _)| word == "Pocket" && *picked),
    "the frame that presses the card is the frame that reports it chosen: {:?}",
    chosen.drawn.strategy_cards
  );
  assert!(
    chosen
      .drawn
      .strategy_fields
      .iter()
      .any(|(label, _)| label == "Stepover"),
    "a pocket's section holds the numbers a pocket has: {:?}",
    chosen.drawn.strategy_fields
  );
  // The clearing pattern is the second choice shaped by a picture, and it appears with the pocket.
  assert!(
    chosen
      .drawn
      .strategy_cards
      .iter()
      .any(|(word, picked, strokes)| word == "Offset" && *picked && *strokes >= 2),
    "a pocket's clearing pattern is drawn as cards too: {:?}",
    chosen.drawn.strategy_cards
  );
  // The frame that applies a choice reports the layout it drew, which is the one before the choice grew the
  // sections above it. A second press reads the frame the operator would be looking at.
  let _ = frame(&ctx, &mut shell, &mut app);
  let (settled, _) = frame(&ctx, &mut shell, &mut app);
  let raster = click_word(&ctx, &mut shell, &mut app, &settled, "Raster");
  assert!(
    raster
      .drawn
      .strategy_cards
      .iter()
      .any(|(word, picked, _)| word == "Raster" && *picked),
    "pressing the raster card chooses it: {:?}",
    raster.drawn.strategy_cards
  );

  let pressed = click_word(&ctx, &mut shell, &mut app, &raster, "Generate toolpath");
  match &pressed.actions[0] {
    UiAction::AddToolpath { operation, .. } => match operation {
      Operation::Pocket(params) => {
        assert_eq!(params.strategy, PocketStrategy::Raster);
      }
      other => panic!("the card chose a pocket, got {other:?}"),
    },
    other => panic!("expected a toolpath to be added, got {other:?}"),
  }
}
