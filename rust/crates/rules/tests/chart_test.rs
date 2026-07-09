//! The chart-return helpers run through the REAL rule engine (rules-for-widgets-scope slice 3). Pure
//! compute over rows literals — no seam needed — so a rule's last line `timeseries(rows, "ts")` /
//! `wide(...)` / `category(...)` produces chart-ready rows. Proves registration + the timestamp
//! normalization across the shapes sources actually return.

mod support;

use std::collections::HashSet;
use std::sync::Arc;

use lb_rules::{AiLimits, Rule, RuleEngine, RuleError, RuleLimits, RuleOutput, RuleRun};
use serde_json::Value;
use support::{RecordingData, RecordingMessaging, ScriptedAi};

fn engine() -> RuleEngine {
    let data = Arc::new(RecordingData::platform(
        &["series"],
        lb_rules::GridJson {
            columns: vec![],
            rows: vec![],
        },
    ));
    let ai = Arc::new(ScriptedAi {
        completion: "ok".into(),
        tokens: 1,
        proposed_sql: "SELECT 1 AS v".into(),
    });
    let messaging = Arc::new(RecordingMessaging::new());
    RuleEngine::new(
        data,
        ai,
        messaging,
        RuleLimits::default(),
        AiLimits::default(),
        32,
    )
}

/// Run a body and return the scalar output as JSON rows (the array-of-maps a chart consumes).
fn scalar_rows(body: &str) -> Vec<Value> {
    let eng = engine();
    let rule = Rule {
        workspace: "acme".into(),
        name: "adhoc".into(),
        body: body.into(),
        params: vec![],
    };
    let mut run = RuleRun::new("acme".into(), Arc::new(HashSet::new()), rhai::Map::new(), 0);
    match eng.run(&rule, &mut run).expect("rule runs") {
        RuleOutput::Scalar(Value::Array(rows)) => rows,
        other => panic!("expected a scalar array output, got {other:?}"),
    }
}

fn run_err(body: &str) -> RuleError {
    let eng = engine();
    let rule = Rule {
        workspace: "acme".into(),
        name: "adhoc".into(),
        body: body.into(),
        params: vec![],
    };
    let mut run = RuleRun::new("acme".into(), Arc::new(HashSet::new()), rhai::Map::new(), 0);
    eng.run(&rule, &mut run).unwrap_err()
}

#[test]
fn timeseries_normalizes_and_sorts() {
    // Unsorted rows, epoch-secs timestamps → renamed `time`, promoted to ms, sorted ascending.
    let rows = scalar_rows(
        r#"
        let rows = [#{ ts: 20, v: 2 }, #{ ts: 10, v: 1 }, #{ ts: 30, v: 3 }];
        timeseries(rows, "ts")
        "#,
    );
    let times: Vec<i64> = rows.iter().map(|r| r["time"].as_i64().unwrap()).collect();
    assert_eq!(
        times,
        vec![10_000, 20_000, 30_000],
        "sorted + epoch-secs→ms"
    );
    // Value column preserved; `ts` renamed away.
    assert_eq!(rows[0]["v"], serde_json::json!(1));
    assert!(
        rows[0].get("ts").is_none(),
        "source ts column renamed to time"
    );
}

#[test]
fn timeseries_iso_string_shape() {
    let rows = scalar_rows(r#"timeseries([#{ ts: "1970-01-01T00:00:01Z", v: 9 }], "ts")"#);
    assert_eq!(
        rows[0]["time"],
        serde_json::json!(1000),
        "ISO-8601 → epoch-ms"
    );
}

#[test]
fn timeseries_trims_to_kept_columns() {
    let rows =
        scalar_rows(r#"timeseries([#{ ts: 1, keep_me: 5, drop_me: 9 }], "ts", ["keep_me"])"#);
    assert!(rows[0].get("keep_me").is_some(), "kept column stays");
    assert!(rows[0].get("time").is_some(), "time always kept");
    assert!(rows[0].get("drop_me").is_none(), "un-kept column trimmed");
}

#[test]
fn timeseries_missing_column_is_author_error() {
    let err = run_err(r#"timeseries([#{ v: 1 }], "ts")"#);
    assert!(
        matches!(err, RuleError::Eval(_)),
        "missing ts column → author error, got {err:?}"
    );
}

#[test]
fn wide_pivots_long_to_wide() {
    // Two timestamps × two series → 2 rows, each with time + both series columns.
    let rows = scalar_rows(
        r#"
        let rows = [
          #{ ts: 1, series: "a", value: 10 },
          #{ ts: 1, series: "b", value: 20 },
          #{ ts: 2, series: "a", value: 30 },
          #{ ts: 2, series: "b", value: 40 },
        ];
        wide(rows, "ts", "series", "value")
        "#,
    );
    assert_eq!(rows.len(), 2, "one row per distinct timestamp");
    assert_eq!(rows[0]["a"], serde_json::json!(10));
    assert_eq!(rows[0]["b"], serde_json::json!(20));
    assert_eq!(rows[1]["a"], serde_json::json!(30));
    // 3 columns: time + a + b.
    assert_eq!(rows[0].as_object().unwrap().len(), 3);
}

#[test]
fn category_trims_and_validates() {
    let rows =
        scalar_rows(r#"category([#{ name: "north", value: 5, extra: 9 }], "name", "value")"#);
    assert_eq!(
        rows[0].as_object().unwrap().len(),
        2,
        "trimmed to label + value"
    );
    assert_eq!(rows[0]["name"], serde_json::json!("north"));
    assert_eq!(rows[0]["value"], serde_json::json!(5));
}

#[test]
fn category_non_numeric_value_is_author_error() {
    let err = run_err(r#"category([#{ name: "x", value: "not-a-number" }], "name", "value")"#);
    assert!(
        matches!(err, RuleError::Eval(_)),
        "non-numeric value → author error, got {err:?}"
    );
}
