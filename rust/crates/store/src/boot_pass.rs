//! `boot_compact` — the **boot-time** compaction pass, preconditioned on this machine's memory and
//! on whether the last pass paid for itself (boot-memory-guard scope slice 1).
//!
//! The online pass ([`crate::compact`]) is deliberately NOT preconditioned: it runs on a node that
//! is already up, where a failed pass costs a skipped job rather than the box, and it is driven by
//! a disk budget that only compaction can satisfy. Boot is the one place the node predictably
//! doubles its memory demand before it can serve anything at all.
//!
//! **Ordering is load-bearing (P0).** A pending `.merge/` is completed FIRST, before any skip
//! decision — skipping compaction must never mean skipping merge completion, or the next *writing*
//! open applies the merge itself and silently eats that session's writes
//! (`debugging/store/compaction-merge-eats-next-sessions-writes.md`). A merge apply is a sub-KB
//! rename-class operation; it is not the memory hazard the pass is.

use crate::boot_guard::boot_compaction_skip;
use crate::compact::{compact_log, complete_pending_merge, epoch_ms};
use crate::compaction_record::CompactionRecord;
use crate::last_pass::{load_last_compaction, store_last_compaction};
use crate::status::log_stats;

/// Run (or deliberately decline) the boot compaction pass at `path`. `available_ram` is this
/// machine's `MemAvailable`, or `None` when it cannot be measured (⇒ the headroom precondition
/// passes — fail open).
///
/// Blocking whole-log file I/O when it does run: call via `spawn_blocking`.
pub(crate) fn boot_compact(path: &str, available_ram: Option<u64>) -> CompactionRecord {
    let dir = std::path::Path::new(path);
    if !dir.exists() {
        // A fresh path (no store yet): nothing to compact, nothing to decide, not an error.
        return CompactionRecord {
            at_epoch_ms: epoch_ms(),
            ok: true,
            before_bytes: 0,
            after_bytes: 0,
            duration_ms: 0,
            error: None,
            skipped: None,
            phases: crate::compaction_record::CompactionPhases::default(),
        };
    }

    // P0 FIRST — before the preconditions can decline anything (see the module doc).
    if dir.join(".merge").exists() {
        if let Err(e) = complete_pending_merge(dir) {
            tracing::warn!(
                path = %path,
                error = %e,
                "store: could not complete a pending compaction merge at boot — continuing; the \
                 next pass retries it"
            );
            return CompactionRecord {
                at_epoch_ms: epoch_ms(),
                ok: false,
                before_bytes: 0,
                after_bytes: 0,
                duration_ms: 0,
                error: Some(format!("pending-merge completion: {e}")),
                skipped: None,
                phases: crate::compaction_record::CompactionPhases::default(),
            };
        }
    }

    let (log_bytes, _) = log_stats(path);
    let last = load_last_compaction(path);
    if let Some(reason) = boot_compaction_skip(log_bytes, available_ram, last.as_ref()) {
        tracing::warn!(
            path = %path,
            log_bytes,
            available_ram_bytes = ?available_ram,
            last_before_bytes = ?last.as_ref().map(|r| r.before_bytes),
            last_after_bytes = ?last.as_ref().map(|r| r.after_bytes),
            "store: SKIPPING the boot compaction pass — {reason}. This is a heuristic guard \
             (boot-memory-guard, issue #128); the node still opens and the online store.compact \
             job is unaffected."
        );
        // NOT persisted: the sidecar holds the last pass that actually RAN, which is precisely the
        // input the next boot's precondition needs. Overwriting it with a skip would erase it.
        return CompactionRecord {
            at_epoch_ms: epoch_ms(),
            ok: false,
            before_bytes: log_bytes,
            after_bytes: log_bytes,
            duration_ms: 0,
            error: None,
            phases: crate::compaction_record::CompactionPhases::default(),
            skipped: Some(reason),
        };
    }

    let rec = compact_log(path);
    store_last_compaction(path, &rec);
    rec
}
