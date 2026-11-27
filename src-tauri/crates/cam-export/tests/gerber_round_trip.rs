//! Gerber round-trip: export contours as RS-274X, parse with `cam-gerber`, image, and compare
//! geometry within tolerance (E5.5 acceptance).

use cam_export::{export_bytes, gerber, Format};
use cam_geom::{Contour, Point2, TolerancePolicy, Vertex};
use cam_model::{Document, Entity, EntityKind, Job, Layer, LayerRole, Provenance, Side};

const TOL: TolerancePolicy = TolerancePolicy::uniform(1.0e-3);

fn document_with_contours(contours: Vec<Contour>) -> Document {
  let mut document = Document::new(Job::default_sheet());
  document.layers.push(Layer {
    id: "copper".to_string(),
    role: LayerRole::Outline,
    side: Side::Top,
    color: "#000".to_string(),
    visible: true,
    source: Provenance::default(),
    entities: contours
      .into_iter()
      .enumerate()
      .map(|(i, contour)| Entity {
        id: format!("e{i}"),
        kind: EntityKind::Contour { contour },
        source: Provenance::default(),
      })
      .collect(),
  });
  document
}

fn sample(contours: &[Contour]) -> Vec<Vec<Point2>> {
  contours
    .iter()
    .map(|c| c.tessellate(TOL).to_vec())
    .collect()
}

fn framed(contours: &[Contour]) -> Vec<Vec<Point2>> {
  let sampled = sample(contours);
  let mut min = Point2::new(f64::INFINITY, f64::INFINITY);
  for pts in &sampled {
    for p in pts {
      min = Point2::new(min.x.min(p.x), min.y.min(p.y));
    }
  }
  sampled
    .into_iter()
    .map(|pts| {
      pts
        .into_iter()
        .map(|p| Point2::new(p.x - min.x, p.y - min.y))
        .collect()
    })
    .collect()
}

fn nearest(from: Point2, to: &[Point2]) -> f64 {
  if to.is_empty() {
    return f64::INFINITY;
  }
  let mut best = f64::INFINITY;
  for i in 0..to.len() {
    let a = to[i];
    let b = to[(i + 1) % to.len()];
    let abx = b.x - a.x;
    let aby = b.y - a.y;
    let len2 = abx * abx + aby * aby;
    let t = if len2 <= 0.0 {
      0.0
    } else {
      (((from.x - a.x) * abx + (from.y - a.y) * aby) / len2).clamp(0.0, 1.0)
    };
    let px = a.x + t * abx;
    let py = a.y + t * aby;
    let d = ((from.x - px).powi(2) + (from.y - py).powi(2)).sqrt();
    if d < best {
      best = d;
    }
  }
  best
}

fn assert_same_shape(written: &[Contour], read: &[Contour], what: &str) {
  assert_eq!(written.len(), read.len(), "{what}: contour count");
  let w = framed(written);
  let r = framed(read);
  for (i, (wp, rp)) in w.iter().zip(r.iter()).enumerate() {
    for p in wp {
      let d = nearest(*p, rp);
      assert!(
        d <= TOL.epsilon * 2.0,
        "{what}/{i}: written point {p:?} is {d:.4}mm from read shape"
      );
    }
    for p in rp {
      let d = nearest(*p, wp);
      assert!(
        d <= TOL.epsilon * 2.0,
        "{what}/{i}: read point {p:?} is {d:.4}mm from written shape"
      );
    }
  }
}

fn round_trip_contours(contours: &[Contour]) -> Vec<Contour> {
  let bytes = gerber::export_contours(contours).expect("export");
  let text = String::from_utf8(bytes).expect("utf8");
  let parsed = cam_gerber::parse(&text).expect("parse");
  let (region, _skipped) = parsed.image_reporting(TOL).expect("image");
  region.boundaries
}

#[test]
fn rectangle_round_trips_through_gerber() {
  let rect = cam_geom::primitive::rectangle(Point2::new(0.0, 0.0), 40.0, 25.0).expect("rect");
  let read = round_trip_contours(std::slice::from_ref(&rect));
  assert_same_shape(&[rect], &read, "rectangle");
}

#[test]
fn circle_round_trips_through_gerber() {
  let circle = cam_geom::primitive::circle(Point2::new(10.0, 10.0), 5.0).expect("circle");
  let read = round_trip_contours(std::slice::from_ref(&circle));
  assert_same_shape(&[circle], &read, "circle");
}

#[test]
fn contour_with_arc_round_trips_through_gerber() {
  let contour = Contour::new(
    vec![
      Vertex::new(Point2::new(0.0, 0.0), 0.0),
      Vertex::new(
        Point2::new(20.0, 0.0),
        (std::f64::consts::FRAC_PI_2 / 4.0).tan(),
      ),
      Vertex::new(Point2::new(20.0, 20.0), 0.0),
    ],
    true,
  )
  .expect("contour");
  let read = round_trip_contours(std::slice::from_ref(&contour));
  assert_same_shape(&[contour], &read, "mixed arc contour");
}

#[test]
fn document_export_gerber_round_trips() {
  let rect = cam_geom::primitive::rectangle(Point2::new(0.0, 0.0), 30.0, 20.0).expect("rect");
  let circle = cam_geom::primitive::circle(Point2::new(50.0, 10.0), 8.0).expect("circle");
  let doc = document_with_contours(vec![rect.clone(), circle.clone()]);
  let bytes = export_bytes(&doc, Format::Gerber).expect("export");
  let text = String::from_utf8(bytes).expect("utf8");
  let parsed = cam_gerber::parse(&text).expect("parse");
  let region = parsed.image(TOL).expect("image");
  assert_same_shape(&[rect, circle], &region.boundaries, "document gerber");
}
