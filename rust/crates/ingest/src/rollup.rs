//! The stored rollup tiers — retention GC's downsampled copy of raw history it is about to evict
//! (series-retention scope). One row per `(series, width_ms, bucket_t)`, carrying `sum` + `count`
//! alongside min/max/last so a later re-aggregation into a wider read bucket is exact, not a
//! mean-of-means. Rows live in SurrealDB like everything else (one datastore); the read side merges
//! them under `series.read {mode:"buckets"}` for windows raw no longer covers.
//!
//! NOT a read-time cache: decimated reads over live raw data never consult this table. Rollup rows
//! exist only where retention has (or is about to have) evicted the raw samples beneath them.

use lb_store::{Store, StoreError};
use serde_json::{json, Value};

use crate::schema::ROLLUP_TABLE;

/// One stored rollup bucket. `t` is the bucket start (epoch ms, aligned to `width_ms`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RollupRow {
    pub series: String,
    pub width_ms: u64,
    pub t: u64,
    pub min: Option<f64>,
    pub max: Option<f64>,
    /// Sum + count of numeric payloads — exact re-aggregation, never a mean-of-means.
    pub sum: f64,
    pub num_count: u64,
    /// Total samples in the bucket (numeric or not).
    pub count: u64,
    pub last: Value,
    pub last_ts: u64,
    /// The bucket's chronologically FIRST payload — the kept representative the `first` and
    /// `nearest` tier methods read (series-normalize scope).
    ///
    /// `first_ts` is `Option` on purpose: it is the PROVENANCE flag. A row folded before this slice
    /// has none, and a bucket built from such a row must refuse `first`/`nearest` with a clear
    /// `BadInput` rather than approximate. Defaults keep every pre-existing row readable.
    #[serde(default)]
    pub first: Value,
    #[serde(default)]
    pub first_ts: Option<u64>,
}

/// Upsert rollup rows at their deterministic id `[series, width_ms, t]` — a re-run GC pass over the
/// same raw data lands identical rows (idempotent).
pub async fn write_rollups(store: &Store, ws: &str, rows: &[RollupRow]) -> Result<(), StoreError> {
    for r in rows {
        // Retry-on-conflict: a GC pass writing rollups races the inline drains committing to the
        // series tables in the same namespace under SurrealDB's optimistic MVCC. The UPSERT id is
        // deterministic (`[series, width_ms, t]`), so a retried write lands the identical row —
        // idempotent, no double-count.
        store
            .query_ws_retrying(
                ws,
                &format!(
                    "UPSERT type::thing('{ROLLUP_TABLE}', [$series, $width, $t]) CONTENT $row"
                ),
                vec![
                    ("series".into(), Value::String(r.series.clone())),
                    ("width".into(), Value::Number(r.width_ms.into())),
                    ("t".into(), Value::Number(r.t.into())),
                    ("row".into(), json!(r)),
                ],
            )
            .await?;
    }
    Ok(())
}

/// All rollup rows of `series` (any tier) whose bucket start falls in `[from_ts, to_ts)`.
pub async fn read_rollups(
    store: &Store,
    ws: &str,
    series: &str,
    from_ts: u64,
    to_ts: u64,
) -> Result<Vec<RollupRow>, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            &format!(
                // Every column is projected explicitly — one added to `RollupRow` but missing here
                // reads back as its serde default forever (the closed-struct trap; the same note
                // guards `list_policies`).
                "SELECT series, width_ms, t, min, max, sum, num_count, count, last, last_ts, \
                 first, first_ts \
                 FROM {ROLLUP_TABLE} WHERE series = $series AND t >= $from AND t < $to \
                 ORDER BY t ASC"
            ),
            vec![
                ("series".into(), Value::String(series.to_string())),
                ("from".into(), Value::Number(from_ts.into())),
                ("to".into(), Value::Number(to_ts.into())),
            ],
        )
        .await?;
    resp.take(0).map_err(|e| StoreError::Decode(e.to_string()))
}

/// Evict a tier's rows older than `before_ts` for every series matching `prefix`. Returns evicted count.
pub async fn evict_rollups(
    store: &Store,
    ws: &str,
    prefix: &str,
    width_ms: u64,
    before_ts: u64,
) -> Result<usize, StoreError> {
    // Retry-on-conflict: this eviction DELETE over the rollup table races concurrent GC/drain writes
    // in the same namespace under SurrealDB's optimistic MVCC. The count+delete is a single idempotent
    // pass, so a retried run evicts the same rows exactly once.
    let mut resp = store
        .query_ws_retrying(
            ws,
            &format!(
                "SELECT count() FROM {ROLLUP_TABLE} WHERE string::starts_with(series, $prefix) \
                 AND width_ms = $width AND t < $before GROUP ALL;
                 DELETE {ROLLUP_TABLE} WHERE string::starts_with(series, $prefix) \
                 AND width_ms = $width AND t < $before;"
            ),
            vec![
                ("prefix".into(), Value::String(prefix.to_string())),
                ("width".into(), Value::Number(width_ms.into())),
                ("before".into(), Value::Number(before_ts.into())),
            ],
        )
        .await?;
    let n: Option<i64> = resp
        .take("count")
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(n.unwrap_or(0).max(0) as usize)
}
