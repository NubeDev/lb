//! Rollup rows at a width the policy NO LONGER declares (series-observability follow-up).
//!
//! **The bug.** Rollup eviction is keyed by an exact `width_ms`, once per DECLARED tier. Editing a
//! policy — 5-minute buckets to 1-minute buckets — strands the old width's rows: nothing writes to
//! them again, and no tier ever matches them for eviction. They were retained forever. That is
//! unbounded growth inside the feature whose entire job is bounding growth, and it is invisible
//! until someone reads per-tier occupancy — which is how it was found, on a live node showing
//! `1 min x 270 · 5 min x 115 · 15 min x 75` under a policy that declared only the 1-minute tier.
//!
//! **The fix drains, it does not destroy.** Deleting stranded rows the instant a tier is dropped
//! would make an ordinary policy edit silently destroy history that raw can no longer regenerate.
//! They age out on the policy's most generous declared horizon instead. The two tests that matter
//! here are therefore the pair: one proves stranded rows DO eventually go, the other proves they are
//! NOT taken at the moment of the edit.
//!
//! Real `Store::memory()` (real SurrealDB kv-mem), real rows, real `run_gc`. No mocks.

use lb_ingest::{run_gc, set_policy, write_rollups, Policy, RollupRow, Tier};
use lb_store::Store;

const PREFIX: &str = "plant-a.";
const SERIES: &str = "plant-a.chiller-1.current-l1";
const MIN: u64 = 60_000;
const DAY: u64 = 86_400_000;
/// A "now" far enough from the epoch that every horizon below is `<= now_ms`.
const NOW: u64 = 30 * DAY;

/// One rollup row at `width` whose bucket start is `age_ms` before [`NOW`].
fn row(width: u64, age_ms: u64) -> RollupRow {
    RollupRow {
        series: SERIES.into(),
        width_ms: width,
        t: NOW - age_ms,
        min: Some(1.0),
        max: Some(3.0),
        sum: 60.0,
        num_count: 30,
        count: 30,
        last: serde_json::json!({ "v": 2.0 }),
        last_ts: NOW - age_ms,
        first: serde_json::json!({ "v": 2.0 }),
        first_ts: Some(NOW - age_ms),
    }
}

/// A policy declaring one tier at `width`, kept for `keep_for_ms`.
fn policy(width: u64, keep_for_ms: u64) -> Policy {
    Policy {
        prefix: PREFIX.into(),
        raw_for_ms: 3_600_000,
        max_samples: 0,
        tiers: vec![Tier {
            width_ms: width,
            keep_for_ms,
            method: None,
            ..Default::default()
        }],
        filter: None,
        ..Default::default()
    }
}

async fn widths_present(store: &Store, ws: &str) -> Vec<u64> {
    lb_ingest::rollup_widths(store, ws, PREFIX).await.unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_width_the_policy_no_longer_declares_is_eventually_evicted() {
    let store = Store::memory().await.unwrap();
    // History at 5 min (the old tier) and 1 min (the new one), all older than the 7-day horizon.
    write_rollups(
        &store,
        "acme",
        &[row(5 * MIN, 10 * DAY), row(MIN, 10 * DAY)],
    )
    .await
    .unwrap();
    set_policy(&store, "acme", &policy(MIN, 7 * DAY))
        .await
        .unwrap();

    assert_eq!(widths_present(&store, "acme").await, vec![MIN, 5 * MIN]);

    let pass = run_gc(&store, "acme", NOW).await.unwrap();

    // Both go: the declared tier by its own rule, the stranded width by the orphan sweep. What
    // matters is that the 5-minute rows are no longer immortal.
    assert!(
        !widths_present(&store, "acme").await.contains(&(5 * MIN)),
        "the undeclared 5-minute width survived GC — it is retained forever again"
    );
    assert!(pass.evicted_rollup >= 1, "pass reported no rollup eviction");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn dropping_a_tier_does_not_destroy_its_history_at_the_moment_of_the_edit() {
    // THE guard on the fix. An operator narrowing a policy must not lose yesterday's aggregates the
    // next time GC ticks — raw is long gone and cannot regenerate them.
    let store = Store::memory().await.unwrap();
    write_rollups(&store, "acme", &[row(5 * MIN, 2 * DAY)])
        .await
        .unwrap();
    set_policy(&store, "acme", &policy(MIN, 7 * DAY))
        .await
        .unwrap();

    run_gc(&store, "acme", NOW).await.unwrap();

    assert!(
        widths_present(&store, "acme").await.contains(&(5 * MIN)),
        "2-day-old stranded rows were destroyed under a 7-day horizon — a policy edit must not \
         silently delete history that raw can no longer rebuild"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_policy_that_keeps_a_tier_forever_keeps_its_stranded_rows_too() {
    // `keep_for_ms: 0` states an intent to keep rollups indefinitely. Holding its stranded rows to
    // a horizon it never declared would invent a retention rule the operator did not write.
    let store = Store::memory().await.unwrap();
    write_rollups(&store, "acme", &[row(5 * MIN, 10 * DAY)])
        .await
        .unwrap();
    set_policy(&store, "acme", &policy(MIN, 0)).await.unwrap();

    run_gc(&store, "acme", NOW).await.unwrap();

    assert!(
        widths_present(&store, "acme").await.contains(&(5 * MIN)),
        "a keep-forever policy evicted stranded rows on a horizon it never declared"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_declared_width_is_untouched_by_the_orphan_sweep() {
    // The sweep must never reach a width the policy DOES declare — that would double-evict on a
    // horizon other than the tier's own.
    let store = Store::memory().await.unwrap();
    write_rollups(&store, "acme", &[row(MIN, 2 * DAY)])
        .await
        .unwrap();
    set_policy(&store, "acme", &policy(MIN, 7 * DAY))
        .await
        .unwrap();

    run_gc(&store, "acme", NOW).await.unwrap();

    assert!(
        widths_present(&store, "acme").await.contains(&MIN),
        "the declared tier's own rows were evicted early by the orphan sweep"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_sweep_never_reaches_another_workspace() {
    let store = Store::memory().await.unwrap();
    write_rollups(&store, "acme", &[row(5 * MIN, 10 * DAY)])
        .await
        .unwrap();
    write_rollups(&store, "other", &[row(5 * MIN, 10 * DAY)])
        .await
        .unwrap();
    set_policy(&store, "acme", &policy(MIN, 7 * DAY))
        .await
        .unwrap();

    run_gc(&store, "acme", NOW).await.unwrap();

    assert!(
        widths_present(&store, "other").await.contains(&(5 * MIN)),
        "GC in ws `acme` evicted ws `other`'s stranded rows"
    );
}
