//! `reminder.get` / `reminder.list` — the read verbs (reminders scope). Gated by
//! `mcp:reminder.get:call` / `mcp:reminder.list:call` (workspace-first §7). Both are workspace-
//! scoped: a ws-B caller can never read a ws-A reminder (the hard wall, at the store layer).
//!
//! `list` returns every non-deleted reminder in the workspace (the UI filters by enabled/kind/
//! channel client-side from the full set; the workspace is small and the read is cheap).

use lb_auth::Principal;
use lb_reminders::{list, load, Reminder, ReminderError};
use lb_store::Store;

use super::authorize::authorize_reminder;

/// Read reminder `id` in workspace `ws`. `None` if absent (or tombstoned — a deleted reminder is
/// gone from the read surface, kept only for audit).
pub async fn reminder_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<Option<Reminder>, ReminderError> {
    authorize_reminder(principal, ws, "get")?;
    let reminder = load(store, ws, id).await?;
    Ok(reminder.filter(|r| !r.deleted))
}

/// Every non-deleted reminder in workspace `ws`, oldest→newest by `next_attempt_ts`. Never another
/// workspace's.
pub async fn reminder_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<Reminder>, ReminderError> {
    authorize_reminder(principal, ws, "list")?;
    Ok(list(store, ws).await?)
}
