//! The per-tier FIFO rollup-row cap ([`crate::Tier::max_rows`]) — the COUNT bound on stored
//! rollups, and the only bound on them that does not read the clock.
//!
//! **Why it exists.** Every other exit from the rollup table is a TIME horizon: a tier's
//! `keep_for_ms` and the orphan-width drain both evict rows older than `now_ms - horizon`. On the
//! field hardware `now_ms` comes from a wall clock with no RTC battery and no NTP route — it stops,
//! runs behind, and resets on every power cycle. A clock behind the data makes every horizon land
//! before the oldest row, so the pass evicts NOTHING while reporting exactly what a healthy idle
//! pass reports (rubix-ai#84; observed live twice, 46 min and 53 min behind). Raw already has a
//! clock-free bound (`max_samples`, [`crate::cap`]) — worse, that cap FOLDS its evictions into the
//! tiers first, so rollups were the one table that grew without bound under a dead clock, and
//! rollups are the term that actually fills a disc (~96 rows/day/point at 15-min buckets, forever).
//!
//! **Why counting is clock-proof.** "At most N rows" compares the table to a number; ordering is by
//! the rows' own `t` axis. No wall-clock value appears anywhere, so the bound holds when `now_ms`
//! is arbitrarily wrong, stopped, or jumping backwards.
//!
//! Over-cap rows are deleted outright, oldest `t` first — a rollup is already the coarsest copy,
//! there is nothing further to fold into. That is real data loss and the operator's explicit choice
//! when they set `max_rows`, exactly the posture `max_samples` takes for raw. Unlike the raw cap
//! there is no tie-break subtlety: the rollup id is deterministic at `[series, width_ms, t]`, so
//! `t` is unique within a tier and "keep the newest N" is exact.

use lb_store::{Store, StoreError};
use serde_json::Value;

use crate::schema::ROLLUP_TABLE;

/// Count of stored rollup rows for `series` at exactly `width_ms`.
pub async fn rollup_count(
    store: &Store,
    ws: &str,
    series: &str,
    width_ms: u64,
) -> Result<u64, StoreError> {
    let mut resp = store
        .query_ws_retrying(
            ws,
            &format!(
                "SELECT count() FROM {ROLLUP_TABLE} \
                 WHERE series = $series AND width_ms = $width GROUP ALL"
            ),
            vec![
                ("series".into(), Value::String(series.to_string())),
                ("width".into(), Value::Number(width_ms.into())),
            ],
        )
        .await?;
    let n: Option<i64> = resp
        .take("count")
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(n.unwrap_or(0).max(0) as u64)
}

/// The `t` of the OLDEST row we intend to keep when retaining the newest `keep` rows — rows with
/// `t` strictly below it are the eviction set. `None` when the tier holds `keep` rows or fewer.
///
/// The order key is in the projection — `ORDER BY` only sees selected idioms
/// (`debugging/store/order-by-needs-selected-idiom.md`, the same idiom `cap::keep_cutoff_ts` uses).
async fn keep_cutoff_t(
    store: &Store,
    ws: &str,
    series: &str,
    width_ms: u64,
    keep: u64,
) -> Result<Option<u64>, StoreError> {
    let mut resp = store
        .query_ws_retrying(
            ws,
            &format!(
                "SELECT t FROM {ROLLUP_TABLE} WHERE series = $series AND width_ms = $width \
                 ORDER BY t DESC LIMIT 1 START $skip"
            ),
            vec![
                ("series".into(), Value::String(series.to_string())),
                ("width".into(), Value::Number(width_ms.into())),
                (
                    "skip".into(),
                    Value::Number((keep.saturating_sub(1)).into()),
                ),
            ],
        )
        .await?;
    let rows: Vec<u64> = resp
        .take("t")
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(rows.first().copied())
}

/// Evict the oldest rollup rows of `series` at `width_ms` until at most `max_rows` remain. Returns
/// how many were deleted. `max_rows == 0` is unbounded (no-op) — the compatibility default.
pub async fn cap_rollup_rows(
    store: &Store,
    ws: &str,
    series: &str,
    width_ms: u64,
    max_rows: u64,
) -> Result<usize, StoreError> {
    if max_rows == 0 {
        return Ok(0); // explicitly unbounded — every pre-existing tier
    }
    if rollup_count(store, ws, series, width_ms).await? <= max_rows {
        return Ok(0);
    }
    let Some(cutoff) = keep_cutoff_t(store, ws, series, width_ms, max_rows).await? else {
        return Ok(0);
    };
    // Count-then-delete in one statement, retrying on MVCC conflict — the identical idempotent
    // shape as `rollup::evict_rollups`, narrowed from prefix to the one series being capped.
    let mut resp = store
        .query_ws_retrying(
            ws,
            &format!(
                "SELECT count() FROM {ROLLUP_TABLE} WHERE series = $series \
                 AND width_ms = $width AND t < $before GROUP ALL;
                 DELETE {ROLLUP_TABLE} WHERE series = $series \
                 AND width_ms = $width AND t < $before;"
            ),
            vec![
                ("series".into(), Value::String(series.to_string())),
                ("width".into(), Value::Number(width_ms.into())),
                ("before".into(), Value::Number(cutoff.into())),
            ],
        )
        .await?;
    let n: Option<i64> = resp
        .take("count")
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(n.unwrap_or(0).max(0) as usize)
}
