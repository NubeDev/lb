//! `increment` — an **atomic server-side numeric accumulate** on a record's `data.count` field.
//!
//! This is the durable primitive behind a stateful counter node (the Node-RED / PLC "rung holds its
//! last result"): a flow counter must read its prior total and add to it, and two firings that race
//! must NOT lose an update. Doing the add in the host (read → +1 → write) reintroduces exactly the
//! read-modify-write race [`write_locked`](crate::write_locked) exists to kill. Instead we add
//! **inside the UPSERT**, server-side, in one statement — the same trick [`write`](crate::write) uses
//! for the monotonic `rev` — so the accumulate is atomic without any lock: the new total is
//! `(prior.count ?? 0) + by` (or `by` alone when `reset`), computed against the record's own prior
//! value as the statement commits.
//!
//! Returns the new running total. Workspace-walled like every store verb (the namespace is selected
//! from `ws`, so a counter in workspace A is invisible to workspace B even at the same `table:id`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use tokio::sync::Mutex as AsyncMutex;

use crate::conflict::{conflict_backoff, is_retryable_conflict, MAX_CONFLICT_RETRIES};
use crate::open::{Store, StoreError};
use crate::record::FIRST_REV;
use crate::taint::mark_store_written;

/// Per-`(ws,table,id)` serialization for the accumulate — the SAME discipline as `write_locked`. An
/// atomic UPSERT is enough to never *lose* an update, but a `+1` is NOT idempotent on retry: if a
/// retryable conflict made us re-run the statement after it had partially applied, the count would
/// double. Serializing same-record increments in-process (a node owns its own counter writes) means a
/// retry only ever follows a genuine abort (not-committed), so the accumulate stays exactly-once.
fn key_lock(ws: &str, table: &str, id: &str) -> Arc<AsyncMutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<String, Arc<AsyncMutex<()>>>>> = OnceLock::new();
    let map = LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let composite = format!("{ws}\u{1}{table}\u{1}{id}");
    let mut guard = map.lock().expect("increment key-lock map poisoned");
    guard
        .entry(composite)
        .or_insert_with(|| Arc::new(AsyncMutex::new(())))
        .clone()
}

/// Atomically add `by` to `table:id`'s `data.count` in workspace `ws`, returning the new total. When
/// `reset` is true the prior value is discarded first (the total becomes `by`). Also stamps `data.ts`
/// and bumps the record's monotonic `rev` (so a watcher sees the value moved). `by` may be negative.
pub async fn increment(
    store: &Store,
    ws: &str,
    table: &str,
    id: &str,
    by: i64,
    reset: bool,
    ts: u64,
) -> Result<i64, StoreError> {
    // Serialize same-record increments (see `key_lock`): removes the interleaving that would make a
    // retry double-add. Different records never contend.
    let lock = key_lock(ws, table, id);
    let _guard = lock.lock().await;

    let mut attempt = 0;
    loop {
        // The accumulate is server-side: `count` is derived from the record's OWN prior `data.count`
        // inside the same UPSERT, so two concurrent firings each commit `prior + by` against their own
        // snapshot and the loser retries — never a lost update (the host never reads-then-writes).
        // `RETURN VALUE data.count` projects just the post-write scalar — never the record's `id`
        // (a RecordId that can't deserialize to a plain JSON value), and it is THIS statement's
        // committed total (not a re-read a concurrent firing could have moved).
        let res = store
            .query_ws(
                ws,
                "UPSERT type::record($tb, $id) CONTENT { \
                    data: { \
                        count: (IF $reset THEN 0 ELSE (type::record($tb, $id).data.count ?? 0) END) + $by, \
                        ts: $ts \
                    }, \
                    rev: (type::record($tb, $id).rev ?? ($first - 1)) + 1 \
                 } RETURN VALUE data.count",
                vec![
                    ("tb".into(), serde_json::Value::String(table.to_string())),
                    ("id".into(), serde_json::Value::String(id.to_string())),
                    ("by".into(), serde_json::Value::from(by)),
                    ("reset".into(), serde_json::Value::Bool(reset)),
                    ("ts".into(), serde_json::Value::from(ts)),
                    ("first".into(), serde_json::Value::from(FIRST_REV)),
                ],
            )
            .await;

        let outcome = match res {
            Ok(mut checked) => {
                let row: Option<i64> = checked
                    .take(0)
                    .map_err(|e| StoreError::Decode(e.to_string()))?;
                Ok(row.unwrap_or(by))
            }
            Err(e) => Err(e),
        };

        match outcome {
            Ok(total) => {
                mark_store_written();
                return Ok(total);
            }
            Err(e) if is_retryable_conflict(&e) && attempt < MAX_CONFLICT_RETRIES => {
                attempt += 1;
                tokio::time::sleep(conflict_backoff(attempt)).await;
            }
            Err(e) => return Err(e),
        }
    }
}
