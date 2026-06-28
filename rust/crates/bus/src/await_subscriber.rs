//! A real subscription-readiness barrier (motion, §3.3): wait until a publisher's key
//! expression has at least one *matching subscriber reachable on the mesh* before publishing.
//!
//! Why this exists: Zenoh pub/sub is fire-and-forget, and a subscription declared on one peer
//! propagates to other peers **asynchronously**. So a publisher that `put`s the instant after a
//! peer calls `subscribe` can send before that peer's interest has reached it — the message is
//! dropped on the floor (no buffering for a not-yet-known subscriber). That is the
//! subscription-vs-publish race behind the flaky offline-sync replay
//! (debugging/host-tools/offline-sync-replay-races-subscription.md): `replay_history` published
//! before the hub's `sync_channel` subscription was live, so the hub applied 0 of 3 items.
//!
//! The honest fix is a barrier on the **publisher** side, because the publisher is the only one
//! who can observe (via Zenoh's `matching_status`) whether a matching subscriber is actually
//! reachable — a subscriber cannot know that its interest has propagated to every peer. We poll
//! `Publisher::matching_status()` until it reports a match, or a deadline elapses. This is a
//! poll-until-reachable loop (nothing mocked, no blind sleep): it returns the instant a real
//! subscriber is visible. The deadline only guards a genuinely-broken/never-subscribed path.

use std::time::{Duration, Instant};

use crate::key::ws_key;
use crate::peer::{Bus, BusError};

/// How long to wait for a matching subscriber before giving up. Generous: with a deterministic
/// link the match appears in well under a second, so the headroom is free (the loop returns as
/// soon as a subscriber is visible) and only bites a truly-broken path under heavy load.
const READY_DEADLINE: Duration = Duration::from_secs(5);

/// Poll interval between `matching_status` checks while waiting for a subscriber to appear.
const POLL_EVERY: Duration = Duration::from_millis(20);

/// Wait until `(ws, rel)` has at least one matching subscriber reachable on the mesh, then
/// return `Ok(true)`. Returns `Ok(false)` if the deadline elapses with no subscriber (the caller
/// can still publish — it just won't be observed by anyone yet). Errors only on a Zenoh fault.
///
/// This declares a short-lived publisher purely to observe matching status; it is undeclared on
/// drop and does not itself publish. Callers that go on to `publish` pay one extra declare, which
/// is cheap relative to the correctness it buys on the replay/sync path.
pub async fn await_subscriber(bus: &Bus, ws: &str, rel: &str) -> Result<bool, BusError> {
    let key = ws_key(ws, rel);
    let publisher = bus
        .session()
        .declare_publisher(&key)
        .await
        .map_err(|e| BusError::Session(e.to_string()))?;

    let deadline = Instant::now() + READY_DEADLINE;
    loop {
        let matching = publisher
            .matching_status()
            .await
            .map_err(|e| BusError::Session(e.to_string()))?
            .matching();
        if matching {
            return Ok(true);
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
        tokio::time::sleep(POLL_EVERY).await;
    }
}
