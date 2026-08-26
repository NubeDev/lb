//! Calendar arithmetic for the file decoders: civil date/time → epoch milliseconds.
//!
//! **Why this is not `chrono`.** `lb-ingest` is a core crate compiled into every node, and the whole
//! of what the decoders need from a calendar is one direction of one conversion: a `YYYY-MM-DD
//! hh:mm:ss` that a file literally spells out, into epoch milliseconds. That is Howard Hinnant's
//! `days_from_civil` — twenty lines, proven, and exact for every date in the proleptic Gregorian
//! calendar. Pulling a date/time library into the data plane to avoid writing them would be a
//! dependency taken for a rounding of convenience.
//!
//! **There is no timezone database here, and that is deliberate.** A decoder is handed a fixed
//! `offset_minutes` by its caller ([`DecodeOptions`](super::DecodeOptions)) and applies it. Named
//! zones with DST are a *policy* question about what a file's local wall-clock means — the caller's
//! configuration, not a property of the bytes. Guessing here would silently shift a whole meter's
//! data by an hour twice a year.

/// Days since 1970-01-01 for a proleptic-Gregorian civil date. Hinnant's algorithm.
///
/// Valid for any `y`/`m`/`d` the caller has already range-checked; a nonsensical date produces a
/// nonsensical (but not panicking) day number, which is why [`epoch_ms`] validates first.
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let mp = (m as i64 + 9) % 12; // March = 0
    let doy = (153 * mp + 2) / 5 + d as i64 - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe - 719_468
}

/// A civil date + time-of-day + a fixed UTC offset → epoch **milliseconds**.
///
/// `offset_minutes` is how far the civil time is AHEAD of UTC (`+600` for AEST, `-300` for EST), so
/// the conversion subtracts it. Returns `None` for a date/time outside the calendar (month 13, day
/// 32, hour 24) rather than producing a plausible-looking wrong instant — a decoder that silently
/// accepted `20260732` would write a month of data at the wrong time.
pub fn epoch_ms(
    year: i64,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
    offset_minutes: i64,
) -> Option<u64> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    if day > days_in_month(year, month) || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let days = days_from_civil(year, month, day);
    let secs = days * 86_400 + hour as i64 * 3600 + minute as i64 * 60 + second as i64;
    let utc = secs - offset_minutes * 60;
    // Pre-epoch instants are refused rather than wrapped: the platform's `ts` is a `u64`, and a
    // 1969 timestamp wrapping to the year 584 million is the least debuggable outcome available.
    u64::try_from(utc).ok()?.checked_mul(1000)
}

/// Days in `month` of `year` (Gregorian leap rules).
pub fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Parse a compact `YYYYMMDD` date. Returns `(year, month, day)`.
pub fn parse_compact_date(s: &str) -> Option<(i64, u32, u32)> {
    let s = s.trim();
    if s.len() != 8 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((
        s[0..4].parse().ok()?,
        s[4..6].parse().ok()?,
        s[6..8].parse().ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_epoch_is_day_zero() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(epoch_ms(1970, 1, 1, 0, 0, 0, 0), Some(0));
    }

    #[test]
    fn a_known_instant_round_trips() {
        // 2026-07-01T00:15:00+10:00 == 2026-06-30T14:15:00Z.
        let aest = epoch_ms(2026, 7, 1, 0, 15, 0, 600).expect("valid");
        let utc = epoch_ms(2026, 6, 30, 14, 15, 0, 0).expect("valid");
        assert_eq!(aest, utc);
    }

    #[test]
    fn leap_day_exists_only_in_a_leap_year() {
        assert!(epoch_ms(2024, 2, 29, 0, 0, 0, 0).is_some());
        assert!(epoch_ms(2026, 2, 29, 0, 0, 0, 0).is_none());
        assert!(
            epoch_ms(1900, 2, 29, 0, 0, 0, 0).is_none(),
            "1900 is not a leap year"
        );
        assert!(epoch_ms(2000, 2, 29, 0, 0, 0, 0).is_some(), "2000 is");
    }

    #[test]
    fn an_impossible_date_is_refused_not_normalized() {
        assert!(epoch_ms(2026, 13, 1, 0, 0, 0, 0).is_none());
        assert!(epoch_ms(2026, 7, 32, 0, 0, 0, 0).is_none());
        assert!(epoch_ms(2026, 7, 1, 24, 0, 0, 0).is_none());
    }

    #[test]
    fn a_pre_epoch_instant_is_refused_rather_than_wrapped() {
        assert!(epoch_ms(1969, 12, 31, 23, 59, 59, 0).is_none());
    }

    #[test]
    fn compact_dates_parse_and_reject_junk() {
        assert_eq!(parse_compact_date("20260701"), Some((2026, 7, 1)));
        assert_eq!(parse_compact_date("2026-07-01"), None);
        assert_eq!(parse_compact_date("2026070"), None);
    }
}
