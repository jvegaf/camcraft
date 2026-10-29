//! Document model: the job, its layers of geometry, and the toolpaths assigned to it.
//!
//! # Schema
//!
//! A document carries an explicit schema tag, [`DOCUMENT_SCHEMA`]. Anything else is rejected with a
//! message naming both the found and the supported version (A3), because a CAM document that is
//! silently half-understood produces wrong G-code, which is the one failure this application must
//! never have.
//!
//! New geometry or toolpath kinds are a schema-version event, not an additive tweak: `EntityKind`
//! and `Operation` are internally tagged enums, so an unknown tag is a hard error rather than a
//! silently ignored record.
//!
//! Must not depend on `cam-store`, `cam-post` or any UI crate.

#![warn(missing_docs)]

use cam_geom::Contour;
use cam_units::{Length, UnitSystem};
use serde::{Deserialize, Serialize};

/// The only document schema this build understands.
pub const DOCUMENT_SCHEMA: &str = "c2d/document@1";

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

/// The machining operation of a toolpath.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Operation {
  /// Cut along the selected closed vectors.
  Profile(ProfileParams),
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
  /// A profile toolpath was assigned an open contour.
  #[error("toolpath {toolpath:?} needs closed geometry, entity {entity:?} is open")]
  OpenContourNotCuttable {
    /// The toolpath id.
    toolpath: String,
    /// The entity id.
    entity: String,
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
      if !(value > 0.0) || !value.is_finite() {
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
        // One operation kind exists in M0; the match keeps this a compile error to update when
        // a second kind lands, instead of silently skipping its rules.
        match toolpath.operation {
          Operation::Profile(_) => {
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
      }
    }

    Ok(())
  }
}
