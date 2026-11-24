//! DXF import: group-code stream, layers, and the exact entity types we can cut.
//!
//! Supported entities: `LINE`, `LWPOLYLINE` (with bulges), `POLYLINE`/`VERTEX`/`SEQEND` (with
//! bulges), `ARC`, `CIRCLE`, `ELLIPSE`. A bulge becomes a [`cam_geom::Arc`], never a polyline.
//! Unknown entity types are skipped and reported; a malformed group code is a located parse error.
//!
//! # Acceptance criteria
//!
//! * **A4** — the entity set above, each DXF layer carried through as one [`crate::ImportedLayer`]
//!   under the name the file gives it, and `$INSUNITS`/`$MEASUREMENT` deciding the millimetre
//!   scale (D-M3-3).
//! * **A5** — a malformed group code is refused with the line number that carries it, and an entity
//!   type this importer does not read is skipped and reported with its own line, never fatal.
//!
//! Everything that is met and not converted is in the report: an entity type outside the set above,
//! an entity missing a required group code, and a partial ellipse (flattened and counted at the
//! caller's tolerance, per D-M3-2).

use std::f64::consts::TAU;
use std::ops::Range;

use cam_geom::{primitive, transform, Arc, Contour, LineSeg, Point2, Segment, Vec2, Vertex};

use crate::{ImportError, ImportOptions, ImportReport, ImportedLayer};

/// The format name every error and skip in this module carries.
const FORMAT: &str = "dxf";

/// How a binary DXF announces itself. ASCII DXF is the topic of this module.
const BINARY_SIGNATURE: &[u8] = b"AutoCAD Binary DXF";

/// Angles closer than this (in degrees) are the same angle.
const ANGLE_EPSILON: f64 = 1.0e-9;

/// Deepest adaptive subdivision of a flattened curve: `2^12 = 4096` samples, the kernel's bound.
const MAX_FLATTEN_DEPTH: usize = 12;

/// The reason an entity type outside the supported set carries.
const UNSUPPORTED_ENTITY: &str =
  "not a supported DXF entity: dxf imports LINE, LWPOLYLINE, POLYLINE, ARC, CIRCLE and ELLIPSE";

/// Import an ASCII DXF.
///
/// `report.contours` is left to the caller: it counts what came out of all importers at once.
pub fn import(
  bytes: &[u8],
  options: &ImportOptions,
  report: &mut ImportReport,
) -> Result<Vec<ImportedLayer>, ImportError> {
  let pairs = tokenize(bytes)?;
  let sections = sections(&pairs);

  let mut header = Header::default();
  let mut layer_names: Vec<String> = Vec::new();
  for (name, range) in &sections {
    match name.as_str() {
      "HEADER" => header = read_header(&pairs[range.clone()]),
      "TABLES" => layer_names = read_layer_table(&pairs[range.clone()]),
      _ => {}
    }
  }

  let scale = unit_scale(&header, report)?;
  report.unit_scale = scale;

  // Layer-table order first; a layer that only appears on entities is appended when it is first
  // met, so the output order never depends on a hash.
  let mut layers: Vec<(String, Vec<Contour>)> = layer_names
    .into_iter()
    .map(|name| (name, Vec::new()))
    .collect();

  if let Some((_, range)) = sections.iter().find(|(name, _)| name == "ENTITIES") {
    read_entities(&pairs[range.clone()], &mut layers, options, report)?;
  }

  let mut imported: Vec<ImportedLayer> = Vec::new();
  for (name, contours) in layers {
    if contours.is_empty() {
      continue;
    }
    let mut scaled = Vec::with_capacity(contours.len());
    for contour in contours {
      scaled.push(scale_contour(contour, scale, options)?);
    }
    imported.push(ImportedLayer {
      name,
      contours: scaled,
    });
  }
  Ok(imported)
}

// ---------------------------------------------------------------------------------------------
// The group-code stream.
// ---------------------------------------------------------------------------------------------

/// One group-code pair: the code, its value, and the line the code is on.
struct Pair {
  code: i32,
  value: String,
  line: usize,
}

/// Read the stream of `code` / `value` line pairs.
fn tokenize(bytes: &[u8]) -> Result<Vec<Pair>, ImportError> {
  if bytes.starts_with(BINARY_SIGNATURE) {
    return Err(ImportError::refused(
      FORMAT,
      "binary DXF is not supported: re-save the drawing as ASCII DXF",
    ));
  }

  let text = String::from_utf8_lossy(bytes);
  let lines: Vec<&str> = text.lines().collect();
  let mut pairs = Vec::with_capacity(lines.len() / 2);
  let mut index = 0;
  while index < lines.len() {
    let code_line = lines[index].trim();
    let code = code_line.parse::<i32>().map_err(|_| {
      ImportError::parse(
        FORMAT,
        format!("group code {code_line:?} is not an integer"),
        format!("line {}", index + 1),
      )
    })?;
    let Some(value_line) = lines.get(index + 1) else {
      return Err(ImportError::parse(
        FORMAT,
        format!("group code {code} has no value line"),
        format!("line {}", index + 1),
      ));
    };
    pairs.push(Pair {
      code,
      value: value_line.trim().to_string(),
      line: index + 1,
    });
    index += 2;
  }
  Ok(pairs)
}

/// The `SECTION`/`ENDSEC` pairs, with the name each section carries.
///
/// A section with no `ENDSEC` runs to the end of the file rather than failing: a truncated file is
/// still worth importing as far as it goes.
fn sections(pairs: &[Pair]) -> Vec<(String, Range<usize>)> {
  let mut out = Vec::new();
  let mut index = 0;
  while index < pairs.len() {
    if pairs[index].code != 0 || !pairs[index].value.trim().eq_ignore_ascii_case("SECTION") {
      index += 1;
      continue;
    }
    let name = pairs
      .get(index + 1)
      .filter(|pair| pair.code == 2)
      .map(|pair| pair.value.trim().to_ascii_uppercase())
      .unwrap_or_default();
    let start = (index + 2).min(pairs.len());
    let mut end = start;
    while end < pairs.len() {
      if pairs[end].code == 0 && pairs[end].value.trim().eq_ignore_ascii_case("ENDSEC") {
        break;
      }
      end += 1;
    }
    out.push((name, start..end));
    index = end + 1;
  }
  out
}

/// One `0`-record: its type, the line it starts on, and the pairs that follow it.
struct Record {
  kind: String,
  line: usize,
  fields: Range<usize>,
}

/// Split a run of pairs into the records that each start with code `0`.
fn split_records(pairs: &[Pair]) -> Vec<Record> {
  let mut records = Vec::new();
  let mut index = 0;
  while index < pairs.len() {
    if pairs[index].code != 0 {
      index += 1;
      continue;
    }
    let start = index + 1;
    let mut end = start;
    while end < pairs.len() && pairs[end].code != 0 {
      end += 1;
    }
    records.push(Record {
      kind: pairs[index].value.trim().to_ascii_uppercase(),
      line: pairs[index].line,
      fields: start..end,
    });
    index = end;
  }
  records
}

// ---------------------------------------------------------------------------------------------
// Header variables and units (D-M3-3).
// ---------------------------------------------------------------------------------------------

/// A header variable's raw value, and where it was read from.
struct HeaderValue {
  raw: String,
  line: usize,
}

impl HeaderValue {
  fn code(&self, variable: &str) -> Result<i64, ImportError> {
    self.raw.parse::<i64>().map_err(|_| {
      ImportError::parse(
        FORMAT,
        format!("{variable} value {:?} is not an integer", self.raw),
        format!("line {}", self.line),
      )
    })
  }
}

/// The header variables this importer reads.
#[derive(Default)]
struct Header {
  insunits: Option<HeaderValue>,
  measurement: Option<HeaderValue>,
}

fn read_header(pairs: &[Pair]) -> Header {
  let mut header = Header::default();
  let mut index = 0;
  while index < pairs.len() {
    if pairs[index].code != 9 {
      index += 1;
      continue;
    }
    let variable = pairs[index].value.trim().to_ascii_uppercase();
    let mut value = None;
    let mut look = index + 1;
    while look < pairs.len() && pairs[look].code != 9 {
      if pairs[look].code == 70 {
        value = Some(HeaderValue {
          raw: pairs[look].value.clone(),
          line: pairs[look].line,
        });
        break;
      }
      look += 1;
    }
    match variable.as_str() {
      "$INSUNITS" => header.insunits = value,
      "$MEASUREMENT" => header.measurement = value,
      _ => {}
    }
    index = look.max(index + 1);
  }
  header
}

/// The factor that turns the file's coordinates into millimetres.
fn unit_scale(header: &Header, report: &mut ImportReport) -> Result<f64, ImportError> {
  if let Some(value) = &header.insunits {
    let scale = match value.code("$INSUNITS")? {
      1 => Some(25.4),
      2 => Some(304.8),
      4 => Some(1.0),
      5 => Some(10.0),
      6 => Some(1000.0),
      0 => None,
      other => {
        return Err(ImportError::refused(
          FORMAT,
          format!(
            "$INSUNITS {other} is a unit this importer does not know: supported codes are 0 \
             (unitless), 1 (inch), 2 (foot), 4 (millimetre), 5 (centimetre) and 6 (metre)"
          ),
        ))
      }
    };
    if let Some(scale) = scale {
      return Ok(scale);
    }
    report.warn("$INSUNITS is 0 (unitless): falling back to $MEASUREMENT");
  }

  if let Some(value) = &header.measurement {
    match value.code("$MEASUREMENT")? {
      1 => {
        report.warn("no $INSUNITS and $MEASUREMENT is 1 (metric): millimetres are assumed");
        return Ok(1.0);
      }
      0 => {
        report.warn("no $INSUNITS and $MEASUREMENT is 0 (imperial): inches are assumed");
        return Ok(25.4);
      }
      other => {
        return Err(ImportError::refused(
          FORMAT,
          format!("$MEASUREMENT {other} is not a known value: 0 is imperial and 1 is metric"),
        ))
      }
    }
  }

  report.warn("no $INSUNITS and no $MEASUREMENT: millimetres are assumed");
  Ok(1.0)
}

// ---------------------------------------------------------------------------------------------
// The layer table.
// ---------------------------------------------------------------------------------------------

/// The layer names the `TABLES` section declares, in table order.
fn read_layer_table(pairs: &[Pair]) -> Vec<String> {
  let mut names: Vec<String> = Vec::new();
  for record in split_records(pairs) {
    if record.kind != "LAYER" {
      continue;
    }
    let fields = &pairs[record.fields.clone()];
    let Some(pair) = fields.iter().find(|pair| pair.code == 2) else {
      continue;
    };
    let name = pair.value.trim().to_string();
    if !name.is_empty() && !names.contains(&name) {
      names.push(name);
    }
  }
  names
}

// ---------------------------------------------------------------------------------------------
// Entities.
// ---------------------------------------------------------------------------------------------

/// The fields of one record, with the accessors that turn a malformed number into a parse error.
struct Fields<'a> {
  pairs: &'a [Pair],
}

impl<'a> Fields<'a> {
  fn new(pairs: &'a [Pair], record: &Record) -> Self {
    Self {
      pairs: &pairs[record.fields.clone()],
    }
  }

  fn pairs(&self) -> &'a [Pair] {
    self.pairs
  }

  fn text(&self, code: i32) -> Option<&'a str> {
    self
      .pairs
      .iter()
      .find(|pair| pair.code == code)
      .map(|pair| pair.value.as_str())
  }

  fn number(&self, code: i32) -> Result<Option<f64>, Stop> {
    let Some(pair) = self.pairs.iter().find(|pair| pair.code == code) else {
      return Ok(None);
    };
    number(pair).map(Some)
  }

  fn integer(&self, code: i32) -> Result<Option<i64>, Stop> {
    let Some(pair) = self.pairs.iter().find(|pair| pair.code == code) else {
      return Ok(None);
    };
    pair
      .value
      .parse::<i64>()
      .map(Some)
      .map_err(|_| not_an_integer(pair))
  }

  fn require(&self, code: i32, when_missing: &str) -> Result<f64, Stop> {
    self
      .number(code)?
      .ok_or_else(|| Stop::Skip(when_missing.to_string()))
  }
}

/// Why an entity produced no geometry.
enum Stop {
  /// The file cannot be read past this point.
  Parse(ImportError),
  /// The construct is reported as skipped and the import carries on.
  Skip(String),
}

fn not_a_number(pair: &Pair) -> Stop {
  Stop::Parse(ImportError::parse(
    FORMAT,
    format!(
      "group code {} value {:?} is not a number",
      pair.code, pair.value
    ),
    format!("line {}", pair.line),
  ))
}

fn not_an_integer(pair: &Pair) -> Stop {
  Stop::Parse(ImportError::parse(
    FORMAT,
    format!(
      "group code {} value {:?} is not an integer",
      pair.code, pair.value
    ),
    format!("line {}", pair.line),
  ))
}

fn number(pair: &Pair) -> Result<f64, Stop> {
  pair.value.parse::<f64>().map_err(|_| not_a_number(pair))
}

/// Turn every entity record into geometry, or into a reported skip.
fn read_entities(
  pairs: &[Pair],
  layers: &mut Vec<(String, Vec<Contour>)>,
  options: &ImportOptions,
  report: &mut ImportReport,
) -> Result<(), ImportError> {
  let records = split_records(pairs);
  let mut index = 0;
  while index < records.len() {
    let record = &records[index];
    let fields = Fields::new(pairs, record);
    let kind = record.kind.as_str();

    let converted = match kind {
      "LINE" => line(&fields),
      "LWPOLYLINE" => lwpolyline(&fields),
      "POLYLINE" => {
        let mut vertices = Vec::new();
        let mut next = index + 1;
        while next < records.len() && records[next].kind == "VERTEX" {
          vertices.push(Fields::new(pairs, &records[next]));
          next += 1;
        }
        if next < records.len() && records[next].kind == "SEQEND" {
          next += 1;
        }
        index = next.min(records.len()) - 1;
        polyline(&fields, &vertices)
      }
      "ARC" => arc(&fields),
      "CIRCLE" => circle(&fields),
      "ELLIPSE" => ellipse_entity(&fields, options, report),
      "INSERT" => Err(Stop::Skip(
        "block references are not expanded: geometry inside BLOCKS is not imported".to_string(),
      )),
      "VERTEX" | "SEQEND" => Err(Stop::Skip(format!(
        "a {kind} record with no POLYLINE around it"
      ))),
      _ => Err(Stop::Skip(UNSUPPORTED_ENTITY.to_string())),
    };

    match converted {
      Ok(contour) => {
        report.count(kind, 1);
        let name = fields
          .text(8)
          .filter(|name| !name.is_empty())
          .unwrap_or("0");
        add_contour(layers, name, contour);
      }
      Err(Stop::Skip(reason)) => {
        report.skip(kind, reason, Some(format!("line {}", record.line)));
      }
      Err(Stop::Parse(error)) => return Err(error),
    }
    index += 1;
  }
  Ok(())
}

/// File a contour under its layer, appending a layer that is not in the table yet.
fn add_contour(layers: &mut Vec<(String, Vec<Contour>)>, layer: &str, contour: Contour) {
  if let Some(entry) = layers.iter_mut().find(|(name, _)| name == layer) {
    entry.1.push(contour);
    return;
  }
  layers.push((layer.to_string(), vec![contour]));
}

/// A closed contour is always counter-clockwise (the kernel's contract for authored geometry).
fn oriented(contour: Contour) -> Contour {
  if contour.is_closed() && !contour.is_ccw() {
    contour.reverse()
  } else {
    contour
  }
}

/// Turn a kernel builder result into geometry, or into a reported skip.
fn geometry(shape: Result<Contour, cam_geom::GeomError>) -> Result<Contour, Stop> {
  match shape {
    Ok(contour) => Ok(oriented(contour)),
    Err(error) => Err(Stop::Skip(format!("cannot build the contour: {error}"))),
  }
}

fn contour(vertices: Vec<Vertex>, closed: bool) -> Result<Contour, Stop> {
  geometry(Contour::new(vertices, closed))
}

/// A required point, or the reason the entity is skipped.
fn point(fields: &Fields, x_code: i32, y_code: i32, when_missing: &str) -> Result<Point2, Stop> {
  match (fields.number(x_code)?, fields.number(y_code)?) {
    (Some(x), Some(y)) => Ok(Point2::new(x, y)),
    _ => Err(Stop::Skip(when_missing.to_string())),
  }
}

fn line(fields: &Fields) -> Result<Contour, Stop> {
  let start = point(
    fields,
    10,
    20,
    "missing the start point (group codes 10 and 20)",
  )?;
  let end = point(
    fields,
    11,
    21,
    "missing the end point (group codes 11 and 21)",
  )?;
  contour(vec![Vertex::corner(start), Vertex::corner(end)], false)
}

fn circle(fields: &Fields) -> Result<Contour, Stop> {
  let center = point(fields, 10, 20, "missing the centre (group codes 10 and 20)")?;
  let radius = fields.require(40, "missing the radius (group code 40)")?;
  geometry(primitive::circle(center, radius))
}

fn arc(fields: &Fields) -> Result<Contour, Stop> {
  let center = point(fields, 10, 20, "missing the centre (group codes 10 and 20)")?;
  let radius = fields.require(40, "missing the radius (group code 40)")?;
  let start = fields.require(50, "missing the start angle (group code 50)")?;
  let end = fields.require(51, "missing the end angle (group code 51)")?;

  // DXF arcs run counter-clockwise from the start angle to the end angle; a sweep written as a
  // full turn is a circle, and one written as no turn has no length.
  let raw = end - start;
  if raw.abs() >= 360.0 - ANGLE_EPSILON {
    return geometry(primitive::circle(center, radius));
  }
  let sweep = raw.rem_euclid(360.0);
  if sweep <= ANGLE_EPSILON {
    return Err(Stop::Skip(
      "the start and end angles coincide, so the arc has no length".to_string(),
    ));
  }

  let arc = Arc::new(center, radius, start.to_radians(), sweep.to_radians());
  geometry(arc.and_then(|arc| Contour::from_segments(&[Segment::Arc(arc)], false)))
}

fn lwpolyline(fields: &Fields) -> Result<Contour, Stop> {
  let closed = closed_flag(fields)?;
  let declared = fields.integer(90)?;
  let mut raw: Vec<RawVertex> = Vec::new();
  for pair in fields.pairs() {
    match pair.code {
      10 => raw.push(RawVertex {
        x: number(pair)?,
        y: None,
        bulge: 0.0,
      }),
      20 => match raw.last_mut() {
        Some(vertex) => vertex.y = Some(number(pair)?),
        None => {
          return Err(Stop::Skip(
            "a Y coordinate appears before any vertex (group code 10)".to_string(),
          ))
        }
      },
      42 => match raw.last_mut() {
        Some(vertex) => vertex.bulge = number(pair)?,
        None => {
          return Err(Stop::Skip(
            "a bulge appears before any vertex (group code 10)".to_string(),
          ))
        }
      },
      _ => {}
    }
  }
  finish_polyline(raw, declared, closed)
}

fn polyline(fields: &Fields, vertex_records: &[Fields]) -> Result<Contour, Stop> {
  let closed = closed_flag(fields)?;
  if vertex_records.is_empty() {
    return Err(Stop::Skip("the polyline has no VERTEX records".to_string()));
  }
  let mut raw = Vec::with_capacity(vertex_records.len());
  for vertex in vertex_records {
    let point = point(
      vertex,
      10,
      20,
      "a VERTEX record is missing its point (group codes 10 and 20)",
    )?;
    raw.push(RawVertex {
      x: point.x,
      y: Some(point.y),
      bulge: vertex.number(42)?.unwrap_or(0.0),
    });
  }
  finish_polyline(raw, None, closed)
}

/// One vertex as the group codes read it, before it becomes a [`Vertex`].
struct RawVertex {
  x: f64,
  y: Option<f64>,
  bulge: f64,
}

fn finish_polyline(
  raw: Vec<RawVertex>,
  declared: Option<i64>,
  closed: bool,
) -> Result<Contour, Stop> {
  if let Some(declared) = declared {
    if declared != raw.len() as i64 {
      return Err(Stop::Skip(format!(
        "declares {declared} vertices but carries {}",
        raw.len()
      )));
    }
  }
  let mut vertices = Vec::with_capacity(raw.len());
  for vertex in raw {
    let Some(y) = vertex.y else {
      return Err(Stop::Skip(
        "a vertex has no Y coordinate (group code 20)".to_string(),
      ));
    };
    vertices.push(Vertex::new(Point2::new(vertex.x, y), vertex.bulge));
  }
  contour(vertices, closed)
}

fn closed_flag(fields: &Fields) -> Result<bool, Stop> {
  Ok(fields.integer(70)?.unwrap_or(0) & 1 != 0)
}

fn ellipse_entity(
  fields: &Fields,
  options: &ImportOptions,
  report: &mut ImportReport,
) -> Result<Contour, Stop> {
  let center = point(fields, 10, 20, "missing the centre (group codes 10 and 20)")?;
  let major_point = point(
    fields,
    11,
    21,
    "missing the major-axis endpoint (group codes 11 and 21)",
  )?;
  let ratio = fields.require(40, "missing the minor/major axis ratio (group code 40)")?;
  let start = fields.number(41)?.unwrap_or(0.0);
  let end = fields.number(42)?.unwrap_or(TAU);

  let major = Vec2::new(major_point.x, major_point.y);
  let major_length = major.length();
  if major_length <= 0.0 {
    return Err(Stop::Skip("the major axis has no length".to_string()));
  }
  let minor_length = major_length * ratio;
  if minor_length <= 0.0 {
    return Err(Stop::Skip(
      "the axis ratio makes the minor axis zero or negative".to_string(),
    ));
  }

  let tolerance = flatten_tolerance(options);
  let raw_span = end - start;

  // A full ellipse stays arc-bearing: the rotation by the major-axis angle is exact on arcs.
  if raw_span.abs() >= TAU - tolerance {
    let approximation = primitive::ellipse(
      center,
      major_length,
      minor_length,
      cam_geom::TolerancePolicy::standard().with_chord(tolerance),
    )
    .map_err(|error| Stop::Skip(format!("cannot build the ellipse: {error}")))?;
    if approximation.max_radial_error > tolerance {
      report.warn(format!(
        "a full ellipse is approximated by {} arcs with a radial error of {:.4} mm, above the \
         requested tolerance of {:.4} mm",
        approximation.arc_count(),
        approximation.max_radial_error,
        tolerance
      ));
    }
    let rotated = transform::rotate(&approximation.contour, major.angle(), center)
      .map_err(|error| Stop::Skip(format!("cannot rotate the ellipse: {error}")))?;
    return Ok(oriented(rotated));
  }

  let span = if raw_span < 0.0 {
    raw_span + TAU
  } else {
    raw_span
  };
  if span <= 1e-12 {
    return Err(Stop::Skip(
      "the start and end parameters coincide, so the ellipse has no length".to_string(),
    ));
  }

  // A partial elliptical arc is not a circular arc, so it is flattened at the caller's tolerance
  // and counted (D-M3-2).
  let rotation = major.angle();
  let (sin_a, cos_a) = rotation.sin_cos();
  let curve = |t: f64| {
    let (sin_t, cos_t) = t.sin_cos();
    let x = major_length * cos_t;
    let y = minor_length * sin_t;
    Point2::new(
      center.x + x * cos_a - y * sin_a,
      center.y + x * sin_a + y * cos_a,
    )
  };
  let mut points = Vec::new();
  flatten(&curve, start, start + span, tolerance, &mut points);
  report.flatten(1);
  contour(points.iter().copied().map(Vertex::corner).collect(), false)
}

// ---------------------------------------------------------------------------------------------
// Flattening and scaling.
// ---------------------------------------------------------------------------------------------

/// The tolerance to flatten at: the caller's, unless it is not a usable distance.
fn flatten_tolerance(options: &ImportOptions) -> f64 {
  if options.tolerance.is_finite() && options.tolerance > 0.0 {
    options.tolerance
  } else {
    cam_geom::DEFAULT_TOLERANCE
  }
}

/// Sample a parametric curve so no chord deviates from it by more than `tolerance`.
fn flatten<F>(curve: &F, from: f64, to: f64, tolerance: f64, points: &mut Vec<Point2>)
where
  F: Fn(f64) -> Point2,
{
  let start = curve(from);
  let end = curve(to);
  points.push(start);
  subdivide(curve, (from, start), (to, end), tolerance, 0, points);
}

/// Adaptive bisection: halve until the chord is flat enough, or the sample budget runs out.
fn subdivide<F>(
  curve: &F,
  start: (f64, Point2),
  end: (f64, Point2),
  tolerance: f64,
  depth: usize,
  points: &mut Vec<Point2>,
) where
  F: Fn(f64) -> Point2,
{
  let middle_time = 0.5 * (start.0 + end.0);
  let middle = curve(middle_time);
  let deviation = LineSeg::new(start.1, end.1).distance_to_point(middle);
  if depth >= MAX_FLATTEN_DEPTH || deviation <= tolerance {
    points.push(end.1);
    return;
  }
  subdivide(
    curve,
    start,
    (middle_time, middle),
    tolerance,
    depth + 1,
    points,
  );
  subdivide(
    curve,
    (middle_time, middle),
    end,
    tolerance,
    depth + 1,
    points,
  );
}

/// Apply the unit scale. A uniform scale is exact, so arcs stay arcs.
fn scale_contour(
  contour: Contour,
  scale: f64,
  options: &ImportOptions,
) -> Result<Contour, ImportError> {
  if (scale - 1.0).abs() <= f64::EPSILON {
    return Ok(contour);
  }
  transform::scale(
    &contour,
    scale,
    scale,
    Point2::ORIGIN,
    cam_geom::TolerancePolicy::standard().with_chord(flatten_tolerance(options)),
  )
  .map(|outcome| outcome.contour)
  .map_err(|error| ImportError::parse_plain(FORMAT, error.to_string()))
}
