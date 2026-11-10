//! The preview column: one tool drawn vertically inside a dark card, with its two dimensions.
//!
//! The drawing is the same idea the section had before it moved here — the tool's own numbers and nothing
//! else — turned on its side, because that is how the reference draws it: length runs down the card and the
//! diameter is measured across the tip. The **horizontal scale is still shared by every tool at once**
//! ([`SPAN_MM`] of tool is the card's inner width), so two tools drawn in it are comparable by eye, and a
//! tool wider than the span gets the card's width with the callout saying how wide it really is: a picture
//! that overflowed its frame would not be a picture.
//!
//! Two numbers are drawn, and only when the record holds them: the overall length down the left side and
//! the diameter across the tip. A tool with no overall length gets no length line, because a dimension the
//! library does not have is a dimension the shell invented.

use cam_store::tools::{ToolGeometry, ToolKind};
use cam_units::Format;

use crate::theme::{Metrics, Palette, Role};

use super::family::{ShapeField, ToolFamily};
use super::form::decimal;

/// The width of tool, in millimetres, the card's inner width represents.
const SPAN_MM: f32 = 40.0;

/// The preview column's width: the card, plus the padding that keeps the caption off its edge.
pub(super) const PREVIEW_WIDTH: f32 = 200.0;

/// The card's height: the drawing, its `Ø` dimension line and the room the label under it needs.
pub(super) const PREVIEW_HEIGHT: f32 = 330.0;

/// Draw one tool inside its card and answer the width its cutting diameter was drawn at.
///
/// The caller owns the caption under the card; this owns everything inside it.
pub(super) fn paint_preview(
  painter: &egui::Painter,
  rect: egui::Rect,
  palette: &Palette,
  metrics: &Metrics,
  geometry: &ToolGeometry,
  kind: ToolKind,
  callouts: &mut Vec<String>,
) -> f32 {
  let radius = egui::CornerRadius::same(6);
  let card_border = egui::Color32::from_rgb(0xb9, 0xbc, 0xc0);
  let tool_outline = egui::Color32::from_rgb(0x43, 0x47, 0x4e);
  painter.rect_filled(rect, radius, palette.get(Role::PreviewCard));
  painter.rect_stroke(
    rect,
    radius,
    egui::Stroke::new(metrics.hairline, card_border),
    egui::StrokeKind::Inside,
  );

  let Some(diameter) = geometry.diameter else {
    return 0.0;
  };
  let diameter = diameter.mm();
  if !diameter.is_finite() || diameter <= 0.0 {
    return 0.0;
  }

  // The room the two callouts need: the length line down the left, the diameter line under the tip.
  let gutter = 30.0;
  let foot = 26.0;
  let inner = egui::Rect::from_min_max(
    egui::pos2(rect.left() + gutter, rect.top() + metrics.space(2)),
    egui::pos2(rect.right() - metrics.space(2), rect.bottom() - foot),
  );
  let span = SPAN_MM.max(diameter as f32);
  let width = (diameter as f32 * inner.width() / span).max(1.0);
  let half = width / 2.0;
  let centre = inner.center().x;

  let family = ToolFamily::of(kind);
  // How far up the tool the cutting end reaches: the flutes when the record holds a length for them, and a
  // share of the card when it does not. A guessed length is not drawn as a dimension — only as a shape.
  let shaft_share = match (geometry.flute_length, geometry.overall_length) {
    (Some(flutes), Some(overall)) if overall.mm() > 0.0 => {
      (flutes.mm() / overall.mm()).clamp(0.05, 1.0) as f32
    }
    _ => 0.45,
  };
  let shoulder = inner.bottom() - inner.height() * shaft_share;

  let body = palette.get(Role::PreviewTool);
  let outline = egui::Stroke::new(1.0, tool_outline);
  // The shank: filled, because the reference's card is a picture of a tool and a tool has a body.
  painter.rect_filled(
    egui::Rect::from_min_max(
      egui::pos2(centre - half, inner.top()),
      egui::pos2(centre + half, shoulder),
    ),
    0.0,
    body,
  );
  painter.line_segment(
    [
      egui::pos2(centre - half, inner.top()),
      egui::pos2(centre - half, shoulder),
    ],
    outline,
  );
  painter.line_segment(
    [
      egui::pos2(centre + half, inner.top()),
      egui::pos2(centre + half, shoulder),
    ],
    outline,
  );

  // The cutting end: the same tone as the shank with its flutes hatched across it, which is what makes the
  // two halves of the tool legible at 20 pixels wide.
  let cutting = egui::Rect::from_min_max(
    egui::pos2(centre - half, shoulder),
    egui::pos2(centre + half, inner.bottom()),
  );
  painter.rect_filled(cutting, 0.0, body);
  let hatch = egui::Stroke::new(0.8, tool_outline);
  let mut offset = 3.0;
  while offset < cutting.height() {
    let y = shoulder + offset;
    painter.line_segment(
      [egui::pos2(centre - half, y), egui::pos2(centre + half, y)],
      hatch,
    );
    offset += 4.0;
  }
  let ink = egui::Stroke::new(1.6, tool_outline);
  painter.line_segment(
    [
      egui::pos2(centre - half, shoulder),
      egui::pos2(centre - half, inner.bottom()),
    ],
    ink,
  );
  painter.line_segment(
    [
      egui::pos2(centre + half, shoulder),
      egui::pos2(centre + half, inner.bottom()),
    ],
    ink,
  );
  paint_tip(
    painter,
    geometry,
    family,
    centre,
    half,
    shoulder,
    inner.bottom(),
    ink,
  );

  let dim = palette.get(Role::PreviewDimension);
  // The length, down the left side, when the record holds one.
  if let Some(overall) = geometry.overall_length {
    let label = decimal(overall);
    preview_dimension_line(
      painter,
      dim,
      egui::pos2(rect.left() + 16.0, inner.top()),
      egui::pos2(rect.left() + 16.0, inner.bottom()),
      &label,
      egui::vec2(-2.0, 0.0),
      egui::Align2::RIGHT_CENTER,
    );
    callouts.push(format!("Overall length {label} mm"));
  }

  // The diameter, across the tip, which every tool has.
  let line_y = inner.bottom() + metrics.space(2);
  let label = format!("Ø{}", decimal(cam_units::Length::from_mm(diameter)));
  preview_dimension_line(
    painter,
    dim,
    egui::pos2(centre - half, line_y),
    egui::pos2(centre + half, line_y),
    &label,
    egui::vec2(0.0, 2.0),
    egui::Align2::CENTER_TOP,
  );
  callouts.push(format!("Diameter {label} mm"));

  // The kind's own number, when it has one worth naming, written under the card by the caller.
  if let Some(angle) = ShapeField::IncludedAngle.value(geometry) {
    if kind.has_included_angle() {
      callouts.push(format!("Included angle {angle}°"));
    }
  }
  if let Some(flutes) = geometry.flute_length {
    callouts.push(format!(
      "Flute length {} mm",
      Format::fixed(2).apply(flutes.mm())
    ));
  }

  width
}

/// One dimension line on the preview card: its two end ticks, the line between them, and its number.
fn preview_dimension_line(
  painter: &egui::Painter,
  colour: egui::Color32,
  from: egui::Pos2,
  to: egui::Pos2,
  label: &str,
  label_offset: egui::Vec2,
  align: egui::Align2,
) {
  let stroke = egui::Stroke::new(1.0, colour);
  painter.line_segment([from, to], stroke);
  let tick = (to - from).normalized().rot90() * 3.5;
  painter.line_segment([from - tick, from + tick], stroke);
  painter.line_segment([to - tick, to + tick], stroke);
  let mid = from + (to - from) / 2.0 + label_offset;
  painter.text(
    mid,
    align,
    label,
    egui::FontId::monospace(crate::theme::CAPTION),
    colour,
  );
}

/// The tip profile, on the bottom edge of the cutting portion.
#[allow(clippy::too_many_arguments)]
fn paint_tip(
  painter: &egui::Painter,
  geometry: &ToolGeometry,
  family: ToolFamily,
  centre: f32,
  half: f32,
  shoulder: f32,
  bottom: f32,
  ink: egui::Stroke,
) {
  match family {
    ToolFamily::Flat => {
      painter.line_segment(
        [
          egui::pos2(centre - half, bottom),
          egui::pos2(centre + half, bottom),
        ],
        ink,
      );
    }
    ToolFamily::BallNose => {
      let radius = half;
      let centre_y = bottom - radius;
      // Eleven points on the arc: egui has no arc primitive, and a circle would bulge the wrong way.
      let points: Vec<egui::Pos2> = (0..=10)
        .map(|step| {
          let angle = std::f32::consts::PI * (step as f32 / 10.0);
          egui::pos2(
            centre - radius * angle.cos(),
            centre_y + radius * angle.sin(),
          )
        })
        .collect();
      painter.add(egui::Shape::line(points, ink));
    }
    ToolFamily::CornerRadius => {
      let radius = geometry
        .tip_radius
        .map(|radius| radius.mm() as f32)
        .unwrap_or(0.0)
        .min(half);
      let points = vec![
        egui::pos2(centre - half, bottom - radius),
        egui::pos2(centre - half + radius, bottom),
        egui::pos2(centre + half - radius, bottom),
        egui::pos2(centre + half, bottom - radius),
      ];
      painter.add(egui::Shape::line(points, ink));
    }
    ToolFamily::VBit => {
      let flat = geometry
        .flat_diameter
        .map(|flat| flat.mm() as f32 / 2.0)
        .unwrap_or(0.0)
        .min(half);
      let rise = (half - flat).max(1.0);
      let tip_top = (bottom - rise * 2.0).max(shoulder);
      let points = vec![
        egui::pos2(centre - half, tip_top),
        egui::pos2(centre - flat, bottom),
        egui::pos2(centre + flat, bottom),
        egui::pos2(centre + half, tip_top),
      ];
      painter.add(egui::Shape::line(points, ink));
    }
  }
}
