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

use cam_geom::{Contour, Point2};
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

/// The document frame's relation to the machine frame, for one job (M7 A7, S17).
///
/// A document's geometry runs from the job's own corner, `(0, 0)` to `(width, height)`, while a
/// program's moves are in machine coordinates — the frame the selected [`OriginCorner`] puts the
/// machine's origin in. Two of the three corners make the frames differ, and both differences are exact
/// isometries: `center` translates the job so its centre is the machine's origin, and `top_left`
/// reflects Y so distance grows towards the operator.
///
/// This is the **one** definition of that map. The toolpath layer emits with [`Frame::to_machine`], and
/// the simulation reads a program back into the document frame with [`Frame::from_machine`], so a
/// program cannot mean one place in G-code and another in the report. M6 recorded the defect that
/// motivated it: `cam_sim`'s field was in the document frame while the programs it swept were in the
/// machine frame, so at any corner but `bottom_left` a correct program was reported as gouge and uncut.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
  corner: OriginCorner,
  width: f64,
  height: f64,
}

impl Frame {
  /// The frame of `job`, from its origin corner and its size.
  pub fn of(job: &Job) -> Self {
    Self {
      corner: job.origin,
      width: job.width.mm(),
      height: job.height.mm(),
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
  /// The exact inverse of [`Frame::to_machine`], which is what lets the simulation sweep an emitted
  /// program onto a field built in the document frame. A reflection is its own inverse, so `top_left`
  /// reads the same in both directions.
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
  ///
  /// A reflection is not just a different position: an arc traversed counter-clockwise in one frame is
  /// traversed clockwise in the other, so a caller reading a program back into the document frame has
  /// to flip the arc's direction as well as its points. `center` translates, so it keeps handedness;
  /// `top_left` reflects, so it does not. It is part of this map rather than the caller's business,
  /// because a map that silently reverses a path's sense would leave the sweep cutting the mirror
  /// image of the arc the document asked for.
  pub fn is_reflection(&self) -> bool {
    self.corner == OriginCorner::TopLeft
  }
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
  /// This job's document frame against the machine frame.
  pub fn frame(&self) -> Frame {
    Frame::of(self)
  }

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

  /// The stock's surface as a closed contour in document coordinates, from `(0, 0)` to
  /// `(width, height)`.
  ///
  /// The **one** place the job rectangle is built (M6 A6). An inlay's male half clears it with the
  /// region standing as an island, and `cam-sim` authorises the same rectangle, so both ask here: a
  /// program and a target that each built their own rectangle would eventually disagree about where the
  /// stock ends, and a male that cleared past the stock or stopped short of it would be a ruined part
  /// rather than a visible error.
  ///
  /// Document coordinates always run from the job's own corner; the corner *selector* is applied later,
  /// once, by the toolpath layer (`cam_toolpath::machine_point`), and never here.
  ///
  /// # Panics
  ///
  /// Panics when `width` or `height` is not a positive, finite length, which cannot describe a
  /// rectangle. Every caller reaches this through [`Document::validate`] or a grid, both of which
  /// refuse such a job by name first, and the panic says so rather than returning a degenerate
  /// contour no strategy could cut.
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
    .expect("validated job dimensions describe a closed rectangle")
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

/// Parameters of a fluting toolpath.
///
/// A flute is one pass along each selected vector at one authored depth, with the tool's centre
/// **on** the vector (D-M6-5). The ramp is a *length*, not a depth: with `ramp > 0` the pass enters
/// over the first `ramp` millimetres of the vector and leaves over the last, so a round-nosed bit
/// never plunges at full depth into the end of a visible flute; `ramp = 0` plunges and retracts in
/// place, which is the plain groove. Nothing else about a profile's machinery applies: no side, no
/// pass law, no lead and no tab, because a second pass would cut a second groove into the first.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FlutingParams {
  /// Depth at which cutting starts, for a part already partly machined.
  pub start_depth: Length,
  /// Depth the pass cuts to: the bottom of the ramp in and the top of the ramp out.
  pub cut_depth: Length,
  /// Length in millimetres of the ramped entry and of the ramped exit; zero plunges in place.
  ///
  /// Defaulted rather than required, so a hand-written or older document that names a `fluting`
  /// toolpath without the field loads as the in-place plunge it describes instead of failing the
  /// load. The ramp bounds are input errors, not defaults: a negative ramp is refused by
  /// `cam-toolpath::fluting` by name, and so is one longer than half the vector it runs on (A4).
  #[serde(default)]
  pub ramp: Length,
}

impl Default for FlutingParams {
  fn default() -> Self {
    Self {
      start_depth: Length::ZERO,
      cut_depth: Length::from_mm(18.0),
      ramp: Length::ZERO,
    }
  }
}

/// Parameters of a texture toolpath.
///
/// A texture is a pattern of parallel grooves across a closed region (D-M6-6), and its value is the
/// **pattern**, not the clearing: the material between the grooves is meant to stand, so nothing here
/// clears it and nothing here may be read as a pocket. Each groove is one pass at one authored depth
/// with the tool centre on it, and the pattern itself is **derived** — [`TextureParams::grooves`] —
/// because a stored pattern and a recomputed one diverge, and a pattern has far more in it to get
/// subtly wrong twice than a chamfer's single depth does (the argument D-M6-2 makes for that depth).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextureParams {
  /// Depth at which cutting starts, for a part already partly machined.
  pub start_depth: Length,
  /// Total cut depth below the stock surface; each groove's one pass lands exactly on it.
  pub cut_depth: Length,
  /// Distance between neighbouring grooves, measured across them.
  ///
  /// Defaulted rather than required, so a hand-written document that names a `texture` toolpath
  /// without the field loads as the pattern it describes instead of failing the load. A spacing that
  /// is zero or negative is an **input error** and never an empty pattern: it is refused by name at
  /// document load (`ModelError::TextureSpacingNotPositive`) and again by
  /// `cam-toolpath::texture`, because a groove lattice with no spacing behind it is a request the
  /// user did not make.
  #[serde(default = "default_texture_spacing")]
  pub spacing: Length,
  /// Direction of the grooves, in radians, counter-clockwise from the X axis.
  ///
  /// Defaulted like `spacing`, so an older or hand-written document describing a horizontal pattern
  /// loads. The pattern is clipped to the region in the **groove frame**, so this rotates the region
  /// and never the document (D-M6-6).
  #[serde(default)]
  pub angle: Angle,
}

impl TextureParams {
  /// The grooves this pattern cuts across `contours`, as polylines in document XY.
  ///
  /// Derived here and nowhere else (D-M6-6): `cam-toolpath::texture` cuts these vertices and
  /// `cam-sim::target` authorises along the same ones, so the program and the target cannot be made to
  /// agree by construction.
  ///
  /// The grooves are clipped to the region `contours` bound under the kernel's parity rule, by
  /// [`cam_geom::region::Region::raster_spans`] in the **groove frame**: rotating the region by
  /// `-angle` turns "a groove at `angle`" into "a row at some `y`", which is exactly the question
  /// `raster_spans` answers, arcs and islands included, without a second intersector (D-M6-6). The
  /// first groove sits half a spacing inside the region's lower extent in that frame, so the lattice
  /// starts off the region's own edge instead of on it, where the kernel would report a tangency.
  ///
  /// A row that misses the region contributes nothing, and a row that only **touches** a boundary is a
  /// tangency the kernel reports as ambiguous and this method drops: a tangent groove has no length to
  /// cut. Returns no grooves at all when the contours do not bound a region or the spacing has no
  /// pattern behind it; those refusals belong to the callers, which are the ones that can name an
  /// entity or a number (`ToolpathError::OpenContour`, `ToolpathError::InvalidSpacing`,
  /// `ToolpathError::NoGroove`).
  pub fn grooves(&self, contours: &[&Contour]) -> Vec<Vec<Point2>> {
    let spacing = self.spacing.mm();
    if contours.is_empty() || !spacing.is_finite() || spacing <= 0.0 {
      return Vec::new();
    }
    let angle = self.angle.radians();
    // Into the groove frame: a groove at `angle` is a horizontal row here, so the clipping below is the
    // kernel's own raster question and not a second intersector (D-M6-6).
    let Some(region) = rotated_region(contours, -angle) else {
      return Vec::new();
    };
    let bounds = region
      .boundaries
      .iter()
      .fold(cam_geom::Bounds::empty(), |bounds, boundary| {
        bounds.union(boundary.bounds())
      });
    let origin = Point2::new(0.0, 0.0);
    let mut grooves = Vec::new();
    let mut row = bounds.min.y + spacing / 2.0;
    while row <= bounds.max.y {
      // `None` is the kernel's ambiguity, which for a row means a tangency: the row touches a boundary
      // without crossing it, so the groove there has no length to cut and contributes nothing.
      if let Some(spans) = region.raster_spans(row, cam_geom::DEFAULT_TOLERANCE) {
        for (left, right) in spans {
          if right - left <= 0.0 {
            continue;
          }
          grooves.push(vec![
            Point2::new(left, row).rotated_about(origin, angle),
            Point2::new(right, row).rotated_about(origin, angle),
          ]);
        }
      }
      row += spacing;
    }
    grooves
  }
}

impl Default for TextureParams {
  fn default() -> Self {
    Self {
      start_depth: Length::ZERO,
      cut_depth: Length::from_mm(18.0),
      spacing: default_texture_spacing(),
      angle: Angle::ZERO,
    }
  }
}

/// The default distance between a texture's grooves: 3 mm, the pocket stepover's own default.
fn default_texture_spacing() -> Length {
  Length::from_mm(3.0)
}

/// Parameters of an inlay toolpath pair.
///
/// An inlay is **two** toolpaths from one command (M6 A6, D-M6-7): a *female* that clears the selected
/// region grown by `allowance` — the cavity — and a *male* that clears the job rectangle with the region
/// standing as an island, which leaves a plug of exactly the region's shape. The plug fits the cavity
/// with `allowance` millimetres of clearance, which is why there is one allowance and no second "gap"
/// parameter: the number **is** the clearance.
///
/// Both halves carry the same five parameters, and which half a toolpath is, is stated where the
/// document already states such things: the **female** names the region it clears as its `entity_ids`
/// and leaves nothing standing, and the **male** has no boundary entity at all — its boundary is the
/// job rectangle, which is not an entity ([`Job::outline`]) — and names the region it leaves standing
/// as its `island_ids`. Nothing here is derived, so the two halves cannot disagree about the depth, the
/// stepover or the allowance: there is one copy of them between the pair, and
/// [`ModelError::InlayMaleTakesNoEntities`] refuses a record that names both, because two halves in one
/// toolpath would leave the program's meaning to a guess.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InlayParams {
  /// Clearance between the male plug and the female cavity: how far the female's region grows.
  pub allowance: Length,
  /// Distance between adjacent rings or rows of either half's clearing, the pocket's own parameter.
  pub stepover: Length,
  /// Depth of material removed per pass.
  pub pass_depth: Length,
  /// Depth at which cutting starts, for a part already partly machined.
  pub start_depth: Length,
  /// Total cut depth below the stock surface, shared by both halves.
  pub cut_depth: Length,
}

impl Default for InlayParams {
  /// The pocket's own defaults, taken from it rather than restated: an inlay that authors nothing but
  /// its region clears at the depth and the stepover a pocket would.
  fn default() -> Self {
    let pocket = PocketParams::default();
    Self {
      allowance: Length::ZERO,
      stepover: pocket.stepover,
      pass_depth: pocket.pass_depth,
      start_depth: pocket.start_depth,
      cut_depth: pocket.cut_depth,
    }
  }
}

/// `contours` as a region in a frame rotated by `radians`, or `None` when they do not bound one.
///
/// A rotation moves every vertex and leaves every bulge alone — a bulge is an angle between two
/// segments, and both turn with the frame — so the region keeps its arcs and its parity, and nothing
/// about it is tessellated to rotate it.
fn rotated_region(contours: &[&Contour], radians: f64) -> Option<cam_geom::region::Region> {
  let origin = Point2::new(0.0, 0.0);
  let mut boundaries = Vec::with_capacity(contours.len());
  for contour in contours {
    let vertices: Vec<cam_geom::Vertex> = contour
      .vertices()
      .iter()
      .map(|vertex| {
        cam_geom::Vertex::new(vertex.point.rotated_about(origin, radians), vertex.bulge)
      })
      .collect();
    boundaries.push(Contour::new(vertices, contour.is_closed()).ok()?);
  }
  cam_geom::region::Region::new(boundaries).ok()
}

/// Which side of the circle a milled thread runs on.
///
/// An **internal** thread is cut inside a hole — the wall the tool approaches from within — and an
/// **external** one on a boss's outside. The direction decides one number, the radius of the tool
/// centre's own circle: `(D − t) / 2` for an internal thread and `(D + t) / 2` for an external one, `D`
/// being the selected circle's diameter and `t` the tool's (M6 A7).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadDirection {
  /// A thread inside a hole: the tool centre runs inside the circle.
  #[default]
  Internal,
  /// A thread on a boss's outside: the tool centre runs outside the circle.
  External,
}

/// Parameters of a thread-milling toolpath.
///
/// A thread milled on a circle is one **helical** descent: the tool orbits the selected circle at a
/// fixed radius and drops one `pitch` per revolution, from the surface to `-cut_depth`, then retracts
/// (M6 A7). Nothing about the circle's diameter is stored here, and nothing about the tool either: the
/// circle is the entity the toolpath names and the tool is the library's, so a document cannot disagree
/// with the geometry it was written against.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThreadParams {
  /// Distance the tool descends per revolution, in millimetres.
  ///
  /// A pitch that is zero or negative has no descent per revolution behind it, so it is refused by
  /// name when the document is validated and never clamped into a thread.
  pub pitch: Length,
  /// Whether the thread is cut inside the circle or around its outside.
  pub direction: ThreadDirection,
  /// Depth at which the descent starts, for a part already partly machined.
  pub start_depth: Length,
  /// Total cut depth below the stock surface: the depth the helix reaches before it retracts.
  pub cut_depth: Length,
}

impl Default for ThreadParams {
  fn default() -> Self {
    Self {
      pitch: Length::from_mm(1.5),
      direction: ThreadDirection::Internal,
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
  /// Flute the selected vectors, open or closed, in one pass whose ends ramp in and out.
  Fluting(FlutingParams),
  /// Cut parallel grooves across the selected closed region; the material between them stands.
  Texture(TextureParams),
  /// Cut one half of a male/female inlay pair; the pair is two toolpaths sharing one `InlayParams`.
  Inlay(InlayParams),
  /// Mill a thread: one helical descent around each selected circle, one `pitch` per revolution.
  Thread(ThreadParams),
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
  ///
  /// Two operations take islands, and they are the two that clear a region: a `pocket`, whose islands
  /// are contours inside its boundary, and an `inlay`, whose **male** half has no boundary entity at
  /// all — its boundary is the job rectangle — so its island list is the region it leaves standing
  /// (D-M6-13). The sentence names both, because "only for a pocket" stopped being true when the inlay
  /// pair landed (M6 L16).
  #[error("toolpath {toolpath:?} takes islands only for a pocket or an inlay")]
  IslandsRequirePocketOrInlay {
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
  /// A flute's ramp is negative, which would enter upwards.
  ///
  /// The document model refuses it as the document loads, because a negative length has no meaning here
  /// and the machine has nothing sensible to do with it. `cam-toolpath::fluting` refuses it too, naming
  /// the same value: the document is the boundary that sees it before any toolpath exists, and a
  /// caller that bypasses the CLI reaches the strategy without passing through here (D-M6-5, A4).
  #[error("toolpath {toolpath:?} fluting ramp must not be negative, got {value} mm")]
  FlutingRampNegative {
    /// The toolpath id.
    toolpath: String,
    /// The offending ramp in millimetres.
    value: f64,
  },
  /// A texture's spacing is zero, negative or not a finite number.
  ///
  /// A pattern is a lattice of grooves `spacing` millimetres apart, so a spacing with no distance
  /// behind it is an input error and not a silently empty pattern: it is refused here, as the document
  /// loads, and again by `cam-toolpath::texture`, which sees a caller that bypassed the CLI (D-M6-6).
  #[error(
    "toolpath {toolpath:?} texture spacing must be a positive number of millimetres, got {value} mm"
  )]
  TextureSpacingNotPositive {
    /// The toolpath id.
    toolpath: String,
    /// The offending spacing in millimetres.
    value: f64,
  },
  /// An inlay's stepover is zero, negative or not a finite number.
  ///
  /// Both halves clear with the pocket's ring machinery, so both need a distance between rings: a ring
  /// lattice with no stepover behind it leaves material between the rings, which is the one thing a
  /// clearing pass must never do (M6 A6).
  #[error(
    "toolpath {toolpath:?} inlay stepover must be a positive number of millimetres, got {value} mm"
  )]
  InlayStepoverNotPositive {
    /// The toolpath id.
    toolpath: String,
    /// The offending stepover in millimetres.
    value: f64,
  },
  /// An inlay's cut depth does not extend below its start depth.
  ///
  /// The rule every chunked strategy inherits from the profile: a pass whose cut depth is not below its
  /// start depth has no pass behind it, so no clearing would be emitted at all (M6 A6).
  #[error(
    "toolpath {toolpath:?} inlay cut depth {cut_depth} mm must be greater than its start depth \
     {start_depth} mm"
  )]
  InlayDepthRange {
    /// The toolpath id.
    toolpath: String,
    /// The requested cut depth in millimetres.
    cut_depth: f64,
    /// The requested start depth in millimetres.
    start_depth: f64,
  },
  /// An inlay toolpath names both boundary entities and islands, so it is not one of the two halves.
  ///
  /// A female's boundary is the region it clears; a male's boundary is the job rectangle and its island
  /// list is that same region. A record naming both is two halves in one toolpath, and which of them a
  /// program would cut is not something to guess (see [`InlayParams`]).
  #[error(
    "toolpath {toolpath:?} is an inlay male with entity_ids: a male's boundary is the job rectangle, so \
     an inlay toolpath names either the region it clears or the islands it leaves standing, never both"
  )]
  InlayMaleTakesNoEntities {
    /// The toolpath id.
    toolpath: String,
  },
  /// A thread's pitch is zero, negative or not a finite number.
  ///
  /// The pitch is how far the helix drops each revolution, so a pitch with no distance behind it asks
  /// for a descent that never descends. Refused here, as the document loads, and again by
  /// `cam-toolpath::thread`, which sees a caller that bypassed the CLI (M6 A7).
  #[error(
    "toolpath {toolpath:?} thread pitch must be a positive number of millimetres, got {value} mm"
  )]
  ThreadPitchNotPositive {
    /// The toolpath id.
    toolpath: String,
    /// The offending pitch in millimetres.
    value: f64,
  },
  /// A thread's cut depth does not extend below its start depth.
  ///
  /// The rule every one-pass strategy inherits from the profile: a helix whose cut depth is not below
  /// its start depth has no descent behind it (M6 A7).
  #[error(
    "toolpath {toolpath:?} thread cut depth {cut_depth} mm must be greater than its start depth \
     {start_depth} mm"
  )]
  ThreadDepthRange {
    /// The toolpath id.
    toolpath: String,
    /// The requested cut depth in millimetres.
    cut_depth: f64,
    /// The requested start depth in millimetres.
    start_depth: f64,
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
          | Operation::Chamfer(_)
          // A texture's grooves are clipped to a region, and an open vector bounds none (D-M6-6).
          | Operation::Texture(_)
          // An inlay grows a region and leaves it standing as an island, so its selection must bound
          // one: an open vector bounds none (M6 A6).
          | Operation::Inlay(_)
          // A thread runs around a circle, and an open vector bounds no circle (M6 A7).
          | Operation::Thread(_) => true,
          // Engraving an open vector is the strategy's whole point (M6 A3, D-M6-4): it cuts along the
          // vector and stops at its last vertex. Fluting is that same walk with a ramp at each end,
          // and a flute is one groove per vector, drawn as a line as often as a loop (A4), so the two
          // accept an open vector and nothing else here does.
          Operation::Engrave(_) | Operation::Fluting(_) => false,
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
      check_fluting(toolpath)?;
      check_texture(toolpath)?;
      check_inlay(toolpath)?;
      check_thread(toolpath)?;
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

/// Check a flute's own parameters: a ramp is a length along the vector, so it cannot be negative.
///
/// Only the sign is checked here. Whether the ramp *fits* — it may not be longer than half the vector —
/// needs the geometry's length, which the strategy measures when it walks the vector, and that is where
/// the refusal names both numbers (`ToolpathError::RampTooLong`, A4).
fn check_fluting(toolpath: &Toolpath) -> Result<(), ModelError> {
  let Operation::Fluting(params) = &toolpath.operation else {
    return Ok(());
  };
  let ramp = params.ramp.mm();
  if !ramp.is_finite() || ramp < 0.0 {
    return Err(ModelError::FlutingRampNegative {
      toolpath: toolpath.id.clone(),
      value: ramp,
    });
  }
  Ok(())
}

/// Check a texture's own parameters: a lattice of grooves needs a distance between them.
///
/// Only the spacing is checked here. Whether the pattern crosses the region at all needs the geometry,
/// which the strategy has when it asks for the grooves, and that is where the refusal names both
/// numbers (`ToolpathError::NoGroove`, D-M6-6).
fn check_texture(toolpath: &Toolpath) -> Result<(), ModelError> {
  let Operation::Texture(params) = &toolpath.operation else {
    return Ok(());
  };
  let spacing = params.spacing.mm();
  if !spacing.is_finite() || spacing <= 0.0 {
    return Err(ModelError::TextureSpacingNotPositive {
      toolpath: toolpath.id.clone(),
      value: spacing,
    });
  }
  Ok(())
}

/// Check an inlay pair's own parameters, and that each toolpath is one half of the pair.
///
/// The stepover and the depth range are the pocket's rules, refused here by name because the document
/// is the boundary that sees them before any toolpath exists. Which half a toolpath is comes from its
/// island list (see [`InlayParams`]), so a record that names both a boundary and an island is refused
/// rather than guessed at.
fn check_inlay(toolpath: &Toolpath) -> Result<(), ModelError> {
  let Operation::Inlay(params) = &toolpath.operation else {
    return Ok(());
  };
  let stepover = params.stepover.mm();
  if !stepover.is_finite() || stepover <= 0.0 {
    return Err(ModelError::InlayStepoverNotPositive {
      toolpath: toolpath.id.clone(),
      value: stepover,
    });
  }
  if params.cut_depth.mm() <= params.start_depth.mm() {
    return Err(ModelError::InlayDepthRange {
      toolpath: toolpath.id.clone(),
      cut_depth: params.cut_depth.mm(),
      start_depth: params.start_depth.mm(),
    });
  }
  if !toolpath.island_ids.is_empty() && !toolpath.entity_ids.is_empty() {
    return Err(ModelError::InlayMaleTakesNoEntities {
      toolpath: toolpath.id.clone(),
    });
  }
  Ok(())
}

/// Check a thread's own parameters: a helix needs a pitch to descend by and a depth to descend to.
///
/// Both are refused here, at the boundary that sees the document before any toolpath exists, and again
/// by `cam-toolpath::thread`. Whether the selection is a **circle** is not asked here: the model has no
/// opinion about a shape's identity beyond closed-or-open, and the strategy names the entity that is
/// not a circle (M6 A7).
fn check_thread(toolpath: &Toolpath) -> Result<(), ModelError> {
  let Operation::Thread(params) = &toolpath.operation else {
    return Ok(());
  };
  let pitch = params.pitch.mm();
  if !pitch.is_finite() || pitch <= 0.0 {
    return Err(ModelError::ThreadPitchNotPositive {
      toolpath: toolpath.id.clone(),
      value: pitch,
    });
  }
  if params.cut_depth.mm() <= params.start_depth.mm() {
    return Err(ModelError::ThreadDepthRange {
      toolpath: toolpath.id.clone(),
      cut_depth: params.cut_depth.mm(),
      start_depth: params.start_depth.mm(),
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
  if !matches!(
    toolpath.operation,
    Operation::Pocket(_) | Operation::Inlay(_)
  ) {
    return Err(ModelError::IslandsRequirePocketOrInlay {
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
    }) || (matches!(toolpath.operation, Operation::Inlay(_))
      && matches!(
        // An inlay male's boundary is the job rectangle, which is not an entity: its region has to sit
        // strictly inside the stock, or the male has nothing to clear around it (M6 A6, D-M6-7).
        cam_geom::winding::relation(island, &document.job.outline(), tolerance),
        cam_geom::winding::Relation::FirstInside
      ));
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
