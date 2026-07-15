//! Compact the SurrealKV commit log — the boot-time pass ([`compact_log`], called by
//! `Store::open`) and the **online** pass ([`compact`], the `store.compact` job's engine).
//!
//! The engine is append-only: every write (including each superseded version and every
//! tombstone) stays in the log forever, and open replays ALL of it to rebuild the in-memory
//! index — so boot time grows with write history, not with live data. SurrealKV ships
//! `Store::compact()`, but surrealdb 2.x exposes no path to it, so it is invoked here directly,
//! on the same engine version cargo resolves for surrealdb (one `surrealkv` copy in the lock).
//!
//! **The merge-completion rule (P0, do not remove).** `surrealkv::Store::compact()` does NOT
//! rewrite the log in place: it writes the live set into `.merge/` and the swap happens at the
//! NEXT `Store::new` (`restore_from_compaction`). At the pinned surrealkv 0.9.3 that next open
//! applies the merge with the append-log **already open**, leaving the session appending into
//! unlinked inodes — every write made in a merge-applying session is silently lost at close
//! (debugging/store/compaction-merge-eats-next-sessions-writes.md; upstream ordering bug in
//! `Core::new`). So this module guarantees **no writing session ever applies a merge**: after
//! `compact()` we immediately do a throwaway open+close (applies the merge, writes nothing),
//! and if that throwaway fails the fresh `.merge/` is DELETED — dropping a compaction is always
//! safe (the old log is untouched until a merge applies); leaving one pending never is.
//!
//! Best-effort by contract: any failure leaves the log valid and only costs a slower boot /
//! a skipped pass; errors are recorded in the returned [`CompactionRecord`], never panicked.

use serde::{Deserialize, Serialize};

use surrealdb::engine::local::SurrealKv;
use surrealdb::Surreal;

use crate::open::{Store, StoreError};
use crate::status::log_stats;

/// Outcome of one compaction pass (boot or online). Served by `status`, returned by the
/// `store.compact` job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionRecord {
    /// Wall-clock epoch ms when the pass finished.
    pub at_epoch_ms: u64,
    pub ok: bool,
    /// Commit-log bytes before / after the pass (after includes the applied merge).
    pub before_bytes: u64,
    pub after_bytes: u64,
    /// How long the blocking pass took.
    pub duration_ms: u64,
    /// The failure, when `ok` is false. A failed pass leaves the log exactly as it was.
    pub error: Option<String>,
}

/// Engine options for a direct `surrealkv` handle — MUST mirror surrealdb's own wrapper
/// (`surrealdb-core/src/kvs/surrealkv/mod.rs`, the unversioned `surrealkv://` scheme lb uses):
/// versions off, disk persistence on, 512 MiB segments, 64-byte value threshold. SurrealKV
/// persists options in its manifest and merges on load, so the runtime-only cache knob is left
/// at its default.
fn engine_options(dir: &std::path::Path) -> surrealkv::Options {
    let mut opts = surrealkv::Options::new();
    opts.dir = dir.to_path_buf();
    opts.disk_persistence = true;
    opts.enable_versions = false;
    opts.max_segment_size = 1 << 29;
    opts.max_value_threshold = 64;
    opts
}

/// Open + close a throwaway direct handle: applies any pending `.merge/` (the physical swap)
/// while performing zero user writes. See the module doc for why this MUST happen before any
/// writing session opens the store.
fn complete_pending_merge(dir: &std::path::Path) -> Result<(), String> {
    let store = surrealkv::Store::new(engine_options(dir)).map_err(|e| e.to_string())?;
    store.close().map_err(|e| e.to_string())
}

/// Compact the SurrealKV commit log at `path` while **no other handle holds the directory**
/// (the caller guarantees that: boot runs it before SurrealDB opens; the online pass swaps the
/// handle out first). Blocking file I/O over the whole log — call via `spawn_blocking`.
pub(crate) fn compact_log(path: &str) -> CompactionRecord {
    let started = std::time::Instant::now();
    let dir = std::path::Path::new(path);
    let mut rec = CompactionRecord {
        at_epoch_ms: epoch_ms(),
        ok: false,
        before_bytes: 0,
        after_bytes: 0,
        duration_ms: 0,
        error: None,
    };
    let fail = |mut rec: CompactionRecord, started: std::time::Instant, e: String| {
        eprintln!("store: log compaction failed ({e}) — continuing on the uncompacted log");
        rec.error = Some(e);
        rec.duration_ms = started.elapsed().as_millis() as u64;
        rec.at_epoch_ms = epoch_ms();
        rec
    };

    if !dir.exists() {
        // A fresh path (no store yet): nothing to compact, and not an error.
        rec.ok = true;
        return rec;
    }
    let (before_bytes, _) = log_stats(path);
    rec.before_bytes = before_bytes;

    // An earlier interrupted run may have left a merge pending — complete it FIRST, so the
    // compacting open below never applies a merge itself (the merge-completion rule).
    if dir.join(".merge").exists() {
        if let Err(e) = complete_pending_merge(dir) {
            return fail(rec, started, format!("pending-merge completion: {e}"));
        }
    }

    let store = match surrealkv::Store::new(engine_options(dir)) {
        Ok(s) => s,
        Err(e) => return fail(rec, started, format!("open-for-compaction: {e}")),
    };
    if let Err(e) = store.compact() {
        let _ = store.close();
        return fail(rec, started, format!("compact: {e}"));
    }
    if let Err(e) = store.close() {
        return fail(rec, started, format!("close-after-compaction: {e}"));
    }

    // Apply the merge NOW, with a non-writing session. If this fails, drop the compaction —
    // a pending merge left behind would be applied by the (writing!) surrealdb open and eat
    // that session's writes (the P0).
    if let Err(e) = complete_pending_merge(dir) {
        let _ = std::fs::remove_dir_all(dir.join(".merge"));
        let _ = std::fs::remove_dir_all(dir.join(".tmp.merge"));
        return fail(
            rec,
            started,
            format!("merge-completion (compaction dropped): {e}"),
        );
    }

    let (after_bytes, _) = log_stats(path);
    rec.after_bytes = after_bytes;
    rec.ok = true;
    rec.duration_ms = started.elapsed().as_millis() as u64;
    rec.at_epoch_ms = epoch_ms();
    rec
}

/// How long the online pass will wait for the dropped engine to quiesce before giving up
/// (and reopening WITHOUT compacting — never compact under an engine that might still write).
const RELEASE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Fast-path window for full fd release. When no index-builder leak exists (see
/// [`wait_for_quiesce`]) the engine releases in 74–240 ms (spike Q2); past this window we stop
/// expecting release and fall back to the stability check.
const RELEASE_FAST_PATH: std::time::Duration = std::time::Duration::from_secs(5);

/// The stability window for the quiesce fallback: every file in the store dir must keep an
/// unchanged (size, mtime) across this span before the pass may proceed.
const QUIESCE_WINDOW: std::time::Duration = std::time::Duration::from_millis(2000);

/// Run an **online** compaction pass: quiesce writes (the session mutex), swap the handle out,
/// compact the log on disk (shared with boot), reopen, swap back in. Concurrent store
/// operations block on the mutex for the duration — exactly as they would behind any long
/// transaction — and land after the swap; none are lost.
///
/// Whole-log I/O with no upper bound: callers MUST treat this as a job (`store.compact`),
/// never a tick. Errors leave the log valid; the handle is always restored (a fresh open) even
/// when the pass itself failed.
pub async fn compact(store: &Store) -> Result<CompactionRecord, StoreError> {
    let path = store
        .dir()
        .ok_or_else(|| StoreError::Backend("memory store has no commit log to compact".into()))?
        .to_string();

    // 1. Quiesce: hold the session mutex. No store operation can run while we hold it, and
    //    acquiring it means none is in flight (every verb holds it across its query).
    let cell = store.session_cell();
    let mut guard = cell.lock_owned().await;

    // 2. Swap the live handle out for an unconnected placeholder and drop it. The local engine
    //    shuts down asynchronously after the last clone drops (router drain + kvs.shutdown), so
    //    quiescence is DETECTED, never assumed (spike Q2: 74–240 ms observed).
    let old = std::mem::replace(&mut *guard, Surreal::init());
    drop(old);
    let released = wait_for_quiesce(&path, RELEASE_TIMEOUT).await;

    // 3. Compact on disk — only once the old engine provably quiesced. Compacting under an
    //    engine that can still write is the exact data-loss the spike disqualified (Q1);
    //    a timeout skips the pass.
    let rec = if released {
        let p = path.clone();
        tokio::task::spawn_blocking(move || compact_log(&p))
            .await
            .unwrap_or_else(|e| CompactionRecord {
                at_epoch_ms: epoch_ms(),
                ok: false,
                before_bytes: 0,
                after_bytes: 0,
                duration_ms: 0,
                error: Some(format!("compaction task join error: {e}")),
            })
    } else {
        CompactionRecord {
            at_epoch_ms: epoch_ms(),
            ok: false,
            before_bytes: 0,
            after_bytes: 0,
            duration_ms: 0,
            error: Some(format!(
                "engine did not quiesce at {path} within {RELEASE_TIMEOUT:?}; pass skipped"
            )),
        }
    };

    // 4. Reopen and swap back in — ALWAYS, even after a failed pass (the log is still valid;
    //    the node must keep serving). One retry for transient open failures.
    let reopened = match Surreal::new::<SurrealKv>(path.as_str()).await {
        Ok(db) => db,
        Err(first) => {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            Surreal::new::<SurrealKv>(path.as_str()).await.map_err(|e| {
                StoreError::Backend(format!(
                    "reopen after compaction failed twice ({first}; then {e}) — store handle is down"
                ))
            })?
        }
    };
    *guard = reopened;
    drop(guard);

    *store
        .last_compaction_slot()
        .lock()
        .expect("last_compaction poisoned") = Some(rec.clone());
    if rec.ok {
        Ok(rec)
    } else {
        Err(StoreError::Backend(
            rec.error.unwrap_or_else(|| "compaction failed".into()),
        ))
    }
}

/// Wait until the dropped engine has provably quiesced. Two acceptable end states:
///
/// 1. **Full release** (the fast path): this process holds no fd under `dir`
///    (`/proc/self/fd`). Observed 74–240 ms after the drop (spike Q2) — but it CANNOT be the
///    only gate: at surrealdb-core 2.6.5 a `DEFINE INDEX` spawns an index-builder task that
///    holds the transaction factory (the engine) **forever**, so any store that ever defined
///    an index (every real node — the jobs `(kind,status)` index) never reaches fd-zero.
///    Measured: still held 120 s after the last handle dropped.
/// 2. **Stability** (the leak fallback): every file under `dir` keeps an unchanged
///    (size, mtime) across [`QUIESCE_WINDOW`]. The leaked holder is inert by construction —
///    the router exited (last handle dropped), its background tickers were cancelled and
///    `kvs.shutdown()` completed before the router task ended, and no query can reach the old
///    engine (no handle points at it) — so once its shutdown writes stop moving the files,
///    nothing can ever write through it again. Stability across the window IS that proof.
///
/// Returns false only on `timeout` (the pass is skipped — never compact under an engine that
/// might still write). On platforms without `/proc`, the stability check alone decides.
async fn wait_for_quiesce(dir: &str, timeout: std::time::Duration) -> bool {
    let started = std::time::Instant::now();
    let has_proc = std::fs::read_dir("/proc/self/fd").is_ok();
    let mut last_snapshot: Option<(
        std::time::Instant,
        Vec<(std::path::PathBuf, u64, std::time::SystemTime)>,
    )> = None;
    loop {
        // Fast path: full fd release (only reachable when no index-builder leak exists).
        if has_proc && started.elapsed() < RELEASE_FAST_PATH {
            let open = std::fs::read_dir("/proc/self/fd")
                .map(|rd| {
                    rd.flatten()
                        .filter_map(|e| std::fs::read_link(e.path()).ok())
                        .any(|t| t.starts_with(dir))
                })
                .unwrap_or(false);
            if !open {
                return true;
            }
        } else {
            // Fallback: (size, mtime) stability across the window.
            let snap = dir_snapshot(std::path::Path::new(dir));
            match &last_snapshot {
                Some((at, prev)) if *prev == snap => {
                    if at.elapsed() >= QUIESCE_WINDOW {
                        return true;
                    }
                }
                _ => last_snapshot = Some((std::time::Instant::now(), snap)),
            }
        }
        if started.elapsed() > timeout {
            eprintln!(
                "store: compaction quiesce-wait timed out at {dir} — files still changing; pass skipped"
            );
            return false;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

/// Every file under `dir` (recursive) with its (size, mtime) — the stability probe's unit.
fn dir_snapshot(dir: &std::path::Path) -> Vec<(std::path::PathBuf, u64, std::time::SystemTime)> {
    let mut out = Vec::new();
    fn walk(d: &std::path::Path, out: &mut Vec<(std::path::PathBuf, u64, std::time::SystemTime)>) {
        if let Ok(rd) = std::fs::read_dir(d) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if let Ok(m) = e.metadata() {
                    out.push((p, m.len(), m.modified().unwrap_or(std::time::UNIX_EPOCH)));
                }
            }
        }
    }
    walk(dir, &mut out);
    out.sort();
    out
}

fn epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
