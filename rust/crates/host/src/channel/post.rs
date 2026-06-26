//! Post a message to a channel — the write verb of the messaging slice.
//!
//! The flow is the state-vs-motion split made concrete (§3.3):
//!   1. authorize (`bus:chan/{cid}:pub`, workspace-first) — capability-first, before anything;
//!   2. persist the normalized item to the store via the inbox (STATE — survives a restart);
//!   3. publish the same item onto the bus (MOTION — subscribers see it appear in real time).
//!
//! Persist-before-publish on purpose: the durable record is the source of truth, the bus push
//! is the live echo. A subscriber that missed the push recovers the message from `history`;
//! the inverse (publish first, persist later) could echo a message that never durably landed.

use lb_auth::Principal;
use lb_bus::{publish, Bus};
use lb_caps::Action;
use lb_inbox::{record, Item};
use lb_store::Store;

use super::authorize::authorize;
use super::error::ChannelError;
use super::key::msg_key;

/// Post `item` to channel `cid` in workspace `ws` as `principal`. The item's `channel` is set
/// to `cid` (the caller need not repeat it). Returns once persisted *and* published.
pub async fn post(
    store: &Store,
    bus: &Bus,
    principal: &Principal,
    ws: &str,
    cid: &str,
    mut item: Item,
) -> Result<Item, ChannelError> {
    authorize(principal, ws, cid, Action::Pub)?;
    item.channel = cid.to_string();

    // STATE: durable first.
    record(store, ws, &item).await?;

    // MOTION: live echo. Serialized item JSON is the payload; subscribers deserialize it.
    let payload = serde_json::to_vec(&item)
        .map_err(|e| ChannelError::Store(lb_store::StoreError::Decode(e.to_string())))?;
    publish(bus, ws, &msg_key(cid, &item.id), &payload).await?;

    Ok(item)
}
