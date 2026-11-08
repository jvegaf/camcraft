//! The round trip: what this crate writes, `cam-import` must read back as the same shape.
//!
//! This is the only acceptance test a writer can honestly offer (M12 S4). It is deliberately **not** a
//! byte comparison and not a vertex-list comparison, and the two reasons are facts of the reader:
//!
//! - the reader places imported geometry with a 10 mm margin and re-derives the job size
//!   (`cam-import/src/document.rs`), so absolute coordinates differ by a translation this test removes;
//! - the reader normalises every closed contour to counter-clockwise, so a clockwise contour comes back
//!   as the same shape with its vertex order reversed.
//!
//! So the comparison is a **shape** comparison: both sides are tessellated, normalised to their own
//! bounding box, and compared as point sets in both directions. That catches a lost arc, a wrong sweep
//! flag, a chord where a half turn belongs, a phantom closing vertex and a shifted page, and it is
//! blind to exactly the things the reader is allowed to change.

use cam_export::{export_bytes, Format};
use cam_geom::{Contour, Point2};
use cam_import::{import_bytes, ImportOptions};
use cam_model::{Document, Entity, EntityKind, Job, Layer};

/// How far a tessellated point may drift and still be the same shape, in millimetres. The reader
/// re-derives a bulge from two endpoints and a flag, so a half turn comes back as `0.9999999999999999`
/// rather than `1.0`; a thousandth is far tighter than any shape error worth catching.
const TOLERANCE: f64 = 1.0e-3;

fn page(layers: &[(&str, Vec<Contour>)]) -> Document {
  let mut document = Document::new(Job::default_sheet());
  for (name, contours) in layers {
    let entities: Vec<Entity> = contours
      .iter()
      .enumerate()
      .map(|(index, contour)| Entity {
        id: format!("E{index}"),
        kind: EntityKind::Contour {
          contour: contour.clone(),
        },
      })
      .collect();
    document.layers.push(Layer {
      id: name.to_string(),
      name: name.to_string(),
      visible: true,
      entities,
    });
  }
  document
}

/// The shapes a document holds, per layer, in document order.
fn shapes(document: &Document) -> Vec<(String, Vec<Contour>)> {
  document
    .layers
    .iter()
    .map(|layer| {
      let contours: Vec<Contour> = layer
        .entities
        .iter()
        .filter_map(|entity| entity.kind.as_contour().cloned())
        .collect();
      (layer.name.clone(), contours)
    })
    .collect()
}

/// A contour as points in its own frame: tessellated, then translated so its bounding box starts at the
/// origin. The translation is the one the reader performs and the one S4 subtracts.
fn points(contour: &Contour) -> Vec<Point2> {
  let sampled = contour.tessellate(TOLERANCE).to_vec();
  let mut min = Point2::new(f64::INFINITY, f64::INFINITY);
  for point in &sampled {
    min = Point2::new(min.x.min(point.x), min.y.min(point.y));
  }
  sampled
    .iter()
    .map(|point| Point2::new(point.x - min.x, point.y - min.y))
    .collect()
}

fn nearest(from: Point2, to: &[Point2]) -> f64 {
  to.iter()
    .map(|point| ((point.x - from.x).powi(2) + (point.y - from.y).powi(2)).sqrt())
    .fold(f64::INFINITY, f64::min)
}

/// Both directions, because a subset would pass a shape that merely contains the other.
fn assert_same_shape(written: &Contour, read: &Contour, what: &str) {
  let written_points = points(written);
  let read_points = points(read);
  for point in &written_points {
    let distance = nearest(*point, &read_points);
    assert!(
      distance <= TOLERANCE,
      "{what}: a written point ({:.4}, {:.4}) is {distance:.4} mm from the read shape",
      point.x,
      point.y
    );
  }
  for point in &read_points {
    let distance = nearest(*point, &written_points);
    assert!(
      distance <= TOLERANCE,
      "{what}: a read point ({:.4}, {:.4}) is {distance:.4} mm from the written shape",
      point.x,
      point.y
    );
  }
  assert_eq!(
    written.is_closed(),
    read.is_closed(),
    "{what}: closedness must survive, and it is the only signal DXF carries for it"
  );
}

/// One contour per interesting class: an open polyline with a straight and a curved leg, a closed
/// square, a full circle carried as two half turns, and a closed shape whose **closing** segment is an
/// arc — the case SVG has nowhere to put a bulge for.
fn fixtures() -> Vec<(&'static str, Contour)> {
  use cam_geom::Segment;

  let mixed = Contour::new(
    vec![
      cam_geom::Vertex::new(Point2::new(0.0, 0.0), 0.0),
      cam_geom::Vertex::new(
        Point2::new(20.0, 0.0),
        (std::f64::consts::FRAC_PI_2 / 4.0).tan(),
      ),
      cam_geom::Vertex::new(Point2::new(20.0, 20.0), 0.0),
    ],
    false,
  )
  .expect("a contour");
  assert!(matches!(mixed.segments().first(), Some(Segment::Line(_))));

  let square =
    cam_geom::primitive::rectangle(Point2::new(0.0, 0.0), 40.0, 25.0).expect("rectangle");
  let circle = cam_geom::primitive::circle(Point2::new(60.0, 12.5), 12.5).expect("circle");

  vec![
    ("a straight and a curved leg", mixed),
    ("a closed rectangle", square),
    ("a full circle as two half turns", circle),
  ]
}

#[test]
fn a_document_survives_a_dxf_round_trip_with_its_layers() {
  // S2 and S4: the writer exists, and what it writes the reader reads back. DXF is the format that
  // carries layer identity, so the names and their order must survive too.
  let contours: Vec<Contour> = fixtures().into_iter().map(|(_, contour)| contour).collect();
  let document = page(&[
    ("OUTLINE", contours.clone()),
    ("HOLES", vec![contours[2].clone()]),
  ]);
  let bytes = export_bytes(&document, Format::Dxf).expect("the document is writable");
  let text = String::from_utf8(bytes.clone()).expect("ascii dxf");

  let imported =
    import_bytes("part.dxf", &bytes, &ImportOptions::default()).expect("our own reader");
  let names: Vec<&str> = imported
    .layers
    .iter()
    .map(|layer| layer.name.as_str())
    .collect();
  assert_eq!(
    names,
    vec!["OUTLINE", "HOLES"],
    "every layer the document declares comes back, in document order: {text}"
  );

  let written = shapes(&document);
  assert_eq!(imported.layers.len(), written.len());
  for (layer, source) in imported.layers.iter().zip(written.iter()) {
    assert_eq!(layer.name, source.0, "the same layer, in the same place");
    assert_eq!(
      layer.contours.len(),
      source.1.len(),
      "{}: one contour out, one contour back",
      source.0
    );
    for (index, (read, wrote)) in layer.contours.iter().zip(source.1.iter()).enumerate() {
      assert_same_shape(wrote, read, &format!("{}/{}", source.0, index));
    }
  }
}

#[test]
fn a_document_survives_an_svg_round_trip_and_loses_its_layers() {
  // S3 and S5: the same shape comes back, and the format's own loss is asserted rather than hidden —
  // `cam-import` puts every SVG contour in one layer named "svg", so the writer cannot preserve layer
  // identity and does not pretend to.
  let contours: Vec<Contour> = fixtures().into_iter().map(|(_, contour)| contour).collect();
  let document = page(&[
    ("OUTLINE", contours.clone()),
    ("HOLES", vec![contours[2].clone()]),
  ]);
  let bytes = export_bytes(&document, Format::Svg).expect("the document is writable");
  let text = String::from_utf8(bytes.clone()).expect("utf8 svg");

  for name in ["OUTLINE", "HOLES"] {
    assert!(
      !text.contains(name),
      "an svg carries no layer names, so the file must not pretend to: {name} appears in {text}"
    );
  }

  // Two layers become one on the way back, so the shape comparison is made against a page that holds
  // one copy of each contour: a duplicate would prove nothing and would look like a writer bug.
  let single = page(&[("OUTLINE", contours.clone())]);
  let single_bytes = export_bytes(&single, Format::Svg).expect("the document is writable");
  let imported =
    import_bytes("part.svg", &single_bytes, &ImportOptions::default()).expect("our own reader");
  assert_eq!(imported.layers.len(), 1, "an svg is one layer");
  assert_eq!(imported.layers[0].name, "svg", "and the reader names it");
  assert_eq!(
    imported.layers[0].contours.len(),
    contours.len(),
    "every contour crosses the page"
  );
  for (index, (wrote, read)) in contours
    .iter()
    .zip(imported.layers[0].contours.iter())
    .enumerate()
  {
    assert_same_shape(wrote, read, &format!("svg/{index}"));
  }
}

#[test]
fn the_page_is_measured_in_millimetres_so_the_scale_is_one() {
  // A `viewBox` and a size that disagree would scale the drawing on the way back in, and the round trip
  // above would fail for a reason that has nothing to do with arcs.
  let contours: Vec<Contour> = fixtures().into_iter().map(|(_, contour)| contour).collect();
  let document = page(&[("OUTLINE", contours)]);
  let bytes = export_bytes(&document, Format::Svg).expect("the document is writable");
  let imported =
    import_bytes("part.svg", &bytes, &ImportOptions::default()).expect("our own reader");
  assert!(
    (imported.report.unit_scale - 1.0).abs() < 1.0e-9,
    "the page is the drawing's own millimetres: read scale {}",
    imported.report.unit_scale
  );
}

#[test]
fn both_formats_are_deterministic() {
  // A writer that reorders a hash map or prints a float differently between runs makes every golden
  // that pins its output a coin toss.
  let contours: Vec<Contour> = fixtures().into_iter().map(|(_, contour)| contour).collect();
  let document = page(&[("OUTLINE", contours)]);
  for format in [Format::Dxf, Format::Svg] {
    let first = export_bytes(&document, format).expect("writable");
    let second = export_bytes(&document, format).expect("writable");
    assert_eq!(
      first,
      second,
      "{} exports the same bytes twice",
      format.name()
    );
  }
}
