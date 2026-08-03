//! The proleptic-Gregorian calendar maths the resolver steps with — Howard Hinnant's
//! `civil_from_days` / `days_from_civil` (exact over the whole calendar; the maths was ported from
//! the downstream `rubix-ai/src/report/preset.rs`, which this module's grammar replaces outright —
//! there is no preset compat layer), plus the clamped month stepping and the Monday-week arithmetic the grammar's
//! calendar tokens need. Pure integer maths, no clocks, no timezone — the timezone lives in
//! [`super::resolve`], which converts an instant to a *local* civil datetime before calling in here.

/// Days-since-epoch → civil `(year, month, day)`. Hinnant's `civil_from_days` — exact for the whole
/// proleptic Gregorian calendar, and it cannot drift the way a hand-rolled leap-year loop does.
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// The exact inverse of [`civil_from_days`].
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let mp = if m > 2 { m - 3 } else { m + 9 } as u64;
    let doy = (153 * mp + 2) / 5 + d as u64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe as i64 - 719_468
}

/// How many days month `m` of year `y` holds (the clamp bound for month stepping).
pub fn days_in_month(y: i64, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
                29
            } else {
                28
            }
        }
    }
}

/// Step `(y, m, d)` by `delta` calendar months, **clamping the day** to the target month's length —
/// the scope's decided semantics: 31 Mar − 1 month = 28 Feb (29 in a leap year), never a rollover
/// into 2/3 March.
pub fn add_months(y: i64, m: u32, d: u32, delta: i64) -> (i64, u32, u32) {
    let months = y * 12 + (m as i64 - 1) + delta;
    let ny = months.div_euclid(12);
    let nm = months.rem_euclid(12) as u32 + 1;
    (ny, nm, d.min(days_in_month(ny, nm)))
}

/// The weekday of a day number, **0 = Monday** … 6 = Sunday (weeks start Monday, decided in the
/// scope). Day 0 (1970-01-01) was a Thursday.
pub fn weekday(days: i64) -> u32 {
    (days + 3).rem_euclid(7) as u32
}

/// The first month of the calendar quarter containing month `m` — quarters are Jan/Apr/Jul/Oct.
pub fn quarter_start_month(m: u32) -> u32 {
    1 + 3 * ((m - 1) / 3)
}

/// Format a day number as an ISO `yyyy-mm-dd` day.
pub fn iso_day(days: i64) -> String {
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The epoch conversion is exact both ways, including the leap day a hand-rolled calendar gets
    /// wrong — the same pin the downstream `preset.rs` carried.
    #[test]
    fn conversion_is_exact_both_ways() {
        assert_eq!(iso_day(0), "1970-01-01");
        assert_eq!(iso_day(days_from_civil(2028, 2, 29)), "2028-02-29");
        assert_eq!(civil_from_days(days_from_civil(2028, 2, 29)), (2028, 2, 29));
    }

    /// Month stepping clamps to the target month's length (the 31-Mar → 28-Feb decision) and steps
    /// cleanly across a year boundary in both directions.
    #[test]
    fn month_stepping_clamps_and_crosses_years() {
        assert_eq!(add_months(2026, 3, 31, -1), (2026, 2, 28));
        assert_eq!(add_months(2028, 3, 31, -1), (2028, 2, 29)); // leap year keeps the 29th
        assert_eq!(add_months(2027, 1, 15, -1), (2026, 12, 15));
        assert_eq!(add_months(2026, 12, 31, 2), (2027, 2, 28));
    }

    /// Weeks start Monday: 2026-07-27 was a Monday, 2026-08-02 the Sunday closing that week.
    #[test]
    fn weeks_start_monday() {
        assert_eq!(weekday(days_from_civil(2026, 7, 27)), 0);
        assert_eq!(weekday(days_from_civil(2026, 8, 2)), 6);
        assert_eq!(weekday(days_from_civil(1970, 1, 1)), 3); // a Thursday
    }

    /// Quarters are Jan/Apr/Jul/Oct.
    #[test]
    fn quarters_are_calendar_quarters() {
        assert_eq!(quarter_start_month(1), 1);
        assert_eq!(quarter_start_month(3), 1);
        assert_eq!(quarter_start_month(4), 4);
        assert_eq!(quarter_start_month(11), 10);
    }
}
