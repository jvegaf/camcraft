use cam_ui::tool_library::{cut_line, tool_title, ToolFamily};
use cam_units::Length;

use super::support::*;

// ---------------------------------------------------------------------------------------------
// The family the library's own geometry proves
// ---------------------------------------------------------------------------------------------

#[test]
fn the_family_is_the_shape_the_record_proves_not_the_raw_type_code() {
  // Every fixture below carries `tool_type: 0`, the same opaque code — so if the family came from that number
  // the tree would be one group. The shape is what separates them, and the code names nothing (cam-store says
  // so itself).
  assert_eq!(
    ToolFamily::of(&flat_mill("a", Some(1), 6.0).geometry),
    ToolFamily::Flat
  );
  assert_eq!(
    ToolFamily::of(&ball_nose("b", Some(2), 6.0).geometry),
    ToolFamily::BallNose
  );
  assert_eq!(
    ToolFamily::of(&corner_radius("c", Some(3), 12.0, 2.0).geometry),
    ToolFamily::CornerRadius
  );
  assert_eq!(
    ToolFamily::of(&v_bit("d", Some(4), 12.7, 90.0, 0.2).geometry),
    ToolFamily::VBit
  );
  // A V is decided by its angle even when it also carries a tip radius, and a record with no diameter has no
  // radius to prove a ball: both are readings of the rule, not of a list.
  let mut both = v_bit("e", Some(5), 12.7, 60.0, 0.0);
  both.geometry.tip_radius = Some(Length::from_mm(6.0));
  assert_eq!(ToolFamily::of(&both.geometry), ToolFamily::VBit);
  assert_eq!(
    ToolFamily::of(&base("f", Some(6), None).geometry),
    ToolFamily::Flat
  );
}

#[test]
fn a_row_names_the_tool_and_carries_its_own_diameter_and_cut_count() {
  // The reference's second line, in its own form: the diameter without a unit and the number of cuts. The
  // zeroes a reader does not need are trimmed, so a Ø12.7 tool does not read as `Ø12.700`.
  assert_eq!(cut_line(&flat_mill("G1", Some(1), 6.0)), "Ø6 · 2 cut(s)");
  assert_eq!(
    cut_line(&flat_mill("G5", Some(5), 3.175)),
    "Ø3.175 · 2 cut(s)"
  );
  assert_eq!(
    cut_line(&v_bit("G3", Some(3), 12.7, 90.0, 0.2)),
    "Ø12.7 · 2 cut(s)"
  );
  // A number the library does not hold is a dash: `Ø0 · 0 cut(s)` would be two numbers nobody wrote.
  let mut half_described = base("G9", None, None);
  half_described.geometry.num_flutes = None;
  assert_eq!(cut_line(&half_described), "Ø— · — cut(s)");
  // The title is the card's own spelling, so the row and the card name one tool one way.
  assert_eq!(
    tool_title(&flat_mill("G1", Some(1), 6.0)),
    "T1 - Tool (6mm)"
  );
  assert_eq!(tool_title(&base("G8", None, Some(6.0))), "Tool (6mm)");
}
