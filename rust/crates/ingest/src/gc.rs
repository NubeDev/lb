//! The retention GC pass — rollup-then-evict, executed on demand (series-retention scope). For
//! every policy: each matching series' raw samples older than the raw horizon are folded into the
//! policy's rollup tiers (stored, exact — sum+count travel with min/max/last), then the raw rows
//! are deleted, then each tier's own horizon evicts its stale rollup rows. The table stops growing
//! forever; coarse history survives eviction.
//!
//! The eviction cutoff is **snapped down to the widest tier's bucket boundary**, so a bucket is
//! only ever rolled up once, complete — a later pass never re-aggregates a half-evicted bucket
//! (that would silently shrink its min/max/count). `now_ms` is caller-injected (determinism §3):
//! the host verb stamps wall-clock; tests stamp a constant.

use lb_store::{Store, StoreError};
use serde_json::Value;

use crate::bucket::{read_buckets, BucketQuery};
use crate::meta::series_names;
use crate::page::PageError;
use crate::retention::{list_policies, Policy};
use crate::rollup::{evict_rollups, write_rollups, RollupRow};
use crate::staging::SERIES_TABLE;

/// Outcome of one GC pass over a workspace.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct GcPass {
    pub evicted_raw: usize,
    pub rollup_rows: usize,
    pub evicted_rollup: usize,
}

/// Run one retention pass over every policy in `ws` at logical time `now_ms`.
pub async fn run_gc(store: &Store, ws: &str, now_ms: u64) -> Result<GcPass, StoreError> {
    let mut pass = GcPass::default();
    for policy in list_policies(store, ws).await? {
        if policy.raw_for_ms == 0 || policy.raw_for_ms > now_ms {
            continue; // disabled, or the horizon predates the epoch — nothing is old enough
        }
        let cutoff = snap_cutoff(&policy, now_ms - policy.raw_for_ms);
        for series in series_names(store, ws, &policy.prefix).await? {
            pass.rollup_rows += rollup_series(store, ws, &series, &policy, cutoff).await?;
            pass.evicted_raw += evict_raw(store, ws, &series, cutoff).await?;
        }
        for tier in &policy.tiers {
            if tier.keep_for_ms > 0 && tier.keep_for_ms <= now_ms {
                pass.evicted_rollup += evict_rollups(
                    store,
                    ws,
                    &policy.prefix,
                    tier.width_ms,
                    now_ms - tier.keep_for_ms,
                )
                .await?;
            }
        }
    }
    Ok(pass)
}

/// Snap the raw cutoff DOWN to the widest tier's bucket boundary — only complete buckets roll up.
fn snap_cutoff(policy: &Policy, cutoff: u64) -> u64 {
    match policy.tiers.iter().map(|t| t.width_ms).max() {
        Some(w) if w > 0 => cutoff / w * w,
        _ => cutoff,
    }
}

/// Fold one series' raw samples in `[0, cutoff)` into each tier and store the rows.
async fn rollup_series(
    store: &Store,
    ws: &str,
    series: &str,
    policy: &Policy,
    cutoff: u64,
) -> Result<usize, StoreError> {
    let mut written = 0;
    for tier in &policy.tiers {
        let q = BucketQuery {
            from_ts: 0,
            to_ts: cutoff,
            width_ms: Some(tier.width_ms),
            budget: None,
        };
        let buckets = read_buckets(store, ws, series, &q, tier.width_ms)
            .await
            .map_err(|e| match e {
                PageError::Store(s) => s,
                PageError::BadCursor(m) => StoreError::Decode(m),
            })?;
        let rows: Vec<RollupRow> = buckets
            .iter()
            // A bucket that is itself rollup-backed (count from a prior pass) re-upserts the same
            // row — idempotent; only buckets with data are stored (sparse).
            .map(|b| RollupRow {
                series: series.to_string(),
                width_ms: tier.width_ms,
                t: b.t,
                min: b.min,
                max: b.max,
                sum: b.sum,
                num_count: b.num_count,
                count: b.count,
                last: b.last.clone(),
                last_ts: b.last_ts,
            })
            .collect();
        written += rows.len();
        write_rollups(store, ws, &rows).await?;
    }
    Ok(written)
}

/// Delete raw samples of `series` older than `cutoff`. Returns the number evicted.
async fn evict_raw(
    store: &Store,
    ws: &str,
    series: &str,
    cutoff: u64,
) -> Result<usize, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            &format!(
                "SELECT count() FROM {SERIES_TABLE} WHERE series = $series \
                 AND ts < time::from::millis($cutoff) GROUP ALL;
                 DELETE {SERIES_TABLE} WHERE series = $series AND ts < time::from::millis($cutoff);"
            ),
            vec![
                ("series".into(), Value::String(series.to_string())),
                ("cutoff".into(), Value::Number(cutoff.into())),
            ],
        )
        .await?;
    let n: Option<i64> = resp
        .take("count")
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(n.unwrap_or(0).max(0) as usize)
}
