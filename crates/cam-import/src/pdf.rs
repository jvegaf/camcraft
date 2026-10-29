//! PDF import: vector content streams only (M3 A9, A10).
//!
//! Reading is tolerant on purpose (A9): the whole file is scanned for `N G obj … endobj` records
//! instead of trusting the cross-reference table, so a missing or damaged xref cannot stop an
//! import. Streams are inflated when `/Filter` says `FlateDecode`, and the content-stream path
//! operators (`m l c v y re h`) are interpreted through a graphics-state stack (`q`/`Q`, `cm`), with
//! the page `/MediaBox` origin subtracted from every point so the page lands with its lower-left
//! corner at `(0, 0)`.
//!
//! Nothing is dropped silently (A10). Images and form XObjects (`Do`), inline images
//! (`BI … ID … EI`), shadings (`sh`), text (`Tj TJ ' "`) and Type3 glyph procedures each appear in
//! [`ImportReport::skipped`] with the reason and the page it came from; an encrypted file is refused
//! by name. A path ended by `n`, `W` or `W*` is not geometry at all — a clip is not a cut path, and
//! the page does not paint it.
//!
//! Two notes on interpretation:
//!
//! * PDF has no arc operator, so curves are cubic Béziers and they cannot stay arcs. They are
//!   flattened adaptively at the caller's tolerance, in output space so the bound holds after the
//!   transform, and each one is counted through [`ImportReport::flatten`] (D-M3-2).
//! * `v` takes its missing first control point from the current point and `y` takes its missing
//!   second control point from the end point, exactly as PDF 32000-1 §8.5.2.2 defines both forms.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;

use cam_geom::{Contour, LineSeg, Point2};

use crate::{ImportError, ImportOptions, ImportReport, ImportedLayer};

/// The format name used in errors, refusals and reports.
const FORMAT: &str = "pdf";

/// Deepest subdivision of one cubic before it is accepted as flat.
///
/// `2 ^ MAX_CURVE_DEPTH == cam_geom::MAX_TESSELLATION_STEPS`, so a pathological curve cannot make the
/// importer allocate without limit.
const MAX_CURVE_DEPTH: usize = 12;

/// Points closer than this are the same point; a duplicate would become a zero-length segment.
const POINT_EPSILON: f64 = 1.0e-9;

/// Deepest page-tree descent, so a `/Kids` cycle cannot recurse forever.
const MAX_PAGE_DEPTH: usize = 32;

/// Import the vector paths of every page.
pub fn import(
  bytes: &[u8],
  options: &ImportOptions,
  report: &mut ImportReport,
) -> Result<Vec<ImportedLayer>, ImportError> {
  if !bytes.starts_with(b"%PDF-") {
    return Err(ImportError::parse_plain(
      FORMAT,
      "the file does not start with %PDF-",
    ));
  }
  if has_encrypt(bytes) {
    return Err(ImportError::refused(
      FORMAT,
      "encrypted documents are not supported",
    ));
  }

  let objects = Objects::scan(bytes)?;
  report_type3_fonts(&objects, report);
  let pages = collect_pages(&objects, report);

  // PDF user units are nominally 1/72 inch, but the format carries no real world unit unless the
  // producer says so; one unit is one millimetre, and the user is told.
  report.unit_scale = 1.0;
  report.warn("PDF user units carry no physical size; one unit is imported as one millimetre");

  let mut layers = Vec::new();
  for (index, page) in pages.iter().enumerate() {
    let label = format!("page {}", index + 1);
    let content = page_content(&objects, page)?;
    let xobjects = xobject_kinds(&objects, &page.resources);
    let mut state = ContentState::new(page.origin, options.tolerance);
    state.run(&content, report, &xobjects);
    state.finish(report, &label);
    if !state.contours.is_empty() {
      layers.push(ImportedLayer {
        name: label,
        contours: state.contours,
      });
    }
  }

  Ok(layers)
}

// ---------------------------------------------------------------------------------------------
// Objects
// ---------------------------------------------------------------------------------------------

/// One indirect object recovered from the file.
struct PdfObject {
  /// The object number as written in its header.
  number: u32,
  /// The object's single value, which for nearly every object is a dictionary.
  value: Value,
  /// The decoded-and-still-compressed stream bytes, when the object has a stream.
  stream: Option<Vec<u8>>,
}

impl PdfObject {
  fn dict(&self) -> Option<&Dict> {
    match self.value {
      Value::Dict(ref dict) => Some(dict),
      _ => None,
    }
  }
}

/// Every object in the file, in document order, plus the object-number index.
struct Objects {
  ordered: Vec<PdfObject>,
  by_number: BTreeMap<u32, usize>,
}

impl Objects {
  /// Scan the whole file for `N G obj … endobj` records, ignoring the xref table.
  fn scan(bytes: &[u8]) -> Result<Self, ImportError> {
    let mut ordered: Vec<PdfObject> = Vec::new();
    let mut by_number: BTreeMap<u32, usize> = BTreeMap::new();
    let mut cursor = 0;

    while let Some(keyword) = find_keyword(bytes, b"obj", cursor) {
      cursor = keyword + 3;
      let Some(number) = object_number(bytes, keyword) else {
        continue;
      };
      let content_start = skip_whitespace(bytes, keyword + 3);

      // A dictionary is the object's value; when there is none, the record still has one value
      // (an array, a number), and `endobj` bounds it.
      let (value, after_value) = if bytes[content_start..].starts_with(b"<<") {
        let Some((start, end)) = dict_span(bytes, content_start) else {
          return Err(ImportError::parse(
            FORMAT,
            "an object dictionary is never closed",
            format!("object {number}"),
          ));
        };
        (first_value(&bytes[start..end]), end)
      } else {
        let end = find_keyword(bytes, b"endobj", content_start).unwrap_or(bytes.len());
        (first_value(&bytes[content_start..end]), end)
      };

      let mut stream = None;
      let mut after = after_value;
      let stream_keyword = skip_whitespace(bytes, after_value);
      if bytes[stream_keyword..].starts_with(b"stream") {
        let length = direct_length(&value);
        let (data, next) = extract_stream(bytes, stream_keyword, length, number)?;
        stream = Some(data);
        after = next;
      }

      cursor = find_keyword(bytes, b"endobj", after)
        .map(|index| index + b"endobj".len())
        .unwrap_or(after);

      let index = ordered.len();
      by_number.insert(number, index);
      ordered.push(PdfObject {
        number,
        value,
        stream,
      });
    }

    Ok(Self { ordered, by_number })
  }

  fn get(&self, number: u32) -> Option<&PdfObject> {
    self
      .by_number
      .get(&number)
      .and_then(|index| self.ordered.get(*index))
  }
}

/// The object number of the header that ends at `keyword` (`N G obj`).
fn object_number(bytes: &[u8], keyword: usize) -> Option<u32> {
  let mut index = keyword;
  let mut parsed = Vec::new();
  while parsed.len() < 2 {
    while index > 0 && is_whitespace(bytes[index - 1]) {
      index -= 1;
    }
    let end = index;
    while index > 0 && bytes[index - 1].is_ascii_digit() {
      index -= 1;
    }
    if index == end {
      return None;
    }
    let text = std::str::from_utf8(&bytes[index..end]).ok()?;
    parsed.push(text.parse::<u32>().ok()?);
  }
  // The pair is `number generation`; the generation was parsed first, scanning backwards.
  parsed.last().copied()
}

/// The bytes of a stream, recovered from `/Length` when it is honest and from `endstream` otherwise.
fn extract_stream(
  bytes: &[u8],
  keyword: usize,
  length: Option<usize>,
  number: u32,
) -> Result<(Vec<u8>, usize), ImportError> {
  let mut start = keyword + b"stream".len();
  if bytes.get(start) == Some(&b'\r') {
    start += 1;
  }
  if bytes.get(start) == Some(&b'\n') {
    start += 1;
  }

  if let Some(length) = length {
    if let Some(candidate) = start.checked_add(length) {
      if candidate <= bytes.len() {
        let next = skip_whitespace(bytes, candidate);
        if bytes[next..].starts_with(b"endstream") {
          return Ok((bytes[start..candidate].to_vec(), next + b"endstream".len()));
        }
      }
    }
  }

  if let Some(endstream) = find_keyword(bytes, b"endstream", start) {
    return Ok((
      trim_trailing_eol(bytes, start, endstream).to_vec(),
      endstream + b"endstream".len(),
    ));
  }
  if let Some(endobj) = find_keyword(bytes, b"endobj", start) {
    return Ok((trim_trailing_eol(bytes, start, endobj).to_vec(), endobj));
  }

  Err(ImportError::parse(
    FORMAT,
    "the file ends inside a stream",
    format!("object {number}"),
  ))
}

/// `bytes[start..end]` without the end-of-line that precedes `endstream`.
fn trim_trailing_eol(bytes: &[u8], start: usize, end: usize) -> &[u8] {
  let mut end = end;
  while end > start && matches!(bytes[end - 1], b'\r' | b'\n') {
    end -= 1;
  }
  &bytes[start..end]
}

/// A stream `/Length` that is a direct integer, which is the only case the scan can trust.
fn direct_length(value: &Value) -> Option<usize> {
  let dict = match value {
    Value::Dict(dict) => dict,
    _ => return None,
  };
  match dict.get("Length") {
    Some(Value::Number(length)) if *length >= 0.0 && length.is_finite() => Some(*length as usize),
    _ => None,
  }
}

/// True when the file names an encryption dictionary.
fn has_encrypt(bytes: &[u8]) -> bool {
  let mut cursor = 0;
  while let Some(index) = find_bytes(bytes, b"/Encrypt", cursor) {
    let after = index + b"/Encrypt".len();
    cursor = after;
    let delimited = match bytes.get(after) {
      None => true,
      Some(byte) => is_whitespace(*byte) || is_delimiter(*byte),
    };
    if delimited {
      return true;
    }
  }
  false
}

// ---------------------------------------------------------------------------------------------
// Pages
// ---------------------------------------------------------------------------------------------

/// One page's geometry inputs: where its origin is, which objects hold its content and the
/// resources that name its XObjects.
struct Page {
  origin: Point2,
  resources: Dict,
  contents: Vec<u32>,
}

/// Values a page inherits from its ancestors in the page tree.
#[derive(Clone, Default)]
struct Inherited {
  media_box: Option<[f64; 4]>,
  resources: Option<Dict>,
}

/// The pages in the order they are read: page-tree order, document order as the fallback.
fn collect_pages(objects: &Objects, report: &mut ImportReport) -> Vec<Page> {
  let mut pages = Vec::new();

  let catalog_pages = objects
    .ordered
    .iter()
    .find(|object| object.dict().and_then(|dict| dict.name("Type")) == Some("Catalog"))
    .and_then(|object| object.dict())
    .and_then(|dict| dict.get("Pages"))
    .map(references)
    .and_then(|numbers| numbers.first().copied());
  let tree_root = catalog_pages.or_else(|| {
    objects
      .ordered
      .iter()
      .find(|object| object.dict().and_then(|dict| dict.name("Type")) == Some("Pages"))
      .map(|object| object.number)
  });

  if let Some(tree_root) = tree_root {
    let mut visited = BTreeSet::new();
    walk_page_tree(
      objects,
      tree_root,
      &Inherited::default(),
      &mut visited,
      &mut pages,
      0,
    );
  }

  if pages.is_empty() {
    // A damaged page tree is not fatal: every `/Type /Page` object is still a page.
    for object in &objects.ordered {
      let Some(dict) = object.dict() else {
        continue;
      };
      if dict.name("Type") == Some("Page") {
        pages.push(page_from(dict, &Inherited::default()));
      }
    }
  }

  if pages.is_empty() {
    // No page objects at all: every stream that is not a typed object is a content stream.
    let default_box = objects
      .ordered
      .iter()
      .find_map(|object| {
        object
          .dict()
          .and_then(|dict| dict.get("MediaBox"))
          .and_then(|value| media_box(value, objects))
      })
      .unwrap_or([0.0; 4]);
    let origin = Point2::new(default_box[0], default_box[1]);
    for object in &objects.ordered {
      if object.stream.is_none() {
        continue;
      }
      let typed = match object.dict() {
        Some(dict) => dict.get("Type").is_some() || dict.get("Subtype").is_some(),
        None => false,
      };
      if typed {
        continue;
      }
      pages.push(Page {
        origin,
        resources: Dict::default(),
        contents: vec![object.number],
      });
    }
    if !pages.is_empty() {
      report.warn("no readable page tree: content streams are imported in document order");
    }
  }

  pages
}

fn walk_page_tree(
  objects: &Objects,
  number: u32,
  inherited: &Inherited,
  visited: &mut BTreeSet<u32>,
  pages: &mut Vec<Page>,
  depth: usize,
) {
  if depth > MAX_PAGE_DEPTH || !visited.insert(number) {
    return;
  }
  let Some(dict) = objects.get(number).and_then(PdfObject::dict) else {
    return;
  };

  let mut inherited = inherited.clone();
  if let Some(value) = dict.get("MediaBox") {
    if let Some(media_box) = media_box(value, objects) {
      inherited.media_box = Some(media_box);
    }
  }
  if let Some(value) = dict.get("Resources") {
    if let Some(resources) = dict_or_ref(value, objects) {
      inherited.resources = Some(resources.clone());
    }
  }

  let kids: Vec<u32> = dict.get("Kids").map(references).unwrap_or_default();
  if dict.name("Type") == Some("Pages") || !kids.is_empty() {
    for kid in kids {
      walk_page_tree(objects, kid, &inherited, visited, pages, depth + 1);
    }
  } else {
    pages.push(page_from(dict, &inherited));
  }
}

fn page_from(dict: &Dict, inherited: &Inherited) -> Page {
  let media_box = inherited.media_box.unwrap_or([0.0; 4]);
  Page {
    origin: Point2::new(media_box[0], media_box[1]),
    resources: inherited.resources.clone().unwrap_or_default(),
    contents: dict.get("Contents").map(references).unwrap_or_default(),
  }
}

/// The page content stream, concatenated across its parts and inflated when it is compressed.
fn page_content(objects: &Objects, page: &Page) -> Result<Vec<u8>, ImportError> {
  let mut content = Vec::new();
  for number in &page.contents {
    let Some(object) = objects.get(*number) else {
      continue;
    };
    let Some(data) = &object.stream else {
      continue;
    };
    let filters = object.dict().map(filter_names).unwrap_or_default();
    let mut data = data.clone();
    for filter in filters {
      match filter.as_str() {
        "FlateDecode" | "Fl" => data = inflate(&data, *number)?,
        other => {
          return Err(ImportError::refused(
            FORMAT,
            format!("{other} content streams are not supported"),
          ))
        }
      }
    }
    content.extend_from_slice(&data);
    content.push(b'\n');
  }
  Ok(content)
}

fn filter_names(dict: &Dict) -> Vec<String> {
  match dict.get("Filter") {
    Some(Value::Name(name)) => vec![name.clone()],
    Some(Value::Array(items)) => items
      .iter()
      .filter_map(|item| match item {
        Value::Name(name) => Some(name.clone()),
        _ => None,
      })
      .collect(),
    _ => Vec::new(),
  }
}

fn inflate(data: &[u8], number: u32) -> Result<Vec<u8>, ImportError> {
  let mut out = Vec::new();
  flate2::read::ZlibDecoder::new(data)
    .read_to_end(&mut out)
    .map_err(|error| {
      ImportError::parse(
        FORMAT,
        format!("the FlateDecode stream could not be inflated: {error}"),
        format!("object {number}"),
      )
    })?;
  Ok(out)
}

/// Type3 glyph procedures are content streams too, and none of them is cut geometry.
fn report_type3_fonts(objects: &Objects, report: &mut ImportReport) {
  for object in &objects.ordered {
    let is_type3 = object
      .dict()
      .and_then(|dict| dict.name("Subtype"))
      .is_some_and(|subtype| subtype == "Type3");
    if is_type3 {
      report.skip(
        "Type3",
        "Type3 glyph procedures are not traced (D-M3-5)",
        Some(format!("object {}", object.number)),
      );
    }
  }
}

/// Which kind of XObject a name in the page's resource dictionary points at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum XObjectKind {
  Image,
  Form,
  Unknown,
}

fn xobject_kinds(objects: &Objects, resources: &Dict) -> BTreeMap<String, XObjectKind> {
  let mut kinds = BTreeMap::new();
  let Some(xobjects) = resources
    .get("XObject")
    .and_then(|value| dict_or_ref(value, objects))
  else {
    return kinds;
  };
  for (name, value) in xobjects.entries() {
    let kind = match dict_or_ref(value, objects)
      .and_then(|dict| dict.get("Subtype"))
      .and_then(name_of)
    {
      Some("Image") => XObjectKind::Image,
      Some("Form") => XObjectKind::Form,
      _ => XObjectKind::Unknown,
    };
    kinds.insert(name.clone(), kind);
  }
  kinds
}

// ---------------------------------------------------------------------------------------------
// Content streams
// ---------------------------------------------------------------------------------------------

/// The painting operator that turns the current path into geometry.
#[derive(Clone, Copy)]
enum Paint {
  Stroke,
  CloseStroke,
  Fill,
  FillStroke,
  CloseFillStroke,
}

impl Paint {
  fn of(operator: &str) -> Option<Self> {
    match operator {
      "S" => Some(Self::Stroke),
      "s" => Some(Self::CloseStroke),
      "f" | "F" | "f*" => Some(Self::Fill),
      "B" | "B*" => Some(Self::FillStroke),
      "b" | "b*" => Some(Self::CloseFillStroke),
      _ => None,
    }
  }

  /// `(closes every subpath, closes the current subpath first)`.
  fn flags(self) -> (bool, bool) {
    match self {
      Self::Stroke => (false, false),
      Self::CloseStroke => (false, true),
      Self::Fill => (true, false),
      Self::FillStroke => (true, false),
      Self::CloseFillStroke => (true, true),
    }
  }
}

/// One subpath under construction, in output coordinates.
struct SubPath {
  points: Vec<Point2>,
  closed: bool,
}

/// The current path: the subpaths the next paint operator will turn into contours.
#[derive(Default)]
struct PathState {
  subpaths: Vec<SubPath>,
}

impl PathState {
  fn current(&self) -> Option<Point2> {
    self
      .subpaths
      .last()
      .and_then(|subpath| subpath.points.last().copied())
  }

  fn move_to(&mut self, point: Point2) {
    self.subpaths.push(SubPath {
      points: vec![point],
      closed: false,
    });
  }

  fn line_to(&mut self, point: Point2) {
    let restart = match self.subpaths.last() {
      None => true,
      Some(subpath) => subpath.closed,
    };
    if restart {
      // After a close, the current point is the closed subpath's start.
      let start = self
        .subpaths
        .last()
        .and_then(|subpath| subpath.points.first().copied());
      let mut points = Vec::new();
      if let Some(start) = start {
        points.push(start);
      }
      self.subpaths.push(SubPath {
        points,
        closed: false,
      });
    }
    if let Some(subpath) = self.subpaths.last_mut() {
      subpath.points.push(point);
    }
  }

  fn push_samples(&mut self, samples: &[Point2]) {
    for point in samples {
      self.line_to(*point);
    }
  }

  fn close(&mut self) {
    if let Some(subpath) = self.subpaths.last_mut() {
      if subpath.points.len() >= 2 {
        // A repeated first point would become a zero-length closing segment.
        if let (Some(first), Some(last)) = (
          subpath.points.first().copied(),
          subpath.points.last().copied(),
        ) {
          if first.is_near(last, POINT_EPSILON) {
            subpath.points.pop();
          }
        }
        subpath.closed = true;
      }
    }
  }
}

/// Everything one content stream changes: the transform, the path, and what it produced.
struct ContentState {
  ctm: Matrix,
  saved: Vec<Matrix>,
  path: PathState,
  contours: Vec<Contour>,
  skipped: Vec<(String, String)>,
  unknown: BTreeSet<String>,
  text_shows: usize,
  tolerance: f64,
}

impl ContentState {
  fn new(origin: Point2, tolerance: f64) -> Self {
    Self {
      // Subtracting the MediaBox origin puts the page's lower-left corner at (0, 0).
      ctm: Matrix::translate(-origin.x, -origin.y),
      saved: Vec::new(),
      path: PathState::default(),
      contours: Vec::new(),
      skipped: Vec::new(),
      unknown: BTreeSet::new(),
      text_shows: 0,
      tolerance: tolerance.max(f64::MIN_POSITIVE),
    }
  }

  /// Interpret one content stream.
  fn run(
    &mut self,
    content: &[u8],
    report: &mut ImportReport,
    xobjects: &BTreeMap<String, XObjectKind>,
  ) {
    let tokens = lex(content);
    let mut operands: Vec<Value> = Vec::new();
    let mut inline_image = false;
    let mut index = 0;

    while index < tokens.len() {
      match tokens.get(index) {
        Some(Token::ArrayStart | Token::DictStart) => {
          if let Some(value) = parse_value(&tokens, &mut index) {
            operands.push(value);
          }
        }
        Some(Token::Keyword(keyword)) => {
          let keyword = keyword.clone();
          index += 1;
          if inline_image {
            if keyword == "EI" {
              inline_image = false;
            }
            continue;
          }
          if keyword == "BI" {
            // `BI … ID <binary> EI`; the data is not token text, so nothing before `EI` is read.
            inline_image = true;
            self.skipped.push((
              "BI".to_string(),
              "inline images are not vector geometry".to_string(),
            ));
            operands.clear();
            continue;
          }
          self.apply(&keyword, &operands, report, xobjects);
          operands.clear();
        }
        Some(other) => {
          if let Some(value) = token_value(other) {
            operands.push(value);
          }
          index += 1;
        }
        None => break,
      }
    }
  }

  fn apply(
    &mut self,
    operator: &str,
    operands: &[Value],
    report: &mut ImportReport,
    xobjects: &BTreeMap<String, XObjectKind>,
  ) {
    match operator {
      "q" => self.saved.push(self.ctm),
      "Q" => {
        if let Some(ctm) = self.saved.pop() {
          self.ctm = ctm;
        }
      }
      "cm" => {
        if let Some([a, b, c, d, e, f]) = last_six(operands) {
          self.ctm = self.ctm.followed_by(Matrix::new(a, b, c, d, e, f));
        }
      }
      "m" => {
        if let (Some(x), Some(y)) = (number(operands, 0), number(operands, 1)) {
          let point = self.ctm.apply(Point2::new(x, y));
          self.path.move_to(point);
          report.count("m", 1);
        }
      }
      "l" => {
        if let (Some(x), Some(y)) = (number(operands, 0), number(operands, 1)) {
          let point = self.ctm.apply(Point2::new(x, y));
          self.path.line_to(point);
          report.count("l", 1);
        }
      }
      "c" => {
        if let (Some(x1), Some(y1), Some(x2), Some(y2), Some(x3), Some(y3)) = (
          number(operands, 0),
          number(operands, 1),
          number(operands, 2),
          number(operands, 3),
          number(operands, 4),
          number(operands, 5),
        ) {
          if let Some(start) = self.path.current() {
            let mut samples = Vec::new();
            flatten_cubic(
              start,
              self.ctm.apply(Point2::new(x1, y1)),
              self.ctm.apply(Point2::new(x2, y2)),
              self.ctm.apply(Point2::new(x3, y3)),
              self.tolerance,
              &mut samples,
            );
            self.path.push_samples(&samples);
            report.count("c", 1);
            report.flatten(1);
          }
        }
      }
      "v" => {
        // The first control point is the current point.
        if let (Some(x2), Some(y2), Some(x3), Some(y3)) = (
          number(operands, 0),
          number(operands, 1),
          number(operands, 2),
          number(operands, 3),
        ) {
          if let Some(start) = self.path.current() {
            let mut samples = Vec::new();
            flatten_cubic(
              start,
              start,
              self.ctm.apply(Point2::new(x2, y2)),
              self.ctm.apply(Point2::new(x3, y3)),
              self.tolerance,
              &mut samples,
            );
            self.path.push_samples(&samples);
            report.count("v", 1);
            report.flatten(1);
          }
        }
      }
      "y" => {
        // The second control point is the end point.
        if let (Some(x1), Some(y1), Some(x3), Some(y3)) = (
          number(operands, 0),
          number(operands, 1),
          number(operands, 2),
          number(operands, 3),
        ) {
          if let Some(start) = self.path.current() {
            let end = self.ctm.apply(Point2::new(x3, y3));
            let mut samples = Vec::new();
            flatten_cubic(
              start,
              self.ctm.apply(Point2::new(x1, y1)),
              end,
              end,
              self.tolerance,
              &mut samples,
            );
            self.path.push_samples(&samples);
            report.count("y", 1);
            report.flatten(1);
          }
        }
      }
      "re" => {
        if let (Some(x), Some(y), Some(width), Some(height)) = (
          number(operands, 0),
          number(operands, 1),
          number(operands, 2),
          number(operands, 3),
        ) {
          self.rectangle(x, y, width, height);
          report.count("re", 1);
        }
      }
      "h" => self.path.close(),
      "n" | "W" | "W*" => {
        // A path that is only clipped or discarded is not a cut path.
        self.path = PathState::default();
      }
      "Do" => self.xobject(operands, xobjects),
      "sh" => self.skipped.push((
        "sh".to_string(),
        "shadings are not vector geometry".to_string(),
      )),
      "Tj" | "TJ" | "'" | "\"" => self.text_shows += 1,
      other => {
        if let Some(paint) = Paint::of(other) {
          self.paint(paint);
          report.count(other, 1);
        } else if !is_geometry_free_operator(other) {
          self.unknown.insert(other.to_string());
        }
      }
    }
  }

  fn rectangle(&mut self, x: f64, y: f64, width: f64, height: f64) {
    let corners = [
      Point2::new(x, y),
      Point2::new(x + width, y),
      Point2::new(x + width, y + height),
      Point2::new(x, y + height),
    ];
    let points = corners.iter().map(|point| self.ctm.apply(*point)).collect();
    self.path.subpaths.push(SubPath {
      points,
      closed: true,
    });
  }

  fn xobject(&mut self, operands: &[Value], xobjects: &BTreeMap<String, XObjectKind>) {
    let Some(name) = operands.first().and_then(name_of) else {
      return;
    };
    let reason = match xobjects.get(name) {
      Some(XObjectKind::Image) => format!("image XObject /{name} is not vector geometry"),
      Some(XObjectKind::Form) => format!("form XObject /{name} is not expanded"),
      _ => format!("XObject /{name} is not in the page resources"),
    };
    self.skipped.push(("Do".to_string(), reason));
  }

  /// Turn the current path into contours, normalising closed ones to counter-clockwise.
  fn paint(&mut self, paint: Paint) {
    let (closes_all, close_current) = paint.flags();
    if close_current {
      self.path.close();
    }
    let path = std::mem::take(&mut self.path);

    for subpath in path.subpaths {
      let closed = closes_all || subpath.closed;
      let mut points: Vec<Point2> = Vec::with_capacity(subpath.points.len());
      for point in subpath.points {
        let duplicate = match points.last() {
          Some(last) => last.is_near(point, POINT_EPSILON),
          None => false,
        };
        if !duplicate {
          points.push(point);
        }
      }
      if closed {
        while points.len() > 1 {
          let first = points.first().copied().unwrap_or(Point2::ORIGIN);
          let last = points.last().copied().unwrap_or(Point2::ORIGIN);
          if first.is_near(last, POINT_EPSILON) {
            points.pop();
          } else {
            break;
          }
        }
      }
      if points.len() < 2 {
        continue;
      }
      match Contour::from_points(&points, closed) {
        Ok(contour) => {
          let contour = if closed && !contour.is_ccw() {
            contour.reverse()
          } else {
            contour
          };
          self.contours.push(contour);
        }
        Err(_) => self.skipped.push((
          "path".to_string(),
          "the painted path is degenerate".to_string(),
        )),
      }
    }
  }

  /// Hand everything the stream could not convert to the report, with the page as its location.
  fn finish(&mut self, report: &mut ImportReport, label: &str) {
    for (construct, reason) in std::mem::take(&mut self.skipped) {
      report.skip(construct, reason, Some(label.to_string()));
    }
    if self.text_shows > 0 {
      let plural = if self.text_shows == 1 { "" } else { "s" };
      report.skip(
        "text",
        format!(
          "text is not traced (D-M3-5); {} text-showing operator{plural} skipped",
          self.text_shows
        ),
        Some(label.to_string()),
      );
    }
    let unknown = std::mem::take(&mut self.unknown);
    if !unknown.is_empty() {
      let operators: Vec<&str> = unknown.iter().map(String::as_str).collect();
      report.warn(format!(
        "{label}: ignored unknown content-stream operators: {}",
        operators.join(", ")
      ));
    }
  }
}

/// Operators that change state or paint but never carry a path.
fn is_geometry_free_operator(operator: &str) -> bool {
  matches!(
    operator,
    "w"
      | "J"
      | "j"
      | "M"
      | "d"
      | "ri"
      | "i"
      | "gs"
      | "CS"
      | "cs"
      | "SC"
      | "SCN"
      | "sc"
      | "scn"
      | "G"
      | "g"
      | "RG"
      | "rg"
      | "K"
      | "k"
      | "BT"
      | "ET"
      | "Tc"
      | "Tw"
      | "Tz"
      | "TL"
      | "Tf"
      | "Tr"
      | "Ts"
      | "Td"
      | "TD"
      | "Tm"
      | "T*"
      | "MP"
      | "DP"
      | "BMC"
      | "BDC"
      | "EMC"
      | "BX"
      | "EX"
      | "d0"
      | "d1"
      | "ID"
      | "EI"
  )
}

// ---------------------------------------------------------------------------------------------
// The current transformation matrix
// ---------------------------------------------------------------------------------------------

/// The affine part of a PDF transformation matrix, laid out as `[a b c d e f]`.
#[derive(Clone, Copy)]
struct Matrix {
  a: f64,
  b: f64,
  c: f64,
  d: f64,
  e: f64,
  f: f64,
}

impl Matrix {
  fn new(a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) -> Self {
    Self { a, b, c, d, e, f }
  }

  fn translate(x: f64, y: f64) -> Self {
    Self::new(1.0, 0.0, 0.0, 1.0, x, y)
  }

  fn apply(self, point: Point2) -> Point2 {
    Point2::new(
      self.a * point.x + self.c * point.y + self.e,
      self.b * point.x + self.d * point.y + self.f,
    )
  }

  /// The matrix that applies `self` first and `after` second, which is what `cm` concatenates.
  fn followed_by(self, after: Self) -> Self {
    Self {
      a: after.a * self.a + after.c * self.b,
      b: after.b * self.a + after.d * self.b,
      c: after.a * self.c + after.c * self.d,
      d: after.b * self.c + after.d * self.d,
      e: after.a * self.e + after.c * self.f + after.e,
      f: after.b * self.e + after.d * self.f + after.f,
    }
  }
}

// ---------------------------------------------------------------------------------------------
// Curve flattening
// ---------------------------------------------------------------------------------------------

/// Append the samples of a cubic Bézier, already in output space, that stay within `tolerance`.
fn flatten_cubic(
  start: Point2,
  control1: Point2,
  control2: Point2,
  end: Point2,
  tolerance: f64,
  out: &mut Vec<Point2>,
) {
  subdivide(start, control1, control2, end, tolerance, 0, out);
}

fn subdivide(
  p0: Point2,
  p1: Point2,
  p2: Point2,
  p3: Point2,
  tolerance: f64,
  depth: usize,
  out: &mut Vec<Point2>,
) {
  if depth >= MAX_CURVE_DEPTH || is_flat(p0, p1, p2, p3, tolerance) {
    out.push(p3);
    return;
  }
  let p01 = p0.lerp(p1, 0.5);
  let p12 = p1.lerp(p2, 0.5);
  let p23 = p2.lerp(p3, 0.5);
  let p012 = p01.lerp(p12, 0.5);
  let p123 = p12.lerp(p23, 0.5);
  let middle = p012.lerp(p123, 0.5);
  subdivide(p0, p01, p012, middle, tolerance, depth + 1, out);
  subdivide(middle, p123, p23, p3, tolerance, depth + 1, out);
}

/// True when the chord `p0 p3` is within `tolerance` of the whole curve.
///
/// A curve stays within three quarters of the larger control-point offset from its chord, so the
/// test bounds the real deviation and not just the control polygon.
fn is_flat(p0: Point2, p1: Point2, p2: Point2, p3: Point2, tolerance: f64) -> bool {
  let chord = LineSeg::new(p0, p3);
  let offset = chord.distance_to_point(p1).max(chord.distance_to_point(p2));
  0.75 * offset <= tolerance
}

// ---------------------------------------------------------------------------------------------
// Values
// ---------------------------------------------------------------------------------------------

/// One PDF value: enough of the object grammar for dictionaries, arrays, references and numbers.
#[derive(Clone, Debug, PartialEq)]
enum Value {
  Number(f64),
  Name(String),
  Str(Vec<u8>),
  Bool(bool),
  Null,
  Array(Vec<Value>),
  Ref(u32),
  Dict(Dict),
}

/// A PDF dictionary, kept in the order its entries were written.
#[derive(Clone, Debug, Default, PartialEq)]
struct Dict(Vec<(String, Value)>);

impl Dict {
  fn get(&self, key: &str) -> Option<&Value> {
    self
      .0
      .iter()
      .find(|(name, _)| name == key)
      .map(|(_, value)| value)
  }

  fn name(&self, key: &str) -> Option<&str> {
    self.get(key).and_then(name_of)
  }

  fn entries(&self) -> &[(String, Value)] {
    &self.0
  }
}

fn name_of(value: &Value) -> Option<&str> {
  match value {
    Value::Name(name) => Some(name),
    _ => None,
  }
}

fn token_value(token: &Token) -> Option<Value> {
  Some(match token {
    Token::Number(number) => Value::Number(*number),
    Token::Name(name) => Value::Name(name.clone()),
    Token::Str(bytes) => Value::Str(bytes.clone()),
    Token::Bool(value) => Value::Bool(*value),
    Token::Null => Value::Null,
    _ => return None,
  })
}

fn number(operands: &[Value], index: usize) -> Option<f64> {
  match operands.get(index) {
    Some(Value::Number(value)) => Some(*value),
    _ => None,
  }
}

/// The last six numbers of the operand list, which is how `cm` and its friends are called.
fn last_six(operands: &[Value]) -> Option<[f64; 6]> {
  if operands.len() < 6 {
    return None;
  }
  let tail = &operands[operands.len() - 6..];
  let mut out = [0.0; 6];
  for (slot, value) in out.iter_mut().zip(tail) {
    match value {
      Value::Number(number) => *slot = *number,
      _ => return None,
    }
  }
  Some(out)
}

/// Every indirect reference in a value: a single reference or an array of them.
fn references(value: &Value) -> Vec<u32> {
  match value {
    Value::Ref(number) => vec![*number],
    Value::Array(items) => items
      .iter()
      .filter_map(|item| match item {
        Value::Ref(number) => Some(*number),
        _ => None,
      })
      .collect(),
    _ => Vec::new(),
  }
}

fn dict_or_ref<'a>(value: &'a Value, objects: &'a Objects) -> Option<&'a Dict> {
  match value {
    Value::Dict(dict) => Some(dict),
    Value::Ref(number) => objects.get(*number).and_then(PdfObject::dict),
    _ => None,
  }
}

fn numbers_of(value: &Value) -> Option<Vec<f64>> {
  match value {
    Value::Array(items) => items
      .iter()
      .map(|item| match item {
        Value::Number(number) => Some(*number),
        _ => None,
      })
      .collect(),
    _ => None,
  }
}

/// A `/MediaBox` value, which may be written inline or held in its own object.
fn media_box(value: &Value, objects: &Objects) -> Option<[f64; 4]> {
  let numbers = match value {
    Value::Ref(number) => numbers_of(&objects.get(*number)?.value)?,
    other => numbers_of(other)?,
  };
  match numbers.as_slice() {
    [x0, y0, x1, y1] => Some([*x0, *y0, *x1, *y1]),
    _ => None,
  }
}

// ---------------------------------------------------------------------------------------------
// Lexing and parsing
// ---------------------------------------------------------------------------------------------

/// One lexical token: the object grammar and content streams share it.
#[derive(Clone, Debug, PartialEq)]
enum Token {
  Number(f64),
  Name(String),
  Str(Vec<u8>),
  Bool(bool),
  Null,
  ArrayStart,
  ArrayEnd,
  DictStart,
  DictEnd,
  Keyword(String),
}

fn lex(bytes: &[u8]) -> Vec<Token> {
  let mut tokens = Vec::new();
  let mut index = 0;

  while index < bytes.len() {
    let byte = bytes[index];
    if is_whitespace(byte) {
      index += 1;
      continue;
    }
    match byte {
      b'%' => {
        while index < bytes.len() && !matches!(bytes[index], b'\n' | b'\r') {
          index += 1;
        }
      }
      b'(' => {
        let (value, next) = lex_literal_string(bytes, index);
        tokens.push(Token::Str(value));
        index = next;
      }
      b'<' => {
        if bytes.get(index + 1) == Some(&b'<') {
          tokens.push(Token::DictStart);
          index += 2;
        } else {
          let (value, next) = lex_hex_string(bytes, index);
          tokens.push(Token::Str(value));
          index = next;
        }
      }
      b'>' => {
        if bytes.get(index + 1) == Some(&b'>') {
          tokens.push(Token::DictEnd);
          index += 2;
        } else {
          index += 1;
        }
      }
      b'[' => {
        tokens.push(Token::ArrayStart);
        index += 1;
      }
      b']' => {
        tokens.push(Token::ArrayEnd);
        index += 1;
      }
      b'/' => {
        let (name, next) = lex_name(bytes, index);
        tokens.push(Token::Name(name));
        index = next;
      }
      b')' | b'{' | b'}' => index += 1,
      _ => {
        let start = index;
        while index < bytes.len() && !is_whitespace(bytes[index]) && !is_delimiter(bytes[index]) {
          index += 1;
        }
        let word = String::from_utf8_lossy(&bytes[start..index]).into_owned();
        tokens.push(match word.as_str() {
          "true" => Token::Bool(true),
          "false" => Token::Bool(false),
          "null" => Token::Null,
          _ => match word.parse::<f64>() {
            Ok(number) => Token::Number(number),
            Err(_) => Token::Keyword(word),
          },
        });
      }
    }
  }

  tokens
}

/// A `( … )` string, with nesting and the escapes PDF defines.
fn lex_literal_string(bytes: &[u8], start: usize) -> (Vec<u8>, usize) {
  let mut out = Vec::new();
  let mut depth = 1usize;
  let mut index = start + 1;
  while index < bytes.len() {
    let byte = bytes[index];
    match byte {
      b'\\' => {
        index += 1;
        match bytes.get(index).copied() {
          Some(b'n') => {
            out.push(b'\n');
            index += 1;
          }
          Some(b'r') => {
            out.push(b'\r');
            index += 1;
          }
          Some(b't') => {
            out.push(b'\t');
            index += 1;
          }
          Some(b'b') => {
            out.push(0x08);
            index += 1;
          }
          Some(b'f') => {
            out.push(0x0c);
            index += 1;
          }
          Some(b'\r') => {
            index += 1;
            if bytes.get(index) == Some(&b'\n') {
              index += 1;
            }
          }
          Some(b'\n') => index += 1,
          Some(b'0'..=b'7') => {
            let mut value: u32 = 0;
            let mut digits = 0;
            while digits < 3 {
              match bytes.get(index).copied() {
                Some(digit @ b'0'..=b'7') => {
                  value = value * 8 + u32::from(digit - b'0');
                  index += 1;
                  digits += 1;
                }
                _ => break,
              }
            }
            out.push(value as u8);
          }
          Some(other) => {
            out.push(other);
            index += 1;
          }
          None => break,
        }
      }
      b'(' => {
        depth += 1;
        out.push(byte);
        index += 1;
      }
      b')' => {
        index += 1;
        depth -= 1;
        if depth == 0 {
          break;
        }
        out.push(b')');
      }
      _ => {
        out.push(byte);
        index += 1;
      }
    }
  }
  (out, index)
}

/// A `< … >` hexadecimal string.
fn lex_hex_string(bytes: &[u8], start: usize) -> (Vec<u8>, usize) {
  let mut out = Vec::new();
  let mut high: Option<u8> = None;
  let mut index = start + 1;
  while index < bytes.len() {
    let byte = bytes[index];
    index += 1;
    if byte == b'>' {
      break;
    }
    let Some(digit) = hex_digit(byte) else {
      continue;
    };
    match high.take() {
      Some(first) => out.push(first * 16 + digit),
      None => high = Some(digit),
    }
  }
  if let Some(first) = high {
    out.push(first * 16);
  }
  (out, index)
}

fn hex_digit(byte: u8) -> Option<u8> {
  match byte {
    b'0'..=b'9' => Some(byte - b'0'),
    b'a'..=b'f' => Some(byte - b'a' + 10),
    b'A'..=b'F' => Some(byte - b'A' + 10),
    _ => None,
  }
}

fn lex_name(bytes: &[u8], start: usize) -> (String, usize) {
  let mut out = String::new();
  let mut index = start + 1;
  while index < bytes.len() && !is_whitespace(bytes[index]) && !is_delimiter(bytes[index]) {
    if bytes[index] == b'#' {
      let escape = bytes.get(index + 1..index + 3).and_then(|pair| {
        let text = std::str::from_utf8(pair).ok()?;
        u8::from_str_radix(text, 16).ok()
      });
      if let Some(value) = escape {
        out.push(char::from(value));
        index += 3;
        continue;
      }
    }
    out.push(char::from(bytes[index]));
    index += 1;
  }
  (out, index)
}

/// Parse one value from `tokens` at `index`, leaving `index` after it.
fn parse_value(tokens: &[Token], index: &mut usize) -> Option<Value> {
  let token = tokens.get(*index)?.clone();
  *index += 1;
  Some(match token {
    Token::Number(number) => {
      // `N G R` is a reference; the generation number is not needed to resolve it.
      if let (Some(Token::Number(_)), Some(Token::Keyword(keyword))) =
        (tokens.get(*index), tokens.get(*index + 1))
      {
        if keyword == "R" {
          *index += 2;
          return Some(Value::Ref(number.max(0.0) as u32));
        }
      }
      Value::Number(number)
    }
    Token::Name(name) => Value::Name(name),
    Token::Str(bytes) => Value::Str(bytes),
    Token::Bool(value) => Value::Bool(value),
    Token::Null => Value::Null,
    Token::ArrayStart => {
      let mut items = Vec::new();
      while let Some(token) = tokens.get(*index) {
        if matches!(token, Token::ArrayEnd) {
          *index += 1;
          break;
        }
        match parse_value(tokens, index) {
          Some(value) => items.push(value),
          None => break,
        }
      }
      Value::Array(items)
    }
    Token::DictStart => {
      let mut entries = Vec::new();
      while let Some(token) = tokens.get(*index) {
        if matches!(token, Token::DictEnd) {
          *index += 1;
          break;
        }
        let key = match token {
          Token::Name(key) => key.clone(),
          _ => {
            *index += 1;
            continue;
          }
        };
        *index += 1;
        match parse_value(tokens, index) {
          Some(value) => entries.push((key, value)),
          None => break,
        }
      }
      Value::Dict(Dict(entries))
    }
    Token::ArrayEnd | Token::DictEnd | Token::Keyword(_) => return None,
  })
}

/// The first value in `bytes`, or `Null` when it holds none.
fn first_value(bytes: &[u8]) -> Value {
  let tokens = lex(bytes);
  let mut index = 0;
  parse_value(&tokens, &mut index).unwrap_or(Value::Null)
}

// ---------------------------------------------------------------------------------------------
// Byte scanning
// ---------------------------------------------------------------------------------------------

fn is_whitespace(byte: u8) -> bool {
  matches!(byte, b'\0' | b'\t' | b'\n' | b'\x0c' | b'\r' | b' ')
}

fn is_delimiter(byte: u8) -> bool {
  matches!(
    byte,
    b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
  )
}

fn is_regular(byte: u8) -> bool {
  !is_whitespace(byte) && !is_delimiter(byte)
}

fn skip_whitespace(bytes: &[u8], from: usize) -> usize {
  let mut index = from;
  while index < bytes.len() && is_whitespace(bytes[index]) {
    index += 1;
  }
  index
}

/// The first occurrence of `needle` at or after `from`.
fn find_bytes(bytes: &[u8], needle: &[u8], from: usize) -> Option<usize> {
  if needle.is_empty() {
    return None;
  }
  bytes
    .get(from..)?
    .windows(needle.len())
    .position(|window| window == needle)
    .map(|offset| offset + from)
}

/// The first occurrence of `word` as a whole token, so `obj` never matches inside `endobj`.
fn find_keyword(bytes: &[u8], word: &[u8], from: usize) -> Option<usize> {
  let mut cursor = from;
  while let Some(index) = find_bytes(bytes, word, cursor) {
    let before_ok = index == 0 || !is_regular(bytes[index - 1]);
    let after = index + word.len();
    let after_ok = after >= bytes.len() || !is_regular(bytes[after]);
    if before_ok && after_ok {
      return Some(index);
    }
    cursor = index + 1;
  }
  None
}

/// The span of the dictionary that starts at or after `from`, `<<` and its `>>` included.
fn dict_span(bytes: &[u8], from: usize) -> Option<(usize, usize)> {
  let start = find_bytes(bytes, b"<<", from)?;
  let mut depth = 0usize;
  let mut index = start;
  while index + 1 < bytes.len() {
    if bytes.get(index..index + 2) == Some(b"<<") {
      depth += 1;
      index += 2;
    } else if bytes.get(index..index + 2) == Some(b">>") {
      depth -= 1;
      index += 2;
      if depth == 0 {
        return Some((start, index));
      }
    } else {
      index += 1;
    }
  }
  None
}
