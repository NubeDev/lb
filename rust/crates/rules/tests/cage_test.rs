//! The cage holds — port rubix-cube's sandbox tests. `eval`/`import` rejected; `max_operations` trips
//! an infinite loop fast; the wall-clock deadline aborts a slow run; oversized values rejected. These
//! are DoS-boundary unit tests with tight `RuleLimits`.

mod support;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use lb_rules::{AiLimits, Rule, RuleEngine, RuleError, RuleLimits, RuleRun};
use support::{RecordingData, RecordingMessaging, ScriptedAi};

fn engine(limits: RuleLimits) -> RuleEngine {
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
    RuleEngine::new(data, ai, messaging, limits, AiLimits::default(), 32)
}

fn run_body(eng: &RuleEngine, body: &str) -> Result<lb_rules::RuleOutput, RuleError> {
    let rule = Rule {
        workspace: "acme".into(),
        name: "adhoc".into(),
        body: body.into(),
        params: vec![],
    };
    let mut run = RuleRun::new("acme".into(), Arc::new(HashSet::new()), rhai::Map::new(), 0);
    eng.run(&rule, &mut run)
}

#[test]
fn eval_is_rejected() {
    let eng = engine(RuleLimits::default());
    let err = run_body(&eng, r#"eval("1 + 1")"#).unwrap_err();
    assert!(matches!(err, RuleError::Eval(_)), "got {err:?}");
}

#[test]
fn import_is_rejected() {
    let eng = engine(RuleLimits::default());
    let err = run_body(&eng, r#"import "x" as y; 1"#).unwrap_err();
    assert!(matches!(err, RuleError::Eval(_)), "got {err:?}");
}

#[test]
fn max_operations_trips_infinite_loop_fast() {
    let limits = RuleLimits {
        max_operations: 10_000,
        ..RuleLimits::default()
    };
    let eng = engine(limits);
    let err = run_body(&eng, "let i = 0; while true { i += 1; } i").unwrap_err();
    assert!(matches!(err, RuleError::Eval(_)), "got {err:?}");
}

#[test]
fn wall_clock_deadline_aborts_slow_run() {
    let limits = RuleLimits {
        max_operations: u64::MAX,
        timeout: Duration::from_millis(50),
        ..RuleLimits::default()
    };
    let eng = engine(limits);
    // A loop that does little per-op but runs long enough to cross the deadline.
    let err = run_body(&eng, "let i = 0; while i < 100000000 { i += 1; } i").unwrap_err();
    assert!(matches!(err, RuleError::Eval(_)), "got {err:?}");
}

#[test]
fn oversized_array_rejected() {
    let limits = RuleLimits {
        max_array_len: 4,
        ..RuleLimits::default()
    };
    let eng = engine(limits);
    let err = run_body(&eng, "let a = []; for i in 0..100 { a.push(i); } a").unwrap_err();
    assert!(matches!(err, RuleError::Eval(_)), "got {err:?}");
}
