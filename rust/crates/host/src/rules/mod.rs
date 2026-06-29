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
mod record;
mod run;
mod save;
mod seam;

pub use config::{ai_limits, max_chain_steps, rule_limits};
pub use delete::rules_delete;
pub use error::RulesError;
pub use get::{rules_get, rules_list};
pub use record::SavedRule;
pub use run::{params_to_rhai, rules_run, RunResult};
pub use save::rules_save;
pub use seam::{workspace_datasources, HostAiSeam, HostDataSeam, RuleModel};

use std::sync::Arc;

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_rules::RuleParam;
use serde_json::{json, Value};

use crate::boot::Node;

/// The default model seam for the bridge path: AI is not configured unless a role wires one. A rule
/// that calls `ai.*` without a configured model gets a clear error (rubix-cube's posture); a rule that
/// only reads data + emits runs fine. Tests inject a deterministic [`RuleModel`].
struct DisabledModel;

impl RuleModel for DisabledModel {
    fn complete(&self, _prompt: &str) -> Result<(String, u32), String> {
        Err("AI not configured for rules".into())
    }
    fn propose_sql(&self, _question: &str, _schema_hint: &str) -> Result<String, String> {
        Err("AI not configured for rules".into())
    }
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
            let result = rules_run(
                node,
                principal,
                ws,
                body,
                rule_id,
                params,
                Arc::new(DisabledModel),
                now,
            )
            .await?;
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
