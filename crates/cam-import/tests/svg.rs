//! Behaviour tests for the SVG importer (M3 A6, A7, A8).
//!
//! Every assertion is on observable geometry — bounds, area, perimeter, the arcs that survived, the
//! reported counts — never on a vertex dump. The fixtures are byte literals in this file so the tests
//! are hermetic: no network, no absolute path, no dependence on the current directory.

use std::f64::consts::{PI, TAU};

use cam_geom::{Contour, Point2};
use cam_import::{import_bytes, ImportError, ImportOptions, Imported, DEFAULT_TOLERANCE};

/// A page whose user space is millimetres, so shape coordinates and millimetres are the same
/// number: `viewBox` 100×100 mapped onto 100 mm × 100 mm.
const PAGE: &str =
  r#"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="100mm" viewBox="0 0 100 100">"#;

fn import_document(body: &str) -> Imported {
  import_source(&format!("{PAGE}{body}</svg>"))
}

fn import_source(source: &str) -> Imported {
  import_bytes("drawing.svg", source.as_bytes(), &ImportOptions::default()).expect("import")
}

fn contours(imported: &Imported) -> &[Contour] {
  assert_eq!(imported.layers.len(), 1, "an svg is one layer");
  &imported.layers[0].contours
}

fn only(imported: &Imported) -> &Contour {
  let contours = contours(imported);
  assert_eq!(contours.len(), 1, "expected exactly one contour");
  &contours[0]
}

fn arc_count(contour: &Contour) -> usize {
  contour
    .segments()
    .iter()
    .filter(|segment| segment.as_arc().is_some())
    .count()
}

fn distance_to(contour: &Contour, point: Point2) -> f64 {
  contour
    .segments()
    .iter()
    .map(|segment| segment.distance_to_point(point))
    .fold(f64::INFINITY, f64::min)
}

fn assert_near(actual: f64, expected: f64, what: &str) {
  assert!(
    (actual - expected).abs() < 1e-9,
    "{what}: expected {expected}, got {actual}"
  );
}

fn assert_point(actual: Point2, expected: Point2, what: &str) {
  assert!(
    actual.is_near(expected, 1e-9),
    "{what}: expected {expected:?}, got {actual:?}"
  );
}

fn assert_bounds(contour: &Contour, min: (f64, f64), max: (f64, f64)) {
  let bounds = contour.bounds();
  assert_point(
    bounds.min,
    Point2::new(min.0, min.1),
    "contour bounds minimum",
  );
  assert_point(
    bounds.max,
    Point2::new(max.0, max.1),
    "contour bounds maximum",
  );
}

/// The y-axis in millimetres, from SVG's y-down page coordinate (D-M3-10).
///
/// [`PAGE`] is 100 mm tall, so `up(y) = 100 - y`: every expectation below writes the flip down
/// instead of hiding it behind a number that happens to be right.
fn up(y: f64) -> f64 {
  100.0 - y
}

#[test]
fn a_rect_keeps_its_exact_bounds() {
  let imported = import_document(r#"<rect x="10" y="20" width="30" height="40"/>"#);
  let contour = only(&imported);

  assert!(contour.is_closed());
  assert_eq!(arc_count(contour), 0);
  assert_bounds(contour, (10.0, up(60.0)), (40.0, up(20.0)));
  assert_near(contour.signed_area(), 1200.0, "rect area");
  assert_near(contour.perimeter(), 140.0, "rect perimeter");
  assert_eq!(imported.report.sources.get("<rect>"), Some(&1));
  assert_near(imported.report.unit_scale, 1.0, "unit scale");
}

#[test]
fn a_circle_keeps_both_of_its_arcs() {
  let imported = import_document(r#"<circle cx="50" cy="50" r="5"/>"#);
  let contour = only(&imported);

  assert!(contour.is_closed());
  assert_eq!(arc_count(contour), 2, "a circle is two half-turn arcs");
  assert_bounds(contour, (45.0, 45.0), (55.0, 55.0));
  assert_near(contour.signed_area(), PI * 25.0, "circle area");
  assert_near(contour.perimeter(), TAU * 5.0, "circle perimeter");
  assert_eq!(imported.report.flattened_curves, 0);
}

#[test]
fn an_ellipse_is_arc_bearing_and_keeps_its_bounds() {
  let imported = import_document(r#"<ellipse cx="50" cy="50" rx="10" ry="5"/>"#);
  let contour = only(&imported);

  assert!(contour.is_closed());
  assert!(
    contour.has_arcs(),
    "an ellipse is approximated by circular arcs, not by a polyline"
  );
  assert_bounds(contour, (40.0, 45.0), (60.0, 55.0));
  assert!(
    (contour.signed_area() - PI * 50.0).abs() < 0.5,
    "ellipse area {} is close to {}",
    contour.signed_area(),
    PI * 50.0
  );
}

#[test]
fn a_line_polyline_and_polygon_keep_their_bounds() {
  let line = import_document(r#"<line x1="0" y1="0" x2="30" y2="40"/>"#);
  let contour = only(&line);
  assert!(!contour.is_closed());
  assert_bounds(contour, (0.0, up(40.0)), (30.0, up(0.0)));
  assert_near(contour.perimeter(), 50.0, "line length");

  let polyline = import_document(r#"<polyline points="0,0 10,0 10,10"/>"#);
  let contour = only(&polyline);
  assert!(!contour.is_closed());
  assert_bounds(contour, (0.0, up(10.0)), (10.0, up(0.0)));
  assert_near(contour.perimeter(), 20.0, "polyline length");

  let polygon = import_document(r#"<polygon points="0,0 10,0 10,10"/>"#);
  let contour = only(&polygon);
  assert!(contour.is_closed());
  assert_near(contour.signed_area(), 50.0, "polygon area");
  assert_near(
    contour.perimeter(),
    20.0 + 200.0_f64.sqrt(),
    "polygon perimeter",
  );
}

#[test]
fn two_closed_subpaths_in_one_path_yield_two_contours() {
  let imported =
    import_document(r#"<path d="M 10 10 L 30 10 L 30 30 Z M 40 10 L 60 10 L 60 30 Z"/>"#);
  let contours = contours(&imported);

  assert_eq!(contours.len(), 2);
  assert!(contours.iter().all(Contour::is_closed));
  assert_bounds(&contours[0], (10.0, up(30.0)), (30.0, up(10.0)));
  assert_bounds(&contours[1], (40.0, up(30.0)), (60.0, up(10.0)));
  assert_near(contours[0].signed_area(), 200.0, "first triangle area");
  assert_eq!(imported.report.sources.get("<path>"), Some(&1));
}

#[test]
fn a_circular_arc_stays_an_exact_arc() {
  let imported = import_document(r#"<path d="M 10 10 A 5 5 0 0 1 20 10"/>"#);
  let contour = only(&imported);

  assert_eq!(arc_count(contour), 1, "one `A` is one exact arc");
  assert_eq!(imported.report.flattened_curves, 0);
  let arc = contour.segments()[0]
    .as_arc()
    .expect("the segment is an arc");
  assert_point(arc.center(), Point2::new(15.0, up(10.0)), "arc centre");
  assert_near(arc.radius(), 5.0, "arc radius");
  // The y inversion reverses the turn direction, so the sweep it came in with changes sign.
  assert_near(arc.sweep(), -PI, "arc sweep");
  assert_point(arc.mid_point(), Point2::new(15.0, up(5.0)), "arc midpoint");
}

#[test]
fn the_sweep_flag_decides_which_side_the_arc_bends_to() {
  let imported = import_document(r#"<path d="M 10 10 A 5 5 0 0 0 20 10"/>"#);
  let contour = only(&imported);
  let arc = contour.segments()[0]
    .as_arc()
    .expect("the segment is an arc");

  assert_point(arc.center(), Point2::new(15.0, up(10.0)), "arc centre");
  assert_near(arc.sweep(), PI, "arc sweep");
  assert_point(arc.mid_point(), Point2::new(15.0, up(15.0)), "arc midpoint");
}

#[test]
fn an_arc_with_too_small_a_radius_is_scaled_up_to_fit() {
  // SVG F.6.6: a radius that cannot reach both endpoints is grown until it can, which makes this a
  // half turn of radius 10 rather than an unrepresentable request.
  let imported = import_document(r#"<path d="M 0 0 A 1 1 0 0 1 20 0"/>"#);
  let contour = only(&imported);
  let arc = contour.segments()[0]
    .as_arc()
    .expect("the segment is an arc");

  assert_near(arc.radius(), 10.0, "grown radius");
  assert_near(arc.sweep(), -PI, "half turn");
}

#[test]
fn an_elliptical_arc_is_flattened_within_the_tolerance() {
  let imported = import_document(r#"<path d="M 0 0 A 10 5 0 0 1 20 0"/>"#);
  let contour = only(&imported);

  assert!(
    !contour.has_arcs(),
    "an elliptical arc is not a circular arc"
  );
  assert_eq!(imported.report.flattened_curves, 1);
  assert_bounds(contour, (0.0, up(0.0)), (20.0, up(-5.0)));

  // (10, -5) is the middle of the true ellipse; the polyline must pass within the tolerance.
  let deviation = distance_to(contour, Point2::new(10.0, up(-5.0)));
  assert!(
    deviation <= DEFAULT_TOLERANCE,
    "deviation {deviation} exceeds the tolerance {DEFAULT_TOLERANCE}"
  );
}

#[test]
fn a_cubic_bezier_is_flattened_within_the_tolerance_and_counted_once() {
  let imported = import_document(r#"<path d="M 0 0 C 0 10 10 10 10 0"/>"#);
  let contour = only(&imported);

  assert!(!contour.has_arcs());
  assert_eq!(imported.report.flattened_curves, 1);
  assert_bounds(contour, (0.0, up(7.5)), (10.0, up(0.0)));

  // (5, 7.5) is the middle of the true cubic.
  let deviation = distance_to(contour, Point2::new(5.0, up(7.5)));
  assert!(
    deviation <= DEFAULT_TOLERANCE,
    "deviation {deviation} exceeds the tolerance {DEFAULT_TOLERANCE}"
  );
  let end = contour.vertices().last().expect("vertices").point;
  assert_point(end, Point2::new(10.0, up(0.0)), "curve end point");
}

#[test]
fn a_quadratic_bezier_is_flattened_within_the_tolerance_and_counted_once() {
  let imported = import_document(r#"<path d="M 0 0 Q 5 10 10 0"/>"#);
  let contour = only(&imported);

  assert!(!contour.has_arcs());
  assert_eq!(imported.report.flattened_curves, 1);
  assert_bounds(contour, (0.0, up(5.0)), (10.0, up(0.0)));

  let deviation = distance_to(contour, Point2::new(5.0, up(5.0)));
  assert!(
    deviation <= DEFAULT_TOLERANCE,
    "deviation {deviation} exceeds the tolerance {DEFAULT_TOLERANCE}"
  );
}

#[test]
fn shorthand_curves_flatten_too_and_count_each_curve() {
  let imported = import_document(r#"<path d="M 0 0 C 0 10 10 10 10 0 S 20 -10 20 0"/>"#);
  let contour = only(&imported);

  assert_eq!(imported.report.flattened_curves, 2, "C and S, one each");
  let end = contour.vertices().last().expect("vertices").point;
  assert_point(end, Point2::new(20.0, up(0.0)), "curve end point");
}

#[test]
fn a_group_transform_moves_and_scales_its_child() {
  let imported = import_document(
    r#"<g transform="translate(10,20) scale(2)"><rect width="10" height="5"/></g>"#,
  );
  let contour = only(&imported);

  assert_bounds(contour, (10.0, up(30.0)), (30.0, up(20.0)));
}

#[test]
fn nested_groups_compose_their_transforms() {
  let imported = import_document(
    r#"<g transform="translate(10,20)"><g transform="scale(2)"><rect width="10" height="5"/></g></g>"#,
  );

  assert_bounds(only(&imported), (10.0, up(30.0)), (30.0, up(20.0)));
}

#[test]
fn a_matrix_transform_on_the_shape_itself_is_honoured() {
  let imported =
    import_document(r#"<rect transform="matrix(1 0 0 1 5 7)" width="10" height="5"/>"#);

  assert_bounds(only(&imported), (5.0, up(12.0)), (15.0, up(7.0)));
}

#[test]
fn the_viewbox_scales_the_drawing_to_millimetres() {
  let imported = import_source(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="50mm" height="50mm" viewBox="0 0 100 100"><rect width="100" height="100"/></svg>"#,
  );

  assert_near(imported.report.unit_scale, 0.5, "unit scale");
  assert_bounds(only(&imported), (0.0, 0.0), (50.0, 50.0));
}

#[test]
fn preserve_aspect_ratio_meet_scales_by_the_smaller_factor_and_centres() {
  let imported = import_source(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="100mm" viewBox="0 0 100 50"><rect width="100" height="50"/></svg>"#,
  );

  assert_near(imported.report.unit_scale, 1.0, "unit scale");
  assert_bounds(only(&imported), (0.0, 25.0), (100.0, 75.0));
}

#[test]
fn a_unitless_width_is_px_at_ninety_six_dpi() {
  let imported = import_source(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="96" height="96"><rect width="96" height="96"/></svg>"#,
  );

  assert_near(imported.report.unit_scale, 25.4 / 96.0, "unit scale");
  let bounds = only(&imported).bounds();
  assert!(
    (bounds.width() - 25.4).abs() < 1e-9,
    "96 px is 25.4 mm, got {}",
    bounds.width()
  );
}

#[test]
fn a_viewbox_without_a_size_is_taken_as_millimetres_and_says_so() {
  let imported = import_source(
    r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 20"><rect x="10" y="5" width="10" height="10"/></svg>"#,
  );

  assert_near(imported.report.unit_scale, 1.0, "unit scale");
  assert_bounds(only(&imported), (10.0, 5.0), (20.0, 15.0));
  assert!(
    imported
      .report
      .warnings
      .iter()
      .any(|warning| warning.contains("millimetres")),
    "the report must say the viewBox was read as millimetres: {:?}",
    imported.report.warnings
  );
}

#[test]
fn comments_processing_instructions_doctype_and_entities_are_tolerated() {
  let source = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
    "<!DOCTYPE svg PUBLIC \"-//W3C//DTD SVG 1.1//EN\" ",
    "\"http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd\">\n",
    "<!-- a comment with <tags> and -- inside -->\n",
    "<svg xmlns='http://www.w3.org/2000/svg' width='100mm' height='100mm' viewBox='0 0 100 100'>\n",
    "  <desc><![CDATA[not geometry: <rect/> &amp; more]]></desc>\n",
    "  <rect id='a&amp;b&#65;' x='10' y='20' width='30' height='40'/>\n",
    "</svg>\n",
  );
  let imported = import_source(source);

  assert_bounds(only(&imported), (10.0, up(60.0)), (40.0, up(20.0)));
  assert!(
    imported
      .report
      .skipped
      .iter()
      .any(|skipped| skipped.construct == "<desc>"),
    "the description is skipped and reported"
  );
}

#[test]
fn a_mismatched_tag_is_refused_with_its_line() {
  let source = concat!(
    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"100mm\" height=\"100mm\" ",
    "viewBox=\"0 0 100 100\">\n",
    "  <g>\n",
    "    <rect width=\"10\" height=\"10\"/>\n",
    "  </svg>\n",
    "</svg>\n",
  );
  let error =
    import_bytes("broken.svg", source.as_bytes(), &ImportOptions::default()).expect_err("refused");

  match error {
    ImportError::Parse {
      format,
      message,
      location,
    } => {
      assert_eq!(format, "svg");
      assert_eq!(location.as_deref(), Some("line 4"));
      assert!(
        message.contains("</svg>") && message.contains("<g>"),
        "the message names both tags: {message}"
      );
    }
    other => panic!("unexpected error: {other:?}"),
  }
}

#[test]
fn an_unclosed_tag_is_refused_with_the_line_it_was_opened_on() {
  let source = concat!(
    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"100mm\" height=\"100mm\" ",
    "viewBox=\"0 0 100 100\">\n",
    "  <g>\n",
  );
  let error =
    import_bytes("broken.svg", source.as_bytes(), &ImportOptions::default()).expect_err("refused");

  match error {
    ImportError::Parse {
      message, location, ..
    } => {
      assert_eq!(location.as_deref(), Some("line 2"));
      assert!(message.contains("<g>"), "{message}");
    }
    other => panic!("unexpected error: {other:?}"),
  }
}

#[test]
fn y_is_inverted_because_svg_user_space_is_y_down() {
  // D-M3-10: the apex is at y=10, the top of the page in SVG, so after the flip it must be the
  // *highest* millimetre y and the base at y=30 must end up below it. A reflection, not a
  // translation: the drawing is not merely moved.
  let imported = import_document(r#"<path d="M 10 10 L 20 30 L 0 30 Z"/>"#);
  let contour = only(&imported);

  assert_bounds(contour, (0.0, up(30.0)), (20.0, up(10.0)));
  assert_near(
    contour.signed_area(),
    200.0,
    "the flip does not lose the area",
  );

  // The reflection is about the page's own height, never a constant.
  let small = import_source(concat!(
    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"50mm\" height=\"50mm\" ",
    "viewBox=\"0 0 50 50\"><rect x=\"0\" y=\"0\" width=\"10\" height=\"10\"/></svg>"
  ));
  assert_bounds(only(&small), (0.0, 40.0), (10.0, 50.0));

  // And it is an isometry: arcs stay arcs, with their turn direction reversed.
  let circle = import_document(r#"<circle cx="50" cy="50" r="5"/>"#);
  assert_eq!(
    arc_count(only(&circle)),
    2,
    "the flip keeps both arcs exact"
  );
  assert_eq!(circle.report.flattened_curves, 0);
}

#[test]
fn text_is_skipped_and_counted_in_the_report() {
  let imported = import_document(
    r#"<text x="0" y="0">not cut geometry</text><rect x="0" y="0" width="10" height="10"/>"#,
  );

  assert_eq!(imported.report.contours, 1);
  assert_eq!(imported.report.skipped_count(), 1);
  let skipped = &imported.report.skipped[0];
  assert_eq!(skipped.construct, "<text>");
  assert_eq!(skipped.location.as_deref(), Some("element <text>"));
  assert!(
    skipped.reason.contains("text"),
    "the reason names the feature: {}",
    skipped.reason
  );
  assert!(
    imported
      .report
      .render()
      .contains("  - <text> at element <text>:"),
    "the rendered report names the skipped element"
  );
}

#[test]
fn a_rounded_rect_keeps_four_quarter_arcs() {
  let imported = import_document(r#"<rect x="0" y="0" width="20" height="10" rx="5"/>"#);
  let contour = only(&imported);

  assert!(contour.is_closed());
  assert_eq!(arc_count(contour), 4);
  assert_eq!(imported.report.flattened_curves, 0);
  assert_bounds(contour, (0.0, up(10.0)), (20.0, up(0.0)));
  assert_near(
    contour.perimeter(),
    20.0 + TAU * 5.0,
    "rounded rect perimeter",
  );
  assert_near(
    contour.signed_area(),
    200.0 - (4.0 - PI) * 25.0,
    "rounded rect area",
  );
}

#[test]
fn an_elliptical_corner_is_flattened_and_counted() {
  let imported = import_document(r#"<rect x="0" y="0" width="20" height="10" rx="6" ry="3"/>"#);
  let contour = only(&imported);

  assert!(!contour.has_arcs(), "an elliptical corner is not an arc");
  assert_eq!(imported.report.flattened_curves, 4);
  assert_bounds(contour, (0.0, up(10.0)), (20.0, up(0.0)));
}

#[test]
fn relative_and_implicit_path_commands_follow_the_grammar() {
  let relative = import_document(r#"<path d="m 10 10 h 20 v 10 h -20 z"/>"#);
  let contour = only(&relative);
  assert!(contour.is_closed());
  assert_bounds(contour, (10.0, up(20.0)), (30.0, up(10.0)));
  assert_near(contour.signed_area(), 200.0, "relative rect area");

  let implicit = import_document(r#"<path d="M 0 0 L 10 0 20 0 20 10 0 10 Z"/>"#);
  let contour = only(&implicit);
  assert!(contour.is_closed());
  assert_bounds(contour, (0.0, up(10.0)), (20.0, up(0.0)));
  assert_near(contour.signed_area(), 200.0, "implicit lineto area");
}

#[test]
fn every_shape_element_is_counted_in_the_report() {
  let imported = import_document(concat!(
    r#"<rect x="0" y="0" width="10" height="10"/>"#,
    r#"<circle cx="50" cy="50" r="5"/>"#,
    r#"<ellipse cx="20" cy="20" rx="10" ry="5"/>"#,
    r#"<line x1="0" y1="0" x2="10" y2="10"/>"#,
    r#"<polyline points="0,0 10,0"/>"#,
    r#"<polygon points="0,0 10,0 5,5"/>"#,
    r#"<path d="M 0 0 L 5 5"/>"#,
  ));

  for name in [
    "<rect>",
    "<circle>",
    "<ellipse>",
    "<line>",
    "<polyline>",
    "<polygon>",
    "<path>",
  ] {
    assert_eq!(
      imported.report.sources.get(name),
      Some(&1),
      "the report counts {name}"
    );
  }
  assert_eq!(imported.report.contours, 7);
  assert_eq!(imported.report.skipped_count(), 0);
}

#[test]
fn a_shape_with_fill_none_and_no_stroke_warns_but_is_still_imported() {
  let imported = import_document(r#"<rect width="10" height="10" fill="none"/>"#);

  assert_eq!(only(&imported).len(), 4, "the boundary is imported");
  assert!(
    imported
      .report
      .warnings
      .iter()
      .any(|warning| warning.contains("fill=\"none\"")),
    "D-M3-7 imports the boundary and says so: {:?}",
    imported.report.warnings
  );
}

#[test]
fn a_path_that_draws_nothing_is_reported_not_dropped() {
  let imported = import_document(r#"<path d="M 10 10"/><rect width="5" height="5"/>"#);

  assert_eq!(imported.report.contours, 1);
  let skipped = imported
    .report
    .skipped
    .iter()
    .find(|skipped| skipped.construct == "<path>")
    .expect("the empty path is reported");
  assert_eq!(skipped.location.as_deref(), Some("element <path>"));
}
