//! What a schedule runs at one local wall time, by the box's rules (rubix-os
//! `services/schedule/schedule.go`, design §4):
//!
//! 1. weekly entries and events are OR'd;
//! 2. exceptions are applied last and switch the schedule off for their windows;
//! 3. where entries overlap, the one that started earliest gives the name and value; on the same
//!    minute a weekly entry beats an event (as on the box), then keys go in natural order
//!    (`key_order.rs` says where this comes from);
//! 4. weekly ends are inclusive to `:59`, dated ends exact, `yearly` entries repeat every year;
//! 5. disabled weekly entries are off. (A disabled schedule is the caller's check.)

use chrono::{Datelike, NaiveDateTime, Timelike};

use crate::doc::{DatedEntry, ScheduleDoc, WeeklyEntry};
use crate::key_order::natural_cmp;
use crate::parse::{minutes_of, weekday_from_name};
use crate::windows::occurrences;

/// The running entry: `source` is `weekly:<key>` or `event:<key>`.
#[derive(Debug, Clone, PartialEq)]
pub struct Running {
    pub source: String,
    pub name: String,
    pub value: f64,
}

/// The state at one wall time. `weekly_active` holds even when an exception switches it off.
#[derive(Debug, Clone, PartialEq)]
pub struct ScheduleNow {
    pub running: Option<Running>,
    pub weekly_active: bool,
    pub exception_active: bool,
}

pub fn evaluate_local(doc: &ScheduleDoc, t: NaiveDateTime) -> ScheduleNow {
    let s = &doc.schedules;
    // A window that started last year (a shutdown over New Year) still covers `t`.
    let years = [t.year() - 1, t.year(), t.year() + 1];
    let exception_active = s
        .exception
        .values()
        .any(|e| started_at(e, &years, t).is_some());
    let weekly: Vec<_> = s
        .weekly
        .iter()
        .filter_map(|(k, w)| {
            weekly_started_at(w, t).map(|at| (at, k.as_str(), "weekly", w.name.as_str(), w.value))
        })
        .collect();
    let weekly_active = !weekly.is_empty();
    let events = s.events.iter().filter_map(|(k, e)| {
        started_at(e, &years, t).map(|at| (at, k.as_str(), "event", e.name.as_str(), e.value))
    });
    let running = if exception_active {
        None
    } else {
        weekly
            .into_iter()
            .chain(events)
            .min_by(|a, b| {
                a.0.cmp(&b.0)
                    .then_with(|| (a.2 == "event").cmp(&(b.2 == "event")))
                    .then_with(|| natural_cmp(a.1, b.1))
            })
            .map(|(_, key, map, name, value)| Running {
                source: format!("{map}:{key}"),
                name: name.to_string(),
                value,
            })
    };
    ScheduleNow {
        running,
        weekly_active,
        exception_active,
    }
}

/// When today's run of `w` started, if `w` covers `t` (end inclusive to the last second).
fn weekly_started_at(w: &WeeklyEntry, t: NaiveDateTime) -> Option<NaiveDateTime> {
    if !w.enable
        || !w
            .days
            .iter()
            .any(|d| weekday_from_name(d) == Some(t.weekday()))
    {
        return None;
    }
    let (start, end) = (minutes_of(&w.start).ok()?, minutes_of(&w.end).ok()?);
    let minute = t.hour() * 60 + t.minute();
    (start <= minute && minute <= end)
        .then(|| t.date().and_hms_opt(start / 60, start % 60, 0))
        .flatten()
}

/// The earliest start among `e`'s windows that cover `t` (start inclusive, end exact).
fn started_at(e: &DatedEntry, years: &[i32], t: NaiveDateTime) -> Option<NaiveDateTime> {
    occurrences(e, years)
        .into_iter()
        .filter(|(start, end)| *start <= t && t < *end)
        .map(|(start, _)| start)
        .min()
}
