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
use crate::cap::{cap_cutoff_ms, cap_series, over_cap_warning, sample_count};
use crate::meta::series_names;
use crate::page::PageError;
use crate::retention::{list_policies, Policy};
use crate::rollup::{evict_rollups, write_rollups, RollupRow};
use crate::staging::SERIES_TABLE;

/// Outcome of one GC pass over a workspace.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct GcPass {
    pub evicted_raw: usize,
    pub rollup_rows: usize,
    pub evicted_rollup: usize,
    /// Raw rows evicted by the per-series FIFO count cap (`max_samples`), as distinct from
    /// `evicted_raw`'s time horizon. Eviction is a policy decision, but it must be observable —
    /// never an invisible drop (issue #65).
    pub capped_raw: usize,
    /// Advisory warnings for unpoliced series past the recommended cap — release 1 makes the need
    /// for a policy visible while nothing is evicted by default (see `DEFAULT_MAX_SAMPLES`).
    ///
    /// Returned as DATA rather than logged here: `lb-ingest` is a primitives crate with no
    /// `tracing` dependency, and the caller (the retention reactor / the `series.retention.gc`
    /// verb) is what owns an output channel. The verb hands them to its caller too, so an operator
    /// sees them without reading node logs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

/// Run one retention pass over every policy in `ws` at logical time `now_ms`.
///
/// Each series is governed by exactly ONE policy — the **longest matching prefix**. Iterating every
/// policy blindly would process a series under both `fleet.` and `fleet.eu.`, letting the tighter
/// bound win *by accident*; with a count cap that ambiguity evicts real rows, so the precedence is
/// specified here rather than left emergent (a latent bug in the shipped GC, fixed with this slice).
pub async fn run_gc(store: &Store, ws: &str, now_ms: u64) -> Result<GcPass, StoreError> {
    let mut pass = GcPass::default();
    let policies = list_policies(store, ws).await?;
    for policy in &policies {
        // Only the series this policy actually GOVERNS — a series under a longer prefix belongs to
        // that policy alone.
        let owned: Vec<String> = series_names(store, ws, &policy.prefix)
            .await?
            .into_iter()
            .filter(|s| governs(&policies, &policy.prefix, s))
            .collect();

        // The TIME horizon: roll up then evict what is older than it.
        if policy.raw_for_ms > 0 && policy.raw_for_ms <= now_ms {
            let cutoff = snap_cutoff(policy, now_ms - policy.raw_for_ms);
            for series in &owned {
                pass.rollup_rows += rollup_series(store, ws, series, policy, cutoff).await?;
                pass.evicted_raw += evict_raw(store, ws, series, cutoff).await?;
            }
        }

        // The COUNT cap: an INDEPENDENT bound on the same series — a sample is evicted when it
        // violates EITHER. Runs after the time horizon, so it only sees what survived it.
        for series in &owned {
            let (rolled, capped) = cap_pass(store, ws, series, policy).await?;
            pass.rollup_rows += rolled;
            pass.capped_raw += capped;
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

    // Series no policy covers: unbounded, and in this release only WARNED about (see
    // `DEFAULT_MAX_SAMPLES`) — release 2 flips them to bounded-by-default.
    pass.warnings = warn_unpoliced(store, ws, &policies).await?;
    Ok(pass)
}

/// Does the policy at `prefix` govern `series` — i.e. is it the LONGEST matching prefix?
fn governs(policies: &[Policy], prefix: &str, series: &str) -> bool {
    !policies
        .iter()
        .any(|p| p.prefix.len() > prefix.len() && series.starts_with(&p.prefix))
}

/// Apply one policy's count cap to one series: roll the over-cap rows into the tiers FIRST (so
/// coarse history survives, exactly as the time horizon does), then FIFO-evict them.
///
/// Returns `(rollup_rows, capped_raw)` — both halves are reported, so a cap eviction is as
/// observable in the pass counts as a time eviction is.
///
/// Cap-evicting without tiers is real data loss — the operator's explicit choice when they set
/// `max_samples` with no tier to fold into.
async fn cap_pass(
    store: &Store,
    ws: &str,
    series: &str,
    policy: &Policy,
) -> Result<(usize, usize), StoreError> {
    let count = sample_count(store, ws, series).await?;
    if policy.max_samples == 0 || count <= policy.max_samples {
        return Ok((0, 0));
    }
    let mut rolled = 0;
    if !policy.tiers.is_empty() {
        // Everything strictly older than the newest `max_samples` rows is about to go; fold exactly
        // that window into the tiers first. Rollup is idempotent (deterministic bucket ids), so an
        // overlap with the time horizon's earlier rollup re-upserts identical rows.
        //
        // The cutoff is snapped DOWN to the widest tier boundary for the same reason the time
        // horizon snaps: only COMPLETE buckets roll up, so a later pass never re-aggregates a
        // half-evicted bucket and silently shrinks its min/max/count.
        if let Some(cutoff) = cap_cutoff_ms(store, ws, series, policy.max_samples).await? {
            rolled = rollup_series(store, ws, series, policy, snap_cutoff(policy, cutoff)).await?;
        }
    }
    Ok((
        rolled,
        cap_series(store, ws, series, policy.max_samples).await?,
    ))
}

/// One warning per registered series that NO policy covers and that has grown past the recommended
/// cap. This is release 1's whole job on the default axis: make the need for a policy visible while
/// nothing is evicted yet.
async fn warn_unpoliced(
    store: &Store,
    ws: &str,
    policies: &[Policy],
) -> Result<Vec<String>, StoreError> {
    let mut warnings = Vec::new();
    for series in series_names(store, ws, "").await? {
        if policies.iter().any(|p| series.starts_with(&p.prefix)) {
            continue; // governed by a policy — its own bounds apply (possibly deliberately none)
        }
        let count = sample_count(store, ws, series.as_str()).await?;
        if let Some(warning) = over_cap_warning(&series, count, 0) {
            warnings.push(warning);
        }
    }
    Ok(warnings)
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
