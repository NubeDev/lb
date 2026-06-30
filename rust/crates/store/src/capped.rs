//! `capped_insert` — the reusable **bounded-retention** primitive. Inserts one row into a table and,
//! when the count for its FIFO key exceeds `cap`, deletes the oldest rows for that key down to `cap`
//! — all in **one SurrealDB transaction** (the `write_tx` family). One verb file beside
//! `scan.rs`/`write_tx.rs`/`tables.rs`.
//!
//! ## Why one transaction (the load-bearing correctness invariant)
//!
//! A naïve "count then delete oldest" races: two concurrent inserts both see `count == cap` and each
//! delete one, **over-evicting**. A background reaper-only design overshoots the cap in a burst
//! (unbounded between sweeps — the exact failure a bounded ring exists to prevent). The correct
//! implementation does insert + trim as **one transaction**: SurrealDB serializes the two
//! `BEGIN…COMMIT` blocks on the same key, so the final count is exactly `cap`, never over-evicted and
//! never overgrown. The concurrency test ([`tests::capped_concurrency_test`]) is what proves we did
//! not ship the racy version.
//!
//! ## FIFO ordering — ULID, not wall-clock
//!
//! The record id is a **ULID** (monotonic-ish, lexicographically sortable, no clock dependency and no
//! counter row — the scope's resolved open question). Trim orders by `id ASC` and keeps the newest
//! `cap`; the ULID gives a total, insertion-correlated order without `Date::now` (banned in some
//! paths) and without a per-key sequence counter. Two same-millisecond concurrent inserts may sort in
//! either order, but the trim still leaves exactly `cap` survivors — the invariant is the *count* and
//! the *oldest-evicted* property, not a wall-clock-exact order.
//!
//! ## The key selector is the caller's choice (per-source OR global, same helper)
//!
//! `cap_key` is whatever bucket the caller wants capped independently. Per-source retention passes
//! `cap_key = source` (a chatty source can't evict a quiet one); a global per-workspace backstop
//! passes `cap_key = ws`. Both come from the **same** `capped_insert` with a different selector —
//! proving the "configurable both" requirement. Defaults live in the caller (prefs/config), never
//! baked into this primitive — it is generic, reused by `series`/`run-events`/any future bounded ring.
//!
//! `Secret<T>` redaction is **not** this primitive's concern: the `value` it stores is already the
//! redacted event schema by the time any caller reaches here. This is a store verb run *after*
//! `caps::check`; it is not an authorization point.
//!
//! ## Considered and rejected
//!
//! - **SurrealDB native TTL / `DEFINE TABLE … DROP`** — TTL is *age*-based, not *count*-based; it
//!   cannot express "newest 1000" (a quiet key ages out at 3 rows; a chatty key blows past 1000
//!   inside the window). We need count-bounded FIFO, so we own the primitive.
//! - **Reaper job as the primary mechanism** — overshoots the cap in a burst. At most an optional
//!   secondary safety net; the transactional trim is the guarantee.
//! - **Count-then-delete in two statements without a transaction** — races into over-eviction.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use serde_json::Value;
use tokio::sync::Mutex as AsyncMutex;

use crate::open::{Store, StoreError};
use crate::taint::mark_store_written;

/// Per-key serialization for the insert+trim transaction.
///
/// SurrealDB's `kv-mem` engine uses optimistic, snapshot-isolated transactions: two concurrent
/// `capped_insert`s on the *same* key can each compute `$keep` from a snapshot that does **not** yet
/// see the sibling's just-inserted row, so each trim under-deletes and the ring overgrows the cap by
/// the number of overlapping in-flight inserts (observed as a flaky `count == cap + k`). The engine
/// does not reliably raise a write-conflict for this read-set/write-set shape, so a retry loop alone
/// cannot fix it.
///
/// The transaction is still the atomic unit (insert + trim commit together or not at all); this lock
/// removes the *interleaving* that defeats snapshot isolation, by serializing the at-most-millisecond
/// transaction for one `(ns, table, cap_key)` bucket. Inserts to **different** keys never contend
/// (a chatty source can't block a quiet one — the same property the per-source cap gives), so this is
/// not a global write lock. It is an in-process guard: a single node owns its ring, and a capped table
/// is recent operational data, not cross-node synced state (telemetry-console-scope, "each node's ring
/// is independent"). The concurrency test is what proves the invariant now holds deterministically.
fn key_lock(ns: &str, table: &str, cap_key: &str) -> Arc<AsyncMutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<String, Arc<AsyncMutex<()>>>>> = OnceLock::new();
    let map = LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let composite = format!("{ns}\u{1}{table}\u{1}{cap_key}");
    let mut guard = map.lock().expect("capped key-lock map poisoned");
    guard
        .entry(composite)
        .or_insert_with(|| Arc::new(AsyncMutex::new(())))
        .clone()
}

/// Insert `value` at `<table>:<id>` in workspace `ws`, then trim the table to the newest `cap` rows
/// whose `cap_key` equals `key`, in **one SurrealDB transaction**. The `cap_key` field is injected
/// into the stored record (the caller passes the body *without* it), so the trim's `WHERE cap_key`
/// always matches the rows this call governs. `cap == 0` is clamped to 1 (a zero cap would delete
/// every row including the one just written — never useful and surprising).
///
/// `id` should be a ULID (see [`new_ulid`]) so the `ORDER BY id` trim is a true FIFO order; any
/// lexicographically-sortable, monotonic string works.
pub async fn capped_insert(
    store: &Store,
    ws: &str,
    table: &str,
    id: &str,
    key: &str,
    cap: usize,
    value: &Value,
) -> Result<(), StoreError> {
    let n = cap.max(1);

    // Serialize the insert+trim for this bucket so concurrent inserts can't each trim against a
    // snapshot that misses the other's new row (see `key_lock`). Held across the transaction; other
    // keys proceed unblocked.
    let lock = key_lock(ws, table, key);
    let _guard = lock.lock().await;

    // Inject `cap_key` (the FIFO bucket this row belongs to) and `seq` (the monotonic insert-seq —
    // the same ULID as the record id) into the stored body. The trim orders by `seq`, NOT by `id`:
    // SurrealDB requires an `ORDER BY` idiom to be literally selected, and `seq` is a plain field
    // (a string), which orders cleanly, can be indexed (telemetry scope names an insert-seq index),
    // and needs no special handling. The body author passes neither field.
    let mut obj = value.as_object().cloned().unwrap_or_default();
    obj.insert("cap_key".into(), Value::String(key.to_string()));
    obj.insert("seq".into(), Value::String(id.to_string()));
    let merged = Value::Object(obj);

    // One transaction: create the row, then delete every row for this key that is NOT among the
    // newest `cap` (ordered by insert-seq descending). The subquery sees the just-inserted row, so a
    // flood past the cap trims back to exactly `cap` on the same COMMIT.
    //
    // We compute `$keep` into a LET variable and `DELETE ... NOT IN $keep`, NOT an inline
    // `DELETE ... NOT IN (subquery)`: SurrealDB mis-evaluates the inline `NOT IN (SELECT ...)` form
    // inside a DELETE (it drops every row, not just the complement — verified in the capped spike).
    // The LET-bound array compares correctly. `LIMIT {n}` is a formatted integer literal (the only
    // cross-version-safe shape SurrealDB accepts for LIMIT); `n` is a caller constant, not runtime.
    let sql = format!(
        "BEGIN TRANSACTION;\
         CREATE type::thing($tb, $id) CONTENT $value;\
         LET $keep = (SELECT VALUE seq FROM type::table($tb) WHERE cap_key = $key ORDER BY seq DESC LIMIT {n});\
         DELETE FROM type::table($tb) WHERE cap_key = $key AND seq NOT IN $keep;\
         COMMIT TRANSACTION;"
    );
    // Retry on a SurrealDB **retryable transaction conflict**. The per-key lock above prevents two
    // same-key inserts from racing, but the insert+trim transaction can still conflict with a
    // concurrent reader or a cross-key writer in the same namespace (SurrealDB `kv-mem` uses
    // optimistic MVCC and aborts one side with "…failed transaction…can be retried"). Without a retry
    // the Layer would silently DROP that telemetry row (fire-and-forget swallows the error) — an
    // under-count, exactly the flake the concurrency tests caught. A small bounded retry makes the
    // write land; the cap invariant still holds because each attempt is the same single transaction.
    let binds = || {
        vec![
            ("tb".into(), Value::String(table.to_string())),
            ("id".into(), Value::String(id.to_string())),
            ("value".into(), merged.clone()),
            ("key".into(), Value::String(key.to_string())),
        ]
    };
    let mut attempt = 0;
    loop {
        match store.query_ws(ws, &sql, binds()).await {
            Ok(_) => break,
            Err(e) if is_retryable_conflict(&e) && attempt < MAX_CONFLICT_RETRIES => {
                attempt += 1;
                // Escalating backoff so a burst of conflicting transactions DESYNCHRONIZES rather than
                // livelocking (a bare yield lets the same two re-collide on the next tick). The sleep
                // is sub-millisecond at first and stays small; a capped write is off the hot path.
                let backoff = std::time::Duration::from_micros(50 * (1 << attempt.min(6)) as u64);
                tokio::time::sleep(backoff).await;
            }
            Err(e) => return Err(e),
        }
    }
    // A capped write mutates the store (no-op outside a dispatch taint scope).
    mark_store_written();
    Ok(())
}

/// How many times a conflicting insert+trim is retried before giving up. A telemetry write is
/// fire-and-forget, so a give-up drops one sampled row (acceptable); this bound is high enough that a
/// realistic contention burst lands and low enough to never spin.
const MAX_CONFLICT_RETRIES: usize = 16;

/// True when a [`StoreError`] is SurrealDB's retryable optimistic-transaction conflict (the engine
/// explicitly says "This transaction can be retried"). Matched on the message because SurrealDB does
/// not expose a typed variant for it through this surface.
fn is_retryable_conflict(e: &StoreError) -> bool {
    let m = e.to_string();
    m.contains("can be retried") || m.contains("read or write conflict")
}

/// Mint a fresh ULID string — the recommended record id + FIFO ordering key for a capped table.
/// Monotonic-ish and lexicographically sortable with no clock and no counter row (the resolved open
/// question). Kept here so every capped caller mints ids the same way.
pub fn new_ulid() -> String {
    ulid::Ulid::new().to_string()
}
