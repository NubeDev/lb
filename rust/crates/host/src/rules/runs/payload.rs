//! The `rule-run` job payload + the checkpoint fold (long-running-rules-scope). The payload pins
//! everything a resume must replay identically: the body (or saved id), the params, the logical
//! `now` (deterministic ids across attempts), and the `route` flag. Checkpoint/result state is NOT
//! here — it lives in the transcript (append-addressed, replay-idempotent).

use lb_jobs::{Job, TranscriptEvent};
use serde::{Deserialize, Serialize};

/// Reserved checkpoint keys the worker writes on settle; folded OUT of the author-visible state.
pub const RESULT_KEY: &str = "__result";
pub const ERROR_KEY: &str = "__error";

/// The durable input of one job-backed rule run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleJobPayload {
    pub body: Option<String>,
    pub rule_id: Option<String>,
    /// Raw JSON params (re-coerced to rhai on every attempt — same input, same run).
    #[serde(default)]
    pub params: serde_json::Value,
    /// The pinned logical clock (ms). Reused on every resume so deterministic ids never drift.
    pub now: u64,
    /// The alert fan-out flag (routed once, on successful completion).
    #[serde(default = "default_route")]
    pub route: bool,
}

fn default_route() -> bool {
    true
}

impl RuleJobPayload {
    pub fn parse(job: &Job) -> Result<Self, String> {
        serde_json::from_str(&job.payload).map_err(|e| format!("bad rule-run payload: {e}"))
    }
}

/// Fold the transcript's checkpoints into the resume state map — last write per key wins,
/// reserved (`__`-prefixed) keys excluded. What `job.get`/`job.step` see on replay.
pub fn fold_checkpoints(job: &Job) -> rhai::Map {
    let mut state = rhai::Map::new();
    for step in &job.steps {
        if let TranscriptEvent::Checkpoint { key, value } = &step.event {
            if key.starts_with("__") {
                continue;
            }
            let json: serde_json::Value =
                serde_json::from_str(value).unwrap_or(serde_json::Value::Null);
            state.insert(key.as_str().into(), lb_rules::json_to_dynamic(&json));
        }
    }
    state
}

/// The latest progress beat in the transcript, if any — `(pct, msg)`.
pub fn latest_progress(job: &Job) -> Option<(Option<u32>, String)> {
    job.steps.iter().rev().find_map(|s| match &s.event {
        TranscriptEvent::Progress { pct, msg } => Some((*pct, msg.clone())),
        _ => None,
    })
}

/// A reserved-key checkpoint value (the settle result / error), if recorded.
pub fn reserved_value(job: &Job, key: &str) -> Option<serde_json::Value> {
    job.steps.iter().rev().find_map(|s| match &s.event {
        TranscriptEvent::Checkpoint { key: k, value } if k == key => {
            serde_json::from_str(value).ok()
        }
        _ => None,
    })
}

/// The author-visible checkpoint keys (dedup'd, transcript order) — the `runs.get` inventory.
pub fn checkpoint_keys(job: &Job) -> Vec<String> {
    let mut keys = Vec::new();
    for step in &job.steps {
        if let TranscriptEvent::Checkpoint { key, .. } = &step.event {
            if !key.starts_with("__") && !keys.iter().any(|k| k == key) {
                keys.push(key.clone());
            }
        }
    }
    keys
}
