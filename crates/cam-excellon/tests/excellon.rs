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
fn a_relative_program_accumulates_from_the_last_position() {
  // E2: G91 makes every coordinate a step from where the tool already is. At 000.000 a step of
  // 1000 is a millimetre, so the two holes sit at 1 mm and 1.5 mm.
  let file = parse("M48\nMETRIC,000.000\nT1C1.0\n%\nT1\nG00X0Y0\nG91\nX1000Y0\nX500Y0\nM30\n")
    .expect("relative coordinates are read");
  let xs: Vec<f64> = file
    .features
    .iter()
    .filter_map(|feature| match feature {
      cam_excellon::Feature::Hole { at, .. } => Some(at.x),
      _ => None,
    })
    .collect();
  assert_eq!(xs.len(), 2, "two holes: {xs:?}");
  assert!(
    (xs[0] - 1.0).abs() < 1.0e-9,
    "first hole at 1 mm: {:?}",
    xs[0]
  );
  assert!(
    (xs[1] - 1.5).abs() < 1.0e-9,
    "the second is one step further: {:?}",
    xs[1]
  );
}

#[test]
fn g90_returns_to_absolute_coordinates() {
  let file =
    parse("M48\nMETRIC,000.000\nT1C1.0\n%\nT1\nG00X0Y0\nG91\nX1000Y0\nG90\nX2000Y0\nM30\n")
      .expect("both modes are read");
  let xs: Vec<f64> = file
    .features
    .iter()
    .filter_map(|feature| match feature {
      cam_excellon::Feature::Hole { at, .. } => Some(at.x),
      _ => None,
    })
    .collect();
  assert!((xs[0] - 1.0).abs() < 1.0e-9, "the step: {:?}", xs[0]);
  assert!(
    (xs[1] - 2.0).abs() < 1.0e-9,
    "absolute again, not 3: {:?}",
    xs[1]
  );
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

#[test]
fn an_integer_coordinate_is_read_with_the_declared_file_format() {
  // E1: KiCad writes its digit split in a comment. 11528623 is eight digits, so 2:6 reads it as
  // 11.528623 mm; reading those digits as millimetres would put the hole a million times too far.
  let file = parse("M48\n;FILE_FORMAT=2:6\nMETRIC\nT1C0.800\n%\nT1\nX11528623Y0\nM30\n")
    .expect("fixture parses");
  match &file.features[0] {
    cam_excellon::Feature::Hole { at, .. } => {
      assert!((at.x - 11.528623).abs() < 1.0e-9, "x {:?}", at.x);
      assert!(at.y.abs() < 1.0e-9, "y {:?}", at.y);
    }
    other => panic!("a hole, got {other:?}"),
  }
}

#[test]
fn a_zero_pattern_declares_the_digit_format_too() {
  let file = parse("M48\nMETRIC,000.000\nT1C0.800\n%\nT1\nX11528Y0\nM30\n").expect("parses");
  match &file.features[0] {
    cam_excellon::Feature::Hole { at, .. } => {
      assert!((at.x - 11.528).abs() < 1.0e-9, "x {:?}", at.x);
    }
    other => panic!("a hole, got {other:?}"),
  }
}

#[test]
fn a_coordinate_with_a_decimal_point_keeps_its_own_scale() {
  let file = parse("M48\nMETRIC\nT1C0.800\n%\nT1\nX11.528623Y0\nM30\n").expect("parses");
  match &file.features[0] {
    cam_excellon::Feature::Hole { at, .. } => {
      assert!((at.x - 11.528623).abs() < 1.0e-9, "x {:?}", at.x);
    }
    other => panic!("a hole, got {other:?}"),
  }
}

#[test]
fn an_integer_coordinate_without_a_declared_format_is_refused() {
  // METRIC,LZ says the unit and the suppression, and nothing about how many digits there are, so
  // there is no honest scale to apply.
  let error = parse("M48\nMETRIC,LZ\nT1C0.800\n%\nT1\nX11528623Y0\nM30\n")
    .expect_err("a bare unit does not declare the digits");
  let text = format!("{error}");
  assert!(
    text.contains("digit format"),
    "the refusal says why: {text}"
  );
}

/// The six fabrication drill files the corpus ships: each must read with the tool table and hit
/// count the file declares. "Hits" are the features the file cuts — holes and routed slots.
#[test]
fn the_six_corpus_drill_files_match_their_tools_and_hits() {
  struct Board {
    slug: &'static str,
    tools: &'static [(u32, f64)],
    features: usize,
  }

  let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../fixtures/corpus")
    .canonicalize()
    .expect("fixtures/corpus is part of the repository");

  let boards = [
    Board {
      slug: "kicad-2l-simple",
      tools: &[(1, 0.3), (2, 0.4), (3, 1.5)],
      features: 88,
    },
    Board {
      slug: "kicad-4l-pours",
      tools: &[(1, 0.3), (2, 0.3), (3, 0.4), (4, 0.6), (5, 1.0), (6, 3.2)],
      features: 692,
    },
    Board {
      slug: "kicad-large-pours",
      tools: &[
        (1, 0.6),
        (2, 0.75),
        (3, 0.8),
        (4, 0.9),
        (5, 1.0),
        (6, 1.1),
        (7, 1.2),
        (8, 1.27),
        (9, 1.3),
        (10, 2.0),
        (11, 3.2),
        (12, 3.5),
        (13, 4.3),
      ],
      features: 251,
    },
    Board {
      slug: "substitute-altium",
      tools: &[
        (1, 0.4),
        (2, 0.75),
        (3, 0.8),
        (4, 0.813),
        (5, 1.0),
        (6, 1.016),
        (7, 1.2),
        (8, 1.3),
        (9, 1.6),
        (10, 3.2),
      ],
      features: 526,
    },
    Board {
      slug: "substitute-eagle",
      tools: &[
        (1, 0.3),
        (2, 0.4),
        (3, 0.6),
        (4, 0.65),
        (5, 0.7),
        (6, 0.9),
        (7, 0.914),
        (8, 1.0),
        (9, 1.1),
        (10, 1.3),
        (11, 1.6),
        (12, 1.8),
      ],
      features: 445,
    },
    Board {
      slug: "substitute-easyeda",
      tools: &[
        (1, 0.7),
        (2, 0.75),
        (3, 0.8),
        (4, 1.0),
        (5, 1.1),
        (6, 1.2),
        (7, 1.3),
      ],
      features: 165,
    },
  ];

  for board in boards {
    let slug = board.slug;
    let path = root.join(slug).join("drill.drl");
    let text = std::fs::read_to_string(&path)
      .unwrap_or_else(|error| panic!("{slug}: cannot read {}: {error}", path.display()));
    let file = parse(&text).unwrap_or_else(|error| panic!("{slug}: {error}"));

    assert_eq!(file.tools.len(), board.tools.len(), "{slug}: tool count");
    for (actual, &(number, diameter_mm)) in file.tools.iter().zip(board.tools) {
      assert_eq!(actual.number, number, "{slug}: tool number");
      assert!(
        (actual.diameter.mm() - diameter_mm).abs() < 1.0e-9,
        "{slug}: tool T{number} diameter: got {} mm, expected {diameter_mm} mm",
        actual.diameter.mm()
      );
    }
    assert_eq!(
      file.features.len(),
      board.features,
      "{slug}: feature/hit count"
    );
  }
}
