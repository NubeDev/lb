//! The **time-range conformance fixture** (relative-time-range scope, build step 8) — the one
//! artefact that keeps the host resolver and the downstream TypeScript twin from drifting
//! silently. The test RE-GENERATES `docs/contracts/time-range-conformance.json` from the real
//! resolver and asserts it equals the committed file, so any semantic change is a red test here
//! and a red vitest downstream (which vendors the same file). Regenerate deliberately with:
//!
//! ```sh
//! UPDATE_CONFORMANCE=1 cargo test -p lb-host --test timerange_conformance_test
//! ```
//!
//! The rows span every token family, both `last-month` spellings, the 29-Feb-2028 leap day,
//! 31 Mar → `last-1-month` (the clamp), 1 Jan → `last-month` (the year boundary), the
//! Australia/Sydney 2026 spring-forward (4 Oct) and fall-back (5 Apr), a Monday-start week, the
//! snap suffixes, and the ISO day / ISO instant / epoch-ms endpoints.

use lb_host::timerange::resolve_range;
use serde_json::{json, Value};

/// The committed fixture, relative to this crate's manifest (rust/crates/host → the repo root).
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../docs/contracts/time-range-conformance.json"
);

/// Epoch ms of an RFC3339 instant (test-side sugar for naming the fixed clocks).
fn ms(s: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(s)
        .expect("fixture clock parses")
        .timestamp_millis()
}

/// One row spec: (from-expr, to-expr?, nowMs, tz).
fn rows() -> Vec<(&'static str, Option<&'static str>, i64, &'static str)> {
    // Wed 2026-07-29 10:30 UTC — the workhorse clock (mid-week, mid-month, mid-year).
    let n = ms("2026-07-29T10:30:00Z");
    // The pinned edge clocks.
    let leap = ms("2028-02-29T12:00:00Z"); // a leap day
    let mar31 = ms("2026-03-31T12:00:00Z"); // month-end clamp
    let jan = ms("2027-01-15T09:00:00Z"); // year boundary
    let syd_spring = ms("2026-10-04T10:00:00+11:00"); // after the 2026-10-04 02:00→03:00 jump
    let syd_fall = ms("2026-04-05T12:00:00+10:00"); // after the 2026-04-05 03:00→02:00 repeat
    let syd = "Australia/Sydney";

    let mut rows: Vec<(&str, Option<&str>, i64, &str)> = Vec::new();
    // Endpoints: now, offsets in every unit class, snaps in every unit.
    for from in [
        "now", "now-4h", "now-30m", "now-90s", "now-1d", "now-2w", "now-1M", "now-1y", "now/d",
        "now/w", "now/M", "now/y", "now-1d/d", "now-7d/d", "now-1M/M", "now-1y/y", "now-4h/h",
        "now/m",
    ] {
        rows.push((from, None, n, "UTC"));
    }
    // Future-pointing endpoints need an explicit pair (a bare future `from` would invert on now).
    rows.push(("now", Some("now+2h"), n, "UTC"));
    rows.push(("now/M", Some("now+1M/M"), n, "UTC"));
    // Range tokens: the day trio + this/last/next over every calendar unit.
    for from in [
        "today",
        "yesterday",
        "tomorrow",
        "this-hour",
        "last-hour",
        "next-hour",
        "this-day",
        "last-day",
        "this-week",
        "last-week",
        "next-week",
        "this-month",
        "last-month",
        "next-month",
        "this-quarter",
        "last-quarter",
        "next-quarter",
        "this-year",
        "last-year",
        "next-year",
    ] {
        rows.push((from, None, n, "UTC"));
    }
    // Counted trailing windows, long + short spellings — and BOTH last-month spellings side by side.
    for from in [
        "last-1-month",
        "last-3-months",
        "last-7-days",
        "last-30-days",
        "last-6-hours",
        "last-90-minutes",
        "last-2-weeks",
        "last-1-year",
        "last-2-quarters",
        "last-12h",
        "last-2w",
        "last-45m",
        "last-1d",
    ] {
        rows.push((from, None, n, "UTC"));
    }
    // Explicit endpoint pairs: ISO days, ISO instants, epoch ms, snapped now-pairs.
    rows.push(("2026-07-01", Some("2026-08-01"), n, "UTC"));
    rows.push(("2026-07-01", Some("2026-08-01"), n, syd));
    rows.push((
        "2026-07-01T06:00:00Z",
        Some("2026-07-02T06:00:00Z"),
        n,
        "UTC",
    ));
    rows.push(("1785283200000", None, n, "UTC"));
    rows.push(("now-1d/d", Some("now/d"), n, "UTC"));
    rows.push(("now-7d/d", Some("now/d"), n, syd));
    // The leap day.
    for from in ["today", "this-month", "last-month", "last-1-month"] {
        rows.push((from, None, leap, "UTC"));
    }
    rows.push(("now", Some("now+1d"), leap, "UTC"));
    // 31 Mar: the trailing-month clamp (→ 28 Feb) beside the calendar month.
    for from in ["last-1-month", "now-1M", "last-month"] {
        rows.push((from, None, mar31, "UTC"));
    }
    // January: the year boundary.
    for from in ["last-month", "last-quarter", "this-year", "last-1-month"] {
        rows.push((from, None, jan, "UTC"));
    }
    // Sydney spring-forward (2026-10-04, 02:00→03:00: a 23-hour day) — these rows DISCRIMINATE
    // the offset split (the moment.js rule Grafana inherits): `d`/`w`/`M`/`y` are
    // calendar-anchored and wall-clock preserving (now-1d spans 23 REAL hours here; now-1w spans
    // 167), while `s`/`m`/`h` are exact fixed-width ms (now-4h is 4×3 600 000 even across the
    // jump). `today`/`this-week` now end at NOW — they show the honest time since the LOCAL
    // midnight / the LOCAL Monday week.
    for from in [
        "today",
        "now-1d",
        "now-1w",
        "this-week",
        "last-1-day",
        "this-month",
    ] {
        rows.push((from, None, syd_spring, syd));
    }
    // …and a `now-4h` whose window actually CROSSES the 02:00 jump (05:00 AEDT − 4h → 01:00 AEST).
    let syd_spring_cross = ms("2026-10-04T05:00:00+11:00");
    rows.push(("now-4h", None, syd_spring_cross, syd));
    // Sydney fall-back (2026-04-05, 03:00→02:00: a 25-hour day) — the same discrimination in the
    // other direction (now-1d spans 25 real hours; now-1w spans 169; now-4h stays exactly 4h).
    for from in ["today", "now-1d", "now-1w", "last-6-hours"] {
        rows.push((from, None, syd_fall, syd));
    }
    let syd_fall_cross = ms("2026-04-05T04:00:00+10:00");
    rows.push(("now-4h", None, syd_fall_cross, syd));
    rows
}

/// The whole table, generated from the REAL resolver — the same call every consumer makes.
fn generate() -> Value {
    let table: Vec<Value> = rows()
        .into_iter()
        .map(|(from, to, now_ms, tz)| {
            let r = resolve_range(from, to, now_ms, tz)
                .unwrap_or_else(|e| panic!("conformance row {from:?} must resolve: {e}"));
            let mut row = json!({
                "expr": from,
                "nowMs": now_ms,
                "tz": tz,
                "fromMs": r.from_ms,
                "toMs": r.to_ms,
                "fromIso": r.from_day,
                "toIso": r.to_day,
            });
            if let Some(to) = to {
                row["to"] = json!(to);
            }
            row
        })
        .collect();
    json!(table)
}

/// Drift is a red test: the committed fixture must equal what the resolver produces today.
#[test]
fn conformance_fixture_matches_the_resolver() {
    let generated = format!("{}\n", serde_json::to_string_pretty(&generate()).unwrap());
    if std::env::var("UPDATE_CONFORMANCE").is_ok() {
        std::fs::write(FIXTURE, &generated).expect("write fixture");
        return;
    }
    let committed = std::fs::read_to_string(FIXTURE).unwrap_or_else(|_| {
        panic!("missing {FIXTURE}; generate it with UPDATE_CONFORMANCE=1 cargo test -p lb-host --test timerange_conformance_test")
    });
    assert_eq!(
        committed, generated,
        "time-range conformance fixture drifted from the resolver — review the semantic change; \
         regenerate deliberately with UPDATE_CONFORMANCE=1 if intended (the downstream TS twin \
         asserts the same file)"
    );
}

/// The fixture keeps its promised coverage: both `last-month` spellings at the same clock, the
/// leap day, the clamp, the year boundary, and both Sydney DST transitions.
#[test]
fn conformance_fixture_spans_the_pinned_edges() {
    let table = generate();
    let rows = table.as_array().unwrap();
    assert!(rows.len() >= 60, "~60 rows promised, got {}", rows.len());
    let has = |expr: &str, tz: &str| rows.iter().any(|r| r["expr"] == expr && r["tz"] == tz);
    assert!(has("last-month", "UTC") && has("last-1-month", "UTC"));
    assert!(has("today", "Australia/Sydney"));
    // `today` ends at NOW, not at the day's end — so its width is the honest time since midnight,
    // on a 23h spring day no less a 23h window than on a 25h fall day. The day's own 23h/25h
    // widths are still pinned by the `now-1d` rows below.
    let spring_today = rows
        .iter()
        .find(|r| r["expr"] == "today" && r["nowMs"] == json!(ms("2026-10-04T10:00:00+11:00")))
        .unwrap();
    assert_eq!(
        spring_today["toMs"].as_i64().unwrap(),
        ms("2026-10-04T10:00:00+11:00"),
        "today ends at now"
    );
    assert_eq!(spring_today["fromIso"], "2026-10-04");
    let fall_today = rows
        .iter()
        .find(|r| r["expr"] == "today" && r["nowMs"] == json!(ms("2026-04-05T12:00:00+10:00")))
        .unwrap();
    assert_eq!(
        fall_today["toMs"].as_i64().unwrap(),
        ms("2026-04-05T12:00:00+10:00"),
        "today ends at now"
    );
    // The offset split across DST (the moment.js rule): day/week offsets are calendar-anchored
    // (23h / 167h across the spring-forward, 25h / 169h across the fall-back), hour offsets are
    // exact fixed-width even when the window crosses the transition.
    let width = |expr: &str, now: i64| {
        let r = rows
            .iter()
            .find(|r| r["expr"] == expr && r["nowMs"] == json!(now))
            .unwrap_or_else(|| panic!("row {expr} @ {now} present"));
        r["toMs"].as_i64().unwrap() - r["fromMs"].as_i64().unwrap()
    };
    let spring = ms("2026-10-04T10:00:00+11:00");
    let fall = ms("2026-04-05T12:00:00+10:00");
    assert_eq!(
        width("now-1d", spring),
        23 * 3_600_000,
        "now-1d is wall-clock anchored"
    );
    assert_eq!(
        width("now-1w", spring),
        167 * 3_600_000,
        "now-1w is wall-clock anchored"
    );
    assert_eq!(width("now-1d", fall), 25 * 3_600_000);
    assert_eq!(width("now-1w", fall), 169 * 3_600_000);
    assert_eq!(
        width("now-4h", ms("2026-10-04T05:00:00+11:00")),
        4 * 3_600_000,
        "now-4h is exact fixed-width even across the spring-forward"
    );
    assert_eq!(
        width("now-4h", ms("2026-04-05T04:00:00+10:00")),
        4 * 3_600_000
    );

    // The Monday-start week: 2026-07-29 (a Wednesday) falls in the week of Monday 2026-07-27, and
    // `this-week` ends at NOW — the exclusive `toIso` is the day after the 29th.
    let week = rows
        .iter()
        .find(|r| r["expr"] == "this-week" && r["tz"] == "UTC")
        .unwrap();
    assert_eq!(week["fromIso"], "2026-07-27");
    assert_eq!(week["toIso"], "2026-07-30");
}
