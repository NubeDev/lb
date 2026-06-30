//! `reminder.create` — persist a new reminder (reminders scope). A bounded, always-fast single-
//! record write — synchronous, NOT a job (the firing is the job; the CRUD verbs never are).
//!
//! What it does, in order:
//!   1. the CRUD capability gate (`mcp:reminder.create:call`, workspace-first) — the deny path;
//!   2. a best-effort validation of the cron schedule + the action shape (UX feedback only — the
//!      authoritative check is at fire time, because tool schemas evolve between create and fire);
//!   3. compute the first `next_attempt_ts` = the next slot strictly after `now` (the injected
//!      clock), so the reminder first fires in the future;
//!   4. persist under the caller's principal (the stored principal the firing re-resolves).

use lb_auth::Principal;
use lb_reminders::{is_valid, next_after, save, Action, Reminder, ReminderError};
use lb_store::Store;

use super::authorize::authorize_reminder;

/// Create a reminder in workspace `ws` as `principal`. Returns the persisted reminder (with its
/// computed first `next_attempt_ts`). Idempotent on `id` (a re-create upserts).
#[allow(clippy::too_many_arguments)]
pub async fn reminder_create(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    schedule: &str,
    max_runs: Option<u32>,
    action: Action,
    now: u64,
) -> Result<Reminder, ReminderError> {
    authorize_reminder(principal, ws, "create")?;

    if !is_valid(schedule) {
        return Err(ReminderError::BadCron(schedule.to_string()));
    }
    if let Some(n) = max_runs {
        if n == 0 {
            return Err(ReminderError::BadInput("max_runs must be ≥ 1".into()));
        }
    }
    best_effort_check_action(&action)?;

    // First fire: the next slot strictly after `now` (never fire at create time).
    let next_attempt_ts = next_after(schedule, now)?;

    let mut reminder = Reminder::new(id, schedule, max_runs, action, principal.sub(), now);
    reminder.next_attempt_ts = next_attempt_ts;
    save(store, ws, &reminder).await?;
    Ok(reminder)
}

/// A best-effort structural check of the action payload (UX feedback at create time). This is NOT
/// authoritative — the firing re-validates under the live tool schema. Here we only reject shapes
/// that could never fire (empty channel/tool/target), so an author gets immediate feedback.
pub(super) fn best_effort_check_action(action: &Action) -> Result<(), ReminderError> {
    match action {
        Action::ChannelPost { channel, .. } if channel.trim().is_empty() => Err(
            ReminderError::BadInput("channel-post action needs a channel".into()),
        ),
        Action::McpTool { tool, .. } if tool.trim().is_empty() => Err(ReminderError::BadInput(
            "mcp-tool action needs a tool name".into(),
        )),
        Action::Outbox { target, .. } if target.trim().is_empty() => Err(ReminderError::BadInput(
            "outbox action needs a target".into(),
        )),
        _ => Ok(()),
    }
}
