//! Grid plan-builders + reductions produce the expected composed query + columns, and the engine
//! classifies output. The recording seam captures the SurrealQL the engine composes.

mod support;

use std::collections::HashSet;
use std::sync::Arc;

use lb_rules::{AiLimits, GridJson, Rule, RuleEngine, RuleLimits, RuleOutput, RuleRun};
use serde_json::Value;
use support::{RecordingData, RecordingMessaging, ScriptedAi};

fn seeded() -> Arc<RecordingData> {
    Arc::new(RecordingData::platform(
        &["series"],
        GridJson {
            columns: vec!["ts".into(), "value".into()],
            rows: vec![
                serde_json::json!({ "ts": 1, "value": 3.0 }),
                serde_json::json!({ "ts": 2, "value": 7.0 }),
            ],
        },
    ))
}

fn run(
    data: Arc<RecordingData>,
    body: &str,
) -> (RuleOutput, lb_rules::RuleRun, Arc<RecordingData>) {
    run_with(data, "series", body)
}

/// Same as `run()` but lets a test name the granted source (the federation-shape tests use
/// `"warehouse"` instead of the platform `"series"` fixture).
fn run_with(
    data: Arc<RecordingData>,
    source: &str,
    body: &str,
) -> (RuleOutput, lb_rules::RuleRun, Arc<RecordingData>) {
    let ai = Arc::new(ScriptedAi {
        completion: "x".into(),
        tokens: 1,
        proposed_sql: "SELECT 1 AS v".into(),
    });
    let messaging = Arc::new(RecordingMessaging::new());
    let eng = RuleEngine::new(
        data.clone(),
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
    let mut allow = HashSet::new();
    allow.insert(source.to_string());
    let mut rr = RuleRun::new("acme".into(), Arc::new(allow), rhai::Map::new(), 0);
    let out = eng.run(&rule, &mut rr).unwrap();
    (out, rr, data)
}

#[test]
fn filter_composes_a_where_subquery() {
    let (_out, _rr, data) = run(
        seeded(),
        r#"source("series").filter("value > 5").records()"#,
    );
    let q = data.last_query().unwrap();
    assert!(q.contains("WHERE value > 5"), "query was: {q}");
    assert!(q.contains("SELECT * FROM `series`"), "query was: {q}");
}

#[test]
fn rollup_buckets_with_time_group() {
    let (_out, _rr, data) = run(
        seeded(),
        r#"source("series").rollup("1h", "max").records()"#,
    );
    let q = data.last_query().unwrap();
    assert!(q.contains("time::group(ts, 1h)"), "query was: {q}");
    assert!(q.contains("math::max(value) AS `max`"), "query was: {q}");
}

#[test]
fn col_max_reduces_to_a_scalar() {
    let (out, _rr, _data) = run(seeded(), r#"source("series").col("value").max()"#);
    // The seeded collect returns the rows; reduction shape is exercised by the composed SQL.
    assert!(matches!(out, RuleOutput::Scalar(_)), "got {out:?}");
}

#[test]
fn returning_grid_materializes() {
    let (out, _rr, _data) = run(seeded(), r#"source("series")"#);
    match out {
        RuleOutput::Grid(g) => assert_eq!(g.rows.len(), 2),
        other => panic!("expected grid, got {other:?}"),
    }
}

#[test]
fn emit_classifies_as_findings() {
    let (out, rr, _data) = run(seeded(), r#"emit(#{ level: "warning", msg: "hi" });"#);
    assert!(matches!(out, RuleOutput::Findings), "got {out:?}");
    assert_eq!(rr.findings.len(), 1);
    assert_eq!(rr.findings[0].level, "warning");
    assert!(!rr.findings[0].is_alert());
}

#[test]
fn alert_marks_finding_for_routing() {
    let (_out, rr, _data) = run(seeded(), r#"alert(#{ level: "critical", msg: "hot" });"#);
    assert_eq!(rr.findings.len(), 1);
    assert!(rr.findings[0].is_alert());
}

#[test]
fn empty_body_is_nothing() {
    let (out, _rr, _data) = run(seeded(), r#"let x = 1;"#);
    assert!(matches!(out, RuleOutput::Nothing), "got {out:?}");
}

/// Federation's wire shape is column-aligned ARRAYS (`crates/federation/src/query.rs::shape`
/// re-projects Arrow objects to `[v, …]`), not keyed objects. `records()` must collapse those to named
/// maps at the seam boundary so the catalog's `Array<Map>` contract holds on every source kind —
/// otherwise the documented `category(query(...).records(), ...)` one-liner fails on the federation
/// path with "every row must be a record". This pins the contract on the federation shape directly.
#[test]
fn records_returns_named_maps_from_federation_positional_rows() {
    let data = Arc::new(RecordingData::federation(
        &["warehouse"],
        GridJson {
            columns: vec!["building".into(), "kwh".into()],
            rows: vec![
                serde_json::json!(["Riverside", 4.68]),
                serde_json::json!(["Westend", 0.79]),
            ],
        },
    ));
    // `r.building` (named field access) works only if `records()` returned a map; on positional
    // rows it would be `Unknown property 'building' for type 'array'`.
    let (out, _rr, _data) = run_with(
        data,
        "warehouse",
        r#"let rows = source("warehouse").records(); rows[0].building"#,
    );
    match out {
        RuleOutput::Scalar(v) => assert_eq!(v, serde_json::json!("Riverside")),
        other => panic!("expected the named field to resolve, got {other:?}"),
    }
}

/// The chart helpers require maps; on the federation path that used to fail. With `records()`
/// returning maps, the documented one-liner `category(query(...).records(), ...)` works end-to-end
/// on federation data — the slice-3 (rules-for-widgets) promise, pinned at the unit layer.
#[test]
fn category_runs_on_federation_records() {
    let data = Arc::new(RecordingData::federation(
        &["warehouse"],
        GridJson {
            columns: vec!["building".into(), "kwh".into()],
            rows: vec![
                serde_json::json!(["Riverside", 4.68]),
                serde_json::json!(["Westend", 0.79]),
            ],
        },
    ));
    let (out, _rr, _data) = run_with(
        data,
        "warehouse",
        r#"let rows = source("warehouse").records(); category(rows, "building", "kwh")"#,
    );
    match out {
        RuleOutput::Scalar(Value::Array(rows)) => {
            assert_eq!(rows.len(), 2, "one trimmed row per source row");
            assert_eq!(rows[0]["building"], serde_json::json!("Riverside"));
            assert_eq!(rows[0]["kwh"], serde_json::json!(4.68));
            assert_eq!(
                rows[0].as_object().unwrap().len(),
                2,
                "category trimmed to label + value only"
            );
        }
        other => panic!("expected a scalar array from category(), got {other:?}"),
    }
}
