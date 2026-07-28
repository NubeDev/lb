//! The RETENTION half of the series plane, against the real store (series-retention scope #58 +
//! series-sample-cap #65): rollup-then-evict on the time horizon, the `max_samples` FIFO count cap,
//! how the two axes compose, longest-prefix-wins, the unpoliced-series warning, and the workspace
//! wall around all of it.
//!
//! Paging, decimation, cardinality and the `series_latest` pointer stay in `series_plane_test.rs`;
//! the write-time filters and tier methods this later grew are in `series_filter*_test.rs` /
//! `series_method*_test.rs`.
//!
//! **Load-bearing:** eviction orders by `ts`, NEVER by raw `seq` — `seq` is monotonic per
//! `(series, producer)` only, and ordering by it across producers is exactly what caused #63.

use lb_ingest::{
    commit_batch, over_cap_warning, read_buckets, read_page, run_gc, sample_count, set_policy,
    write, BucketQuery, PageQuery, Policy, Qos, Sample, Tier, DEFAULT_MAX_SAMPLES,
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

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn retention_gc_rolls_up_then_evicts_and_buckets_merge_rollups() {
    let store = Store::memory().await.unwrap();
    // 700 samples at 1s cadence starting at t=0; value = seq.
    //
    // 700, not 200: `seed` above is a LOOP over `commit_batch(…, 256)`, and a backlog under one
    // batch never makes it iterate — the blind spot that let the fully-filtered-batch stall ship
    // green (`debugging/ingest/filtered-batch-stops-the-drain-loop.md`, testing-scope §3.2). Three
    // batches force the loop round, so a broken termination condition is visible here too.
    seed(
        &store,
        "acme",
        (0..700u64)
            .map(|i| sample("hist", "p", i + 1, i * 1000, json!(i as f64)))
            .collect(),
    )
    .await;

    // Keep raw 100s; roll everything older into 10s buckets kept forever.
    set_policy(
        &store,
        "acme",
        &Policy {
            prefix: "hist".into(),
            raw_for_ms: 100_000,
            max_samples: 0,
            tiers: vec![Tier {
                width_ms: 10_000,
                keep_for_ms: 0,
                method: None,
                ..Default::default()
            }],
            filter: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let now = 700_000u64; // raw cutoff = 600_000, already tier-aligned
    let pass = run_gc(&store, "acme", now).await.unwrap();
    assert_eq!(
        pass.evicted_raw, 600,
        "raw older than the horizon is evicted"
    );
    assert_eq!(pass.rollup_rows, 60, "60× 10s rollup buckets stored");

    // Raw reads no longer see the evicted history…
    let page = read_page(&store, "acme", "hist", &PageQuery::default())
        .await
        .unwrap();
    assert_eq!(page.rows.len(), 100);
    assert_eq!(page.rows[0].seq, 601);

    // …but a bucketed read over the FULL window still covers it via the rollup tier.
    let q = BucketQuery {
        from_ts: 0,
        to_ts: 700_000,
        width_ms: Some(20_000),
        budget: None,
        ..Default::default()
    };
    let buckets = read_buckets(&store, "acme", "hist", &q, 20_000)
        .await
        .unwrap();
    assert_eq!(
        buckets.len(),
        35,
        "full window: rollup-backed history + live raw"
    );
    let first = &buckets[0]; // t=0..20s, values 0..=19 — served entirely from rollups
    assert_eq!(first.min, Some(0.0));
    assert_eq!(first.max, Some(19.0));
    assert_eq!(first.count, 20);
    assert!(
        (first.avg.unwrap() - 9.5).abs() < 1e-9,
        "exact re-aggregation (sum+count)"
    );

    // A second pass is idempotent: nothing left to evict or newly roll up beyond the same rows.
    let pass2 = run_gc(&store, "acme", now).await.unwrap();
    assert_eq!(pass2.evicted_raw, 0);

    // Tier eviction: shrink the tier horizon so old rollup rows fall off too.
    set_policy(
        &store,
        "acme",
        &Policy {
            prefix: "hist".into(),
            raw_for_ms: 100_000,
            max_samples: 0,
            tiers: vec![Tier {
                width_ms: 10_000,
                keep_for_ms: 150_000, // rollup rows with t < 550_000 evict at now=700_000
                method: None,
                ..Default::default()
            }],
            filter: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let pass3 = run_gc(&store, "acme", now).await.unwrap();
    assert_eq!(
        pass3.evicted_rollup, 55,
        "tier horizon evicts stale rollup rows"
    );
}

// ---------------------------------------------------------------------------------------------
// The per-series FIFO sample cap (series-sample-cap scope, issue #65).
// ---------------------------------------------------------------------------------------------

/// Every committed `(seq, ts)` of a series, oldest-ts first — the identity assertions below check
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_longest_matching_prefix_governs_a_series() {
    let store = Store::memory().await.unwrap();
    for s in ["fleet.us.a", "fleet.eu.b"] {
        seed(
            &store,
            "acme",
            (1..=30u64)
                .map(|i| sample(s, "p", i, i * 1000, json!(i)))
                .collect(),
        )
        .await;
    }
    // Broad policy: keep 5. Specific override for the EU fleet: keep 20 (a LONGER prefix, LOOSER
    // bound — so "tightest wins" and "longest wins" disagree, and only the latter is correct).
    set_policy(
        &store,
        "acme",
        &Policy {
            prefix: "fleet.".into(),
            raw_for_ms: 0,
            max_samples: 5,
            tiers: vec![],
            filter: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    set_policy(
        &store,
        "acme",
        &Policy {
            prefix: "fleet.eu.".into(),
            raw_for_ms: 0,
            max_samples: 20,
            tiers: vec![],
            filter: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    run_gc(&store, "acme", 1_000_000).await.unwrap();
    assert_eq!(
        sample_count(&store, "acme", "fleet.us.a").await.unwrap(),
        5,
        "only the broad policy matches: its bound applies"
    );
    assert_eq!(
        sample_count(&store, "acme", "fleet.eu.b").await.unwrap(),
        20,
        "the LONGER prefix governs — its looser bound is the override, not overruled by the broad one"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_unpoliced_series_is_warned_about_not_evicted() {
    // The warning predicate itself — the 100k threshold is not exercised by seeding 100k rows.
    assert!(
        over_cap_warning("s", DEFAULT_MAX_SAMPLES + 1, 0).is_some(),
        "unbounded + past the recommended cap → warn"
    );
    assert!(
        over_cap_warning("s", DEFAULT_MAX_SAMPLES + 1, 50).is_none(),
        "a series with a max_samples policy is bounded, not warned"
    );
    assert!(
        over_cap_warning("s", DEFAULT_MAX_SAMPLES, 0).is_none(),
        "at the cap, not past it"
    );

    // And the GC does not evict from an unpoliced series (release 1: advisory only).
    let store = Store::memory().await.unwrap();
    seed(
        &store,
        "acme",
        (1..=30u64)
            .map(|i| sample("unpoliced", "p", i, i * 1000, json!(i)))
            .collect(),
    )
    .await;
    let pass = run_gc(&store, "acme", 1_000_000).await.unwrap();
    assert_eq!(
        pass.capped_raw, 0,
        "no policy → nothing evicted in release 1"
    );
    assert_eq!(sample_count(&store, "acme", "unpoliced").await.unwrap(), 30);
}

// ── The series_latest POINTER: forward-only, restart-safe, replay-idempotent (perf fix) ─────────
// `latest`/`latest_many` read a materialized newest-pointer (schema::SERIES_LATEST_TABLE) advanced
// transactionally by the commit worker, so they are point lookups not an O(rows) ordered scan. These
// pin the pointer's correctness contract against the real store — the exact corners the ts-primary,
// forward-only advance exists to get right.
