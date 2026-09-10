//! **Where a GC fold starts and stops** — the three bounds `run_gc` needs before it can roll a tier
//! up, kept together because they are one question with three answers and getting any of them wrong
//! is silent (series-observability scope, Decision 21).
//!
//! Before per-tier alignment there was one cutoff for everything: the raw horizon floored by the
//! WIDEST tier's width. That worked while every tier shared the epoch grid and the widths divided
//! each other. Neither is true now — a tier can declare its own [`crate::Align`], and two tiers'
//! boundaries need not nest — so "complete bucket" became a per-tier question.
//!
//! Three bounds, and the reason each exists:
//!
//! 1. [`tier_cutoff`] — how far ONE tier may fold: the horizon snapped down to that tier's own
//!    boundary. Only complete buckets roll up, or a later pass re-aggregates a half-evicted bucket
//!    and silently shrinks its min/max/count.
//! 2. [`evict_cutoff`] — how far RAW may be deleted: no further than the LEAST-advanced tier has
//!    folded. Raw that some tier has not yet seen is history no rollup can regenerate.
//! 3. [`oldest_raw_ts`] — where the fold BEGINS. This one is new, and it is the load-bearing half.
//!
//! # Why the fold begins at the oldest surviving raw sample
//!
//! `rollup_series` used to fold from `0` — the whole history, every pass. That is not merely
//! wasteful (it re-reads every bucket a series has ever had, on every 5-minute pass); once the raw
//! beneath a bucket is gone, the only thing left to re-derive that bucket from is ANOTHER tier's
//! rollup rows. Re-aggregating one tier from another is exact only when the two grids NEST, and with
//! independent per-tier anchors they need not: a 10-minute bucket anchored at :07 straddles the
//! boundary of a 90-minute bucket anchored at :30, so folding it whole into whichever 90-minute
//! bucket contains its START moves samples across a boundary. Measured, not theorised: a coarse
//! bucket holding 30 samples re-derived itself as 37 on the next pass, then stayed there.
//!
//! So the fold reads RAW ONLY, over exactly the window raw still covers. Every stored row is then
//! the exact fold of complete raw, written once while that raw existed, and never rewritten from a
//! lossy substitute. What this gives up is stated plainly rather than discovered: **adding a tier to
//! an existing policy does not backfill it** from the tiers already on disc. It takes effect from
//! the current raw window forward. Backfilling would mean re-deriving history from a grid that may
//! not nest — i.e. writing numbers that are wrong in a way nothing downstream could detect.

use lb_store::{Store, StoreError};
use serde_json::Value;

use crate::align::bucket_start;
use crate::retention::{Policy, Tier};
use crate::tables::SERIES_TABLE;

/// How far `tier` may fold: `horizon` snapped down to that tier's own bucket boundary.
///
/// A `width_ms` of `0` is not a tier — it describes no bucket, and folding it would divide by zero
/// in the pushed-down `GROUP BY`. It yields `0` (fold nothing). `series_retention_set` refuses to
/// write one; this is the belt to that braces, covering a row an older node may already hold.
pub(crate) fn tier_cutoff(tier: &Tier, horizon: u64) -> u64 {
    if tier.width_ms == 0 {
        return 0;
    }
    bucket_start(horizon, tier.width_ms, tier.align)
}

/// How far RAW may be evicted: no further than the least-advanced tier has folded.
///
/// A policy with no tiers evicts at the bare horizon — the existing behaviour for the tierless case,
/// which is an explicit choice to drop rather than to downsample.
///
/// This is also what keeps [`oldest_raw_ts`]'s window honest: because every tier folds at least as
/// far as raw is evicted, no bucket can have raw deleted out from under it before the tier that owns
/// it has folded it whole.
pub(crate) fn evict_cutoff(policy: &Policy, horizon: u64) -> u64 {
    policy
        .tiers
        .iter()
        .map(|t| tier_cutoff(t, horizon))
        .min()
        .unwrap_or(horizon)
}

/// The timestamp of the OLDEST raw sample of `series` still on disc, or `None` when the series holds
/// no raw at all (fully evicted, or never written).
///
/// The fold's left edge. Cheap — an indexed `LIMIT 1` on the `(series, ts)` window the paged read
/// already rides.
pub(crate) async fn oldest_raw_ts(
    store: &Store,
    ws: &str,
    series: &str,
) -> Result<Option<u64>, StoreError> {
    let mut resp = store
        .query_ws_retrying(
            ws,
            &format!(
                "SELECT time::millis(ts) AS ts FROM {SERIES_TABLE} \
                 WHERE series = $series ORDER BY ts ASC LIMIT 1"
            ),
            vec![("series".into(), Value::String(series.to_string()))],
        )
        .await?;
    let rows: Vec<TsRow> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(rows.first().map(|r| r.ts))
}

#[derive(serde::Serialize, serde::Deserialize)]
struct TsRow {
    ts: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::align::Align;

    const MIN: u64 = 60_000;
    const HOUR: u64 = 3_600_000;

    fn tier(width_ms: u64, align: Option<Align>) -> Tier {
        Tier {
            width_ms,
            keep_for_ms: 0,
            align,
            ..Default::default()
        }
    }

    /// Each tier snaps to ITS OWN grid — the whole reason one shared cutoff had to go.
    #[test]
    fn each_tier_snaps_to_its_own_boundary() {
        let horizon = 1_785_136_800_000; // 2026-07-27T07:00:00Z — on NEITHER tier's grid
        let fine = tier(
            10 * MIN,
            Some(Align {
                origin_ms: 7 * MIN as i64,
            }),
        );
        let coarse = tier(
            90 * MIN,
            Some(Align {
                origin_ms: 6 * HOUR as i64 + 30 * MIN as i64,
            }),
        );
        assert_eq!(tier_cutoff(&fine, horizon), 1_785_136_620_000); // 06:57Z
        assert_eq!(tier_cutoff(&coarse, horizon), 1_785_133_800_000); // 06:30Z
    }

    /// Raw follows the LEAST-advanced tier, so no tier is ever asked to fold data that is gone.
    #[test]
    fn raw_is_evicted_no_further_than_the_least_advanced_tier() {
        let horizon = 1_785_136_800_000;
        let policy = Policy {
            prefix: "p.".into(),
            tiers: vec![
                tier(
                    10 * MIN,
                    Some(Align {
                        origin_ms: 7 * MIN as i64,
                    }),
                ),
                tier(
                    90 * MIN,
                    Some(Align {
                        origin_ms: 6 * HOUR as i64 + 30 * MIN as i64,
                    }),
                ),
            ],
            ..Default::default()
        };
        let evict = evict_cutoff(&policy, horizon);
        assert_eq!(evict, 1_785_133_800_000, "the coarse tier is the laggard");
        for t in &policy.tiers {
            assert!(
                tier_cutoff(t, horizon) >= evict,
                "a tier folded less far than raw was cut"
            );
        }
    }

    /// No tiers = no downsampling: raw is dropped at the bare horizon, which is an operator's
    /// explicit choice and must not silently become "keep everything".
    #[test]
    fn a_tierless_policy_evicts_at_the_bare_horizon() {
        let policy = Policy {
            prefix: "p.".into(),
            ..Default::default()
        };
        assert_eq!(evict_cutoff(&policy, 12_345), 12_345);
    }

    /// A width-0 row an older node may hold folds nothing rather than dividing by zero.
    #[test]
    fn a_zero_width_tier_folds_nothing() {
        assert_eq!(tier_cutoff(&tier(0, None), 999_999), 0);
    }
}

// SurrealDB 3 reads query rows through `SurrealValue`. These delegate to serde rather than
// deriving, so `#[serde(default)]` and `deserialize_with = "de_opt_lenient_f64"` keep working
// unchanged — the derive supports neither. See `lb_store::surreal_value_via_serde!`.
lb_store::surreal_value_via_serde!(TsRow);
