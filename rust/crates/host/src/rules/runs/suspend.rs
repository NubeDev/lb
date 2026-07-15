//! `rules.runs.suspend {run_id}` — pause a run (long-running-rules-scope). Live on this node →
//! set the cooperative pause intent; the governor bites within one bytecode op and the worker
//! parks the job `Suspended` (the durable status trails the flag by design — the worker owns the
//! unwind, job-control doctrine). Not live but `Running` (an orphan) → park the record directly.
//! Already `Suspended` → clean no-op. Terminal → clean author error.

use std::sync::Arc;

use lb_jobs::JobStatus;
use serde_json::{json, Value};

use crate::boot::Node;

use super::super::error::RulesError;
use super::get::load_run;

pub async fn rules_runs_suspend(
    node: &Arc<Node>,
    ws: &str,
    run_id: &str,
) -> Result<Value, RulesError> {
    let job = load_run(node, ws, run_id).await?;
    match job.status {
        JobStatus::Suspended => Ok(json!({ "run_id": run_id, "status": "suspended" })),
        JobStatus::Running => {
            if let Some(control) = node.rule_runs.get(ws, run_id) {
                // Live: the worker observes the flag and writes `Suspended` when it parks.
                control.request_pause();
                Ok(json!({ "run_id": run_id, "status": "suspending" }))
            } else {
                // Orphan (no live worker — e.g. after a restart): park the record directly.
                lb_jobs::suspend(&node.store, ws, run_id)
                    .await
                    .map_err(|e| RulesError::Internal(e.to_string()))?;
                Ok(json!({ "run_id": run_id, "status": "suspended" }))
            }
        }
        _ => Err(RulesError::BadInput(format!(
            "run is already {} — nothing to suspend",
            json!(job.status)
        ))),
    }
}
