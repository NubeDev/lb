//! Persistence for `NotifyState` rows — the ladder state store seam (insight-notify-scope.md).
//!
//! One row per `(ws, sub_id, dedup_key)`; the record id derives from the pair so the raise path
//! and the digest reactor read/write the same row. The pure `ladder_step` is the only *transition*
//! author (crate `ladder`); this file is just the load/store around it. Kept separate from the
//! reactor (FILE-LAYOUT §3 — one responsibility per file).

use lb_store::{read, write, Store, StoreError};

use crate::notify_state::{NotifyState, TABLE};
use crate::table_scan::scan_all;

/// The record id for a `(sub_id, dedup_key)` ladder state row. A `\u{1}` separator keeps the two
/// halves unambiguous (neither a sub ULID nor a dedup key contains it).
pub fn notify_id(sub_id: &str, dedup_key: &str) -> String {
    format!("{sub_id}\u{1}{dedup_key}")
}

/// Read the ladder state for `(sub_id, dedup_key)`, or `None` if no row yet (the first-key case).
pub async fn read_notify(
    store: &Store,
    ws: &str,
    sub_id: &str,
    dedup_key: &str,
) -> Result<Option<NotifyState>, StoreError> {
    let Some(value) = read(store, ws, TABLE, &notify_id(sub_id, dedup_key)).await? else {
        return Ok(None);
    };
    Ok(serde_json::from_value(value).ok())
}

/// Upsert one ladder state row.
pub async fn write_notify(store: &Store, ws: &str, state: &NotifyState) -> Result<(), StoreError> {
    let id = notify_id(&state.sub_id, &state.dedup_key);
    let value = serde_json::to_value(state).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, &id, &value).await
}

/// Every ladder state row in the workspace — the digest reactor's scan surface.
pub async fn all_notify(store: &Store, ws: &str) -> Result<Vec<NotifyState>, StoreError> {
    let rows = scan_all(store, ws, TABLE).await?;
    Ok(rows
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect())
}
