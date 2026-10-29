//! Behaviour tests for text as contours (M2 A14).
//!
//! Text becomes geometry at creation time, so what matters here is the layout contract: every glyph
//! sits inside the advance box it was given, cap height is what `height` means, a newline starts a
//! line below, and a character the font cannot draw is **named** instead of dropped. No test dumps
//! vertices: a shape may be redrawn freely as long as the contract holds.

use cam_geom::text::{self, GlyphSource, SingleStrokeFont};
use cam_geom::{Contour, GeomError, Point2};

const EPS: f64 = 1.0e-9;

/// Exactly the characters the single-stroke table promises to draw.
const COVERAGE: &str =
  " ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789.,:;-+/\\()[]=*!?";

/// The punctuation part of the coverage, kept separately so the coverage test can name it.
const PUNCTUATION: &str = ".,:;-+/\\()[]=*!?";

fn assert_close(actual: f64, expected: f64, tolerance: f64, what: &str) {
  assert!(
    (actual - expected).abs() <= tolerance,
    "{what}: expected {expected}, got {actual}"
  );
}

/// An axis-aligned extent over the vertices of a group of contours.
#[derive(Clone, Copy, Debug)]
struct Extent {
  min_x: f64,
  max_x: f64,
  min_y: f64,
  max_y: f64,
}

impl Extent {
  fn of(contours: &[Contour]) -> Self {
    let mut extent = Self {
      min_x: f64::INFINITY,
      max_x: f64::NEG_INFINITY,
      min_y: f64::INFINITY,
      max_y: f64::NEG_INFINITY,
    };
    for contour in contours {
      for vertex in contour.vertices() {
        extent.min_x = extent.min_x.min(vertex.point.x);
        extent.max_x = extent.max_x.max(vertex.point.x);
        extent.min_y = extent.min_y.min(vertex.point.y);
        extent.max_y = extent.max_y.max(vertex.point.y);
      }
    }
    extent
  }

  fn height(self) -> f64 {
    self.max_y - self.min_y
  }
}

/// Lay out one character on its own, so a test can measure a single glyph.
fn one_character(font: &SingleStrokeFont, character: char, height: f64) -> Vec<Contour> {
  text::text_outlines(&character.to_string(), Point2::ORIGIN, height, font)
    .expect("a covered character is laid out")
}

/// How many contours one character contributes; a character with several strokes adds several.
fn stroke_count(font: &SingleStrokeFont, character: char) -> usize {
  font
    .glyph(character)
    .expect("the character is covered")
    .len()
}

fn assert_within_x(contour: &Contour, low: f64, high: f64, what: &str) {
  let extent = Extent::of(std::slice::from_ref(contour));
  assert!(
    extent.min_x >= low - EPS && extent.max_x <= high + EPS,
    "{what}: contour spans x {}..{}, outside the advance box {low}..{high}",
    extent.min_x,
    extent.max_x
  );
}

#[test]
fn two_capitals_land_each_in_its_own_advance_box() {
  let font = SingleStrokeFont::new();
  let origin = Point2::new(10.0, 20.0);
  let height = 5.0;
  let contours = text::text_outlines("AB", origin, height, &font).expect("A and B are covered");

  assert!(
    contours.len() >= 2,
    "two characters produce at least two contours, got {}",
    contours.len()
  );

  let metrics = font.line_metrics();
  let scale = height / metrics.cap_height;
  let advance_a = font.advance('A') * scale;
  let split = stroke_count(&font, 'A');
  let (first, second) = contours.split_at(split);

  for contour in first {
    assert_within_x(contour, origin.x, origin.x + advance_a, "'A'");
  }
  for contour in second {
    assert_within_x(
      contour,
      origin.x + advance_a,
      origin.x + advance_a + font.advance('B') * scale,
      "'B'",
    );
  }
}

#[test]
fn every_glyph_stays_inside_its_advance_box_in_a_whole_line() {
  let font = SingleStrokeFont::new();
  let line: String = COVERAGE
    .chars()
    .filter(|character| *character != ' ')
    .collect();
  let origin = Point2::new(-3.0, 7.0);
  let height = 12.0;
  let contours = text::text_outlines(&line, origin, height, &font).expect("the line is covered");
  let scale = height / font.line_metrics().cap_height;

  let mut pen = 0.0;
  let mut index = 0;
  for character in line.chars() {
    let strokes = stroke_count(&font, character);
    let low = origin.x + pen * scale;
    let high = origin.x + (pen + font.advance(character)) * scale;
    for contour in &contours[index..index + strokes] {
      assert_within_x(contour, low, high, &format!("{character:?}"));
      // The layout only ever returns contours built by the kernel's own constructor, so rebuilding
      // one must succeed: a contour that fails here is geometry nobody can cut.
      Contour::new(contour.vertices().to_vec(), contour.is_closed())
        .expect("a laid-out contour satisfies the kernel invariants");
    }
    index += strokes;
    pen += font.advance(character);
  }
  assert_eq!(index, contours.len(), "the whole line is accounted for");
}

#[test]
fn every_covered_glyph_is_valid_and_fits_its_box() {
  let font = SingleStrokeFont::new();
  let cap_height = font.line_metrics().cap_height;
  assert!(cap_height > 0.0, "the cap height is the height reference");
  assert!(
    font.line_metrics().line_advance > cap_height,
    "a line advance below the cap height would overlap two lines"
  );

  for character in COVERAGE.chars() {
    let glyph = font
      .glyph(character)
      .unwrap_or_else(|| panic!("{character:?} is promised by the coverage"));
    let advance = font.advance(character);
    assert!(advance > 0.0, "{character:?} must advance the pen");
    if character != ' ' {
      assert!(!glyph.is_empty(), "{character:?} must draw something");
    }
    for contour in &glyph {
      Contour::new(contour.vertices().to_vec(), contour.is_closed())
        .unwrap_or_else(|error| panic!("{character:?} is invalid geometry: {error}"));
      let extent = Extent::of(std::slice::from_ref(contour));
      assert!(
        extent.min_x >= -EPS && extent.max_x <= advance + EPS,
        "{character:?} spans x {}..{}, outside its advance box 0..{advance}",
        extent.min_x,
        extent.max_x
      );
      assert!(
        extent.min_y >= -0.5 * cap_height && extent.max_y <= cap_height + EPS,
        "{character:?} spans y {}..{}, outside the −cap/2..cap box",
        extent.min_y,
        extent.max_y
      );
    }
  }
}

#[test]
fn lowercase_is_its_own_shape_not_a_copy_of_uppercase() {
  let font = SingleStrokeFont::new();
  for (upper, lower) in ('A'..='Z').zip('a'..='z') {
    let upper_glyph = font.glyph(upper).expect("uppercase is covered");
    let lower_glyph = font.glyph(lower).expect("lowercase is covered");
    assert_ne!(
      upper_glyph, lower_glyph,
      "{lower:?} must not be a copy of {upper:?}"
    );
  }
  for character in "0123456789".chars().chain(PUNCTUATION.chars()) {
    assert!(
      font.glyph(character).is_some(),
      "{character:?} is promised by the coverage"
    );
  }
  assert_eq!(
    font.glyph(' ').expect("space is covered"),
    Vec::new(),
    "space advances the pen and draws nothing"
  );
}

#[test]
fn empty_text_and_spaces_draw_nothing() {
  let font = SingleStrokeFont::new();
  let empty = text::text_outlines("", Point2::new(1.0, 2.0), 5.0, &font).expect("empty text");
  assert!(empty.is_empty(), "an empty string has no contours");

  let spaces = text::text_outlines("   ", Point2::new(1.0, 2.0), 5.0, &font).expect("spaces");
  assert!(spaces.is_empty(), "spaces have no contours");

  let blank_lines = text::text_outlines("\n\n", Point2::new(1.0, 2.0), 5.0, &font)
    .expect("blank lines are still text");
  assert!(blank_lines.is_empty(), "blank lines have no contours");
}

#[test]
fn a_newline_starts_the_next_line_below_the_first() {
  let font = SingleStrokeFont::new();
  let origin = Point2::new(4.0, 30.0);
  let height = 6.0;
  let contours = text::text_outlines("A\nA", origin, height, &font).expect("two lines");

  let strokes = stroke_count(&font, 'A');
  assert_eq!(contours.len(), 2 * strokes, "each line draws one 'A'");
  let (first, second) = contours.split_at(strokes);
  let first = Extent::of(first);
  let second = Extent::of(second);

  let metrics = font.line_metrics();
  let scale = height / metrics.cap_height;
  assert_close(
    first.min_y,
    origin.y,
    EPS,
    "the first baseline is the origin",
  );
  assert_close(
    second.max_y,
    first.max_y - metrics.line_advance * scale,
    EPS,
    "the second line sits one line advance lower",
  );
  assert!(
    second.max_y < first.min_y,
    "the second line is entirely below the first: {} < {}",
    second.max_y,
    first.min_y
  );
  assert_close(
    first.min_x,
    origin.x,
    EPS,
    "the first line starts at the origin",
  );
  assert_close(
    second.min_x,
    origin.x,
    EPS,
    "a newline returns the pen to the origin",
  );
}

#[test]
fn doubling_the_height_doubles_every_glyph() {
  let font = SingleStrokeFont::new();
  for character in ['H', 'a', '0', '.', 'g', 'y'] {
    let single = Extent::of(&one_character(&font, character, 1.0));
    let double = Extent::of(&one_character(&font, character, 2.0));
    assert!(
      single.height() > 0.0,
      "{character:?} must have ink so scaling is observable"
    );
    assert_close(
      double.height(),
      2.0 * single.height(),
      EPS,
      &format!("{character:?} bounding-box height at twice the height"),
    );
    assert_close(
      double.max_x,
      2.0 * single.max_x,
      EPS,
      &format!("{character:?} right edge at twice the height"),
    );
  }
}

#[test]
fn the_same_text_twice_gives_the_same_contours() {
  let font = SingleStrokeFont::new();
  let origin = Point2::new(2.5, -1.5);
  let first = text::text_outlines("Ab9\nx y!", origin, 7.5, &font).expect("covered text");
  let second = text::text_outlines("Ab9\nx y!", origin, 7.5, &font).expect("covered text");
  assert_eq!(
    first, second,
    "the same input must produce the same vertex order and numbers"
  );
}

#[test]
fn an_unsupported_character_is_refused_and_named() {
  let font = SingleStrokeFont::new();
  let error = text::text_outlines("A€B", Point2::new(0.0, 0.0), 5.0, &font)
    .expect_err("the euro sign is not in the single-stroke table");
  assert!(
    matches!(error, GeomError::UnsupportedCharacter { character: '€' }),
    "{error:?}"
  );
  let message = error.to_string();
  assert!(
    message.contains('€'),
    "the refusal must name the character it could not draw, got {message:?}"
  );

  let tab = text::text_outlines("\t", Point2::new(0.0, 0.0), 5.0, &font)
    .expect_err("a tab is not in the single-stroke table");
  assert!(
    matches!(tab, GeomError::UnsupportedCharacter { character: '\t' }),
    "{tab:?}"
  );
}

#[test]
fn a_non_positive_height_is_refused() {
  let font = SingleStrokeFont::new();
  for height in [0.0, -1.0, f64::NAN, f64::INFINITY] {
    let error = text::text_outlines("A", Point2::new(0.0, 0.0), height, &font)
      .expect_err("a height that is not positive and finite must be refused");
    assert!(
      matches!(error, GeomError::InvalidPrimitive { .. }),
      "{height}: {error:?}"
    );
  }
}
