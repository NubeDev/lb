//! The inbox **resolution facet** — the approve/reject/defer outcome of an inbox item (the
//! vision §5 "inbox item resolution" finding, coding-workflow scope).
//!
//! An approval is not a core primitive: it is an [`Item`](crate::Item) tagged `needs:approval` plus
//! a [`Resolution`] record — a small sibling addressed by the same item id. So "a human signs off"
//! is expressible generically (any item can carry a resolution), without a bespoke approval table.
//! State, like the item itself: it lives in the store behind the workspace wall (§7). `ts` is a
//! caller-injected logical timestamp (testing §3 — no wall-clock).
//!
//! The resolution is what the workflow's job-start gate reads: it starts the coding job only when an
//! item's resolution is `Approved` (coding-workflow scope). Raw verbs — `caps::check` is the host's
//! job (the workflow service is the chokepoint).

use serde::{Deserialize, Serialize};

use lb_store::{read, write, Store, StoreError};

/// The table resolutions live in (one per workspace namespace), keyed by the resolved item's id.
pub const RESOLUTION_TABLE: &str = "resolution";

/// A reviewer's decision on an inbox item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Decision {
    Approved,
    Rejected,
    Deferred,
}

/// The resolution of an inbox item: who decided what, and when. Stable on `item_id` — re-resolving
/// upserts the same row (the last decision wins; deliberate, so a deferred item can later approve).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolution {
    /// The id of the inbox item this resolves (the same id the `needs:approval` item carries).
    pub item_id: String,
    pub decision: Decision,
    /// The deciding actor (`user:…`) — recorded for audit, set by the host from the principal.
    pub actor: String,
    /// Caller-injected logical timestamp (no wall-clock — testing §3).
    pub ts: u64,
}

impl Resolution {
    pub fn new(
        item_id: impl Into<String>,
        decision: Decision,
        actor: impl Into<String>,
        ts: u64,
    ) -> Self {
        Self {
            item_id: item_id.into(),
            decision,
            actor: actor.into(),
            ts,
        }
    }
}

/// Record `resolution` for its item in workspace `ws`. Idempotent on `item_id` (last decision wins).
pub async fn resolve(store: &Store, ws: &str, resolution: &Resolution) -> Result<(), StoreError> {
    let value = serde_json::to_value(resolution).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, RESOLUTION_TABLE, &resolution.item_id, &value).await
}

/// Read the resolution of item `item_id` in workspace `ws`. `None` if unresolved (or in another
/// workspace — the namespace is the hard wall, §7).
pub async fn resolution(
    store: &Store,
    ws: &str,
    item_id: &str,
) -> Result<Option<Resolution>, StoreError> {
    match read(store, ws, RESOLUTION_TABLE, item_id).await? {
        Some(value) => Ok(Some(
            serde_json::from_value(value).map_err(|e| StoreError::Decode(e.to_string()))?,
        )),
        None => Ok(None),
    }
}
