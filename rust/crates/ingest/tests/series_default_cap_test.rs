//! Bounded BY DEFAULT (disk-budget scope, slice 3): a series **no policy record covers** is
//! FIFO-evicted at `DEFAULT_MAX_SAMPLES`, and a policy record saying `max_samples: 0` is the
//! explicit opt-out that keeps it unbounded (decision 9 — policy-record EXISTENCE decides).
//!
//! These are the tests that cost real time: the bound is 100k and it is not mockable, so each case
//! seeds >100k samples through the REAL ingest path (`write` → `commit_batch`) into the real store.
//! Seeding a smaller store and asserting against a smaller constant would prove nothing about the
//! constant that actually ships — and this is the behaviour change that starts deleting operators'
//! history on upgrade, so it is exactly the wrong place to test a stand-in.
//!
//! Reverting `gc::cap_unpoliced` to the previous advisory `warn_unpoliced` fails
//! `an_unpoliced_series_is_bounded_by_the_default_cap` (the series stays at 100_005).

use lb_ingest::{
    commit_batch, run_gc, sample_count, set_policy, write, Policy, Qos, Sample, DEFAULT_MAX_SAMPLES,
};
use lb_store::Store;
use serde_json::json;

/// Seed `n` samples for `series` through the real staging→commit path, in chunks so no single
/// transaction is absurd. Timestamps are `i * 1000` (1 s cadence), which is also the eviction axis.
async fn seed_n(store: &Store, ws: &str, series: &str, n: u64) {
    for chunk in (1..=n).collect::<Vec<_>>().chunks(2_000) {
        let samples: Vec<Sample> = chunk
            .iter()
            .map(|i| Sample {
                series: series.into(),
                producer: "p".into(),
                ts: i * 1_000,
                seq: *i,
                payload: json!(i),
                labels: json!({}),
                qos: Qos::BestEffort,
            })
            .collect();
        write(store, ws, &samples, 0).await.unwrap();
        while commit_batch(store, ws, 2_000).await.unwrap().drained() > 0 {}
    }
    assert_eq!(sample_count(store, ws, series).await.unwrap(), n, "seeded");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unpoliced_series_is_bounded_by_the_default_cap() {
    let store = Store::memory().await.unwrap();
    let n = DEFAULT_MAX_SAMPLES + 5;
    seed_n(&store, "acme", "unpoliced", n).await;

    // No policy record exists anywhere in this workspace.
    let pass = run_gc(&store, "acme", 1_000_000_000).await.unwrap();

    assert_eq!(
        pass.capped_raw, 5,
        "the default cap evicted exactly the overshoot"
    );
    assert_eq!(
        sample_count(&store, "acme", "unpoliced").await.unwrap(),
        DEFAULT_MAX_SAMPLES,
        "a series with NO policy stops at the default bound instead of growing forever"
    );
    assert_eq!(
        pass.warnings.len(),
        1,
        "an eviction the operator did not configure is REPORTED, never silent"
    );
    assert!(
        pass.warnings[0].contains("max_samples:0"),
        "the notice names the opt-out: {}",
        pass.warnings[0]
    );

    // FIFO: the OLDEST went. ts is `i * 1000`, so the 5 evicted are ts 1_000..=5_000.
    let survivors = sample_count(&store, "acme", "unpoliced").await.unwrap();
    assert_eq!(survivors, DEFAULT_MAX_SAMPLES);
    let mut resp = store
        .query_ws(
            "acme",
            "SELECT count() FROM series WHERE series = 'unpoliced' \
             AND ts < time::from::millis(6000) GROUP ALL",
            vec![],
        )
        .await
        .unwrap();
    let oldest: Option<i64> = resp.take("count").unwrap();
    assert_eq!(
        oldest, None,
        "the five OLDEST samples are the ones that went (FIFO by ts)"
    );

    // Idempotent: a second pass at the bound evicts nothing and says nothing.
    let pass2 = run_gc(&store, "acme", 1_000_000_000).await.unwrap();
    assert_eq!(pass2.capped_raw, 0);
    assert!(pass2.warnings.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn max_samples_zero_opts_out_while_an_unpoliced_series_is_capped() {
    // The ambiguity the flip introduces, in one test: BOTH series are over the default bound and
    // NEITHER has an explicit `max_samples`. The only difference between them is whether a policy
    // RECORD exists — which is precisely what decision 9 says must decide.
    let store = Store::memory().await.unwrap();
    let n = DEFAULT_MAX_SAMPLES + 5;
    seed_n(&store, "acme", "optout.keep", n).await;
    seed_n(&store, "acme", "nopolicy.grow", n).await;

    set_policy(
        &store,
        "acme",
        &Policy {
            prefix: "optout.".into(),
            raw_for_ms: 0,  // no time horizon either
            max_samples: 0, // the EXPLICIT opt-out: genuinely unbounded, honoured as written
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let pass = run_gc(&store, "acme", 1_000_000_000).await.unwrap();

    assert_eq!(
        sample_count(&store, "acme", "optout.keep").await.unwrap(),
        n,
        "a policy record with max_samples:0 is unbounded — untouched past the default cap"
    );
    assert_eq!(
        sample_count(&store, "acme", "nopolicy.grow").await.unwrap(),
        DEFAULT_MAX_SAMPLES,
        "no policy record at all → the default cap applies"
    );
    assert_eq!(pass.capped_raw, 5, "only the unpoliced series was evicted");
    assert_eq!(
        pass.warnings.len(),
        1,
        "and only it is reported — the opt-out is a choice, not a warning"
    );
    assert!(pass.warnings[0].contains("nopolicy.grow"));
}
