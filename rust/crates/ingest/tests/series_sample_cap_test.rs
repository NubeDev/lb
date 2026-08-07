//! The `max_samples` FIFO count cap (series-sample-cap scope, issue #65) against the real store:
//! oldest-first eviction, the ordering axis, `0` = unbounded, the GC reporting what it capped,
//! how the count bound composes with the time horizon, rolling up before evicting so bucket reads
//! survive, and the workspace wall.
//!
//! Time-horizon rollup/eviction and prefix precedence are in `series_retention_test.rs`.
//!
//! **Load-bearing:** the cap evicts by `ts`, NEVER by raw `seq` — `seq` is monotonic per
//! `(series, producer)` only, and ordering by it across producers is exactly what caused #63.

use lb_ingest::{
    cap_series, commit_batch, read_buckets, read_page, run_gc, sample_count, set_policy, write,
    BucketQuery, PageQuery, Policy, Qos, Sample, Tier,
};
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
        // `drained()`, not `committed` — a fully-filtered batch commits nothing while consuming a
        // whole batch, and stopping there would leave staging half-drained (see
        // `debugging/ingest/filtered-batch-stops-the-drain-loop.md`).
        if commit_batch(store, ws, 256).await.unwrap().drained() == 0 {
            break;
        }
    }
}

/// WHICH rows survived, not merely how many. A cap that keeps the wrong M is worse than no cap.
async fn rows_by_ts(store: &Store, ws: &str, series: &str) -> Vec<(u64, u64)> {
    let page = read_page(
        store,
        ws,
        series,
        &PageQuery {
            limit: Some(10_000),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let mut rows: Vec<(u64, u64)> = page.rows.iter().map(|s| (s.ts, s.seq)).collect();
    rows.sort();
    rows
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_cap_evicts_oldest_first_and_keeps_the_newest_m() {
    let store = Store::memory().await.unwrap();
    // 50 samples, ts 1000..=50_000.
    seed(
        &store,
        "nube",
        (1..=50u64)
            .map(|i| sample("cap", "p", i, i * 1000, json!(i)))
            .collect(),
    )
    .await;

    let evicted = cap_series(&store, "nube", "cap", 20).await.unwrap();
    assert_eq!(evicted, 30, "50 - 20 = the 30 oldest are evicted");

    let rows = rows_by_ts(&store, "nube", "cap").await;
    assert_eq!(rows.len(), 20, "exactly the bound remains");
    // The survivors are ts 31_000..=50_000 — the NEWEST 20, not just any 20.
    let expected: Vec<(u64, u64)> = (31..=50u64).map(|i| (i * 1000, i)).collect();
    assert_eq!(
        rows, expected,
        "the newest M survive; the oldest went first"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_cap_orders_by_ts_never_seq_across_producers() {
    let store = Store::memory().await.unwrap();
    let mut samples = Vec::new();
    // OLD data from a long-running producer: high seqs (900..=919), OLD ts (1_000..=20_000).
    for i in 0..20u64 {
        samples.push(sample(
            "mixed",
            "old-prod",
            900 + i,
            1_000 + i * 1_000,
            json!("old"),
        ));
    }
    // NEW data from a producer that just restarted: seq back to 1..=20, NEW ts (100_000..=119_000).
    for i in 0..20u64 {
        samples.push(sample(
            "mixed",
            "new-prod",
            1 + i,
            100_000 + i * 1_000,
            json!("new"),
        ));
    }
    seed(&store, "nube", samples).await;

    // Keep 20 of 40. By `ts` that is exactly the "new" rows; by `seq` it would be the "old" ones.
    let evicted = cap_series(&store, "nube", "mixed", 20).await.unwrap();
    assert_eq!(evicted, 20);

    let page = read_page(
        &store,
        "nube",
        "mixed",
        &PageQuery {
            limit: Some(100),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(page.rows.len(), 20);
    assert!(
        page.rows.iter().all(|s| s.payload == json!("new")),
        "a seq-ordered cap would have evicted the LIVE rows and kept the dead ones"
    );
    assert!(
        page.rows.iter().all(|s| s.producer == "new-prod"),
        "survivors are the restarted producer's rows — newest by the shared ts axis"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn max_samples_zero_is_unbounded() {
    let store = Store::memory().await.unwrap();
    seed(
        &store,
        "nube",
        (1..=30u64)
            .map(|i| sample("keep", "p", i, i * 1000, json!(i)))
            .collect(),
    )
    .await;
    let evicted = cap_series(&store, "nube", "keep", 0).await.unwrap();
    assert_eq!(evicted, 0, "0 = unbounded, the explicit opt-out");
    assert_eq!(sample_count(&store, "nube", "keep").await.unwrap(), 30);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn gc_applies_the_cap_reports_it_and_is_idempotent() {
    let store = Store::memory().await.unwrap();
    seed(
        &store,
        "nube",
        (1..=40u64)
            .map(|i| sample("fleet.a", "p", i, i * 1000, json!(i)))
            .collect(),
    )
    .await;
    set_policy(
        &store,
        "nube",
        &Policy {
            prefix: "fleet.".into(),
            raw_for_ms: 0, // time axis OFF — this proves the COUNT axis stands alone
            max_samples: 10,
            tiers: vec![],
            filter: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let pass = run_gc(&store, "nube", 1_000_000).await.unwrap();
    assert_eq!(pass.capped_raw, 30, "the cap reports what it evicted");
    assert_eq!(
        pass.evicted_raw, 0,
        "the time horizon is off; this was the cap"
    );
    assert_eq!(sample_count(&store, "nube", "fleet.a").await.unwrap(), 10);

    let pass2 = run_gc(&store, "nube", 1_000_000).await.unwrap();
    assert_eq!(
        pass2.capped_raw, 0,
        "a second pass evicts nothing (idempotent)"
    );
    assert_eq!(sample_count(&store, "nube", "fleet.a").await.unwrap(), 10);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn cap_composes_with_the_time_horizon() {
    let store = Store::memory().await.unwrap();
    // 100 samples at 1s cadence, ts 0..=99_000.
    seed(
        &store,
        "nube",
        (0..100u64)
            .map(|i| sample("both", "p", i + 1, i * 1000, json!(i)))
            .collect(),
    )
    .await;
    // now=100_000, raw_for_ms=50_000 → the time horizon alone would keep ts >= 50_000 (50 rows).
    // max_samples=10 is TIGHTER, so the cap bites and only 10 survive.
    set_policy(
        &store,
        "nube",
        &Policy {
            prefix: "both".into(),
            raw_for_ms: 50_000,
            max_samples: 10,
            tiers: vec![],
            filter: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let pass = run_gc(&store, "nube", 100_000).await.unwrap();
    assert_eq!(pass.evicted_raw, 50, "the time horizon took the oldest 50");
    assert_eq!(pass.capped_raw, 40, "the tighter count cap took 40 more");
    let rows = rows_by_ts(&store, "nube", "both").await;
    assert_eq!(rows.len(), 10, "the tighter bound wins");
    assert_eq!(rows[0].0, 90_000, "survivors are the newest 10 by ts");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn cap_rolls_up_before_evicting_so_bucket_reads_survive() {
    let store = Store::memory().await.unwrap();
    // 100 samples at 1s cadence, value = i.
    seed(
        &store,
        "nube",
        (0..100u64)
            .map(|i| sample("roll", "p", i + 1, i * 1000, json!(i as f64)))
            .collect(),
    )
    .await;
    set_policy(
        &store,
        "nube",
        &Policy {
            prefix: "roll".into(),
            raw_for_ms: 0, // count axis only
            max_samples: 10,
            tiers: vec![Tier {
                width_ms: 10_000,
                keep_for_ms: 0, // rollups kept forever
                method: None,
                ..Default::default()
            }],
            filter: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let pass = run_gc(&store, "nube", 1_000_000).await.unwrap();
    assert_eq!(pass.capped_raw, 90);
    assert!(pass.rollup_rows > 0, "the over-cap window rolled up first");
    assert_eq!(sample_count(&store, "nube", "roll").await.unwrap(), 10);

    // A bucketed read over the FULL window still covers the cap-evicted history via the tier.
    let q = BucketQuery {
        from_ts: 0,
        to_ts: 100_000,
        width_ms: Some(10_000),
        budget: None,
        ..Default::default()
    };
    let buckets = read_buckets(&store, "nube", "roll", &q, 10_000)
        .await
        .unwrap();
    assert_eq!(buckets.len(), 10, "cap-evicted history survives as rollups");
    let first = &buckets[0]; // ts 0..10s, values 0..=9 — served entirely from rollups
    assert_eq!(first.min, Some(0.0));
    assert_eq!(first.max, Some(9.0));
    assert_eq!(first.count, 10);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_cap_never_crosses_the_workspace_wall() {
    let store = Store::memory().await.unwrap();
    for ws in ["nube", "globex"] {
        seed(
            &store,
            ws,
            (1..=30u64)
                .map(|i| sample("shared.name", "p", i, i * 1000, json!(i)))
                .collect(),
        )
        .await;
    }
    // The policy exists ONLY in nube.
    set_policy(
        &store,
        "nube",
        &Policy {
            prefix: "shared.".into(),
            raw_for_ms: 0,
            max_samples: 5,
            tiers: vec![],
            filter: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let pass = run_gc(&store, "nube", 1_000_000).await.unwrap();
    assert_eq!(pass.capped_raw, 25);
    assert_eq!(
        sample_count(&store, "nube", "shared.name").await.unwrap(),
        5
    );
    assert_eq!(
        sample_count(&store, "globex", "shared.name").await.unwrap(),
        30,
        "ws-B's identically-named series is untouched by ws-A's GC (the hard wall)"
    );

    // And a GC in globex — which has NO policy — evicts nothing.
    let pass_b = run_gc(&store, "globex", 1_000_000).await.unwrap();
    assert_eq!(pass_b.capped_raw, 0);
    assert_eq!(
        sample_count(&store, "globex", "shared.name").await.unwrap(),
        30
    );
}
