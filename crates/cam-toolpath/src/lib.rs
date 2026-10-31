//! Toolpath strategies: turn document geometry plus a tool into the moves a machine executes.
//!
//! M0 implemented `profile` and the rules every other strategy reuses: the side offset, the
//! allowance, depth chunking, tab lifting and lead moves. M4 adds the two strategies that define 2.5D
//! routing: `pocket` (offset rings or raster rows) and `drill` (pecks, retracts, drill banks). M6 adds
//! the depth-variant strategies: `chamfer`, one V-bit pass whose Z is derived from the bit's own
//! included angle instead of authored, `engrave`, one authored-depth pass along open or closed
//! vectors with no pass law at all, and `fluting`, one pass along a vector that ramps in over its
//! first millimetres and out over its last.
//!
//! Must not depend on `cam-post` or any UI crate.

#![warn(missing_docs)]

pub mod chamfer;
pub mod drill;
pub mod engrave;
pub mod fluting;
pub mod pocket;

pub use chamfer::{chamfer, ChamferRequest};
pub use drill::{circle_centre, drill, DrillHole, DrillRequest, Drillpath};
pub use engrave::{engrave, EngraveRequest};
pub use fluting::{fluting, FlutingRequest};
pub use pocket::{pocket, PocketRequest, Pocketpath};

use cam_geom::{Contour, LineSeg, Point2, Segment};
use cam_model::{Job, ProfileParams, ProfileSide, Tab};
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
  /// A hole is not a circle, so it has no centre to drill.
  #[error("entity {entity:?} is not a circle, so it has no drill centre")]
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
  /// A flute's ramp runs along a contour that contains an arc.
  ///
  /// A ramped move cannot ride on `Move::Arc`, which is documented constant-Z (S9), and the move
  /// vocabulary has no helical move yet — T7 adds one, and with it this refusal goes. Chording the ramp
  /// to a sagitta would put the vocabulary's limit into the program the machine runs, which is the trade
  /// the staircase already lost, so the strategy refuses by name instead: on a 25 mm circle a 10 mm chord
  /// sits half a millimetre inside the vector. A constant-depth flute along the same arc is untouched.
  #[error(
    "fluting ramp {ramp} mm would run along a contour with arcs, and a ramped move cannot follow one: \
     the move vocabulary has no helical move, and its only curved move is constant-Z, so a ramped arc \
     would be cut as a straight chord. Cut this contour with ramp 0, or ramp a polyline"
  )]
  RampOnArc {
    /// The offending ramp in millimetres.
    ramp: f64,
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
  match job.origin {
    cam_model::OriginCorner::BottomLeft => point,
    cam_model::OriginCorner::TopLeft => Point2::new(point.x, job.height.mm() - point.y),
    cam_model::OriginCorner::Center => Point2::new(
      point.x - job.width.mm() / 2.0,
      point.y - job.height.mm() / 2.0,
    ),
  }
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

/// The point `along` millimetres along the contour's arc length, clamped to its ends.
///
/// The inverse of the walk: a flute's ramp is a length along the vector, so it is a position that has
/// to be found on whichever segment holds it, arcs included.
pub(crate) fn point_at_arc(contour: &Contour, along: f64) -> Option<Point2> {
  let segments = contour.segments();
  let mut walked = 0.0;
  for (index, segment) in segments.iter().enumerate() {
    let length = segment.length();
    let last = index + 1 == segments.len();
    if along <= walked + length || last {
      let fraction = if length <= 0.0 {
        0.0
      } else {
        ((along - walked) / length).clamp(0.0, 1.0)
      };
      return Some(piece_endpoints(*segment, fraction, fraction).0);
    }
    walked += length;
  }
  None
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
        ccw: arc.is_ccw(),
      });
    }
  }
}
