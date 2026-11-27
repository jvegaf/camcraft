//! Picking, box selection and snapping.
//!
//! All of it is measured in **screen pixels**, because that is the space the user aims in: what
//! matters is whether the cursor is close enough to a line to have meant it, and that tolerance must
//! not change when the user zooms. Every decision here is a distance comparison, which is why it can
//! be tested without a window.

use cam_app::{Camera, ScreenPoint};
use cam_geom::{Bounds, Point2};

use crate::{Scene, SceneItem, Style};

/// What a click landed on.
#[derive(Clone, Debug, PartialEq)]
pub struct Pick {
  /// The entity that was hit.
  pub entity: String,
  /// How far the cursor was from it, in pixels.
  pub distance_px: f64,
}

/// The entity nearest to `at` within `tolerance_px`, measured in pixels.
///
/// Ties are broken by entity id so a click between two coincident edges always picks the same one.
pub fn pick(scene: &Scene, camera: &Camera, at: ScreenPoint, tolerance_px: f64) -> Option<Pick> {
  let world = camera.screen_to_world(at);
  let pixels_per_mm = camera.pixels_per_mm();
  let tolerance_mm = tolerance_px / pixels_per_mm;

  let mut best: Option<Pick> = None;
  for item in &scene.items {
    if item.style() == Style::Grid {
      continue;
    }
    let Some(entity) = item.entity() else {
      continue;
    };
    let Some(segment) = item.as_segment() else {
      continue;
    };
    let distance_mm = segment.distance_to_point(world);
    if distance_mm > tolerance_mm {
      continue;
    }
    let candidate = Pick {
      entity: entity.to_owned(),
      distance_px: distance_mm * pixels_per_mm,
    };
    best = match best {
      None => Some(candidate),
      Some(current) => {
        let closer = candidate.distance_px < current.distance_px - 1.0e-9;
        let tie = (candidate.distance_px - current.distance_px).abs() <= 1.0e-9
          && candidate.entity < current.entity;
        if closer || tie {
          Some(candidate)
        } else {
          Some(current)
        }
      }
    };
  }
  best
}

/// Entities with any geometry intersecting the screen-space rectangle between `from` and `to`.
///
/// Intersecting, not merely contained: users expect a box that crosses a line to catch it.
pub fn box_select(
  scene: &Scene,
  camera: &Camera,
  from: ScreenPoint,
  to: ScreenPoint,
) -> Vec<String> {
  let a = camera.screen_to_world(from);
  let b = camera.screen_to_world(to);
  let box_world = Bounds::from_points(&[a, b]);
  let mut hits: Vec<String> = Vec::new();

  for item in &scene.items {
    let Some(entity) = item.entity() else {
      continue;
    };
    if item.style() == Style::Grid {
      continue;
    }
    if hits.iter().any(|hit| hit == entity) {
      continue;
    }
    if item_intersects(item, box_world) {
      hits.push(entity.to_owned());
    }
  }
  hits.sort();
  hits
}

/// True when an item touches a world-space box, sampled at a tolerance fine enough for selection.
fn item_intersects(item: &SceneItem, box_world: Bounds) -> bool {
  let Some(segment) = item.as_segment() else {
    return false;
  };
  let mut points = Vec::new();
  points.push(segment.start_point());
  segment.tessellate_into(
    cam_geom::TolerancePolicy::standard().with_chord(0.05),
    &mut points,
  );

  if points
    .iter()
    .any(|point| box_world.contains(*point, cam_geom::TolerancePolicy::uniform(0.0)))
  {
    return true;
  }
  // Any crossing of the box edges counts too.
  let corners = [
    box_world.min,
    Point2::new(box_world.max.x, box_world.min.y),
    box_world.max,
    Point2::new(box_world.min.x, box_world.max.y),
  ];
  for index in 0..points.len().saturating_sub(1) {
    let (start, end) = (points[index], points[index + 1]);
    for corner in 0..4 {
      if segments_cross(start, end, corners[corner], corners[(corner + 1) % 4]) {
        return true;
      }
    }
  }
  false
}

/// True when two finite segments intersect.
fn segments_cross(a1: Point2, a2: Point2, b1: Point2, b2: Point2) -> bool {
  let r = a2 - a1;
  let s = b2 - b1;
  let denominator = r.cross(s);
  if denominator.abs() < 1.0e-15 {
    return false;
  }
  let offset = b1 - a1;
  let t = offset.cross(s) / denominator;
  let u = offset.cross(r) / denominator;
  (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)
}

/// The kinds of point the cursor can snap to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SnapKind {
  /// Ends of a segment.
  Endpoint,
  /// Middle of a segment.
  Midpoint,
  /// Centre of an arc.
  Center,
  /// Intersection of the grid lines.
  Grid,
}

/// A point the cursor can snap to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snap {
  /// What kind of point it is.
  pub kind: SnapKind,
  /// Where it is, in world millimetres.
  pub point: Point2,
}

/// Every enabled snap point within `tolerance_px` of the cursor, nearest first.
///
/// `grid_spacing` is the current grid step in millimetres, or zero to disable the grid candidate.
pub fn snap_candidates(
  scene: &Scene,
  camera: &Camera,
  at: ScreenPoint,
  tolerance_px: f64,
  kinds: &[SnapKind],
  grid_spacing: f64,
) -> Vec<Snap> {
  let world = camera.screen_to_world(at);
  let pixels_per_mm = camera.pixels_per_mm();
  let tolerance_mm = tolerance_px / pixels_per_mm;
  let enabled = |kind: SnapKind| kinds.contains(&kind);
  let mut candidates: Vec<Snap> = Vec::new();

  for item in &scene.items {
    if item.style() == Style::Grid {
      continue;
    }
    match item {
      SceneItem::Polyline { points, .. } => {
        if let (Some(first), Some(second)) = (points.first(), points.get(1)) {
          if enabled(SnapKind::Endpoint) {
            for point in [*first, *second] {
              if point.distance_to(world) <= tolerance_mm {
                candidates.push(Snap {
                  kind: SnapKind::Endpoint,
                  point,
                });
              }
            }
          }
          if enabled(SnapKind::Midpoint) {
            let middle = first.lerp(*second, 0.5);
            if middle.distance_to(world) <= tolerance_mm {
              candidates.push(Snap {
                kind: SnapKind::Midpoint,
                point: middle,
              });
            }
          }
        }
      }
      SceneItem::Arc {
        center,
        radius,
        start_angle,
        sweep,
        ..
      } => {
        if let Ok(arc) = cam_geom::Arc::new(*center, *radius, *start_angle, *sweep) {
          if enabled(SnapKind::Endpoint) {
            for point in [arc.start_point(), arc.end_point()] {
              if point.distance_to(world) <= tolerance_mm {
                candidates.push(Snap {
                  kind: SnapKind::Endpoint,
                  point,
                });
              }
            }
          }
          if enabled(SnapKind::Midpoint) {
            let middle = arc.mid_point();
            if middle.distance_to(world) <= tolerance_mm {
              candidates.push(Snap {
                kind: SnapKind::Midpoint,
                point: middle,
              });
            }
          }
          if enabled(SnapKind::Center) && center.distance_to(world) <= tolerance_mm {
            candidates.push(Snap {
              kind: SnapKind::Center,
              point: *center,
            });
          }
        }
      }
    }
  }

  if enabled(SnapKind::Grid) && grid_spacing > 0.0 && grid_spacing.is_finite() {
    let snapped = Point2::new(
      (world.x / grid_spacing).round() * grid_spacing,
      (world.y / grid_spacing).round() * grid_spacing,
    );
    if snapped.distance_to(world) <= tolerance_mm {
      candidates.push(Snap {
        kind: SnapKind::Grid,
        point: snapped,
      });
    }
  }

  candidates.sort_by(|a, b| {
    let distance_a = a.point.distance_to(world);
    let distance_b = b.point.distance_to(world);
    distance_a
      .partial_cmp(&distance_b)
      .unwrap_or(std::cmp::Ordering::Equal)
      // Geometry beats the grid at equal distance, and the kind order decides the rest.
      .then_with(|| a.kind.cmp(&b.kind))
  });
  candidates.dedup_by(|a, b| {
    a.point
      .is_near(b.point, cam_geom::TolerancePolicy::uniform(1.0e-9))
  });
  candidates
}

/// The nearest snap point, if any is within tolerance.
pub fn choose_snap(
  scene: &Scene,
  camera: &Camera,
  at: ScreenPoint,
  tolerance_px: f64,
  kinds: &[SnapKind],
  grid_spacing: f64,
) -> Option<Snap> {
  snap_candidates(scene, camera, at, tolerance_px, kinds, grid_spacing)
    .into_iter()
    .next()
}

/// The default pick tolerance in pixels: three pixels is about a fingertip on a mouse.
pub const PICK_TOLERANCE_PX: f64 = 6.0;

/// The default snap tolerance in pixels.
pub const SNAP_TOLERANCE_PX: f64 = 10.0;
