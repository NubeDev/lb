//! The normalized inbox item — one shape every source (a chat message, a job result, a
//! system notice) collapses into (README §6.10, inbox-outbox scope).
//!
//! An item is *state*: it lives in the store, addressed by `(channel, id)` within a
//! workspace. The bus moves a copy as motion; the store keeps this as the durable record
//! (§3.3). Keeping one normalized shape is what lets a single channel view, a single unread
//! count, and a single triage flow work across every kind of source.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A normalized inbox item. `id` is caller-supplied and stable (so a re-delivery is
/// idempotent — the same id upserts the same row, never a duplicate). `ts` is a caller-
/// injected logical timestamp (testing §3 determinism: no wall-clock inside the crate).
// NOT `Eq`: `meta` is a `serde_json::Value`, which is only `PartialEq` (floats). Nothing compares
// items for equality outside tests, and the dedup identity is `(channel, id)` — a record key, not a
// derived trait.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// **Source-specific structured payload**, opaque to the inbox.
    ///
    /// The inbox-outbox scope left this as an open question — "a `meta: Value` field on `Item`, or a
    /// typed per-source extension record the item references? (Defer until a second source exists.)"
    /// Mail is that second source, and it settles it: a `meta` field, because the alternative
    /// (a sibling record per source kind) would make the *reader* — one inbox view rendering items
    /// from every source — join against a table it has to know the name of, which is exactly the
    /// per-source knowledge the normalized `Item` exists to abolish.
    ///
    /// The rule that keeps it from becoming a dumping ground: **nothing in the inbox ever reads
    /// inside it.** No ordering, no filtering, no gating, no dedup key. It rides through `record` and
    /// out of `list` untouched, for whichever renderer knows what the source meant. `Option` +
    /// `skip_serializing_if` so every existing item and every existing `Item::new` call site is
    /// byte-for-byte unchanged (77 of them at the time of writing).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
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
            meta: None,
        }
    }

    /// Attach a source-specific payload. Builder-style so [`Item::new`]'s signature — and its
    /// callers — stay exactly as they were.
    pub fn with_meta(mut self, meta: Value) -> Self {
        self.meta = (!meta.is_null()).then_some(meta);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_item_without_meta_serializes_exactly_as_it_always_did() {
        let item = Item::new("m1", "general", "user:test", "hi", 1);
        let json = serde_json::to_value(&item).expect("serialize");
        assert!(
            json.get("meta").is_none(),
            "an absent meta must not appear on the wire: {json}"
        );
    }

    #[test]
    fn meta_rides_through_a_round_trip_untouched() {
        let item = Item::new("m1", "mail", "mail:a@b.com", "hi", 1)
            .with_meta(json!({"subject": "NEM12", "attachments": [{"filename": "x.csv"}]}));
        let round: Item = serde_json::from_value(serde_json::to_value(&item).unwrap()).unwrap();
        assert_eq!(round, item);
        assert_eq!(round.meta.unwrap()["subject"], "NEM12");
    }

    #[test]
    fn a_null_meta_is_the_same_as_no_meta() {
        let item = Item::new("m1", "mail", "mail:a@b.com", "hi", 1).with_meta(Value::Null);
        assert_eq!(item.meta, None);
    }
}
