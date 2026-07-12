//! Attempt one model turn through the fault lanes (agent-loop-hardening slices D + A): a
//! **transient** fault (network, timeout, 429/5xx) is retried mechanically, bounded, honoring
//! `Retry-After` — *below* step accounting, so the loop sees one turn however many attempts it took
//! (same turn number, same idempotency key; the gateway never caches a fault, so a retry really
//! re-calls the provider). An **overflow** fault is recovered by compacting the live context and
//! continuing the *same* run (slice A), never retried verbatim. A **fatal** fault (or an exhausted
//! transient/overflow) is returned to the loop, which ends the run honestly ([`fail_run`]).
//!
//! Compaction also runs as a **preflight** before the first attempt: when the conversation + tool
//! schemas already exceed the budget, the oldest turn groups are dropped (breadcrumbed) without
//! waiting for the provider to reject the request.

use lb_jobs::{complete, JobStatus};
use lb_run_events::{RunEvent, RunOutcome};

use super::compact::{compact_to_budget, estimate_message_tokens};
use super::error::AgentError;
use super::model_access::{AllowedTool, CallOutcome, ModelAccess, Turn, TurnError};
use crate::boot::Node;
use crate::run_events::publish_run_event;

/// How many times one turn is attempted before a persistent transient fault turns fatal (the first
/// call + bounded retries). Deliberately small — a stuck provider should surface, not spin.
pub(super) const MAX_TURN_ATTEMPTS: u32 = 3;

/// How many compact-and-retry rounds an overflow gets before the run fails honestly. Each round
/// halves the estimate, so three rounds is already an eighth of the original context.
const MAX_OVERFLOW_ROUNDS: u32 = 3;

/// The ceiling on how long a `Retry-After` is honored. A provider asking for more than this is
/// treated as "come back later" — the run fails honestly rather than parking a live loop.
const RETRY_AFTER_CAP_SECS: u64 = 30;

/// The run's compaction bookkeeping, threaded through every turn: the (workspace-configured)
/// budget, the pre-computed tool-schema cost, and the cumulative dropped-group count (the
/// breadcrumb shows the running total).
pub(super) struct CompactState {
    pub budget_tokens: u32,
    pub tool_tokens: u32,
    pub dropped: u32,
}

impl CompactState {
    /// Compact `messages` to `target` tokens, updating the cumulative counter. Returns how many
    /// groups were newly dropped.
    fn compact(&mut self, messages: &mut Vec<(String, String)>, target: u32) -> u32 {
        let newly = compact_to_budget(messages, target, self.tool_tokens, self.dropped);
        self.dropped += newly;
        newly
    }
}

/// One model turn through the fault lanes: preflight compaction, bounded transient retry,
/// compact-and-continue overflow recovery. Returns the turn, or the **fatal** error the loop must
/// surface (transient/overflow exhaustion is folded into a fatal with the original evidence).
pub(super) async fn attempt_turn<M: ModelAccess>(
    model: &M,
    ws: &str,
    messages: &mut Vec<(String, String)>,
    tools: &[AllowedTool],
    prior: &[CallOutcome],
    idempotency_key: &str,
    compact: &mut CompactState,
) -> Result<Turn, TurnError> {
    // Preflight: over budget already → compact before spending a provider call on a rejection.
    compact.compact(messages, compact.budget_tokens);

    let mut attempt: u32 = 0;
    let mut overflow_rounds: u32 = 0;
    loop {
        match model
            .turn(ws, messages, tools, prior, idempotency_key)
            .await
        {
            Ok(turn) => return Ok(turn),
            Err(TurnError::Transient {
                detail,
                retry_after_secs,
            }) => {
                attempt += 1;
                if attempt >= MAX_TURN_ATTEMPTS {
                    // A transient that outlives the retry budget IS fatal — say so with the
                    // original evidence, never a silent stop.
                    return Err(TurnError::Fatal {
                        detail: format!(
                            "transient fault persisted after {MAX_TURN_ATTEMPTS} attempts: {detail}"
                        ),
                    });
                }
                // Honor the provider's `Retry-After` (capped); default to a linear backoff.
                let wait = retry_after_secs
                    .unwrap_or(attempt as u64)
                    .min(RETRY_AFTER_CAP_SECS);
                if wait > 0 {
                    tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
                }
            }
            Err(TurnError::Overflow { detail }) => {
                // The estimate undershot this model's real window — force below HALF the current
                // estimate and continue the same run (never a verbatim retry). No droppable group
                // left (or too many rounds) → honest failure.
                overflow_rounds += 1;
                let target = (estimate_message_tokens(messages) + compact.tool_tokens) / 2;
                let newly = compact.compact(messages, target);
                if newly == 0 || overflow_rounds > MAX_OVERFLOW_ROUNDS {
                    return Err(TurnError::Fatal {
                        detail: format!("context overflow not recoverable by compaction: {detail}"),
                    });
                }
            }
            Err(fatal) => return Err(fatal),
        }
    }
}

/// The honest terminal exit for an unrecoverable turn fault: the job is marked **Failed** (not a
/// fake `Done`), the watcher's stream ends with `RunFinish(Failed)`, and the answer carries the
/// attributed detail — the pre-slice-D behavior (a fault dressed as a normal completion) is the
/// anti-pattern this replaces. Returns the answer so the caller (channel worker, invoke) still has
/// one message to surface.
pub(super) async fn fail_run(
    node: &Node,
    ws: &str,
    job_id: &str,
    last_content: &str,
    detail: &str,
) -> Result<String, AgentError> {
    let note = format!("[run failed: {detail}]");
    let answer = if last_content.is_empty() {
        note
    } else {
        format!("{last_content}\n\n{note}")
    };
    complete(&node.store, ws, job_id, JobStatus::Failed).await?;
    publish_run_event(
        &node.bus,
        ws,
        job_id,
        &RunEvent::RunFinish {
            outcome: RunOutcome::Failed,
            answer: answer.clone(),
        },
    )
    .await;
    Ok(answer)
}

/// The normal terminal exit: mark the job **Done** and end the watcher's stream with
/// `RunFinish(Done)` carrying the final answer (best-effort motion — the durable status is the
/// record; `project` derives the same RunFinish on reattach).
pub(super) async fn finish_run(
    node: &Node,
    ws: &str,
    job_id: &str,
    answer: String,
) -> Result<String, AgentError> {
    complete(&node.store, ws, job_id, JobStatus::Done).await?;
    publish_run_event(
        &node.bus,
        ws,
        job_id,
        &RunEvent::RunFinish {
            outcome: RunOutcome::Done,
            answer: answer.clone(),
        },
    )
    .await;
    Ok(answer)
}
