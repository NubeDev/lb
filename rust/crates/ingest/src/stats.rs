//! `series.stats(series)` — what one series actually holds (series-observability scope).
//!
//! Raw counts, rolled-up counts, the wall-clock extent, and the set of producers writing to it.
//! This is the read-back that makes retention legible: without it an operator can see samples but
//! not whether the window is small because retention is aggressive, because polling stopped, or
//! because a capability was refused.
//!
//! **SINGLE-SUBJECT BY DESIGN — do not add an array or all-series mode.** Every query here is a
//! `count()`/`LIMIT 1` narrowed by `series =`, which is cheap for ONE series and ruinous when a
//! caller fans it out. A `count()` per series behind the store is precisely the shape that produced
//! the node-wide serialization stall fixed in `node-v0.11.0` (a 10k-series workspace would issue
//! 40k queries for one page render). A well-meaning UI wanting per-row counts in the series library
//! is the exact caller this signature exists to refuse: that need is a separate BATCHED verb with
//! its own perf scope and its own bound on subject count, never a loop over this one.
//!
//! **Raw vs rolled-up needs no marker and no policy lookup** — they are two different tables
//! (`series` and `series_rollup`), so the split is structural. Rollup rows exist once per
//! `(series, width_ms, t)`, i.e. once per TIER, so a total row count over a multi-tier policy would
//! silently double-count the same history at two resolutions. `tiers` therefore reports the
//! per-width breakdown and `rollup_rows` is the honest total of stored rows (not of folded
//! samples), which is what an operator comparing against `raw_count` needs to read a sawtooth.

use lb_store::{Store, StoreError};
use serde_json::Value;

use crate::schema::ROLLUP_TABLE;
use crate::staging::SERIES_TABLE;

/// One rollup tier's stored-row count for a series.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TierRows {
    pub width_ms: u64,
    pub rows: u64,
}

/// What a series holds right now. Every field is a measurement, never a default: a series with no
/// samples returns zeroes and `None` extents — a VALID result, never an error (a series that simply
/// has no data yet must not render as a failure).
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct SeriesStats {
    pub series: String,
    /// Rows in the raw `series` table.
    pub raw_count: u64,
    /// Stored rows in `series_rollup` across every tier (see the module note on double-counting).
    pub rollup_rows: u64,
    /// Per-tier breakdown of `rollup_rows`, ascending by bucket width.
    pub tiers: Vec<TierRows>,
    /// Wall-clock extent of the RAW rows, epoch ms. `None` when the series holds no raw samples —
    /// the caller renders "unknown", never `0` (which would read as 1970).
    pub first_ts: Option<u64>,
    pub last_ts: Option<u64>,
    /// Every producer with at least one raw row in this series, ascending. Empty is honest: it
    /// means no raw samples, not "no producer".
    pub producers: Vec<String>,
}

/// Collect [`SeriesStats`] for exactly one series in `ws`.
///
/// See the module doc for why this takes one series and not a list.
pub async fn series_stats(
    store: &Store,
    ws: &str,
    series: &str,
) -> Result<SeriesStats, StoreError> {
    let bind = || vec![("series".into(), Value::String(series.to_string()))];

    let mut resp = store
        .query_ws(
            ws,
            &format!("SELECT count() FROM {SERIES_TABLE} WHERE series = $series GROUP ALL"),
            bind(),
        )
        .await?;
    let raw: Option<i64> = resp
        .take("count")
        .map_err(|e| StoreError::Decode(e.to_string()))?;

    let (first_ts, last_ts) = raw_extent(store, ws, series).await?;
    let (rollup_rows, tiers) = rollup_tiers(store, ws, series).await?;

    Ok(SeriesStats {
        series: series.to_string(),
        raw_count: raw.unwrap_or(0).max(0) as u64,
        rollup_rows,
        tiers,
        first_ts,
        last_ts,
        producers: producers(store, ws, series).await?,
    })
}

/// The oldest and newest raw `ts` for a series, epoch ms.
///
/// Two `ORDER BY ts … LIMIT 1` reads over the `(series, ts)` index rather than `math::min/max`
/// aggregates: the aggregate form needs a subquery-collect to be correct here and reads worse for
/// no gain, and the index already serves the ordered limit at both ends. The order key is in the
/// projection because the engine only orders by what is selected (`cap.rs` pins the same rule).
async fn raw_extent(
    store: &Store,
    ws: &str,
    series: &str,
) -> Result<(Option<u64>, Option<u64>), StoreError> {
    let sql = format!(
        "SELECT ts, time::millis(ts) AS ts_ms FROM {SERIES_TABLE} \
         WHERE series = $series ORDER BY ts ASC LIMIT 1;
         SELECT ts, time::millis(ts) AS ts_ms FROM {SERIES_TABLE} \
         WHERE series = $series ORDER BY ts DESC LIMIT 1;"
    );
    let mut resp = store
        .query_ws(
            ws,
            &sql,
            vec![("series".into(), Value::String(series.to_string()))],
        )
        .await?;
    let first: Vec<TsRow> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    let last: Vec<TsRow> = resp
        .take(1)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok((
        first.first().map(|r| r.ts_ms),
        last.first().map(|r| r.ts_ms),
    ))
}

#[derive(serde::Serialize, serde::Deserialize)]
struct TsRow {
    ts_ms: u64,
}

/// Stored rollup rows per tier width, plus their total. Ascending by width so the finest tier reads
/// first — the order an operator expects when comparing against the raw count.
async fn rollup_tiers(
    store: &Store,
    ws: &str,
    series: &str,
) -> Result<(u64, Vec<TierRows>), StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            &format!(
                "SELECT width_ms, count() AS rows FROM {ROLLUP_TABLE} \
                 WHERE series = $series GROUP BY width_ms"
            ),
            vec![("series".into(), Value::String(series.to_string()))],
        )
        .await?;
    let mut tiers: Vec<TierRows> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    tiers.sort_by_key(|t| t.width_ms);
    let total = tiers.iter().map(|t| t.rows).sum();
    Ok((total, tiers))
}

/// The distinct producers with raw rows in this series.
///
/// `GROUP BY producer` over the projected column — the store's aggregate rules allow grouping a
/// projected column but not arithmetic over the aggregate, so this stays a bare projection and the
/// ordering is done in Rust (ordering by a grouped column in-engine is the fragile half).
///
/// Public as [`series_producers`] because the producer-health fan-out needs exactly this and nothing
/// else: routing it through [`series_stats`] would drag three `count()`s and two extent reads along
/// for a list it does not use. Same single-subject rule as the module doc — do not fan this out.
pub async fn series_producers(
    store: &Store,
    ws: &str,
    series: &str,
) -> Result<Vec<String>, StoreError> {
    producers(store, ws, series).await
}

async fn producers(store: &Store, ws: &str, series: &str) -> Result<Vec<String>, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            &format!(
                "SELECT producer FROM {SERIES_TABLE} WHERE series = $series GROUP BY producer"
            ),
            vec![("series".into(), Value::String(series.to_string()))],
        )
        .await?;
    let mut names: Vec<String> = resp
        .take("producer")
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    names.sort();
    names.dedup();
    Ok(names)
}

// SurrealDB 3 reads query rows through `SurrealValue`. These delegate to serde rather than
// deriving, so `#[serde(default)]` and `deserialize_with = "de_opt_lenient_f64"` keep working
// unchanged — the derive supports neither. See `lb_store::surreal_value_via_serde!`.
lb_store::surreal_value_via_serde!(TsRow, TierRows);
