//! The series-plane readiness slices, proven against the REAL store (no mocks): keyset paging
//! (exactly-once walk, tiebreaker, clamp, malformed cursor), bucketed decimation (spike survives,
//! bounded budget, last/avg correctness), the series cardinality cap (dead-letter, never silent),
//! label→tag conversion at commit, wall-clock window reads over the datetime `ts`, and retention
//! GC (rollup-then-evict + tier eviction + rollup-backed bucket reads).

use lb_ingest::{
    cap_series, commit_batch, commit_batch_capped, over_cap_warning, read_buckets, read_page,
    run_gc, sample_count, set_policy, write, BucketQuery, Cursor, Direction, PageQuery, Policy,
    Qos, Sample, Tier, DEAD_LETTER_TABLE, DEFAULT_MAX_SAMPLES,
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
            max_samples: 0,
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
            max_samples: 0,
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

// ---------------------------------------------------------------------------------------------
// The per-series FIFO sample cap (series-sample-cap scope, issue #65).
// ---------------------------------------------------------------------------------------------

/// Every committed `(seq, ts)` of a series, oldest-ts first — the identity assertions below check
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

/// THE HEADLINE: the cap evicts oldest-first and stops exactly at the bound — and the survivors are
/// the NEWEST M, asserted by identity.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_cap_evicts_oldest_first_and_keeps_the_newest_m() {
    let store = Store::memory().await.unwrap();
    // 50 samples, ts 1000..=50_000.
    seed(
        &store,
        "acme",
        (1..=50u64)
            .map(|i| sample("cap", "p", i, i * 1000, json!(i)))
            .collect(),
    )
    .await;

    let evicted = cap_series(&store, "acme", "cap", 20).await.unwrap();
    assert_eq!(evicted, 30, "50 - 20 = the 30 oldest are evicted");

    let rows = rows_by_ts(&store, "acme", "cap").await;
    assert_eq!(rows.len(), 20, "exactly the bound remains");
    // The survivors are ts 31_000..=50_000 — the NEWEST 20, not just any 20.
    let expected: Vec<(u64, u64)> = (31..=50u64).map(|i| (i * 1000, i)).collect();
    assert_eq!(
        rows, expected,
        "the newest M survive; the oldest went first"
    );
}

/// THE TRAP: eviction must order by `ts`, NEVER `seq`. `seq` is monotonic per `(series, producer)`
/// only — a restarted producer's seq goes BACKWARDS while the clock goes forwards. This is exactly
/// what pinned `series.latest` to a pre-restart sample in issue #63.
///
/// Seeded so the two axes DISAGREE: the newest rows by `ts` carry the LOWEST seqs. A `seq`-ordered
/// cap evicts the live rows and keeps the dead ones — this test fails on that implementation.
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
    seed(&store, "acme", samples).await;

    // Keep 20 of 40. By `ts` that is exactly the "new" rows; by `seq` it would be the "old" ones.
    let evicted = cap_series(&store, "acme", "mixed", 20).await.unwrap();
    assert_eq!(evicted, 20);

    let page = read_page(
        &store,
        "acme",
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

/// `max_samples: 0` is the explicit opt-out — unbounded, exactly as a policy written before the
/// count axis existed behaves.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn max_samples_zero_is_unbounded() {
    let store = Store::memory().await.unwrap();
    seed(
        &store,
        "acme",
        (1..=30u64)
            .map(|i| sample("keep", "p", i, i * 1000, json!(i)))
            .collect(),
    )
    .await;
    let evicted = cap_series(&store, "acme", "keep", 0).await.unwrap();
    assert_eq!(evicted, 0, "0 = unbounded, the explicit opt-out");
    assert_eq!(sample_count(&store, "acme", "keep").await.unwrap(), 30);
}

/// The cap runs from the GC pass, reports itself in `capped_raw`, and is idempotent: a second pass
/// at the same `now_ms` evicts nothing.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn gc_applies_the_cap_reports_it_and_is_idempotent() {
    let store = Store::memory().await.unwrap();
    seed(
        &store,
        "acme",
        (1..=40u64)
            .map(|i| sample("fleet.a", "p", i, i * 1000, json!(i)))
            .collect(),
    )
    .await;
    set_policy(
        &store,
        "acme",
        &Policy {
            prefix: "fleet.".into(),
            raw_for_ms: 0, // time axis OFF — this proves the COUNT axis stands alone
            max_samples: 10,
            tiers: vec![],
        },
    )
    .await
    .unwrap();

    let pass = run_gc(&store, "acme", 1_000_000).await.unwrap();
    assert_eq!(pass.capped_raw, 30, "the cap reports what it evicted");
    assert_eq!(
        pass.evicted_raw, 0,
        "the time horizon is off; this was the cap"
    );
    assert_eq!(sample_count(&store, "acme", "fleet.a").await.unwrap(), 10);

    let pass2 = run_gc(&store, "acme", 1_000_000).await.unwrap();
    assert_eq!(
        pass2.capped_raw, 0,
        "a second pass evicts nothing (idempotent)"
    );
    assert_eq!(sample_count(&store, "acme", "fleet.a").await.unwrap(), 10);
}

/// The two bounds are INDEPENDENT: whichever bites first wins, and neither resurrects what the
/// other evicted.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn cap_composes_with_the_time_horizon() {
    let store = Store::memory().await.unwrap();
    // 100 samples at 1s cadence, ts 0..=99_000.
    seed(
        &store,
        "acme",
        (0..100u64)
            .map(|i| sample("both", "p", i + 1, i * 1000, json!(i)))
            .collect(),
    )
    .await;
    // now=100_000, raw_for_ms=50_000 → the time horizon alone would keep ts >= 50_000 (50 rows).
    // max_samples=10 is TIGHTER, so the cap bites and only 10 survive.
    set_policy(
        &store,
        "acme",
        &Policy {
            prefix: "both".into(),
            raw_for_ms: 50_000,
            max_samples: 10,
            tiers: vec![],
        },
    )
    .await
    .unwrap();

    let pass = run_gc(&store, "acme", 100_000).await.unwrap();
    assert_eq!(pass.evicted_raw, 50, "the time horizon took the oldest 50");
    assert_eq!(pass.capped_raw, 40, "the tighter count cap took 40 more");
    let rows = rows_by_ts(&store, "acme", "both").await;
    assert_eq!(rows.len(), 10, "the tighter bound wins");
    assert_eq!(rows[0].0, 90_000, "survivors are the newest 10 by ts");
}

/// With tiers, the over-cap window folds into the rollups BEFORE it is evicted — coarse history
/// survives a cap eviction and a bucketed read still renders.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn cap_rolls_up_before_evicting_so_bucket_reads_survive() {
    let store = Store::memory().await.unwrap();
    // 100 samples at 1s cadence, value = i.
    seed(
        &store,
        "acme",
        (0..100u64)
            .map(|i| sample("roll", "p", i + 1, i * 1000, json!(i as f64)))
            .collect(),
    )
    .await;
    set_policy(
        &store,
        "acme",
        &Policy {
            prefix: "roll".into(),
            raw_for_ms: 0, // count axis only
            max_samples: 10,
            tiers: vec![Tier {
                width_ms: 10_000,
                keep_for_ms: 0, // rollups kept forever
            }],
        },
    )
    .await
    .unwrap();

    let pass = run_gc(&store, "acme", 1_000_000).await.unwrap();
    assert_eq!(pass.capped_raw, 90);
    assert!(pass.rollup_rows > 0, "the over-cap window rolled up first");
    assert_eq!(sample_count(&store, "acme", "roll").await.unwrap(), 10);

    // A bucketed read over the FULL window still covers the cap-evicted history via the tier.
    let q = BucketQuery {
        from_ts: 0,
        to_ts: 100_000,
        width_ms: Some(10_000),
        budget: None,
    };
    let buckets = read_buckets(&store, "acme", "roll", &q, 10_000)
        .await
        .unwrap();
    assert_eq!(buckets.len(), 10, "cap-evicted history survives as rollups");
    let first = &buckets[0]; // ts 0..10s, values 0..=9 — served entirely from rollups
    assert_eq!(first.min, Some(0.0));
    assert_eq!(first.max, Some(9.0));
    assert_eq!(first.count, 10);
}

/// MANDATORY (rule 6): a policy in one workspace never evicts another's rows. Same series name,
/// same cap, two workspaces — GC in `acme` leaves `globex` untouched.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_cap_never_crosses_the_workspace_wall() {
    let store = Store::memory().await.unwrap();
    for ws in ["acme", "globex"] {
        seed(
            &store,
            ws,
            (1..=30u64)
                .map(|i| sample("shared.name", "p", i, i * 1000, json!(i)))
                .collect(),
        )
        .await;
    }
    // The policy exists ONLY in acme.
    set_policy(
        &store,
        "acme",
        &Policy {
            prefix: "shared.".into(),
            raw_for_ms: 0,
            max_samples: 5,
            tiers: vec![],
        },
    )
    .await
    .unwrap();

    let pass = run_gc(&store, "acme", 1_000_000).await.unwrap();
    assert_eq!(pass.capped_raw, 25);
    assert_eq!(
        sample_count(&store, "acme", "shared.name").await.unwrap(),
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

/// Longest-prefix-wins: a series matching both `fleet.` and `fleet.eu.` is governed by the LONGER
/// prefix alone — not processed twice with the tighter bound winning by accident.
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

/// Release 1's default axis: an unpoliced series past the recommended cap is WARNED about, not
/// evicted. (Release 2 flips this to bounded-by-default.)
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
