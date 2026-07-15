//! The `time` verb family through the REAL engine (data-stdlib-scope): rule bodies against a
//! pinned logical clock. Proves the four contracts — correct math for a pinned now, run-twice
//! byte-identical determinism, `timestamp()` disabled in the cage, and NO new authority (every
//! body here runs with an EMPTY source allowlist and no caps).

mod support;

use std::collections::HashSet;
use std::sync::Arc;

use lb_rules::{AiLimits, GridJson, Rule, RuleEngine, RuleLimits, RuleOutput, RuleRun};
use serde_json::{json, Value};
use support::{RecordingData, RecordingMessaging, ScriptedAi};

/// 2026-07-04T03:21:00Z — the scope doc's own example instant.
const NOW_SECS: i64 = 1_783_135_260;
/// The pinned clock in ms, with a sub-second remainder so `now()` proves the /1000 floor.
const NOW_MS: u64 = NOW_SECS as u64 * 1000 + 250;

/// Run a body through the real engine with the pinned clock and an EMPTY allowlist — the time
/// family must need no granted source and no capability.
fn run_pinned(body: &str, now_ms: u64) -> Result<RuleOutput, lb_rules::RuleError> {
    let data = Arc::new(RecordingData::platform(
        &[],
        GridJson {
            columns: vec![],
            rows: vec![],
        },
    ));
    let ai = Arc::new(ScriptedAi {
        completion: "x".into(),
        tokens: 1,
        proposed_sql: "SELECT 1 AS v".into(),
    });
    let messaging = Arc::new(RecordingMessaging::new());
    let eng = RuleEngine::new(
        data,
        ai,
        messaging,
        RuleLimits::default(),
        AiLimits::default(),
        32,
    );
    let rule = Rule {
        workspace: "acme".into(),
        name: "adhoc".into(),
        body: body.into(),
        params: vec![],
    };
    let mut rr = RuleRun::new(
        "acme".into(),
        Arc::new(HashSet::new()),
        rhai::Map::new(),
        now_ms,
    );
    eng.run(&rule, &mut rr)
}

fn scalar(out: RuleOutput) -> Value {
    match out {
        RuleOutput::Scalar(v) => v,
        other => panic!("expected a scalar output, got {other:?}"),
    }
}

#[test]
fn now_iso_add_floor_against_the_pinned_clock() {
    let out = run_pinned(
        r#"[
            time.now(),
            time.now_ms(),
            time.iso(time.now()),
            time.add(time.now(), "90m"),
            time.sub(time.now(), "7d"),
            time.floor(time.now(), "1h"),
            time.ceil(time.now(), "1h"),
        ]"#,
        NOW_MS,
    )
    .unwrap();
    assert_eq!(
        scalar(out),
        json!([
            NOW_SECS,               // now() = now_ms/1000, remainder floored
            NOW_MS as i64,          // now_ms() keeps the remainder
            "2026-07-04T03:21:00Z", // iso formats UTC
            NOW_SECS + 5400,        // +90m
            NOW_SECS - 7 * 86_400,  // -7d
            NOW_SECS - 21 * 60,     // floor 1h → 03:00:00
            NOW_SECS + 39 * 60,     // ceil 1h → 04:00:00
        ])
    );
}

#[test]
fn parts_bounds_and_parse_compose_in_a_body() {
    let out = run_pinned(
        r#"
            let ts = time.parse("2024-02-15T12:30:00Z");
            [
                time.weekday_name(ts),
                time.days_in_month(ts),
                time.is_leap_year(ts),
                time.iso(time.end_of_month(ts)),
                time.iso(time.start_of_week(ts)),
                time.parse("1609459200") == time.parse("1609459200000"),
                time.from_ymd(2021, 1, 1),
            ]
        "#,
        NOW_MS,
    )
    .unwrap();
    assert_eq!(
        scalar(out),
        json!([
            "Thursday",
            29, // leap February
            true,
            "2024-02-29T23:59:59Z",
            "2024-02-12T00:00:00Z",
            true, // epoch-secs and epoch-ms strings parse to the same instant
            1_609_459_200_i64,
        ])
    );
}

#[test]
fn ago_format_offset_and_dur_verbs() {
    let out = run_pinned(
        r#"[
            time.ago(time.sub(time.now(), "200m")),
            time.ago(time.add(time.now(), "200m")),
            time.format(time.now(), "%Y-%m-%d %H:%M", "+10:00"),
            dur_secs("24h"),
            dur_ms("15m"),
            dur_human(93900),
            [seconds(5), minutes(2), hours(1), days(1), weeks(1)],
        ]"#,
        NOW_MS,
    )
    .unwrap();
    assert_eq!(
        scalar(out),
        json!([
            "3h 20m ago",
            "in 3h 20m",
            "2026-07-04 13:21", // 03:21 UTC at +10:00
            86_400,
            900_000,
            "1d 2h 5m",
            [5, 120, 3600, 86_400, 604_800],
        ])
    );
}

/// Determinism is the contract: same body + same pinned now, twice → byte-identical output
/// (rules-messaging-scope; no wall-clock leaks into any time verb).
#[test]
fn same_body_and_now_twice_is_byte_identical() {
    let body = r#"
        let report = `${time.date(time.now())} ${time.clock(time.now())}Z `
            + `${time.ago(time.floor(time.sub(time.now(), "3h"), "15m"))} `
            + `${dur_human(time.since(time.start_of_day(time.now())))}`;
        report
    "#;
    let a = serde_json::to_string(&scalar(run_pinned(body, NOW_MS).unwrap())).unwrap();
    let b = serde_json::to_string(&scalar(run_pinned(body, NOW_MS).unwrap())).unwrap();
    assert_eq!(a, b, "re-run with the same now must be byte-identical");
    // And a different pinned now visibly changes the output (the clock IS the injected value).
    let c = serde_json::to_string(&scalar(run_pinned(body, NOW_MS + 3_600_000).unwrap())).unwrap();
    assert_ne!(a, c);
}

/// rhai's built-in `timestamp()` is a live wall-clock `Instant` — the sandbox disables it so the
/// only clock in the cage is the injected one.
#[test]
fn wall_clock_timestamp_is_disabled() {
    let err = run_pinned(r#"timestamp()"#, NOW_MS).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("timestamp() is disabled"),
        "expected the sandbox's disable message, got: {msg}"
    );
}

/// No new authority: every test above already runs with an empty allowlist, but this pins the
/// claim explicitly — time + duration verbs work while a data fetch in the SAME body is denied.
#[test]
fn time_verbs_need_no_allowlist_but_sources_still_do() {
    let ok = run_pinned(r#"time.iso(time.now())"#, NOW_MS).unwrap();
    assert_eq!(scalar(ok), json!("2026-07-04T03:21:00Z"));
    let err = run_pinned(r#"let t = time.now(); source("series")"#, NOW_MS).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("not allowed") || msg.contains("denied"),
        "the allowlist deny must still fire: {msg}"
    );
}

/// Author-feedback errors: a bad duration, a bad offset, and an unparseable string each surface a
/// clear message (never a panic, never a silent zero).
#[test]
fn author_errors_are_clear() {
    for (body, needle) in [
        (r#"time.add(time.now(), "10y")"#, "unknown unit"),
        (r#"time.format(time.now(), "%H:%M", "10:00")"#, "+HH:MM"),
        (r#"time.parse("not-a-date")"#, "cannot parse"),
        (r#"time.floor(time.now(), "0m")"#, "must be positive"),
        (r#"time.from_ymd(2023, 2, 29)"#, "invalid date"),
    ] {
        let msg = run_pinned(body, NOW_MS).unwrap_err().to_string();
        assert!(msg.contains(needle), "{body}: expected {needle:?} in {msg}");
    }
}
