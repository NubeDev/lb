//! The live bus subscription wrapper (widget-config-vars scope, "Platform fix"). Wraps the raw
//! `lb_bus::Subscription` so a host caller receives the published payload bytes back as-is (the JSON the
//! publisher sent). Mirrors `SeriesSub`, but the payload is an opaque value (a generic subject, not a
//! typed series sample) — the gateway SSE route emits it verbatim.

use lb_bus::Subscription;

/// A live subscription to one workspace-walled subject. `recv` yields the next published payload (the
/// raw bytes the publisher sent); `None` once the subscription closes.
pub struct BusSub {
    inner: Subscription,
}

impl BusSub {
    pub(super) fn new(inner: Subscription) -> Self {
        Self { inner }
    }

    /// Await the next published payload (raw bytes). `None` once closed.
    pub async fn recv(&self) -> Option<Vec<u8>> {
        self.inner.recv().await
    }
}
