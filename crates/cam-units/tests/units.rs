//! Behaviour tests for units and formatting (A1, A6).

use cam_units::{Angle, Format, Length, UnitSystem};
use proptest::prelude::*;

fn job_scale() -> impl Strategy<Value = f64> {
  prop_oneof![
    -1.0e6..1.0e6f64,
    Just(0.0f64),
    Just(-0.0f64),
    -1.0e-6..1.0e-6f64,
    Just(f64::MIN_POSITIVE),
  ]
}

/// Values that provably round to zero at the paired decimal count, so the property never has to
/// reject generated cases.
fn rounds_to_zero() -> impl Strategy<Value = (f64, u8)> {
  (0u8..6).prop_flat_map(|decimals| {
    let bound = 0.49 / 10f64.powi(i32::from(decimals));
    (-bound..bound).prop_map(move |value| (value, decimals))
  })
}

proptest! {
    /// Round-tripping through inches is stable to 1e-9 mm.
    #[test]
    fn inch_round_trip_is_stable(mm in job_scale()) {
        let len = Length::from_mm(mm);
        let again = Length::from_value_in(UnitSystem::Inch, len.value_in(UnitSystem::Inch));
        prop_assert!(
            (len.mm() - again.mm()).abs() <= 1e-9,
            "{} != {}",
            len.mm(),
            again.mm()
        );
    }

    /// A value whose rounded result is zero must never render as a negative zero.
    #[test]
    fn format_never_emits_negative_zero((value, decimals) in rounds_to_zero()) {
        let text = Format::fixed(decimals).apply(value);
        prop_assert!(
            !text.starts_with("-0"),
            "negative zero rendered: {:?} for {} at {} decimals",
            text, value, decimals
        );
    }

    /// A negative value that does not round to zero keeps its sign.
    #[test]
    fn format_keeps_sign_when_non_zero(value in -1.0e6..-0.5f64, decimals in 0u8..4) {
        let text = Format::fixed(decimals).apply(value);
        prop_assert!(text.starts_with('-'), "lost sign in {:?}", text);
    }

    /// Formatting is locale-independent and never uses scientific notation at job scale.
    #[test]
    fn format_is_plain_decimal(value in -1.0e6..1.0e6f64, decimals in 0u8..6) {
        let text = Format::fixed(decimals).apply(value);
        prop_assert!(!text.contains('e') && !text.contains('E'), "{}", text);
        prop_assert!(!text.contains(','), "{}", text);
        prop_assert_eq!(
            text.split('.').nth(1).map_or(0, str::len),
            usize::from(decimals),
            "in {:?} ({})",
            text,
            value
        );
    }
}

#[test]
fn format_rounds_half_away_from_zero() {
  // Documented choice: ties go away from zero, so a 0.5 mm feature never shrinks silently.
  // Only binary-exact ties are assertable; see the representability test below.
  assert_eq!(Format::fixed(0).apply(0.5), "1");
  assert_eq!(Format::fixed(0).apply(-0.5), "-1");
  assert_eq!(Format::fixed(2).apply(0.125), "0.13");
  assert_eq!(Format::fixed(2).apply(-0.125), "-0.13");
  assert_eq!(Format::fixed(2).apply(2.5), "2.50");
}

/// Rounding operates on the exact binary value held by the `f64`, never on the decimal literal the
/// user typed. 1.005 is 1.00499999999999989… in binary, so two decimals correctly yield 1.00.
///
/// This matches what a C printf-based emitter does, which is what the analysed product uses; adding
/// an epsilon to force 1.01 would make 0.0049999 round up and hide a real geometry error.
#[test]
fn format_rounds_the_binary_value_not_the_decimal_literal() {
  assert_eq!(Format::fixed(2).apply(1.005), "1.00");
  assert_eq!(Format::fixed(2).apply(0.125), "0.13");
}

#[test]
fn format_pins_four_decimal_post_processor_style() {
  // The recovered .pp dialect asks for 4 decimals on positions and 3 on some variants.
  assert_eq!(Format::fixed(4).apply(12.34567), "12.3457");
  assert_eq!(Format::fixed(3).apply(-12.34567), "-12.346");
  assert_eq!(Format::fixed(4).apply(0.0), "0.0000");
}

#[test]
fn inch_conversion_matches_definition() {
  assert_eq!(Length::from_value_in(UnitSystem::Inch, 1.0).mm(), 25.4);
  assert!((Length::from_mm(25.4).value_in(UnitSystem::Inch) - 1.0).abs() < 1e-12);
}

#[test]
fn angle_constructors_agree() {
  assert!((Angle::from_degrees(180.0).radians() - std::f64::consts::PI).abs() < 1e-12);
  assert!((Angle::from_radians(std::f64::consts::TAU).degrees() - 360.0).abs() < 1e-12);
}

#[test]
fn unit_system_serialises_as_lowercase() {
  assert_eq!(serde_json::to_string(&UnitSystem::Mm).unwrap(), "\"mm\"");
  assert_eq!(
    serde_json::to_string(&UnitSystem::Inch).unwrap(),
    "\"inch\""
  );
}
