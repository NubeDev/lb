//! The FIFO sample cap must CONVERGE, not merely make progress — `cap_series` deletes in slices of
//! [`CAP_EVICT_BATCH`] and loops until the series is at or under its bound, and only a backlog
//! larger than one slice can prove the loop goes round.
//!
//! Why its own file: every other cap test (`series_retention_test.rs`, `series_cap_reactor_test.rs`)
//! evicts a few tens of rows — comfortably inside one 5 000-row slice, so the loop breaks on its
//! first pass and a broken termination condition is invisible. That is the second blind spot in
//! `docs/scope/testing/testing-scope.md` §3.2, and the exact shape of the drain-loop stall recorded
//! in `docs/debugging/ingest/filtered-batch-stops-the-drain-loop.md`.
//!
//! Real embedded store, real committed rows through the real write→commit path — no mocks
//! (testing §0).

use lb_ingest::{
    cap_series, commit_direct, run_gc, sample_count, set_policy, Policy, Qos, Sample,
    CAP_EVICT_BATCH,
};
use lb_store::Store;
use serde_json::json;

fn sample(series: &str, seq: u64) -> Sample {
    Sample {
        series: series.into(),
        producer: "p".into(),
        ts: seq * 1_000,
        seq,
        payload: json!(seq as f64),
        labels: json!({}),
        qos: Qos::BestEffort,
    }
}

/// Commit `n` samples through the real write path, which chunks them into several transactions.
async fn seed(store: &Store, ws: &str, series: &str, n: u64) {
    let samples: Vec<Sample> = (1..=n).map(|i| sample(series, i)).collect();
    commit_direct(store, ws, &samples).await.unwrap();
    assert_eq!(sample_count(store, ws, series).await.unwrap(), n);
}

/// A series more than ONE eviction slice over its cap converges to the bound in a single
/// `cap_series` call — the loop keeps going after its first full slice.
///
/// A one-slice test cannot see this: `evict_older_than` returns a full slice, and a loop that
/// stopped right there would still land on the bound whenever the overshoot fit in one slice.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_series_more_than_one_evict_slice_over_cap_converges_in_one_call() {
    let store = Store::memory().await.unwrap();
    const CAP: u64 = 100;
    // Over-cap by 5 900 — two slices (5 000 + 900), so the loop MUST iterate.
    let n = CAP + CAP_EVICT_BATCH as u64 + 900;
    seed(&store, "nube", "wide", n).await;

    let evicted = cap_series(&store, "nube", "wide", CAP).await.unwrap();
    assert_eq!(
        evicted,
        (n - CAP) as usize,
        "one call converges: {} rows is more than one {CAP_EVICT_BATCH}-row slice, so a loop that \
         stopped after its first slice would strand the remainder",
        n - CAP
    );
    assert_eq!(sample_count(&store, "nube", "wide").await.unwrap(), CAP);

    // FIFO: the NEWEST `CAP` survive (the oldest `ts` go first).
    let kept = lb_ingest::read(&store, "nube", "wide", None, None)
        .await
        .unwrap();
    assert_eq!(kept.len(), CAP as usize);
    assert_eq!(
        kept.iter().map(|s| s.seq).min().unwrap(),
        n - CAP + 1,
        "the oldest rows are the ones evicted, across BOTH slices"
    );

    // Idempotent: a second call at the bound evicts nothing.
    assert_eq!(cap_series(&store, "nube", "wide", CAP).await.unwrap(), 0);
}

/// The same convergence through the GC pass the reactor actually calls — `run_gc` must report the
/// full multi-slice eviction, not one slice's worth.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_gc_pass_reports_a_multi_slice_cap_eviction_in_full() {
    let store = Store::memory().await.unwrap();
    const CAP: u64 = 50;
    let n = CAP + CAP_EVICT_BATCH as u64 + 500;
    seed(&store, "nube", "fleet.a", n).await;

    set_policy(
        &store,
        "nube",
        &Policy {
            prefix: "fleet.".into(),
            raw_for_ms: 0, // the count axis only — isolate the cap loop
            max_samples: CAP,
            tiers: vec![],
            filter: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let pass = run_gc(&store, "nube", n * 1_000 + 1).await.unwrap();
    assert_eq!(
        pass.capped_raw,
        (n - CAP) as usize,
        "the GC pass accounts for every evicted row, across both slices"
    );
    assert_eq!(sample_count(&store, "nube", "fleet.a").await.unwrap(), CAP);

    // A second pass is a no-op — the first one converged rather than needing another tick.
    assert_eq!(
        run_gc(&store, "nube", n * 1_000 + 1)
            .await
            .unwrap()
            .capped_raw,
        0
    );
}
