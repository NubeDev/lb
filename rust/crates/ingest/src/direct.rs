//! `commit_direct` — commit a caller's live batch straight to the series plane, skipping staging.
//!
//! **Why this exists (compaction-write-availability scope, lever 1).** The engine under the store is
//! append-only: every write, every superseded version and every tombstone stays in the commit log
//! until a compaction pass rewrites it. On the staged path one committed sample costs **three** log
//! appends — the staging UPSERT ([`crate::write`]), the `series` UPSERT at commit, and the staging
//! DELETE tombstone in the same transaction. Measured on an armv7 edge node: ~500 MB of log growth
//! per hour at a modest two-network modbus poll, which is also the node's RSS high-water mark
//! (engine memory tracks the log at ~1.4× for key-dense samples) and which drove an hourly ~94 s
//! stop-the-world compaction pass. Cutting three appends to one stretches every one of those
//! consequences out by the same factor.
//!
//! **Why it is safe.** Staging buys two things: a cheap unindexed landing zone when a burst arrives
//! faster than the indexed commit can absorb it, and durability for samples the caller's own request
//! will not commit. A live producer whose caller-path drain commits its own batch within the same
//! request gets neither — it stages, immediately drains what it just staged, and tombstones it. For
//! that caller the round-trip is pure amplification.
//!
//! Acceptance is unchanged: `ingest.write` acks only after the store write returns, and here that
//! write is the COMMIT of the very transaction that stores the sample — strictly stronger than
//! "durably staged, commit pending". A crash before COMMIT rolls the whole batch back and the
//! producer never saw an ack, so a must-deliver producer re-pushes and the `[series, producer, seq]`
//! UPSERT absorbs it exactly once — the same contract, one hop shorter.
//!
//! **When the staged path still runs** ([`crate::write`] decides): whenever staging is not already
//! empty. A non-empty staging means either a backlog the reactor is still working through or a
//! concurrent producer, and in both cases the batch must queue behind what is already there rather
//! than commit past it. Crash recovery, bursts, and every offline/backlog case therefore keep
//! staging exactly as before — this path only removes the round-trip that was provably a no-op.

use lb_store::{Store, StoreError};

use crate::commit::{commit_staged, CommitPass, Dequeue};
use crate::meta::DEFAULT_SERIES_CAP;
use crate::sample::Sample;
use crate::staging::Staged;

/// How many samples one direct-commit transaction carries. Matches the staged drain's
/// `COMMIT_BATCH` deliberately: it is the batch size the whole ingest plane is tuned around, and the
/// two paths having different transaction shapes is exactly how their costs come to differ for no
/// stated reason.
///
/// **This bound is load-bearing, not tidiness.** `commit_staged` builds one statement per sample
/// into a single `BEGIN…COMMIT`, so without a chunk the transaction grows with whatever a producer
/// chose to push — an unbounded statement string, an unbounded write set, and an unbounded conflict
/// window, all on a caller's request path. The staged drain has always refused that (`COMMIT_BATCH`,
/// "kept modest so a single tx stays bounded"); a second path that did not would be the same hazard
/// re-entered through a new door.
///
/// A measurement suggested the cost is not merely linear but superlinear in transaction size —
/// 2400 samples in one transaction produced substantially more commit-log growth than the same
/// samples in ten. It is recorded as an observation only, and no test asserts it: the commit log
/// cannot be metered reliably in-process (see `ingest_write_amplification_test`), so the number is
/// indicative and the bound above stands on the argument, not on it.
///
/// A large push is therefore several transactions rather than one, exactly as the staged drain
/// already made it. Nothing weakens: exactly-once is keyed per sample on
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
///
/// Runs the SAME transaction builder the staged drain runs ([`commit_staged`]) — same UPSERT key,
/// same cardinality gate, same normalize filters, same forward-only `series_latest` pointer, same
/// filter anchors in the same tx — differing only in that there is no staged row to delete.
pub async fn commit_direct_capped(
    store: &Store,
    ws: &str,
    samples: &[Sample],
    series_cap: usize,
) -> Result<CommitPass, StoreError> {
    let mut out = CommitPass::default();
    for chunk in samples.chunks(DIRECT_COMMIT_BATCH) {
        // `Staged` is the commit builder's input shape, not a claim that these rows were staged: it
        // is the `Sample` and nothing else (`staging::Staged`), so this wrap is free of meaning.
        let rows: Vec<Staged> = chunk.iter().map(|s| Staged { sample: s.clone() }).collect();
        let pass = commit_staged(store, ws, &rows, series_cap, Dequeue::No).await?;
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
