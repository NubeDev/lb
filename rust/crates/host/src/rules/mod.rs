//! The `rules.*` host service — the lb-rules engine wired to the platform chokepoints (rules-engine-
//! scope). Five MCP verbs over the embedded store + the re-seam to `store.query`/`series.*`/
//! `federation.query`/the AI-gateway/inbox-outbox:
//!   - `rules.run {body|rule_id, params}` → `{output, findings, log, ms, ai}` ([`run::rules_run`]);
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
mod get;
mod model;
mod record;
mod run;
mod save;
mod seam;

pub use config::{ai_limits, rule_limits};
pub use delete::rules_delete;
pub use error::RulesError;
pub use get::{rules_get, rules_list};
pub use model::AgentRuleModel;
pub use record::SavedRule;
pub use run::{params_to_rhai, rules_run, RunResult};
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
/// workspace's rules use" is the agent-catalog pick (`agent.config`), the SAME record the agent
/// reads (rules-ai-wiring-scope). A rule's model is real only when BOTH hold:
///   1. the workspace has **selected** a model endpoint in `agent.config` (the catalog wrote one), and
///   2. the node actually has a **real** provider wired ([`ErasedModel::is_configured`] — not the
///      `UnconfiguredModel` placeholder boot binds before a provider exists).
/// Either missing → the honest [`DisabledModel`] (a rule's `ai.*` errors clearly; data-only rules run).
/// This reads the workspace-scoped `agent.config` (the hard wall) on the already-authorized
/// `rules.run` path — a host-internal read, not a new caller-facing verb.
async fn resolve_rule_model(node: &Arc<Node>, ws: &str, idem: String) -> Arc<dyn RuleModel> {
    // (1) Did this workspace select a model endpoint? (best-effort: a read error → treat as unset).
    let selected = matches!(
        crate::agent::get_agent_config(&node.store, ws).await,
        Ok(Some(cfg)) if cfg.model_endpoint.is_some()
    );
    if !selected {
        return Arc::new(DisabledModel);
    }
    // (2) Does the node have a real model provider? (the in-house `default` runtime's model).
    let model = node.runtimes().default_model();
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
            let now = input.get("ts").and_then(|v| v.as_u64()).unwrap_or(0);
            // Resolve the workspace's model — the catalog pick (`agent.config`) → the node's model.
            // A configured workspace gets the real model; an unconfigured one gets the honest
            // `DisabledModel` error (now a *resolved* outcome, not a hardcoded default).
            let idem = idem_prefix(ws, body.as_deref(), rule_id.as_deref(), now);
            let model = resolve_rule_model(node, ws, idem).await;
            let result = rules_run(node, principal, ws, body, rule_id, params, model, now).await?;
            Ok(serde_json::to_value(result).unwrap_or(Value::Null))
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
        _ => Err(ToolError::NotFound),
    }
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadInput(format!("missing/invalid arg: {key}")))
}
