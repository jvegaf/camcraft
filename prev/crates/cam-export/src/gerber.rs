//! Gerber RS-274X export: a dark image layer written as G36/G37 regions.
//!
//! # What is written, and why in this shape
//!
//! * A header declaring `%FSLAX46Y46*%` (4 integer, 6 decimal, leading-zero suppression, absolute),
//!   `%MOMM*%` (millimetres), `%LPD*%` (dark polarity) and `G01*` (linear interpolation). This is the
//!   same dialect the reader accepts (`cam-gerber` parses FSLAX46, MM, LPD, G01/G02/G03, G36/G37).
//! * One contour becomes one G36/G37 region. The contour's own segments are written as they are: a
//!   straight segment is a `D01` line, a circular segment is a `G02` (clockwise) or `G03`
//!   (counter-clockwise) arc with `I,J` offsets from the start point to the arc centre, which is the
//!   form `cam-gerber::arc_from_gerber` reads back.
//! * Only contour entities are written. A Gerber file is a 2D copper/image layer; drill and slot
//!   entities belong in Excellon, so they are not written here.
//!
//! # What a Gerber cannot carry
//!
//! Layer identity, drill/slot entities, and toolpath semantics are not Gerber image constructs.
//! The round-trip this writer promises is geometric: export, parse with `cam-gerber`, image with
//! `image()`, and compare the resulting region to the written contours within tolerance.

use cam_geom::{Contour, Segment};
use cam_model::{Document, EntityKind};

use crate::ExportError;

/// The format name every error in this module carries.
const FORMAT: &str = "gerber";

/// Decimal digits in the FSLAX46Y46 coordinate format.
const DECIMALS: i32 = 6;

/// Export `document` as an RS-274X Gerber image.
///
/// Contour entities from all layers are written as dark G36/G37 regions, in document/layer/entity
/// order. The bytes depend on the document and on nothing else.
pub fn export(document: &Document) -> Result<Vec<u8>, ExportError> {
  let mut contours: Vec<&Contour> = Vec::new();
  for layer in &document.layers {
    for entity in &layer.entities {
      if let EntityKind::Contour { contour } = &entity.kind {
        refuse_non_finite(contour, &entity.id, &layer.id)?;
        contours.push(contour);
      }
    }
  }
  Ok(write_gerber(&contours).into_bytes())
}

/// Export a set of contours as an RS-274X Gerber image.
///
/// This is the lower-level geometric entry point used by round-trip tests: it writes exactly the
/// given contours as dark regions, without looking at document roles or entities.
pub fn export_contours(contours: &[Contour]) -> Result<Vec<u8>, ExportError> {
  for (index, contour) in contours.iter().enumerate() {
    refuse_non_finite(contour, &format!("contour[{index}]"), "contours")?;
  }
  let refs: Vec<&Contour> = contours.iter().collect();
  Ok(write_gerber(&refs).into_bytes())
}

fn refuse_non_finite(
  contour: &Contour,
  entity_id: &str,
  layer_id: &str,
) -> Result<(), ExportError> {
  for vertex in contour.vertices() {
    if !vertex.point.x.is_finite() || !vertex.point.y.is_finite() || !vertex.bulge.is_finite() {
      return Err(ExportError::refused(
        FORMAT,
        format!(
          "entity {entity_id:?} on layer {layer_id:?} carries a coordinate or bulge that is not a \
           finite number, which a Gerber coordinate cannot spell"
        ),
      ));
    }
  }
  Ok(())
}

fn write_gerber(contours: &[&Contour]) -> String {
  let mut out = String::new();
  out.push_str("G04 camcraft gerber export*\n");
  out.push_str("%FSLAX46Y46*%\n");
  out.push_str("%MOMM*%\n");
  out.push_str("%LPD*%\n");
  out.push_str("G01*\n");
  out.push_str("G75*\n");

  for contour in contours {
    write_contour_region(&mut out, contour);
  }

  out.push_str("M02*\n");
  out
}

fn write_contour_region(out: &mut String, contour: &Contour) {
  let segments = contour.segments();
  if segments.is_empty() {
    return;
  }
  let segments = ensure_min_segments(&segments, 3);
  let Some(start) = segment_start(&segments[0]) else {
    return;
  };

  out.push_str("G36*\n");
  out.push_str(&format!("X{}Y{}D02*\n", coord(start.x), coord(start.y)));

  for segment in &segments {
    let end = match segment {
      Segment::Line(line) => line.end,
      Segment::Arc(arc) => arc.end_point(),
    };
    match segment {
      Segment::Line(_) => {
        out.push_str(&format!("X{}Y{}D01*\n", coord(end.x), coord(end.y)));
      }
      Segment::Arc(arc) => {
        let center = arc.center();
        let arc_start = arc.start_point();
        let i_off = center.x - arc_start.x;
        let j_off = center.y - arc_start.y;
        let gcode = if arc.is_ccw() { "G03" } else { "G02" };
        out.push_str(&format!("{gcode}*\n"));
        out.push_str(&format!(
          "X{}Y{}I{}J{}D01*\n",
          coord(end.x),
          coord(end.y),
          coord(i_off),
          coord(j_off)
        ));
        out.push_str("G01*\n");
      }
    }
  }

  out.push_str("G37*\n");
}

fn segment_start(segment: &Segment) -> Option<cam_geom::Point2> {
  Some(match segment {
    Segment::Line(line) => line.start,
    Segment::Arc(arc) => arc.start_point(),
  })
}

/// cam-gerber requires at least 3 segments to close a region contour. A closed 2-segment contour
/// (e.g. a circle represented as two semicircles) is geometrically valid; split segments until the
/// minimum is met.
fn ensure_min_segments(segments: &[Segment], min: usize) -> Vec<Segment> {
  let mut out = segments.to_vec();
  while out.len() < min {
    let (idx, _) = out
      .iter()
      .enumerate()
      .max_by(|a, b| {
        seg_len(*a.1)
          .partial_cmp(&seg_len(*b.1))
          .unwrap_or(std::cmp::Ordering::Equal)
      })
      .unwrap();
    let (a, b) = split_segment(out[idx]);
    out.splice(idx..=idx, [a, b]);
  }
  out
}

fn seg_len(segment: Segment) -> f64 {
  match segment {
    Segment::Line(line) => line.length(),
    Segment::Arc(arc) => arc.length(),
  }
}

fn split_segment(segment: Segment) -> (Segment, Segment) {
  match segment {
    Segment::Line(line) => {
      let mid = line.point_at(0.5);
      (
        Segment::Line(cam_geom::LineSeg::new(line.start, mid)),
        Segment::Line(cam_geom::LineSeg::new(mid, line.end)),
      )
    }
    Segment::Arc(arc) => {
      let mid_angle = arc.start_angle() + arc.sweep() / 2.0;
      let a = cam_geom::Arc::new(
        arc.center(),
        arc.radius(),
        arc.start_angle(),
        arc.sweep() / 2.0,
      )
      .expect("half arc");
      let b = cam_geom::Arc::new(arc.center(), arc.radius(), mid_angle, arc.sweep() / 2.0)
        .expect("half arc");
      (Segment::Arc(a), Segment::Arc(b))
    }
  }
}

fn coord(value_mm: f64) -> String {
  let scale = 10f64.powi(DECIMALS);
  let scaled = (value_mm * scale).round() as i64;
  if scaled == 0 {
    return "0".to_string();
  }
  // Leading zero suppression: omit leading zeros. For integers, that's just the number itself.
  // Negative sign is preserved.
  scaled.to_string()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn coord_formats_leading_zero_suppressed_mm() {
    assert_eq!(coord(0.0), "0");
    assert_eq!(coord(10.5), "10500000");
    assert_eq!(coord(-1.1528623), "-1152862");
  }
}
