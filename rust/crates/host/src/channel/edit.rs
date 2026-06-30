//! Edit the body of one of your own channel messages — the edit verb of the messaging slice.
//!
//! Two gates (capability-first, §3.5), in order:
//!   1. authorize (`bus:chan/{cid}:pub`, workspace-first) — editing is a write on the channel;
//!   2. author ownership — the STORED item's `author` MUST equal the caller's `principal.sub()`.
//!
//! The ownership check reads the durable record, not the request, so a forged `author` in a
//! payload cannot grant edit of another member's message. A non-author caller collapses to
//! [`ChannelError::Denied`] (opaque — indistinguishable from a missing capability); only a
//! legitimate owner hitting a genuinely absent id gets [`ChannelError::NotFound`].
//!
//! Persist-before-publish, like `post`: overwrite the stored `Item` (same `(channel, id)`, new
//! `body`, caller-supplied `ts`), then re-publish the updated item on `chan/{cid}/msg/{id}`. The
//! live merge is an id upsert, so every viewer's row updates in place — no new event type needed.

use lb_auth::Principal;
use lb_bus::publish;
use lb_caps::Action;
use lb_inbox::{get, record, Item};

use super::authorize::authorize;
use super::error::ChannelError;
use super::key::msg_key;
use crate::boot::Node;

/// Edit message `id` in channel `cid` (workspace `ws`) as `principal`: set its body to `body`
/// and its ordering timestamp to `ts`. Only the message's author may edit it. Returns the stored
/// item (channel filled in). `NotFound` if `principal` owns an id that is not present;
/// `Denied` if the caller lacks the `pub` grant or is not the author.
pub async fn edit(
    node: &Node,
    principal: &Principal,
    ws: &str,
    cid: &str,
    id: &str,
    body: &str,
    ts: u64,
) -> Result<Item, ChannelError> {
    authorize(principal, ws, cid, Action::Pub)?;

    // Load the durable record BEFORE any mutation: the ownership gate runs against the stored
    // author, never the request. A miss by the legitimate owner is NotFound; a miss by anyone we
    // could not prove ownership for never reaches here (the capability gate already refused a
    // cross-workspace caller above, and a same-workspace non-owner is caught just below).
    let stored = get(&node.store, ws, cid, id)
        .await?
        .ok_or(ChannelError::NotFound)?;
    if stored.author != principal.sub() {
        return Err(ChannelError::Denied);
    }

    let updated = Item {
        id: stored.id,
        channel: cid.to_string(),
        author: stored.author,
        body: body.to_string(),
        ts,
    };

    // STATE: durable first.
    record(&node.store, ws, &updated).await?;

    // MOTION: live echo. Republishing the same id updates every viewer's row in place (the live
    // merge is an id-keyed upsert), so an edit needs no new event type.
    let payload = serde_json::to_vec(&updated)
        .map_err(|e| ChannelError::Store(lb_store::StoreError::Decode(e.to_string())))?;
    publish(&node.bus, ws, &msg_key(cid, &updated.id), &payload).await?;

    Ok(updated)
}
