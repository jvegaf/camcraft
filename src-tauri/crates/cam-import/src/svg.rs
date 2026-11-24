//! SVG import: the XML subset, the path grammar and the unit mapping (M3 A6, A7, A8).
//!
//! # What this reads (A6)
//!
//! `<svg>` (its `width`, `height`, `viewBox` and `transform`), `<g>` with an inherited `transform`,
//! and the shape elements `<path>`, `<line>`, `<polyline>`, `<polygon>`, `<rect>` (including
//! `rx`/`ry`), `<circle>` and `<ellipse>`, each with its own `transform`. `matrix`, `translate`,
//! `scale`, `rotate`, `skewX` and `skewY` are all honoured, in one attribute and composed left to
//! right with the inherited transform.
//!
//! The XML reader is hand-written (D-M3-1): elements, attributes in single or double quotes,
//! self-closing tags, `<!-- comments -->`, `<?xml ...?>`, `<!DOCTYPE ...>` with an internal subset,
//! `<![CDATA[...]]>`, the five named entities and numeric character references. A mismatched or
//! unclosed tag is a located parse error, never a silent truncation. Elements outside the subset
//! (`<text>`, `<image>`, `<use>`, `<defs>`, `<style>`, `<clipPath>`, …) produce no geometry, so they
//! are listed in [`ImportReport::skipped`] with their name and location.
//!
//! # Units (A7, D-M3-4)
//!
//! A unitless length in the root viewport is a CSS pixel: `1px = 25.4/96 mm`, never a millimetre.
//! `px`, `mm`, `cm`, `in`, `pt` and `pc` are accepted and converted there. On a shape attribute a
//! bare number is a user unit, which is what `viewBox` scales; a physical unit is a length of the
//! page and becomes user units through that same scale. `width`/`height` plus `viewBox` map the
//! user-space box onto the physical page with `preserveAspectRatio`'s default `xMidYMid meet`: the
//! smaller of the two scale factors, centred. Without `width`/`height` the `viewBox` is read as
//! millimetres and the report warns. [`ImportReport::unit_scale`] carries the millimetres per user
//! unit applied.
//!
//! # Arcs and flattening (A8, S8)
//!
//! An `A` with `rx == ry` and no rotation becomes an exact [`cam_geom::Arc`]: the centre from the
//! SVG endpoint parameterisation (F.6.5), the sweep sign from the sweep flag, the large-arc flag
//! choosing which half of the circle. `<circle>` is two half-turn bulges and `<ellipse>` is the
//! kernel's arc approximation with its measured error reported. Every curve that cannot stay an arc
//! — a cubic or quadratic Bézier, an elliptical or rotated `A`, an elliptical corner of a rounded
//! `rect` — is flattened adaptively to within `options.tolerance` and counted once with
//! [`ImportReport::flatten`]. A transform that is not a rotation, uniform scale plus reflection
//! turns circles into ellipses, so those arcs are flattened and counted for the same reason.
//!
//! Two conventions worth knowing when reading the output:
//!
//! * Y is **inverted** (D-M3-10): SVG user space is y-down and millimetre space is y-up, so the
//!   drawing is reflected about the page's horizontal centre line and `<rect y="20">` on a 100 mm
//!   page has its bounds at `y = 40..80`. The reflection goes through the root mapping, so it is an
//!   isometry: arcs stay exact with their bulge sign flipped, and a closed contour keeps its
//!   counter-clockwise winding. It is the same convention the raster tracer uses.
//! * A filled shape is imported as its boundary, which is the cut path we can define (D-M3-7);
//!   `style` and `class` do not change the geometry, and a shape with `fill="none"` and no stroke is
//!   imported and mentioned in [`ImportReport::warnings`].

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use cam_geom::{Arc, Contour, Point2, Vertex, MAX_TESSELLATION_STEPS};

use crate::{ImportError, ImportOptions, ImportReport, ImportedLayer};

/// The format name used in errors.
const FORMAT: &str = "svg";

/// Millimetres per CSS pixel at 96 dpi (D-M3-4).
const PX_MM: f64 = 25.4 / 96.0;

/// The bulge of a quarter turn: `tan(PI / 8)`.
const QUARTER_TURN_BULGE: f64 = 0.414_213_562_373_095_1;

/// How close two output points must be to be the same point, in millimetres.
const POINT_EPSILON: f64 = 1.0e-9;

/// Deepest adaptive subdivision, a guard against a pathological tolerance.
const MAX_SUBDIVISION_DEPTH: usize = 32;

/// Import an SVG.
pub fn import(
  bytes: &[u8],
  options: &ImportOptions,
  report: &mut ImportReport,
) -> Result<Vec<ImportedLayer>, ImportError> {
  let text = String::from_utf8_lossy(bytes);
  let mut parser = XmlParser::new(text.as_bytes());
  let root = parser.parse_document()?;

  if root.name != "svg" {
    return Err(ImportError::parse(
      FORMAT,
      format!("the root element is <{}>, not <svg>", root.name),
      format!("line {}", root.line),
    ));
  }

  let mapping = root_mapping(&root, report)?;
  report.unit_scale = mapping.unit_scale;

  let tolerance = if options.tolerance.is_finite() && options.tolerance > 0.0 {
    options.tolerance
  } else {
    crate::DEFAULT_TOLERANCE
  };

  let root_transform = match root.attribute("transform") {
    Some(text) => parse_transform(text, root.line)?,
    None => Matrix::IDENTITY,
  };

  let mut builder = Builder {
    tolerance,
    unit_scale: mapping.unit_scale,
    report,
    contours: Vec::new(),
  };
  // SVG user space is y-down and millimetres are y-up, so the page is reflected about its
  // horizontal centre line (D-M3-10). A reflection is an isometry: arcs stay exact, with the bulge
  // sign flipped, and the winding is normalised when each contour is built.
  let flip = Matrix::new(1.0, 0.0, 0.0, -1.0, 0.0, mapping.page_height);
  builder.visit_children(&root, flip.times(mapping.matrix).times(root_transform))?;

  if builder.contours.is_empty() {
    return Ok(Vec::new());
  }

  Ok(vec![ImportedLayer {
    name: "svg".to_string(),
    contours: builder.contours,
  }])
}

// --------------------------------------------------------------------------------------------
// XML: a small tree built from a hand-written reader, never a stream of half-read attributes.
// --------------------------------------------------------------------------------------------

/// One element of the document tree.
struct Element {
  name: String,
  attributes: Vec<(String, String)>,
  line: usize,
  children: Vec<Element>,
}

impl Element {
  /// The value of `name`, when the attribute is present.
  fn attribute(&self, name: &str) -> Option<&str> {
    self
      .attributes
      .iter()
      .find(|(key, _)| key == name)
      .map(|(_, value)| value.as_str())
  }

  /// Where this element is, in the form the report uses for skipped constructs.
  fn location(&self) -> String {
    format!("element <{}>", self.name)
  }
}

/// A byte-oriented XML reader: positions and line numbers, no allocation until a value is read.
struct XmlParser<'a> {
  bytes: &'a [u8],
  position: usize,
  line: usize,
}

impl<'a> XmlParser<'a> {
  fn new(bytes: &'a [u8]) -> Self {
    Self {
      bytes,
      position: 0,
      line: 1,
    }
  }

  fn error(&self, message: impl Into<String>) -> ImportError {
    ImportError::parse(FORMAT, message, format!("line {}", self.line))
  }

  fn peek(&self) -> Option<u8> {
    self.bytes.get(self.position).copied()
  }

  fn advance(&mut self) -> Option<u8> {
    let byte = self.bytes.get(self.position).copied()?;
    self.position += 1;
    if byte == b'\n' {
      self.line += 1;
    }
    Some(byte)
  }

  fn starts_with(&self, needle: &[u8]) -> bool {
    self.bytes[self.position..].starts_with(needle)
  }

  fn skip_whitespace(&mut self) {
    while matches!(self.peek(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
      self.advance();
    }
  }

  /// Consume everything up to and including `needle`.
  fn skip_past(&mut self, needle: &[u8]) -> Result<(), ImportError> {
    let end = self.bytes[self.position..]
      .windows(needle.len())
      .position(|window| window == needle)
      .map(|offset| self.position + offset + needle.len())
      .ok_or_else(|| {
        self.error(format!(
          "unterminated construct, expected {:?}",
          String::from_utf8_lossy(needle)
        ))
      })?;
    while self.position < end {
      self.advance();
    }
    Ok(())
  }

  /// Skip whitespace, comments, processing instructions and declarations.
  fn skip_misc(&mut self) -> Result<(), ImportError> {
    loop {
      self.skip_whitespace();
      if self.starts_with(b"<!--") || self.starts_with(b"<?") {
        let terminator: &[u8] = if self.starts_with(b"<!--") {
          b"-->"
        } else {
          b"?>"
        };
        self.skip_past(terminator)?;
      } else if self.starts_with(b"<!") {
        self.skip_declaration()?;
      } else {
        return Ok(());
      }
    }
  }

  /// Skip a `<!...>` declaration, including a DOCTYPE with an internal subset.
  fn skip_declaration(&mut self) -> Result<(), ImportError> {
    self.advance();
    self.advance();
    let mut brackets = 0usize;
    let mut quote: Option<u8> = None;
    loop {
      let byte = self
        .advance()
        .ok_or_else(|| self.error("unterminated declaration"))?;
      match quote {
        Some(open) => {
          if byte == open {
            quote = None;
          }
        }
        None => match byte {
          b'"' | b'\'' => quote = Some(byte),
          b'[' => brackets += 1,
          b']' => brackets = brackets.saturating_sub(1),
          b'>' if brackets == 0 => return Ok(()),
          _ => {}
        },
      }
    }
  }

  fn parse_name(&mut self) -> String {
    let start = self.position;
    while let Some(byte) = self.peek() {
      let is_name_byte =
        byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'_' | b'-' | b'.') || byte >= 0x80;
      if !is_name_byte {
        break;
      }
      self.advance();
    }
    String::from_utf8_lossy(&self.bytes[start..self.position]).into_owned()
  }

  /// Parse the whole document and return its root element.
  fn parse_document(&mut self) -> Result<Element, ImportError> {
    self.skip_misc()?;
    if self.peek() != Some(b'<') {
      return Err(self.error("the document has no root element"));
    }
    let root = self.parse_element()?;
    self.skip_misc()?;
    if self.peek() == Some(b'<') {
      return Err(self.error("the document has more than one root element"));
    }
    Ok(root)
  }

  fn parse_element(&mut self) -> Result<Element, ImportError> {
    let line = self.line;
    if self.advance() != Some(b'<') {
      return Err(self.error("expected an element"));
    }
    let name = self.parse_name();
    if name.is_empty() {
      return Err(self.error("an element needs a name"));
    }

    let mut attributes: Vec<(String, String)> = Vec::new();
    loop {
      self.skip_whitespace();
      match self.peek() {
        Some(b'/') => {
          self.advance();
          if self.advance() != Some(b'>') {
            return Err(self.error(format!("expected '>' after '/' in <{name}>")));
          }
          return Ok(Element {
            name,
            attributes,
            line,
            children: Vec::new(),
          });
        }
        Some(b'>') => {
          self.advance();
          break;
        }
        Some(_) => {
          let key = self.parse_name();
          if key.is_empty() {
            return Err(self.error(format!("malformed attribute in <{name}>")));
          }
          self.skip_whitespace();
          if self.advance() != Some(b'=') {
            return Err(self.error(format!("attribute {key} of <{name}> has no value")));
          }
          self.skip_whitespace();
          let quote = self
            .advance()
            .ok_or_else(|| self.error(format!("attribute {key} of <{name}> has no value")))?;
          if quote != b'"' && quote != b'\'' {
            return Err(self.error(format!("attribute {key} of <{name}> is not quoted")));
          }
          let value = self.parse_attribute_value(quote)?;
          attributes.push((key, value));
        }
        None => return Err(self.error(format!("unclosed tag <{name}>"))),
      }
    }

    let mut children: Vec<Element> = Vec::new();
    loop {
      self.skip_whitespace();
      if self.peek().is_none() {
        return Err(ImportError::parse(
          FORMAT,
          format!("unclosed tag <{name}>"),
          format!("line {line}"),
        ));
      }
      if self.starts_with(b"</") {
        self.advance();
        self.advance();
        let closing = self.parse_name();
        self.skip_whitespace();
        if self.advance() != Some(b'>') {
          return Err(self.error(format!("expected '>' after </{closing}>")));
        }
        if closing != name {
          return Err(self.error(format!(
            "mismatched closing tag </{closing}> for <{name}>, which opened on line {line}"
          )));
        }
        return Ok(Element {
          name,
          attributes,
          line,
          children,
        });
      }
      if self.starts_with(b"<!--") {
        self.skip_past(b"-->")?;
        continue;
      }
      if self.starts_with(b"<![CDATA[") {
        self.skip_past(b"]]>")?;
        continue;
      }
      if self.starts_with(b"<?") {
        self.skip_past(b"?>")?;
        continue;
      }
      if self.starts_with(b"<!") {
        self.skip_declaration()?;
        continue;
      }
      if self.starts_with(b"<") {
        children.push(self.parse_element()?);
        continue;
      }
      // A text node: it carries no geometry, so it is read past.
      while let Some(byte) = self.peek() {
        if byte == b'<' {
          break;
        }
        self.advance();
      }
    }
  }

  fn parse_attribute_value(&mut self, quote: u8) -> Result<String, ImportError> {
    let start = self.position;
    loop {
      match self.advance() {
        Some(byte) if byte == quote => break,
        Some(_) => {}
        None => return Err(self.error("unterminated attribute value")),
      }
    }
    Ok(decode_entities(&self.bytes[start..self.position - 1]))
  }
}

/// Decode the five named entities and numeric character references; unknown ones stay literal.
fn decode_entities(raw: &[u8]) -> String {
  let mut out: Vec<u8> = Vec::with_capacity(raw.len());
  let mut index = 0usize;
  while index < raw.len() {
    if raw[index] == b'&' {
      if let Some(offset) = raw[index..].iter().position(|&byte| byte == b';') {
        let end = index + offset;
        let entity = &raw[index + 1..end];
        if let Some(byte) = named_entity(entity) {
          out.push(byte);
          index = end + 1;
          continue;
        }
        if let Some(character) = numeric_entity(entity) {
          let mut buffer = [0u8; 4];
          out.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
          index = end + 1;
          continue;
        }
      }
    }
    out.push(raw[index]);
    index += 1;
  }
  String::from_utf8_lossy(&out).into_owned()
}

fn named_entity(entity: &[u8]) -> Option<u8> {
  match entity {
    b"amp" => Some(b'&'),
    b"lt" => Some(b'<'),
    b"gt" => Some(b'>'),
    b"quot" => Some(b'"'),
    b"apos" => Some(b'\''),
    _ => None,
  }
}

fn numeric_entity(entity: &[u8]) -> Option<char> {
  let body = entity.strip_prefix(b"#")?;
  let (digits, radix) = match body {
    [b'x' | b'X', rest @ ..] => (rest, 16),
    _ => (body, 10),
  };
  let text = std::str::from_utf8(digits).ok()?;
  char::from_u32(u32::from_str_radix(text, radix).ok()?)
}

// --------------------------------------------------------------------------------------------
// Numbers and lengths
// --------------------------------------------------------------------------------------------

fn skip_separators(bytes: &[u8], position: &mut usize) {
  while let Some(&byte) = bytes.get(*position) {
    if byte.is_ascii_whitespace() || byte == b',' {
      *position += 1;
    } else {
      break;
    }
  }
}

fn is_number_start(byte: u8) -> bool {
  byte.is_ascii_digit() || matches!(byte, b'+' | b'-' | b'.')
}

/// Read one SVG number at `position`, or `None` when there is no number there.
fn scan_number(bytes: &[u8], position: &mut usize) -> Option<f64> {
  skip_separators(bytes, position);
  let start = *position;
  if matches!(bytes.get(*position), Some(b'+' | b'-')) {
    *position += 1;
  }
  let mut digits = false;
  while matches!(bytes.get(*position), Some(byte) if byte.is_ascii_digit()) {
    *position += 1;
    digits = true;
  }
  if matches!(bytes.get(*position), Some(b'.')) {
    *position += 1;
    while matches!(bytes.get(*position), Some(byte) if byte.is_ascii_digit()) {
      *position += 1;
      digits = true;
    }
  }
  if !digits {
    *position = start;
    return None;
  }
  if matches!(bytes.get(*position), Some(b'e' | b'E')) {
    let before_exponent = *position;
    *position += 1;
    if matches!(bytes.get(*position), Some(b'+' | b'-')) {
      *position += 1;
    }
    let mut exponent_digits = false;
    while matches!(bytes.get(*position), Some(byte) if byte.is_ascii_digit()) {
      *position += 1;
      exponent_digits = true;
    }
    if !exponent_digits {
      *position = before_exponent;
    }
  }
  std::str::from_utf8(&bytes[start..*position])
    .ok()?
    .parse()
    .ok()
}

/// A length unit SVG understands here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Unit {
  /// No suffix. In the root viewport this is px (D-M3-4); on a shape attribute it is a user
  /// unit, which is what the viewport mapping scales.
  User,
  Px,
  Mm,
  Cm,
  In,
  Pt,
  Pc,
  Percent,
}

impl Unit {
  /// Millimetres in one of this unit, read as a length of the root viewport (so `User` is px).
  fn mm(self) -> f64 {
    match self {
      Self::User | Self::Px => PX_MM,
      Self::Mm => 1.0,
      Self::Cm => 10.0,
      Self::In => 25.4,
      Self::Pt => 25.4 / 72.0,
      Self::Pc => 25.4 / 6.0,
      Self::Percent => 1.0,
    }
  }
}

/// Split `10mm`, `10`, `10px` into its number and its unit.
fn parse_length(text: &str) -> Option<(f64, Unit)> {
  let bytes = text.trim().as_bytes();
  let mut position = 0usize;
  let value = scan_number(bytes, &mut position)?;
  let suffix = std::str::from_utf8(&bytes[position..]).ok()?.trim();
  let unit = match suffix.to_ascii_lowercase().as_str() {
    "" => Unit::User,
    "px" => Unit::Px,
    "mm" => Unit::Mm,
    "cm" => Unit::Cm,
    "in" => Unit::In,
    "pt" => Unit::Pt,
    "pc" => Unit::Pc,
    "%" => Unit::Percent,
    _ => return None,
  };
  Some((value, unit))
}

// --------------------------------------------------------------------------------------------
// Transforms: a 2x3 affine matrix
// --------------------------------------------------------------------------------------------

/// An affine transform, in the SVG order `x' = a*x + c*y + e`, `y' = b*x + d*y + f`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Matrix {
  a: f64,
  b: f64,
  c: f64,
  d: f64,
  e: f64,
  f: f64,
}

impl Matrix {
  const IDENTITY: Self = Self {
    a: 1.0,
    b: 0.0,
    c: 0.0,
    d: 1.0,
    e: 0.0,
    f: 0.0,
  };

  const fn new(a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) -> Self {
    Self { a, b, c, d, e, f }
  }

  /// Apply the transform to a point.
  fn apply(self, point: Point2) -> Point2 {
    Point2::new(
      self.a * point.x + self.c * point.y + self.e,
      self.b * point.x + self.d * point.y + self.f,
    )
  }

  /// `self` applied first, then `next`: the composition an inherited transform needs.
  fn times(self, rhs: Self) -> Self {
    Self {
      a: self.a * rhs.a + self.c * rhs.b,
      b: self.b * rhs.a + self.d * rhs.b,
      c: self.a * rhs.c + self.c * rhs.d,
      d: self.b * rhs.c + self.d * rhs.d,
      e: self.a * rhs.e + self.c * rhs.f + self.e,
      f: self.b * rhs.e + self.d * rhs.f + self.f,
    }
  }

  /// The uniform scale factor, when this transform maps circles onto circles.
  ///
  /// A rotation, a uniform scale and a reflection all do; a non-uniform scale and a skew do not, and
  /// their arcs have to be flattened.
  fn similarity_scale(self) -> Option<f64> {
    let x_length = (self.a * self.a + self.b * self.b).sqrt();
    let y_length = (self.c * self.c + self.d * self.d).sqrt();
    if !x_length.is_finite() || x_length <= 0.0 || !y_length.is_finite() {
      return None;
    }
    let scale = x_length.max(y_length);
    if (x_length - y_length).abs() > 1.0e-9 * scale {
      return None;
    }
    if (self.a * self.c + self.b * self.d).abs() > 1.0e-9 * scale * scale {
      return None;
    }
    Some(x_length)
  }

  /// True when the linear part flips orientation, which negates every bulge.
  fn is_reflection(self) -> bool {
    self.a * self.d - self.b * self.c < 0.0
  }

  /// An upper bound on how much this transform can stretch a length (the Frobenius norm).
  ///
  /// Flattening in user space at `tolerance / max_stretch` therefore lands within `tolerance` in
  /// millimetres, whatever the transform does.
  fn max_stretch(self) -> f64 {
    let squared = self.a * self.a + self.b * self.b + self.c * self.c + self.d * self.d;
    squared.sqrt().max(f64::MIN_POSITIVE)
  }
}

/// Parse a `transform` attribute: one or more functions, applied left to right.
fn parse_transform(text: &str, line: usize) -> Result<Matrix, ImportError> {
  let bytes = text.as_bytes();
  let mut position = 0usize;
  let mut matrix = Matrix::IDENTITY;
  loop {
    skip_separators(bytes, &mut position);
    if position >= bytes.len() {
      return Ok(matrix);
    }
    let start = position;
    while matches!(bytes.get(position), Some(byte) if byte.is_ascii_alphabetic()) {
      position += 1;
    }
    let name = std::str::from_utf8(&bytes[start..position]).unwrap_or_default();
    if name.is_empty() {
      return Err(ImportError::parse(
        FORMAT,
        format!("the transform {text:?} has an unreadable function name"),
        format!("line {line}"),
      ));
    }
    skip_separators(bytes, &mut position);
    if bytes.get(position) != Some(&b'(') {
      return Err(ImportError::parse(
        FORMAT,
        format!("the transform function {name} has no argument list"),
        format!("line {line}"),
      ));
    }
    position += 1;

    let mut arguments: Vec<f64> = Vec::new();
    loop {
      skip_separators(bytes, &mut position);
      match bytes.get(position) {
        Some(b')') => {
          position += 1;
          break;
        }
        Some(_) => match scan_number(bytes, &mut position) {
          Some(value) => arguments.push(value),
          None => {
            return Err(ImportError::parse(
              FORMAT,
              format!("the transform function {name} has a non-numeric argument"),
              format!("line {line}"),
            ))
          }
        },
        None => {
          return Err(ImportError::parse(
            FORMAT,
            format!("the transform function {name} is not closed"),
            format!("line {line}"),
          ))
        }
      }
    }

    let function = match transform_function(name, &arguments) {
      Some(function) => function,
      None => {
        return Err(ImportError::parse(
          FORMAT,
          format!(
            "the transform function {name}({}) is not one this import understands",
            arguments
              .iter()
              .map(|value| value.to_string())
              .collect::<Vec<_>>()
              .join(" ")
          ),
          format!("line {line}"),
        ))
      }
    };
    // Left to right: each function is applied to the result of the ones before it.
    matrix = matrix.times(function);
  }
}

/// One transform function with its arguments checked.
fn transform_function(name: &str, arguments: &[f64]) -> Option<Matrix> {
  match (name, arguments) {
    ("matrix", [a, b, c, d, e, f]) => Some(Matrix::new(*a, *b, *c, *d, *e, *f)),
    ("translate", [tx]) => Some(Matrix::new(1.0, 0.0, 0.0, 1.0, *tx, 0.0)),
    ("translate", [tx, ty]) => Some(Matrix::new(1.0, 0.0, 0.0, 1.0, *tx, *ty)),
    ("scale", [s]) => Some(Matrix::new(*s, 0.0, 0.0, *s, 0.0, 0.0)),
    ("scale", [sx, sy]) => Some(Matrix::new(*sx, 0.0, 0.0, *sy, 0.0, 0.0)),
    ("rotate", [degrees]) => {
      let (sin, cos) = degrees.to_radians().sin_cos();
      Some(Matrix::new(cos, sin, -sin, cos, 0.0, 0.0))
    }
    ("rotate", [degrees, cx, cy]) => {
      let (sin, cos) = degrees.to_radians().sin_cos();
      let rotation = Matrix::new(cos, sin, -sin, cos, 0.0, 0.0);
      let to_origin = Matrix::new(1.0, 0.0, 0.0, 1.0, -*cx, -*cy);
      let back = Matrix::new(1.0, 0.0, 0.0, 1.0, *cx, *cy);
      Some(to_origin.times(rotation).times(back))
    }
    ("skewX", [degrees]) => Some(Matrix::new(
      1.0,
      0.0,
      degrees.to_radians().tan(),
      1.0,
      0.0,
      0.0,
    )),
    ("skewY", [degrees]) => Some(Matrix::new(
      1.0,
      degrees.to_radians().tan(),
      0.0,
      1.0,
      0.0,
      0.0,
    )),
    _ => None,
  }
}

// --------------------------------------------------------------------------------------------
// The root mapping: width, height, viewBox and the millimetre scale
// --------------------------------------------------------------------------------------------

/// The user-space to millimetre mapping, and the scale it applied.
struct Mapping {
  matrix: Matrix,
  /// Millimetres per user unit along x, which is what a shape attribute in a physical unit is
  /// divided by to become user units.
  unit_scale: f64,
  /// The page height in millimetres, which is where the y axis is reflected (D-M3-10).
  page_height: f64,
}

/// Resolve `width`/`height`/`viewBox` into a millimetre mapping (A7, D-M3-4).
fn root_mapping(root: &Element, report: &mut ImportReport) -> Result<Mapping, ImportError> {
  let width = root_length_mm(root, "width", report);
  let height = root_length_mm(root, "height", report);
  let view_box = match root.attribute("viewBox") {
    Some(text) => Some(parse_view_box(text, root.line)?),
    None => None,
  };

  let Some([min_x, min_y, box_width, box_height]) = view_box else {
    if width.is_none() || height.is_none() {
      report.warn("no width/height and no viewBox: user units are px at 96 dpi");
    }
    return Ok(Mapping {
      matrix: Matrix::new(PX_MM, 0.0, 0.0, PX_MM, 0.0, 0.0),
      unit_scale: PX_MM,
      page_height: height.unwrap_or(0.0),
    });
  };

  if let (Some(width_mm), Some(height_mm)) = (width, height) {
    // preserveAspectRatio default `xMidYMid meet`: the smaller factor, then centre.
    let scale = (width_mm / box_width).min(height_mm / box_height);
    let offset_x = (width_mm - box_width * scale) / 2.0 - min_x * scale;
    let offset_y = (height_mm - box_height * scale) / 2.0 - min_y * scale;
    return Ok(Mapping {
      matrix: Matrix::new(scale, 0.0, 0.0, scale, offset_x, offset_y),
      unit_scale: scale,
      page_height: height_mm,
    });
  }

  report.warn(
    "no width/height: the viewBox is taken as millimetres, so one user unit is one millimetre",
  );
  Ok(Mapping {
    matrix: Matrix::new(1.0, 0.0, 0.0, 1.0, -min_x, -min_y),
    unit_scale: 1.0,
    page_height: box_height,
  })
}

/// `width` or `height` in millimetres; a percentage is ignored with a warning.
fn root_length_mm(root: &Element, name: &str, report: &mut ImportReport) -> Option<f64> {
  let text = root.attribute(name)?;
  match parse_length(text) {
    Some((_, Unit::Percent)) => {
      report.warn(format!(
        "the svg {name} attribute is a percentage, which is not resolved; it is ignored"
      ));
      None
    }
    Some((value, unit)) => Some(value * unit.mm()),
    None => {
      report.warn(format!(
        "the svg {name} attribute {text:?} is not a length; it is ignored"
      ));
      None
    }
  }
}

/// Parse `min-x min-y width height`.
fn parse_view_box(text: &str, line: usize) -> Result<[f64; 4], ImportError> {
  let bytes = text.as_bytes();
  let mut position = 0usize;
  let mut values = Vec::with_capacity(4);
  while values.len() < 4 {
    match scan_number(bytes, &mut position) {
      Some(value) => values.push(value),
      None => {
        return Err(ImportError::parse(
          FORMAT,
          format!("the viewBox {text:?} needs four numbers"),
          format!("line {line}"),
        ))
      }
    }
  }
  let [min_x, min_y, width, height] = [values[0], values[1], values[2], values[3]];
  if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
    return Err(ImportError::parse(
      FORMAT,
      format!("the viewBox {text:?} has no area"),
      format!("line {line}"),
    ));
  }
  Ok([min_x, min_y, width, height])
}

// --------------------------------------------------------------------------------------------
// Geometry building
// --------------------------------------------------------------------------------------------

/// One subpath in user space: corners, plus bulges for the arcs that are still exact.
struct SubPath {
  vertices: Vec<Vertex>,
  closed: bool,
}

/// Why a shape element produced no geometry.
enum ShapeFailure {
  /// The element is skipped with this reason, in the report.
  Skip(String),
  /// The document is malformed and the import stops.
  Fatal(ImportError),
}

type ShapeResult = Result<Vec<SubPath>, ShapeFailure>;

/// Walks the tree, accumulating millimetre contours.
struct Builder<'a> {
  tolerance: f64,
  unit_scale: f64,
  report: &'a mut ImportReport,
  contours: Vec<Contour>,
}

impl Builder<'_> {
  fn visit_children(&mut self, element: &Element, matrix: Matrix) -> Result<(), ImportError> {
    for child in &element.children {
      self.visit(child, matrix)?;
    }
    Ok(())
  }

  fn visit(&mut self, element: &Element, parent: Matrix) -> Result<(), ImportError> {
    let local = match element.attribute("transform") {
      Some(text) => parse_transform(text, element.line)?,
      None => Matrix::IDENTITY,
    };
    let matrix = parent.times(local);

    match element.name.as_str() {
      "g" => self.visit_children(element, matrix)?,
      "svg" => {
        // A nested viewport re-maps user space; this import reads it as a plain group and says so.
        self.report.warn(format!(
          "{}: a nested <svg> viewport is treated as a group, so its width/height/viewBox are ignored",
          element.location()
        ));
        self.visit_children(element, matrix)?;
      }
      "path" | "line" | "polyline" | "polygon" | "rect" | "circle" | "ellipse" => {
        self.shape(element, matrix)?;
      }
      other => {
        let name = format!("<{other}>");
        self
          .report
          .skip(name, skip_reason(other), Some(element.location()));
      }
    }
    Ok(())
  }

  /// A shape element: build its subpaths, transform them, and report what happened.
  fn shape(&mut self, element: &Element, matrix: Matrix) -> Result<(), ImportError> {
    self.notice_paint(element);
    let name = format!("<{}>", element.name);
    let location = element.location();
    let input_tolerance = (self.tolerance / matrix.max_stretch()).max(1.0e-12);

    let outcome = match element.name.as_str() {
      "path" => self.path_subpaths(element, input_tolerance),
      "line" => self.line_subpaths(element),
      "polyline" | "polygon" => self.polyline_subpaths(element),
      "rect" => self.rect_subpaths(element, input_tolerance),
      "circle" => self.circle_subpaths(element),
      "ellipse" => self.ellipse_subpaths(element, input_tolerance),
      // `visit` only routes these seven names here; anything else is a bug, not a drawing.
      _ => ShapeResult::Ok(Vec::new()),
    };

    let (subpaths, reason) = match outcome {
      Ok(subpaths) => (subpaths, None),
      Err(ShapeFailure::Skip(reason)) => (Vec::new(), Some(reason)),
      Err(ShapeFailure::Fatal(error)) => return Err(error),
    };

    let mut produced = false;
    for subpath in subpaths {
      if self.convert(subpath, matrix, &name, &location) {
        produced = true;
      }
    }

    if produced {
      self.report.count(&name, 1);
    } else {
      self.report.skip(
        name,
        reason.unwrap_or_else(|| "the element produced no geometry".to_string()),
        Some(location),
      );
    }
    Ok(())
  }

  /// Turn one user-space subpath into a millimetre contour.
  ///
  /// Arcs survive when the transform is a rotation, uniform scale plus reflection; otherwise the
  /// shape is no longer made of circular arcs and they are flattened at the caller's tolerance.
  fn convert(&mut self, subpath: SubPath, matrix: Matrix, name: &str, location: &str) -> bool {
    if subpath.vertices.len() < 2 {
      return false;
    }
    let exact = matrix.similarity_scale().is_some();
    let reflect = matrix.is_reflection();

    let mut vertices: Vec<Vertex> = Vec::with_capacity(subpath.vertices.len());
    let count = subpath.vertices.len();
    for (index, vertex) in subpath.vertices.iter().enumerate() {
      if exact {
        vertices.push(Vertex::new(
          matrix.apply(vertex.point),
          if reflect { -vertex.bulge } else { vertex.bulge },
        ));
        continue;
      }
      vertices.push(Vertex::corner(matrix.apply(vertex.point)));
      if vertex.bulge.abs() <= f64::EPSILON {
        continue;
      }
      let to = subpath.vertices[(index + 1) % count].point;
      let Ok(arc) = Arc::from_endpoints_bulge(vertex.point, to, vertex.bulge) else {
        continue;
      };
      let samples = arc.tessellate(
        cam_geom::TolerancePolicy::standard().with_chord(self.tolerance / matrix.max_stretch()),
      );
      for sample in samples.iter().skip(1) {
        vertices.push(Vertex::corner(matrix.apply(*sample)));
      }
      self.report.flatten(1);
    }

    let mut vertices = tidy_vertices(vertices);
    if subpath.closed && vertices.len() > 2 {
      if let (Some(first), Some(last)) = (vertices.first().copied(), vertices.last().copied()) {
        if first.point.is_near(
          last.point,
          cam_geom::TolerancePolicy::uniform(POINT_EPSILON),
        ) {
          vertices.pop();
        }
      }
    }
    if vertices.len() < 2 {
      self.report.skip(
        name,
        "the subpath collapses to fewer than two points after its transform",
        Some(location.to_string()),
      );
      return false;
    }

    let contour = match Contour::new(vertices, subpath.closed) {
      Ok(contour) => contour,
      Err(error) => {
        self.report.skip(
          name,
          format!("the contour is not valid: {error}"),
          Some(location.to_string()),
        );
        return false;
      }
    };
    // Closed imported geometry comes out counter-clockwise: the direction of a closed cut is the
    // kernel's convention, not the draughtsman's, and downstream code must not have to guess.
    let contour = if contour.is_closed() && !contour.is_ccw() {
      contour.reverse()
    } else {
      contour
    };
    self.contours.push(contour);
    true
  }

  /// A shape that a painter would not fill is still a boundary, but the user should know (D-M3-7).
  fn notice_paint(&mut self, element: &Element) {
    let fill_none = element
      .attribute("fill")
      .is_some_and(|value| value.trim() == "none");
    let has_stroke = element
      .attribute("stroke")
      .is_some_and(|value| value.trim() != "none");
    if fill_none && !has_stroke {
      self.report.warn(format!(
        "{} has fill=\"none\" and no stroke; its boundary is imported as cut geometry anyway (D-M3-7)",
        element.location()
      ));
    }
  }

  // -- shapes --------------------------------------------------------------------------------

  fn path_subpaths(&mut self, element: &Element, tolerance: f64) -> ShapeResult {
    let Some(data) = element.attribute("d") else {
      return Err(ShapeFailure::Skip("the path has no d attribute".into()));
    };
    let reader = PathReader::new(data.as_bytes(), tolerance, &mut *self.report, element.line);
    reader.run().map_err(ShapeFailure::Fatal)
  }

  fn line_subpaths(&self, element: &Element) -> ShapeResult {
    let from = Point2::new(
      self.coordinate(element, "x1", 0.0)?,
      self.coordinate(element, "y1", 0.0)?,
    );
    let to = Point2::new(
      self.coordinate(element, "x2", 0.0)?,
      self.coordinate(element, "y2", 0.0)?,
    );
    if from.is_near(to, cam_geom::TolerancePolicy::uniform(POINT_EPSILON)) {
      return Err(ShapeFailure::Skip(
        "the line has zero length and is not geometry".into(),
      ));
    }
    Ok(vec![SubPath {
      vertices: vec![Vertex::corner(from), Vertex::corner(to)],
      closed: false,
    }])
  }

  fn polyline_subpaths(&self, element: &Element) -> ShapeResult {
    let points = points_attribute(element)?;
    if points.len() < 2 {
      return Err(ShapeFailure::Skip(
        "the points attribute names fewer than two points".into(),
      ));
    }
    Ok(vec![SubPath {
      vertices: points.into_iter().map(Vertex::corner).collect(),
      closed: element.name == "polygon",
    }])
  }

  fn rect_subpaths(&mut self, element: &Element, tolerance: f64) -> ShapeResult {
    let x = self.coordinate(element, "x", 0.0)?;
    let y = self.coordinate(element, "y", 0.0)?;
    let width = self.coordinate(element, "width", 0.0)?;
    let height = self.coordinate(element, "height", 0.0)?;
    if width <= 0.0 || height <= 0.0 {
      return Err(ShapeFailure::Skip(
        "the rect has no width or no height, so it has no area".into(),
      ));
    }
    let rx_attribute = self.optional_coordinate(element, "rx")?;
    let ry_attribute = self.optional_coordinate(element, "ry")?;
    let rx = rx_attribute
      .or(ry_attribute)
      .unwrap_or(0.0)
      .clamp(0.0, width / 2.0);
    let ry = ry_attribute
      .or(rx_attribute)
      .unwrap_or(0.0)
      .clamp(0.0, height / 2.0);

    if rx <= 0.0 || ry <= 0.0 {
      return Ok(vec![SubPath {
        vertices: vec![
          Vertex::corner(Point2::new(x, y)),
          Vertex::corner(Point2::new(x + width, y)),
          Vertex::corner(Point2::new(x + width, y + height)),
          Vertex::corner(Point2::new(x, y + height)),
        ],
        closed: true,
      }]);
    }

    if (rx - ry).abs() <= 1.0e-12 * rx.max(ry) {
      // Circular corners: four exact quarter arcs, so nothing is flattened.
      let radius = rx;
      return Ok(vec![SubPath {
        vertices: vec![
          Vertex::corner(Point2::new(x + radius, y)),
          Vertex::new(Point2::new(x + width - radius, y), QUARTER_TURN_BULGE),
          Vertex::corner(Point2::new(x + width, y + radius)),
          Vertex::new(
            Point2::new(x + width, y + height - radius),
            QUARTER_TURN_BULGE,
          ),
          Vertex::corner(Point2::new(x + width - radius, y + height)),
          Vertex::new(Point2::new(x + radius, y + height), QUARTER_TURN_BULGE),
          Vertex::corner(Point2::new(x, y + height - radius)),
          Vertex::new(Point2::new(x, y + radius), QUARTER_TURN_BULGE),
        ],
        closed: true,
      }]);
    }

    // Elliptical corners cannot be arcs; each quarter is flattened and counted.
    let corners = [
      (Point2::new(x + width - rx, y + ry), -FRAC_PI_2),
      (Point2::new(x + width - rx, y + height - ry), 0.0),
      (Point2::new(x + rx, y + height - ry), FRAC_PI_2),
      (Point2::new(x + rx, y + ry), PI),
    ];
    let mut vertices: Vec<Vertex> = Vec::new();
    for (center, start) in corners {
      let mut samples = Vec::new();
      let quarter = Ellipse {
        center,
        rx,
        ry,
        rotation: 0.0,
      };
      elliptical_samples(quarter, start, FRAC_PI_2, tolerance, &mut samples);
      vertices.push(Vertex::corner(quarter.point(start)));
      vertices.extend(samples.into_iter().map(Vertex::corner));
      self.report.flatten(1);
    }
    Ok(vec![SubPath {
      vertices,
      closed: true,
    }])
  }

  fn circle_subpaths(&self, element: &Element) -> ShapeResult {
    let center = Point2::new(
      self.coordinate(element, "cx", 0.0)?,
      self.coordinate(element, "cy", 0.0)?,
    );
    let radius = self.coordinate(element, "r", 0.0)?;
    if radius <= 0.0 {
      return Err(ShapeFailure::Skip(
        "the circle has no radius, so it has no area".into(),
      ));
    }
    Ok(vec![circle_subpath(center, radius)])
  }

  fn ellipse_subpaths(&mut self, element: &Element, tolerance: f64) -> ShapeResult {
    let center = Point2::new(
      self.coordinate(element, "cx", 0.0)?,
      self.coordinate(element, "cy", 0.0)?,
    );
    let rx = self.coordinate(element, "rx", 0.0)?;
    let ry = self.coordinate(element, "ry", 0.0)?;
    if rx <= 0.0 || ry <= 0.0 {
      return Err(ShapeFailure::Skip(
        "the ellipse has no radius, so it has no area".into(),
      ));
    }
    if (rx - ry).abs() <= 1.0e-12 * rx.max(ry) {
      return Ok(vec![circle_subpath(center, rx)]);
    }
    let approximation = cam_geom::primitive::ellipse(
      center,
      rx,
      ry,
      cam_geom::TolerancePolicy::standard().with_chord(tolerance),
    )
    .map_err(|error| ShapeFailure::Skip(format!("the ellipse could not be built: {error}")))?;
    if approximation.max_radial_error > 0.0 {
      self.report.warn(format!(
        "<ellipse> is approximated by {} circular arcs with a radial error of {:.6} mm",
        approximation.arc_count(),
        approximation.max_radial_error
      ));
    }
    Ok(vec![SubPath {
      vertices: approximation.contour.vertices().to_vec(),
      closed: true,
    }])
  }

  // -- attribute helpers ----------------------------------------------------------------------

  /// A length attribute in user units; unitless is px, and anything else is converted through
  /// millimetres so the user-space value is right whatever the root mapping is.
  fn coordinate(&self, element: &Element, name: &str, default: f64) -> Result<f64, ShapeFailure> {
    match self.optional_coordinate(element, name)? {
      Some(value) => Ok(value),
      None => Ok(default),
    }
  }

  fn optional_coordinate(
    &self,
    element: &Element,
    name: &str,
  ) -> Result<Option<f64>, ShapeFailure> {
    let Some(text) = element.attribute(name) else {
      return Ok(None);
    };
    let Some((value, unit)) = parse_length(text) else {
      return Err(ShapeFailure::Skip(format!(
        "the {name} attribute {text:?} is not a length"
      )));
    };
    if unit == Unit::Percent {
      return Err(ShapeFailure::Skip(format!(
        "the {name} attribute is a percentage, which this import does not resolve"
      )));
    }
    // A bare number is already in user units; a unit is a length of the page, so it becomes user
    // units through the millimetres-per-user-unit scale the viewport mapping chose.
    let user_units = if unit == Unit::User {
      value
    } else {
      value * unit.mm() / self.unit_scale
    };
    Ok(Some(user_units))
  }
}

/// The subpath of a circle: two half turns, so both arcs stay exact (S8).
fn circle_subpath(center: Point2, radius: f64) -> SubPath {
  SubPath {
    vertices: vec![
      Vertex::new(Point2::new(center.x + radius, center.y), 1.0),
      Vertex::new(Point2::new(center.x - radius, center.y), 1.0),
    ],
    closed: true,
  }
}

/// Residual styling that changes nothing here, said once per element so it is not a silent drop.
fn skip_reason(element: &str) -> &'static str {
  match element {
    "text" | "tspan" | "textPath" => "text is not traced as cut geometry (D-M3-5)",
    "image" => "raster images are not vector geometry",
    "use" => "referenced content is not resolved by this import",
    "defs" => "definitions are not drawn, so their content is not part of the drawing",
    "title" | "desc" | "metadata" => "document metadata is not geometry",
    "style" => "styling does not change the geometry this import reads",
    "script" => "scripts are not executed",
    "clipPath" | "mask" | "marker" | "pattern" | "symbol" | "linearGradient" | "radialGradient" => {
      "clipping, masking and paint servers are not applied to the imported geometry"
    }
    "filter" => "filters are not applied to the imported geometry",
    "a" | "switch" | "foreignObject" => "the element is not part of the supported svg subset (A6)",
    _ => "the element is not part of the supported svg subset (A6)",
  }
}

/// Parse a `points` attribute into user-space points.
fn points_attribute(element: &Element) -> Result<Vec<Point2>, ShapeFailure> {
  let Some(text) = element.attribute("points") else {
    return Ok(Vec::new());
  };
  let bytes = text.as_bytes();
  let mut position = 0usize;
  let mut values: Vec<f64> = Vec::new();
  while let Some(value) = scan_number(bytes, &mut position) {
    values.push(value);
  }
  skip_separators(bytes, &mut position);
  if position < bytes.len() {
    return Err(ShapeFailure::Skip(format!(
      "the points attribute has unreadable text at {:?}",
      &text[position..]
    )));
  }
  Ok(
    values
      .chunks_exact(2)
      .map(|pair| Point2::new(pair[0], pair[1]))
      .collect(),
  )
}

/// The reflection of `control` about `current`: what `S` and `T` continue with.
fn reflected(current: Point2, control: Point2) -> Point2 {
  Point2::new(current.x * 2.0 - control.x, current.y * 2.0 - control.y)
}

/// Merge consecutive coincident vertices, keeping the bulge of the last one.
///
/// A sampled arc ends exactly where the next vertex starts; without this the contour would carry a
/// zero-length segment, which the kernel rightly refuses.
fn tidy_vertices(vertices: Vec<Vertex>) -> Vec<Vertex> {
  let mut out: Vec<Vertex> = Vec::with_capacity(vertices.len());
  for vertex in vertices {
    match out.last_mut() {
      Some(last)
        if last.point.is_near(
          vertex.point,
          cam_geom::TolerancePolicy::uniform(POINT_EPSILON),
        ) =>
      {
        last.bulge = vertex.bulge;
      }
      _ => out.push(vertex),
    }
  }
  out
}

/// Distance from `point` to the line through `from` and `to`.
fn distance_to_chord(point: Point2, from: Point2, to: Point2) -> f64 {
  let span = to - from;
  let length = span.length();
  if length <= f64::MIN_POSITIVE {
    return point.distance_to(from);
  }
  (span.cross(point - from) / length).abs()
}

// --------------------------------------------------------------------------------------------
// Path data: the whole grammar of A6, arcs kept exact where they can be
// --------------------------------------------------------------------------------------------

/// Reads path data and produces user-space subpaths.
struct PathReader<'a> {
  bytes: &'a [u8],
  position: usize,
  line: usize,
  tolerance: f64,
  report: &'a mut ImportReport,
  vertices: Vec<Vertex>,
  subpaths: Vec<SubPath>,
  current: Point2,
  start: Point2,
  cubic_control: Option<Point2>,
  quadratic_control: Option<Point2>,
}

impl<'a> PathReader<'a> {
  fn new(bytes: &'a [u8], tolerance: f64, report: &'a mut ImportReport, line: usize) -> Self {
    Self {
      bytes,
      position: 0,
      line,
      tolerance,
      report,
      vertices: Vec::new(),
      subpaths: Vec::new(),
      current: Point2::ORIGIN,
      start: Point2::ORIGIN,
      cubic_control: None,
      quadratic_control: None,
    }
  }

  fn error(&self, message: impl Into<String>) -> ImportError {
    ImportError::parse(FORMAT, message, format!("line {}", self.line))
  }

  fn number(&mut self) -> Result<f64, ImportError> {
    scan_number(self.bytes, &mut self.position)
      .ok_or_else(|| self.error("expected a number in the path data"))
  }

  /// An arc flag: a single `0` or `1`, which the grammar allows to run together.
  fn flag(&mut self) -> Result<bool, ImportError> {
    skip_separators(self.bytes, &mut self.position);
    match self.bytes.get(self.position) {
      Some(b'0') => {
        self.position += 1;
        Ok(false)
      }
      Some(b'1') => {
        self.position += 1;
        Ok(true)
      }
      _ => Err(self.error("expected an arc flag (0 or 1) in the path data")),
    }
  }

  fn point(&mut self) -> Result<Point2, ImportError> {
    let x = scan_number(self.bytes, &mut self.position)
      .ok_or_else(|| self.error("expected a coordinate pair in the path data"))?;
    let y = scan_number(self.bytes, &mut self.position)
      .ok_or_else(|| self.error("expected a coordinate pair in the path data"))?;
    Ok(Point2::new(x, y))
  }

  fn run(mut self) -> Result<Vec<SubPath>, ImportError> {
    let mut command: Option<u8> = None;
    loop {
      skip_separators(self.bytes, &mut self.position);
      let Some(byte) = self.bytes.get(self.position).copied() else {
        break;
      };
      if byte.is_ascii_alphabetic() {
        self.position += 1;
        command = Some(byte);
      } else if is_number_start(byte) {
        command = match command {
          Some(b'M') => Some(b'L'),
          Some(b'm') => Some(b'l'),
          Some(other) if !matches!(other, b'Z' | b'z') => Some(other),
          _ => {
            return Err(self.error(
              "path data must start with a moveto, and a coordinate after Z needs a command",
            ))
          }
        };
      } else {
        return Err(self.error(format!(
          "unexpected {:?} in the path data",
          char::from(byte)
        )));
      }
      let command = command.expect("a command was selected above");
      self.execute(command)?;
    }
    self.finish();
    Ok(self.subpaths)
  }

  /// Read one argument group for `command` and apply it.
  fn execute(&mut self, command: u8) -> Result<(), ImportError> {
    match command {
      b'M' | b'm' => {
        let point = self.read_point(command == b'm')?;
        self.move_to(point);
      }
      b'L' | b'l' => {
        let point = self.read_point(command == b'l')?;
        self.line_to(point);
      }
      b'H' | b'h' => {
        let value = self.number()?;
        let x = if command == b'h' {
          self.current.x + value
        } else {
          value
        };
        self.line_to(Point2::new(x, self.current.y));
      }
      b'V' | b'v' => {
        let value = self.number()?;
        let y = if command == b'v' {
          self.current.y + value
        } else {
          value
        };
        self.line_to(Point2::new(self.current.x, y));
      }
      b'C' | b'c' => {
        let relative = command == b'c';
        let (first, second, to) = self.read_triple(relative)?;
        self.cubic_to(first, second, to);
      }
      b'S' | b's' => {
        let relative = command == b's';
        let first = match self.cubic_control {
          Some(control) => reflected(self.current, control),
          None => self.current,
        };
        let (second, to) = self.read_pair(relative)?;
        self.cubic_to(first, second, to);
      }
      b'Q' | b'q' => {
        let relative = command == b'q';
        let (control, to) = self.read_pair(relative)?;
        self.quadratic_to(control, to);
      }
      b'T' | b't' => {
        let relative = command == b't';
        let control = match self.quadratic_control {
          Some(control) => reflected(self.current, control),
          None => self.current,
        };
        let to = self.read_point(relative)?;
        self.quadratic_to(control, to);
      }
      b'A' | b'a' => {
        let relative = command == b'a';
        let rx = self.number()?;
        let ry = self.number()?;
        let rotation = self.number()?;
        let large_arc = self.flag()?;
        let sweep = self.flag()?;
        let to = self.read_point(relative)?;
        self.arc_to(rx, ry, rotation, large_arc, sweep, to);
      }
      b'Z' | b'z' => self.close(),
      other => {
        return Err(self.error(format!(
          "path command {:?} is not part of the svg grammar",
          char::from(other)
        )))
      }
    }
    Ok(())
  }

  /// Read one coordinate pair; a relative pair is an offset from the current point, which is why
  /// the whole argument group is read before anything moves.
  fn read_point(&mut self, relative: bool) -> Result<Point2, ImportError> {
    let point = self.point()?;
    Ok(if relative {
      Point2::new(self.current.x + point.x, self.current.y + point.y)
    } else {
      point
    })
  }

  fn read_pair(&mut self, relative: bool) -> Result<(Point2, Point2), ImportError> {
    let first = self.read_point(relative)?;
    let second = self.read_point(relative)?;
    Ok((first, second))
  }

  fn read_triple(&mut self, relative: bool) -> Result<(Point2, Point2, Point2), ImportError> {
    let first = self.read_point(relative)?;
    let second = self.read_point(relative)?;
    let third = self.read_point(relative)?;
    Ok((first, second, third))
  }

  fn push_point(&mut self, point: Point2) {
    if self.vertices.is_empty() {
      self.vertices.push(Vertex::corner(self.current));
    }
    self.vertices.push(Vertex::corner(point));
  }

  fn move_to(&mut self, point: Point2) {
    self.flush(false);
    self.vertices.clear();
    self.vertices.push(Vertex::corner(point));
    self.start = point;
    self.current = point;
    self.cubic_control = None;
    self.quadratic_control = None;
  }

  fn line_to(&mut self, point: Point2) {
    self.push_point(point);
    self.current = point;
    self.cubic_control = None;
    self.quadratic_control = None;
  }

  fn cubic_to(&mut self, first: Point2, second: Point2, to: Point2) {
    let from = self.current;
    let mut samples = Vec::new();
    cubic_samples(from, first, second, to, self.tolerance, 0, &mut samples);
    for sample in samples {
      self.push_point(sample);
    }
    self.current = to;
    self.cubic_control = Some(second);
    self.quadratic_control = None;
    self.report.flatten(1);
  }

  fn quadratic_to(&mut self, control: Point2, to: Point2) {
    let from = self.current;
    let mut samples = Vec::new();
    quadratic_samples(from, control, to, self.tolerance, 0, &mut samples);
    for sample in samples {
      self.push_point(sample);
    }
    self.current = to;
    self.quadratic_control = Some(control);
    self.cubic_control = None;
    self.report.flatten(1);
  }

  fn arc_to(
    &mut self,
    rx: f64,
    ry: f64,
    rotation: f64,
    large_arc: bool,
    sweep_flag: bool,
    to: Point2,
  ) {
    let from = self.current;
    if from.is_near(to, cam_geom::TolerancePolicy::uniform(POINT_EPSILON)) {
      // The grammar says an arc whose endpoints coincide draws nothing.
      return;
    }
    let rx = rx.abs();
    let ry = ry.abs();
    if rx <= 0.0 || ry <= 0.0 {
      self.line_to(to);
      return;
    }
    let rotation_degrees = rotation.rem_euclid(360.0);
    let circular = (rx - ry).abs() <= 1.0e-12 * rx.max(ry)
      && rotation_degrees.rem_euclid(180.0).abs() <= 1.0e-12;
    let (center, radius_x, radius_y, start_angle, sweep) = end_point_to_center(
      from,
      to,
      rx,
      ry,
      rotation_degrees.to_radians(),
      large_arc,
      sweep_flag,
    );

    if circular {
      if let Ok(arc) = Arc::new(center, radius_x, start_angle, sweep) {
        if self.vertices.is_empty() {
          self.vertices.push(Vertex::corner(from));
        }
        if let Some(last) = self.vertices.last_mut() {
          last.bulge = arc.bulge();
        }
        self.push_point(to);
        self.current = to;
        self.cubic_control = None;
        self.quadratic_control = None;
        return;
      }
    }

    let mut samples = Vec::new();
    elliptical_samples(
      Ellipse {
        center,
        rx: radius_x,
        ry: radius_y,
        rotation: rotation_degrees.to_radians(),
      },
      start_angle,
      sweep,
      self.tolerance,
      &mut samples,
    );
    if let Some(last) = samples.last_mut() {
      *last = to;
    }
    for sample in samples {
      self.push_point(sample);
    }
    self.current = to;
    self.cubic_control = None;
    self.quadratic_control = None;
    self.report.flatten(1);
  }

  fn close(&mut self) {
    self.flush(true);
    self.vertices.clear();
    self.vertices.push(Vertex::corner(self.start));
    self.current = self.start;
    self.cubic_control = None;
    self.quadratic_control = None;
  }

  fn flush(&mut self, closed: bool) {
    if self.vertices.len() >= 2 {
      self.subpaths.push(SubPath {
        vertices: std::mem::take(&mut self.vertices),
        closed,
      });
    } else {
      self.vertices.clear();
    }
  }

  fn finish(&mut self) {
    self.flush(false);
  }
}

/// Convert an endpoint-parameterised arc into centre parameterisation (SVG 1.1 F.6.5).
///
/// The radii are grown first when they cannot reach both endpoints (F.6.6), so a request that no
/// circle satisfies still produces the arc the standard defines.
fn end_point_to_center(
  from: Point2,
  to: Point2,
  rx: f64,
  ry: f64,
  rotation: f64,
  large_arc: bool,
  sweep_flag: bool,
) -> (Point2, f64, f64, f64, f64) {
  let (sin_phi, cos_phi) = rotation.sin_cos();
  let half_x = (from.x - to.x) / 2.0;
  let half_y = (from.y - to.y) / 2.0;
  let x1 = cos_phi * half_x + sin_phi * half_y;
  let y1 = -sin_phi * half_x + cos_phi * half_y;

  let (mut rx, mut ry) = (rx, ry);
  let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
  if lambda > 1.0 {
    let growth = lambda.sqrt();
    rx *= growth;
    ry *= growth;
  }

  let numerator = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
  let denominator = rx * rx * y1 * y1 + ry * ry * x1 * x1;
  let ratio = if denominator > 0.0 {
    (numerator.max(0.0) / denominator).sqrt()
  } else {
    0.0
  };
  let sign = if large_arc == sweep_flag { -1.0 } else { 1.0 };
  let cx1 = sign * ratio * rx * y1 / ry;
  let cy1 = -sign * ratio * ry * x1 / rx;

  let center = Point2::new(
    cos_phi * cx1 - sin_phi * cy1 + (from.x + to.x) / 2.0,
    sin_phi * cx1 + cos_phi * cy1 + (from.y + to.y) / 2.0,
  );

  let start = Point2::new((x1 - cx1) / rx, (y1 - cy1) / ry);
  let end = Point2::new((-x1 - cx1) / rx, (-y1 - cy1) / ry);
  let start_angle = start.y.atan2(start.x);
  let mut sweep = end.y.atan2(end.x) - start_angle;
  if sweep > PI {
    sweep -= TAU;
  } else if sweep < -PI {
    sweep += TAU;
  }
  if !sweep_flag && sweep > 0.0 {
    sweep -= TAU;
  } else if sweep_flag && sweep < 0.0 {
    sweep += TAU;
  }
  (center, rx, ry, start_angle, sweep)
}

// --------------------------------------------------------------------------------------------
// Adaptive flattening
// --------------------------------------------------------------------------------------------

/// An ellipse in user space: centre, radii and rotation, which is what an elliptical arc needs.
#[derive(Clone, Copy, Debug)]
struct Ellipse {
  center: Point2,
  rx: f64,
  ry: f64,
  rotation: f64,
}

impl Ellipse {
  /// The point on the ellipse at parameter `angle`.
  fn point(self, angle: f64) -> Point2 {
    let (sin_phi, cos_phi) = self.rotation.sin_cos();
    let x = self.rx * angle.cos();
    let y = self.ry * angle.sin();
    Point2::new(
      self.center.x + cos_phi * x - sin_phi * y,
      self.center.y + sin_phi * x + cos_phi * y,
    )
  }
}

/// Sample an elliptical arc, excluding its start and including its end.
fn elliptical_samples(
  ellipse: Ellipse,
  start_angle: f64,
  sweep: f64,
  tolerance: f64,
  out: &mut Vec<Point2>,
) {
  let point_at = |angle: f64| ellipse.point(angle);
  let mut budget = MAX_TESSELLATION_STEPS;
  subdivide_elliptical(
    &point_at,
    start_angle,
    start_angle + sweep,
    tolerance,
    0,
    &mut budget,
    out,
  );
}

fn subdivide_elliptical(
  point_at: &dyn Fn(f64) -> Point2,
  from: f64,
  to: f64,
  tolerance: f64,
  depth: usize,
  budget: &mut usize,
  out: &mut Vec<Point2>,
) {
  let start = point_at(from);
  let end = point_at(to);
  let span = to - from;
  let deviation = [
    point_at(from + span * 0.25),
    point_at(from + span * 0.5),
    point_at(from + span * 0.75),
  ]
  .into_iter()
  .map(|sample| distance_to_chord(sample, start, end))
  .fold(0.0_f64, f64::max);

  if depth >= MAX_SUBDIVISION_DEPTH || *budget == 0 || deviation <= tolerance {
    *budget = budget.saturating_sub(1);
    out.push(end);
    return;
  }
  let middle = from + span / 2.0;
  subdivide_elliptical(point_at, from, middle, tolerance, depth + 1, budget, out);
  subdivide_elliptical(point_at, middle, to, tolerance, depth + 1, budget, out);
}

/// Sample a cubic Bézier, excluding its start and including its end.
fn cubic_samples(
  from: Point2,
  first: Point2,
  second: Point2,
  to: Point2,
  tolerance: f64,
  depth: usize,
  out: &mut Vec<Point2>,
) {
  if depth >= MAX_SUBDIVISION_DEPTH || out.len() >= MAX_TESSELLATION_STEPS {
    out.push(to);
    return;
  }
  // The curve stays within 3/4 of the far control points' distance from its chord.
  let deviation = distance_to_chord(first, from, to).max(distance_to_chord(second, from, to));
  if 0.75 * deviation <= tolerance {
    out.push(to);
    return;
  }
  let first_half = from.lerp(first, 0.5);
  let middle = first.lerp(second, 0.5);
  let second_half = second.lerp(to, 0.5);
  let lower = first_half.lerp(middle, 0.5);
  let upper = middle.lerp(second_half, 0.5);
  let midpoint = lower.lerp(upper, 0.5);
  cubic_samples(from, first_half, lower, midpoint, tolerance, depth + 1, out);
  cubic_samples(midpoint, upper, second_half, to, tolerance, depth + 1, out);
}

/// Sample a quadratic Bézier, excluding its start and including its end.
fn quadratic_samples(
  from: Point2,
  control: Point2,
  to: Point2,
  tolerance: f64,
  depth: usize,
  out: &mut Vec<Point2>,
) {
  if depth >= MAX_SUBDIVISION_DEPTH || out.len() >= MAX_TESSELLATION_STEPS {
    out.push(to);
    return;
  }
  // A quadratic reaches exactly half of the control point's distance from its chord, at its
  // middle, so that is the error to bound.
  let deviation = distance_to_chord(control, from, to) / 2.0;
  if deviation <= tolerance {
    out.push(to);
    return;
  }
  let first_half = from.lerp(control, 0.5);
  let second_half = control.lerp(to, 0.5);
  let midpoint = first_half.lerp(second_half, 0.5);
  quadratic_samples(from, first_half, midpoint, tolerance, depth + 1, out);
  quadratic_samples(midpoint, second_half, to, tolerance, depth + 1, out);
}
