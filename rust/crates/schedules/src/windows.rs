//! The concrete local windows of a dated entry. A `yearly` entry repeats on the same dates every
//! year from its own first year on.

use chrono::{Datelike, NaiveDateTime};

use crate::doc::DatedEntry;
use crate::parse::parse_local;

/// `entry`'s windows as `(start, end)` wall times. A non-yearly entry gives its own dates and ignores
/// `years`. A yearly entry gives one copy starting in each of `years`: never before the entry's own
/// year, and a 29 February start is skipped in a year without one. Windows that do not parse are
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
            if let (Some(s), Some(e)) = (start.with_year(year), end.with_year(end.year() + by)) {
                out.push((s, e));
            }
        }
    }
    out
}
