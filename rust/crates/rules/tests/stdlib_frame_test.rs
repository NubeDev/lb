//! The polars `Frame` surface through the REAL engine (data-stdlib-scope Phase 2). The load-bearing
//! claims, asserted from a rule author's seat:
//!   - `frame(records)` composes a real pipeline (sort → filter → group_agg → records);
//!   - `g.frame()` materializes a Grid THROUGH the existing gated seam — so it grants nothing new:
//!     on a disallowed source it denies exactly as `g.records()` does (the same opaque deny);
//!   - `f.sql("… FROM self")` runs in-memory only and cannot reach the filesystem;
//!   - `max_frame_rows` aborts an over-cap materialization with a clear author error (the polars
//!     call is native — the wall-clock governor cannot interrupt it, so the INPUT cap is the bound).

mod support;

use std::collections::HashSet;
use std::sync::Arc;

use lb_rules::{AiLimits, GridJson, Rule, RuleEngine, RuleError, RuleLimits, RuleOutput, RuleRun};
use serde_json::{json, Value};
use support::{RecordingData, RecordingMessaging, ScriptedAi};

/// Four (series, value) rows behind the granted `series` source.
fn seeded() -> Arc<RecordingData> {
    Arc::new(RecordingData::platform(
        &["series"],
        GridJson {
            columns: vec!["series".into(), "value".into()],
            rows: vec![
                json!({ "series": "a", "value": 3.0 }),
                json!({ "series": "b", "value": 7.0 }),
                json!({ "series": "a", "value": 5.0 }),
                json!({ "series": "b", "value": 9.0 }),
            ],
        },
    ))
}

fn engine_with(limits: RuleLimits) -> RuleEngine {
    RuleEngine::new(
        seeded(),
        Arc::new(ScriptedAi {
            completion: "x".into(),
            tokens: 1,
            proposed_sql: "SELECT 1 AS v".into(),
        }),
        Arc::new(RecordingMessaging::new()),
        limits,
        AiLimits::default(),
        32,
    )
}

fn try_run(allow: &[&str], body: &str, limits: RuleLimits) -> Result<RuleOutput, RuleError> {
    let eng = engine_with(limits);
    let rule = Rule {
        workspace: "acme".into(),
        name: "adhoc".into(),
        body: body.into(),
        params: vec![],
    };
    let allow: HashSet<String> = allow.iter().map(|s| s.to_string()).collect();
    let mut rr = RuleRun::new("acme".into(), Arc::new(allow), rhai::Map::new(), 0);
    eng.run(&rule, &mut rr)
}

fn run_allowing(allow: &[&str], body: &str) -> RuleOutput {
    try_run(allow, body, RuleLimits::default()).expect("body runs green")
}

fn scalar(out: RuleOutput) -> Value {
    match out {
        RuleOutput::Scalar(v) => v,
        other => panic!("expected a scalar output, got {other:?}"),
    }
}

#[test]
fn frame_from_records_composes_a_pipeline() {
    // No source at all: pure local compute over author-supplied rows (zero authority).
    let out = scalar(run_allowing(
        &[],
        r#"
            let f = frame([
                #{ site: "north", kwh: 10.0 },
                #{ site: "south", kwh: 4.0 },
                #{ site: "north", kwh: 6.0 },
            ]);
            let by_site = f.filter_gt("kwh", 5.0)
                           .sort("kwh")
                           .group_agg(["site"], #{ kwh: "sum" });
            #{ rows: by_site.records(), tall: f.height(), wide: f.width() }
        "#,
    ));
    assert_eq!(out["tall"], json!(3));
    assert_eq!(out["wide"], json!(2));
    // Only the two >5 rows survive; both are `north`, summed.
    let rows = out["rows"].as_array().expect("group_agg returns rows");
    assert_eq!(rows.len(), 1, "one group survived the filter: {rows:?}");
    assert_eq!(rows[0]["site"], json!("north"));
    assert_eq!(rows[0]["kwh"], json!(16.0));
}

#[test]
fn grid_frame_materializes_through_the_seam_and_feeds_the_stats_family() {
    // `g.frame()` collects through the SAME gated seam `g.records()` uses, then `f.col()` hands a
    // plain array to the stats family — the documented "push down when big, compute local when
    // shaped" hand-off.
    let out = scalar(run_allowing(
        &["series"],
        r#"
            let f = source("series").frame();
            let vals = f.col("value");
            #{ n: f.height(), mean: mean(vals), max: f.max("value") }
        "#,
    ));
    assert_eq!(out["n"], json!(4));
    assert_eq!(out["mean"], json!(6.0));
    assert_eq!(out["max"], json!(9.0));
}

#[test]
fn grid_frame_on_a_disallowed_source_is_denied_exactly_like_records() {
    // THE no-new-authority proof: a Frame is not a back door. Same body, same source, one via
    // `.records()` and one via `.frame()` — both must deny opaquely, before any row is read.
    let records_err = try_run(
        &["series"],
        r#"source("payroll").records()"#,
        RuleLimits::default(),
    )
    .expect_err("records() on an ungranted source must deny");
    let frame_err = try_run(
        &["series"],
        r#"source("payroll").frame()"#,
        RuleLimits::default(),
    )
    .expect_err("frame() on an ungranted source must deny");
    assert!(
        matches!(records_err, RuleError::SourceNotAllowed(_)),
        "records deny shape: {records_err:?}"
    );
    assert!(
        matches!(frame_err, RuleError::SourceNotAllowed(_)),
        "frame() must produce the SAME opaque deny as records(), got {frame_err:?}"
    );
}

#[test]
fn sql_runs_in_memory_over_self_only() {
    let out = scalar(run_allowing(
        &["series"],
        r#"
            let f = source("series").frame();
            f.sql("SELECT series, avg(value) AS v FROM self GROUP BY series ORDER BY series")
             .records()
        "#,
    ));
    let rows = out.as_array().expect("sql returns rows");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["series"], json!("a"));
    assert_eq!(rows[0]["v"], json!(4.0));
    assert_eq!(rows[1]["v"], json!(8.0));
}

#[test]
fn sql_cannot_reach_the_filesystem() {
    // The cage stays zero-I/O: only `self` is registered, so a scan function has no table to bind.
    // (The crate-level probe in lb-frame proves the same at the SQLContext boundary.)
    let err = try_run(
        &["series"],
        r#"source("series").frame().sql("SELECT * FROM read_csv('/etc/hostname')")"#,
        RuleLimits::default(),
    )
    .expect_err("a filesystem read through f.sql must fail");
    let msg = err.to_string();
    assert!(
        !msg.contains("hostname root") && !msg.is_empty(),
        "the scan must never resolve, got: {msg}"
    );
}

#[test]
fn over_max_frame_rows_aborts_with_a_clear_author_error() {
    // The governors move to the INPUT for native calls: a tight cap refuses the materialization
    // rather than letting an uninterruptible polars call run away.
    let tight = RuleLimits {
        max_frame_rows: 2,
        ..RuleLimits::default()
    };
    let err = try_run(&["series"], r#"source("series").frame()"#, tight)
        .expect_err("4 seeded rows past a 2-row cap must abort");
    let msg = err.to_string();
    assert!(
        msg.contains("max_frame_rows") || msg.to_lowercase().contains("row"),
        "author error should name the row cap, got: {msg}"
    );
}

#[test]
fn frame_family_carries_its_catalog_rows() {
    let frame_rows = lb_rules::CATALOG
        .iter()
        .filter(|e| e.family == "frame")
        .count();
    assert!(
        frame_rows >= 50,
        "the frame family should carry its full surface, got {frame_rows}"
    );
    for e in lb_rules::CATALOG.iter().filter(|e| e.family == "frame") {
        assert!(
            e.description.ends_with('.'),
            "description of {} must end with '.'",
            e.name
        );
    }
}
