//! The DURABLE half of the write-time filters (series-normalize scope): the per-`(series,
//! producer)` anchor — that it is keyed per producer and not per series, that it survives a restart
//! because it lives on `series_meta`, which policy owns a series when prefixes nest, that a ws-B
//! filter touches nothing in ws-A, and that a policy row written before this slice keeps its exact
//! meaning.
//!
//! The predicates themselves are `filter_predicate_test.rs`; their per-batch store behaviour is
//! `series_filter_test.rs`. Real store, no mocks (testing §0).

use lb_ingest::{
    commit_batch, read, read_filter_state, set_policy, write, Deadband, Filter, LastCommitted,
    Policy, Qos, Sample, Tier,
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
            ..Default::default()
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
async fn the_deadband_anchor_is_per_producer_not_per_series() {
    let store = Store::memory().await.unwrap();
    policy(
        &store,
        "temp.",
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
    // The prefix above is deliberately keyed correctly below; re-set to be unambiguous.
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

    // TWO producers on ONE series, with INDEPENDENT ts and seq axes. Producer A sits near 0,
    // producer B near 100. If the anchor were per-SERIES, each producer's sample would look like a
    // huge move against the other's value and NOTHING would ever be filtered — and interleaving
    // them by ts would filter the wrong ones.
    let samples = vec![
        sample_at("temp.a", "pa", 1, 1_000, json!(0.0)), // A first  → stored
        sample_at("temp.a", "pb", 7, 1_500, json!(100.0)), // B first  → stored
        sample_at("temp.a", "pa", 2, 2_000, json!(1.0)), // A +1 within band → dropped
        sample_at("temp.a", "pb", 8, 2_500, json!(101.0)), // B +1 within band → dropped
        sample_at("temp.a", "pa", 3, 3_000, json!(9.0)), // A +9 → stored
        sample_at("temp.a", "pb", 9, 3_500, json!(102.0)), // B +2 from 100 → dropped
    ];
    let pass = seed(&store, "acme", samples).await;

    let got = stored(&store, "acme", "temp.a").await;
    assert_eq!(got.len(), 3, "one per real move, per producer: {got:?}");
    assert!(got.contains(&json!(0.0)) && got.contains(&json!(100.0)) && got.contains(&json!(9.0)));
    assert_eq!(pass.filtered.deadband, 3);

    // And the persisted anchors are keyed by producer, holding each one's own last committed value.
    let state = read_filter_state(&store, "acme", &["temp.a".to_string()])
        .await
        .unwrap();
    let producers = state.get("temp.a").expect("anchors persisted");
    assert_eq!(
        producers.get("pa"),
        Some(&LastCommitted {
            ts: 3_000,
            value: Some(9.0)
        })
    );
    assert_eq!(
        producers.get("pb"),
        Some(&LastCommitted {
            ts: 1_500,
            value: Some(100.0)
        })
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_anchor_survives_a_node_restart_because_it_lives_on_series_meta() {
    // A process-local cache would re-open the deadband on every reboot and store a redundant burst.
    // Proven by draining in two SEPARATE commit calls with the anchor read back from the store in
    // between — the state crosses the process boundary the same way it crosses a restart.
    let store = Store::memory().await.unwrap();
    policy(
        &store,
        "acme",
        "temp.",
        Filter {
            deadband: Some(Deadband {
                abs: Some(1.0),
                pct: None,
            }),
            ..Default::default()
        },
    )
    .await;

    seed(
        &store,
        "acme",
        vec![sample_at("temp.a", "p1", 1, 1_000, json!(20.0))],
    )
    .await;
    let persisted = read_filter_state(&store, "acme", &["temp.a".to_string()])
        .await
        .unwrap();
    assert_eq!(
        persisted["temp.a"]["p1"],
        LastCommitted {
            ts: 1_000,
            value: Some(20.0)
        },
        "the anchor is durable, not in-process"
    );

    // A wholly separate later batch — the deadband must still be closed against 20.0.
    let pass = seed(
        &store,
        "acme",
        vec![sample_at("temp.a", "p1", 2, 2_000, json!(20.5))],
    )
    .await;
    assert_eq!(pass.committed, 0);
    assert_eq!(
        pass.filtered.deadband, 1,
        "the reboot did NOT re-open the band"
    );
    assert_eq!(stored(&store, "acme", "temp.a").await, vec![json!(20.0)]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_longest_matching_prefix_owns_the_filter() {
    let store = Store::memory().await.unwrap();
    // Parent mutes everything; the child prefix overrides with a plain store-everything filter.
    policy(
        &store,
        "acme",
        "site.",
        Filter {
            drop: true,
            ..Default::default()
        },
    )
    .await;
    policy(
        &store,
        "acme",
        "site.keep.",
        Filter {
            min_interval_ms: 1,
            ..Default::default()
        },
    )
    .await;

    let pass = seed(
        &store,
        "acme",
        vec![
            sample_at("site.mute.v", "p", 1, 1_000, json!(1.0)),
            sample_at("site.keep.v", "p", 1, 1_000, json!(2.0)),
            sample_at("site.keep.v", "p", 2, 2_000, json!(3.0)),
        ],
    )
    .await;

    assert!(stored(&store, "acme", "site.mute.v").await.is_empty());
    assert_eq!(
        stored(&store, "acme", "site.keep.v").await,
        vec![json!(2.0), json!(3.0)],
        "the child prefix wins outright — the parent's mute does not leak into it"
    );
    assert_eq!(pass.filtered.muted, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_filter_in_workspace_b_never_touches_workspace_a() {
    // MANDATORY workspace-isolation test. Same series name, same prefix, two tenants.
    let store = Store::memory().await.unwrap();
    policy(
        &store,
        "beta",
        "temp.",
        Filter {
            drop: true,
            ..Default::default()
        },
    )
    .await;

    let samples = |ws: &str| {
        vec![
            sample_at("temp.a", "p", 1, 1_000, json!(1.0)),
            sample_at("temp.a", "p", 2, 2_000, json!(2.0)),
        ]
        .into_iter()
        .map(|mut s| {
            s.producer = format!("{ws}-p");
            s
        })
        .collect::<Vec<_>>()
    };

    let a = seed(&store, "acme", samples("acme")).await;
    let b = seed(&store, "beta", samples("beta")).await;

    assert_eq!(a.committed, 2, "ws-A has NO policy — it stores everything");
    assert!(a.filtered.is_zero());
    assert_eq!(stored(&store, "acme", "temp.a").await.len(), 2);

    assert_eq!(b.committed, 0, "ws-B's own mute applies only inside ws-B");
    assert_eq!(b.filtered.muted, 2);
    assert!(stored(&store, "beta", "temp.a").await.is_empty());

    // And the anchors are workspace-scoped too — no cross-tenant read.
    assert!(read_filter_state(&store, "beta", &["temp.a".to_string()])
        .await
        .unwrap()
        .get("temp.a")
        .is_none_or(|p| p.is_empty()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_absent_filter_block_is_byte_for_byte_todays_behaviour() {
    // The compatibility guarantee: a policy row written before this slice keeps its exact meaning.
    let store = Store::memory().await.unwrap();
    set_policy(
        &store,
        "acme",
        &Policy {
            prefix: "old.".into(),
            raw_for_ms: 900_000,
            max_samples: 100,
            tiers: vec![Tier {
                width_ms: 60_000,
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

    let samples: Vec<Sample> = (1..=20u64)
        .map(|i| sample_at("old.v", "p", i, i * 100, json!(1.0))) // identical values, 100ms apart
        .collect();
    let pass = seed(&store, "acme", samples).await;

    assert_eq!(
        pass.committed, 20,
        "no filter → every sample stores, even identical ones"
    );
    assert!(pass.filtered.is_zero());

    // And the policy round-trips with both new fields absent.
    let listed = lb_ingest::list_policies(&store, "acme").await.unwrap();
    let p = listed.iter().find(|p| p.prefix == "old.").unwrap();
    assert!(p.filter.is_none());
    assert!(p.tiers[0].method.is_none());
}
