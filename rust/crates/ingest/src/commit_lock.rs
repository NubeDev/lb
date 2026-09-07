//! `ws_commit_lock` — the per-workspace serializer that stops two commit transactions racing on the
//! same workspace's `series_latest` rows.
//!
//! ## Why
//!
//! Every commit transaction reads a series' `series_latest` pointer and conditionally UPSERTs it
//! (the forward-only guard in [`commit_samples`](crate::commit_samples)). Under SurrealDB's
//! optimistic MVCC a read of a row another open transaction is writing is a conflict: the loser
//! aborts with "This transaction can be retried". Producers writing the SAME series all touch the
//! SAME pointer row, so with several of them pushing at once every transaction collides with every
//! other one — and because each transaction carries up to `DIRECT_COMMIT_BATCH` statements, the
//! collision window is wide. Measured at six concurrent producers on one series, the bounded retry
//! (16 attempts, sub-millisecond backoff) is exhausted and a whole batch is surfaced as an error.
//!
//! Retrying harder is the wrong answer: six transactions each doing the same work up to sixteen
//! times is strictly more work than six doing it once in turn. Serializing them removes the
//! collision at its source, and the retry stays underneath as the backstop for the collisions this
//! lock cannot see — the GC pass evicting raw from `series`, and any other process on the same store.
//!
//! ## Shape
//!
//! A process-wide `static` keyed by workspace, the same idiom `store::write_locked`/`increment` use
//! for their per-record locks. Different workspaces never contend.
//!
//! **Held per TRANSACTION, not per push, on purpose.** `commit_direct` chunks a large push into
//! several transactions and re-takes this lock for each one, so a producer pushing 100,000 samples
//! never blocks a producer pushing 10 for the whole of its push — the longest anyone waits is one
//! chunk. Holding it across the whole push would make one caller's latency scale with another
//! caller's batch size, which is the coupling the drain-backpressure work removed and must not
//! return through a new door.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use tokio::sync::Mutex as AsyncMutex;

/// The async lock guarding `ws`'s commit transactions. One `Arc<Mutex<()>>` per workspace, minted on
/// first use; different workspaces get different locks and never block each other.
pub(crate) fn ws_commit_lock(ws: &str) -> Arc<AsyncMutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<String, Arc<AsyncMutex<()>>>>> = OnceLock::new();
    let map = LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = map.lock().expect("ingest commit-lock map poisoned");
    guard
        .entry(ws.to_string())
        .or_insert_with(|| Arc::new(AsyncMutex::new(())))
        .clone()
}
