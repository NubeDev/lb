//! Record (persist) a normalized item into the durable inbox.
//!
//! The inbox is *state*, so this writes through `lb_store` into the workspace's namespace —
//! the same hard wall as every other record (README §7). The item is stored at
//! `inbox:<channel>__<id>`; the `id` is stable, so re-recording the same item upserts the
//! same row (idempotent delivery, inbox-outbox scope). Authorization is the caller's job —
//! this is the raw verb, run *after* `caps::check` (the host channel service does that).

use lb_store::{write, Store, StoreError};

use crate::item::Item;

/// The store table all inbox items live in. One table per workspace namespace; the channel
/// is a `data` field (so a channel view is a `list` by field, not a separate table).
pub const TABLE: &str = "inbox";

/// The stable record id for an item: `<channel>__<id>`. Keeping the channel in the key keeps
/// per-channel ids independent (two channels may reuse an id without colliding).
pub fn record_id(channel: &str, id: &str) -> String {
    format!("{channel}__{id}")
}

/// Persist `item` into workspace `ws`'s inbox. Idempotent on `(channel, id)`.
pub async fn record(store: &Store, ws: &str, item: &Item) -> Result<(), StoreError> {
    let value = serde_json::to_value(item).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(
        store,
        ws,
        TABLE,
        &record_id(&item.channel, &item.id),
        &value,
    )
    .await
}
