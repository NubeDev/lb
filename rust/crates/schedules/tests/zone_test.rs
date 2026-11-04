//! The zone a schedule's wall times live in, and how DST edges map to instants.

use chrono::{NaiveDate, NaiveDateTime, TimeZone, Utc};
use lb_schedules::Zone;

fn local(y: i32, m: u32, d: u32, h: u32, mi: u32) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(y, m, d)
        .unwrap()
        .and_hms_opt(h, mi, 0)
        .unwrap()
}

fn sydney() -> Zone {
    Zone::new(false, "Australia/Sydney").unwrap()
}

#[test]
fn utc_when_use_utc_or_no_timezone() {
    assert_eq!(Zone::new(true, "Australia/Sydney").unwrap(), Zone::Utc);
    assert_eq!(Zone::new(false, "").unwrap(), Zone::Utc);
}

#[test]
fn an_unknown_zone_is_refused() {
    let e = Zone::new(false, "Mars/Olympus").unwrap_err();
    assert_eq!(e.to_string(), "unknown timezone: Mars/Olympus");
}

#[test]
fn wall_time_and_back_in_sydney() {
    // 2026-11-03T20:00Z is Wednesday 4 Nov 07:00 AEDT (+11:00).
    let z = sydney();
    assert_eq!(
        z.local(Utc.with_ymd_and_hms(2026, 11, 3, 20, 0, 0).unwrap()),
        local(2026, 11, 4, 7, 0)
    );
    assert_eq!(
        z.instant(local(2026, 11, 4, 7, 0)).to_rfc3339(),
        "2026-11-04T07:00:00+11:00"
    );
}

#[test]
fn a_skipped_wall_time_moves_to_the_first_minute_that_exists() {
    // Sydney clocks jump 02:00 to 03:00 on Sunday 4 October 2026.
    assert_eq!(
        sydney().instant(local(2026, 10, 4, 2, 30)).to_rfc3339(),
        "2026-10-04T03:00:00+11:00"
    );
}

#[test]
fn a_repeated_wall_time_takes_its_first_occurrence() {
    // Sydney clocks go back 03:00 to 02:00 on Sunday 5 April 2026, so 02:30 happens twice.
    assert_eq!(
        sydney().instant(local(2026, 4, 5, 2, 30)).to_rfc3339(),
        "2026-04-05T02:30:00+11:00"
    );
}
