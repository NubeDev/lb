//! `write_if_rev`: an upsert that lands only while the record is still at the revision the caller
//! read. The check and the write run in ONE transaction, the same guard the undo journal's
//! conditional restore uses (`lb_undo` `restore.rs`), so two writers holding the same `rev` can never
//! both land: one commits; the other either fails the guard or aborts on SurrealDB's optimistic
//! conflict, is retried here, re-reads the bumped `rev` and fails the guard.
//!
//! `expected_rev` 0 ([`crate::Versioned::ABSENT_REV`]) means "must not exist yet", a create. A raw
//! store verb: the caller has already passed `caps::check`.

use serde_json::Value;

use crate::conflict::{conflict_backoff, is_retryable_conflict, MAX_CONFLICT_RETRIES};
use crate::open::{Store, StoreError};
use crate::record::FIRST_REV;
use crate::taint::mark_store_written;

/// The guard and the write, in one transaction. The live revision is the stored `rev`; a row written
/// before revisions existed has none and counts as `FIRST_REV` (what `read_versioned` reports for
/// it); an absent row is 0. A guard failure rolls the transaction back.
const GUARDED_UPSERT: &str = "BEGIN TRANSACTION;
    LET $live = IF type::record($tb, $id).data = NONE { 0 } ELSE { type::record($tb, $id).rev ?? $first };
    IF $live != $expected { THROW 'rev-mismatch' };
    UPSERT type::record($tb, $id) CONTENT { data: $data, rev: $expected + 1 } RETURN NONE;
    COMMIT TRANSACTION;";

/// The live revision alone, by the same rule as the guard.
const LIVE_REV: &str =
    "RETURN IF type::record($tb, $id).data = NONE { 0 } ELSE { type::record($tb, $id).rev ?? $first };";

/// The outcome of [`write_if_rev`]: written at the new `rev`, or refused because the record moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevWrite {
    Written { rev: u64 },
    Stale,
}

/// Upsert `value` at `table:id` in `ws` only if the stored `rev` equals `expected_rev`.
pub async fn write_if_rev(
    store: &Store,
    ws: &str,
    table: &str,
    id: &str,
    value: &Value,
    expected_rev: u64,
) -> Result<RevWrite, StoreError> {
    let binds = vec![
        ("tb".to_string(), Value::String(table.to_string())),
        ("id".to_string(), Value::String(id.to_string())),
        ("data".to_string(), value.clone()),
        ("expected".to_string(), Value::from(expected_rev)),
        ("first".to_string(), Value::from(FIRST_REV)),
    ];
    let mut attempt = 0;
    loop {
        match store.query_ws(ws, GUARDED_UPSERT, binds.clone()).await {
            Ok(_) => {
                mark_store_written();
                return Ok(RevWrite::Written {
                    rev: expected_rev + 1,
                });
            }
            Err(e) => {
                // A THROW does not reach us as text: the engine reports the lowest-indexed errored
                // statement, which is the generic "failed transaction" of a statement it cancelled.
                // So after any failure, re-read: if the record has moved off `expected_rev`, the
                // guard (or a racing writer that won) refused us. The transaction rolled back, so
                // nothing was written.
                let mut resp = store.query_ws(ws, LIVE_REV, binds.clone()).await?;
                let live: Option<u64> = resp
                    .take(0)
                    .map_err(|e| StoreError::Decode(e.to_string()))?;
                if live != Some(expected_rev) {
                    return Ok(RevWrite::Stale);
                }
                // Rev unchanged: a conflict abort rolled everything back, re-running is safe.
                if is_retryable_conflict(&e) && attempt < MAX_CONFLICT_RETRIES {
                    attempt += 1;
                    tokio::time::sleep(conflict_backoff(attempt)).await;
                } else {
                    return Err(e);
                }
            }
        }
    }
}
