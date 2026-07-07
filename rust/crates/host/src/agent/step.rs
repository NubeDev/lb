//! The loop's per-step **helpers** — extracted from `run.rs` so the orchestrator there stays within
//! the one-responsibility / ≤400-line file budget (FILE-LAYOUT §3). These are the small, reused
//! mechanics the loop calls each turn: the durable-append-then-emit ([`emit`]), the cancellation
//! check, the turn counter, and running a batch of proposed calls under the derived principal.
//!
//! Kept together because they share the loop's vocabulary (transcript events + `RunEvent` motion);
//! the *policy* and *decision* and *catalog* mechanics live in their own sibling modules.

use std::sync::Arc;

use lb_auth::Principal;
use lb_jobs::{append_event, load, JobStatus, TranscriptEvent};
use lb_run_events::project_one;

use super::error::AgentError;
use super::model_access::{CallOutcome, ProposedCall};
use crate::boot::Node;
use crate::run_events::publish_run_event;
use crate::tool_call::call_tool;

/// Re-read the durable job to see whether it was cancelled mid-run (agent-run scope Part 0). Cheap at
/// S5 scale (one record); a tighter signal (a bus subject) is a follow-up, but the durable check is
/// the authority — a `cancel` written by a UI stop button / ACP `session/cancel` between turns.
pub(super) async fn is_cancelled(node: &Node, ws: &str, job_id: &str) -> Result<bool, AgentError> {
    Ok(load(&node.store, ws, job_id)
        .await?
        .map(|j| j.status == JobStatus::Cancelled)
        .unwrap_or(false))
}

/// Re-read the durable job to see whether it was **paused** mid-run (agent-dock run controls). A
/// `pause_run` (a UI pause button) flips the job to `Suspended` between turns; the loop honors it at
/// the next turn boundary, emits a `Suspended` `RunEvent`, and returns — the transcript + cursor are
/// intact, so a later `resume_run` continues faithfully. Distinct from `is_cancelled`: pause is
/// **restartable** (`Suspended`), cancel is terminal (`Cancelled`). Cheap (one record read), same as
/// the cancel check.
pub(super) async fn is_paused(node: &Node, ws: &str, job_id: &str) -> Result<bool, AgentError> {
    Ok(load(&node.store, ws, job_id)
        .await?
        .map(|j| j.status == JobStatus::Suspended)
        .unwrap_or(false))
}

/// Append a transcript event durably AND emit its live [`RunEvent`](lb_run_events::RunEvent)
/// projection onto the run's bus subject (Part 3). The order is deliberate: the durable append is the
/// **record** (it must land, hence `?`), then the bus publish is **motion** (best-effort — a watcher
/// that misses it catches up from the transcript snapshot, §3.3). `turn` is the loop's current turn
/// number, so a `StepStart` carries the right index; `project_one` is the SAME projection the snapshot
/// uses, so a live watcher and a reconnecting one can never diverge (Part 1, review point 5).
pub(super) async fn emit(
    node: &Node,
    ws: &str,
    job_id: &str,
    index: u32,
    event: TranscriptEvent,
    turn: u32,
) -> Result<(), AgentError> {
    append_event(&node.store, ws, job_id, index, event.clone()).await?;
    for run_event in project_one(&event, turn) {
        publish_run_event(&node.bus, ws, job_id, &run_event).await;
    }
    Ok(())
}

/// How many model turns the transcript already records — one per [`TranscriptEvent::AssistantTurn`].
/// Used to resume the turn counter (and so the ceiling + idempotency key) at the right place.
pub(super) fn count_turns(events: &[&TranscriptEvent]) -> u32 {
    events
        .iter()
        .filter(|e| matches!(e, TranscriptEvent::AssistantTurn { .. }))
        .count() as u32
}

/// Run each proposed tool call under the derived principal, collecting outcomes. The calls route
/// through the host's ONE MCP bridge [`call_tool`] — the SAME entry the gateway's `POST /mcp/call`
/// uses — so the loop reaches **host-native** verbs (`agent.memory.*`, `assets.*`, `series.*`, …) AND
/// extension tools behind the identical `authorize_tool` + per-verb caps wall (the default-agent-wiring
/// fix; previously `lb_mcp::call` resolved only the extension registry, so a host-native verb was
/// `NotFound`). Authorization runs workspace-first under `agent = agent_caps ∩ caller.caps`, so a tool
/// the intersection forbids is `Denied` — captured as an error outcome (the model is told; the loop
/// continues). `skill.activate` is the loop-internal built-in intercepted in `run.rs` BEFORE this, so
/// it never reaches the bridge. `pub(crate)` so the Part-2 `decision/resume` path can replay an
/// `Allow→replay` call through the identical mechanism.
pub(crate) async fn run_calls(
    node: &Arc<Node>,
    agent: &Principal,
    ws: &str,
    calls: &[ProposedCall],
) -> Vec<CallOutcome> {
    let mut outcomes = Vec::with_capacity(calls.len());
    for c in calls {
        let outcome = match call_tool(node, agent, ws, &c.name, &c.input).await {
            Ok(out) => CallOutcome {
                id: c.id.clone(),
                name: c.name.clone(),
                input: c.input.clone(),
                ok: Some(out),
                error: None,
            },
            Err(e) => CallOutcome {
                id: c.id.clone(),
                name: c.name.clone(),
                input: c.input.clone(),
                ok: None,
                error: Some(e.to_string()),
            },
        };
        outcomes.push(outcome);
    }
    outcomes
}
