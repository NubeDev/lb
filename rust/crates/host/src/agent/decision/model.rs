//! The **agent_decision** record — the durable first-settle decision on one suspended tool call
//! (agent-run scope Part 2, "Ask settle → a dedicated first-settle `agent_decision` record").
//!
//! Why a dedicated record and NOT `lb_inbox::Resolution`: `Resolution` is **last-writer-wins** (it
//! upserts the same row so the coding workflow can flip a deferred item to approved later). An agent
//! Ask needs the **opposite** — once a tool call is decided and acted on, a later decision must not
//! flip it. Overloading `Resolution` would couple two consumers with opposite requirements (Resolved
//! decisions). So the binding settle lives here, keyed by `{job, tool_call}`, written with a
//! conditional first-write (`lb_store::create` — the first write binds, a second is `Conflict`). The
//! Ask still *surfaces* an inbox `needs:approval` item for routing/visibility, but the inbox row is
//! not the authority.
//!
//! **First-settle design (the invariant the test pins):** `open` `create`s the record in the
//! `Pending` state. `settle` does a **conditional flip** that only succeeds while still `Pending`;
//! the second `agent.decide` on the same `{job,tool_call}` finds it already settled and is rejected
//! (not an upsert), and a decide arriving after the tool already ran is a no-op. We model that with an
//! explicit [`DecisionState`] + a guarded update (see `settle.rs`) rather than a second `create`,
//! because `open` must `create` (to reserve the key first-write) and `settle` must update the *same*
//! row — a guarded "only-if-pending" update is the clean conditional flip.
//!
//! The `resume_mode` is a field from day one so `UseDecisionAsResult` is additive later (scope:
//! ship Allow+Deny, design for the third). No wall-clock — `ts` is caller-injected (testing §3).

use serde::{Deserialize, Serialize};

use lb_jobs::SuspensionDecision;

/// The fixed table the per-call decision records live in.
pub const DECISION_TABLE: &str = "agent_decision";

/// The lifecycle of one decision. `Pending` is the reserved (created-at-open) state; a settle flips
/// it to `Settled` exactly once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DecisionState {
    /// Opened (the run suspended); awaiting a human decision.
    Pending,
    /// Bound by the first `agent.decide`; the `decision` field carries the outcome.
    Settled,
}

/// The durable decision record at `agent_decision:{job}:{tool_call}`. Created `Pending` at suspend
/// time; flipped to `Settled` once by the binding `agent.decide`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentDecision {
    /// The job (run) this decision belongs to — part of the composite key + the resume target.
    pub job_id: String,
    /// The proposed tool call id this decision gates — part of the composite key. Matches the
    /// `ToolCallProposed.id` / `SuspensionOpened.tool_call_id` in the transcript.
    pub tool_call_id: String,
    pub state: DecisionState,
    /// The bound outcome — `None` while `Pending`, `Some` once `Settled`.
    #[serde(default)]
    pub decision: Option<SuspensionDecision>,
    /// Caller-injected logical timestamp of the open (and updated on settle). No wall-clock.
    pub ts: u64,
}

impl AgentDecision {
    /// A fresh pending decision (written at `open` with `lb_store::create`, so a second open on the
    /// same key is rejected — the suspension is reserved once).
    pub fn pending(job_id: impl Into<String>, tool_call_id: impl Into<String>, ts: u64) -> Self {
        Self {
            job_id: job_id.into(),
            tool_call_id: tool_call_id.into(),
            state: DecisionState::Pending,
            decision: None,
            ts,
        }
    }
}

/// The deterministic record id for a decision — `{job}:{tool_call}`. Stable so a re-scan / duplicate
/// `agent.decide` addresses the same row (the first-settle guard makes the duplicate a no-op).
pub fn decision_id(job_id: &str, tool_call_id: &str) -> String {
    format!("{job_id}:{tool_call_id}")
}
