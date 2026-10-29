//! Behaviour tests for the PDF importer: tolerant object and content-stream reading, the transform
//! and the page origin, and the constructs that cannot become cut geometry (M3 A9, A10).

use std::collections::BTreeMap;
use std::io::Write;

use cam_geom::{Contour, Point2};
use cam_import::{import_bytes, ImportError, ImportOptions, Imported};
use flate2::write::ZlibEncoder;
use flate2::Compression;

/// A PDF assembled from explicitly numbered indirect objects, written in object-number order.
///
/// There is deliberately no cross-reference table: the importer must recover the objects by
/// scanning, which is what A9 asks for.
#[derive(Default)]
struct Pdf {
  bodies: BTreeMap<u32, Vec<u8>>,
}

impl Pdf {
  fn new() -> Self {
    Self::default()
  }

  /// A plain dictionary object.
  fn dict(&mut self, number: u32, text: &str) {
    self
      .bodies
      .insert(number, format!("<< {text} >>").into_bytes());
  }

  /// A stream object whose `/Length` is the real length.
  fn stream(&mut self, number: u32, extra: &str, data: &[u8]) {
    self.stream_declaring(number, extra, data.len(), data);
  }

  /// A stream object whose `/Length` may be a lie, to prove the scan recovers from it.
  fn stream_declaring(&mut self, number: u32, extra: &str, declared: usize, data: &[u8]) {
    let head = if extra.is_empty() {
      format!("<< /Length {declared} >>\nstream\n")
    } else {
      format!("<< {extra} /Length {declared} >>\nstream\n")
    };
    let mut body = head.into_bytes();
    body.extend_from_slice(data);
    body.extend_from_slice(b"\nendstream");
    self.bodies.insert(number, body);
  }

  fn build(&self, root: u32, trailer_extra: &str) -> Vec<u8> {
    let mut out = b"%PDF-1.7\n".to_vec();
    for (number, body) in &self.bodies {
      out.extend_from_slice(format!("{number} 0 obj\n").as_bytes());
      out.extend_from_slice(body);
      out.extend_from_slice(b"\nendobj\n");
    }
    out.extend_from_slice(
      format!(
        "trailer\n<< /Size {} /Root {root} 0 R {trailer_extra}>>\n%%EOF\n",
        self.bodies.len() + 1
      )
      .as_bytes(),
    );
    out
  }
}

fn deflate(data: &[u8]) -> Vec<u8> {
  let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
  encoder.write_all(data).expect("encode");
  encoder.finish().expect("finish")
}

const RECTANGLE: &str = "0 0 m 100 0 l 100 50 l 0 50 l h S";

/// A one-page PDF: object 1 is the content stream, 2 the page, 3 the page tree, 4 the catalog.
fn single_page(media_box: &str, content: &[u8], compressed: bool, resources: &str) -> Vec<u8> {
  let mut pdf = Pdf::new();
  if compressed {
    pdf.stream(1, "/Filter /FlateDecode", &deflate(content));
  } else {
    pdf.stream(1, "", content);
  }
  pdf.dict(
    2,
    &format!(
      "/Type /Page /Parent 3 0 R /MediaBox {media_box} /Resources << {resources} >> /Contents 1 0 R"
    ),
  );
  pdf.dict(3, "/Type /Pages /Kids [2 0 R] /Count 1");
  pdf.dict(4, "/Type /Catalog /Pages 3 0 R");
  pdf.build(4, "")
}

fn import(bytes: &[u8]) -> Imported {
  import_bytes("sheet.pdf", bytes, &ImportOptions::default()).expect("import")
}

fn import_with_tolerance(bytes: &[u8], tolerance: f64) -> Imported {
  let options = ImportOptions {
    tolerance,
    ..ImportOptions::default()
  };
  import_bytes("sheet.pdf", bytes, &options).expect("import")
}

fn assert_close(actual: f64, expected: f64) {
  assert!(
    (actual - expected).abs() < 1.0e-9,
    "expected {expected}, got {actual}"
  );
}

fn assert_bounds(contour: &Contour, min: Point2, max: Point2) {
  let bounds = contour.bounds();
  assert_close(bounds.min.x, min.x);
  assert_close(bounds.min.y, min.y);
  assert_close(bounds.max.x, max.x);
  assert_close(bounds.max.y, max.y);
}

/// The point of the cubic Bézier `p0 p1 p2 p3` at `t`.
fn cubic_at(p0: Point2, p1: Point2, p2: Point2, p3: Point2, t: f64) -> Point2 {
  let a = p0.lerp(p1, t);
  let b = p1.lerp(p2, t);
  let c = p2.lerp(p3, t);
  let d = a.lerp(b, t);
  let e = b.lerp(c, t);
  d.lerp(e, t)
}

/// The distance from `point` to the polyline that makes up `contour`.
fn distance_to_contour(contour: &Contour, point: Point2) -> f64 {
  contour
    .segments()
    .iter()
    .map(|segment| segment.distance_to_point(point))
    .fold(f64::INFINITY, f64::min)
}

#[test]
fn an_uncompressed_content_stream_imports_a_closed_rectangle() {
  let imported = import(&single_page(
    "[0 0 595 842]",
    RECTANGLE.as_bytes(),
    false,
    "",
  ));

  assert_eq!(imported.layers.len(), 1);
  assert_eq!(imported.layers[0].name, "page 1");
  let contours = &imported.layers[0].contours;
  assert_eq!(contours.len(), 1);
  let contour = &contours[0];
  assert!(contour.is_closed());
  assert!(!contour.has_arcs());
  assert_eq!(contour.len(), 4);
  assert_bounds(contour, Point2::ORIGIN, Point2::new(100.0, 50.0));
  assert_close(contour.perimeter(), 300.0);
  assert_close(contour.signed_area(), 5000.0);

  let report = &imported.report;
  assert_eq!(report.contours, 1);
  assert_eq!(report.flattened_curves, 0);
  assert_eq!(report.unit_scale, 1.0);
  assert!(report.skipped.is_empty());
  assert_eq!(report.sources.get("m"), Some(&1));
  assert_eq!(report.sources.get("l"), Some(&3));
  assert_eq!(report.sources.get("S"), Some(&1));
  assert_eq!(
    report.warnings,
    vec![
      "PDF user units carry no physical size; one unit is imported as one millimetre".to_string()
    ]
  );
}

#[test]
fn a_flate_compressed_content_stream_imports_the_same_contours() {
  let plain = import(&single_page(
    "[0 0 595 842]",
    RECTANGLE.as_bytes(),
    false,
    "",
  ));
  let compressed = import(&single_page(
    "[0 0 595 842]",
    RECTANGLE.as_bytes(),
    true,
    "",
  ));

  assert_eq!(compressed.layers, plain.layers);
  assert_eq!(compressed.report.sources, plain.report.sources);
  assert_eq!(
    compressed.report.flattened_curves,
    plain.report.flattened_curves
  );
  assert_eq!(compressed.report.skipped, plain.report.skipped);
  assert_eq!(compressed.report.unit_scale, plain.report.unit_scale);
}

#[test]
fn the_content_transform_matrix_scales_and_translates_the_geometry() {
  let content = "2 0 0 2 10 20 cm 0 0 m 100 0 l 100 50 l 0 50 l h S";
  let imported = import(&single_page("[0 0 595 842]", content.as_bytes(), false, ""));

  let contour = &imported.layers[0].contours[0];
  assert_bounds(contour, Point2::new(10.0, 20.0), Point2::new(210.0, 120.0));
  assert_close(contour.signed_area(), 20_000.0);
}

#[test]
fn the_media_box_origin_is_subtracted_from_every_point() {
  let content = "150 250 m 250 250 l 250 300 l 150 300 l h S";
  let imported = import(&single_page(
    "[100 200 400 500]",
    content.as_bytes(),
    false,
    "",
  ));

  let contour = &imported.layers[0].contours[0];
  assert_bounds(contour, Point2::new(50.0, 50.0), Point2::new(150.0, 100.0));
}

#[test]
fn both_pages_are_imported_in_page_tree_order() {
  let mut pdf = Pdf::new();
  pdf.stream(1, "", b"0 0 m 10 0 l 10 10 l 0 10 l h S");
  pdf.stream(2, "", b"0 0 m 20 0 l 20 5 l 0 5 l h S");
  pdf.dict(
    3,
    "/Type /Page /Parent 5 0 R /MediaBox [0 0 100 100] /Contents 1 0 R",
  );
  pdf.dict(
    4,
    "/Type /Page /Parent 5 0 R /MediaBox [0 0 100 100] /Contents 2 0 R",
  );
  // The tree lists the pages in the opposite order to their object numbers, so the test proves the
  // page order comes from the tree and not from the scan.
  pdf.dict(5, "/Type /Pages /Kids [4 0 R 3 0 R] /Count 2");
  pdf.dict(6, "/Type /Catalog /Pages 5 0 R");

  let imported = import(&pdf.build(6, ""));
  assert_eq!(imported.layers.len(), 2);
  assert_eq!(imported.layers[0].name, "page 1");
  assert_eq!(imported.layers[1].name, "page 2");
  assert_bounds(
    &imported.layers[0].contours[0],
    Point2::ORIGIN,
    Point2::new(20.0, 5.0),
  );
  assert_bounds(
    &imported.layers[1].contours[0],
    Point2::ORIGIN,
    Point2::new(10.0, 10.0),
  );
  assert_eq!(imported.report.contours, 2);
  assert_eq!(imported.report.sources.get("S"), Some(&2));
}

#[test]
fn an_image_and_a_form_xobject_and_a_text_show_are_skipped_and_counted() {
  let mut pdf = Pdf::new();
  pdf.stream(
    1,
    "/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8",
    &[0u8],
  );
  pdf.stream(2, "/Type /XObject /Subtype /Form /BBox [0 0 1 1]", b"");
  let content = b"q 10 0 0 10 0 0 cm /Im1 Do Q /Fm1 Do BT /F1 12 Tf (hi) Tj ET \
                  0 0 m 100 0 l 100 50 l 0 50 l h S";
  pdf.stream(3, "", content);
  pdf.dict(
    4,
    "/Type /Page /Parent 5 0 R /MediaBox [0 0 595 842] \
     /Resources << /XObject << /Im1 1 0 R /Fm1 2 0 R >> >> /Contents 3 0 R",
  );
  pdf.dict(5, "/Type /Pages /Kids [4 0 R] /Count 1");
  pdf.dict(6, "/Type /Catalog /Pages 5 0 R");

  let imported = import(&pdf.build(6, ""));

  // The skipped constructs change nothing about the geometry.
  let contours = &imported.layers[0].contours;
  assert_eq!(contours.len(), 1);
  assert_bounds(&contours[0], Point2::ORIGIN, Point2::new(100.0, 50.0));

  let skipped = &imported.report.skipped;
  assert_eq!(skipped.len(), 3);
  assert_eq!(skipped[0].construct, "Do");
  assert_eq!(
    skipped[0].reason,
    "image XObject /Im1 is not vector geometry"
  );
  assert_eq!(skipped[0].location.as_deref(), Some("page 1"));
  assert_eq!(skipped[1].construct, "Do");
  assert_eq!(skipped[1].reason, "form XObject /Fm1 is not expanded");
  assert_eq!(skipped[2].construct, "text");
  assert_eq!(
    skipped[2].reason,
    "text is not traced (D-M3-5); 1 text-showing operator skipped"
  );
  assert_eq!(skipped[2].location.as_deref(), Some("page 1"));
  assert!(!imported.report.render().contains("sh: "));
}

#[test]
fn an_encrypted_document_is_refused_by_name() {
  let mut pdf = Pdf::new();
  pdf.stream(1, "", RECTANGLE.as_bytes());
  pdf.dict(
    2,
    "/Type /Page /Parent 3 0 R /MediaBox [0 0 595 842] /Contents 1 0 R",
  );
  pdf.dict(3, "/Type /Pages /Kids [2 0 R] /Count 1");
  pdf.dict(4, "/Type /Catalog /Pages 3 0 R");

  let error = import_bytes(
    "sheet.pdf",
    &pdf.build(4, "/Encrypt 9 0 R "),
    &ImportOptions::default(),
  )
  .expect_err("refused");
  assert_eq!(
    error,
    ImportError::Refused {
      format: "pdf",
      reason: "encrypted documents are not supported".to_string(),
    }
  );
  assert_eq!(
    error.to_string(),
    "pdf refused: encrypted documents are not supported"
  );
}

#[test]
fn a_file_cut_inside_a_stream_is_a_parse_error() {
  let bytes = single_page("[0 0 595 842]", RECTANGLE.as_bytes(), true, "");
  let marker = b"stream\n";
  let start = bytes
    .windows(marker.len())
    .position(|window| window == marker)
    .expect("stream marker")
    + marker.len();

  let error = import_bytes("sheet.pdf", &bytes[..start + 5], &ImportOptions::default())
    .expect_err("parse error");
  match error {
    ImportError::Parse {
      format,
      message,
      location,
    } => {
      assert_eq!(format, "pdf");
      assert_eq!(message, "the file ends inside a stream");
      assert_eq!(location.as_deref(), Some("object 1"));
    }
    other => panic!("unexpected error {other:?}"),
  }
}

#[test]
fn a_file_without_the_pdf_header_is_a_parse_error() {
  let error = import_bytes(
    "sheet.pdf",
    b"just words, no header",
    &ImportOptions::default(),
  )
  .expect_err("parse error");
  match error {
    ImportError::Parse {
      format, message, ..
    } => {
      assert_eq!(format, "pdf");
      assert_eq!(message, "the file does not start with %PDF-");
    }
    other => panic!("unexpected error {other:?}"),
  }
}

#[test]
fn a_stream_with_a_wrong_declared_length_is_recovered_by_its_end_marker() {
  let mut pdf = Pdf::new();
  pdf.stream_declaring(1, "", 5, RECTANGLE.as_bytes());
  pdf.dict(
    2,
    "/Type /Page /Parent 3 0 R /MediaBox [0 0 595 842] /Contents 1 0 R",
  );
  pdf.dict(3, "/Type /Pages /Kids [2 0 R] /Count 1");
  pdf.dict(4, "/Type /Catalog /Pages 3 0 R");

  let imported = import(&pdf.build(4, ""));
  assert_bounds(
    &imported.layers[0].contours[0],
    Point2::ORIGIN,
    Point2::new(100.0, 50.0),
  );
}

#[test]
fn a_clipped_or_unpainted_path_is_not_geometry() {
  let content = "0 0 m 10 0 l 10 10 l 0 10 l h W n 0 0 m 100 0 l 100 50 l 0 50 l h S";
  let imported = import(&single_page("[0 0 595 842]", content.as_bytes(), false, ""));

  let contours = &imported.layers[0].contours;
  assert_eq!(contours.len(), 1);
  assert_bounds(&contours[0], Point2::ORIGIN, Point2::new(100.0, 50.0));
}

#[test]
fn a_re_operator_is_a_rectangle_contour() {
  let imported = import(&single_page(
    "[0 0 595 842]",
    b"10 20 100 50 re S",
    false,
    "",
  ));

  let contour = &imported.layers[0].contours[0];
  assert!(contour.is_closed());
  assert_eq!(contour.len(), 4);
  assert_bounds(contour, Point2::new(10.0, 20.0), Point2::new(110.0, 70.0));
  assert_eq!(imported.report.sources.get("re"), Some(&1));
}

#[test]
fn a_filled_clockwise_path_comes_out_counter_clockwise() {
  let imported = import(&single_page(
    "[0 0 595 842]",
    b"0 0 m 0 50 l 100 50 l 100 0 l h f",
    false,
    "",
  ));

  let contour = &imported.layers[0].contours[0];
  assert!(contour.is_closed());
  assert!(contour.is_ccw());
  assert_close(contour.signed_area(), 5000.0);
  assert_eq!(imported.report.sources.get("f"), Some(&1));
}

#[test]
fn the_graphics_state_stack_restores_the_transform() {
  let content = "q 2 0 0 2 0 0 cm 0 0 m 10 0 l 10 10 l 0 10 l h S Q \
                 0 0 m 30 0 l 30 30 l 0 30 l h S";
  let imported = import(&single_page("[0 0 595 842]", content.as_bytes(), false, ""));

  let contours = &imported.layers[0].contours;
  assert_eq!(contours.len(), 2);
  assert_close(contours[0].signed_area(), 400.0);
  assert_close(contours[1].signed_area(), 900.0);
}

#[test]
fn the_shorthand_curve_forms_use_the_control_point_they_are_missing() {
  // `v` supplies the current point as its first control point, `y` supplies the end point as its
  // second, so both curves peak at 200/4.5 mm above their chord.
  let content = "0 0 m 100 100 100 0 v 0 0 m 0 100 100 0 y S";
  let imported = import(&single_page("[0 0 595 842]", content.as_bytes(), false, ""));

  assert_eq!(imported.report.flattened_curves, 2);
  assert_eq!(imported.report.sources.get("v"), Some(&1));
  assert_eq!(imported.report.sources.get("y"), Some(&1));
  let contours = &imported.layers[0].contours;
  assert_eq!(contours.len(), 2);
  for contour in contours {
    assert!(!contour.is_closed());
    let bounds = contour.bounds();
    assert_close(bounds.min.x, 0.0);
    assert_close(bounds.max.x, 100.0);
    assert_close(bounds.min.y, 0.0);
    assert!((bounds.max.y - 200.0 / 4.5).abs() <= 0.01);
  }
}

#[test]
fn curves_are_flattened_within_the_tolerance_and_counted() {
  // A cubic from (0, 0) to (100, 0) that peaks at y = 75.
  let content = "0 0 m 0 100 100 100 100 0 c S";
  let start = Point2::ORIGIN;
  let control1 = Point2::new(0.0, 100.0);
  let control2 = Point2::new(100.0, 100.0);
  let end = Point2::new(100.0, 0.0);

  let mut vertex_counts = Vec::new();
  for tolerance in [1.0, 0.05, 0.001] {
    let imported = import_with_tolerance(
      &single_page("[0 0 595 842]", content.as_bytes(), false, ""),
      tolerance,
    );
    assert_eq!(imported.report.flattened_curves, 1);
    assert_eq!(imported.report.tolerance, tolerance);

    let contour = &imported.layers[0].contours[0];
    assert!(!contour.is_closed());
    assert!(!contour.has_arcs());
    assert!(contour.len() > 3);
    vertex_counts.push(contour.len());

    // Every point of the true curve is within the tolerance of the polyline that replaced it.
    let mut worst: f64 = 0.0;
    for step in 0..=2000 {
      let t = step as f64 / 2000.0;
      let point = cubic_at(start, control1, control2, end, t);
      worst = worst.max(distance_to_contour(contour, point));
    }
    assert!(
      worst <= tolerance + 1.0e-9,
      "tolerance {tolerance}: {worst}"
    );

    // The endpoints are exact, and the apex is only as low as the tolerance allows.
    let bounds = contour.bounds();
    assert_close(bounds.min.x, 0.0);
    assert_close(bounds.min.y, 0.0);
    assert_close(bounds.max.x, 100.0);
    assert!((bounds.max.y - 75.0).abs() <= tolerance + 1.0e-9);
  }

  // A tighter tolerance buys more segments.
  assert!(vertex_counts[0] < vertex_counts[1]);
  assert!(vertex_counts[1] < vertex_counts[2]);
}
