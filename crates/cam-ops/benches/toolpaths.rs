use cam_geom::primitive::rectangle;
use cam_geom::Point2;
use cam_model::{Job, OriginCorner};
use cam_ops::params::{
  DrillParams, PocketParams, PocketStrategy, ProfileParams, ProfileSide, RetractMode,
};
use cam_ops::peck::DrillHole;
use cam_ops::shared::ToolSpec;
use cam_ops::{drill, pocket, profile, DrillRequest, PocketRequest, ProfileRequest};
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

criterion_group!(
  toolpaths,
  bench_profile_rectangle,
  bench_pocket_offset_rectangle,
  bench_drill_holes
);
criterion_main!(toolpaths);
