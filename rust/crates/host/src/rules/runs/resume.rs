//! `rules.runs.resume {run_id}` — resume a paused (or orphaned) run (long-running-rules-scope).
//! The body replays from the top under the RESUMER's `caller ∩ grant` (no stored principal — the
//! scope's refused-forgery decision) with the persisted checkpoints folded back in: memoized
//! `job.step` blocks return as lookups, replayed writes land on their original deterministic ids.
//! `Suspended` → unsuspend + spawn. `Running` + not live (an orphan) → re-attach. `Running` +
//! live → clean no-op. Terminal → clean author error.

use std::sync::Arc;

use lb_auth::Principal;
use lb_jobs::JobStatus;
use serde_json::{json, Value};

use crate::boot::Node;

use super::super::error::RulesError;
use super::get::load_run;
use super::worker::spawn_worker;

pub async fn rules_runs_resume(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    run_id: &str,
) -> Result<Value, RulesError> {
    let job = load_run(node, ws, run_id).await?;
    match job.status {
        JobStatus::Suspended => {
            lb_jobs::unsuspend(&node.store, ws, run_id)
                .await
                .map_err(|e| RulesError::Internal(e.to_string()))?;
            spawn_worker(
                node.clone(),
                principal.clone(),
                ws.to_string(),
                run_id.to_string(),
            );
            Ok(json!({ "run_id": run_id, "status": "running" }))
        }
        JobStatus::Running if !node.rule_runs.is_live(ws, run_id) => {
            // An orphan (the node restarted mid-run): re-attach a worker; the replay is
            // idempotent so a half-finished first attempt is safe.
            spawn_worker(
                node.clone(),
                principal.clone(),
                ws.to_string(),
                run_id.to_string(),
            );
            Ok(json!({ "run_id": run_id, "status": "running" }))
        }
        JobStatus::Running => Ok(json!({ "run_id": run_id, "status": "running", "live": true })),
        _ => Err(RulesError::BadInput(format!(
            "run is {} — not resumable",
            json!(job.status)
        ))),
    }
}
