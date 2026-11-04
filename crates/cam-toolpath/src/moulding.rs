//! Moulding: sweep a cross-section along a rail with a stepover (M8 T4; S3, S5).
//!
//! S3's reading is the whole strategy. The **rail** is the selected XY path; the **profile** is
//! authored data, a cross-section of `(lateral, depth)` points; the **stepover** spaces the overlapped
//! lateral passes. So the program is a family of passes, one per lateral offset the stepover law admits
//! (`cam_geom::moulding::StepoverLaw`), each of them the rail walked at that offset and at the depth the
//! cross-section authors there. The rail is walked by arc length ([`cam_geom::moulding::Rail`]), so an
//! arc answers its own points and the walk chords it only to the declared tolerance.
//!
//! # Why feeds, and no arcs
//!
//! The program's Z is not a property of the rail position but of the **lateral offset**: every pass sits
//! at the depth its own cross-section point demands, so the program carries a family of Z heights rather
//! than one pass sequence. This follows M7's `vcarve` (D-M7-7) in shape and reason: `Move::Arc` is
//! constant-Z by contract, and a strategy whose Z comes from a field states its path with `Move::Feed`s
//! — the post already carries `[Z]` on a feed block and the sweep already interpolates Z along one, so
//! no block kind and no post contract changes. A corner in the rail is the other reason: the offset of
//! a corner has two normals (one arriving, one leaving), and the walk carries both, bridging them with a
//! straight feed instead of inventing an arc the rail never had.
//!
//! # Linking
//!
//! Between two passes the tool retracts to the job's safe Z and rapids, because a pass that merely looks
//! adjacent can be on the far side of a wall — D-M4-6's rule, inherited unchanged from the pocket, the
//! V-carve and every other multi-pass strategy.
//!
//! # What the tool does not decide
//!
//! The profile is authored, so the bit's **diameter moves nothing**: a 40 mm bit and a 1 mm bit cut the
//! same path through the same cross-section. The tool record is consulted for one refusal — a diameter
//! that is not a positive number is not a tool — and the program carries the tool's real shape for the
//! simulation to sweep (M6 A8).

use cam_geom::moulding::{
  CrossSection, CrossSectionPoint, Rail, StepoverLaw, DEFAULT_CHORD_TOLERANCE,
};
use cam_geom::{Contour, GeomError};
use cam_model::{Job, MouldingParams};
use cam_move::{Move, Point3};
use cam_units::Length;

use crate::{machine_point, ToolSpec, ToolpathError};

/// Everything a moulding needs.
#[derive(Clone, Copy, Debug)]
pub struct MouldingRequest<'a> {
  /// The rail: the selected XY path. Open sweeps a strip, closed sweeps a frame.
  pub rail: &'a Contour,
  /// The authored cross-section and the stepover that spaces its lateral passes.
  pub params: &'a MouldingParams,
  /// The tool doing the cutting.
  ///
  /// Consulted for the diameter refusal and nothing else: a moulding's geometry is the rail's and its
  /// depth is the cross-section's, so the bit's size moves neither (S3's profile is authored data).
  pub tool: &'a ToolSpec,
  /// Job setup, used to map document coordinates onto machine coordinates and to read safe Z.
  pub job: &'a Job,
}

/// Compute the moves for a moulding: one walk of the rail per lateral pass, each at its own depth.
///
/// Per pass, in the stepover law's order: a rapid to the pass's first point at the job's safe Z, a
/// plunge there to the depth the cross-section authors at that lateral offset, one [`Move::Feed`] per
/// point of the pass's offset walk ([`Rail::offset_walk`]), and a retract back to safe Z. A closed rail's
/// walk closes on the point where the rail closes, and `offset_walk` carries that closing edge — the
/// corner's diagonal at a seam that sits on a corner, and nothing at all at a seam in the middle of an
/// edge, where the two ends are one point.
///
/// # The refusals, and why they are here
///
/// * a **tool diameter that is not positive** ([`ToolpathError::InvalidToolDiameter`]), because a tool
///   with no diameter is not a tool;
/// * a **profile shorter than two points** ([`ToolpathError::MouldingProfileTooShort`]) and a profile
///   whose **laterals do not strictly increase** ([`ToolpathError::MouldingProfileLateral`]), because a
///   cross-section is one depth per lateral distance: fewer than two points sweeps nothing, and a
///   doubled lateral is two depths for one place;
/// * a **stepover that is zero, negative or not finite** ([`ToolpathError::InvalidStepover`]), because a
///   stepover with no distance behind it cannot space passes;
/// * anything else the kernel refuses about the geometry ([`ToolpathError::MouldingGeometry`]), with the
///   kernel's own words — a rail with no length, for instance, has no sweep behind it.
///
/// The document model refuses the profile and stepover cases as the document loads (M8 T1); they are
/// repeated here because three boundaries can see a toolpath — the command that stores the document, the
/// program that emits it, and a caller of this function — and a caller that hands the strategy its
/// parameters directly never passes through the document.
pub fn moulding(request: &MouldingRequest<'_>) -> Result<Vec<Move>, ToolpathError> {
  if request.tool.diameter.mm() <= 0.0 {
    return Err(ToolpathError::InvalidToolDiameter);
  }
  let cross = cross_section(request.params).map_err(geometry_error)?;
  let law = StepoverLaw::new(&cross, request.params.stepover.mm()).map_err(geometry_error)?;
  let rail = Rail::new(request.rail).map_err(geometry_error)?;
  let samples = rail
    .samples(DEFAULT_CHORD_TOLERANCE)
    .map_err(geometry_error)?;
  if samples.is_empty() {
    return Err(ToolpathError::MouldingGeometry {
      reason: "the rail produced no samples to walk".to_owned(),
    });
  }

  let safe_z = request.job.safe_z;
  let at = |point: cam_geom::Point2, z: Length| Point3 {
    x: Length::from_mm(point.x),
    y: Length::from_mm(point.y),
    z,
  };
  let mut moves = Vec::new();
  for lateral in law.passes() {
    let z = Length::from_mm(cross.depth_at(*lateral));
    let walk = rail.offset_walk(&samples, *lateral);
    let mut points = walk.into_iter();
    let Some(first) = points.next() else {
      continue;
    };
    let start = machine_point(request.job, first);
    moves.push(Move::Rapid {
      to: at(start, safe_z),
    });
    moves.push(Move::Feed { to: at(start, z) });
    for point in points {
      let point = machine_point(request.job, point);
      moves.push(Move::Feed { to: at(point, z) });
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

/// The document's authored cross-section in the kernel's own numbers.
fn cross_section(params: &MouldingParams) -> Result<CrossSection, GeomError> {
  let points = params
    .profile
    .iter()
    .map(|point| CrossSectionPoint {
      lateral: point.lateral.mm(),
      depth: point.depth.mm(),
    })
    .collect();
  CrossSection::new(points)
}

/// The strategy's refusal for whatever the geometry kernel refused.
///
/// The three refusals the strategy owns are restated as its own named variants so the operator reads the
/// moulding's contract and not the kernel's; everything else keeps the kernel's sentence, because a
/// refusal this crate did not anticipate must not be replaced by a guess about which one it was.
fn geometry_error(error: GeomError) -> ToolpathError {
  match error {
    GeomError::MouldingProfileTooShort { points } => {
      ToolpathError::MouldingProfileTooShort { points }
    }
    GeomError::MouldingProfileLateralOrder {
      index,
      previous,
      lateral,
    } => ToolpathError::MouldingProfileLateral {
      index,
      previous,
      lateral,
    },
    GeomError::MouldingStepoverNotPositive { stepover } => {
      ToolpathError::InvalidStepover { stepover }
    }
    other => ToolpathError::MouldingGeometry {
      reason: other.to_string(),
    },
  }
}
