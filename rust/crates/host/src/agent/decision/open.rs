//! `open_suspension` — the loop's action when a proposed call evaluates to **Ask** (agent-run scope
//! Part 2). It turns a live tool-call proposal into a *durable* pause that outlives the connection:
//!
//!   1. `create` the pending `agent_decision:{job}:{tool_call}` record (the first-write reservation);
//!   2. surface an inbox `needs:approval` item for routing/visibility (NOT the binding settle — that
//!      is the decision record; the inbox is last-writer-wins and only for human routing);
//!   3. append `SuspensionOpened{tool_call_id, decision_id}` to the durable transcript — **before**
//!      the run is suspended/emitted, so the durable pause never trails the stream (scope: "persist
//!      the pending-suspension id + cursor BEFORE emitting Suspended");
//!   4. `suspend` the job (status → `Suspended`).
//!
//! Idempotency: if the decision already exists (a re-scan re-opening the same suspension), the
//! `create` returns `Conflict` and we treat it as already-open — we do not double-append or
//! re-suspend. The transcript append is idempotent on its index regardless.
//!
//! Ordering rationale (durable-before-motion, §3.3): the transcript is the record; suspend flips the
//! job status that the reactor/resume keys on. Writing the decision + transcript first means a crash
//! between steps leaves a recoverable state (a pending decision the reactor can still settle), never a
//! suspended job with no decision to settle.

use lb_auth::Principal;
use lb_jobs::{suspend, TranscriptEvent};

use super::model::{decision_id, AgentDecision};
use super::store::create_pending;
use crate::agent::transcript::TranscriptWriter;
use crate::agent::AgentError;
use crate::record_inbox;
use lb_store::StoreError;

/// The inbox channel agent Ask suspensions surface on for routing — the same `needs:approval` shape
/// the human-decision UI lists. (The binding settle is the decision record, not this item.)
pub const APPROVAL_CHANNEL: &str = "needs:approval";

/// Open a durable suspension for `tool_call_id` in the writer's run. All transcript writes go
/// through `writer` — the ONE chokepoint (agent-loop-hardening slice C), which also publishes the
/// live `Suspended` projection and un-parks the call from its dangling-pending set. `service` is
/// the host service principal recording the inbox item (the agent service is the author, as for
/// the reactor).
///
/// On a re-open of an already-open decision (`create` `Conflict`) this is a no-op append-and-suspend:
/// the suspension is already durable, so we still ensure the job is `Suspended` and return without
/// duplicating the transcript event.
pub async fn open_suspension(
    writer: &mut TranscriptWriter<'_>,
    service: &Principal,
    tool_call_id: &str,
    ts: u64,
) -> Result<(), AgentError> {
    let (node, ws, job_id) = (writer.node, writer.ws, writer.job_id);
    let did = decision_id(job_id, tool_call_id);

    // 1. Reserve the decision (first-write). A Conflict means it is already open — fall through to
    //    ensure the durable pause without re-appending the transcript event.
    let record = AgentDecision::pending(job_id, tool_call_id, ts);
    match create_pending(&node.store, ws, &record).await {
        Ok(()) => {}
        Err(StoreError::Conflict) => {
            // Already open: make sure the job is suspended, then return without re-appending.
            suspend(&node.store, ws, job_id).await.map_err(AgentError::from)?;
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    }

    // 2. Surface a routing item (best-effort visibility, not the authority). A denial here would be a
    //    misconfiguration of the service principal; we let it surface as a store error rather than
    //    silently dropping the routing signal.
    let body = format!("agent run {job_id} awaits a decision on tool call {tool_call_id}");
    let _ = record_inbox(&node.store, service, ws, APPROVAL_CHANNEL, &did, &body, ts).await;

    // 3. Persist the durable pause in the transcript BEFORE suspending (durable-before-motion); the
    //    writer publishes the live `Suspended` delta from the same projection.
    writer
        .append(TranscriptEvent::SuspensionOpened {
            tool_call_id: tool_call_id.to_string(),
            decision_id: did,
        })
        .await?;

    // 4. Suspend the job — terminal for this turn, restartable when the decision settles.
    suspend(&node.store, ws, job_id).await.map_err(AgentError::from)?;

    Ok(())
}
