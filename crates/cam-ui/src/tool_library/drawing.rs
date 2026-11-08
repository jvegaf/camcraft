use cam_store::tools::ToolGeometry;
use cam_units::Format;

use crate::theme::{self, Metrics, Palette, Role};

use super::family::ToolFamily;

/// The width of tool, in millimetres, the drawing's box represents.
///
/// One number for every tool, which is what "to scale" means: a Ø6 tool is drawn at half the width of a Ø12
/// one in the same box. A tool wider than this gets a scale of its own (see [`paint_section`]) so it stays
/// inside the frame, and the callout is what says how wide it really is — the picture is never the only place
/// a number is written.
const SPAN_MM: f32 = 40.0;

/// How tall the drawing's box is: the section, its `Ø` dimension line, and the callouts under them.
pub(super) const DRAWING_HEIGHT: f32 = 186.0;

/// Draw one tool's cutting end to scale, and its dimension callouts.
///
/// The section is built from the tool's own numbers and nothing else: the width is its diameter, the tip is
/// the profile its family describes — a square end, a hemisphere, a rounded corner, or a cone ending in a
/// flat — and every callout points at a dimension the record holds. A number the library does not have is
/// not drawn at all, which is why a tool with no flute length gets no length callout.
///
/// The **horizontal scale is shared** ([`SPAN_MM`] across the box), so two tools drawn in it are comparable:
/// that is what "to scale" means here, and it is why the scale is not computed from the tool alone, which
/// would draw a Ø3 and a Ø12 tool the same width. A tool wider than the span gets the box's width and the
/// callout says how wide it really is, because a picture that overflowed its frame would be unreadable.
pub(super) fn paint_section(
  painter: &egui::Painter,
  rect: egui::Rect,
  palette: &Palette,
  metrics: &Metrics,
  geometry: &ToolGeometry,
  callouts: &mut Vec<String>,
) -> f32 {
  let Some(diameter) = geometry.diameter else {
    return 0.0;
  };
  let diameter = diameter.mm();
  if !diameter.is_finite() || diameter <= 0.0 {
    return 0.0;
  }
  let span = SPAN_MM.max(diameter as f32);
  let scale = rect.width() / span;
  let width = (diameter as f32 * scale).max(1.0);
  let half = width / 2.0;
  let family = ToolFamily::of(geometry);

  let centre = rect.center().x;
  // The shape's own band: the diameter line and the callouts live under it, so the section is never crossed
  // by a number that makes it unreadable — the lesson the material plan's first screenshot taught.
  let top = rect.top() + metrics.space(2);
  let bottom = rect.top() + DRAWING_HEIGHT * 0.62;
  let tip_height = (tip_height_mm(geometry) as f32 * scale).min(bottom - top - 1.0);
  let shoulder = bottom - tip_height;

  let ink = egui::Stroke::new(1.6, palette.get(Role::TextHi));
  let flank = |painter: &egui::Painter, y: f32| {
    painter.line_segment(
      [egui::pos2(centre - half, top), egui::pos2(centre - half, y)],
      ink,
    );
    painter.line_segment(
      [egui::pos2(centre + half, top), egui::pos2(centre + half, y)],
      ink,
    );
  };
  // The shank's end is the frame's own edge: the library holds no overall length, so the top edge is where the
  // box ends and no callout points at it (`view3d::paint_tool`'s rule about the spindle).
  painter.line_segment(
    [
      egui::pos2(centre - half, top),
      egui::pos2(centre + half, top),
    ],
    ink,
  );

  match family {
    ToolFamily::Flat => {
      flank(painter, bottom);
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
      flank(painter, centre_y);
      // The hemisphere, as eleven points on the arc: egui has no arc primitive and a circle would bulge the
      // wrong way.
      let points: Vec<egui::Pos2> = (0..=10)
        .map(|step| {
          let angle = std::f32::consts::PI * (1.0 - step as f32 / 10.0);
          egui::pos2(
            centre + radius * angle.cos(),
            centre_y + radius * angle.sin(),
          )
        })
        .collect();
      painter.add(egui::Shape::line(points, ink));
    }
    ToolFamily::CornerRadius => {
      let radius = (geometry.tip_radius.map(|r| r.mm()).unwrap_or(0.0) as f32 * scale)
        .min(half)
        .min(bottom - top);
      flank(painter, bottom - radius);
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
        .map(|flat| (flat.mm() as f32 * scale) / 2.0)
        .unwrap_or(0.0)
        .min(half);
      flank(painter, shoulder);
      let points = vec![
        egui::pos2(centre - half, shoulder),
        egui::pos2(centre - flat, bottom),
        egui::pos2(centre + flat, bottom),
        egui::pos2(centre + half, shoulder),
      ];
      painter.add(egui::Shape::line(points, ink));
    }
  }

  // The width's dimension line, under the section: the one number every family has.
  let line_y = bottom + metrics.space(3);
  let width_label = format!("Ø {} mm", Format::fixed(2).apply(diameter));
  crate::chrome::dimension_line(
    painter,
    palette,
    egui::pos2(centre - half, line_y),
    egui::pos2(centre + half, line_y),
    &width_label,
    egui::vec2(0.0, 2.0),
    egui::Align2::CENTER_TOP,
  );
  callouts.push(width_label);

  // The family's own dimension, when the record holds the number that describes it.
  let family_callout = match family {
    ToolFamily::Flat => None,
    ToolFamily::BallNose => geometry
      .tip_radius
      .map(|radius| format!("Ball radius R {} mm", Format::fixed(2).apply(radius.mm()))),
    ToolFamily::CornerRadius => geometry
      .tip_radius
      .map(|radius| format!("Corner radius R {} mm", Format::fixed(2).apply(radius.mm()))),
    ToolFamily::VBit => match (geometry.included_angle, geometry.flat_diameter) {
      (Some(angle), flat) => Some(format!(
        "Included angle {}° · flat {} mm",
        Format::fixed(1).apply(angle.degrees()),
        match flat {
          Some(flat) => Format::fixed(2).apply(flat.mm()),
          None => "—".to_owned(),
        }
      )),
      (None, Some(flat)) => Some(format!("Flat {} mm", Format::fixed(2).apply(flat.mm()))),
      (None, None) => None,
    },
  };
  if let Some(callout) = family_callout {
    callouts.push(callout);
  }
  if let Some(length) = geometry.flute_length {
    callouts.push(format!(
      "Flute length {} mm",
      Format::fixed(2).apply(length.mm())
    ));
  }

  // The legend: one line per callout that is not the width, which the dimension line already carries. It
  // starts a whole step under that line so the `Ø` number and the first legend line never cross.
  let mut cursor = bottom + metrics.space(6);
  for callout in callouts.iter().skip(1) {
    painter.text(
      egui::pos2(rect.left(), cursor),
      egui::Align2::LEFT_TOP,
      callout,
      egui::FontId::monospace(theme::CAPTION),
      palette.get(Role::TextDim),
    );
    cursor += metrics.space(4);
  }

  width
}

/// How tall the tool's tip profile is, in millimetres, for the families whose geometry sets one.
///
/// A flat end mill has no profile: its tip is the end of the body, so its height is `0`. A ball nose's is its
/// radius, a corner radius's is its radius, and a V's is how far the cone rises from its flat to its full
/// diameter — the one length that is arithmetic on the tool's own angle rather than a number it carries.
fn tip_height_mm(geometry: &ToolGeometry) -> f64 {
  match ToolFamily::of(geometry) {
    ToolFamily::Flat => 0.0,
    ToolFamily::BallNose => geometry
      .diameter
      .map(|diameter| diameter.mm() / 2.0)
      .unwrap_or(0.0),
    ToolFamily::CornerRadius => geometry.tip_radius.map(|radius| radius.mm()).unwrap_or(0.0),
    ToolFamily::VBit => {
      let Some(diameter) = geometry.diameter else {
        return 0.0;
      };
      let flat = geometry.flat_diameter.map(|flat| flat.mm()).unwrap_or(0.0);
      let Some(angle) = geometry.included_angle else {
        return 0.0;
      };
      let half_angle = (angle.degrees() / 2.0).to_radians();
      if half_angle <= 0.0 || !half_angle.is_finite() {
        return 0.0;
      }
      ((diameter.mm() - flat) / 2.0) / half_angle.tan()
    }
  }
}
