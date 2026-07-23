//! `series.samples.delete` — bulk-remove **committed raw** samples of one series (the raw tail in
//! the `series` table). Rolled-up history is immutable through this verb: a sample already evicted
//! into `series_rollup` no longer exists as a raw row, so it simply matches nothing — rollups,
//! `series_meta`, retention policies, and tag edges are never touched here (that whole footprint is
//! `series.delete`'s job).
//!
//! Two selector modes (the caller picks exactly one — the host gate enforces that):
//!   - **keys**: explicit `(producer, seq)` identities, resolved to the composite record id
//!     `[series, producer, seq]` (`sample.rs::record_id`) — a key can never cross series.
//!   - **range**: an inclusive `[from_seq, to_seq]` over `seq`, across ALL producers, mirroring
//!     `read.rs`'s WHERE construction. An open bound OMITS the clause — never a `u64::MAX`
//!     sentinel (it coerces to a float and the comparison mis-evaluates;
//!     debugging/ingest/u64-max-bound-coerces-to-float.md).
//!
//! Both return the number of rows that actually existed and were removed. Authorization is NOT
//! here — raw verbs run after the host's `caps::check` (capability-first §3.5). Namespace-scoped:
//! every statement runs in `ws` (the hard wall).

use lb_store::{Store, StoreError};
use serde::Deserialize;
use serde_json::Value;

use crate::staging::SERIES_TABLE;

/// One committed sample's identity within a series — the `(producer, seq)` half of the composite
/// record id `[series, producer, seq]`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SampleKey {
    pub producer: String,
    pub seq: u64,
}

/// Delete the samples of `series` in `ws` named by `keys`. Returns how many actually existed (a
/// key naming a missing sample is a no-op, not an error). Empty `keys` deletes nothing.
pub async fn delete_samples_by_keys(
    store: &Store,
    ws: &str,
    series: &str,
    keys: &[SampleKey],
) -> Result<usize, StoreError> {
    if keys.is_empty() {
        return Ok(0);
    }
    // Bind every id part; assemble the composite things inline (same shape as commit.rs's UPSERT
    // ids). `$doomed` is the set that actually exists — the count we report (cap.rs's idiom).
    let mut things: Vec<String> = Vec::with_capacity(keys.len());
    let mut bindings: Vec<(String, Value)> =
        vec![("series".into(), Value::String(series.to_string()))];
    for (i, k) in keys.iter().enumerate() {
        let (pr, sq) = (format!("pr{i}"), format!("sq{i}"));
        things.push(format!(
            "type::thing('{SERIES_TABLE}', [$series, ${pr}, ${sq}])"
        ));
        bindings.push((pr, Value::String(k.producer.clone())));
        bindings.push((sq, Value::Number(k.seq.into())));
    }
    let sql = format!(
        "LET $ids = [{}];
         LET $doomed = (SELECT VALUE id FROM {SERIES_TABLE} WHERE id IN $ids);
         DELETE $doomed;
         RETURN count($doomed);",
        things.join(", ")
    );
    let mut resp = store.query_ws(ws, &sql, bindings).await?;
    // Statement 3 is the RETURN (the LETs and the DELETE produce no result set we read).
    let n: Option<i64> = resp
        .take(3)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(n.unwrap_or(0).max(0) as usize)
}

/// Delete every committed sample of `series` in `ws` whose `seq` is in the inclusive (optionally
/// half-open) range `[from_seq, to_seq]`, across ALL producers. Returns the number removed. The
/// caller (host gate) requires at least one bound — a both-open call would silently mean "delete
/// the whole raw tail", which must stay an explicit `series.delete`.
pub async fn delete_samples_in_range(
    store: &Store,
    ws: &str,
    series: &str,
    from_seq: Option<u64>,
    to_seq: Option<u64>,
) -> Result<usize, StoreError> {
    let mut clauses = String::from("series = $series");
    let mut bindings: Vec<(String, Value)> =
        vec![("series".into(), Value::String(series.to_string()))];
    if let Some(from) = from_seq {
        clauses.push_str(" AND seq >= $from");
        bindings.push(("from".into(), Value::Number(from.into())));
    }
    if let Some(to) = to_seq {
        clauses.push_str(" AND seq <= $to");
        bindings.push(("to".into(), Value::Number(to.into())));
    }
    // Count-then-delete in one query (gc.rs::evict_raw's idiom) so the reply reports what was
    // actually removed.
    let sql = format!(
        "SELECT count() FROM {SERIES_TABLE} WHERE {clauses} GROUP ALL;
         DELETE {SERIES_TABLE} WHERE {clauses};"
    );
    let mut resp = store.query_ws(ws, &sql, bindings).await?;
    let n: Option<i64> = resp
        .take("count")
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(n.unwrap_or(0).max(0) as usize)
}
