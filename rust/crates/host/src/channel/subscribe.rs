//! Subscribe to a channel's live messages — the motion read verb (§3.3).
//!
//! Authorization (`bus:chan/{cid}:sub`, workspace-first) runs before any bus interest is
//! declared, so a denied or cross-workspace caller never even expresses interest in the keys.
//! The returned [`ChannelSub`] yields each posted [`Item`] as it arrives; the durable history
//! is `history`'s job — together they give "see the backlog, then watch it grow".

use lb_auth::Principal;
use lb_bus::{subscribe, Bus, Subscription};
use lb_caps::Action;
use lb_inbox::Item;

use super::authorize::authorize;
use super::error::ChannelError;
use super::key::sub_key;

/// A live channel subscription. Wraps the bus subscription and deserializes each payload back
/// into the normalized [`Item`] the channel speaks.
pub struct ChannelSub {
    inner: Subscription,
}

impl ChannelSub {
    /// Await the next posted item. `None` once the subscription closes. A payload that fails
    /// to deserialize is skipped (a malformed message never stalls the stream).
    pub async fn recv(&self) -> Option<Item> {
        loop {
            let bytes = self.inner.recv().await?;
            match serde_json::from_slice::<Item>(&bytes) {
                Ok(item) => return Some(item),
                Err(_) => continue,
            }
        }
    }
}

/// Subscribe to live messages on channel `cid` in workspace `ws` as `principal`.
pub async fn subscribe_channel(
    bus: &Bus,
    principal: &Principal,
    ws: &str,
    cid: &str,
) -> Result<ChannelSub, ChannelError> {
    authorize(principal, ws, cid, Action::Sub)?;
    let inner = subscribe(bus, ws, &sub_key(cid)).await?;
    Ok(ChannelSub { inner })
}
