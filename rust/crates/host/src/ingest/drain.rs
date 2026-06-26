//! The ingest **commit worker** — the read-side analog of the outbox relay. Drains `ws`'s staging
//! and commits it to the `series` tables, one batch = one transaction (exactly-once on re-drain).
//! Mounted by the **ingest role** (config, like the gateway / sync relay — no `if cloud`); a node
//! without the role simply never calls this, so its staging is never drained *there* (one
//! authoritative ingest path per producer, ingest scope).
//!
//! A restart re-drains uncommitted staging: because the commit deletes the staged row in the SAME
//! transaction as the series upsert, a crash mid-commit rolls the whole batch back and the next
//! drain re-commits it exactly once. This is the durable, exactly-once round-trip the slice proves.

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
pub async fn drain_workspace(store: &Store, ws: &str) -> Result<DrainPass, IngestError> {
    let mut total = 0;
    loop {
        let pass = commit_batch(store, ws, COMMIT_BATCH).await?;
        if pass.committed == 0 {
            break;
        }
        total += pass.committed;
    }
    Ok(DrainPass { committed: total })
}
