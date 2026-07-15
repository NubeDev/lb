//! `time` boundary snapping — start/end of day/week/month/year. `start_of_*` floors to the
//! boundary; `end_of_*` is the boundary's LAST second (next boundary − 1s), matching the scope's
//! "ceil-1s" contract so `start..=end` covers the period inclusively. Weeks are ISO (Monday).

use chrono::{Datelike, Days, NaiveDate};
use rhai::{Engine, EvalAltResult};

use super::{utc, TimeHandle};
use crate::grid::rhai_err;

pub(super) fn register(engine: &mut Engine) {
    engine.register_fn("start_of_day", |_: &mut TimeHandle, ts: i64| {
        Ok::<i64, Box<EvalAltResult>>(midnight(utc(ts)?.date_naive()))
    });
    engine.register_fn("start_of_week", |_: &mut TimeHandle, ts: i64| {
        start_of_week(ts)
    });
    engine.register_fn("start_of_month", |_: &mut TimeHandle, ts: i64| {
        start_of_month(ts)
    });
    engine.register_fn("start_of_year", |_: &mut TimeHandle, ts: i64| {
        let y = utc(ts)?.year();
        // Jan 1 exists in every chrono-representable year.
        Ok::<i64, Box<EvalAltResult>>(midnight(NaiveDate::from_ymd_opt(y, 1, 1).unwrap()))
    });
    engine.register_fn("end_of_day", |_: &mut TimeHandle, ts: i64| {
        Ok::<i64, Box<EvalAltResult>>(midnight(utc(ts)?.date_naive()) + 86_400 - 1)
    });
    engine.register_fn("end_of_month", |_: &mut TimeHandle, ts: i64| {
        end_of_month(ts)
    });
}

fn start_of_week(ts: i64) -> Result<i64, Box<EvalAltResult>> {
    let d = utc(ts)?.date_naive();
    let back = Days::new(d.weekday().num_days_from_monday() as u64);
    let monday = d
        .checked_sub_days(back)
        .ok_or_else(|| rhai_err(format!("timestamp {ts} is out of range")))?;
    Ok(midnight(monday))
}

fn start_of_month(ts: i64) -> Result<i64, Box<EvalAltResult>> {
    let d = utc(ts)?.date_naive();
    // Day 1 exists in every month of a valid date.
    Ok(midnight(
        NaiveDate::from_ymd_opt(d.year(), d.month(), 1).unwrap(),
    ))
}

/// Last second of the month: first of the next month − 1s (leap-February safe by construction).
fn end_of_month(ts: i64) -> Result<i64, Box<EvalAltResult>> {
    let d = utc(ts)?.date_naive();
    let first_of_next = if d.month() == 12 {
        NaiveDate::from_ymd_opt(d.year() + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(d.year(), d.month() + 1, 1)
    };
    Ok(midnight(first_of_next.unwrap()) - 1)
}

fn midnight(d: NaiveDate) -> i64 {
    d.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2021-01-01T10:30:00Z — a Friday, so the week starts the previous Monday (2020-12-28).
    const TS: i64 = 1_609_459_200 + 10 * 3600 + 30 * 60;

    fn iso(ts: i64) -> String {
        utc(ts).unwrap().format("%Y-%m-%dT%H:%M:%SZ").to_string()
    }

    #[test]
    fn floors_to_each_boundary() {
        assert_eq!(
            iso(midnight(utc(TS).unwrap().date_naive())),
            "2021-01-01T00:00:00Z"
        );
        assert_eq!(iso(start_of_week(TS).unwrap()), "2020-12-28T00:00:00Z"); // crosses the year
        assert_eq!(iso(start_of_month(TS).unwrap()), "2021-01-01T00:00:00Z");
    }

    #[test]
    fn end_of_month_handles_length_and_leap() {
        // Feb 2024 (leap) → 29th; Feb 2023 → 28th; Dec wraps the year.
        let feb24 = 1_708_000_000; // 2024-02-15T…
        assert_eq!(iso(end_of_month(feb24).unwrap()), "2024-02-29T23:59:59Z");
        let feb23 = 1_676_500_000; // 2023-02-15T…
        assert_eq!(iso(end_of_month(feb23).unwrap()), "2023-02-28T23:59:59Z");
        let dec21 = 1_639_500_000; // 2021-12-14T…
        assert_eq!(iso(end_of_month(dec21).unwrap()), "2021-12-31T23:59:59Z");
    }

    #[test]
    fn end_of_day_is_last_second() {
        let midn = midnight(utc(TS).unwrap().date_naive());
        assert_eq!(iso(midn + 86_400 - 1), "2021-01-01T23:59:59Z");
    }
}
