//! Toolpath strategies: turn document geometry plus a tool into the moves a machine executes.
//!
//! M0 implemented `profile` and the rules every other strategy reuses: the side offset, the
//! allowance, depth chunking, tab lifting and lead moves. M4 adds the two strategies that define 2.5D
//! routing: `pocket` (offset rings or raster rows) and `drill` (pecks, retracts, drill banks). M6 adds
//! the depth-variant strategies: `chamfer`, one V-bit pass whose Z is derived from the bit's own
//! included angle instead of authored, `engrave`, one authored-depth pass along open or closed
//! vectors with no pass law at all, `fluting`, one pass along a vector that ramps in over its
//! first millimetres and out over its last, `texture`, one authored-depth pass along each groove
//! of a pattern clipped to a closed region, `inlay`, the male/female pair of pockets one command
//! appends: a cavity grown by the allowance and a plug of the region itself, and `thread`, one helical
//! descent around a circle at a pitch per revolution — the one strategy whose program is not a stack of
//! constant-Z moves. M7 adds `vcarve`, which walks the selected region's medial axis with one tip per
//! vertex at the depth that vertex's own clearance demands, so its program is a depth field rather than
//! a set of passes at all. M8 adds `moulding`, which sweeps an authored cross-section along a rail with a
//! stepover: one walk of the rail per overlapped lateral pass, each at the depth the cross-section
//! authors there.
//!
//! Must not depend on `cam-post` or any UI crate.

#![warn(missing_docs)]

pub mod chamfer;
pub mod drill;
pub mod engrave;
pub mod fluting;
pub mod inlay;
pub mod moulding;
pub mod pocket;
pub mod texture;
pub mod thread;
pub mod vcarve;

pub use chamfer::{chamfer, ChamferRequest};
pub use drill::{circle_centre, drill, DrillHole, DrillRequest, Drillpath};
pub use engrave::{engrave, EngraveRequest};
pub use fluting::{fluting, FlutingRequest};
pub use inlay::{female, inlay, male, InlayRequest, Inlaypath};
pub use moulding::{moulding, MouldingRequest};
pub use pocket::{pocket, PocketRequest, Pocketpath};
pub use texture::{texture, TextureRequest};
pub use thread::{thread, ThreadRequest};
pub use vcarve::{vcarve, VCarveRequest};

use cam_geom::{Contour, LineSeg, Point2, Segment};
use cam_model::{Document, Job, Operation, ProfileParams, ProfileSide, Tab, Toolpath};
use cam_move::{Move, MoveStream, Point3, Segment as MoveSegment, ToolShape};
use cam_units::{Angle, Length};

/// The subset of tool data a toolpath strategy needs.
///
/// `diameter` is what the offset strategies and the cylinder footprint need; `included_angle` is what a
/// V-bit strategy needs, and it is an `Option` because the library records an angle only for tools that
/// have one (D-M6-9).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToolSpec {
  /// Tool number used in the program.
  pub number: u32,
  /// Cutting diameter.
  pub diameter: Length,
  /// Full included angle of the tool's cone, when the library records one.
  ///
  /// `None` for a tool the library does not describe as a V-bit. A strategy that derives geometry from
  /// the angle refuses such a tool by name instead of assuming 90 degrees, because a defaulted angle
  /// silently cuts a different bevel (D-M6-9).
  pub included_angle: Option<Angle>,
  /// Feed rate along the cut.
  pub feed_rate: f64,
  /// Plunge rate.
  pub plunge_rate: f64,
  /// Spindle speed in rpm.
  pub spindle_speed: u32,
}

/// A tool as [`program_for`] needs it: the strategy's [`ToolSpec`] plus the flat at the bit's tip.
///
/// The flat is the one thing a program reads that no strategy does. A strategy asks for a cone or a
/// cylinder and the tip's flat is part of the *shape the simulation sweeps*, not of the path it
/// computes — so it belongs to the program and not to [`ToolSpec`], which every strategy takes.
/// `cam-toolpath` has no library to read it from (S6): the caller, which knows both its own tool
/// record and this crate's types, maps one onto the other and hands the list over.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProgramTool {
  /// The tool the strategies read.
  pub spec: ToolSpec,
  /// Width of the flat at the tip, zero for a sharp point.
  ///
  /// A caller whose library records none passes [`Length::ZERO`], which is what the shape means: a
  /// cone that comes to a point.
  pub flat_diameter: Length,
}

/// Why a toolpath could not be computed.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum ToolpathError {
  /// Every cutting operation needs a closed contour.
  #[error("cutting toolpaths need closed geometry")]
  OpenContour,
  /// The tool has no usable diameter.
  #[error("tool diameter must be greater than zero")]
  InvalidToolDiameter,
  /// A per-pass depth of zero would never reach the cut depth.
  #[error("pass depth must be greater than zero")]
  InvalidPassDepth,
  /// The cut depth does not extend below the start depth.
  #[error("cut depth {cut_depth} mm must be greater than start depth {start_depth} mm")]
  InvalidDepthRange {
    /// Requested cut depth in millimetres.
    cut_depth: f64,
    /// Requested start depth in millimetres.
    start_depth: f64,
  },
  /// The side offset collapsed the contour, usually because the tool is bigger than the feature.
  #[error("the tool does not fit: {reason}")]
  OffsetCollapsed {
    /// What collapsed.
    reason: String,
  },
  /// A tab position lies outside `[0, 1)`.
  #[error("tab {index} position {position} must be within [0, 1)")]
  TabOutOfRange {
    /// Index of the offending tab.
    index: usize,
    /// Its requested position.
    position: f64,
  },
  /// Adjacent rings or rows would not overlap, leaving material between them.
  #[error("stepover {stepover} mm must be greater than zero")]
  InvalidStepover {
    /// The offending stepover in millimetres.
    stepover: f64,
  },
  /// A negative peck depth would plunge upwards.
  #[error("peck depth {peck_depth} mm must not be negative")]
  InvalidPeckDepth {
    /// The offending peck depth in millimetres.
    peck_depth: f64,
  },
  /// A hole is not a circle, so it has no centre to drill or to orbit.
  #[error("entity {entity:?} is not a circle, so it has no centre to work from")]
  NotACircle {
    /// Id of the offending entity.
    entity: String,
  },
  /// A drilling pass was given no holes.
  #[error("drilling needs at least one hole")]
  NoHoles,
  /// A drill bank was described with no drills, or with drills that overlap.
  #[error("drill bank needs at least one drill and a positive pitch")]
  InvalidBank,
  /// A chamfer was given an included angle with no finite tangent.
  ///
  /// The document model refuses an impossible angle for a stored toolpath, but a caller that hands the
  /// strategy its parameters directly must not receive an infinite Z either: the tangent of zero is
  /// zero, so there is no depth behind the bevel (D-M6-10).
  #[error("chamfer included angle {degrees} degrees must be strictly between 0 and 180")]
  InvalidIncludedAngle {
    /// The offending angle in degrees.
    degrees: f64,
  },
  /// A flute's ramp is negative, which would enter upwards.
  #[error("fluting ramp {ramp} mm must not be negative")]
  InvalidRamp {
    /// The offending ramp in millimetres.
    ramp: f64,
  },
  /// A flute's ramp is longer than half the vector it runs on, so its entry and its exit would meet.
  ///
  /// Shortening the ramp silently is exactly what this refusal exists to prevent: the document asks
  /// for a ramped entry and a ramped exit of that length, and a silently shortened one cuts both ends
  /// at a depth nobody authorised (A4). Both numbers are named, so the message says which pair cannot
  /// fit.
  #[error("fluting ramp {ramp} mm is longer than half of the vector's {length} mm length")]
  RampTooLong {
    /// The offending ramp in millimetres.
    ramp: f64,
    /// The vector's length in millimetres.
    length: f64,
  },
  /// A thread's pitch is zero or negative, so one revolution descends nowhere.
  ///
  /// The document model refuses it as the document loads, but a caller that hands the strategy its
  /// parameters directly reaches it without passing through there. A pitch with no distance behind it
  /// is an input error and never a helix that stays at one depth (M6 A7).
  #[error("thread pitch {pitch} mm must be greater than zero")]
  InvalidPitch {
    /// The offending pitch in millimetres.
    pitch: f64,
  },
  /// An internal thread's tool is at least as wide as the hole it is meant to cut inside.
  ///
  /// The tool centre orbited at `(D − t)/2`, which is not a radius when `t ≥ D`: the tool's centre would
  /// have to sit on the hole's axis or outside it. Refused by name rather than collapsed into a plunge on
  /// the axis, and the same tool outside the same circle is the external case, where it fits by
  /// construction (M6 A7).
  #[error(
    "tool diameter {diameter} mm does not fit a {hole} mm hole: an internal thread's tool centre orbits \
     at (D - t)/2, which has no radius when the tool is as wide as the hole"
  )]
  ThreadToolTooWide {
    /// The tool's diameter in millimetres.
    diameter: f64,
    /// The hole's diameter in millimetres.
    hole: f64,
  },
  /// A texture's spacing is not a positive number of millimetres.
  ///
  /// The document model refuses it as the document loads, but a caller that hands the strategy its
  /// parameters directly reaches it without passing through there. A zero spacing is an input error and
  /// never a silently empty pattern (D-M6-6).
  #[error("texture spacing {spacing} mm must be greater than zero")]
  InvalidSpacing {
    /// The offending spacing in millimetres.
    spacing: f64,
  },
  /// A texture's spacing yields no groove across the region, so the pattern would cut nothing.
  ///
  /// The lattice is the region's own extent divided by the spacing, so a spacing wider than the region
  /// is a request with no groove behind it. Emitting an empty program instead would look like a
  /// strategy that ran and found nothing to do, which is the one thing a mis-typed spacing must never
  /// look like (D-M6-6).
  #[error(
    "texture spacing {spacing} mm at {angle} degrees leaves no groove across the region: the \
     pattern would cut nothing, so this is a spacing with no pattern behind it"
  )]
  NoGroove {
    /// The offending spacing in millimetres.
    spacing: f64,
    /// The groove direction in degrees.
    angle: f64,
  },
  /// An inlay's allowance shrinks the region until there is none of it left.
  ///
  /// A negative allowance is an interference fit — the cavity smaller than the plug — so it is a
  /// legitimate request up to the point where the region has no area left. The kernel is the only thing
  /// that can say where that point is, and the refusal names the allowance rather than surfacing as a
  /// pocket that "does not fit", which would blame the tool for an allowance (M6 A6, D-M6-7).
  #[error("inlay allowance {allowance} mm leaves no region to cut: {reason}")]
  InlayAllowanceCollapsed {
    /// The offending allowance in millimetres.
    allowance: f64,
    /// What the kernel said, verbatim.
    reason: String,
  },
  /// An inlay's region does not lie strictly inside the job rectangle.
  ///
  /// The male clears the stock *around* the region, so a region that reaches or crosses the stock's edge
  /// has no material beside it to clear and the plug would come out bounded by the saw cut instead of by
  /// a machined wall (M6 A6, D-M6-7).
  #[error(
    "inlay region is not strictly inside the job's {width} x {height} mm rectangle: the male clears \
     the stock around the region, so a region on or across the stock's edge leaves nothing for it to \
     clear"
  )]
  InlayRegionOutsideJob {
    /// The job's width in millimetres.
    width: f64,
    /// The job's height in millimetres.
    height: f64,
  },
  /// A V-carve was asked of a tool the library does not describe as an angled bit.
  ///
  /// The carve's whole depth law is `clearance / tan(θ/2)`, so a cylinder has no angle to derive it
  /// from and defaulting 90 degrees would cut a different carve than the one asked for (D-M6-9).
  #[error(
    "tool T{tool_number} has no included angle in the library, so a V-carve's depth cannot be \
     derived: a V-carve needs a V-bit"
  )]
  VCarveNeedsAngledTool {
    /// The tool the caller named.
    tool_number: u32,
  },
  /// A V-carve was asked of a bit whose tip has a flat.
  ///
  /// With a flat of diameter `f` the tool cannot carve to a sharp edge: the surface its tip leaves is
  /// `f/2 · cot(θ/2)` wide at the bottom, while the depth law the whole operation stands on reaches
  /// zero exactly at the vectors. That is the chamfer's `D-M6-10` in the other direction, and it is
  /// refused by name rather than absorbed.
  #[error(
    "tool T{tool_number} has a {flat_diameter} mm flat tip, so it cannot carve to a sharp edge: a \
     V-carve's depth law is the cone of a sharp point, so a V-carve needs a sharp V-bit"
  )]
  VCarveNeedsSharpBit {
    /// The tool the caller named.
    tool_number: u32,
    /// The flat at its tip, in millimetres.
    flat_diameter: f64,
  },
  /// The medial axis a V-carve walks could not be computed.
  ///
  /// `cam_geom::medial` refuses a tolerance that is not positive and finite, and a lattice the
  /// tolerance would make larger than its own sample cap. Neither is clamped into a coarser axis: a
  /// silently coarsened tolerance changes what the carve leaves standing (D-M7-5).
  #[error("the V-carve's medial axis could not be computed: {reason}")]
  MedialAxis {
    /// What the kernel said, verbatim.
    reason: String,
  },
  /// A V-carve's region has no medial axis at all, so there is nothing to carve.
  ///
  /// The axis's lattice step is the operation's tolerance, and a step at least as large as the
  /// region's own clearance puts every point of it within that step of the boundary: no point is
  /// material, no candidate is certified, and the axis comes back with no branch. A program built
  /// from that is zero moves, and `simulate` then reports the whole field as uncut — an input the
  /// command can see, so it is refused by name with both numbers in the message rather than written
  /// as a document whose only outcome is a later exit `6` (M7 A5, D-M7-5).
  #[error(
    "the V-carve's {tolerance} mm tolerance leaves nothing to carve in the {width} x {height} mm \
     region: every point of the region is within that distance of its boundary, so the medial axis \
     is empty and there is no tip to plunge"
  )]
  VCarveNothingToCarve {
    /// The axis's lattice step, in millimetres.
    tolerance: f64,
    /// The region's extent along X, in millimetres.
    width: f64,
    /// The region's extent along Y, in millimetres.
    height: f64,
  },
  /// A toolpath names a tool the caller's spec list does not hold.
  ///
  /// The library is not this crate's business (S6), so the list it is handed **is** the library: a
  /// number the list does not hold is refused by name, and the toolpath that asked for it is named
  /// with it.
  #[error("tool T{tool_number} used by toolpath {toolpath:?} is not in the library")]
  ToolNotInLibrary {
    /// The number the toolpath named.
    tool_number: u32,
    /// Id of the toolpath that named it.
    toolpath: String,
  },
  /// The tool has no diameter, so no offset can be computed from it.
  #[error("tool T{tool_number} has no diameter, so a profile offset cannot be computed")]
  ToolWithoutDiameter {
    /// The number of the offending tool.
    tool_number: u32,
  },
  /// A chamfer's tool is not an angled bit, so the bevel's depth has no angle to derive it from.
  #[error(
    "tool T{tool_number} has no included angle, so toolpath {toolpath:?} cannot cut a chamfer: a \
     V-bit strategy needs the tool's angle, and defaulting 90 degrees would cut a different bevel"
  )]
  ChamferNeedsAngle {
    /// The number of the offending tool.
    tool_number: u32,
    /// Id of the toolpath that asked for the chamfer.
    toolpath: String,
  },
  /// A chamfer's tool has a flat tip, which widens the bevel past the width the document authored.
  #[error(
    "tool T{tool_number} has a {flat_diameter} mm flat tip, so toolpath {toolpath:?} would cut a \
     bevel wider than its {chamfer_width} mm chamfer width: a chamfer needs a sharp V-bit"
  )]
  ChamferNeedsSharpBit {
    /// The number of the offending tool.
    tool_number: u32,
    /// Id of the toolpath that asked for the chamfer.
    toolpath: String,
    /// The flat at the tool's tip, in millimetres.
    flat_diameter: f64,
    /// The chamfer width the document authored, in millimetres.
    chamfer_width: f64,
  },
  /// A V-carve's tool is not an angled bit, so the carve's depth law has no angle to stand on.
  #[error(
    "tool T{tool_number} has no included angle, so toolpath {toolpath:?} cannot carve: a V-carve's \
     depth cannot be derived, and defaulting 90 degrees would cut a different carve"
  )]
  VCarveNeedsAngle {
    /// The number of the offending tool.
    tool_number: u32,
    /// Id of the toolpath that asked for the carve.
    toolpath: String,
  },
  /// A V-carve's tool has a flat tip, so its depth law — a sharp point's cone — does not hold.
  #[error(
    "tool T{tool_number} has a {flat_diameter} mm flat tip, so toolpath {toolpath:?} would not \
     carve to a sharp edge: a V-carve needs a sharp V-bit"
  )]
  VCarveNeedsSharpTip {
    /// The number of the offending tool.
    tool_number: u32,
    /// Id of the toolpath that asked for the carve.
    toolpath: String,
    /// The flat at the tool's tip, in millimetres.
    flat_diameter: f64,
  },
  /// A toolpath cites an entity the document does not have.
  #[error("toolpath {toolpath:?} cites unknown entity {entity:?}")]
  UnknownEntity {
    /// Id of the toolpath that cites it.
    toolpath: String,
    /// Id of the entity it cites.
    entity: String,
  },
  /// An entity a toolpath cuts is not a contour.
  #[error("entity {entity:?} is not a contour")]
  NotAContour {
    /// Id of the offending entity.
    entity: String,
  },
  /// A toolpath cites an island the document does not have.
  #[error("toolpath {toolpath:?} cites unknown island {island:?}")]
  UnknownIsland {
    /// Id of the toolpath that cites it.
    toolpath: String,
    /// Id of the island it cites.
    island: String,
  },
  /// An island a toolpath leaves standing is not a contour.
  #[error("island {island:?} is not a contour")]
  IslandNotAContour {
    /// Id of the offending island.
    island: String,
  },
  /// A moulding profile has fewer than two points, so no cross-section exists to sweep.
  ///
  /// The document model refuses it as the document loads (M8 T1), but a caller that hands the strategy
  /// its parameters directly reaches it without passing through there, and a profile of one point sweeps
  /// nothing at all.
  #[error("a moulding profile needs at least two points, got {points}")]
  MouldingProfileTooShort {
    /// How many points the profile held.
    points: usize,
  },
  /// A moulding profile's lateral distances do not strictly increase.
  ///
  /// A cross-section is one depth per lateral distance, so a lateral that does not exceed the one before
  /// it is two depths for the same place. Refused with both values and the offending index rather than
  /// resolved by picking one, because either pick cuts a profile the document did not author (S3).
  #[error(
    "moulding profile point {index} at lateral {lateral} mm must be greater than the previous \
     {previous} mm: a cross-section is one depth per lateral distance"
  )]
  MouldingProfileLateral {
    /// Index of the offending point in the profile.
    index: usize,
    /// The lateral distance of the point before it, in millimetres.
    previous: f64,
    /// The offending lateral distance in millimetres.
    lateral: f64,
  },
  /// The geometry kernel refused a moulding's geometry for a reason the strategy does not restate.
  ///
  /// A rail with no length is the case this is reached for: it has no sweep behind it, and a program of
  /// zero moves would look like a strategy that ran and found nothing to cut. The kernel's sentence is
  /// kept verbatim instead of being guessed at (the `MedialAxis` precedent).
  #[error("the moulding's geometry could not be built: {reason}")]
  MouldingGeometry {
    /// What the kernel said, verbatim.
    reason: String,
  },
  /// A strategy refused the toolpath it was handed.
  ///
  /// The strategy's own refusal is the cause and keeps its wording; this states which toolpath asked
  /// for it, so a document with many toolpaths says which one cannot be cut.
  #[error("toolpath {toolpath:?}: {source}")]
  InToolpath {
    /// Id of the toolpath that could not be cut.
    toolpath: String,
    /// The strategy's refusal.
    source: Box<ToolpathError>,
  },
}

/// Everything a profile pass needs.
#[derive(Clone, Copy, Debug)]
pub struct ProfileRequest<'a> {
  /// The closed geometry to cut.
  pub contour: &'a Contour,
  /// Side, allowance, depths, leads and tabs.
  pub params: &'a ProfileParams,
  /// The tool doing the cutting.
  pub tool: &'a ToolSpec,
  /// Job setup, used to map document coordinates onto machine coordinates.
  pub job: &'a Job,
}

/// Compute the moves for a profile cut.
///
/// The moves always retract to the job's safe Z between passes: a wrong-depth pass is a ruined part,
/// and staying down between passes buys nothing on a profile cut.
pub fn profile(request: &ProfileRequest<'_>) -> Result<Vec<Move>, ToolpathError> {
  if !request.contour.is_closed() {
    return Err(ToolpathError::OpenContour);
  }
  if request.tool.diameter.mm() <= 0.0 {
    return Err(ToolpathError::InvalidToolDiameter);
  }
  let params = request.params;
  if params.pass_depth.mm() <= 0.0 {
    return Err(ToolpathError::InvalidPassDepth);
  }
  if params.cut_depth.mm() <= params.start_depth.mm() {
    return Err(ToolpathError::InvalidDepthRange {
      cut_depth: params.cut_depth.mm(),
      start_depth: params.start_depth.mm(),
    });
  }
  for (index, tab) in params.tabs.iter().enumerate() {
    if !(0.0..1.0).contains(&tab.position) {
      return Err(ToolpathError::TabOutOfRange {
        index,
        position: tab.position,
      });
    }
  }

  let lateral = lateral_offset(params, request.tool);
  let cut = if lateral.mm() == 0.0 {
    request.contour.clone()
  } else {
    cam_geom::offset::offset_contour(
      request.contour,
      lateral.mm(),
      &cam_geom::offset::OffsetOptions::default(),
    )
    .map_err(|error| ToolpathError::OffsetCollapsed {
      reason: error.to_string(),
    })?
    .contour
  };

  let safe_z = request.job.safe_z;
  let passes = pass_depths(
    params.start_depth.mm(),
    params.cut_depth.mm(),
    params.pass_depth.mm(),
  )?;
  let mut moves = Vec::new();

  for depth in passes {
    // Start point: the lead-in origin, so the plunge never happens on the finished edge.
    let start = cut.segments().first().map(|segment| segment.start_point());
    let Some(start) = start else {
      return Err(ToolpathError::OffsetCollapsed {
        reason: "the offset contour has no segments".to_owned(),
      });
    };
    let approach = approach_point(&cut, start, params.lead_in);
    let approach_machine = machine_point(request.job, approach);
    let surface_machine = machine_point(request.job, start);

    moves.push(Move::Rapid {
      to: Point3 {
        x: Length::from_mm(approach_machine.x),
        y: Length::from_mm(approach_machine.y),
        z: safe_z,
      },
    });
    moves.push(Move::Feed {
      to: Point3 {
        x: Length::from_mm(approach_machine.x),
        y: Length::from_mm(approach_machine.y),
        z: Length::from_mm(-depth),
      },
    });
    if params.lead_in.mm() > 0.0 {
      moves.push(Move::Feed {
        to: Point3 {
          x: Length::from_mm(surface_machine.x),
          y: Length::from_mm(surface_machine.y),
          z: Length::from_mm(-depth),
        },
      });
    }

    let lift_for = |tab: &Tab| -> Option<Length> {
      // A tab only matters once the pass is deeper than the material the tab keeps behind.
      let keep_level = params.cut_depth.mm() - tab.height.mm();
      if depth > keep_level + 1.0e-9 {
        Some(tab.height)
      } else {
        None
      }
    };
    moves.extend(walk_contour(
      &cut,
      request.job,
      -depth,
      &params.tabs,
      &lift_for,
    ));

    if params.lead_out.mm() > 0.0 {
      let end = cut
        .segments()
        .last()
        .map(|segment| segment.end_point())
        .unwrap_or(start);
      let departure = departure_point(&cut, end, params.lead_out);
      let departure_machine = machine_point(request.job, departure);
      moves.push(Move::Feed {
        to: Point3 {
          x: Length::from_mm(departure_machine.x),
          y: Length::from_mm(departure_machine.y),
          z: Length::from_mm(-depth),
        },
      });
    }

    let last = moves
      .last()
      .map(|movement| movement.destination())
      .unwrap_or(Point3::ORIGIN);
    moves.push(Move::Retract {
      to: Point3 {
        x: last.x,
        y: last.y,
        z: safe_z,
      },
    });
  }

  Ok(moves)
}

/// The lateral distance between the vector and the tool centre.
fn lateral_offset(params: &ProfileParams, tool: &ToolSpec) -> Length {
  let radius = tool.diameter.mm() / 2.0;
  match params.side {
    ProfileSide::On => Length::ZERO,
    ProfileSide::Outside => Length::from_mm(radius + params.allowance.mm()),
    // Growing the region is a positive offset, so cutting inside shrinks it.
    ProfileSide::Inside => Length::from_mm(-(radius + params.allowance.mm())),
  }
}

/// Depth of each pass, from the start depth down to the cut depth.
///
/// Shared by every strategy that chunks depth, so a pocket and a profile with the same numbers cut
/// the same number of passes at the same depths.
pub(crate) fn pass_depths(start: f64, cut: f64, step: f64) -> Result<Vec<f64>, ToolpathError> {
  let mut passes = Vec::new();
  let mut depth = start;
  while depth < cut - 1.0e-9 {
    depth = (depth + step).min(cut);
    passes.push(depth);
  }
  if passes.is_empty() {
    return Err(ToolpathError::InvalidDepthRange {
      cut_depth: cut,
      start_depth: start,
    });
  }
  Ok(passes)
}

/// Map a document point onto machine coordinates for the job's origin corner.
pub fn machine_point(job: &Job, point: Point2) -> Point2 {
  // One definition, in the model: the simulation reads a program back with the same map (M7 A7).
  job.frame().to_machine(point)
}

/// The sense an arc keeps when the job's frame maps it onto the machine.
///
/// A frame is not only a position: `top_left` is a **reflection**, and a reflection reverses
/// orientation, so a curve the document traverses counter-clockwise is traversed clockwise in the
/// machine's own numbering. Mapping an arc's points and keeping its `ccw` therefore states the
/// **complementary** arc — a `G03` where the machine needs `G02`, which is a wrong cut rather than a
/// wrong report. `center` translates and keeps handedness, so it flips nothing (M7 A7, L6). The map
/// itself is the model's, exactly as it is for [`machine_point`]: this only asks it.
fn machine_ccw(job: &Job, ccw: bool) -> bool {
  ccw != job.frame().is_reflection()
}

/// A point `lead` millimetres behind `start` along the contour's direction of travel.
fn approach_point(contour: &Contour, start: Point2, lead: Length) -> Point2 {
  if lead.mm() <= 0.0 {
    return start;
  }
  let tangent = start_tangent(contour);
  start - tangent * lead.mm()
}

/// A point `lead` millimetres beyond `end` along the contour's direction of travel.
fn departure_point(contour: &Contour, end: Point2, lead: Length) -> Point2 {
  if lead.mm() <= 0.0 {
    return end;
  }
  let tangent = end_tangent(contour);
  end + tangent * lead.mm()
}

fn start_tangent(contour: &Contour) -> cam_geom::Vec2 {
  contour
    .segments()
    .first()
    .map(|segment| tangent_of(*segment))
    .unwrap_or(cam_geom::Vec2::X_AXIS)
}

fn end_tangent(contour: &Contour) -> cam_geom::Vec2 {
  contour
    .segments()
    .last()
    .map(|segment| tangent_of(*segment))
    .unwrap_or(cam_geom::Vec2::X_AXIS)
}

/// Direction of travel at the start of a segment.
fn tangent_of(segment: Segment) -> cam_geom::Vec2 {
  match segment {
    Segment::Line(line) => line.direction().unwrap_or(cam_geom::Vec2::X_AXIS),
    Segment::Arc(arc) => arc.tangent_at_start(),
  }
}

/// Walk the contour once at `z`, splitting at tab boundaries and lifting where a tab applies.
///
/// Shared with `chamfer`, which has no tabs: with an empty `tabs` the split is the whole contour, so a
/// one-pass strategy gets the same arc-preserving walk a profile pass does instead of a second
/// implementation of it.
pub(crate) fn walk_contour(
  contour: &Contour,
  job: &Job,
  z: f64,
  tabs: &[Tab],
  lift_for: &dyn Fn(&Tab) -> Option<Length>,
) -> Vec<Move> {
  let segments = contour.segments();
  let lengths: Vec<f64> = segments.iter().map(|segment| segment.length()).collect();
  let total: f64 = lengths.iter().sum();
  let mut moves = Vec::new();
  let mut current_z = z;

  if tabs.is_empty() || total <= 0.0 {
    for (index, segment) in segments.iter().enumerate() {
      let (from, _) = piece_endpoints(*segment, 0.0, 1.0);
      let _ = index;
      let to = piece_endpoints(*segment, 0.0, 1.0).1;
      push_horizontal(&mut moves, job, *segment, from, to, z);
    }
    return moves;
  }

  // Boundaries along the whole contour: 0, each tab edge, each tab start, and the total.
  let spans = tab_spans(tabs, total);
  for (index, segment) in segments.iter().enumerate() {
    let segment_start: f64 = lengths[..index].iter().sum();
    let segment_end = segment_start + lengths[index];
    let length = lengths[index];
    if length <= 0.0 {
      continue;
    }

    // Every boundary that falls inside this segment, in order, plus its own ends.
    let mut cuts: Vec<f64> = vec![segment_start];
    for span in &spans {
      for boundary in [span.start, span.end] {
        if boundary > segment_start + 1.0e-9 && boundary < segment_end - 1.0e-9 {
          cuts.push(boundary);
        }
      }
    }
    cuts.push(segment_end);
    cuts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    cuts.dedup_by(|a, b| (*a - *b).abs() < 1.0e-9);

    for window in cuts.windows(2) {
      let (from_along, to_along) = (window[0], window[1]);
      let from_fraction = (from_along - segment_start) / length;
      let to_fraction = (to_along - segment_start) / length;
      let (from, to) = piece_endpoints(*segment, from_fraction, to_fraction);

      let lift = spans
        .iter()
        .find(|span| span.start <= from_along + 1.0e-9 && span.end >= to_along - 1.0e-9)
        .and_then(|span| lift_for(&tabs[span.tab_index]))
        .map(Length::mm)
        .unwrap_or(0.0);
      let target_z = z + lift;

      if (target_z - current_z).abs() > 1.0e-9 {
        let boundary = machine_point(job, from);
        moves.push(Move::Feed {
          to: Point3 {
            x: Length::from_mm(boundary.x),
            y: Length::from_mm(boundary.y),
            z: Length::from_mm(target_z),
          },
        });
        current_z = target_z;
      }

      let piece = sub_segment(*segment, from_fraction, to_fraction);
      push_horizontal(&mut moves, job, piece, from, to, target_z);
    }
  }

  moves
}

/// A region of the contour covered by one tab.
struct Span {
  start: f64,
  end: f64,
  tab_index: usize,
}

/// The perimeter spans covered by tabs, with wrap-around applied.
fn tab_spans(tabs: &[Tab], total: f64) -> Vec<Span> {
  let mut spans = Vec::with_capacity(tabs.len());
  for (index, tab) in tabs.iter().enumerate() {
    let centre = tab.position * total;
    let half = tab.width.mm() / 2.0;
    let mut start = centre - half;
    let mut end = centre + half;
    if start < 0.0 {
      start += total;
      end += total;
    }
    if end > total {
      // A tab straddling the contour's start splits into two spans.
      spans.push(Span {
        start: 0.0,
        end: end - total,
        tab_index: index,
      });
      spans.push(Span {
        start,
        end: total,
        tab_index: index,
      });
    } else {
      spans.push(Span {
        start,
        end,
        tab_index: index,
      });
    }
  }
  spans.sort_by(|a, b| {
    a.start
      .partial_cmp(&b.start)
      .unwrap_or(std::cmp::Ordering::Equal)
  });
  spans
}

/// Walk the contour from arc length `from` to `to` at a constant `z`, keeping arcs as arcs.
///
/// [`walk_contour`] walks the whole contour; a flute's plateau is the middle of one, between the two
/// ramps. The pieces go through the same [`sub_segment`] and [`push_horizontal`] pair, so a plateau
/// that lands on an arc is still cut as an arc.
pub(crate) fn walk_range(contour: &Contour, job: &Job, from: f64, to: f64, z: f64) -> Vec<Move> {
  let mut moves = Vec::new();
  let mut walked = 0.0;
  for segment in contour.segments() {
    let length = segment.length();
    let start = walked;
    walked += length;
    if length <= 0.0 {
      continue;
    }
    let piece_from = from.max(start);
    let piece_to = to.min(start + length);
    if piece_to <= piece_from + 1.0e-12 {
      continue;
    }
    let first = (piece_from - start) / length;
    let last = (piece_to - start) / length;
    let (from_point, to_point) = piece_endpoints(segment, first, last);
    push_horizontal(
      &mut moves,
      job,
      sub_segment(segment, first, last),
      from_point,
      to_point,
      z,
    );
  }
  moves
}

/// Append a cutting move along `segment` whose Z is `z`, as an arc when the piece is one.
///
/// The helical sibling of [`push_horizontal`], and the reason T4's ramp refusal could go: a piece of an
/// arc that also changes Z is a [`Move::Helix`], so a ramp riding on a curve is expressed by the move
/// vocabulary itself instead of being chorded (M6 A7, D-M6-8). A line stays a line.
pub(crate) fn push_ramp(moves: &mut Vec<Move>, job: &Job, segment: Segment, to: Point2, z: f64) {
  let destination = machine_point(job, to);
  match segment {
    Segment::Line(_) => moves.push(Move::Feed {
      to: Point3 {
        x: Length::from_mm(destination.x),
        y: Length::from_mm(destination.y),
        z: Length::from_mm(z),
      },
    }),
    Segment::Arc(arc) => {
      let center = machine_point(job, arc.center());
      moves.push(Move::Helix {
        to: Point3 {
          x: Length::from_mm(destination.x),
          y: Length::from_mm(destination.y),
          z: Length::from_mm(z),
        },
        center,
        ccw: machine_ccw(job, arc.is_ccw()),
      });
    }
  }
}

/// Walk the contour from arc length `from` to `to`, with `z` running linearly between `z_from` and
/// `z_to` along it: a ramp that follows the vector instead of chording across it.
///
/// The plateau's [`walk_range`] at a constant `z` is the same walk with both ends equal. Each piece's Z
/// is interpolated by **arc length**, so the depth the tool has reached depends on how far along the ramp
/// it is and on nothing else — which is exactly the law `cam-sim`'s flute claim authorises, and the law
/// the sweep applies to the moves this emits (D-M6-12).
#[allow(clippy::too_many_arguments)]
pub(crate) fn ramp_range(
  contour: &Contour,
  job: &Job,
  from: f64,
  to: f64,
  z_from: f64,
  z_to: f64,
) -> Vec<Move> {
  let total = to - from;
  let mut moves = Vec::new();
  let mut walked = 0.0;
  for segment in contour.segments() {
    let length = segment.length();
    let start = walked;
    walked += length;
    if length <= 0.0 {
      continue;
    }
    let piece_from = from.max(start);
    let piece_to = to.min(start + length);
    if piece_to <= piece_from + 1.0e-12 {
      continue;
    }
    let first = (piece_from - start) / length;
    let last = (piece_to - start) / length;
    let (_from_point, to_point) = piece_endpoints(segment, first, last);
    // The fraction of the whole ramp this piece ends at, which is where its Z is read. Written as
    // `z_from + (z_to − z_from) · t` so the first and last pieces land exactly on the two ends instead
    // of on an expression that can round away from them.
    let t = if total > 0.0 {
      (piece_to - from) / total
    } else {
      1.0
    };
    push_ramp(
      &mut moves,
      job,
      sub_segment(segment, first, last),
      to_point,
      z_from + (z_to - z_from) * t,
    );
  }
  moves
}

/// Endpoints of the piece of `segment` between two fractions of its length.
fn piece_endpoints(segment: Segment, from: f64, to: f64) -> (Point2, Point2) {
  match segment {
    Segment::Line(line) => (line.point_at(from), line.point_at(to)),
    Segment::Arc(arc) => (arc.point_at_fraction(from), arc.point_at_fraction(to)),
  }
}

/// The sub-piece of `segment` between two fractions, keeping arcs as arcs.
fn sub_segment(segment: Segment, from: f64, to: f64) -> Segment {
  match segment {
    Segment::Line(line) => Segment::Line(LineSeg::new(line.point_at(from), line.point_at(to))),
    Segment::Arc(arc) => {
      let sweep = arc.sweep() * (to - from);
      let start_angle = arc.start_angle() + arc.sweep() * from;
      match cam_geom::Arc::new(arc.center(), arc.radius(), start_angle, sweep) {
        Ok(sub_arc) => Segment::Arc(sub_arc),
        Err(_) => Segment::Line(LineSeg::new(
          arc.point_at_fraction(from),
          arc.point_at_fraction(to),
        )),
      }
    }
  }
}

/// Append a horizontal move, as an arc when the piece is one.
pub(crate) fn push_horizontal(
  moves: &mut Vec<Move>,
  job: &Job,
  segment: Segment,
  from: Point2,
  to: Point2,
  z: f64,
) {
  let _ = from;
  let destination = machine_point(job, to);
  match segment {
    Segment::Line(_) => moves.push(Move::Feed {
      to: Point3 {
        x: Length::from_mm(destination.x),
        y: Length::from_mm(destination.y),
        z: Length::from_mm(z),
      },
    }),
    Segment::Arc(arc) => {
      let center = machine_point(job, arc.center());
      moves.push(Move::Arc {
        to: Point3 {
          x: Length::from_mm(destination.x),
          y: Length::from_mm(destination.y),
          z: Length::from_mm(z),
        },
        center,
        ccw: machine_ccw(job, arc.is_ccw()),
      });
    }
  }
}

/// The program a document asks for: every toolpath's segment, in document order.
///
/// This is the whole of what the emitter emits and what the simulation sweeps (D-M5-1: the same
/// segments, built by the same function, so the two cannot disagree about what was cut). It used to
/// live in `cam-cli`'s binary, where nothing else could reach it; moving it here is what lets a
/// preview or an estimate describe the document's own toolpaths instead of a demonstrator's sample.
///
/// `tools` is the caller's own library, already mapped onto this crate's types: `cam-toolpath` gains
/// no `cam-store` edge (S6), so a number the list does not hold is refused by name, with the toolpath
/// that asked for it. Every rate and the shape the simulation sweeps come from that list and from
/// nowhere else — the builder opens no file and no database.
pub fn program_for(
  document: &Document,
  tools: &[ProgramTool],
) -> Result<MoveStream, ToolpathError> {
  let mut stream = MoveStream::new(document.job.safe_z, Point2::new(0.0, 0.0));
  for toolpath in &document.toolpaths {
    stream
      .segments
      .push(segment_for(document, toolpath, tools)?);
  }
  Ok(stream)
}

/// Turn one document toolpath into a segment of machine moves.
fn segment_for(
  document: &Document,
  toolpath: &Toolpath,
  tools: &[ProgramTool],
) -> Result<MoveSegment, ToolpathError> {
  let number = toolpath.tool_number;
  let tool = tools
    .iter()
    .find(|tool| tool.spec.number == number)
    .ok_or_else(|| ToolpathError::ToolNotInLibrary {
      tool_number: number,
      toolpath: toolpath.id.clone(),
    })?;
  let spec = tool.spec;
  let flat_diameter = tool.flat_diameter;

  if spec.diameter.mm() <= 0.0 {
    return Err(ToolpathError::ToolWithoutDiameter {
      tool_number: number,
    });
  }

  // The shape the simulation sweeps (A8). A chamfer is the only operation that cuts with a cone, and
  // its cone is the library's own geometry: the bit's included angle and its flat, zero when the
  // library does not record one. A chamfer on a tool with no angle is refused by name rather than
  // defaulted to 90 degrees, because that would cut a different bevel (D-M6-9).
  let tool_shape = match &toolpath.operation {
    Operation::Chamfer(params) => {
      let included_angle = spec
        .included_angle
        .ok_or_else(|| ToolpathError::ChamferNeedsAngle {
          tool_number: number,
          toolpath: toolpath.id.clone(),
        })?;
      // The same refusal as at authoring, repeated here because the library can change under a
      // document that was written against a sharp bit: a flat widens the bevel to
      // `chamfer_width + flat/2`, and the document's derived depth knows nothing of it (D-M6-10).
      // A program that would cut a bevel nobody asked for is not written, and the simulation refuses
      // the same shape, so the bit fails at every boundary that can see it.
      if flat_diameter.mm() > 0.0 {
        return Err(ToolpathError::ChamferNeedsSharpBit {
          tool_number: number,
          toolpath: toolpath.id.clone(),
          flat_diameter: flat_diameter.mm(),
          chamfer_width: params.chamfer_width.mm(),
        });
      }
      ToolShape::v_bit(included_angle, flat_diameter)
    }
    // An engrave, a flute, a texture or a moulding carries the tool's real shape, whatever it is: a
    // V-bit when the library records an angle and a cylinder otherwise (M6 A8). Nothing about the path
    // depends on it — a groove's depth is authored, a ramp is a length and a moulding's cross-section is
    // authored data too (M8 S3) — but the simulation must sweep the groove the bit actually cuts, and a
    // segment that said "cylinder" for a V-bit would report a correct engrave as gouge, a correct
    // V-groove flute or texture as a flat slot, and a correct moulding as the wrong cross-section.
    Operation::Engrave(_)
    | Operation::Fluting(_)
    | Operation::Texture(_)
    | Operation::Moulding(_) => match spec.included_angle {
      Some(included_angle) => ToolShape::v_bit(included_angle, flat_diameter),
      None => ToolShape::cylinder(spec.diameter),
    },
    // A V-carve's cone is the bit's own, and the simulation must sweep the shape the machine really
    // has: the program's Z says `clearance / tan(document's angle / 2)`, and if the library's angle is
    // not that one, the carve really is a different shape and the report must say so rather than
    // restating the document's assumption (M6 A8's rule for a groove, applied to a cone). A tool the
    // library does not describe as an angled bit, or one with a flat, is refused by name here as it is
    // at authoring and in the strategy: the library can change under a document.
    Operation::VCarve(_) => {
      let included_angle = spec
        .included_angle
        .ok_or_else(|| ToolpathError::VCarveNeedsAngle {
          tool_number: number,
          toolpath: toolpath.id.clone(),
        })?;
      if flat_diameter.mm() > 0.0 {
        return Err(ToolpathError::VCarveNeedsSharpTip {
          tool_number: number,
          toolpath: toolpath.id.clone(),
          flat_diameter: flat_diameter.mm(),
        });
      }
      ToolShape::v_bit(included_angle, flat_diameter)
    }
    _ => ToolShape::cylinder(spec.diameter),
  };

  let mut moves = Vec::new();
  match &toolpath.operation {
    Operation::Profile(params) => {
      for entity_id in &toolpath.entity_ids {
        let contour = contour_of(document, toolpath, entity_id)?;
        moves.extend(
          profile(&ProfileRequest {
            contour,
            params,
            tool: &spec,
            job: &document.job,
          })
          .map_err(|error| in_toolpath(toolpath, error))?,
        );
      }
    }
    Operation::Pocket(params) => {
      // Each selected contour is its own outer boundary, and the toolpath's islands are holes in
      // every one of them (A9, D-RB-1: the document names the islands, nothing is inferred from
      // nesting). A nested selection therefore still pockets both contours, without the inner one
      // silently becoming a hole.
      for entity_id in &toolpath.entity_ids {
        let contour = contour_of(document, toolpath, entity_id)?;
        let mut islands = Vec::with_capacity(toolpath.island_ids.len());
        for island_id in &toolpath.island_ids {
          // `Document::validate` already refuses an island that does not resolve, but a toolpath
          // reaching the program without having been validated must fail by name, not panic.
          let island = document
            .entity(island_id)
            .ok_or_else(|| ToolpathError::UnknownIsland {
              toolpath: toolpath.id.clone(),
              island: island_id.clone(),
            })?;
          let island =
            island
              .kind
              .as_contour()
              .ok_or_else(|| ToolpathError::IslandNotAContour {
                island: island_id.clone(),
              })?;
          islands.push(island);
        }
        let path = pocket(&PocketRequest {
          contour,
          islands: &islands,
          params,
          tool: &spec,
          job: &document.job,
        })
        .map_err(|error| in_toolpath(toolpath, error))?;
        moves.extend(path.moves);
      }
    }
    Operation::Drilling(params) => {
      let mut holes = Vec::with_capacity(toolpath.entity_ids.len());
      for entity_id in &toolpath.entity_ids {
        let contour = contour_of(document, toolpath, entity_id)?;
        let centre = circle_centre(contour).ok_or_else(|| {
          in_toolpath(
            toolpath,
            ToolpathError::NotACircle {
              entity: entity_id.clone(),
            },
          )
        })?;
        holes.push(DrillHole {
          entity: entity_id.clone(),
          centre,
        });
      }
      let path = drill(&DrillRequest {
        holes: &holes,
        params,
        tool: &spec,
        job: &document.job,
      })
      .map_err(|error| in_toolpath(toolpath, error))?;
      moves.extend(path.moves);
    }
    Operation::Chamfer(params) => {
      for entity_id in &toolpath.entity_ids {
        let contour = contour_of(document, toolpath, entity_id)?;
        moves.extend(
          chamfer(&ChamferRequest {
            contour,
            params,
            tool: &spec,
            job: &document.job,
          })
          .map_err(|error| in_toolpath(toolpath, error))?,
        );
      }
    }
    Operation::VCarve(params) => {
      // One region, and its parity rule is what turns an appended contour into a hole (D-RB-2), so the
      // walk is asked for the whole selection at once rather than once per entity: a V-carve of two
      // nested contours is the ring between them, exactly as a pocket of the same vectors is.
      let mut contours = Vec::with_capacity(toolpath.entity_ids.len());
      for entity_id in &toolpath.entity_ids {
        contours.push(contour_of(document, toolpath, entity_id)?);
      }
      moves.extend(
        vcarve(&VCarveRequest {
          contours: &contours,
          params,
          tool: &spec,
          flat_diameter,
          job: &document.job,
        })
        .map_err(|error| in_toolpath(toolpath, error))?,
      );
    }
    Operation::Fluting(params) => {
      // Open and closed alike, like an engrave: a flute is one groove per vector (D-M6-5).
      for entity_id in &toolpath.entity_ids {
        let contour = contour_of(document, toolpath, entity_id)?;
        moves.extend(
          fluting(&FlutingRequest {
            contour,
            params,
            tool: &spec,
            job: &document.job,
          })
          .map_err(|error| in_toolpath(toolpath, error))?,
        );
      }
    }
    Operation::Engrave(params) => {
      // Open and closed vectors alike: engraving an open one is the strategy's point (D-M6-4).
      for entity_id in &toolpath.entity_ids {
        let contour = contour_of(document, toolpath, entity_id)?;
        moves.extend(
          engrave(&EngraveRequest {
            contour,
            params,
            tool: &spec,
            job: &document.job,
          })
          .map_err(|error| in_toolpath(toolpath, error))?,
        );
      }
    }
    Operation::Texture(params) => {
      // Every selected contour is a boundary of **one** region, and the region's parity rule is what
      // turns an appended contour into a hole (D-RB-2), so the pattern is clipped against the whole
      // selection at once rather than once per entity (D-M6-6).
      let mut contours = Vec::with_capacity(toolpath.entity_ids.len());
      for entity_id in &toolpath.entity_ids {
        contours.push(contour_of(document, toolpath, entity_id)?);
      }
      moves.extend(
        texture(&TextureRequest {
          contours: &contours,
          params,
          tool: &spec,
          job: &document.job,
        })
        .map_err(|error| in_toolpath(toolpath, error))?,
      );
    }
    Operation::Thread(params) => {
      // One descent per selected circle, and each is its own thread: a thread's diameter is the entity's
      // own, so two circles are two threads with two orbits and never one shared one (M6 A7).
      for entity_id in &toolpath.entity_ids {
        let contour = contour_of(document, toolpath, entity_id)?;
        moves.extend(
          thread(&ThreadRequest {
            entity: entity_id,
            contour,
            params,
            tool: &spec,
            job: &document.job,
          })
          .map_err(|error| in_toolpath(toolpath, error))?,
        );
      }
    }
    Operation::Inlay(params) => {
      // One half per toolpath, and the document says which half it is (see
      // `cam_model::InlayParams`): the female names the region it clears, and the male names that same
      // region as the islands it leaves standing inside the job rectangle, whose boundary is not an
      // entity at all.
      let ids = if toolpath.island_ids.is_empty() {
        &toolpath.entity_ids
      } else {
        &toolpath.island_ids
      };
      let mut contours = Vec::with_capacity(ids.len());
      for entity_id in ids {
        contours.push(contour_of(document, toolpath, entity_id)?);
      }
      let request = InlayRequest {
        contours: &contours,
        params,
        tool: &spec,
        job: &document.job,
      };
      let half = if toolpath.island_ids.is_empty() {
        female(&request)
      } else {
        male(&request)
      };
      moves.extend(half.map_err(|error| in_toolpath(toolpath, error))?);
    }
    Operation::Moulding(params) => {
      // A moulding's rail is the selected entity, and one rail is one sweep: two selected rails are two
      // mouldings, the way two selected vectors are two flutes. The profile and the stepover live in the
      // operation itself (M8 T1), so the selection stays a single list of rails.
      for entity_id in &toolpath.entity_ids {
        let contour = contour_of(document, toolpath, entity_id)?;
        moves.extend(
          moulding(&MouldingRequest {
            rail: contour,
            params,
            tool: &spec,
            job: &document.job,
          })
          .map_err(|error| in_toolpath(toolpath, error))?,
        );
      }
    }
  }

  Ok(MoveSegment {
    name: toolpath.name.clone(),
    tool_number: number,
    tool: tool_shape,
    feed_rate: spec.feed_rate,
    plunge_rate: spec.plunge_rate,
    spindle_speed: spec.spindle_speed,
    moves,
  })
}

/// A strategy's refusal, stated with the toolpath that asked for it.
fn in_toolpath(toolpath: &Toolpath, error: ToolpathError) -> ToolpathError {
  ToolpathError::InToolpath {
    toolpath: toolpath.id.clone(),
    source: Box::new(error),
  }
}

/// The contour an entity id names, refused by name when the document has no such contour.
fn contour_of<'a>(
  document: &'a Document,
  toolpath: &Toolpath,
  entity_id: &str,
) -> Result<&'a Contour, ToolpathError> {
  let entity = document
    .entity(entity_id)
    .ok_or_else(|| ToolpathError::UnknownEntity {
      toolpath: toolpath.id.clone(),
      entity: entity_id.to_owned(),
    })?;
  entity
    .kind
    .as_contour()
    .ok_or_else(|| ToolpathError::NotAContour {
      entity: entity_id.to_owned(),
    })
}
