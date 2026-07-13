//! Transcript-structural **context compaction** (agent-loop-hardening slice A). The loop sends the
//! whole conversation every step; when it (plus the serialized tool schemas) outgrows the budget,
//! the oldest whole **turn groups** are dropped from the *live* message list — an assistant message
//! and the tool/user messages that follow it are one atomic unit, never split (splitting produces
//! provider-invalid transcripts, the bug class every hand-rolled compactor hits).
//!
//! What is never dropped: **system** messages wherever they sit (the seed, the skill catalog, the
//! memory index, activated skill bodies — dropping them silently changes the run's grounding), the
//! **first user message** (the goal), and the **latest group** (the exchange the next turn answers,
//! including the tool summary paired with the provider's `prior` echo). Every drop is announced by
//! one visible breadcrumb line — loss is never silent.
//!
//! Compaction transforms only the **live context**; the durable transcript (the job record) keeps
//! every event — nothing is lost from the record, so a resume re-folds the full history and
//! re-compacts deterministically to the same view. Pure: messages in, messages out, no store/clock.

use super::model_access::AllowedTool;

/// The node-default compaction budget in **estimated tokens** (chars/4 — a threshold heuristic,
/// never billing; close-out slice A owns real usage). ~192 KB of context: comfortably inside every
/// mainstream 128k-token model while leaving room for the completion. Workspace-tunable via
/// `agent.config.compact_budget`.
pub const DEFAULT_COMPACT_BUDGET_TOKENS: u32 = 48_000;

/// The breadcrumb's stable prefix — recognized (and replaced) on re-compaction so one cumulative
/// line, not a trail of them, marks the loss.
pub const BREADCRUMB_PREFIX: &str = "[earlier steps compacted:";

/// Estimate the token cost of a conversation: every message's role + content, chars/4. A threshold
/// heuristic (deliberately simple and deterministic), never a billing number.
pub fn estimate_message_tokens(messages: &[(String, String)]) -> u32 {
    let chars: usize = messages.iter().map(|(r, c)| r.len() + c.len()).sum();
    (chars / 4) as u32
}

/// Estimate the token cost of the advertised tool schemas — they ride every request, so the
/// preflight must count them (a large catalog can dwarf the conversation).
pub fn estimate_tool_tokens(tools: &[AllowedTool]) -> u32 {
    let chars: usize = tools
        .iter()
        .map(|t| {
            t.name.len()
                + t.description.len()
                + t.input_schema
                    .as_ref()
                    .map(|s| s.to_string().len())
                    .unwrap_or(0)
        })
        .sum();
    (chars / 4) as u32
}

/// Compact `messages` until `estimate_message_tokens(messages) + extra_tokens <= budget_tokens` (or
/// nothing droppable remains), dropping the oldest whole turn groups. `dropped_before` is the run's
/// cumulative drop count from earlier compactions (the breadcrumb shows the total). Returns how
/// many groups were newly dropped (0 = already under budget, or nothing left to drop).
pub fn compact_to_budget(
    messages: &mut Vec<(String, String)>,
    budget_tokens: u32,
    extra_tokens: u32,
    dropped_before: u32,
) -> u32 {
    let over = |m: &[(String, String)]| estimate_message_tokens(m) + extra_tokens > budget_tokens;
    if !over(messages) {
        return 0;
    }

    // Remove a previous breadcrumb — it is re-inserted below with the cumulative count.
    messages
        .retain(|(role, content)| !(role == "system" && content.starts_with(BREADCRUMB_PREFIX)));

    let mut dropped = 0u32;
    while over(messages) {
        let Some(group) = oldest_droppable_group(messages) else {
            break;
        };
        messages.drain(group.0..group.1);
        dropped += 1;
    }

    if dropped > 0 {
        let total = dropped_before + dropped;
        let at = breadcrumb_slot(messages);
        messages.insert(
            at,
            (
                "system".into(),
                format!("{BREADCRUMB_PREFIX} {total} turns]"),
            ),
        );
    }
    dropped
}

/// The half-open index range of the oldest droppable turn group, or `None` when only protected
/// content remains. A group starts at a non-system, non-goal message and runs until the next
/// assistant/user message or a system message (system lines are carved out — never dropped). The
/// LAST group is protected (the exchange the next turn answers).
fn oldest_droppable_group(messages: &[(String, String)]) -> Option<(usize, usize)> {
    let goal = first_user_index(messages)?;

    // Candidate group starts: every non-system message after the goal.
    let mut starts: Vec<usize> = Vec::new();
    for (i, (role, _)) in messages.iter().enumerate() {
        if i > goal && role != "system" {
            // A "tool" message belongs to the assistant message before it — only an "assistant" or
            // "user" message begins a group. A tool message directly after a system line (possible
            // when a skill body was injected mid-turn) joins the group in progress, not a new one.
            if role == "assistant" || role == "user" {
                starts.push(i);
            } else if starts.is_empty() {
                // A leading orphan tool summary (rehydrated fold) is its own droppable group.
                starts.push(i);
            }
        }
    }
    // Protect the tail: the latest group always (the exchange the next turn answers), and — when a
    // later user message exists (a nudge, a follow-up) — everything from that latest user group on
    // (the scope's "always keep … the latest user group").
    let last_user = starts.iter().rposition(|&i| messages[i].0 == "user");
    let protect_from = last_user.unwrap_or(starts.len() - 1).min(starts.len() - 1);
    if protect_from == 0 {
        return None;
    }
    let start = starts[0];
    // The group ends at the next group start, but never swallows a system message: stop early at
    // the first system line inside the range (it stays; the drain runs up to it, and the remainder
    // of the group — after the system line — becomes the next candidate on the following pass).
    let hard_end = starts[1];
    let end = (start..hard_end)
        .find(|&i| messages[i].0 == "system")
        .unwrap_or(hard_end);
    Some((start, end))
}

/// Where the breadcrumb lands: right after the protected head (the leading system seed + the first
/// user message), so it reads as the opening of the (compacted) history.
fn breadcrumb_slot(messages: &[(String, String)]) -> usize {
    match first_user_index(messages) {
        Some(goal) => goal + 1,
        None => messages.len(),
    }
}

/// The index of the first "user" message — the goal, the anchor of the protected head.
fn first_user_index(messages: &[(String, String)]) -> Option<usize> {
    messages.iter().position(|(role, _)| role == "user")
}
