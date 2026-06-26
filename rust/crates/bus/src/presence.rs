//! Presence via Zenoh liveliness tokens (README §6.2 presence).
//!
//! Presence is *motion-derived state we don't persist*: a member declares a liveliness token
//! under `ws/{id}/presence/{member}`; while the token is held the member is present, and when
//! the peer drops (cleanly or by crash) Zenoh retracts it automatically. That auto-retract is
//! why presence rides liveliness, not a stored "online" flag that would go stale on a crash.
//!
//! The token key is workspace-scoped exactly like pub/sub, so a member in workspace B can
//! neither declare nor observe presence in workspace A (README §7).

use zenoh::handlers::FifoChannelHandler;
use zenoh::liveliness::LivelinessToken;
use zenoh::pubsub::Subscriber;
use zenoh::sample::{Sample, SampleKind};

use crate::key::ws_key;
use crate::peer::{Bus, BusError};

/// A held presence token. While alive the member is present; drop it (or crash) and Zenoh
/// retracts it, so observers see the member leave without any explicit "offline" message.
pub struct Presence {
    _token: LivelinessToken,
}

/// Declare presence for `member` in workspace `ws` (key `presence/{member}`).
pub async fn declare_presence(bus: &Bus, ws: &str, member: &str) -> Result<Presence, BusError> {
    let key = ws_key(ws, &format!("presence/{member}"));
    let token = bus
        .session()
        .liveliness()
        .declare_token(&key)
        .await
        .map_err(|e| BusError::Session(e.to_string()))?;
    Ok(Presence { _token: token })
}

/// A live watch over presence in a workspace. Yields a `(member, present)` event each time a
/// member appears or disappears.
pub struct PresenceWatch {
    inner: Subscriber<FifoChannelHandler<Sample>>,
    ws: String,
}

impl PresenceWatch {
    /// Await the next presence change: `(member, true)` joined, `(member, false)` left.
    pub async fn recv(&self) -> Option<(String, bool)> {
        let sample = self.inner.recv_async().await.ok()?;
        let present = matches!(sample.kind(), SampleKind::Put);
        let member = self.member_of(sample.key_expr().as_str())?;
        Some((member, present))
    }

    /// Extract the `{member}` tail from a `ws/{id}/presence/{member}` key.
    fn member_of(&self, key: &str) -> Option<String> {
        let prefix = ws_key(&self.ws, "presence/");
        key.strip_prefix(&prefix).map(|m| m.to_string())
    }
}

/// Watch all presence in workspace `ws` (key expr `presence/*`). Existing tokens are
/// reported via `history(true)`, so a watcher started after a join still learns who is here.
pub async fn watch_presence(bus: &Bus, ws: &str) -> Result<PresenceWatch, BusError> {
    let key = ws_key(ws, "presence/*");
    let inner = bus
        .session()
        .liveliness()
        .declare_subscriber(&key)
        .history(true)
        .await
        .map_err(|e| BusError::Session(e.to_string()))?;
    Ok(PresenceWatch {
        inner,
        ws: ws.to_string(),
    })
}
