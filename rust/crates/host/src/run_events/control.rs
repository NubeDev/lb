//! **Run lifecycle control** (agent-dock scope, run controls) — the durable stop / pause / resume a
//! UI (or ACP, or any MCP caller) drives a live run with, over ONE new capability gate
//! `mcp:agent.control:call`. The sibling of [`watch`](super::watch): watch is *read-only* on a run,
//! control *acts* on it — so it takes its own cap, not `agent.watch`.
//!
//! The three verbs map onto primitives that already ship (`lb_jobs`): none of this is a new
//! persistence surface, it is a thin, authorized front door onto the run-job lifecycle.
//!   - **stop**   → [`lb_jobs::cancel`] — terminal, NON-restartable. The loop notices at its next
//!     turn boundary (`is_cancelled`) and ends with a restorable transcript.
//!   - **pause**  → [`lb_jobs::suspend`] (Running → Suspended). The loop notices at its next turn
//!     boundary (`is_paused`), emits a `Suspended` `RunEvent`, and returns — the transcript + cursor
//!     are intact, so nothing is lost. A Suspended run job is NOT auto-driven (the reactor drains
//!     *enqueue* jobs, and pause retires none), so it stays paused until an explicit resume.
//!   - **resume** → [`lb_jobs::unsuspend`] (Suspended → Running) + **re-activate the enqueue job** so
//!     the reactor re-drives it; `run_session` then rehydrates from the durable cursor and continues
//!     the conversation (resume is faithful, agent-run scope Part 0). Idempotent: the drive's own
//!     `answer_already_posted` guard skips a run whose answer already landed.
//!
//! Every verb is **workspace-first** (the token's ws is the wall — a ws-B caller can neither authorize
//! for ws-A nor reach ws-A's job rows) and **opaque on deny** (`AgentError::Denied`, no leak of which
//! run exists). Authorizing to *watch* a run never implies authority to *control* it (distinct caps).

use lb_jobs::{cancel, suspend, unsuspend, Job};
use lb_mcp::authorize_tool;

use crate::agent::AgentError;
use crate::boot::Node;
use crate::channel::{ChannelAgentJob, CHANNEL_AGENT_KIND};

/// The MCP tool id whose `mcp:agent.control:call` cap gates all three lifecycle actions.
pub const AGENT_CONTROL_TOOL: &str = "agent.control";

/// **Stop** run `job_id` — the durable, non-restartable cancel (a UI stop button / ACP
/// `session/cancel`). Gated `mcp:agent.control:call`, workspace-first. Idempotent (cancelling an
/// already-cancelled run is a no-op); a `Done`/`Failed` run cannot be cancelled (returns the store's
/// honest error, mapped to `BadInput`).
pub async fn stop_run(
    node: &Node,
    principal: &lb_auth::Principal,
    ws: &str,
    job_id: &str,
) -> Result<(), AgentError> {
    authorize_tool(principal, ws, AGENT_CONTROL_TOOL).map_err(|_| AgentError::Denied)?;
    cancel(&node.store, ws, job_id)
        .await
        .map_err(|e| AgentError::BadInput(e.to_string()))
}

/// **Pause** run `job_id` — suspend it (Running → Suspended). The loop honors it at its next turn
/// boundary. Gated `mcp:agent.control:call`, workspace-first. Idempotent on an already-paused run; a
/// terminal run (Done/Failed/Cancelled) can't be paused → `BadInput`.
pub async fn pause_run(
    node: &Node,
    principal: &lb_auth::Principal,
    ws: &str,
    job_id: &str,
) -> Result<(), AgentError> {
    authorize_tool(principal, ws, AGENT_CONTROL_TOOL).map_err(|_| AgentError::Denied)?;
    suspend(&node.store, ws, job_id)
        .await
        .map_err(|e| AgentError::BadInput(e.to_string()))
}

/// **Resume** a paused run `job_id` — unsuspend it (Suspended → Running) and re-activate its channel
/// enqueue job so the background reactor re-drives it (rehydrating from the cursor). Gated
/// `mcp:agent.control:call`, workspace-first.
///
/// Re-activation flips the retired enqueue job (`q:<job_id>`) back to `Running` so
/// [`agent_reactor`](crate::agent_reactor) re-picks it — the enqueue record still carries the poster's
/// identity + caps, so the re-drive runs under the ORIGINAL asker's authority, never the resumer's
/// (co-trust is preserved). A missing enqueue job (e.g. a routed/`invoke` run with no channel worker)
/// is not an error here — unsuspend alone lets a resume driven by the caller pick it up; the UI dock
/// path always has an enqueue job.
pub async fn resume_run(
    node: &Node,
    principal: &lb_auth::Principal,
    ws: &str,
    job_id: &str,
) -> Result<(), AgentError> {
    authorize_tool(principal, ws, AGENT_CONTROL_TOOL).map_err(|_| AgentError::Denied)?;

    // Clear the pause on the RUN job first (Suspended → Running) so the loop's `is_paused` check does
    // not immediately re-pause it on the next turn. Idempotent on an already-running run.
    unsuspend(&node.store, ws, job_id)
        .await
        .map_err(|e| AgentError::BadInput(e.to_string()))?;

    // Re-activate the channel ENQUEUE job so the reactor re-drives the run. The enqueue job is
    // `q:<job_id>` and was retired `Done` by the prior drive; flipping it back to `Running` re-enters
    // the reactor's `pending()` set. Best-effort: a run with no channel enqueue job (non-dock caller)
    // simply relies on its own driver — unsuspend already reopened the run.
    reactivate_enqueue(node, ws, job_id).await;
    Ok(())
}

/// Re-arm the channel enqueue job `q:<job_id>` so the reactor's next drain re-drives the run. The
/// prior drive retired it `Done`; we **re-`create`** it from its own persisted payload — `create` is
/// an upsert and mints a fresh `Running` job, so the reactor's `pending()` set includes it again. The
/// payload (the [`ChannelAgentJob`] record with the poster's identity + caps) is preserved verbatim,
/// so the re-drive runs under the ORIGINAL asker's authority. Best-effort — a run with no channel
/// enqueue job (a non-dock caller) is simply left to its own driver; `unsuspend` already reopened it.
async fn reactivate_enqueue(node: &Node, ws: &str, job_id: &str) {
    let enqueue_id = ChannelAgentJob::job_id(job_id);
    let Ok(Some(existing)) = lb_jobs::load(&node.store, ws, &enqueue_id).await else {
        return;
    };
    // Re-`create` (upsert) a fresh Running enqueue job carrying the SAME payload — the reactor re-picks
    // it and `drive_queued_run` resumes the run from its durable cursor. Idempotent by construction:
    // if the run already completed, the drive's `answer_already_posted` guard short-circuits.
    let rearmed = Job::new(
        &enqueue_id,
        CHANNEL_AGENT_KIND,
        existing.payload,
        existing.ts,
    );
    let _ = lb_jobs::create(&node.store, ws, &rearmed).await;
}
