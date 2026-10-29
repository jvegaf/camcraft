//! Pocket clearing: concentric offset rings, or parallel raster rows.
//!
//! Both modes cut the region inside the boundary with the tool centre `radius + allowance` away from
//! it, chunk the depth the way a profile does, and cut every ring or row at each depth pass.
//!
//! Geometry is never tessellated here: offset rings keep their arcs, and a raster row's ends are true
//! line/arc intersections.

use cam_geom::intersect::{intersect, Intersections};
use cam_geom::offset::{offset_contour, OffsetOptions};
use cam_geom::{Contour, LineSeg, Point2, Segment, DEFAULT_TOLERANCE};
use cam_model::{Job, PocketParams, PocketStrategy};
use cam_move::{Move, Point3};
use cam_units::Length;

use crate::{machine_point, pass_depths, push_horizontal, ToolSpec, ToolpathError};

/// How far an ambiguous raster row is nudged before its clipping is retried, in millimetres.
///
/// Ten times [`cam_geom::DEFAULT_TOLERANCE`], so the retried row is unambiguously clear of the
/// boundary edge or tangency that made it ambiguous. Ten nanometres is far below any machine
/// resolution and far above `f64` noise for job-scale coordinates.
const ROW_RETRY_NUDGE: f64 = 1.0e-5;

/// How far a ring vertex may fall short of the offset distance, in millimetres.
///
/// A ring that had self-intersections removed is flattened to the offset kernel's chord tolerance
/// (0.001 mm), so a correct ring's vertices can sit a chord's sagitta inside the exact offset curve.
const RING_DISTANCE_SLACK: f64 = 0.002;

/// Everything a pocket pass needs.
#[derive(Clone, Copy, Debug)]
pub struct PocketRequest<'a> {
  /// The closed region to clear.
  pub contour: &'a Contour,
  /// Strategy, allowance, stepover and depths.
  pub params: &'a PocketParams,
  /// The tool doing the cutting.
  pub tool: &'a ToolSpec,
  /// Job setup, used to map document coordinates onto machine coordinates.
  pub job: &'a Job,
}

/// What a pocket produced, beyond the moves themselves.
#[derive(Clone, Debug, PartialEq)]
pub struct Pocketpath {
  /// The moves, in cutting order.
  pub moves: Vec<Move>,
  /// Number of rings (offset) or rows (raster) cut, before depth passes.
  pub rings: usize,
}

/// Compute the moves for a pocket cut.
///
/// The geometry is computed once and then cut at every depth pass, shallow to deep. Between rings,
/// rows and spans the tool retracts to the job's safe Z: staying down across cleared material needs a
/// gouge-free link, which is a later milestone's problem.
pub fn pocket(request: &PocketRequest<'_>) -> Result<Pocketpath, ToolpathError> {
  let params = request.params;
  if !request.contour.is_closed() {
    return Err(ToolpathError::OpenContour);
  }
  if request.tool.diameter.mm() <= 0.0 {
    return Err(ToolpathError::InvalidToolDiameter);
  }
  if params.stepover.mm() <= 0.0 {
    return Err(ToolpathError::InvalidStepover {
      stepover: params.stepover.mm(),
    });
  }
  if params.pass_depth.mm() <= 0.0 {
    return Err(ToolpathError::InvalidPassDepth);
  }
  if params.cut_depth.mm() <= params.start_depth.mm() {
    return Err(ToolpathError::InvalidDepthRange {
      cut_depth: params.cut_depth.mm(),
      start_depth: params.start_depth.mm(),
    });
  }

  // The tool centre rides `radius + allowance` inside the boundary, so an allowance of `a` leaves
  // exactly `a` millimetres of wall material in both modes (A3).
  let inset = request.tool.diameter.mm() / 2.0 + params.allowance.mm();
  let stepover = params.stepover.mm();
  let geometry = match params.strategy {
    PocketStrategy::Offset => Geometry::Rings(offset_rings(request.contour, inset, stepover)?),
    PocketStrategy::Raster => Geometry::Rows(raster_rows(request.contour, inset, stepover)?),
  };

  let passes = pass_depths(
    params.start_depth.mm(),
    params.cut_depth.mm(),
    params.pass_depth.mm(),
  )?;
  let mut moves = Vec::new();
  for depth in passes {
    geometry.cut(&mut moves, request.job, -depth);
  }

  Ok(Pocketpath {
    moves,
    rings: geometry.count(),
  })
}

/// The ring or row geometry of one pocket, computed once and cut at every depth pass (A6).
enum Geometry {
  /// Closed offset rings, outermost first.
  Rings(Vec<Contour>),
  /// Raster rows, each an ordered list of straight spans.
  Rows(Vec<Vec<(Point2, Point2)>>),
}

impl Geometry {
  /// How many rings or rows the pocket has, before depth passes.
  fn count(&self) -> usize {
    match self {
      Self::Rings(rings) => rings.len(),
      Self::Rows(rows) => rows.len(),
    }
  }

  /// Cut every ring or row once, at depth `z`, retracting to safe Z in between.
  fn cut(&self, moves: &mut Vec<Move>, job: &Job, z: f64) {
    match self {
      Self::Rings(rings) => {
        for ring in rings {
          cut_ring(moves, job, ring, z);
        }
      }
      Self::Rows(rows) => {
        for (index, row) in rows.iter().enumerate() {
          // Zigzag: consecutive rows run in opposite directions, so the tool starts each row at
          // the end it finished the previous one.
          if index % 2 == 1 {
            for (start, end) in row.iter().rev() {
              cut_span(moves, job, *end, *start, z);
            }
          } else {
            for (start, end) in row {
              cut_span(moves, job, *start, *end, z);
            }
          }
        }
      }
    }
  }
}

/// Concentric rings, outermost first, until the inward offset collapses.
///
/// Every ring is offset from the **original** boundary rather than from the previous ring: chaining
/// offsets accumulates the kernel's join error and lets the stepover drift.
fn offset_rings(
  contour: &Contour,
  inset: f64,
  stepover: f64,
) -> Result<Vec<Contour>, ToolpathError> {
  let options = OffsetOptions::default();
  let original_ccw = contour.is_ccw();
  let boundary = contour.segments();
  let mut rings = Vec::new();
  let mut index = 0usize;
  loop {
    let distance = inset + index as f64 * stepover;
    let ring = match offset_contour(contour, -distance, &options) {
      Ok(offset) => offset.contour,
      Err(error) => {
        // A refused offset is the collapsed centre of the region, not a failure of the request —
        // unless it is the first ring, which means the tool never fitted.
        if index == 0 {
          return Err(ToolpathError::OffsetCollapsed {
            reason: error.to_string(),
          });
        }
        break;
      }
    };
    if let Some(reason) = collapse_reason(&ring, original_ccw, &boundary, distance) {
      if index == 0 {
        return Err(ToolpathError::OffsetCollapsed { reason });
      }
      break;
    }
    rings.push(ring);
    index += 1;
  }
  Ok(rings)
}

/// Why a ring is the collapsed centre of the region, if it is.
fn collapse_reason(
  ring: &Contour,
  original_ccw: bool,
  boundary: &[Segment],
  distance: f64,
) -> Option<String> {
  if ring.is_ccw() != original_ccw {
    return Some("the ring inverted its winding".to_owned());
  }
  if ring.signed_area() <= 0.0 {
    return Some("the ring has no area left".to_owned());
  }
  if !clears_boundary(ring, boundary, distance) {
    return Some(format!(
      "the tool centre cannot stay {distance} mm inside the region"
    ));
  }
  None
}

/// Whether every vertex of `ring` really is `distance` away from the original boundary.
///
/// The offset kernel refuses an offset that passes through the shape, but a shrink wider than the
/// region can still come back as a well-formed ring that has *escaped* through the far side of it.
/// Cutting such a ring would gouge the wall it was supposed to leave standing, so it ends the
/// ringing instead. A correct offset vertex sits exactly `distance` from the boundary it was offset
/// from, so anything closer means the ring crossed it; this also bounds the ring loop, because in a
/// bounded region no point can stay further and further from the boundary forever.
fn clears_boundary(ring: &Contour, boundary: &[Segment], distance: f64) -> bool {
  let threshold = distance - RING_DISTANCE_SLACK - distance.abs() * 1.0e-9;
  ring.vertices().iter().all(|vertex| {
    let nearest = boundary
      .iter()
      .map(|segment| segment.distance_to_point(vertex.point))
      .fold(f64::INFINITY, f64::min);
    nearest >= threshold
  })
}

/// Parallel rows across the inset region, clipped to it exactly.
fn raster_rows(
  contour: &Contour,
  inset: f64,
  stepover: f64,
) -> Result<Vec<Vec<(Point2, Point2)>>, ToolpathError> {
  let region = offset_contour(contour, -inset, &OffsetOptions::default())
    .map_err(|error| ToolpathError::OffsetCollapsed {
      reason: error.to_string(),
    })?
    .contour;
  if let Some(reason) = collapse_reason(&region, contour.is_ccw(), &contour.segments(), inset) {
    return Err(ToolpathError::OffsetCollapsed { reason });
  }
  let bounds = region.bounds();
  let segments = region.segments();
  // The raster line reaches well past the region on both sides so it always crosses the boundary
  // instead of ending on it.
  let reach = bounds.width().max(bounds.height()) + 10.0;
  let line = |y: f64| {
    LineSeg::new(
      Point2::new(bounds.min.x - reach, y),
      Point2::new(bounds.max.x + reach, y),
    )
  };

  let mut rows = Vec::new();
  let mut index = 0usize;
  loop {
    let y = bounds.min.y + index as f64 * stepover;
    if y > bounds.max.y {
      break;
    }
    // An ambiguous clipping is retried once just inside the region, then dropped: half a row is
    // worse than no row, because it leaves the tool in material it cannot reach from the other side.
    let spans =
      row_spans(&segments, line(y)).or_else(|| row_spans(&segments, line(y + ROW_RETRY_NUDGE)));
    if let Some(spans) = spans {
      if !spans.is_empty() {
        rows.push(spans);
      }
    }
    index += 1;
  }
  Ok(rows)
}

/// Clip one raster row against the region, or `None` when the clipping is ambiguous.
///
/// The ends are true intersections with the boundary segments, never tessellated approximations:
/// every crossing point's x is collected, sorted, deduped within the offset tolerance and paired
/// even-odd into spans. A leftover odd point is a tangency and a `Coincident` result is a row lying
/// along a straight boundary; neither can be paired, so neither is cut.
fn row_spans(segments: &[Segment], line: LineSeg) -> Option<Vec<(Point2, Point2)>> {
  let mut xs = Vec::with_capacity(segments.len());
  for segment in segments {
    match intersect(Segment::Line(line), *segment, DEFAULT_TOLERANCE) {
      Intersections::Coincident => return None,
      intersections => xs.extend(intersections.points().into_iter().map(|point| point.x)),
    }
  }
  xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

  // Two segments meeting at a vertex both report that vertex, so the shared crossing is deduped
  // before pairing.
  let mut unique: Vec<f64> = Vec::with_capacity(xs.len());
  for x in xs {
    if unique
      .last()
      .map_or(true, |last| (x - last).abs() > DEFAULT_TOLERANCE)
    {
      unique.push(x);
    }
  }
  if unique.len() % 2 != 0 {
    return None;
  }

  let y = line.start.y;
  Some(
    unique
      .chunks(2)
      .map(|pair| (Point2::new(pair[0], y), Point2::new(pair[1], y)))
      .collect(),
  )
}

/// Cut one closed ring, keeping its arcs as arcs (S8).
fn cut_ring(moves: &mut Vec<Move>, job: &Job, ring: &Contour, z: f64) {
  let segments = ring.segments();
  let Some(first) = segments.first().copied() else {
    return;
  };
  let start = machine_point(job, first.start_point());
  moves.push(Move::Rapid {
    to: point3(start, job.safe_z),
  });
  moves.push(Move::Feed {
    to: point3(start, Length::from_mm(z)),
  });
  for segment in segments {
    push_horizontal(
      moves,
      job,
      segment,
      segment.start_point(),
      segment.end_point(),
      z,
    );
  }
  let last = moves
    .last()
    .map(|movement| movement.destination())
    .unwrap_or(Point3::ORIGIN);
  moves.push(Move::Retract {
    to: Point3 {
      x: last.x,
      y: last.y,
      z: job.safe_z,
    },
  });
}

/// Cut one straight raster span.
fn cut_span(moves: &mut Vec<Move>, job: &Job, from: Point2, to: Point2, z: f64) {
  let start = machine_point(job, from);
  let end = machine_point(job, to);
  moves.push(Move::Rapid {
    to: point3(start, job.safe_z),
  });
  moves.push(Move::Feed {
    to: point3(start, Length::from_mm(z)),
  });
  moves.push(Move::Feed {
    to: point3(end, Length::from_mm(z)),
  });
  moves.push(Move::Retract {
    to: point3(end, job.safe_z),
  });
}

/// A machine position at `point` in XY and `z` in Z.
fn point3(point: Point2, z: Length) -> Point3 {
  Point3 {
    x: Length::from_mm(point.x),
    y: Length::from_mm(point.y),
    z,
  }
}
