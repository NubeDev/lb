//! The next wall time a schedule changes: on to off, off to on, or the value changing.
//! Calendar: Wed 4 Nov 2026, Fri 6 Nov, Mon 9 Nov, Tue 10 Nov.

use chrono::{NaiveDate, NaiveDateTime};
use lb_schedules::{next_change, ScheduleDoc};

fn offices() -> ScheduleDoc {
    serde_json::from_str(include_str!("fixtures/offices_doc.json")).unwrap()
}

fn t(y: i32, m: u32, d: u32, h: u32, mi: u32) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(y, m, d)
        .unwrap()
        .and_hms_opt(h, mi, 0)
        .unwrap()
}

#[test]
fn standby_to_occupied_is_a_change_of_value() {
    assert_eq!(
        next_change(&offices(), t(2026, 11, 4, 6, 45)),
        Some(t(2026, 11, 4, 7, 0))
    );
}

#[test]
fn occupied_ends_after_its_last_minute() {
    assert_eq!(
        next_change(&offices(), t(2026, 11, 4, 12, 0)),
        Some(t(2026, 11, 4, 18, 0))
    );
}

#[test]
fn the_weekend_is_skipped() {
    assert_eq!(
        next_change(&offices(), t(2026, 11, 6, 18, 0)),
        Some(t(2026, 11, 9, 6, 30))
    );
}

#[test]
fn an_exception_moves_the_next_start() {
    // Tue 10 Nov is closed 06:30 to 10:00, so Monday evening's next change is 10:00 Tuesday.
    assert_eq!(
        next_change(&offices(), t(2026, 11, 9, 18, 0)),
        Some(t(2026, 11, 10, 10, 0))
    );
}

#[test]
fn nothing_changes_in_an_empty_document() {
    assert_eq!(
        next_change(&ScheduleDoc::default(), t(2026, 11, 4, 9, 0)),
        None
    );
}
