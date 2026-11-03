//! The brand block's `Saved …` line: a civil date and time out of a [`SystemTime`], and no dependency.
//!
//! camui-prancheta S4 puts the document's own last-write stamp at the foot of the rail, and L7 fixed what
//! that line means: it stamps **the document**, not the build, so it comes from the file rather than from
//! `build.rs`. Formatting a `SystemTime` is where the design meets the dependency list, and the answer L7
//! named before paying for it — about thirty lines of civil-from-days instead of `chrono` or `time` — is what
//! this module is.
//!
//! **The clock is UTC, and the line says so.** A local offset needs a timezone database (or `libc`'s
//! `localtime_r`, which is a new dependency of this crate and a delivery question), and a shell that printed
//! UTC as if it were local would be wrong by hours in exactly the case the operator is checking. So
//! [`format_stamp`] prints `2026-11-03 19:30 UTC`: comparable, unambiguous, and honest about which clock it
//! read.

use std::time::{SystemTime, UNIX_EPOCH};

/// Whether a proleptic Gregorian year has a 29 February.
///
/// Unsigned arithmetic on purpose: `year % 4` is `0` for a leap year and for the year before one, and a
/// remainder is not a leap rule. The three-way rule is the calendar's, not an optimisation.
#[cfg(test)]
fn is_leap(year: i64) -> bool {
  (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Days since 1970-01-01 to a civil `(year, month, day)`.
///
/// Howard Hinnant's `civil_from_days`, which is exact for every day an `i64` can hold and needs no loop over
/// years: the era is a 400-year cycle of exactly 146 097 days, so one division lands in the right era and the
/// remaining arithmetic is the calendar's own.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
  // The era starts on 0000-03-01, which is why the day count is shifted by the 60 days January and February
  // would have contributed: that shift is what makes the leap day the *last* day of a year, so the month
  // division that follows is uniform.
  let shifted = days + 719_468;
  let era = if shifted >= 0 {
    shifted
  } else {
    shifted - 146_096
  } / 146_097;
  let day_of_era = shifted - era * 146_097;
  // 1460-day and 36 524-day cycles: the four-year cycle that carries one leap day, and the century cycle
  // that drops one.
  let year_of_era =
    (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
  let year = year_of_era + era * 400;
  let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
  let month_prime = (5 * day_of_year + 2) / 153;
  let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
  let month = if month_prime < 10 {
    month_prime + 3
  } else {
    month_prime - 9
  };
  let year = if month <= 2 { year + 1 } else { year };
  (year, month as u32, day as u32)
}

/// A civil date and time, in UTC, as the brand block writes it: `2026-11-03 19:30 UTC`.
///
/// The minute is the whole resolution: a second-by-second stamp is noise in a block that answers "is this the
/// file I just wrote", and a date alone cannot tell two edits of the same afternoon apart.
pub fn format_stamp(time: SystemTime) -> String {
  let (seconds, _) = match time.duration_since(UNIX_EPOCH) {
    Ok(after) => (after.as_secs() as i64, after.subsec_nanos()),
    Err(before) => {
      // A time before the epoch is what a clock set backwards looks like, not an error: the arithmetic below
      // is signed, so it formats as a date before 1970 rather than panicking or refusing to draw the block.
      let before = before.duration();
      let seconds = -(before.as_secs() as i64) - i64::from(before.subsec_nanos() > 0);
      let nanos = if before.subsec_nanos() > 0 {
        1_000_000_000 - before.subsec_nanos()
      } else {
        0
      };
      (seconds, nanos)
    }
  };
  // Floored division, so a negative timestamp still lands on the right day: `-1` second is 1969-12-31 23:59,
  // not 1970-01-01 -00:01.
  let days = seconds.div_euclid(86_400);
  let seconds_of_day = seconds.rem_euclid(86_400);
  let (year, month, day) = civil_from_days(days);
  let hour = seconds_of_day / 3_600;
  let minute = (seconds_of_day % 3_600) / 60;
  // The minute is the whole resolution, so the sub-second remainder is never read: nothing in the frame shows
  // it, and the arithmetic above only needs it to floor correctly on the negative side.
  // Years are printed with at least four digits; a year outside that range is a clock, not a document.
  format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02} UTC")
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::time::Duration;

  #[test]
  fn the_epoch_is_1970_and_the_day_it_starts() {
    assert_eq!(civil_from_days(0), (1970, 1, 1));
    assert_eq!(format_stamp(UNIX_EPOCH), "1970-01-01 00:00 UTC");
  }

  #[test]
  fn a_leap_day_and_the_day_after_it_are_the_calendars_own() {
    // 2000-02-29 is the leap day a century rule could have dropped: it is divisible by 400.
    let leap = UNIX_EPOCH + Duration::from_secs(951_782_400);
    assert_eq!(format_stamp(leap), "2000-02-29 00:00 UTC");
    let march = UNIX_EPOCH + Duration::from_secs(951_868_800);
    assert_eq!(format_stamp(march), "2000-03-01 00:00 UTC");
    // 1900 was not a leap year — divisible by 100 and not by 400 — which is the branch a two-rule calendar
    // gets wrong.
    assert!(!is_leap(1900));
    assert!(is_leap(2000));
    assert!(is_leap(2024));
    assert!(!is_leap(2025));
  }

  #[test]
  fn the_minute_and_the_hour_are_utc_and_the_output_says_so() {
    // 2026-11-03 19:30 UTC, the example L7 wrote down.
    let when = UNIX_EPOCH + Duration::from_secs(1_793_734_200);
    assert_eq!(format_stamp(when), "2026-11-03 19:30 UTC");
  }

  #[test]
  fn a_time_before_the_epoch_formats_as_a_date_before_1970() {
    // One second before the epoch is 1969-12-31 23:59, which floored division has to get right: truncating
    // division would answer 1970-01-01 -00:01 instead.
    let before = UNIX_EPOCH - Duration::from_secs(1);
    assert_eq!(format_stamp(before), "1969-12-31 23:59 UTC");
  }
}
