//! `rules.run {body|rule_id, params}` → `{output, findings, log, ms, ai}` — the hot path
//! (rules-engine-scope "MCP surface"). Build a fresh sandboxed engine with the host seams (the
//! workspace + caller pinned), evaluate the body ON A BLOCKING THREAD (CPU-bound rhai + the wall-clock
//! governor), then route every `alert` finding to the inbox + outbox (resolving rubix-cube's stage-03
//! TODO). Bounded by the governors, so it stays a synchronous call — a long/batch rule is a CHAIN.

use std::collections::HashSet;
use std::sync::Arc;

use lb_auth::Principal;
use lb_rules::{Finding, Rule, RuleEngine, RuleError, RuleOutput, RuleParam, RuleRun};
use serde::Serialize;
use serde_json::{json, Value};

use crate::boot::Node;

use super::config::{ai_limits, rule_limits};
use super::error::RulesError;
use super::get::rules_get;
use super::seam::{workspace_datasources, HostAiSeam, HostDataSeam, RuleModel};

/// The JSON-shaped result of a run.
#[derive(Serialize)]
pub struct RunResult {
    pub output: RuleOutput,
    pub findings: Vec<Finding>,
    pub log: Vec<lb_rules::LogLine>,
    pub ms: u64,
    pub ai: lb_rules::AiBudget,
}

/// Run an ad-hoc (`body`) or saved (`rule_id`) rule. The host authorizes `mcp:rules.run:call` at the
/// bridge; the per-source `caps::check` runs inside every collect (the `caller ∩ grant` chokepoint).
/// `now` is the logical clock for inbox/outbox routing (no wall-clock in core); `model` is the AI seam.
#[allow(clippy::too_many_arguments)]
pub async fn rules_run(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    body: Option<String>,
    rule_id: Option<String>,
    params: rhai::Map,
    model: Arc<dyn RuleModel>,
    now: u64,
) -> Result<RunResult, RulesError> {
    // Resolve the rule body: ad-hoc or by saved id.
    let (name, body, declared) = match (body, rule_id) {
        (Some(b), _) => ("adhoc".to_string(), b, Vec::<RuleParam>::new()),
        (None, Some(id)) => {
            let saved = rules_get(&node.store, principal, ws, &id).await?;
            (saved.name, saved.body, saved.params)
        }
        (None, None) => {
            return Err(RulesError::BadInput("missing body or rule_id".into()));
        }
    };
    let _ = declared;

    // Build the host seams, closed over the caller's principal + the pinned workspace.
    let datasources = workspace_datasources(node, ws).await;
    let handle = tokio::runtime::Handle::current();
    let data = Arc::new(HostDataSeam::new(
        node.clone(),
        principal.clone(),
        ws.to_string(),
        handle,
        datasources,
    ));
    let allowed: HashSet<String> = data.allowed_sources();
    let ai = Arc::new(HostAiSeam::new(model));

    let engine = RuleEngine::new(data, ai, rule_limits(), ai_limits());
    let rule = Rule {
        workspace: ws.to_string(),
        name,
        body,
        params: Vec::new(),
    };

    // Run on a blocking thread: the rhai eval is CPU-bound and uses the wall-clock governor; the seam
    // methods block_on the host's async surface (so the engine must NOT run on the async worker itself).
    let allowed = Arc::new(allowed);
    let result = tokio::task::spawn_blocking(move || {
        let mut run = RuleRun::new(rule.workspace.clone(), allowed, params);
        let out = engine.run(&rule, &mut run);
        (out, run)
    })
    .await
    .map_err(|e| RulesError::Internal(format!("rule task panicked: {e}")))?;

    let (out, run) = result;
    let output = match out {
        Ok(o) => o,
        Err(RuleError::SourceNotAllowed(_)) => return Err(RulesError::Denied),
        Err(RuleError::Eval(m)) => return Err(RulesError::Eval(m)),
        Err(e) => return Err(RulesError::Internal(e.to_string())),
    };

    // Route alert findings → inbox (attention) + outbox (must-deliver), per the scope.
    route_alerts(node, principal, ws, &run.findings, now).await?;

    Ok(RunResult {
        output,
        findings: run.findings,
        log: run.log,
        ms: 0, // logical: no wall-clock in the result (determinism, testing §3)
        ai: run.ai_spend,
    })
}

/// Hand each `alert`-marked finding to the inbox (an attention item on the `rules` channel) and route
/// a must-deliver notification through the outbox. `emit` findings stay in the result only.
async fn route_alerts(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    findings: &[Finding],
    now: u64,
) -> Result<(), RulesError> {
    for (i, f) in findings.iter().enumerate().filter(|(_, f)| f.is_alert()) {
        let body = serde_json::to_string(&f.data).unwrap_or_default();
        // A deterministic, content-derived id so a re-run upserts (idempotent — no wall-clock id).
        let id = format!("rule-alert-{}-{}", now, i);
        crate::record_inbox(&node.store, principal, ws, "rules", &id, &body, now)
            .await
            .map_err(|_| RulesError::Denied)?;
        let payload = json!({ "level": f.level, "data": f.data }).to_string();
        crate::enqueue_outbox(
            &node.store,
            principal,
            ws,
            &format!("{id}-effect"),
            "notify",
            "alert",
            &payload,
            now,
        )
        .await
        .map_err(|_| RulesError::Denied)?;
    }
    Ok(())
}

/// Coerce a JSON object of params into a rhai map.
pub fn params_to_rhai(params: &Value) -> rhai::Map {
    let mut m = rhai::Map::new();
    if let Some(obj) = params.as_object() {
        for (k, v) in obj {
            m.insert(k.as_str().into(), lb_rules::json_to_dynamic(v));
        }
    }
    m
}
