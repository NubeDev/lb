//! The `stats` + `window` families through the REAL engine (data-stdlib-scope): rule bodies
//! computing statistics over rows fetched from the seeded recording seam, the run-twice
//! determinism contract (seeded `sample`/`shuffle`, no ambient randomness), and the family's
//! no-new-authority claim — a stats-only body runs green with an EMPTY allowlist and no grants.

mod support;

use std::collections::HashSet;
use std::sync::Arc;

use lb_rules::{AiLimits, GridJson, Rule, RuleEngine, RuleLimits, RuleOutput, RuleRun};
use serde_json::{json, Value};
use support::{RecordingData, RecordingMessaging, ScriptedAi};

fn seeded() -> Arc<RecordingData> {
    Arc::new(RecordingData::platform(
        &["series"],
        GridJson {
            columns: vec!["ts".into(), "value".into()],
            rows: vec![
                json!({ "ts": 1, "value": 3.0 }),
                json!({ "ts": 2, "value": 7.0 }),
                json!({ "ts": 3, "value": 5.0 }),
                json!({ "ts": 4, "value": 9.0 }),
            ],
        },
    ))
}

/// Run `body` with the given allowlist against the real engine over the recording seam.
fn run_allowing(allow: &[&str], body: &str) -> RuleOutput {
    let eng = RuleEngine::new(
        seeded(),
        Arc::new(ScriptedAi {
            completion: "x".into(),
            tokens: 1,
            proposed_sql: "SELECT 1 AS v".into(),
        }),
        Arc::new(RecordingMessaging::new()),
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
    let allow: HashSet<String> = allow.iter().map(|s| s.to_string()).collect();
    let mut rr = RuleRun::new("acme".into(), Arc::new(allow), rhai::Map::new(), 0);
    eng.run(&rule, &mut rr).unwrap()
}

fn scalar(out: RuleOutput) -> Value {
    match out {
        RuleOutput::Scalar(v) => v,
        other => panic!("expected a scalar output, got {other:?}"),
    }
}

#[test]
fn stats_over_col_fetched_rows() {
    // Fetch through the real Grid path, pull the column into a plain array, run the family.
    let out = scalar(run_allowing(
        &["series"],
        r#"
            let vals = source("series").records().map(|r| r.value);
            #{
                mean: mean(vals),
                median: median(vals),
                max: max_of(vals),
                p75: percentile(vals, 75.0),
                roll: rolling_mean(vals, 2),
                fit: linreg([0, 1, 2, 3], vals),
            }
        "#,
    ));
    // values are [3, 7, 5, 9]: mean 6, median 6, max 9, p75 = 7.5 (linear interpolation).
    assert_eq!(out["mean"], json!(6.0));
    assert_eq!(out["median"], json!(6.0));
    assert_eq!(out["max"], json!(9.0));
    assert_eq!(out["p75"], json!(7.5));
    // rolling_mean keeps the input length with a leading null (rhai () → JSON null).
    assert_eq!(out["roll"], json!([null, 5.0, 6.0, 7.0]));
    // Least squares over [3,7,5,9]: slope 1.6, intercept 3.6.
    assert!((out["fit"]["slope"].as_f64().unwrap() - 1.6).abs() < 1e-9);
    assert!((out["fit"]["intercept"].as_f64().unwrap() - 3.6).abs() < 1e-9);
}

#[test]
fn same_body_twice_is_byte_identical() {
    // Seeded sample/shuffle + the whole pure surface: two runs must serialize identically.
    let body = r#"
        let vals = source("series").records().map(|r| r.value);
        #{
            shuffled: shuffle(vals, 42),
            picked: sample([10, 20, 30, 40, 50], 3, 7),
            z: zscores(vals),
            ema: ema(vals, 0.5),
            hist: histogram(vals, 2),
            forecast: forecast_linear(vals, 2),
        }
    "#;
    let a = serde_json::to_string(&scalar(run_allowing(&["series"], body))).unwrap();
    let b = serde_json::to_string(&scalar(run_allowing(&["series"], body))).unwrap();
    assert_eq!(a, b, "same body + same seed must be byte-identical");
}

#[test]
fn stats_only_body_needs_no_caps_and_no_allowlist() {
    // The no-new-authority claim: EMPTY allowlist, no grants — pure compute still runs green,
    // including the documented missing-value policy (mean([1,(),3]) = 2.0).
    let out = scalar(run_allowing(
        &[],
        r#"
            let a = [1, (), 3.0, 10, "not-a-number"];
            #{
                mean: mean(a),
                dropped: dropna(a),
                filled: ffill([(), 1, (), 3, ()]),
                interp: interp_linear([1, (), 3]),
                outliers: outliers_iqr([1, 2, 3, 4, 100], 1.5),
                order: shuffle([1, 2, 3, 4], 11),
                top: top_k(a, 2),
                roll: rolling_sum([1, 2, 3], 2),
            }
        "#,
    ));
    // mean skips () and the string: (1 + 3 + 10) / 3.
    assert!((out["mean"].as_f64().unwrap() - 14.0 / 3.0).abs() < 1e-9);
    assert_eq!(out["dropped"], json!([1.0, 3.0, 10.0]));
    assert_eq!(out["filled"], json!([null, 1.0, 1.0, 3.0, 3.0]));
    assert_eq!(out["interp"], json!([1.0, 2.0, 3.0]));
    assert_eq!(out["outliers"], json!([4]));
    assert_eq!(out["top"], json!([10.0, 3.0]));
    assert_eq!(out["roll"], json!([null, 3.0, 5.0]));
    assert_eq!(out["order"].as_array().unwrap().len(), 4);
}

#[test]
fn author_errors_surface_clearly() {
    // A body hitting an undefined case fails the run with the verb's own message, not a panic.
    let eng = RuleEngine::new(
        seeded(),
        Arc::new(ScriptedAi {
            completion: "x".into(),
            tokens: 1,
            proposed_sql: "SELECT 1 AS v".into(),
        }),
        Arc::new(RecordingMessaging::new()),
        RuleLimits::default(),
        AiLimits::default(),
        32,
    );
    let rule = Rule {
        workspace: "acme".into(),
        name: "adhoc".into(),
        body: r#"mean([])"#.into(),
        params: vec![],
    };
    let mut rr = RuleRun::new("acme".into(), Arc::new(HashSet::new()), rhai::Map::new(), 0);
    let err = format!("{:?}", eng.run(&rule, &mut rr).unwrap_err());
    assert!(
        err.contains("mean"),
        "error should name the verb, got: {err}"
    );
}
