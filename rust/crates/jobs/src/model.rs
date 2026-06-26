//! The durable job record — the resumable session state of a remote workflow (README §6.9,
//! jobs scope "the record shape, S5").
//!
//! A job is *state*: it lives in the store, addressed by `job:{id}` within a workspace, so a
//! long-running agent session survives a restart and the edge disconnecting (agent scope). The
//! transcript is **append-addressed**: `steps[i]` is the durable result of step `i`. That is what
//! makes resume idempotent — re-applying a persisted step is an upsert of the same slot, never a
//! duplicate or a re-spend (jobs scope, testing §2.3).
//!
//! No wall-clock inside the crate: `ts` is a caller-injected logical timestamp (testing §3).

use serde::{Deserialize, Serialize};

/// The lifecycle of a job. S5 needs only the resumable-session subset; the queue states
/// (`queued`/`claimed`/`dead`) land with the multi-worker queue (jobs scope, deferred past S5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JobStatus {
    /// Created, the loop has not finished — the resume point is `cursor`.
    Running,
    /// The loop finished normally (model returned no tool calls, or the ceiling was hit).
    Done,
    /// The loop ended in an unrecoverable error (recorded for audit; not retried at S5).
    Failed,
}

/// One durable step of the session transcript — the result of the agent's step `index`.
/// Addressed by `index` so re-applying it on resume is a no-op (jobs scope idempotent resume).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    /// The step's position in the loop. `steps` is kept dense and ordered by this.
    pub index: u32,
    /// The opaque step result (the agent stores its tool-call outcome / model turn here).
    pub result: String,
}

/// A durable job = a resumable workflow session. `id` is workspace-unique and stable (re-`create`
/// or any write upserts the same `job:{id}` row). `payload` is opaque to this crate — the agent
/// stores its goal + caller identity there. `cursor` is the next step index to run on resume.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    /// The dispatch kind. S5 has one: `agent-session`.
    pub kind: String,
    /// Opaque session input (the agent's goal, the caller's sub, …). Not interpreted here.
    pub payload: String,
    pub status: JobStatus,
    /// The next step index to run — the resume point. Advances only past steps that landed.
    pub cursor: u32,
    /// The append-addressed transcript; `steps[i].index == i`. Resume re-reads from `cursor`.
    pub steps: Vec<Step>,
    /// How many times this session has been (re)started — for audit, not retry policy at S5.
    pub attempts: u32,
    /// Caller-injected logical timestamp (no wall-clock — testing §3).
    pub ts: u64,
}

impl Job {
    /// Build a fresh running job. Explicit (no `Default`) so every field is a deliberate choice.
    pub fn new(
        id: impl Into<String>,
        kind: impl Into<String>,
        payload: impl Into<String>,
        ts: u64,
    ) -> Self {
        Self {
            id: id.into(),
            kind: kind.into(),
            payload: payload.into(),
            status: JobStatus::Running,
            cursor: 0,
            steps: Vec::new(),
            attempts: 1,
            ts,
        }
    }
}
