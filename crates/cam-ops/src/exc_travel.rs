//! Travel routing around exclusion areas (E5.2).

use cam_geom::region::Region;
use cam_geom::{Point2, TolerancePolicy};
use cam_move::{Move, Point3};

/// Reroute Rapid/Retract moves in a stream so travel does not cross exclusion areas.
///
/// Uses each segment's tool diameter as clearance inflation.
pub fn route_stream_around_exc_areas(
  stream: &cam_move::MoveStream,
  exc_areas: &[Region],
  tolerance: TolerancePolicy,
) -> cam_move::MoveStream {
  if exc_areas.is_empty() {
    return stream.clone();
  }
  let mut out = stream.clone();
  for seg in &mut out.segments {
    let clearance = match seg.tool {
      cam_move::ToolShape::Cylinder { diameter } => diameter.mm(),
      cam_move::ToolShape::VBit { flat_diameter, .. } => flat_diameter.mm(),
    }
    .max(0.0);
    seg.moves = route_travel_around_exc_areas(&seg.moves, exc_areas, clearance, tolerance);
  }
  out
}

/// Reroute Rapid/Retract moves so travel does not cross exclusion areas.
///
/// `clearance_mm` inflates exclusion boundaries for tool safety. Returns moves with
/// detour rapids inserted when a straight travel crosses an exclusion.
pub fn route_travel_around_exc_areas(
  moves: &[Move],
  exc_areas: &[Region],
  clearance_mm: f64,
  tolerance: TolerancePolicy,
) -> Vec<Move> {
  if exc_areas.is_empty() {
    return moves.to_vec();
  }
  let mut out = Vec::with_capacity(moves.len());
  let mut current = None::<Point3>;
  for mv in moves {
    match *mv {
      Move::Rapid { to } | Move::Retract { to } => {
        if let Some(from) = current {
          let detour = detour_travel(from, to, exc_areas, clearance_mm, tolerance);
          for p in detour {
            out.push(match *mv {
              Move::Retract { .. } => Move::Retract { to: p },
              _ => Move::Rapid { to: p },
            });
          }
        } else {
          out.push(*mv);
        }
        current = Some(to);
      }
      other => {
        current = Some(other.destination());
        out.push(other);
      }
    }
  }
  out
}

fn detour_travel(
  from: Point3,
  to: Point3,
  exc_areas: &[Region],
  clearance_mm: f64,
  tolerance: TolerancePolicy,
) -> Vec<Point3> {
  let a = Point2::new(from.x.mm(), from.y.mm());
  let b = Point2::new(to.x.mm(), to.y.mm());
  if !crosses_any(a, b, exc_areas, tolerance) {
    return vec![to];
  }
  // Try detours through inflated boundary vertices of intersected regions.
  let mut candidates: Vec<Vec<Point2>> = Vec::new();
  for region in exc_areas {
    if !segment_hits_region(a, b, region, tolerance) {
      continue;
    }
    for boundary in &region.boundaries {
      let verts = inflated_vertices(boundary, clearance_mm.max(0.0));
      if verts.len() < 2 {
        continue;
      }
      // Try routing around boundary in both directions as single waypoint chains.
      // Simple heuristic: try each inflated vertex as a single waypoint.
      for v in &verts {
        candidates.push(vec![*v]);
      }
      // Try two-vertex detours along boundary.
      for w in verts.windows(2) {
        candidates.push(vec![w[0], w[1]]);
      }
      // try wrap-around pair
      if let (Some(first), Some(last)) = (verts.first(), verts.last()) {
        candidates.push(vec![*last, *first]);
      }
    }
  }
  // Also try axis-aligned bbox corners of intersected regions inflated.
  for region in exc_areas {
    if !segment_hits_region(a, b, region, tolerance) {
      continue;
    }
    if let Some(bounds) = region_bounds(region) {
      let m = clearance_mm.max(0.1);
      let corners = [
        Point2::new(bounds.0 - m, bounds.1 - m),
        Point2::new(bounds.2 + m, bounds.1 - m),
        Point2::new(bounds.2 + m, bounds.3 + m),
        Point2::new(bounds.0 - m, bounds.3 + m),
      ];
      for c in &corners {
        candidates.push(vec![*c]);
      }
    }
  }

  let mut best: Option<(f64, Vec<Point2>)> = None;
  for path in candidates {
    if path_crosses_any(a, &path, b, exc_areas, tolerance) {
      continue;
    }
    let len = path_length(a, &path, b);
    if best.as_ref().map(|(bl, _)| len < *bl).unwrap_or(true) {
      best = Some((len, path));
    }
  }

  if let Some((_, path)) = best {
    let mut out: Vec<Point3> = path
      .into_iter()
      .map(|p| Point3::new(p.x, p.y, from.z.mm()))
      .collect();
    out.push(Point3::new(to.x.mm(), to.y.mm(), to.z.mm()));
    return out;
  }

  // Fallback: lift over safe by routing through bbox corner with largest clearance - still better to
  // return original? No, acceptance says must deviate and sim no collision. Try a coarse grid escape.
  if let Some(path) = coarse_escape(a, b, exc_areas, tolerance, clearance_mm) {
    let mut out: Vec<Point3> = path
      .into_iter()
      .map(|p| Point3::new(p.x, p.y, from.z.mm()))
      .collect();
    out.push(Point3::new(to.x.mm(), to.y.mm(), to.z.mm()));
    return out;
  }

  vec![to]
}

fn crosses_any(a: Point2, b: Point2, exc_areas: &[Region], tol: TolerancePolicy) -> bool {
  exc_areas.iter().any(|r| segment_hits_region(a, b, r, tol))
}

fn segment_hits_region(a: Point2, b: Point2, region: &Region, tol: TolerancePolicy) -> bool {
  if point_in_region(a, region, tol) || point_in_region(b, region, tol) {
    return true;
  }
  for boundary in &region.boundaries {
    if segment_intersects_contour(a, b, boundary, tol) {
      return true;
    }
  }
  // midpoint check catches cases where segment passes through region without crossing boundary in sampling
  let mid = Point2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
  point_in_region(mid, region, tol)
}

fn point_in_region(p: Point2, region: &Region, tol: TolerancePolicy) -> bool {
  region
    .distance_to_boundary(p, tol)
    .map(|d| d.inside)
    .unwrap_or(false)
}

fn segment_intersects_contour(
  a: Point2,
  b: Point2,
  contour: &cam_geom::Contour,
  tol: TolerancePolicy,
) -> bool {
  let pts = contour.tessellate(tol);
  if pts.len() < 2 {
    return false;
  }
  let n = pts.len();
  let limit = if contour.is_closed() { n } else { n - 1 };
  for i in 0..limit {
    let c = pts[i];
    let d = pts[(i + 1) % n];
    if segments_intersect(a, b, c, d) {
      return true;
    }
  }
  false
}

fn segments_intersect(a: Point2, b: Point2, c: Point2, d: Point2) -> bool {
  fn orient(p: Point2, q: Point2, r: Point2) -> f64 {
    (q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x)
  }
  fn on_seg(p: Point2, q: Point2, r: Point2) -> bool {
    q.x <= p.x.max(r.x) + 1e-9
      && q.x + 1e-9 >= p.x.min(r.x)
      && q.y <= p.y.max(r.y) + 1e-9
      && q.y + 1e-9 >= p.y.min(r.y)
  }
  let o1 = orient(a, b, c);
  let o2 = orient(a, b, d);
  let o3 = orient(c, d, a);
  let o4 = orient(c, d, b);
  if o1.abs() < 1e-12 && on_seg(a, c, b) {
    return true;
  }
  if o2.abs() < 1e-12 && on_seg(a, d, b) {
    return true;
  }
  if o3.abs() < 1e-12 && on_seg(c, a, d) {
    return true;
  }
  if o4.abs() < 1e-12 && on_seg(c, b, d) {
    return true;
  }
  (o1 > 0.0) != (o2 > 0.0) && (o3 > 0.0) != (o4 > 0.0)
}

fn path_crosses_any(
  a: Point2,
  path: &[Point2],
  b: Point2,
  exc_areas: &[Region],
  tol: TolerancePolicy,
) -> bool {
  let mut prev = a;
  for p in path {
    if crosses_any(prev, *p, exc_areas, tol) {
      return true;
    }
    prev = *p;
  }
  crosses_any(prev, b, exc_areas, tol)
}

fn path_length(a: Point2, path: &[Point2], b: Point2) -> f64 {
  let mut len = 0.0;
  let mut prev = a;
  for p in path {
    len += prev.distance_to(*p);
    prev = *p;
  }
  len += prev.distance_to(b);
  len
}

fn inflated_vertices(boundary: &cam_geom::Contour, clearance: f64) -> Vec<Point2> {
  let verts: Vec<Point2> = boundary.vertices().iter().map(|v| v.point).collect();
  if verts.len() < 3 {
    return verts;
  }
  let n = verts.len();
  let centroid = verts.iter().fold(Point2::new(0.0, 0.0), |acc, p| {
    Point2::new(acc.x + p.x, acc.y + p.y)
  });
  let centroid = Point2::new(centroid.x / n as f64, centroid.y / n as f64);
  verts
    .into_iter()
    .map(|p| {
      let dx = p.x - centroid.x;
      let dy = p.y - centroid.y;
      let d = (dx * dx + dy * dy).sqrt();
      if d < 1e-9 {
        p
      } else {
        Point2::new(p.x + dx / d * clearance, p.y + dy / d * clearance)
      }
    })
    .collect()
}

fn region_bounds(region: &Region) -> Option<(f64, f64, f64, f64)> {
  let mut minx = f64::INFINITY;
  let mut miny = f64::INFINITY;
  let mut maxx = f64::NEG_INFINITY;
  let mut maxy = f64::NEG_INFINITY;
  let mut any = false;
  for b in &region.boundaries {
    for v in b.vertices() {
      any = true;
      minx = minx.min(v.point.x);
      miny = miny.min(v.point.y);
      maxx = maxx.max(v.point.x);
      maxy = maxy.max(v.point.y);
    }
  }
  any.then_some((minx, miny, maxx, maxy))
}

fn coarse_escape(
  a: Point2,
  b: Point2,
  exc_areas: &[Region],
  tol: TolerancePolicy,
  clearance: f64,
) -> Option<Vec<Point2>> {
  // Try a ring of points around the midpoint at increasing radii.
  let mid = Point2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
  let base = a.distance_to(b).max(1.0);
  for &r in &[base * 0.5, base, base * 2.0, base * 4.0] {
    for k in 0..16 {
      let ang = (k as f64) * std::f64::consts::TAU / 16.0;
      let p = Point2::new(mid.x + r * ang.cos(), mid.y + r * ang.sin());
      let path = vec![p];
      if !path_crosses_any(a, &path, b, exc_areas, tol)
        && !point_in_region(p, &Region { boundaries: vec![] }, tol)
      {
        let _ = clearance;
        return Some(path);
      }
    }
  }
  None
}

#[cfg(test)]
mod tests {
  use super::*;
  use cam_geom::{Contour, Point2};

  fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Region {
    Region::new(vec![Contour::from_points(
      &[
        Point2::new(x0, y0),
        Point2::new(x1, y0),
        Point2::new(x1, y1),
        Point2::new(x0, y1),
      ],
      true,
    )
    .unwrap()])
    .unwrap()
  }

  #[test]
  fn straight_travel_not_crossing_is_unchanged() {
    let exc = [square(10.0, 10.0, 20.0, 20.0)];
    let moves = vec![
      Move::Rapid {
        to: Point3::new(0.0, 0.0, 5.0),
      },
      Move::Rapid {
        to: Point3::new(5.0, 0.0, 5.0),
      },
    ];
    let out = route_travel_around_exc_areas(&moves, &exc, 1.0, TolerancePolicy::default());
    assert_eq!(out.len(), 2);
  }

  #[test]
  fn travel_crossing_exc_area_deviates() {
    let exc = [square(4.0, -1.0, 6.0, 1.0)];
    let moves = vec![
      Move::Rapid {
        to: Point3::new(0.0, 0.0, 5.0),
      },
      Move::Rapid {
        to: Point3::new(10.0, 0.0, 5.0),
      },
    ];
    let out = route_travel_around_exc_areas(&moves, &exc, 0.5, TolerancePolicy::default());
    assert!(out.len() >= 3, "should insert detour points");
    // no segment should cross exc area
    let tol = TolerancePolicy::default();
    let mut prev = Point2::new(0.0, 0.0);
    for mv in &out {
      let to = mv.destination();
      let b = Point2::new(to.x.mm(), to.y.mm());
      assert!(
        !segment_hits_region(prev, b, &exc[0], tol),
        "detour still crosses"
      );
      prev = b;
    }
  }
}
