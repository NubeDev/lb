//! The retention GC pass — rollup-then-evict, executed on demand (series-retention scope). For
//! every policy: each matching series' raw samples older than the raw horizon are folded into the
//! policy's rollup tiers (stored, exact — sum+count travel with min/max/last), then the raw rows
//! are deleted, then each tier's own horizon evicts its stale rollup rows. The table stops growing
//! forever; coarse history survives eviction.
//!
//! Cutoffs are **snapped down to a bucket boundary**, so a bucket is only ever rolled up once,
//! complete — a later pass never re-aggregates a half-evicted bucket (that would silently shrink its
//! min/max/count). `now_ms` is caller-injected (determinism §3): the host verb stamps wall-clock;
//! tests stamp a constant.
//!
//! **The snap is PER TIER** ([`tier_cutoff`]), not one cutoff floored by the widest width. Each tier
//! has its own grid now that a tier can declare an [`crate::Align`], and "complete bucket" is a
//! question about the grid the tier is folded on — a single shared cutoff can only be a boundary on
//! one of them. Raw is then evicted no further than the LEAST-advanced tier has folded
//! ([`evict_cutoff`]), which is what makes "every tier has seen this data" true before it is deleted.
//! The old one-cutoff rule was already subtly wrong for widths that do not divide each other (an
//! hourly floor is not a 7-minute boundary); alignment made the hole reachable rather than creating
//! it.

use lb_store::{Store, StoreError};
use serde_json::Value;

use crate::align::bucket_start;
use crate::bucket::{read_buckets, BucketQuery};
use crate::cap::{
    cap_cutoff_ms, cap_series, default_cap_notice, sample_count, DEFAULT_MAX_SAMPLES,
};
use crate::clock_sanity::{backwards_warning, newest_sample_ms, skew, skew_warning};
use crate::dead_letter_gc::{prune_dead_letters, DEAD_LETTER_KEEP_MS};
use crate::meta::series_names;
use crate::page::PageError;
use crate::pass_record::{last_pass, record_pass, GcPassRecord};
use crate::retention::{list_policies, resolve_policy, Policy};
use crate::rollup::{evict_rollups, read_rollups, rollup_widths, write_rollups, RollupRow};
use crate::rollup_cap::cap_rollup_rows;
use crate::rollup_window::{evict_cutoff, oldest_raw_ts, tier_cutoff};
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
    /// Rollup rows evicted by a tier's FIFO row cap (`Tier::max_rows`), as distinct from
    /// `evicted_rollup`'s time horizon. The one bound on rollups that never reads `now_ms`
    /// (rubix-ai#84's dead-clock half) — see [`crate::rollup_cap`].
    #[serde(default)]
    pub capped_rollup: usize,
    /// Dead-letter rows evicted by [`DEAD_LETTER_KEEP_MS`] — reported separately from `evicted_raw`
    /// because they are a different table on a different horizon (disk-budget decision 7).
    #[serde(default)]
    pub evicted_dead_letters: usize,
    /// Notices for unpoliced series the DEFAULT cap evicted from (see `DEFAULT_MAX_SAMPLES`). The
    /// cap is enforced now rather than advisory, so these report what was deleted — an eviction is a
    /// policy decision, but never an invisible one.
    ///
    /// Returned as DATA rather than logged here: `lb-ingest` is a primitives crate with no
    /// `tracing` dependency, and the caller (the retention reactor / the `series.retention.gc`
    /// verb) is what owns an output channel. The verb hands them to its caller too, so an operator
    /// sees them without reading node logs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    /// How far the DATA leads the clock — i.e. the clock is BEHIND, which makes horizons land too
    /// early so the pass evicts too little (possibly nothing) while the store grows. Set when that
    /// exceeds [`crate::SKEW_TOLERANCE_MS`]; `None` on a healthy pass.
    ///
    /// Only this direction is representable, and that is deliberate: a clock AHEAD of the data is
    /// the ordinary state of any idle series, not a fault (`clock_sanity::skew`). The fast-clock
    /// failure is reported through `warnings` by the independent floor check instead.
    ///
    /// The number exists alongside the human warning because the two have different consumers: the
    /// warning is for the operator reading a GC result, this is for the thing that ALARMS without a
    /// human reading anything. A monitor cannot threshold a sentence, and grepping prose for
    /// "clock skew" is a contract nobody agreed to.
    ///
    /// This is the field that makes an inert pass distinguishable from an idle one. Before it,
    /// `evicted_raw: 0, warnings: 0` was returned by both a healthy node with nothing to evict and a
    /// node whose clock had drifted 46 minutes and was evicting NOTHING while the store grew
    /// (rubix-ai#84 AC 7, observed live on RC-6 2026-08-04).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clock_skew_ms: Option<u64>,
}

/// Run one retention pass over every policy in `ws` at logical time `now_ms`.
///
/// Each series is governed by exactly ONE policy — the **longest matching prefix**. Iterating every
/// policy blindly would process a series under both `fleet.` and `fleet.eu.`, letting the tighter
/// bound win *by accident*; with a count cap that ambiguity evicts real rows, so the precedence is
/// specified here rather than left emergent (a latent bug in the shipped GC, fixed with this slice).
pub async fn run_gc(store: &Store, ws: &str, now_ms: u64) -> Result<GcPass, StoreError> {
    let started = std::time::Instant::now();
    let mut pass = GcPass::default();

    // FIRST, before any horizon is computed: is `now_ms` even plausible? Every bound below is
    // `now_ms - horizon`, so a clock behind the data makes every one of them land before the oldest
    // row and the whole pass evicts nothing — while returning exactly what a healthy idle pass
    // returns. This does not stop the pass (a stopped GC is the failure being guarded against); it
    // makes the pass stop being SILENT about running on a clock the data contradicts.
    //
    // Only ONE direction is checkable here, and the asymmetry is fundamental: data ahead of the
    // clock is impossible (so always a clock error), while a clock ahead of the data is ORDINARY —
    // it just means nothing was written recently, which is true of every idle or decommissioned
    // series. `clock_sanity::skew` carries the full reasoning. The genuine fast-clock case is caught
    // by the independent floor check below instead.
    if let Some(newest) = newest_sample_ms(store, ws).await? {
        if let Some(delta) = skew(now_ms, Some(newest)) {
            pass.clock_skew_ms = Some(delta);
            if let Some(warning) = skew_warning(now_ms, Some(newest)) {
                pass.warnings.push(warning);
            }
        }
    }

    // SECOND, and INDEPENDENT of the data: has the clock gone backwards since this node's own last
    // pass? The check above compares two clocks and is therefore blind when both are wrong together
    // — which is the normal case on a box whose producer and node share one bad system clock, and
    // total when the series table is empty (a fresh boot has no data to contradict anything).
    //
    // This one compares `now_ms` to a MONOTONIC FLOOR instead: a pass demonstrably ran at
    // `last_run_ms`, so the true time is at least that. A clock below it moved backwards between two
    // boots, which a correct clock cannot do. That is the power-cycle-with-no-RTC case (AC 8), and
    // it is detectable even on a workspace holding no samples at all.
    //
    // Read BEFORE `record_pass` overwrites the row at the end of this function.
    if let Some(warning) =
        backwards_warning(now_ms, last_pass(store, ws).await?.map(|p| p.last_run_ms))
    {
        pass.warnings.push(warning);
    }

    let policies = list_policies(store, ws).await?;
    for policy in &policies {
        // Only the series this policy actually GOVERNS — a series under a longer prefix belongs to
        // that policy alone.
        let owned: Vec<String> = series_names(store, ws, &policy.prefix)
            .await?
            .into_iter()
            .filter(|s| governs(&policies, &policy.prefix, s))
            .collect();

        // The TIME horizon: roll up then evict what is older than it. Each tier folds up to its own
        // grid boundary; raw goes only as far as the least-advanced tier reached.
        if policy.raw_for_ms > 0 && policy.raw_for_ms <= now_ms {
            let horizon = now_ms - policy.raw_for_ms;
            let evict_to = evict_cutoff(policy, horizon);
            for series in &owned {
                pass.rollup_rows += rollup_series(store, ws, series, policy, horizon).await?;
                pass.evicted_raw += evict_raw(store, ws, series, evict_to).await?;
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

        // The rollup COUNT cap: independent of the horizons above and — deliberately — of `now_ms`.
        // Every `keep_for_ms` bound is `now_ms - horizon`, so a clock that is stopped, behind, or
        // reset by a power cycle evicts nothing while the tier grows; this compares the tier's row
        // count to a number and holds with a dead clock. Runs after the time horizon, so it only
        // evicts what survived it.
        for tier in &policy.tiers {
            if tier.max_rows == 0 {
                continue; // unbounded — the default, and every pre-existing tier
            }
            for series in &owned {
                pass.capped_rollup +=
                    cap_rollup_rows(store, ws, series, tier.width_ms, tier.max_rows).await?;
            }
        }

        // ...and the widths the policy NO LONGER declares. Eviction above is keyed by an exact
        // `width_ms`, so editing a policy (5-minute buckets -> 1-minute buckets) strands the old
        // width's rows: nothing writes to them again and no tier ever matches them, so they were
        // retained forever. That is unbounded growth in the feature whose entire job is bounding
        // growth, and it is invisible until someone reads the per-tier occupancy — which is exactly
        // how it was found (a live series showed 5-min and 15-min rows under a policy declaring only
        // 1-min).
        //
        // They are DRAINED, not destroyed. Deleting them the moment a tier is dropped would make an
        // ordinary policy edit silently destroy history that raw can no longer regenerate, so they
        // age out on the policy's most generous declared horizon instead: bounded, and never a
        // surprise deletion at the instant of the edit. A policy that keeps ANY tier forever states
        // an intent to keep rollups indefinitely, so its orphans are left alone rather than being
        // held to a horizon the policy never declared.
        let keeps_a_tier_forever = policy.tiers.iter().any(|t| t.keep_for_ms == 0);
        let longest_keep = policy
            .tiers
            .iter()
            .map(|t| t.keep_for_ms)
            .max()
            .unwrap_or(0);
        if !keeps_a_tier_forever && longest_keep > 0 && longest_keep <= now_ms {
            let declared: Vec<u64> = policy.tiers.iter().map(|t| t.width_ms).collect();
            let before = now_ms - longest_keep;
            for width in rollup_widths(store, ws, &policy.prefix).await? {
                if !declared.contains(&width) {
                    // The SAME narrow, tested delete the declared tiers use — one exact width at a
                    // time. A single `width_ms NOT IN [...]` statement would be tidier and is not
                    // worth it: a deletion whose predicate is subtly wrong takes the whole table.
                    pass.evicted_rollup +=
                        evict_rollups(store, ws, &policy.prefix, width, before).await?;
                }
            }
        }
    }

    // Series NO policy covers: bounded by `DEFAULT_MAX_SAMPLES` (disk-budget slice 3). Policy-record
    // EXISTENCE is what decides — a record saying `max_samples: 0` is handled by the loop above and
    // left genuinely unbounded, which is the opt-out the previous release's warning promised.
    cap_unpoliced(store, ws, &policies, &mut pass).await?;

    // The dead-letter table, on its own 30-day horizon: the one ingest table nothing used to prune.
    pass.evicted_dead_letters = prune_dead_letters(store, ws, now_ms, DEAD_LETTER_KEEP_MS).await?;

    // Record the pass — UNCONDITIONALLY, even when it evicted nothing. `run_gc` (not the reactor)
    // owns this write so the on-demand `series.retention.gc` verb and the periodic reactor record
    // through ONE path; recording in the reactor would let a manual GC leave the status stale.
    //
    // Do NOT make this conditional on `evicted_raw > 0` (or any other "did something" test): a
    // healthy idle node would then show a frozen `last_run_ms` and be indistinguishable from a node
    // whose reactor died. That is the single behaviour in this feature that is easiest to implement
    // backwards, and `idle_pass_still_stamps_last_run_ms` is the test that holds it.
    let elapsed = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
    record_pass(store, ws, &GcPassRecord::new(&pass, now_ms, elapsed)).await?;

    Ok(pass)
}

/// Does the policy at `prefix` govern `series` — i.e. is it the LONGEST matching prefix?
///
/// Delegates to [`resolve_policy`] so the GC and the commit-time filter can never disagree about
/// which policy owns a series (they used to hold separate copies of this rule).
fn governs(policies: &[Policy], prefix: &str, series: &str) -> bool {
    resolve_policy(policies, series).is_some_and(|p| p.prefix == prefix)
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
        // The cutoff is snapped DOWN to each tier's own boundary for the same reason the time
        // horizon snaps: only COMPLETE buckets roll up, so a later pass never re-aggregates a
        // half-evicted bucket and silently shrinks its min/max/count. `rollup_series` does that snap
        // per tier, so this hands it the raw cap boundary.
        if let Some(cutoff) = cap_cutoff_ms(store, ws, series, policy.max_samples).await? {
            rolled = rollup_series(store, ws, series, policy, cutoff).await?;
        }
    }
    Ok((
        rolled,
        cap_series(store, ws, series, policy.max_samples).await?,
    ))
}

/// FIFO-evict every registered series that NO policy covers down to [`DEFAULT_MAX_SAMPLES`], and
/// report each eviction. This is what "bounded by default" means: a node whose operator set no
/// policy at all still cannot grow a series forever.
///
/// **The predicate is `starts_with` on a policy's prefix — the same "is there a record covering this
/// series" test the loop above uses, NOT `max_samples != 0`.** That distinction is the whole of
/// disk-budget decision 9: a policy row that says `max_samples: 0` is covered here, skipped, and
/// left unbounded exactly as written, while a series with no row at all is capped.
///
/// There are no tiers to fold into (a series with no policy has none), so this evicts raw history
/// outright — which is why it is loud. The alternative is the shape this slice exists to end: a
/// default that keeps everything until the disk decides for you.
async fn cap_unpoliced(
    store: &Store,
    ws: &str,
    policies: &[Policy],
    pass: &mut GcPass,
) -> Result<(), StoreError> {
    for series in series_names(store, ws, "").await? {
        if policies.iter().any(|p| series.starts_with(&p.prefix)) {
            continue; // governed by a policy record — its own bounds apply (possibly deliberately none)
        }
        let count = sample_count(store, ws, series.as_str()).await?;
        if count <= DEFAULT_MAX_SAMPLES {
            continue;
        }
        let evicted = cap_series(store, ws, series.as_str(), DEFAULT_MAX_SAMPLES).await?;
        if evicted > 0 {
            pass.capped_raw += evicted;
            pass.warnings
                .push(default_cap_notice(&series, evicted, count));
        }
    }
    Ok(())
}

/// Fold one series' RAW samples into each tier, each up to ITS OWN [`tier_cutoff`], and store the
/// rows. `horizon` is the unsnapped boundary (the raw time horizon, or the count cap's oldest-kept
/// timestamp); the snap happens per tier because the grids differ.
///
/// The window is `[oldest surviving raw, cutoff)` — never `[0, cutoff)`. A tier is derived from RAW
/// and only from raw: re-deriving it from another tier's rows is exact only when the two grids nest,
/// which per-tier anchors no longer guarantee ([`crate::rollup_window`] carries the full reasoning
/// and the measurement). Buckets older than the window keep the rows they were written with, back
/// when their raw was whole.
async fn rollup_series(
    store: &Store,
    ws: &str,
    series: &str,
    policy: &Policy,
    horizon: u64,
) -> Result<usize, StoreError> {
    // No raw at all → nothing to fold, and nothing to overwrite. A fully-evicted series keeps the
    // rows it already has rather than re-deriving them from a coarser substitute.
    let Some(oldest_raw) = oldest_raw_ts(store, ws, series).await? else {
        return Ok(0);
    };
    let mut written = 0;
    for tier in &policy.tiers {
        let cutoff = tier_cutoff(tier, horizon);
        let from = bucket_start(oldest_raw, tier.width_ms, tier.align);
        if cutoff == 0 || cutoff <= from {
            continue; // nothing complete to fold yet (or a width-0 non-tier)
        }
        // The bucket CONTAINING the oldest surviving raw may have had older raw evicted beneath it
        // on an earlier pass — it was folded whole then. Re-folding it from what is left would
        // shrink a correct row, so an existing row at that start wins. Every later bucket starts at
        // or after `oldest_raw` and is therefore wholly raw-backed.
        let partial = if from < oldest_raw {
            read_rollups(store, ws, series, from, from + tier.width_ms)
                .await?
                .iter()
                .any(|r| r.width_ms == tier.width_ms && r.t == from)
        } else {
            false
        };
        let q = BucketQuery {
            from_ts: from,
            to_ts: cutoff,
            width_ms: Some(tier.width_ms),
            budget: None,
            // The fold's grid IS the tier's grid. Reads resolve the same alignment through
            // `Policy::align_for`, and `series_align_grid_test` folds-then-reads to prove they agree
            // — a disagreement here mixes two griddings inside one tier and nothing errors.
            align: tier.align,
        };
        let buckets = read_buckets(store, ws, series, &q, tier.width_ms)
            .await
            .map_err(|e| match e {
                PageError::Store(s) => s,
                PageError::BadCursor(m) => StoreError::Decode(m),
            })?;
        let rows: Vec<RollupRow> = buckets
            .iter()
            .filter(|b| !(partial && b.t == from))
            // Re-folding the same raw lands the same row at the same deterministic id — idempotent;
            // only buckets with data are stored (sparse).
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
                // Keep the bucket's first representative so the tier can answer `first`/`nearest`
                // after the raw samples beneath it are gone. `None` when the bucket itself was built
                // from a pre-normalize rollup row — the missing provenance propagates rather than
                // being invented (series-normalize scope).
                first: b.first.clone(),
                first_ts: b.has_first.then_some(b.first_ts),
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
    // Retry-on-conflict: this DELETE over `series` races the inline drains' `series` upserts under
    // SurrealDB's optimistic MVCC (drain-vs-GC — the periodic collision surface WS-B's per-ws drain
    // lock does NOT cover). The count+delete is a single idempotent pass, so a retried run evicts the
    // same rows exactly once.
    let mut resp = store
        .query_ws_retrying(
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
