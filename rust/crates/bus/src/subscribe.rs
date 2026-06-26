//! Subscribe to a workspace-scoped bus key expression (motion, §3.3).
//!
//! The caller passes a workspace-relative key expression (`chan/general/**`); `ws_key`
//! prepends `ws/{id}/`, so the declared subscription is `ws/{id}/chan/general/**`. Because
//! the prefix is the host's to set, a subscriber for workspace B *cannot* express interest in
//! workspace A's keys — the isolation guarantee on the bus (README §7). This is the structural
//! reason the mandatory cross-workspace-leak test passes.
//!
//! Returns a [`Subscription`] holding the live Zenoh subscriber; the caller awaits messages
//! on it. Dropping it undeclares the interest.

use zenoh::handlers::FifoChannelHandler;
use zenoh::pubsub::Subscriber;
use zenoh::sample::Sample;

use crate::key::ws_key;
use crate::peer::{Bus, BusError};

/// A live subscription to a workspace-scoped key expression. Holds the Zenoh subscriber; the
/// next message is awaited via [`Subscription::recv`]. Drop to stop subscribing.
pub struct Subscription {
    inner: Subscriber<FifoChannelHandler<Sample>>,
}

impl Subscription {
    /// Await the next message's payload bytes. `None` once the subscription is closed.
    pub async fn recv(&self) -> Option<Vec<u8>> {
        let sample = self.inner.recv_async().await.ok()?;
        Some(sample.payload().to_bytes().to_vec())
    }

    /// The full workspace-scoped key the next sample matched (for the caller to route on).
    pub async fn recv_keyed(&self) -> Option<(String, Vec<u8>)> {
        let sample = self.inner.recv_async().await.ok()?;
        Some((
            sample.key_expr().as_str().to_string(),
            sample.payload().to_bytes().to_vec(),
        ))
    }
}

/// Declare a subscription to `(ws, rel)` (a key expression, may contain `*`/`**`).
pub async fn subscribe(bus: &Bus, ws: &str, rel: &str) -> Result<Subscription, BusError> {
    let key = ws_key(ws, rel);
    let inner = bus
        .session()
        .declare_subscriber(&key)
        .await
        .map_err(|e| BusError::Session(e.to_string()))?;
    Ok(Subscription { inner })
}
