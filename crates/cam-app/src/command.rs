//! Editing commands: every user-visible change to the document.
//!
//! A command performs no validation of the resulting document beyond its own preconditions: the
//! caller ([`crate::AppState::execute`]) applies it to a copy and validates before committing, so a
//! refused edit leaves nothing behind.
//!
//! Entity ids are supplied by the caller (`AppState::next_entity_id`) rather than generated here, so
//! applying a command is a pure function of its arguments. That is what makes golden tests and bug
//! reports reproducible.

use cam_geom::edit::{delete_vertex, insert_vertex, move_vertex};
use cam_geom::join::join;
use cam_geom::offset::{offset_contour, OffsetOptions};
use cam_geom::transform;
use cam_geom::trim::trim;
use cam_geom::{primitive, Bounds, Contour, Point2, TolerancePolicy, Vec2};
use cam_model::{Document, Entity, EntityKind, ModelError, Operation, Toolpath};

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
    /// Tolerance policy used when arcs must be tessellated, in millimetres.
    tolerance: TolerancePolicy,
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

/// One node edit applied to a single entity's contour.
///
/// The geometry comes from [`cam_geom::edit`] (D-M2-3): `cam-app` resolves the entity, hands the
/// kernel the edit and stores what came back. Insertion projects onto the segment, so clicking near
/// a straight segment keeps it straight and clicking near an arc splits the arc on its own circle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VertexEdit {
  /// Move the vertex at `index` to `to`, keeping every surrounding bulge.
  Move {
    /// Index of the vertex to move.
    index: usize,
    /// New position, in millimetres.
    to: Point2,
  },
  /// Insert a vertex inside segment `segment_index`, projecting `at` onto that segment.
  Insert {
    /// Index of the segment to split.
    segment_index: usize,
    /// Requested position, in millimetres; the kernel projects it onto the segment.
    at: Point2,
  },
  /// Remove the vertex at `index`; refused when fewer than two vertices would remain.
  Delete {
    /// Index of the vertex to remove.
    index: usize,
  },
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
  /// Add an ellipse approximated by arcs within the policy's chord tolerance.
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
    /// Tolerance policy for the approximation, in millimetres.
    tolerance: TolerancePolicy,
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
    /// literally: an id is an opaque string, and the guarantee that matters is that every minted id
    /// is free, which is checked before anything is stored.
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
  /// Offset a closed entity by `distance` and store the result as a **new** entity.
  ///
  /// The geometry comes from [`cam_geom::offset::offset_contour`] and nowhere else (D-M2-3), which
  /// keeps arcs as arcs and refuses a collapse instead of handing back a self-intersecting contour.
  /// The source entity stays exactly where it was: an offset adds a contour, it never replaces one.
  ///
  /// The new id is `E{first_entity_index}`, taken literally, and it is checked for a collision
  /// before the kernel runs, so a taken id refuses the whole command and the document never gains
  /// the offset of anything (M2 A12). The number is supplied by the
  /// caller rather than generated here so that applying the command stays a pure function of its
  /// arguments.
  OffsetContour {
    /// Entity to offset; it must be a closed contour.
    entity_id: String,
    /// Distance in millimetres. Positive grows the enclosed region of a counter-clockwise contour,
    /// which is why the sign always means "away from the material".
    distance: f64,
    /// Number in the minted id: `1` mints `E1`, zero mints `E0`. The index is taken literally, and
    /// the guarantee that matters — the minted id is free — is checked before anything is stored.
    first_entity_index: u32,
  },
  /// Join the selected open entities into one contour, replacing the first and removing the rest.
  ///
  /// The chain is built by [`cam_geom::join::join`] and is refused, naming why, when the group is
  /// ambiguous (three or more ends at one point) or does not form exactly one chain. All of it is
  /// one undo step, so a join is undone in one press rather than one per removed entity (M2 A12).
  JoinEntities {
    /// Entities to join, in selection order; the joined contour takes the first one's id and layer.
    entity_ids: Vec<String>,
    /// How close two endpoints must be to count as touching, in millimetres.
    tolerance: TolerancePolicy,
  },
  /// Remove the span of `target_id` between its two crossings with `cutter_id`.
  ///
  /// [`cam_geom::trim::trim`] decides which span goes and returns the pieces that survive; the
  /// removed span is discarded. The target is replaced by those pieces: one entity per kept piece,
  /// each minted from `first_entity_index` in the kernel's order along the original contour, so a
  /// closed target opened by one cut becomes exactly one entity and an open target cut in the
  /// middle becomes two. The target id is freed, and the cutter is never touched.
  TrimEntity {
    /// Entity to trim; it must cross the cutter at exactly two usable points.
    target_id: String,
    /// The cutting contour.
    cutter_id: String,
    /// How close a crossing must be to count, in millimetres.
    tolerance: TolerancePolicy,
    /// Number in the first minted id: `1` starts at `E1`, zero starts at `E0`. Every id the batch
    /// needs is checked for a collision before the target is removed, so a taken id leaves both
    /// entities exactly as they were (M2 A12).
    first_entity_index: u32,
  },
  /// Apply one [`VertexEdit`] to a single entity's contour.
  ///
  /// The kernel's refusal is the command's refusal, message included, and a refused edit leaves the
  /// contour byte for byte as it was.
  EditVertex {
    /// Entity to edit.
    entity_id: String,
    /// Which node edit to apply.
    edit: VertexEdit,
  },
  /// Add a toolpath, binding selected geometry to a tool number and an operation.
  ///
  /// This is the layer the shell's toolpath form calls (S7): before it, authoring existed only as
  /// `cam-cli toolpath`, which pushes a [`cam_model::Toolpath`] directly. The variant builds that
  /// same record and nothing else, so the document gains no field and no rule from it.
  ///
  /// It does **not** consult the tool library: `cam-app` has no `cam-store` edge, and a number the
  /// library does not hold is refused by `emit`, where the library is actually read. What it does
  /// check is what the document can check without one — the selection is not empty, the id is free,
  /// and the document still validates afterwards, which is what refuses an absent entity or a
  /// cutting operation over an open contour, naming both.
  ///
  /// The id is supplied by the caller rather than minted here, the way every drawing variant takes
  /// its `entity_id`: the document carries no toolpath counter to mint one from, so applying the
  /// command stays a pure function of its arguments.
  AddToolpath {
    /// Identifier for the new toolpath; it must be free.
    toolpath_id: String,
    /// Display name, as the toolpath list shows it.
    name: String,
    /// Entities the toolpath is computed from; at least one.
    entity_ids: Vec<String>,
    /// Tool number, matched against the tool library at emission time.
    tool_number: u32,
    /// The operation and its parameters.
    operation: Operation,
  },
  /// Give an existing toolpath a different tool, leaving its geometry and operation alone.
  ///
  /// This is the tool card's "choose another tool": one number changes and nothing else does. The
  /// number is not checked against the library for the reason [`Command::AddToolpath`] gives.
  SetToolpathToolNumber {
    /// Identifier of the toolpath to retool.
    toolpath_id: String,
    /// The new tool number.
    tool_number: u32,
  },
  /// Replace one toolpath's operation, with its parameters, and leave the rest of it alone.
  ///
  /// An operation that cannot cut the toolpath's own selection — a pocket over an open vector, say —
  /// is refused by [`Document::validate`], naming the toolpath and the entity, and the document is
  /// left exactly as it was.
  SetToolpathOperation {
    /// Identifier of the toolpath to change.
    toolpath_id: String,
    /// The new operation and its parameters.
    operation: Operation,
  },
  /// Rename one toolpath, leaving its geometry, its tool and its operation alone.
  ///
  /// A name that is empty or only whitespace is refused before anything is assigned: a blank row in the
  /// toolpath list is a toolpath nobody can pick out, and an undo step that changed no name an operator can
  /// read is a step that lied about what happened. The stored name is the caller's own string — the caller
  /// trims it, the way it trims the name [`Command::AddToolpath`] is given.
  SetToolpathName {
    /// Identifier of the toolpath to rename.
    toolpath_id: String,
    /// The new display name.
    name: String,
  },
  /// Remove a toolpath from the document, leaving its geometry and every other toolpath alone.
  DeleteToolpath {
    /// Identifier of the toolpath to remove.
    toolpath_id: String,
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
      Self::TransformEntities { transform, .. } => match transform {
        Transform::Translate { .. } => "move entities",
        Transform::Rotate { .. } => "rotate entities",
        Transform::Scale { .. } => "scale entities",
        Transform::Mirror { .. } => "mirror entities",
      },
      Self::ArrayCopy { .. } => "array copy",
      Self::AlignEntities { .. } => "align entities",
      Self::OffsetContour { .. } => "offset contour",
      Self::JoinEntities { .. } => "join entities",
      Self::TrimEntity { .. } => "trim entity",
      Self::EditVertex { edit, .. } => match edit {
        VertexEdit::Move { .. } => "move vertex",
        VertexEdit::Insert { .. } => "insert vertex",
        VertexEdit::Delete { .. } => "delete vertex",
      },
      Self::AddToolpath { .. } => "add toolpath",
      Self::SetToolpathToolNumber { .. } => "set toolpath tool",
      Self::SetToolpathOperation { .. } => "set toolpath operation",
      Self::SetToolpathName { .. } => "set toolpath name",
      Self::DeleteToolpath { .. } => "delete toolpath",
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
        // Checked before anything is assigned, so a refusal leaves the job exactly as it was. The
        // check has to live here and not only in `AppState::execute_with_outcome`: that path does
        // catch a zero, but only by validating the document *after* the assignment, and `apply` is
        // public — a caller that hands a document in directly would get an invalid job back and the
        // next `Job::outline` would panic on it. This is the last place before that panic, so the
        // three dimensions are refused here, with the model's own error: `Document::validate` refuses
        // a non-positive dimension by exactly this name, and a second spelling of the same rule would
        // be a second answer to one question.
        for (field, value) in [
          ("width", *width),
          ("height", *height),
          ("thickness", *thickness),
        ] {
          if !value.is_finite() || value <= 0.0 {
            return Err(ModelError::NonPositiveJobDimension { field, value }.into());
          }
        }
        document.board.width = cam_units::Length::from_mm(*width);
        document.board.height = cam_units::Length::from_mm(*height);
        document.board.thickness = cam_units::Length::from_mm(*thickness);
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
        require_positive("ellipse tolerance", tolerance.chord)?;
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
      Self::DeleteEntity { entity_id } => {
        remove_entity(document, entity_id)?;
        Ok(CommandOutcome::default())
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
      Self::OffsetContour {
        entity_id,
        distance,
        first_entity_index,
      } => offset_entity(document, entity_id, *distance, *first_entity_index),
      Self::JoinEntities {
        entity_ids,
        tolerance,
      } => join_entities(document, entity_ids, *tolerance),
      Self::TrimEntity {
        target_id,
        cutter_id,
        tolerance,
        first_entity_index,
      } => trim_entity(
        document,
        target_id,
        cutter_id,
        *tolerance,
        *first_entity_index,
      ),
      Self::EditVertex { entity_id, edit } => edit_vertex(document, entity_id, *edit),
      Self::AddToolpath {
        toolpath_id,
        name,
        entity_ids,
        tool_number,
        operation,
      } => {
        // The order is the drawing commands' order, so a refusal names the first thing that was
        // wrong: something to cut, then an id nobody holds. What is left — an absent entity, an open
        // vector under a cutting operation — is the document's own rule and is refused by
        // validation, which is why this variant adds no second spelling of it.
        require_selection(entity_ids)?;
        ensure_toolpath_id_is_free(document, toolpath_id)?;
        document.toolpaths.push(Toolpath {
          id: toolpath_id.clone(),
          name: name.clone(),
          entity_ids: entity_ids.clone(),
          tool_number: *tool_number,
          operation: operation.clone(),
        });
        Ok(CommandOutcome::default())
      }
      Self::SetToolpathToolNumber {
        toolpath_id,
        tool_number,
      } => {
        let index = toolpath_index(document, toolpath_id)?;
        document.toolpaths[index].tool_number = *tool_number;
        Ok(CommandOutcome::default())
      }
      Self::SetToolpathOperation {
        toolpath_id,
        operation,
      } => {
        let index = toolpath_index(document, toolpath_id)?;
        document.toolpaths[index].operation = operation.clone();
        Ok(CommandOutcome::default())
      }
      Self::SetToolpathName { toolpath_id, name } => {
        // The toolpath is resolved first, the way every other toolpath command resolves its target, so an id
        // the document does not hold is refused by name rather than by a complaint about the name. The name
        // check comes before the assignment, so a refusal leaves the document exactly as it was.
        let index = toolpath_index(document, toolpath_id)?;
        if name.trim().is_empty() {
          return Err(CommandError::EmptyToolpathName);
        }
        document.toolpaths[index].name = name.clone();
        Ok(CommandOutcome::default())
      }
      Self::DeleteToolpath { toolpath_id } => {
        let index = toolpath_index(document, toolpath_id)?;
        document.toolpaths.remove(index);
        Ok(CommandOutcome::default())
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
/// A single-entity drawing command reports nothing; an ellipse reports the radial error its arc
/// approximation actually achieved (M2 A3), and a batch command may report the entities it created,
/// so the shell can surface both instead of the kernel swallowing them.
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
  /// The target toolpath does not exist.
  #[error("toolpath {0:?} does not exist")]
  UnknownToolpath(String),
  /// A toolpath with that identifier already exists.
  #[error("toolpath {0:?} already exists")]
  DuplicateToolpath(String),
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
  /// The toolpath's new name was empty or only whitespace, so there is nothing to show and nothing to undo.
  ///
  /// A command that changes nothing must not record an undo step: the user would press undo once and see the
  /// document change for a name that never did.
  #[error("a toolpath's name must not be empty")]
  EmptyToolpathName,
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

/// The index of the toolpath a command targets, refusing an id the document does not hold.
///
/// An index rather than a reference for the reason [`EntitySite`] is one: the command reads the
/// toolpath and then writes it back, and holding a borrow across that write is not possible.
fn toolpath_index(document: &Document, toolpath_id: &str) -> Result<usize, CommandError> {
  document
    .toolpaths
    .iter()
    .position(|toolpath| toolpath.id == toolpath_id)
    .ok_or_else(|| CommandError::UnknownToolpath(toolpath_id.to_owned()))
}

/// Refuse a toolpath id the document already holds.
///
/// The same rule [`ensure_id_is_free`] applies to entities, and the same reason: `Document::validate`
/// refuses a duplicate toolpath id, and a command that reported success while leaving a document its
/// own validation rejects would be lying to the caller that applies it directly.
fn ensure_toolpath_id_is_free(document: &Document, toolpath_id: &str) -> Result<(), CommandError> {
  if document
    .toolpaths
    .iter()
    .any(|toolpath| toolpath.id == toolpath_id)
  {
    return Err(CommandError::DuplicateToolpath(toolpath_id.to_owned()));
  }
  Ok(())
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
      require_positive("scale tolerance", tolerance.chord)?;
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
  let mut minted = 0;
  for row in 0..rows {
    for column in 0..columns {
      let delta = Vec2::new(column as f64 * dx, row as f64 * dy);
      for (site, contour) in &sources {
        let id = minted_entity_id(first_entity_index, minted);
        minted += 1;
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

/// The `offset`th id a batch command mints from `first_entity_index`.
///
/// The index is used literally (D-M2-4): `0` mints `E0`, because an id is an opaque string and the
/// guarantee that matters is that every minted id is free. Callers check that for the whole batch
/// before storing anything, which is what keeps a refusal atomic.
fn minted_entity_id(first_entity_index: u32, offset: usize) -> String {
  format!("E{}", u64::from(first_entity_index) + offset as u64)
}

/// Resolve one id to the site where its entity lives.
fn resolve_site(document: &Document, entity_id: &str) -> Result<EntitySite, CommandError> {
  let ids = [entity_id.to_owned()];
  Ok(resolve_sites(document, &ids)?[0])
}

/// Remove the entity with `entity_id` from whichever layer holds it.
///
/// Every command that drops entities goes through here, so "the entity is gone" means the same
/// thing everywhere: the layer stays, the id becomes free, and the entity is not silently skipped.
fn remove_entity(document: &mut Document, entity_id: &str) -> Result<(), CommandError> {
  for layer in &mut document.layers {
    if let Some(index) = layer
      .entities
      .iter()
      .position(|entity| entity.id == entity_id)
    {
      layer.entities.remove(index);
      return Ok(());
    }
  }
  Err(CommandError::UnknownEntity(entity_id.to_owned()))
}

/// Offset one entity and store the result as a fresh entity on the source's layer.
fn offset_entity(
  document: &mut Document,
  entity_id: &str,
  distance: f64,
  first_entity_index: u32,
) -> Result<CommandOutcome, CommandError> {
  require_finite("offset distance", distance)?;
  let site = resolve_site(document, entity_id)?;
  let source = contour_at(document, site).clone();
  let id = minted_entity_id(first_entity_index, 0);
  ensure_id_is_free(document, &id)?;

  // The kernel owns the geometry (D-M2-3) and its refusal names what collapsed: that message is
  // the command's message, so "the offset passed through the shape" reaches the caller intact.
  let offset =
    offset_contour(&source, distance, &OffsetOptions::standard()).map_err(geometry_error)?;
  let layer_id = document.layers[site.0].id.clone();
  push_entity(document, &layer_id, &id, offset.contour)?;
  Ok(CommandOutcome {
    created_entities: vec![id],
    ..CommandOutcome::default()
  })
}

/// Join a selection into one contour, which replaces the first selected entity.
fn join_entities(
  document: &mut Document,
  entity_ids: &[String],
  tolerance: TolerancePolicy,
) -> Result<CommandOutcome, CommandError> {
  require_positive("join tolerance", tolerance.epsilon)?;
  require_selection(entity_ids)?;
  let sites = resolve_sites(document, entity_ids)?;
  let contours: Vec<Contour> = sites
    .iter()
    .map(|site| contour_at(document, *site).clone())
    .collect();

  // The kernel builds and validates the whole chain before anything is written, so an ambiguous
  // group refuses with nothing removed and nothing replaced.
  let joined = join(&contours, tolerance).map_err(geometry_error)?;

  replace_contour_at(document, sites[0], joined);
  for id in &entity_ids[1..] {
    remove_entity(document, id)?;
  }
  Ok(CommandOutcome::default())
}

/// Trim one entity with another and store the surviving pieces as fresh entities.
fn trim_entity(
  document: &mut Document,
  target_id: &str,
  cutter_id: &str,
  tolerance: TolerancePolicy,
  first_entity_index: u32,
) -> Result<CommandOutcome, CommandError> {
  require_positive("trim tolerance", tolerance.epsilon)?;
  let target_site = resolve_site(document, target_id)?;
  let cutter_site = resolve_site(document, cutter_id)?;
  let target = contour_at(document, target_site).clone();
  let cutter = contour_at(document, cutter_site).clone();

  let trimmed = trim(&target, &cutter, tolerance).map_err(geometry_error)?;

  // Every id the batch needs is checked before the target is removed, so an id taken late in the
  // batch refuses the trim instead of leaving a target deleted and half of its pieces stored.
  let ids: Vec<String> = (0..trimmed.kept.len())
    .map(|offset| minted_entity_id(first_entity_index, offset))
    .collect();
  for id in &ids {
    ensure_id_is_free(document, id)?;
  }

  let layer_id = document.layers[target_site.0].id.clone();
  remove_entity(document, target_id)?;
  for (id, piece) in ids.iter().zip(trimmed.kept) {
    push_entity(document, &layer_id, id, piece)?;
  }
  Ok(CommandOutcome {
    created_entities: ids,
    ..CommandOutcome::default()
  })
}

/// Apply one node edit to one entity's contour.
fn edit_vertex(
  document: &mut Document,
  entity_id: &str,
  edit: VertexEdit,
) -> Result<CommandOutcome, CommandError> {
  let site = resolve_site(document, entity_id)?;
  let contour = contour_at(document, site);
  let edited = match edit {
    VertexEdit::Move { index, to } => {
      require_finite_point("vertex position", to)?;
      move_vertex(contour, index, to)
    }
    VertexEdit::Insert { segment_index, at } => {
      require_finite_point("insertion point", at)?;
      insert_vertex(contour, segment_index, at)
    }
    VertexEdit::Delete { index } => delete_vertex(contour, index),
  }
  .map_err(geometry_error)?;
  replace_contour_at(document, site, edited);
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
