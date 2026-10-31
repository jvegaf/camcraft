//! V-carve: walk the region's medial axis, one tip per vertex at the depth that vertex's own
//! clearance demands (M7 A1, A3, D-M7-7).
//!
//! A V-bit's body is **above** its cone, so the material left standing after sweeping a set of tips is
//! the **lower** envelope of their flanks. Put the tip at
//!
//! ```text
//! z_p = -d(p) / tan(θ/2)
//! ```
//!
//! with `d(p)` the exact distance from `p` to the region's boundary and `θ` the bit's included angle,
//! and the surface the sweep leaves is at `-d(q) / tan(θ/2)` for every `q` **provided the tips reach
//! every point of the region**: `d` is 1-Lipschitz, so `|q − p| − d(p) ≥ −d(q)` for every tip and the
//! carve can only leave material standing, never cut deeper than the distance field; and equality is
//! attained at the tip whose nearest boundary point is radially aligned with `q`. `R` is exactly the
//! union of the medial balls `B(p, d(p))` over its medial axis, so the axis is the smallest set of
//! tips that covers the region — which is why the axis **is** the toolpath.
//!
//! The walk is therefore not a pass sequence and not a set of levels: it is
//! [`cam_geom::medial::medial_axis`], branch by branch, each vertex at its own Z. Two consequences are
//! worth stating because they are decisions rather than details:
//!
//! * **No arc move is emitted** (D-M7-7). A constant-`d` contour of the distance field is not a
//!   constant-Z curve, and [`cam_move::Move::Arc`] is constant-Z by contract, so a V-carve's Z is a
//!   function of *position* and the walk says so with one `Move::Feed` per vertex. The project's post
//!   blocks already carry `[Z]` on a feed move and the sweep already interpolates Z along one, so no
//!   block kind and no post contract changes.
//! * **Branches are separate walks** (D-M4-6's rule, inherited unchanged). Between two branches the
//!   tool retracts to the job's safe Z and rapids, because a branch that merely *looks* adjacent can be
//!   on the far side of a wall.
//!
//! A branch is a polyline, not necessarily a simple one: `cam_geom::medial`'s stitch pass may join two
//! arms of the same junction into one walk, so a branch can pass through a junction whose clearance is
//! a local maximum and come out the other side. Nothing here assumes otherwise — every vertex is
//! walked in the branch's own order and carries its own Z, so a joined branch is cut correctly and a
//! tip at a junction is exactly as deep as its own ball.

use std::f64::consts::PI;

use cam_geom::medial::medial_axis;
use cam_geom::{region::Region, Bounds, Contour};
use cam_model::{Job, VCarveParams};
use cam_move::{Move, Point3};
use cam_units::Length;

use crate::{machine_point, ToolSpec, ToolpathError};

/// Everything a V-carve needs.
#[derive(Clone, Copy, Debug)]
pub struct VCarveRequest<'a> {
  /// The closed contours whose even-odd union is the region to carve.
  ///
  /// One region, not one per contour: an appended contour is a hole under the kernel's parity rule
  /// (D-RB-2), which is exactly how a pocket reads its selection, so a V-carve and a pocket of the same
  /// vectors carve the same area.
  pub contours: &'a [&'a Contour],
  /// The bit's included angle, the depth law that angle implies, and the axis's tolerance.
  pub params: &'a VCarveParams,
  /// The tool doing the cutting.
  ///
  /// Consulted for two refusals and nothing else: a V-carve's geometry is the region's, and its depth
  /// is [`VCarveParams::depth`]'s, so the bit's diameter moves neither. The angle the *law* uses is the
  /// document's, because the document is what a later `emit` and `simulate` read; this record is only
  /// asked whether the library describes the tool as an angled bit at all (D-M6-9).
  pub tool: &'a ToolSpec,
  /// The flat at the tool's tip, in millimetres; zero for a sharp point.
  ///
  /// A separate field rather than part of [`ToolSpec`] because the emitter's tool record predates the
  /// V-carve and carries no flat, while the refusal below needs one number from it.
  pub flat_diameter: Length,
  /// Job setup, used to map document coordinates onto machine coordinates.
  pub job: &'a Job,
}

/// Compute the moves for a V-carve: one walk per medial branch, each vertex at its own depth.
///
/// The walk, per branch and in the axis's own order: a rapid to safe Z over the branch's first vertex,
/// a plunge there to that vertex's own depth, one [`Move::Feed`] per further vertex carrying that
/// vertex's own Z, and a retract back to safe Z. A branch that is a loop returns to its first vertex
/// (the axis does not repeat it, the way a [`Contour`] does not), so the loop's closing edge is cut
/// too. A branch of one vertex — a disc's whole axis — is a plunge and a retract, which is exactly what
/// a disc's V-carve is.
///
/// # The refusals, and why they are here
///
/// Five inputs are refused by name rather than absorbed:
///
/// * an **open contour**, because a region is an area and an open vector encloses none
///   ([`ToolpathError::OpenContour`]);
/// * an **angle outside `(0, 180)`** degrees, because the law divides by `tan(θ/2)` and the tangent of
///   either end is zero ([`ToolpathError::InvalidIncludedAngle`], the chamfer's own refusal: the
///   document refuses it first, and a caller that bypassed the document must not receive an infinite
///   Z);
/// * a tool the library **does not describe as an angled bit** ([`ToolpathError::VCarveNeedsAngledTool`]),
///   because defaulting 90 degrees would cut a different carve (D-M6-9);
/// * a region whose **medial axis is empty** ([`ToolpathError::VCarveNothingToCarve`]), because a
///   lattice step at least as large as the region's own clearance leaves no point that qualifies as
///   material: there is nothing to carve, and a program of zero moves would only be found later by
///   `simulate`, as a whole field of `uncut`;
/// * a tool whose tip has a **flat** ([`ToolpathError::VCarveNeedsSharpBit`]), because a flat of
///   diameter `f` cannot carve to a sharp edge: the surface its tip leaves is `f/2 · cot(θ/2)` wide at
///   the bottom, and the depth law this operation is built on — `d / tan(θ/2)`, which reaches zero
///   exactly at the vectors — is the law of a point. This is the refusal the **strategy** owns, and it
///   belongs here rather than only at authoring: three boundaries can see a tool (the command that
///   stores the document, the program that emits it, and a caller of this function), the library can
///   change under a document written against a sharp bit, and a flat bit silently absorbed here would
///   cut a carve nobody asked for. Thinking a flat is "close enough" is precisely the mistake the
///   chamfer's `D-M6-10` recorded.
pub fn vcarve(request: &VCarveRequest<'_>) -> Result<Vec<Move>, ToolpathError> {
  let params = request.params;
  let angle = params.included_angle.radians();
  if !(angle > 0.0 && angle < PI) {
    return Err(ToolpathError::InvalidIncludedAngle {
      degrees: params.included_angle.degrees(),
    });
  }
  if request.tool.included_angle.is_none() {
    return Err(ToolpathError::VCarveNeedsAngledTool {
      tool_number: request.tool.number,
    });
  }
  if request.flat_diameter.mm() > 0.0 {
    return Err(ToolpathError::VCarveNeedsSharpBit {
      tool_number: request.tool.number,
      flat_diameter: request.flat_diameter.mm(),
    });
  }

  let mut boundaries = Vec::with_capacity(request.contours.len());
  for contour in request.contours {
    if !contour.is_closed() {
      return Err(ToolpathError::OpenContour);
    }
    boundaries.push((*contour).clone());
  }
  // The only way building a region from closed boundaries can fail is a boundary that is not closed,
  // which the loop above has already refused, so the same refusal is the honest answer here.
  let region = Region::new(boundaries).map_err(|_| ToolpathError::OpenContour)?;
  let axis =
    medial_axis(&region, params.tolerance.mm()).map_err(|error| ToolpathError::MedialAxis {
      reason: error.to_string(),
    })?;
  // An axis with no branch is not a carve: a lattice step at least as large as the region's own
  // clearance puts every point of it within that step of the boundary, so nothing qualifies as
  // material and there is no tip to plunge. The alternative is a program of zero moves, which
  // `simulate` then reports as the whole field left standing — so the two numbers that produced it
  // are the refusal (M7 A5). See [`ToolpathError::VCarveNothingToCarve`].
  if axis.branches.is_empty() {
    let mut bounds = Bounds::empty();
    for contour in request.contours {
      for segment in contour.segments() {
        bounds = bounds.union(segment.bounds());
      }
    }
    let (width, height) = if bounds.is_empty() {
      (0.0, 0.0)
    } else {
      (bounds.width(), bounds.height())
    };
    return Err(ToolpathError::VCarveNothingToCarve {
      tolerance: params.tolerance.mm(),
      width,
      height,
    });
  }

  let safe_z = request.job.safe_z;
  let depth_of = |clearance: f64| -params.depth(Length::from_mm(clearance)).mm();
  let tip = |point: cam_geom::Point2, clearance: f64| {
    let machine = machine_point(request.job, point);
    Point3::new(machine.x, machine.y, depth_of(clearance))
  };

  let mut moves = Vec::new();
  for branch in &axis.branches {
    let Some(first) = branch.vertices.first() else {
      continue;
    };
    let start = tip(first.point, first.clearance);
    moves.push(Move::Rapid {
      to: Point3::new(start.x.mm(), start.y.mm(), safe_z.mm()),
    });
    moves.push(Move::Feed { to: start });
    for vertex in &branch.vertices[1..] {
      moves.push(Move::Feed {
        to: tip(vertex.point, vertex.clearance),
      });
    }
    if branch.closed {
      // The axis does not repeat its first vertex, and a loop that stopped at its last vertex would
      // leave the edge between them uncut.
      moves.push(Move::Feed { to: start });
    }
    let last = moves
      .last()
      .map(|movement| movement.destination())
      .unwrap_or(start);
    moves.push(Move::Retract {
      to: Point3::new(last.x.mm(), last.y.mm(), safe_z.mm()),
    });
  }
  Ok(moves)
}
