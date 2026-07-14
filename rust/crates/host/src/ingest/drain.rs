//! The ingest **commit worker** — the read-side analog of the outbox relay. Drains `ws`'s staging
//! and commits it to the `series` tables, one batch = one transaction (exactly-once on re-drain).
//! Mounted by the **ingest role** (config, like the gateway / sync relay — no `if cloud`); a node
//! without the role simply never calls this, so its staging is never drained *there* (one
//! authoritative ingest path per producer, ingest scope).
//!
//! A restart re-drains uncommitted staging: because the commit deletes the staged row in the SAME
//! transaction as the series upsert, a crash mid-commit rolls the whole batch back and the next
//! drain re-commits it exactly once. This is the durable, exactly-once round-trip the slice proves.
//!
//! **Two bounds, one worker (drain-backpressure scope).** Draining until empty is right for the
//! background reactor ([`spawn_ingest_reactors`](super::spawn_ingest_reactors)) and for tests, but it
//! is WRONG on a caller's path: `ingest.write` used to call [`drain_workspace`] and was therefore
//! billed for the whole workspace's backlog — one sample against a 4,671-row backlog measured 18.5s,
//! versus 21ms at backlog 0. So a caller uses [`drain_workspace_bounded`], which commits at most a
//! fixed number of batches: enough to make the caller's OWN samples visible to its very next read
//! over the same bridge (the round-trip `tool.rs` deliberately buys), never enough to make one
//! producer's latency scale with another producer's backlog. The reactor drains the remainder off
//! every caller's path.

use lb_ingest::commit_batch;
use lb_store::Store;

use super::error::IngestError;

/// The batch size one commit transaction drains. Kept modest so a single tx stays bounded; the
/// worker loops until staging is empty.
pub const COMMIT_BATCH: usize = 256;

/// Outcome of a full drain pass for one workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrainPass {
    pub committed: usize,
}

/// Drain ALL currently-staged samples in `ws`, committing in batches until staging is empty.
/// Returns the total committed. Idempotent across restarts (exactly-once per `(series, producer,
/// seq)`), so a re-drain after a kill never double-commits.
///
/// **Unbounded — for the background reactor and tests, never a caller's path.** Its cost is O(the
/// workspace's whole backlog); putting it on a request makes that request pay for every other
/// producer's staged rows. A caller wants [`drain_workspace_bounded`].
pub async fn drain_workspace(store: &Store, ws: &str) -> Result<DrainPass, IngestError> {
    drain_at_most(store, ws, usize::MAX).await
}

/// Drain at most `max_batches` batches (≤ `max_batches * COMMIT_BATCH` samples) from `ws`'s staging.
/// Returns the total committed, stopping early when staging empties.
///
/// **This is the caller-path drain.** `max_batches` bounds the caller's bill: its cost is O(batch),
/// not O(backlog), so one producer's write latency no longer scales with another's staging depth.
/// The remainder is the reactor's job.
pub async fn drain_workspace_bounded(
    store: &Store,
    ws: &str,
    max_batches: usize,
) -> Result<DrainPass, IngestError> {
    drain_at_most(store, ws, max_batches).await
}

/// How many batches a caller that just accepted `accepted` samples may drain on its own request —
/// the ONE definition of "pay for your own work, never the backlog", shared by every caller-path
/// drain (the MCP verb, the gateway's `POST /ingest`, the webhook accept, the federation mirror).
///
/// `ceil(accepted / COMMIT_BATCH)`, floor 1. The floor keeps the write-then-read round-trip honest
/// even for a zero-sample call; the ceiling means a caller writing more than one batch still commits
/// all of what it wrote inline — that cost is fair, it is the caller's own data. What it can NEVER
/// do is scale with another producer's backlog, which is the whole bug (drain-backpressure scope).
///
/// Note this is an upper bound on batches ATTEMPTED, not on whose rows commit: staging drains
/// oldest-first `(seq, ts)`, so a caller with a backlog ahead of it may spend its budget committing
/// some of that backlog instead of its own rows. That is bounded and fair (O(batch) either way), and
/// the reactor guarantees the remainder — including the caller's tail — commits shortly after.
pub fn own_batches(accepted: usize) -> usize {
    accepted.div_ceil(COMMIT_BATCH).max(1)
}

/// The one drain loop both bounds share — `max_batches` passes of `commit_batch`, stopping early on
/// an empty staging. Keeping ONE loop means the bounded and unbounded paths can never drift in their
/// exactly-once behaviour; they differ only in where they stop.
async fn drain_at_most(
    store: &Store,
    ws: &str,
    max_batches: usize,
) -> Result<DrainPass, IngestError> {
    let mut total = 0;
    for _ in 0..max_batches {
        let pass = commit_batch(store, ws, COMMIT_BATCH).await?;
        if pass.committed == 0 {
            break;
        }
        total += pass.committed;
    }
    Ok(DrainPass { committed: total })
}
