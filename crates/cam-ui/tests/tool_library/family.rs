use cam_store::tools::ToolKind;
use cam_ui::tool_library::{
  cut_line, tool_title, FormField, ShapeField, ToolFamily, ToolFormGroup,
};

use super::support::*;

// ---------------------------------------------------------------------------------------------
// The vocabulary: what a kind paints as, and which rows it decides
// ---------------------------------------------------------------------------------------------

#[test]
fn the_six_kinds_collapse_to_the_four_shapes_the_drawing_can_paint() {
  // One vocabulary is what the operator called the tool and what the library stores; the other is what can be
  // painted. `Engraving` ends flat like an end mill and `Drill` ends in a cone like a V-bit, so six kinds are
  // four shapes and the pairing is stated rather than discovered.
  assert_eq!(
    ToolKind::ALL.map(ToolFamily::of),
    [
      ToolFamily::Flat,
      ToolFamily::BallNose,
      ToolFamily::CornerRadius,
      ToolFamily::VBit,
      ToolFamily::Flat,
      ToolFamily::VBit,
    ]
  );
  // A pointed tip is the one shape property the form asks about, and the two kinds that have one are the two
  // the `Included angle` row belongs to.
  for kind in ToolKind::ALL {
    assert_eq!(
      kind.has_included_angle(),
      ShapeField::of(kind).contains(&ShapeField::IncludedAngle),
      "{} and its GEOMETRY rows disagree about whether its tip is pointed",
      kind.label()
    );
  }
}

#[test]
fn the_geometry_rows_are_the_kinds_own_in_s5s_order() {
  // A flat end: no angle, no radius, and S5's five rows in S5's order.
  assert_eq!(
    ShapeField::of(ToolKind::FlatEndMill),
    &[
      ShapeField::Diameter,
      ShapeField::FluteLength,
      ShapeField::ShankDiameter,
      ShapeField::OverallLength,
      ShapeField::Flutes,
    ]
  );
  assert_eq!(
    ShapeField::of(ToolKind::Engraving),
    ShapeField::of(ToolKind::FlatEndMill),
    "an engraving cutter is a flat end with a name"
  );
  // A pointed tip: the angle sits second, where S5 puts it.
  for pointed in [ToolKind::VBit, ToolKind::Drill] {
    assert_eq!(
      ShapeField::of(pointed),
      &[
        ShapeField::Diameter,
        ShapeField::IncludedAngle,
        ShapeField::FluteLength,
        ShapeField::ShankDiameter,
        ShapeField::OverallLength,
        ShapeField::Flutes,
      ]
    );
  }
  // A bull nose: the radius its name comes from, in the angle's own place (the deviation L5 records).
  assert_eq!(
    ShapeField::of(ToolKind::CornerRadius),
    &[
      ShapeField::Diameter,
      ShapeField::CornerRadius,
      ShapeField::FluteLength,
      ShapeField::ShankDiameter,
      ShapeField::OverallLength,
      ShapeField::Flutes,
    ]
  );
  // The labels and the units are the reference's own words, and a count has no unit.
  assert_eq!(ShapeField::Diameter.label(), "Diameter");
  assert_eq!(ShapeField::IncludedAngle.label(), "Included angle");
  assert_eq!(ShapeField::CornerRadius.label(), "Corner radius");
  assert_eq!(ShapeField::FluteLength.label(), "Flute length");
  assert_eq!(ShapeField::ShankDiameter.label(), "Shank diameter");
  assert_eq!(ShapeField::OverallLength.label(), "Overall length");
  assert_eq!(ShapeField::Flutes.label(), "Number of cuts");
  assert_eq!(ShapeField::Diameter.unit(), Some("mm"));
  assert_eq!(ShapeField::IncludedAngle.unit(), Some("°"));
  assert_eq!(ShapeField::Flutes.unit(), None);
  // Every shape row belongs to `GEOMETRY`, whatever the tool is.
  for kind in ToolKind::ALL {
    for field in ShapeField::of(kind) {
      assert_eq!(FormField::Shape(*field).group(), ToolFormGroup::Geometry);
      assert!(FormField::Shape(*field).editable());
    }
  }
}

#[test]
fn a_shape_row_shows_the_number_the_library_holds_and_nothing_it_does_not() {
  let tool = measured("G1", Some(1));
  assert_eq!(
    ShapeField::Diameter.value(&tool.geometry),
    Some("6".to_owned())
  );
  assert_eq!(
    ShapeField::OverallLength.value(&tool.geometry),
    Some("38".to_owned())
  );
  // A number the record does not hold is `None`, never a zero nobody wrote.
  let thin = flat_mill("G2", Some(2), 6.0);
  assert_eq!(ShapeField::FluteLength.value(&thin.geometry), None);
  assert_eq!(ShapeField::IncludedAngle.value(&thin.geometry), None);
  // A rate the record does not hold is an empty box, not a zero.
  assert_eq!(FormField::Feed.value(&bare("G3", Some(3), 6.0)), "");
  assert_eq!(FormField::Feed.value(&tool), "600");
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
    "T1 - Flat end mill (6mm)"
  );
  assert_eq!(
    tool_title(&base("G8", None, Some(6.0))),
    "Flat end mill (6mm)"
  );
  // A kind the record names is the word the row and the card use, even when its shape could paint another.
  assert_eq!(
    tool_title(&drill("G6", Some(6), 5.0, 118.0)),
    "T6 - Drill (5mm)"
  );
  assert_eq!(
    tool_title(&engraving("G5", Some(5), 3.175)),
    "T5 - Engraving (3.175mm)"
  );
}
