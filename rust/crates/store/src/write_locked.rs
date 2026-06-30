//! `write_locked` — the **conflict-safe** variant of [`write`](crate::write). Same upsert + monotonic
//! `rev` bump, but serialized per-`(ws,table,id)` with an in-process async lock and wrapped in a
//! bounded retry on SurrealDB's retryable transaction conflict.
//!
//! ## Why this exists (the run-store rev race)
//!
//! The bare [`write`] derives the new `rev` server-side (`(rev ?? 0) + 1`) so a *single* write is
//! atomic. But two writers targeting the **same** `table:id` under the durable engine
//! (`kv-surrealkv`) each open an optimistic transaction over the same prior snapshot; one commits
//! and the other aborts with `read or write conflict … can be retried`, or — worse, observed live on
//! the flows run-store — a later read deserializes a half-applied `rev` as `Invalid revision '…'`.
//! A frozen run id made every `flows.run` hammer the *same* `flow_run` / `flow_step:*` rows at once,
//! turning this latent race into a wall of errors (`debugging/flows/run-store-rev-conflict-…`).
//!
//! The fix mirrors [`capped_insert`](crate::capped_insert) exactly: an in-process per-key async lock
//! removes the *interleaving* (same-record writers run one at a time), and a bounded retry-on-conflict
//! absorbs a cross-key/reader conflict the lock can't see. This hardens the **primitive**, so every
//! same-record writer is safe — not just the flows caller (the scope's preferred placement).
//!
//! The lock is in-process: a node owns its own writes, and the records this guards (flow runs, steps)
//! are that node's durable state, resumed by the same node on boot — not cross-node-synced live state.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use serde_json::Value;
use tokio::sync::Mutex as AsyncMutex;

use crate::open::{Store, StoreError};
use crate::write::write;

/// Per-`(ws,table,id)` serialization for the rev-bumping write. Same shape as `capped::key_lock`,
/// scoped here to a single record id (the unit that races on `rev`). Different records never contend.
fn key_lock(ws: &str, table: &str, id: &str) -> Arc<AsyncMutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<String, Arc<AsyncMutex<()>>>>> = OnceLock::new();
    let map = LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let composite = format!("{ws}\u{1}{table}\u{1}{id}");
    let mut guard = map.lock().expect("write_locked key-lock map poisoned");
    guard
        .entry(composite)
        .or_insert_with(|| Arc::new(AsyncMutex::new(())))
        .clone()
}

/// How many times a conflicting rev-bump is retried before surfacing the error. High enough that a
/// realistic contention burst lands; low enough to never spin.
const MAX_CONFLICT_RETRIES: usize = 16;

/// True when a [`StoreError`] is SurrealDB's retryable optimistic-transaction conflict. Matched on the
/// message because SurrealDB exposes no typed variant through this surface (same matcher as `capped`).
fn is_retryable_conflict(e: &StoreError) -> bool {
    let m = e.to_string();
    m.contains("can be retried")
        || m.contains("read or write conflict")
        || m.contains("Invalid revision")
}

/// Conflict-safe upsert of `value` at `table:id` in workspace `ws`. Serializes same-record writers
/// and retries the retryable conflict. Identical observable result to [`write`] (same monotonic `rev`
/// bump, same taint); use this wherever a record can be written concurrently.
pub async fn write_locked(
    store: &Store,
    ws: &str,
    table: &str,
    id: &str,
    value: &Value,
) -> Result<(), StoreError> {
    // Serialize same-record writers: removes the interleaving that defeats the server-side rev bump
    // (see module docs). Other records proceed unblocked — this is not a global write lock.
    let lock = key_lock(ws, table, id);
    let _guard = lock.lock().await;

    let mut attempt = 0;
    loop {
        match write(store, ws, table, id, value).await {
            Ok(()) => return Ok(()),
            Err(e) if is_retryable_conflict(&e) && attempt < MAX_CONFLICT_RETRIES => {
                attempt += 1;
                // Escalating sub-millisecond backoff so a burst desynchronizes rather than livelocks
                // (a bare retry lets the same two re-collide on the next tick). Same shape as capped.
                let backoff = std::time::Duration::from_micros(50 * (1 << attempt.min(6)) as u64);
                tokio::time::sleep(backoff).await;
            }
            Err(e) => return Err(e),
        }
    }
}
