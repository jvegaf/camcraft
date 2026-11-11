//! Pocket clearing: concentric offset rings, or parallel raster rows.
//!
//! Both modes cut the region inside the boundary with the tool centre `radius + allowance` away from
//! it, chunk the depth the way a profile does, and cut every ring or row at each depth pass.
//!
//! The boundary and its islands are one [`Region`], and the region is what makes an island a hole:
//! membership is by parity across its boundaries (A3), so an island needs no flag of its own and a
//! boss inside an island needs nothing extra either. The geometry, including the proof that an offset
//! has not escaped the material, comes from the kernel (A6, D-RB-6) — this module decides *how much*
//! to offset and in what order to cut, never whether an offset is honest.
//!
//! Geometry is never tessellated here: offset rings keep their arcs, and a raster row's ends are true
//! line/arc intersections.

use cam_geom::region::Region;
use cam_geom::{Bounds, Contour, Point2, DEFAULT_TOLERANCE};
use cam_model::Job;

use crate::params::{PocketParams, PocketStrategy};
use cam_move::{Move, Point3};
use cam_units::Length;

use crate::shared::{machine_point, pass_depths, push_horizontal, ToolSpec, ToolpathError};

/// How far an ambiguous raster row is nudged before its clipping is retried, in millimetres.
///
/// Ten times [`cam_geom::DEFAULT_TOLERANCE`], so the retried row is unambiguously clear of the
/// boundary edge or tangency that made it ambiguous. Ten nanometres is far below any machine
/// resolution and far above `f64` noise for job-scale coordinates.
const ROW_RETRY_NUDGE: f64 = 1.0e-5;

/// One raster row: the straight spans of material it crosses, in cutting order.
type Row = Vec<(Point2, Point2)>;

/// Everything a pocket pass needs.
#[derive(Clone, Copy, Debug)]
pub struct PocketRequest<'a> {
  /// The closed outer boundary of the region to clear.
  pub contour: &'a Contour,
  /// Closed contours to leave standing inside `contour`; empty when the pocket has no islands.
  ///
  /// A slice of references rather than of owned contours because every caller already holds the
  /// contours by reference: the document's entity table hands out `&Contour`, and the island list a
  /// toolpath carries is resolved to those same references. `&[&Contour]` passes what the caller has
  /// instead of making it clone a temporary table, and the strategy clones each contour exactly once,
  /// when it builds the [`Region`] it cuts.
  ///
  /// An island is a boundary of the same region, not a second request: the region's parity rule is
  /// what turns it into a hole (A3), so there is no island flag anywhere below this point.
  pub islands: &'a [&'a Contour],
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
  /// Rings (offset) or rows (raster) cut, before depth passes.
  ///
  /// Named for the ring path, which is the one that produces rings: the raster path counts rows here
  /// instead, so a caller reading this on a raster pocket is reading a row count.
  ///
  /// One ring is one closed contour, so a stage of the offset loop that came apart into several
  /// contours — a grown island crossing the shrunken outer boundary, say — counts once per contour.
  pub rings: usize,
  /// True when some ring had to be rebuilt as straight segments instead of keeping its arcs (A8).
  ///
  /// This is the kernel's own [`cam_geom::region::RegionOffset::flattened`] flag carried through to
  /// the caller. Flattening happens only where it has to: the offset kernel's self-intersection
  /// removal, or a piece the contour model's bulge cannot hold. A program that lost its arcs is
  /// something the user should be told about, so it is reported here rather than hidden.
  pub flattened: bool,
}

/// Compute the moves for a pocket cut.
///
/// The region to clear is `request.contour` with `request.islands` left standing, so the offset
/// strategy cuts rings on **both** sides of every boundary: the wall's rings shrink, the island's
/// rings grow (A12). `Pocketpath::flattened` reports the one thing the geometry cannot keep.
///
/// The geometry is computed once and then cut at every depth pass, shallow to deep. Between rings,
/// rows and spans the tool retracts to the job's safe Z: staying down across cleared material needs a
/// gouge-free link, which is a later milestone's problem.
pub fn pocket(request: &PocketRequest<'_>) -> Result<Pocketpath, ToolpathError> {
  let params = request.params;
  if !request.contour.is_closed() {
    return Err(ToolpathError::OpenContour);
  }
  if request.islands.iter().any(|island| !island.is_closed()) {
    // An island is a region boundary too. Refusing it here keeps the reason the caller's own
    // ("this pocket needs closed geometry") instead of the kernel's, which speaks about regions.
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
    PocketStrategy::Offset => {
      let (contours, flattened) = offset_rings(request.contour, request.islands, inset, stepover)?;
      Geometry::Rings {
        contours,
        flattened,
      }
    }
    PocketStrategy::Raster => {
      let (rows, flattened) = raster_rows(request.contour, request.islands, inset, stepover)?;
      Geometry::Rows { rows, flattened }
    }
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
    flattened: geometry.flattened(),
  })
}

/// The ring or row geometry of one pocket, computed once and cut at every depth pass (A6).
enum Geometry {
  /// Closed offset rings, outermost first.
  Rings {
    /// The rings, in the order the kernel produced them.
    contours: Vec<Contour>,
    /// Whether any ring was rebuilt as straight segments.
    flattened: bool,
  },
  /// Raster rows, each an ordered list of straight spans.
  Rows {
    /// The rows, outermost first.
    rows: Vec<Row>,
    /// Whether the inset region had to be rebuilt as straight segments.
    flattened: bool,
  },
}

impl Geometry {
  /// How many rings or rows the pocket has, before depth passes.
  fn count(&self) -> usize {
    match self {
      Self::Rings { contours, .. } => contours.len(),
      Self::Rows { rows, .. } => rows.len(),
    }
  }

  /// Whether any ring or row lost its arcs.
  fn flattened(&self) -> bool {
    match self {
      Self::Rings { flattened, .. } | Self::Rows { flattened, .. } => *flattened,
    }
  }

  /// Cut every ring or row once, at depth `z`, retracting to safe Z in between.
  fn cut(&self, moves: &mut Vec<Move>, job: &Job, z: f64) {
    match self {
      Self::Rings { contours, .. } => {
        for ring in contours {
          cut_ring(moves, job, ring, z);
        }
      }
      Self::Rows { rows, .. } => {
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

/// The pocket as a region: the outer boundary plus every island.
///
/// The parity rule of [`Region`] is the whole island concept (A3): appending a boundary turns the
/// material inside it back into a hole, so a boss inside an island would need nothing more. The
/// boundaries are *not* normalized here — the region offset does that, and it refuses a boundary
/// list whose boundaries cross rather than guessing what the user drew.
fn pocket_region(boundary: &Contour, islands: &[&Contour]) -> Result<Region, ToolpathError> {
  let mut boundaries = Vec::with_capacity(1 + islands.len());
  boundaries.push(boundary.clone());
  boundaries.extend(islands.iter().map(|island| (*island).clone()));
  Region::new(boundaries).map_err(|error| ToolpathError::OffsetCollapsed {
    reason: error.to_string(),
  })
}

/// Concentric rings of the pocket region, outermost first, until the region collapses.
///
/// Every ring is offset from the **original** boundaries rather than from the previous ring:
/// chaining offsets accumulates the kernel's join error and lets the stepover drift. The kernel picks
/// each boundary's sign from its nesting depth, so the outer boundary moves inward while an island
/// moves outward, and it drops any piece that cannot prove it is the distance it claims (A6). No
/// distance proof of its own is done here (D-RB-6): that rule lives in
/// [`cam_geom::region::Region::offset`] and nowhere else.
///
/// Returns the rings and whether any of them was rebuilt as straight segments.
fn offset_rings(
  boundary: &Contour,
  islands: &[&Contour],
  inset: f64,
  stepover: f64,
) -> Result<(Vec<Contour>, bool), ToolpathError> {
  let region = pocket_region(boundary, islands)?;
  let mut rings = Vec::new();
  let mut flattened = false;
  let mut index = 0usize;
  loop {
    let distance = inset + index as f64 * stepover;
    let offset = match region.offset(distance, DEFAULT_TOLERANCE) {
      Ok(offset) => offset,
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
    // A ring set with no ring in it is the same collapse stated the other way; it is never cut.
    if offset.region.boundaries.is_empty() {
      if index == 0 {
        return Err(ToolpathError::OffsetCollapsed {
          reason: "the region offset left no ring to cut".to_owned(),
        });
      }
      break;
    }
    flattened |= offset.flattened;
    // A stage that comes back as several contours is material the inset has already split into
    // pieces — a grown island cutting into the shrunken outer boundary, for instance. Every piece
    // is material the tool can still reach, so every piece is cut.
    rings.extend(offset.region.boundaries);
    index += 1;
  }
  Ok((rings, flattened))
}

/// Parallel rows across the inset region, clipped to it exactly.
///
/// The inset region comes from the kernel too, so the distance proof is the same one the ring path
/// uses and a pocket wider than the tool is never rastered over material it should leave standing.
/// The rows themselves are the kernel's [`Region::raster_spans`] (A13), which clips a row against
/// **every** boundary of the region and pairs the crossings even-odd: a row that crosses an island is
/// therefore cut as two spans, never as one, and a contour the inset split into pieces is clipped
/// against every piece. The region is the same one the ring path cuts — the boundary plus the
/// request's islands — because an island *is* a boundary of it (A3), not a separate mode.
///
/// An island-free pocket is clipped exactly the way M4's single-contour path clipped it (A15).
fn raster_rows(
  boundary: &Contour,
  islands: &[&Contour],
  inset: f64,
  stepover: f64,
) -> Result<(Vec<Row>, bool), ToolpathError> {
  let offset = pocket_region(boundary, islands)?
    .offset(inset, DEFAULT_TOLERANCE)
    .map_err(|error| ToolpathError::OffsetCollapsed {
      reason: error.to_string(),
    })?;
  // The inset can collapse the region the way it can empty a ring set, and an empty region has no
  // material to raster: `Ok` with no row would report a success that cuts nothing (R3-1).
  if offset.region.boundaries.is_empty() {
    return Err(ToolpathError::OffsetCollapsed {
      reason: "the region offset left no area to raster".to_owned(),
    });
  }
  let bounds = region_bounds(&offset.region);

  let mut rows = Vec::new();
  let mut index = 0usize;
  loop {
    let y = bounds.min.y + index as f64 * stepover;
    if y > bounds.max.y {
      break;
    }
    // A row that lands **on** a boundary is material: `row_spans` retries it to either side, which
    // is what keeps a row the lattice lands on from being dropped. `emit_row` owns both cases.
    emit_row(&mut rows, &offset.region, y);
    index += 1;
  }
  // The grid ends on the last line that fits, which is one stepover short of the far edge whenever the
  // stepover does not divide the region's height — and the strip between them is material only the
  // tool's disc at that edge reaches. So the lattice is *finished*: one more row, on the edge itself.
  //
  // Skipped when the grid already ends there. The break above is a strict `y > bounds.max.y`, so a row
  // that lands exactly on the edge is already emitted and already retried onto the material; appending
  // here would plunge into that same strip a second time. The gap is compared against the kernel's
  // tolerance for the same reason: a row a millionth of a millimetre from the edge is the same row.
  let last_grid_row = bounds.min.y + index.saturating_sub(1) as f64 * stepover;
  if bounds.max.y - last_grid_row > DEFAULT_TOLERANCE {
    emit_row(&mut rows, &offset.region, bounds.max.y);
  }
  Ok((rows, offset.flattened))
}

/// Emit one row at `y`, if the region has material on it.
///
/// A row that misses the material is dropped rather than emitted empty: half a row is worse than no
/// row, because it leaves the tool in material it cannot reach from the other side.
fn emit_row(rows: &mut Vec<Row>, region: &Region, y: f64) {
  if let Some((row_y, spans)) = row_spans(region, y) {
    if !spans.is_empty() {
      rows.push(
        spans
          .into_iter()
          .map(|(left, right)| (Point2::new(left, row_y), Point2::new(right, row_y)))
          .collect(),
      );
    }
  }
}

/// The spans of one raster row, and the `y` they were really clipped at.
///
/// The ends are true intersections with the region's boundaries, never tessellated approximations,
/// and the parity rule is the kernel's (A7): every crossing's x is collected, sorted, deduped and
/// paired even-odd, so one row across an island is two spans.
///
/// A row that lands on a boundary edge or on a tangency comes back from the kernel as ambiguous
/// (`None`): a leftover odd crossing cannot be paired honestly. A row on a boundary is **not** an
/// empty row — it is the row the region's edge sits on, and the material it clears is the strip that
/// edge ends — so the row is retried a hair to either side ([`ROW_RETRY_NUDGE`]) and the first side
/// that clips any material wins. Both sides are needed: the near edge of the region has material
/// above it and the far edge has material below it, so nudging one way only drops whichever edge the
/// row lattice happens to land on (the far edge of a 100 mm square at a 2 mm stepover, S6). The
/// candidates are tried in a fixed order — the nominal height, then up, then down — so the retry is a
/// function of the region and the height alone and never of the order the boundaries were cut.
///
/// A row that clips nothing on any of the three heights is dropped: it misses the material. The
/// **retried** height is returned with the spans, because that is the height they honestly lie on —
/// emitting a tangent row's ends at the nominal `y` would put them off the curve they were clipped
/// against.
fn row_spans(region: &Region, y: f64) -> Option<(f64, Vec<(f64, f64)>)> {
  for candidate in [y, y + ROW_RETRY_NUDGE, y - ROW_RETRY_NUDGE] {
    if let Some(spans) = region.raster_spans(candidate, DEFAULT_TOLERANCE) {
      if !spans.is_empty() {
        return Some((candidate, spans));
      }
    }
  }
  None
}

/// The box containing every boundary of a region.
fn region_bounds(region: &Region) -> Bounds {
  region
    .boundaries
    .iter()
    .fold(Bounds::empty(), |bounds, boundary| {
      bounds.union(boundary.bounds())
    })
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
