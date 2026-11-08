use cam_app::AppState;
use cam_ui::chrome::Category;
use cam_ui::Shell;

use super::support::*;

// ---------------------------------------------------------------------------------------------
// Preserved: the card the opener lives on
// ---------------------------------------------------------------------------------------------

#[test]
fn the_tool_card_still_reads_out_its_own_three_lines_while_it_offers_the_opener() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  shell.open_task(Category::Toolpaths);

  let (output, _) = frame_with(&ctx, &mut shell, &mut app);

  // The card's fields are exactly the ones the reference frame shows (camui-toolpath S2): the control that
  // opens the library is a row of its own and did not become a fourth field.
  assert_eq!(
    output.drawn.tool_card,
    vec![
      "TOOL — Perfil 01".to_owned(),
      "T2 - Tool (6mm)".to_owned(),
      "code 0 · Ø 6.00 mm · — · 2 flutes".to_owned(),
    ]
  );
  assert!(
    output
      .drawn
      .tool_card_hits
      .iter()
      .any(|(word, _)| word == "Choose another tool…"),
    "the card no longer offers the way to change its tool: {:?}",
    output.drawn.tool_card_hits
  );
}
