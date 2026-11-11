//! Byte-level tests for the DXF writer: the header, the layer table in document order, and one
//! `LWPOLYLINE` per contour carrying its bulges and its closedness flag (M12 T1, S2 S3 S5).
//!
//! Every fixture is built in code and every assertion reads the bytes that come out. Nothing here
//! imports the result back: the round trip through `cam-import` is T3's test, so that a wrong group
//! code is caught here, where it is written, rather than only as a mangled contour later.

use cam_export::{dxf, export_bytes, ExportError, Format};
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
  let bytes = export_bytes(document, Format::Dxf).expect("the document should export");
  String::from_utf8(bytes).expect("the writer emits text")
}

/// A closed square of straight segments: four vertices and no bulges.
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

/// A closed contour whose segments are straight, a half turn and a clockwise quarter-ish arc.
fn bulged() -> Contour {
  Contour::new(
    vec![
      Vertex::corner(Point2::new(0.0, 0.0)),
      Vertex::new(Point2::new(10.0, 0.0), 1.0),
      Vertex::new(Point2::new(20.0, 10.0), -0.5),
    ],
    true,
  )
  .expect("the bulged contour is valid")
}

/// An open three-vertex chain.
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
// Reading the bytes back, as group-code pairs.
// ---------------------------------------------------------------------------------------------

/// The file as `(group code, value)` pairs, which is the only shape a DXF has.
fn pairs(text: &str) -> Vec<(i32, String)> {
  let lines: Vec<&str> = text.lines().collect();
  assert_eq!(
    lines.len() % 2,
    0,
    "every group code needs a value line, and the file ends with one: {text}"
  );
  lines
    .chunks(2)
    .map(|pair| {
      let code = pair[0]
        .parse::<i32>()
        .unwrap_or_else(|_| panic!("group code {:?} is not an integer", pair[0]));
      (code, pair[1].to_string())
    })
    .collect()
}

/// The layer names the `LAYER` table declares, in table order.
fn table_layers(pairs: &[(i32, String)]) -> Vec<String> {
  let mut names = Vec::new();
  for (index, (code, value)) in pairs.iter().enumerate() {
    if *code == 0 && value == "LAYER" {
      match pairs.get(index + 1) {
        Some((2, name)) => names.push(name.clone()),
        other => panic!("a LAYER record must start with its name in group code 2, got {other:?}"),
      }
    }
  }
  names
}

/// Every `0`-record of the entity type, as its own run of field pairs.
fn records(pairs: &[(i32, String)], kind: &str) -> Vec<Vec<(i32, String)>> {
  let mut records = Vec::new();
  let mut index = 0;
  while index < pairs.len() {
    if pairs[index].0 != 0 || pairs[index].1 != kind {
      index += 1;
      continue;
    }
    let start = index + 1;
    let mut end = start;
    while end < pairs.len() && pairs[end].0 != 0 {
      end += 1;
    }
    records.push(pairs[start..end].to_vec());
    index = end;
  }
  records
}

/// The values of one group code in a record, in the order they appear.
fn values(record: &[(i32, String)], code: i32) -> Vec<String> {
  record
    .iter()
    .filter(|(pair_code, _)| *pair_code == code)
    .map(|(_, value)| value.clone())
    .collect()
}

/// The single value of a group code in a record.
fn value(record: &[(i32, String)], code: i32) -> String {
  let found = values(record, code);
  assert_eq!(
    found.len(),
    1,
    "group code {code} appears {} times",
    found.len()
  );
  found[0].clone()
}

// ---------------------------------------------------------------------------------------------
// The header (S3, S5: millimetres, scale exactly 1.0).
// ---------------------------------------------------------------------------------------------

/// The header the writer emits, pinned byte for byte: `AC1015` (R2000, the first version whose
/// `LWPOLYLINE` has bulges) and `$INSUNITS` 4, millimetres.
const HEADER: &str = "\
0
SECTION
2
HEADER
9
$ACADVER
1
AC1015
9
$INSUNITS
70
4
0
ENDSEC
";

#[test]
fn the_header_declares_millimetre_insunits() {
  let text = export(&document(&[("OUTLINE", vec![square()])]));

  assert!(
    text.starts_with(HEADER),
    "the file must open with the pinned header, got:\n{text}"
  );
  // Scale 1.0 is the whole point: the importer multiplies by 1.0 for code 4 and by nothing else.
  let pairs = pairs(&text);
  let insunits = pairs
    .iter()
    .position(|(code, value)| *code == 9 && value == "$INSUNITS")
    .expect("the header carries $INSUNITS");
  assert_eq!(pairs[insunits + 1], (70, "4".to_string()));
}

// ---------------------------------------------------------------------------------------------
// The layer table (S5: DXF keeps layers; the reader takes the table's order as the document's).
// ---------------------------------------------------------------------------------------------

#[test]
fn the_layer_table_lists_every_layer_in_document_order() {
  // Deliberately not alphabetical and not in any hash order: document order is the contract.
  let document = document(&[
    ("OUTLINE", vec![square()]),
    ("WALLS", vec![square()]),
    ("HOLES", vec![square()]),
  ]);

  let text = export(&document);
  let pairs = pairs(&text);

  assert_eq!(
    table_layers(&pairs),
    vec![
      "OUTLINE".to_string(),
      "WALLS".to_string(),
      "HOLES".to_string()
    ]
  );
  // The `TABLE` record declares how many `LAYER` records follow.
  let table = pairs
    .iter()
    .position(|(code, value)| *code == 2 && value == "LAYER")
    .expect("the LAYER table is declared");
  assert_eq!(pairs[table + 1], (70, "3".to_string()));
  // The layer's own `name` is written verbatim, never its id.
  assert!(
    !text.contains("layer-0"),
    "the writer emits layer ids:\n{text}"
  );
}

#[test]
fn a_layer_with_no_geometry_is_still_declared() {
  let document = document(&[("EMPTY", Vec::new()), ("WALLS", vec![square()])]);

  let pairs = pairs(&export(&document));

  assert_eq!(
    table_layers(&pairs),
    vec!["EMPTY".to_string(), "WALLS".to_string()]
  );
  assert_eq!(records(&pairs, "LWPOLYLINE").len(), 1);
}

// ---------------------------------------------------------------------------------------------
// One LWPOLYLINE per contour (S3: the bulge is the arc, verbatim).
// ---------------------------------------------------------------------------------------------

/// The record for `bulged()` on layer `OUTLINE`, pinned group code by group code.
const BULGED_RECORD: &str = "\
0
LWPOLYLINE
8
OUTLINE
90
3
70
1
10
0
20
0
42
0
10
10
20
0
42
1
10
20
20
10
42
-0.5
";

#[test]
fn one_lwpolyline_per_contour_carries_each_bulge_verbatim() {
  let text = export(&document(&[("OUTLINE", vec![bulged()])]));

  assert!(
    text.ends_with(&format!("{BULGED_RECORD}0\nENDSEC\n0\nEOF\n")),
    "the contour must go out as the pinned record, got:\n{text}"
  );

  let pairs = pairs(&text);
  let polylines = records(&pairs, "LWPOLYLINE");
  assert_eq!(polylines.len(), 1);
  let record = &polylines[0];
  assert_eq!(value(record, 8), "OUTLINE");
  assert_eq!(value(record, 90), "3", "the declared vertex count");
  assert_eq!(value(record, 70), "1", "a closed contour is 70 = 1");
  // Each vertex is a point followed by the bulge of the segment leaving it: straight is 0, not
  // omitted, so every vertex reads back with the bulge the kernel gave it.
  assert_eq!(values(record, 42), vec!["0", "1", "-0.5"]);
  assert_eq!(values(record, 10), vec!["0", "10", "20"]);
  assert_eq!(values(record, 20), vec!["0", "0", "10"]);
}

#[test]
fn a_full_circle_goes_out_as_two_half_turn_bulges() {
  let pairs = pairs(&export(&document(&[("HOLES", vec![circle()])])));

  let polylines = records(&pairs, "LWPOLYLINE");
  assert_eq!(polylines.len(), 1);
  let record = &polylines[0];
  assert_eq!(values(record, 42), vec!["1", "1"]);
  assert_eq!(value(record, 90), "2");
  assert_eq!(value(record, 70), "1");
}

#[test]
fn every_contour_of_every_layer_is_written_under_its_own_layer() {
  let document = document(&[
    ("OUTLINE", vec![square(), open_chain()]),
    ("HOLES", vec![circle()]),
  ]);

  let pairs = pairs(&export(&document));
  let polylines = records(&pairs, "LWPOLYLINE");

  assert_eq!(polylines.len(), 3);
  let layers: Vec<String> = polylines.iter().map(|record| value(record, 8)).collect();
  assert_eq!(layers, vec!["OUTLINE", "OUTLINE", "HOLES"]);
}

// ---------------------------------------------------------------------------------------------
// Closedness is explicit (contour.rs: closedness never becomes a phantom vertex).
// ---------------------------------------------------------------------------------------------

#[test]
fn an_open_contour_is_flagged_open_and_carries_no_extra_vertex() {
  let pairs = pairs(&export(&document(&[("OUTLINE", vec![open_chain()])])));

  let polylines = records(&pairs, "LWPOLYLINE");
  let record = &polylines[0];
  assert_eq!(value(record, 70), "0", "an open contour is 70 = 0");
  assert_eq!(value(record, 90), "3");
  assert_eq!(values(record, 10), vec!["0", "10", "10"]);
  assert_eq!(values(record, 20), vec!["0", "0", "10"]);
}

#[test]
fn a_closed_contour_does_not_repeat_its_first_vertex() {
  let pairs = pairs(&export(&document(&[("OUTLINE", vec![square()])])));

  let polylines = records(&pairs, "LWPOLYLINE");
  let record = &polylines[0];
  assert_eq!(value(record, 70), "1");
  assert_eq!(
    value(record, 90),
    "4",
    "closedness is the 70 bit, never a fifth vertex"
  );
  let xs = values(record, 10);
  let ys = values(record, 20);
  assert_eq!(xs, vec!["0", "20", "20", "0"]);
  assert_eq!(ys, vec!["0", "0", "10", "10"]);
  assert_ne!(
    (xs.last(), ys.last()),
    (xs.first(), ys.first()),
    "the closing segment is implied by 70 = 1, so the first vertex is written once"
  );
}

// ---------------------------------------------------------------------------------------------
// Determinism, and the empty document.
// ---------------------------------------------------------------------------------------------

#[test]
fn exporting_the_same_document_twice_gives_the_same_bytes() {
  let document = document(&[
    ("OFFAL", vec![square(), circle()]),
    ("OUTLINE", vec![bulged(), open_chain()]),
  ]);

  let first = export_bytes(&document, Format::Dxf).expect("the first export succeeds");
  let second = export_bytes(&document, Format::Dxf).expect("the second export succeeds");

  assert_eq!(first, second);
  assert_eq!(
    first,
    dxf::export(&document).expect("the format module agrees with the dispatch"),
  );
  assert!(!first.is_empty());
}

#[test]
fn an_empty_document_still_writes_the_three_sections() {
  let text = export(&document(&[]));

  let pairs = pairs(&text);
  let sections: Vec<String> = pairs
    .iter()
    .enumerate()
    .filter(|(index, (code, value))| {
      *code == 0 && value == "SECTION" && pairs.get(index + 1).map(|(code, _)| *code) == Some(2)
    })
    .map(|(index, _)| pairs[index + 1].1.clone())
    .collect();

  assert_eq!(
    sections,
    vec![
      "HEADER".to_string(),
      "TABLES".to_string(),
      "ENTITIES".to_string()
    ]
  );
  assert_eq!(table_layers(&pairs), Vec::<String>::new());
  assert!(records(&pairs, "LWPOLYLINE").is_empty());
  assert!(
    text.ends_with("0\nEOF\n"),
    "the file ends with EOF:\n{text}"
  );
}

// ---------------------------------------------------------------------------------------------
// Refusals: what the reader would drop or merge is refused instead of written.
// ---------------------------------------------------------------------------------------------

fn refusal(document: &Document) -> ExportError {
  export_bytes(document, Format::Dxf).expect_err("the document should be refused")
}

#[test]
fn an_empty_layer_name_is_refused() {
  let document = document(&[("", vec![square()])]);

  let error = refusal(&document);

  assert_eq!(
    error,
    ExportError::Refused {
      format: "dxf",
      reason: "a layer has an empty name and a DXF layer record needs one: the reader skips a \
               nameless record, so the layer would vanish from the file"
        .to_string()
    }
  );
  assert_eq!(error.to_string().split(':').next(), Some("dxf refused"));
  assert_eq!(document.layers.len(), 1, "a refused export mutates nothing");
}

#[test]
fn a_layer_name_the_reader_would_trim_is_refused() {
  let document = document(&[(" OUTLINE", vec![square()])]);

  let error = refusal(&document);

  assert!(
    error.to_string().contains("leading or trailing whitespace"),
    "got {error}"
  );
}

#[test]
fn a_layer_name_with_a_line_break_is_refused() {
  let document = document(&[("OUTLINE\nHOLES", vec![square()])]);

  let error = refusal(&document);

  assert!(
    error.to_string().contains("control character"),
    "got {error}"
  );
}

#[test]
fn two_layers_with_the_same_name_are_refused() {
  let document = document(&[("OUTLINE", vec![square()]), ("OUTLINE", vec![circle()])]);

  let error = refusal(&document);

  assert!(
    error.to_string().contains("two layers are named"),
    "got {error}"
  );
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
