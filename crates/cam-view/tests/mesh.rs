//! Behaviour tests for the 3D stock mesh (M5 A9, D-M5-7): the surface, its depth ramp, the stock box
//! and the determinism the painter relies on.

use cam_app::Viewport;
use cam_model::{Job, OriginCorner};
use cam_move::Point3;
use cam_sim::Heightfield;
use cam_units::Length;
use cam_view::{build_stock_box, build_surface, Camera3d, Line3, SurfaceMesh};

fn job(width: f64, height: f64, thickness: f64) -> Job {
  Job {
    width: Length::from_mm(width),
    height: Length::from_mm(height),
    thickness: Length::from_mm(thickness),
    origin: OriginCorner::BottomLeft,
    side_up: cam_model::BoardSideUp::Top,
  }
}

/// A flat field of `columns` x `rows` one-millimetre cells.
fn flat_field(columns: f64, rows: f64) -> Heightfield {
  Heightfield::new(&job(columns, rows, 18.0), Length::from_mm(1.0)).expect("a one millimetre grid")
}

fn viewer(target: Point3) -> Camera3d {
  Camera3d::new(target, Viewport::new(800.0, 600.0))
}

/// The smallest and largest value `pick` finds over the ends of some lines.
fn extent(ends: &[Point3], pick: fn(&Point3) -> f64) -> (f64, f64) {
  let values: Vec<f64> = ends.iter().map(pick).collect();
  (
    values.iter().copied().fold(f64::MAX, f64::min),
    values.iter().copied().fold(f64::MIN, f64::max),
  )
}

/// The start (`end == 0`) or the end of a line.
fn corner(line: &Line3, end: usize) -> Point3 {
  if end == 0 {
    line.start
  } else {
    line.end
  }
}

#[test]
fn the_surface_has_the_documented_counts_at_each_stride() {
  // A 10 x 8 mm stock in one-millimetre cells is a 10 column, 8 row grid.
  let field = flat_field(10.0, 8.0);
  let camera = viewer(Point3::new(5.0, 4.0, 0.0));

  // Stride 1 samples every cell: (columns - 1) * (rows - 1) * 2 triangles, one quad per cell pair.
  let fine = build_surface(&field, &camera, 1);
  assert_eq!(fine.vertices.len(), 80);
  assert_eq!(fine.triangles.len(), 126);

  // Stride 4 samples columns 0, 4, 8 and the far edge 9 (4 of them), rows 0, 4 and 7 (3 of them):
  // (4 - 1) * (3 - 1) * 2 = 12 triangles from 12 vertices.
  let coarse = build_surface(&field, &camera, 4);
  assert_eq!(coarse.vertices.len(), 12);
  assert_eq!(coarse.triangles.len(), 12);

  // A stride wider than the grid still spans the stock: the four corners, two triangles.
  let blocky = build_surface(&field, &camera, 100);
  assert_eq!(blocky.vertices.len(), 4);
  assert_eq!(blocky.triangles.len(), 2);

  for mesh in [&fine, &coarse, &blocky] {
    for triangle in &mesh.triangles {
      for index in triangle {
        assert!(*index < mesh.vertices.len(), "index {index} out of range");
      }
    }
  }
}

#[test]
fn a_zero_stride_is_treated_as_one() {
  // `stride` is a decimation factor, not a divisor: zero must not panic or empty the mesh.
  let field = flat_field(4.0, 3.0);
  let camera = viewer(Point3::new(2.0, 1.5, 0.0));
  assert_eq!(
    build_surface(&field, &camera, 0),
    build_surface(&field, &camera, 1)
  );
}

#[test]
fn the_mesh_sits_on_the_simulated_surface() {
  // A flat field is untouched stock: every vertex is at the stock top, depth 0.
  let field = flat_field(6.0, 6.0);
  let camera = viewer(Point3::new(3.0, 3.0, 0.0));
  let mesh = build_surface(&field, &camera, 1);
  for vertex in &mesh.vertices {
    assert_eq!(vertex.position.z.mm(), 0.0);
    assert_eq!(vertex.depth, 0.0);
  }
  // Vertices are cell centres: the first one is the centre of cell (0, 0).
  assert_eq!(mesh.vertices[0].position.x.mm(), 0.5);
  assert_eq!(mesh.vertices[0].position.y.mm(), 0.5);
}

#[test]
fn the_depth_ramp_maps_the_stock_bottom_and_top_to_the_ends_of_the_palette() {
  let mut field = flat_field(6.0, 6.0);
  // One cell cut through to the bottom, one cut a little way in, the rest untouched.
  field.lower(0, 0, -100.0);
  field.lower(3, 3, -2.0);
  let camera = viewer(Point3::new(3.0, 3.0, 0.0));
  let mesh = build_surface(&field, &camera, 1);

  let bottom = mesh.vertices[0];
  assert_eq!(field.surface(0, 0), -18.0, "lower() clamps at the bottom");
  assert_eq!(bottom.position.z.mm(), -18.0);
  assert_eq!(bottom.depth, 1.0, "the stock bottom is the far end");

  let untouched = mesh.vertices[1];
  assert_eq!(untouched.position.z.mm(), 0.0);
  assert_eq!(untouched.depth, 0.0, "the stock top is the near end");

  // Row 3, column 3 of a six column grid.
  let shallow = mesh.vertices[3 * 6 + 3];
  assert_eq!(shallow.position.z.mm(), -2.0);
  assert_eq!(shallow.depth, (2.0f64 / 18.0) as f32);

  for vertex in &mesh.vertices {
    assert!(
      (0.0..=1.0).contains(&vertex.depth),
      "depth {} is outside the palette",
      vertex.depth
    );
  }
}

#[test]
fn two_builds_of_the_same_input_are_identical() {
  // Determinism (S9): no `HashMap`, no clock, no ambient state — the same field, camera and stride
  // must give the same mesh, vertex for vertex.
  let mut field = flat_field(8.0, 6.0);
  field.lower(2, 2, -5.0);
  field.lower(5, 3, -12.0);
  let camera = viewer(Point3::new(4.0, 3.0, 0.0));

  let first = build_surface(&field, &camera, 2);
  let second = build_surface(&field, &camera, 2);
  assert_eq!(first, second);

  // The vertices are camera free: another viewpoint reorders the triangles for the painter, but the
  // geometry it paints is the same list.
  let mut elsewhere = camera;
  elsewhere.orbit(1.0, 0.2);
  let other = build_surface(&field, &elsewhere, 2);
  assert_eq!(first.vertices, other.vertices);
}

#[test]
fn triangles_are_ordered_back_to_front_for_the_painter() {
  // `egui::Mesh` has no depth buffer, so the mesh is handed over farthest triangle first and the
  // camera is what decides that order.
  let field = flat_field(8.0, 6.0);
  let camera = viewer(Point3::new(4.0, 3.0, 0.0));
  let mesh: SurfaceMesh = build_surface(&field, &camera, 1);

  let triangle_depth = |triangle: &[usize; 3]| {
    triangle
      .iter()
      .map(|index| {
        camera
          .project(mesh.vertices[*index].position)
          .expect("the whole stock is in front of the camera")
          .depth
      })
      .sum::<f64>()
      / 3.0
  };

  let depths: Vec<f64> = mesh.triangles.iter().map(triangle_depth).collect();
  for pair in depths.windows(2) {
    assert!(
      pair[0] <= pair[1] + 1.0e-9,
      "triangles are out of paint order: {} after {}",
      pair[1],
      pair[0]
    );
  }
}

#[test]
fn the_stock_box_has_the_edges_a_box_has() {
  let stock = job(100.0, 60.0, 18.0);
  let edges = build_stock_box(&stock);
  assert_eq!(edges.len(), 12, "a box has twelve edges");

  // Eight corners, each the end of exactly three edges: no duplicate edge, no missing one.
  let corners = [
    (0.0, 0.0, -18.0),
    (100.0, 0.0, -18.0),
    (100.0, 60.0, -18.0),
    (0.0, 60.0, -18.0),
    (0.0, 0.0, 0.0),
    (100.0, 0.0, 0.0),
    (100.0, 60.0, 0.0),
    (0.0, 60.0, 0.0),
  ];
  for (x, y, z) in corners {
    let touching = edges
      .iter()
      .flat_map(|edge| [corner(edge, 0), corner(edge, 1)])
      .filter(|point| point.x.mm() == x && point.y.mm() == y && point.z.mm() == z)
      .count();
    assert_eq!(
      touching, 3,
      "corner ({x}, {y}, {z}) touches {touching} edges"
    );
  }

  // Every edge is axis aligned, so the box reads as a box and not as a scatter of diagonals.
  for edge in &edges {
    let differing = [
      (edge.start.x.mm(), edge.end.x.mm()),
      (edge.start.y.mm(), edge.end.y.mm()),
      (edge.start.z.mm(), edge.end.z.mm()),
    ]
    .into_iter()
    .filter(|(start, end)| start != end)
    .count();
    assert_eq!(differing, 1, "an edge bent in {differing} axes");
  }

  // The stock spans its own extents: x 0..width, y 0..height, z -thickness..0 (S13).
  let ends: Vec<Point3> = edges
    .iter()
    .flat_map(|edge| [corner(edge, 0), corner(edge, 1)])
    .collect();
  assert_eq!(extent(&ends, |point| point.x.mm()), (0.0, 100.0));
  assert_eq!(extent(&ends, |point| point.y.mm()), (0.0, 60.0));
  assert_eq!(extent(&ends, |point| point.z.mm()), (-18.0, 0.0));
}
