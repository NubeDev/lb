//! The next wall time a schedule's state changes (on to off, off to on, or the running value
//! changing). Behind `schedule.evaluate`'s `next_transition`.

use chrono::{Datelike, Duration, NaiveDateTime};

use crate::doc::ScheduleDoc;
use crate::evaluate::evaluate_local;
use crate::parse::{minutes_of, weekday_from_name};
use crate::windows::occurrences;

/// How far ahead to look. A year and a day covers every yearly entry once.
// ponytail: evaluates every entry boundary in the horizon in time order, a few thousand points for a
// large document; index boundaries per day if evaluate ever profiles slow.
const HORIZON_DAYS: i64 = 367;

pub fn next_change(doc: &ScheduleDoc, t: NaiveDateTime) -> Option<NaiveDateTime> {
    let current = state(doc, t);
    let mut points = boundaries(doc, t);
    points.sort();
    points.dedup();
    points.into_iter().find(|p| state(doc, *p) != current)
}

/// `None` when off, `Some(value)` when on: a change in either is a transition.
fn state(doc: &ScheduleDoc, t: NaiveDateTime) -> Option<f64> {
    evaluate_local(doc, t).running.map(|r| r.value)
}

/// Every instant in `(t, t + HORIZON_DAYS]` where some entry starts or stops.
fn boundaries(doc: &ScheduleDoc, t: NaiveDateTime) -> Vec<NaiveDateTime> {
    let horizon = t + Duration::days(HORIZON_DAYS);
    let mut out = Vec::new();
    for w in doc.schedules.weekly.values().filter(|w| w.enable) {
        let (Ok(start), Ok(end)) = (minutes_of(&w.start), minutes_of(&w.end)) else {
            continue;
        };
        for offset in 0..=HORIZON_DAYS {
            let date = t.date() + Duration::days(offset);
            if !w
                .days
                .iter()
                .any(|d| weekday_from_name(d) == Some(date.weekday()))
            {
                continue;
            }
            let midnight = date.and_hms_opt(0, 0, 0).expect("midnight exists");
            out.push(midnight + Duration::minutes(i64::from(start)));
            // Inclusive end: the entry stops at the start of the next minute.
            out.push(midnight + Duration::minutes(i64::from(end) + 1));
        }
    }
    let years: Vec<i32> = (t.year() - 1..=horizon.year()).collect();
    let s = &doc.schedules;
    for e in s.events.values().chain(s.exception.values()) {
        for (start, end) in occurrences(e, &years) {
            out.push(start);
            out.push(end);
        }
    }
    out.retain(|p| *p > t && *p <= horizon);
    out
}
