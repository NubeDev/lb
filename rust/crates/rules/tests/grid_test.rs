//! Grid plan-builders + reductions produce the expected composed query + columns, and the engine
//! classifies output. The recording seam captures the SurrealQL the engine composes.

mod support;

use std::collections::HashSet;
use std::sync::Arc;

use lb_rules::{AiLimits, GridJson, Rule, RuleEngine, RuleLimits, RuleOutput, RuleRun};
use support::{RecordingData, ScriptedAi};

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
    let ai = Arc::new(ScriptedAi {
        completion: "x".into(),
        tokens: 1,
        proposed_sql: "SELECT 1 AS v".into(),
    });
    let eng = RuleEngine::new(data.clone(), ai, RuleLimits::default(), AiLimits::default());
    let rule = Rule {
        workspace: "acme".into(),
        name: "adhoc".into(),
        body: body.into(),
        params: vec![],
    };
    let mut allow = HashSet::new();
    allow.insert("series".to_string());
    let mut rr = RuleRun::new("acme".into(), Arc::new(allow), rhai::Map::new());
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
