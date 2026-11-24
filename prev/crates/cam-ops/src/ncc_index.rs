//! Simple spatial grid for 2D AABB/segment neighborhood queries during NCC fill (E4.3).
//!
//! A uniform grid indexes contour segment bounding boxes so intersection/neighbor queries can
//! visit only nearby cells instead of scanning every segment.

use cam_geom::{Bounds, Segment};

/// Axis-aligned bounding box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
  /// Minimum x.
  pub min_x: f64,
  /// Minimum y.
  pub min_y: f64,
  /// Maximum x.
  pub max_x: f64,
  /// Maximum y.
  pub max_y: f64,
}

impl Aabb {
  /// Build an AABB from bounds.
  pub fn from_bounds(bounds: Bounds) -> Self {
    Self {
      min_x: bounds.min.x,
      min_y: bounds.min.y,
      max_x: bounds.max.x,
      max_y: bounds.max.y,
    }
  }

  /// Whether two AABBs overlap (including touching edges).
  pub fn overlaps(self, other: Self) -> bool {
    self.min_x <= other.max_x
      && self.max_x >= other.min_x
      && self.min_y <= other.max_y
      && self.max_y >= other.min_y
  }

  /// Expand by `margin` in all directions.
  pub fn expanded(self, margin: f64) -> Self {
    Self {
      min_x: self.min_x - margin,
      min_y: self.min_y - margin,
      max_x: self.max_x + margin,
      max_y: self.max_y + margin,
    }
  }
}

/// A segment reference stored in the grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SegmentRef {
  /// Contour index.
  pub contour: usize,
  /// Segment index within contour.
  pub segment: usize,
}

/// Uniform spatial grid over segment AABBs.
#[derive(Clone, Debug)]
pub struct SpatialGrid {
  origin_x: f64,
  origin_y: f64,
  cell_size: f64,
  cols: usize,
  rows: usize,
  cells: Vec<Vec<usize>>,
  items: Vec<(Aabb, SegmentRef)>,
}

impl SpatialGrid {
  /// Build a grid over `segments` grouped by contour.
  pub fn build(contours: &[Vec<Segment>], cell_size: f64) -> Self {
    let mut items = Vec::new();
    let mut bounds: Option<Aabb> = None;
    for (ci, contour) in contours.iter().enumerate() {
      for (si, segment) in contour.iter().enumerate() {
        let aabb = Aabb::from_bounds(segment.bounds());
        bounds = Some(match bounds {
          None => aabb,
          Some(b) => Aabb {
            min_x: b.min_x.min(aabb.min_x),
            min_y: b.min_y.min(aabb.min_y),
            max_x: b.max_x.max(aabb.max_x),
            max_y: b.max_y.max(aabb.max_y),
          },
        });
        items.push((
          aabb,
          SegmentRef {
            contour: ci,
            segment: si,
          },
        ));
      }
    }

    let Some(b) = bounds else {
      return Self {
        origin_x: 0.0,
        origin_y: 0.0,
        cell_size: cell_size.max(1.0),
        cols: 1,
        rows: 1,
        cells: vec![Vec::new()],
        items,
      };
    };
    let cell_size = if cell_size.is_finite() && cell_size > 0.0 {
      cell_size
    } else {
      1.0
    };
    let margin = cell_size;
    let origin_x = b.min_x - margin;
    let origin_y = b.min_y - margin;
    let max_x = b.max_x + margin;
    let max_y = b.max_y + margin;
    let cols = (((max_x - origin_x) / cell_size).ceil() as usize).max(1);
    let rows = (((max_y - origin_y) / cell_size).ceil() as usize).max(1);
    let mut cells: Vec<Vec<usize>> = vec![Vec::new(); cols.saturating_mul(rows).max(1)];

    for (i, (aabb, _)) in items.iter().enumerate() {
      let (c0, r0, c1, r1) = cell_range(*aabb, origin_x, origin_y, cell_size, cols, rows);
      for r in r0..=r1 {
        for c in c0..=c1 {
          let idx = r * cols + c;
          cells[idx].push(i);
        }
      }
    }

    Self {
      origin_x,
      origin_y,
      cell_size,
      cols,
      rows,
      cells,
      items,
    }
  }

  /// Query candidate segment refs whose AABB overlaps `query`.
  pub fn query_aabb(&self, query: Aabb) -> Vec<SegmentRef> {
    let (c0, r0, c1, r1) = cell_range(
      query,
      self.origin_x,
      self.origin_y,
      self.cell_size,
      self.cols,
      self.rows,
    );
    let mut out = Vec::new();
    let mut seen = vec![false; self.items.len()];
    for r in r0..=r1 {
      for c in c0..=c1 {
        let idx = r * self.cols + c;
        for &item_idx in &self.cells[idx] {
          if seen[item_idx] {
            continue;
          }
          seen[item_idx] = true;
          let (aabb, seg_ref) = &self.items[item_idx];
          if aabb.overlaps(query) {
            out.push(*seg_ref);
          }
        }
      }
    }
    out
  }

  /// Indexed items (for naive comparison/benchmarks).
  pub fn items(&self) -> &[(Aabb, SegmentRef)] {
    &self.items
  }

  /// Number of indexed segments.
  pub fn len(&self) -> usize {
    self.items.len()
  }

  /// Whether the grid has no segments.
  pub fn is_empty(&self) -> bool {
    self.items.is_empty()
  }
}

fn cell_range(
  aabb: Aabb,
  origin_x: f64,
  origin_y: f64,
  cell_size: f64,
  cols: usize,
  rows: usize,
) -> (usize, usize, usize, usize) {
  let c0 =
    (((aabb.min_x - origin_x) / cell_size).floor() as isize).clamp(0, cols as isize - 1) as usize;
  let r0 =
    (((aabb.min_y - origin_y) / cell_size).floor() as isize).clamp(0, rows as isize - 1) as usize;
  let c1 =
    (((aabb.max_x - origin_x) / cell_size).floor() as isize).clamp(0, cols as isize - 1) as usize;
  let r1 =
    (((aabb.max_y - origin_y) / cell_size).floor() as isize).clamp(0, rows as isize - 1) as usize;
  (c0, r0, c1, r1)
}

/// Naive query: scan all segments.
pub fn naive_query_aabb(items: &[(Aabb, SegmentRef)], query: Aabb) -> Vec<SegmentRef> {
  items
    .iter()
    .filter_map(|(aabb, seg_ref)| {
      if aabb.overlaps(query) {
        Some(*seg_ref)
      } else {
        None
      }
    })
    .collect()
}

/// Build indexed items for naive comparison.
pub fn build_items(contours: &[Vec<Segment>]) -> Vec<(Aabb, SegmentRef)> {
  let mut items = Vec::new();
  for (ci, contour) in contours.iter().enumerate() {
    for (si, segment) in contour.iter().enumerate() {
      items.push((
        Aabb::from_bounds(segment.bounds()),
        SegmentRef {
          contour: ci,
          segment: si,
        },
      ));
    }
  }
  items
}

#[cfg(test)]
mod tests {
  use super::*;
  use cam_geom::{LineSeg, Point2, Segment};

  #[test]
  fn grid_query_returns_overlapping_segments() {
    let contours = vec![vec![
      Segment::Line(LineSeg::new(Point2::new(0.0, 0.0), Point2::new(10.0, 0.0))),
      Segment::Line(LineSeg::new(
        Point2::new(10.0, 0.0),
        Point2::new(10.0, 10.0),
      )),
      Segment::Line(LineSeg::new(
        Point2::new(20.0, 20.0),
        Point2::new(30.0, 20.0),
      )),
    ]];
    let grid = SpatialGrid::build(&contours, 2.0);
    let query = Aabb {
      min_x: -1.0,
      min_y: -1.0,
      max_x: 11.0,
      max_y: 1.0,
    };
    let hits = grid.query_aabb(query);
    let all = naive_query_aabb(grid.items(), query);
    assert_eq!(hits.len(), all.len());
    assert_eq!(hits.len(), 2); // horizontal base + vertical edge at x=10
  }
}
