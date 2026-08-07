//! The per-tier rollup-row FIFO cap (`Tier::max_rows`) against the real store — the ONLY bound on
//! rollups that holds when the wall clock is arbitrarily wrong (rubix-ai#84's dead-clock half).
//!
//! **Load-bearing:** every other exit from the rollup table is `now_ms - horizon`. These tests run
//! the GC with `now_ms` BEHIND every row on disc — the no-RTC power-cycle state observed live —
//! and the cap must still bound the tier, keep the NEWEST rows, and report what it deleted.
//! Raw's count cap and time-horizon composition live in `series_sample_cap_test.rs` /
//! `series_retention_test.rs`.

use lb_ingest::{
    commit_batch, last_pass, read_rollups, run_gc, set_policy, write, write_rollups, Policy, Qos,
    RollupRow, Sample, Tier,
};
use lb_store::Store;
use serde_json::json;

const WIDTH: u64 = 10_000;

fn sample(series: &str, seq: u64, ts: u64) -> Sample {
    Sample {
        series: series.into(),
        producer: "p".into(),
        ts,
        seq,
        payload: json!(seq as f64),
        labels: json!({}),
        qos: Qos::BestEffort,
    }
}

fn rollup_row(series: &str, t: u64) -> RollupRow {
    RollupRow {
        series: series.into(),
        width_ms: WIDTH,
        t,
        min: Some(1.0),
        max: Some(2.0),
        sum: 3.0,
        num_count: 2,
        count: 2,
        last: json!(2.0),
        last_ts: t + 1,
        first: json!(1.0),
        first_ts: Some(t),
    }
}

/// Register `series` (the GC only visits series the meta table knows) and store `n` rollup rows on
/// the tier grid, oldest at `t = WIDTH`.
async fn seed(store: &Store, ws: &str, series: &str, n: u64) {
    write(store, ws, &[sample(series, 1, WIDTH)], 0)
        .await
        .unwrap();
    loop {
        // `drained()`, not `committed` — see `series_retention_test.rs`.
        if commit_batch(store, ws, 256).await.unwrap().drained() == 0 {
            break;
        }
    }
    let rows: Vec<RollupRow> = (1..=n).map(|i| rollup_row(series, i * WIDTH)).collect();
    write_rollups(store, ws, &rows).await.unwrap();
}

fn policy(max_rows: u64) -> Policy {
    Policy {
        prefix: "m.".into(),
        raw_for_ms: 0,
        max_samples: 0,
        // `keep_for_ms: 0` — keep forever by TIME, which is the shipped modbus default and the
        // configuration under which the tier was unbounded on a dead clock.
        tiers: vec![Tier {
            width_ms: WIDTH,
            keep_for_ms: 0,
            max_rows,
            ..Default::default()
        }],
        ..Default::default()
    }
}

/// THE test this feature exists for: the clock is at 0 — behind every row on disc, the state a
/// no-RTC power cycle produces — so every time horizon no-ops, and the tier must STILL be bounded.
/// Fails without `cap_rollup_rows` in `run_gc`: all 50 rows survive and the store grows forever.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_dead_clock_cannot_grow_a_capped_tier() {
    let store = Store::memory().await.unwrap();
    seed(&store, "nube", "m.s", 50).await;
    set_policy(&store, "nube", &policy(5)).await.unwrap();

    let pass = run_gc(&store, "nube", 0).await.unwrap();

    assert_eq!(pass.capped_rollup, 45, "the 45 oldest rows are evicted");
    assert_eq!(
        pass.evicted_rollup, 0,
        "the TIME horizon evicted nothing — the clock is dead"
    );

    // WHICH rows survived, not merely how many: the NEWEST 5 by the rows' own `t` axis.
    let left = read_rollups(&store, "nube", "m.s", 0, 1_000_000_000)
        .await
        .unwrap();
    let mut ts: Vec<u64> = left.iter().map(|r| r.t).collect();
    ts.sort_unstable();
    assert_eq!(ts, (46..=50u64).map(|i| i * WIDTH).collect::<Vec<_>>());
}

/// The cap must be observable AFTER the pass returns, not merely on its return value: an operator
/// discovers a capped tier through `series.retention.status`, which reads the persisted row, and
/// every other test here asserts on the in-memory `GcPass`. `last_pass` projects its columns BY
/// NAME, so a field added to `GcPassRecord` and omitted from that projection reads back as its
/// serde default forever — 0 evictions — with the row on disc perfectly correct. That is the
/// silent-drop failure this feature exists to prevent, reappearing on the one surface that reports
/// it (issue #65's observable-never-silent rule).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_capped_pass_is_observable_on_the_persisted_record() {
    let store = Store::memory().await.unwrap();
    seed(&store, "nube", "m.s", 50).await;
    set_policy(&store, "nube", &policy(5)).await.unwrap();

    let pass = run_gc(&store, "nube", 0).await.unwrap();
    assert_eq!(pass.capped_rollup, 45, "the pass itself counted the cap");

    let rec = last_pass(&store, "nube")
        .await
        .unwrap()
        .expect("run_gc records a pass");
    assert_eq!(
        rec.capped_rollup, 45,
        "the persisted record must carry the SAME count the pass returned"
    );
}

/// `max_rows: 0` is unbounded — the default, and the exact meaning of every tier written before
/// the field existed ("the rollup is kept forever" stays true unless the operator says otherwise).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn zero_keeps_the_tier_unbounded() {
    let store = Store::memory().await.unwrap();
    seed(&store, "nube", "m.s", 50).await;
    set_policy(&store, "nube", &policy(0)).await.unwrap();

    let pass = run_gc(&store, "nube", 0).await.unwrap();

    assert_eq!(pass.capped_rollup, 0);
    let left = read_rollups(&store, "nube", "m.s", 0, 1_000_000_000)
        .await
        .unwrap();
    assert_eq!(left.len(), 50, "keep-forever is honoured as written");
}

/// A tier at or under its cap is untouched — the cap converges, it does not churn.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_under_cap_tier_is_untouched() {
    let store = Store::memory().await.unwrap();
    seed(&store, "nube", "m.s", 5).await;
    set_policy(&store, "nube", &policy(5)).await.unwrap();

    let pass = run_gc(&store, "nube", 0).await.unwrap();

    assert_eq!(pass.capped_rollup, 0);
    let left = read_rollups(&store, "nube", "m.s", 0, 1_000_000_000)
        .await
        .unwrap();
    assert_eq!(left.len(), 5);
}

/// The workspace wall: capping `nube` must not touch the same series name in `globex`.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_cap_stops_at_the_workspace_wall() {
    let store = Store::memory().await.unwrap();
    seed(&store, "nube", "m.s", 50).await;
    seed(&store, "globex", "m.s", 50).await;
    set_policy(&store, "nube", &policy(5)).await.unwrap();

    run_gc(&store, "nube", 0).await.unwrap();

    let other = read_rollups(&store, "globex", "m.s", 0, 1_000_000_000)
        .await
        .unwrap();
    assert_eq!(other.len(), 50, "the neighbouring workspace is untouched");
}
