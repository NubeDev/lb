//! The live [`RunEvent`] subscription wrapper (agent-run scope Part 3). Wraps the raw
//! `lb_bus::Subscription` so a caller receives decoded [`RunEvent`]s, not raw bytes — the SSE route
//! and the ACP encoder both consume this. Mirrors `BusSub`/`SeriesSub`, but typed to the run-event
//! vocabulary (Part 1), so the encoders never re-parse JSON by hand.

use lb_bus::Subscription;
use lb_run_events::RunEvent;

/// A live subscription to one run's event subject. `recv` yields the next [`RunEvent`] (decoded from
/// the published JSON); `None` once the subscription closes. A payload that fails to decode is
/// skipped (forward-compatible: a newer publisher variant an older watcher can't parse is dropped,
/// not fatal) — the watcher can always re-read the durable transcript snapshot.
pub struct RunEventSub {
    inner: Subscription,
}

impl RunEventSub {
    pub(crate) fn new(inner: Subscription) -> Self {
        Self { inner }
    }

    /// Await the next decoded run event. Skips an undecodable payload and waits for the next; `None`
    /// once the subscription closes.
    pub async fn recv(&self) -> Option<RunEvent> {
        loop {
            let bytes = self.inner.recv().await?;
            match serde_json::from_slice::<RunEvent>(&bytes) {
                Ok(event) => return Some(event),
                Err(_) => continue,
            }
        }
    }
}
