//! `store.status` — the observability read: log bytes, segment count, last-compaction outcome,
//! and the threshold advisory. Authorizes (`store:status:read`), then stats files; it never
//! reads a record as any principal (compaction lives below the namespace wall).

use lb_auth::Principal;
use lb_store::{status, CompactionRecord, Store};
use serde::{Deserialize, Serialize};

use super::authorize::authorize_store_status;
use super::error::StoreAdminError;
use super::marks::budget_marks;

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
    /// The advisory threshold this node warns at: the soft mark when a budget is configured, else
    /// the flat [`LOG_ADVISORY_BYTES`].
    pub threshold_bytes: u64,
    /// Present iff `log_bytes` exceeds the threshold — the same string the reactor logs.
    pub advisory: Option<String>,
    /// The node's configured store disk budget (`BootConfig::store_budget_bytes`), if any. `None`
    /// ⇒ unbudgeted: the flat advisory, no marks, nothing auto-triggers.
    pub budget_bytes: Option<u64>,
    /// Bytes left before the budget is spent (`budget - log_bytes`, saturating). `None` when
    /// unbudgeted.
    pub headroom_bytes: Option<u64>,
    /// Free space on the filesystem holding the store directory, when this node can measure it.
    ///
    /// This is deliberately a **different** number from the headroom: the budget bounds the *store
    /// directory*, not the partition — extension artifacts, sidecar binaries and OS logs share the
    /// disk and are outside it. It also answers the question the budget cannot: compaction rewrites
    /// the log, so it needs room to run, and a budget set close to the physical disk can leave the
    /// remedy unable to fit.
    ///
    /// `None` when the free-space figure is unavailable (see [`free_disk_bytes`]) — never guessed.
    pub free_disk_bytes: Option<u64>,
}

/// Read the store's operational status (gated `store:status:read`) on an **unbudgeted** node —
/// today's behaviour exactly. Equivalent to [`store_status_run_with_budget`] with `None`.
pub fn store_status_run(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<StoreStatusReport, StoreAdminError> {
    store_status_run_with_budget(store, principal, ws, None)
}

/// Read the store's operational status against a node's configured disk budget (gated
/// `store:status:read`). `budget` is `BootConfig::store_budget_bytes` — `None` ⇒ the flat
/// [`LOG_ADVISORY_BYTES`] advisory and no marks (scope decision 1).
pub fn store_status_run_with_budget(
    store: &Store,
    principal: &Principal,
    ws: &str,
    budget: Option<u64>,
) -> Result<StoreStatusReport, StoreAdminError> {
    authorize_store_status(principal, ws)?;
    let snap = status(store);
    let marks = budget_marks(budget);
    let advisory = over_threshold_advisory(snap.log_bytes, marks.threshold_bytes);
    Ok(StoreStatusReport {
        persistent: snap.persistent,
        log_bytes: snap.log_bytes,
        segment_count: snap.segment_count,
        last_compaction: snap.last_compaction,
        threshold_bytes: marks.threshold_bytes,
        advisory,
        budget_bytes: marks.budget_bytes,
        headroom_bytes: marks.headroom_bytes(snap.log_bytes),
        free_disk_bytes: free_disk_bytes(),
    })
}

/// Free bytes on the filesystem holding the store directory, or `None` when this build cannot
/// measure it.
///
/// Today it is always `None`: the figure needs a `statvfs`-class syscall, `std` exposes none, and
/// no filesystem-stat crate (`libc`, `nix`, `fs4`, `sysinfo`) is a direct dependency of this
/// workspace. Adding one is a real decision with a real cost, so the field ships honestly absent
/// rather than guessed — a wrong free-disk number is worse than no number, because the operator
/// would plan a compaction against it. The seam is here so filling it in is a one-function change
/// with no shape churn for callers.
fn free_disk_bytes() -> Option<u64> {
    None
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
