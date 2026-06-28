//! The `RunEvent -> ACP session/update` encoder (agent-run scope Part 4) — a pure function, the
//! whole point of the Part-1 vocabulary: every external protocol is a thin `RunEvent -> wire`
//! mapping in its own role crate, and the loop never knows the word "ACP". The gateway SSE route is
//! one such encoder; this is another. Unit-testable with no I/O.
//!
//! ACP streams `session/update` notifications carrying an `update` payload tagged by kind
//! (agent_message_chunk, tool_call, tool_call_update, …). A [`RunEvent::Suspended`] is NOT a
//! `session/update` — it maps to a `session/request_permission` *request* the driver issues — so
//! `encode_update` returns `None` for it (and for the terminal finish, which the driver turns into a
//! prompt response with a `StopReason`). Everything else becomes one `session/update`.

use lb_run_events::{RunEvent, RunOutcome};
use serde_json::{json, Value};

/// The ACP `StopReason` a turn ends with — pinned here (Part 4 "an ACP `StopReason` … pinned during
/// build"). `end_turn` is the normal completion; `cancelled` for a cancel; `refusal` is the variant
/// we map "suspended/awaiting-permission" onto (the editor disconnected mid-permission, so the turn
/// ends without an answer and the run is resumed out-of-band via `session/resume`).
pub fn stop_reason(outcome: RunOutcome) -> &'static str {
    match outcome {
        RunOutcome::Done => "end_turn",
        RunOutcome::Failed => "refusal",
        RunOutcome::Cancelled => "cancelled",
        // The disconnect-mid-permission contract: the turn ends "suspended", mapped to an ACP
        // stop-reason the editor treats as "no answer this turn"; the run is picked back up via
        // session/resume once the decision settles out-of-band.
        RunOutcome::Suspended => "refusal",
        // `#[non_exhaustive]`: an unmodeled future outcome defaults to a refusal (no answer).
        _ => "refusal",
    }
}

/// Encode one [`RunEvent`] as the `params` of an ACP `session/update` notification for `session_id`.
/// Returns `None` for events the driver handles specially (suspension → a permission request; finish
/// → the prompt response's stop reason).
pub fn encode_update(session_id: &str, event: &RunEvent) -> Option<Value> {
    let update = match event {
        RunEvent::RunStart { .. } => return None,
        RunEvent::StepStart { .. } => return None,
        RunEvent::TextDelta { text, .. } => json!({
            "sessionUpdate": "agent_message_chunk",
            "content": { "type": "text", "text": text },
        }),
        RunEvent::ReasoningDelta { text, .. } => json!({
            "sessionUpdate": "agent_thought_chunk",
            "content": { "type": "text", "text": text },
        }),
        RunEvent::ToolCallStart { id, name } => json!({
            "sessionUpdate": "tool_call",
            "toolCallId": id,
            "title": name,
            "status": "pending",
        }),
        RunEvent::ToolCallArgsDelta { id, args } => json!({
            "sessionUpdate": "tool_call_update",
            "toolCallId": id,
            "rawInput": args,
        }),
        RunEvent::ToolCallResult { id, ok, err } => json!({
            "sessionUpdate": "tool_call_update",
            "toolCallId": id,
            "status": if err.is_some() { "failed" } else { "completed" },
            "content": ok.clone().or_else(|| err.clone()).unwrap_or_default(),
        }),
        RunEvent::SkillActivated { id } => json!({
            "sessionUpdate": "plan",
            "entries": [ { "content": format!("activated skill {id}"), "status": "completed" } ],
        }),
        // Handled by the driver as a request_permission / stop-reason, not a streamed update.
        RunEvent::Suspended { .. } | RunEvent::Settled { .. } | RunEvent::RunFinish { .. } => {
            return None
        }
        // `#[non_exhaustive]`: a future RunEvent variant this pinned encoder doesn't model is simply
        // not streamed (forward-compatible), never a panic.
        _ => return None,
    };
    Some(json!({ "sessionId": session_id, "update": update }))
}
