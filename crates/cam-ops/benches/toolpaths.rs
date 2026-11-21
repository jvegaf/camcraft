use cam_geom::primitive::rectangle;
use cam_geom::region::Region;
use cam_geom::{Contour, Point2, TolerancePolicy};
use cam_model::{Job, OriginCorner};
use cam_ops::ncc_index::{build_items, naive_query_aabb, Aabb, SpatialGrid};
use cam_ops::params::{
  DrillParams, PocketParams, PocketStrategy, ProfileParams, ProfileSide, RetractMode,
};
use cam_ops::peck::DrillHole;
use cam_ops::shared::ToolSpec;
use cam_ops::{drill, pocket, profile, DrillRequest, PocketRequest, ProfileRequest};
use cam_ops::{ncc_fill_rings, non_copper_envelope};
use cam_units::Length;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn job() -> Job {
  Job {
    width: Length::from_mm(100.0),
    height: Length::from_mm(100.0),
    thickness: Length::from_mm(10.0),
    origin: OriginCorner::BottomLeft,
    side_up: cam_model::BoardSideUp::Top,
  }
}

fn tool() -> ToolSpec {
  ToolSpec {
    number: 1,
    diameter: Length::from_mm(3.175),
    included_angle: None,
    feed_rate: 1200.0,
    plunge_rate: 300.0,
    spindle_speed: 18000,
  }
}

fn bench_profile_rectangle(c: &mut Criterion) {
  let contour = rectangle(Point2::new(10.0, 10.0), 60.0, 40.0).expect("rect");
  let params = ProfileParams {
    side: ProfileSide::Outside,
    allowance: Length::from_mm(0.0),
    pass_depth: Length::from_mm(2.0),
    start_depth: Length::from_mm(0.0),
    cut_depth: Length::from_mm(4.0),
    lead_in: Length::from_mm(0.0),
    lead_out: Length::from_mm(0.0),
    tabs: Vec::new(),
  };
  let job = job();
  let tool = tool();

  c.bench_function("ops/profile_rectangle", |bench| {
    bench.iter(|| {
      let out = profile(black_box(&ProfileRequest {
        contour: &contour,
        params: &params,
        tool: &tool,
        job: &job,
      }))
      .expect("profile");
      black_box(out.len());
    })
  });
}

fn bench_pocket_offset_rectangle(c: &mut Criterion) {
  let contour = rectangle(Point2::new(10.0, 10.0), 60.0, 40.0).expect("rect");
  let islands: Vec<&cam_geom::Contour> = Vec::new();
  let params = PocketParams {
    strategy: PocketStrategy::Offset,
    allowance: Length::from_mm(0.0),
    stepover: Length::from_mm(1.5),
    pass_depth: Length::from_mm(2.0),
    start_depth: Length::from_mm(0.0),
    cut_depth: Length::from_mm(4.0),
  };
  let job = job();
  let tool = tool();

  c.bench_function("ops/pocket_offset_rectangle", |bench| {
    bench.iter(|| {
      let out = pocket(black_box(&PocketRequest {
        contour: &contour,
        islands: &islands,
        params: &params,
        tool: &tool,
        job: &job,
      }))
      .expect("pocket");
      black_box(out.moves.len());
    })
  });
}

fn bench_drill_holes(c: &mut Criterion) {
  let holes = vec![
    DrillHole {
      entity: "h1".to_string(),
      centre: Point2::new(10.0, 10.0),
    },
    DrillHole {
      entity: "h2".to_string(),
      centre: Point2::new(30.0, 10.0),
    },
    DrillHole {
      entity: "h3".to_string(),
      centre: Point2::new(50.0, 10.0),
    },
    DrillHole {
      entity: "h4".to_string(),
      centre: Point2::new(10.0, 30.0),
    },
    DrillHole {
      entity: "h5".to_string(),
      centre: Point2::new(30.0, 30.0),
    },
    DrillHole {
      entity: "h6".to_string(),
      centre: Point2::new(50.0, 30.0),
    },
  ];
  let params = DrillParams {
    peck_depth: Length::from_mm(1.5),
    retract: RetractMode::Full,
    start_depth: Length::from_mm(0.0),
    cut_depth: Length::from_mm(6.0),
    bank: None,
  };
  let job = job();
  let tool = tool();

  c.bench_function("ops/drill_6_holes", |bench| {
    bench.iter(|| {
      let out = drill(black_box(&DrillRequest {
        holes: &holes,
        params: &params,
        tool: &tool,
        job: &job,
      }))
      .expect("drill");
      black_box(out.moves.len());
    })
  });
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Contour {
  Contour::from_points(
    &[
      Point2::new(x0, y0),
      Point2::new(x1, y0),
      Point2::new(x1, y1),
      Point2::new(x0, y1),
    ],
    true,
  )
  .unwrap()
}

fn bench_ncc_fill_rings(c: &mut Criterion) {
  let tol = TolerancePolicy::uniform(1.0e-6);
  let board = Region::new(vec![rect(0.0, 0.0, 100.0, 100.0)])
    .unwrap()
    .normalize(tol)
    .unwrap()
    .region;
  let copper = vec![
    rect(40.0, 40.0, 60.0, 60.0),
    rect(10.0, 70.0, 25.0, 85.0),
    rect(70.0, 10.0, 90.0, 30.0),
  ];
  let envelope = non_copper_envelope(&board, &copper, 0.0, 0.5, tol).unwrap();
  c.bench_function("ops/ncc_fill_rings", |bench| {
    bench.iter(|| {
      let out = ncc_fill_rings(black_box(&envelope), 2.0, 0.5, tol).unwrap();
      black_box(out.contours.len());
    })
  });
}

fn bench_ncc_index_naive_vs_grid(c: &mut Criterion) {
  let tol = TolerancePolicy::uniform(1.0e-6);
  let board = Region::new(vec![rect(0.0, 0.0, 100.0, 100.0)])
    .unwrap()
    .normalize(tol)
    .unwrap()
    .region;
  let copper = vec![
    rect(40.0, 40.0, 60.0, 60.0),
    rect(10.0, 70.0, 25.0, 85.0),
    rect(70.0, 10.0, 90.0, 30.0),
  ];
  let envelope = non_copper_envelope(&board, &copper, 0.0, 0.5, tol).unwrap();
  let fill = ncc_fill_rings(&envelope, 1.0, 0.5, tol).unwrap();
  let contours: Vec<Vec<cam_geom::Segment>> = fill.contours.iter().map(|c| c.segments()).collect();
  let items = build_items(&contours);
  let grid = SpatialGrid::build(&contours, 2.0);
  let queries: Vec<Aabb> = (0..200)
    .map(|i| {
      let x = (i as f64 * 0.73) % 100.0;
      let y = (i as f64 * 1.37) % 100.0;
      Aabb {
        min_x: x - 1.0,
        min_y: y - 1.0,
        max_x: x + 1.0,
        max_y: y + 1.0,
      }
    })
    .collect();

  c.bench_function("ops/ncc_index_naive_aabb", |bench| {
    bench.iter(|| {
      let mut total = 0usize;
      for q in &queries {
        total += naive_query_aabb(black_box(&items), *q).len();
      }
      black_box(total)
    })
  });
  c.bench_function("ops/ncc_index_grid_aabb", |bench| {
    bench.iter(|| {
      let mut total = 0usize;
      for q in &queries {
        total += grid.query_aabb(black_box(*q)).len();
      }
      black_box(total)
    })
  });
}

criterion_group!(
  toolpaths,
  bench_profile_rectangle,
  bench_pocket_offset_rectangle,
  bench_drill_holes,
  bench_ncc_fill_rings,
  bench_ncc_index_naive_vs_grid
);
criterion_main!(toolpaths);
