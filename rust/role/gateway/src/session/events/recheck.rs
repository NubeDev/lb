//! [`WatchRecheck`] — the **revoke-terminates-stream** guard for an open `bus.watch` SSE stream
//! (bus-watch-subject-scope, issue #49, Gap 2). The subscribe gate runs once, before the stream
//! opens; a `grants.revoke` after that would not close an already-open stream. This wraps the
//! stream's `recv` loop with a bounded re-check tick: on each tick it re-runs the *same*
//! subject-scoped authorize the subscribe used ([`lb_host::authorize_subject_scoped`]) against the
//! caller's LIVE grants; the first tick that denies ends the stream.
//!
//! Bounded latency (a few seconds), not instantaneous — the same freshness posture as the rest of
//! authz (revoke → deny on next check). It is node-local (no cross-node signal), so it is
//! symmetric-node-safe: the re-check reads the local store wherever the stream lives, and a synced
//! revoke closes the stream on the next tick.
//!
//! Shared by BOTH stream paths: the dedicated `GET /bus/{subject}/stream` route folds a `BusSub`
//! through [`next_authorized`](WatchRecheck::next_authorized); the mux hub's `bus:` subject wraps
//! its stream with [`guard_stream`] so a revoke closes just that one multiplexed subscription while
//! the connection lives on.

use std::pin::Pin;
use std::time::Duration;

use futures::stream::{Stream, StreamExt};
use lb_auth::Principal;
use lb_host::{BusSub, WatchMode};
use lb_store::Store;
use tokio::time::{interval, Interval};

/// How often an open stream re-checks its subject-scoped grant. A few seconds keeps revoke latency
/// bounded (the ask) without re-reading grants on a hot path per frame.
pub const RECHECK_INTERVAL: Duration = Duration::from_secs(3);

/// Re-authorizes an open bus-watch stream on a bounded tick. Holds the caller + subject the stream
/// was opened for and the store it re-reads grants from; owns its own interval so each stream ticks
/// independently.
pub struct WatchRecheck {
    store: Store,
    principal: Principal,
    ws: String,
    subject: String,
    tick: Interval,
    /// The mode this stream was opened under, captured on the first re-check. `None` until then.
    /// A `Scoped` stream must STAY scoped (revoking the matching grant — even the last one — closes
    /// it, never re-opens); an `Open` stream must not become denied (a newly-added non-matching
    /// grant tightens it). This stickiness is what prevents a last-grant revoke from silently
    /// dropping a scoped subject back into open (back-compat) mode.
    mode: Option<WatchMode>,
}

impl WatchRecheck {
    /// Build a re-check for `principal` watching `subject` in `ws`. The interval's first tick fires
    /// immediately (tokio default) — harmless: the subscribe just authorized, so the first re-check
    /// confirms it, and only a *later* revoke flips it to deny.
    pub fn new(store: Store, principal: Principal, ws: String, subject: String) -> Self {
        Self::with_interval(store, principal, ws, subject, RECHECK_INTERVAL)
    }

    /// [`new`](Self::new) with an explicit re-check interval — the seam a test uses to drive the
    /// revoke-terminates-stream path in milliseconds instead of the production seconds.
    pub fn with_interval(
        store: Store,
        principal: Principal,
        ws: String,
        subject: String,
        period: Duration,
    ) -> Self {
        Self {
            store,
            principal,
            ws,
            subject,
            tick: interval(period),
            mode: None,
        }
    }

    /// True while this stream remains authorized under the mode it was opened in. On the first call
    /// it captures the mode; thereafter it enforces stickiness:
    ///   - `Scoped` → the matching `bus:<subject>:watch` grant must STILL exist. Revoking it (even the
    ///     caller's last one) returns false → the stream closes; it can never silently re-open under
    ///     back-compat because the requirement is anchored to the grant, not to "any grant exists".
    ///   - `Open` → the subject must not have become `Denied` (a newly-added non-matching scoped grant
    ///     tightens an open stream). Staying `Open` (or gaining a matching grant → `Scoped`) is fine.
    async fn still_authorized(&mut self) -> bool {
        let now = lb_host::authorize_subject_scoped(
            &self.store,
            &self.principal,
            &self.ws,
            &self.subject,
        )
        .await;
        match self.mode {
            None => {
                // First tick: establish the mode. A subscribe that got here was authorized, but read
                // fresh — if it already denies, close.
                match now {
                    Ok(m) => {
                        self.mode = Some(m);
                        true
                    }
                    Err(_) => false,
                }
            }
            Some(WatchMode::Scoped) => {
                // Must still be scoped-authorized by a live matching grant (revoke → close).
                lb_host::still_scoped_authorized(
                    &self.store,
                    &self.principal,
                    &self.ws,
                    &self.subject,
                )
                .await
                .unwrap_or(false)
            }
            Some(WatchMode::Open) => now.is_ok(),
        }
    }

    /// Await the next authorized payload from `sub`. Returns the raw bytes, or `None` when the
    /// stream should end — either the subscription closed OR a re-check tick found the grant
    /// revoked. A payload is never emitted after the grant is gone: a tick that denies wins the
    /// select and ends the stream.
    pub async fn next_authorized(&mut self, sub: &BusSub) -> Option<Vec<u8>> {
        loop {
            tokio::select! {
                bytes = sub.recv() => return bytes,
                _ = self.tick.tick() => {
                    if !self.still_authorized().await {
                        return None; // Revoked ⇒ close the stream (Gap 2).
                    }
                    // Still authorized — loop back and keep waiting for the next payload/tick.
                }
            }
        }
    }
}

/// Wrap a `bus:` subject's frame stream so a `grants.revoke` ends it within a tick (the mux-hub
/// path). Each item of `inner` is one already-encoded frame value `T`; the wrapped stream yields the
/// same items until a re-check tick denies, then ends. Used by the hub's `open_subject` `bus:` arm.
pub fn guard_stream<T, S>(recheck: WatchRecheck, inner: S) -> Pin<Box<dyn Stream<Item = T> + Send>>
where
    T: Send + 'static,
    S: Stream<Item = T> + Send + 'static,
{
    // Box+pin the inner stream so it is `Unpin` for the unfold state (a `bus.watch` feed is itself
    // an `unfold` over `BusSub`, which is not `Unpin`).
    let inner: Pin<Box<dyn Stream<Item = T> + Send>> = Box::pin(inner);
    Box::pin(futures::stream::unfold(
        (inner, recheck),
        move |(mut inner, mut recheck)| async move {
            loop {
                tokio::select! {
                    item = inner.next() => return item.map(|i| (i, (inner, recheck))),
                    _ = recheck.tick.tick() => {
                        if !recheck.still_authorized().await {
                            return None;
                        }
                    }
                }
            }
        },
    ))
}
