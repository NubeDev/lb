//! The normalized inbox item — one shape every source (a chat message, a job result, a
//! system notice) collapses into (README §6.10, inbox-outbox scope).
//!
//! An item is *state*: it lives in the store, addressed by `(channel, id)` within a
//! workspace. The bus moves a copy as motion; the store keeps this as the durable record
//! (§3.3). Keeping one normalized shape is what lets a single channel view, a single unread
//! count, and a single triage flow work across every kind of source.

use serde::{Deserialize, Serialize};

/// A normalized inbox item. `id` is caller-supplied and stable (so a re-delivery is
/// idempotent — the same id upserts the same row, never a duplicate). `ts` is a caller-
/// injected logical timestamp (testing §3 determinism: no wall-clock inside the crate).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    /// Stable item id, unique within `(ws, channel)`. Re-delivering the same id is idempotent.
    pub id: String,
    /// The channel this item belongs to (a bus subject tail / a logical inbox bucket).
    pub channel: String,
    /// The normalized author/source identity (`user:…`, `key:…`, `ext:…`).
    pub author: String,
    /// The item's textual body. Richer payloads ride in `meta`.
    pub body: String,
    /// A logical, caller-supplied ordering timestamp (monotone per channel). Not wall-clock.
    pub ts: u64,
}

impl Item {
    /// Build an item. Kept explicit (no `Default`) so every field is a deliberate choice at
    /// the call site — an item with an empty author or channel is almost always a bug.
    pub fn new(
        id: impl Into<String>,
        channel: impl Into<String>,
        author: impl Into<String>,
        body: impl Into<String>,
        ts: u64,
    ) -> Self {
        Self {
            id: id.into(),
            channel: channel.into(),
            author: author.into(),
            body: body.into(),
            ts,
        }
    }
}
