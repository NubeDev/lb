//! Unit tests for `timerange/resolve.rs` — sibling test file per FILE-LAYOUT (the source file
//! sits near the 400-line ceiling, so its tests live here, over the same public API).

use chrono_tz::Tz;
use lb_host::timerange::{resolve, resolve_range, ResolvedRange, TimeRangeError};

/// 2026-07-29T10:30:00Z — a Wednesday.
const NOW: i64 = 1_785_321_000_000;

fn utc(from: &str, to: Option<&str>) -> ResolvedRange {
    resolve(from, to, NOW, Tz::UTC).unwrap()
}

/// The current-period tokens run from the START of the period to NOW — "so far this period".
#[test]
fn today_and_this_periods_end_at_now() {
    let today = utc("today", None);
    assert_eq!(today.to_ms, NOW);
    assert_eq!(today.from_day, "2026-07-29");
    for expr in [
        "this-hour",
        "this-day",
        "this-week",
        "this-month",
        "this-quarter",
        "this-year",
    ] {
        let r = utc(expr, None);
        assert_eq!(r.to_ms, NOW, "{expr} must end at now");
    }
    // The start of `today` is still midnight, not now.
    assert_eq!(today.from_ms, 1_785_283_200_000);
}

/// The scope's headline decision: `last-month` (previous whole calendar month) is NOT
/// `last-1-month` (a trailing month ending now).
#[test]
fn last_month_and_last_1_month_differ() {
    let cal = utc("last-month", None);
    assert_eq!(
        (cal.from_day.as_str(), cal.to_day.as_str()),
        ("2026-06-01", "2026-07-01")
    );
    let trailing = utc("last-1-month", None);
    assert_eq!(trailing.from_day, "2026-06-29");
    assert_eq!(trailing.to_ms, NOW);
}

/// 31 Mar − 1 trailing month = 28 Feb (clamped, the decided semantics).
#[test]
fn trailing_month_clamps_at_month_end() {
    let mar31 = 1_774_953_000_000; // 2026-03-31T10:30:00Z
    let r = resolve("last-1-month", None, mar31, Tz::UTC).unwrap();
    assert_eq!(r.from_day, "2026-02-28");
}

#[test]
fn this_year_runs_from_jan_1_to_now() {
    let r = utc("this-year", None);
    assert_eq!(r.from_day, "2026-01-01");
    assert_eq!(r.to_day, "2026-07-30"); // NOW is the 29th; the exclusive day-after projects past it
    assert_eq!(r.to_ms, NOW);
}

#[test]
fn weeks_start_monday_and_quarters_on_jan_apr_jul_oct() {
    let r = utc("this-week", None);
    assert_eq!(
        (r.from_day.as_str(), r.to_day.as_str()),
        ("2026-07-27", "2026-07-30")
    );
    assert_eq!(r.to_ms, NOW);
    let q = utc("this-quarter", None);
    assert_eq!(q.from_day, "2026-07-01");
    assert_eq!(q.to_ms, NOW);
    let lq = utc("last-quarter", None);
    assert_eq!(
        (lq.from_day.as_str(), lq.to_day.as_str()),
        ("2026-04-01", "2026-07-01")
    );
}

#[test]
fn endpoints_snap_and_default_to_now() {
    let r = utc("now-4h", None);
    assert_eq!(r.from_ms, NOW - 4 * 3_600_000);
    assert_eq!(r.to_ms, NOW);
    let s = utc("now-1d/d", Some("now/d"));
    assert_eq!(
        (s.from_day.as_str(), s.to_day.as_str()),
        ("2026-07-28", "2026-07-29")
    );
    assert_eq!(s.to_ms - s.from_ms, 86_400_000);
}

#[test]
fn iso_and_epoch_endpoints_resolve() {
    let r = utc("2026-07-01", Some("2026-08-01"));
    assert_eq!(
        (r.from_day.as_str(), r.to_day.as_str()),
        ("2026-07-01", "2026-08-01")
    );
    let e = utc("1785283200000", None);
    assert_eq!(e.from_ms, 1_785_283_200_000);
    let i = utc("2026-07-01T06:00:00Z", Some("now"));
    assert_eq!(i.from_ms, 1_782_885_600_000);
}

/// Windows are computed in the RANGE timezone — "today" in Sydney is not "today" in UTC.
#[test]
fn windows_are_timezone_local() {
    let sydney: Tz = "Australia/Sydney".parse().unwrap();
    // 2026-07-29T20:30:00Z = 2026-07-30T06:30 AEST (+10).
    let r = resolve("today", None, 1_785_357_000_000, sydney).unwrap();
    assert_eq!(
        (r.from_day.as_str(), r.to_day.as_str()),
        ("2026-07-30", "2026-07-31")
    );
}

/// The shape rules: a range token with `to`, a range token in `to`, and an inverted pair are
/// all loud refusals naming the offender.
#[test]
fn shape_violations_are_refused_loudly() {
    let e = resolve("this-month", Some("now"), NOW, Tz::UTC).unwrap_err();
    assert!(matches!(e, TimeRangeError::WindowWithTo { ref token } if token == "this-month"));
    let e = resolve("now-1d", Some("yesterday"), NOW, Tz::UTC).unwrap_err();
    assert!(matches!(e, TimeRangeError::WindowInTo { ref token } if token == "yesterday"));
    let e = resolve("now", Some("now-1d"), NOW, Tz::UTC).unwrap_err();
    assert!(matches!(e, TimeRangeError::Inverted { .. }));
}

/// The string-tz embedder form: empty = UTC, unknown = a normal refusal naming the value.
#[test]
fn string_tz_form_parses_and_refuses() {
    assert!(resolve_range("today", None, NOW, "").is_ok());
    assert!(resolve_range("today", None, NOW, "Australia/Sydney").is_ok());
    let e = resolve_range("today", None, NOW, "Mars/Olympus").unwrap_err();
    assert!(matches!(e, TimeRangeError::BadTimezone { ref tz } if tz == "Mars/Olympus"));
}
