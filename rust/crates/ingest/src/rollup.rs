//! The stored rollup tiers — retention GC's downsampled copy of raw history it is about to evict
//! (series-retention scope). One row per `(series, width_ms, bucket_t)`, carrying `sum` + `count`
//! alongside min/max/last so a later re-aggregation into a wider read bucket is exact, not a
//! mean-of-means. Rows live in SurrealDB like everything else (one datastore); the read side merges
//! them under `series.read {mode:"buckets"}` for windows raw no longer covers.
//!
//! NOT a read-time cache: decimated reads over live raw data never consult this table. Rollup rows
//! exist only where retention has (or is about to have) evicted the raw samples beneath them.

use lb_store::{Store, StoreError};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::schema::ROLLUP_TABLE;

/// Deserialize a float that the datastore may have narrowed to an INTEGER.
///
/// `sum`/`min`/`max` are `f64` in Rust, but SurrealDB stores a whole-numbered float as an `i64` —
/// a meter reading of exactly `6432914451` round-trips as an integer, not a float. Plain
/// `#[derive(Deserialize)]` then fails the whole row with "expected a 64-bit floating point,
/// found 6432914451i64", which takes down the entire GC pass that reads it.
///
/// That failure is silent and self-perpetuating: the FIRST pass writes rollups fine, and every
/// later pass dies reading them back, so retention GC stops evicting and the store grows without
/// bound — the precise failure the retention feature exists to prevent. Observed on RC-6 with 30
/// modbus meters (2026-08-03): one successful pass, then `last_run_ms` frozen forever while the
/// store climbed ~3.6 MB/min.
///
/// Accepting both numeric shapes on the way IN is the honest fix — the writer cannot control which
/// one the datastore picks, so the reader must tolerate either.
fn de_lenient_f64<'de, D>(d: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // `serde_json::Value` accepts any JSON number; `as_f64` widens an integer losslessly here
    // (rollup sums are far inside f64's exact-integer range).
    let v = Value::deserialize(d)?;
    v.as_f64()
        .ok_or_else(|| serde::de::Error::custom(format!("expected a number, found {v}")))
}

/// The `Option` twin of [`de_lenient_f64`] — `min`/`max` are absent on a non-numeric bucket.
fn de_opt_lenient_f64<'de, D>(d: D) -> Result<Option<f64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match Option::<Value>::deserialize(d)? {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_f64()
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom(format!("expected a number, found {v}"))),
    }
}

/// One stored rollup bucket. `t` is the bucket start (epoch ms, aligned to `width_ms`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RollupRow {
    pub series: String,
    pub width_ms: u64,
    pub t: u64,
    #[serde(default, deserialize_with = "de_opt_lenient_f64")]
    pub min: Option<f64>,
    #[serde(default, deserialize_with = "de_opt_lenient_f64")]
    pub max: Option<f64>,
    /// Sum + count of numeric payloads — exact re-aggregation, never a mean-of-means.
    #[serde(deserialize_with = "de_lenient_f64")]
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
    /// `default` covers a row written before the column existed (the field is absent). The
    /// `deserialize_with` covers a row written WITH the column as `None`: SurrealDB 3 stores that
    /// as NULL and hands serde a **unit**, which `Option<u64>` rejects outright — "invalid type:
    /// unit value, expected u64". Both shapes are on disc, so both have to read.
    #[serde(default, deserialize_with = "lb_store::null_as_none")]
    pub first_ts: Option<u64>,
}

/// Upsert rollup rows at their deterministic id `[series, width_ms, t]` — a re-run GC pass over the
/// same raw data lands identical rows (idempotent).
pub async fn write_rollups(store: &Store, ws: &str, rows: &[RollupRow]) -> Result<(), StoreError> {
    for r in rows {
        // Retry-on-conflict: a GC pass writing rollups races producers committing to the
        // series tables in the same namespace under SurrealDB's optimistic MVCC. The UPSERT id is
        // deterministic (`[series, width_ms, t]`), so a retried write lands the identical row —
        // idempotent, no double-count.
        store
            .query_ws_retrying(
                ws,
                &format!(
                    "UPSERT type::record('{ROLLUP_TABLE}', [$series, $width, $t]) CONTENT $row"
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

/// The distinct bucket widths that actually EXIST under `prefix`, ascending.
///
/// Needed because a policy's declared tiers and the widths on disc can disagree: editing a policy
/// (5-minute buckets -> 1-minute buckets) leaves the old width's rows behind, and nothing writes to
/// that width again. Eviction is keyed by an exact `width_ms`, so those rows match no declared tier
/// and would otherwise be retained FOREVER — unbounded growth in the feature whose job is bounding
/// growth. `run_gc` diffs this against the policy to find them.
///
/// `GROUP BY` over the projected column with the ordering done in Rust — the store's aggregate rules
/// allow grouping a projected column but ordering by it in-engine is the fragile half (same shape as
/// `stats::producers`).
pub async fn rollup_widths(store: &Store, ws: &str, prefix: &str) -> Result<Vec<u64>, StoreError> {
    let mut resp = store
        .query_ws_retrying(
            ws,
            &format!(
                "SELECT width_ms FROM {ROLLUP_TABLE} \
                 WHERE string::starts_with(series, $prefix) GROUP BY width_ms"
            ),
            vec![("prefix".into(), Value::String(prefix.to_string()))],
        )
        .await?;
    let mut widths: Vec<u64> = resp
        .take("width_ms")
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    widths.sort_unstable();
    widths.dedup();
    Ok(widths)
}

/// Evict a tier's rows older than `before_ts` for every series matching `prefix`. Returns evicted count.
pub async fn evict_rollups(
    store: &Store,
    ws: &str,
    prefix: &str,
    width_ms: u64,
    before_ts: u64,
) -> Result<usize, StoreError> {
    // Retry-on-conflict: this eviction DELETE over the rollup table races concurrent GC/commit writes
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The row shape `read_rollups` projects, with `sum`/`min`/`max` as the datastore returns them.
    fn row_json(sum: Value, min: Value, max: Value) -> Value {
        json!({
            "series": "modbus.loadtest.meter-1.active-power-total",
            "width_ms": 300_000u64,
            "t": 1_785_729_000_000u64,
            "min": min,
            "max": max,
            "sum": sum,
            "num_count": 3u64,
            "count": 3u64,
            "last": 42.0,
            "last_ts": 1_785_729_200_000u64,
        })
    }

    /// The RC-6 regression: SurrealDB narrows a whole-numbered float to an integer, and the derived
    /// impl rejected it — killing every GC pass that read the row back, so retention silently
    /// stopped evicting. An integer `sum` must deserialize.
    #[test]
    fn integer_sum_deserializes() {
        let r: RollupRow =
            serde_json::from_value(row_json(json!(6_432_914_451i64), json!(1i64), json!(9i64)))
                .expect("an integer sum/min/max must read back");
        assert_eq!(r.sum, 6_432_914_451.0);
        assert_eq!(r.min, Some(1.0));
        assert_eq!(r.max, Some(9.0));
    }

    /// The ordinary float path must keep working unchanged.
    #[test]
    fn float_sum_still_deserializes() {
        let r: RollupRow =
            serde_json::from_value(row_json(json!(1.5), json!(0.25), json!(2.75))).unwrap();
        assert_eq!(r.sum, 1.5);
        assert_eq!(r.min, Some(0.25));
        assert_eq!(r.max, Some(2.75));
    }

    /// A non-numeric bucket carries no min/max; null must stay `None` rather than error.
    #[test]
    fn null_min_max_is_none() {
        let r: RollupRow =
            serde_json::from_value(row_json(json!(0i64), Value::Null, Value::Null)).unwrap();
        assert_eq!(r.min, None);
        assert_eq!(r.max, None);
    }

    /// A genuinely wrong type is still an error — leniency is about NUMERIC SHAPE, not anything-goes.
    #[test]
    fn non_numeric_sum_still_errors() {
        let e = serde_json::from_value::<RollupRow>(row_json(
            json!("not-a-number"),
            Value::Null,
            Value::Null,
        ));
        assert!(e.is_err(), "a string sum must not silently become a number");
    }
}

// SurrealDB 3 reads query rows through `SurrealValue`. These delegate to serde rather than
// deriving, so `#[serde(default)]` and `deserialize_with = "de_opt_lenient_f64"` keep working
// unchanged — the derive supports neither. See `lb_store::surreal_value_via_serde!`.
lb_store::surreal_value_via_serde!(RollupRow);
