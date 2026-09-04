//! `commit_direct` — THE ingest write path: commit a caller's batch to the series plane.
//!
//! ## What it replaced, and why
//!
//! Until this, `ingest.write` appended each sample to a durable `ingest_staging` table and a
//! background worker later moved it into `series`. Storing one sample therefore cost **three** writes
//! to the same store: the staging UPSERT, the `series` UPSERT at commit, and the staging DELETE
//! tombstone. That is now one.
//!
//! Staging was justified as a "cheap unindexed landing zone" — a burst would land somewhere cheap
//! and the expensive indexed `series` write would happen later, off the burst. Both halves of that
//! were wrong:
//!
//! - **The landing zone was not cheap.** It was a table in the same database, so a staged sample paid
//!   the same write-ahead-log append and the same memtable insert as a committed one, plus a
//!   tombstone when it left. Staging did not defer work; it added work, and it added it to the same
//!   store that was already loaded.
//! - **The indexed write it deferred was not expensive.** The engine underneath is an LSM tree, where
//!   a secondary index entry is just another key-value pair appended to the same memtable and log.
//!   There is no index page to read, lock, or rewrite, so the "expensive" write staging saved us from
//!   costs roughly one extra append.
//!
//! Measured on this store, 200,000 samples cost **115,398 ms and 11.87 MB** through staging against
//! **3,752 ms and 4.02 MB** committed directly.
//!
//! Staging was also described as backpressure. It was not: when the store was too loaded to take the
//! write, staging responded by writing to that same store two extra times. Real backpressure is a
//! buffer somewhere the store is not — a producer's own memory, for instance — and that belongs to
//! the producer, not here.
//!
//! ## What the caller gets
//!
//! `ingest.write` acks only after the store write returns, and here that write is the COMMIT of the
//! very transaction that stores the sample — strictly stronger than the old "durably staged, commit
//! pending". A crash before COMMIT rolls the whole batch back and the producer never saw an ack, so
//! a must-deliver producer re-pushes and the `[series, producer, seq]` UPSERT absorbs it exactly
//! once. A sample is visible to the caller's very next read the moment its write returns; under
//! staging it was not visible until a drain ran.

use lb_store::{Store, StoreError};

use crate::commit::{commit_samples, CommitPass};
use crate::commit_lock::ws_commit_lock;
use crate::meta::DEFAULT_SERIES_CAP;
use crate::sample::Sample;

/// How many samples one commit transaction carries.
///
/// **This bound is load-bearing, not tidiness.** [`commit_samples`] builds one statement per sample
/// into a single `BEGIN…COMMIT`, so without a chunk the transaction grows with whatever a producer
/// chose to push — an unbounded statement string, an unbounded write set, and an unbounded conflict
/// window, all on a caller's request path.
///
/// A measurement suggested the cost is not merely linear but superlinear in transaction size —
/// 2400 samples in one transaction produced substantially more commit-log growth than the same
/// samples in ten. It is recorded as an observation only, and no test asserts it: the commit log
/// cannot be metered reliably in-process (see `ingest_write_amplification_test`), so the number is
/// indicative and the bound above stands on the argument, not on it.
///
/// A large push is therefore several transactions rather than one. Nothing weakens: exactly-once is
/// keyed per sample on
/// `[series, producer, seq]`, so a failure part-way leaves the earlier chunks committed, the caller
/// un-acked, and the producer's re-push idempotent on every one of them.
pub const DIRECT_COMMIT_BATCH: usize = 256;

/// Commit `samples` to `ws`'s series plane, under the default cardinality cap, in transactions of
/// at most [`DIRECT_COMMIT_BATCH`] samples.
pub async fn commit_direct(
    store: &Store,
    ws: &str,
    samples: &[Sample],
) -> Result<CommitPass, StoreError> {
    commit_direct_capped(store, ws, samples, DEFAULT_SERIES_CAP).await
}

/// [`commit_direct`] with an explicit per-workspace cap on distinct series names (`0` = unbounded).
pub async fn commit_direct_capped(
    store: &Store,
    ws: &str,
    samples: &[Sample],
    series_cap: usize,
) -> Result<CommitPass, StoreError> {
    let mut out = CommitPass::default();
    // Serialize each transaction against other writers in this workspace (`commit_lock`): every
    // commit reads and conditionally advances the same `series_latest` rows, and concurrent writers
    // to one series otherwise collide under optimistic MVCC until the retry budget is gone. Taken
    // per CHUNK so a large push never blocks a small one for longer than one transaction.
    let lock = ws_commit_lock(ws);
    for chunk in samples.chunks(DIRECT_COMMIT_BATCH) {
        let pass = {
            let _guard = lock.lock().await;
            commit_samples(store, ws, chunk, series_cap).await?
        };
        out.committed += pass.committed;
        out.dead_lettered += pass.dead_lettered;
        let f = &mut out.filtered;
        f.muted += pass.filtered.muted;
        f.range += pass.filtered.range;
        f.min_interval += pass.filtered.min_interval;
        f.deadband += pass.filtered.deadband;
        f.clamped += pass.filtered.clamped;
    }
    Ok(out)
}
