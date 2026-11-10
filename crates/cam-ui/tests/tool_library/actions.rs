use cam_app::AppState;
use cam_ui::Shell;

use super::support::*;

// ---------------------------------------------------------------------------------------------
// S3: the library action rows — deliberately absent until the slice that can honour them
// ---------------------------------------------------------------------------------------------

#[test]
fn no_library_action_row_is_drawn_because_this_slice_cannot_honour_one() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  let opened = open_library(&ctx, &mut shell, &mut app);

  // S3 asks the master column to end in three rows of buttons — `+ New`, `Duplicate`, `Delete`; `Export…`,
  // `Import…`; `Restore defaults`. S10 and S11 are the slices that make them write, so drawing them now would
  // be six controls that emit nothing, which is L14's defect six times over. The report says so out loud, and
  // this test is written to fail the day the rows arrive: that is the deliberate change, not a stray one.
  assert!(
    opened.drawn.tool_actions.is_empty(),
    "the master column drew library action rows this slice cannot honour: {:?}",
    opened.drawn.tool_actions
  );
  let pressable = hits(&opened);
  let painted = opened.drawn.tool_library.join(" | ");
  for absent in [
    "New",
    "Duplicate",
    "Delete",
    "Export…",
    "Import…",
    "Restore defaults",
  ] {
    assert!(
      !pressable.iter().any(|word| word.contains(absent)),
      "{absent:?} is pressable, which means this slice claims a write it cannot do: {pressable:?}"
    );
    assert!(
      !painted.contains(absent),
      "{absent:?} was drawn, so the master column claims an action that is not there: {painted}"
    );
  }

  // And the words are not merely unreported: the frame painted none of them (L23 — a control nobody can press
  // is only honest when it is not on the screen either).
  let (_, texts) = frame_with(&ctx, &mut shell, &mut app);
  let frame = joined(&texts);
  for absent in ["Duplicate", "Restore defaults", "Export…", "Import…"] {
    assert!(
      !frame.contains(absent),
      "{absent:?} was painted by the frame: {frame}"
    );
  }
}
