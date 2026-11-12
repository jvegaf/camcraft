//! Shared tool and geometry helpers used by the machining operations.

use cam_geom::{LineSeg, Point2, Segment};
use cam_model::Job;
use cam_move::{Move, Point3};
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
  /// The medial walk refuses a tolerance that is not positive and finite, and a lattice the
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
pub(crate) fn machine_ccw(job: &Job, ccw: bool) -> bool {
  ccw != job.frame().is_reflection()
}

/// Endpoints of the piece of `segment` between two fractions of its length.
pub(crate) fn piece_endpoints(segment: Segment, from: f64, to: f64) -> (Point2, Point2) {
  match segment {
    Segment::Line(line) => (line.point_at(from), line.point_at(to)),
    Segment::Arc(arc) => (arc.point_at_fraction(from), arc.point_at_fraction(to)),
  }
}

/// The sub-piece of `segment` between two fractions, keeping arcs as arcs.
pub(crate) fn sub_segment(segment: Segment, from: f64, to: f64) -> Segment {
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
