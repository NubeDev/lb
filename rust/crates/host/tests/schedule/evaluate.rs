//! `schedule.evaluate` by the box's rules (design §4). Sydney is AEDT (+11:00) in November.

use chrono::{TimeZone, Utc};
use lb_host::ScheduleRecord;

use super::support::offices;

fn record() -> ScheduleRecord {
    serde_json::from_value(offices()).expect("offices is a valid record")
}

#[test]
fn occupied_hours_name_the_entry_and_its_value() {
    // 2026-11-03T20:00Z is Wednesday 4 Nov 07:00 in Sydney.
    let ev = record()
        .evaluate_at(Utc.with_ymd_and_hms(2026, 11, 3, 20, 0, 0).unwrap())
        .unwrap();
    assert!(ev.is_active);
    assert_eq!(ev.active_source, "weekly:w2");
    assert_eq!(ev.entry_name.as_deref(), Some("Occupied"));
    assert_eq!(ev.value, Some(1.0));
    assert!(ev.weekly_active);
    assert!(!ev.exception_active);
    assert_eq!(ev.priority, 1);
    // Occupied runs to 17:59 inclusive, so the next change is 18:00 Sydney, 07:00Z.
    assert_eq!(
        ev.next_transition.as_deref(),
        Some("2026-11-04T07:00:00+00:00")
    );
}

#[test]
fn standby_is_active_with_its_own_value() {
    // 2026-11-03T19:45Z is Wednesday 06:45 in Sydney.
    let ev = record()
        .evaluate_at(Utc.with_ymd_and_hms(2026, 11, 3, 19, 45, 0).unwrap())
        .unwrap();
    assert!(ev.is_active);
    assert_eq!(ev.active_source, "weekly:w1");
    assert_eq!(ev.entry_name.as_deref(), Some("Standby"));
    assert_eq!(ev.value, Some(2.0));
    assert_eq!(
        ev.next_transition.as_deref(),
        Some("2026-11-03T20:00:00+00:00")
    );
}

#[test]
fn a_changed_hours_exception_switches_the_schedule_off() {
    // 2026-11-09T21:00Z is Tuesday 10 Nov 08:00 in Sydney, inside x1 (06:30 to 10:00).
    let ev = record()
        .evaluate_at(Utc.with_ymd_and_hms(2026, 11, 9, 21, 0, 0).unwrap())
        .unwrap();
    assert!(!ev.is_active);
    assert!(ev.exception_active);
    assert!(ev.weekly_active);
    assert_eq!(ev.active_source, "none");
    assert_eq!(ev.entry_name, None);
    assert_eq!(ev.value, None);
    assert_eq!(
        ev.next_transition.as_deref(),
        Some("2026-11-09T23:00:00+00:00")
    );
}

#[test]
fn an_event_adds_hours_the_week_does_not_have() {
    // 2026-11-21T11:00Z is Saturday 21 Nov 22:00 in Sydney, inside e1 (21:00 to 23:00).
    let ev = record()
        .evaluate_at(Utc.with_ymd_and_hms(2026, 11, 21, 11, 0, 0).unwrap())
        .unwrap();
    assert!(ev.is_active);
    assert_eq!(ev.active_source, "event:e1");
    assert_eq!(ev.entry_name.as_deref(), Some("Occupied"));
    assert_eq!(ev.value, Some(1.0));
    assert!(!ev.weekly_active);
}

#[test]
fn a_disabled_schedule_is_off() {
    let mut r = record();
    r.enabled = false;
    let ev = r
        .evaluate_at(Utc.with_ymd_and_hms(2026, 11, 3, 20, 0, 0).unwrap())
        .unwrap();
    assert!(!ev.is_active);
    assert_eq!(ev.active_source, "disabled");
    assert_eq!(ev.entry_name, None);
    assert_eq!(ev.next_transition, None);
}

/// The next change is wall time in the record's zone, turned into UTC with the offset that holds AT
/// that wall time. Sydney goes from +10:00 to +11:00 at 02:00 on Sunday 4 Oct 2026, so an entry that
/// started at 01:30 AEST and runs to 03:59 ends at 04:00 AEDT, 17:00Z (not 18:00Z, the old offset).
#[test]
fn next_transition_uses_the_offset_after_a_dst_change() {
    let r: ScheduleRecord = serde_json::from_value(serde_json::json!({
        "id": "dst", "timezone": "Australia/Sydney",
        "schedule": { "schedules": { "weekly": { "w1": {
            "name": "Default", "days": ["sunday"], "start": "01:00", "end": "03:59"
        } } } }
    }))
    .unwrap();
    // 2026-10-03T15:30Z is Sunday 4 Oct 01:30 AEST.
    let ev = r
        .evaluate_at(Utc.with_ymd_and_hms(2026, 10, 3, 15, 30, 0).unwrap())
        .unwrap();
    assert!(ev.is_active);
    assert_eq!(
        ev.next_transition.as_deref(),
        Some("2026-10-03T17:00:00+00:00")
    );
}
