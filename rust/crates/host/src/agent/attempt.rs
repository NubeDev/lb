//! Attempt one model turn through the slice-D fault lanes (agent-loop-hardening): a **transient**
//! fault (network, timeout, 429/5xx) is retried mechanically, bounded, honoring `Retry-After` —
//! *below* step accounting, so the loop sees one turn however many attempts it took (same turn
//! number, same idempotency key; the gateway never caches a fault, so a retry really re-calls the
//! provider). An **overflow** or **fatal** fault is returned to the loop, which compacts (slice A)
//! or ends the run honestly ([`fail_run`]).

use lb_jobs::{complete, JobStatus};
use lb_run_events::{RunEvent, RunOutcome};

use super::error::AgentError;
use super::model_access::{AllowedTool, CallOutcome, ModelAccess, Turn, TurnError};
use crate::boot::Node;
use crate::run_events::publish_run_event;

/// How many times one turn is attempted before a persistent transient fault turns fatal (the first
/// call + bounded retries). Deliberately small — a stuck provider should surface, not spin.
pub(super) const MAX_TURN_ATTEMPTS: u32 = 3;

/// The ceiling on how long a `Retry-After` is honored. A provider asking for more than this is
/// treated as "come back later" — the run fails honestly rather than parking a live loop.
const RETRY_AFTER_CAP_SECS: u64 = 30;

/// One model turn with the transient-retry lane applied. Overflow/fatal faults pass through to the
/// caller (the loop) — overflow recovery needs the transcript, which lives there (slice A).
pub(super) async fn attempt_turn<M: ModelAccess>(
    model: &M,
    ws: &str,
    messages: &[(String, String)],
    tools: &[AllowedTool],
    prior: &[CallOutcome],
    idempotency_key: &str,
) -> Result<Turn, TurnError> {
    let mut attempt: u32 = 0;
    loop {
        match model.turn(ws, messages, tools, prior, idempotency_key).await {
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
            Err(other) => return Err(other),
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
