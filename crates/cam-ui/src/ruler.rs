//! The two rulers, and what they reported (camui-prancheta S5).
//!
//! A ruler is a band of the shell's own chrome, not part of the drawing surface: it is painted on
//! [`Role::Panel`](crate::theme::Role::Panel), its numbers are read off the camera, and the drawing area
//! starts where it ends. That last part is the whole reason this module exists: a band that only *looks*
//! like an edge would still hand the press underneath it to the sheet, so
//! [`Shell::canvas`](crate::Shell) insets the canvas by the band before it makes a response.
//!
//! # Three rules, each with one owner
//!
//! **Where the numbers are.** They are not a sequence this module picks: each tick is the world coordinate
//! the camera puts under that pixel, so a document whose origin sits at the centre of the stock reads
//! negative to the left of and below it — which is what the reference frame shows and what an operator
//! checking a zero offset needs to see.
//!
//! **How far apart they are.** [`cam_view::nice_spacing`] with the grid's own target
//! ([`cam_view::TARGET_GRID_PIXELS`]): one "nice number" rule for the whole view, so a ruler tick lands on a
//! grid line instead of teaching a second set of round numbers.
//!
//! **Which amber.** [`Role::Cut`], because the marker is the position the tool would cut at — the same thing
//! the cut signal means everywhere else in the frame — and not [`Role::Warn`], which is a verdict about a
//! program. Both are amber by accident; the meaning is what chooses.

use cam_app::{Camera, ScreenPoint};
use cam_geom::Point2;
use egui::{Color32, Stroke};

use crate::theme::{Metrics, Palette, Role};
use crate::CanvasRect;

/// The unit the rulers are in, named on the top band.
///
/// One word, spelled where the tick numbers are formatted, so the band cannot claim a unit the numbers are
/// not in.
pub const UNIT: &str = "mm";

/// One number a ruler painted.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RulerTick {
  /// The world coordinate the tick names, in millimetres.
  pub value: f64,
  /// The text painted at the tick: [`Self::value`] formatted for the spacing it sits in.
  pub text: String,
  /// Where the tick mark was drawn, in window pixels.
  ///
  /// The mark, not the number: its coordinate is the one the number describes, so a test can map it back
  /// through the camera and check the two agree.
  pub at: (f32, f32),
}

/// The amber marker both bands carry while the pointer is over the drawing area.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RulerMarker {
  /// The world position it tracks, in millimetres: the pointer's, after snapping.
  pub world: (f64, f64),
  /// The colour it is drawn with, so a test can prove it is the palette's cut signal and not a literal.
  pub colour: Color32,
  /// The leg it drew across the top band: its two ends, in window pixels.
  pub top: ((f32, f32), (f32, f32)),
  /// The leg it drew across the left band.
  pub left: ((f32, f32), (f32, f32)),
}

/// What the two rulers drew this frame.
///
/// Empty (all defaults) while the frame drew no canvas at all — the 3D view owns the central area instead —
/// because a ruler nobody drew is not an empty ruler.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rulers {
  /// The top band, as `(x, y, width, height)` in window pixels.
  pub top_band: (f32, f32, f32, f32),
  /// The left band, below the top one, in the same shape.
  pub left_band: (f32, f32, f32, f32),
  /// The colour both bands are filled with.
  pub band: Color32,
  /// The colour the ticks and the numbers are drawn in.
  pub ink: Color32,
  /// The unit word the top band painted.
  pub unit: String,
  /// Where the unit word was drawn, in window pixels.
  pub unit_at: (f32, f32),
  /// The step between two ticks, in millimetres, as [`cam_view::nice_spacing`] gave it.
  pub spacing: f64,
  /// The numbers the top band drew, left to right.
  pub top: Vec<RulerTick>,
  /// The numbers the left band drew, top to bottom.
  pub left: Vec<RulerTick>,
  /// The cursor marker, while the pointer was over the drawing area.
  pub marker: Option<RulerMarker>,
}

/// Beyond this many ticks the band stops subdividing, for the reason [`cam_view::MAX_GRID_LINES`] exists: a
/// deep zoom must not be able to hand the painter an unbounded list.
pub(crate) const MAX_TICKS: usize = 200;

/// The readable text for one tick, given the step it sits in.
///
/// The decimals follow the step and nothing else, so a `1`/`2`/`5`-style step never prints
/// `-0.30000000000000004`, and a step of `0.5` never prints `0` for a tick that is not on zero.
pub(crate) fn tick_text(value: f64, spacing: f64) -> String {
  // A tick one rounding step on the wrong side of zero is still zero: printing `-0` would say the origin is
  // somewhere it is not.
  let value = if value == 0.0 { 0.0 } else { value };
  let decimals = if spacing.is_finite() && spacing > 0.0 && spacing < 1.0 {
    (-spacing.log10().floor()).max(0.0) as usize
  } else {
    0
  };
  format!("{value:.decimals$}")
}

/// Every multiple of `spacing` inside `min..=max`, in ascending order.
///
/// Multiples of the step rather than `min + k * spacing`, so the numbers are the round ones the step
/// promises: a tick at `20` and not at `19.999999999999996`.
pub(crate) fn ticks(min: f64, max: f64, spacing: f64) -> Vec<f64> {
  if !spacing.is_finite() || spacing <= 0.0 || !min.is_finite() || !max.is_finite() || max < min {
    return Vec::new();
  }
  let first = (min / spacing).ceil();
  let last = (max / spacing).floor();
  if !first.is_finite() || !last.is_finite() || last < first {
    return Vec::new();
  }
  let mut out = Vec::new();
  let mut index = first;
  while index <= last && out.len() < MAX_TICKS {
    out.push(index * spacing);
    index += 1.0;
  }
  out
}

/// The font the tick numbers and the unit word are drawn in.
///
/// Monospace, so `-80` and `80` are the same width and two numbers never jostle their neighbours as the view
/// scrolls.
fn font() -> egui::FontId {
  egui::FontId::monospace(crate::theme::CAPTION)
}

/// The three roles these bands paint with, in one place.
///
/// The band is chrome ([`Role::Panel`]), the ink is the dim text every secondary number in the frame is
/// written in ([`Role::TextDim`]), and the marker is the cut signal ([`Role::Cut`]). No literal colour is
/// named here, which is what lets the light set be a re-resolution of the same design.
fn colours(palette: &Palette) -> (Color32, Color32, Color32) {
  (
    palette.get(Role::Panel),
    palette.get(Role::TextDim),
    palette.get(Role::Cut),
  )
}

/// Paint the two bands across `area` and report what they drew.
///
/// `canvas` is the drawing area the caller has already inset by [`Metrics::ruler_thickness`], and `cursor` is
/// the world position the pointer is on — `None` while it is anywhere else, which is the whole of "the marker
/// is drawn only while the pointer is over the drawing area".
///
/// The world ranges come from the camera and not from the canvas rectangle, because the numbers have to be
/// the coordinates the camera puts under each pixel: that is what makes an origin at the centre of the stock
/// read negative to the left of and below it.
pub fn paint(
  painter: &egui::Painter,
  area: egui::Rect,
  canvas: &CanvasRect,
  camera: &Camera,
  palette: &Palette,
  metrics: &Metrics,
  cursor: Option<Point2>,
) -> Rulers {
  let (band_colour, ink, marker_colour) = colours(palette);
  let thickness = metrics.ruler_thickness;
  // The top band spans the whole width, the left one starts beneath it: the corner belongs to the unit word.
  let top = egui::Rect::from_min_max(area.min, egui::pos2(area.max.x, area.min.y + thickness));
  let left = egui::Rect::from_min_max(
    egui::pos2(area.min.x, area.min.y + thickness),
    egui::pos2(area.min.x + thickness, area.max.y),
  );
  let mut rulers = Rulers {
    top_band: (top.min.x, top.min.y, top.width(), top.height()),
    left_band: (left.min.x, left.min.y, left.width(), left.height()),
    band: band_colour,
    ink,
    unit: UNIT.to_owned(),
    unit_at: (top.min.x, top.min.y),
    spacing: cam_view::nice_spacing(camera.pixels_per_mm(), cam_view::TARGET_GRID_PIXELS),
    ..Rulers::default()
  };
  if !top.is_positive() || !left.is_positive() {
    return rulers;
  }

  painter.rect_filled(top, 0.0, band_colour);
  painter.rect_filled(left, 0.0, band_colour);
  // One hairline each, where the chrome ends and the drawing begins.
  let division = Stroke::new(metrics.hairline, palette.get(Role::Hairline));
  painter.line_segment([top.left_bottom(), top.right_bottom()], division);
  painter.line_segment([left.right_top(), left.right_bottom()], division);

  let ink_stroke = Stroke::new(1.0, ink);
  let tick_length = metrics.space(1);
  let padding = metrics.space(0);
  let font = font();
  let top_painter = painter.with_clip_rect(top);
  let left_painter = painter.with_clip_rect(left);

  // The unit word owns the band's left end, so the first number that would run into it is dropped rather
  // than printed over it.
  let unit_rect = top_painter.text(
    egui::pos2(top.min.x + padding, top.min.y + padding),
    egui::Align2::LEFT_TOP,
    UNIT,
    font.clone(),
    ink,
  );
  rulers.unit_at = (unit_rect.min.x, unit_rect.min.y);

  let viewport = camera.viewport();
  let first = camera.screen_to_world(ScreenPoint::new(0.0, 0.0));
  let last = camera.screen_to_world(ScreenPoint::new(viewport.width, viewport.height));
  let (min_x, max_x) = (first.x.min(last.x), first.x.max(last.x));
  let (min_y, max_y) = (first.y.min(last.y), first.y.max(last.y));

  for value in ticks(min_x, max_x, rulers.spacing) {
    let x = (canvas.x + camera.world_to_screen(Point2::new(value, 0.0)).x) as f32;
    if !top.x_range().contains(x) {
      continue;
    }
    let text = tick_text(value, rulers.spacing);
    let galley = top_painter.layout_no_wrap(text.clone(), font.clone(), ink);
    if x - galley.size().x / 2.0 < unit_rect.max.x + padding {
      continue;
    }
    top_painter.line_segment(
      [
        egui::pos2(x, top.max.y),
        egui::pos2(x, top.max.y - tick_length),
      ],
      ink_stroke,
    );
    let label = egui::pos2(x - galley.size().x / 2.0, top.min.y + padding);
    top_painter.galley(label, galley, ink);
    rulers.top.push(RulerTick {
      value,
      text,
      at: (x, top.max.y - tick_length),
    });
  }

  // Down the left band, top to bottom: the world's `y` grows upward, so this reads the ticks in reverse.
  for value in ticks(min_y, max_y, rulers.spacing).into_iter().rev() {
    let y = (canvas.y + camera.world_to_screen(Point2::new(0.0, value)).y) as f32;
    if !left.y_range().contains(y) {
      continue;
    }
    left_painter.line_segment(
      [
        egui::pos2(left.max.x, y),
        egui::pos2(left.max.x - tick_length, y),
      ],
      ink_stroke,
    );
    let text = tick_text(value, rulers.spacing);
    let galley = left_painter.layout_no_wrap(text.clone(), font.clone(), ink);
    // Rotated a quarter turn clockwise about its centre, so the number reads bottom to top: a band 22 px
    // wide cannot hold `-1000` written the way the top band writes it.
    let centre = egui::pos2((left.min.x + left.max.x - tick_length) / 2.0, y);
    left_painter.add(
      egui::epaint::TextShape::new(centre, galley, ink)
        .with_angle_and_anchor(-std::f32::consts::FRAC_PI_2, egui::Align2::CENTER_CENTER),
    );
    rulers.left.push(RulerTick {
      value,
      text,
      at: (left.max.x - tick_length, y),
    });
  }

  if let Some(world) = cursor {
    let screen = camera.world_to_screen(world);
    let x = (canvas.x + screen.x) as f32;
    let y = (canvas.y + screen.y) as f32;
    let stroke = Stroke::new(1.0, marker_colour);
    top_painter.line_segment([egui::pos2(x, top.min.y), egui::pos2(x, top.max.y)], stroke);
    left_painter.line_segment(
      [egui::pos2(left.min.x, y), egui::pos2(left.max.x, y)],
      stroke,
    );
    rulers.marker = Some(RulerMarker {
      world: (world.x, world.y),
      colour: marker_colour,
      top: ((x, top.min.y), (x, top.max.y)),
      left: ((left.min.x, y), (left.max.x, y)),
    });
  }
  rulers
}

#[cfg(test)]
mod tests {
  use super::{tick_text, ticks, MAX_TICKS};

  #[test]
  fn a_tick_names_a_round_multiple_of_the_step() {
    // The reason `ticks` multiplies an index instead of accumulating: a run of additions drifts, and a
    // ruler that says -19.999999 would be wrong about the one thing it is for.
    assert_eq!(ticks(-29.0, 29.0, 20.0), vec![-20.0, 0.0, 20.0]);
    assert_eq!(ticks(0.3, 0.9, 0.5), vec![0.5]);
    assert!(
      ticks(0.3, 0.4, 0.5).is_empty(),
      "no multiple of the step is inside"
    );
    assert!(
      ticks(0.0, 1.0, 0.0).is_empty(),
      "a step of zero is not a step"
    );
    assert!(ticks(0.0, 1.0, f64::NAN).is_empty());
  }

  #[test]
  fn the_band_stops_subdividing_at_its_own_limit() {
    let all = ticks(0.0, 1.0e6, 0.001);
    assert_eq!(all.len(), MAX_TICKS, "a deep zoom must not be unbounded");
    assert_eq!(all.first().copied(), Some(0.0));
  }

  #[test]
  fn a_tick_prints_the_decimals_its_step_promises() {
    assert_eq!(tick_text(-1000.0, 200.0), "-1000");
    assert_eq!(tick_text(20.0, 20.0), "20");
    assert_eq!(tick_text(0.0, 0.5), "0.0");
    // A tick that arithmetic put a rounding step the wrong side of zero is still zero.
    assert_eq!(tick_text(-0.0, 1.0), "0");
    assert_eq!(tick_text(-0.30000000000000004, 0.1), "-0.3");
    assert_eq!(tick_text(0.05, 0.05), "0.05");
  }
}
