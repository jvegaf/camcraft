//! Assembling imported contours into a fresh document.
//!
//! Importing never mutates a document (D-M3-9): the caller gets a new one, or none at all.

use cam_geom::{transform, Bounds, Vec2};
use cam_model::{Document, Entity, EntityKind, Job, Layer};
use cam_units::{Length, UnitSystem};

use crate::{ImportError, Imported};

/// How far the imported geometry is placed from the job's bottom-left corner, in millimetres.
///
/// A cut path flush with the stock edge is a cut path that runs off the machine, so the import
/// leaves a margin and says where the geometry went.
pub const PLACEMENT_MARGIN: f64 = 10.0;

/// Build a document from an import.
///
/// The job is the imported bounding box plus [`PLACEMENT_MARGIN`] on each side, and the geometry is
/// translated so its bounds start at `(PLACEMENT_MARGIN, PLACEMENT_MARGIN)`. Layers become `L1..Ln`
/// and entities `E1..En` in the source order; when `single_layer` is given, every contour lands in
/// one layer with that name.
pub fn into_document(
  imported: &Imported,
  single_layer: Option<&str>,
) -> Result<Document, ImportError> {
  let format = imported.format.name();
  let mut bounds = Bounds::empty();
  for layer in &imported.layers {
    for contour in &layer.contours {
      bounds = bounds.union(contour.bounds());
    }
  }
  if bounds.is_empty() {
    return Err(ImportError::NothingImported {
      format,
      reason: "no contour produced bounds".to_string(),
    });
  }

  let offset = Vec2::new(
    PLACEMENT_MARGIN - bounds.min.x,
    PLACEMENT_MARGIN - bounds.min.y,
  );

  let mut job = Job::default_sheet();
  job.width = Length::from_mm(bounds.width() + 2.0 * PLACEMENT_MARGIN);
  job.height = Length::from_mm(bounds.height() + 2.0 * PLACEMENT_MARGIN);
  job.units = UnitSystem::Mm;

  let mut document = Document::new(job);
  let mut entity_number: usize = 1;

  let groups: Vec<(String, Vec<cam_geom::Contour>)> = match single_layer {
    Some(name) => vec![(
      name.to_string(),
      imported
        .layers
        .iter()
        .flat_map(|layer| layer.contours.clone())
        .collect(),
    )],
    None => imported
      .layers
      .iter()
      .map(|layer| (layer.name.clone(), layer.contours.clone()))
      .collect(),
  };

  for (index, (name, contours)) in groups.into_iter().enumerate() {
    let mut layer = Layer {
      id: format!("L{}", index + 1),
      name,
      visible: true,
      entities: Vec::new(),
    };
    for contour in contours {
      let contour = transform::translate(&contour, offset)
        .map_err(|error| ImportError::parse_plain(format, error.to_string()))?;
      layer.entities.push(Entity {
        id: format!("E{entity_number}"),
        kind: EntityKind::Contour { contour },
      });
      entity_number += 1;
    }
    if !layer.entities.is_empty() {
      document.layers.push(layer);
    }
  }

  document
    .validate()
    .map_err(|error| ImportError::parse_plain(format, error.to_string()))?;
  Ok(document)
}
