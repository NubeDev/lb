//! `status` — the store's operational snapshot: commit-log size, segment count, and the last
//! compaction outcome. The read half of the online-compaction scope's "observable before
//! painful" goal: one cheap verb call answers "why is this node's disk/boot growing" without
//! touching a single record (it stats files, below the namespace wall — no principal, no rows).

use serde::{Deserialize, Serialize};

use crate::compact::CompactionRecord;
use crate::open::Store;

/// Snapshot served by the `store.status` verb.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreStatus {
    /// False for a `memory()` store (no log; the byte fields are zero).
    pub persistent: bool,
    /// Total bytes across the commit-log segments (`clog/*.clog`) — what open replays at boot.
    pub log_bytes: u64,
    /// Number of commit-log segment files.
    pub segment_count: u32,
    /// Outcome of the most recent compaction pass (boot or online) in this process, if any.
    pub last_compaction: Option<CompactionRecord>,
}

/// Read the store's operational status. Cheap: file metadata only, no store queries, no lock.
pub fn status(store: &Store) -> StoreStatus {
    let (log_bytes, segment_count) = store.dir().map(log_stats).unwrap_or((0, 0));
    StoreStatus {
        persistent: store.dir().is_some(),
        log_bytes,
        segment_count,
        last_compaction: store
            .last_compaction_slot()
            .lock()
            .expect("last_compaction poisoned")
            .clone(),
    }
}

/// Sum the commit-log segment sizes under `path` → (bytes, segment file count). Zero for a
/// path with no store yet.
pub(crate) fn log_stats(path: impl AsRef<std::path::Path>) -> (u64, u32) {
    let clog = path.as_ref().join("clog");
    let mut bytes = 0u64;
    let mut count = 0u32;
    if let Ok(rd) = std::fs::read_dir(clog) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) == Some("clog") {
                bytes += e.metadata().map(|m| m.len()).unwrap_or(0);
                count += 1;
            }
        }
    }
    (bytes, count)
}
