//! Compaction — **a no-op since SurrealDB 3 / surrealkv 0.21**.
//!
//! ## Why this file is now empty of machinery
//!
//! Under surrealkv 0.9.x the engine was an append-only commit log: every write, every superseded
//! version and every tombstone stayed in the log for ever, and only a manual `Store::compact()`
//! pass reclaimed any of it. surrealdb 2.x exposed no path to that call, so this module reached
//! past surrealdb with a SECOND direct `surrealkv` handle — quiescing writes, swapping the live
//! handle out, compacting on disk, reopening, and working around an upstream ordering bug that
//! silently lost a merge-applying session's writes (lb#68).
//!
//! surrealkv 0.21 is an LSM tree. Compaction is **automatic and continuous** (`task.rs`'s
//! `TaskManager` runs memtable-flush and level-compaction as background tasks), there is no
//! stop-the-world pass, and open reads a manifest plus the WAL tail rather than replaying history.
//! There is nothing left to invoke, so the direct handle, the engine-options mirroring, the
//! merge-completion rule and the boot-time pass all go away with it.
//!
//! The public shape is kept so callers (the `store.compact` verb, the budget driver, `store.status`)
//! keep compiling; each call now reports "nothing to do" rather than doing whole-log I/O.

use crate::compaction_record::{CompactionPhases, CompactionRecord};
use crate::open::{Store, StoreError};

/// A record describing a pass that did not need to happen.
fn noop_record(reason: &str) -> CompactionRecord {
    CompactionRecord {
        at_epoch_ms: epoch_ms(),
        ok: true,
        before_bytes: 0,
        after_bytes: 0,
        duration_ms: 0,
        error: None,
        skipped: Some(reason.to_string()),
        phases: CompactionPhases::default(),
    }
}

/// Online pass: nothing to do, and — importantly — **no write stall**. The previous implementation
/// held the handle write guard for the duration of whole-log I/O (~94 s measured on RC-6).
pub async fn compact(store: &Store) -> Result<CompactionRecord, StoreError> {
    let _ = store;
    Ok(noop_record(
        "engine compacts automatically (surrealkv 0.21 LSM)",
    ))
}

pub(crate) fn epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
