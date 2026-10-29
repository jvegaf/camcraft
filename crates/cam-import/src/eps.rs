//! EPS (Encapsulated PostScript) import: the PostScript path subset (M3 A11).
//!
//! # What is interpreted
//!
//! The path operators `newpath`, `moveto`, `lineto`, `curveto`, `rlineto`, `rcurveto`,
//! `closepath`, `arc` and `arcn` are read with `translate`, `scale`, `rotate`, `concat` and
//! `setmatrix` tracked through a `gsave`/`grestore` matrix stack. `stroke`/`s`, `fill`/`f`/`F`/`b`/
//! `B` and `eofill`/`f*`/`b*`/`B*` turn the current path into contours; `clip` and `n` throw it
//! away, exactly as A11 says (`W`/`W*` only set the clip, so the path survives them).
//! `%%BoundingBox`/`%%HiResBoundingBox` place the drawing: its lower-left corner becomes the origin.
//!
//! The path is built in *device* space, as PostScript specifies, so a transformation that changes
//! in the middle of a path does not retroactively move the segments already added.
//!
//! # Arcs stay arcs
//!
//! `arc` and `arcn` are circular, so each one becomes an exact [`cam_geom::Arc`], and a full turn
//! becomes a closed circle of two half-turn arcs (A11, S8). Only a matrix that cannot be a
//! similarity — a non-uniform scale or a skew, which turns the circle into an ellipse — makes the
//! importer flatten, and then the flattening is counted in [`crate::ImportReport::flattened_curves`]
//! (D-M3-2). `curveto`/`rcurveto` are cubic Béziers: they are always flattened at the caller's
//! tolerance, subdivided until the control points are within that tolerance of the chord, and each
//! one is counted.
//!
//! A full turn starts its own closed subpath. A single contour cannot hold both the circle and a
//! joining line that came from an earlier current point, because closing a contour connects its
//! last point to its first; splitting them keeps every arc exact and loses no segment.
//!
//! # What is not a path
//!
//! Style, dictionary and text-layout operators (`setlinewidth`, `setrgbcolor`, `setdash`, `def`,
//! `bind`, `selectfont`, …) change nothing about the geometry, so they are ignored without a report
//! entry. Operators that *would* produce geometry this milestone cannot import — `show` and the
//! other text operators, `image`/`imagemask`, `sh` — are listed in `skipped` under their own name,
//! and any other operator is listed once with the number of times it was met. Nothing is dropped
//! silently: a path that is still open at the end of the file (never stroked, filled or clipped) is
//! reported too.
//!
//! # Units
//!
//! PostScript user coordinates are 1/72 inch, so every coordinate is scaled by [`MM_PER_POINT`] on
//! the way out; [`crate::ImportReport::unit_scale`] records that factor and a warning states it.
//!
//! # Layers
//!
//! PostScript has no layers, so everything lands in one layer named `eps`; which paint operator
//! produced a contour group is visible in [`crate::ImportReport::sources`] as the `stroke` and
//! `fill` counts (D-M3-7).

use std::collections::BTreeMap;
use std::f64::consts::{PI, TAU};

use cam_geom::{Arc, Contour, LineSeg, Point2, Segment, Vec2};

use crate::{ImportError, ImportOptions, ImportReport, ImportedLayer, DEFAULT_TOLERANCE};

/// One PostScript point (1/72 inch) in millimetres: the unit an EPS coordinate is written in.
pub const MM_PER_POINT: f64 = 25.4 / 72.0;

/// The layer name every EPS contour lands in, because PostScript has no layers.
const LAYER_NAME: &str = "eps";

/// Deepest subdivision of a cubic Bézier before the flattener accepts the chord.
const MAX_CURVE_DEPTH: usize = 16;

/// Upper bound on the points one flattened cubic may produce, mirroring the kernel's cap.
const MAX_CURVE_POINTS: usize = cam_geom::MAX_TESSELLATION_STEPS;

/// Two user-space points closer than this fraction of the arc radius are the same point.
const SAME_POINT: f64 = 1.0e-9;

/// Shortest segment that is still worth emitting, in output millimetres.
const SHORTEST_SEGMENT: f64 = 1.0e-12;

/// A full turn in degrees, and the slop under which a sweep counts as one.
const FULL_TURN_DEGREES: f64 = 360.0;
const FULL_TURN_EPSILON: f64 = 1.0e-6;

/// A zero sweep in degrees, below which an arc draws nothing.
const ZERO_SWEEP_DEGREES: f64 = 1.0e-9;

/// Import an EPS.
pub fn import(
  bytes: &[u8],
  options: &ImportOptions,
  report: &mut ImportReport,
) -> Result<Vec<ImportedLayer>, ImportError> {
  let origin = read_header(bytes, report)?;
  let tolerance = if options.tolerance.is_finite() && options.tolerance > 0.0 {
    options.tolerance
  } else {
    DEFAULT_TOLERANCE
  };
  report.tolerance = tolerance;

  let tokens = lex(bytes)?;
  let contours = {
    let mut interpreter = Interpreter::new(tolerance, origin, report);
    interpreter.run(&tokens);
    interpreter.finish()
  };

  if contours.is_empty() {
    if report.skipped.is_empty() {
      // A PostScript file that never names a path operator is not a drawing. The spine turns an
      // empty layer set into `NothingImported`; refusing the header-plus-nothing case here keeps
      // the reason precise instead of blaming a construct that does not exist.
      return Err(ImportError::parse_plain(
        "eps",
        "no path is stroked or filled: the file draws nothing",
      ));
    }
    return Ok(Vec::new());
  }

  Ok(vec![ImportedLayer {
    name: LAYER_NAME.to_string(),
    contours,
  }])
}

/// Check the PostScript header, and return the origin the drawing has to be shifted by, in mm.
fn read_header(bytes: &[u8], report: &mut ImportReport) -> Result<Point2, ImportError> {
  let first_line_end = bytes
    .iter()
    .position(|byte| *byte == b'\n')
    .unwrap_or(bytes.len());
  let first_line = String::from_utf8_lossy(&bytes[..first_line_end]);
  if !first_line.starts_with("%!PS") && !first_line.contains("%!PS-Adobe") {
    return Err(ImportError::refused("eps", "not a PostScript file"));
  }

  report.unit_scale = MM_PER_POINT;
  report.warn("PostScript units are 1/72 inch: coordinates are scaled by 25.4/72 to millimetres");

  let head = String::from_utf8_lossy(&bytes[..bytes.len().min(65_536)]);
  let mut coarse = None;
  let mut precise = None;
  for line in head.lines() {
    let line = line.trim_start();
    if let Some(rest) = line.strip_prefix("%%HiResBoundingBox:") {
      precise = parse_bounding_box(rest).or(precise);
    } else if let Some(rest) = line.strip_prefix("%%BoundingBox:") {
      coarse = parse_bounding_box(rest).or(coarse);
    }
  }

  match precise.or(coarse) {
    Some([lower_left_x, lower_left_y, _, _]) => Ok(Point2::new(
      lower_left_x * MM_PER_POINT,
      lower_left_y * MM_PER_POINT,
    )),
    None => {
      report.warn("no %%BoundingBox or %%HiResBoundingBox: the drawing keeps its own origin");
      Ok(Point2::ORIGIN)
    }
  }
}

/// Read `llx lly urx ury` from a bounding-box header, or `None` when it is not four numbers.
fn parse_bounding_box(text: &str) -> Option<[f64; 4]> {
  let mut values = [0.0; 4];
  let mut words = text.split_whitespace();
  for value in &mut values {
    *value = words.next()?.parse::<f64>().ok()?;
  }
  Some(values)
}

/// One lexed PostScript token.
#[derive(Clone, Debug, PartialEq)]
enum Token {
  /// A number.
  Number(f64),
  /// A `/name` literal.
  Literal,
  /// A `(string)` or `<hex string>`.
  Text,
  /// `[`, `{` or `<<`.
  Open,
  /// `]`, `}` or `>>`.
  Close,
  /// A bare name: an operator, or a name the file looks up later.
  Operator(String),
}

/// A token and the line it came from, for report locations.
#[derive(Clone, Debug, PartialEq)]
struct Lexeme {
  token: Token,
  line: usize,
}

/// Split PostScript source into tokens.
///
/// Everything is a token stream: `%` comments run to the end of the line, strings may nest and be
/// escaped, and the operator of a construct is whatever name follows its arguments. Tokens carrying
/// binary bytes — image data — are dropped rather than reported as operators.
fn lex(bytes: &[u8]) -> Result<Vec<Lexeme>, ImportError> {
  let mut tokens = Vec::new();
  let mut index = 0usize;
  let mut line = 1usize;
  while index < bytes.len() {
    let byte = bytes[index];
    match byte {
      b'\n' => {
        line += 1;
        index += 1;
      }
      b' ' | b'\t' | b'\r' | 0x0c | 0x00 => index += 1,
      b'%' => {
        while index < bytes.len() && bytes[index] != b'\n' {
          index += 1;
        }
      }
      b'(' => {
        let opening_line = line;
        index = skip_literal_string(bytes, index, &mut line)?;
        tokens.push(Lexeme {
          token: Token::Text,
          line: opening_line,
        });
      }
      b'<' => {
        if bytes.get(index + 1) == Some(&b'<') {
          index += 2;
          tokens.push(Lexeme {
            token: Token::Open,
            line,
          });
        } else {
          let opening_line = line;
          index = skip_hex_string(bytes, index, &mut line)?;
          tokens.push(Lexeme {
            token: Token::Text,
            line: opening_line,
          });
        }
      }
      b'>' => {
        index += if bytes.get(index + 1) == Some(&b'>') {
          2
        } else {
          1
        };
        tokens.push(Lexeme {
          token: Token::Close,
          line,
        });
      }
      b'[' | b'{' => {
        index += 1;
        tokens.push(Lexeme {
          token: Token::Open,
          line,
        });
      }
      b']' | b'}' | b')' => {
        index += 1;
        tokens.push(Lexeme {
          token: Token::Close,
          line,
        });
      }
      b'/' => {
        index += 1;
        let (_, next) = read_word(bytes, index);
        index = next;
        tokens.push(Lexeme {
          token: Token::Literal,
          line,
        });
      }
      _ => {
        let opening_line = line;
        let (word, next) = read_word(bytes, index);
        index = next.max(index + 1);
        if word.is_empty() || word.bytes().any(|byte| !(0x21..=0x7e).contains(&byte)) {
          continue;
        }
        let token = match word.parse::<f64>() {
          Ok(value) if value.is_finite() => Token::Number(value),
          _ => Token::Operator(word),
        };
        tokens.push(Lexeme {
          token,
          line: opening_line,
        });
      }
    }
  }
  Ok(tokens)
}

/// True when `byte` ends a token.
fn is_delimiter(byte: u8) -> bool {
  matches!(
    byte,
    b' '
      | b'\t'
      | b'\r'
      | b'\n'
      | 0x0c
      | 0x00
      | b'('
      | b')'
      | b'<'
      | b'>'
      | b'['
      | b']'
      | b'{'
      | b'}'
      | b'/'
      | b'%'
  )
}

/// Read a word starting at `start`, returning it and the index just past it.
fn read_word(bytes: &[u8], start: usize) -> (String, usize) {
  let mut end = start;
  while end < bytes.len() && !is_delimiter(bytes[end]) {
    end += 1;
  }
  (
    String::from_utf8_lossy(&bytes[start..end]).into_owned(),
    end,
  )
}

/// Skip a `(literal string)`, honouring escapes and nested parentheses.
fn skip_literal_string(bytes: &[u8], start: usize, line: &mut usize) -> Result<usize, ImportError> {
  let opening_line = *line;
  let mut index = start;
  let mut depth = 0usize;
  while index < bytes.len() {
    match bytes[index] {
      b'\\' => {
        if index + 1 < bytes.len() {
          if bytes[index + 1] == b'\n' {
            *line += 1;
          }
          index += 2;
        } else {
          index += 1;
        }
      }
      b'(' => {
        depth += 1;
        index += 1;
      }
      b')' => {
        depth = depth.saturating_sub(1);
        index += 1;
        if depth == 0 {
          return Ok(index);
        }
      }
      b'\n' => {
        *line += 1;
        index += 1;
      }
      _ => index += 1,
    }
  }
  Err(ImportError::parse(
    "eps",
    "unterminated string literal",
    format!("line {opening_line}"),
  ))
}

/// Skip a `<hex string>`, counting the lines it spans.
fn skip_hex_string(bytes: &[u8], start: usize, line: &mut usize) -> Result<usize, ImportError> {
  let opening_line = *line;
  let mut index = start + 1;
  while index < bytes.len() {
    match bytes[index] {
      b'>' => return Ok(index + 1),
      b'\n' => {
        *line += 1;
        index += 1;
      }
      _ => index += 1,
    }
  }
  Err(ImportError::parse(
    "eps",
    "unterminated hex string",
    format!("line {opening_line}"),
  ))
}

/// A PostScript transformation matrix: `x' = a·x + c·y + e`, `y' = b·x + d·y + f`.
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
  /// The identity: user space and device space agree.
  const IDENTITY: Self = Self {
    a: 1.0,
    b: 0.0,
    c: 0.0,
    d: 1.0,
    e: 0.0,
    f: 0.0,
  };

  /// Map a point through this matrix.
  fn apply(self, point: Point2) -> Point2 {
    Point2::new(
      self.a * point.x + self.c * point.y + self.e,
      self.b * point.x + self.d * point.y + self.f,
    )
  }

  /// `self` applied *after* `other`, which is what PostScript's `concat` and friends do.
  fn compose(self, other: Self) -> Self {
    Self {
      a: self.a * other.a + self.c * other.b,
      b: self.b * other.a + self.d * other.b,
      c: self.a * other.c + self.c * other.d,
      d: self.b * other.c + self.d * other.d,
      e: self.a * other.e + self.c * other.f + self.e,
      f: self.b * other.e + self.d * other.f + self.f,
    }
  }

  /// Signed area scale: negative when the matrix reflects.
  fn determinant(self) -> f64 {
    self.a * self.d - self.b * self.c
  }

  /// The X column's length.
  fn x_stretch(self) -> f64 {
    self.a.hypot(self.b)
  }

  /// The Y column's length.
  fn y_stretch(self) -> f64 {
    self.c.hypot(self.d)
  }

  /// A translation.
  fn translated(tx: f64, ty: f64) -> Self {
    Self {
      e: tx,
      f: ty,
      ..Self::IDENTITY
    }
  }

  /// A scale.
  fn scaled(sx: f64, sy: f64) -> Self {
    Self {
      a: sx,
      d: sy,
      ..Self::IDENTITY
    }
  }

  /// A rotation by `radians`, counter-clockwise.
  fn rotated(radians: f64) -> Self {
    let (sin, cos) = radians.sin_cos();
    Self {
      a: cos,
      b: sin,
      c: -sin,
      d: cos,
      ..Self::IDENTITY
    }
  }
}

/// A value on the operand stack.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Operand {
  /// A number, which an operator can consume.
  Number(f64),
  /// Anything else: a name, a string, an array. Operators that need numbers skip instead of
  /// silently reading the wrong value.
  Other,
}

/// One operator outside the imported subset, aggregated by name.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Unsupported {
  /// How many times it was met.
  count: usize,
  /// The line of its first use.
  line: usize,
  /// Why it produced no geometry.
  reason: &'static str,
}

/// The current path, in device millimetres, with the user-space points operators need to resolve
/// relative coordinates.
#[derive(Default)]
struct Path {
  finished: Vec<(Vec<Segment>, bool)>,
  open: Vec<Segment>,
  current: Option<Point2>,
  current_device: Option<Point2>,
  start: Option<Point2>,
  start_device: Option<Point2>,
}

impl Path {
  /// Forget everything, as `newpath` does.
  fn clear(&mut self) {
    *self = Self::default();
  }

  /// True when a segment is waiting to be painted or reported.
  fn has_geometry(&self) -> bool {
    !self.open.is_empty() || !self.finished.is_empty()
  }

  /// Start a new subpath, finishing the previous one as open.
  fn move_to(&mut self, user: Point2, device: Point2) {
    self.flush(false);
    self.current = Some(user);
    self.current_device = Some(device);
    self.start = Some(user);
    self.start_device = Some(device);
  }

  /// Finish the current subpath, open or closed.
  fn flush(&mut self, closed: bool) {
    if self.open.is_empty() {
      return;
    }
    let segments = std::mem::take(&mut self.open);
    self.finished.push((segments, closed));
  }

  /// Take every subpath and forget the path, as a paint operator does.
  fn take(&mut self) -> Vec<(Vec<Segment>, bool)> {
    self.flush(false);
    self.current = None;
    self.current_device = None;
    self.start = None;
    self.start_device = None;
    std::mem::take(&mut self.finished)
  }
}

/// The PostScript interpreter: a token loop, a matrix stack and a path builder.
struct Interpreter<'a> {
  report: &'a mut ImportReport,
  tolerance: f64,
  origin: Point2,
  ctm: Matrix,
  saved: Vec<Matrix>,
  operands: Vec<Operand>,
  path: Path,
  contours: Vec<Contour>,
  unsupported: BTreeMap<String, Unsupported>,
  path_line: Option<usize>,
}

impl<'a> Interpreter<'a> {
  /// A fresh interpreter whose device space is millimetres with `origin` removed.
  fn new(tolerance: f64, origin: Point2, report: &'a mut ImportReport) -> Self {
    Self {
      report,
      tolerance,
      origin,
      ctm: Matrix::IDENTITY,
      saved: Vec::new(),
      operands: Vec::new(),
      path: Path::default(),
      contours: Vec::new(),
      unsupported: BTreeMap::new(),
      path_line: None,
    }
  }

  /// Map a user-space point to output millimetres.
  fn device(&self, point: Point2) -> Point2 {
    let user = self.ctm.apply(point);
    Point2::new(
      user.x * MM_PER_POINT - self.origin.x,
      user.y * MM_PER_POINT - self.origin.y,
    )
  }

  /// Run the token stream. Nothing here is fatal: a construct that cannot be imported is reported.
  fn run(&mut self, tokens: &[Lexeme]) {
    let mut collected = 0usize;
    for lexeme in tokens {
      match &lexeme.token {
        Token::Open => {
          collected += 1;
          if collected == 1 {
            // An array, dictionary or procedure is one opaque operand: its contents are not
            // executed here, and the operators that would (`exec`, `if`, `forall`) are reported.
            self.operands.push(Operand::Other);
          }
        }
        Token::Close => collected = collected.saturating_sub(1),
        _ if collected > 0 => {}
        Token::Number(value) => self.operands.push(Operand::Number(*value)),
        Token::Literal | Token::Text => self.operands.push(Operand::Other),
        Token::Operator(name) => self.operate(name, lexeme.line),
      }
    }
  }

  /// Consume `count` numeric operands, bottom first, or `None` when they are not there.
  fn take(&mut self, count: usize) -> Option<Vec<f64>> {
    let start = self.operands.len().checked_sub(count)?;
    let mut values = Vec::with_capacity(count);
    for operand in &self.operands[start..] {
      match operand {
        Operand::Number(value) => values.push(*value),
        Operand::Other => return None,
      }
    }
    self.operands.truncate(start);
    Some(values)
  }

  /// Report an operator whose operands were not the numbers it needs, and resynchronise.
  fn mismatch(&mut self, name: &str, line: usize) {
    self.report.skip(
      format!("operator {name}"),
      "the operands on the stack are not numbers, so the operator was ignored",
      Some(format!("line {line}")),
    );
    self.operands.clear();
  }

  /// Remember where the path currently being built started, for the "never painted" report.
  fn note_path(&mut self, line: usize) {
    if self.path_line.is_none() {
      self.path_line = Some(line);
    }
  }

  /// Apply one operator.
  fn operate(&mut self, name: &str, line: usize) {
    match name {
      "newpath" => {
        self.path.clear();
        self.path_line = None;
      }
      "moveto" => match self.take(2) {
        Some(values) => {
          let user = Point2::new(values[0], values[1]);
          let device = self.device(user);
          self.note_path(line);
          self.path.move_to(user, device);
          self.report.count("moveto", 1);
        }
        None => self.mismatch(name, line),
      },
      "lineto" => match self.take(2) {
        Some(values) => {
          self.line_to(Point2::new(values[0], values[1]));
          self.report.count("lineto", 1);
        }
        None => self.mismatch(name, line),
      },
      "rlineto" => match self.take(2) {
        Some(values) => {
          let from = self.path.current.unwrap_or(Point2::ORIGIN);
          self.line_to(Point2::new(from.x + values[0], from.y + values[1]));
          self.report.count("rlineto", 1);
        }
        None => self.mismatch(name, line),
      },
      "curveto" => match self.take(6) {
        Some(values) => {
          let first = Point2::new(values[0], values[1]);
          let second = Point2::new(values[2], values[3]);
          let end = Point2::new(values[4], values[5]);
          let start = self.path.current.unwrap_or(first);
          self.curve_to(start, first, second, end);
          self.report.flatten(1);
          self.report.count("curveto", 1);
        }
        None => self.mismatch(name, line),
      },
      "rcurveto" => match self.take(6) {
        Some(values) => {
          let from = self.path.current.unwrap_or(Point2::ORIGIN);
          let first = Point2::new(from.x + values[0], from.y + values[1]);
          let second = Point2::new(from.x + values[2], from.y + values[3]);
          let end = Point2::new(from.x + values[4], from.y + values[5]);
          self.curve_to(from, first, second, end);
          self.report.flatten(1);
          self.report.count("rcurveto", 1);
        }
        None => self.mismatch(name, line),
      },
      "closepath" => {
        if let (Some(start), Some(start_device)) = (self.path.start, self.path.start_device) {
          if let Some(current) = self.path.current_device {
            if current.distance_to(start_device) > SHORTEST_SEGMENT {
              self
                .path
                .open
                .push(Segment::Line(LineSeg::new(current, start_device)));
            }
          }
          self.note_path(line);
          self.close_current(start, start_device);
          self.report.count("closepath", 1);
        }
      }
      "arc" | "arcn" => match self.take(5) {
        Some(values) => self.arc(&values, name == "arc", name, line),
        None => self.mismatch(name, line),
      },
      "stroke" | "s" => self.paint(false, "stroke", line),
      "fill" | "f" | "F" | "b" | "B" => self.paint(true, "fill", line),
      "eofill" | "f*" | "b*" | "B*" => self.paint(true, "eofill", line),
      "clip" => {
        if self.path.has_geometry() {
          self.report.count("clip", 1);
        }
        self.path.take();
        self.path_line = None;
      }
      "n" => {
        self.path.take();
        self.path_line = None;
      }
      // `W`/`W*` set a clip from the current path; the path itself survives until a paint operator
      // or `n` consumes it, so there is nothing to do here.
      "W" | "W*" => {}
      "translate" => match self.take(2) {
        Some(values) => self.ctm = self.ctm.compose(Matrix::translated(values[0], values[1])),
        None => self.mismatch(name, line),
      },
      "scale" => match self.take(2) {
        Some(values) => self.ctm = self.ctm.compose(Matrix::scaled(values[0], values[1])),
        None => self.mismatch(name, line),
      },
      "rotate" => match self.take(1) {
        Some(values) => self.ctm = self.ctm.compose(Matrix::rotated(values[0].to_radians())),
        None => self.mismatch(name, line),
      },
      "concat" => match self.take(6) {
        Some(values) => self.ctm = self.ctm.compose(matrix_from(&values)),
        None => self.mismatch(name, line),
      },
      "setmatrix" => match self.take(6) {
        Some(values) => self.ctm = matrix_from(&values),
        None => self.mismatch(name, line),
      },
      "gsave" => self.saved.push(self.ctm),
      "grestore" => {
        if let Some(matrix) = self.saved.pop() {
          self.ctm = matrix;
        }
      }
      "show" | "ashow" | "widthshow" | "awidthshow" | "kshow" | "glyphshow" | "charpath" => {
        self.unsupported_operator(name, line, "text is not vector geometry");
      }
      "image" | "imagemask" | "colorimage" => {
        self.unsupported_operator(name, line, "raster images are not vector geometry");
      }
      "sh" | "shfill" => {
        self.unsupported_operator(name, line, "shadings are not vector geometry");
      }
      other => {
        if !is_ignored(other) {
          self.unsupported_operator(
            other,
            line,
            "the operator is outside the imported PostScript path subset",
          );
        }
      }
    }
  }

  /// Aggregate a construct we cannot import, so the report names it once with its count.
  fn unsupported_operator(&mut self, name: &str, line: usize, reason: &'static str) {
    let entry = self
      .unsupported
      .entry(name.to_string())
      .or_insert(Unsupported {
        count: 0,
        line,
        reason,
      });
    entry.count += 1;
  }

  /// Add a straight segment from the current point, or start the path when there is none.
  fn line_to(&mut self, user: Point2) {
    let device = self.device(user);
    match self.path.current_device {
      Some(from) => {
        if from.distance_to(device) > SHORTEST_SEGMENT {
          self
            .path
            .open
            .push(Segment::Line(LineSeg::new(from, device)));
        }
      }
      None => self.path.move_to(user, device),
    }
    self.path.current = Some(user);
    self.path.current_device = Some(device);
  }

  /// Flatten a cubic Bézier into straight segments at the caller's tolerance.
  fn curve_to(&mut self, start: Point2, first: Point2, second: Point2, end: Point2) {
    if self.path.current_device.is_none() {
      let device = self.device(start);
      self.path.move_to(start, device);
    }
    let points = flatten_cubic(
      self.device(start),
      self.device(first),
      self.device(second),
      self.device(end),
      self.tolerance,
    );
    self.push_points(&points[1..], end);
  }

  /// Append straight segments through `points`, ending at `end` in user space.
  fn push_points(&mut self, points: &[Point2], end: Point2) {
    let Some(mut previous) = self.path.current_device else {
      return;
    };
    for point in points {
      if previous.distance_to(*point) > SHORTEST_SEGMENT {
        self
          .path
          .open
          .push(Segment::Line(LineSeg::new(previous, *point)));
      }
      previous = *point;
    }
    self.path.current = Some(end);
    self.path.current_device = Some(previous);
  }

  /// Close the current subpath and leave the current point on its start, as `closepath` does.
  fn close_current(&mut self, start: Point2, start_device: Point2) {
    self.path.flush(true);
    self.path.current = Some(start);
    self.path.current_device = Some(start_device);
    self.path.start = Some(start);
    self.path.start_device = Some(start_device);
  }

  /// How much a matrix scales lengths by, in output millimetres.
  fn max_stretch(&self) -> f64 {
    self.ctm.x_stretch().max(self.ctm.y_stretch()) * MM_PER_POINT
  }

  /// The uniform scale in millimetres and the reflection sign, when the matrix is a similarity.
  ///
  /// A similarity keeps a circle a circle, so an arc can stay exact; anything else turns it into an
  /// ellipse, which circular arcs cannot express.
  fn similarity(&self) -> Option<(f64, f64)> {
    let x = self.ctm.x_stretch();
    let y = self.ctm.y_stretch();
    let scale = x.max(y);
    if scale <= 0.0 {
      return None;
    }
    let slack = 1.0e-9 * scale;
    let shear = self.ctm.a * self.ctm.c + self.ctm.b * self.ctm.d;
    if (x - y).abs() > slack || shear.abs() > slack {
      return None;
    }
    let sign = if self.ctm.determinant() >= 0.0 {
      1.0
    } else {
      -1.0
    };
    Some((scale * MM_PER_POINT, sign))
  }

  /// Interpret `arc` (`counter_clockwise`) or `arcn`.
  fn arc(&mut self, values: &[f64], counter_clockwise: bool, name: &str, line: usize) {
    self.note_path(line);
    let center = Point2::new(values[0], values[1]);
    let radius = values[2];
    let start_degrees = values[3];
    let end_degrees = values[4];

    if !radius.is_finite()
      || radius <= 0.0
      || !start_degrees.is_finite()
      || !end_degrees.is_finite()
    {
      self.report.skip(
        name,
        "the arc's radius or angles are not numbers",
        Some(format!("line {line}")),
      );
      return;
    }

    let mut end = end_degrees;
    if counter_clockwise {
      while end < start_degrees {
        end += FULL_TURN_DEGREES;
      }
    } else {
      while end > start_degrees {
        end -= FULL_TURN_DEGREES;
      }
    }
    let mut sweep = end - start_degrees;
    sweep = if counter_clockwise {
      sweep.min(FULL_TURN_DEGREES)
    } else {
      sweep.max(-FULL_TURN_DEGREES)
    };

    let start_radians = start_degrees.to_radians();
    let start_point = center + Vec2::from_angle(start_radians, radius);
    let start_device = self.device(start_point);

    // PostScript joins the current point to the arc's start with a straight line when they differ.
    match self.path.current_device {
      None => self.path.move_to(start_point, start_device),
      Some(current) => {
        if current.distance_to(start_device) > SAME_POINT * radius.max(1.0) {
          self.line_to(start_point);
        }
      }
    }

    if sweep.abs() < ZERO_SWEEP_DEGREES {
      self.report.skip(
        name,
        "the arc sweeps zero degrees, so it draws nothing",
        Some(format!("line {line}")),
      );
      return;
    }

    if sweep.abs() >= FULL_TURN_DEGREES - FULL_TURN_EPSILON {
      self.full_circle(center, radius, start_radians, name, line);
      return;
    }

    let sweep_radians = sweep.to_radians();
    let Some((scale, sign)) = self.similarity() else {
      self.flatten_arc(center, radius, start_radians, sweep_radians, name, line);
      return;
    };

    let center_device = self.device(center);
    let start_angle = (start_device - center_device).angle();
    match Arc::new(
      center_device,
      radius * scale,
      start_angle,
      sweep_radians * sign,
    ) {
      Ok(arc) => {
        self.path.open.push(Segment::Arc(arc));
        self.path.current = Some(center + Vec2::from_angle(start_radians + sweep_radians, radius));
        self.path.current_device = Some(arc.end_point());
        self.report.count(name, 1);
      }
      Err(error) => self.report.skip(
        name,
        format!("the arc is degenerate: {error}"),
        Some(format!("line {line}")),
      ),
    }
  }

  /// A full turn: two exact half-turn arcs forming a closed circle of its own.
  fn full_circle(&mut self, center: Point2, radius: f64, start: f64, name: &str, line: usize) {
    let start_point = center + Vec2::from_angle(start, radius);
    let start_device = self.device(start_point);
    // The circle is its own closed subpath: whatever was drawn before stays a separate one.
    self.path.flush(false);
    self.path.move_to(start_point, start_device);

    match self.similarity() {
      Some((scale, sign)) => {
        let center_device = self.device(center);
        let start_angle = (start_device - center_device).angle();
        let half = PI * sign;
        let first = Arc::new(center_device, radius * scale, start_angle, half);
        let second = Arc::new(center_device, radius * scale, start_angle + half, half);
        match (first, second) {
          (Ok(first), Ok(second)) => {
            self.path.open.push(Segment::Arc(first));
            self.path.open.push(Segment::Arc(second));
            self.close_current(start_point, start_device);
            self.report.count(name, 1);
          }
          (first, _) => {
            let error = first
              .err()
              .map(|error| error.to_string())
              .unwrap_or_default();
            self.report.skip(
              name,
              format!("the circle is degenerate: {error}"),
              Some(format!("line {line}")),
            );
          }
        }
      }
      None => {
        let sign = if self.ctm.determinant() >= 0.0 {
          1.0
        } else {
          -1.0
        };
        self.flatten_arc(center, radius, start, TAU * sign, name, line);
        self.close_current(start_point, start_device);
      }
    }
  }

  /// Sample an arc the matrix has turned into an ellipse, count it, and keep the endpoint.
  fn flatten_arc(
    &mut self,
    center: Point2,
    radius: f64,
    start: f64,
    sweep: f64,
    name: &str,
    line: usize,
  ) {
    let stretch = self.max_stretch().max(f64::MIN_POSITIVE);
    let steps = match Arc::new(center, radius, start, sweep) {
      Ok(arc) => arc.step_count(self.tolerance / stretch),
      Err(error) => {
        self.report.skip(
          name,
          format!("the arc is degenerate: {error}"),
          Some(format!("line {line}")),
        );
        return;
      }
    };
    let points: Vec<Point2> = (0..=steps)
      .map(|step| {
        let angle = start + sweep * step as f64 / steps as f64;
        self.device(center + Vec2::from_angle(angle, radius))
      })
      .collect();
    self.push_points(
      &points[1..],
      center + Vec2::from_angle(start + sweep, radius),
    );
    self.report.flatten(1);
    self.report.count(name, 1);
  }

  /// Turn the current path into contours, `close_open` when the paint operator fills it.
  fn paint(&mut self, close_open: bool, name: &str, line: usize) {
    self.path_line = None;
    let before = self.contours.len();
    for (segments, closed) in self.path.take() {
      let closed = closed || close_open;
      match Contour::from_segments(&segments, closed) {
        Ok(contour) => self.contours.push(normalise_winding(contour)),
        Err(error) => self.report.skip(
          name,
          format!("the path does not form a contour: {error}"),
          Some(format!("line {line}")),
        ),
      }
    }
    if self.contours.len() > before {
      self.report.count(name, 1);
    }
  }

  /// Report what was left over, then hand the contours back.
  fn finish(mut self) -> Vec<Contour> {
    if self.path.has_geometry() || self.path_line.is_some() {
      let location = self.path_line.map(|line| format!("line {line}"));
      self.report.skip(
        "path",
        "the path was never stroked, filled or clipped",
        location,
      );
    }
    for (construct, entry) in std::mem::take(&mut self.unsupported) {
      let reason = if entry.count > 1 {
        format!("{} ({} occurrences)", entry.reason, entry.count)
      } else {
        entry.reason.to_string()
      };
      self
        .report
        .skip(construct, reason, Some(format!("line {}", entry.line)));
    }
    self.contours
  }
}

/// Build a matrix from the six operands `a b c d e f`.
fn matrix_from(values: &[f64]) -> Matrix {
  Matrix {
    a: values[0],
    b: values[1],
    c: values[2],
    d: values[3],
    e: values[4],
    f: values[5],
  }
}

/// Closed contours leave the importer counter-clockwise, the kernel's convention.
fn normalise_winding(contour: Contour) -> Contour {
  if contour.is_closed() && contour.signed_area() < 0.0 {
    contour.reverse()
  } else {
    contour
  }
}

/// Split a cubic Bézier until each chord is within `tolerance` of the curve.
///
/// The flattener compares the control points with the chord: a cubic never strays further from its
/// chord than its control points do, so accepting a chord whose controls are within `tolerance` of
/// it bounds the deviation by `tolerance`. The subdivision is capped in depth and in points, so a
/// pathological curve cannot allocate without limit.
fn flatten_cubic(
  start: Point2,
  first: Point2,
  second: Point2,
  end: Point2,
  tolerance: f64,
) -> Vec<Point2> {
  let mut points = vec![start];
  let mut pending = vec![(start, first, second, end, 0usize)];
  while let Some((from, first, second, to, depth)) = pending.pop() {
    if depth >= MAX_CURVE_DEPTH
      || points.len() >= MAX_CURVE_POINTS
      || is_flat(from, first, second, to, tolerance)
    {
      points.push(to);
      continue;
    }
    let (left, right) = split_cubic(from, first, second, to);
    pending.push((right.0, right.1, right.2, right.3, depth + 1));
    pending.push((left.0, left.1, left.2, left.3, depth + 1));
  }
  points
}

/// True when both control points are within `tolerance` of the chord.
fn is_flat(start: Point2, first: Point2, second: Point2, end: Point2, tolerance: f64) -> bool {
  let chord = LineSeg::new(start, end);
  chord
    .distance_to_point(first)
    .max(chord.distance_to_point(second))
    <= tolerance
}

/// A cubic Bézier's four control points.
type Cubic = (Point2, Point2, Point2, Point2);

/// Split a cubic at its midpoint, returning the two halves in order.
fn split_cubic(start: Point2, first: Point2, second: Point2, end: Point2) -> (Cubic, Cubic) {
  let low = start.lerp(first, 0.5);
  let middle = first.lerp(second, 0.5);
  let high = second.lerp(end, 0.5);
  let low_middle = low.lerp(middle, 0.5);
  let high_middle = middle.lerp(high, 0.5);
  let joint = low_middle.lerp(high_middle, 0.5);
  (
    (start, low, low_middle, joint),
    (joint, high_middle, high, end),
  )
}

/// Operators that change style, the dictionary stack or the operand stack, none of which affects
/// the geometry this importer builds.
const IGNORED_OPERATORS: &[&str] = &[
  "a3",
  "a4",
  "a5",
  "b5",
  "begin",
  "bind",
  "clear",
  "cleardictstack",
  "cleartomark",
  "composefont",
  "copy",
  "count",
  "counttomark",
  "currentcacheparams",
  "currentdict",
  "currentdash",
  "currentfont",
  "currentgray",
  "currentlinecap",
  "currentlinejoin",
  "currentlinewidth",
  "currentmiterlimit",
  "currentpagedevice",
  "currentrgbcolor",
  "currentscreen",
  "currenttransfer",
  "def",
  "definefont",
  "definefontinfo",
  "defineusername",
  "dict",
  "dup",
  "end",
  "erasepage",
  "exch",
  "exec",
  "exit",
  "findfont",
  "forall",
  "globaldict",
  "if",
  "ifelse",
  "index",
  "known",
  "languagelevel",
  "legal",
  "length",
  "letter",
  "load",
  "loop",
  "makefont",
  "mark",
  "nulldevice",
  "pop",
  "product",
  "repeat",
  "restore",
  "roll",
  "save",
  "scalefont",
  "selectfont",
  "setcachedevice",
  "setcachedevice2",
  "setcharwidth",
  "setcmykcolor",
  "setcolor",
  "setcolorspace",
  "setcolortransfer",
  "setdash",
  "setflat",
  "setfont",
  "setgray",
  "sethsbcolor",
  "setlinecap",
  "setlinejoin",
  "setlinewidth",
  "setmiterlimit",
  "setoverprint",
  "setpagedevice",
  "setrgbcolor",
  "setscreen",
  "setstrokeadjust",
  "setsystemparams",
  "settransfer",
  "showpage",
  "stack",
  "stop",
  "stopped",
  "store",
  "stringwidth",
  "systemdict",
  "transform",
  "itransform",
  "userdict",
  "version",
  "where",
  "currentpoint",
  "currentmatrix",
];

/// True when an operator can be ignored because it changes nothing we import.
fn is_ignored(name: &str) -> bool {
  IGNORED_OPERATORS.contains(&name)
}
