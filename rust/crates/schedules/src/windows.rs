//! The concrete local windows of a dated entry. A `yearly` entry repeats on the same dates every
//! year from its own first year on.

use chrono::{Datelike, NaiveDate, NaiveDateTime};

use crate::doc::DatedEntry;
use crate::parse::parse_local;

/// `entry`'s windows as `(start, end)` wall times. A non-yearly entry gives its own dates and ignores
/// `years`. A yearly entry gives one copy starting in each of `years`: never before the entry's own
/// year, and a 29 February start is skipped in a year without one (a 29 February end moves to
/// 1 March 00:00). Windows that do not parse are
/// skipped; `validate` refuses them at save, so a stored record has none.
pub fn occurrences(entry: &DatedEntry, years: &[i32]) -> Vec<(NaiveDateTime, NaiveDateTime)> {
    let mut out = Vec::new();
    for w in &entry.dates {
        let (Ok(start), Ok(end)) = (parse_local(&w.start), parse_local(&w.end)) else {
            continue;
        };
        if !entry.yearly {
            out.push((start, end));
            continue;
        }
        for &year in years {
            let by = year - start.year();
            if by < 0 {
                continue;
            }
            // An end on a missing 29 February is 1 March at midnight, as the other readers do.
            let e = end
                .with_year(end.year() + by)
                .or_else(|| NaiveDate::from_ymd_opt(end.year() + by, 3, 1)?.and_hms_opt(0, 0, 0));
            if let (Some(s), Some(e)) = (start.with_year(year), e) {
                out.push((s, e));
            }
        }
    }
    out
}
