//! Document model: the job, its layers of geometry, and the toolpaths assigned to it.
//!
//! # Schema
//!
//! A document carries an explicit schema tag, [`DOCUMENT_SCHEMA`]. Anything else is rejected with a
//! message naming both the found and the supported version (A3), because a CAM document that is
//! silently half-understood produces wrong G-code, which is the one failure this application must
//! never have.
//!
//! `EntityKind` and `Operation` are internally tagged enums, so an unknown tag is a hard error rather
//! than a silently ignored record: a document written by a newer build never loads half-understood.
//!
//! Because of that, **adding a variant is additive and keeps the schema tag** (decision D-M4-1): the
//! tag protects the meaning of existing fields, and an unknown `kind` already fails loudly. Changing
//! what an existing field means, or its units, is what makes a new tag.
//!
//! Must not depend on `cam-store`, `cam-post` or any UI crate.

#![warn(missing_docs)]

use cam_geom::Contour;
use cam_units::{Angle, Length, UnitSystem};
use serde::{Deserialize, Serialize};

/// The document schema this build writes.
pub const DOCUMENT_SCHEMA: &str = "c2d/document@2";

/// Schema tags this build still reads, newest first.
///
/// `@2` added `Toolpath::island_ids`. Nothing else changed, so reading an `@1` document is not a
/// transformation: the field is simply absent and therefore empty. The bump exists because the
/// alternative — keeping `@1` and adding an optional field — is the exactly wrong behaviour here: an
/// older build ignores a field it does not know, so it would pocket straight through an island the user
/// asked to keep, and a ruined part is far worse than a refusal (D-RB-3's analogue for the schema,
/// A10).
pub const LEGACY_DOCUMENT_SCHEMAS: &[&str] = &["c2d/document@1"];

/// Which corner of the job the machine's XY origin sits on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OriginCorner {
  /// Origin at the bottom-left corner, Y increasing away from the operator.
  #[default]
  BottomLeft,
  /// Origin at the top-left corner, Y increasing towards the operator.
  TopLeft,
  /// Origin at the centre of the job.
  Center,
}

/// The stock and coordinate setup of the job.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Job {
  /// Stock size along X.
  pub width: Length,
  /// Stock size along Y.
  pub height: Length,
  /// Stock thickness, which bounds the maximum cut depth.
  pub thickness: Length,
  /// Unit system the user works in. Metadata only: stored geometry is always millimetres.
  pub units: UnitSystem,
  /// Where the machine origin sits on the stock.
  pub origin: OriginCorner,
  /// Clearance height for rapids between cutting moves.
  pub safe_z: Length,
}

impl Job {
  /// A job with the analysed product's default setup: 1219 x 2438 x 18 mm, 5 mm safe Z.
  pub fn default_sheet() -> Self {
    Self {
      width: Length::from_mm(1219.0),
      height: Length::from_mm(2438.0),
      thickness: Length::from_mm(18.0),
      units: UnitSystem::Mm,
      origin: OriginCorner::BottomLeft,
      safe_z: Length::from_mm(5.0),
    }
  }
}

/// A named, independently switchable group of geometry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layer {
  /// Stable identifier, referenced by toolpaths.
  pub id: String,
  /// Display name.
  pub name: String,
  /// Whether the layer is shown and cuttable by default.
  pub visible: bool,
  /// The geometry in this layer.
  pub entities: Vec<Entity>,
}

/// One piece of geometry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entity {
  /// Stable identifier, unique across the whole document.
  pub id: String,
  /// What the entity is.
  #[serde(flatten)]
  pub kind: EntityKind,
}

/// The geometry kinds a document can hold.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EntityKind {
  /// A chain of straight and circular segments.
  Contour {
    /// The contour itself, validated on read.
    contour: Contour,
  },
}

impl EntityKind {
  /// The contour, when the entity is one.
  pub fn as_contour(&self) -> Option<&Contour> {
    match self {
      Self::Contour { contour } => Some(contour),
    }
  }
}

/// Which side of the selected vectors the tool runs on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileSide {
  /// Cut inside the selected closed vector.
  Inside,
  /// Cut outside the selected closed vector.
  #[default]
  Outside,
  /// Cut directly on the vector.
  On,
}

/// Which clearing pattern a pocket uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PocketStrategy {
  /// Parallel rows across the region.
  Raster,
  /// Concentric rings following the boundary.
  #[default]
  Offset,
}

/// Parameters of a pocket toolpath.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PocketParams {
  /// How the region is cleared.
  pub strategy: PocketStrategy,
  /// Material left on the wall, measured from the vector.
  pub allowance: Length,
  /// Distance between adjacent rings or raster rows.
  pub stepover: Length,
  /// Depth of material removed per pass.
  pub pass_depth: Length,
  /// Depth at which cutting starts, for a part already partly machined.
  pub start_depth: Length,
  /// Total cut depth below the stock surface.
  pub cut_depth: Length,
}

impl Default for PocketParams {
  fn default() -> Self {
    Self {
      strategy: PocketStrategy::Offset,
      allowance: Length::ZERO,
      stepover: Length::from_mm(3.0),
      pass_depth: Length::from_mm(6.0),
      start_depth: Length::ZERO,
      cut_depth: Length::from_mm(18.0),
    }
  }
}

/// How far the tool comes back up between pecks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum RetractMode {
  /// Lift by a fixed distance above the current depth.
  Amount {
    /// How far above the bottom of the current peck the tool returns.
    amount: Length,
  },
  /// Lift all the way to the job's safe Z, clearing the hole completely.
  #[default]
  Full,
}

/// A gang of drills on one head, spaced a fixed pitch apart along X.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DrillBank {
  /// How many drills the head carries.
  pub count: u32,
  /// Distance between neighbouring drills along X.
  pub pitch: Length,
}

/// Parameters of a drilling toolpath.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DrillParams {
  /// Depth of material removed per peck. Zero cuts the hole in one plunge.
  pub peck_depth: Length,
  /// How far the tool retracts between pecks.
  pub retract: RetractMode,
  /// Depth at which cutting starts, for a part already partly machined.
  pub start_depth: Length,
  /// Total cut depth below the stock surface.
  pub cut_depth: Length,
  /// The head's drill gang, when the machine has one.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub bank: Option<DrillBank>,
}

impl Default for DrillParams {
  fn default() -> Self {
    Self {
      peck_depth: Length::ZERO,
      retract: RetractMode::Full,
      start_depth: Length::ZERO,
      cut_depth: Length::from_mm(18.0),
      bank: None,
    }
  }
}

/// A holding tab that keeps the part attached to the stock.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tab {
  /// Position along the contour, as a fraction of its perimeter in `[0, 1)`.
  pub position: f64,
  /// Tab length along the contour.
  pub width: Length,
  /// How far the tool lifts over the tab.
  pub height: Length,
}

/// Parameters of a profile (contour) toolpath.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProfileParams {
  /// Which side of the vector the tool runs on.
  pub side: ProfileSide,
  /// Extra material left outside the tool (positive) or removed beyond it (negative).
  pub allowance: Length,
  /// Depth of material removed per pass.
  pub pass_depth: Length,
  /// Depth at which cutting starts, for a part already partly machined.
  pub start_depth: Length,
  /// Total cut depth below the stock surface.
  pub cut_depth: Length,
  /// Straight lead-in length before the tool reaches the contour.
  pub lead_in: Length,
  /// Straight lead-out length after the contour is complete.
  pub lead_out: Length,
  /// Holding tabs, empty for none.
  pub tabs: Vec<Tab>,
}

impl Default for ProfileParams {
  fn default() -> Self {
    Self {
      side: ProfileSide::Outside,
      allowance: Length::ZERO,
      pass_depth: Length::from_mm(6.0),
      start_depth: Length::ZERO,
      cut_depth: Length::from_mm(18.0),
      lead_in: Length::from_mm(3.0),
      lead_out: Length::from_mm(3.0),
      tabs: Vec::new(),
    }
  }
}

/// Parameters of a chamfer (V-bit bevel) toolpath.
///
/// A chamfer's depth is **derived, never authored** (D-M6-2 as amended in L7): the bevel's width and
/// the bit's included angle fix how deep the tip has to go for the flank to meet the surface exactly
/// `chamfer_width` millimetres from the vector. There is no `side`: the tip runs on the vector and the
/// bevel is symmetric about it, so the same bevel comes out whichever way the tool travels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChamferParams {
  /// Horizontal width of the bevel, from the vector to where the flank meets the surface.
  pub chamfer_width: Length,
  /// Full included angle of the V-bit, as the tool library records it (S11).
  pub included_angle: Angle,
  /// Depth of the surface the bevel starts from, for a part already partly machined.
  pub start_depth: Length,
}

impl ChamferParams {
  /// The one depth the tip reaches: `start_depth + chamfer_width / tan(included_angle / 2)`.
  ///
  /// Derived here and nowhere else, so the toolpath that cuts and the simulation that checks it can
  /// never disagree about how deep the bevel is (D-M6-2 as amended in L7). The formula does not clamp:
  /// a width or angle with no bevel behind it is refused by [`Document::validate`] naming the value,
  /// because clamping it into a plausible-looking bevel would cut a different chamfer than the one
  /// asked for.
  pub fn depth(&self) -> Length {
    let half = self.included_angle.radians() / 2.0;
    Length::from_mm(self.start_depth.mm() + self.chamfer_width.mm() / half.tan())
  }
}

impl Default for ChamferParams {
  fn default() -> Self {
    Self {
      chamfer_width: Length::from_mm(6.0),
      included_angle: Angle::from_degrees(90.0),
      start_depth: Length::ZERO,
    }
  }
}

/// Parameters of a quick engrave toolpath.
///
/// One pass at one authored depth, with the tool's centre **on** the vector (D-M6-4). There is no
/// side, no pass law, no lead and no tab: a second pass would cut a second groove into the first, and
/// the feature is a single groove. The depth is authored here rather than derived, which is what
/// separates an engrave from a chamfer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EngraveParams {
  /// Depth at which cutting starts, for a part already partly machined.
  pub start_depth: Length,
  /// Total cut depth below the stock surface; the one pass lands exactly on it.
  pub cut_depth: Length,
}

impl Default for EngraveParams {
  fn default() -> Self {
    Self {
      start_depth: Length::ZERO,
      cut_depth: Length::from_mm(18.0),
    }
  }
}

/// The machining operation of a toolpath.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Operation {
  /// Cut along the selected closed vectors.
  Profile(ProfileParams),
  /// Clear the area inside the selected closed vectors.
  Pocket(PocketParams),
  /// Plunge at the centre of each selected circle.
  Drilling(DrillParams),
  /// Bevel the selected closed vectors with a V-bit, at a depth derived from the bit.
  Chamfer(ChamferParams),
  /// Engrave the selected vectors, open or closed, in one pass at an authored depth.
  Engrave(EngraveParams),
}

/// A machining operation bound to geometry and a tool.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Toolpath {
  /// Stable identifier.
  pub id: String,
  /// Display name.
  pub name: String,
  /// Entities the toolpath is computed from.
  pub entity_ids: Vec<String>,
  /// Closed contours inside the region that must be left standing, such as a pocket's islands.
  ///
  /// Explicit, never inferred: a contour drawn inside another may be an island or may simply be a
  /// separate feature, and guessing would reverse the user's intent (D-RB-1). Islands may be
  /// **nested**: an island strictly inside another is legal and is how a boss inside a hole is
  /// expressed, since the region's parity rule turns a boundary at an odd nesting depth back into
  /// material (D-RB-2). An island pair that crosses or touches is refused instead. Omitted when empty,
  /// so a document without islands is byte-identical to one written before they existed.
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub island_ids: Vec<String>,
  /// Tool number, matched against the tool library at emission time.
  pub tool_number: u32,
  /// The operation and its parameters.
  pub operation: Operation,
}

/// A complete document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
  /// Schema tag. Must equal [`DOCUMENT_SCHEMA`].
  pub schema: String,
  /// Stock and coordinate setup.
  pub job: Job,
  /// Layers of geometry.
  pub layers: Vec<Layer>,
  /// Toolpaths.
  pub toolpaths: Vec<Toolpath>,
  /// Optional tool library filename, resolved by the store.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub tools_ref: Option<String>,
  /// Optional post-processor filename, resolved by the store.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub postp: Option<String>,
}

/// Why a document was rejected.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ModelError {
  /// The document's schema tag is not the one this build writes and reads.
  #[error("unsupported document schema {found:?}; this build supports {supported:?}")]
  UnsupportedSchema {
    /// Tag found in the document.
    found: String,
    /// Tag this build supports.
    supported: String,
  },
  /// Two records share an identifier.
  #[error("duplicate {kind} id {id:?}")]
  DuplicateId {
    /// What kind of record duplicated.
    kind: &'static str,
    /// The duplicated id.
    id: String,
  },
  /// A toolpath cites geometry that does not exist.
  #[error("toolpath {toolpath:?} references unknown entity {entity:?}")]
  UnknownEntity {
    /// The toolpath id.
    toolpath: String,
    /// The missing entity id.
    entity: String,
  },
  /// A job dimension is zero or negative.
  #[error("job {field} must be positive, got {value} mm")]
  NonPositiveJobDimension {
    /// Which dimension.
    field: &'static str,
    /// The offending value in millimetres.
    value: f64,
  },
  /// Geometry stored in the document failed the kernel's validation.
  #[error("invalid geometry: {0}")]
  InvalidGeometry(String),
  /// A toolpath names an island that is not a closed contour.
  #[error("toolpath {toolpath:?} island {island:?} is not a closed contour")]
  IslandNotClosed {
    /// The toolpath id.
    toolpath: String,
    /// The island entity id.
    island: String,
  },
  /// An island is not strictly inside any of the toolpath's boundaries.
  #[error("toolpath {toolpath:?} island {island:?} is not inside any of its boundaries")]
  IslandOutsideBoundary {
    /// The toolpath id.
    toolpath: String,
    /// The island entity id.
    island: String,
  },
  /// Two islands of the same toolpath overlap or touch.
  ///
  /// Nesting is not an overlap: an island strictly inside another is legal, and is how a boss inside
  /// a hole is expressed — the region's parity rule makes a boundary at an odd nesting depth
  /// material again (D-RB-2). What is refused is an island pair whose boundaries **cross** or
  /// **touch**, because neither leaves a region the parity rule can describe.
  #[error("toolpath {toolpath:?} islands {first:?} and {second:?} overlap or touch")]
  IslandsOverlap {
    /// The toolpath id.
    toolpath: String,
    /// The first island entity id.
    first: String,
    /// The second island entity id.
    second: String,
  },
  /// Islands were given to an operation that does not clear a region.
  #[error("toolpath {toolpath:?} takes islands only for a pocket")]
  IslandsRequirePocket {
    /// The toolpath id.
    toolpath: String,
  },
  /// A cutting toolpath was assigned an open contour.
  #[error("toolpath {toolpath:?} needs closed geometry, entity {entity:?} is open")]
  OpenContourNotCuttable {
    /// The toolpath id.
    toolpath: String,
    /// The entity id.
    entity: String,
  },
  /// A chamfer's bevel width is zero, negative or not a finite number.
  #[error(
    "toolpath {toolpath:?} chamfer width must be a positive number of millimetres, got {value} mm"
  )]
  ChamferWidthNotPositive {
    /// The toolpath id.
    toolpath: String,
    /// The offending width in millimetres.
    value: f64,
  },
  /// A chamfer's included angle is not strictly between 0 and 180 degrees.
  #[error(
    "toolpath {toolpath:?} chamfer included angle must be between 0 and 180 degrees, got {degrees} \
     degrees"
  )]
  ChamferAngleOutOfRange {
    /// The toolpath id.
    toolpath: String,
    /// The offending angle in degrees.
    degrees: f64,
  },
}

impl Document {
  /// A new, empty document for `job`.
  pub fn new(job: Job) -> Self {
    Self {
      schema: DOCUMENT_SCHEMA.to_owned(),
      job,
      layers: Vec::new(),
      toolpaths: Vec::new(),
      tools_ref: None,
      postp: None,
    }
  }

  /// Every entity in the document, whatever layer holds it.
  pub fn entities(&self) -> impl Iterator<Item = &Entity> {
    self.layers.iter().flat_map(|layer| layer.entities.iter())
  }

  /// Look an entity up by id.
  pub fn entity(&self, id: &str) -> Option<&Entity> {
    self.entities().find(|entity| entity.id == id)
  }

  /// Rewrite a schema tag this build still reads into the one it writes.
  ///
  /// Returns true when the document was written by an older build, so a caller can report it. The
  /// migration is an identity: `@1` only lacks `island_ids`, which is absent-and-therefore-empty.
  pub fn migrate(&mut self) -> bool {
    if LEGACY_DOCUMENT_SCHEMAS.contains(&self.schema.as_str()) {
      self.schema = DOCUMENT_SCHEMA.to_owned();
      return true;
    }
    false
  }

  /// Check the document's schema tag and every referential invariant.
  pub fn validate(&self) -> Result<(), ModelError> {
    if self.schema != DOCUMENT_SCHEMA {
      return Err(ModelError::UnsupportedSchema {
        found: self.schema.clone(),
        supported: DOCUMENT_SCHEMA.to_owned(),
      });
    }

    for (field, value) in [
      ("width", self.job.width.mm()),
      ("height", self.job.height.mm()),
      ("thickness", self.job.thickness.mm()),
    ] {
      if value <= 0.0 || !value.is_finite() {
        return Err(ModelError::NonPositiveJobDimension { field, value });
      }
    }

    let mut layer_ids = std::collections::BTreeSet::new();
    for layer in &self.layers {
      if !layer_ids.insert(layer.id.as_str()) {
        return Err(ModelError::DuplicateId {
          kind: "layer",
          id: layer.id.clone(),
        });
      }
    }

    let mut entity_ids = std::collections::BTreeSet::new();
    for entity in self.entities() {
      if !entity_ids.insert(entity.id.as_str()) {
        return Err(ModelError::DuplicateId {
          kind: "entity",
          id: entity.id.clone(),
        });
      }
    }

    let mut toolpath_ids = std::collections::BTreeSet::new();
    for toolpath in &self.toolpaths {
      if !toolpath_ids.insert(toolpath.id.as_str()) {
        return Err(ModelError::DuplicateId {
          kind: "toolpath",
          id: toolpath.id.clone(),
        });
      }
      for entity_id in &toolpath.entity_ids {
        let entity = self
          .entity(entity_id)
          .ok_or_else(|| ModelError::UnknownEntity {
            toolpath: toolpath.id.clone(),
            entity: entity_id.clone(),
          })?;
        // Listing the kinds explicitly keeps this a compile error to update when another one
        // lands, instead of silently skipping its rules (D-M4-1).
        let needs_closed_geometry = match toolpath.operation {
          Operation::Profile(_)
          | Operation::Pocket(_)
          | Operation::Drilling(_)
          | Operation::Chamfer(_) => true,
          // Engraving an open vector is the strategy's whole point (M6 A3, D-M6-4): it cuts along the
          // vector and stops at its last vertex, so an open one is legal here and only here.
          Operation::Engrave(_) => false,
        };
        if needs_closed_geometry {
          let closed = entity
            .kind
            .as_contour()
            .map(cam_geom::Contour::is_closed)
            .unwrap_or(false);
          if !closed {
            return Err(ModelError::OpenContourNotCuttable {
              toolpath: toolpath.id.clone(),
              entity: entity_id.clone(),
            });
          }
        }
      }
      check_chamfer(toolpath)?;
      check_islands(self, toolpath)?;
    }

    Ok(())
  }
}

/// Check a chamfer's own parameters: a bevel needs a width and an angle that describe one.
///
/// The depth is derived from the two (D-M6-2 as amended in L7), so a zero width has no bevel behind
/// it and an angle at either end of `(0, 180)` degrees has a tangent of zero or infinity. Either
/// would emit an infinite Z, so both are refused naming the offending value rather than clamped.
fn check_chamfer(toolpath: &Toolpath) -> Result<(), ModelError> {
  let Operation::Chamfer(params) = &toolpath.operation else {
    return Ok(());
  };
  let width = params.chamfer_width.mm();
  if !width.is_finite() || width <= 0.0 {
    return Err(ModelError::ChamferWidthNotPositive {
      toolpath: toolpath.id.clone(),
      value: width,
    });
  }
  let angle = params.included_angle.radians();
  if !(angle > 0.0 && angle < std::f64::consts::PI) {
    return Err(ModelError::ChamferAngleOutOfRange {
      toolpath: toolpath.id.clone(),
      degrees: params.included_angle.degrees(),
    });
  }
  Ok(())
}

/// Check a toolpath's islands: closed, strictly inside a boundary, and never crossing or touching
/// each other.
///
/// Nested islands are legal, unlike crossing or touching ones: a boss inside a hole is an island
/// inside an island, and the region's parity rule is what makes it material again (D-RB-2).
fn check_islands(document: &Document, toolpath: &Toolpath) -> Result<(), ModelError> {
  if toolpath.island_ids.is_empty() {
    return Ok(());
  }
  if !matches!(toolpath.operation, Operation::Pocket(_)) {
    return Err(ModelError::IslandsRequirePocket {
      toolpath: toolpath.id.clone(),
    });
  }

  let mut seen = std::collections::BTreeSet::new();
  for island_id in &toolpath.island_ids {
    if !seen.insert(island_id.as_str()) {
      return Err(ModelError::DuplicateId {
        kind: "island",
        id: island_id.clone(),
      });
    }
  }

  let tolerance = cam_geom::DEFAULT_TOLERANCE;
  let mut islands = Vec::with_capacity(toolpath.island_ids.len());
  for island_id in &toolpath.island_ids {
    let entity = document
      .entity(island_id)
      .ok_or_else(|| ModelError::UnknownEntity {
        toolpath: toolpath.id.clone(),
        entity: island_id.clone(),
      })?;
    let island = entity
      .kind
      .as_contour()
      .filter(|contour| contour.is_closed())
      .ok_or_else(|| ModelError::IslandNotClosed {
        toolpath: toolpath.id.clone(),
        island: island_id.clone(),
      })?;

    // The island has to sit strictly inside one of the boundaries. "Inside" is the exact winding
    // question, not a bounds test: a contour in a C-shaped pocket's notch is inside its bounds and
    // outside the material.
    let inside = toolpath.entity_ids.iter().any(|boundary_id| {
      self_boundary(document, boundary_id).is_some_and(|boundary| {
        // Asked island-first on purpose: `relation` is directional, and the pocket's boundary holds
        // the island rather than the other way round.
        matches!(
          cam_geom::winding::relation(island, boundary, tolerance),
          cam_geom::winding::Relation::FirstInside
        )
      })
    });
    if !inside {
      return Err(ModelError::IslandOutsideBoundary {
        toolpath: toolpath.id.clone(),
        island: island_id.clone(),
      });
    }
    islands.push((island_id, island));
  }

  for (index, (first_id, first)) in islands.iter().enumerate() {
    for (second_id, second) in &islands[index + 1..] {
      // Nesting is allowed and is the boss case: `SecondInside` means `second` sits strictly within
      // `first`, and parity makes the inner boundary material again (D-RB-2). Crossing and touching
      // are refused by name instead, because where two islands intersect the material between them
      // is not a region any parity describes, and where they touch the wall between them has no width.
      if matches!(
        cam_geom::winding::relation(first, second, tolerance),
        cam_geom::winding::Relation::Crossing | cam_geom::winding::Relation::Touching
      ) {
        return Err(ModelError::IslandsOverlap {
          toolpath: toolpath.id.clone(),
          first: (*first_id).clone(),
          second: (*second_id).clone(),
        });
      }
    }
  }

  Ok(())
}

/// The contour of an entity, when the entity is one.
fn self_boundary<'a>(document: &'a Document, id: &str) -> Option<&'a cam_geom::Contour> {
  document.entity(id)?.kind.as_contour()
}
