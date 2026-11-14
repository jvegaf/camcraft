use cam_geom::boolean::{combine, BooleanOp, Combined};
use cam_geom::offset::{offset_contour, OffsetOptions};
use cam_geom::primitive::{circle, rectangle};
use cam_geom::region::Region;
use cam_geom::Point2;
use cam_geom::TolerancePolicy;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_combine_union(c: &mut Criterion) {
  let a = rectangle(Point2::new(0.0, 0.0), 20.0, 10.0).expect("rect a");
  let b = rectangle(Point2::new(8.0, 2.0), 20.0, 10.0).expect("rect b");
  let ra = Region::new(vec![a]).expect("region a");
  let rb = Region::new(vec![b]).expect("region b");

  c.bench_function("geom/combine_union_rect_overlap", |bench| {
    bench.iter(|| {
      let out = combine(
        black_box(&ra),
        black_box(&rb),
        BooleanOp::Union,
        TolerancePolicy::uniform(1.0e-6),
      )
      .expect("union");
      black_box(match out {
        Combined::Empty => 0usize,
        Combined::Region { region, .. } => region.boundaries.len(),
      });
    })
  });
}

fn bench_offset_circle(c: &mut Criterion) {
  let contour = circle(Point2::new(0.0, 0.0), 10.0).expect("circle");
  let options = OffsetOptions::standard();

  c.bench_function("geom/offset_circle", |bench| {
    bench.iter(|| {
      let out = offset_contour(black_box(&contour), 1.0, &options).expect("offset");
      black_box(out.contour.vertices().len());
    })
  });
}

criterion_group!(kernel, bench_combine_union, bench_offset_circle);
criterion_main!(kernel);
