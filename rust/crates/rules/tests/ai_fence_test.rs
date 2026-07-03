//! The AI fence + budget — port verbatim. A malicious-LLM stub proposing a foreign/blocked query via
//! `ai.ask` is re-validated through the SAME collect path a hand-written query takes (the fence — there
//! is no nsql path that skips it). The `AiMeter` caps calls + summed tokens (a loop can't overspend);
//! a rejected call is not counted.

mod support;

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use std::sync::Mutex;

use lb_rules::seam::{AiSeam, DataSeam, SchemaColumn, SourceKind};
use lb_rules::{
    AiCompletion, AiLimits, GridJson, Rule, RuleEngine, RuleError, RuleLimits, RuleRun,
};
use support::ScriptedAi;

/// A data seam that REFUSES any collect whose query mentions a blocked table — modelling the host's
/// `caps::check` + workspace pin rejecting a proposed cross-tenant query. The fence is proven by the
/// proposed SQL flowing through THIS gate before it can run.
struct FencingData {
    proposed_reached_collect: Arc<Mutex<bool>>,
}

impl DataSeam for FencingData {
    fn resolve(&self, source: &str) -> Result<(SourceKind, String), String> {
        if source == "series" {
            Ok((SourceKind::Platform, "series".into()))
        } else {
            Err(format!("source not allowed: {source}"))
        }
    }
    fn collect(&self, _k: SourceKind, _s: &str, query: &str) -> Result<GridJson, String> {
        if query.contains("payroll") {
            // the gate the fence routes through rejects the foreign table
            return Err("source not allowed: payroll".into());
        }
        *self.proposed_reached_collect.lock().unwrap() = true;
        Ok(GridJson {
            columns: vec!["v".into()],
            rows: vec![serde_json::json!({ "v": 1 })],
        })
    }
    fn schemas(&self) -> Result<BTreeMap<String, Vec<SchemaColumn>>, String> {
        Ok(BTreeMap::new())
    }
}

fn allow_series() -> Arc<HashSet<String>> {
    let mut a = HashSet::new();
    a.insert("series".to_string());
    Arc::new(a)
}

#[test]
fn proposed_sql_is_revalidated_through_the_gate() {
    // The malicious LLM proposes a query against a blocked table.
    let ai = Arc::new(ScriptedAi {
        completion: "x".into(),
        tokens: 1,
        proposed_sql: "SELECT * FROM payroll".into(),
    });
    let reached = Arc::new(Mutex::new(false));
    let data = Arc::new(FencingData {
        proposed_reached_collect: reached.clone(),
    });
    let eng = RuleEngine::new(
        data,
        ai,
        Arc::new(support::RecordingMessaging::new()),
        RuleLimits::default(),
        AiLimits::default(),
        32,
    );
    let rule = Rule {
        workspace: "acme".into(),
        name: "adhoc".into(),
        body: r#"ai.ask("how much is payroll?").records()"#.into(),
        params: vec![],
    };
    let mut rr = RuleRun::new("acme".into(), allow_series(), rhai::Map::new(), 0);
    let err = eng.run(&rule, &mut rr).unwrap_err();
    // The proposed SQL was rejected at the collect gate — the fence held (routed through the same
    // deny the host's caps::check + workspace pin produces; never reached execution).
    assert!(
        matches!(err, RuleError::SourceNotAllowed(_) | RuleError::Eval(_)),
        "got {err:?}"
    );
    assert!(
        !*reached.lock().unwrap(),
        "the proposed query must NOT have collected successfully"
    );
}

#[test]
fn budget_caps_calls_a_loop_cannot_overspend() {
    let ai = Arc::new(ScriptedAi {
        completion: "x".into(),
        tokens: 1,
        proposed_sql: "SELECT 1 AS v".into(),
    });
    let data = Arc::new(FencingData {
        proposed_reached_collect: Arc::new(Mutex::new(false)),
    });
    let limits = AiLimits {
        max_calls: 3,
        max_tokens: 1_000_000,
        context_rows: 100,
    };
    let eng = RuleEngine::new(
        data,
        ai,
        Arc::new(support::RecordingMessaging::new()),
        RuleLimits::default(),
        limits,
        32,
    );
    let rule = Rule {
        workspace: "acme".into(),
        name: "adhoc".into(),
        body: r#"for i in 0..100 { ai.complete("hi"); } 1"#.into(),
        params: vec![],
    };
    let mut rr = RuleRun::new("acme".into(), allow_series(), rhai::Map::new(), 0);
    let err = eng.run(&rule, &mut rr).unwrap_err();
    assert!(matches!(err, RuleError::Eval(_)), "got {err:?}");
    // exactly the cap was charged (the rejected call rolled back, not counted past the cap)
    assert_eq!(rr.ai_spend.calls, 3);
}

#[test]
fn budget_caps_summed_tokens() {
    let ai = Arc::new(ScriptedAi {
        completion: "x".into(),
        tokens: 500,
        proposed_sql: "SELECT 1 AS v".into(),
    });
    let data = Arc::new(FencingData {
        proposed_reached_collect: Arc::new(Mutex::new(false)),
    });
    let limits = AiLimits {
        max_calls: 100,
        max_tokens: 1200,
        context_rows: 100,
    };
    let eng = RuleEngine::new(
        data,
        ai,
        Arc::new(support::RecordingMessaging::new()),
        RuleLimits::default(),
        limits,
        32,
    );
    let rule = Rule {
        workspace: "acme".into(),
        name: "adhoc".into(),
        body: r#"for i in 0..100 { ai.complete("hi"); } 1"#.into(),
        params: vec![],
    };
    let mut rr = RuleRun::new("acme".into(), allow_series(), rhai::Map::new(), 0);
    let err = eng.run(&rule, &mut rr).unwrap_err();
    assert!(matches!(err, RuleError::Eval(_)), "got {err:?}");
    // 500 + 500 ok (1000), third call's tokens push over 1200 -> abort after 3 calls
    assert!(rr.ai_spend.tokens >= 1200, "tokens={}", rr.ai_spend.tokens);
}

#[test]
fn source_not_in_allowlist_is_denied_mid_run() {
    let ai = Arc::new(ScriptedAi {
        completion: "x".into(),
        tokens: 1,
        proposed_sql: "SELECT 1 AS v".into(),
    });
    let data = Arc::new(FencingData {
        proposed_reached_collect: Arc::new(Mutex::new(false)),
    });
    let eng = RuleEngine::new(
        data,
        ai,
        Arc::new(support::RecordingMessaging::new()),
        RuleLimits::default(),
        AiLimits::default(),
        32,
    );
    let rule = Rule {
        workspace: "acme".into(),
        name: "adhoc".into(),
        body: r#"source("payroll_db").records()"#.into(),
        params: vec![],
    };
    // allowlist has only "series" — payroll_db is denied before any query runs.
    let mut rr = RuleRun::new("acme".into(), allow_series(), rhai::Map::new(), 0);
    let err = eng.run(&rule, &mut rr).unwrap_err();
    assert!(matches!(err, RuleError::SourceNotAllowed(_)), "got {err:?}");
}
