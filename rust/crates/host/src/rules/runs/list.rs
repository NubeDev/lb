//! `rules.runs.list {status?, limit?}` — the workspace's rule runs, newest first
//! (long-running-rules-scope). Terminal rows included (the observe read, not the reactor drain);
//! `status` narrows to one lifecycle value; `limit` defaults to 50.

use std::sync::Arc;

use serde_json::{json, Value};

use crate::boot::Node;

use super::super::error::RulesError;
use super::get::shape_run;
use super::worker::RULE_RUN_KIND;

pub async fn rules_runs_list(
    node: &Arc<Node>,
    ws: &str,
    input: &Value,
) -> Result<Value, RulesError> {
    let status = input.get("status").and_then(|v| v.as_str());
    let limit = input
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or(50);
    let jobs = lb_jobs::list_kind(&node.store, ws, RULE_RUN_KIND, status, limit)
        .await
        .map_err(|e| RulesError::Internal(e.to_string()))?;
    let items: Vec<Value> = jobs.iter().map(|j| shape_run(node, ws, j, false)).collect();
    Ok(json!({ "items": items }))
}
