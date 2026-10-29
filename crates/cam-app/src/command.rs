//! Editing commands: every user-visible change to the document.
//!
//! A command performs no validation of the resulting document beyond its own preconditions: the
//! caller ([`crate::AppState::execute`]) applies it to a copy and validates before committing, so a
//! refused edit leaves nothing behind.
//!
//! Entity ids are supplied by the caller (`AppState::next_entity_id`) rather than generated here, so
//! applying a command is a pure function of its arguments. That is what makes golden tests and bug
//! reports reproducible.

use cam_geom::text::{text_outlines, SingleStrokeFont};
use cam_geom::transform;
use cam_geom::{primitive, Bounds, Contour, Point2, Vec2};
use cam_model::{Document, Entity, EntityKind, ModelError};

/// One geometric transform applied to a whole selection.
///
/// The geometry comes from [`cam_geom::transform`] and nowhere else (D-M2-3): `cam-app` validates the
/// input, stores what the kernel returned, and reports what that cost. That split is what keeps the
/// kernel usable by a future importer without going through the command layer.
#[derive(Clone, Debug, PartialEq)]
pub enum Transform {
  /// Move every selected entity by `delta`.
  Translate {
    /// Displacement, in millimetres.
    delta: Vec2,
  },
  /// Turn every selected entity `radians` counter-clockwise about `about`.
  Rotate {
    /// Angle in radians, counter-clockwise about the positive Z axis.
    radians: f64,
    /// Pivot, in millimetres.
    about: Point2,
  },
  /// Scale every selected entity about `about`, tessellating at `tolerance` when it must.
  ///
  /// A uniform factor, or an arc-free selection, stays exact. A non-uniform factor over arcs
  /// cannot keep them: those contours come back tessellated within `tolerance` and the command's
  /// [`CommandOutcome::lost_arcs`] says how many arcs the selection lost (M2 A5, D-M2-2).
  Scale {
    /// Factor along X; it must be finite and non-zero.
    sx: f64,
    /// Factor along Y; it must be finite and non-zero.
    sy: f64,
    /// Anchor point, in millimetres.
    about: Point2,
    /// Largest deviation the caller accepts when arcs must be tessellated, in millimetres.
    tolerance: f64,
  },
  /// Reflect every selected entity about the line through `point` with direction `direction`.
  Mirror {
    /// A point on the mirror line, in millimetres.
    point: Point2,
    /// Direction of the mirror line; it must have a non-zero length.
    direction: Vec2,
  },
}

/// Which edge or centre line of the selection's own combined bounds an alignment uses.
///
/// The target is a coordinate of the selection, never of the job: `Left` means "on the left edge of
/// this group", not "on the left edge of the sheet".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlignMode {
  /// Put every selected entity's left edge on the left edge of the selection's combined bounds.
  Left,
  /// Put every selected entity's right edge on the right edge of those bounds.
  Right,
  /// Put every selected entity's top edge on the top edge of those bounds.
  Top,
  /// Put every selected entity's bottom edge on the bottom edge of those bounds.
  Bottom,
  /// Put every selected entity's horizontal centre on the vertical centre line of those bounds.
  HorizontalCentre,
  /// Put every selected entity's vertical centre on the horizontal centre line of those bounds.
  VerticalCentre,
}

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
  /// Add `text` as one entity per glyph stroke-contour, ids running from `first_entity_index`.
  ///
  /// A document entity holds exactly one contour, so a string becomes **several** entities, the way
  /// any vector editor stores a word: "AB" is one entity per pen stroke of the `A` and the `B`.
  /// That is why this variant takes a starting index instead of an `entity_id`: the ids are
  /// `E{first_entity_index}`, `E{first_entity_index + 1}`, … in
  /// [`cam_geom::text::text_outlines`] order, so they stay caller-supplied and applying the command
  /// stays a pure function of its arguments. If any of those ids is already taken the whole command
  /// is refused before the document is touched, so text is never left half drawn (M2 A12).
  ///
  /// The text is turned into contours here, at creation time (M2 A14): the document schema stays
  /// `c2d/document@1` and nothing downstream has to know a word was ever typed.
  AddText {
    /// Layer to add the entities to.
    layer_id: String,
    /// Number in the first id: `1` starts at `E1`.
    first_entity_index: u32,
    /// The string to lay out; it must draw at least one contour.
    text: String,
    /// Baseline start of the first line, in millimetres.
    origin: Point2,
    /// Cap height in millimetres.
    height: f64,
  },
  /// Remove an entity from whichever layer holds it.
  DeleteEntity {
    /// Identifier of the entity to remove.
    entity_id: String,
  },
  /// Apply one [`Transform`] to every entity in `entity_ids`, as a single undo step.
  ///
  /// Refused atomically (M2 A12): when any id is unknown or repeated, when the transform's own
  /// numbers are invalid, or when the kernel refuses the geometry, not one entity moves. An empty
  /// selection is refused too, because a transform of nothing would record an undo step for no
  /// change the user can see.
  TransformEntities {
    /// Entities to transform, in selection order.
    entity_ids: Vec<String>,
    /// The transform to apply to each one.
    transform: Transform,
  },
  /// Copy the selection into an explicit `rows` × `columns` grid of duplicates.
  ///
  /// Every cell is a **full duplicate of the selection**, not something accumulated from the
  /// previous cell, so a copy is exactly its source moved by `(column * dx, row * dy)`. Each copy
  /// lands on the layer its source came from and the originals stay where they are.
  ///
  /// The ids are `E{first_entity_index}`, `E{first_entity_index + 1}`, … in a documented order:
  /// **row-major — rows outer, columns inner, and inside a cell the order of `entity_ids`**. A 2×3
  /// array of a two-entity selection therefore mints twelve ids, `E3..=E14` when it starts at 3,
  /// and reports them through [`CommandOutcome::created_entities`]. Every one of them is checked
  /// for a collision before the first entity is stored, so a taken id refuses the whole grid
  /// instead of leaving half of it behind (M2 A12).
  ArrayCopy {
    /// Entities to copy; the selection must not be empty.
    entity_ids: Vec<String>,
    /// Number of rows, at least one.
    rows: usize,
    /// Number of columns, at least one.
    columns: usize,
    /// Spacing between columns, in millimetres along X; it must be finite.
    dx: f64,
    /// Spacing between rows, in millimetres along Y; it must be finite.
    dy: f64,
    /// Number in the first minted id: `1` starts at `E1`, zero starts at `E0`. The index is taken
    /// literally, as [`Command::AddText`] takes it: an id is an opaque string, and the guarantee
    /// that matters is that every minted id is free, which is checked before anything is stored.
    first_entity_index: u32,
  },
  /// Move the selection so it lines up on an edge or centre line of its own combined bounds.
  ///
  /// The target coordinate comes from the selection, not from the job, so aligning a group left
  /// puts every entity's left edge on the group's leftmost edge and leaves the other axis alone.
  AlignEntities {
    /// Entities to align; the selection must not be empty.
    entity_ids: Vec<String>,
    /// Which edge or centre line to line up on.
    mode: AlignMode,
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
      Self::AddText { .. } => "add text",
      Self::DeleteEntity { .. } => "delete entity",
      Self::SetLayerVisible { .. } => "change layer visibility",
      Self::TransformEntities { transform, .. } => match transform {
        Transform::Translate { .. } => "move entities",
        Transform::Rotate { .. } => "rotate entities",
        Transform::Scale { .. } => "scale entities",
        Transform::Mirror { .. } => "mirror entities",
      },
      Self::ArrayCopy { .. } => "array copy",
      Self::AlignEntities { .. } => "align entities",
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
          ..CommandOutcome::default()
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
      Self::AddText {
        layer_id,
        first_entity_index,
        text,
        origin,
        height,
      } => {
        require_positive("text height", *height)?;
        let contours = text_outlines(text, *origin, *height, &SingleStrokeFont::new())
          .map_err(|error| CommandError::Geometry(error.to_string()))?;
        if contours.is_empty() {
          return Err(CommandError::EmptyText);
        }
        // How many ids this command mints is itself an output of the layout, so they can only be
        // checked here — and every one of them is checked before a single entity is stored, which
        // is what makes a collision an all-or-nothing refusal instead of a half-drawn word.
        let ids: Vec<String> = (0..contours.len())
          .map(|offset| format!("E{}", u64::from(*first_entity_index) + offset as u64))
          .collect();
        for id in &ids {
          ensure_id_is_free(document, id)?;
        }
        ensure_layer_exists(document, layer_id)?;
        for (id, contour) in ids.iter().zip(contours) {
          push_entity(document, layer_id, id, contour)?;
        }
        Ok(CommandOutcome {
          created_entities: ids,
          ..CommandOutcome::default()
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
      Self::TransformEntities {
        entity_ids,
        transform,
      } => transform_entities(document, entity_ids, transform),
      Self::ArrayCopy {
        entity_ids,
        rows,
        columns,
        dx,
        dy,
        first_entity_index,
      } => array_copy(
        document,
        entity_ids,
        *rows,
        *columns,
        *dx,
        *dy,
        *first_entity_index,
      ),
      Self::AlignEntities { entity_ids, mode } => align_entities(document, entity_ids, *mode),
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
/// A single-entity drawing command reports nothing; an ellipse reports the radial error its arc
/// approximation actually achieved (M2 A3) and a text command reports the several entities it
/// created, so the shell can surface both instead of the kernel swallowing them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CommandOutcome {
  /// The largest measured radial error of the arc approximation, in millimetres.
  ///
  /// `None` when the command approximated nothing.
  pub max_radial_error: Option<f64>,
  /// The ids of the entities this command created, in the order it created them.
  ///
  /// Empty for every command that creates nothing or creates a single entity it already named.
  pub created_entities: Vec<String>,
  /// How many arcs the command cost the document, because a non-uniform scale had to tessellate
  /// them (M2 A5).
  ///
  /// Summed over the whole selection, so a caller can report "n arcs lost" without asking each
  /// entity. Zero for every command that approximates nothing this way.
  pub lost_arcs: usize,
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
  /// The command needs at least one entity and was given none.
  #[error("no entities were selected")]
  EmptySelection,
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
  /// The text to draw produced no contours, so there is nothing to draw and nothing to undo.
  ///
  /// A command that draws nothing must not record an undo step: the user would press undo once
  /// and see the document change for a word that was never visible.
  #[error("the text to draw must not be empty")]
  EmptyText,
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

fn ensure_layer_exists(document: &Document, layer_id: &str) -> Result<(), CommandError> {
  if document.layers.iter().any(|layer| layer.id == layer_id) {
    return Ok(());
  }
  Err(CommandError::UnknownLayer(layer_id.to_owned()))
}

/// Where an entity lives: its layer index and its index inside that layer.
///
/// Indexes rather than a reference, because a command has to read every contour of the selection
/// before it writes any of them back: that is what makes a multi-entity edit all or nothing.
type EntitySite = (usize, usize);

/// Refuse a command that was given nothing to act on.
fn require_selection(entity_ids: &[String]) -> Result<(), CommandError> {
  if entity_ids.is_empty() {
    return Err(CommandError::EmptySelection);
  }
  Ok(())
}

/// Resolve every id to a site, refusing an unknown or repeated id.
///
/// A repeated id is refused rather than silently applied twice: a "move selection" listing one
/// entity twice would otherwise move it twice, which no caller can mean.
fn resolve_sites(
  document: &Document,
  entity_ids: &[String],
) -> Result<Vec<EntitySite>, CommandError> {
  let mut seen = std::collections::BTreeSet::new();
  let mut sites = Vec::with_capacity(entity_ids.len());
  for id in entity_ids {
    if !seen.insert(id.as_str()) {
      return Err(CommandError::DuplicateEntity(id.clone()));
    }
    let site = document
      .layers
      .iter()
      .enumerate()
      .find_map(|(layer_index, layer)| {
        layer
          .entities
          .iter()
          .position(|entity| entity.id == *id)
          .map(|entity_index| (layer_index, entity_index))
      })
      .ok_or_else(|| CommandError::UnknownEntity(id.clone()))?;
    sites.push(site);
  }
  Ok(sites)
}

/// The contour at a resolved site.
fn contour_at(document: &Document, (layer_index, entity_index): EntitySite) -> &Contour {
  let EntityKind::Contour { contour } = &document.layers[layer_index].entities[entity_index].kind;
  contour
}

fn replace_contour_at(
  document: &mut Document,
  (layer_index, entity_index): EntitySite,
  contour: Contour,
) {
  document.layers[layer_index].entities[entity_index].kind = EntityKind::Contour { contour };
}

fn geometry_error(error: cam_geom::GeomError) -> CommandError {
  CommandError::Geometry(error.to_string())
}

fn require_finite(kind: &'static str, value: f64) -> Result<(), CommandError> {
  if !value.is_finite() {
    return Err(CommandError::OutOfRange {
      kind,
      requirement: "a finite number",
      value,
    });
  }
  Ok(())
}

fn require_finite_point(kind: &'static str, point: Point2) -> Result<(), CommandError> {
  require_finite(kind, point.x)?;
  require_finite(kind, point.y)
}

fn require_finite_vector(kind: &'static str, vector: Vec2) -> Result<(), CommandError> {
  require_finite(kind, vector.x)?;
  require_finite(kind, vector.y)
}

fn require_scale_factor(kind: &'static str, value: f64) -> Result<(), CommandError> {
  if !value.is_finite() || value == 0.0 {
    return Err(CommandError::OutOfRange {
      kind,
      requirement: "a finite, non-zero factor",
      value,
    });
  }
  Ok(())
}

/// Check a transform's own numbers before any geometry is touched.
///
/// The kernel refuses the same inputs, but doing it here keeps the refusal a property of the
/// command rather than of whichever contour happened to be reached first, and stops a `NaN` from
/// ever reaching the document.
fn validate_transform(transform: &Transform) -> Result<(), CommandError> {
  match transform {
    Transform::Translate { delta } => require_finite_vector("translate delta", *delta),
    Transform::Rotate { radians, about } => {
      require_finite("rotation angle", *radians)?;
      require_finite_point("rotate pivot", *about)
    }
    Transform::Scale {
      sx,
      sy,
      about,
      tolerance,
    } => {
      require_scale_factor("scale X factor", *sx)?;
      require_scale_factor("scale Y factor", *sy)?;
      require_positive("scale tolerance", *tolerance)?;
      require_finite_point("scale anchor", *about)
    }
    Transform::Mirror { point, direction } => {
      require_finite_point("mirror line point", *point)?;
      require_finite_vector("mirror line direction", *direction)
    }
  }
}

/// Run one contour through the kernel, returning it with the arcs it lost.
fn apply_transform(
  transform: &Transform,
  contour: &Contour,
) -> Result<(Contour, usize), CommandError> {
  match transform {
    Transform::Translate { delta } => Ok((
      transform::translate(contour, *delta).map_err(geometry_error)?,
      0,
    )),
    Transform::Rotate { radians, about } => Ok((
      transform::rotate(contour, *radians, *about).map_err(geometry_error)?,
      0,
    )),
    Transform::Scale {
      sx,
      sy,
      about,
      tolerance,
    } => {
      let scaled =
        transform::scale(contour, *sx, *sy, *about, *tolerance).map_err(geometry_error)?;
      Ok((scaled.contour, scaled.lost_arcs))
    }
    Transform::Mirror { point, direction } => Ok((
      transform::mirror(contour, *point, *direction).map_err(geometry_error)?,
      0,
    )),
  }
}

fn transform_entities(
  document: &mut Document,
  entity_ids: &[String],
  transform: &Transform,
) -> Result<CommandOutcome, CommandError> {
  require_selection(entity_ids)?;
  validate_transform(transform)?;
  let sites = resolve_sites(document, entity_ids)?;

  // Every contour is transformed before any of them is stored, so a refusal part way through
  // leaves the selection untouched instead of half moved.
  let mut planned = Vec::with_capacity(sites.len());
  let mut lost_arcs = 0;
  for site in sites {
    let (contour, lost) = apply_transform(transform, contour_at(document, site))?;
    lost_arcs += lost;
    planned.push((site, contour));
  }
  for (site, contour) in planned {
    replace_contour_at(document, site, contour);
  }
  Ok(CommandOutcome {
    lost_arcs,
    ..CommandOutcome::default()
  })
}

fn array_copy(
  document: &mut Document,
  entity_ids: &[String],
  rows: usize,
  columns: usize,
  dx: f64,
  dy: f64,
  first_entity_index: u32,
) -> Result<CommandOutcome, CommandError> {
  require_at_least("array rows", rows, 1)?;
  require_at_least("array columns", columns, 1)?;
  require_finite("array X spacing", dx)?;
  require_finite("array Y spacing", dy)?;
  // Zero is refused: it would mint `E0`, which the drawing counter never produces.
  require_selection(entity_ids)?;
  let sources: Vec<(EntitySite, Contour)> = resolve_sites(document, entity_ids)?
    .into_iter()
    .map(|site| (site, contour_at(document, site).clone()))
    .collect();

  // Plan the whole grid — ids and geometry — before storing any of it, so an id taken late in the
  // run refuses the command instead of leaving the first copies behind (M2 A12).
  let mut planned: Vec<(usize, String, Contour)> = Vec::new();
  let mut next = u64::from(first_entity_index);
  for row in 0..rows {
    for column in 0..columns {
      let delta = Vec2::new(column as f64 * dx, row as f64 * dy);
      for (site, contour) in &sources {
        let id = format!("E{next}");
        next += 1;
        ensure_id_is_free(document, &id)?;
        let copy = transform::translate(contour, delta).map_err(geometry_error)?;
        planned.push((site.0, id, copy));
      }
    }
  }

  let created: Vec<String> = planned.iter().map(|(_, id, _)| id.clone()).collect();
  for (layer_index, id, contour) in planned {
    // Reuse the drawing path's insert so a copy is stored exactly like a drawn entity.
    let layer_id = document.layers[layer_index].id.clone();
    push_entity(document, &layer_id, &id, contour)?;
  }
  Ok(CommandOutcome {
    created_entities: created,
    ..CommandOutcome::default()
  })
}

/// The displacement that brings one entity's bounds onto the selection's aligned coordinate.
fn alignment_offset(mode: AlignMode, combined: Bounds, entity: Bounds) -> Vec2 {
  match mode {
    AlignMode::Left => Vec2::new(combined.min.x - entity.min.x, 0.0),
    AlignMode::Right => Vec2::new(combined.max.x - entity.max.x, 0.0),
    AlignMode::Bottom => Vec2::new(0.0, combined.min.y - entity.min.y),
    AlignMode::Top => Vec2::new(0.0, combined.max.y - entity.max.y),
    AlignMode::HorizontalCentre => Vec2::new(combined.center().x - entity.center().x, 0.0),
    AlignMode::VerticalCentre => Vec2::new(0.0, combined.center().y - entity.center().y),
  }
}

fn align_entities(
  document: &mut Document,
  entity_ids: &[String],
  mode: AlignMode,
) -> Result<CommandOutcome, CommandError> {
  require_selection(entity_ids)?;
  let sites = resolve_sites(document, entity_ids)?;
  // The target is the selection's own combined bounds, computed before anything moves.
  let combined = sites
    .iter()
    .map(|site| contour_at(document, *site).bounds())
    .reduce(Bounds::union)
    .unwrap_or_default();

  let mut planned = Vec::with_capacity(sites.len());
  for site in sites {
    let contour = contour_at(document, site);
    let delta = alignment_offset(mode, combined, contour.bounds());
    planned.push((
      site,
      transform::translate(contour, delta).map_err(geometry_error)?,
    ));
  }
  for (site, contour) in planned {
    replace_contour_at(document, site, contour);
  }
  Ok(CommandOutcome::default())
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
