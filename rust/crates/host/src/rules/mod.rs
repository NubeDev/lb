//! The `rules.*` host service — the lb-rules engine wired to the platform chokepoints (rules-engine-
//! scope). MCP verbs over the embedded store + the re-seam to `store.query`/`series.*`/
//! `federation.query`/the AI-gateway/inbox-outbox:
//!   - `rules.run {body|rule_id, params}` → `{output, findings, log, ms, ai}` ([`run::rules_run`]);
//!   - `rules.eval {body|rule_id, payload, topic, params, timeout_ms}` → the same result, shaped for a
//!     flow node: the message envelope in, findings out (rules-workflow-convergence scope, [`eval`]);
//!   - `rules.save` / `rules.delete` — CRUD over `rule:{ws}:{id}` records;
//!   - `rules.get {id}` / `rules.list {filter?}` — workspace-scoped reads.
//!
//! Host-native (not a wasm extension) — reached through the one MCP contract, authorized workspace-
//! first then `mcp:rules.<verb>:call` at the bridge ([`call_rules_tool`]), then each verb adds its own
//! store/inbox/outbox surface gate. The cage + the per-source `caps::check` inside every collect are
//! the security boundary (rule 5/7). Attribution + the port lives in the `lb-rules` crate docs.

mod config;
mod delete;
mod error;
mod eval;
mod get;
mod model;
mod record;
mod run;
mod runs;
mod save;
mod seam;

pub use config::{ai_limits, rule_limits};
pub use delete::rules_delete;
pub use error::RulesError;
pub use eval::rules_eval;
pub use get::{rules_get, rules_list};
pub use model::AgentRuleModel;
pub use record::SavedRule;
pub use run::{params_to_rhai, rules_run, RunResult};
pub use runs::RuleRunMap;
pub use save::rules_save;
pub use seam::{workspace_datasources, workspace_queries, HostAiSeam, HostDataSeam, RuleModel};

use std::sync::Arc;

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_rules::RuleParam;
use serde_json::{json, Value};

use crate::boot::Node;

/// The **resolved-disabled** model seam: the honest "AI not configured" outcome for a workspace with
/// no model selected, or a node with no real provider wired. A rule that calls `ai.*` gets this clear
/// error; a rule that only reads data + emits runs fine. This is a *resolved* state now (see
/// [`resolve_rule_model`]), not a hardcoded default — a configured workspace gets [`AgentRuleModel`].
/// Tests may still inject their own deterministic [`RuleModel`] into [`rules_run`] directly.
struct DisabledModel;

impl RuleModel for DisabledModel {
    fn complete(&self, _prompt: &str) -> Result<(String, u32), String> {
        Err("AI not configured for rules".into())
    }
    fn propose_sql(&self, _question: &str, _schema_hint: &str) -> Result<String, String> {
        Err("AI not configured for rules".into())
    }
}

/// Resolve the model a `rules.run` uses — the single source of truth for "which model do this
/// workspace's rules use" is the workspace's **active agent** (active-agent-wiring scope), resolved
/// through the same [`resolve_workspace_model`](crate::agent::resolve_workspace_model) the in-house loop
/// rides at run start. That maps the active definition's `model_endpoint` through the OpenAI-compatible
/// adapter (so a workspace whose active agent is external over GLM-4.6 gets working `ai.*` from that
/// endpoint), falling back to the node-level in-house model.
///
/// **A rule's `ai.*` requires the workspace to have configured a model — a node-level model alone is
/// NOT enough** (unlike the in-house *loop*, whose fallback tier IS the node model). A workspace with no
/// `agent.config` (no active pick AND no selected `model_endpoint`) keeps the honest [`DisabledModel`]:
/// a rule that only reads data + emits still runs; only `ai.*` errors. So the two gates are:
///   1. the workspace **configured** a model in `agent.config` (an `active_definition` OR a
///      `model_endpoint` — the catalog pick writes both; a bare endpoint selection sets the latter), and
///   2. the resolved model is a **real** provider ([`ErasedModel::is_configured`] — not the placeholder).
/// Either missing → [`DisabledModel`]. The reads are workspace-scoped `agent.config` under the caller
/// (the hard wall) on the already-authorized `rules.run` path — a host-internal read, not a new verb.
async fn resolve_rule_model(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    idem: String,
) -> Arc<dyn RuleModel> {
    // (1) Did this workspace configure a model at all? (best-effort: a read error → treat as unconfigured).
    // A node-level model is the in-house LOOP's fallback, not a rule's — a rule needs the workspace to
    // have chosen one, so `ai.*` stays honestly disabled for a workspace that picked nothing.
    let configured_ws = matches!(
        crate::agent::get_agent_config(&node.store, ws).await,
        Ok(Some(cfg)) if cfg.active_definition.is_some() || cfg.model_endpoint.is_some()
    );
    if !configured_ws {
        return Arc::new(DisabledModel);
    }
    // (2) Resolve the workspace's model (active pick's endpoint → node fallback → placeholder), memoized.
    // Only a REAL provider drives `ai.*`; the placeholder keeps the honest "not configured" (a selected
    // endpoint on a provider-less node still errors, never fabricates).
    let model = crate::agent::resolve_workspace_model(node, principal, ws).await;
    if !model.is_configured() {
        return Arc::new(DisabledModel);
    }
    Arc::new(AgentRuleModel::new(model, ws, idem))
}

/// A per-run idempotency prefix for the model call — deterministic in `(ws, rule body/id, run-ts)` so a
/// re-run replays cleanly through the gateway's turn cache (a rule run is NOT itself durable — no
/// resume — only the model call is replay-cached; rules-ai-wiring-scope open question).
fn idem_prefix(ws: &str, body: Option<&str>, rule_id: Option<&str>, now: u64) -> String {
    let sel = match (rule_id, body) {
        (Some(id), _) => format!("rule:{id}"),
        (None, Some(b)) => format!("adhoc:{:x}", body_hash(b)),
        (None, None) => "empty".to_string(),
    };
    format!("rules.run:{ws}:{sel}:{now}")
}

/// The host wall-clock as epoch milliseconds — the unit `insight.ts` is stored + rendered in
/// (`InsightsList.timeAgo` does `Date.now() - ts`, `insight::reactor` stamps `as_millis`). Used ONLY
/// to backfill `now` when a `rules.*` caller omits `ts`; an explicit `ts` always wins so deterministic
/// callers (flows, tests) stay reproducible. Read once here, at the MCP chokepoint (mirrors `now_ts`).
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// A cheap stable hash of an ad-hoc rule body for the idempotency key (FNV-1a — no wall-clock, no rng,
/// deterministic so a re-run of the same body reuses the same key).
fn body_hash(body: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in body.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Dispatch a `rules.*` MCP call (the bridge entry; gate already run in `tool_call`). `input` is the
/// verb's JSON args; the return is the verb's JSON result.
pub async fn call_rules_tool(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "rules.run" => {
            let body = input.get("body").and_then(|v| v.as_str()).map(String::from);
            let rule_id = input
                .get("rule_id")
                .and_then(|v| v.as_str())
                .map(String::from);
            let params = params_to_rhai(input.get("params").unwrap_or(&Value::Null));
            // `ts` is the run's logical clock — the cage stamps it onto every `insight.raise`
            // (`ts: now`) and every routed alert id. A deterministic caller (a flow, a test seeding a
            // fixed clock) passes it explicitly; an interactive/UI `rules.run` omits it, and defaulting
            // to `0` stamped every raised insight with the Unix epoch ("1/1/1970"). Backfill the host
            // wall-clock (epoch millis, matching `now_ts`/`insight::reactor`) when the caller omits it,
            // so an interactive run gets a real date while an explicit `ts` still wins (determinism).
            let now = input
                .get("ts")
                .and_then(|v| v.as_u64())
                .unwrap_or_else(now_ms);
            // `route` gates the alert fan-out — default `true` (existing behavior unchanged). A panel
            // source sets `route:false` so a dashboard repaint doesn't spam the Inbox/Outbox
            // (rules-for-widgets-scope slice 2). The viz plane never learns the flag exists — the
            // picker composes it into `args`, exactly like the params form.
            let route = input.get("route").and_then(|v| v.as_bool()).unwrap_or(true);
            // Resolve the workspace's model — the catalog pick (`agent.config`) → the node's model.
            // A configured workspace gets the real model; an unconfigured one gets the honest
            // `DisabledModel` error (now a *resolved* outcome, not a hardcoded default).
            let idem = idem_prefix(ws, body.as_deref(), rule_id.as_deref(), now);
            let model = resolve_rule_model(node, principal, ws, idem).await;
            let result = rules_run(
                node, principal, ws, body, rule_id, params, model, now, None, route,
            )
            .await?;
            Ok(serde_json::to_value(result).unwrap_or(Value::Null))
        }
        "rules.eval" => {
            // The flow-node entry: the message envelope in (`payload`/`topic`/carried fields), findings
            // out. `body` xor `rule_id`; the envelope fields + explicit `params` become rule params.
            let body = input.get("body").and_then(|v| v.as_str()).map(String::from);
            let rule_id = input
                .get("rule_id")
                .and_then(|v| v.as_str())
                .map(String::from);
            let envelope = input
                .get("envelope")
                .and_then(|v| v.as_object())
                .cloned()
                .unwrap_or_default();
            let explicit_params = input.get("params").cloned().unwrap_or(Value::Null);
            let timeout_ms = input.get("timeout_ms").and_then(|v| v.as_u64());
            // `ts` is the run's logical clock — the cage stamps it onto every `insight.raise`
            // (`ts: now`) and every routed alert id. A deterministic caller (a flow, a test seeding a
            // fixed clock) passes it explicitly; an interactive/UI `rules.run` omits it, and defaulting
            // to `0` stamped every raised insight with the Unix epoch ("1/1/1970"). Backfill the host
            // wall-clock (epoch millis, matching `now_ts`/`insight::reactor`) when the caller omits it,
            // so an interactive run gets a real date while an explicit `ts` still wins (determinism).
            let now = input
                .get("ts")
                .and_then(|v| v.as_u64())
                .unwrap_or_else(now_ms);
            let route = input.get("route").and_then(|v| v.as_bool()).unwrap_or(true);
            let idem = idem_prefix(ws, body.as_deref(), rule_id.as_deref(), now);
            let model = resolve_rule_model(node, principal, ws, idem).await;
            let result = crate::rules::rules_eval(
                node,
                principal,
                ws,
                body,
                rule_id,
                &envelope,
                &explicit_params,
                timeout_ms,
                model,
                now,
                route,
            )
            .await?;
            Ok(serde_json::to_value(result).unwrap_or(Value::Null))
        }
        // ---- job-backed runs (long-running-rules-scope): start + observe/control ----
        "rules.run_async" => Ok(runs::rules_run_async(node, principal, ws, input).await?),
        "rules.runs.get" => {
            let run_id = str_arg(input, "run_id")?;
            Ok(runs::rules_runs_get(node, ws, run_id).await?)
        }
        "rules.runs.list" => Ok(runs::rules_runs_list(node, ws, input).await?),
        "rules.runs.suspend" => {
            let run_id = str_arg(input, "run_id")?;
            Ok(runs::rules_runs_suspend(node, ws, run_id).await?)
        }
        "rules.runs.resume" => {
            let run_id = str_arg(input, "run_id")?;
            Ok(runs::rules_runs_resume(node, principal, ws, run_id).await?)
        }
        "rules.runs.cancel" => {
            let run_id = str_arg(input, "run_id")?;
            Ok(runs::rules_runs_cancel(node, ws, run_id).await?)
        }
        "rules.save" => {
            let id = str_arg(input, "id").or_else(|_| str_arg(input, "name"))?;
            let name = input.get("name").and_then(|v| v.as_str()).unwrap_or(id);
            let body = str_arg(input, "body")?;
            let params: Vec<RuleParam> = input
                .get("params")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            let saved = rules_save(&node.store, principal, ws, id, name, body, params).await?;
            Ok(json!({ "id": saved }))
        }
        "rules.get" => {
            let id = str_arg(input, "id")?;
            let rule = rules_get(&node.store, principal, ws, id).await?;
            Ok(serde_json::to_value(rule).unwrap_or(Value::Null))
        }
        "rules.list" => {
            let rules = rules_list(&node.store, principal, ws).await?;
            Ok(json!({ "rules": rules }))
        }
        "rules.delete" => {
            let id = str_arg(input, "id")?;
            rules_delete(&node.store, principal, ws, id).await?;
            Ok(json!({ "ok": true }))
        }
        "rules.help" => {
            // The introspection surface: return the rule-cage function catalog verbatim (the single
            // source of truth in `lb_rules::CATALOG`). Lets an agent/UI discover the verb surface +
            // descriptions without reading the skill doc. Gated `mcp:rules.help:call` like the other
            // verbs; no per-verb host code beyond serialization.
            let functions = lb_rules::CATALOG
                .iter()
                .map(|e| {
                    json!({
                        "name": e.name,
                        "family": e.family,
                        "signature": e.signature,
                        "description": e.description,
                    })
                })
                .collect::<Vec<_>>();
            Ok(json!({ "functions": functions }))
        }
        _ => Err(ToolError::NotFound),
    }
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadInput(format!("missing/invalid arg: {key}")))
}
