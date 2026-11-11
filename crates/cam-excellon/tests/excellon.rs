//! Behaviour tests for the Excellon reader.

use cam_excellon::{parse, ExcellonError, Feature, Units};
use cam_geom::Point2;

/// A KiCad 10.0.7 drill file, exactly as it was written: metric, decimal coordinates, one tool, and
/// two routed slots instead of holes.
const KICAD_SLOTS: &str = "\
M48
; DRILL file KiCad 10.0.7 date 2026-11-02T23:43:16
; FORMAT={-:-/ absolute / metric / decimal}
; #@! TF.GenerationSoftware,Kicad,Pcbnew,10.0.7
FMAT,2
METRIC
; #@! TA.AperFunction,Plated,PTH,ComponentDrill
T1C1.000
%
G90
G05
T1
G00X-17.272Y1.75
M15
G01X-17.272Y-1.75
M16
G05
G00X17.221Y1.699
M15
G01X17.221Y-1.801
M16
G05
M30
";

fn close(actual: Point2, expected: (f64, f64)) -> bool {
  (actual.x - expected.0).abs() < 1.0e-9 && (actual.y - expected.1).abs() < 1.0e-9
}

#[test]
fn a_kicad_file_reads_its_tool_and_its_two_slots() {
  // The real file has no holes at all: both features are slots, written as a rapid, the tool down,
  // a feed and the tool up. A reader that only understands `X..Y..` would report an empty program.
  let file = parse(KICAD_SLOTS).expect("a KiCad drill file reads");
  assert_eq!(file.units, Units::Metric);
  assert_eq!(file.tools.len(), 1);
  assert_eq!(file.tools[0].number, 1);
  assert!((file.tools[0].diameter.mm() - 1.0).abs() < 1.0e-9);

  assert_eq!(file.features.len(), 2, "two slots: {:?}", file.features);
  match file.features[0] {
    Feature::Slot { tool, from, to } => {
      assert_eq!(tool, 1);
      assert!(close(from, (-17.272, 1.75)), "{from:?}");
      assert!(close(to, (-17.272, -1.75)), "{to:?}");
    }
    other => panic!("expected a slot, got {other:?}"),
  }
  match file.features[1] {
    Feature::Slot { from, to, .. } => {
      assert!(close(from, (17.221, 1.699)), "{from:?}");
      assert!(close(to, (17.221, -1.801)), "{to:?}");
    }
    other => panic!("expected a slot, got {other:?}"),
  }
}

#[test]
fn bare_coordinates_drill_holes_with_the_selected_tool() {
  let file =
    parse("M48\nMETRIC\nT1C0.8\nT2C1.2\n%\nT1\nX10.0Y10.0\nX20.0Y10.0\nT2\nX30.5Y15.25\nM30\n")
      .expect("a hole file reads");
  assert_eq!(file.tools.len(), 2);
  assert_eq!(file.features.len(), 3);
  match (file.features[0], file.features[1], file.features[2]) {
    (
      Feature::Hole { tool: first, at: a },
      Feature::Hole {
        tool: second,
        at: b,
      },
      Feature::Hole { tool: third, at: c },
    ) => {
      assert_eq!(
        (first, second, third),
        (1, 1, 2),
        "the tool change picks T2"
      );
      assert!(close(a, (10.0, 10.0)));
      assert!(close(b, (20.0, 10.0)));
      assert!(close(c, (30.5, 15.25)));
    }
    other => panic!("expected three holes, got {other:?}"),
  }
}

#[test]
fn an_inch_file_is_converted_to_millimetres() {
  let file = parse("M48\nINCH\nT1C0.04\n%\nT1\nX1.0Y2.0\nM30\n").expect("an inch file reads");
  assert_eq!(file.units, Units::Inch);
  assert!((file.tools[0].diameter.mm() - 1.016).abs() < 1.0e-9);
  match file.features[0] {
    Feature::Hole { at, .. } => assert!(close(at, (25.4, 50.8)), "{at:?}"),
    other => panic!("expected a hole, got {other:?}"),
  }
}

#[test]
fn a_relative_program_is_refused_naming_the_line() {
  let error = parse("M48\nMETRIC\nT1C1.0\n%\nT1\nG00X0Y0\nG91\n").expect_err("G91 is not read");
  assert!(
    matches!(error, ExcellonError::Unsupported { line: 7, .. }),
    "{error:?}"
  );
  assert!(error.to_string().contains("G91"), "{error}");
}

#[test]
fn a_hole_before_any_tool_is_refused() {
  let error = parse("M48\nMETRIC\nT1C1.0\n%\nX1.0Y1.0\n").expect_err("no tool is selected");
  assert!(
    matches!(error, ExcellonError::NoToolSelected { line: 5 }),
    "{error:?}"
  );
}

#[test]
fn selecting_a_tool_the_header_never_defined_is_refused() {
  let error = parse("M48\nMETRIC\nT1C1.0\n%\nT2\nX1.0Y1.0\n").expect_err("T2 is not in the table");
  assert!(
    matches!(error, ExcellonError::UnknownTool { line: 5, tool: 2 }),
    "{error:?}"
  );
  assert!(error.to_string().contains('2'), "{error}");
}

#[test]
fn a_coordinate_without_units_has_no_scale_and_is_refused() {
  let error = parse("M48\nT1C1.0\n%\nT1\nX1.0Y1.0\n").expect_err("no units are declared");
  assert!(
    matches!(error, ExcellonError::MissingUnits { line: 5 }),
    "{error:?}"
  );
}

#[test]
fn a_header_entry_this_reader_does_not_know_is_refused_naming_it() {
  let error = parse("M48\nSOMETHING,1\n%\n").expect_err("the header entry is unknown");
  assert!(
    matches!(error, ExcellonError::Unsupported { line: 2, .. }),
    "{error:?}"
  );
  assert!(error.to_string().contains("SOMETHING"), "{error}");
}

#[test]
fn a_line_that_is_neither_a_command_nor_a_coordinate_is_refused() {
  let error = parse("M48\nMETRIC\nT1C1.0\n%\nT1\nnot a command\n").expect_err("not a word");
  assert!(
    matches!(error, ExcellonError::UnknownWord { line: 6, .. }),
    "{error:?}"
  );
}
