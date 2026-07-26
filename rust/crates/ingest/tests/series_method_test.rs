//! The per-tier rollup METHOD against the REAL store: each method's value over real folded
//! buckets, `avg` exact across a two-pass re-aggregation (never a mean-of-means), `nearest` snapping
//! across a bucket boundary, and a method the tier never stored erroring instead of approximating.
//!
//! The pure selection logic is unit-tested in `src/method.rs`; this file proves it end to end over
//! rows that a real GC pass wrote and real raw eviction left behind.

use lb_ingest::{
    apply_method, commit_batch, read_buckets, read_rollups, run_gc, sample_count, set_policy,
    write, Bucket, BucketQuery, Method, Policy, Qos, Sample, Tier,
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
    write(store, ws, &samples, 0).await.unwrap();
    while commit_batch(store, ws, 256).await.unwrap().drained() != 0 {}
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
            }],
            filter: None,
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
    };
    read_buckets(store, ws, series, &q, width).await.unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn every_method_reads_its_own_value_over_a_real_folded_tier() {
    let store = Store::memory().await.unwrap();
    // One 10s bucket holding 1..=5 at 1s spacing, then a second bucket so the fold has a boundary.
    let mut samples = Vec::new();
    for i in 1..=5u64 {
        samples.push(sample_at("m.v", "p", i, i * 1_000, json!(i as f64)));
    }
    for i in 1..=3u64 {
        samples.push(sample_at(
            "m.v",
            "p",
            10 + i,
            10_000 + i * 1_000,
            json!(100.0 * i as f64),
        ));
    }
    seed(&store, "acme", samples).await;

    // Fold everything into the tier and evict the raw beneath it — the state a history read sees.
    tiered(&store, "acme", "m.", 1, 10_000, Some(Method::Avg)).await;
    run_gc(&store, "acme", 30_000).await.unwrap();
    assert_eq!(
        sample_count(&store, "acme", "m.v").await.unwrap(),
        0,
        "raw evicted"
    );
    assert_eq!(
        read_rollups(&store, "acme", "m.v", 0, 30_000)
            .await
            .unwrap()
            .len(),
        2
    );

    // Bucket 0 holds 1,2,3,4,5 → sum 15, count 5, avg 3, min 1, max 5, first 1, last 5.
    for (method, want) in [
        (Method::Avg, json!(3.0)),
        (Method::Min, json!(1.0)),
        (Method::Max, json!(5.0)),
        (Method::Sum, json!(15.0)),
        (Method::Count, json!(5)),
        (Method::First, json!(1.0)),
        (Method::Last, json!(5.0)),
    ] {
        let mut b = buckets(&store, "acme", "m.v", 0, 30_000, 10_000).await;
        apply_method(&mut b, method).unwrap();
        assert_eq!(b[0].t, 0);
        assert_eq!(
            b[0].value.as_ref().unwrap(),
            &want,
            "method {} over the folded tier",
            method.as_str()
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn avg_is_exact_across_a_two_pass_re_aggregation_never_a_mean_of_means() {
    let store = Store::memory().await.unwrap();
    // Deliberately UNEVEN bucket populations: bucket 0 gets 1 sample, bucket 1 gets 9. A
    // mean-of-means would read (10 + 2)/2 = 6; the true mean of the ten values is 2.8.
    let mut samples = vec![sample_at("a.v", "p", 1, 1_000, json!(10.0))];
    for i in 1..=9u64 {
        samples.push(sample_at("a.v", "p", 10 + i, 10_000 + i * 100, json!(2.0)));
    }
    seed(&store, "acme", samples).await;

    tiered(&store, "acme", "a.", 1, 10_000, Some(Method::Avg)).await;
    run_gc(&store, "acme", 40_000).await.unwrap();

    // Re-aggregate the two 10s tier rows into ONE 20s read bucket — the second pass.
    let mut wide = buckets(&store, "acme", "a.v", 0, 20_000, 20_000).await;
    assert_eq!(wide.len(), 1);
    assert_eq!(wide[0].count, 10);
    apply_method(&mut wide, Method::Avg).unwrap();
    let got = wide[0].value.as_ref().unwrap().as_f64().unwrap();
    assert!(
        (got - 2.8).abs() < 1e-9,
        "exact mean 2.8 from sum/num_count, not the mean-of-means 6.0 — got {got}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn nearest_snaps_to_the_grid_across_a_bucket_boundary() {
    let store = Store::memory().await.unwrap();
    // Grid width 10_000. Bucket 10_000's own first sample is far into it (at 19_000); the sample at
    // 9_900 — in the PREVIOUS bucket — is 100ms before the 10_000 boundary and is what "the value at
    // 10:00" honestly means. `first` would report 19_000's value instead.
    seed(
        &store,
        "acme",
        vec![
            sample_at("n.v", "p", 1, 1_000, json!(1.0)),
            sample_at("n.v", "p", 2, 9_900, json!(2.0)),
            sample_at("n.v", "p", 3, 19_000, json!(3.0)),
            sample_at("n.v", "p", 4, 19_500, json!(4.0)),
        ],
    )
    .await;
    tiered(&store, "acme", "n.", 1, 10_000, Some(Method::Nearest)).await;
    run_gc(&store, "acme", 40_000).await.unwrap();

    let mut b = buckets(&store, "acme", "n.v", 0, 20_000, 10_000).await;
    assert_eq!(b.len(), 2);
    apply_method(&mut b, Method::Nearest).unwrap();
    assert_eq!(
        b[0].value.as_ref().unwrap(),
        &json!(1.0),
        "no earlier bucket → its own first"
    );
    assert_eq!(
        b[1].value.as_ref().unwrap(),
        &json!(2.0),
        "the 9_900 sample is nearer to the 10_000 boundary than 19_000 is"
    );

    // And it is genuinely a different answer from `first` — otherwise the method would be redundant.
    apply_method(&mut b, Method::First).unwrap();
    assert_eq!(b[1].value.as_ref().unwrap(), &json!(3.0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn last_gives_a_state_series_step_chart_semantics() {
    let store = Store::memory().await.unwrap();
    // A coil that is 0 for most of the bucket and flips to 1 at the end. `avg` would report 0.2 —
    // a value the point can never physically hold. `last` reports 1.
    let mut samples = Vec::new();
    for i in 0..4u64 {
        samples.push(sample_at("s.coil", "p", i + 1, i * 1_000, json!(0)));
    }
    samples.push(sample_at("s.coil", "p", 5, 4_000, json!(1)));
    seed(&store, "acme", samples).await;

    tiered(&store, "acme", "s.", 1, 10_000, Some(Method::Last)).await;
    run_gc(&store, "acme", 40_000).await.unwrap();

    let mut b = buckets(&store, "acme", "s.coil", 0, 10_000, 10_000).await;
    apply_method(&mut b, Method::Last).unwrap();
    // `last` returns the payload VERBATIM — the coil's integer `1`, not a float. A representative
    // method must not retype the value it kept; only the computed statistics are floats.
    assert_eq!(b[0].value.as_ref().unwrap(), &json!(1));
    apply_method(&mut b, Method::Avg).unwrap();
    assert_eq!(
        b[0].value.as_ref().unwrap(),
        &json!(0.2),
        "avg over a coil is the nonsense `last` exists to avoid"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_bucket_folded_before_the_first_column_existed_refuses_first_and_nearest() {
    // Simulate a rollup row written by the pre-normalize GC: `first_ts` absent. This is the exact
    // shape on disc in any workspace that ran retention before this slice.
    let store = Store::memory().await.unwrap();
    lb_ingest::ensure_series_schema(&store, "acme")
        .await
        .unwrap();
    lb_ingest::write_rollups(
        &store,
        "acme",
        &[lb_ingest::RollupRow {
            series: "legacy.v".into(),
            width_ms: 10_000,
            t: 0,
            min: Some(1.0),
            max: Some(5.0),
            sum: 15.0,
            num_count: 5,
            count: 5,
            last: json!(5.0),
            last_ts: 9_000,
            first: Value::Null,
            first_ts: None,
        }],
    )
    .await
    .unwrap();

    let mut b = buckets(&store, "acme", "legacy.v", 0, 10_000, 10_000).await;
    assert_eq!(b.len(), 1);

    for m in [Method::First, Method::Nearest] {
        let err = apply_method(&mut b, m).unwrap_err();
        assert!(
            err.contains(m.as_str()),
            "the error names the method: {err}"
        );
        assert!(
            err.contains("set the method on the tier"),
            "and it names the fix rather than approximating: {err}"
        );
    }

    // The re-aggregable methods still answer off the stats the legacy row DOES carry.
    for (m, want) in [
        (Method::Avg, json!(3.0)),
        (Method::Min, json!(1.0)),
        (Method::Max, json!(5.0)),
        (Method::Last, json!(5.0)),
    ] {
        apply_method(&mut b, m).unwrap();
        assert_eq!(b[0].value.as_ref().unwrap(), &want);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_second_gc_pass_is_idempotent_and_the_method_value_is_unchanged() {
    let store = Store::memory().await.unwrap();
    let samples: Vec<Sample> = (1..=8u64)
        .map(|i| sample_at("i.v", "p", i, i * 1_000, json!(i as f64)))
        .collect();
    seed(&store, "acme", samples).await;
    tiered(&store, "acme", "i.", 1, 10_000, Some(Method::Avg)).await;

    run_gc(&store, "acme", 40_000).await.unwrap();
    let mut first_pass = buckets(&store, "acme", "i.v", 0, 10_000, 10_000).await;
    apply_method(&mut first_pass, Method::Avg).unwrap();

    let second = run_gc(&store, "acme", 40_000).await.unwrap();
    assert_eq!(second.evicted_raw, 0, "nothing left to evict");
    let rows = read_rollups(&store, "acme", "i.v", 0, 10_000)
        .await
        .unwrap();
    assert_eq!(
        rows.len(),
        1,
        "the re-run upserts the SAME row id, never a duplicate"
    );

    let mut again = buckets(&store, "acme", "i.v", 0, 10_000, 10_000).await;
    apply_method(&mut again, Method::Avg).unwrap();
    assert_eq!(again[0].value, first_pass[0].value);
    assert_eq!(again[0].count, first_pass[0].count);
    assert_eq!(
        again[0].first, first_pass[0].first,
        "the representative survives a re-fold"
    );
}
