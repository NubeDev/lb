//! Channel presence — who is here, via Zenoh liveliness (README §6.2, bus presence).
//!
//! Presence is authorized like listening: a `bus:chan/{cid}:sub` grant lets a member both
//! *declare* their own presence and *watch* others' in that channel's workspace. The token is
//! workspace-scoped, so presence in workspace A is invisible to workspace B (§7). Declaring
//! holds a [`ChannelPresence`]; drop it (or crash) and the member auto-leaves — no stale
//! "online" state to clean up.

use lb_auth::Principal;
use lb_bus::{declare_presence, watch_presence, Bus, Presence, PresenceWatch};
use lb_caps::Action;

use super::authorize::authorize;
use super::error::ChannelError;

/// A held presence registration for a member in a channel's workspace.
pub struct ChannelPresence {
    _inner: Presence,
}

/// A live feed of `(member, present)` changes in a channel's workspace.
pub struct PresenceFeed {
    inner: PresenceWatch,
}

impl PresenceFeed {
    /// Await the next presence change: `(member, true)` joined, `(member, false)` left.
    pub async fn recv(&self) -> Option<(String, bool)> {
        self.inner.recv().await
    }
}

/// Declare `member` present in channel `cid`'s workspace `ws`, as `principal`.
pub async fn join(
    bus: &Bus,
    principal: &Principal,
    ws: &str,
    cid: &str,
    member: &str,
) -> Result<ChannelPresence, ChannelError> {
    authorize(principal, ws, cid, Action::Sub)?;
    let inner = declare_presence(bus, ws, member).await?;
    Ok(ChannelPresence { _inner: inner })
}

/// Watch presence in channel `cid`'s workspace `ws`, as `principal`.
pub async fn watch(
    bus: &Bus,
    principal: &Principal,
    ws: &str,
    cid: &str,
) -> Result<PresenceFeed, ChannelError> {
    authorize(principal, ws, cid, Action::Sub)?;
    let inner = watch_presence(bus, ws).await?;
    Ok(PresenceFeed { inner })
}
