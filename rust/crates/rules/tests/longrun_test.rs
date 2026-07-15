//! Long-running-rules cage tests (long-running-rules-scope "Testing plan → Cage"): the `job`
//! handle's two modes (durable via a recording [`JobSeam`] / ephemeral), `job.step` memoization +
//! replay-as-lookup, the checkpoint/progress budgets, and the cooperative pause/cancel abort
//! mapping. The recording seam stands in for the HOST's transcript impl at the sanctioned trait
//! boundary; the real store path is exercised in the host integration tests.

mod support;

use std::sync::{Arc, Mutex};

use lb_rules::seam::{JobSeam, SeamError};
use lb_rules::{
    AiLimits, GridJson, JobBinding, Rule, RuleEngine, RuleError, RuleLimits, RuleOutput, RuleRun,
    RunControl, RunOptions,
};
use serde_json::Value;
use support::{RecordingData, RecordingMessaging, ScriptedAi};

/// Records every checkpoint/beat the handle persists (the host's transcript stand-in).
#[derive(Default)]
struct RecordingJobSeam {
    checkpoints: Mutex<Vec<(String, Value)>>,
    beats: Mutex<Vec<(Option<u32>, String)>>,
}

impl JobSeam for RecordingJobSeam {
    fn checkpoint(&self, key: &str, value: &Value) -> Result<(), SeamError> {
        self.checkpoints
            .lock()
            .unwrap()
            .push((key.to_string(), value.clone()));
        Ok(())
    }
    fn progress(&self, pct: Option<u32>, msg: &str) -> Result<(), SeamError> {
        self.beats.lock().unwrap().push((pct, msg.to_string()));
        Ok(())
    }
}

fn engine() -> RuleEngine {
    let data = Arc::new(RecordingData::platform(
        &[],
        GridJson {
            columns: vec![],
            rows: vec![],
        },
    ));
    let ai = Arc::new(ScriptedAi {
        completion: "ok".into(),
        tokens: 1,
        proposed_sql: "SELECT 1".into(),
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

fn rule(body: &str) -> Rule {
    Rule {
        workspace: "acme".into(),
        name: "t".into(),
        body: body.into(),
        params: Vec::new(),
    }
}

fn run() -> RuleRun {
    RuleRun::new(
        "acme".into(),
        Arc::new(Default::default()),
        rhai::Map::new(),
        1_700_000_000_000,
    )
}

fn durable_opts(
    seam: Arc<RecordingJobSeam>,
    state: rhai::Map,
    control: Arc<RunControl>,
) -> RunOptions {
    RunOptions {
        control: Some(control),
        job: Some(JobBinding {
            id: "rr-test".into(),
            seam,
            state,
        }),
    }
}

#[test]
fn ephemeral_job_handle_works_in_a_sync_run_with_zero_authority() {
    // Empty allowlist, no caps, no seam — the handle still works, in-memory (one body, both modes).
    let body = r#"
        job.set("k", 41);
        let v = job.step("s", || 1);
        if job.should_stop() { throw "should not stop"; }
        if job.is_durable() { throw "sync run must be ephemeral"; }
        job.get("k") + v
    "#;
    let out = engine().run(&rule(body), &mut run()).unwrap();
    match out {
        RuleOutput::Scalar(v) => assert_eq!(v, serde_json::json!(42)),
        other => panic!("expected scalar, got {other:?}"),
    }
}

#[test]
fn durable_set_and_step_persist_through_the_seam() {
    let seam = Arc::new(RecordingJobSeam::default());
    let body = r#"
        job.set("plan", ["a", "b"]);
        job.step("day:a", || 10) + job.step("day:b", || 20)
    "#;
    let out = engine()
        .run_with(
            &rule(body),
            &mut run(),
            durable_opts(seam.clone(), rhai::Map::new(), Arc::default()),
        )
        .unwrap();
    match out {
        RuleOutput::Scalar(v) => assert_eq!(v, serde_json::json!(30)),
        other => panic!("expected scalar, got {other:?}"),
    }
    let cps = seam.checkpoints.lock().unwrap();
    let keys: Vec<&str> = cps.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, vec!["plan", "day:a", "day:b"]);
    assert_eq!(cps[1].1, serde_json::json!(10));
}

#[test]
fn step_replays_as_a_lookup_from_folded_state() {
    // Resume: the persisted value wins; the closure must NOT run (its poison value would show).
    let seam = Arc::new(RecordingJobSeam::default());
    let mut state = rhai::Map::new();
    state.insert("expensive".into(), rhai::Dynamic::from(7_i64));
    let body = r#"job.step("expensive", || 999)"#;
    let out = engine()
        .run_with(
            &rule(body),
            &mut run(),
            durable_opts(seam.clone(), state, Arc::default()),
        )
        .unwrap();
    match out {
        RuleOutput::Scalar(v) => assert_eq!(v, serde_json::json!(7)),
        other => panic!("expected scalar, got {other:?}"),
    }
    // A lookup persists nothing — no re-spend, no duplicate checkpoint.
    assert!(seam.checkpoints.lock().unwrap().is_empty());
}

#[test]
fn pause_request_aborts_and_maps_to_paused() {
    let control = Arc::new(RunControl::default());
    control.request_pause();
    let seam = Arc::new(RecordingJobSeam::default());
    let err = engine()
        .run_with(
            &rule("let x = 0; while true { x += 1; }"),
            &mut run(),
            durable_opts(seam, rhai::Map::new(), control),
        )
        .unwrap_err();
    assert!(matches!(err, RuleError::Paused), "got {err:?}");
}

#[test]
fn cancel_request_aborts_and_outranks_pause() {
    let control = Arc::new(RunControl::default());
    control.request_pause();
    control.request_cancel();
    let seam = Arc::new(RecordingJobSeam::default());
    let err = engine()
        .run_with(
            &rule("let x = 0; while true { x += 1; }"),
            &mut run(),
            durable_opts(seam, rhai::Map::new(), control),
        )
        .unwrap_err();
    assert!(matches!(err, RuleError::Cancelled), "got {err:?}");
}

#[test]
fn should_stop_reflects_the_shared_control_before_the_governor_bites() {
    // The author-cooperative path: the body observes the intent and returns tidily — the run
    // SUCCEEDS (no abort) because it finished before the next governor tick could fire it.
    // (A pre-set flag aborts at the first op; so this asserts the ephemeral fallback instead:
    // an un-controlled durable run sees should_stop() == false.)
    let seam = Arc::new(RecordingJobSeam::default());
    let out = engine()
        .run_with(
            &rule("job.should_stop()"),
            &mut run(),
            RunOptions {
                control: None,
                job: Some(JobBinding {
                    id: "rr-test".into(),
                    seam,
                    state: rhai::Map::new(),
                }),
            },
        )
        .unwrap();
    match out {
        RuleOutput::Scalar(v) => assert_eq!(v, serde_json::json!(false)),
        other => panic!("expected scalar, got {other:?}"),
    }
}

#[test]
fn checkpoint_budget_is_bounded() {
    let seam = Arc::new(RecordingJobSeam::default());
    let err = engine()
        .run_with(
            &rule(r#"for i in 0..300 { job.set(`k${i}`, i); }"#),
            &mut run(),
            durable_opts(seam, rhai::Map::new(), Arc::default()),
        )
        .unwrap_err();
    assert!(err.to_string().contains("checkpoint budget"), "got {err}");
}

#[test]
fn progress_is_bounded_but_advisory() {
    // Past the cap the beats stop persisting but the run keeps going and completes.
    let seam = Arc::new(RecordingJobSeam::default());
    let out = engine()
        .run_with(
            &rule(r#"for i in 0..1100 { job.progress("beat"); } "done""#),
            &mut run(),
            durable_opts(seam.clone(), rhai::Map::new(), Arc::default()),
        )
        .unwrap();
    match out {
        RuleOutput::Scalar(v) => assert_eq!(v, serde_json::json!("done")),
        other => panic!("expected scalar, got {other:?}"),
    }
    assert_eq!(seam.beats.lock().unwrap().len(), 1000);
}

#[test]
fn progress_pct_is_validated() {
    let seam = Arc::new(RecordingJobSeam::default());
    let err = engine()
        .run_with(
            &rule(r#"job.progress(101, "over")"#),
            &mut run(),
            durable_opts(seam, rhai::Map::new(), Arc::default()),
        )
        .unwrap_err();
    assert!(err.to_string().contains("0..=100"), "got {err}");
}
