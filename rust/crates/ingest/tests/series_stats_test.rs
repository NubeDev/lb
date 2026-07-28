//! `series_stats` against the real store (series-observability scope): what one series actually
//! holds — raw count, wall-clock extent, the producers writing to it, and the per-tier rollup
//! breakdown after a real GC pass.
//!
//! Every assertion here goes through the REAL write path (`write` + `commit_batch`) and the REAL
//! `run_gc`, then reads the REAL stored rows back. No fixtures, no fakes.
//!
//! **Load-bearing:** a series with no rows is a valid measurement, not an error — an operator
//! looking at a never-written series must see zeroes, not a failure, and never `first_ts: Some(0)`
//! (which renders as 1970).

use lb_ingest::{commit_batch, run_gc, series_stats, set_policy, write, Policy, Qos, Sample, Tier};
use lb_store::Store;
use serde_json::json;

fn sample(series: &str, producer: &str, seq: u64, ts: u64, payload: serde_json::Value) -> Sample {
    Sample {
        series: series.into(),
        producer: producer.into(),
        ts,
        seq,
        payload,
        labels: json!({}),
        qos: Qos::BestEffort,
    }
}

async fn seed(store: &Store, ws: &str, samples: Vec<Sample>) {
    write(store, ws, &samples, 0).await.unwrap();
    loop {
        // `drained()`, not `committed` — see `series_retention_test.rs`.
        if commit_batch(store, ws, 256).await.unwrap().drained() == 0 {
            break;
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn stats_count_extent_and_every_producer_of_one_series() {
    let store = Store::memory().await.unwrap();
    // TWO producers writing into ONE series — the case that makes `producers` a set rather than a
    // field: `seq` is monotonic per (series, producer) only, so both write seq 1..=10 and the rows
    // must not collide.
    let mut samples: Vec<Sample> = (1..=10u64)
        .map(|i| sample("cpu", "edge-a", i, i * 1000, json!(i as f64)))
        .collect();
    samples
        .extend((1..=10u64).map(|i| sample("cpu", "edge-b", i, (i + 100) * 1000, json!(i as f64))));
    seed(&store, "acme", samples).await;

    let stats = series_stats(&store, "acme", "cpu").await.unwrap();
    assert_eq!(stats.series, "cpu");
    assert_eq!(stats.raw_count, 20, "both producers' rows are counted");
    assert_eq!(
        stats.first_ts,
        Some(1_000),
        "extent is the seeded epoch-ms, oldest first"
    );
    assert_eq!(stats.last_ts, Some(110_000), "…and newest last");
    assert_eq!(
        stats.producers,
        vec!["edge-a".to_string(), "edge-b".to_string()],
        "every producer with a raw row, sorted and deduped"
    );
    assert_eq!(stats.rollup_rows, 0, "no GC has run: nothing rolled up");
    assert!(stats.tiers.is_empty());

    // A DIFFERENT series in the same workspace is not folded in.
    seed(
        &store,
        "acme",
        (1..=3u64)
            .map(|i| sample("mem", "edge-a", i, i * 1000, json!(i as f64)))
            .collect(),
    )
    .await;
    let stats = series_stats(&store, "acme", "cpu").await.unwrap();
    assert_eq!(stats.raw_count, 20, "counts are narrowed by series");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_never_written_series_is_zeroes_not_an_error() {
    let store = Store::memory().await.unwrap();
    seed(
        &store,
        "acme",
        (1..=5u64)
            .map(|i| sample("cpu", "p", i, i * 1000, json!(i as f64)))
            .collect(),
    )
    .await;

    // NOT `unwrap_err`: an unknown series is a valid measurement of nothing. If this ever becomes an
    // error the UI can no longer distinguish "no data yet" from "the read failed".
    let stats = series_stats(&store, "acme", "never.written")
        .await
        .expect("an unknown series is Ok, never an error");
    assert_eq!(stats.series, "never.written", "the subject is echoed back");
    assert_eq!(stats.raw_count, 0);
    assert_eq!(stats.rollup_rows, 0);
    assert!(stats.tiers.is_empty());
    assert_eq!(
        stats.first_ts, None,
        "None, never Some(0) — a zero extent renders as 1970"
    );
    assert_eq!(stats.last_ts, None);
    assert!(
        stats.producers.is_empty(),
        "empty is honest: no rows, so no producer"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rollup_rows_break_down_per_tier_after_a_real_gc() {
    let store = Store::memory().await.unwrap();
    // 700 samples at 1s cadence — three `commit_batch(…, 256)` rounds, so the drain loop iterates.
    seed(
        &store,
        "acme",
        (0..700u64)
            .map(|i| sample("hist", "p", i + 1, i * 1000, json!(i as f64)))
            .collect(),
    )
    .await;

    // TWO tiers: 10s and 60s. The whole point of the per-tier breakdown is that one history is
    // stored at two resolutions, so a bare total would double-count it.
    set_policy(
        &store,
        "acme",
        &Policy {
            prefix: "hist".into(),
            raw_for_ms: 100_000,
            max_samples: 0,
            tiers: vec![
                Tier {
                    width_ms: 60_000,
                    keep_for_ms: 0,
                    method: None,
                    ..Default::default()
                },
                Tier {
                    width_ms: 10_000,
                    keep_for_ms: 0,
                    method: None,
                    ..Default::default()
                },
            ],
            filter: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let pass = run_gc(&store, "acme", 700_000).await.unwrap();
    assert!(pass.rollup_rows > 0, "the pass rolled up: {pass:?}");

    let stats = series_stats(&store, "acme", "hist").await.unwrap();
    assert!(stats.rollup_rows > 0, "rollup rows are visible in stats");
    assert_eq!(
        stats.tiers.len(),
        2,
        "one entry per tier width: {:?}",
        stats.tiers
    );
    assert_eq!(
        stats.tiers.iter().map(|t| t.width_ms).collect::<Vec<_>>(),
        vec![10_000, 60_000],
        "ascending by width — the finest tier reads first"
    );
    assert_eq!(
        stats.tiers.iter().map(|t| t.rows).sum::<u64>(),
        stats.rollup_rows,
        "`rollup_rows` is exactly the sum of the per-tier row counts"
    );
    assert!(
        stats.tiers[0].rows > stats.tiers[1].rows,
        "the 10s tier holds more rows than the 60s one: {:?}",
        stats.tiers
    );
    // Raw eviction happened underneath: the raw count is now the surviving window, not the seed.
    assert_eq!(
        stats.raw_count, 100,
        "raw newer than the 100s horizon survives"
    );
    assert_eq!(stats.producers, vec!["p".to_string()]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn stats_are_workspace_scoped() {
    let store = Store::memory().await.unwrap();
    // The SAME series name in two workspaces, with different depths.
    seed(
        &store,
        "ws-a",
        (1..=7u64)
            .map(|i| sample("cpu", "pa", i, i * 1000, json!(i as f64)))
            .collect(),
    )
    .await;
    seed(
        &store,
        "ws-b",
        (1..=3u64)
            .map(|i| sample("cpu", "pb", i, i * 1000, json!(i as f64)))
            .collect(),
    )
    .await;

    let a = series_stats(&store, "ws-a", "cpu").await.unwrap();
    let b = series_stats(&store, "ws-b", "cpu").await.unwrap();
    assert_eq!(a.raw_count, 7, "ws-a reports its own rows");
    assert_eq!(b.raw_count, 3, "ws-b reports its own rows — never ws-a's");
    assert_eq!(a.producers, vec!["pa".to_string()]);
    assert_eq!(
        b.producers,
        vec!["pb".to_string()],
        "the producer set does not leak across the workspace wall"
    );
    assert_eq!(a.last_ts, Some(7_000));
    assert_eq!(b.last_ts, Some(3_000));

    // A third, never-touched workspace sees nothing at all.
    let c = series_stats(&store, "ws-c", "cpu").await.unwrap();
    assert_eq!(c.raw_count, 0);
    assert_eq!(c.last_ts, None);
}
