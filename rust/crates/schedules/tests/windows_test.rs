//! A dated entry's concrete windows, including yearly copies.

use chrono::{NaiveDate, NaiveDateTime};
use lb_schedules::{occurrences, DatedEntry};
use serde_json::json;

fn t(y: i32, m: u32, d: u32, h: u32, mi: u32) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(y, m, d)
        .unwrap()
        .and_hms_opt(h, mi, 0)
        .unwrap()
}

fn entry(v: serde_json::Value) -> DatedEntry {
    serde_json::from_value(v).unwrap()
}

#[test]
fn a_one_off_entry_keeps_its_own_dates() {
    let e = entry(json!({ "name": "Occupied", "kind": "changed",
        "dates": [{ "start": "2026-11-10T06:30", "end": "2026-11-10T10:00" }] }));
    assert_eq!(
        occurrences(&e, &[2030]),
        vec![(t(2026, 11, 10, 6, 30), t(2026, 11, 10, 10, 0))]
    );
}

#[test]
fn a_yearly_entry_copies_into_each_year_from_its_first() {
    let e = entry(
        json!({ "name": "Occupied", "kind": "closed", "yearly": true,
        "dates": [{ "start": "2026-12-24T00:00", "end": "2027-01-03T00:00" }] }),
    );
    assert_eq!(
        occurrences(&e, &[2025, 2026, 2027]),
        vec![
            (t(2026, 12, 24, 0, 0), t(2027, 1, 3, 0, 0)),
            (t(2027, 12, 24, 0, 0), t(2028, 1, 3, 0, 0)),
        ]
    );
}

#[test]
fn a_29_february_start_is_skipped_in_a_short_year() {
    let e = entry(
        json!({ "name": "Occupied", "kind": "holiday", "yearly": true,
        "dates": [{ "start": "2028-02-29T00:00", "end": "2028-03-01T00:00" }] }),
    );
    assert_eq!(
        occurrences(&e, &[2028, 2029]),
        vec![(t(2028, 2, 29, 0, 0), t(2028, 3, 1, 0, 0))]
    );
}
