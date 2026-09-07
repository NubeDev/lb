//! `status` — the store's operational snapshot: on-disc size, segment count, and the last
//! compaction outcome. The read half of the online-compaction scope's "observable before
//! painful" goal: one cheap verb call answers "why is this node's disc growing" without
//! touching a single record (it stats files, below the namespace wall — no principal, no rows).
//!
//! **The measurement follows the engine's layout, and that layout changed.** surrealkv 0.9 kept
//! everything in one append-only commit log at `clog/*.clog`; surrealkv 0.21 is an LSM tree and
//! writes `sstables/*.sst` (the data), `wal/` (the write-ahead log), `vlog/` (the value log),
//! `versioned_index/` and `manifest`. Measuring `clog/` under 0.21 finds nothing — every one of
//! those directories is absent — so `log_bytes` collapses to the size of `manifest` alone: a few
//! kilobytes reported for a store holding gigabytes.
//!
//! That is not a cosmetic under-count. `store_admin`'s budget driver decides purely on this number
//! (`budget.rs`: below the soft mark it returns `Idle`), so a `log_bytes` pinned near zero means the
//! soft and hard marks are never crossed and **the disc budget never fires at all** — the one
//! mechanism standing between a node and a full disc, silently inert. So this file sums what the
//! engine actually writes, and [`STORE_DIRS`] is the single list of where that is.

use serde::{Deserialize, Serialize};

use crate::compaction_record::CompactionRecord;
use crate::open::Store;

/// Snapshot served by the `store.status` verb.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreStatus {
    /// False for a `memory()` store (no log; the byte fields are zero).
    pub persistent: bool,
    /// Total bytes the store occupies on disc: every directory the engine writes, summed
    /// recursively, plus the `manifest`. See [`STORE_DIRS`].
    ///
    /// The name is historical — under surrealkv 0.9 the store WAS one commit log. It is kept
    /// because it is the field the `store.status` verb and the disc budget already speak, and
    /// renaming it would break every reader for no gain in truth.
    pub log_bytes: u64,
    /// Number of on-disc data segment files — `sstables/*.sst` under surrealkv 0.21, which is what
    /// a segment is now. The `wal`, `vlog` and `manifest` contribute bytes, never a segment.
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

/// Every directory surrealkv writes beneath the store path, and the one plain file beside them.
///
/// Listed in ONE place so a future engine-layout change is a single edit rather than a silent
/// under-count: the 0.9 → 0.21 upgrade moved the data out of `clog/` and nothing here noticed,
/// which is exactly the failure this constant exists to make impossible to repeat.
///
/// `clog` stays on the list deliberately. It is dead under 0.21, costs one failed `read_dir` when
/// absent, and means a directory left behind by an older engine is still counted against the disc
/// budget rather than being invisible bytes on the volume.
pub const STORE_DIRS: &[&str] = &["sstables", "wal", "vlog", "versioned_index", "clog"];

/// The extension of an on-disc data segment — what [`StoreStatus::segment_count`] counts.
const SEGMENT_EXT: &str = "sst";

/// Sum the store's on-disc bytes under `path` → (bytes, segment file count). Walks each of
/// [`STORE_DIRS`] recursively and adds the sibling `manifest`. Zero for a path with no store yet.
pub(crate) fn log_stats(path: impl AsRef<std::path::Path>) -> (u64, u32) {
    let path = path.as_ref();
    let mut bytes = 0u64;
    let mut segments = 0u32;
    for dir in STORE_DIRS {
        let (b, s) = dir_stats(&path.join(dir));
        bytes += b;
        segments += s;
    }
    bytes += manifest_bytes(&path.join("manifest"));
    (bytes, segments)
}

/// Recursively sum one directory's file bytes, counting `.sst` files as segments.
///
/// Recursive because surrealkv nests: `versioned_index/` holds `index.bpt`, and the sstable tree is
/// free to grow levels under it. A flat `read_dir` would miss whatever it nests next, which is the
/// same class of silent under-count as reading `clog/`. A missing directory is 0, not an error —
/// a memory store, a fresh path, or a layout that no longer uses it.
fn dir_stats(dir: &std::path::Path) -> (u64, u32) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return (0, 0);
    };
    let mut bytes = 0u64;
    let mut segments = 0u32;
    for e in rd.flatten() {
        let p = e.path();
        let Ok(meta) = e.metadata() else { continue };
        if meta.is_dir() {
            let (b, s) = dir_stats(&p);
            bytes += b;
            segments += s;
        } else if meta.is_file() {
            bytes += meta.len();
            if p.extension().and_then(|x| x.to_str()) == Some(SEGMENT_EXT) {
                segments += 1;
            }
        }
    }
    (bytes, segments)
}

/// Bytes of the store's `manifest`, whichever shape the engine wrote it in: a plain file (its
/// length) or a directory (the sum of its immediate entries, which is all SurrealKV ever writes
/// there). Missing ⇒ 0 — a memory store, or a directory with no store yet.
fn manifest_bytes(manifest: &std::path::Path) -> u64 {
    let Ok(meta) = std::fs::metadata(manifest) else {
        return 0;
    };
    if meta.is_file() {
        return meta.len();
    }
    std::fs::read_dir(manifest)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| e.metadata().ok())
                .filter(|m| m.is_file())
                .map(|m| m.len())
                .sum()
        })
        .unwrap_or(0)
}
