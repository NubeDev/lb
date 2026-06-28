//! Channel sync — edge↔hub, per the README §6.8 authority partition (NOT multi-master).
//!
//! Channel items are **append-style shared data**: each is addressed by a stable
//! `(channel, id)`, and the inbox `record` upserts on that key. That is the whole reason sync
//! here is tractable without conflict resolution: applying the same item twice is a no-op, and
//! two edges' items never collide (distinct ids). So sync is just *idempotent apply* of items
//! that arrive over the bus, into the local store — exactly §6.8's "Zenoh → idempotent apply".
//!
//! Two pieces, both config-driven (no role branch in this code — a node runs a sync if its
//! wiring layer starts one; the hub typically does):
//!   - [`ChannelSync`] — subscribe to a channel's bus messages and `record` each into the local
//!     store. A live post on a peer lands in this node's durable history.
//!   - [`replay_history`] — re-publish this node's durable items for a channel onto the bus, so
//!     a node that was OFFLINE during the original posts catches up on reconnect. Because apply
//!     is idempotent, replaying items the hub already has changes nothing (last-writer-wins on
//!     the rare contested record, §6.8 — here items are immutable so it never contends).
//!
//! This is deliberately the *minimal* reusable sync: the durable outbox with a delivery cursor
//! (§6.10) is the next step (still open in the inbox-outbox scope). For append-style channel
//! items, persist-before-publish + idempotent apply + replay already gives at-least-once.

use lb_bus::{await_subscriber, subscribe, Bus, Subscription};
use lb_inbox::{list, record, Item};
use lb_store::{Store, StoreError};

use crate::channel::sub_key_for;

/// A running channel sync: it consumes items off the bus and applies them to the local store
/// until dropped. The hub holds one per channel it mirrors; an edge can hold one too (sync is
/// symmetric — direction is which node runs it, config, not code).
pub struct ChannelSync {
    sub: Subscription,
    store: Store,
    ws: String,
}

impl ChannelSync {
    /// Apply the next item that arrives on the bus to the local store. Returns the applied
    /// item, or `None` when the subscription closes. Idempotent: re-applying a known
    /// `(channel, id)` upserts the same row (§6.8 idempotent apply).
    pub async fn apply_next(&self) -> Option<Item> {
        loop {
            let bytes = self.sub.recv().await?;
            let item: Item = match serde_json::from_slice(&bytes) {
                Ok(i) => i,
                Err(_) => continue, // a malformed payload never stalls the sync
            };
            // Persist into THIS node's namespace for `self.ws`. The item carries no workspace —
            // the bus key it arrived on was workspace-scoped, and we record under that same ws.
            if record(&self.store, &self.ws, &item).await.is_ok() {
                return Some(item);
            }
        }
    }
}

/// Start syncing channel `cid` (workspace `ws`) from the bus into `store`. Subscribes to the
/// channel's message key expression — the same keys `post` publishes on, so every peer's post
/// is mirrored here. No capability check: sync is an internal node↔node mechanism wired by the
/// role layer, not a principal-facing surface (the principal-facing reads are `history`/
/// `subscribe_channel`, which DO check caps).
pub async fn sync_channel(
    bus: &Bus,
    store: &Store,
    ws: &str,
    cid: &str,
) -> Result<ChannelSync, StoreError> {
    let sub = subscribe(bus, ws, &sub_key_for(cid))
        .await
        .map_err(|e| StoreError::Backend(format!("sync subscribe: {e}")))?;
    Ok(ChannelSync {
        sub,
        store: store.clone(),
        ws: ws.to_string(),
    })
}

/// Re-publish every durable item this node holds for `(ws, cid)` back onto the bus, so a node
/// that was offline during the original posts applies them on reconnect (§6.8 catch-up). The
/// receiver's apply is idempotent, so replay is always safe — including replaying to a node
/// that already has them. This is the edge's "flush my offline writes to the hub" verb.
pub async fn replay_history(
    bus: &Bus,
    store: &Store,
    ws: &str,
    cid: &str,
) -> Result<usize, StoreError> {
    let items = list(store, ws, cid).await?;
    let count = items.len();

    // Subscription-readiness barrier (the fix for the flaky offline-sync replay race,
    // debugging/host-tools/offline-sync-replay-races-subscription.md). Zenoh pub/sub is
    // fire-and-forget and a peer's subscription propagates asynchronously, so publishing the
    // instant after a hub calls `sync_channel` can send before the hub's interest is live —
    // the replayed items land on the floor and the hub applies 0. Wait until a matching
    // subscriber for this channel is actually reachable before replaying. The barrier polls
    // real `matching_status` (no sleep, no mock) and returns the instant a subscriber appears;
    // if none ever subscribes it falls through after a deadline and we publish anyway (a replay
    // to nobody is a harmless no-op — apply is idempotent on reconnect).
    await_subscriber(bus, ws, &sub_key_for(cid))
        .await
        .map_err(|e| StoreError::Backend(format!("sync replay readiness: {e}")))?;

    for item in &items {
        let payload = serde_json::to_vec(item).map_err(|e| StoreError::Decode(e.to_string()))?;
        lb_bus::publish(
            bus,
            ws,
            &crate::channel::msg_key_for(cid, &item.id),
            &payload,
        )
        .await
        .map_err(|e| StoreError::Backend(format!("sync replay publish: {e}")))?;
    }
    Ok(count)
}
