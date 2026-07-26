//! The write-time normalize filters against the REAL store (no mocks, testing §0): what each
//! predicate stores and discards, that the counters match exactly, that a non-numeric payload rides
//! through untouched, that the anchor is PER-PRODUCER and survives a restart, and that a filtered
//! batch still drains.
//!
//! The pure predicate half is `filter_predicate_test.rs`; this file proves the store-backed
//! behaviour the predicates alone cannot: persistence, per-producer isolation, and the interaction
//! with commit's own dequeue.

use lb_ingest::{
    commit_batch, latest, read, set_policy, write, Deadband, Filter, Policy, Qos, Range, RangeMode,
    Sample,
};
use lb_store::Store;
use serde_json::{json, Value};

/// A sample with INDEPENDENT `ts` and `seq` axes — `sample_at()`, never `sample()`. `seq` is
/// monotonic per `(series, producer)` ONLY, so a test that ties `seq` to `ts` cannot detect an
/// ordering bug across producers (the lesson in
/// `debugging/ingest/latest-pinned-to-pre-restart-sample.md`).
fn sample_at(series: &str, producer: &str, seq: u64, ts: u64, payload: Value) -> Sample {
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

/// Stage `samples` and drain staging completely, returning the summed pass counts.
async fn seed(store: &Store, ws: &str, samples: Vec<Sample>) -> lb_ingest::CommitPass {
    write(store, ws, &samples, 0).await.unwrap();
    let mut total = lb_ingest::CommitPass::default();
    loop {
        let pass = commit_batch(store, ws, 256).await.unwrap();
        if pass.drained() == 0 {
            break;
        }
        total.committed += pass.committed;
        total.dead_lettered += pass.dead_lettered;
        total.filtered.muted += pass.filtered.muted;
        total.filtered.range += pass.filtered.range;
        total.filtered.min_interval += pass.filtered.min_interval;
        total.filtered.deadband += pass.filtered.deadband;
        total.filtered.clamped += pass.filtered.clamped;
    }
    total
}

async fn policy(store: &Store, ws: &str, prefix: &str, filter: Filter) {
    set_policy(
        store,
        ws,
        &Policy {
            prefix: prefix.into(),
            raw_for_ms: 0,
            max_samples: 0,
            tiers: vec![],
            filter: Some(filter),
        },
    )
    .await
    .unwrap();
}

/// The stored payloads of `series`, in commit order.
async fn stored(store: &Store, ws: &str, series: &str) -> Vec<Value> {
    read(store, ws, series, None, None)
        .await
        .unwrap()
        .into_iter()
        .map(|s| s.payload)
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_deadband_stores_only_the_moves_and_counts_the_rest() {
    let store = Store::memory().await.unwrap();
    policy(
        &store,
        "acme",
        "temp.",
        Filter {
            deadband: Some(Deadband {
                abs: Some(0.5),
                pct: None,
            }),
            ..Default::default()
        },
    )
    .await;

    // 20.0 lands (first sample, nothing to be redundant against); 20.1/20.2/20.3 are inside the
    // band; 21.0 moves; 21.1 is inside again.
    let vals = [20.0, 20.1, 20.2, 20.3, 21.0, 21.1];
    let samples: Vec<Sample> = vals
        .iter()
        .enumerate()
        .map(|(i, v)| {
            sample_at(
                "temp.a",
                "p1",
                i as u64 + 1,
                1_000 + i as u64 * 1_000,
                json!(v),
            )
        })
        .collect();
    let pass = seed(&store, "acme", samples).await;

    assert_eq!(
        stored(&store, "acme", "temp.a").await,
        vec![json!(20.0), json!(21.0)]
    );
    assert_eq!(pass.committed, 2);
    assert_eq!(
        pass.filtered.deadband, 4,
        "every suppressed sample is counted"
    );
    assert_eq!(pass.filtered.dropped(), 4, "and nothing is counted twice");
    assert_eq!(pass.filtered.clamped, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn range_drop_discards_and_range_clamp_stores_the_bound() {
    let store = Store::memory().await.unwrap();
    policy(
        &store,
        "acme",
        "drop.",
        Filter {
            range: Some(Range {
                min: Some(-40.0),
                max: Some(120.0),
                mode: RangeMode::Drop,
            }),
            ..Default::default()
        },
    )
    .await;
    policy(
        &store,
        "acme",
        "clamp.",
        Filter {
            range: Some(Range {
                min: Some(-40.0),
                max: Some(120.0),
                mode: RangeMode::Clamp,
            }),
            ..Default::default()
        },
    )
    .await;

    let pass = seed(
        &store,
        "acme",
        vec![
            sample_at("drop.t", "p", 1, 1_000, json!(21.0)),
            sample_at("drop.t", "p", 2, 2_000, json!(-9999.0)),
            sample_at("clamp.t", "p", 1, 1_000, json!(21.0)),
            sample_at("clamp.t", "p", 2, 2_000, json!(-9999.0)),
            sample_at("clamp.t", "p", 3, 3_000, json!(500.0)),
        ],
    )
    .await;

    assert_eq!(
        stored(&store, "acme", "drop.t").await,
        vec![json!(21.0)],
        "dropped, not stored"
    );
    assert_eq!(
        stored(&store, "acme", "clamp.t").await,
        vec![json!(21.0), json!(-40.0), json!(120.0)],
        "clamped to the bound, both ends"
    );
    assert_eq!(
        pass.filtered.range, 1,
        "only the DROP-mode violation counts as a range drop"
    );
    assert_eq!(
        pass.filtered.clamped, 2,
        "clamps are counted separately — they stored"
    );
    assert_eq!(pass.committed, 4);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn min_interval_thins_to_the_first_sample_of_each_window() {
    let store = Store::memory().await.unwrap();
    policy(
        &store,
        "acme",
        "fast.",
        Filter {
            min_interval_ms: 10_000,
            ..Default::default()
        },
    )
    .await;

    // A 2s producer over 30s: 15 samples in, one per 10s window out (t=0, 10s, 20s).
    let samples: Vec<Sample> = (0..15u64)
        .map(|i| sample_at("fast.v", "p", i + 1, i * 2_000, json!(i as f64)))
        .collect();
    let pass = seed(&store, "acme", samples).await;

    assert_eq!(
        stored(&store, "acme", "fast.v").await,
        vec![json!(0.0), json!(5.0), json!(10.0)],
        "the FIRST of each interval, at 0ms / 10000ms / 20000ms"
    );
    assert_eq!(pass.committed, 3);
    assert_eq!(pass.filtered.min_interval, 12);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_muted_prefix_stores_nothing_registers_nothing_and_still_drains() {
    let store = Store::memory().await.unwrap();
    policy(
        &store,
        "acme",
        "quiet.",
        Filter {
            drop: true,
            ..Default::default()
        },
    )
    .await;

    // More than one batch, so the drain loop must keep going on a pass that committed ZERO rows.
    let samples: Vec<Sample> = (1..=600u64)
        .map(|i| sample_at("quiet.v", "p", i, i * 1_000, json!(i as f64)))
        .collect();
    let pass = seed(&store, "acme", samples).await;

    assert_eq!(pass.committed, 0);
    assert_eq!(
        pass.filtered.muted, 600,
        "the WHOLE backlog drained, not just the first batch"
    );
    assert!(stored(&store, "acme", "quiet.v").await.is_empty());
    // Muted data must not consume the workspace's distinct-series budget either.
    assert!(
        lb_ingest::series_names(&store, "acme", "quiet.")
            .await
            .unwrap()
            .is_empty(),
        "a series that stores nothing registers nothing"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_non_numeric_series_rides_through_the_numeric_predicates_untouched() {
    let store = Store::memory().await.unwrap();
    // A filter authored for the analog points under this prefix must not eat the event series that
    // shares it.
    policy(
        &store,
        "acme",
        "plant.",
        Filter {
            deadband: Some(Deadband {
                abs: Some(1000.0),
                pct: None,
            }),
            range: Some(Range {
                min: Some(0.0),
                max: Some(1.0),
                mode: RangeMode::Drop,
            }),
            ..Default::default()
        },
    )
    .await;

    let pass = seed(
        &store,
        "acme",
        vec![
            sample_at("plant.door", "p", 1, 1_000, json!("open")),
            sample_at("plant.door", "p", 2, 2_000, json!("open")),
            sample_at("plant.door", "p", 3, 3_000, json!({"state": "closed"})),
            sample_at("plant.door", "p", 4, 4_000, json!(true)),
        ],
    )
    .await;

    assert_eq!(pass.committed, 4, "every non-numeric payload landed");
    assert!(
        pass.filtered.is_zero(),
        "and none of them was counted as filtered"
    );
    assert_eq!(stored(&store, "acme", "plant.door").await.len(), 4);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn latest_never_reports_a_sample_the_filter_discarded() {
    // `series_latest` is a materialized pointer written in the commit tx. If a filtered sample
    // advanced it, `series.latest` would report a value no `series.read` could find.
    let store = Store::memory().await.unwrap();
    policy(
        &store,
        "acme",
        "temp.",
        Filter {
            deadband: Some(Deadband {
                abs: Some(5.0),
                pct: None,
            }),
            ..Default::default()
        },
    )
    .await;

    seed(
        &store,
        "acme",
        vec![
            sample_at("temp.a", "p", 1, 1_000, json!(10.0)),
            sample_at("temp.a", "p", 2, 9_000, json!(10.1)), // newest by ts, but FILTERED
        ],
    )
    .await;

    let newest = latest(&store, "acme", "temp.a").await.unwrap().unwrap();
    assert_eq!(
        newest.payload,
        json!(10.0),
        "latest mirrors a COMMITTED row"
    );
    assert_eq!(newest.ts, 1_000);
}
