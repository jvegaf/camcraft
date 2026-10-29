//! Text as contours: one single-stroke glyph table behind a [`GlyphSource`] trait.
//!
//! A document stores geometry, not font references (M2 A14): text is turned into contours here, at
//! creation time, so the schema stays `c2d/document@1` and no later stage — toolpath, post, view —
//! has to know that some geometry used to be a word.
//!
//! Everything behind [`GlyphSource`] works in **glyph units**, an abstract box whose baseline is
//! `y == 0` and whose cap height is [`GlyphSource::line_metrics`]'s `cap_height`. Only
//! [`text_outlines`] knows about millimetres, so a TrueType source (S4, D-M2-1) can be added later
//! by implementing the trait and changing nothing else.

use crate::contour::{Contour, Vertex};
use crate::point::Point2;
use crate::GeomError;

/// The metrics a text engine needs to lay out a line: both in **glyph units**.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineMetrics {
  /// Height of a capital letter above the baseline. This is what a caller's `height` refers to.
  pub cap_height: f64,
  /// Baseline-to-baseline distance for a new line: the same line advance for every character.
  pub line_advance: f64,
}

/// A source of glyph outlines.
///
/// Implementations return **glyph units** and never millimetres: the caller owns the size. A
/// character that the source cannot draw must answer [`None`] from [`GlyphSource::glyph`] instead of
/// returning an empty outline, because an empty outline is also what a space legitimately returns.
///
/// The trio is deliberately the whole trait: outlines, advance and the line metrics. A TrueType
/// implementation needs exactly these three (contours from the glyph outline, advance from `hmtx`,
/// cap height and line advance from the font header) and nothing in the engine should presume more.
pub trait GlyphSource {
  /// The contours of `character` in glyph units, or [`None`] when the source has no such glyph.
  ///
  /// A supported character that draws nothing (a space) returns `Some(Vec::new())`.
  fn glyph(&self, character: char) -> Option<Vec<Contour>>;

  /// The advance width of `character` in glyph units, i.e. how far the pen moves after it.
  ///
  /// Only meaningful for a character [`GlyphSource::glyph`] accepts; the engine refuses an unknown
  /// character before it ever asks for an advance.
  fn advance(&self, character: char) -> f64;

  /// The cap height and line advance of this source, in glyph units.
  fn line_metrics(&self) -> LineMetrics;
}

/// Cap height of the single-stroke table, in glyph units.
///
/// The table is drawn on a 21-unit box: baseline at `0`, cap height at `21`, x-height at `14`.
const CAP_HEIGHT: f64 = 21.0;

/// Baseline-to-baseline distance of the single-stroke table, in glyph units.
///
/// `30` units leaves nine units between a descender (`-6`) and the next cap (`21`), so two lines
/// never touch.
const LINE_ADVANCE: f64 = 30.0;

/// One glyph of the single-stroke table.
///
/// `strokes` is a compact coordinate table: every stroke is a flat `[x0, y0, x1, y1, …]` list of
/// integer glyph-unit coordinates. A stroke whose last point repeats its first is drawn **closed**;
/// anything else is an open polyline. Integer coordinates keep the table readable and make the
/// layout bit-for-bit reproducible.
struct GlyphData {
  /// The character this glyph draws.
  character: char,
  /// Advance width in glyph units; every point of every stroke stays inside `0..=advance`.
  advance: i8,
  /// The strokes, in drawing order.
  strokes: &'static [&'static [i8]],
}

/// A single-stroke stick font with integer coordinates in a 0..=21 unit box.
///
/// It draws with straight segments only: a stroke font is what an engraver's pen does, and an arc
/// here would have to be re-tessellated by every consumer anyway. Lowercase letters are their own
/// shapes on the `14`-unit x-height, not scaled copies of the capitals.
#[derive(Clone, Copy, Debug, Default)]
pub struct SingleStrokeFont;

impl SingleStrokeFont {
  /// Build the font. It carries no state, so this is just a readable way to write it.
  pub const fn new() -> Self {
    Self
  }
}

impl GlyphSource for SingleStrokeFont {
  fn glyph(&self, character: char) -> Option<Vec<Contour>> {
    glyph_data(character).map(build_glyph)
  }

  fn advance(&self, character: char) -> f64 {
    glyph_data(character).map_or(0.0, |data| f64::from(data.advance))
  }

  fn line_metrics(&self) -> LineMetrics {
    LineMetrics {
      cap_height: CAP_HEIGHT,
      line_advance: LINE_ADVANCE,
    }
  }
}

/// Turn `text` into contours in millimetres.
///
/// `height` is the **cap height** in millimetres, so the same text at twice the height is exactly
/// twice as tall. `origin` is the baseline start of the first line: text runs along `+X`, and every
/// `'\n'` starts a new line one [`LineMetrics::line_advance`] **below** the previous one, back at
/// `origin.x`. Glyphs are emitted character by character in reading order, one contour per stroke,
/// so the result is deterministic: the same input always yields the same vertex order and numbers.
///
/// A character the source cannot draw is **refused**, never skipped: losing a letter silently would
/// produce a cut path with a hole in it that nobody asked for.
///
/// # Errors
///
/// [`GeomError::InvalidPrimitive`] when `height` is not positive and finite, when the source's
/// metrics are not usable, or when `text` holds a character the source has no glyph for (the reason
/// names the character). An error from [`Contour::new`] when a source hands back geometry that
/// breaks the kernel invariants.
pub fn text_outlines(
  text: &str,
  origin: Point2,
  height: f64,
  source: &impl GlyphSource,
) -> Result<Vec<Contour>, GeomError> {
  if !height.is_finite() || height <= 0.0 {
    return Err(GeomError::InvalidPrimitive {
      reason: "the text height must be a positive finite number of millimetres",
    });
  }
  let metrics = source.line_metrics();
  if !metrics.cap_height.is_finite() || metrics.cap_height <= 0.0 {
    return Err(GeomError::InvalidPrimitive {
      reason: "the glyph source must declare a positive cap height in glyph units",
    });
  }
  if !metrics.line_advance.is_finite() || metrics.line_advance <= 0.0 {
    return Err(GeomError::InvalidPrimitive {
      reason: "the glyph source must declare a positive line advance in glyph units",
    });
  }
  let scale = height / metrics.cap_height;

  let mut contours = Vec::new();
  let mut pen = 0.0_f64;
  let mut line = 0_usize;
  for character in text.chars() {
    if character == '\n' {
      line += 1;
      pen = 0.0;
      continue;
    }
    // Refuse before emitting anything for this character, so a refusal can never look like a
    // partially laid-out word.
    let glyph = source
      .glyph(character)
      .ok_or_else(|| unsupported_character(character))?;
    let offset_x = pen;
    let offset_y = -(line as f64) * metrics.line_advance;
    for stroke in &glyph {
      contours.push(place(stroke, origin, offset_x, offset_y, scale)?);
    }
    pen += source.advance(character);
  }
  Ok(contours)
}

/// Move one glyph-unit contour to millimetres: uniform scale, then translate.
///
/// The scale is uniform, so every bulge stays the bulge it was and no arc is lost. The vertices go
/// back through [`Contour::new`], which is where the "every contour is valid" guarantee comes from.
fn place(
  contour: &Contour,
  origin: Point2,
  offset_x: f64,
  offset_y: f64,
  scale: f64,
) -> Result<Contour, GeomError> {
  let vertices = contour
    .vertices()
    .iter()
    .map(|vertex| {
      Vertex::new(
        Point2::new(
          origin.x + (offset_x + vertex.point.x) * scale,
          origin.y + (offset_y + vertex.point.y) * scale,
        ),
        vertex.bulge,
      )
    })
    .collect();
  Contour::new(vertices, contour.is_closed())
}

/// The refusal for a character the source cannot draw, naming the character.
fn unsupported_character(character: char) -> GeomError {
  GeomError::UnsupportedCharacter { character }
}

/// Look a character up in the single-stroke table.
fn glyph_data(character: char) -> Option<&'static GlyphData> {
  GLYPHS.iter().find(|glyph| glyph.character == character)
}

/// Build one table entry into contours.
///
/// The table is compile-time data, so an entry that breaks the kernel invariants is a programming
/// error rather than a caller error; the coverage test walks every glyph, so it cannot ship unnoticed.
fn build_glyph(data: &GlyphData) -> Vec<Contour> {
  data
    .strokes
    .iter()
    .map(|stroke| build_stroke(stroke, data.character))
    .collect()
}

/// Build one stroke of a glyph, closing it when its last point repeats its first.
fn build_stroke(stroke: &[i8], character: char) -> Contour {
  debug_assert!(
    stroke.len() % 2 == 0,
    "a stroke is a flat list of x/y pairs"
  );
  let points: Vec<Point2> = stroke
    .chunks_exact(2)
    .map(|pair| Point2::new(f64::from(pair[0]), f64::from(pair[1])))
    .collect();
  let closed = points.len() > 2 && points.first() == points.last();
  let points = if closed {
    &points[..points.len() - 1]
  } else {
    &points[..]
  };
  Contour::from_points(points, closed).unwrap_or_else(|error| {
    panic!("the single-stroke glyph table entry for {character:?} is not valid geometry: {error}")
  })
}

/// The single-stroke table.
///
/// Coordinates are glyph units: baseline `0`, x-height `14`, cap height `21`, descenders down to
/// `-6`. Every long stroke is broken where a real pen would lift, and no point leaves `0..=advance`
/// horizontally, which is what makes the advance box a reliable bound for the layout.
static GLYPHS: &[GlyphData] = &[
  // --- space ---------------------------------------------------------------------------------
  GlyphData {
    character: ' ',
    advance: 8,
    strokes: &[],
  },
  // --- capitals ------------------------------------------------------------------------------
  GlyphData {
    character: 'A',
    advance: 14,
    strokes: &[&[0, 0, 7, 21, 14, 0], &[3, 7, 11, 7]],
  },
  GlyphData {
    character: 'B',
    advance: 13,
    strokes: &[
      &[0, 0, 0, 21],
      &[0, 21, 8, 21, 11, 18, 11, 14, 8, 11, 0, 11],
      &[0, 11, 9, 11, 12, 8, 12, 4, 9, 0, 0, 0],
    ],
  },
  GlyphData {
    character: 'C',
    advance: 13,
    strokes: &[&[12, 18, 9, 21, 4, 21, 0, 16, 0, 5, 4, 0, 9, 0, 12, 3]],
  },
  GlyphData {
    character: 'D',
    advance: 14,
    strokes: &[&[0, 0, 0, 21, 8, 21, 13, 16, 13, 5, 8, 0, 0, 0]],
  },
  GlyphData {
    character: 'E',
    advance: 14,
    strokes: &[
      &[0, 21, 0, 0],
      &[0, 21, 13, 21],
      &[0, 10, 10, 10],
      &[0, 0, 13, 0],
    ],
  },
  GlyphData {
    character: 'F',
    advance: 13,
    strokes: &[&[0, 0, 0, 21], &[0, 21, 12, 21], &[0, 10, 9, 10]],
  },
  GlyphData {
    character: 'G',
    advance: 15,
    strokes: &[&[
      12, 18, 9, 21, 4, 21, 0, 16, 0, 5, 4, 0, 10, 0, 14, 4, 14, 10, 8, 10,
    ]],
  },
  GlyphData {
    character: 'H',
    advance: 15,
    strokes: &[&[0, 0, 0, 21], &[14, 0, 14, 21], &[0, 10, 14, 10]],
  },
  GlyphData {
    character: 'I',
    advance: 7,
    strokes: &[&[3, 0, 3, 21], &[1, 21, 5, 21], &[1, 0, 5, 0]],
  },
  GlyphData {
    character: 'J',
    advance: 12,
    strokes: &[&[11, 21, 11, 4, 8, 0, 4, 0, 1, 4]],
  },
  GlyphData {
    character: 'K',
    advance: 13,
    strokes: &[&[0, 0, 0, 21], &[12, 21, 0, 8], &[4, 12, 12, 0]],
  },
  GlyphData {
    character: 'L',
    advance: 13,
    strokes: &[&[0, 21, 0, 0, 12, 0]],
  },
  GlyphData {
    character: 'M',
    advance: 15,
    strokes: &[&[0, 0, 0, 21, 7, 10, 14, 21, 14, 0]],
  },
  GlyphData {
    character: 'N',
    advance: 15,
    strokes: &[&[0, 0, 0, 21, 14, 0, 14, 21]],
  },
  GlyphData {
    character: 'O',
    advance: 15,
    strokes: &[&[
      7, 21, 12, 17, 14, 10, 12, 3, 7, 0, 2, 3, 0, 10, 2, 17, 7, 21,
    ]],
  },
  GlyphData {
    character: 'P',
    advance: 14,
    strokes: &[&[0, 0, 0, 21, 9, 21, 13, 17, 13, 12, 9, 8, 0, 8]],
  },
  GlyphData {
    character: 'Q',
    advance: 15,
    strokes: &[
      &[
        7, 21, 12, 17, 14, 10, 12, 3, 7, 0, 2, 3, 0, 10, 2, 17, 7, 21,
      ],
      &[8, 6, 14, -2],
    ],
  },
  GlyphData {
    character: 'R',
    advance: 14,
    strokes: &[
      &[0, 0, 0, 21, 9, 21, 13, 17, 13, 12, 9, 8, 0, 8],
      &[5, 8, 13, 0],
    ],
  },
  GlyphData {
    character: 'S',
    advance: 13,
    strokes: &[&[
      12, 18, 9, 21, 3, 21, 0, 17, 0, 14, 3, 11, 9, 10, 12, 7, 12, 4, 9, 0, 3, 0, 0, 3,
    ]],
  },
  GlyphData {
    character: 'T',
    advance: 15,
    strokes: &[&[0, 21, 14, 21], &[7, 21, 7, 0]],
  },
  GlyphData {
    character: 'U',
    advance: 15,
    strokes: &[&[0, 21, 0, 5, 4, 0, 10, 0, 14, 5, 14, 21]],
  },
  GlyphData {
    character: 'V',
    advance: 15,
    strokes: &[&[0, 21, 7, 0, 14, 21]],
  },
  GlyphData {
    character: 'W',
    advance: 15,
    strokes: &[&[0, 21, 4, 0, 7, 11, 10, 0, 14, 21]],
  },
  GlyphData {
    character: 'X',
    advance: 15,
    strokes: &[&[0, 21, 14, 0], &[14, 21, 0, 0]],
  },
  GlyphData {
    character: 'Y',
    advance: 15,
    strokes: &[&[0, 21, 7, 10, 14, 21], &[7, 10, 7, 0]],
  },
  GlyphData {
    character: 'Z',
    advance: 15,
    strokes: &[&[0, 21, 14, 21, 0, 0, 14, 0]],
  },
  // --- lowercase -----------------------------------------------------------------------------
  GlyphData {
    character: 'a',
    advance: 12,
    strokes: &[
      &[10, 14, 10, 0],
      &[10, 10, 7, 14, 4, 14, 1, 11, 1, 3, 4, 0, 7, 0, 10, 4],
    ],
  },
  GlyphData {
    character: 'b',
    advance: 12,
    strokes: &[
      &[0, 21, 0, 0],
      &[0, 10, 4, 14, 8, 14, 10, 10, 10, 4, 8, 0, 4, 0, 0, 4],
    ],
  },
  GlyphData {
    character: 'c',
    advance: 11,
    strokes: &[&[9, 11, 6, 14, 3, 14, 0, 11, 0, 3, 3, 0, 6, 0, 9, 3]],
  },
  GlyphData {
    character: 'd',
    advance: 11,
    strokes: &[
      &[9, 21, 9, 0],
      &[9, 10, 6, 14, 3, 14, 0, 11, 0, 3, 3, 0, 6, 0, 9, 4],
    ],
  },
  GlyphData {
    character: 'e',
    advance: 12,
    strokes: &[&[
      0, 7, 10, 7, 10, 10, 7, 14, 3, 14, 0, 11, 0, 3, 3, 0, 7, 0, 10, 3,
    ]],
  },
  GlyphData {
    character: 'f',
    advance: 9,
    strokes: &[&[7, 21, 4, 21, 2, 18, 2, 0], &[0, 14, 6, 14]],
  },
  GlyphData {
    character: 'g',
    advance: 10,
    strokes: &[
      &[8, 14, 8, -2, 6, -5, 2, -6, 0, -4],
      &[8, 10, 5, 14, 2, 14, 0, 10, 0, 4, 2, 1, 5, 1, 8, 5],
    ],
  },
  GlyphData {
    character: 'h',
    advance: 10,
    strokes: &[&[0, 21, 0, 0], &[0, 9, 3, 14, 6, 14, 8, 10, 8, 0]],
  },
  GlyphData {
    character: 'i',
    advance: 7,
    strokes: &[&[3, 19, 3, 20], &[3, 0, 3, 14]],
  },
  GlyphData {
    character: 'j',
    advance: 7,
    strokes: &[&[4, 19, 4, 20], &[4, 14, 4, -3, 2, -6, 0, -5]],
  },
  GlyphData {
    character: 'k',
    advance: 10,
    strokes: &[&[0, 21, 0, 0], &[8, 14, 0, 6], &[3, 9, 8, 0]],
  },
  GlyphData {
    character: 'l',
    advance: 5,
    strokes: &[&[0, 21, 0, 2, 2, 0]],
  },
  GlyphData {
    character: 'm',
    advance: 11,
    strokes: &[
      &[0, 14, 0, 0],
      &[0, 10, 2, 14, 4, 12, 4, 0],
      &[4, 10, 6, 14, 8, 12, 8, 0],
    ],
  },
  GlyphData {
    character: 'n',
    advance: 9,
    strokes: &[&[0, 14, 0, 0], &[0, 10, 3, 14, 6, 12, 7, 0]],
  },
  GlyphData {
    character: 'o',
    advance: 12,
    strokes: &[&[6, 14, 9, 11, 9, 3, 6, 0, 3, 3, 3, 11, 6, 14]],
  },
  GlyphData {
    character: 'p',
    advance: 10,
    strokes: &[
      &[0, 14, 0, -6],
      &[0, 10, 3, 14, 6, 14, 8, 10, 8, 4, 6, 0, 3, 0, 0, 4],
    ],
  },
  GlyphData {
    character: 'q',
    advance: 10,
    strokes: &[
      &[8, 14, 8, -6],
      &[8, 10, 5, 14, 2, 14, 0, 10, 0, 4, 2, 0, 5, 0, 8, 4],
    ],
  },
  GlyphData {
    character: 'r',
    advance: 9,
    strokes: &[&[0, 14, 0, 0], &[0, 10, 3, 14, 7, 14]],
  },
  GlyphData {
    character: 's',
    advance: 9,
    strokes: &[&[
      7, 12, 5, 14, 2, 13, 1, 11, 3, 9, 6, 8, 8, 6, 7, 2, 4, 0, 0, 2,
    ]],
  },
  GlyphData {
    character: 't',
    advance: 8,
    strokes: &[&[2, 19, 2, 3, 4, 0, 6, 1], &[0, 14, 6, 14]],
  },
  GlyphData {
    character: 'u',
    advance: 9,
    strokes: &[&[0, 14, 0, 4, 2, 1, 5, 0, 7, 3], &[7, 14, 7, 0]],
  },
  GlyphData {
    character: 'v',
    advance: 10,
    strokes: &[&[0, 14, 4, 0, 8, 14]],
  },
  GlyphData {
    character: 'w',
    advance: 10,
    strokes: &[&[0, 14, 2, 0, 4, 7, 6, 0, 8, 14]],
  },
  GlyphData {
    character: 'x',
    advance: 10,
    strokes: &[&[0, 14, 8, 0], &[8, 14, 0, 0]],
  },
  GlyphData {
    character: 'y',
    advance: 10,
    strokes: &[&[0, 14, 4, 0], &[8, 14, 1, -6, 0, -4]],
  },
  GlyphData {
    character: 'z',
    advance: 10,
    strokes: &[&[0, 14, 8, 14, 0, 0, 8, 0]],
  },
  // --- digits --------------------------------------------------------------------------------
  GlyphData {
    character: '0',
    advance: 13,
    strokes: &[&[
      6, 21, 10, 18, 12, 12, 12, 9, 10, 3, 6, 0, 2, 3, 0, 9, 0, 12, 2, 18, 6, 21,
    ]],
  },
  GlyphData {
    character: '1',
    advance: 12,
    strokes: &[&[3, 17, 6, 21, 6, 0], &[2, 0, 10, 0]],
  },
  GlyphData {
    character: '2',
    advance: 13,
    strokes: &[&[0, 18, 3, 21, 8, 21, 11, 17, 9, 12, 0, 3, 0, 0, 12, 0]],
  },
  GlyphData {
    character: '3',
    advance: 13,
    strokes: &[&[
      0, 18, 3, 21, 8, 21, 11, 17, 8, 12, 4, 11, 8, 10, 11, 5, 8, 0, 3, 0, 0, 3,
    ]],
  },
  GlyphData {
    character: '4',
    advance: 13,
    strokes: &[&[9, 0, 9, 21, 0, 6, 12, 6]],
  },
  GlyphData {
    character: '5',
    advance: 13,
    strokes: &[&[11, 21, 1, 21, 0, 11, 4, 13, 9, 13, 12, 9, 9, 0, 4, 0, 0, 3]],
  },
  GlyphData {
    character: '6',
    advance: 13,
    strokes: &[&[
      11, 18, 8, 21, 4, 21, 1, 17, 0, 6, 3, 0, 8, 0, 11, 4, 11, 7, 8, 10, 4, 10, 0, 7,
    ]],
  },
  GlyphData {
    character: '7',
    advance: 13,
    strokes: &[&[0, 21, 12, 21, 4, 0]],
  },
  GlyphData {
    character: '8',
    advance: 13,
    strokes: &[
      &[6, 21, 9, 18, 9, 14, 6, 11, 3, 14, 3, 18, 6, 21],
      &[6, 11, 10, 8, 10, 3, 6, 0, 2, 3, 2, 8, 6, 11],
    ],
  },
  GlyphData {
    character: '9',
    advance: 13,
    strokes: &[&[
      1, 18, 4, 21, 8, 21, 11, 17, 12, 6, 9, 0, 4, 0, 1, 4, 1, 7, 4, 10, 8, 10, 12, 7,
    ]],
  },
  // --- punctuation ---------------------------------------------------------------------------
  GlyphData {
    character: '.',
    advance: 5,
    strokes: &[&[2, 0, 2, 2]],
  },
  GlyphData {
    character: ',',
    advance: 5,
    strokes: &[&[2, 4, 1, 0, 0, -3]],
  },
  GlyphData {
    character: ':',
    advance: 5,
    strokes: &[&[2, 11, 2, 13], &[2, 1, 2, 3]],
  },
  GlyphData {
    character: ';',
    advance: 5,
    strokes: &[&[2, 11, 2, 13], &[2, 4, 1, 0, 0, -3]],
  },
  GlyphData {
    character: '-',
    advance: 11,
    strokes: &[&[0, 10, 9, 10]],
  },
  GlyphData {
    character: '+',
    advance: 13,
    strokes: &[&[0, 7, 12, 7], &[6, 1, 6, 13]],
  },
  GlyphData {
    character: '/',
    advance: 11,
    strokes: &[&[0, -2, 10, 21]],
  },
  GlyphData {
    character: '\\',
    advance: 11,
    strokes: &[&[0, 21, 10, -2]],
  },
  GlyphData {
    character: '(',
    advance: 8,
    strokes: &[&[5, 21, 2, 17, 1, 10, 2, 3, 5, -1]],
  },
  GlyphData {
    character: ')',
    advance: 8,
    strokes: &[&[1, 21, 4, 17, 5, 10, 4, 3, 1, -1]],
  },
  GlyphData {
    character: '[',
    advance: 7,
    strokes: &[&[5, 21, 1, 21, 1, 0, 5, 0]],
  },
  GlyphData {
    character: ']',
    advance: 7,
    strokes: &[&[1, 21, 5, 21, 5, 0, 1, 0]],
  },
  GlyphData {
    character: '=',
    advance: 11,
    strokes: &[&[0, 12, 10, 12], &[0, 7, 10, 7]],
  },
  GlyphData {
    character: '*',
    advance: 11,
    strokes: &[&[5, 21, 5, 11], &[0, 18, 10, 14], &[0, 14, 10, 18]],
  },
  GlyphData {
    character: '!',
    advance: 6,
    strokes: &[&[2, 21, 2, 6], &[2, 0, 2, 2]],
  },
  GlyphData {
    character: '?',
    advance: 12,
    strokes: &[
      &[0, 17, 3, 21, 7, 21, 10, 17, 7, 12, 5, 10, 5, 7],
      &[5, 0, 5, 2],
    ],
  },
];
