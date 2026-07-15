//! `rules.runs.cancel {run_id}` — cancel a run from ANY non-final state (job-control D2:
//! suspended cancels like running; a re-cancel of a final run is a clean no-op). Live → set the
//! cooperative cancel intent (outranks a pending pause); the governor bites within one bytecode
//! op and the worker writes `Cancelled`. Not live → cancel the record directly.

use std::sync::Arc;

use lb_jobs::JobStatus;
use serde_json::{json, Value};

use crate::boot::Node;

use super::super::error::RulesError;
use super::get::load_run;

pub async fn rules_runs_cancel(
    node: &Arc<Node>,
    ws: &str,
    run_id: &str,
) -> Result<Value, RulesError> {
    let job = load_run(node, ws, run_id).await?;
    match job.status {
        JobStatus::Cancelled => Ok(json!({ "run_id": run_id, "status": "cancelled" })),
        JobStatus::Done | JobStatus::Failed => {
            // Final — a cancel is a clean no-op that reports the honest status (D2).
            Ok(json!({ "run_id": run_id, "status": job.status }))
        }
        JobStatus::Running if node.rule_runs.is_live(ws, run_id) => {
            let control = node
                .rule_runs
                .get(ws, run_id)
                .expect("is_live checked above");
            control.request_cancel();
            Ok(json!({ "run_id": run_id, "status": "cancelling" }))
        }
        // Orphaned `Running` or parked `Suspended`: cancel the record directly.
        JobStatus::Running | JobStatus::Suspended => {
            lb_jobs::cancel(&node.store, ws, run_id)
                .await
                .map_err(|e| RulesError::Internal(e.to_string()))?;
            Ok(json!({ "run_id": run_id, "status": "cancelled" }))
        }
    }
}
