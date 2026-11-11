//! Byte-level tests for the SVG writer: the millimetre page, one `<path>` per contour, the `A`
//! commands whose radii are equal, the Y reflection and determinism (M12 T2, S3 S5).
//!
//! Every fixture is built in code and every assertion reads the bytes that come out. Nothing here
//! imports the result back: the round trip through `cam-import` is T3's test, so that a wrong sweep
//! flag is caught here, where it is written, rather than only as a mirrored arc later.

use cam_export::{export_bytes, svg, ExportError, Format};
use cam_geom::{Contour, Point2, Vertex};
use cam_model::{Document, Entity, EntityKind, Job, Layer, LayerRole};

// ---------------------------------------------------------------------------------------------
// Fixtures.
// ---------------------------------------------------------------------------------------------

fn document(layers: &[(&str, Vec<Contour>)]) -> Document {
  let mut document = Document::new(Job::default_sheet());
  for (index, (name, contours)) in layers.iter().enumerate() {
    document.layers.push(Layer {
      id: format!("layer-{index}"),
      name: (*name).to_string(),
      role: LayerRole::Geometry,
      visible: true,
      entities: contours
        .iter()
        .enumerate()
        .map(|(position, contour)| Entity {
          id: format!("entity-{index}-{position}"),
          kind: EntityKind::Contour {
            contour: contour.clone(),
          },
        })
        .collect(),
    });
  }
  document
}

fn export(document: &Document) -> String {
  let bytes = export_bytes(document, Format::Svg).expect("the document should export");
  String::from_utf8(bytes).expect("the writer emits text")
}

/// A closed square of straight segments at the origin: four vertices and no bulges.
fn square() -> Contour {
  Contour::from_points(
    &[
      Point2::new(0.0, 0.0),
      Point2::new(20.0, 0.0),
      Point2::new(20.0, 10.0),
      Point2::new(0.0, 10.0),
    ],
    true,
  )
  .expect("the square is valid")
}

/// The same square, moved so its bounds do not start at the origin: the page and the reflection
/// must both come from the drawing, never from an assumed origin.
fn shifted_square() -> Contour {
  Contour::from_points(
    &[
      Point2::new(5.0, -5.0),
      Point2::new(25.0, -5.0),
      Point2::new(25.0, 5.0),
      Point2::new(5.0, 5.0),
    ],
    true,
  )
  .expect("the shifted square is valid")
}

/// An open three-vertex chain of straight segments.
fn open_chain() -> Contour {
  Contour::from_points(
    &[
      Point2::new(0.0, 0.0),
      Point2::new(10.0, 0.0),
      Point2::new(10.0, 10.0),
    ],
    false,
  )
  .expect("the chain is valid")
}

/// An open contour with one straight segment and one arc: a half turn of radius 5 leaving
/// `(10, 0)`, counter-clockwise, so it bulges towards `+x` and the chain ends at `(10, 10)`.
fn line_and_arc() -> Contour {
  Contour::new(
    vec![
      Vertex::corner(Point2::new(0.0, 0.0)),
      Vertex::new(Point2::new(10.0, 0.0), 1.0),
      Vertex::corner(Point2::new(10.0, 10.0)),
    ],
    false,
  )
  .expect("the line-and-arc chain is valid")
}

/// One open contour whose only segment is an arc with the given bulge, so that the flags of the
/// single `A` command can be read back on their own.
fn single_arc(bulge: f64) -> Contour {
  Contour::new(
    vec![
      Vertex::new(Point2::new(0.0, 0.0), bulge),
      Vertex::corner(Point2::new(10.0, 10.0)),
    ],
    false,
  )
  .expect("a single-arc contour is valid")
}

/// A full circle as the model holds it: two vertices at bulge 1.0, closed.
fn circle() -> Contour {
  Contour::new(
    vec![
      Vertex::new(Point2::new(-5.0, 0.0), 1.0),
      Vertex::new(Point2::new(5.0, 0.0), 1.0),
    ],
    true,
  )
  .expect("the circle is valid")
}

// ---------------------------------------------------------------------------------------------
// Reading the bytes back.
// ---------------------------------------------------------------------------------------------

/// The value of an attribute, found by its leading space so that `d` does not match inside
/// `width`.
fn attribute<'a>(text: &'a str, name: &str) -> &'a str {
  let needle = format!(" {name}=\"");
  let start = text
    .find(&needle)
    .unwrap_or_else(|| panic!("no {name} attribute in:\n{text}"))
    + needle.len();
  let rest = &text[start..];
  let end = rest
    .find('"')
    .unwrap_or_else(|| panic!("the {name} value is not quoted"));
  &rest[..end]
}

/// A root length, in millimetres, from its `mm` suffix.
fn millimetres(text: &str, name: &str) -> f64 {
  let value = attribute(text, name);
  let number = value
    .strip_suffix("mm")
    .unwrap_or_else(|| panic!("{name} is not a millimetre length: {value:?}"));
  number
    .parse()
    .unwrap_or_else(|_| panic!("{name} is not a number: {number:?}"))
}

/// The four numbers of the `viewBox`.
fn view_box(text: &str) -> [f64; 4] {
  let numbers: Vec<f64> = attribute(text, "viewBox")
    .split(' ')
    .map(|value| value.parse().expect("the viewBox is four numbers"))
    .collect();
  assert_eq!(numbers.len(), 4, "a viewBox is four numbers");
  [numbers[0], numbers[1], numbers[2], numbers[3]]
}

/// Every `<path>` line, in file order.
fn paths(text: &str) -> Vec<&str> {
  text
    .lines()
    .filter(|line| line.trim_start().starts_with("<path"))
    .collect()
}

/// The `d` of the only path in the file.
fn only_path(text: &str) -> &str {
  let paths = paths(text);
  assert_eq!(paths.len(), 1, "expected exactly one path in:\n{text}");
  attribute(paths[0], "d")
}

/// Every `A` command of a `d`, as its seven arguments.
fn arcs(d: &str) -> Vec<[String; 7]> {
  let tokens: Vec<&str> = d.split(' ').collect();
  let mut arcs = Vec::new();
  for (index, token) in tokens.iter().enumerate() {
    if *token != "A" {
      continue;
    }
    let arguments = &tokens[index + 1..index + 8];
    assert_eq!(arguments.len(), 7, "A takes rx, ry, rotation and four more");
    arcs.push([
      arguments[0].to_string(),
      arguments[1].to_string(),
      arguments[2].to_string(),
      arguments[3].to_string(),
      arguments[4].to_string(),
      arguments[5].to_string(),
      arguments[6].to_string(),
    ]);
  }
  arcs
}

// ---------------------------------------------------------------------------------------------
// The page (S3: millimetres with no scale factor).
// ---------------------------------------------------------------------------------------------

/// The whole file for one closed square, pinned byte for byte.
///
/// `20mm` by `10mm` is the square's own size, the same numbers are in the `viewBox`, so the
/// user unit is a millimetre and the reader's `unit_scale` is exactly 1.0. The square's top edge
/// (`y = 10`) is the top of the page, so in the y-down user space it is `y = 0` and the bottom edge
/// is `y = 10`: the drawing is reflected about the horizontal centre line of its own bounds.
const SQUARE_FILE: &str = "\
<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"20mm\" height=\"10mm\" viewBox=\"0 0 20 10\">
  <path d=\"M 0 10 L 20 10 L 20 0 L 0 0 Z\"/>
</svg>
";

#[test]
fn a_document_exports_as_the_pinned_millimetre_file() {
  let text = export(&document(&[("OUTLINE", vec![square()])]));

  assert_eq!(text, SQUARE_FILE);
}

#[test]
fn the_size_and_the_view_box_are_the_documents_own_bounds_in_millimetres() {
  // Bounds at (5, -5)..(25, 5): the page is the drawing, not the origin plus the drawing, and the
  // flattened viewBox is not "0 0 w h".
  let text = export(&document(&[("OUTLINE", vec![shifted_square()])]));

  assert_eq!(
    attribute(&text, "viewBox"),
    "5 -5 20 10",
    "the viewBox is the bounds, in millimetres"
  );
  assert_eq!(millimetres(&text, "width"), 20.0);
  assert_eq!(millimetres(&text, "height"), 10.0);
  // The reader scales by `min(width / box_width, height / box_height)`, so equal pairs are the
  // only way to pin that factor at exactly 1.0 on both axes.
  let [_, _, box_width, box_height] = view_box(&text);
  assert_eq!(
    (millimetres(&text, "width"), millimetres(&text, "height")),
    (box_width, box_height),
    "a width that differs from the viewBox would scale the drawing"
  );
  // Reflected about the centre line of the bounds, which is y = 0 here: the top edge at y = 5
  // becomes 0 and the bottom edge at -5 becomes 5, and both stay inside the viewBox.
  assert_eq!(only_path(&text), "M 5 5 L 25 5 L 25 -5 L 5 -5 Z");
}

#[test]
fn an_axis_the_geometry_does_not_span_gets_a_one_millimetre_page() {
  // A straight line at y = 7 has no height at all, and an svg page needs one: the axis is widened
  // symmetrically around the drawing, so the line sits in the middle of the page.
  let line = Contour::from_points(&[Point2::new(0.0, 7.0), Point2::new(10.0, 7.0)], false)
    .expect("a straight line is a valid contour");

  let text = export(&document(&[("OUTLINE", vec![line])]));

  assert_eq!(attribute(&text, "viewBox"), "0 6.5 10 1");
  assert_eq!(millimetres(&text, "height"), 1.0);
  assert_eq!(only_path(&text), "M 0 7 L 10 7");
}

// ---------------------------------------------------------------------------------------------
// One `<path>` per contour, and the command sequence (S3, S5).
// ---------------------------------------------------------------------------------------------

#[test]
fn every_contour_is_one_path_and_the_layer_order_is_the_document_order() {
  let document = document(&[
    ("OUTLINE", vec![square(), open_chain()]),
    ("MOVE", vec![shifted_square()]),
  ]);

  let text = export(&document);
  let paths = paths(&text);

  // Bounds are the union, (0, -5)..(25, 10), so one page is shared by every contour of every
  // layer and the reflection constant is the centre line of that page: y = 5.
  assert_eq!(attribute(&text, "viewBox"), "0 -5 25 15");
  assert_eq!(paths.len(), 3, "one path per contour, in:\n{text}");
  assert_eq!(attribute(paths[0], "d"), "M 0 5 L 20 5 L 20 -5 L 0 -5 Z");
  assert_eq!(attribute(paths[1], "d"), "M 0 5 L 10 5 L 10 -5");
  assert_eq!(attribute(paths[2], "d"), "M 5 10 L 25 10 L 25 0 L 5 0 Z");
}

#[test]
fn a_straight_segment_is_l_and_an_arc_is_a_with_equal_radii() {
  // One straight segment then a half turn: the pinned path is the whole contract of the writer.
  let text = export(&document(&[("OUTLINE", vec![line_and_arc()])]));

  assert_eq!(
    only_path(&text),
    "M 0 10 L 10 10 A 5 5 0 0 0 10 0",
    "the straight segment is L, the arc is A, and the radii are equal"
  );

  let arc = arcs(only_path(&text))[0].clone();
  assert_eq!(
    (arc[0].as_str(), arc[1].as_str()),
    ("5", "5"),
    "rx must equal ry, which is the only form the reader keeps exact: {arc:?}"
  );
  assert_eq!(arc[2], "0", "an svg arc with a rotation is not circular");
}

#[test]
fn an_arcs_sweep_flag_follows_the_sign_of_its_turn() {
  // The page reflects the document about the horizontal centre line of its own bounds, and a
  // reflection negates every bulge: an arc that turns counter-clockwise in the document (positive
  // bulge) turns clockwise in the y-down user space. The reader takes the bulge from
  // `tan(sweep / 4)` with a positive sweep exactly when the sweep flag is 1, so a positive document
  // bulge writes sweep flag 0 and a negative one writes 1. `large-arc` carries the size: an arc
  // wider than half a turn (|bulge| > 1) needs it.
  let cases = [
    (0.5, "0", "0"),
    (-0.5, "0", "1"),
    (2.0, "1", "0"),
    (-2.0, "1", "1"),
  ];

  for (bulge, large_arc, sweep_flag) in cases {
    let text = export(&document(&[("OUTLINE", vec![single_arc(bulge)])]));
    let d = only_path(&text);
    let arc = &arcs(d)[0];
    assert_eq!(arc[3].as_str(), large_arc, "large-arc for bulge {bulge}");
    assert_eq!(arc[4].as_str(), sweep_flag, "sweep for bulge {bulge}");
    assert_eq!(arc[0], arc[1], "rx == ry for bulge {bulge}");

    // The radius is the arc's own: for a chord of length c and a bulge b it is c(1 + b²)/(4|b|).
    let chord = Point2::new(10.0, 10.0).distance_to(Point2::ORIGIN);
    let expected = chord * (1.0 + bulge * bulge) / (4.0 * bulge.abs());
    let written: f64 = arc[0].parse().expect("the radius is a number");
    assert!(
      (written - expected).abs() <= 1.0e-9 * expected,
      "bulge {bulge} needs radius {expected}, got {written}"
    );
  }
}

#[test]
fn a_closed_contour_ends_in_z_without_repeating_its_first_vertex() {
  let text = export(&document(&[("OUTLINE", vec![square()])]));
  let d = only_path(&text);

  assert_eq!(d, "M 0 10 L 20 10 L 20 0 L 0 0 Z");
  assert!(d.ends_with(" Z"), "a closed contour ends in Z: {d:?}");
  // Four corners is eight numbers: the closing segment is implied by Z rather than written as a
  // fifth point on top of the first.
  let numbers: Vec<&str> = d
    .split(' ')
    .filter(|token| token.parse::<f64>().is_ok())
    .collect();
  assert_eq!(numbers.len(), 8, "four points, in {d:?}");
  assert_ne!(
    &numbers[..2],
    &numbers[6..],
    "the first vertex must not be repeated before Z: {d:?}"
  );
}

#[test]
fn an_open_contour_does_not_end_in_z() {
  let text = export(&document(&[("OUTLINE", vec![open_chain()])]));

  assert_eq!(only_path(&text), "M 0 10 L 10 10 L 10 0");
}

#[test]
fn a_full_circle_is_one_path_with_two_arcs_back_to_its_start() {
  // A circle is two vertices at bulge 1.0, closed, so its closing segment is an arc. `Z` closes a
  // subpath with a straight line, so the second half turn can only be carried by an explicit `A`
  // back to the first vertex; `Z` then marks the subpath closed and draws nothing.
  let text = export(&document(&[("HOLES", vec![circle()])]));
  let d = only_path(&text);

  assert_eq!(d, "M -5 0 A 5 5 0 0 0 5 0 A 5 5 0 0 0 -5 0 Z");
  let arcs = arcs(d);
  assert_eq!(arcs.len(), 2, "two half turns, not one and a chord: {d:?}");
  assert_eq!(
    (arcs[0][0].as_str(), arcs[0][1].as_str()),
    ("5", "5"),
    "a circle's arc radius is the circle's radius"
  );
  assert_eq!(
    (arcs[1][5].as_str(), arcs[1][6].as_str()),
    ("-5", "0"),
    "the second arc ends where the path started"
  );
  assert!(d.ends_with(" Z"), "the subpath is closed: {d:?}");
  // The page comes from the exact arc bounds, not from a tessellation: a circle of radius 5 about
  // the origin is exactly 10 by 10.
  assert_eq!(attribute(&text, "viewBox"), "-5 -5 10 10");
}

// ---------------------------------------------------------------------------------------------
// The loss the format carries (S5: svg keeps no layers).
// ---------------------------------------------------------------------------------------------

#[test]
fn no_layer_name_reaches_the_file_because_an_svg_carries_no_layers() {
  let document = document(&[("A&B", vec![square()]), ("<OUTLINE>", vec![circle()])]);

  let text = export(&document);

  assert_eq!(paths(&text).len(), 2);
  assert!(
    !text.contains("A&B") && !text.contains("OUTLINE") && !text.contains("layer-"),
    "an svg has no layer record, so nothing of the layer name is written:\n{text}"
  );
}

// ---------------------------------------------------------------------------------------------
// Determinism, and the refusals.
// ---------------------------------------------------------------------------------------------

#[test]
fn exporting_the_same_document_twice_gives_the_same_bytes() {
  let document = document(&[
    ("OFFAL", vec![square(), circle()]),
    ("OUTLINE", vec![line_and_arc(), open_chain()]),
  ]);

  let first = export_bytes(&document, Format::Svg).expect("the first export succeeds");
  let second = export_bytes(&document, Format::Svg).expect("the second export succeeds");

  assert_eq!(first, second);
  assert_eq!(
    first,
    svg::export(&document).expect("the format module agrees with the dispatch"),
  );
  assert!(!first.is_empty());
}

fn refusal(document: &Document) -> ExportError {
  export_bytes(document, Format::Svg).expect_err("the document should be refused")
}

#[test]
fn a_document_with_no_geometry_is_refused() {
  // An svg page is the drawing's own bounds: with no drawing there is no page, and a zero-sized
  // viewBox is not a page any reader accepts.
  let document = document(&[("EMPTY", Vec::new())]);

  let error = refusal(&document);

  assert_eq!(
    error,
    ExportError::Refused {
      format: "svg",
      reason: "the document has no geometry, so an svg has no page: its width, height and viewBox \
               are the drawing's own bounds"
        .to_string()
    }
  );
  assert_eq!(error.to_string().split(':').next(), Some("svg refused"));
  assert_eq!(document.layers.len(), 1, "a refused export mutates nothing");
}

#[test]
fn a_non_finite_coordinate_is_refused() {
  let broken = Contour::from_points(
    &[Point2::new(f64::INFINITY, 0.0), Point2::new(10.0, 0.0)],
    false,
  )
  .expect("the kernel accepts a non-finite point, which is why the writer must refuse one");
  let document = document(&[("OUTLINE", vec![broken])]);

  let error = refusal(&document);

  assert!(
    error.to_string().contains("not a finite number"),
    "got {error}"
  );
}
