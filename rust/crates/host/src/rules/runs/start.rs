//! `rules.run_async {body|rule_id, params, ts?, route?, run_id?}` → `{run_id}` — start a rule
//! evaluation as a durable background job (long-running-rules-scope). Seeds the `Running` job
//! synchronously (an immediate `runs.get`/`suspend`/`cancel` finds it — the `flows_run_async`
//! precedent), then spawns the detached worker and returns at once.

use std::sync::Arc;

use lb_auth::Principal;
use lb_jobs::Job;
use serde_json::{json, Value};

use crate::boot::Node;

use super::super::error::RulesError;
use super::payload::RuleJobPayload;
use super::worker::{spawn_worker, RULE_RUN_KIND};

pub async fn rules_run_async(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, RulesError> {
    let body = input.get("body").and_then(|v| v.as_str()).map(String::from);
    let rule_id = input
        .get("rule_id")
        .and_then(|v| v.as_str())
        .map(String::from);
    if body.is_none() && rule_id.is_none() {
        return Err(RulesError::BadInput("missing body or rule_id".into()));
    }
    // A saved id is validated NOW (author feedback at start, not a failed job a minute later).
    if let (None, Some(id)) = (&body, &rule_id) {
        super::super::get::rules_get(&node.store, principal, ws, id).await?;
    }

    // A caller-supplied `run_id` is the idempotent-retry path (honored verbatim); otherwise a
    // fresh collision-proof ULID (the flows.run precedent).
    let run_id = input
        .get("run_id")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(lb_store::new_ulid);
    // The pinned logical clock — explicit `ts` wins (deterministic callers); else the host clock.
    // Reused verbatim on every resume so replayed write ids never drift.
    let now = input
        .get("ts")
        .and_then(|v| v.as_u64())
        .unwrap_or_else(super::super::now_ms);
    let route = input.get("route").and_then(|v| v.as_bool()).unwrap_or(true);

    let payload = RuleJobPayload {
        body,
        rule_id,
        params: input.get("params").cloned().unwrap_or(Value::Null),
        now,
        route,
    };
    let payload_json = serde_json::to_string(&payload)
        .map_err(|e| RulesError::Internal(format!("payload encode: {e}")))?;

    // Seed synchronously; `create` is an idempotent upsert on the id, so a retried start with the
    // same `run_id` re-seeds the same job rather than forking a second run.
    lb_jobs::create(
        &node.store,
        ws,
        &Job::new(&run_id, RULE_RUN_KIND, payload_json, now),
    )
    .await
    .map_err(|e| RulesError::Internal(e.to_string()))?;

    spawn_worker(
        node.clone(),
        principal.clone(),
        ws.to_string(),
        run_id.clone(),
    );
    Ok(json!({ "run_id": run_id }))
}
