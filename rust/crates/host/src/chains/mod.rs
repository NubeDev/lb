//! The `chains.*` host service — a rule DAG driven over `lb-jobs` + SurrealDB (rule-chains-scope). The
//! pure DAG model + binding/result logic is lifted from rubix-cube (in the `lb-rules` crate); the
//! durable backend is OURS — `lb-jobs` for the run job + SurrealDB per-step run-store, inheriting the
//! workspace wall + `caps::check` rubix-cube's Postgres version never had.
//!
//! Verbs (one file each, FILE-LAYOUT): `chains.save`/`chains.delete` (CRUD, DAG-validated up front),
//! `chains.run`/`chains.resume` (manual trigger → durable job), `chains.get`/`chains.list`/
//! `chains.runs.get` (reads + the DAG-canvas snapshot). Host-native; gated `mcp:chains.<verb>:call` at
//! the bridge, then each verb's store surface. A step runs its rule under `caller ∩ grant` — the chain
//! cannot let a rule read a source its principal lacks (no widening via chaining).

mod coordinator;
mod error;
mod get;
mod record;
mod run;
mod run_store;
mod save;

pub use error::ChainsError;
pub use get::{chains_get, chains_list, chains_run_get};
pub use run::{chains_resume, chains_run, params_map};
pub use save::{chains_delete, chains_save};

use std::sync::Arc;

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_rules::workflow::Chain;
use serde_json::{json, Value};

use crate::boot::Node;
use crate::rules::RuleModel;

/// Default model seam for the bridge path (AI not configured unless a role wires one). Tests inject a
/// deterministic [`RuleModel`].
struct DisabledModel;

impl RuleModel for DisabledModel {
    fn complete(&self, _prompt: &str) -> Result<(String, u32), String> {
        Err("AI not configured for chains".into())
    }
    fn propose_sql(&self, _q: &str, _hint: &str) -> Result<String, String> {
        Err("AI not configured for chains".into())
    }
}

/// Dispatch a `chains.*` MCP call (gate already run in `tool_call`).
pub async fn call_chains_tool(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "chains.save" => {
            let chain: Chain = serde_json::from_value(input.clone())
                .map_err(|e| ToolError::BadInput(format!("chain: {e}")))?;
            let id = chains_save(&node.store, principal, ws, &chain).await?;
            Ok(json!({ "id": id }))
        }
        "chains.delete" => {
            let id = str_arg(input, "id")?;
            chains_delete(&node.store, principal, ws, id).await?;
            Ok(json!({ "ok": true }))
        }
        "chains.get" => {
            let id = str_arg(input, "id")?;
            let chain = chains_get(&node.store, principal, ws, id).await?;
            Ok(serde_json::to_value(chain).unwrap_or(Value::Null))
        }
        "chains.list" => {
            let chains = chains_list(&node.store, principal, ws).await?;
            Ok(json!({ "chains": chains }))
        }
        "chains.run" => {
            let chain_id = str_arg(input, "chain_id")?;
            let params = params_map(input.get("params").unwrap_or(&Value::Null));
            // The run id is caller-supplied for idempotency; default to a chain-scoped logical id.
            let now = input.get("ts").and_then(|v| v.as_u64()).unwrap_or(0);
            let run_id = input
                .get("run_id")
                .and_then(|v| v.as_str())
                .map(String::from)
                .unwrap_or_else(|| format!("{chain_id}-run-{now}"));
            let id = chains_run(
                node,
                principal,
                ws,
                chain_id,
                params,
                Arc::new(DisabledModel),
                &run_id,
                now,
            )
            .await?;
            Ok(json!({ "run_id": id }))
        }
        "chains.runs.get" => {
            let chain_id = str_arg(input, "chain_id")?;
            let run_id = str_arg(input, "run_id")?;
            let chain = chains_get(&node.store, principal, ws, chain_id).await?;
            let snapshot = chains_run_get(&node.store, principal, ws, &chain, run_id).await?;
            Ok(snapshot)
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
