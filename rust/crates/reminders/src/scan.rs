//! Scan a workspace's reminders — the `list`/reactor reads (reminders scope).
//!
//! The generic store `list` is a pure string-equality filter (it does not order — the caller sorts).
//! Reminders carry a string `status` (`active`/`done`), so we list each non-terminal status and
//! drop tombstoned rows in Rust. The namespace is selected from `ws`, so a ws-B scan can physically
//! only return ws-B reminders (the hard wall, §7).
//!
//! Two reads on this set:
//!   - [`list`] — every non-deleted reminder (active or done), for the UI `list` verb.
//!   - [`due`] — the subset the reactor fires this pass: `active` AND `enabled` AND
//!     `next_attempt_ts <= now`. Oldest→newest by `next_attempt_ts` (deterministic — `ts` injected).

use lb_store::{list as store_list, Store, StoreError};

use crate::model::{Reminder, ReminderStatus};

/// Every non-deleted reminder in workspace `ws`, oldest→newest by `next_attempt_ts`. Never another
/// workspace's. Tombstoned rows are dropped (idempotent delete).
pub async fn list(store: &Store, ws: &str) -> Result<Vec<Reminder>, StoreError> {
    let mut out = Vec::new();
    for status in ["active", "done"] {
        for v in store_list(store, ws, super::TABLE, "status", status).await? {
            let r: Reminder =
                serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string()))?;
            if !r.deleted {
                out.push(r);
            }
        }
    }
    out.sort_by_key(|r| r.next_attempt_ts);
    Ok(out)
}

/// The reminders the reactor should fire at logical time `now`: `active` AND `enabled` AND
/// `next_attempt_ts <= now`. Oldest→newest. A `Done` or `enabled=false` reminder is owed nothing.
pub async fn due(store: &Store, ws: &str, now: u64) -> Result<Vec<Reminder>, StoreError> {
    let mut out = Vec::new();
    for v in store_list(store, ws, super::TABLE, "status", "active").await? {
        let r: Reminder =
            serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string()))?;
        if !r.deleted && r.enabled && r.status == ReminderStatus::Active && r.next_attempt_ts <= now
        {
            out.push(r);
        }
    }
    out.sort_by_key(|r| r.next_attempt_ts);
    Ok(out)
}
