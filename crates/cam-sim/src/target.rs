//! What the document authorised each cell to lose, and the two ways a program deviates from it
//! (M5 A5).
//!
//! One target surface, derived from the document alone: a pocket authorises its region inset by the
//! wall its allowance keeps (D-M5-10), a drilling toolpath authorises each hole's circle, a profile
//! authorises the band its tool *sweeps* — `allowance … 2·radius + allowance` from the vector, less
//! whatever a tab declares it keeps (D-M5-5) — and every cell nothing claims is authorised to lose
//! nothing. Gouge and uncut are then one subtraction with two signs (D-M5-2): removed deeper than
//! authorised, and authorised deeper than removed.
//!
//! The tool's diameter is not in the document — it is in the tool library — so a caller that wants
//! profile bands supplies it through [`ToolDiameters`]. A pocket or a drilling toolpath's target does
//! not need it.

use std::collections::BTreeMap;
use std::f64::consts::TAU;
use std::ops::Range;

use cam_geom::offset::{offset_contour, OffsetOptions};
use cam_geom::{region::Region, Contour, Point2, Segment, DEFAULT_TOLERANCE};
use cam_model::{Document, Operation, ProfileParams, ProfileSide, Tab, Toolpath};
use cam_units::Length;
use thiserror::Error;

use crate::grid::{grid_for, Grid, GridError};
use crate::heightfield::Heightfield;

/// The cutting diameters a document's toolpaths refer to, by tool number.
///
/// A `BTreeMap` rather than a `HashMap`: the target surface is on a path to an output line, and
/// iteration order on that path is explicit (S9).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ToolDiameters {
  diameters: BTreeMap<u32, f64>,
}

impl ToolDiameters {
  /// Build the table from `(tool number, diameter)` pairs, later pairs winning.
  pub fn new<I: IntoIterator<Item = (u32, Length)>>(tools: I) -> Self {
    let mut diameters = BTreeMap::new();
    for (number, diameter) in tools {
      diameters.insert(number, diameter.mm());
    }
    Self { diameters }
  }

  /// The diameter of a tool, when the caller supplied one.
  pub fn diameter(&self, tool_number: u32) -> Option<Length> {
    self
      .diameters
      .get(&tool_number)
      .copied()
      .map(Length::from_mm)
  }
}

/// Why a target surface could not be derived.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum TargetError {
  /// The grid the target would live on could not be built.
  #[error("{0}")]
  Grid(#[from] GridError),
  /// A profile's band needs the tool's size, and the caller did not supply it.
  #[error(
    "toolpath {toolpath:?} uses tool {tool_number}, whose diameter the caller did not supply"
  )]
  MissingTool {
    /// The toolpath that needs it.
    toolpath: String,
    /// The tool number it refers to.
    tool_number: u32,
  },
  /// A toolpath's geometry could not be turned into a region.
  #[error("toolpath {toolpath:?}: {reason}")]
  Geometry {
    /// The toolpath whose geometry failed.
    toolpath: String,
    /// What the kernel said, verbatim.
    reason: String,
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
/// `tools` supplies the diameters the profile bands need; a document whose toolpaths are pockets
/// and drills alone derives without it. A cell claimed by more than one toolpath is authorised the
/// **deeper** of them, never the sum: two toolpaths that both cut a cell remove it once (D-M5-2).
pub fn target_for(
  document: &Document,
  cell: Length,
  tools: &ToolDiameters,
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
  tools: &ToolDiameters,
) -> Result<(), TargetError> {
  match &toolpath.operation {
    Operation::Pocket(params) => {
      // Parity does the subtraction (D-RB-2): an island is appended as one more boundary, so a cell
      // inside it is enclosed an even number of times and is not material.
      let mut boundaries =
        Vec::with_capacity(toolpath.entity_ids.len() + toolpath.island_ids.len());
      for id in toolpath.entity_ids.iter().chain(&toolpath.island_ids) {
        boundaries.push(contour_of(document, toolpath, id)?.clone());
      }
      let region = region_of(&toolpath.id, boundaries)?;
      // The wall the allowance keeps is material the document asked to leave standing, so it is not
      // authorised (D-M5-10).
      let region = inset_by(&toolpath.id, region, params.allowance.mm())?;
      claim_region(target, &region, authorised_depth(params.cut_depth), None);
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
      let diameter =
        tools
          .diameter(toolpath.tool_number)
          .ok_or_else(|| TargetError::MissingTool {
            toolpath: toolpath.id.clone(),
            tool_number: toolpath.tool_number,
          })?;
      let radius = diameter.mm() / 2.0;
      for id in &toolpath.entity_ids {
        let contour = contour_of(document, toolpath, id)?;
        let region = kerf_band(&toolpath.id, contour, params, radius)?;
        let tabs = Tabs::new(contour, &params.tabs);
        claim_region(target, &region, depth, Some(&tabs));
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
  radius: f64,
) -> Result<Region, TargetError> {
  let offset = |distance: f64| -> Result<Contour, TargetError> {
    if distance == 0.0 {
      // Offsetting by nothing is the contour itself; asking the kernel to walk a zero offset only
      // risks it reporting a collapse for a question nobody asked.
      return Ok(contour.clone());
    }
    offset_contour(contour, distance, &OffsetOptions::default())
      .map(|offset| offset.contour)
      .map_err(|error| TargetError::Geometry {
        toolpath: toolpath.to_owned(),
        reason: error.to_string(),
      })
  };
  let allowance = params.allowance.mm();
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
  region_of(toolpath, boundaries)
}

fn region_of(toolpath: &str, boundaries: Vec<Contour>) -> Result<Region, TargetError> {
  Region::new(boundaries).map_err(|error| TargetError::Geometry {
    toolpath: toolpath.to_owned(),
    reason: error.to_string(),
  })
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

/// The tab spans of a profile, in arc length along the contour.
///
/// A tab is a length of the perimeter the tool lifts over, so the whole kerf band under that span
/// keeps `height` millimetres of material — not only a half-cell halo around the vector. The band's
/// own bounds are what confine it: a cell outside the band is never claimed, so a tab can only
/// reduce a depth that the band already authorises.
#[derive(Clone, Debug)]
struct Tabs {
  /// Each of the contour's segments, with the arc length at which it starts.
  segments: Vec<(Segment, f64)>,
  /// Each tab's span, as `(from, to, kept height)` in arc-length millimetres.
  spans: Vec<(f64, f64, f64)>,
}

impl Tabs {
  /// Place `tabs` on `contour`.
  fn new(contour: &Contour, tabs: &[Tab]) -> Self {
    if tabs.is_empty() {
      return Self {
        segments: Vec::new(),
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
    let contour_segments = contour.segments();
    let mut segments = Vec::with_capacity(contour_segments.len());
    let mut walked = 0.0;
    for segment in contour_segments {
      segments.push((segment, walked));
      walked += segment.length();
    }
    Self { segments, spans }
  }

  /// The height this tab set keeps back at `point`, or zero when no tab covers it.
  fn kept(&self, point: Point2) -> f64 {
    if self.spans.is_empty() {
      return 0.0;
    }
    let Some(walked) = self.walked_at(point) else {
      return 0.0;
    };
    for (from, to, height) in &self.spans {
      if walked >= *from && walked <= *to {
        return *height;
      }
    }
    0.0
  }

  /// The arc length of the contour's point nearest to `point`.
  fn walked_at(&self, point: Point2) -> Option<f64> {
    let mut nearest: Option<(f64, f64)> = None;
    for (segment, start) in &self.segments {
      let distance = segment.distance_to_point(point);
      // `<` rather than `<=`: two adjacent segments report the same point and the same arc length at
      // the vertex they share, so keeping the earlier one leaves the mapping continuous.
      if nearest.is_some_and(|(nearest, _)| nearest <= distance) {
        continue;
      }
      nearest = Some((
        distance,
        start + fraction_along(*segment, point) * segment.length(),
      ));
    }
    nearest.map(|(_, walked)| walked)
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

/// The cells that can hold a point of a region, clamped to the grid.
struct Window {
  /// The columns the region's bounds touch.
  columns: Range<usize>,
  /// The rows the region's bounds touch.
  rows: Range<usize>,
}

/// The cell window that contains a region, clamped to the grid.
fn cell_window(region: &Region, grid: Grid) -> Option<Window> {
  let mut bounds = cam_geom::Bounds::empty();
  for boundary in &region.boundaries {
    let one = boundary.bounds();
    bounds.min = Point2::new(bounds.min.x.min(one.min.x), bounds.min.y.min(one.min.y));
    bounds.max = Point2::new(bounds.max.x.max(one.max.x), bounds.max.y.max(one.max.y));
  }
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
