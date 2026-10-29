//! Behaviour tests for the EPS importer: the PostScript path subset (M3 A11).
//!
//! Every test drives the public entry point with a byte literal and asserts observable geometry
//! (bounds, area, perimeter, arc count and sweep, contour count) and the report. Nothing depends on
//! the working directory, the network or a file on disk.

use std::f64::consts::{FRAC_PI_2, PI};

use cam_geom::{Contour, Point2, Segment};
use cam_import::{eps, import_bytes, ImportError, ImportOptions, ImportReport, Imported};

/// PostScript's 1/72 inch in millimetres, as the importer documents it.
const MM_PER_POINT: f64 = 25.4 / 72.0;

/// The header every fixture below carries.
const HEADER: &str = "%!PS-Adobe-3.0 EPSF-3.0\n";

fn options() -> ImportOptions {
  ImportOptions::default()
}

/// Import `body` behind the standard EPS header.
fn import(body: &str) -> Imported {
  import_text(&format!("{HEADER}{body}"))
}

/// Import a whole file as text.
fn import_text(text: &str) -> Imported {
  import_bytes("plot.eps", text.as_bytes(), &options()).expect("eps import")
}

/// The single layer's contours, which every EPS fixture produces.
fn contours(imported: &Imported) -> &[Contour] {
  assert_eq!(
    imported.layers.len(),
    1,
    "an EPS has no layers, so it imports as one"
  );
  assert_eq!(imported.layers[0].name, "eps");
  &imported.layers[0].contours
}

/// User-space points to millimetres.
fn millimetres(points: f64) -> f64 {
  points * MM_PER_POINT
}

fn assert_near(actual: f64, expected: f64, label: &str) {
  let slack = 1.0e-9 * expected.abs().max(1.0);
  assert!(
    (actual - expected).abs() <= slack,
    "{label}: {actual} is not within {slack} of {expected}"
  );
}

/// The two arcs of a circle contour.
fn arcs(contour: &Contour) -> Vec<cam_geom::Arc> {
  contour
    .segments()
    .into_iter()
    .filter_map(|segment| segment.as_arc())
    .collect()
}

#[test]
fn a_stroked_triangle_is_one_closed_contour_with_exact_bounds() {
  let imported = import("10 10 moveto 60 10 lineto 60 60 lineto closepath stroke\n");
  let contours = contours(&imported);
  assert_eq!(contours.len(), 1);
  let contour = &contours[0];
  assert!(contour.is_closed());
  assert_eq!(contour.len(), 3);
  assert!(!contour.has_arcs());

  let bounds = contour.bounds();
  assert_near(bounds.min.x, millimetres(10.0), "min x");
  assert_near(bounds.min.y, millimetres(10.0), "min y");
  assert_near(bounds.max.x, millimetres(60.0), "max x");
  assert_near(bounds.max.y, millimetres(60.0), "max y");

  let report = &imported.report;
  assert_eq!(report.contours, 1);
  assert_eq!(report.sources.get("moveto"), Some(&1));
  assert_eq!(report.sources.get("lineto"), Some(&2));
  assert_eq!(report.sources.get("closepath"), Some(&1));
  assert_eq!(report.sources.get("stroke"), Some(&1));
  assert_eq!(report.flattened_curves, 0);
  assert!(report.skipped.is_empty(), "skipped: {:?}", report.skipped);
}

#[test]
fn the_report_states_the_postscript_unit_scale() {
  let imported = import("0 0 moveto 10 0 lineto stroke\n");
  assert_near(imported.report.unit_scale, 25.4 / 72.0, "unit scale");
  assert!(
    imported
      .report
      .warnings
      .iter()
      .any(|warning| warning.contains("1/72 inch")),
    "warnings: {:?}",
    imported.report.warnings
  );
  assert!(
    imported
      .report
      .warnings
      .iter()
      .any(|warning| warning.contains("%%BoundingBox")),
    "a file without a bounding box says so: {:?}",
    imported.report.warnings
  );
}

#[test]
fn relative_lineto_matches_the_absolute_form() {
  let absolute = import("10 10 moveto 60 10 lineto 60 60 lineto 10 60 lineto closepath stroke\n");
  let relative = import("10 10 moveto 50 0 rlineto 0 50 rlineto -50 0 rlineto closepath stroke\n");
  let (absolute, relative) = (&contours(&absolute)[0], &contours(&relative)[0]);

  assert_eq!(relative.len(), absolute.len());
  let (expected, actual) = (absolute.bounds(), relative.bounds());
  assert_near(actual.min.x, expected.min.x, "min x");
  assert_near(actual.min.y, expected.min.y, "min y");
  assert_near(actual.max.x, expected.max.x, "max x");
  assert_near(actual.max.y, expected.max.y, "max y");
  assert_near(relative.perimeter(), absolute.perimeter(), "perimeter");
}

#[test]
fn translate_and_scale_place_a_unit_square_in_millimetres() {
  let imported = import(
    "10 20 translate 2 2 scale 0 0 moveto 1 0 lineto 1 1 lineto 0 1 lineto closepath stroke\n",
  );
  let contour = &contours(&imported)[0];
  let bounds = contour.bounds();
  // `10 20 translate 2 2 scale` scales first and then translates: the unit square lands at
  // (10, 20) with sides of 2 user units.
  assert_near(bounds.min.x, millimetres(10.0), "min x");
  assert_near(bounds.min.y, millimetres(20.0), "min y");
  assert_near(bounds.width(), millimetres(2.0), "width");
  assert_near(bounds.height(), millimetres(2.0), "height");
  assert_near(imported.report.unit_scale, 25.4 / 72.0, "unit scale");
}

#[test]
fn a_full_turn_arc_is_a_circle_made_of_two_arcs() {
  let imported = import("0 0 10 0 360 arc fill\n");
  let contours = contours(&imported);
  assert_eq!(contours.len(), 1);
  let contour = &contours[0];
  assert!(contour.is_closed());
  assert_eq!(contour.len(), 2, "a full turn is two half-turn arcs");
  assert!(contour.has_arcs());

  let radius = millimetres(10.0);
  for arc in arcs(contour) {
    assert_near(arc.radius(), radius, "radius");
    assert_near(arc.sweep().abs(), PI, "half-turn sweep");
    assert_near(arc.center().x, 0.0, "centre x");
    assert_near(arc.center().y, 0.0, "centre y");
  }
  assert_near(contour.perimeter(), 2.0 * PI * radius, "perimeter");
  assert_near(contour.signed_area(), PI * radius * radius, "area");
  assert!(
    contour.is_ccw(),
    "closed contours come out counter-clockwise"
  );

  assert_eq!(imported.report.flattened_curves, 0);
  assert_eq!(imported.report.sources.get("arc"), Some(&1));
  assert_eq!(imported.report.sources.get("fill"), Some(&1));
  assert!(imported.report.skipped.is_empty());
}

#[test]
fn a_partial_arc_keeps_its_centre_radius_and_sweep() {
  let imported = import("0 0 10 0 90 arc stroke\n");
  let contour = &contours(&imported)[0];
  assert!(!contour.is_closed(), "stroke leaves the path open");
  let segments = contour.segments();
  assert_eq!(segments.len(), 1, "one arc, not a polyline");
  let Segment::Arc(arc) = segments[0] else {
    panic!("expected an arc, got {:?}", segments[0]);
  };
  assert_near(arc.center().x, 0.0, "centre x");
  assert_near(arc.center().y, 0.0, "centre y");
  assert_near(arc.radius(), millimetres(10.0), "radius");
  assert_near(arc.start_angle(), 0.0, "start angle");
  assert_near(arc.sweep(), FRAC_PI_2, "sweep");
  assert_near(arc.start_point().x, millimetres(10.0), "start x");
  assert_near(arc.end_point().y, millimetres(10.0), "end y");
  assert_eq!(imported.report.flattened_curves, 0);
}

#[test]
fn arc_joins_the_current_point_with_a_straight_segment() {
  // Teleporting to the arc's start would lose the leg the file draws to get there. The circle
  // becomes its own closed subpath, because a closed contour cannot also carry a leg that does not
  // come back to the circle's start.
  let imported = import("0 0 moveto 0 0 10 0 360 arc stroke\n");
  let contours = contours(&imported);
  assert_eq!(contours.len(), 2);
  let leg = &contours[0];
  assert!(!leg.is_closed());
  assert_eq!(leg.len(), 2);
  assert!(matches!(leg.segments()[0], Segment::Line(_)));
  assert_near(
    leg.bounds().max.x,
    millimetres(10.0),
    "the leg reaches the arc start",
  );
  assert!(contours[1].is_closed());
  assert_eq!(arcs(&contours[1]).len(), 2);
}

#[test]
fn curveto_is_flattened_within_the_tolerance_and_counted() {
  let imported = import("10 10 moveto 20 40 40 40 50 10 curveto stroke\n");
  assert_eq!(imported.report.flattened_curves, 1);
  assert_eq!(imported.report.sources.get("curveto"), Some(&1));

  let contour = &contours(&imported)[0];
  assert!(!contour.is_closed());
  assert!(!contour.has_arcs(), "a cubic is never an arc");
  assert!(
    contour.len() > 4,
    "the curve should flatten into several segments, got {}",
    contour.len()
  );

  // The analytic Bézier at t = 0.5, and a few other samples, must be within the tolerance of the
  // flattened polyline.
  let start = Point2::new(millimetres(10.0), millimetres(10.0));
  let first = Point2::new(millimetres(20.0), millimetres(40.0));
  let second = Point2::new(millimetres(40.0), millimetres(40.0));
  let end = Point2::new(millimetres(50.0), millimetres(10.0));
  let tolerance = imported.report.tolerance;
  for step in 0..=10 {
    let t = f64::from(step) / 10.0;
    let point = bezier(start, first, second, end, t);
    let deviation = contour
      .segments()
      .iter()
      .map(|segment| segment.distance_to_point(point))
      .fold(f64::INFINITY, f64::min);
    assert!(
      deviation <= tolerance,
      "at t = {t} the polyline is {deviation} mm from the curve, tolerance {tolerance}"
    );
  }
}

/// A point on the cubic Bézier through the four control points.
fn bezier(start: Point2, first: Point2, second: Point2, end: Point2, t: f64) -> Point2 {
  let inverse = 1.0 - t;
  let weights = [
    inverse * inverse * inverse,
    3.0 * inverse * inverse * t,
    3.0 * inverse * t * t,
    t * t * t,
  ];
  Point2::new(
    start.x * weights[0] + first.x * weights[1] + second.x * weights[2] + end.x * weights[3],
    start.y * weights[0] + first.y * weights[1] + second.y * weights[2] + end.y * weights[3],
  )
}

#[test]
fn rcurveto_matches_the_absolute_form() {
  let absolute = import("10 10 moveto 20 40 40 40 50 10 curveto stroke\n");
  let relative = import("10 10 moveto 10 30 30 30 40 0 rcurveto stroke\n");
  let (absolute, relative) = (&contours(&absolute)[0], &contours(&relative)[0]);
  assert_eq!(relative.len(), absolute.len());
  let (expected, actual) = (absolute.bounds(), relative.bounds());
  assert_near(actual.min.x, expected.min.x, "min x");
  assert_near(actual.max.y, expected.max.y, "max y");
  assert_near(relative.perimeter(), absolute.perimeter(), "perimeter");
}

#[test]
fn the_bounding_box_puts_the_lower_left_corner_at_the_origin() {
  let imported = import_text(&format!(
    "{HEADER}%%BoundingBox: 100 100 300 300\n\
     100 100 moveto 300 100 lineto 300 300 lineto 100 300 lineto closepath stroke\n"
  ));
  let bounds = contours(&imported)[0].bounds();
  assert_near(bounds.min.x, 0.0, "min x");
  assert_near(bounds.min.y, 0.0, "min y");
  assert_near(bounds.max.x, millimetres(200.0), "max x");
  assert_near(bounds.max.y, millimetres(200.0), "max y");
}

#[test]
fn the_hires_bounding_box_wins_over_the_coarse_one() {
  let imported = import_text(&format!(
    "{HEADER}%%BoundingBox: 0 0 612 792\n%%HiResBoundingBox: 100.5 100.5 300 300\n\
     100.5 100.5 moveto 300 100.5 lineto 300 300 lineto 100.5 300 lineto closepath stroke\n"
  ));
  let bounds = contours(&imported)[0].bounds();
  assert_near(bounds.min.x, 0.0, "min x");
  assert_near(bounds.min.y, 0.0, "min y");
}

#[test]
fn gsave_and_grestore_restore_the_matrix() {
  let body = "0 0 moveto 10 0 lineto 10 10 lineto 0 10 lineto closepath stroke\n\
              gsave 100 100 translate 0 0 moveto 10 0 lineto 10 10 lineto 0 10 lineto closepath \
              stroke grestore\n\
              0 0 moveto 10 0 lineto 10 10 lineto 0 10 lineto closepath stroke\n";
  let imported = import(body);
  let contours = contours(&imported);
  assert_eq!(contours.len(), 3);
  assert_near(contours[1].bounds().min.x, millimetres(100.0), "moved x");
  assert_near(contours[1].bounds().min.y, millimetres(100.0), "moved y");
  assert_near(
    contours[2].bounds().min.x,
    contours[0].bounds().min.x,
    "the matrix came back",
  );
  assert_near(
    contours[2].bounds().min.y,
    contours[0].bounds().min.y,
    "min y",
  );
}

#[test]
fn setmatrix_replaces_the_matrix_absolutely() {
  let imported = import(
    "100 100 translate 1 0 0 1 0 0 setmatrix 0 0 moveto 10 0 lineto 10 10 lineto 0 10 lineto \
     closepath stroke\n",
  );
  let bounds = contours(&imported)[0].bounds();
  assert_near(bounds.min.x, 0.0, "min x");
  assert_near(bounds.min.y, 0.0, "min y");
  assert_near(bounds.max.x, millimetres(10.0), "max x");
}

#[test]
fn concat_applies_the_given_matrix() {
  let imported =
    import("3 0 0 3 0 0 concat 0 0 moveto 10 0 lineto 10 10 lineto 0 10 lineto closepath stroke\n");
  let bounds = contours(&imported)[0].bounds();
  assert_near(bounds.width(), millimetres(30.0), "width");
  assert_near(bounds.height(), millimetres(30.0), "height");
}

#[test]
fn a_reflected_matrix_keeps_the_circle_exact() {
  // A reflection has a negative determinant: the arc's direction flips, but it is still a circle.
  let imported = import("-1 1 scale 0 0 10 0 360 arc fill\n");
  let contour = &contours(&imported)[0];
  assert!(contour.is_closed());
  assert_eq!(contour.len(), 2);
  assert!(contour.has_arcs());
  assert!(contour.is_ccw());
  assert_eq!(imported.report.flattened_curves, 0);
  let radius = millimetres(10.0);
  assert_near(contour.signed_area(), PI * radius * radius, "area");
}

#[test]
fn a_non_uniform_scale_flattens_arcs_and_counts_them() {
  let imported = import("2 1 scale 0 0 10 0 360 arc fill\n");
  let contour = &contours(&imported)[0];
  assert!(!contour.has_arcs(), "an ellipse is not a circular arc");
  assert_eq!(imported.report.flattened_curves, 1);
  let bounds = contour.bounds();
  assert!(
    (bounds.width() - millimetres(40.0)).abs() < 0.05,
    "width {} is not 40 points wide",
    bounds.width()
  );
  assert!(
    (bounds.height() - millimetres(20.0)).abs() < 0.05,
    "height {} is not 20 points high",
    bounds.height()
  );
}

#[test]
fn clip_discards_the_current_path() {
  let imported = import(
    "0 0 moveto 10 0 lineto 10 10 lineto closepath clip\n\
     newpath 20 20 moveto 30 20 lineto 30 30 lineto closepath stroke\n",
  );
  let contours = contours(&imported);
  assert_eq!(contours.len(), 1);
  assert_near(contours[0].bounds().min.x, millimetres(20.0), "min x");
  assert_eq!(imported.report.sources.get("clip"), Some(&1));
}

#[test]
fn text_and_image_operators_are_skipped_with_their_names() {
  let imported = import(
    "72 700 moveto (Hello) show\n\
     8 8 8 [8 0 0 8 0 0] {<00>} image\n\
     0 0 moveto 10 0 lineto 10 10 lineto closepath stroke\n",
  );
  let mut skipped: Vec<(&str, &str)> = imported
    .report
    .skipped
    .iter()
    .map(|skipped| (skipped.construct.as_str(), skipped.reason.as_str()))
    .collect();
  skipped.sort_unstable();
  assert_eq!(
    skipped,
    vec![
      ("image", "raster images are not vector geometry"),
      ("show", "text is not vector geometry"),
    ]
  );
  assert_eq!(
    imported.report.skipped[0].location.as_deref(),
    Some("line 3"),
    "the first use of the first construct in name order is located"
  );
  assert_eq!(contours(&imported).len(), 1, "the path still imports");
}

#[test]
fn an_unknown_operator_is_counted_not_fatal() {
  let imported = import(
    "1 2 frobnicate\n3 4 frobnicate\n0 0 moveto 10 0 lineto 10 10 lineto closepath stroke\n",
  );
  let skipped = imported
    .report
    .skipped
    .iter()
    .find(|skipped| skipped.construct == "frobnicate")
    .expect("the unknown operator is reported");
  assert_eq!(
    skipped.reason,
    "the operator is outside the imported PostScript path subset (2 occurrences)"
  );
  assert_eq!(skipped.location.as_deref(), Some("line 2"));
  assert_eq!(contours(&imported).len(), 1);
}

#[test]
fn style_and_dictionary_operators_are_ignored_without_noise() {
  let imported = import(
    "/DeviceRGB setcolorspace 0 0 0 setrgbcolor 0.5 setlinewidth 1 setlinecap [2 2] 0 setdash\n\
     /x 1 def 0 0 moveto 10 0 lineto 10 10 lineto closepath stroke\n",
  );
  assert!(
    imported.report.skipped.is_empty(),
    "skipped: {:?}",
    imported.report.skipped
  );
  assert_eq!(contours(&imported).len(), 1);
}

#[test]
fn a_path_that_is_never_painted_is_reported_not_imported() {
  let text = format!("{HEADER}0 0 moveto 10 0 lineto\n");
  let mut report = ImportReport::new("eps", options().tolerance);
  let layers = eps::import(text.as_bytes(), &options(), &mut report).expect("the file parses");
  assert!(layers.is_empty(), "an unpainted path is not geometry");
  assert_eq!(report.skipped.len(), 1);
  assert_eq!(report.skipped[0].construct, "path");
  assert_eq!(
    report.skipped[0].reason,
    "the path was never stroked, filled or clipped"
  );
  assert_eq!(report.skipped[0].location.as_deref(), Some("line 2"));

  let error = import_bytes("open.eps", text.as_bytes(), &options()).expect_err("nothing imported");
  assert!(
    matches!(error, ImportError::NothingImported { format, .. } if format == "eps"),
    "{error:?}"
  );
}

#[test]
fn a_header_with_no_path_reports_nothing_imported() {
  // The module returns no geometry and the spine names the failure, so every format reports an
  // empty drawing the same way instead of each inventing its own error.
  let error =
    import_bytes("empty.eps", b"%!PS-Adobe-3.0\n", &options()).expect_err("nothing drawn");
  assert_eq!(
    error,
    ImportError::NothingImported {
      format: "eps",
      reason: "the file contains no vector geometry".to_string()
    }
  );
}

#[test]
fn a_file_that_is_not_postscript_is_refused_by_name() {
  for text in ["%PDF-1.7\n", "just words\n", "<svg/>\n"] {
    let error = import_bytes("plot.eps", text.as_bytes(), &options()).expect_err("refused");
    assert_eq!(
      error,
      ImportError::refused("eps", "not a PostScript file"),
      "{text:?}"
    );
    assert_eq!(error.to_string(), "eps refused: not a PostScript file");
  }
}

#[test]
fn a_postscript_header_without_the_adobe_comment_is_still_postscript() {
  let imported = import_text("%!PS\n0 0 moveto 10 0 lineto closepath stroke\n");
  assert_eq!(contours(&imported).len(), 1);
}

#[test]
fn comments_and_strings_do_not_reach_the_geometry() {
  let imported = import(
    "% a comment mentioning moveto 0 0 lineto\n\
     (a string with a stroke in it) pop\n\
     0 0 moveto 10 0 lineto 10 10 lineto closepath stroke\n",
  );
  let contours = contours(&imported);
  assert_eq!(contours.len(), 1);
  assert_near(contours[0].bounds().max.x, millimetres(10.0), "max x");
  assert!(imported.report.skipped.is_empty());
}

#[test]
fn a_closepath_without_a_path_is_harmless() {
  let imported = import("closepath 0 0 moveto 10 0 lineto closepath stroke\n");
  let contours = contours(&imported);
  assert_eq!(contours.len(), 1);
  assert!(contours[0].is_closed());
}
