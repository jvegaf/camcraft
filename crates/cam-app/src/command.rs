//! Editing commands: every user-visible change to the document.
//!
//! A command performs no validation of the resulting document beyond its own preconditions: the
//! caller ([`crate::AppState::execute`]) applies it to a copy and validates before committing, so a
//! refused edit leaves nothing behind.
//!
//! Entity ids are supplied by the caller (`AppState::next_entity_id`) rather than generated here, so
//! applying a command is a pure function of its arguments. That is what makes golden tests and bug
//! reports reproducible.

use cam_geom::{primitive, Contour, Point2};
use cam_model::{Document, Entity, EntityKind, ModelError};

/// One user-visible edit.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
  /// Change the stock size. Values are millimetres.
  SetJob {
    /// Stock size along X.
    width: f64,
    /// Stock size along Y.
    height: f64,
    /// Stock thickness.
    thickness: f64,
  },
  /// Add a rectangle, counter-clockwise, with its lower-left corner at `at`.
  AddRectangle {
    /// Layer to add the entity to.
    layer_id: String,
    /// Identifier for the new entity.
    entity_id: String,
    /// Lower-left corner, in millimetres.
    at: Point2,
    /// Width in millimetres.
    width: f64,
    /// Height in millimetres.
    height: f64,
  },
  /// Add a circle as two half-turn arcs.
  AddCircle {
    /// Layer to add the entity to.
    layer_id: String,
    /// Identifier for the new entity.
    entity_id: String,
    /// Centre, in millimetres.
    at: Point2,
    /// Radius in millimetres.
    radius: f64,
  },
  /// Remove an entity from whichever layer holds it.
  DeleteEntity {
    /// Identifier of the entity to remove.
    entity_id: String,
  },
  /// Show or hide a layer.
  SetLayerVisible {
    /// Layer to change.
    layer_id: String,
    /// New visibility.
    visible: bool,
  },
}

impl Command {
  /// A short human label, used by the status bar and the undo tooltip.
  pub fn label(&self) -> &'static str {
    match self {
      Self::SetJob { .. } => "set job size",
      Self::AddRectangle { .. } => "add rectangle",
      Self::AddCircle { .. } => "add circle",
      Self::DeleteEntity { .. } => "delete entity",
      Self::SetLayerVisible { .. } => "change layer visibility",
    }
  }

  /// Apply the command to a document.
  pub fn apply(&self, document: &mut Document) -> Result<(), CommandError> {
    match self {
      Self::SetJob {
        width,
        height,
        thickness,
      } => {
        document.job.width = cam_units::Length::from_mm(*width);
        document.job.height = cam_units::Length::from_mm(*height);
        document.job.thickness = cam_units::Length::from_mm(*thickness);
        Ok(())
      }
      Self::AddRectangle {
        layer_id,
        entity_id,
        at,
        width,
        height,
      } => {
        require_positive("rectangle width", *width)?;
        require_positive("rectangle height", *height)?;
        ensure_id_is_free(document, entity_id)?;
        let contour = primitive::rectangle(*at, *width, *height)
          .map_err(|error| CommandError::Geometry(error.to_string()))?;
        push_entity(document, layer_id, entity_id, contour)
      }
      Self::AddCircle {
        layer_id,
        entity_id,
        at,
        radius,
      } => {
        require_positive("circle radius", *radius)?;
        ensure_id_is_free(document, entity_id)?;
        let contour = primitive::circle(*at, *radius)
          .map_err(|error| CommandError::Geometry(error.to_string()))?;
        push_entity(document, layer_id, entity_id, contour)
      }
      Self::DeleteEntity { entity_id } => {
        for layer in &mut document.layers {
          if let Some(index) = layer
            .entities
            .iter()
            .position(|entity| entity.id == *entity_id)
          {
            layer.entities.remove(index);
            return Ok(());
          }
        }
        Err(CommandError::UnknownEntity(entity_id.clone()))
      }
      Self::SetLayerVisible { layer_id, visible } => {
        let layer = document
          .layers
          .iter_mut()
          .find(|layer| layer.id == *layer_id)
          .ok_or_else(|| CommandError::UnknownLayer(layer_id.clone()))?;
        layer.visible = *visible;
        Ok(())
      }
    }
  }
}

/// Why a command was refused before the document was even validated.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum CommandError {
  /// The document would have become invalid, so the command was not applied.
  #[error("the command was refused: {0}")]
  Invalid(#[from] ModelError),
  /// The target layer does not exist.
  #[error("layer {0:?} does not exist")]
  UnknownLayer(String),
  /// The target entity does not exist.
  #[error("entity {0:?} does not exist")]
  UnknownEntity(String),
  /// An entity with that identifier already exists.
  #[error("entity {0:?} already exists")]
  DuplicateEntity(String),
  /// A size parameter was zero, negative or not finite.
  #[error("{kind} must be a positive finite number, got {value}")]
  NonPositiveSize {
    /// Which parameter was wrong.
    kind: &'static str,
    /// The offending value.
    value: f64,
  },
  /// Geometry construction failed.
  #[error("invalid geometry: {0}")]
  Geometry(String),
}

fn require_positive(kind: &'static str, value: f64) -> Result<(), CommandError> {
  if !value.is_finite() || value <= 0.0 {
    return Err(CommandError::NonPositiveSize { kind, value });
  }
  Ok(())
}

fn ensure_id_is_free(document: &Document, id: &str) -> Result<(), CommandError> {
  if document.entity(id).is_some() {
    return Err(CommandError::DuplicateEntity(id.to_owned()));
  }
  Ok(())
}

fn push_entity(
  document: &mut Document,
  layer_id: &str,
  entity_id: &str,
  contour: Contour,
) -> Result<(), CommandError> {
  let layer = document
    .layers
    .iter_mut()
    .find(|layer| layer.id == layer_id)
    .ok_or_else(|| CommandError::UnknownLayer(layer_id.to_owned()))?;
  layer.entities.push(Entity {
    id: entity_id.to_owned(),
    kind: EntityKind::Contour { contour },
  });
  Ok(())
}
