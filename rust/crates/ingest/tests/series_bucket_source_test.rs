//! Per-bucket PROVENANCE over a real GC fold — which table each bucket was actually built from.
//!
//! A merged bucketed read draws from two tables (`series` and `series_rollup`) and returns one flat
//! wire shape. Before `Bucket::source`, a bucket folded from an evicted tier was byte-identical to
//! one folded from live raw, so no caller could tell "this window is empty by retention policy"
//! from "this read is broken" — the ambiguity that makes an empty chart unreadable.
//!
//! Every assertion here runs against rows a REAL `run_gc` pass wrote and real eviction left behind.
//! A provenance test over an empty rollup table proves nothing: `Raw` is the default, so it would
//! pass while the rollup half of the feature was entirely absent.

use lb_ingest::{
    commit_batch, read_buckets, read_buckets_fold, read_rollups, run_gc, sample_count, set_policy,
    write, Bucket, BucketQuery, Policy, Qos, Sample, Source, Tier,
};
use lb_store::Store;
use serde_json::{json, Value};

fn sample_at(series: &str, seq: u64, ts: u64, payload: Value) -> Sample {
    Sample {
        series: series.into(),
        producer: "p".into(),
        ts,
        seq,
        payload,
        labels: json!({}),
        qos: Qos::BestEffort,
    }
}

async fn seed(store: &Store, ws: &str, samples: Vec<Sample>) {
    write(store, ws, &samples, 0).await.unwrap();
    while commit_batch(store, ws, 256).await.unwrap().drained() != 0 {}
}

/// A policy with one tier at `width`, keeping raw for `raw_for_ms` and rollups forever.
async fn tiered(store: &Store, ws: &str, prefix: &str, raw_for_ms: u64, width: u64) {
    set_policy(
        store,
        ws,
        &Policy {
            prefix: prefix.into(),
            raw_for_ms,
            max_samples: 0,
            tiers: vec![Tier {
                width_ms: width,
                keep_for_ms: 0, // keep forever
                method: None,
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

/// Raw that the GC has NOT touched reads as `Raw`, and the split counts agree with the total.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn live_raw_buckets_report_raw() {
    let store = Store::memory().await.unwrap();
    let samples = (1..=5u64)
        .map(|i| sample_at("m.v", i, i * 1_000, json!(i as f64)))
        .collect();
    seed(&store, "nube", samples).await;

    let b = buckets(&store, "nube", "m.v", 0, 10_000, 10_000).await;
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].source, Source::Raw);
    assert_eq!(b[0].raw_count, 5);
    assert_eq!(b[0].rollup_count, 0);
    assert_eq!(b[0].raw_count + b[0].rollup_count, b[0].count);
}

/// After a fold that evicted the raw beneath it, the SAME window reads as `Rollup` — the state a
/// long-horizon history read actually sees.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn folded_and_evicted_buckets_report_rollup() {
    let store = Store::memory().await.unwrap();
    let samples = (1..=5u64)
        .map(|i| sample_at("m.v", i, i * 1_000, json!(i as f64)))
        .collect();
    seed(&store, "nube", samples).await;

    tiered(&store, "nube", "m.", 1, 10_000).await;
    run_gc(&store, "nube", 30_000).await.unwrap();
    assert_eq!(
        sample_count(&store, "nube", "m.v").await.unwrap(),
        0,
        "raw evicted"
    );
    assert_eq!(
        read_rollups(&store, "nube", "m.v", 0, 30_000)
            .await
            .unwrap()
            .len(),
        1
    );

    let b = buckets(&store, "nube", "m.v", 0, 10_000, 10_000).await;
    assert_eq!(b.len(), 1);
    assert_eq!(
        b[0].source,
        Source::Rollup,
        "raw is gone; this row came off the tier"
    );
    assert_eq!(b[0].rollup_count, 5);
    assert_eq!(b[0].raw_count, 0);
    // The stat set survives the fold exactly — provenance rides ALONGSIDE the data, not instead.
    assert_eq!(b[0].min, Some(1.0));
    assert_eq!(b[0].max, Some(5.0));
    assert_eq!(b[0].avg, Some(3.0));
    assert_eq!(b[0].count, 5);
}

/// A read bucket WIDER than the tier can span the eviction boundary, absorbing both an evicted tier
/// row and raw that is still live. That bucket is `Mixed` — reporting it as either pure source
/// would be a lie in one direction, and the split counts say exactly how much came from where.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_bucket_straddling_the_boundary_reports_mixed() {
    let store = Store::memory().await.unwrap();
    // Old half (0–10s) will be folded + evicted; new half (10–20s) stays raw.
    let mut samples: Vec<Sample> = (1..=5u64)
        .map(|i| sample_at("m.v", i, i * 1_000, json!(i as f64)))
        .collect();
    samples.extend(
        (1..=3u64).map(|i| sample_at("m.v", 10 + i, 10_000 + i * 1_000, json!(100.0 * i as f64))),
    );
    seed(&store, "nube", samples).await;

    // `now` = 20s with raw kept for 10s puts the horizon at 10s, which is exactly a tier-grid
    // boundary — so the 0–10s bucket folds and evicts while the 10–20s raw survives. (A horizon
    // mid-bucket floors DOWN to the previous boundary and would evict nothing; `evict_cutoff` only
    // lets raw go as far as the least-advanced tier actually reached.)
    tiered(&store, "nube", "m.", 10_000, 10_000).await;
    run_gc(&store, "nube", 20_000).await.unwrap();
    assert_eq!(
        sample_count(&store, "nube", "m.v").await.unwrap(),
        3,
        "only the pre-horizon half was evicted"
    );

    // ONE 20s read bucket over both halves.
    let b = buckets(&store, "nube", "m.v", 0, 20_000, 20_000).await;
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].source, Source::Mixed);
    assert_eq!(b[0].rollup_count, 5, "the evicted half");
    assert_eq!(b[0].raw_count, 3, "the surviving half");
    assert_eq!(b[0].count, 8);
    // Nothing double-counted: 5 + 3, not 5 + 8.
    assert_eq!(b[0].raw_count + b[0].rollup_count, b[0].count);
}

/// The pushdown and the fold oracle must agree on provenance too — the oracle is the parity test
/// for the pushdown, so a `source` the two disagree on would silently split the two read paths.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn pushdown_and_fold_oracle_agree_on_source() {
    let store = Store::memory().await.unwrap();
    let mut samples: Vec<Sample> = (1..=5u64)
        .map(|i| sample_at("m.v", i, i * 1_000, json!(i as f64)))
        .collect();
    samples.extend(
        (1..=3u64).map(|i| sample_at("m.v", 10 + i, 10_000 + i * 1_000, json!(100.0 * i as f64))),
    );
    seed(&store, "nube", samples).await;

    tiered(&store, "nube", "m.", 10_000, 10_000).await;
    run_gc(&store, "nube", 20_000).await.unwrap();

    let q = BucketQuery {
        from_ts: 0,
        to_ts: 20_000,
        width_ms: Some(10_000),
        budget: None,
        ..Default::default()
    };
    let pushed = read_buckets(&store, "nube", "m.v", &q, 10_000)
        .await
        .unwrap();
    let folded = read_buckets_fold(&store, "nube", "m.v", &q, 10_000)
        .await
        .unwrap();

    assert_eq!(pushed.len(), folded.len());
    for (p, f) in pushed.iter().zip(folded.iter()) {
        assert_eq!(p.t, f.t);
        assert_eq!(p.source, f.source, "bucket {} disagreed on source", p.t);
        assert_eq!(
            p.raw_count, f.raw_count,
            "bucket {} disagreed on raw_count",
            p.t
        );
        assert_eq!(
            p.rollup_count, f.rollup_count,
            "bucket {} disagreed on rollup_count",
            p.t
        );
    }
    // And the split is the real one: the older bucket off the tier, the newer off live raw.
    assert_eq!(pushed[0].source, Source::Rollup);
    assert_eq!(pushed[1].source, Source::Raw);
}
