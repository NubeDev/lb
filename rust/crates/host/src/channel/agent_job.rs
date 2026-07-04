//! The **durable enqueue record** for a background channel agent run (run-lifecycle #5). The inline
//! v1 worker drove the run right inside [`channel::post`](super::post), so the run was tied to the
//! held POST connection — closing the tab mid-run cancelled it, and a node restart lost it. This
//! record is how the run is instead **detached and made durable**: the worker persists one of these
//! as an `lb_jobs` job, `post` returns immediately, and the background [`agent_reactor`](super::super::agent_reactor)
//! (which holds `Arc<Node>`) drains it and drives the run.
//!
//! The record carries everything the reactor needs to drive the run *exactly as the inline worker
//! would have* — under the **poster's** authority, not the reactor's. The poster's `sub` + `caps` are
//! carried so the reactor reconstructs the poster principal via [`Principal::routed`], the SAME
//! co-trust reconstruction the routed-agent hub already performs (`agent::route`): in-process,
//! workspace-scoped, unsigned. It is never used to *widen* — the run's effective grant is still
//! `agent_caps ∩ poster.caps` at every tool call.
//!
//! **Two jobs, two responsibilities.** This enqueue job (`kind: CHANNEL_AGENT_KIND`) is the durable
//! "a run is queued for this channel" signal the reactor drains; the *run itself* is a separate
//! `agent-session` job keyed on `run_job` that `run_session` owns (transcript, cursor, resume). They
//! never collide: the enqueue job id is `q:<run_job>`, the run job id is `<run_job>`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The `lb_jobs` kind tag for a queued channel agent run — what the reactor scans for.
pub const CHANNEL_AGENT_KIND: &str = "channel-agent-run";

/// The durable payload of a queued channel agent run. Serialized into the enqueue job's opaque
/// `payload` field; the reactor deserializes it to drive the run and post the result back.
// `Eq` is intentionally NOT derived: the additive `context` field is a `serde_json::Value`, which is
// only `PartialEq` (floats). `PartialEq` is all the tests need (round-trip equality).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelAgentJob {
    /// The channel the request was posted into (where the `agent_result`/`agent_error` goes back).
    pub cid: String,
    /// The agent goal.
    pub goal: String,
    /// The runtime selector (`None` → in-house default; a profile id → external). Same seam as inline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<String>,
    /// The durable *run* id (the `job` the UI minted). The run job is keyed on this; the posted
    /// answer item's id is `a:<run_job>`, which is ALSO the idempotency key (skip if it already exists).
    pub run_job: String,
    /// Optional **page context** (agent-dock scope) — the client-reported `{ surface, path, search }`
    /// object carried from the `kind:"agent"` payload so the reactor fences it into the run's goal
    /// exactly as an inline drive would. `#[serde(default)]` so an older enqueue record (no context)
    /// deserializes as `None`, and absent → the drive is byte-identical to today.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<Value>,
    /// The poster's global identity — carried so the reactor reconstructs the poster principal
    /// (`Principal::routed`) and drives the run under the ASKER's authority (audit shows the asker).
    pub poster_sub: String,
    /// The poster's held caps — the upper bound of the run's effective grant (`agent ∩ poster`).
    /// Co-trust reconstruction only (unsigned), identical to the routed-agent hub path.
    pub poster_caps: Vec<String>,
    /// The logical timestamp the request landed at — the run + result item order after the request.
    pub ts: u64,
}

impl ChannelAgentJob {
    /// The enqueue job id for a run: `q:<run_job>`. Distinct from the run job's own id (`<run_job>`)
    /// so the two durable records never collide. Idempotent — re-enqueuing the same run upserts.
    pub fn job_id(run_job: &str) -> String {
        format!("q:{run_job}")
    }

    /// The correlated answer item id: `a:<run_job>` — what the worker posts and the reactor checks
    /// for idempotency (a drained run whose answer already landed is not re-driven).
    pub fn result_item_id(run_job: &str) -> String {
        format!("a:{run_job}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_distinct_and_derived_from_the_run_id() {
        assert_eq!(ChannelAgentJob::job_id("run-1"), "q:run-1");
        assert_eq!(ChannelAgentJob::result_item_id("run-1"), "a:run-1");
        // The enqueue job id and the run job id must never be equal (they share the `job` table).
        assert_ne!(ChannelAgentJob::job_id("run-1"), "run-1");
    }

    #[test]
    fn round_trips_through_the_opaque_job_payload() {
        let j = ChannelAgentJob {
            cid: "ops".into(),
            goal: "summarize the logs".into(),
            runtime: Some("open-interpreter-default".into()),
            run_job: "run-9".into(),
            context: Some(serde_json::json!({ "surface": "dashboards" })),
            poster_sub: "user:ada".into(),
            poster_caps: vec!["mcp:agent.invoke:call".into()],
            ts: 42,
        };
        let payload = serde_json::to_string(&j).unwrap();
        let back: ChannelAgentJob = serde_json::from_str(&payload).unwrap();
        assert_eq!(j, back);
    }

    #[test]
    fn absent_runtime_is_omitted_and_round_trips_as_none() {
        let j = ChannelAgentJob {
            cid: "ops".into(),
            goal: "hi".into(),
            runtime: None,
            run_job: "run-2".into(),
            context: None,
            poster_sub: "user:ada".into(),
            poster_caps: vec![],
            ts: 1,
        };
        let payload = serde_json::to_string(&j).unwrap();
        assert!(
            !payload.contains("runtime"),
            "absent runtime dropped from wire"
        );
        assert!(
            !payload.contains("context"),
            "absent context dropped from wire (byte-identical to today)"
        );
        let back: ChannelAgentJob = serde_json::from_str(&payload).unwrap();
        assert_eq!(back.runtime, None);
        assert_eq!(back.context, None);
    }
}
