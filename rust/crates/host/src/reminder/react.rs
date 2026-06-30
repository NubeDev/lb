//! `react_to_reminders` — the **reminder reactor**: a durable scan that fires every `enabled`
//! reminder that is `due` (`next_attempt_ts ≤ now`), enqueuing ONE `lb-jobs` job per firing (so
//! the work is durable/resumable/retried for free), dispatching the action, then advancing the
//! reminder (reminders scope). Same altitude/cadence as the shipped `react_to_approvals` /
//! `relay_outbox` reactors — a stateless function over a durable set, holding no state.
//!
//! Idempotency — **one scheduled instant → one job → one effect.** The reactor derives a
//! deterministic job id from `(reminder_id, scheduled_ts)` ([`fire_job_id`]) and SKIPS an instant
//! whose job already exists (`lb_jobs::load`). So a re-scan before `next_attempt_ts` advances
//! fires nothing twice — the dedup is the existence check, exactly like the approval reactor's
//! `job:{approval_id}`.
//!
//! Missed-firing policy — **fire-once-then-skip-to-next-future-slot** (no backfill storm). After an
//! outage a recurring reminder may have skipped several slots; the reactor fires ONCE for the due
//! instant and advances `next_attempt_ts` to the next slot strictly after `now` (not after the
//! missed instant), so "every minute" can't enqueue a thousand jobs after a long outage.
//!
//! Authorization — the firing runs under the reminder's STORED principal (caps re-resolved at fire
//! time). A denied firing (the action's grant was revoked) is LOGGED and the reminder is LEFT
//! SCHEDULED (not advanced) — the scope's "stays scheduled / dead-letters per the job's retry
//! policy". The job for that instant already exists, so a re-scan does not re-fire it (no
//! privilege escalation, no double-fire). The workspace wall holds at the scan: `due` selects the
//! namespace, so a ws-B reactor never sees/fires/advances a ws-A reminder.

use std::sync::Arc;

use lb_jobs::{create, load, Job};
use lb_reminders::{due, next_after, save, Reminder, ReminderError, ReminderStatus};
use serde_json::json;

use super::fire::{fire_job_id, fire_reminder, FIRE_KIND};
use crate::boot::Node;

/// The outcome of one reactor pass: how many firings fired (and advanced), how many were skipped as
/// already-fired (the idempotent no-op), and how many were denied at the action's own gate.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ReactorPass {
    pub fired: usize,
    pub skipped: usize,
    pub denied: usize,
}

/// Run one reactor pass over workspace `ws` at logical time `now`: for every `enabled` reminder
/// that is `due` (`next_attempt_ts ≤ now`), enqueue one `kind="reminder-fire"` job (idempotent on
/// the deterministic per-firing id), dispatch the action under the stored principal, then advance
/// the reminder. Returns the pass tally.
pub async fn react_to_reminders(
    node: &Arc<Node>,
    ws: &str,
    now: u64,
) -> Result<ReactorPass, ReminderError> {
    let mut pass = ReactorPass::default();
    for reminder in due(&node.store, ws, now).await? {
        // `next_attempt_ts` IS the scheduled instant being fired (it was due, so it is ≤ now).
        let scheduled_ts = reminder.next_attempt_ts;
        let job_id = fire_job_id(&reminder.id, scheduled_ts);

        // Idempotency: a job already exists for this (reminder, instant) → no-op (no double-fire).
        if load(&node.store, ws, &job_id).await?.is_some() {
            pass.skipped += 1;
            continue;
        }

        // Record the durable firing job BEFORE dispatching, so a crash mid-fire leaves an
        // idempotent marker (a re-scan finds the job and skips). Payload names the reminder + the
        // scheduled instant for audit/replay.
        let payload =
            json!({ "reminder_id": reminder.id, "scheduled_ts": scheduled_ts }).to_string();
        create(&node.store, ws, &Job::new(&job_id, FIRE_KIND, payload, now)).await?;

        // Dispatch the action under the stored principal (re-checked). A deny is logged and the
        // reminder is LEFT scheduled (not advanced) — the job already exists, so a re-scan will not
        // re-fire this instant.
        match fire_reminder(node, ws, &reminder, scheduled_ts, now).await {
            Ok(()) => {
                advance(&node.store, ws, reminder, now).await?;
                pass.fired += 1;
            }
            Err(ReminderError::Denied) => {
                pass.denied += 1;
            }
            Err(e) => return Err(e),
        }
    }
    Ok(pass)
}

/// Advance `reminder` after a successful firing: bump `runs`, and either mark `Done` (a one-shot,
/// or `runs` reached `max_runs`) or recompute the next fire to the slot strictly after `now`
/// (fire-once-then-skip — no backfill). Persists the updated record.
async fn advance(
    store: &lb_store::Store,
    ws: &str,
    mut reminder: Reminder,
    now: u64,
) -> Result<(), ReminderError> {
    reminder.runs = reminder.runs.saturating_add(1);
    let exhausted = reminder
        .max_runs
        .map(|max| reminder.runs >= max)
        .unwrap_or(false);
    if exhausted {
        reminder.status = ReminderStatus::Done;
        reminder.enabled = false;
    } else {
        // Fire-once-then-skip: the next fire is the next future slot after NOW (not after the
        // missed instant), so a long outage yields one fire + one advance, never a backfill storm.
        reminder.next_attempt_ts = next_after(&reminder.schedule, now)?;
    }
    reminder.ts = now;
    save(store, ws, &reminder).await?;
    Ok(())
}
