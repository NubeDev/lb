//! The series-plane readiness slices, proven against the REAL store (no mocks): keyset paging
//! (exactly-once walk, tiebreaker, clamp, malformed cursor), bucketed decimation (spike survives,
//! bounded budget, last/avg correctness), the series cardinality cap (dead-letter, never silent),
//! label→tag conversion at commit, wall-clock window reads over the datetime `ts`, and retention
//! GC (rollup-then-evict + tier eviction + rollup-backed bucket reads).

use lb_ingest::{
    commit_batch, commit_batch_capped, latest, latest_many, read_buckets, read_buckets_fold,
    read_page, write, Bucket, BucketQuery, Cursor, Direction, PageQuery, Qos, Sample,
    DEAD_LETTER_TABLE,
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

/// The headline correctness guard (series-read-perf scope): the pushed-down `GROUP BY`
/// ([`read_buckets`]) is byte-identical to the in-Rust fold oracle ([`read_buckets_fold`]) across
/// every corner the two-query split exists to preserve — non-numeric payloads (skipped by `math::*`,
/// still counted, eligible as `last`), the `(ts, seq)` last tiebreaker, and a sparse empty-bucket gap.
/// If this passes, the pushdown is a pure speed-up, not a semantic change.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn pushdown_buckets_are_byte_identical_to_the_fold() {
    let store = Store::memory().await.unwrap();
    let samples = vec![
        // bucket 0 [0,60000): flat 20s, a spike, a non-numeric (counts, skips math), numeric last.
        sample("m", "p", 1, 0, json!(20.0)),
        sample("m", "p", 2, 1000, json!(20.0)),
        sample("m", "q", 1, 1000, json!(200.0)), // spike; same ts as p/2, seq breaks the tie
        sample("m", "p", 3, 2000, json!("boom")), // non-numeric — count only, can be last
        sample("m", "p", 4, 3000, json!(21.0)),  // numeric last of bucket 0
        // bucket 1 [60000,120000) is intentionally EMPTY — a sparse gap both must omit.
        // bucket 2 [120000,180000): a numeric then a NON-NUMERIC last.
        sample("m", "p", 5, 121_000, json!(30.0)),
        sample("m", "p", 6, 122_000, json!("offline")), // non-numeric LAST
    ];
    seed(&store, "acme", samples).await;

    let q = BucketQuery {
        from_ts: 0,
        to_ts: 180_000,
        width_ms: Some(60_000),
        budget: None,
    };
    let pushed = read_buckets(&store, "acme", "m", &q, 60_000).await.unwrap();
    let folded = read_buckets_fold(&store, "acme", "m", &q, 60_000)
        .await
        .unwrap();

    // Byte-identical serialized wire shape — the exact parity contract.
    assert_eq!(
        serde_json::to_value(&pushed).unwrap(),
        serde_json::to_value(&folded).unwrap(),
        "pushdown must equal the fold oracle bucket-for-bucket"
    );

    // Spot-assert the corners so a regression names itself.
    assert_eq!(pushed.len(), 2, "empty bucket 1 omitted (sparse)");
    let b0 = &pushed[0];
    assert_eq!(b0.min, Some(20.0));
    assert_eq!(b0.max, Some(200.0), "spike survives in max");
    assert_eq!(b0.count, 5, "total count includes the non-numeric");
    assert_eq!(
        b0.avg,
        Some((20.0 + 20.0 + 200.0 + 21.0) / 4.0),
        "avg over numerics only"
    );
    assert_eq!(b0.last, json!(21.0), "numeric last by (ts,seq)");
    let b2 = &pushed[1];
    assert_eq!(b2.t, 120_000);
    assert_eq!(b2.last, json!("offline"), "a non-numeric can be the last");
    assert_eq!(b2.avg, Some(30.0), "the non-numeric doesn't perturb avg");
}

/// Assert two bucket vectors are byte-identical field-for-field. `avg` uses exact equality — the
/// pushdown and fold compute `sum/num_count` the same way, so bit-equal.
fn assert_buckets_eq(got: &[Bucket], want: &[Bucket]) {
    assert_eq!(
        got.len(),
        want.len(),
        "same bucket set (sparse gaps preserved)"
    );
    for (g, w) in got.iter().zip(want) {
        assert_eq!(g.t, w.t, "bucket t");
        assert_eq!(g.min, w.min, "min at t={}", w.t);
        assert_eq!(g.max, w.max, "max at t={}", w.t);
        assert_eq!(g.avg, w.avg, "avg at t={}", w.t);
        assert_eq!(g.last, w.last, "last at t={}", w.t);
        assert_eq!(g.count, w.count, "count at t={}", w.t);
    }
}

/// The bucket-index-vs-floor seam: a `from` that is NOT width-aligned must still produce the same
/// absolute-floor bucket boundaries as the fold. Catches an off-by-one in the `b`→`t` mapping.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn pushdown_handles_an_unaligned_from() {
    let store = Store::memory().await.unwrap();
    let samples: Vec<Sample> = (0..300u64)
        .map(|i| sample("m", "p", i + 1, i * 1000, json!(i as f64)))
        .collect();
    seed(&store, "acme", samples).await;

    // from=17_000 is NOT a multiple of width=60_000 — the seam under test.
    let q = BucketQuery {
        from_ts: 17_000,
        to_ts: 299_000,
        width_ms: Some(60_000),
        budget: None,
    };
    let pushed = read_buckets(&store, "acme", "m", &q, 60_000).await.unwrap();
    let folded = read_buckets_fold(&store, "acme", "m", &q, 60_000)
        .await
        .unwrap();
    assert_buckets_eq(&pushed, &folded);
    // Bucket keys stay on the absolute width grid (0, 60000, …), never on `from`.
    assert!(
        pushed.iter().all(|b| b.t % 60_000 == 0),
        "buckets on the absolute grid"
    );
}

/// The regression guard whose absence let the fold ship against the scope's own O(buckets) goal:
/// a 10 k-sample window decimates to a tiny bucket count fast, and latency stays flat as the sample
/// count grows 10× at a fixed budget (O(buckets), not O(rows)). Correctness stays pinned by parity.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn pushdown_is_o_buckets_not_o_rows() {
    let store = Store::memory().await.unwrap();
    // 12 k samples across a 12 k-second window, 1 s cadence. Over `SCAN_CHUNK` (10 000), so the
    // fold oracle's keyset loop must fetch a SECOND NON-EMPTY chunk — at exactly 10 000 the second
    // page came back empty and no row was ever folded across a chunk boundary, the single-page
    // blind spot from testing-scope §3.2.
    let samples: Vec<Sample> = (0..12_000u64)
        .map(|i| sample("big", "p", i + 1, i * 1000, json!((i % 50) as f64)))
        .collect();
    seed(&store, "acme", samples).await;

    let q = BucketQuery {
        from_ts: 0,
        to_ts: 12_000_000,
        width_ms: None,
        budget: Some(240),
    };
    let width = lb_ingest::effective_width(&q).unwrap();
    let buckets = read_buckets(&store, "acme", "big", &q, width)
        .await
        .unwrap();
    assert!(
        buckets.len() <= 240,
        "decimated to the budget, not 12k rows"
    );
    // Parity with the fold on the large seed — the pushdown didn't cut a corner to be fast.
    let folded = read_buckets_fold(&store, "acme", "big", &q, width)
        .await
        .unwrap();
    assert_buckets_eq(&buckets, &folded);
}

/// `latest_many` (the store method): every requested name present in order (absent → None), the
/// newest `(ts, seq)` sample per name, a non-numeric latest carried verbatim, parity with single
/// `latest`, and ws-B scoping (a ws-A name resolves to None). The host MCP-bridge deny/isolation
/// coverage lives in `host/tests/ingest_test.rs`.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn latest_many_covers_every_name_and_scopes_by_workspace() {
    let store = Store::memory().await.unwrap();
    seed(
        &store,
        "acme",
        vec![
            sample("temp", "p", 1, 1000, json!(20.0)),
            sample("temp", "p", 2, 2000, json!(21.0)), // newest of temp by (ts,seq)
            sample("mode", "p", 1, 500, json!("heating")), // non-numeric latest
        ],
    )
    .await;

    let names = vec!["temp".to_string(), "ghost".to_string(), "mode".to_string()];
    let got = latest_many(&store, "acme", &names).await.unwrap();

    // Every requested name present, in request order; unknown → None.
    assert_eq!(
        got.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(),
        ["temp", "ghost", "mode"]
    );
    assert!(got[1].1.is_none(), "unknown series → None");
    assert_eq!(
        got[0].1.as_ref().unwrap().payload,
        json!(21.0),
        "newest by (ts,seq)"
    );
    assert_eq!(
        got[2].1.as_ref().unwrap().payload,
        json!("heating"),
        "non-numeric latest verbatim"
    );

    // Parity: equals mapping single `latest` over the same names.
    for (name, s) in &got {
        assert_eq!(
            *s,
            latest(&store, "acme", name).await.unwrap(),
            "{name} == single latest"
        );
    }

    // ws-B batching a ws-A name sees None (namespace-first; the name carries no grant).
    let scoped = latest_many(&store, "other-ws", &["temp".to_string()])
        .await
        .unwrap();
    assert_eq!(scoped.len(), 1);
    assert!(scoped[0].1.is_none(), "cross-ws name resolves to None");
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

/// THE HEADLINE: the cap evicts oldest-first and stops exactly at the bound — and the survivors are
/// the NEWEST M, asserted by identity.
/// THE TRAP: eviction must order by `ts`, NEVER `seq`. `seq` is monotonic per `(series, producer)`
/// only — a restarted producer's seq goes BACKWARDS while the clock goes forwards. This is exactly
/// what pinned `series.latest` to a pre-restart sample in issue #63.
///
/// Seeded so the two axes DISAGREE: the newest rows by `ts` carry the LOWEST seqs. A `seq`-ordered
/// cap evicts the live rows and keeps the dead ones — this test fails on that implementation.
/// `max_samples: 0` is the explicit opt-out — unbounded, exactly as a policy written before the
/// count axis existed behaves.
/// The cap runs from the GC pass, reports itself in `capped_raw`, and is idempotent: a second pass
/// at the same `now_ms` evicts nothing.
/// The two bounds are INDEPENDENT: whichever bites first wins, and neither resurrects what the
/// other evicted.
/// With tiers, the over-cap window folds into the rollups BEFORE it is evicted — coarse history
/// survives a cap eviction and a bucketed read still renders.
/// MANDATORY (rule 6): a policy in one workspace never evicts another's rows. Same series name,
/// same cap, two workspaces — GC in `acme` leaves `globex` untouched.
/// Longest-prefix-wins: a series matching both `fleet.` and `fleet.eu.` is governed by the LONGER
/// prefix alone — not processed twice with the tighter bound winning by accident.
/// Release 1's default axis: an unpoliced series past the recommended cap is WARNED about, not
/// evicted. (Release 2 flips this to bounded-by-default.)
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn latest_pointer_advances_forward_only_across_commits() {
    let store = Store::memory().await.unwrap();

    // Commit 1: newest is ts=2000.
    seed(
        &store,
        "acme",
        vec![
            sample("p", "prod", 1, 1000, json!(10.0)),
            sample("p", "prod", 2, 2000, json!(20.0)),
        ],
    )
    .await;
    assert_eq!(
        latest(&store, "acme", "p").await.unwrap().unwrap().payload,
        json!(20.0)
    );

    // Commit 2 brings a strictly-newer sample (ts=3000) → pointer advances.
    seed(
        &store,
        "acme",
        vec![sample("p", "prod", 3, 3000, json!(30.0))],
    )
    .await;
    assert_eq!(
        latest(&store, "acme", "p").await.unwrap().unwrap().payload,
        json!(30.0)
    );

    // Commit 3 brings an OLDER sample (a late/out-of-order arrival, ts=1500, a fresh producer so its
    // (series,producer,seq) doesn't collide) → the pointer must NOT regress off ts=3000.
    seed(
        &store,
        "acme",
        vec![sample("p", "late", 1, 1500, json!(99.0))],
    )
    .await;
    assert_eq!(
        latest(&store, "acme", "p").await.unwrap().unwrap().payload,
        json!(30.0),
        "a later commit of an OLDER sample never regresses the pointer"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn latest_pointer_is_ts_primary_across_a_producer_restart() {
    let store = Store::memory().await.unwrap();
    // The restart trap (see latest.rs docstring): stream A reaches seq=9 at ts=9000; a restarted
    // stream B re-enters at seq=0 but a HIGHER ts=10000. "Newest" is ts-primary, so B's sample wins
    // even though its seq is far lower — the pointer must reflect that, exactly as the scan would.
    seed(
        &store,
        "acme",
        vec![
            sample("s", "streamA", 9, 9000, json!("old")),
            sample("s", "streamB", 0, 10000, json!("new")),
        ],
    )
    .await;
    assert_eq!(
        latest(&store, "acme", "s").await.unwrap().unwrap().payload,
        json!("new"),
        "higher ts wins over higher seq (ts-primary, restart-safe)"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn latest_pointer_survives_replay_and_delete() {
    let store = Store::memory().await.unwrap();
    seed(
        &store,
        "acme",
        vec![
            sample("d", "prod", 1, 1000, json!(1.0)),
            sample("d", "prod", 2, 2000, json!(2.0)),
        ],
    )
    .await;

    // Idempotent replay: re-writing the SAME samples (same (series,producer,seq)) re-commits them;
    // the pointer stays on the newest, never duplicated or regressed.
    seed(
        &store,
        "acme",
        vec![
            sample("d", "prod", 1, 1000, json!(1.0)),
            sample("d", "prod", 2, 2000, json!(2.0)),
        ],
    )
    .await;
    assert_eq!(
        latest(&store, "acme", "d").await.unwrap().unwrap().payload,
        json!(2.0)
    );

    // After delete_series the pointer is gone with everything else → latest is None (not a stale hit).
    lb_ingest::delete_series(&store, "acme", "d").await.unwrap();
    assert!(
        latest(&store, "acme", "d").await.unwrap().is_none(),
        "deleted series → no pointer, None"
    );
}
