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
  /// Add a straight line, as an open contour.
  AddLine {
    /// Layer to add the entity to.
    layer_id: String,
    /// Identifier for the new entity.
    entity_id: String,
    /// Start point, in millimetres.
    from: Point2,
    /// End point, in millimetres.
    to: Point2,
  },
  /// Add a chain of straight segments through `points`.
  AddPolyline {
    /// Layer to add the entity to.
    layer_id: String,
    /// Identifier for the new entity.
    entity_id: String,
    /// Points to pass through, in millimetres.
    points: Vec<Point2>,
    /// Whether the last point joins the first.
    closed: bool,
  },
  /// Add a single arc through the ordered points `start`, `through` and `end`.
  AddArc {
    /// Layer to add the entity to.
    layer_id: String,
    /// Identifier for the new entity.
    entity_id: String,
    /// Start point, in millimetres.
    start: Point2,
    /// A point the arc passes through, in millimetres.
    through: Point2,
    /// End point, in millimetres.
    end: Point2,
  },
  /// Add an ellipse approximated by arcs within `tolerance`.
  ///
  /// The approximation's achieved error is reported through
  /// [`Command::apply_with_outcome`] / [`crate::AppState::execute_with_outcome`], because an ellipse
  /// cannot be represented exactly by circular arcs and the caller must not be left believing the
  /// tolerance was met.
  AddEllipse {
    /// Layer to add the entity to.
    layer_id: String,
    /// Identifier for the new entity.
    entity_id: String,
    /// Centre, in millimetres.
    at: Point2,
    /// Radius along X, in millimetres.
    rx: f64,
    /// Radius along Y, in millimetres.
    ry: f64,
    /// Largest radial error the caller accepts, in millimetres.
    tolerance: f64,
  },
  /// Add a regular polygon with `sides` straight segments.
  AddPolygon {
    /// Layer to add the entity to.
    layer_id: String,
    /// Identifier for the new entity.
    entity_id: String,
    /// Centre, in millimetres.
    at: Point2,
    /// Circumscribed radius, in millimetres.
    radius: f64,
    /// Number of sides, at least three.
    sides: usize,
    /// Angle of the first vertex, in radians from the positive X axis.
    rotation: f64,
  },
  /// Add a star alternating `points` outer tips and `points` inner valleys.
  AddStar {
    /// Layer to add the entity to.
    layer_id: String,
    /// Identifier for the new entity.
    entity_id: String,
    /// Centre, in millimetres.
    at: Point2,
    /// Radius of the outer tips, in millimetres.
    outer_radius: f64,
    /// Radius of the inner valleys, in millimetres; must be smaller than `outer_radius`.
    inner_radius: f64,
    /// Number of tips, at least three.
    points: usize,
    /// Angle of the first tip, in radians from the positive X axis.
    rotation: f64,
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
      Self::AddLine { .. } => "add line",
      Self::AddPolyline { .. } => "add polyline",
      Self::AddArc { .. } => "add arc",
      Self::AddEllipse { .. } => "add ellipse",
      Self::AddPolygon { .. } => "add polygon",
      Self::AddStar { .. } => "add star",
      Self::DeleteEntity { .. } => "delete entity",
      Self::SetLayerVisible { .. } => "change layer visibility",
    }
  }

  /// Apply the command to a document, discarding whatever it reports.
  pub fn apply(&self, document: &mut Document) -> Result<(), CommandError> {
    self.apply_with_outcome(document).map(|_| ())
  }

  /// Apply the command to a document and report what it produced beyond "it worked".
  ///
  /// The outcome is a separate channel rather than a changed [`Command::apply`] signature so that
  /// every variant keeps the same shape and the callers that do not care about an approximation's
  /// error do not have to unwrap anything.
  pub fn apply_with_outcome(
    &self,
    document: &mut Document,
  ) -> Result<CommandOutcome, CommandError> {
    match self {
      Self::SetJob {
        width,
        height,
        thickness,
      } => {
        document.job.width = cam_units::Length::from_mm(*width);
        document.job.height = cam_units::Length::from_mm(*height);
        document.job.thickness = cam_units::Length::from_mm(*thickness);
        Ok(CommandOutcome::default())
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
        add_shape(document, layer_id, entity_id, || {
          primitive::rectangle(*at, *width, *height)
        })
      }
      Self::AddCircle {
        layer_id,
        entity_id,
        at,
        radius,
      } => {
        require_positive("circle radius", *radius)?;
        add_shape(document, layer_id, entity_id, || {
          primitive::circle(*at, *radius)
        })
      }
      Self::AddLine {
        layer_id,
        entity_id,
        from,
        to,
      } => add_shape(document, layer_id, entity_id, || {
        primitive::line(*from, *to)
      }),
      Self::AddPolyline {
        layer_id,
        entity_id,
        points,
        closed,
      } => add_shape(document, layer_id, entity_id, || {
        primitive::polyline(points, *closed)
      }),
      Self::AddArc {
        layer_id,
        entity_id,
        start,
        through,
        end,
      } => add_shape(document, layer_id, entity_id, || {
        primitive::arc_through(*start, *through, *end)
      }),
      Self::AddEllipse {
        layer_id,
        entity_id,
        at,
        rx,
        ry,
        tolerance,
      } => {
        require_positive("ellipse X radius", *rx)?;
        require_positive("ellipse Y radius", *ry)?;
        require_positive("ellipse tolerance", *tolerance)?;
        ensure_id_is_free(document, entity_id)?;
        let approximation = primitive::ellipse(*at, *rx, *ry, *tolerance)
          .map_err(|error| CommandError::Geometry(error.to_string()))?;
        push_entity(document, layer_id, entity_id, approximation.contour)?;
        Ok(CommandOutcome {
          max_radial_error: Some(approximation.max_radial_error),
        })
      }
      Self::AddPolygon {
        layer_id,
        entity_id,
        at,
        radius,
        sides,
        rotation,
      } => {
        require_at_least("polygon sides", *sides, 3)?;
        require_positive("polygon radius", *radius)?;
        add_shape(document, layer_id, entity_id, || {
          primitive::polygon(*at, *radius, *sides, *rotation)
        })
      }
      Self::AddStar {
        layer_id,
        entity_id,
        at,
        outer_radius,
        inner_radius,
        points,
        rotation,
      } => {
        require_at_least("star points", *points, 3)?;
        require_positive("star outer radius", *outer_radius)?;
        require_positive("star inner radius", *inner_radius)?;
        if inner_radius >= outer_radius {
          return Err(CommandError::OutOfRange {
            kind: "star inner radius",
            requirement: "smaller than the outer radius",
            value: *inner_radius,
          });
        }
        add_shape(document, layer_id, entity_id, || {
          primitive::star(*at, *outer_radius, *inner_radius, *points, *rotation)
        })
      }
      Self::DeleteEntity { entity_id } => {
        for layer in &mut document.layers {
          if let Some(index) = layer
            .entities
            .iter()
            .position(|entity| entity.id == *entity_id)
          {
            layer.entities.remove(index);
            return Ok(CommandOutcome::default());
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
        Ok(CommandOutcome::default())
      }
    }
  }
}

/// What a command produced beyond "it worked".
///
/// A drawing command reports nothing; an ellipse reports the radial error its arc approximation
/// actually achieved (M2 A3), so the shell can surface it instead of the kernel swallowing it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CommandOutcome {
  /// The largest measured radial error of the arc approximation, in millimetres.
  ///
  /// `None` when the command approximated nothing.
  pub max_radial_error: Option<f64>,
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
  /// A count parameter was below the minimum the shape needs.
  #[error("{kind} must be at least {minimum}, got {value}")]
  TooFew {
    /// Which parameter was wrong.
    kind: &'static str,
    /// The smallest acceptable count.
    minimum: usize,
    /// The offending count.
    value: usize,
  },
  /// A parameter was outside the range the shape needs.
  #[error("{kind} must be {requirement}, got {value}")]
  OutOfRange {
    /// Which parameter was wrong.
    kind: &'static str,
    /// What the value should have been, as a short phrase.
    requirement: &'static str,
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

fn require_at_least(kind: &'static str, value: usize, minimum: usize) -> Result<(), CommandError> {
  if value < minimum {
    return Err(CommandError::TooFew {
      kind,
      minimum,
      value,
    });
  }
  Ok(())
}

fn ensure_id_is_free(document: &Document, id: &str) -> Result<(), CommandError> {
  if document.entity(id).is_some() {
    return Err(CommandError::DuplicateEntity(id.to_owned()));
  }
  Ok(())
}

/// Build one contour with the kernel, refuse a taken id, and store it on `layer_id`.
///
/// Every drawing command goes through here so that the order of checks — id free, geometry, layer —
/// is the same for all of them (D-M2-3: `cam-app` validates and stores, `cam-geom` builds).
fn add_shape(
  document: &mut Document,
  layer_id: &str,
  entity_id: &str,
  build: impl FnOnce() -> Result<Contour, cam_geom::GeomError>,
) -> Result<CommandOutcome, CommandError> {
  ensure_id_is_free(document, entity_id)?;
  let contour = build().map_err(|error| CommandError::Geometry(error.to_string()))?;
  push_entity(document, layer_id, entity_id, contour)?;
  Ok(CommandOutcome::default())
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
