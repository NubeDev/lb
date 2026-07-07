//! Rehydrate the loop's working state from the durable transcript (agent-run scope Part 0). This is
//! the fix for the old `run.rs` behavior: resume used to rebuild the message list from the **goal
//! alone** and start `prior` empty, so a resumed run re-asked the model from scratch — fine for a
//! one-shot answer, *wrong* once a run can pause mid-conversation. Here we fold the recorded
//! [`TranscriptEvent`]s back into the exact `messages` / `prior` / active-skills the live loop held,
//! so a resumed run **continues the conversation**.
//!
//! The transcript is the **record**; this fold is a deterministic projection of it (the same
//! discipline as the Part 1 `RunEvent` projection — state vs motion, §3.3). Pure: no store, no bus,
//! no clock — it takes the already-loaded events and returns the working state, so it is trivially
//! unit-testable and identical whether called on a fresh run (empty transcript) or a resume.

use lb_jobs::TranscriptEvent;

use super::model_access::CallOutcome;

/// The loop state reconstructed from the transcript — everything the next model turn needs.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LoopState {
    /// The running conversation (role, content) the model sees, in order.
    pub messages: Vec<(String, String)>,
    /// The previous turn's tool outcomes, fed to the next turn (empty on a fresh run or right after
    /// an assistant-only turn).
    pub prior: Vec<CallOutcome>,
    /// The skills the model has activated so far — re-loaded into context on resume so an activated
    /// skill survives (Part 5 depends on this).
    pub active_skills: Vec<String>,
    /// The text of the last assistant turn — the run's "answer so far", returned if the loop is
    /// already complete on load.
    pub last_content: String,
}

/// Fold the durable `events` (the job's `steps[..cursor]`, in order) into the [`LoopState`] the loop
/// resumes from. `system` + `goal` seed the conversation exactly as a fresh run would, then each
/// recorded event is replayed in index order.
pub fn rehydrate(system: &str, goal: &str, events: &[&TranscriptEvent]) -> LoopState {
    let mut state = LoopState {
        messages: vec![
            ("system".into(), system.to_string()),
            ("user".into(), goal.to_string()),
        ],
        ..LoopState::default()
    };

    // The tool outcomes accumulated within the current turn — flushed to `messages` + `prior` when
    // the turn's results are all in (the live loop pushed one combined "tool" message per turn).
    let mut pending: Vec<CallOutcome> = Vec::new();
    // id → (name, args) of every proposed call, so each result folds back with its call context.
    let mut proposed: std::collections::HashMap<String, (String, String)> =
        std::collections::HashMap::new();
    let flush = |pending: &mut Vec<CallOutcome>, state: &mut LoopState| {
        if pending.is_empty() {
            return;
        }
        state.messages.push(("tool".into(), summarize(pending)));
        state.prior = std::mem::take(pending);
    };

    for event in events {
        match event {
            TranscriptEvent::AssistantTurn { content } => {
                // A new assistant turn means the previous turn's tool results are settled.
                flush(&mut pending, &mut state);
                state.last_content = content.clone();
                if !content.is_empty() {
                    state.messages.push(("assistant".into(), content.clone()));
                }
            }
            // Proposed calls are not replayed into `messages` (the model already saw them as its own
            // output); only their *results* re-enter the conversation. The args live on the record
            // for an `Allow→replay` resume (Part 2), read directly off the transcript there — and
            // are remembered here so the paired `ToolResult` outcome carries name+input (the
            // provider echoes them as the assistant `tool_calls` message the wire shape requires).
            TranscriptEvent::ToolCallProposed { id, name, args } => {
                proposed.insert(id.clone(), (name.clone(), args.clone()));
            }
            TranscriptEvent::ToolResult { id, ok, err } => {
                let (name, input) = proposed.get(id).cloned().unwrap_or_default();
                pending.push(CallOutcome {
                    id: id.clone(),
                    name,
                    input,
                    ok: ok.clone(),
                    error: err.clone(),
                })
            }
            TranscriptEvent::SkillActivated { id } => {
                if !state.active_skills.iter().any(|s| s == id) {
                    state.active_skills.push(id.clone());
                }
            }
            // Suspension bookkeeping does not change the message view — it gates *whether* the loop
            // proceeds (handled in `run.rs`), not what the model has seen.
            TranscriptEvent::SuspensionOpened { .. }
            | TranscriptEvent::SuspensionSettled { .. } => {}
            // `#[non_exhaustive]`: a future variant an old host doesn't model is ignored for the fold
            // (it cannot change the message view it doesn't understand) rather than panicking.
            _ => {}
        }
    }
    flush(&mut pending, &mut state);
    state
}

/// The compact, durable summary of a turn's tool outcomes — the "tool" message the model reads next
/// turn. Kept identical in shape to what the live loop pushes so live and rehydrated views match.
pub fn summarize(outcomes: &[CallOutcome]) -> String {
    let parts: Vec<String> = outcomes
        .iter()
        .map(|o| match (&o.ok, &o.error) {
            (Some(ok), _) => format!("{}=ok:{ok}", o.id),
            (_, Some(err)) => format!("{}=err:{err}", o.id),
            _ => format!("{}=empty", o.id),
        })
        .collect();
    parts.join("; ")
}
