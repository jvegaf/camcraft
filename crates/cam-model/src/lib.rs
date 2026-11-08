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

/// Parameters of a V-carve toolpath.
///
/// A V-carve is not a stack of constant-Z passes: the tip walks the **medial axis** of the selected
/// region at the depth its own clearance demands, so the surface it leaves is the distance field
/// (M7 A1). The depth law therefore lives here, beside the tool, the way [`ChamferParams::depth`]
/// does — one function that turns a clearance into a depth, called by `cam-toolpath::vcarve` and by
/// `cam-sim`'s claim, so the program and the authorisation cannot disagree about how deep a tip goes
/// (M6's D-M6-2 precedent).
///
/// There is deliberately **no `start_depth`** (D-M7-3). With a start depth `s` the tip sits `s` below
/// the surface, so the cone's intersection with the stock top reaches `s · tan(θ/2)` *outside* the
/// vectors and the authorised surface is no longer `-d(q)/tan(θ/2)` inside the region alone. That is
/// a claim of its own, and shipping it here would make the milestone's cleanest assertion — `gouge`
/// exactly zero — false for a legitimate program. The CLI refuses `--start-depth` by name rather
/// than storing a field that defaults to zero.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VCarveParams {
  /// Full included angle of the V-bit, as the tool library records it.
  ///
  /// The law is derived from **this** angle, not from the library's, because the document is what a
  /// later `emit` and `simulate` read; the toolpath authoring command writes the library's angle
  /// here, so the two agree by construction. A tool the library does not describe as an angled bit is
  /// refused by name at authoring and again at emission (D-M6-9): defaulting 90 degrees would cut a
  /// different carve.
  pub included_angle: Angle,
  /// The medial axis's lattice step, in millimetres (D-M7-5).
  ///
  /// It is the caller's tolerance in the only sense the milestone has: the axis is refined to this
  /// step, so it bounds the chording between certified vertices, the shortest piece of axis the walk
  /// can see, the band of material left standing near a convex corner, and the truncation at a
  /// curvature extremum. All four are `cam_geom::medial`'s declared costs and all four cost coverage
  /// only, never depth.
  ///
  /// Stored with the operation rather than passed to `emit`, because the program is recomputed from
  /// the document alone (S9, S10): a tolerance that was not stored would make `emit`'s output depend
  /// on a constant nobody named, and `cam-cli toolpath --tolerance` would be a silently dropped flag.
  /// Defaulted so a hand-written or older document that names a `v_carve` toolpath without the field
  /// loads as the axis the CLI's own default would have written.
  #[serde(default = "default_v_carve_tolerance")]
  pub tolerance: Length,
}

impl VCarveParams {
  /// The depth a tip whose clearance is `clearance` sits at: `clearance / tan(included_angle / 2)`.
  ///
  /// Derived here and nowhere else, so the toolpath that cuts and the simulation that checks it can
  /// never disagree about how deep the carve goes (A1). The formula does not clamp: an angle with no
  /// cone behind it is refused by [`Document::validate`] naming the value, and again by
  /// `cam-toolpath::vcarve`, because a clamped depth would cut a different carve than the one asked
  /// for.
  pub fn depth(&self, clearance: Length) -> Length {
    let half = self.included_angle.radians() / 2.0;
    Length::from_mm(clearance.mm() / half.tan())
  }
}

impl Default for VCarveParams {
  fn default() -> Self {
    Self {
      included_angle: Angle::from_degrees(90.0),
      tolerance: default_v_carve_tolerance(),
    }
  }
}

/// The default medial-axis lattice step for a V-carve: 0.1 mm (D-M7-5).
///
/// The measured cost of the coarser 0.5 mm step was 0.20 mm of missed depth near a convex corner and
/// an ellipse's axis 0.7 mm short of its analytic end; at 0.1 mm both are `0.00` mm. The default is
/// the finer number because the axis is the toolpath, and a mismatch between the two is material left
/// standing.
fn default_v_carve_tolerance() -> Length {
  Length::from_mm(0.1)
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

/// One point of a moulding cross-section: how deep the cut sits at a lateral distance from the rail.
///
/// S3: the rail goes in XY as the selected entity; the profile is authored data, a cross-section of
/// (lateral offset, Z depth) points the sweep walks while stepping over by `stepover`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MouldingProfilePoint {
  /// Lateral distance from the rail, in millimetres. May be negative (the other side of the rail).
  pub lateral: Length,
  /// Cut depth at this lateral distance, in millimetres: zero at the stock surface, negative below.
  pub depth: Length,
}

/// Parameters of a moulding toolpath: a cross-section swept along a rail with a stepover (S1, S3).
///
/// The rail is the selected entity (an open path or a closed loop); the profile and the stepover live
/// here, beside the operation, so `entity_ids` still carries a single selection list and documents
/// written before M8 load byte-identically (T1 design decision, see the feature document's L5).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MouldingParams {
  /// Cross-section points ordered from one side to the other; at least two.
  pub profile: Vec<MouldingProfilePoint>,
  /// Lateral distance between consecutive sweep passes; must be a positive finite number.
  pub stepover: Length,
}

impl Default for MouldingParams {
  fn default() -> Self {
    Self {
      profile: vec![
        MouldingProfilePoint {
          lateral: Length::from_mm(0.0),
          depth: Length::ZERO,
        },
        MouldingProfilePoint {
          lateral: Length::from_mm(6.0),
          depth: Length::from_mm(-4.0),
        },
        MouldingProfilePoint {
          lateral: Length::from_mm(12.0),
          depth: Length::ZERO,
        },
      ],
      stepover: Length::from_mm(2.0),
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
  /// Carve the selected closed region by walking its medial axis at the distance field's own depth.
  VCarve(VCarveParams),
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
  /// Sweep a cross-section along the selected rail with a stepover (M8, S3).
  Moulding(MouldingParams),
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
  /// Optional nesting section: the authored sheets, the parts, where each instance landed and the
  /// cost record the report reads (M9, S1-S4).
  ///
  /// Optional and top level, exactly like `tools_ref`/`postp`, so a document written before M9 loads
  /// and re-saves byte-identically: no existing field learns a new meaning, which is why the schema
  /// tag stays `c2d/document@2` (invariant 1).
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
  /// A V-carve's included angle is not strictly between 0 and 180 degrees.
  ///
  /// The depth law is `clearance / tan(angle / 2)`, so an angle at either end of the range has a
  /// tangent of zero or infinity and no depth behind it. Refused here, as the document loads, naming
  /// the value (A4), and again by `cam-toolpath::vcarve`, which sees a caller that bypassed the CLI.
  #[error(
    "toolpath {toolpath:?} V-carve included angle must be between 0 and 180 degrees, got {degrees} \
     degrees"
  )]
  VCarveAngleOutOfRange {
    /// The toolpath id.
    toolpath: String,
    /// The offending angle in degrees.
    degrees: f64,
  },
  /// A V-carve's medial-axis tolerance is zero, negative or not a finite number.
  ///
  /// The tolerance is the lattice step `cam_geom::medial` refines the axis to, so a step with no
  /// distance behind it is refused by name rather than clamped into a plausible one (D-M7-5).
  #[error(
    "toolpath {toolpath:?} V-carve tolerance must be a positive number of millimetres, got {value} mm"
  )]
  VCarveToleranceNotPositive {
    /// The toolpath id.
    toolpath: String,
    /// The offending tolerance in millimetres.
    value: f64,
  },
  /// A moulding profile has fewer than two points, so there is no cross-section to sweep.
  #[error("toolpath {toolpath:?} moulding profile needs at least two points, got {points}")]
  MouldingProfileTooShort {
    /// The toolpath id.
    toolpath: String,
    /// How many profile points were supplied.
    points: usize,
  },
  /// A moulding profile point is not finite, so no depth behind it exists.
  #[error(
    "toolpath {toolpath:?} moulding profile point must use finite millimetres, got lateral \
     {lateral} mm and depth {depth} mm"
  )]
  MouldingProfilePointNotFinite {
    /// The toolpath id.
    toolpath: String,
    /// The offending lateral distance in millimetres.
    lateral: f64,
    /// The offending depth in millimetres.
    depth: f64,
  },
  /// A moulding stepover is zero, negative or not a finite number.
  #[error(
    "toolpath {toolpath:?} moulding stepover must be a positive number of millimetres, got {value} mm"
  )]
  MouldingStepoverNotPositive {
    /// The toolpath id.
    toolpath: String,
    /// The offending stepover in millimetres.
    value: f64,
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
  /// A new, empty document for `job`.
  pub fn new(job: Job) -> Self {
    Self {
      schema: DOCUMENT_SCHEMA.to_owned(),
      job,
      layers: Vec::new(),
      toolpaths: Vec::new(),
      tools_ref: None,
      postp: None,
      nesting: None,
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
          // A V-carve's region is what the contours enclose, and its medial axis is the toolpath:
          // an open vector bounds no region to carve (M7 A4).
          | Operation::VCarve(_)
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
          // A moulding rail is a sweep path: an open vector sweeps a strip, a closed loop sweeps a
          // frame, so either bounds the rail and the closed-geometry rule does not apply (S3).
          Operation::Moulding(_) => false,
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
      check_v_carve(toolpath)?;
      check_fluting(toolpath)?;
      check_texture(toolpath)?;
      check_inlay(toolpath)?;
      check_thread(toolpath)?;
      check_moulding(toolpath)?;
      check_islands(self, toolpath)?;
    }

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
  // A box wider than its sheet's cell span is kept out of the grid entirely, in a short per-sheet wide
  // list. Stored in every cell it covers it would make each of those cells hold O(N) occupants, and
  // every placement would iterate all of them before the visited stamp discarded them one by one —
  // O(N²·sqrt N) stamp-loop steps for a document whose pairs are Θ(N²), which is exactly N full-width
  // strips stacked in y. The wide list gives that document the optimal Θ(N²) scan with no duplication,
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
    // Both ends are padded by `tolerance`, exactly as the query range below is, and that padding is what
    // makes two boxes that meet within the tolerance share a cell: the classifier accepts a pair whose
    // gap is at most the tolerance, so the index has to reach such a pair. Deleting the padding as
    // redundant would silently stop reporting every near-tolerance overlap. `cell_width` is floored at
    // the tolerance for the same reason: a narrower cell would make a padded box's range more than a
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

/// Check a V-carve's own parameters: the depth law needs an angle and the axis needs a step.
///
/// Both are refused naming the value rather than clamped, because a clamped carve is a different
/// shape: an angle at either end of `(0, 180)` degrees has no depth behind it, and a lattice step of
/// zero is an axis `cam_geom::medial` would refuse, which would leave a document whose only failure
/// mode is a later `emit` (A4, D-M7-5).
fn check_v_carve(toolpath: &Toolpath) -> Result<(), ModelError> {
  let Operation::VCarve(params) = &toolpath.operation else {
    return Ok(());
  };
  let angle = params.included_angle.radians();
  if !(angle > 0.0 && angle < std::f64::consts::PI) {
    return Err(ModelError::VCarveAngleOutOfRange {
      toolpath: toolpath.id.clone(),
      degrees: params.included_angle.degrees(),
    });
  }
  let tolerance = params.tolerance.mm();
  if !tolerance.is_finite() || tolerance <= 0.0 {
    return Err(ModelError::VCarveToleranceNotPositive {
      toolpath: toolpath.id.clone(),
      value: tolerance,
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

/// Check a moulding's own parameters: a sweep needs a cross-section and a stepover with distance
/// behind them.
///
/// A profile of fewer than two points sweeps nothing, a non-finite coordinate would emit an infinite
/// Z, and a stepover with no distance behind it cannot space passes. All three are refused naming the
/// offending value rather than clamped into a different moulding.
fn check_moulding(toolpath: &Toolpath) -> Result<(), ModelError> {
  let Operation::Moulding(params) = &toolpath.operation else {
    return Ok(());
  };
  if params.profile.len() < 2 {
    return Err(ModelError::MouldingProfileTooShort {
      toolpath: toolpath.id.clone(),
      points: params.profile.len(),
    });
  }
  for point in &params.profile {
    let lateral = point.lateral.mm();
    let depth = point.depth.mm();
    if !lateral.is_finite() || !depth.is_finite() {
      return Err(ModelError::MouldingProfilePointNotFinite {
        toolpath: toolpath.id.clone(),
        lateral,
        depth,
      });
    }
  }
  let stepover = params.stepover.mm();
  if !stepover.is_finite() || stepover <= 0.0 {
    return Err(ModelError::MouldingStepoverNotPositive {
      toolpath: toolpath.id.clone(),
      value: stepover,
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
