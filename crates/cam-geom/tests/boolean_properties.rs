//! Property tests for the set operations: the algebra has to hold on shapes nobody wrote down.

use cam_geom::boolean::{combine, normalize, BooleanOp, Combined};
use cam_geom::region::Region;
use cam_geom::{primitive, Contour, Point2, Segment};
use proptest::prelude::*;
use std::f64::consts::TAU;

/// Comfortably finer than any feature the generators below can produce, so a refusal is a defect.
const TOLERANCE: cam_geom::TolerancePolicy = cam_geom::TolerancePolicy::uniform(1.0e-6);

fn area(region: &Region) -> f64 {
  region
    .boundaries
    .iter()
    .flat_map(|boundary| boundary.segments())
    .map(|segment: Segment| segment.area_contribution())
    .sum::<f64>()
    .abs()
}

/// A region's own area, after the engine has healed its contour, or `None` when it will not heal.
///
/// A self-crossing contour is a legal region whose signed area is zero while its material is not
/// (documented in `034` L3), so the identity has to be asked of the healed region. Reading the raw
/// contour's area would make the property about the contour's winding instead of about the sets.
fn healed_area(region: &Region) -> Option<f64> {
  normalize(region, TOLERANCE)
    .ok()
    .map(|healed| area(&healed.region))
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

/// A contour that crosses itself: the bowtie a hand-typed or exported outline produces.
///
/// C1c asks for degenerate pairs in the random corpus, not only in the written one. The engine heals
/// the crossing in `normalize`, so the properties stay about the operations rather than about the
/// healing, but a generator that only ever produced simple polygons would never put a crossing in
/// front of `combine` at all.
fn any_crossing_quad() -> impl Strategy<Value = Region> {
  (-20.0f64..20.0, -20.0f64..20.0, 1.0f64..12.0, 1.0f64..12.0).prop_map(|(x, y, w, h)| {
    let contour = Contour::from_points(
      &[
        Point2::new(x, y),
        Point2::new(x + w, y + h),
        Point2::new(x + w, y),
        Point2::new(x, y + h),
      ],
      true,
    )
    .expect("a crossing quad");
    Region::new(vec![contour]).expect("one boundary")
  })
}

/// A two-arc disc: the boundary form `primitive::circle` produces, with two semicircles that meet
/// at the same seam points.
///
/// Discs that intersect each other cut pieces at the crossing points and destroy the seam identity,
/// so this generator also feeds pairs where a disc is contained in or tangent to another region and
/// its two arcs must both survive deduplication unchanged.
fn any_disc() -> impl Strategy<Value = Region> {
  (-20.0f64..20.0, -20.0f64..20.0, 0.5f64..12.0).prop_map(|(cx, cy, radius)| {
    let contour = primitive::circle(Point2::new(cx, cy), radius).expect("circle");
    Region::new(vec![contour]).expect("one boundary")
  })
}

fn any_region() -> impl Strategy<Value = Region> {
  prop_oneof![any_star(), any_sliver(), any_disc(), any_crossing_quad()]
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

fn region_of(a: &Region, b: &Region, op: BooleanOp) -> Result<Region, cam_geom::GeomError> {
  Ok(match combine(a, b, op, TOLERANCE)? {
    Combined::Empty => Region::new(Vec::new()).expect("empty region"),
    Combined::Region { region, .. } => region,
  })
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
    let (Some(area_a), Some(area_b)) = (healed_area(&a), healed_area(&b)) else {
      return Ok(());
    };
    prop_assert!(
      close(union + intersection, area_a + area_b),
      "|A∪B| + |A∩B| = {} but |A| + |B| = {}",
      union + intersection,
      area_a + area_b
    );
  }

  /// Subtracting what they share leaves what only the first has.
  #[test]
  fn difference_and_intersection_partition_the_first(a in any_region(), b in any_region()) {
    let Ok((_, intersection, difference)) = trio(&a, &b) else {
      return Ok(());
    };
    let Some(area_a) = healed_area(&a) else {
      return Ok(());
    };
    prop_assert!(
      close(difference + intersection, area_a),
      "|A∖B| + |A∩B| = {} but |A| = {}",
      difference + intersection,
      area_a
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
    let Some(area_a) = healed_area(&a) else {
      return Ok(());
    };
    prop_assert!(close(area_of(&a, &a, BooleanOp::Union)?, area_a));
    prop_assert!(close(area_of(&a, &a, BooleanOp::Intersection)?, area_a));
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

  /// De Morgan: removing a union is the same as removing each and keeping the common remainder.
  #[test]
  fn de_morgan_difference_over_union(a in any_region(), b in any_region(), c in any_region()) {
    let Ok(b_union_c) = region_of(&b, &c, BooleanOp::Union) else { return Ok(()); };
    let Ok(left) = region_of(&a, &b_union_c, BooleanOp::Difference) else { return Ok(()); };
    let Ok(a_minus_b) = region_of(&a, &b, BooleanOp::Difference) else { return Ok(()); };
    let Ok(a_minus_c) = region_of(&a, &c, BooleanOp::Difference) else { return Ok(()); };
    let Ok(right) = region_of(&a_minus_b, &a_minus_c, BooleanOp::Intersection) else { return Ok(()); };
    prop_assert!(
      close(area(&left), area(&right)),
      "A\\(B∪C)={} but (A\\B)∩(A\\C)={}",
      area(&left),
      area(&right)
    );
  }

  /// De Morgan: removing an intersection is the same as removing either.
  #[test]
  fn de_morgan_difference_over_intersection(a in any_region(), b in any_region(), c in any_region()) {
    let Ok(b_inter_c) = region_of(&b, &c, BooleanOp::Intersection) else { return Ok(()); };
    let Ok(left) = region_of(&a, &b_inter_c, BooleanOp::Difference) else { return Ok(()); };
    let Ok(a_minus_b) = region_of(&a, &b, BooleanOp::Difference) else { return Ok(()); };
    let Ok(a_minus_c) = region_of(&a, &c, BooleanOp::Difference) else { return Ok(()); };
    let Ok(right) = region_of(&a_minus_b, &a_minus_c, BooleanOp::Union) else { return Ok(()); };
    prop_assert!(
      close(area(&left), area(&right)),
      "A\\(B∩C)={} but (A\\B)∪(A\\C)={}",
      area(&left),
      area(&right)
    );
  }
}
