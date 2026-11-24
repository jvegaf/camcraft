//! The preview column: the tool drawn as the reference draws it, with its dimensions (S13).
//!
//! The card is light, the tool is a solid shank above a cutting section above an amber tip, and the dimensions
//! are read from the record: a number the library does not hold is not drawn.
//!
//! Two rules shape the drawing, and the second is why this module reports geometry as well as words:
//!
//! 1. **A V-bit and a drill are drawn as their cone**, because the included angle is what makes them different
//!    from every other kind. The section below the shank tapers from the shank's width to the tip's over
//!    `half_width / tan(angle / 2)`, so the shape on screen is the angle the form states.
//! 2. **A dimension's label is offset from its own line**, perpendicular to it, by eight pixels plus half the
//!    label's own box. The first delivery laid the `Ø` labels along their arrows, which reads as a strikethrough
//!    rather than as a measurement; the offset is computed from the laid-out galley rather than guessed, and the
//!    rectangles are reported so a test can assert that no label touches its line, the tool, or another label.

use cam_store::tools::{ToolGeometry, ToolKind};
use egui::{Color32, FontId, Pos2, Rect, Stroke, Vec2};

use crate::theme::{Metrics, Palette, Role};

use super::form::decimal;

/// The preview column's width and the card's height.
pub(super) const PREVIEW_WIDTH: f32 = 200.0;
pub(super) const PREVIEW_HEIGHT: f32 = 330.0;

/// How far a dimension's label sits from its own line, before half the label's own box is added.
const LABEL_OFFSET: f32 = 8.0;

/// One dimension the card drew: its words, the line it measures and the box its label occupies.
#[derive(Clone, Debug)]
pub(super) struct Dimension {
  /// The number as painted: `38`, `Ø1`, `90°`.
  pub label: String,
  /// The line between its two arrowheads.
  pub line: Rect,
  /// The box the label was laid out in.
  pub label_rect: Rect,
}

/// One tool drawn in its card, reported for the tests that pin S13's geometry.
#[derive(Clone, Debug, Default)]
pub(super) struct Drawing {
  /// The cutting section's width, in pixels: what the diameter was drawn at.
  pub cutting_width: f32,
  /// The shank, the cutting section and the amber tip.
  pub body: Vec<Rect>,
  /// A cone's `(top width, tip width, height)` in pixels, when the kind has one.
  pub cone: Option<(f32, f32, f32)>,
  /// Every dimension, in the order it was drawn.
  pub dimensions: Vec<Dimension>,
}

/// Draw one tool inside its card and report what was drawn.
pub(super) fn paint_preview(
  painter: &egui::Painter,
  rect: Rect,
  palette: &Palette,
  metrics: &Metrics,
  geometry: &ToolGeometry,
  kind: ToolKind,
  callouts: &mut Vec<String>,
) -> Drawing {
  let card_radius = egui::CornerRadius::same(6);
  painter.rect_filled(rect, card_radius, palette.get(Role::PreviewCard));
  painter.rect_stroke(
    rect,
    card_radius,
    Stroke::new(metrics.hairline, Color32::from_rgb(0xb9, 0xbc, 0xc0)),
    egui::StrokeKind::Inside,
  );

  let Some(diameter_len) = geometry.diameter else {
    return Drawing::default();
  };
  let diameter_mm = diameter_len.mm() as f32;
  if !diameter_mm.is_finite() || diameter_mm <= 0.0 {
    return Drawing::default();
  }
  let shank_mm = geometry
    .shank_diameter
    .map(|d| d.mm() as f32)
    .filter(|d| d.is_finite() && *d > 0.0)
    .unwrap_or(diameter_mm);
  let overall_mm = geometry
    .overall_length
    .map(|d| d.mm() as f32)
    .filter(|d| d.is_finite() && *d > 0.0)
    .unwrap_or(diameter_mm * 2.5);
  let flute_mm = geometry
    .flute_length
    .map(|d| d.mm() as f32)
    .filter(|d| d.is_finite() && *d > 0.0);

  // 16 px margins inside the card, gutters for the vertical dimensions, and a strip under the tip for the
  // cutting diameter's own line and label.
  let inner = rect.shrink(16.0);
  let left_gutter = 34.0;
  let right_gutter = 34.0;
  let bottom_strip = 34.0;
  let top_strip = 26.0;
  let draw = Rect::from_min_max(
    Pos2::new(inner.left() + left_gutter, inner.top() + top_strip),
    Pos2::new(inner.right() - right_gutter, inner.bottom() - bottom_strip),
  );
  if draw.width() <= 0.0 || draw.height() <= 0.0 {
    return Drawing::default();
  }

  let shank_w = 0.16 * inner.width();
  let cut_w = (shank_w * (diameter_mm / shank_mm)).max(6.0).min(shank_w);

  let angle = geometry
    .included_angle
    .map(|angle| angle.degrees() as f32)
    .filter(|degrees| degrees.is_finite() && *degrees > 0.0 && *degrees < 180.0);
  let cone_kind = matches!(kind, ToolKind::VBit | ToolKind::Drill);
  let tip_w = geometry
    .flat_diameter
    .map(|flat| (flat.mm() as f32 / shank_mm * shank_w).max(0.0))
    .filter(|width| width.is_finite())
    .unwrap_or(0.0)
    .min(shank_w);

  // A cone's height is its angle's business, not the card's: the taper drawn is the taper the form states.
  let cone_height = match (cone_kind, angle) {
    (true, Some(degrees)) => {
      let half = (degrees.to_radians() / 2.0).max(0.01);
      Some((((shank_w - tip_w) / 2.0) / half.tan()).max(8.0))
    }
    _ => None,
  };

  let cutting_h_mm = flute_mm.unwrap_or(overall_mm * 0.30);
  let shank_h_mm = (overall_mm - cutting_h_mm).max(overall_mm * 0.15);
  let total_mm = (shank_h_mm + cutting_h_mm).max(0.001);
  let scale = draw.height() / total_mm;

  let (mut shank_h, mut cutting_h) = match cone_height {
    Some(height) => (
      (draw.height() - height).max(draw.height() * 0.35),
      height.min(draw.height() * 0.6),
    ),
    None => (shank_h_mm * scale, (cutting_h_mm * scale).max(8.0)),
  };
  let mut total_h = shank_h + cutting_h;
  if total_h > draw.height() {
    let fit = draw.height() / total_h;
    shank_h *= fit;
    cutting_h *= fit;
    total_h = draw.height();
  }
  let tip_h = 4.0_f32.min(cutting_h / 3.0);

  let cx = draw.center().x;
  let top = draw.top() + (draw.height() - total_h) / 2.0;
  let shank_bottom = top + shank_h;
  let cutting_bottom = shank_bottom + cutting_h;
  let tip_top = cutting_bottom - tip_h;

  let tool_fill = palette.get(Role::PreviewTool);
  let outline_colour = Color32::from_rgb(0x43, 0x47, 0x4e);
  let outline = Stroke::new(1.0, outline_colour);
  let mut drawing = Drawing {
    cutting_width: cut_w,
    ..Drawing::default()
  };

  let shank_rect = Rect::from_min_max(
    Pos2::new(cx - shank_w / 2.0, top),
    Pos2::new(cx + shank_w / 2.0, shank_bottom),
  );
  painter.rect_filled(shank_rect, 0.0, tool_fill);
  painter.rect_stroke(shank_rect, 0.0, outline, egui::StrokeKind::Inside);
  drawing.body.push(shank_rect);

  // The cutting section: a trapezoid when the kind's tip is a cone, a rectangle when it is not.
  let cut_top_w = if cone_height.is_some() {
    shank_w
  } else {
    cut_w
  };
  let cut_bottom_w = if cone_height.is_some() { tip_w } else { cut_w };
  if cone_height.is_some() {
    let points = vec![
      Pos2::new(cx - cut_top_w / 2.0, shank_bottom),
      Pos2::new(cx + cut_top_w / 2.0, shank_bottom),
      Pos2::new(cx + cut_bottom_w / 2.0, cutting_bottom),
      Pos2::new(cx - cut_bottom_w / 2.0, cutting_bottom),
    ];
    painter.add(egui::Shape::convex_polygon(points, tool_fill, outline));
    drawing.body.push(Rect::from_min_max(
      Pos2::new(cx - cut_top_w / 2.0, shank_bottom),
      Pos2::new(cx + cut_top_w / 2.0, cutting_bottom),
    ));
    drawing.cone = Some((cut_top_w, cut_bottom_w, cutting_h));
  } else {
    let cut_rect = Rect::from_min_max(
      Pos2::new(cx - cut_w / 2.0, shank_bottom),
      Pos2::new(cx + cut_w / 2.0, cutting_bottom),
    );
    painter.rect_filled(cut_rect, 0.0, tool_fill);
    painter.rect_stroke(cut_rect, 0.0, outline, egui::StrokeKind::Inside);
    drawing.body.push(cut_rect);
  }

  // The amber tip is the edge that cuts: the flat of a cone, the end of a square cutter.
  let tip_width = cut_bottom_w.max(2.0);
  let tip_rect = Rect::from_min_max(
    Pos2::new(cx - tip_width / 2.0, tip_top),
    Pos2::new(cx + tip_width / 2.0, cutting_bottom),
  );
  painter.rect_filled(tip_rect, 0.0, palette.get(Role::Cut));
  drawing.body.push(tip_rect);

  let dim = palette.get(Role::PreviewDimension);
  let widest = shank_w.max(cut_top_w);

  if let Some(overall) = geometry.overall_length {
    let label = decimal(overall);
    let x = cx - widest / 2.0 - 12.0;
    draw_vertical(
      painter,
      Pos2::new(x, top),
      Pos2::new(x, cutting_bottom),
      dim,
      &label,
      Side::Left,
      &mut drawing.dimensions,
    );
    callouts.push(format!("Overall length {label} mm"));
  }
  if let Some(flute) = geometry.flute_length {
    let label = decimal(flute);
    let x = cx + widest / 2.0 + 12.0;
    draw_vertical(
      painter,
      Pos2::new(x, shank_bottom),
      Pos2::new(x, cutting_bottom),
      dim,
      &label,
      Side::Right,
      &mut drawing.dimensions,
    );
    callouts.push(format!("Flute length {label} mm"));
  }

  // Two diameters at most, one at each end of the drawing, and never the same number twice.
  //
  // A cone's widest part is its major diameter, which belongs at the top with its flat under the tip; every
  // other kind carries its shank's diameter at the top (when the record has one) and its cutting diameter under
  // the tip, which is where the reference puts them.
  if drawing.cone.is_some() {
    let label = format!("Ø{}", decimal(diameter_len));
    let y_top = (top - 12.0).max(inner.top() + 4.0);
    draw_horizontal(
      painter,
      Pos2::new(cx - widest / 2.0, y_top),
      Pos2::new(cx + widest / 2.0, y_top),
      dim,
      &label,
      Side::Above,
      &mut drawing.dimensions,
    );
    callouts.push(format!("Diameter {label} mm"));

    // The flat is a number even when it is sub-pixel on screen — a 0.2 mm flat on a 12.7 mm body is a third of
    // a pixel here, and the reference still writes it under the tip, because the dimension is the measurement
    // and not the width it happens to occupy.
    if let (Some((_, tip, _)), Some(flat)) = (drawing.cone, geometry.flat_diameter) {
      let label = decimal(flat);
      let y = cutting_bottom + 12.0;
      draw_horizontal(
        painter,
        Pos2::new(cx - tip.max(6.0) / 2.0, y),
        Pos2::new(cx + tip.max(6.0) / 2.0, y),
        dim,
        &label,
        Side::Below,
        &mut drawing.dimensions,
      );
      callouts.push(format!("Tip width {label} mm"));
    }
  } else {
    if let Some(shank) = geometry.shank_diameter {
      let label = format!("Ø{}", decimal(shank));
      let y_top = (top - 12.0).max(inner.top() + 4.0);
      draw_horizontal(
        painter,
        Pos2::new(cx - widest / 2.0, y_top),
        Pos2::new(cx + widest / 2.0, y_top),
        dim,
        &label,
        Side::Above,
        &mut drawing.dimensions,
      );
      callouts.push(format!("Shank diameter {label} mm"));
    }
    let label = format!("Ø{}", decimal(diameter_len));
    let y = cutting_bottom + 12.0;
    draw_horizontal(
      painter,
      Pos2::new(cx - cut_w.max(6.0) / 2.0, y),
      Pos2::new(cx + cut_w.max(6.0) / 2.0, y),
      dim,
      &label,
      Side::Below,
      &mut drawing.dimensions,
    );
    callouts.push(format!("Diameter {label} mm"));
  }

  // The angle, beside the cone: the one number that tells a V-bit from a drill from an end mill.
  if let (Some(degrees), Some(_)) = (angle, cone_height) {
    let label = if degrees.fract() == 0.0 {
      format!("{degrees:.0}°")
    } else {
      format!("{degrees:.1}°")
    };
    let at = Pos2::new(cx + widest / 2.0 + 4.0, shank_bottom + cutting_h / 2.0);
    let galley = painter.layout_no_wrap(
      label.clone(),
      FontId::proportional(crate::theme::CAPTION),
      dim,
    );
    let size = galley.size();
    let label_rect = Rect::from_min_size(
      Pos2::new(
        at.x.min(rect.right() - size.x - 4.0),
        (at.y - size.y / 2.0).max(rect.top() + 2.0),
      ),
      size,
    );
    painter.galley(label_rect.min, galley, dim);
    // An angle is not measured between two arrowheads; the label alone is the dimension, so its `line` is the
    // degenerate rect at its own anchor and the overlap rule is about the body and the other labels.
    drawing.dimensions.push(Dimension {
      label: label.clone(),
      line: Rect::from_min_size(at, Vec2::ZERO),
      label_rect,
    });
    callouts.push(label);
  }

  drawing
}

/// Which side of its line a label sits on.
#[derive(Clone, Copy, PartialEq)]
enum Side {
  Above,
  Below,
  Left,
  Right,
}

fn arrow_head(painter: &egui::Painter, tip: Pos2, dir: Vec2, colour: Color32) {
  let n = if dir.length_sq() > 0.0 {
    dir.normalized()
  } else {
    Vec2::X
  };
  let side = n.rot90() * 3.0;
  let back = -n * 6.0;
  let points = vec![tip, tip + back + side, tip + back - side];
  painter.add(egui::Shape::convex_polygon(points, colour, Stroke::NONE));
}

/// A horizontal dimension: line, two arrowheads, and its label offset by [`LABEL_OFFSET`] and half its box.
fn draw_horizontal(
  painter: &egui::Painter,
  from: Pos2,
  to: Pos2,
  colour: Color32,
  label: &str,
  side: Side,
  out: &mut Vec<Dimension>,
) {
  painter.line_segment([from, to], Stroke::new(1.0, colour));
  arrow_head(painter, from, Vec2::new(-1.0, 0.0), colour);
  arrow_head(painter, to, Vec2::new(1.0, 0.0), colour);
  let galley = painter.layout_no_wrap(
    label.to_owned(),
    FontId::proportional(crate::theme::CAPTION),
    colour,
  );
  let size = galley.size();
  let mid_x = (from.x + to.x) / 2.0;
  let top = match side {
    Side::Above => from.y - LABEL_OFFSET - size.y,
    _ => from.y + LABEL_OFFSET,
  };
  let label_rect = Rect::from_min_size(Pos2::new(mid_x - size.x / 2.0, top), size);
  painter.galley(label_rect.min, galley, colour);
  out.push(Dimension {
    label: label.to_owned(),
    line: Rect::from_two_pos(from, to),
    label_rect,
  });
}

/// A vertical dimension, with its label rotated a quarter turn and offset out of the line's way.
fn draw_vertical(
  painter: &egui::Painter,
  from: Pos2,
  to: Pos2,
  colour: Color32,
  label: &str,
  side: Side,
  out: &mut Vec<Dimension>,
) {
  painter.line_segment([from, to], Stroke::new(1.0, colour));
  arrow_head(painter, from, Vec2::new(0.0, -1.0), colour);
  arrow_head(painter, to, Vec2::new(0.0, 1.0), colour);

  let galley = painter.layout_no_wrap(
    label.to_owned(),
    FontId::proportional(crate::theme::CAPTION),
    colour,
  );
  let size = galley.size();
  // Rotated -90°, the galley's box is `size.y` wide and `size.x` tall, and the shape turns about `pos`, so the
  // box's centre is `pos + (size.y / 2, -size.x / 2)`.
  let mid_y = (from.y + to.y) / 2.0;
  let centre_x = match side {
    Side::Left => from.x - LABEL_OFFSET - size.y / 2.0,
    _ => from.x + LABEL_OFFSET + size.y / 2.0,
  };
  let label_rect = Rect::from_center_size(Pos2::new(centre_x, mid_y), Vec2::new(size.y, size.x));
  let pos = Pos2::new(centre_x - size.y / 2.0, mid_y + size.x / 2.0);
  painter.add(egui::epaint::TextShape {
    pos,
    galley,
    underline: Stroke::NONE,
    fallback_color: colour,
    override_text_color: Some(colour),
    opacity_factor: 1.0,
    angle: -std::f32::consts::FRAC_PI_2,
  });
  out.push(Dimension {
    label: label.to_owned(),
    line: Rect::from_two_pos(from, to),
    label_rect,
  });
}
