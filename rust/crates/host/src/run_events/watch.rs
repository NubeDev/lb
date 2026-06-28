//! `agent.watch {job_id}` — observe a run live (agent-run scope Part 3). The headline core add: the
//! browser UI (and any MCP caller) sees a run's `RunEvent` stream — text/tool-call/skill/suspension
//! deltas — instead of only a final answer. This is **motion** (a `watch`, not a polled `list`, §3.3),
//! surfaced over the gateway SSE route that mirrors `channel_stream`.
//!
//! **A late watcher gets a transcript SNAPSHOT then deltas** (review point 5). The snapshot is the
//! [`project`](lb_run_events::project)ion of the *durable transcript* — the same projection the live
//! loop emits — so a watcher that joins mid-run (or reconnects) reconstructs state from the record,
//! never from replayed deltas it missed. Live and replay can't drift because they are the same
//! function of the same record.
//!
//! Authorization is the `mcp:agent.watch:call` capability through the shared chokepoint, workspace-
//! first: a ws-B principal can neither authorize for ws-A nor (structurally) subscribe to ws-A's
//! subject (`lb_bus` walls it under `ws/{id}/`). Settling/driving a run needs *other* caps; watch is
//! read-only on the run.

use lb_auth::Principal;
use lb_bus::{subscribe, Bus};
use lb_mcp::authorize_tool;
use lb_run_events::{project, RunEvent};
use lb_store::Store;

use super::subject::run_subject;
use crate::agent::AgentError;
use crate::run_events::stream::RunEventSub;

/// What a watcher receives on attach: the catch-up `snapshot` (the projection of the durable
/// transcript so far) followed by the live `stream` (subsequent `RunEvent` deltas). The SSE route
/// emits the snapshot events first, then folds the stream — so a late join is seamless.
pub struct RunWatch {
    /// The transcript-derived catch-up — what the run has emitted up to now (review point 5).
    pub snapshot: Vec<RunEvent>,
    /// The live delta feed for everything after the snapshot.
    pub stream: RunEventSub,
}

/// Begin watching run `job_id` in workspace `ws` as `principal`. Gated `mcp:agent.watch:call`
/// (opaque deny). Reads the durable job for the snapshot, then subscribes to its event subject for
/// live deltas. `None`-job (absent or cross-workspace) yields an empty snapshot but a live
/// subscription — a watcher may legitimately attach before the run's first event lands.
pub async fn watch_run(
    store: &Store,
    bus: &Bus,
    principal: &Principal,
    ws: &str,
    job_id: &str,
) -> Result<RunWatch, AgentError> {
    authorize_tool(principal, ws, "agent.watch").map_err(|_| AgentError::Denied)?;

    // Subscribe BEFORE reading the snapshot so no delta can slip through the gap between the
    // snapshot read and the subscription starting (a delta that arrives during the read is buffered
    // by the subscription; the SSE consumer may see one snapshot/delta overlap, which is benign —
    // the events are idempotent projections, and a UI keys them by tool-call id / turn).
    let inner = subscribe(bus, ws, &run_subject(job_id))
        .await
        .map_err(|e| AgentError::Store(lb_store::StoreError::Backend(e.to_string())))?;

    let snapshot = match lb_jobs::load(store, ws, job_id).await? {
        Some(job) => project(&job),
        None => Vec::new(),
    };

    Ok(RunWatch {
        snapshot,
        stream: RunEventSub::new(inner),
    })
}
