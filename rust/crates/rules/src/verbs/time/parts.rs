//! `time` components + calendar predicates — timestamp → year/month/…/weekday/ISO-week and the
//! leap-year/weekend/days-in-month questions. All in UTC (the offset arity of `format` is the only
//! place a zone exists in v1).

use chrono::{Datelike, NaiveDate, Timelike, Weekday};
use rhai::{Engine, EvalAltResult};

use super::{utc, TimeHandle};

pub(super) fn register(engine: &mut Engine) {
    engine.register_fn("year", |_: &mut TimeHandle, ts: i64| {
        Ok::<i64, Box<EvalAltResult>>(utc(ts)?.year() as i64)
    });
    engine.register_fn("month", |_: &mut TimeHandle, ts: i64| {
        Ok::<i64, Box<EvalAltResult>>(utc(ts)?.month() as i64)
    });
    engine.register_fn("day", |_: &mut TimeHandle, ts: i64| {
        Ok::<i64, Box<EvalAltResult>>(utc(ts)?.day() as i64)
    });
    engine.register_fn("hour", |_: &mut TimeHandle, ts: i64| {
        Ok::<i64, Box<EvalAltResult>>(utc(ts)?.hour() as i64)
    });
    engine.register_fn("minute", |_: &mut TimeHandle, ts: i64| {
        Ok::<i64, Box<EvalAltResult>>(utc(ts)?.minute() as i64)
    });
    engine.register_fn("second", |_: &mut TimeHandle, ts: i64| {
        Ok::<i64, Box<EvalAltResult>>(utc(ts)?.second() as i64)
    });
    engine.register_fn("weekday", |_: &mut TimeHandle, ts: i64| {
        Ok::<i64, Box<EvalAltResult>>(utc(ts)?.weekday().number_from_monday() as i64)
    });
    engine.register_fn("weekday_name", |_: &mut TimeHandle, ts: i64| {
        Ok::<String, Box<EvalAltResult>>(weekday_name(utc(ts)?.weekday()))
    });
    engine.register_fn("day_of_year", |_: &mut TimeHandle, ts: i64| {
        Ok::<i64, Box<EvalAltResult>>(utc(ts)?.ordinal() as i64)
    });
    engine.register_fn("iso_week", |_: &mut TimeHandle, ts: i64| {
        Ok::<i64, Box<EvalAltResult>>(utc(ts)?.iso_week().week() as i64)
    });
    engine.register_fn("days_in_month", |_: &mut TimeHandle, ts: i64| {
        let d = utc(ts)?.date_naive();
        Ok::<i64, Box<EvalAltResult>>(days_in_month(d.year(), d.month()))
    });
    engine.register_fn("is_leap_year", |_: &mut TimeHandle, ts: i64| {
        // A year is leap exactly when Feb 29 exists in it.
        Ok::<bool, Box<EvalAltResult>>(NaiveDate::from_ymd_opt(utc(ts)?.year(), 2, 29).is_some())
    });
    engine.register_fn("is_weekend", |_: &mut TimeHandle, ts: i64| {
        Ok::<bool, Box<EvalAltResult>>(matches!(utc(ts)?.weekday(), Weekday::Sat | Weekday::Sun))
    });
}

fn weekday_name(wd: Weekday) -> String {
    match wd {
        Weekday::Mon => "Monday",
        Weekday::Tue => "Tuesday",
        Weekday::Wed => "Wednesday",
        Weekday::Thu => "Thursday",
        Weekday::Fri => "Friday",
        Weekday::Sat => "Saturday",
        Weekday::Sun => "Sunday",
    }
    .to_string()
}

/// Day count of a civil month — first of the next month, stepped back one day.
pub(super) fn days_in_month(year: i32, month: u32) -> i64 {
    let first_of_next = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    };
    // month is 1..=12 by construction (came from a valid date), so both lookups succeed.
    first_of_next.unwrap().pred_opt().unwrap().day() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    const T0: i64 = 1_609_459_200; // 2021-01-01T00:00:00Z, a Friday

    #[test]
    fn components_of_a_known_instant() {
        let ts = T0 + 3 * 3600 + 21 * 60 + 5; // 03:21:05
        let dt = utc(ts).unwrap();
        assert_eq!((dt.year(), dt.month(), dt.day()), (2021, 1, 1));
        assert_eq!((dt.hour(), dt.minute(), dt.second()), (3, 21, 5));
        assert_eq!(dt.weekday().number_from_monday(), 5); // Friday
        assert_eq!(weekday_name(dt.weekday()), "Friday");
        assert_eq!(dt.ordinal(), 1);
    }

    #[test]
    fn iso_week_53_edge() {
        // 2021-01-01 (Friday) belongs to ISO week 53 of 2020.
        assert_eq!(utc(T0).unwrap().iso_week().week(), 53);
        // 2026-01-01 (Thursday) is ISO week 1.
        assert_eq!(utc(1_767_225_600).unwrap().iso_week().week(), 1);
    }

    #[test]
    fn month_lengths_and_leap_years() {
        assert_eq!(days_in_month(2024, 2), 29); // leap year (div 4)
        assert_eq!(days_in_month(2023, 2), 28);
        assert_eq!(days_in_month(2100, 2), 28); // century, NOT leap (div 100)
        assert_eq!(days_in_month(2000, 2), 29); // …unless div 400
        assert_eq!(days_in_month(2021, 4), 30);
        assert_eq!(days_in_month(2021, 12), 31); // the year-wrap branch
    }

    #[test]
    fn weekend_detection() {
        // 2021-01-02 was a Saturday, 2021-01-04 a Monday.
        assert!(matches!(utc(T0 + 86_400).unwrap().weekday(), Weekday::Sat));
        assert!(!matches!(
            utc(T0 + 3 * 86_400).unwrap().weekday(),
            Weekday::Sat | Weekday::Sun
        ));
    }
}
