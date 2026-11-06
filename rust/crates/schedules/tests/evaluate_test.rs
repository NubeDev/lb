//! Evaluation by the box's rules (design §4), on the design's "Offices" document.
//! Calendar: Wed 4 Nov 2026, Mon 9 Nov, Tue 10 Nov, Sat 21 Nov, Fri 1 Jan 2027, Tue 28 Dec 2027,
//! Tue 4 Jan 2028, Mon 22 Dec 2025.

use chrono::{NaiveDate, NaiveDateTime};
use lb_schedules::{evaluate_local, Running, ScheduleDoc};
use serde_json::json;

fn offices() -> ScheduleDoc {
    serde_json::from_str(include_str!("fixtures/offices_doc.json")).unwrap()
}

fn at(y: i32, m: u32, d: u32, h: u32, mi: u32, s: u32) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(y, m, d)
        .unwrap()
        .and_hms_opt(h, mi, s)
        .unwrap()
}

fn running(source: &str, name: &str, value: f64) -> Option<Running> {
    Some(Running {
        source: source.into(),
        name: name.into(),
        value,
    })
}

#[test]
fn standby_then_occupied_on_a_weekday() {
    let d = offices();
    assert_eq!(
        evaluate_local(&d, at(2026, 11, 4, 6, 45, 0)).running,
        running("weekly:w1", "Standby", 2.0)
    );
    assert_eq!(
        evaluate_local(&d, at(2026, 11, 4, 7, 0, 0)).running,
        running("weekly:w2", "Occupied", 1.0)
    );
}

#[test]
fn weekly_ends_are_inclusive_to_the_last_second() {
    let d = offices();
    assert_eq!(
        evaluate_local(&d, at(2026, 11, 4, 17, 59, 59)).running,
        running("weekly:w2", "Occupied", 1.0)
    );
    let after = evaluate_local(&d, at(2026, 11, 4, 18, 0, 0));
    assert_eq!(after.running, None);
    assert!(!after.weekly_active);
}

#[test]
fn exceptions_switch_the_schedule_off_and_dated_ends_are_exact() {
    let d = offices();
    let during = evaluate_local(&d, at(2026, 11, 10, 9, 59, 59));
    assert_eq!(during.running, None);
    assert!(during.weekly_active, "the week still covers 09:59");
    assert!(during.exception_active);
    let after = evaluate_local(&d, at(2026, 11, 10, 10, 0, 0));
    assert_eq!(after.running, running("weekly:w2", "Occupied", 1.0));
    assert!(!after.exception_active);
}

#[test]
fn events_add_hours_the_week_does_not_have() {
    let d = offices();
    let now = evaluate_local(&d, at(2026, 11, 21, 22, 0, 0));
    assert_eq!(now.running, running("event:e1", "Occupied", 1.0));
    assert!(!now.weekly_active);
    assert_eq!(evaluate_local(&d, at(2026, 11, 21, 23, 0, 0)).running, None);
}

#[test]
fn yearly_closures_repeat_including_across_new_year() {
    let d = offices();
    let base = evaluate_local(&d, at(2027, 1, 1, 9, 0, 0));
    assert_eq!(base.running, None);
    assert!(base.exception_active, "inside the 2026 shutdown");
    let next = evaluate_local(&d, at(2027, 12, 28, 9, 0, 0));
    assert_eq!(next.running, None);
    assert!(next.exception_active, "the 2027 copy");
    assert_eq!(
        evaluate_local(&d, at(2028, 1, 4, 9, 0, 0)).running,
        running("weekly:w2", "Occupied", 1.0)
    );
    let before = evaluate_local(&d, at(2025, 12, 22, 9, 0, 0));
    assert_eq!(before.running, running("weekly:w2", "Occupied", 1.0));
    assert!(
        !before.exception_active,
        "never before the entry's first year"
    );
}

#[test]
fn the_entry_that_started_first_wins_and_ties_go_to_the_lower_key() {
    let d: ScheduleDoc = serde_json::from_value(json!({
        "config": { "names": ["A", "B", "C"], "default_name": "A" },
        "schedules": {
            "weekly": {
                "a": { "name": "A", "days": ["monday"], "start": "07:00", "end": "17:59", "value": 1 },
                "b": { "name": "B", "days": ["monday"], "start": "06:00", "end": "06:59", "value": 3 },
                "c": { "name": "C", "days": ["monday"], "start": "07:00", "end": "08:59", "value": 4 }
            },
            "events": {
                "e": { "name": "C", "dates": [{ "start": "2026-11-08T23:00", "end": "2026-11-09T06:30" }],
                       "value": 5 }
            }
        }
    }))
    .unwrap();
    // The event started Sunday 23:00, before `b` at 06:00.
    assert_eq!(
        evaluate_local(&d, at(2026, 11, 9, 6, 15, 0)).running,
        running("event:e", "C", 5.0)
    );
    assert_eq!(
        evaluate_local(&d, at(2026, 11, 9, 6, 45, 0)).running,
        running("weekly:b", "B", 3.0)
    );
    // `a` and `c` both started 07:00: the key first in natural order wins.
    assert_eq!(
        evaluate_local(&d, at(2026, 11, 9, 8, 0, 0)).running,
        running("weekly:a", "A", 1.0)
    );
}

#[test]
fn on_the_same_minute_weekly_beats_an_event_and_keys_sort_naturally() {
    let d: ScheduleDoc = serde_json::from_value(json!({
        "config": { "names": ["A", "B", "C"], "default_name": "A" },
        "schedules": {
            "weekly": {
                "w10": { "name": "A", "days": ["monday"], "start": "07:00", "end": "17:59", "value": 1 },
                "w2": { "name": "B", "days": ["monday"], "start": "07:00", "end": "17:59", "value": 2 }
            },
            "events": {
                "a": { "name": "C", "dates": [{ "start": "2026-11-09T07:00", "end": "2026-11-09T09:00" }],
                       "value": 3 }
            }
        }
    }))
    .unwrap();
    assert_eq!(
        evaluate_local(&d, at(2026, 11, 9, 8, 0, 0)).running,
        running("weekly:w2", "B", 2.0)
    );
}

#[test]
fn disabled_weekly_entries_never_run() {
    let mut d = offices();
    d.schedules.weekly.get_mut("w2").unwrap().enable = false;
    let now = evaluate_local(&d, at(2026, 11, 4, 9, 0, 0));
    assert_eq!(now.running, None);
    assert!(!now.weekly_active);
}

#[test]
fn an_empty_document_is_off() {
    let now = evaluate_local(&ScheduleDoc::default(), at(2026, 11, 4, 9, 0, 0));
    assert_eq!(now.running, None);
    assert!(!now.weekly_active);
    assert!(!now.exception_active);
}
