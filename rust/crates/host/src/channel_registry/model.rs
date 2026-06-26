//! The channel registry record — one row per `(ws, channel)` (collaboration scope, slice 2).
//!
//! Deliberately small: the channel id, who created it, and a logical `ts`. Metadata (topic,
//! description) is the obvious next field — the record exists precisely so that growth is additive
//! (the open question's lean: "a registry record, cheap, explicit, supports metadata later"). State,
//! workspace-scoped — it lives in the store behind the hard wall (§7). `ts` is caller-injected
//! (no wall-clock in the crate — testing §3).

use serde::{Deserialize, Serialize};

/// The table channels are registered in, within a workspace namespace. One owner of the name.
pub const TABLE: &str = "channel_registry";

/// The constant `kind` discriminant every channel row carries, so the generic store equality
/// filter can select "all channels" (the store `list` is an equality filter, not a table dump —
/// the same trick the workflow directory uses with its `status` field).
pub const KIND: &str = "channel";

/// A registered channel within a workspace. `id` is the channel id (e.g. `general`), stable — both
/// `channel_create` and create-on-post upsert the same row, so the two paths reconcile (the open
/// question: "reconcile create-on-post with explicit create — both upsert the registry record").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelRecord {
    /// The channel id — the same string used as the bus subject tail / `Item::channel`.
    pub id: String,
    /// The principal that first registered it (`user:…`), for audit / a future "created by" column.
    pub created_by: String,
    /// A constant discriminant (`channel`) so `channel_list` can equality-filter every row.
    pub kind: String,
    /// Caller-injected logical timestamp (no wall-clock — testing §3).
    pub ts: u64,
}

impl ChannelRecord {
    pub fn new(id: impl Into<String>, created_by: impl Into<String>, ts: u64) -> Self {
        Self {
            id: id.into(),
            created_by: created_by.into(),
            kind: KIND.to_string(),
            ts,
        }
    }
}
