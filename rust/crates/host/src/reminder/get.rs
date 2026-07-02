//! `reminder.get` / `reminder.list` — the read verbs (reminders scope). Gated by
//! `mcp:reminder.get:call` / `mcp:reminder.list:call` (workspace-first §7). Both are workspace-
//! scoped: a ws-B caller can never read a ws-A reminder (the hard wall, at the store layer).
//!
//! `list` returns every non-deleted reminder in the workspace, oldest→newest. Per the resource-verb
//! convention (core/resource-verbs-scope D3, "every `list` takes `{status?, limit, cursor}`") it
//! accepts an optional `status` filter (`enabled`/`disabled`, over the `enabled` flag) and an optional
//! `limit` (a head truncation of the sorted set) — applied host-side over the ws-scoped read, so the
//! CLI/palette offer `--status`/`--limit` uniformly. The reactor's own due-scan is unaffected (it
//! reads `scan::due`, not this verb).

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

/// The `status` filter a `list` accepts (D3's shared minimal core). `enabled`/`disabled` select on
/// the reminder's on/off switch; `None` (no `--status`) returns both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusFilter {
    Enabled,
    Disabled,
}

impl StatusFilter {
    /// Parse the wire/CLI value. `enabled`/`disabled` only; anything else is author feedback (BadInput),
    /// not an opaque deny — the caller IS authorized, the filter value is just wrong.
    pub fn parse(s: &str) -> Result<Self, ReminderError> {
        match s.to_ascii_lowercase().as_str() {
            "enabled" => Ok(StatusFilter::Enabled),
            "disabled" => Ok(StatusFilter::Disabled),
            other => Err(ReminderError::BadInput(format!(
                "unknown status filter '{other}' (expected enabled|disabled)"
            ))),
        }
    }

    fn matches(&self, r: &Reminder) -> bool {
        match self {
            StatusFilter::Enabled => r.enabled,
            StatusFilter::Disabled => !r.enabled,
        }
    }
}

/// Every non-deleted reminder in workspace `ws`, oldest→newest by `next_attempt_ts`. Never another
/// workspace's. `status` filters on the `enabled` flag (D3); `limit` truncates the head of the sorted
/// set (a bounded page). Both are applied host-side over the full ws read — the workspace is small and
/// the read is cheap, and the isolation wall is already enforced by the ws-scoped scan underneath.
pub async fn reminder_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
    status: Option<StatusFilter>,
    limit: Option<usize>,
) -> Result<Vec<Reminder>, ReminderError> {
    authorize_reminder(principal, ws, "list")?;
    let mut reminders = list(store, ws).await?;
    if let Some(f) = status {
        reminders.retain(|r| f.matches(r));
    }
    if let Some(n) = limit {
        reminders.truncate(n);
    }
    Ok(reminders)
}
