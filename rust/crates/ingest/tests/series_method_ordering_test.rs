//! The ORDERING axis of the tier methods (series-normalize scope): every "first/last/nearest" is
//! by `(ts, seq-within-producer)` and NEVER by raw `seq` across producers — the axis already burned
//! once (`debugging/ingest/latest-pinned-to-pre-restart-sample.md`), so the multi-producer case is
//! mandatory. Plus the pushdown-vs-fold-oracle parity on the new `first` representative.
//!
//! The methods' values are `series_method_test.rs`. Real store, no mocks (testing §0).

use lb_ingest::{
    apply_method, commit_direct, read_buckets, run_gc, sample_count, set_policy, Bucket,
    BucketQuery, Method, Policy, Qos, Sample, Tier,
};
use lb_store::Store;
use serde_json::{json, Value};

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

async fn seed(store: &Store, ws: &str, samples: Vec<Sample>) {
    commit_direct(store, ws, &samples).await.unwrap();
}

/// A policy with one tier at `width`, keeping raw for `raw_for_ms`.
async fn tiered(
    store: &Store,
    ws: &str,
    prefix: &str,
    raw_for_ms: u64,
    width: u64,
    method: Option<Method>,
) {
    set_policy(
        store,
        ws,
        &Policy {
            prefix: prefix.into(),
            raw_for_ms,
            max_samples: 0,
            tiers: vec![Tier {
                width_ms: width,
                keep_for_ms: 0,
                method,
                ..Default::default()
            }],
            filter: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();
}

async fn buckets(
    store: &Store,
    ws: &str,
    series: &str,
    from: u64,
    to: u64,
    width: u64,
) -> Vec<Bucket> {
    let q = BucketQuery {
        from_ts: from,
        to_ts: to,
        width_ms: Some(width),
        budget: None,
        ..Default::default()
    };
    read_buckets(store, ws, series, &q, width).await.unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ordering_is_by_ts_then_seq_within_producer_never_raw_seq_across_producers() {
    // The mandatory multi-producer test. Producer `late` has LOW seqs but LATE timestamps; producer
    // `early` has HIGH seqs but EARLY timestamps. Ordering the bucket by raw `seq` would report
    // `late`'s value as first and `early`'s as last — exactly backwards.
    let store = Store::memory().await.unwrap();
    seed(
        &store,
        "nube",
        vec![
            sample_at("o.v", "early", 900, 1_000, json!(11.0)), // earliest ts, HIGHEST seq
            sample_at("o.v", "early", 901, 2_000, json!(22.0)),
            sample_at("o.v", "late", 1, 8_000, json!(88.0)),
            sample_at("o.v", "late", 2, 9_000, json!(99.0)), // latest ts, LOWEST seq
        ],
    )
    .await;

    // Over live raw first…
    let mut b = buckets(&store, "nube", "o.v", 0, 10_000, 10_000).await;
    apply_method(&mut b, Method::First).unwrap();
    assert_eq!(
        b[0].value.as_ref().unwrap(),
        &json!(11.0),
        "first is by TS, not by seq"
    );
    apply_method(&mut b, Method::Last).unwrap();
    assert_eq!(
        b[0].value.as_ref().unwrap(),
        &json!(99.0),
        "last is by TS, not by seq"
    );

    // …and identically after the fold + eviction, off the stored representatives.
    tiered(&store, "nube", "o.", 1, 10_000, Some(Method::First)).await;
    run_gc(&store, "nube", 40_000).await.unwrap();
    assert_eq!(sample_count(&store, "nube", "o.v").await.unwrap(), 0);

    let mut folded = buckets(&store, "nube", "o.v", 0, 10_000, 10_000).await;
    apply_method(&mut folded, Method::First).unwrap();
    assert_eq!(folded[0].value.as_ref().unwrap(), &json!(11.0));
    apply_method(&mut folded, Method::Last).unwrap();
    assert_eq!(folded[0].value.as_ref().unwrap(), &json!(99.0));
    assert_eq!(folded[0].count, 4);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_pushdown_and_the_fold_oracle_agree_on_the_first_representative() {
    // The parity contract: `read_buckets`' pushed-down GROUP BY must be byte-identical to the
    // in-Rust fold. `first` is new on both sides, so it gets the same guard the rest of the row has.
    let store = Store::memory().await.unwrap();
    seed(
        &store,
        "nube",
        vec![
            sample_at("p.v", "a", 5, 1_500, json!(1.0)),
            sample_at("p.v", "b", 1, 1_000, json!(2.0)), // earliest ts, different producer
            sample_at("p.v", "a", 6, 9_000, json!(3.0)),
            sample_at("p.v", "c", 3, 12_000, json!(4.0)),
        ],
    )
    .await;

    let q = BucketQuery {
        from_ts: 0,
        to_ts: 20_000,
        width_ms: Some(10_000),
        budget: None,
        ..Default::default()
    };
    let pushed = read_buckets(&store, "nube", "p.v", &q, 10_000)
        .await
        .unwrap();
    let folded = lb_ingest::read_buckets_fold(&store, "nube", "p.v", &q, 10_000)
        .await
        .unwrap();

    assert_eq!(pushed.len(), folded.len());
    for (p, f) in pushed.iter().zip(folded.iter()) {
        assert_eq!(p.t, f.t);
        assert_eq!(p.first, f.first, "first parity at t={}", p.t);
        assert_eq!(p.first_ts, f.first_ts, "first_ts parity at t={}", p.t);
        assert_eq!(p.last, f.last);
        assert_eq!(p.count, f.count);
        assert_eq!(p.avg, f.avg);
    }
    assert_eq!(
        pushed[0].first,
        json!(2.0),
        "the earliest TS wins across producers"
    );
}
