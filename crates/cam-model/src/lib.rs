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

use cam_geom::{Bounds, Contour, Point2};
use cam_units::{Angle, Length};
use serde::{Deserialize, Serialize};

/// The document schema this build writes.
pub const DOCUMENT_SCHEMA: &str = "camcraft/document@1";

/// Which corner of the board the machine's XY origin sits on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OriginCorner {
  /// Origin at the bottom-left corner, Y increasing away from the operator.
  #[default]
  BottomLeft,
  /// Origin at the top-left corner, Y increasing towards the operator.
  TopLeft,
  /// Origin at the centre of the board.
  Center,
}

/// Which side of the board faces up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoardSideUp {
  /// The top side faces up.
  #[default]
  Top,
}

/// The document frame's relation to the machine frame, for one board.
///
/// A document's geometry runs from the board's own corner, `(0, 0)` to `(width, height)`, while a
/// program's moves are in machine coordinates — the frame the selected [`OriginCorner`] puts the
/// machine's origin in. Two of the three corners make the frames differ, and both differences are exact
/// isometries: `center` translates the board so its centre is the machine's origin, and `top_left`
/// reflects Y so distance grows towards the operator.
///
/// This is the **one** definition of that map. The toolpath layer emits with [`Frame::to_machine`], and
/// the simulation reads a program back into the document frame with [`Frame::from_machine`], so a
/// program cannot mean one place in G-code and another in the report.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
  corner: OriginCorner,
  width: f64,
  height: f64,
}

impl Frame {
  /// The frame of `board`, from its origin corner and its size.
  pub fn of(board: &Board) -> Self {
    Self {
      corner: board.origin,
      width: board.width.mm(),
      height: board.height.mm(),
    }
  }

  /// The corner the machine's XY origin sits on.
  pub fn corner(&self) -> OriginCorner {
    self.corner
  }

  /// A point in document coordinates, as the machine sees it.
  pub fn to_machine(&self, point: Point2) -> Point2 {
    match self.corner {
      OriginCorner::BottomLeft => point,
      OriginCorner::TopLeft => Point2::new(point.x, self.height - point.y),
      OriginCorner::Center => Point2::new(point.x - self.width / 2.0, point.y - self.height / 2.0),
    }
  }

  /// A point in machine coordinates, as the document sees it.
  ///
  /// The exact inverse of [`Frame::to_machine`].
  pub fn from_machine(&self, point: Point2) -> Point2 {
    match self.corner {
      OriginCorner::BottomLeft => point,
      OriginCorner::TopLeft => Point2::new(point.x, self.height - point.y),
      OriginCorner::Center => Point2::new(point.x + self.width / 2.0, point.y + self.height / 2.0),
    }
  }

  /// Whether the two frames are the same one, in which case every map is the identity.
  pub fn is_identity(&self) -> bool {
    self.corner == OriginCorner::BottomLeft
  }

  /// Whether the map is a mirror, which reverses the sense of every arc it carries.
  pub fn is_reflection(&self) -> bool {
    self.corner == OriginCorner::TopLeft
  }
}

/// The physical board the document describes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Board {
  /// Board size along X.
  pub width: Length,
  /// Board size along Y.
  pub height: Length,
  /// Board thickness.
  pub thickness: Length,
  /// Where the machine origin sits on the board.
  pub origin: OriginCorner,
  /// Which side of the board faces up.
  pub side_up: BoardSideUp,
}

impl Board {
  /// This board's document frame against the machine frame.
  pub fn frame(&self) -> Frame {
    Frame::of(self)
  }

  /// A board with a default size and origin.
  pub fn default_board() -> Self {
    Self {
      width: Length::from_mm(1219.0),
      height: Length::from_mm(2438.0),
      thickness: Length::from_mm(18.0),
      origin: OriginCorner::BottomLeft,
      side_up: BoardSideUp::Top,
    }
  }

  /// Compatibility alias for the former default job.
  pub fn default_sheet() -> Self {
    Self::default_board()
  }

  /// The board's outline as a closed contour in document coordinates, from `(0, 0)` to
  /// `(width, height)`.
  pub fn outline(&self) -> Contour {
    Contour::from_points(
      &[
        Point2::new(0.0, 0.0),
        Point2::new(self.width.mm(), 0.0),
        Point2::new(self.width.mm(), self.height.mm()),
        Point2::new(0.0, self.height.mm()),
      ],
      true,
    )
    .expect("validated board dimensions describe a closed rectangle")
  }
}

/// Default safe Z used when no board-level safe Z is stored.
pub const DEFAULT_SAFE_Z_MM: f64 = 5.0;

/// Compatibility alias for the former `Job` type.
pub type Job = Board;

/// The role a layer plays in a PCB document.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayerRole {
  /// Top copper side.
  CopperTop,
  /// Bottom copper side.
  CopperBottom,
  /// Solder mask.
  Mask,
  /// Silkscreen.
  Silk,
  /// Board edge, the outline cutout follows.
  #[default]
  Outline,
  /// Holes and slots.
  Drill,
  /// Areas the tool must not enter.
  Keepout,
}

/// Which side a layer belongs to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
  /// Top side.
  #[default]
  Top,
  /// Bottom side.
  Bottom,
  /// Both sides.
  Both,
}

/// Provenance of a layer's source data.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
  /// Source file name or path.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub file: Option<String>,
  /// Source format.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub format: Option<String>,
  /// Source date.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub date: Option<String>,
  /// Source content hash.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub hash: Option<String>,
}

/// A named, independently switchable group of geometry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layer {
  /// Stable identifier, referenced by toolpaths.
  pub id: String,
  /// What the layer represents in the board.
  pub role: LayerRole,
  /// Which side of the board the layer belongs to.
  pub side: Side,
  /// Display color as `#RRGGBB`.
  pub color: String,
  /// Whether the layer is shown and cuttable by default.
  pub visible: bool,
  /// The geometry in this layer.
  pub entities: Vec<Entity>,
  /// Where the layer's data came from.
  pub source: Provenance,
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

/// Direction a milling tool follows a contour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MillingDirection {
  /// Follow the contour with the cutter's rotation (default).
  #[default]
  Climb,
  /// Follow the contour against the cutter's rotation.
  Conventional,
}

/// How a non-copper or paint area is filled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FillMethod {
  /// Fill with a standard offset pattern (default).
  #[default]
  Standard,
  /// Fill by seeding from inside the area.
  Seed,
  /// Fill with parallel lines.
  Lines,
}

/// Which copper side an isolation cut follows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IsolationKind {
  /// Cut around the outside of copper features.
  Exterior,
  /// Cut around the inside of copper features.
  Interior,
  /// Cut both sides (default).
  #[default]
  Full,
}

/// Shape of the tabs that hold a cutout in place.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TabKind {
  /// A straight rectangular tab (default).
  #[default]
  Straight,
  /// A thin tab.
  Thin,
  /// Mouse-bite perforations.
  MouseBites,
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

/// Parameters of an isolation routing operation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IsolationParams {
  /// Number of isolation passes around each trace.
  pub passes: u32,
  /// Fraction of the tool diameter by which passes overlap, in `0.0..1.0`.
  pub overlap: f64,
  /// Milling direction.
  pub direction: MillingDirection,
  /// Which copper side to isolate.
  #[serde(rename = "isolation_kind")]
  pub kind: IsolationKind,
  /// Total cut depth below the surface.
  pub cut_depth: Length,
  /// Whether to keep copper pours.
  pub keep_pours: bool,
}

/// Parameters of a non-copper clearing operation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NonCopperClearParams {
  /// Fraction of the tool diameter by which passes overlap, in `0.0..1.0`.
  pub overlap: f64,
  /// Margin around the cleared area.
  pub margin: Length,
  /// How the area is filled.
  pub method: FillMethod,
  /// Whether to connect isolated cleared regions.
  pub connect: bool,
  /// Whether to follow the contour.
  pub contour: bool,
  /// Offset from copper; `None` means the clearing touches the copper.
  pub copper_offset: Option<Length>,
  /// Milling direction.
  pub direction: MillingDirection,
  /// Total cut depth below the surface.
  pub cut_depth: Length,
}

/// Parameters of a board cutout operation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CutoutParams {
  /// Margin around the board outline.
  pub margin: Length,
  /// Tabs that hold the board in place.
  pub tabs: Vec<Tab>,
  /// Shape of the tabs.
  pub tab_kind: TabKind,
  /// Depth removed per pass; zero means a single pass.
  pub pass_depth: Length,
  /// Total cut depth below the surface.
  pub cut_depth: Length,
  /// Whether to cut interior outlines.
  pub cut_interiors: bool,
}

/// Parameters of a solder-mask paint operation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PaintParams {
  /// Fraction of the tool diameter by which passes overlap, in `0.0..1.0`.
  pub overlap: f64,
  /// Offset applied to the painted area.
  pub offset: Length,
  /// How the area is filled.
  pub method: FillMethod,
  /// Whether to connect isolated painted regions.
  pub connect: bool,
  /// Whether to follow the contour.
  pub contour: bool,
  /// Total cut depth below the surface.
  pub cut_depth: Length,
}

/// Parameters of a panelization operation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PanelizeParams {
  /// Number of rows in the panel.
  pub rows: u32,
  /// Number of columns in the panel.
  pub columns: u32,
  /// Horizontal spacing between boards.
  pub spacing_x: Length,
  /// Vertical spacing between boards.
  pub spacing_y: Length,
  /// Optional maximum panel width constraint.
  pub constrain_x: Option<Length>,
  /// Optional maximum panel height constraint.
  pub constrain_y: Option<Length>,
}

/// The machining operation of a toolpath.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Operation {
  /// Route isolation paths around copper features.
  Isolation(IsolationParams),
  /// Clear non-copper areas of the board.
  NonCopperClear(NonCopperClearParams),
  /// Cut the board out of the stock.
  Cutout(CutoutParams),
  /// Mill a solder-mask paint pattern.
  Paint(PaintParams),
  /// Arrange multiple board copies into a panel.
  Panelize(PanelizeParams),
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
  /// The physical board the document describes.
  pub board: Board,
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
  /// Optional nesting section.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub nesting: Option<Nesting>,
}

/// The nesting section of a document (M9, S1-S4).
///
/// The user's four answers fix its shape: **one master document with N sheets and N instances** (S1),
/// the placements are **stored in the document** (S2), utilisation and waste are **both the report and
/// a stored field** (S3), and the sheets are **authored here rather than in a library** (S4).
///
/// Nothing here is geometry: a sheet is a size, a part is a selection of the document's own entities
/// plus how many copies are wanted, and a placement is where the nester put one copy. Where a
/// placement's material actually sits is one derivation, [`Document::placement_footprint`], and what
/// fraction of the used sheets it covers is another, [`Document::nesting_utilisation`], so a nester
/// certifying its own output, [`Document::validate`] refusing a bad record and the report printing one
/// can never disagree (invariant 2).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Nesting {
  /// The sheets the parts are nested on, authored in the document (S4).
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub sheets: Vec<NestingSheet>,
  /// The parts, each a selection of the document's own entities and an instance count (S1).
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub parts: Vec<NestingPart>,
  /// One entry per instance the nester placed (S2).
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub placements: Vec<NestingPlacement>,
  /// The cost record, absent until a nesting run has written it (S3).
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub cost: Option<NestingCost>,
}

impl Nesting {
  /// The sheet with this id.
  pub fn sheet(&self, id: &str) -> Option<&NestingSheet> {
    self.sheets.iter().find(|sheet| sheet.id == id)
  }

  /// The part with this id.
  pub fn part(&self, id: &str) -> Option<&NestingPart> {
    self.parts.iter().find(|part| part.id == id)
  }
}

/// One sheet the parts are nested on, authored in the document (S4).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NestingSheet {
  /// Stable identifier, referenced by placements.
  pub id: String,
  /// Sheet size along X; must be a positive finite length.
  pub width: Length,
  /// Sheet size along Y; must be a positive finite length.
  pub height: Length,
}

/// One part to nest: a selection of the document's own entities and how many copies are wanted (S1).
///
/// The selection is ids into this document, the `MouldingParams` precedent, so a part is not a second
/// copy of the geometry and cannot drift from it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NestingPart {
  /// Stable identifier, referenced by placements.
  pub id: String,
  /// Entities the part is cut from; at least one, each one present in the document.
  pub entity_ids: Vec<String>,
  /// How many copies of this part the nesting must place.
  pub instances: usize,
}

impl NestingPart {
  /// The bounding box of every entity this part is cut from, in document coordinates.
  ///
  /// `None` when the part names no entity, names one the document does not hold, or names an entity
  /// that is not a contour. [`Document::validate`] refuses the first two by name before any footprint
  /// is asked for; the third is the `?` on [`EntityKind::as_contour`] below. It is unconstructible
  /// while [`EntityKind`] has a single variant, and it is stated here so that a second kind cannot
  /// arrive as a silent third meaning of `None`.
  pub fn bounds(&self, document: &Document) -> Option<Bounds> {
    let mut bounds = Bounds::empty();
    for entity_id in &self.entity_ids {
      let entity_bounds = document.entity(entity_id)?.kind.as_contour()?.bounds();
      bounds = bounds.include(entity_bounds.min).include(entity_bounds.max);
    }
    self.entity_ids.first().map(|_| bounds)
  }
}

/// One instance on a sheet: which part, which sheet, where it sits and how it is turned (S2).
///
/// `position` is the sheet point the **placed** footprint's lower-left corner occupies, and
/// `rotation` is the turn the nester chose about the part's own footprint corner. Both are read by
/// [`Document::placement_footprint`] and nowhere else, so where the material sits has one definition.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NestingPlacement {
  /// The part this instance is a copy of.
  pub part_id: String,
  /// The sheet it is nested on.
  pub sheet_id: String,
  /// Where the placed footprint's lower-left corner sits on the sheet, in sheet coordinates.
  pub position: Point2,
  /// The turn the nester chose about the part's own footprint corner, counter-clockwise.
  pub rotation: Angle,
}

/// The cost record of a nesting run: what the report prints and what the document stores (S3).
///
/// The user asked for both, so the report and the field read the same two numbers rather than each
/// computing its own. Both are fractions of the used sheets' area, and the one arithmetic that
/// produces them is [`Document::nesting_utilisation`]: the record is written from it, refused against
/// it and printed from it, so the field and the placements cannot state two utilisations (invariant 2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NestingCost {
  /// How many sheets the run consumed.
  pub sheets_used: usize,
  /// Fraction of the used sheets' area covered by parts, between 0 and 1.
  pub utilisation: f64,
  /// Fraction of the used sheets' area left as waste, between 0 and 1.
  pub waste: f64,
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
  /// Two layers claim the same board role.
  #[error("two layers claim the role {role}; a board has one layer with that role")]
  DuplicateLayerRole {
    /// The role both claim.
    role: String,
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
  /// A cutting toolpath was assigned an open contour.
  #[error("toolpath {toolpath:?} needs closed geometry, entity {entity:?} is open")]
  OpenContourNotCuttable {
    /// The toolpath id.
    toolpath: String,
    /// The entity id.
    entity: String,
  },
  /// An isolation operation has an invalid parameter.
  #[error("toolpath {toolpath:?} isolation passes must be at least 1, got {passes}")]
  IsolationPassesTooSmall {
    /// The toolpath id.
    toolpath: String,
    /// The offending pass count.
    passes: u32,
  },
  /// An isolation operation has an invalid overlap.
  #[error(
    "toolpath {toolpath:?} isolation overlap must be a finite number between 0 and 1, got {overlap}"
  )]
  IsolationOverlapOutOfRange {
    /// The toolpath id.
    toolpath: String,
    /// The offending overlap.
    overlap: f64,
  },
  /// An isolation operation has a non-positive cut depth.
  #[error("toolpath {toolpath:?} isolation cut depth must be positive, got {cut_depth} mm")]
  IsolationCutDepthNotPositive {
    /// The toolpath id.
    toolpath: String,
    /// The offending cut depth in millimetres.
    cut_depth: f64,
  },
  /// A non-copper clearing operation has an invalid overlap.
  #[error(
    "toolpath {toolpath:?} non-copper clear overlap must be a finite number between 0 and 1, got {overlap}"
  )]
  NonCopperClearOverlapOutOfRange {
    /// The toolpath id.
    toolpath: String,
    /// The offending overlap.
    overlap: f64,
  },
  /// A non-copper clearing operation has a negative margin.
  #[error("toolpath {toolpath:?} non-copper clear margin must not be negative, got {margin} mm")]
  NonCopperClearMarginNegative {
    /// The toolpath id.
    toolpath: String,
    /// The offending margin in millimetres.
    margin: f64,
  },
  /// A non-copper clearing operation has a non-positive cut depth.
  #[error("toolpath {toolpath:?} non-copper clear cut depth must be positive, got {cut_depth} mm")]
  NonCopperClearCutDepthNotPositive {
    /// The toolpath id.
    toolpath: String,
    /// The offending cut depth in millimetres.
    cut_depth: f64,
  },
  /// A non-copper clearing operation has a negative copper offset.
  #[error(
    "toolpath {toolpath:?} non-copper clear copper offset must not be negative, got {copper_offset} mm"
  )]
  NonCopperClearCopperOffsetNegative {
    /// The toolpath id.
    toolpath: String,
    /// The offending copper offset in millimetres.
    copper_offset: f64,
  },
  /// A cutout operation has a non-positive cut depth.
  #[error("toolpath {toolpath:?} cutout cut depth must be positive, got {cut_depth} mm")]
  CutoutCutDepthNotPositive {
    /// The toolpath id.
    toolpath: String,
    /// The offending cut depth in millimetres.
    cut_depth: f64,
  },
  /// A cutout operation has a negative pass depth.
  #[error("toolpath {toolpath:?} cutout pass depth must not be negative, got {pass_depth} mm")]
  CutoutPassDepthNegative {
    /// The toolpath id.
    toolpath: String,
    /// The offending pass depth in millimetres.
    pass_depth: f64,
  },
  /// A cutout tab has a non-positive width.
  #[error("toolpath {toolpath:?} cutout tab width must be positive, got {width} mm")]
  CutoutTabWidthNotPositive {
    /// The toolpath id.
    toolpath: String,
    /// The offending tab width in millimetres.
    width: f64,
  },
  /// A cutout tab is not inside the closed contour it is cut from.
  #[error(
    "toolpath {toolpath:?} cutout tab at position {position} is not inside a closed contour it is cut from"
  )]
  CutoutTabOutsideContour {
    /// The toolpath id.
    toolpath: String,
    /// The offending tab position as a fraction of the contour perimeter.
    position: f64,
  },
  /// A paint operation has an invalid overlap.
  #[error(
    "toolpath {toolpath:?} paint overlap must be a finite number between 0 and 1, got {overlap}"
  )]
  PaintOverlapOutOfRange {
    /// The toolpath id.
    toolpath: String,
    /// The offending overlap.
    overlap: f64,
  },
  /// A paint operation has a non-positive cut depth.
  #[error("toolpath {toolpath:?} paint cut depth must be positive, got {cut_depth} mm")]
  PaintCutDepthNotPositive {
    /// The toolpath id.
    toolpath: String,
    /// The offending cut depth in millimetres.
    cut_depth: f64,
  },
  /// A panelize operation has fewer than one row.
  #[error("toolpath {toolpath:?} panelize rows must be at least 1, got {rows}")]
  PanelizeRowsTooSmall {
    /// The toolpath id.
    toolpath: String,
    /// The offending row count.
    rows: u32,
  },
  /// A panelize operation has fewer than one column.
  #[error("toolpath {toolpath:?} panelize columns must be at least 1, got {columns}")]
  PanelizeColumnsTooSmall {
    /// The toolpath id.
    toolpath: String,
    /// The offending column count.
    columns: u32,
  },
  /// A panelize operation has a negative horizontal spacing.
  #[error("toolpath {toolpath:?} panelize spacing_x must not be negative, got {spacing_x} mm")]
  PanelizeSpacingXNegative {
    /// The toolpath id.
    toolpath: String,
    /// The offending horizontal spacing in millimetres.
    spacing_x: f64,
  },
  /// A panelize operation has a negative vertical spacing.
  #[error("toolpath {toolpath:?} panelize spacing_y must not be negative, got {spacing_y} mm")]
  PanelizeSpacingYNegative {
    /// The toolpath id.
    toolpath: String,
    /// The offending vertical spacing in millimetres.
    spacing_y: f64,
  },
  /// A nesting sheet is zero, negative or not a finite size, so it holds no part.
  ///
  /// The job's own rule (a stock that is not a positive finite size describes no rectangle) applies to
  /// every authored sheet, refused here with the sheet's id, the dimension and the value rather than
  /// clamped into a plausible one (invariant 3).
  #[error("nesting sheet {sheet:?} {field} must be positive, got {value} mm")]
  NonPositiveSheetDimension {
    /// The sheet id.
    sheet: String,
    /// Which dimension.
    field: &'static str,
    /// The offending value in millimetres.
    value: f64,
  },
  /// A nesting part names no entity, so there is nothing to cut it from.
  #[error("nesting part {part:?} names no entity ids, so there is nothing to cut it from")]
  NestingPartTakesNoEntities {
    /// The part id.
    part: String,
  },
  /// A nesting part cites geometry that does not exist.
  ///
  /// The part's selection is ids into this document, so a missing id is a refusal and never an empty
  /// part: an empty part would nest nothing while the report still counted its instances.
  #[error("nesting part {part:?} references unknown entity {entity:?}")]
  NestingPartUnknownEntity {
    /// The part id.
    part: String,
    /// The missing entity id.
    entity: String,
  },
  /// A placement names a part the nesting section does not hold.
  #[error("nesting placement names unknown part {part:?}")]
  NestingPlacementUnknownPart {
    /// The missing part id.
    part: String,
  },
  /// A placement names a sheet the nesting section does not hold.
  #[error("nesting placement names unknown sheet {sheet:?}")]
  NestingPlacementUnknownSheet {
    /// The missing sheet id.
    sheet: String,
  },
  /// A placement's position is not a non-negative, finite point of its sheet.
  ///
  /// The position is the sheet point the part's footprint origin lands on, so a negative coordinate
  /// puts the part past the sheet's lower edges before any geometry is asked about.
  #[error(
    "nesting placement of part {part:?} must sit at a non-negative finite point of its sheet, got \
     {x} mm and {y} mm"
  )]
  NestingPlacementPositionNotUsable {
    /// The part id.
    part: String,
    /// The offending X coordinate in millimetres.
    x: f64,
    /// The offending Y coordinate in millimetres.
    y: f64,
  },
  /// A placement names a part whose contour has no positive-area footprint, so where it lands
  /// cannot be measured.
  ///
  /// `placement_footprint` answers `None` for such a part — a closed contour whose bounds have no
  /// width or no height — and a `None` used to be read as "nothing to check": the sheet and overlap
  /// checks were skipped and the placement validated anywhere, including off the sheet. A part with
  /// no footprint cannot be placed at all.
  #[error(
    "nesting part {part:?} has no positive-area footprint, so where it is placed cannot be checked"
  )]
  NestingPlacementWithoutFootprint {
    /// The part id.
    part: String,
  },
  /// A placement's footprint leaves its sheet, so the machine would cut past the stock.
  #[error(
    "nesting placement of part {part:?} on sheet {sheet:?} leaves the sheet: its footprint reaches \
     x {x} mm and y {y} mm, the sheet is {width} mm by {height} mm"
  )]
  NestingPlacementOutsideSheet {
    /// The part id.
    part: String,
    /// The sheet id.
    sheet: String,
    /// The footprint's largest X, in millimetres.
    x: f64,
    /// The footprint's largest Y, in millimetres.
    y: f64,
    /// The sheet's width in millimetres.
    width: f64,
    /// The sheet's height in millimetres.
    height: f64,
  },
  /// Two placements on one sheet overlap, so both parts cannot be cut from that sheet.
  ///
  /// Touching is not overlapping: parts nested edge to edge are the point of nesting. What is refused
  /// is two footprints whose interiors intersect.
  #[error("nesting placements of parts {first:?} and {second:?} overlap on sheet {sheet:?}")]
  NestingPlacementsOverlap {
    /// The part placed first.
    first: String,
    /// The part placed second.
    second: String,
    /// The sheet both placements name.
    sheet: String,
  },
  /// A part asks for a number of instances the placements of a nested document do not honour.
  ///
  /// Once any placement exists the authored count and the placements are the nester's two halves of
  /// one answer, so a disagreement means one of them is stale and the report would count copies the
  /// document does not hold; a section with no placement at all is not refused, because it is a
  /// request that has not been nested yet and there is nothing for the count to disagree with.
  #[error("nesting part {part:?} asks for {wanted} instances but {placed} placements name it")]
  NestingInstanceCountMismatch {
    /// The part id.
    part: String,
    /// How many instances the part asks for.
    wanted: usize,
    /// How many placements name it.
    placed: usize,
  },
  /// The stored cost record claims a number of sheets the placements do not name.
  ///
  /// The user asked for the cost to be **both** the report and a stored field (S3), so the record is
  /// the run's own answer: a count the placements do not support is stale or hand-edited, and the
  /// report would print a run the document does not hold. Refused naming both numbers (invariant 3).
  #[error(
    "nesting cost records {recorded} sheets used but the placements name {placed} distinct sheets"
  )]
  NestingCostSheetCountMismatch {
    /// The count the record claims.
    recorded: usize,
    /// The number of distinct sheets the placements name.
    placed: usize,
  },
  /// A stored cost fraction is not a finite fraction of the used area.
  ///
  /// Utilisation and waste are fractions of the used sheets' area, so neither can be `NaN`, infinite
  /// or outside `0.0..=1.0`; refused with the field name and the value rather than clamped into a
  /// plausible fraction (invariant 3).
  #[error("nesting cost {field} must be a finite fraction between 0 and 1, got {value}")]
  NestingCostFractionOutOfRange {
    /// Which fraction.
    field: &'static str,
    /// The offending value.
    value: f64,
  },
  /// A stored cost fraction is not the one the document's own placements and sheets derive (T1c).
  ///
  /// A record can be consistent with itself and still be wrong about the area: 0.25 utilisation and
  /// 0.75 waste sum to one while the placements only cover 0.125 of the used sheets. The user asked
  /// for the cost to be **both** the report and a stored field (S3), so the field is measured against
  /// the document rather than against itself, and the refusal states the field, the recorded
  /// fraction, the fraction [`Document::nesting_utilisation`] derives and the tolerance they were
  /// compared within (invariant 3).
  #[error(
    "nesting cost {field} {recorded} is not the {derived} the document's own placements and sheets \
     derive, within {tolerance}"
  )]
  NestingCostFractionMismatch {
    /// Which fraction.
    field: &'static str,
    /// The recorded fraction.
    recorded: f64,
    /// The fraction the document derives.
    derived: f64,
    /// The tolerance the two were compared within.
    tolerance: f64,
  },
}

impl Document {
  /// A new, empty document for `board`.
  pub fn new(board: Board) -> Self {
    Self {
      schema: DOCUMENT_SCHEMA.to_owned(),
      board,
      layers: Vec::new(),
      toolpaths: Vec::new(),
      tools_ref: None,
      postp: None,
      nesting: None,
    }
  }

  /// Compatibility constructor for the former `Job` type.
  pub fn with_job(job: Board) -> Self {
    Self::new(job)
  }

  /// Every entity in the document, whatever layer holds it.
  pub fn entities(&self) -> impl Iterator<Item = &Entity> {
    self.layers.iter().flat_map(|layer| layer.entities.iter())
  }

  /// Look an entity up by id.
  pub fn entity(&self, id: &str) -> Option<&Entity> {
    self.entities().find(|entity| entity.id == id)
  }

  /// The footprint of the part `placement` places, in its sheet's coordinates.
  ///
  /// This is the **one** reading of a placement: the part's own bounding box in document
  /// coordinates, turned about its own lower-left corner by `placement.rotation`, and then moved so
  /// the turned footprint's lower-left corner lands exactly on `placement.position`.
  /// [`Document::validate`] refuses an overlapping or off-sheet placement through it, and a nester
  /// certifying its own output asks the same function, so a certificate and the load-time check can
  /// never disagree (invariant 2).
  ///
  /// `None` when the part offers no footprint to measure: the document holds none of the entities it
  /// names, one of them is not a contour, or its bounds have no width or no height. All are refused
  /// by name before a placement reaches this function, so a `None` here is a document this build
  /// would not have accepted — and `validate` never reads it as "nothing to check".
  ///
  /// The position is the **placed** footprint's corner and not the part's own origin, because a
  /// quarter turn about a part's own corner sends the material to negative X: a nester that had to
  /// compensate for that would be authoring a number nobody can read off the sheet.
  ///
  /// `None` when the part is unknown, or when its selection has no area to place: a degenerate
  /// footprint covers no point of the sheet, so it can neither leave it nor overlap another.
  pub fn placement_footprint(&self, placement: &NestingPlacement) -> Option<Contour> {
    let part = self.nesting.as_ref()?.part(&placement.part_id)?;
    let bounds = part.bounds(self)?;
    let width = bounds.max.x - bounds.min.x;
    let height = bounds.max.y - bounds.min.y;
    if !(width > 0.0 && height > 0.0) {
      return None;
    }
    let local = Contour::from_points(
      &[
        Point2::new(0.0, 0.0),
        Point2::new(width, 0.0),
        Point2::new(width, height),
        Point2::new(0.0, height),
      ],
      true,
    )
    .ok()?;
    let turned =
      cam_geom::transform::rotate(&local, placement.rotation.radians(), Point2::ORIGIN).ok()?;
    let turned_bounds = turned.bounds();
    cam_geom::transform::translate(
      &turned,
      cam_geom::Vec2::new(
        placement.position.x - turned_bounds.min.x,
        placement.position.y - turned_bounds.min.y,
      ),
    )
    .ok()
  }

  /// The utilisation this document's own placements and sheets imply (T1c, S3).
  ///
  /// This is **the** arithmetic for utilisation and waste: the placed footprints' area over the area
  /// of the sheets those placements name, with waste its complement, `1 - utilisation`. The stored
  /// cost record is written from it, [`Document::validate`] refuses a record that disagrees with it
  /// and the report prints it, so the document's halves cannot claim two different utilisations and no
  /// caller needs a third (invariant 2).
  ///
  /// The denominator is the sheets **the placements name**, never every sheet the section holds: a
  /// sheet that is authored but carries no placement is not used, and counting it would report a
  /// utilisation no part of the run supports.
  ///
  /// `None` when the document cannot say — it holds no sheets, or no placement on any of them — rather
  /// than an invented zero, which is why a cost record is never refused for disagreeing with it.
  pub fn nesting_utilisation(&self) -> Option<f64> {
    let nesting = self.nesting.as_ref()?;
    if nesting.sheets.is_empty() || nesting.placements.is_empty() {
      return None;
    }

    let mut used_sheets: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    let mut used_area = 0.0;
    let mut placed_area = 0.0;
    for placement in &nesting.placements {
      let sheet = nesting.sheet(&placement.sheet_id)?;
      if used_sheets.insert(placement.sheet_id.as_str()) {
        used_area += sheet.width.mm() * sheet.height.mm();
      }
      // A placement with no area covers nothing, so it adds nothing, but the sheet it names is still
      // one the run used and stays in the denominator (D-M9-6).
      if let Some(footprint) = self.placement_footprint(placement) {
        placed_area += footprint.signed_area().abs();
      }
    }

    if !(used_area > 0.0 && used_area.is_finite()) {
      return None;
    }
    Some(placed_area / used_area)
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
      ("width", self.board.width.mm()),
      ("height", self.board.height.mm()),
      ("thickness", self.board.thickness.mm()),
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
        let needs_closed_geometry = match toolpath.operation {
          Operation::Isolation(_)
          | Operation::NonCopperClear(_)
          | Operation::Cutout(_)
          | Operation::Paint(_) => true,
          Operation::Panelize(_) => false,
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
      check_isolation(toolpath)?;
      check_non_copper_clear(toolpath)?;
      check_cutout(self, toolpath)?;
      check_paint(toolpath)?;
      check_panelize(toolpath)?;
    }

    check_layers(self)?;
    check_nesting(self)?;

    Ok(())
  }
}

/// Check the optional nesting section: authored sheets, parts, placements, their agreement and the
/// stored cost record.
///
/// Every rule refuses naming the offending value rather than defaulting, because each one describes a
/// job a nester would otherwise silently "fix" into a different one: a sheet with no size, a part with
/// no geometry, a placement that leaves its sheet or lands on another part, an instance count the
/// placements do not honour **once the document holds one**, and a cost record the placements do not
/// support (invariant 3, M8's rule).
///
/// The instance counts are the one rule with a state of its own: a section whose parts ask for
/// instances and which holds no placement at all is a request that has not been nested yet — the
/// state the user authors, and the one `nest` answers — so only a partly nested document is refused.
///
/// Where a placement's material sits is not re-derived here: [`Document::placement_footprint`] is the
/// one reading of a placement, so this check and a nester certifying its own output agree by
/// construction (invariant 2).
fn check_layers(document: &Document) -> Result<(), ModelError> {
  // Keepout may repeat; other (role, side) combinations must be unique.
  let mut seen = std::collections::BTreeSet::new();
  for layer in &document.layers {
    if matches!(layer.role, LayerRole::Keepout) {
      continue;
    }
    let key = (layer.role, layer.side);
    if !seen.insert(key) {
      return Err(ModelError::DuplicateLayerRole {
        role: format!("{:?}/{:?}", layer.role, layer.side),
      });
    }
  }
  Ok(())
}

fn check_nesting(document: &Document) -> Result<(), ModelError> {
  let Some(nesting) = &document.nesting else {
    return Ok(());
  };

  let mut sheet_ids = std::collections::BTreeSet::new();
  for sheet in &nesting.sheets {
    if !sheet_ids.insert(sheet.id.as_str()) {
      return Err(ModelError::DuplicateId {
        kind: "nesting sheet",
        id: sheet.id.clone(),
      });
    }
    for (field, value) in [("width", sheet.width.mm()), ("height", sheet.height.mm())] {
      if value <= 0.0 || !value.is_finite() {
        return Err(ModelError::NonPositiveSheetDimension {
          sheet: sheet.id.clone(),
          field,
          value,
        });
      }
    }
  }

  let mut part_ids = std::collections::BTreeSet::new();
  for part in &nesting.parts {
    if !part_ids.insert(part.id.as_str()) {
      return Err(ModelError::DuplicateId {
        kind: "nesting part",
        id: part.id.clone(),
      });
    }
    if part.entity_ids.is_empty() {
      return Err(ModelError::NestingPartTakesNoEntities {
        part: part.id.clone(),
      });
    }
    for entity_id in &part.entity_ids {
      if document.entity(entity_id).is_none() {
        return Err(ModelError::NestingPartUnknownEntity {
          part: part.id.clone(),
          entity: entity_id.clone(),
        });
      }
    }
  }

  let mut placed_counts: std::collections::BTreeMap<&str, usize> =
    std::collections::BTreeMap::new();
  let mut footprints = Vec::with_capacity(nesting.placements.len());
  for placement in &nesting.placements {
    let part =
      nesting
        .part(&placement.part_id)
        .ok_or_else(|| ModelError::NestingPlacementUnknownPart {
          part: placement.part_id.clone(),
        })?;
    let sheet = nesting.sheet(&placement.sheet_id).ok_or_else(|| {
      ModelError::NestingPlacementUnknownSheet {
        sheet: placement.sheet_id.clone(),
      }
    })?;
    let (x, y) = (placement.position.x, placement.position.y);
    if !(x.is_finite() && y.is_finite() && x >= 0.0 && y >= 0.0) {
      return Err(ModelError::NestingPlacementPositionNotUsable {
        part: part.id.clone(),
        x,
        y,
      });
    }
    *placed_counts.entry(part.id.as_str()).or_default() += 1;

    let footprint = document.placement_footprint(placement);
    let Some(footprint) = footprint else {
      return Err(ModelError::NestingPlacementWithoutFootprint {
        part: part.id.clone(),
      });
    };
    let bounds = footprint.bounds();
    let (width, height) = (sheet.width.mm(), sheet.height.mm());
    let tolerance = cam_geom::DEFAULT_TOLERANCE;
    if bounds.max.x > width + tolerance
      || bounds.max.y > height + tolerance
      || bounds.min.x < -tolerance
      || bounds.min.y < -tolerance
    {
      return Err(ModelError::NestingPlacementOutsideSheet {
        part: part.id.clone(),
        sheet: sheet.id.clone(),
        x: bounds.max.x,
        y: bounds.max.y,
        width,
        height,
      });
    }
    footprints.push((part.id.as_str(), sheet.id.as_str(), footprint));
  }

  // The overlap check is the one quadratic step in loading a nesting, so a pair is pruned twice before
  // any winding relation is asked: two placements on different sheets cannot overlap, and neither can
  // two whose bounding boxes do not meet — `footprints_overlap` answers `Disjoint` for them, and the
  // one relation that survives a separated pair, `Touching`, is reached only through *equal* bounds.
  // Bucketing by sheet makes the cross-sheet half of the count one visit per placement, and sweeping
  // each bucket in x stops the inner walk as soon as a candidate starts past the box it is being
  // compared with, so the work is proportional to the pairs whose x ranges meet and not to
  // N(N-1)/2 — 49 995 000 pair visits for 10 000 placements before this (review-39a6d85c490202c2's
  // `R1-quadratic-overlap-validation` and `R4-002`). The boxes are measured once: `Contour::bounds`
  // walks every vertex, and asking it per pair would trade a quadratic relation count for a quadratic
  // corner count.
  let boxes: Vec<Bounds> = footprints
    .iter()
    .map(|(_, _, footprint)| footprint.bounds())
    .collect();
  // A uniform grid over x, one per sheet and keyed by the cell a box occupies, is the interval index:
  // a candidate is reached only through a cell the placement being tested also occupies, so a box a
  // metre away is never looked at. The cell is wide enough that the grid has about as many columns as a
  // row of placements has entries, which keeps insertion and query near the square root of the
  // placement count each.
  //
  // Both the extent and the column count are **per sheet**. A single global x extent let one placement
  // far away on one sheet stretch every other sheet's cells until they covered that whole sheet, which
  // dropped the crowded sheet back onto the quadratic scan the index exists to avoid
  // (review-1567a12dd01c0090's `R4-001`).
  //
  // A box wider than its sheet's cell span is kept out of the grid entirely, in a per-sheet wide
  // list. Stored in every cell it covers it would make each of those cells hold O(N) occupants, and
  // every placement would iterate all of them before the visited stamp discarded them one by one —
  // O(N²·sqrt N) stamp-loop steps for a document whose pairs are Θ(N²), which is exactly N full-width
  // strips stacked in y. The wide list gives that document the optimal Θ(N²) scan with no duplication,
  // at the price of walking the list once per placement: with W wide boxes and N narrow ones that is
  // O(N·W) guard checks, which is quadratic when most boxes are wide — the counting is honest here
  // because the guards are two f64 comparisons and no relation is asked, but it is quadratic, and a
  // document of a million wide strips would feel it,
  // and costs one extra entry per query when one wide box sits beside many narrow ones
  // (review-1567a12dd01c0090's `R3-001`). This replaces a running maximum of `max.x`: that bound was
  // sound but not tight, because one box spanning the sheet held it high for every placement behind it
  // and forced a scan of the whole prefix, relations included (review-ca5c7eda65951600's
  // `R4-LOAD-REGRESSION`, `R1-001` and `R3-001`).
  let tolerance = cam_geom::DEFAULT_TOLERANCE;
  // Derived from the footprints themselves, so each placement is attributed to its own sheet exactly
  // once and there is no `""` sentinel a later edit could leave behind
  // (review-1567a12dd01c0090's `R2-005`).
  let sheet_of: Vec<&str> = footprints
    .iter()
    .map(|(_, sheet_id, _)| *sheet_id)
    .collect();
  // One sheet's interval index: the cells it occupies, and the boxes too wide to be stored in them.
  struct SheetGrid {
    origin: f64,
    cell_width: f64,
    cells: std::collections::BTreeMap<i64, Vec<usize>>,
    wide: Vec<usize>,
  }
  impl SheetGrid {
    fn cell_of(&self, x: f64) -> i64 {
      ((x - self.origin) / self.cell_width).floor() as i64
    }
  }
  let mut extents: std::collections::BTreeMap<&str, (f64, f64, usize)> =
    std::collections::BTreeMap::new();
  for (index, bounds) in boxes.iter().enumerate() {
    let extent = extents
      .entry(sheet_of[index])
      .or_insert((f64::INFINITY, f64::NEG_INFINITY, 0));
    extent.0 = extent.0.min(bounds.min.x);
    extent.1 = extent.1.max(bounds.max.x);
    extent.2 += 1;
  }
  let mut grids: std::collections::BTreeMap<&str, SheetGrid> = extents
    .iter()
    .map(|(sheet_id, (min, max, count))| {
      let columns = ((*count as f64).sqrt() * 2.0).max(1.0);
      let span = if max.is_finite() && max > min {
        max - min
      } else {
        0.0
      };
      (
        *sheet_id,
        SheetGrid {
          origin: if min.is_finite() { *min } else { 0.0 },
          cell_width: (span / columns).max(tolerance),
          cells: std::collections::BTreeMap::new(),
          wide: Vec::new(),
        },
      )
    })
    .collect();
  for (index, bounds) in boxes.iter().enumerate() {
    let grid = grids
      .get_mut(sheet_of[index])
      .expect("every placement's sheet has a grid");
    if bounds.max.x - bounds.min.x > grid.cell_width {
      grid.wide.push(index);
      continue;
    }
    // Both ends are padded by `tolerance`, exactly as the query range below is, so a box whose edge sits
    // a hair inside the next cell still shares one with a neighbour it meets. Measured, and stated
    // because the measurement is counter-intuitive: **removing this padding changes no refusal** — an
    // offence needs the two boxes to meet at all, a pair meeting at all shares a cell even unpadded
    // because `cell_width` is floored at the tolerance, and the pairs the padding does add are ones the
    // classifier answers `Touching` for, which are accepted either way. So the padding is defensive and
    // the guarantee is that argument, not a test: the two seeded differentials catch a *keying* mistake
    // (mutating the query to one cell fails both) and cannot catch this one. `cell_width` is floored at
    // the tolerance for a different reason: a narrower cell would make a padded box's range more than a
    // couple of cells wide and insert every box over and over.
    let first_cell = grid.cell_of(bounds.min.x - tolerance);
    let last_cell = grid.cell_of(bounds.max.x + tolerance);
    for cell in first_cell..=last_cell {
      grid.cells.entry(cell).or_default().push(index);
    }
  }
  // The refusal must name the pair with the smallest `(earlier, later)` placement indices, which is
  // what a document-order scan names. Visiting pairs in that same order is what lets the first offence
  // end the check instead of being accumulated: `earlier` climbs through the document order, so the
  // first `earlier` that has any partner at all is the one the refusal is about, and the partner taken
  // is the smallest `later`. A sweep in x order cannot do that — it meets pairs in an order the
  // document does not have, which is why the earlier version had to scan them all and this one can
  // stop (review-94cfd23437f72a47's `R1-001` and `R4-001`).
  let mut visited = vec![0_u64; footprints.len()];
  for earlier in 0..footprints.len() {
    let sheet_id = sheet_of[earlier];
    let grid = grids
      .get(sheet_id)
      .expect("every placement's sheet has a grid");
    let box_earlier = boxes[earlier];
    let mut later: Option<usize> = None;
    let consider = |candidate: usize, later: &mut Option<usize>| {
      // The pair is named with the smaller placement index first, so a candidate ahead of `earlier`
      // belongs to a pair an earlier turn of the loop already judged.
      if candidate <= earlier {
        return;
      }
      let box_candidate = boxes[candidate];
      // Both guards are cheap and both are needed: a cell is wider than a box, so sharing one is not
      // the same as meeting.
      if box_candidate.min.x > box_earlier.max.x + tolerance
        || box_candidate.max.x < box_earlier.min.x - tolerance
      {
        return;
      }
      if box_candidate.min.y > box_earlier.max.y + tolerance
        || box_candidate.max.y < box_earlier.min.y - tolerance
      {
        return;
      }
      if !footprints_overlap(&footprints[earlier].2, &footprints[candidate].2) {
        return;
      }
      *later = Some(later.map_or(candidate, |seen| seen.min(candidate)));
    };
    // A box still in more than one cell — its padded range crosses a cell edge — can be reached twice,
    // so the stamp keeps each candidate to one judgement per placement without allocating a set. The
    // wide list is scanned once by index and shares that stamp, so a pair is never judged twice. The
    // wide boxes come first because they are the ones a grid query can no longer see.
    let stamp = earlier as u64 + 1;
    for candidate in &grid.wide {
      if visited[*candidate] == stamp {
        continue;
      }
      visited[*candidate] = stamp;
      consider(*candidate, &mut later);
    }
    for cell in
      grid.cell_of(box_earlier.min.x - tolerance)..=grid.cell_of(box_earlier.max.x + tolerance)
    {
      let Some(occupants) = grid.cells.get(&cell) else {
        continue;
      };
      for candidate in occupants {
        if visited[*candidate] == stamp {
          continue;
        }
        visited[*candidate] = stamp;
        consider(*candidate, &mut later);
      }
    }
    if let Some(later) = later {
      return Err(ModelError::NestingPlacementsOverlap {
        first: footprints[earlier].0.to_owned(),
        second: footprints[later].0.to_owned(),
        sheet: sheet_id.to_owned(),
      });
    }
  }

  // The instance counts are honoured only once the document holds a placement: parts naming instances
  // with no placement at all is a request that has not been nested yet (T1d, S1), and there is nothing
  // for the count to disagree with. From the first placement on the equality holds for every part, so
  // a partial or miscounted nesting is still refused.
  let nested = !nesting.placements.is_empty();
  for part in &nesting.parts {
    let placed = placed_counts.get(part.id.as_str()).copied().unwrap_or(0);
    if nested && placed != part.instances {
      return Err(ModelError::NestingInstanceCountMismatch {
        part: part.id.clone(),
        wanted: part.instances,
        placed,
      });
    }
  }

  check_nesting_cost(document)?;

  Ok(())
}

/// The tolerance the stored cost record's fractions are compared with.
///
/// It is the crate's existing notion of "close enough", `cam_geom::DEFAULT_TOLERANCE` — the value
/// this module already uses for the placement and island checks — reused rather than a second
/// epsilon, so a document has one tolerance and not two. Utilisation and waste are dimensionless
/// fractions, so this is an absolute tolerance, and the comparison is inclusive: a fraction exactly
/// the tolerance away from the one the document derives still loads.
///
/// It is a **fraction** tolerance and not a sum tolerance: no rule adds the two fractions up. Each is
/// compared with the one derivation and with its complement, so a record that agrees with both cannot
/// fail to sum to one — the sum is implied rather than checked, which is the part of the older name
/// the code no longer does.
const NESTING_COST_FRACTION_TOLERANCE: f64 = cam_geom::DEFAULT_TOLERANCE;

/// Check the stored cost record against the placements and sheets that produced it (S3).
///
/// The user asked for utilisation and waste to be **both** the report and a stored field (S3), so the
/// field and the placements must not be able to disagree. Where the document can answer — it holds
/// sheets and placements — [`Document::nesting_utilisation`] is the one arithmetic and the record has
/// to equal it. Three rules, each refusing the record by name and each stating the recorded value
/// beside the value the document implies (invariant 3):
///
/// 1. `sheets_used` is the number of **distinct sheets** the placements name.
/// 2. `utilisation` and `waste` are finite fractions of the used area, so each is in `0.0..=1.0`.
/// 3. `utilisation` is the fraction the placements cover and `waste` is its complement, both within
///    `NESTING_COST_FRACTION_TOLERANCE`. Agreement with itself is not enough: a record can sum to one and
///    still claim an area the placements do not support, which is the record this rule exists for.
///    The sum rule is therefore **implied rather than checked**: both fractions are compared with the
///    one derivation and its complement, so a record that agrees with each cannot fail to sum to one.
///    A document that cannot say what it used — no sheets, or nothing placed on them — has used
///    nothing, so its derivation is utilisation zero and the whole sheet wasted, and a record that
///    claims more is refused by this same comparison instead of being exempted from it.
///
/// This never recomputes the record: validating does not fix the numbers, because a loader that
/// quietly corrected a stale record would hide the staleness instead of reporting it. And it never
/// derives the fractions itself either — the third rule asks the document and compares (invariant 2).
fn check_nesting_cost(document: &Document) -> Result<(), ModelError> {
  let Some(nesting) = &document.nesting else {
    return Ok(());
  };
  let Some(cost) = &nesting.cost else {
    return Ok(());
  };

  let named_sheets: std::collections::BTreeSet<&str> = nesting
    .placements
    .iter()
    .map(|placement| placement.sheet_id.as_str())
    .collect();
  if cost.sheets_used != named_sheets.len() {
    return Err(ModelError::NestingCostSheetCountMismatch {
      recorded: cost.sheets_used,
      placed: named_sheets.len(),
    });
  }

  for (field, value) in [("utilisation", cost.utilisation), ("waste", cost.waste)] {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
      return Err(ModelError::NestingCostFractionOutOfRange { field, value });
    }
  }

  // A document that cannot say what it used — no sheets, or nothing placed on them — used nothing, so
  // the derivation is zero utilisation and the whole sheet wasted rather than an invented fraction.
  // Only a record that agrees with that loads; one claiming a measurement no placement supports is
  // refused by the same comparison as any other, naming the field, the record and the implication.
  let utilisation = document.nesting_utilisation().unwrap_or(0.0);
  for (field, recorded, derived) in [
    ("utilisation", cost.utilisation, utilisation),
    ("waste", cost.waste, 1.0 - utilisation),
  ] {
    if (recorded - derived).abs() > NESTING_COST_FRACTION_TOLERANCE {
      return Err(ModelError::NestingCostFractionMismatch {
        field,
        recorded,
        derived,
        tolerance: NESTING_COST_FRACTION_TOLERANCE,
      });
    }
  }

  Ok(())
}

/// Whether two footprints share any area on their sheet.
///
/// The kernel's own intersector answers, so this is not a second one (T3's rule): crossing and
/// containment are overlaps. `Touching` is not — parts nested edge to edge are the point of nesting —
/// except when the two contours are the same one, which the kernel reports as `Touching` and which is
/// two copies stacked on one spot.
fn footprints_overlap(first: &Contour, second: &Contour) -> bool {
  match cam_geom::winding::relation(first, second, cam_geom::DEFAULT_TOLERANCE) {
    cam_geom::winding::Relation::Disjoint => false,
    cam_geom::winding::Relation::Touching => first.bounds() == second.bounds(),
    cam_geom::winding::Relation::FirstInside
    | cam_geom::winding::Relation::SecondInside
    | cam_geom::winding::Relation::Crossing => true,
  }
}

/// Check an isolation operation's parameters.
fn check_isolation(toolpath: &Toolpath) -> Result<(), ModelError> {
  let Operation::Isolation(params) = &toolpath.operation else {
    return Ok(());
  };
  if params.passes < 1 {
    return Err(ModelError::IsolationPassesTooSmall {
      toolpath: toolpath.id.clone(),
      passes: params.passes,
    });
  }
  if !params.overlap.is_finite() || !(0.0..=1.0).contains(&params.overlap) {
    return Err(ModelError::IsolationOverlapOutOfRange {
      toolpath: toolpath.id.clone(),
      overlap: params.overlap,
    });
  }
  if params.cut_depth.mm() <= 0.0 || !params.cut_depth.mm().is_finite() {
    return Err(ModelError::IsolationCutDepthNotPositive {
      toolpath: toolpath.id.clone(),
      cut_depth: params.cut_depth.mm(),
    });
  }
  Ok(())
}

/// Check a non-copper clearing operation's parameters.
fn check_non_copper_clear(toolpath: &Toolpath) -> Result<(), ModelError> {
  let Operation::NonCopperClear(params) = &toolpath.operation else {
    return Ok(());
  };
  if !params.overlap.is_finite() || !(0.0..=1.0).contains(&params.overlap) {
    return Err(ModelError::NonCopperClearOverlapOutOfRange {
      toolpath: toolpath.id.clone(),
      overlap: params.overlap,
    });
  }
  if params.margin.mm() < 0.0 || !params.margin.mm().is_finite() {
    return Err(ModelError::NonCopperClearMarginNegative {
      toolpath: toolpath.id.clone(),
      margin: params.margin.mm(),
    });
  }
  if params.cut_depth.mm() <= 0.0 || !params.cut_depth.mm().is_finite() {
    return Err(ModelError::NonCopperClearCutDepthNotPositive {
      toolpath: toolpath.id.clone(),
      cut_depth: params.cut_depth.mm(),
    });
  }
  if let Some(copper_offset) = params.copper_offset {
    if copper_offset.mm() < 0.0 || !copper_offset.mm().is_finite() {
      return Err(ModelError::NonCopperClearCopperOffsetNegative {
        toolpath: toolpath.id.clone(),
        copper_offset: copper_offset.mm(),
      });
    }
  }
  Ok(())
}

/// Check a cutout operation's parameters.
fn check_cutout(document: &Document, toolpath: &Toolpath) -> Result<(), ModelError> {
  let Operation::Cutout(params) = &toolpath.operation else {
    return Ok(());
  };
  if params.cut_depth.mm() <= 0.0 || !params.cut_depth.mm().is_finite() {
    return Err(ModelError::CutoutCutDepthNotPositive {
      toolpath: toolpath.id.clone(),
      cut_depth: params.cut_depth.mm(),
    });
  }
  if params.pass_depth.mm() < 0.0 || !params.pass_depth.mm().is_finite() {
    return Err(ModelError::CutoutPassDepthNegative {
      toolpath: toolpath.id.clone(),
      pass_depth: params.pass_depth.mm(),
    });
  }
  for tab in &params.tabs {
    let width = tab.width.mm();
    if width <= 0.0 || !width.is_finite() {
      return Err(ModelError::CutoutTabWidthNotPositive {
        toolpath: toolpath.id.clone(),
        width,
      });
    }
    // A tab's position is a fraction of the contour perimeter in [0, 1). It must lie on a closed
    // contour the cutout is cut from.
    if !(0.0..1.0).contains(&tab.position) || !tab.position.is_finite() {
      return Err(ModelError::CutoutTabOutsideContour {
        toolpath: toolpath.id.clone(),
        position: tab.position,
      });
    }
    let inside = toolpath.entity_ids.iter().any(|id| {
      document
        .entity(id)
        .and_then(|e| e.kind.as_contour())
        .is_some_and(|c| c.is_closed())
    });
    if !inside {
      return Err(ModelError::CutoutTabOutsideContour {
        toolpath: toolpath.id.clone(),
        position: tab.position,
      });
    }
  }
  Ok(())
}

/// Check a paint operation's parameters.
fn check_paint(toolpath: &Toolpath) -> Result<(), ModelError> {
  let Operation::Paint(params) = &toolpath.operation else {
    return Ok(());
  };
  if !params.overlap.is_finite() || !(0.0..=1.0).contains(&params.overlap) {
    return Err(ModelError::PaintOverlapOutOfRange {
      toolpath: toolpath.id.clone(),
      overlap: params.overlap,
    });
  }
  if params.cut_depth.mm() <= 0.0 || !params.cut_depth.mm().is_finite() {
    return Err(ModelError::PaintCutDepthNotPositive {
      toolpath: toolpath.id.clone(),
      cut_depth: params.cut_depth.mm(),
    });
  }
  Ok(())
}

/// Check a panelize operation's parameters.
fn check_panelize(toolpath: &Toolpath) -> Result<(), ModelError> {
  let Operation::Panelize(params) = &toolpath.operation else {
    return Ok(());
  };
  if params.rows < 1 {
    return Err(ModelError::PanelizeRowsTooSmall {
      toolpath: toolpath.id.clone(),
      rows: params.rows,
    });
  }
  if params.columns < 1 {
    return Err(ModelError::PanelizeColumnsTooSmall {
      toolpath: toolpath.id.clone(),
      columns: params.columns,
    });
  }
  if params.spacing_x.mm() < 0.0 || !params.spacing_x.mm().is_finite() {
    return Err(ModelError::PanelizeSpacingXNegative {
      toolpath: toolpath.id.clone(),
      spacing_x: params.spacing_x.mm(),
    });
  }
  if params.spacing_y.mm() < 0.0 || !params.spacing_y.mm().is_finite() {
    return Err(ModelError::PanelizeSpacingYNegative {
      toolpath: toolpath.id.clone(),
      spacing_y: params.spacing_y.mm(),
    });
  }
  Ok(())
}
