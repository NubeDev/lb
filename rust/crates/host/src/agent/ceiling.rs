//! The **graceful ceiling exit** (agent-loop-hardening slice B): when the step ceiling ends a run
//! mid-work, make one final **tools-free** completion asking the model to summarize where it got
//! to — the user reads a coherent wrap-up instead of a bare "max steps exceeded". One bounded call
//! (no tools advertised, so nothing to propose); any fault or empty answer degrades to the plain
//! ceiling note (the honest fallback, never a fabricated summary). In-house runtime only.
//!
//! Note on budget: the scope gates this call on `max_run_tokens`; that budget (close-out slice B)
//! has not shipped, so the summary is currently unconditional — bounded to exactly one call. When
//! the run-token budget lands, the gate composes here (skip if the remaining budget won't cover
//! it; token exhaustion keeps the plain terminal event).

use lb_jobs::TranscriptEvent;

use super::error::AgentError;
use super::model_access::{CallOutcome, ModelAccess};
use super::rehydrate::LoopState;
use super::run::MAX_STEPS;
use super::transcript::TranscriptWriter;

/// The wrap-up prompt appended for the summary turn (live context only).
const SUMMARY_PROMPT: &str = "[the run has reached its turn ceiling and will stop now — in a few \
    sentences, summarize what you accomplished, what remains, and what you would do next]";

/// One tools-free summary turn over the run's final conversation. `Some(text)` on a non-empty
/// completion; `None` on a fault or an empty answer (the caller keeps the plain ceiling note).
pub(super) async fn ceiling_summary<M: ModelAccess>(
    model: &M,
    ws: &str,
    job_id: &str,
    messages: &[(String, String)],
    prior: &[CallOutcome],
) -> Option<String> {
    let mut msgs = messages.to_vec();
    msgs.push(("user".into(), SUMMARY_PROMPT.into()));
    // Its own idempotency key: a resume of a ceiling-terminal run replays the cached summary.
    let key = format!("{ws}:{job_id}:ceiling-summary");
    match model.turn(ws, &msgs, &[], prior, &key).await {
        Ok(turn) if !turn.content.trim().is_empty() => Some(turn.content),
        _ => None,
    }
}

/// The full ceiling exit: one tools-free summary (persisted as a normal assistant turn so the
/// transcript + watchers carry it), then the honest note — the summary leads, the note follows;
/// a fault/empty summary degrades to the prior answer + note. Returns the run's final answer.
pub(super) async fn wrap_up_at_ceiling<M: ModelAccess>(
    model: &M,
    writer: &mut TranscriptWriter<'_>,
    state: &LoopState,
    turn_no: u32,
) -> Result<String, AgentError> {
    let (ws, job_id) = (writer.ws, writer.job_id);
    let note = format!(
        "[the run stopped at its {MAX_STEPS}-turn ceiling before the agent finished; \
         tool effects already applied are saved — ask again to continue the task]"
    );
    let wrap_up = ceiling_summary(model, ws, job_id, &state.messages, &state.prior).await;
    if let Some(summary) = &wrap_up {
        writer.turn = turn_no;
        writer
            .append(TranscriptEvent::AssistantTurn {
                content: summary.clone(),
            })
            .await?;
    }
    let lead = wrap_up.unwrap_or_else(|| state.last_content.clone());
    Ok(if lead.is_empty() {
        note
    } else {
        format!("{lead}\n\n{note}")
    })
}
