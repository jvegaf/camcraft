//! Excellon drill export: the tool table, holes and routed slots.
//!
//! # What is written, and why in this shape
//!
//! * A header between `M48` and `%` declaring `METRIC`, `FMAT,2` and one `TnCdiameter` line per
//!   tool. Diameters are in millimetres, the same unit the reader converts to internally.
//! * A body in absolute mode (`G90`), with `G05` drill mode. Each tool is selected with `Tn`, and
//!   its holes are written as `X...Y...` coordinates. A slot is written as a rapid to the start,
//!   `M15` (tool down), a `G01` feed to the end, `M16` (tool up), `G05`.
//! * Coordinates are written with a decimal point, so the reader takes their scale from the number
//!   itself rather than from a digit-format declaration.
//!
//! Only `Drill` and `Slot` entities are written. Contour entities belong in Gerber, so they are not
//! written here.
//!
//! # Round-trip
//!
//! The acceptance test is the round trip against `cam-excellon`: export, parse, and compare tool
//! diameters, hit count and positions within tolerance.

use std::collections::BTreeMap;

use cam_geom::Point2;
use cam_model::{Document, EntityKind};
use cam_units::Length;

use crate::ExportError;

/// The format name every error in this module carries.
const FORMAT: &str = "excellon";

/// Export `document` as an Excellon drill file.
///
/// Drill and slot entities from all layers are written, in document/layer/entity order. Tools are
/// assigned by unique diameter (sorted, deterministic). The bytes depend on the document and on
/// nothing else.
pub fn export(document: &Document) -> Result<Vec<u8>, ExportError> {
  let mut features: Vec<Feature> = Vec::new();
  for layer in &document.layers {
    for entity in &layer.entities {
      match &entity.kind {
        EntityKind::Drill { center, diameter } => {
          refuse_finite_point(*center, &entity.id, &layer.id)?;
          let d = diameter.mm();
          refuse_finite_positive(d, "diameter", &entity.id, &layer.id)?;
          features.push(Feature::Hole {
            diameter: *diameter,
            at: *center,
          });
        }
        EntityKind::Slot { start, end, width } => {
          refuse_finite_point(*start, &entity.id, &layer.id)?;
          refuse_finite_point(*end, &entity.id, &layer.id)?;
          let w = width.mm();
          refuse_finite_positive(w, "width", &entity.id, &layer.id)?;
          features.push(Feature::Slot {
            width: *width,
            from: *start,
            to: *end,
          });
        }
        EntityKind::Contour { .. } => {}
      }
    }
  }
  Ok(write_excellon(&features).into_bytes())
}

/// A drill feature to write, with its own diameter/width.
#[derive(Clone, Debug)]
enum Feature {
  Hole {
    diameter: Length,
    at: Point2,
  },
  Slot {
    width: Length,
    from: Point2,
    to: Point2,
  },
}

impl Feature {
  fn diameter_mm(&self) -> f64 {
    match self {
      Feature::Hole { diameter, .. } => diameter.mm(),
      Feature::Slot { width, .. } => width.mm(),
    }
  }
}

fn refuse_finite_point(p: Point2, entity_id: &str, layer_id: &str) -> Result<(), ExportError> {
  if !p.x.is_finite() || !p.y.is_finite() {
    return Err(ExportError::refused(
      FORMAT,
      format!(
        "entity {entity_id:?} on layer {layer_id:?} carries a coordinate that is not a finite \
         number, which an Excellon coordinate cannot spell"
      ),
    ));
  }
  Ok(())
}

fn refuse_finite_positive(
  v: f64,
  name: &str,
  entity_id: &str,
  layer_id: &str,
) -> Result<(), ExportError> {
  if !v.is_finite() || v <= 0.0 {
    return Err(ExportError::refused(
      FORMAT,
      format!(
        "entity {entity_id:?} on layer {layer_id:?} carries a {name} that is not a finite \
         positive number, which an Excellon tool cannot spell"
      ),
    ));
  }
  Ok(())
}

/// A lower-level export: write drill features as Excellon.
pub fn export_features(
  holes: &[(Point2, f64)],
  slots: &[(Point2, Point2, f64)],
) -> Result<Vec<u8>, ExportError> {
  let mut features: Vec<Feature> = Vec::new();
  for (at, diameter) in holes {
    if !at.x.is_finite() || !at.y.is_finite() || !diameter.is_finite() || *diameter <= 0.0 {
      return Err(ExportError::refused(
        FORMAT,
        "a hole has a non-finite or non-positive value",
      ));
    }
    features.push(Feature::Hole {
      diameter: Length::from_mm(*diameter),
      at: *at,
    });
  }
  for (from, to, width) in slots {
    if !from.x.is_finite()
      || !from.y.is_finite()
      || !to.x.is_finite()
      || !to.y.is_finite()
      || !width.is_finite()
      || *width <= 0.0
    {
      return Err(ExportError::refused(
        FORMAT,
        "a slot has a non-finite or non-positive value",
      ));
    }
    features.push(Feature::Slot {
      width: Length::from_mm(*width),
      from: *from,
      to: *to,
    });
  }
  Ok(write_excellon(&features).into_bytes())
}

fn write_excellon(features: &[Feature]) -> String {
  // Build tool table: unique diameters, sorted for determinism.
  let mut diameters: Vec<i64> = Vec::new();
  let mut diam_map: BTreeMap<i64, u32> = BTreeMap::new();
  for f in features {
    let key = diameter_key(f.diameter_mm());
    diam_map.entry(key).or_insert_with(|| {
      diameters.push(key);
      diameters.len() as u32
    });
  }

  let mut out = String::new();
  out.push_str("M48\n");
  out.push_str("; camcraft excellon export\n");
  out.push_str("FMAT,2\n");
  out.push_str("METRIC\n");
  for key in &diameters {
    let tool_num = diam_map[key];
    let diameter = key_to_diameter(*key);
    out.push_str(&format!("T{}C{:.6}\n", tool_num, diameter));
  }
  out.push_str("%\n");
  out.push_str("G90\n");
  out.push_str("G05\n");

  let mut current_tool: Option<u32> = None;
  for f in features {
    let key = diameter_key(f.diameter_mm());
    let tool_num = diam_map[&key];
    if current_tool != Some(tool_num) {
      out.push_str(&format!("T{}\n", tool_num));
      current_tool = Some(tool_num);
    }
    match f {
      Feature::Hole { at, .. } => {
        out.push_str(&format!("X{:.6}Y{:.6}\n", at.x, at.y));
      }
      Feature::Slot { from, to, .. } => {
        out.push_str(&format!("G00X{:.6}Y{:.6}\n", from.x, from.y));
        out.push_str("M15\n");
        out.push_str(&format!("G01X{:.6}Y{:.6}\n", to.x, to.y));
        out.push_str("M16\n");
        out.push_str("G05\n");
      }
    }
  }
  out.push_str("M30\n");
  out
}

/// Quantize diameter to a stable key for tool grouping (nanometre precision is enough).
fn diameter_key(mm: f64) -> i64 {
  (mm * 1_000_000.0).round() as i64
}

fn key_to_diameter(key: i64) -> f64 {
  key as f64 / 1_000_000.0
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn diameter_key_is_stable() {
    assert_eq!(diameter_key(1.0), 1_000_000);
    assert_eq!(key_to_diameter(1_000_000), 1.0);
  }
}
