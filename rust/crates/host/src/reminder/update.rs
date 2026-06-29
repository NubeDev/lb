//! `reminder.update` — the bounded, always-fast write that covers pause/resume (`enabled`) and
//! re-scheduling (`schedule`/`max_runs`), plus the action payload (reminders scope). Synchronous,
//! NOT a job. Any field the caller omits is left unchanged (a partial update).
//!
//! Re-scheduling recomputes `next_attempt_ts` from the NEW schedule, anchored at `now`, so a
//! reschedule moves the next fire to the next future slot under the new expression. Toggling
//! `enabled` from `false` → `true` likewise re-anchors the next fire (a resumed reminder fires at
//! the next future slot, not at a stale past instant — no backfill on resume).

use lb_auth::Principal;
use lb_reminders::{is_valid, load, next_after, save, Action, Reminder, ReminderError};
use lb_store::Store;

use super::authorize::authorize_reminder;
use super::create::best_effort_check_action;

/// The patch a `reminder.update` call carries. Every field is optional — `None` ⇒ unchanged.
#[derive(Debug, Default, Clone)]
pub struct ReminderPatch {
    pub schedule: Option<String>,
    pub max_runs: Option<Option<u32>>,
    pub enabled: Option<bool>,
    pub action: Option<Action>,
}

/// Update reminder `id` in workspace `ws` as `principal`, applying `patch`. Returns the updated
/// reminder. `NotFound` if the reminder is absent (a denied caller gets `Denied`, never this).
pub async fn reminder_update(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    patch: ReminderPatch,
    now: u64,
) -> Result<Reminder, ReminderError> {
    authorize_reminder(principal, ws, "update")?;

    let mut reminder = load(store, ws, id).await?.ok_or(ReminderError::NotFound)?;

    let rescheduled = patch.schedule.is_some();
    let resumed = matches!(patch.enabled, Some(true));

    if let Some(schedule) = patch.schedule {
        if !is_valid(&schedule) {
            return Err(ReminderError::BadCron(schedule));
        }
        reminder.schedule = schedule;
    }
    if let Some(max_runs) = patch.max_runs {
        if let Some(n) = max_runs {
            if n == 0 {
                return Err(ReminderError::BadInput("max_runs must be ≥ 1".into()));
            }
        }
        reminder.max_runs = max_runs;
    }
    if let Some(enabled) = patch.enabled {
        reminder.enabled = enabled;
    }
    if let Some(action) = patch.action {
        best_effort_check_action(&action)?;
        reminder.action = action;
    }

    // A reschedule, or a resume, re-anchors the next fire to the next future slot under the
    // (possibly new) schedule. A pause leaves next_attempt_ts alone (the reminder is skipped while
    // disabled; resume re-anchors). A pure action/max_runs edit does not move the next fire.
    if rescheduled || resumed {
        reminder.next_attempt_ts = next_after(&reminder.schedule, now)?;
        // Resuming/rescheduling a `Done` reminder re-activates it (the user asked it to run again).
        if resumed || rescheduled {
            reminder.status = lb_reminders::ReminderStatus::Active;
        }
    }

    reminder.ts = now;
    save(store, ws, &reminder).await?;
    Ok(reminder)
}
