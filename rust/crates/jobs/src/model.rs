//! The durable job record — the resumable session state of a remote workflow (README §6.9,
//! jobs scope; agent-run scope Part 0 made the step typed).
//!
//! A job is *state*: it lives in the store, addressed by `job:{id}` within a workspace, so a
//! long-running agent session survives a restart and the edge disconnecting (agent scope). The
//! transcript is **append-addressed**: `steps[i]` is the durable [`TranscriptEvent`] at index `i`.
//! That is what makes resume idempotent — re-applying a persisted step is an upsert of the same
//! slot, never a duplicate or a re-spend (jobs scope, testing §2.3) — *and* what makes resume
//! **faithful**: the loop folds the events back into its exact message/`prior`/active-skill state
//! (agent-run scope Part 0), instead of re-deriving from the goal (the old opaque-`String` step
//! could not carry enough to do that).
//!
//! No wall-clock inside the crate: `ts` is a caller-injected logical timestamp (testing §3).

use serde::{Deserialize, Serialize};

use crate::transcript::{TranscriptEvent, TRANSCRIPT_SCHEMA_VERSION};

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
    /// The run is paused on a durable human decision (agent-run scope Part 2). Terminal *for the
    /// current turn* — the connection need not be held — but **restartable**: when the
    /// `agent_decision` settles, the reactor rehydrates and resumes from `cursor`.
    Suspended,
    /// The run was cancelled (agent-run scope Part 0 cancel hook — a UI stop button, ACP
    /// `session/cancel`). Terminal and **not** restartable; the transcript is kept for audit/replay.
    Cancelled,
}

impl JobStatus {
    /// Whether the loop should keep running when it loads a job in this state. `Running`/`Suspended`
    /// resume; the genuinely terminal states (`Done`/`Failed`/`Cancelled`) do not re-enter the loop.
    pub fn is_resumable(self) -> bool {
        matches!(self, JobStatus::Running | JobStatus::Suspended)
    }
}

/// One durable step of the session transcript — the [`TranscriptEvent`] at this `index`.
/// Addressed by `index` so re-applying it on resume is a no-op (jobs scope idempotent resume).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    /// The step's position in the transcript. `steps` is kept dense and ordered by this.
    pub index: u32,
    /// The typed event at this position — what makes resume faithful (agent-run scope Part 0).
    pub event: TranscriptEvent,
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
    /// The next step index to APPEND — also the count of recorded events. The loop folds
    /// `steps[..cursor]` to rehydrate; new events land at `cursor` and advance it.
    pub cursor: u32,
    /// The append-addressed transcript; `steps[i].index == i`. Resume folds these to restore state.
    pub steps: Vec<Step>,
    /// The transcript schema version this record was written under (agent-run scope Part 0 guard).
    /// A future reader compares it to [`TRANSCRIPT_SCHEMA_VERSION`] to decide whether to upconvert.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    /// How many times this session has been (re)started — for audit, not retry policy at S5.
    pub attempts: u32,
    /// Caller-injected logical timestamp (no wall-clock — testing §3).
    pub ts: u64,
}

/// Serde default for a transcript record written before `schema_version` existed (treated as v1).
fn default_schema_version() -> u32 {
    1
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
            schema_version: TRANSCRIPT_SCHEMA_VERSION,
            attempts: 1,
            ts,
        }
    }

    /// The recorded transcript events in order — the input to rehydration (agent-run scope Part 0)
    /// and to the [`RunEvent`](crate) projection (Part 1). Borrowed; the caller folds them.
    pub fn events(&self) -> impl Iterator<Item = &TranscriptEvent> {
        self.steps.iter().map(|s| &s.event)
    }
}
