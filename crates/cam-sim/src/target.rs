//! What the document authorised each cell to lose, and the two ways a program deviates from it
//! (M5 A5).
//!
//! One target surface, derived from the document alone: a pocket authorises its region inset by the
//! wall its allowance keeps (D-M5-10), a drilling toolpath authorises each hole's circle, a profile
//! authorises the band its tool *sweeps* — `allowance … 2·radius + allowance` from the vector, less
//! whatever a tab declares it keeps (D-M5-5) — a chamfer authorises the cone its derived depth reaches
//! either side of the vector (M6 A8), an engrave authorises the band or the graded groove its one pass
//! cuts along the vector (M6 A3, D-M6-4), a flute authorises the ramp law *along* the vector rather
//! than at a distance from it (M6 A4, D-M6-5), a texture authorises the same band along every groove of
//! the document's own pattern (M6 A5, D-M6-6), an inlay authorises the half it is — the region grown by
//! the allowance for the female, the job rectangle minus the region for the male (M6 A6, D-M6-7) — and
//! every cell nothing claims is authorised to lose nothing.
//! Gouge and uncut are then one subtraction with two signs (D-M5-2): removed deeper than authorised, and
//! authorised deeper than removed.
//!
//! # What a texture's claim reads
//!
//! A texture's claim reads the document's **derived pattern** — `cam_model::TextureParams::grooves`, the
//! same call `cam-toolpath::texture` cuts — and never the emitted moves (M6 A5, D-M6-6). That is the
//! point of deriving the pattern in the model rather than in each consumer: a target built from the
//! program's own moves would agree with the program by construction, including where both are wrong, and
//! would report a program that cut nothing as clean.
//!
//! The tool's diameter is not in the document — it is in the tool library — so a caller that wants
//! profile bands supplies it through [`ToolShapes`]. A pocket or a drilling toolpath's target does
//! not need it.

use std::collections::BTreeMap;
use std::f64::consts::TAU;
use std::ops::Range;

use cam_geom::offset::{offset_contour, OffsetOptions};
use cam_geom::{region::Region, Contour, Point2, Segment, DEFAULT_TOLERANCE};
use cam_model::{Document, Operation, ProfileParams, ProfileSide, Tab, Toolpath};
use cam_move::ToolShape;
use cam_units::Length;
use thiserror::Error;

use crate::grid::{grid_for, Grid, GridError};
use crate::heightfield::Heightfield;

/// The tools a document's toolpaths refer to, by tool number.
///
/// A `BTreeMap` rather than a `HashMap`: the target surface is on a path to an output line, and
/// iteration order on that path is explicit (S9).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ToolShapes {
  shapes: BTreeMap<u32, ToolShape>,
}

impl ToolShapes {
  /// Build the table from `(tool number, shape)` pairs, later pairs winning.
  pub fn new<I: IntoIterator<Item = (u32, ToolShape)>>(tools: I) -> Self {
    let mut shapes = BTreeMap::new();
    for (number, tool) in tools {
      shapes.insert(number, tool);
    }
    Self { shapes }
  }

  /// The shape of a tool, when the caller supplied one.
  pub fn shape(&self, tool_number: u32) -> Option<ToolShape> {
    self.shapes.get(&tool_number).copied()
  }
}

/// Why a target surface could not be derived.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum TargetError {
  /// The grid the target would live on could not be built.
  #[error("{0}")]
  Grid(#[from] GridError),
  /// A profile's band needs the tool's shape, and the caller did not supply it.
  #[error(
    "toolpath {toolpath:?} uses tool {tool_number}, whose geometry the caller did not supply"
  )]
  MissingTool {
    /// The toolpath that needs it.
    toolpath: String,
    /// The tool number it refers to.
    tool_number: u32,
  },
  /// A V-bit was asked for a band on a side the target cannot authorise exactly.
  ///
  /// A cone's radius grows with depth, so the material it sweeps on one side of a vector is a wedge
  /// whose depth varies from cell to cell; the target authorises that wedge only where the tip travels
  /// on the vector (side `on`), which is where the strategies that use a V-bit put it (D-M6-2).
  #[error(
    "toolpath {toolpath:?} cuts {side} with a V-bit: a cone's band is only authorised with the tip on \
     the vector (side `on`), because a graded band on one side of it would need the cell's signed \
     distance from the offset centre path"
  )]
  ConeOnVectorOnly {
    /// The toolpath that cuts with a cone.
    toolpath: String,
    /// The side it asked for.
    side: &'static str,
  },
  /// A toolpath's geometry could not be turned into a region.
  #[error("toolpath {toolpath:?}: {reason}")]
  Geometry {
    /// The toolpath whose geometry failed.
    toolpath: String,
    /// What the kernel said, verbatim.
    reason: String,
  },
  /// A chamfer was claimed with a tool that is not a V-bit.
  ///
  /// A chamfer's depth is derived from its bit's included angle (D-M6-2), so a cylinder has no rule to
  /// grade its claim with. Approximating one would authorise a flat-bottomed slot the program never
  /// cuts (D-M6-3), and defaulting an angle would cut a different bevel (D-M6-9).
  #[error(
    "toolpath {toolpath:?} chamfers with tool {tool_number}, which the library does not describe as a \
     V-bit: a chamfer's depth comes from the tool's included angle, and there is none to use"
  )]
  ChamferNeedsVBit {
    /// The toolpath that needs it.
    toolpath: String,
    /// The tool it named.
    tool_number: u32,
  },
  /// A chamfer was claimed with a bit whose tip has a flat.
  ///
  /// With a flat of diameter `f` the bevel reaches the surface at `f/2 + d·tan(θ/2)`, so the target
  /// would authorise a groove wider than the `chamfer_width` the document names. The derivation reads
  /// the document alone (L7), so it cannot honour the width on such a bit and the bit is refused by
  /// name instead of absorbed (D-M6-10).
  #[error(
    "toolpath {toolpath:?} chamfers with tool {tool_number}, whose tip has a {flat_diameter} mm flat: \
     a chamfer's bevel would reach the surface wider than its width, so a chamfer needs a sharp V-bit"
  )]
  ChamferNeedsSharpBit {
    /// The toolpath that needs it.
    toolpath: String,
    /// The tool it named.
    tool_number: u32,
    /// The flat at the tip, in millimetres.
    flat_diameter: f64,
  },
}

/// Why a field and a target could not be compared.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum DeviationError {
  /// The two were built on different lattices, so comparing them cell by cell is meaningless.
  #[error(
    "the field is {field_columns} x {field_rows} cells of {field_cell} mm and the target is \
     {target_columns} x {target_rows} cells of {target_cell} mm: both must be built for the same \
     stock and resolution"
  )]
  GridMismatch {
    /// The field's columns.
    field_columns: usize,
    /// The field's rows.
    field_rows: usize,
    /// The field's cell size, in millimetres.
    field_cell: f64,
    /// The target's columns.
    target_columns: usize,
    /// The target's rows.
    target_rows: usize,
    /// The target's cell size, in millimetres.
    target_cell: f64,
  },
}

/// The depth each cell is authorised to lose, on the simulation's grid.
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
  grid: Grid,
  authorised: Vec<f64>,
}

impl Target {
  /// The lattice the target lives on.
  pub fn grid(&self) -> Grid {
    self.grid
  }

  /// The depth cell `(column, row)` is authorised to lose, in millimetres.
  ///
  /// # Panics
  ///
  /// Panics when the cell is outside the grid; callers iterate the grid they were given.
  pub fn authorised(&self, column: usize, row: usize) -> f64 {
    self.authorised[row * self.grid.columns + column]
  }
}

/// The two ways a field can differ from the target, as volumes and cell counts.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Deviation {
  /// Material removed deeper than authorised, in cubic millimetres.
  pub gouge: f64,
  /// Material left above what the toolpath claimed it would reach, in cubic millimetres.
  pub uncut: f64,
  /// Cells that were gouged.
  pub gouge_cells: usize,
  /// Cells that were left uncut.
  pub uncut_cells: usize,
}

/// Derive the target surface for a whole document, on a grid of `cell` millimetres.
///
/// `tools` supplies the shapes the profile bands need; a document whose toolpaths are pockets
/// and drills alone derives without it. A cell claimed by more than one toolpath is authorised the
/// **deeper** of them, never the sum: two toolpaths that both cut a cell remove it once (D-M5-2).
pub fn target_for(
  document: &Document,
  cell: Length,
  tools: &ToolShapes,
) -> Result<Target, TargetError> {
  let grid = grid_for(&document.job, cell)?;
  let mut target = Target {
    grid,
    authorised: vec![0.0; grid.cells()],
  };
  for toolpath in &document.toolpaths {
    claim_toolpath(&mut target, document, toolpath, tools)?;
  }
  Ok(target)
}

fn claim_toolpath(
  target: &mut Target,
  document: &Document,
  toolpath: &Toolpath,
  tools: &ToolShapes,
) -> Result<(), TargetError> {
  match &toolpath.operation {
    Operation::Pocket(params) => {
      // Parity does the subtraction (D-RB-2): an island is appended as one more boundary, so a cell
      // inside it is enclosed an even number of times and is not material.
      let boundaries = contours_of(
        document,
        toolpath,
        toolpath.entity_ids.iter().chain(&toolpath.island_ids),
      )?;
      // The wall the allowance keeps is material the document asked to leave standing, so it is not
      // authorised (D-M5-10).
      claim_clearing(
        target,
        toolpath,
        boundaries,
        params.allowance.mm(),
        authorised_depth(params.cut_depth),
      )?;
    }
    Operation::Inlay(params) => {
      // The pair is two claims, and the document says which half a toolpath is (see
      // `cam_model::InlayParams`). Both go through the pocket's own claim path, so a cell inside the
      // region is authorised by the female, a cell outside it by the male, and a cell both claim keeps
      // the deeper of the two and never the sum (D-M5-2).
      let depth = authorised_depth(params.cut_depth);
      if toolpath.island_ids.is_empty() {
        // The female: the region it clears, grown by the allowance. The pocket's inset is what its tool
        // leaves standing, so the cavity's oversize is the same number negated — at zero allowance no
        // cell moves, which is M5's rule for the pocket's own allowance.
        let boundaries = contours_of(document, toolpath, toolpath.entity_ids.iter())?;
        claim_clearing(target, toolpath, boundaries, -params.allowance.mm(), depth)?;
      } else {
        // The male: the job rectangle with the region standing as an island. The rectangle is the job's
        // own (`Job::outline`), the same call `cam-toolpath::inlay` cuts, so the program's boundary and
        // the authorised one cannot be different rectangles.
        let mut boundaries = vec![document.job.outline()];
        boundaries.extend(contours_of(document, toolpath, toolpath.island_ids.iter())?);
        claim_clearing(target, toolpath, boundaries, 0.0, depth)?;
      }
    }
    Operation::Drilling(params) => {
      let depth = authorised_depth(params.cut_depth);
      for id in &toolpath.entity_ids {
        let region = region_of(
          &toolpath.id,
          vec![contour_of(document, toolpath, id)?.clone()],
        )?;
        claim_region(target, &region, depth, None);
      }
    }
    Operation::Profile(params) => {
      let depth = authorised_depth(params.cut_depth);
      let tool = tools
        .shape(toolpath.tool_number)
        .ok_or_else(|| TargetError::MissingTool {
          toolpath: toolpath.id.clone(),
          tool_number: toolpath.tool_number,
        })?;
      for id in &toolpath.entity_ids {
        let contour = contour_of(document, toolpath, id)?;
        let region = kerf_band(&toolpath.id, contour, params, tool)?;
        let tabs = Tabs::new(contour, &params.tabs);
        match tool {
          ToolShape::Cylinder { .. } => claim_region(target, &region, depth, Some(&tabs)),
          ToolShape::VBit { .. } => {
            claim_cone_region(target, &region, contour, tool, depth, Some(&tabs))
          }
        }
      }
    }
    Operation::Chamfer(params) => {
      // The depth is derived once, in the document model, and `cam-toolpath` cuts at the same number
      // (D-M6-2 as amended in L7): the derivation is called here, never repeated.
      let depth = params.depth().mm();
      let tool = tools
        .shape(toolpath.tool_number)
        .ok_or_else(|| TargetError::MissingTool {
          toolpath: toolpath.id.clone(),
          tool_number: toolpath.tool_number,
        })?;
      if !matches!(tool, ToolShape::VBit { .. }) {
        return Err(TargetError::ChamferNeedsVBit {
          toolpath: toolpath.id.clone(),
          tool_number: toolpath.tool_number,
        });
      }
      if let ToolShape::VBit { flat_diameter, .. } = tool {
        if flat_diameter.mm() > 0.0 {
          // A flat tip widens the bevel by half its diameter, and the derived depth knows nothing of
          // it (L7), so a correct claim is not available from the document (D-M6-10).
          return Err(TargetError::ChamferNeedsSharpBit {
            toolpath: toolpath.id.clone(),
            tool_number: toolpath.tool_number,
            flat_diameter: flat_diameter.mm(),
          });
        }
      }
      // The widest footprint the cone has where it meets the surface, which is also where the bevel is
      // `chamfer_width` from the vector: for a bit with no flat the reach and the width are the same
      // number, and the cone's own law grades the claim inside the band.
      let reach = tool.radius_at(depth);
      for id in &toolpath.entity_ids {
        let contour = contour_of(document, toolpath, id)?;
        let region = cone_band(&toolpath.id, contour, reach)?;
        claim_cone_region(target, &region, contour, tool, depth, None);
      }
    }
    Operation::Engrave(params) => {
      // The depth is authored, so it is claimed as it stands (D-M6-4): nothing is derived and nothing
      // is added to it. The claim is built from the geometry rather than from a region, because an
      // open vector has no interior and cannot be a region at all (see `claim_along_path`).
      let depth = authorised_depth(params.cut_depth);
      let tool = tools
        .shape(toolpath.tool_number)
        .ok_or_else(|| TargetError::MissingTool {
          toolpath: toolpath.id.clone(),
          tool_number: toolpath.tool_number,
        })?;
      for id in &toolpath.entity_ids {
        let contour = contour_of(document, toolpath, id)?;
        claim_along_path(target, contour, tool, depth);
      }
    }
    Operation::Fluting(params) => {
      // A flute's claim is a depth along the *path*, not a depth at a distance: the tool ramps in over
      // the vector's first millimetres and out over its last, so the depth the program is entitled to
      // reach at a point depends on how far along that point is (A4, D-M6-5). The tool's shape then
      // grades it across the groove, exactly as an engrave's does.
      let depth = authorised_depth(params.cut_depth);
      let tool = tools
        .shape(toolpath.tool_number)
        .ok_or_else(|| TargetError::MissingTool {
          toolpath: toolpath.id.clone(),
          tool_number: toolpath.tool_number,
        })?;
      for id in &toolpath.entity_ids {
        let contour = contour_of(document, toolpath, id)?;
        claim_flute(target, contour, tool, depth, params.ramp.mm());
      }
    }
    Operation::Texture(params) => {
      // The pattern is the document's, derived once in `cam-model` (`TextureParams::grooves`) and cut by
      // `cam-toolpath::texture` from the same vertices. This claim reads that pattern and never the
      // program's moves: a claim built from the emitted moves would agree with them by construction,
      // including where both are wrong (M6 A5, D-M6-6).
      let depth = authorised_depth(params.cut_depth);
      let tool = tools
        .shape(toolpath.tool_number)
        .ok_or_else(|| TargetError::MissingTool {
          toolpath: toolpath.id.clone(),
          tool_number: toolpath.tool_number,
        })?;
      let mut contours = Vec::with_capacity(toolpath.entity_ids.len());
      for id in &toolpath.entity_ids {
        contours.push(contour_of(document, toolpath, id)?);
      }
      for groove in params.grooves(&contours) {
        let path = Contour::from_points(&groove, false).map_err(|error| TargetError::Geometry {
          toolpath: toolpath.id.clone(),
          reason: error.to_string(),
        })?;
        // A groove is one pass at one depth with the centre on it, so its claim is the tool's footprint
        // band around the groove: the same law, and the same code, an engrave's claim uses (M6 A3, A5).
        claim_along_path(target, &path, tool, depth);
      }
    }
  }
  Ok(())
}

/// The depth below the stock surface a toolpath's last pass reaches.
///
/// `start_depth` is where a pass sequence *begins* — the depth an earlier job already machined to —
/// so the deepest cut is `cut_depth` itself, an absolute depth below the surface. The emitter is the
/// evidence: `cam-toolpath`'s `pass_depths` walks from `start_depth` to `cut_depth`
/// (`crates/cam-toolpath/src/lib.rs:264-279`), refuses a `cut_depth` at or above `start_depth`, and
/// `docs/m4-toolpaths.md:60` says the last pass "lands exactly on `cut_depth`". Adding the two would
/// authorise material no program cuts, and every such cell would report as uncut.
fn authorised_depth(cut_depth: Length) -> f64 {
  cut_depth.mm()
}

/// A region inset by `distance`, islands grown by the same amount (D-M5-10).
///
/// One call does the whole region: [`Region::offset`] takes each boundary's nesting depth from the
/// kernel, so the enclosing boundaries shrink and the islands grow, which is exactly the wall an
/// allowance keeps. A zero distance returns the region unchanged — the default allowance is zero and
/// must not perturb a single cell.
fn inset_by(toolpath: &str, region: Region, distance: f64) -> Result<Region, TargetError> {
  if distance == 0.0 {
    return Ok(region);
  }
  region
    .offset(distance, DEFAULT_TOLERANCE)
    .map(|offset| offset.region)
    .map_err(|error| TargetError::Geometry {
      toolpath: toolpath.to_owned(),
      reason: error.to_string(),
    })
}

/// The kerf band a profile's tool sweeps, as a parity region (D-M5-5).
///
/// The emitter offsets the tool **centre** by `radius + allowance`
/// (`lateral_offset`, `crates/cam-toolpath/src/lib.rs:249-258`), so the disc of the tool reaches from
/// `allowance` to `2·radius + allowance` away from the vector: that is the material a clean profile
/// removes, and therefore what it is authorised to remove. Authorising `0 … radius + allowance`
/// instead reported the outer half of a perfect kerf as gouge.
fn kerf_band(
  toolpath: &str,
  contour: &Contour,
  params: &ProfileParams,
  tool: ToolShape,
) -> Result<Region, TargetError> {
  let offset = |distance: f64| offset_contour_or(toolpath, contour, distance);
  let allowance = params.allowance.mm();
  // A cone's radius grows with depth, so its widest footprint is the one at the depth it reaches.
  let radius = tool.radius_at(params.cut_depth.mm());
  // The kerf's outer edge: the tool centre's offset plus one radius of the tool that rides there.
  let reach = 2.0 * radius + allowance;
  let boundaries = match params.side {
    ProfileSide::Outside => vec![offset(reach)?, offset(allowance)?],
    ProfileSide::Inside => vec![offset(-allowance)?, offset(-reach)?],
    // The emitter runs the tool centre *on* the vector whatever the allowance says
    // (`lateral_offset`: `ProfileSide::On => Length::ZERO`), so the band is the tool's own width on
    // both sides of it and the keep side is on neither.
    ProfileSide::On => vec![offset(radius)?, offset(-radius)?],
  };
  if !matches!(tool, ToolShape::Cylinder { .. }) && params.side != ProfileSide::On {
    // Refused rather than approximated: a graded band offset to one side of the vector would grade
    // from the wrong line and report a correct cone cut as a deviation (D-M6-3).
    return Err(TargetError::ConeOnVectorOnly {
      toolpath: toolpath.to_owned(),
      side: match params.side {
        ProfileSide::Inside => "inside",
        ProfileSide::Outside => "outside",
        ProfileSide::On => "on",
      },
    });
  }
  region_of(toolpath, boundaries)
}

/// The band a chamfer's cone sweeps: the contour grown and shrunk by the cone's reach where it meets
/// the surface, as a parity region.
///
/// The tip travels **on** the vector (D-M6-2 as amended in L7), so the bevel is symmetric about it and
/// the band is both of the contour's offsets by `reach` — the widest footprint the cone has at the
/// depth it reaches. The grade inside the band is the cone's own law, claimed cell by cell by
/// [`claim_cone_region`].
fn cone_band(toolpath: &str, contour: &Contour, reach: f64) -> Result<Region, TargetError> {
  region_of(
    toolpath,
    vec![
      offset_contour_or(toolpath, contour, reach)?,
      offset_contour_or(toolpath, contour, -reach)?,
    ],
  )
}

/// `contour` offset **outward** by `distance`, or the contour itself when the distance is zero.
fn offset_contour_or(
  toolpath: &str,
  contour: &Contour,
  distance: f64,
) -> Result<Contour, TargetError> {
  if distance == 0.0 {
    // Offsetting by nothing is the contour itself; asking the kernel to walk a zero offset only risks
    // it reporting a collapse for a question nobody asked.
    return Ok(contour.clone());
  }
  offset_contour(contour, distance, &OffsetOptions::default())
    .map(|offset| offset.contour)
    .map_err(|error| TargetError::Geometry {
      toolpath: toolpath.to_owned(),
      reason: error.to_string(),
    })
}

fn region_of(toolpath: &str, boundaries: Vec<Contour>) -> Result<Region, TargetError> {
  Region::new(boundaries).map_err(|error| TargetError::Geometry {
    toolpath: toolpath.to_owned(),
    reason: error.to_string(),
  })
}

/// Claim the region a **clearing** toolpath removes, as one code path for a pocket and for both halves
/// of an inlay (M5 D-M5-10, M6 A6).
///
/// `inset` is what the tool leaves standing at the region's own boundary, in the pocket's own sense: a
/// positive allowance shrinks the region by the wall it keeps, a negative one grows it by the same
/// distance (the female inlay's cavity), and zero claims the region exactly as its boundaries describe it
/// (the male inlay, whose island list *is* the region). Everything else — the parity subtraction, the
/// nested islands and the deeper-claim rule — is the shared machinery and not repeated per strategy.
fn claim_clearing(
  target: &mut Target,
  toolpath: &Toolpath,
  boundaries: Vec<Contour>,
  inset: f64,
  depth: f64,
) -> Result<(), TargetError> {
  let region = region_of(&toolpath.id, boundaries)?;
  let region = inset_by(&toolpath.id, region, inset)?;
  claim_region(target, &region, depth, None);
  Ok(())
}

/// The contours a toolpath names by id, in the order it names them.
fn contours_of<'a>(
  document: &'a Document,
  toolpath: &Toolpath,
  ids: impl Iterator<Item = &'a String>,
) -> Result<Vec<Contour>, TargetError> {
  ids
    .map(|id| contour_of(document, toolpath, id).cloned())
    .collect()
}

fn contour_of<'a>(
  document: &'a Document,
  toolpath: &Toolpath,
  entity_id: &str,
) -> Result<&'a Contour, TargetError> {
  document
    .entity(entity_id)
    .and_then(|entity| entity.kind.as_contour())
    .ok_or_else(|| TargetError::Geometry {
      toolpath: toolpath.id.clone(),
      reason: format!("entity {entity_id:?} is not a contour in this document"),
    })
}

/// The contour's own arc-length map: every segment with the arc length at which it starts.
///
/// One projection with two callers, which is the point of it. A tab asks this where along the contour a
/// cell lies, so it can apply the tab's span; a flute's claim asks the same question, because its depth
/// law is stated in arc length (A4, D-M6-5). A second projection would answer differently at a shared
/// vertex and split one law into two.
#[derive(Clone, Debug)]
struct Walk {
  /// Each of the contour's segments, with the arc length at which it starts.
  segments: Vec<(Segment, f64)>,
}

impl Walk {
  /// Map `contour`'s segments onto their arc lengths.
  fn new(contour: &Contour) -> Self {
    let mut segments = Vec::new();
    let mut walked = 0.0;
    for segment in contour.segments() {
      segments.push((segment, walked));
      walked += segment.length();
    }
    Self { segments }
  }

  /// The arc length of the contour's point nearest to `point`.
  fn walked_at(&self, point: Point2) -> Option<f64> {
    // `min_by` keeps the **first** of equal minima, which is what makes the mapping continuous at a
    // shared vertex: two adjacent segments report the same distance and the same arc length there.
    self
      .nearest_each(point)
      .min_by(|a, b| {
        a.distance
          .partial_cmp(&b.distance)
          .unwrap_or(std::cmp::Ordering::Equal)
      })
      .map(|nearest| nearest.walked())
  }

  /// Every segment's nearest point to `point`, with its distance and its arc length.
  ///
  /// Every segment, not only the nearest one. A contour can pass a cell more than once — a closed
  /// flute's seam is two ends, and a loop can double back on itself — so a claim that asked only the
  /// nearest point would authorise the shallower pass and report the deeper one as a gouge.
  fn nearest_each(&self, point: Point2) -> impl Iterator<Item = Nearest> + '_ {
    self.segments.iter().map(move |(segment, start)| {
      let fraction = fraction_along(*segment, point);
      Nearest {
        segment: *segment,
        start: *start,
        fraction,
        distance: segment.distance_to_point(point),
      }
    })
  }
}

/// One segment's nearest point to a query, as [`Walk::nearest_each`] reports it.
///
/// The fraction is what makes a claim able to look *along* the path from the nearest point: the tool
/// has a footprint, so the deepest material a cell loses can come from a path position a few
/// millimetres away, and a flute's law varies with exactly that position (A4, D-M6-5).
#[derive(Clone, Copy, Debug)]
struct Nearest {
  /// The segment whose nearest point this is.
  segment: Segment,
  /// The arc length at which the segment starts.
  start: f64,
  /// Where on the segment its nearest point to the query lies, as a fraction in `[0, 1]`.
  fraction: f64,
  /// The query's distance to that point.
  distance: f64,
}

impl Nearest {
  /// The arc length of this nearest point along the contour.
  fn walked(&self) -> f64 {
    self.start + self.fraction * self.segment.length()
  }

  /// The arc length of the point `offset` millimetres further along the segment, clamped to it.
  fn walked_further(&self, offset: f64) -> f64 {
    let length = self.segment.length();
    let fraction = if length <= 0.0 {
      0.0
    } else {
      (self.fraction + offset / length).clamp(0.0, 1.0)
    };
    self.start + fraction * length
  }

  /// The offsets, in arc length from this nearest point, of the two ends of the part of the segment the
  /// footprint of a query at `centre` actually reaches.
  ///
  /// `sqrt(reach² − d²)` is the half-width of a chord centred on a perpendicular foot, and it is the
  /// answer only while that foot is *interior* to the segment. Beyond a vertex the foot clamps to the
  /// endpoint, the query sits off the end of the segment, and the reachable stretch is shorter, lies on
  /// one side only, and ends where the query's circle crosses the segment — which is where the law is
  /// deepest on that segment. Missing that end under-authorised every cell outside a ramping corner of a
  /// flute, and the sweep reported a *correct* flute as a gouge there.
  ///
  /// Both segments are solved in closed form, because both are circles the query's circle either crosses
  /// or misses: a line is the degenerate case of a circle with no curvature, solved as a quadratic in
  /// the arc length, and an arc is solved by intersecting the two circles and taking the angular window
  /// the intersection allows.
  fn reach_offsets(&self, centre: Point2, reach: f64) -> (f64, f64) {
    let length = self.segment.length();
    if length <= 0.0 || reach <= 0.0 || !reach.is_finite() {
      return (0.0, 0.0);
    }
    // The reachable stretch as fractions of the segment, then turned into offsets from the nearest
    // point. Fractions keep arcs and lines in one frame: arc length along the segment is
    // `fraction · length` whichever way it runs.
    let (low, high) = match self.segment {
      Segment::Line(line) => {
        // `|line.start + t·û − centre|² ≤ reach²` is `t² + b·t + c ≤ 0` in the arc length `t`.
        let span = line.end - line.start;
        let span_length = span.length();
        if span_length <= 0.0 {
          return (0.0, 0.0);
        }
        let unit = span / span_length;
        let to_start = line.start - centre;
        let b = 2.0 * to_start.dot(unit);
        let c = to_start.length_squared() - reach * reach;
        let discriminant = b * b - 4.0 * c;
        if discriminant < 0.0 {
          return (0.0, 0.0);
        }
        let root = discriminant.sqrt();
        (
          ((-b - root) / 2.0) / span_length,
          ((-b + root) / 2.0) / span_length,
        )
      }
      Segment::Arc(arc) => {
        let radius = arc.radius();
        let sweep = arc.sweep();
        let to_query = centre - arc.center();
        let distance = to_query.length();
        // Two degenerate answers the law of cosines cannot give, because it divides by `distance`: a
        // query at the arc's own centre is `radius` from every point of it, so it reaches all of the arc
        // when the reach is at least the radius and none of it otherwise.
        if sweep == 0.0 || radius <= 0.0 {
          return (0.0, length);
        }
        if distance <= 0.0 {
          return if radius <= reach {
            (0.0, length)
          } else {
            (0.0, 0.0)
          };
        }
        let k = (distance * distance + radius * radius - reach * reach) / (2.0 * distance * radius);
        if k > 1.0 {
          // The query's circle misses the arc's own circle entirely.
          return (0.0, 0.0);
        }
        if k <= -1.0 {
          // It swallows it: every point of the arc is within reach.
          return (0.0, length);
        }
        // `cos(θ − φ) ≥ k`, so the reachable angles are a window of half-width `acos(k)` around the
        // direction to the query, shifted by the nearest whole turn into the arc's own unwrapped range.
        let half_width = k.acos();
        let window = to_query.angle();
        let arc_low = arc.start_angle();
        let arc_high = arc_low + sweep;
        let (range_low, range_high) = if sweep >= 0.0 {
          (arc_low, arc_high)
        } else {
          (arc_high, arc_low)
        };
        let shift = ((range_low + range_high) / 2.0 - window) / TAU;
        let centre_angle = window + shift.round() * TAU;
        let low = (centre_angle - half_width).max(range_low);
        let high = (centre_angle + half_width).min(range_high);
        if high < low {
          return (0.0, 0.0);
        }
        // Angles back to fractions of the arc, in its own direction of travel.
        ((low - arc_low) / sweep, (high - arc_low) / sweep)
      }
    };
    let (low, high) = (low.min(high), low.max(high));
    let (low, high) = (low.clamp(0.0, 1.0), high.clamp(0.0, 1.0));
    (
      (low - self.fraction) * length,
      (high - self.fraction) * length,
    )
  }

  /// The point `offset` millimetres further along the segment, clamped to it.
  fn point_further(&self, offset: f64) -> Point2 {
    let length = self.segment.length();
    let fraction = if length <= 0.0 {
      0.0
    } else {
      (self.fraction + offset / length).clamp(0.0, 1.0)
    };
    match self.segment {
      Segment::Line(line) => line.point_at(fraction),
      Segment::Arc(arc) => arc.point_at_fraction(fraction),
    }
  }
}

/// The tab spans of a profile, in arc length along the contour.
///
/// A tab is a length of the perimeter the tool lifts over, so the whole kerf band under that span
/// keeps `height` millimetres of material — not only a half-cell halo around the vector. The band's
/// own bounds are what confine it: a cell outside the band is never claimed, so a tab can only
/// reduce a depth that the band already authorises.
#[derive(Clone, Debug)]
struct Tabs {
  /// Where along the contour a point lies.
  walk: Walk,
  /// Each tab's span, as `(from, to, kept height)` in arc-length millimetres.
  spans: Vec<(f64, f64, f64)>,
}

impl Tabs {
  /// Place `tabs` on `contour`.
  fn new(contour: &Contour, tabs: &[Tab]) -> Self {
    if tabs.is_empty() {
      return Self {
        walk: Walk {
          segments: Vec::new(),
        },
        spans: Vec::new(),
      };
    }
    let perimeter = contour.perimeter();
    let mut spans = Vec::with_capacity(tabs.len());
    for tab in tabs {
      let start = tab.position.rem_euclid(1.0) * perimeter;
      let end = start + tab.width.mm();
      if end <= perimeter {
        spans.push((start, end, tab.height.mm()));
      } else {
        // A tab that runs off the end of the perimeter comes back at its start.
        spans.push((start, perimeter, tab.height.mm()));
        spans.push((0.0, end - perimeter, tab.height.mm()));
      }
    }
    Self {
      walk: Walk::new(contour),
      spans,
    }
  }

  /// The height this tab set keeps back at `point`, or zero when no tab covers it.
  fn kept(&self, point: Point2) -> f64 {
    if self.spans.is_empty() {
      return 0.0;
    }
    let Some(walked) = self.walk.walked_at(point) else {
      return 0.0;
    };
    for (from, to, height) in &self.spans {
      if walked >= *from && walked <= *to {
        return *height;
      }
    }
    0.0
  }
}

/// Where along `segment` the point nearest to `point` lies, as a fraction in `[0, 1]`.
fn fraction_along(segment: Segment, point: Point2) -> f64 {
  match segment {
    Segment::Line(line) => {
      let span = line.end - line.start;
      let squared = span.length_squared();
      if squared <= 0.0 {
        return 0.0;
      }
      (((point - line.start).dot(span)) / squared).clamp(0.0, 1.0)
    }
    Segment::Arc(arc) => {
      let angle = (point - arc.center()).angle();
      let delta = if arc.sweep() > 0.0 {
        wrap_positive(angle - arc.start_angle())
      } else {
        -wrap_positive(arc.start_angle() - angle)
      };
      (delta / arc.sweep()).clamp(0.0, 1.0)
    }
  }
}

/// `angle` shifted into `[0, 2pi)`.
fn wrap_positive(angle: f64) -> f64 {
  angle - TAU * (angle / TAU).floor()
}

/// Claim every cell of a region for `depth`, less whatever the tabs keep, keeping the deeper claim.
fn claim_region(target: &mut Target, region: &Region, depth: f64, tabs: Option<&Tabs>) {
  let grid = target.grid;
  let columns = grid.columns;
  let Some(window) = cell_window(region, grid) else {
    return;
  };
  for row in window.rows {
    for column in window.columns.clone() {
      let centre = grid.centre(column, row);
      if !region.contains(centre, DEFAULT_TOLERANCE) {
        continue;
      }
      let kept = tabs.map_or(0.0, |tabs| tabs.kept(centre));
      let authorised = (depth - kept).max(0.0);
      let slot = &mut target.authorised[row * columns + column];
      if authorised > *slot {
        *slot = authorised;
      }
    }
  }
}

/// Claim a cone's band: each cell is authorised the depth the cone reaches at its own distance from
/// `contour`, capped at `depth` (M6 A8).
///
/// This is the difference the shape makes. A cylinder's footprint has one radius, so the whole band is
/// cut to one depth; a V-bit's radius grows with depth below the tip, so a cell `d` millimetres from the
/// vector can only be reached as deep as the cone is wide there. Authorising the full depth across the
/// band would report a correct bevel as uncut — the same failure mode D-M5-5 was amended for, with the
/// sign reversed.
fn claim_cone_region(
  target: &mut Target,
  region: &Region,
  contour: &Contour,
  tool: ToolShape,
  depth: f64,
  tabs: Option<&Tabs>,
) {
  let grid = target.grid;
  let columns = grid.columns;
  let Some(window) = cell_window(region, grid) else {
    return;
  };
  let segments = contour.segments();
  for row in window.rows {
    for column in window.columns.clone() {
      let centre = grid.centre(column, row);
      if !region.contains(centre, DEFAULT_TOLERANCE) {
        continue;
      }
      let distance = segments
        .iter()
        .map(|segment| segment.distance_to_point(centre))
        .fold(f64::INFINITY, f64::min);
      let kept = tabs.map_or(0.0, |tabs| tabs.kept(centre));
      // The cone reaches a point `distance` from the vector only as deep as its flank is steep
      // enough to: `depth − rise(distance)`, and nothing beyond the footprint it has at the surface.
      let reached = (depth - tool.rise_at_radius(distance)).max(0.0);
      let authorised = (reached - kept).max(0.0);
      let slot = &mut target.authorised[row * columns + column];
      if authorised > *slot {
        *slot = authorised;
      }
    }
  }
}

/// The cells that can hold a point of a region, clamped to the grid.
struct Window {
  /// The columns the region's bounds touch.
  columns: Range<usize>,
  /// The rows the region's bounds touch.
  rows: Range<usize>,
}

/// Claim the band or groove a one-pass engrave cuts along `contour` (M6 A3, D-M6-4).
///
/// This is [`claim_cone_region`] without a region, and the reason is not layout: an **open** vector has
/// no interior, so it cannot be a `Region` at all, and a closed one claimed as a filled region would
/// authorise a clearing pass an engrave never cuts. The claim is therefore built from the geometry —
/// the cells of the contour's own bounds widened by the tool's reach at `depth`, each authorised
/// `depth − rise(distance to the contour)`, clamped at zero — which is the same law the sweep applies
/// and the same law `claim_cone_region` applies inside its band.
///
/// The distance test is not redundant with the window. A cell at a corner of the window is up to
/// `reach · √2` from the vector, and a cylinder's `rise` is zero at every radius: without the test an
/// end mill would authorise the whole window instead of the band its footprint sweeps. A sharp V-bit
/// clamps itself (its `rise` reaches `depth` exactly at `reach`), so the test only has real work to do
/// for a cylinder or a flat tip — which is exactly why it must be written, not assumed.
fn claim_along_path(target: &mut Target, contour: &Contour, tool: ToolShape, depth: f64) {
  let grid = target.grid;
  let columns = grid.columns;
  let reach = tool.radius_at(depth);
  let bounds = contour.bounds();
  let widened = cam_geom::Bounds {
    min: Point2::new(bounds.min.x - reach, bounds.min.y - reach),
    max: Point2::new(bounds.max.x + reach, bounds.max.y + reach),
  };
  let Some(window) = bounds_window(widened, grid) else {
    return;
  };
  let segments = contour.segments();
  for row in window.rows {
    for column in window.columns.clone() {
      let centre = grid.centre(column, row);
      let distance = segments
        .iter()
        .map(|segment| segment.distance_to_point(centre))
        .fold(f64::INFINITY, f64::min);
      if distance > reach {
        continue;
      }
      // The tool's own law: a cell `distance` from the vector is reached `depth − rise(distance)` deep,
      // and a cylinder's `rise` is zero, so its band is uniform (M6 A3, A8).
      let authorised = (depth - tool.rise_at_radius(distance)).max(0.0);
      let slot = &mut target.authorised[row * columns + column];
      if authorised > *slot {
        *slot = authorised;
      }
    }
  }
}

/// How far outside a tool's footprint a cell may sit and still be claimed, in millimetres.
///
/// Nothing is rounded to this: it only keeps a candidate that lies *exactly* on the footprint's edge
/// from being dropped by a floating-point rounding error, which would lose the deepest claim an end
/// mill has at that cell.
const REACH_SLACK: f64 = 1.0e-9;

/// Claim the groove a flute cuts: the ramp law **along** the vector, swept by the tool's footprint.
///
/// The law is D-M6-5's, and it is stated in arc length rather than in depth at a distance: the depth the
/// program is entitled to reach at a point `s` millimetres along the vector is
///
/// - `min(depth, depth · s / ramp)` on the way in,
/// - `min(depth, depth · (length − s) / ramp)` on the way out, and
/// - `depth` between them,
///
/// with `ramp = 0` the whole vector being the plateau. A cell is authorised that law less the tool's own
/// `rise_at_radius` at its distance from the path, clamped at zero — the same subtraction a chamfer's
/// cone and an engrave's groove pay (M6 A8).
///
/// # Why the claim looks along the path and not only at the nearest point
///
/// The law is stated at the **tip's** position, but a tool occupies a footprint and every part of it
/// removes material: a 6 mm end mill's flat bottom is still in the cut 3 mm ahead of its own tip, so a
/// cell three millimetres ahead of the tip is removed to the depth the tip reaches *there* — which on a
/// descending ramp is the deeper one. That is real geometry and not an artefact: the toe of a ramped
/// groove is a slope, and its floor near the entry lies below the line the tip followed.
///
/// Authorising the tip's law alone would therefore report a **correct** ramped flute as a gouge along
/// both ramps, by up to `radius · depth / ramp` — the mirror image of the failure D-M5-5 was amended for,
/// where a correct kerf's outer half was reported as a gouge. So the claim is the *maximum* the law
/// allows anywhere the tool could have removed that cell: the law at the deepest path position within
/// the footprint, less the rise the tool pays to reach the cell from there.
///
/// The maximum is found exactly, not sampled. On any one piece of the law the claim as a function of
/// the position along the path is a straight line minus the tool's (convex) rise, so it is concave and
/// its maximum is either at an end of the piece — the cell's own point, an edge of the footprint, or a
/// breakpoint of the law — or where its derivative vanishes, which has a closed form. Those candidates
/// are what [`flute_candidates`] returns. For an end mill, whose rise is zero, the interior candidate
/// does not exist and the ends are the whole story.
fn claim_flute(target: &mut Target, contour: &Contour, tool: ToolShape, depth: f64, ramp: f64) {
  let grid = target.grid;
  let columns = grid.columns;
  let reach = tool.radius_at(depth);
  let length = contour.perimeter();
  let walk = Walk::new(contour);
  let bounds = contour.bounds();
  let widened = cam_geom::Bounds {
    min: Point2::new(bounds.min.x - reach, bounds.min.y - reach),
    max: Point2::new(bounds.max.x + reach, bounds.max.y + reach),
  };
  let Some(window) = bounds_window(widened, grid) else {
    return;
  };
  // The depth the ramp authorises at `walked` millimetres along the vector. With no ramp the entry and
  // the exit terms are the whole depth, so both `min`s are no-ops and the plateau is everything.
  let law = |walked: f64| -> f64 {
    if ramp <= 0.0 {
      return depth;
    }
    let entering = depth * walked / ramp;
    let leaving = depth * (length - walked) / ramp;
    depth.min(entering).min(leaving)
  };
  // The ramp's own slope, and the tool's rise per millimetre of radius (zero for a cylinder, `1/tan`
  // of the half angle for a cone). Together they decide whether the claim's maximum can lie between
  // the candidates rather than at them.
  let ramp_slope = if ramp > 0.0 { depth / ramp } else { 0.0 };
  let rise_slope = match tool {
    // A cylinder's business end is flat: its rise is zero at every radius, so the derivative can
    // never vanish and the footprint's ends are the whole answer.
    ToolShape::Cylinder { .. } => 0.0,
    ToolShape::VBit { included_angle, .. } => {
      let half = included_angle.radians() / 2.0;
      if half.tan() > 0.0 {
        1.0 / half.tan()
      } else {
        0.0
      }
    }
  };
  for row in window.rows {
    for column in window.columns.clone() {
      let centre = grid.centre(column, row);
      let mut authorised: f64 = 0.0;
      for nearest in walk.nearest_each(centre) {
        if nearest.distance > reach + REACH_SLACK {
          continue;
        }
        for offset in flute_candidates(
          &nearest, centre, reach, ramp, length, ramp_slope, rise_slope,
        ) {
          let point = nearest.point_further(offset);
          let distance = centre.distance_to(point);
          // The footprint's own edge is where a flute's claim is often *worst* at a cell — an end
          // mill's deepest reach is exactly there — and on an arc or a diagonal the boundary lands a
          // rounding error outside a bare `>`. The slack is a billionth of a millimetre, far below any
          // cell, and it can only matter for a cell sitting on the boundary itself.
          if distance > reach + REACH_SLACK {
            continue;
          }
          let reached =
            (law(nearest.walked_further(offset)) - tool.rise_at_radius(distance)).max(0.0);
          if reached > authorised {
            authorised = reached;
          }
        }
      }
      let slot = &mut target.authorised[row * columns + column];
      if authorised > *slot {
        *slot = authorised;
      }
    }
  }
}

/// The offsets, in millimetres along the path from a cell's nearest point, where a flute's claim can be
/// at its deepest.
///
/// See [`claim_flute`]: the claim is concave on each piece of the law, so its maximum is at a breakpoint
/// — the cell's own point (`0`), an end of the footprint (`±e`, where `e` is how far the footprint
/// reaches along the path from a nearest point `distance` away), or a change of the law's slope
/// (`ramp − s` and `(length − ramp) − s`) — or where the derivative vanishes, which is `u*` below.
fn flute_candidates(
  nearest: &Nearest,
  centre: Point2,
  reach: f64,
  ramp: f64,
  length: f64,
  ramp_slope: f64,
  rise_slope: f64,
) -> Vec<f64> {
  let mut offsets = vec![0.0];
  // The two ends of the stretch of this segment the footprint reaches, measured from the cell's nearest
  // point. On an interior foot these are `±sqrt(reach² − d²)`; past a vertex they are not symmetric and
  // the far end — where the law is deepest — is shorter than that chord, which is the end that has to be
  // evaluated for the claim to cover a corner.
  let (reach_low, reach_high) = nearest.reach_offsets(centre, reach);
  offsets.push(reach_low);
  offsets.push(reach_high);
  if ramp > 0.0 {
    // The breakpoints of the law, as offsets from the cell's own point.
    let walked = nearest.walked();
    offsets.push(ramp - walked);
    offsets.push((length - ramp) - walked);
    // The interior maximum: `d/ds [slope·s − rise(√(x² + (Δs)²))] = 0` is
    // `Δs / √(x² + Δs²) = slope / rise_slope`, so with `c` that ratio, `Δs = c·x / √(1 − c²)`, and it
    // exists only while `|c| < 1`. Its sign follows the law's own slope, so `+` on the way in and `−`
    // on the way out.
    if rise_slope > 0.0 {
      for slope in [ramp_slope, -ramp_slope] {
        let c = slope / rise_slope;
        if c.abs() < 1.0 {
          offsets.push(c * nearest.distance / (1.0 - c * c).sqrt());
        }
      }
    }
  }
  offsets
}

/// The cell window that contains a region, clamped to the grid.
fn cell_window(region: &Region, grid: Grid) -> Option<Window> {
  let mut bounds = cam_geom::Bounds::empty();
  for boundary in &region.boundaries {
    let one = boundary.bounds();
    bounds.min = Point2::new(bounds.min.x.min(one.min.x), bounds.min.y.min(one.min.y));
    bounds.max = Point2::new(bounds.max.x.max(one.max.x), bounds.max.y.max(one.max.y));
  }
  bounds_window(bounds, grid)
}

/// The cells a bounding box touches, clamped to the grid.
fn bounds_window(bounds: cam_geom::Bounds, grid: Grid) -> Option<Window> {
  if !bounds.min.x.is_finite()
    || !bounds.min.y.is_finite()
    || !bounds.max.x.is_finite()
    || !bounds.max.y.is_finite()
  {
    return None;
  }
  let cell = grid.cell.mm();
  let column = |x: f64| cell_index(x, grid.origin.x, cell, grid.columns);
  let row = |y: f64| cell_index(y, grid.origin.y, cell, grid.rows);
  Some(Window {
    columns: column(bounds.min.x)..column(bounds.max.x) + 1,
    rows: row(bounds.min.y)..row(bounds.max.y) + 1,
  })
}

/// The cell that holds `value` along one axis, clamped to `0 .. count`.
fn cell_index(value: f64, origin: f64, cell: f64, count: usize) -> usize {
  let raw = ((value - origin) / cell).floor();
  if raw <= 0.0 {
    0
  } else if raw >= count.saturating_sub(1) as f64 {
    count.saturating_sub(1)
  } else {
    raw as usize
  }
}

/// Compare a field against the target: one subtraction, two signs (D-M5-2).
pub fn deviations(
  field: &Heightfield,
  target: &Target,
  tolerance: Length,
) -> Result<Deviation, DeviationError> {
  let grid = field.grid();
  let cell = grid.cell.mm();
  let lattice = target.grid;
  if grid.columns != lattice.columns || grid.rows != lattice.rows || cell != lattice.cell.mm() {
    return Err(DeviationError::GridMismatch {
      field_columns: grid.columns,
      field_rows: grid.rows,
      field_cell: cell,
      target_columns: lattice.columns,
      target_rows: lattice.rows,
      target_cell: lattice.cell.mm(),
    });
  }
  let area = grid.cell_area();
  let tolerance = tolerance.mm();
  let mut deviation = Deviation::default();
  for row in 0..grid.rows {
    for column in 0..grid.columns {
      let removed = -field.surface(column, row);
      let authorised = target.authorised(column, row);
      if removed > authorised + tolerance {
        deviation.gouge += (removed - authorised) * area;
        deviation.gouge_cells += 1;
      } else if authorised > removed + tolerance {
        deviation.uncut += (authorised - removed) * area;
        deviation.uncut_cells += 1;
      }
    }
  }
  Ok(deviation)
}

#[cfg(test)]
mod reach {
  //! The footprint's reachable stretch along one segment, pinned directly.
  //!
  //! Integration tests can only show that a *field* deviates by nothing, which says nothing about
  //! *why*; this pins the geometry the flute's claim stands on, including the case that was wrong: a
  //! query beyond a vertex, where the reachable stretch is shorter than the chord half-width and lies on
  //! one side only.

  use super::{Nearest, Walk};
  use cam_geom::{Contour, Point2};

  /// The nearest point of `contour`'s single segment to `query`.
  fn nearest(contour: &Contour, query: Point2) -> Nearest {
    Walk::new(contour)
      .nearest_each(query)
      .next()
      .expect("the contour has a segment")
  }

  fn line(points: &[(f64, f64)]) -> Contour {
    Contour::from_points(
      &points
        .iter()
        .map(|(x, y)| Point2::new(*x, *y))
        .collect::<Vec<_>>(),
      false,
    )
    .expect("a polyline")
  }

  #[test]
  fn beside_a_segment_the_reachable_stretch_is_the_chord_about_the_perpendicular_foot() {
    // A cell half a millimetre off a 20 mm segment with a 3 mm reach: `sqrt(9 − 0.25) = 2.958` either
    // way from the foot, which is the half-width the old formula computed and is still right here.
    let contour = line(&[(0.0, 0.0), (20.0, 0.0)]);
    let query = Point2::new(10.5, 0.5);
    let (low, high) = nearest(&contour, query).reach_offsets(query, 3.0);
    let half_chord = (9.0_f64 - 0.25).sqrt();
    assert!((low + half_chord).abs() < 1e-12, "low {low}");
    assert!((high - half_chord).abs() < 1e-12, "high {high}");
  }

  #[test]
  fn past_a_vertex_the_reachable_stretch_ends_where_the_circle_crosses_the_segment() {
    // The case the claim got wrong. A cell at (18.5, 18.5) and a segment starting at (20, 20) and
    // running along +x: `1.5² + 2·1.5·t + t² ≤ 3²` gives `t ≤ (−3 + sqrt(27)) / 2 = 1.098` mm, and
    // nothing on the other side, so the reachable stretch is `[0, 1.098]` — the chord formula's 2.121 mm
    // is nearly twice as far and points outside the footprint.
    let contour = line(&[(20.0, 20.0), (60.0, 20.0)]);
    let query = Point2::new(18.5, 18.5);
    let (low, high) = nearest(&contour, query).reach_offsets(query, 3.0);
    let reachable = (-3.0 + 27.0_f64.sqrt()) / 2.0;
    assert!(low.abs() < 1e-12, "nothing before the vertex: low {low}");
    assert!((high - reachable).abs() < 1e-12, "high {high}");
    assert!(
      high < (9.0_f64 - 4.5).sqrt(),
      "and strictly shorter than the chord half-width"
    );
  }

  #[test]
  fn an_arcs_reachable_stretch_is_the_angular_window_the_law_of_cosines_allows() {
    // A quarter turn of radius 10 about the origin, from angle 0 to π/2, with a 2 mm reach and a cell at
    // (10, 0.5): `k = (d² + r² − reach²) / (2·d·r)` with `d = sqrt(100.25)`, so the window's half-width
    // is `acos(k)` on each side of the direction to the cell, clamped to the arc.
    let arc = cam_geom::Arc::new(
      Point2::new(0.0, 0.0),
      10.0,
      0.0,
      std::f64::consts::FRAC_PI_2,
    )
    .expect("an arc");
    let contour = Contour::from_segments(&[cam_geom::Segment::Arc(arc)], false).expect("a contour");
    let query = Point2::new(10.0, 0.5);
    let (low, high) = nearest(&contour, query).reach_offsets(query, 2.0);
    let distance = (100.25_f64).sqrt();
    let k = (distance * distance + 100.0 - 4.0) / (2.0 * distance * 10.0);
    let half_width = k.acos();
    let direction = 0.5_f64.atan2(10.0);
    // The nearest point is the arc's point in the direction of the cell, `direction · r = 0.4996` mm
    // along the arc. The window's far end is `half_width · r = 2.007` mm beyond it, inside the arc; its
    // near end is `half_width · r` behind it, which is past the arc's own start, so the stretch begins at
    // the start — 0.4996 mm behind the nearest point.
    assert!(
      (low + direction * 10.0).abs() < 1e-9,
      "low {low}, the arc's own start"
    );
    let expected_high = half_width * 10.0;
    assert!(
      (high - expected_high).abs() < 1e-9,
      "the window's far end: high {high}, expected {expected_high}"
    );
  }

  #[test]
  fn a_query_inside_the_reachable_ring_gets_the_whole_segment() {
    // A cell at an arc's own centre is `radius` from every point of it, so a 12 mm reach on a 10 mm arc
    // reaches all of it — the branch that used to fall through a division by zero.
    let arc = cam_geom::Arc::new(
      Point2::new(0.0, 0.0),
      10.0,
      0.0,
      std::f64::consts::FRAC_PI_2,
    )
    .expect("an arc");
    let contour = Contour::from_segments(&[cam_geom::Segment::Arc(arc)], false).expect("a contour");
    let centre = Point2::new(0.0, 0.0);
    let (low, high) = nearest(&contour, centre).reach_offsets(centre, 12.0);
    let length = std::f64::consts::FRAC_PI_2 * 10.0;
    assert!(low.abs() < 1e-12, "low {low}");
    assert!(
      (high - length).abs() < 1e-12,
      "high {high}, length {length}"
    );
  }
}
