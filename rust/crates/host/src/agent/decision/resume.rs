//! `resume_suspensions` — apply settled decisions when a suspended run re-enters the loop (agent-run
//! scope Part 2, "Resume modes"). This is the counterpart to `open_suspension`: open paused the run
//! on an Ask; this resumes past the now-settled decision.
//!
//! A suspended transcript holds, for the gated call(s): `ToolCallProposed{id}` then
//! `SuspensionOpened{tool_call_id=id}` with **no** `ToolResult{id}` yet. On resume we find those open
//! calls, read each `agent_decision` record, and apply the **resume mode**:
//!   - **Deny**  → feed the model a "denied by policy" `ToolResult` (the loop already handles tool
//!     errors; the model picks a safer path);
//!   - **Allow** → **replay** the originally-proposed call from the persisted args in the transcript
//!     (the args were captured in `ToolCallProposed` exactly for this), then record its real outcome.
//!   - `UseDecisionAsResult` is deferred (designed-for: the resume mode is an enum field; only
//!     Deny + Allow are built — scope Resolved decisions).
//!
//! For each resolved call we append a `SuspensionSettled` event (so a rehydrate knows the decision
//! bound and a re-scan is a no-op) followed by the `ToolResult`. A still-`Pending` suspension is left
//! alone — the run is not actually resumable yet (it should not have been re-entered; we return it as
//! "still pending" so the loop can re-suspend rather than ask the model prematurely).
//!
//! Idempotency: a call that already has a `ToolResult` is not re-applied (it was resolved on a prior
//! resume) — duplicate decide + reactor re-scan do not double-run the tool or re-spend.

use std::collections::HashSet;
use std::sync::Arc;

use lb_auth::Principal;
use lb_jobs::{append_event, SuspensionDecision, TranscriptEvent};

use super::model::decision_id;
use super::store::load_decision;
use crate::agent::model_access::{CallOutcome, ProposedCall};
use crate::agent::step::run_calls;
use crate::boot::Node;
use crate::AgentError;

/// The "denied by policy" result text fed to the model on a Deny resume — distinct from a capability
/// denial only in wording; the loop treats both as a tool error.
pub const DENIED_BY_POLICY: &str = "denied by policy";

/// The outcome of resolving the open suspensions on a resumed run.
pub struct ResumeOutcome {
    /// The next free transcript index after the appended settle + result events.
    pub index: u32,
    /// The tool outcomes produced (one per resolved call) — fed into the loop's `prior`/messages so
    /// the next model turn sees them.
    pub outcomes: Vec<CallOutcome>,
    /// True if a suspension was still `Pending` (no decision yet) — the run is not really resumable;
    /// the loop should treat it as still-suspended rather than continue.
    pub still_pending: bool,
}

/// Resolve every open suspension in `events` (proposed + opened, not yet resulted) for run `job_id`,
/// appending the settle + result events starting at `index`. `agent` is the derived principal a
/// replayed Allow runs under (same authority as the original dispatch).
pub async fn resume_suspensions(
    node: &Arc<Node>,
    agent: &Principal,
    ws: &str,
    job_id: &str,
    events: &[&TranscriptEvent],
    mut index: u32,
) -> Result<ResumeOutcome, AgentError> {
    let open = open_calls(events);
    let mut outcomes = Vec::new();
    let mut still_pending = false;

    for call in open {
        let did = decision_id(job_id, &call.id);
        let decision = load_decision(&node.store, ws, job_id, &call.id).await?;
        let settled = match decision.and_then(|d| d.decision) {
            Some(d) => d,
            None => {
                // No decision yet — the run was re-entered while still genuinely paused. Don't ask
                // the model; signal the loop to keep it suspended.
                still_pending = true;
                continue;
            }
        };

        // Record that the decision bound (idempotent marker), then the call's result.
        append_event(
            &node.store,
            ws,
            job_id,
            index,
            TranscriptEvent::SuspensionSettled {
                decision_id: did,
                decision: settled,
            },
        )
        .await?;
        index += 1;

        let outcome = match settled {
            SuspensionDecision::Deny => CallOutcome {
                id: call.id.clone(),
                name: call.name.clone(),
                input: call.input.clone(),
                ok: None,
                error: Some(DENIED_BY_POLICY.to_string()),
            },
            SuspensionDecision::Allow => {
                // Allow→replay: run the ORIGINALLY-proposed call from the persisted args.
                let replay = vec![call.clone()];
                run_calls(node, agent, ws, &replay)
                    .await
                    .into_iter()
                    .next()
                    .unwrap_or(CallOutcome {
                        id: call.id.clone(),
                        name: call.name.clone(),
                        input: call.input.clone(),
                        ok: None,
                        error: Some("replay produced no outcome".into()),
                    })
            }
            // `#[non_exhaustive]`: a future resume mode an old host doesn't model is treated as a
            // safe Deny rather than silently running the tool.
            _ => CallOutcome {
                id: call.id.clone(),
                name: call.name.clone(),
                input: call.input.clone(),
                ok: None,
                error: Some(DENIED_BY_POLICY.to_string()),
            },
        };

        append_event(
            &node.store,
            ws,
            job_id,
            index,
            TranscriptEvent::ToolResult {
                id: outcome.id.clone(),
                ok: outcome.ok.clone(),
                err: outcome.error.clone(),
            },
        )
        .await?;
        index += 1;
        outcomes.push(outcome);
    }

    Ok(ResumeOutcome {
        index,
        outcomes,
        still_pending,
    })
}

/// The proposed calls that have an open suspension (a `SuspensionOpened` for their id) but no
/// `ToolResult` yet — the ones a resume must resolve. Returns them in proposal order with their args.
fn open_calls(events: &[&TranscriptEvent]) -> Vec<ProposedCall> {
    let opened: HashSet<&str> = events
        .iter()
        .filter_map(|e| match e {
            TranscriptEvent::SuspensionOpened { tool_call_id, .. } => Some(tool_call_id.as_str()),
            _ => None,
        })
        .collect();
    let resulted: HashSet<&str> = events
        .iter()
        .filter_map(|e| match e {
            TranscriptEvent::ToolResult { id, .. } => Some(id.as_str()),
            _ => None,
        })
        .collect();

    events
        .iter()
        .filter_map(|e| match e {
            TranscriptEvent::ToolCallProposed { id, name, args }
                if opened.contains(id.as_str()) && !resulted.contains(id.as_str()) =>
            {
                Some(ProposedCall {
                    id: id.clone(),
                    name: name.clone(),
                    // The persisted args are the same JSON-string form `ProposedCall.input` carries
                    // (captured at proposal time) — an Allow→replay re-runs the *original* call.
                    input: args.clone(),
                })
            }
            _ => None,
        })
        .collect()
}
