//! The `json` + `mathx` data-stdlib families through the REAL engine: rule BODIES over seeded rows
//! (the recording seam feeds the real cage — no mocks of node behavior), plus the scope's inverted
//! capability-deny proof: these verbs add NO new authority — a body using them runs green with an
//! EMPTY source allowlist and zero caps.

mod support;

use std::collections::HashSet;
use std::sync::Arc;

use lb_rules::{AiLimits, GridJson, Rule, RuleEngine, RuleLimits, RuleOutput, RuleRun};
use serde_json::{json, Value};
use support::{RecordingData, RecordingMessaging, ScriptedAi};

/// Seeded series rows in the three ts shapes sources actually return: an ISO-8601 string, an
/// epoch-ms number, and an epoch-secs number — exactly what `rows_epoch`/`epoch` must normalize.
fn seeded() -> Arc<RecordingData> {
    Arc::new(RecordingData::platform(
        &["series"],
        GridJson {
            columns: vec!["ts".into(), "status".into(), "value".into()],
            rows: vec![
                json!({ "ts": "2021-01-01T00:00:10Z", "status": "hot",  "value": 3.0 }),
                json!({ "ts": 1_600_000_000_000i64,   "status": "cold", "value": 7.0 }),
                json!({ "ts": 1_600_000_100i64,       "status": "hot",  "value": 9.0 }),
            ],
        },
    ))
}

/// Run a body through the real engine with the given granted sources (may be EMPTY — the
/// no-new-authority proof) and zero messaging caps.
fn run(data: Arc<RecordingData>, sources: &[&str], body: &str) -> RuleOutput {
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
    let allow: HashSet<String> = sources.iter().map(|s| s.to_string()).collect();
    let mut rr = RuleRun::new("acme".into(), Arc::new(allow), rhai::Map::new(), 0);
    eng.run(&rule, &mut rr).unwrap()
}

fn scalar(out: RuleOutput) -> Value {
    match out {
        RuleOutput::Scalar(v) => v,
        other => panic!("expected a scalar output, got {other:?}"),
    }
}

/// The scope's daily chain over seeded records: normalize the ts column across its three source
/// shapes, filter by field equality, pluck the column — `rows_epoch` → `where_eq` → `pluck`.
#[test]
fn body_chains_rows_epoch_where_eq_pluck_over_seeded_records() {
    let out = run(
        seeded(),
        &["series"],
        r#"
            let rows = rows_epoch(source("series").records(), "ts");
            let hot = where_eq(rows, "status", "hot");
            pluck(hot, "ts")
        "#,
    );
    // 2021-01-01T00:00:10Z → 1_609_459_210; the epoch-secs row passes through unchanged.
    assert_eq!(scalar(out), json!([1_609_459_210i64, 1_600_000_100i64]));
}

/// `epoch()` (the scalar normalizer) agrees with the column form inside a body.
#[test]
fn body_epoch_normalizes_a_plucked_iso_string() {
    let out = run(
        seeded(),
        &["series"],
        r#"epoch(pluck(source("series").records(), "ts")[0])"#,
    );
    assert_eq!(scalar(out), json!(1_609_459_210i64));
}

/// mathx in a body: map a raw reading onto a 0..100 scale and round it for the report.
#[test]
fn body_uses_mathx_round_to_and_map_range() {
    let out = run(
        seeded(),
        &[],
        r#"round_to(map_range(7.5, 0.0, 10.0, 0.0, 100.0), 1)"#,
    );
    assert_eq!(scalar(out), json!(75.0));
}

/// The scope's inverted capability-deny test: a body using ONLY json/mathx verbs runs green with
/// an EMPTY allowlist and zero caps — the whole family is pure compute below the capability line.
#[test]
fn json_and_mathx_run_with_empty_allowlist_and_zero_caps() {
    let data = Arc::new(RecordingData::platform(
        &[],
        GridJson {
            columns: vec![],
            rows: vec![],
        },
    ));
    let out = run(
        data,
        &[],
        r#"
            let v = parse_json("{\"a\":{\"b\":[10,20,30]},\"drop\":1}");
            let v = jset(v, "a.c", 5);
            let v = merge(v, #{ drop: () });
            let flat = flatten(v, ".");
            [
                jget(v, "a.b[1]"),
                jget(v, "missing.path", -1),
                flat["a.c"],
                thing_id("sensor:⟨ab-1⟩"),
                sign(-3.5),
                safe_div(4.0, 0.0, -1.0),
                to_json(pick(v, ["drop"]))
            ]
        "#,
    );
    assert_eq!(scalar(out), json!([20, -1, 5, "ab-1", -1, -1.0, "{}"]));
}

/// The row verbs shape a report end-to-end: group/count/sort over seeded records.
#[test]
fn body_groups_counts_and_sorts_rows() {
    let out = run(
        seeded(),
        &["series"],
        r#"
            let rows = source("series").records();
            let by_status = count_by(rows, "status");
            let top = sort_by(rows, "value", true);
            [by_status.hot, by_status.cold, top[0].value]
        "#,
    );
    assert_eq!(scalar(out), json!([2, 1, 9.0]));
}

/// Both families carry their catalog rows (rules.help / autocomplete read these).
#[test]
fn catalog_carries_the_json_and_mathx_families() {
    let json_rows = lb_rules::CATALOG
        .iter()
        .filter(|e| e.family == "json")
        .count();
    let mathx_rows = lb_rules::CATALOG
        .iter()
        .filter(|e| e.family == "mathx")
        .count();
    assert_eq!(
        json_rows, 24,
        "one row per json verb (overload arities share a row)"
    );
    assert_eq!(
        mathx_rows, 12,
        "one row per mathx verb (overload arities share a row)"
    );
    for e in lb_rules::CATALOG
        .iter()
        .filter(|e| e.family == "json" || e.family == "mathx")
    {
        assert!(
            e.description.ends_with('.'),
            "description of {} must end with '.'",
            e.name
        );
    }
}
