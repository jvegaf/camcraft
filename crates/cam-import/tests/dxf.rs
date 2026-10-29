//! Behaviour tests for the DXF importer: one entity type per test, layers, units and the located
//! errors (M3 A4, A5).
//!
//! Every fixture is built here as bytes: no network, no absolute path, no dependence on the current
//! directory.

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use cam_geom::{Point2, Segment};
use cam_import::{
  dxf, import_bytes, Format, ImportError, ImportOptions, ImportReport, ImportedLayer, Skipped,
};

const EPSILON: f64 = 1.0e-9;

fn options() -> ImportOptions {
  ImportOptions::default()
}

/// Import a fixture and hand back the layers with the report the importer filled in.
fn import(bytes: &[u8]) -> (Vec<ImportedLayer>, ImportReport) {
  let options = options();
  let mut report = ImportReport::new("dxf", options.tolerance);
  let layers = dxf::import(bytes, &options, &mut report).expect("the fixture should import");
  (layers, report)
}

fn contour_count(layers: &[ImportedLayer]) -> usize {
  layers.iter().map(|layer| layer.contours.len()).sum()
}

/// Group-code pairs as lines, one pair per two lines, so pair `i` starts on line `2 * i + 1`.
fn text(pairs: &[(&str, &str)]) -> Vec<u8> {
  let mut out = String::new();
  for (code, value) in pairs {
    push_pair(&mut out, code, value);
  }
  out.into_bytes()
}

fn push_pair(out: &mut String, code: &str, value: &str) {
  out.push_str(code);
  out.push('\n');
  out.push_str(value);
  out.push('\n');
}

/// A whole DXF: a header with `header`'s pairs, a layer table declaring `layers`, and `entities`.
fn drawing(header: &[(&str, &str)], layers: &[&str], entities: &[(&str, &str)]) -> Vec<u8> {
  let mut out = String::new();
  push_pair(&mut out, "0", "SECTION");
  push_pair(&mut out, "2", "HEADER");
  for (code, value) in header {
    push_pair(&mut out, code, value);
  }
  push_pair(&mut out, "0", "ENDSEC");

  push_pair(&mut out, "0", "SECTION");
  push_pair(&mut out, "2", "TABLES");
  push_pair(&mut out, "0", "TABLE");
  push_pair(&mut out, "2", "LAYER");
  push_pair(&mut out, "70", &layers.len().to_string());
  for name in layers {
    push_pair(&mut out, "0", "LAYER");
    push_pair(&mut out, "2", name);
    push_pair(&mut out, "70", "0");
    push_pair(&mut out, "62", "7");
    push_pair(&mut out, "6", "CONTINUOUS");
  }
  push_pair(&mut out, "0", "ENDTAB");
  push_pair(&mut out, "0", "ENDSEC");

  push_pair(&mut out, "0", "SECTION");
  push_pair(&mut out, "2", "ENTITIES");
  for (code, value) in entities {
    push_pair(&mut out, code, value);
  }
  push_pair(&mut out, "0", "ENDSEC");
  push_pair(&mut out, "0", "EOF");
  out.into_bytes()
}

#[test]
fn a_line_becomes_a_two_vertex_open_contour() {
  let file = drawing(
    &[],
    &["WALLS"],
    &[
      ("0", "LINE"),
      ("8", "WALLS"),
      ("10", "10.0"),
      ("20", "20.0"),
      ("11", "40.0"),
      ("21", "20.0"),
    ],
  );

  let (layers, report) = import(&file);

  assert_eq!(layers.len(), 1);
  assert_eq!(layers[0].name, "WALLS");
  assert_eq!(layers[0].contours.len(), 1);
  let contour = &layers[0].contours[0];
  assert_eq!(contour.len(), 2);
  assert!(!contour.is_closed());
  assert!(!contour.has_arcs());
  assert_eq!(contour.vertices()[0].point, Point2::new(10.0, 20.0));
  assert_eq!(contour.vertices()[1].point, Point2::new(40.0, 20.0));
  assert_eq!(contour.perimeter(), 30.0);
  assert_eq!(report.sources.get("LINE"), Some(&1));
  assert_eq!(report.skipped_count(), 0);
  assert_eq!(report.unit_scale, 1.0);
}

#[test]
fn a_circle_becomes_two_arcs_with_exact_radius_and_area() {
  let file = drawing(
    &[],
    &["HOLES"],
    &[
      ("0", "CIRCLE"),
      ("8", "HOLES"),
      ("10", "10.0"),
      ("20", "20.0"),
      ("40", "5.0"),
    ],
  );

  let (layers, report) = import(&file);

  let contour = &layers[0].contours[0];
  assert!(contour.is_closed());
  assert!(contour.is_ccw());
  assert!(contour.has_arcs());
  assert_eq!(contour.len(), 2);
  let arcs: Vec<cam_geom::Arc> = contour
    .segments()
    .iter()
    .filter_map(|segment| segment.as_arc())
    .collect();
  assert_eq!(arcs.len(), 2);
  for arc in arcs {
    assert_eq!(arc.center(), Point2::new(10.0, 20.0));
    assert_eq!(arc.radius(), 5.0);
    assert_eq!(arc.sweep(), PI);
  }
  assert!((contour.signed_area() - PI * 25.0).abs() < EPSILON);
  assert!((contour.perimeter() - TAU * 5.0).abs() < EPSILON);
  assert_eq!(report.sources.get("CIRCLE"), Some(&1));
  assert_eq!(report.flattened_curves, 0);
}

#[test]
fn an_arc_keeps_its_centre_and_its_quarter_turn_sweep() {
  let file = drawing(
    &[],
    &["0"],
    &[
      ("0", "ARC"),
      ("8", "0"),
      ("10", "10.0"),
      ("20", "20.0"),
      ("40", "5.0"),
      ("50", "0.0"),
      ("51", "90.0"),
    ],
  );

  let (layers, _report) = import(&file);

  let contour = &layers[0].contours[0];
  assert_eq!(contour.len(), 2);
  assert!(!contour.is_closed());
  let segments = contour.segments();
  assert_eq!(segments.len(), 1);
  let Segment::Arc(arc) = segments[0] else {
    panic!("a DXF ARC must stay an arc, got {:?}", segments[0]);
  };
  assert!(arc.center().is_near(Point2::new(10.0, 20.0), EPSILON));
  assert!((arc.radius() - 5.0).abs() < EPSILON);
  assert_eq!(arc.start_angle(), 0.0);
  assert!((arc.sweep() - FRAC_PI_2).abs() < EPSILON);
  assert!(arc.start_point().is_near(Point2::new(15.0, 20.0), EPSILON));
  assert!(arc.end_point().is_near(Point2::new(10.0, 25.0), EPSILON));
}

#[test]
fn an_lwpolyline_bulge_of_one_is_a_half_circle_of_radius_five() {
  let file = drawing(
    &[],
    &["0"],
    &[
      ("0", "LWPOLYLINE"),
      ("8", "0"),
      ("90", "2"),
      ("70", "0"),
      ("10", "0.0"),
      ("20", "0.0"),
      ("42", "1.0"),
      ("10", "10.0"),
      ("20", "0.0"),
    ],
  );

  let (layers, report) = import(&file);

  let contour = &layers[0].contours[0];
  assert_eq!(contour.len(), 2);
  assert!(!contour.is_closed());
  assert!(contour.has_arcs(), "a bulge must not become a polyline");
  let segments = contour.segments();
  let Segment::Arc(arc) = segments[0] else {
    panic!("the bulge must be an arc, got {:?}", segments[0]);
  };
  assert!(arc.center().is_near(Point2::new(5.0, 0.0), EPSILON));
  assert!((arc.radius() - 5.0).abs() < EPSILON);
  assert!((arc.sweep() - PI).abs() < EPSILON);
  let bounds = contour.bounds();
  assert!(bounds.min.is_near(Point2::new(0.0, -5.0), 1.0e-6));
  assert!(bounds.max.is_near(Point2::new(10.0, 0.0), 1.0e-6));
  assert_eq!(report.sources.get("LWPOLYLINE"), Some(&1));
  assert_eq!(report.flattened_curves, 0);
}

#[test]
fn a_polyline_with_vertex_records_carries_the_same_bulge() {
  let file = drawing(
    &[],
    &["0"],
    &[
      ("0", "POLYLINE"),
      ("8", "0"),
      ("66", "1"),
      ("70", "0"),
      ("10", "0.0"),
      ("20", "0.0"),
      ("30", "0.0"),
      ("0", "VERTEX"),
      ("8", "0"),
      ("10", "0.0"),
      ("20", "0.0"),
      ("42", "1.0"),
      ("0", "VERTEX"),
      ("8", "0"),
      ("10", "10.0"),
      ("20", "0.0"),
      ("0", "SEQEND"),
      ("8", "0"),
    ],
  );

  let (layers, report) = import(&file);

  assert_eq!(contour_count(&layers), 1);
  let contour = &layers[0].contours[0];
  assert_eq!(contour.len(), 2);
  assert!(contour.has_arcs());
  let segments = contour.segments();
  let Segment::Arc(arc) = segments[0] else {
    panic!("the bulge must be an arc, got {:?}", segments[0]);
  };
  assert!(arc.center().is_near(Point2::new(5.0, 0.0), EPSILON));
  assert!((arc.radius() - 5.0).abs() < EPSILON);
  assert!((arc.sweep() - PI).abs() < EPSILON);
  assert_eq!(report.sources.get("POLYLINE"), Some(&1));
  assert_eq!(report.sources.get("VERTEX"), None);
  assert_eq!(report.skipped_count(), 0);
}

#[test]
fn a_closed_polyline_comes_out_counter_clockwise() {
  // The four corners are written clockwise; the importer must hand back a CCW closed contour.
  let file = drawing(
    &[],
    &["0"],
    &[
      ("0", "LWPOLYLINE"),
      ("8", "0"),
      ("90", "4"),
      ("70", "1"),
      ("10", "0.0"),
      ("20", "0.0"),
      ("10", "0.0"),
      ("20", "10.0"),
      ("10", "10.0"),
      ("20", "10.0"),
      ("10", "10.0"),
      ("20", "0.0"),
    ],
  );

  let (layers, _report) = import(&file);

  let contour = &layers[0].contours[0];
  assert!(contour.is_closed());
  assert!(contour.is_ccw());
  assert_eq!(contour.len(), 4);
  assert_eq!(contour.signed_area(), 100.0);
  assert_eq!(contour.perimeter(), 40.0);
}

#[test]
fn a_full_ellipse_is_arc_bearing_and_takes_its_axes_from_the_group_codes() {
  let file = drawing(
    &[],
    &["0"],
    &[
      ("0", "ELLIPSE"),
      ("8", "0"),
      ("10", "0.0"),
      ("20", "0.0"),
      ("11", "10.0"),
      ("21", "0.0"),
      ("40", "0.5"),
      ("41", "0.0"),
      ("42", "6.283185307179586"),
    ],
  );

  let (layers, report) = import(&file);

  let contour = &layers[0].contours[0];
  assert!(contour.is_closed());
  assert!(contour.is_ccw());
  assert!(contour.has_arcs(), "a full ellipse must not be flattened");
  let bounds = contour.bounds();
  assert!((bounds.width() - 20.0).abs() < 0.01, "{:?}", bounds);
  assert!((bounds.height() - 10.0).abs() < 0.01, "{:?}", bounds);
  assert_eq!(report.flattened_curves, 0);
  assert_eq!(report.sources.get("ELLIPSE"), Some(&1));
}

#[test]
fn a_full_ellipse_rotates_with_its_major_axis() {
  // A 45 degree major axis of length 10 and a 0.5 ratio: the analytic bounds of that ellipse are
  // sqrt(a^2 cos^2 + b^2 sin^2) = sqrt(62.5) wide, where an unrotated one would be 20 wide.
  let file = drawing(
    &[],
    &["0"],
    &[
      ("0", "ELLIPSE"),
      ("8", "0"),
      ("10", "0.0"),
      ("20", "0.0"),
      ("11", "7.0710678118654755"),
      ("21", "7.0710678118654755"),
      ("40", "0.5"),
      ("41", "0.0"),
      ("42", "6.283185307179586"),
    ],
  );

  let (layers, _report) = import(&file);

  let bounds = layers[0].contours[0].bounds();
  let extent = 2.0 * 62.5_f64.sqrt();
  assert!((bounds.width() - extent).abs() < 0.05, "{:?}", bounds);
  assert!((bounds.height() - extent).abs() < 0.05, "{:?}", bounds);
  assert!(bounds.center().is_near(Point2::ORIGIN, 0.05));
}

#[test]
fn a_partial_ellipse_is_flattened_and_counted() {
  let file = drawing(
    &[],
    &["0"],
    &[
      ("0", "ELLIPSE"),
      ("8", "0"),
      ("10", "0.0"),
      ("20", "0.0"),
      ("11", "10.0"),
      ("21", "0.0"),
      ("40", "0.5"),
      ("41", "0.0"),
      ("42", "1.5707963267948966"),
    ],
  );

  let (layers, report) = import(&file);

  let contour = &layers[0].contours[0];
  assert!(!contour.is_closed());
  assert!(!contour.has_arcs(), "a partial ellipse cannot stay arcs");
  assert!(contour.len() > 2);
  assert!(contour.vertices()[0]
    .point
    .is_near(Point2::new(10.0, 0.0), EPSILON));
  assert!(contour
    .vertices()
    .last()
    .expect("a last vertex")
    .point
    .is_near(Point2::new(0.0, 5.0), EPSILON));
  for vertex in contour.vertices() {
    let level = (vertex.point.x / 10.0).powi(2) + (vertex.point.y / 5.0).powi(2);
    assert!((level - 1.0).abs() < 0.01, "off the ellipse at {:?}", level);
  }
  assert_eq!(report.flattened_curves, 1);
  assert_eq!(report.sources.get("ELLIPSE"), Some(&1));
}

#[test]
fn every_dxf_layer_with_geometry_becomes_an_imported_layer_in_table_order() {
  let file = drawing(
    &[],
    &["OUTSIDE", "EMPTY", "HOLES"],
    &[
      ("0", "LINE"),
      ("8", "OUTSIDE"),
      ("10", "0.0"),
      ("20", "0.0"),
      ("11", "10.0"),
      ("21", "0.0"),
      ("0", "CIRCLE"),
      ("8", "HOLES"),
      ("10", "20.0"),
      ("20", "0.0"),
      ("40", "5.0"),
      ("0", "CIRCLE"),
      ("8", "EXTRA"),
      ("10", "40.0"),
      ("20", "0.0"),
      ("40", "2.0"),
      // A missing code 8 keeps the DXF default layer name "0".
      ("0", "LINE"),
      ("10", "0.0"),
      ("20", "5.0"),
      ("11", "10.0"),
      ("21", "5.0"),
    ],
  );

  let (layers, report) = import(&file);

  let names: Vec<&str> = layers.iter().map(|layer| layer.name.as_str()).collect();
  assert_eq!(names, vec!["OUTSIDE", "HOLES", "EXTRA", "0"]);
  for layer in &layers {
    assert_eq!(layer.contours.len(), 1, "{}", layer.name);
  }
  assert_eq!(report.sources.get("LINE"), Some(&2));
  assert_eq!(report.sources.get("CIRCLE"), Some(&2));
}

#[test]
fn insunits_inches_scale_every_point_by_twenty_five_point_four() {
  let file = drawing(
    &[("9", "$INSUNITS"), ("70", "1")],
    &["0"],
    &[
      ("0", "LINE"),
      ("8", "0"),
      ("10", "0.0"),
      ("20", "0.0"),
      ("11", "1.0"),
      ("21", "0.0"),
    ],
  );

  let (layers, report) = import(&file);

  assert_eq!(report.unit_scale, 25.4);
  assert_eq!(report.warnings, Vec::<String>::new());
  let contour = &layers[0].contours[0];
  assert!(contour.vertices()[1]
    .point
    .is_near(Point2::new(25.4, 0.0), EPSILON));
  assert!((contour.perimeter() - 25.4).abs() < EPSILON);
}

#[test]
fn measurement_imperial_without_insunits_scales_and_warns() {
  let file = drawing(
    &[("9", "$MEASUREMENT"), ("70", "0")],
    &["0"],
    &[
      ("0", "LINE"),
      ("8", "0"),
      ("10", "0.0"),
      ("20", "0.0"),
      ("11", "1.0"),
      ("21", "0.0"),
    ],
  );

  let (layers, report) = import(&file);

  assert_eq!(report.unit_scale, 25.4);
  assert_eq!(
    report.warnings,
    vec!["no $INSUNITS and $MEASUREMENT is 0 (imperial): inches are assumed".to_string()]
  );
  assert!((layers[0].contours[0].perimeter() - 25.4).abs() < EPSILON);
}

#[test]
fn a_header_with_neither_unit_variable_assumes_millimetres_and_says_so() {
  let file = drawing(
    &[("9", "$ACADVER"), ("1", "AC1032")],
    &["0"],
    &[
      ("0", "LINE"),
      ("8", "0"),
      ("10", "0.0"),
      ("20", "0.0"),
      ("11", "2.0"),
      ("21", "0.0"),
    ],
  );

  let (_layers, report) = import(&file);

  assert_eq!(report.unit_scale, 1.0);
  assert_eq!(
    report.warnings,
    vec!["no $INSUNITS and no $MEASUREMENT: millimetres are assumed".to_string()]
  );
}

#[test]
fn an_unknown_insunits_code_is_refused_by_name() {
  let file = drawing(
    &[("9", "$INSUNITS"), ("70", "7")],
    &["0"],
    &[
      ("0", "LINE"),
      ("8", "0"),
      ("10", "0.0"),
      ("20", "0.0"),
      ("11", "1.0"),
      ("21", "0.0"),
    ],
  );

  let mut report = ImportReport::new("dxf", options().tolerance);
  let error = dxf::import(&file, &options(), &mut report).expect_err("refused");

  assert_eq!(
    error,
    ImportError::Refused {
      format: "dxf",
      reason: "$INSUNITS 7 is a unit this importer does not know: supported codes are 0 \
               (unitless), 1 (inch), 2 (foot), 4 (millimetre), 5 (centimetre) and 6 (metre)"
        .to_string(),
    }
  );
}

#[test]
fn a_binary_dxf_is_refused_by_name() {
  let mut report = ImportReport::new("dxf", options().tolerance);
  let error = dxf::import(
    b"AutoCAD Binary DXF\r\n\x1a\x00binary",
    &options(),
    &mut report,
  )
  .expect_err("refused");

  assert_eq!(
    error,
    ImportError::Refused {
      format: "dxf",
      reason: "binary DXF is not supported: re-save the drawing as ASCII DXF".to_string(),
    }
  );
}

#[test]
fn a_text_entity_is_skipped_with_its_line_number() {
  let file = text(&[
    ("0", "SECTION"),
    ("2", "ENTITIES"),
    ("0", "LINE"),
    ("8", "0"),
    ("10", "0.0"),
    ("20", "0.0"),
    ("11", "5.0"),
    ("21", "0.0"),
    // The ninth pair: its `0` is on line 2 * 8 + 1 = 17.
    ("0", "TEXT"),
    ("8", "NOTES"),
    ("1", "part 42"),
    ("0", "ENDSEC"),
    ("0", "EOF"),
  ]);

  let (layers, report) = import(&file);

  assert_eq!(contour_count(&layers), 1);
  assert_eq!(layers[0].contours[0].perimeter(), 5.0);
  assert_eq!(
    report.skipped,
    vec![Skipped {
      construct: "TEXT".to_string(),
      reason: "not a supported DXF entity: dxf imports LINE, LWPOLYLINE, POLYLINE, ARC, CIRCLE \
               and ELLIPSE"
        .to_string(),
      location: Some("line 17".to_string()),
    }]
  );
  assert_eq!(report.sources.get("TEXT"), None);
}

#[test]
fn a_blocks_section_is_ignored_rather_than_imported() {
  let file = text(&[
    ("0", "SECTION"),
    ("2", "BLOCKS"),
    ("0", "BLOCK"),
    ("2", "PART"),
    ("10", "0.0"),
    ("20", "0.0"),
    ("0", "LINE"),
    ("8", "0"),
    ("10", "0.0"),
    ("20", "0.0"),
    ("11", "100.0"),
    ("21", "0.0"),
    ("0", "ENDBLK"),
    ("0", "ENDSEC"),
    ("0", "SECTION"),
    ("2", "ENTITIES"),
    ("0", "LINE"),
    ("8", "0"),
    ("10", "0.0"),
    ("20", "0.0"),
    ("11", "5.0"),
    ("21", "0.0"),
    ("0", "ENDSEC"),
    ("0", "EOF"),
  ]);

  let (layers, report) = import(&file);

  assert_eq!(contour_count(&layers), 1);
  assert_eq!(layers[0].contours[0].perimeter(), 5.0);
  assert_eq!(report.sources.get("LINE"), Some(&1));
  assert_eq!(report.skipped_count(), 0);
}

#[test]
fn an_insert_is_skipped_rather_than_expanded() {
  let file = text(&[
    ("0", "SECTION"),
    ("2", "ENTITIES"),
    // The third pair: its `0` is on line 5.
    ("0", "INSERT"),
    ("8", "0"),
    ("2", "PART"),
    ("0", "ENDSEC"),
    ("0", "EOF"),
  ]);

  let (layers, report) = import(&file);

  assert!(layers.is_empty());
  assert_eq!(
    report.skipped,
    vec![Skipped {
      construct: "INSERT".to_string(),
      reason: "block references are not expanded: geometry inside BLOCKS is not imported"
        .to_string(),
      location: Some("line 5".to_string()),
    }]
  );
}

#[test]
fn an_entity_missing_a_required_group_code_is_skipped_not_fatal() {
  let file = text(&[
    ("0", "SECTION"),
    ("2", "ENTITIES"),
    // The third pair: its `0` is on line 5. The circle carries no radius.
    ("0", "CIRCLE"),
    ("8", "0"),
    ("10", "0.0"),
    ("20", "0.0"),
    ("0", "LINE"),
    ("8", "0"),
    ("10", "0.0"),
    ("20", "0.0"),
    ("11", "5.0"),
    ("21", "0.0"),
    ("0", "ENDSEC"),
    ("0", "EOF"),
  ]);

  let (layers, report) = import(&file);

  assert_eq!(contour_count(&layers), 1);
  assert_eq!(layers[0].contours[0].perimeter(), 5.0);
  assert_eq!(
    report.skipped,
    vec![Skipped {
      construct: "CIRCLE".to_string(),
      reason: "missing the radius (group code 40)".to_string(),
      location: Some("line 5".to_string()),
    }]
  );
}

#[test]
fn a_non_numeric_group_code_is_refused_with_its_line_number() {
  let file = b"0\nSECTION\n2\nENTITIES\n0\nLINE\n10\n0\n20\n0\n1x\n5\n0\nENDSEC\n0\nEOF\n";

  let mut report = ImportReport::new("dxf", options().tolerance);
  let error = dxf::import(file, &options(), &mut report).expect_err("refused");

  assert_eq!(
    error,
    ImportError::Parse {
      format: "dxf",
      message: "group code \"1x\" is not an integer".to_string(),
      location: Some("line 11".to_string()),
    }
  );
}

#[test]
fn a_group_code_without_its_value_line_is_refused_with_its_line_number() {
  let file = b"0\nSECTION\n2\nENTITIES\n0\nLINE\n10\n0\n20\n";

  let mut report = ImportReport::new("dxf", options().tolerance);
  let error = dxf::import(file, &options(), &mut report).expect_err("refused");

  assert_eq!(
    error,
    ImportError::Parse {
      format: "dxf",
      message: "group code 20 has no value line".to_string(),
      location: Some("line 9".to_string()),
    }
  );
}

#[test]
fn a_malformed_number_inside_a_known_entity_is_refused_with_its_line_number() {
  let file = b"0\nSECTION\n2\nENTITIES\n0\nLINE\n10\nten\n20\n0\n11\n5\n21\n0\n0\nENDSEC\n0\nEOF\n";

  let mut report = ImportReport::new("dxf", options().tolerance);
  let error = dxf::import(file, &options(), &mut report).expect_err("refused");

  assert_eq!(
    error,
    ImportError::Parse {
      format: "dxf",
      message: "group code 10 value \"ten\" is not a number".to_string(),
      location: Some("line 7".to_string()),
    }
  );
}

#[test]
fn a_dxf_that_only_carries_skipped_constructs_is_nothing_imported() {
  let file = drawing(&[], &["0"], &[("0", "TEXT"), ("8", "0"), ("1", "hello")]);

  let error = import_bytes("empty.dxf", &file, &options()).expect_err("nothing imported");

  assert_eq!(
    error,
    ImportError::NothingImported {
      format: "dxf",
      reason: "every construct was skipped, starting with TEXT".to_string(),
    }
  );
}

#[test]
fn importing_the_same_bytes_twice_gives_the_same_geometry_and_report() {
  let file = drawing(
    &[("9", "$INSUNITS"), ("70", "1")],
    &["OUTSIDE", "HOLES"],
    &[
      ("0", "LINE"),
      ("8", "OUTSIDE"),
      ("10", "0.0"),
      ("20", "0.0"),
      ("11", "10.0"),
      ("21", "0.0"),
      ("0", "CIRCLE"),
      ("8", "HOLES"),
      ("10", "20.0"),
      ("20", "0.0"),
      ("40", "5.0"),
      ("0", "LWPOLYLINE"),
      ("8", "OUTSIDE"),
      ("90", "2"),
      ("70", "1"),
      ("10", "0.0"),
      ("20", "0.0"),
      ("42", "0.5"),
      ("10", "10.0"),
      ("20", "0.0"),
      ("0", "ELLIPSE"),
      ("8", "OUTSIDE"),
      ("10", "0.0"),
      ("20", "0.0"),
      ("11", "4.0"),
      ("21", "0.0"),
      ("40", "0.5"),
      ("0", "ELLIPSE"),
      ("8", "OUTSIDE"),
      ("10", "0.0"),
      ("20", "0.0"),
      ("11", "4.0"),
      ("21", "0.0"),
      ("40", "0.5"),
      ("41", "0.0"),
      ("42", "1.0"),
      ("0", "TEXT"),
      ("8", "HOLES"),
      ("1", "note"),
    ],
  );

  let (first_layers, first_report) = import(&file);
  let (second_layers, second_report) = import(&file);

  assert_eq!(first_layers, second_layers);
  assert_eq!(first_report, second_report);
  assert_eq!(first_report.sources.get("LINE"), Some(&1));
  assert_eq!(first_report.sources.get("CIRCLE"), Some(&1));
  assert_eq!(first_report.sources.get("LWPOLYLINE"), Some(&1));
  assert_eq!(first_report.sources.get("ELLIPSE"), Some(&2));
  assert_eq!(first_report.flattened_curves, 1);
  assert_eq!(first_report.skipped_count(), 1);
}

#[test]
fn the_import_spine_still_reports_the_dxf_format_and_its_contour_count() {
  // The shared dispatch was already wired when this module was a stub; importing through it must
  // keep naming the format and counting the contours the importer returned.
  let file = drawing(
    &[],
    &["0"],
    &[
      ("0", "LINE"),
      ("8", "0"),
      ("10", "0.0"),
      ("20", "0.0"),
      ("11", "10.0"),
      ("21", "0.0"),
      ("0", "CIRCLE"),
      ("8", "0"),
      ("10", "5.0"),
      ("20", "5.0"),
      ("40", "1.0"),
    ],
  );

  let imported = import_bytes("part.dxf", &file, &options()).expect("import");

  assert_eq!(imported.format, Format::Dxf);
  assert_eq!(imported.report.format, "dxf");
  assert_eq!(imported.report.tolerance, cam_import::DEFAULT_TOLERANCE);
  assert_eq!(imported.report.contours, 2);
  assert_eq!(imported.layers.len(), 1);
  assert_eq!(imported.layers[0].contours.len(), 2);
}
