//! Delete one of your own channel messages — the delete verb of the messaging slice — and the
//! live feed of deletions other viewers reconcile against.
//!
//! Two gates (capability-first, §3.5), in order — identical to `edit`:
//!   1. authorize (`bus:chan/{cid}:pub`, workspace-first) — deleting is a write on the channel;
//!   2. author ownership — the STORED item's `author` MUST equal the caller's `principal.sub()`.
//!
//! Persist-before-publish, like `post`: erase the stored `Item`, then publish a tombstone (the
//! item id) on `chan/{cid}/del/{id}`. A delete cannot ride the `msg` key — that feed
//! deserializes to `Item` and would drop a non-item payload — so it has its own key expression,
//! its own [`DeletionFeed`], and (at the gateway) its own `event: delete` SSE frame.
//!
//! [`watch_deletions`] is the motion read verb for that feed, authorized by the same `sub` grant
//! the message stream uses: a viewer who may read a channel may also see its deletions.

use lb_auth::Principal;
use lb_bus::{publish, subscribe, Bus, Subscription};
use lb_caps::Action;
use lb_inbox::{delete as erase, get};
use serde::{Deserialize, Serialize};

use super::authorize::authorize;
use super::error::ChannelError;
use super::key::{del_key, del_sub_key};
use crate::boot::Node;

/// Delete message `id` from channel `cid` (workspace `ws`) as `principal`. Only the message's
/// author may delete it. `NotFound` if `principal` owns an id that is not present; `Denied` if the
/// caller lacks the `pub` grant or is not the author.
pub async fn delete(
    node: &Node,
    principal: &Principal,
    ws: &str,
    cid: &str,
    id: &str,
) -> Result<(), ChannelError> {
    authorize(principal, ws, cid, Action::Pub)?;

    // Ownership gate against the STORED author — same opaque-deny contract as `edit`.
    let stored = get(&node.store, ws, cid, id)
        .await?
        .ok_or(ChannelError::NotFound)?;
    if stored.author != principal.sub() {
        return Err(ChannelError::Denied);
    }

    // STATE: durable first.
    erase(&node.store, ws, cid, id).await?;

    // MOTION: publish the id as a tombstone on the delete key. The payload is the bare id (the
    // only fact a viewer needs to drop the row); the store record — the source of truth — is gone.
    let tombstone = serde_json::to_vec(&Tombstone { id: id.to_string() })
        .map_err(|e| ChannelError::Store(lb_store::StoreError::Decode(e.to_string())))?;
    publish(&node.bus, ws, &del_key(cid, id), &tombstone).await?;

    Ok(())
}

/// The delete tombstone published on `chan/{cid}/del/{id}`: just the id of the erased message.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Tombstone {
    id: String,
}

/// A live feed of message deletions in a channel. Wraps the bus subscription; the next deletion
/// yields the erased item's id. Drop to stop watching.
pub struct DeletionFeed {
    inner: Subscription,
}

impl DeletionFeed {
    /// Await the next deleted item id. `None` once the feed closes. A payload that fails to
    /// deserialize is skipped (a malformed tombstone never stalls the feed).
    pub async fn recv(&self) -> Option<String> {
        loop {
            let bytes = self.inner.recv().await?;
            match serde_json::from_slice::<Tombstone>(&bytes) {
                Ok(t) => return Some(t.id),
                Err(_) => continue,
            }
        }
    }
}

/// Watch message deletions in channel `cid` (workspace `ws`) as `principal`. Requires the same
/// `sub` grant as reading/listening: a viewer who may see a channel's messages may see its
/// deletions too.
pub async fn watch_deletions(
    bus: &Bus,
    principal: &Principal,
    ws: &str,
    cid: &str,
) -> Result<DeletionFeed, ChannelError> {
    authorize(principal, ws, cid, Action::Sub)?;
    let inner = subscribe(bus, ws, &del_sub_key(cid)).await?;
    Ok(DeletionFeed { inner })
}
