//! `reminder.delete` — tombstone-upsert (reminders scope, §6.8 idempotent). A re-delete is a
//! no-op, and a delete of an absent reminder is a no-op (not an error) — the idempotency the sync
//! path relies on. Gated by `mcp:reminder.delete:call`.

use lb_auth::Principal;
use lb_reminders::{load, save, ReminderError};
use lb_store::Store;

use super::authorize::authorize_reminder;

/// Soft-delete reminder `id` in workspace `ws` as `principal`, at logical time `now`. Idempotent:
/// an absent or already-tombstoned reminder is a no-op. The row is kept (audit/history); it never
/// fires or lists again.
pub async fn reminder_delete(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    now: u64,
) -> Result<(), ReminderError> {
    authorize_reminder(principal, ws, "delete")?;

    let Some(mut reminder) = load(store, ws, id).await? else {
        return Ok(()); // absent — idempotent no-op
    };
    if reminder.deleted {
        return Ok(()); // already tombstoned — idempotent no-op
    }
    reminder.deleted = true;
    reminder.enabled = false;
    reminder.ts = now;
    save(store, ws, &reminder).await?;
    Ok(())
}
