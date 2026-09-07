//! A real queryable-readiness barrier — the request/reply twin of
//! [`await_subscriber`](crate::await_subscriber).
//!
//! **Why this exists.** A queryable declared on one peer propagates to other peers
//! **asynchronously**, exactly as a subscription does. So a caller that `get`s the instant after a
//! remote node calls `declare_queryable` finds no responder and gets an empty reply stream — which
//! the routing layer reports as `NodeUnreachable`, indistinguishable from a node that is genuinely
//! down.
//!
//! Every routed test worked around this the same way: issue the REAL call in a loop against a
//! wall-clock deadline (20 s) and hope convergence wins the race. That is not a test of the routing
//! behaviour, it is a test of how loaded the machine is — and on CI it lost often enough to fail
//! three different routed tests across two runs of the same commit, each time with
//! `attempt timed out (queryable not yet reachable)`.
//!
//! A deadline can only ever be tuned, never made correct. The honest fix is to stop guessing when
//! the mesh has converged and **observe** it: Zenoh's `Querier::matching_status()` reports whether a
//! Queryable matching a key expression is actually reachable, and `matching_listener()` delivers the
//! transition as an event. So a test awaits the barrier, and only then makes its ONE call — with the
//! precondition established rather than raced. The assertion becomes deterministic: if the call now
//! fails, that is a real routing failure and not a slow runner.
//!
//! This is the same reasoning `await_subscriber` records for pub/sub. It was only ever half-applied
//! because pub/sub had the flaky test that prompted it and request/reply did not yet.

use std::time::Duration;

use crate::key::ws_key;
use crate::peer::{Bus, BusError};

/// How long to wait for a matching queryable before giving up. Generous on purpose: the barrier
/// returns the instant the queryable is visible, so headroom costs nothing on a healthy mesh and
/// only bites a path that is genuinely never going to converge.
const READY_DEADLINE: Duration = Duration::from_secs(20);

/// Wait until `(ws, rel)` has at least one matching **queryable** reachable on the mesh, then
/// return `Ok(true)`. Returns `Ok(false)` if the deadline elapses with none — the caller may still
/// query, it just has no responder yet. Errors only on a Zenoh fault.
///
/// Event-driven, not polled: `matching_status()` is checked once for the already-converged case,
/// then a `matching_listener` is awaited for the transition. There is no sleep interval to tune.
///
/// The listener is declared BEFORE the status check on purpose: a queryable appearing between the
/// two would otherwise fire its event into nothing and the wait would block to its full deadline on
/// a mesh that had already converged. That window is microseconds wide and no test here reproduces
/// it (`tests/await_queryable_test.rs` says so explicitly) — the ordering costs nothing and is kept
/// on the argument alone.
pub async fn await_queryable(bus: &Bus, ws: &str, rel: &str) -> Result<bool, BusError> {
    let key = ws_key(ws, rel);
    let querier = bus
        .session()
        .declare_querier(&key)
        .await
        .map_err(|e| BusError::Session(e.to_string()))?;

    // Declare the listener FIRST. Checking status first and subscribing after leaves a gap in which
    // the queryable appears, its event fires into nothing, and the wait then blocks for the whole
    // deadline on a mesh that had already converged.
    let listener = querier
        .matching_listener()
        .await
        .map_err(|e| BusError::Session(e.to_string()))?;

    if querier
        .matching_status()
        .await
        .map_err(|e| BusError::Session(e.to_string()))?
        .matching()
    {
        return Ok(true);
    }

    let deadline = tokio::time::Instant::now() + READY_DEADLINE;
    loop {
        match tokio::time::timeout_at(deadline, listener.recv_async()).await {
            // A transition arrived: matching ⇒ ready; un-matching ⇒ keep waiting for the next one.
            Ok(Ok(status)) if status.matching() => return Ok(true),
            Ok(Ok(_)) => continue,
            // The listener closed (session going away) — report "not ready" rather than hanging.
            Ok(Err(_)) => return Ok(false),
            Err(_) => return Ok(false),
        }
    }
}
