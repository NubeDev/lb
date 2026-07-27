//! `ws_drain_lock` — the per-workspace serializer that stops two drain passes running an overlapping
//! commit over the **same staging head** (WS-B of the ingest-conflict-storm scope).
//!
//! ## Why (drain-vs-drain, the *continuous* collision)
//!
//! A caller's inline bounded drain ([`drain_workspace_bounded`](super::drain_workspace_bounded)) and
//! the background reactor's unbounded drain ([`drain_workspace`](super::drain_workspace)) both start
//! by `SELECT`ing the oldest-256 staged rows (`ingest::commit`'s `drain`, `ORDER BY seq,ts`) and then
//! committing a transaction that deletes exactly those staging ids and upserts the shared
//! `series`/`series_latest` rows. With ≥2 producers pushing every couple of seconds, two drains grab
//! the *same* head-of-queue rows and one always loses the commit race — the storm the operator saw.
//!
//! Serializing each commit **batch per workspace** removes that overlap at the source: `commit_batch`
//! reads its head and commits in two separate round-trips, so holding this lock across one
//! `commit_batch` makes that read+commit atomic w.r.t. other drains — only one drain touches a
//! workspace's staging head at a time, so the racing pair never forms. This is complementary to
//! WS-A, not a replacement — WS-A's bounded retry still absorbs the *periodic* drain-vs-GC collision
//! (the GC pass deletes raw from `series` outside this lock). Do both.
//!
//! The lock is held per BATCH, not across the whole drain pass, on purpose: the reactor drains
//! unbounded (O(backlog)), and holding it across that would make a concurrent inline caller wait for
//! the entire backlog — re-coupling caller latency to backlog, the regression the drain-backpressure
//! fix removed (`debugging/ingest/write-drains-whole-workspace-backlog.md`). Per-batch, the reactor
//! releases between batches so an inline caller waits at most one batch. This does **not** move
//! commits off the caller's path (the inline bounded drain is the deliberate write-then-read
//! round-trip, `drain.rs` header). Different workspaces never contend.
//!
//! ## Shape
//!
//! A process-wide `static` keyed-by-`ws` async lock — the same idiom `store::write_locked`/`increment`
//! use for their per-record locks (a node owns its own drains; staging is that node's durable state,
//! not cross-node-synced live state). Kept in one small file per FILE-LAYOUT.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use tokio::sync::Mutex as AsyncMutex;

/// The async lock guarding `ws`'s drain commit pass. One `Arc<Mutex<()>>` per workspace, minted on
/// first use; different workspaces get different locks and never block each other.
pub(super) fn ws_drain_lock(ws: &str) -> Arc<AsyncMutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<String, Arc<AsyncMutex<()>>>>> = OnceLock::new();
    let map = LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = map.lock().expect("ingest drain-lock map poisoned");
    guard
        .entry(ws.to_string())
        .or_insert_with(|| Arc::new(AsyncMutex::new(())))
        .clone()
}
