//! Behaviour tests for the moulding rail geometry and the stepover law (M8 T2, T3; S1–S3).
//!
//! Three claims are pinned here, and every other expectation is written from the geometry rather
//! than read back from the code under test:
//!
//! * the rail is walked by **arc length**, and an arc answers a point from its own equation — never
//!   from a tessellation (T2's "arcs preserved");
//! * the band a cross-section sweeps is as wide as the cross-section at every sample and covers the
//!   whole offset family between the samples, to the declared chord tolerance (T2's envelope
//!   certificate);
//! * the stepover law spaces the lateral passes so that every lateral position of the profile is
//!   within half a stepover of one of them (T3's coverage certificate), and the two cases it has to
//!   decide — a spacing exactly the profile's width, and one wider than it — are decided by name.

use cam_geom::moulding::{CrossSection, CrossSectionPoint, Rail, StepoverLaw, SweptBand};
use cam_geom::{Contour, GeomError, LineSeg, Point2, Vertex};
use proptest::prelude::*;
use std::f64::consts::{FRAC_PI_2, PI};

/// Tolerance for comparing millimetre coordinates, well below any machine resolution and far above
/// `f64` noise at job scale.
const EPS: f64 = 1.0e-9;

/// The rail of a fixture, which a valid contour always has.
fn rail_of(contour: &Contour) -> Rail {
  Rail::new(contour).expect("a rail with length")
}

/// A straight open rail along +X at y = 50, so a lateral offset to the right of travel stays inside a
/// default sheet and a wrong sign is visible in the assertion.
fn straight_rail() -> Contour {
  Contour::from_points(&[Point2::new(0.0, 50.0), Point2::new(100.0, 50.0)], false).expect("rail")
}

/// A rail with one corner: two straight segments meeting at a right angle.
fn cornered_rail() -> Contour {
  Contour::from_points(
    &[
      Point2::new(0.0, 50.0),
      Point2::new(60.0, 50.0),
      Point2::new(60.0, 0.0),
    ],
    false,
  )
  .expect("cornered rail")
}

/// A closed circular rail of `radius`, drawn as two half arcs and travelled counter-clockwise, so a
/// positive lateral offset is the outside of the circle.
fn circular_rail(radius: f64) -> Contour {
  Contour::new(
    vec![
      Vertex::new(Point2::new(radius, 0.0), 1.0),
      Vertex::new(Point2::new(-radius, 0.0), 1.0),
    ],
    true,
  )
  .expect("circle from two half arcs")
}

/// The 12 mm wide cross-section the task's own example uses: zero at both edges, 4 mm deep at the
/// middle.
fn cross_section() -> CrossSection {
  CrossSection::new(vec![
    CrossSectionPoint {
      lateral: 0.0,
      depth: 0.0,
    },
    CrossSectionPoint {
      lateral: 6.0,
      depth: -4.0,
    },
    CrossSectionPoint {
      lateral: 12.0,
      depth: 0.0,
    },
  ])
  .expect("cross-section")
}

#[test]
fn arc_length_runs_from_zero_to_the_rails_own_perimeter() {
  let rail = rail_of(&straight_rail());
  assert_eq!(rail.length(), 100.0);
  assert!(!rail.is_closed());
  assert_eq!(rail.point_at(0.0), Point2::new(0.0, 50.0));
  assert_eq!(rail.point_at(25.0), Point2::new(25.0, 50.0));
  assert_eq!(rail.point_at(100.0), Point2::new(100.0, 50.0));

  let cornered = rail_of(&cornered_rail());
  assert_eq!(cornered.length(), 110.0);
  assert_eq!(cornered.point_at(60.0), Point2::new(60.0, 50.0));
  assert_eq!(cornered.point_at(85.0), Point2::new(60.0, 25.0));

  let circle = rail_of(&circular_rail(20.0));
  assert!((circle.length() - 2.0 * PI * 20.0).abs() < EPS);
  assert!(circle.is_closed());
}

#[test]
fn a_distance_beyond_an_open_rail_is_clamped_to_its_ends() {
  // An open rail has two ends and no wrap: a sampler walking past either one lands on it instead of
  // silently continuing into a rail that does not exist.
  let rail = rail_of(&straight_rail());
  assert_eq!(rail.point_at(-5.0), Point2::new(0.0, 50.0));
  assert_eq!(rail.point_at(500.0), Point2::new(100.0, 50.0));
}

#[test]
fn a_closed_rail_wraps_its_arc_length() {
  // A closed rail sweeps a frame, so its arc length is a circle and not an interval: walking a whole
  // perimeter past the start is the start again.
  let rail = rail_of(&circular_rail(20.0));
  let semi = PI * 20.0;
  assert!(rail.point_at(0.0).distance_to(Point2::new(20.0, 0.0)) < EPS);
  assert!(rail.point_at(semi).distance_to(Point2::new(-20.0, 0.0)) < EPS);
  assert!(
    rail
      .point_at(rail.length() + 0.25)
      .distance_to(rail.point_at(0.25))
      < EPS
  );
}

#[test]
fn a_rail_with_no_length_is_refused() {
  // An empty contour is constructible through `Contour::default`, so the rail must refuse it by name
  // rather than answer a point on a shape that has none.
  match Rail::new(&Contour::default()) {
    Err(GeomError::MouldingRailTooShort { length }) => assert_eq!(length, 0.0),
    other => panic!("an empty contour is not a rail: {other:?}"),
  }
}

#[test]
fn a_curved_rail_is_never_tessellated_to_answer_a_point() {
  // The rail's own equation answers every distance: the point at arc length `s` on a circle of
  // radius `r` is `(r cos(s/r), r sin(s/r))`. A chord-based walk would miss this by the sagitta of
  // its step, which is many orders of magnitude above this assertion, so this is what "arcs
  // preserved" means as a test.
  let radius = 20.0;
  let rail = rail_of(&circular_rail(radius));
  for step in 0..=40 {
    let distance = rail.length() * (step as f64 / 40.0);
    let angle = distance / radius;
    let expected = Point2::new(radius * angle.cos(), radius * angle.sin());
    assert!(
      rail.point_at(distance).distance_to(expected) < EPS,
      "at {distance} mm the rail answered {:?}, the arc is at {expected:?}",
      rail.point_at(distance)
    );
  }
  // The samples themselves are on the arc, exactly, whatever the tolerance asked for.
  for sample in rail.samples(0.5).expect("samples") {
    assert!(
      sample.point.distance_to(Point2::new(0.0, 0.0)).abs() - radius < EPS,
      "{sample:?} is not on the rail"
    );
  }
}

#[test]
fn the_normal_is_one_unit_on_the_lateral_side_of_travel() {
  // Lateral is measured to the right of travel, so a rail running along +X has its profile to -Y.
  let rail = rail_of(&straight_rail());
  let normal = rail.normal_at(50.0);
  assert!((normal.x - 0.0).abs() < EPS && (normal.y + 1.0).abs() < EPS);
  assert_eq!(rail.offset_at(50.0, 4.0), Point2::new(50.0, 46.0));
  // The offset really is 4 mm from the rail, which a wrong sign would still satisfy; the direction is
  // what the assertion above pins.
  let offset = rail.offset_at(50.0, 4.0);
  assert!(
    (LineSeg::new(Point2::new(0.0, 50.0), Point2::new(100.0, 50.0)).distance_to_point(offset)
      - 4.0)
      .abs()
      < EPS
  );

  // A counter-clockwise circle's right side is its outside, and every sample's normal is the radial
  // direction: that is the normal T4 offsets each lateral pass by.
  let circle = rail_of(&circular_rail(20.0));
  let outward = circle.normal_at(0.0);
  assert!((outward.x - 1.0).abs() < EPS && outward.y.abs() < EPS);
  assert!(
    circle
      .offset_at(0.0, 3.0)
      .distance_to(Point2::new(23.0, 0.0))
      < EPS
  );
  for distance in [0.0, 7.0, 30.0, 100.0] {
    let sample = circle.point_at(distance);
    let radial = circle.normal_at(distance);
    let tangent = circle.tangent_at(distance);
    assert!(
      (tangent.dot(radial)).abs() < EPS,
      "the normal must be perpendicular to the tangent at {distance} mm"
    );
    assert!(
      sample.distance_to(circle.offset_at(distance, 5.0)) - 5.0 < EPS
        && (radial.length() - 1.0).abs() < EPS
    );
  }
}

#[test]
fn a_band_is_as_wide_as_the_profile_at_every_sample() {
  // The band is the cross-section's own shadow: its width is the profile's lateral span, at every
  // walk sample, which is what makes "profile width" a number the band can be checked against.
  let cross = cross_section();
  for contour in [straight_rail(), cornered_rail(), circular_rail(20.0)] {
    let rail = rail_of(&contour);
    let band = SweptBand::sweep(&rail, &cross, 0.1).expect("band");
    assert_eq!(band.width(), 12.0);
    for sample in band.samples() {
      let width = sample.outer.distance_to(sample.inner);
      assert!(
        (width - 12.0).abs() < EPS,
        "a band sample is {width} mm wide, the profile is 12 mm"
      );
    }
  }
}

#[test]
fn the_band_covers_the_rail_offset_family_at_every_sample() {
  // The envelope certificate: at each walk sample, the band's own cross-section is exactly the rail's
  // offset family. Every lateral position the profile allows lands on the inner-to-outer segment, so
  // no part of the profile's shadow is outside the band it claims.
  let cross = cross_section();
  let rail = rail_of(&cornered_rail());
  let band = SweptBand::sweep(&rail, &cross, 0.1).expect("band");
  for sample in band.samples() {
    let span = LineSeg::new(sample.inner, sample.outer);
    for step in 0..=48 {
      let lateral = 12.0 * (step as f64 / 48.0);
      let point = sample.offset(lateral);
      let fraction = (point - sample.inner).dot(sample.outer - sample.inner)
        / (sample.outer - sample.inner).length_squared();
      assert!(
        span.distance_to_point(point) < EPS,
        "lateral {lateral} mm at {} mm is off the band's cross-section",
        sample.distance
      );
      assert!(
        (fraction - lateral / 12.0).abs() < EPS,
        "lateral {lateral} mm sits at fraction {fraction} of the band, not {}",
        lateral / 12.0
      );
    }
  }
}

#[test]
fn the_band_covers_the_rail_offset_family_between_its_samples() {
  // Between samples the band is chords, so coverage is stated to the chord tolerance the walk was
  // given: every point of the offset family is inside the band, or within that distance of its
  // boundary. This is the certificate a curved rail needs — the sampling may chord the arc, but it
  // may not leave the swept shadow.
  let radius = 20.0;
  let tolerance = 0.05;
  let cross = CrossSection::new(vec![
    CrossSectionPoint {
      lateral: 0.0,
      depth: 0.0,
    },
    CrossSectionPoint {
      lateral: 2.0,
      depth: -1.0,
    },
  ])
  .expect("cross-section");
  let rail = rail_of(&circular_rail(radius));
  let band = SweptBand::sweep(&rail, &cross, tolerance).expect("band");
  assert!(band.samples().len() > 8, "the walk must sample the arcs");
  // A chord at the rail's radius has the declared sagitta; the offset curve at `lateral` is a bigger
  // arc by that lateral distance, so its chord error scales with the radius ratio. Stating the bound
  // that way keeps the assertion a derivation instead of a number that happens to fit.
  let bound = tolerance * (radius + 2.0) / radius + EPS;
  for step in 0..=200 {
    let distance = rail.length() * (step as f64 / 200.0);
    for lateral in [0.0, 0.5, 1.0, 1.5, 2.0] {
      let point = rail.offset_at(distance, lateral);
      assert!(
        band.contains(point, bound),
        "lateral {lateral} mm at {distance} mm is outside the band, bound {bound}"
      );
    }
    // The certificate has to mean something: a point past the profile's own span is not covered.
    assert!(
      !band.contains(rail.offset_at(distance, 2.5), bound),
      "lateral 2.5 mm at {distance} mm is outside the profile but the band claims it"
    );
  }
}

#[test]
fn a_closed_rail_sweeps_a_closed_band() {
  let cross = cross_section();
  let rail = rail_of(&circular_rail(20.0));
  let band = SweptBand::sweep(&rail, &cross, 0.5).expect("band");
  assert!(band.is_closed());
  assert_eq!(
    band.samples().last().expect("a sample").distance,
    rail.length()
  );
}

#[test]
fn two_runs_of_the_same_rail_and_band_are_identical() {
  // S5: the geometry has no iteration order to leak, and constructing it twice must produce the
  // same samples in the same order, byte for byte.
  let cross = cross_section();
  let first_rail = rail_of(&cornered_rail());
  let second_rail = rail_of(&cornered_rail());
  assert_eq!(
    first_rail.samples(0.1).expect("samples"),
    second_rail.samples(0.1).expect("samples")
  );
  assert_eq!(
    SweptBand::sweep(&first_rail, &cross, 0.1).expect("band"),
    SweptBand::sweep(&second_rail, &cross, 0.1).expect("band")
  );
  assert_eq!(
    StepoverLaw::new(&cross, 5.0).expect("law"),
    StepoverLaw::new(&cross, 5.0).expect("law")
  );
}

#[test]
fn depth_is_interpolated_between_the_profiles_own_points() {
  // The cross-section is a function of lateral distance, and its own points are exact: a depth read
  // back from anywhere else in the strategy would let a wrong table agree with itself.
  let cross = cross_section();
  assert_eq!(cross.depth_at(0.0), 0.0);
  assert_eq!(cross.depth_at(6.0), -4.0);
  assert_eq!(cross.depth_at(12.0), 0.0);
  assert!((cross.depth_at(3.0) + 2.0).abs() < EPS);
  assert!((cross.depth_at(9.0) + 2.0).abs() < EPS);
  // Outside its own span a cross-section keeps its edge depth rather than extrapolating a slope the
  // document never authored.
  assert_eq!(cross.depth_at(-5.0), 0.0);
  assert_eq!(cross.depth_at(50.0), 0.0);
}

#[test]
fn the_passes_cover_the_profile_edges_and_never_exceed_the_stepover() {
  let cross = cross_section();
  let law = StepoverLaw::new(&cross, 5.0).expect("law");
  assert_eq!(law.passes(), &[0.0, 5.0, 10.0, 12.0]);
  assert_eq!(law.lateral_min(), 0.0);
  assert_eq!(law.lateral_max(), 12.0);
  assert_eq!(law.stepover(), 5.0);
  for pair in law.passes().windows(2) {
    let gap = pair[1] - pair[0];
    assert!(
      gap > 0.0 && gap <= 5.0 + EPS,
      "the passes {pair:?} are {gap} mm apart, the stepover is 5 mm"
    );
  }
  // Both profile edges are passes: the authored depth at the edge is data, and dropping it would
  // leave the profile's own boundary uncut.
  assert_eq!(law.passes().first().copied(), Some(0.0));
  assert_eq!(law.passes().last().copied(), Some(12.0));
}

#[test]
fn every_lateral_position_is_within_half_a_stepover_of_a_pass() {
  // T3's coverage certificate, stated the way a machinist reads a stepover: no point of the
  // profile's lateral span is further than half a stepover from the nearest pass. The scan is
  // written from the law's public passes, not from its own coverage radius.
  let cross = CrossSection::new(vec![
    CrossSectionPoint {
      lateral: -7.0,
      depth: 0.0,
    },
    CrossSectionPoint {
      lateral: 3.0,
      depth: -3.0,
    },
    CrossSectionPoint {
      lateral: 11.0,
      depth: 0.0,
    },
  ])
  .expect("cross-section");
  for stepover in [0.5, 1.0, 2.25, 3.5] {
    let law = StepoverLaw::new(&cross, stepover).expect("law");
    for step in 0..=400 {
      let lateral = -7.0 + 18.0 * (step as f64 / 400.0);
      let nearest = law
        .passes()
        .iter()
        .map(|pass| (pass - lateral).abs())
        .fold(f64::INFINITY, f64::min);
      assert!(
        nearest <= stepover / 2.0 + EPS,
        "lateral {lateral} mm is {nearest} mm from a pass, more than half a {stepover} mm stepover"
      );
    }
    assert!(law.covers());
    assert!(law.coverage_radius() <= stepover / 2.0 + EPS);
  }
}

#[test]
fn a_stepover_of_exactly_the_profile_width_is_the_two_edge_passes() {
  // The boundary case, decided rather than clamped: at one stepover per profile width, one pass sits
  // on each edge and both authored edge depths are cut. It is *not* collapsed into a single pass.
  let cross = cross_section();
  let law = StepoverLaw::new(&cross, 12.0).expect("law");
  assert_eq!(law.passes(), &[0.0, 12.0]);
}

#[test]
fn a_stepover_wider_than_the_profile_is_one_centred_pass() {
  // The other decision, and the one the task asks to be explicit about: a spacing wider than the
  // whole cross-section cannot produce two distinct passes without passing closer than asked, so the
  // cross-section contributes exactly one pass, centred on it. Coverage still holds on the stepover's
  // own terms: half the profile width is less than half the requested stepover.
  let cross = cross_section();
  let law = StepoverLaw::new(&cross, 20.0).expect("law");
  assert_eq!(law.passes(), &[6.0]);
  assert!(law.covers());
  assert!((law.coverage_radius() - 6.0).abs() < EPS);
}

#[test]
fn a_cross_section_needs_at_least_two_points() {
  let error = CrossSection::new(vec![CrossSectionPoint {
    lateral: 0.0,
    depth: 0.0,
  }])
  .expect_err("one point is not a cross-section");
  match error {
    GeomError::MouldingProfileTooShort { points } => assert_eq!(points, 1),
    other => panic!("{other:?}"),
  }
}

#[test]
fn a_cross_section_refuses_non_increasing_laterals() {
  // A depth is a function of the lateral distance, so two points at the same lateral are two
  // different depths for one place; the refusal names the pair instead of picking one.
  let error = CrossSection::new(vec![
    CrossSectionPoint {
      lateral: 0.0,
      depth: 0.0,
    },
    CrossSectionPoint {
      lateral: 6.0,
      depth: -4.0,
    },
    CrossSectionPoint {
      lateral: 6.0,
      depth: -5.0,
    },
  ])
  .expect_err("a repeated lateral is not a cross-section");
  match error {
    GeomError::MouldingProfileLateralOrder {
      index,
      previous,
      lateral,
    } => {
      assert_eq!(index, 2);
      assert_eq!(previous, 6.0);
      assert_eq!(lateral, 6.0);
    }
    other => panic!("{other:?}"),
  }
}

#[test]
fn a_cross_section_refuses_a_non_finite_point() {
  let error = CrossSection::new(vec![
    CrossSectionPoint {
      lateral: 0.0,
      depth: 0.0,
    },
    CrossSectionPoint {
      lateral: f64::NAN,
      depth: -4.0,
    },
  ])
  .expect_err("a NaN lateral is not a cross-section");
  match error {
    GeomError::MouldingProfilePointNotFinite { lateral, depth } => {
      assert!(lateral.is_nan());
      assert_eq!(depth, -4.0);
    }
    other => panic!("{other:?}"),
  }
}

#[test]
fn a_stepover_must_be_positive_and_finite() {
  let cross = cross_section();
  for stepover in [0.0, -2.0, f64::NAN] {
    let error = StepoverLaw::new(&cross, stepover).expect_err("a pass needs spacing");
    match error {
      GeomError::MouldingStepoverNotPositive { stepover: value } => {
        assert_eq!(value.is_nan(), stepover.is_nan());
        if !stepover.is_nan() {
          assert_eq!(value, stepover);
        }
      }
      other => panic!("{other:?}"),
    }
  }
}

#[test]
fn a_band_refuses_a_non_positive_chord_tolerance() {
  let rail = rail_of(&straight_rail());
  let cross = cross_section();
  for tolerance in [0.0, -0.5, f64::NAN] {
    let error = SweptBand::sweep(&rail, &cross, tolerance).expect_err("no tolerance, no band");
    match error {
      GeomError::MouldingToleranceNotPositive { tolerance: value } => {
        assert_eq!(value.is_nan(), tolerance.is_nan());
      }
      other => panic!("{other:?}"),
    }
  }
}

proptest! {
  /// Any finite span and stepover the law accepts keeps the two certificates: every gap is at most one
  /// stepover and every lateral position is within half of one from a pass.
  #[test]
  fn any_span_and_stepover_still_cover_the_profile(
    min in -50.0f64..0.0,
    width in 0.1f64..50.0,
    stepover in 0.05f64..25.0,
  ) {
    let max = min + width;
    let cross = CrossSection::new(vec![
      CrossSectionPoint { lateral: min, depth: 0.0 },
      CrossSectionPoint { lateral: min + width / 2.0, depth: -(width / 2.0) },
      CrossSectionPoint { lateral: max, depth: 0.0 },
    ]).expect("cross-section");
    let law = StepoverLaw::new(&cross, stepover).expect("law");

    for pass in law.passes() {
      prop_assert!(*pass >= min - EPS, "{pass} is left of the profile");
      prop_assert!(*pass <= max + EPS, "{pass} is right of the profile");
    }
    for pair in law.passes().windows(2) {
      prop_assert!(pair[1] > pair[0]);
      prop_assert!(pair[1] - pair[0] <= stepover + EPS);
    }
    if stepover <= width {
      prop_assert!((law.passes()[0] - min).abs() < EPS);
      prop_assert!((law.passes()[law.passes().len() - 1] - max).abs() < EPS);
    }

    for step in 0..=200 {
      let lateral = min + width * (step as f64 / 200.0);
      let nearest = law.passes().iter().map(|pass| (pass - lateral).abs()).fold(f64::INFINITY, f64::min);
      prop_assert!(nearest <= stepover / 2.0 + EPS);
    }
    prop_assert!(law.covers());
  }

  /// The band's width is the profile's own span, for any span and any rail shape, which is the
  /// "band width equals profile width" certificate in the form a property can state.
  #[test]
  fn the_band_is_as_wide_as_the_span_for_any_span(
    min in -50.0f64..0.0,
    width in 0.1f64..50.0,
  ) {
    let cross = CrossSection::new(vec![
      CrossSectionPoint { lateral: min, depth: 0.0 },
      CrossSectionPoint { lateral: min + width, depth: -1.0 },
    ]).expect("cross-section");
    for contour in [straight_rail(), cornered_rail()] {
      let rail = rail_of(&contour);
      let band = SweptBand::sweep(&rail, &cross, 0.2).expect("band");
      prop_assert!((band.width() - width).abs() < EPS);
      for sample in band.samples() {
        prop_assert!((sample.outer.distance_to(sample.inner) - width).abs() < EPS);
      }
    }
  }
}

#[test]
fn a_walk_keeps_the_rails_segments_in_order_and_its_normals_on_their_own_side() {
  // A corner has two normals — one arriving, one leaving — so the walk carries both instead of
  // silently averaging them into a direction the rail never has. That is what lets T4 bridge the
  // corner with a straight move instead of cutting across it.
  let rail = rail_of(&cornered_rail());
  let samples = rail.samples(0.1).expect("samples");
  let distances: Vec<f64> = samples.iter().map(|sample| sample.distance).collect();
  let mut sorted = distances.clone();
  sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
  assert_eq!(distances, sorted, "the walk is in arc-length order");
  let at_corner: Vec<_> = samples
    .iter()
    .filter(|sample| (sample.distance - 60.0).abs() < EPS)
    .collect();
  assert_eq!(at_corner.len(), 2, "one normal arriving, one leaving");
  assert!((at_corner[0].normal.y + 1.0).abs() < EPS);
  assert!((at_corner[1].normal.x + 1.0).abs() < EPS);
  // The samples cover both segments: the second one runs down -Y from the corner.
  let last = samples.last().expect("a sample");
  assert!((last.distance - 110.0).abs() < EPS);
  assert!(last.point.distance_to(Point2::new(60.0, 0.0)) < EPS);
  assert!((last.normal.x + 1.0).abs() < EPS);
  // A right angle's two normals are a quarter turn apart, never a blend of the two.
  assert!((at_corner[0].normal.dot(at_corner[1].normal)).abs() < EPS);
  let octant = at_corner[0]
    .normal
    .cross(at_corner[1].normal)
    .atan2(at_corner[0].normal.dot(at_corner[1].normal))
    .abs();
  assert!(
    (octant - FRAC_PI_2).abs() < EPS,
    "the two corner normals are {octant} rad apart, a right angle is {FRAC_PI_2}"
  );
}
