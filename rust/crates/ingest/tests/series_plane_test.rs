//! The series-plane readiness slices, proven against the REAL store (no mocks): keyset paging
//! (exactly-once walk, tiebreaker, clamp, malformed cursor), bucketed decimation (spike survives,
//! bounded budget, last/avg correctness), the series cardinality cap (dead-letter, never silent),
//! label→tag conversion at commit, wall-clock window reads over the datetime `ts`, and retention
//! GC (rollup-then-evict + tier eviction + rollup-backed bucket reads).

use lb_ingest::{
    commit_batch, commit_batch_capped, read_buckets, read_page, run_gc, set_policy, write,
    BucketQuery, Cursor, Direction, PageQuery, Policy, Qos, Sample, Tier, DEAD_LETTER_TABLE,
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
        let pass = commit_batch(store, ws, 256).await.unwrap();
        if pass.committed == 0 && pass.dead_lettered == 0 {
            break;
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn keyset_paging_walks_every_row_exactly_once() {
    let store = Store::memory().await.unwrap();
    // Two producers sharing seqs — the tie the (seq, producer) composite must not skip or repeat.
    let mut samples = Vec::new();
    for seq in 1..=25u64 {
        samples.push(sample("cpu", "prod-a", seq, seq * 1000, json!(seq)));
        samples.push(sample("cpu", "prod-b", seq, seq * 1000, json!(seq * 10)));
    }
    seed(&store, "acme", samples).await;

    let mut seen = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let page = read_page(
            &store,
            "acme",
            "cpu",
            &PageQuery {
                limit: Some(7),
                cursor: cursor.clone(),
                direction: Direction::Fwd,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        seen.extend(page.rows.iter().map(|s| (s.seq, s.producer.clone())));
        match page.next_cursor {
            Some(c) => cursor = Some(c),
            None => break,
        }
    }
    assert_eq!(seen.len(), 50, "every row exactly once, no gaps");
    let mut dedup = seen.clone();
    dedup.dedup();
    assert_eq!(dedup.len(), 50, "no duplicates across pages");
    // Ordered by (seq, producer) ascending.
    let mut sorted = seen.clone();
    sorted.sort();
    assert_eq!(seen, sorted);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn paging_back_direction_and_bad_cursor() {
    let store = Store::memory().await.unwrap();
    seed(
        &store,
        "acme",
        (1..=10u64)
            .map(|s| sample("m", "p", s, s * 1000, json!(s)))
            .collect(),
    )
    .await;

    let page = read_page(
        &store,
        "acme",
        "m",
        &PageQuery {
            limit: Some(3),
            direction: Direction::Back,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let seqs: Vec<u64> = page.rows.iter().map(|s| s.seq).collect();
    assert_eq!(seqs, vec![10, 9, 8], "back pages newest-first");

    // A malformed cursor is rejected cleanly — never a mis-seek.
    let err = read_page(
        &store,
        "acme",
        "m",
        &PageQuery {
            cursor: Some("not-a-cursor!!".into()),
            ..Default::default()
        },
    )
    .await;
    assert!(err.is_err(), "malformed cursor must be rejected");

    // Cursor round-trip is exact.
    let c = Cursor {
        seq: 42,
        producer: "prod:x".into(),
    };
    assert_eq!(Cursor::decode(&c.encode()).unwrap(), c);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn wall_clock_window_bounds_apply() {
    let store = Store::memory().await.unwrap();
    seed(
        &store,
        "acme",
        (1..=10u64)
            .map(|s| sample("w", "p", s, s * 1000, json!(s)))
            .collect(),
    )
    .await;
    // Half-open [3000, 7000): ts 3000..=6000 → seqs 3,4,5,6.
    let page = read_page(
        &store,
        "acme",
        "w",
        &PageQuery {
            from_ts: Some(3000),
            to_ts: Some(7000),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let seqs: Vec<u64> = page.rows.iter().map(|s| s.seq).collect();
    assert_eq!(seqs, vec![3, 4, 5, 6]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn buckets_bound_budget_and_spikes_survive() {
    let store = Store::memory().await.unwrap();
    // 600 samples over 10 minutes at 1s cadence, flat ~20.0 with one 3-sample 200.0 spike at ~90s.
    let mut samples = Vec::new();
    for i in 0..600u64 {
        let v = if (90..93).contains(&i) { 200.0 } else { 20.0 };
        samples.push(sample("temp", "p", i + 1, i * 1000, json!(v)));
    }
    seed(&store, "acme", samples).await;

    let q = BucketQuery {
        from_ts: 0,
        to_ts: 600_000,
        width_ms: Some(60_000), // 1-minute buckets → 10 buckets
        budget: None,
    };
    let buckets = read_buckets(&store, "acme", "temp", &q, 60_000)
        .await
        .unwrap();
    assert_eq!(buckets.len(), 10, "bounded: 10 buckets, never 600 rows");

    let spike = buckets
        .iter()
        .find(|b| b.t == 60_000)
        .expect("spike bucket");
    assert_eq!(spike.max, Some(200.0), "the spike survives in max");
    assert_eq!(spike.min, Some(20.0));
    let avg = spike.avg.unwrap();
    assert!(avg < 40.0, "avg alone would have hidden the spike ({avg})");
    assert!(spike.min.unwrap() <= avg && avg <= spike.max.unwrap());
    assert_eq!(spike.count, 60);
    // `last` is the chronologically last sample of the bucket (ts 119s → 20.0).
    assert_eq!(spike.last, json!(20.0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn series_cardinality_cap_dead_letters_new_series() {
    let store = Store::memory().await.unwrap();
    // Cap = 2: series a and b are admitted; series c is diverted to the dead-letter table.
    // Distinct seqs → deterministic drain order (a, b, c) — the cap decision is order-dependent.
    let samples = vec![
        sample("a", "p", 1, 1000, json!(1)),
        sample("b", "p", 2, 1000, json!(2)),
        sample("c", "p", 3, 1000, json!(3)),
    ];
    write(&store, "acme", &samples, 0).await.unwrap();
    let pass = commit_batch_capped(&store, "acme", 256, 2).await.unwrap();
    assert_eq!(pass.committed, 2);
    assert_eq!(
        pass.dead_lettered, 1,
        "the over-cap series is diverted, not dropped"
    );

    let got = lb_ingest::read(&store, "acme", "c", None, None)
        .await
        .unwrap();
    assert!(got.is_empty(), "over-cap series has no committed rows");
    let mut resp = store
        .query_ws(
            "acme",
            &format!("SELECT count() FROM {DEAD_LETTER_TABLE} GROUP ALL"),
            vec![],
        )
        .await
        .unwrap();
    let n: Option<i64> = resp.take("count").unwrap();
    assert_eq!(
        n,
        Some(1),
        "the sample is recoverable from the dead-letter table"
    );

    // An EXISTING series is never blocked by the cap.
    write(&store, "acme", &[sample("a", "p", 4, 2000, json!(4))], 0)
        .await
        .unwrap();
    let pass = commit_batch_capped(&store, "acme", 256, 2).await.unwrap();
    assert_eq!(pass.committed, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn labels_convert_to_tag_edges_once_per_series() {
    let store = Store::memory().await.unwrap();
    let mut s = sample("floor2/temp", "p", 1, 1000, json!(21.5));
    s.labels = json!({"host": "pi-7", "kind": "telemetry"});
    seed(&store, "acme", vec![s]).await;

    // series.find's primitive: the tag graph now knows the series ingest wrote.
    let hits = lb_tags::find(
        &store,
        "acme",
        &[lb_tags::Facet::exact("host", json!("pi-7"))],
    )
    .await
    .unwrap();
    assert_eq!(hits, vec!["series:floor2/temp".to_string()]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn retention_gc_rolls_up_then_evicts_and_buckets_merge_rollups() {
    let store = Store::memory().await.unwrap();
    // 200 samples at 1s cadence starting at t=0; value = seq.
    seed(
        &store,
        "acme",
        (0..200u64)
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
            tiers: vec![Tier {
                width_ms: 10_000,
                keep_for_ms: 0,
            }],
        },
    )
    .await
    .unwrap();

    let now = 200_000u64; // raw cutoff = 100_000, already tier-aligned
    let pass = run_gc(&store, "acme", now).await.unwrap();
    assert_eq!(
        pass.evicted_raw, 100,
        "raw older than the horizon is evicted"
    );
    assert_eq!(pass.rollup_rows, 10, "10× 10s rollup buckets stored");

    // Raw reads no longer see the evicted half…
    let page = read_page(&store, "acme", "hist", &PageQuery::default())
        .await
        .unwrap();
    assert_eq!(page.rows.len(), 100);
    assert_eq!(page.rows[0].seq, 101);

    // …but a bucketed read over the FULL window still covers it via the rollup tier.
    let q = BucketQuery {
        from_ts: 0,
        to_ts: 200_000,
        width_ms: Some(20_000),
        budget: None,
    };
    let buckets = read_buckets(&store, "acme", "hist", &q, 20_000)
        .await
        .unwrap();
    assert_eq!(
        buckets.len(),
        10,
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
            tiers: vec![Tier {
                width_ms: 10_000,
                keep_for_ms: 150_000, // rollup rows with t < 50_000 evict at now=200_000
            }],
        },
    )
    .await
    .unwrap();
    let pass3 = run_gc(&store, "acme", now).await.unwrap();
    assert_eq!(
        pass3.evicted_rollup, 5,
        "tier horizon evicts stale rollup rows"
    );
}
