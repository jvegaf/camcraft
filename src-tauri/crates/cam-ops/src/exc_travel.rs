//! Travel routing around exclusion areas (E5.2).

use cam_geom::region::Region;
use cam_geom::{Point2, TolerancePolicy};
use cam_move::{Move, Point3};

use crate::ToolpathError;

/// Reroute Rapid/Retract moves in a stream so travel does not cross exclusion areas.
///
/// Uses each segment's tool diameter as clearance inflation. Carries position across segments from
/// the stream home so first moves and segment boundaries are checked (S2).
pub fn route_stream_around_exc_areas(
  stream: &cam_move::MoveStream,
  exc_areas: &[Region],
  tolerance: TolerancePolicy,
) -> Result<cam_move::MoveStream, ToolpathError> {
  if exc_areas.is_empty() {
    return Ok(stream.clone());
  }
  let mut out = stream.clone();
  let mut current = Point3::new(stream.home.x, stream.home.y, stream.safe_z.mm());
  for seg in &mut out.segments {
    let clearance = match seg.tool {
      cam_move::ToolShape::Cylinder { diameter } => diameter.mm(),
      cam_move::ToolShape::VBit { flat_diameter, .. } => flat_diameter.mm(),
    }
    .max(0.0);
    let (moves, end) = route_travel_around_exc_areas_with_tool(
      &seg.moves,
      current,
      exc_areas,
      clearance,
      Some(seg.tool),
      tolerance,
    )?;
    seg.moves = moves;
    current = end;
  }
  Ok(out)
}

/// Reroute Rapid/Retract moves so travel does not cross exclusion areas.
///
/// `clearance_mm` is the tool diameter; detection uses half of it as radial clearance. Returns
/// routed moves and the final position. Returns error if no safe detour exists (S2).
pub fn route_travel_around_exc_areas(
  moves: &[Move],
  start: Point3,
  exc_areas: &[Region],
  clearance_mm: f64,
  tolerance: TolerancePolicy,
) -> Result<(Vec<Move>, Point3), ToolpathError> {
  route_travel_around_exc_areas_with_tool(moves, start, exc_areas, clearance_mm, None, tolerance)
}

/// Like `route_travel_around_exc_areas` but computes VBit clearance at travel Z.
pub fn route_travel_around_exc_areas_with_tool(
  moves: &[Move],
  start: Point3,
  exc_areas: &[Region],
  clearance_mm: f64,
  tool: Option<cam_move::ToolShape>,
  tolerance: TolerancePolicy,
) -> Result<(Vec<Move>, Point3), ToolpathError> {
  if exc_areas.is_empty() {
    let end = moves.last().map(|m| m.destination()).unwrap_or(start);
    return Ok((moves.to_vec(), end));
  }
  let mut out = Vec::with_capacity(moves.len());
  let mut current = start;
  for mv in moves {
    match *mv {
      Move::Rapid { to } | Move::Retract { to } => {
        let radius = effective_radius(tool, clearance_mm, current.z.mm(), to.z.mm());
        let detour = detour_travel(current, to, exc_areas, radius, tolerance)?;
        for p in detour {
          out.push(match *mv {
            Move::Retract { .. } => Move::Retract { to: p },
            _ => Move::Rapid { to: p },
          });
        }
        current = to;
      }
      other => {
        current = other.destination();
        out.push(other);
      }
    }
  }
  Ok((out, current))
}

fn effective_radius(tool: Option<cam_move::ToolShape>, clearance_mm: f64, z0: f64, z1: f64) -> f64 {
  let base = (clearance_mm / 2.0).max(0.0);
  match tool {
    Some(t @ cam_move::ToolShape::VBit { .. }) => {
      let z = z0.max(z1).max(0.0);
      (t.radius_at(z) * 2.0 / 2.0).max(base)
    }
    _ => base,
  }
}

fn detour_travel(
  from: Point3,
  to: Point3,
  exc_areas: &[Region],
  radius: f64,
  tolerance: TolerancePolicy,
) -> Result<Vec<Point3>, ToolpathError> {
  let a = Point2::new(from.x.mm(), from.y.mm());
  let b = Point2::new(to.x.mm(), to.y.mm());
  if !crosses_any(a, b, exc_areas, radius, tolerance) {
    return Ok(vec![to]);
  }
  let mut candidates: Vec<Vec<Point2>> = Vec::new();
  for region in exc_areas {
    if !segment_hits_region(a, b, region, radius, tolerance) {
      continue;
    }
    for boundary in &region.boundaries {
      let verts = inflated_vertices(boundary, radius.max(0.1) + 0.5);
      if verts.len() < 2 {
        continue;
      }
      for v in &verts {
        candidates.push(vec![*v]);
      }
      for w in verts.windows(2) {
        candidates.push(vec![w[0], w[1]]);
      }
      if let (Some(first), Some(last)) = (verts.first(), verts.last()) {
        candidates.push(vec![*last, *first]);
      }
    }
    if let Some(bounds) = region_bounds(region) {
      let m = radius.max(0.1) + 1.0;
      for c in [
        Point2::new(bounds.0 - m, bounds.1 - m),
        Point2::new(bounds.2 + m, bounds.1 - m),
        Point2::new(bounds.2 + m, bounds.3 + m),
        Point2::new(bounds.0 - m, bounds.3 + m),
      ] {
        candidates.push(vec![c]);
      }
    }
  }

  let mut best: Option<(f64, Vec<Point2>)> = None;
  for path in candidates {
    if path_crosses_any(a, &path, b, exc_areas, radius, tolerance) {
      continue;
    }
    let len = path_length(a, &path, b);
    if best.as_ref().map(|(bl, _)| len < *bl).unwrap_or(true) {
      best = Some((len, path));
    }
  }
  let chosen = best
    .map(|(_, p)| p)
    .or_else(|| coarse_escape(a, b, exc_areas, tolerance, radius));
  if let Some(path) = chosen {
    let mut out: Vec<Point3> = path
      .into_iter()
      .map(|p| Point3::new(p.x, p.y, from.z.mm()))
      .collect();
    out.push(Point3::new(to.x.mm(), to.y.mm(), to.z.mm()));
    return Ok(out);
  }

  Err(ToolpathError::UnroutableTravel {
    from_x: a.x,
    from_y: a.y,
    to_x: b.x,
    to_y: b.y,
    clearance_mm: radius * 2.0,
  })
}

fn crosses_any(
  a: Point2,
  b: Point2,
  exc_areas: &[Region],
  radius: f64,
  tol: TolerancePolicy,
) -> bool {
  exc_areas
    .iter()
    .any(|r| segment_hits_region(a, b, r, radius, tol))
}

fn segment_hits_region(
  a: Point2,
  b: Point2,
  region: &Region,
  radius: f64,
  tol: TolerancePolicy,
) -> bool {
  if point_too_close(a, region, radius, tol) || point_too_close(b, region, radius, tol) {
    return true;
  }
  for boundary in &region.boundaries {
    if segment_intersects_contour(a, b, boundary, tol) {
      return true;
    }
  }
  let mid = Point2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
  if point_too_close(mid, region, radius, tol) {
    return true;
  }
  let len = a.distance_to(b);
  let step = (radius * 0.5).max(0.05);
  let steps = ((len / step).ceil() as usize).max(2);
  for i in 1..steps {
    let t = i as f64 / steps as f64;
    let p = Point2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
    if point_too_close(p, region, radius, tol) {
      return true;
    }
  }
  false
}

fn point_too_close(p: Point2, region: &Region, radius: f64, tol: TolerancePolicy) -> bool {
  region
    .distance_to_boundary(p, tol)
    .map(|d| d.inside || d.distance < radius)
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
  radius: f64,
  tol: TolerancePolicy,
) -> bool {
  let mut prev = a;
  for p in path {
    if crosses_any(prev, *p, exc_areas, radius, tol) {
      return true;
    }
    prev = *p;
  }
  crosses_any(prev, b, exc_areas, radius, tol)
}

fn path_length(a: Point2, path: &[Point2], b: Point2) -> f64 {
  let mut len = 0.0;
  let mut prev = a;
  for p in path {
    len += prev.distance_to(*p);
    prev = *p;
  }
  len + prev.distance_to(b)
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
  radius: f64,
) -> Option<Vec<Point2>> {
  let mid = Point2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
  let base = a.distance_to(b).max(1.0);
  for &r in &[base * 0.5, base, base * 2.0, base * 4.0, base * 8.0] {
    for k in 0..24 {
      let ang = (k as f64) * std::f64::consts::TAU / 24.0;
      let p = Point2::new(mid.x + r * ang.cos(), mid.y + r * ang.sin());
      let path = vec![p];
      if !path_crosses_any(a, &path, b, exc_areas, radius, tol) {
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
  use cam_move::{Move, MoveStream, Segment, ToolShape};
  use cam_units::Length;

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
    let (out, _) = route_travel_around_exc_areas(
      &moves,
      Point3::new(0.0, 0.0, 5.0),
      &exc,
      1.0,
      TolerancePolicy::default(),
    )
    .unwrap();
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
    let (out, _) = route_travel_around_exc_areas(
      &moves,
      Point3::new(0.0, 0.0, 5.0),
      &exc,
      1.0,
      TolerancePolicy::default(),
    )
    .unwrap();
    assert!(out.len() >= 3, "should insert detour points");
    let tol = TolerancePolicy::default();
    let mut prev = Point2::new(0.0, 0.0);
    for mv in &out {
      let to = mv.destination();
      let b = Point2::new(to.x.mm(), to.y.mm());
      assert!(
        !segment_hits_region(prev, b, &exc[0], 0.5, tol),
        "detour still crosses"
      );
      prev = b;
    }
  }

  #[test]
  fn first_move_from_known_start_is_routed() {
    let exc = [square(4.0, -1.0, 6.0, 1.0)];
    let moves = vec![Move::Rapid {
      to: Point3::new(10.0, 0.0, 5.0),
    }];
    let (out, _) = route_travel_around_exc_areas(
      &moves,
      Point3::new(0.0, 0.0, 5.0),
      &exc,
      1.0,
      TolerancePolicy::default(),
    )
    .unwrap();
    assert!(out.len() >= 2);
  }

  #[test]
  fn unroutable_travel_returns_error() {
    // endpoint inside exclusion -> cannot be collision-free
    let exc = [square(9.0, -1.0, 11.0, 1.0)];
    let moves = vec![Move::Rapid {
      to: Point3::new(10.0, 0.0, 5.0),
    }];
    let err = route_travel_around_exc_areas(
      &moves,
      Point3::new(0.0, 0.0, 5.0),
      &exc,
      1.0,
      TolerancePolicy::default(),
    )
    .unwrap_err();
    assert!(matches!(err, ToolpathError::UnroutableTravel { .. }));
  }

  #[test]
  fn stream_routing_carries_position_across_segments() {
    let exc = [square(4.0, -1.0, 6.0, 1.0)];
    let mut stream = MoveStream::new(Length::from_mm(5.0), Point2::new(0.0, 0.0));
    stream.segments.push(Segment {
      name: "a".to_owned(),
      tool_number: 1,
      tool: ToolShape::cylinder(Length::from_mm(1.0)),
      feed_rate: 100.0,
      plunge_rate: 30.0,
      spindle_speed: 1000,
      moves: vec![Move::Rapid {
        to: Point3::new(0.0, 0.0, 5.0),
      }],
    });
    stream.segments.push(Segment {
      name: "b".to_owned(),
      tool_number: 1,
      tool: ToolShape::cylinder(Length::from_mm(1.0)),
      feed_rate: 100.0,
      plunge_rate: 30.0,
      spindle_speed: 1000,
      moves: vec![Move::Rapid {
        to: Point3::new(10.0, 0.0, 5.0),
      }],
    });
    let routed = route_stream_around_exc_areas(&stream, &exc, TolerancePolicy::default()).unwrap();
    let tol = TolerancePolicy::default();
    let mut prev = Point2::new(0.0, 0.0);
    for seg in &routed.segments {
      for mv in &seg.moves {
        let to = mv.destination();
        let b = Point2::new(to.x.mm(), to.y.mm());
        assert!(!segment_hits_region(prev, b, &exc[0], 0.5, tol));
        prev = b;
      }
    }
  }
}
