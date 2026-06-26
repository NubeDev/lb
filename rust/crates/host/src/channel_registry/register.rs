//! `register_on_post` — the create-on-first-post path: posting to a channel makes it exist, so it
//! must also make it *listable* (collaboration scope, slice 2 — "written on first post AND by an
//! explicit create").
//!
//! No authorization here: it runs INSIDE `channel::post`, strictly after `post`'s own `pub` gate has
//! passed, so the caller is already proven authorized to write this channel. It is a raw upsert,
//! idempotent on the channel id — the 2nd post to a channel re-writes the same row (no duplicate),
//! and it never conflicts with an explicit `channel_create` (both upsert `channel_registry:{cid}`).
//! Additive: a failure to register must NOT fail the post (the message is the source of truth) — the
//! caller treats this as best-effort.

use lb_store::{write, Store, StoreError};

use super::model::{ChannelRecord, TABLE};

/// Upsert the registry record for channel `cid` in workspace `ws`, authored by `created_by`. Called
/// from `post` after its capability gate; idempotent.
pub async fn register_on_post(
    store: &Store,
    ws: &str,
    cid: &str,
    created_by: &str,
    ts: u64,
) -> Result<(), StoreError> {
    let record = ChannelRecord::new(cid, created_by, ts);
    let value = serde_json::to_value(&record).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, cid, &value).await
}
