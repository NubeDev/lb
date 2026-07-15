//! `store.status` — the observability read: log bytes, segment count, last-compaction outcome,
//! and the threshold advisory. Authorizes (`store:status:read`), then stats files; it never
//! reads a record as any principal (compaction lives below the namespace wall).

use lb_auth::Principal;
use lb_store::{status, CompactionRecord, Store};
use serde::{Deserialize, Serialize};

use super::authorize::authorize_store_status;
use super::error::StoreAdminError;

/// Advisory threshold: warn (in the reactor, and in this verb's `advisory`) once the commit log
/// exceeds this many bytes. Chosen from the measured incident (a 1.5 GB log over a ~23 MB live
/// set → a 13–14 s boot): 256 MiB is loud well before the boot pain, and far above any healthy
/// compacted set seen so far. Absolute bytes, not a live-set multiple — a cheap live estimate
/// does not exist yet (scope OQ3; revisit when one does).
pub const LOG_ADVISORY_BYTES: u64 = 256 * 1024 * 1024;

/// The `store.status` result: the store crate's snapshot plus the advisory the reactor logs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreStatusReport {
    pub persistent: bool,
    pub log_bytes: u64,
    pub segment_count: u32,
    pub last_compaction: Option<CompactionRecord>,
    /// The advisory threshold this node warns at.
    pub threshold_bytes: u64,
    /// Present iff `log_bytes` exceeds the threshold — the same string the reactor logs.
    pub advisory: Option<String>,
}

/// Read the store's operational status (gated `store:status:read`).
pub fn store_status_run(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<StoreStatusReport, StoreAdminError> {
    authorize_store_status(principal, ws)?;
    let snap = status(store);
    let advisory = over_threshold_advisory(snap.log_bytes, LOG_ADVISORY_BYTES);
    Ok(StoreStatusReport {
        persistent: snap.persistent,
        log_bytes: snap.log_bytes,
        segment_count: snap.segment_count,
        last_compaction: snap.last_compaction,
        threshold_bytes: LOG_ADVISORY_BYTES,
        advisory,
    })
}

/// The advisory line, iff the log is over `threshold`. Pure — the reactor's tick test pins that
/// a quiet store produces `None` (no warning, no pass) without spinning a reactor up.
pub fn over_threshold_advisory(log_bytes: u64, threshold: u64) -> Option<String> {
    (log_bytes > threshold).then(|| {
        format!(
            "store commit log is {log_bytes} bytes (threshold {threshold}): boot replays every \
             byte of it — run store.compact (a job) to rewrite it down to the live set"
        )
    })
}
