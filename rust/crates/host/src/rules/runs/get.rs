//! `rules.runs.get {run_id}` — one run's status snapshot (long-running-rules-scope): lifecycle
//! status, whether it is live on this node, the latest progress beat, the author checkpoint keys,
//! the settle result/error when terminal, and a bounded transcript tail. The stream-shaped view is
//! a deferred `rules.runs.watch` (scope non-goal v1); this is the poll read.

use std::sync::Arc;

use lb_jobs::{Job, TranscriptEvent};
use serde_json::{json, Value};

use crate::boot::Node;

use super::super::error::RulesError;
use super::payload::{checkpoint_keys, latest_progress, reserved_value, ERROR_KEY, RESULT_KEY};
use super::worker::RULE_RUN_KIND;

/// Max transcript events echoed in the snapshot tail (the full transcript is the record, not a
/// snapshot payload — job-control-scope's bounded-tail rule).
const TAIL: usize = 20;

/// Load a `rule-run` job in the caller's workspace — the shared ws-pinned lookup every `runs.*`
/// verb uses. A missing id, a cross-workspace id, and a non-rule job are the same opaque NotFound.
pub(crate) async fn load_run(node: &Arc<Node>, ws: &str, run_id: &str) -> Result<Job, RulesError> {
    let job = lb_jobs::load(&node.store, ws, run_id)
        .await
        .map_err(|e| RulesError::Internal(e.to_string()))?
        .filter(|j| j.kind == RULE_RUN_KIND)
        .ok_or(RulesError::NotFound)?;
    Ok(job)
}

pub async fn rules_runs_get(node: &Arc<Node>, ws: &str, run_id: &str) -> Result<Value, RulesError> {
    let job = load_run(node, ws, run_id).await?;
    Ok(shape_run(node, ws, &job, true))
}

/// The JSON shape of one run (shared with `runs.list`, which omits the heavy fields).
pub(crate) fn shape_run(node: &Arc<Node>, ws: &str, job: &Job, full: bool) -> Value {
    let progress = latest_progress(job).map(|(pct, msg)| json!({ "pct": pct, "msg": msg }));
    let mut out = json!({
        "run_id": job.id,
        "status": job.status,
        "live": node.rule_runs.is_live(ws, &job.id),
        "progress": progress,
        "attempts": job.attempts,
        "ts": job.ts,
    });
    if full {
        let o = out.as_object_mut().expect("shape_run is an object");
        o.insert("checkpoints".into(), json!(checkpoint_keys(job)));
        if let Some(result) = reserved_value(job, RESULT_KEY) {
            o.insert("result".into(), result);
        }
        if let Some(error) = reserved_value(job, ERROR_KEY) {
            o.insert("error".into(), error);
        }
        let tail: Vec<Value> = job
            .steps
            .iter()
            .rev()
            .take(TAIL)
            .rev()
            .map(|s| match &s.event {
                TranscriptEvent::Checkpoint { key, .. } => {
                    json!({ "i": s.index, "kind": "checkpoint", "key": key })
                }
                TranscriptEvent::Progress { pct, msg } => {
                    json!({ "i": s.index, "kind": "progress", "pct": pct, "msg": msg })
                }
                other => json!({ "i": s.index, "kind": "event", "detail": format!("{other:?}") }),
            })
            .collect();
        o.insert("tail".into(), json!(tail));
    }
    out
}
