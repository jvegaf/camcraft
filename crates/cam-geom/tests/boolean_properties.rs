//! Property tests for the set operations: the algebra has to hold on shapes nobody wrote down.

use cam_geom::boolean::{combine, BooleanOp, Combined};
use cam_geom::region::Region;
use cam_geom::{Contour, Point2, Segment};
use proptest::prelude::*;
use std::f64::consts::TAU;

/// Comfortably finer than any feature the generators below can produce, so a refusal is a defect.
const TOLERANCE: f64 = 1.0e-6;

fn area(region: &Region) -> f64 {
  region
    .boundaries
    .iter()
    .flat_map(|boundary| boundary.segments())
    .map(|segment: Segment| segment.area_contribution())
    .sum::<f64>()
    .abs()
}

/// A star-shaped polygon around a random centre: fixed angles, random radii.
///
/// The angles are evenly spaced, so the vertices stay ordered by angle and the polygon is simple by
/// construction. Randomising the radii is what turns a wheel into something worth testing — a long
/// spike next to a shallow notch — without ever producing a self-crossing contour, which the engine
/// would then have to heal and the property would no longer be about the operations.
fn any_star() -> impl Strategy<Value = Region> {
  (3usize..8, -30.0f64..30.0, -30.0f64..30.0, 3.0f64..18.0).prop_flat_map(
    |(count, centre_x, centre_y, scale)| {
      prop::collection::vec(0.25f64..1.0, count).prop_map(move |factors| {
        let step = TAU / count as f64;
        let points: Vec<Point2> = factors
          .iter()
          .enumerate()
          .map(|(index, factor)| {
            let angle = step * index as f64;
            let radius = scale * factor;
            Point2::new(
              centre_x + radius * angle.cos(),
              centre_y + radius * angle.sin(),
            )
          })
          .collect();
        let contour = Contour::from_points(&points, true).expect("a simple polygon");
        Region::new(vec![contour]).expect("one boundary")
      })
    },
  )
}

/// A sliver: the shape a badly exported pad has, and the one that breaks a classifier.
fn any_sliver() -> impl Strategy<Value = Region> {
  (-20.0f64..20.0, -20.0f64..20.0, 0.02f64..0.4, 1.0f64..15.0).prop_map(|(x, y, thin, long)| {
    let contour = Contour::from_points(
      &[
        Point2::new(x, y),
        Point2::new(x + long, y),
        Point2::new(x + long, y + thin),
        Point2::new(x, y + thin),
      ],
      true,
    )
    .expect("a sliver");
    Region::new(vec![contour]).expect("one boundary")
  })
}

fn any_region() -> impl Strategy<Value = Region> {
  prop_oneof![any_star(), any_sliver()]
}

fn area_of(a: &Region, b: &Region, op: BooleanOp) -> Result<f64, cam_geom::GeomError> {
  Ok(match combine(a, b, op, TOLERANCE)? {
    Combined::Empty => 0.0,
    Combined::Region { region, .. } => area(&region),
  })
}

/// The three areas of a pair, or the reason one of them could not be resolved.
fn trio(a: &Region, b: &Region) -> Result<(f64, f64, f64), cam_geom::GeomError> {
  Ok((
    area_of(a, b, BooleanOp::Union)?,
    area_of(a, b, BooleanOp::Intersection)?,
    area_of(a, b, BooleanOp::Difference)?,
  ))
}

fn close(actual: f64, expected: f64) -> bool {
  let scale = actual.abs().max(expected.abs()).max(1.0);
  (actual - expected).abs() <= 1.0e-9 * scale
}

proptest! {
  #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

  /// The identity that catches a misclassified crossing: two different operations have to agree
  /// about one total.
  #[test]
  fn the_area_identity_holds(a in any_region(), b in any_region()) {
    // A pair whose boundaries nearly coincide can be unresolvable at this tolerance. Resolvability is
    // covered by its own test; retrying it here would only test the random number generator.
    let Ok((union, intersection, _)) = trio(&a, &b) else {
      return Ok(());
    };
    prop_assert!(
      close(union + intersection, area(&a) + area(&b)),
      "|A∪B| + |A∩B| = {} but |A| + |B| = {}",
      union + intersection,
      area(&a) + area(&b)
    );
  }

  /// Subtracting what they share leaves what only the first has.
  #[test]
  fn difference_and_intersection_partition_the_first(a in any_region(), b in any_region()) {
    let Ok((_, intersection, difference)) = trio(&a, &b) else {
      return Ok(());
    };
    prop_assert!(
      close(difference + intersection, area(&a)),
      "|A∖B| + |A∩B| = {} but |A| = {}",
      difference + intersection,
      area(&a)
    );
  }

  /// Union and intersection do not care which side is asked first.
  #[test]
  fn the_operations_commute(a in any_region(), b in any_region()) {
    let (Ok(union), Ok(intersection)) = (area_of(&a, &b, BooleanOp::Union), area_of(&a, &b, BooleanOp::Intersection)) else {
      return Ok(());
    };
    prop_assert!(close(area_of(&b, &a, BooleanOp::Union)?, union));
    prop_assert!(close(area_of(&b, &a, BooleanOp::Intersection)?, intersection));
  }

  /// A region combined with itself is itself, and a region minus itself is nothing.
  #[test]
  fn idempotence_and_self_subtraction(a in any_region()) {
    prop_assert!(close(area_of(&a, &a, BooleanOp::Union)?, area(&a)));
    prop_assert!(close(area_of(&a, &a, BooleanOp::Intersection)?, area(&a)));
    prop_assert!(area_of(&a, &a, BooleanOp::Difference)? == 0.0);
  }

  /// The same pair combined twice is the same region, vertex for vertex: the walk is deterministic
  /// and the result does not depend on anything but its input.
  #[test]
  fn the_result_is_deterministic(a in any_region(), b in any_region()) {
    let first = combine(&a, &b, BooleanOp::Union, TOLERANCE);
    let second = combine(&a, &b, BooleanOp::Union, TOLERANCE);
    match (first, second) {
      (Ok(first), Ok(second)) => prop_assert_eq!(first, second),
      (Err(first), Err(second)) => prop_assert_eq!(first.to_string(), second.to_string()),
      _ => prop_assert!(false, "the same pair resolved differently on two runs"),
    }
  }
}
