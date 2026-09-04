//! The write-conflict storm regression (ingest-conflict-storm scope). Reproduces the live failure on
//! the REAL store (SurrealDB `kv-mem`, the same optimistic MVCC that aborts one side with `read or
//! write conflict … can be retried`; Rule 9 — no fake store):
//!
//!   * **commit-vs-commit** — several producers push the same workspace at once, so their
//!     transactions race on the shared `series`/`series_latest` rows.
//!   * **commit-vs-GC** — a retention pass rolls up and EVICTS raw from `series` while producers
//!     commit new rows to it.
//!
//! In both cases `query_ws_retrying` is the whole defence: there is no lock serializing writers, and
//! there should not be — the store's own optimistic MVCC decides, and the loser retries.
//!
//! Before the fix these concurrent calls surfaced the abort as an error and dropped a whole batch —
//! a permanent gap in the raw series. After it every call succeeds and the stored data is
//! exactly-once: the commit UPSERT is keyed on `[series, producer, seq]`, so a retried transaction
//! re-applies the batch exactly once.
//!
//! Several worker threads so commits run genuinely in parallel — a single-worker runtime interleaves
//! at await points but rarely forces the true two-transactions-one-snapshot collision this guards.

use std::sync::Arc;

use lb_ingest::{
    commit_direct, latest, read, read_buckets, run_gc, set_policy, BucketQuery, Policy, Qos,
    Sample, Tier,
};
use lb_store::Store;
use serde_json::json;

const WS: &str = "nube";
const SERIES: &str = "cpu";
const PRODUCERS: u64 = 6;
const PER_PRODUCER: u64 = 350; // > DIRECT_COMMIT_BATCH (256) so every commit spans several tx
const TS_STEP: u64 = 1_000; // 1s cadence — a realistic ts shape the rollup/bucket read handles

fn sample(producer: &str, seq: u64, ts: u64) -> Sample {
    Sample {
        series: SERIES.into(),
        producer: producer.into(),
        ts,
        seq,
        payload: json!(seq as f64),
        labels: json!({}),
        qos: Qos::MustDeliver,
    }
}

/// One producer's block: `seq_base+1 ..= seq_base+PER_PRODUCER` at 1s cadence, with
/// `ts = ts_base + i*TS_STEP`. Distinct `ts_base` blocks occupy disjoint stretches of the time axis,
/// so a GC cutoff can sit between them and evict one without ever touching the other.
fn block(producer: &str, seq_base: u64, ts_base: u64) -> Vec<Sample> {
    (1..=PER_PRODUCER)
        .map(|i| sample(producer, seq_base + i, ts_base + i * TS_STEP))
        .collect()
}

/// Commit every producer's block one after another — the settled seed, with no race in it.
async fn commit_block(store: &Store, seq_base: u64, ts_base: u64) {
    for p in 0..PRODUCERS {
        let producer = format!("p{p}");
        commit_direct(store, WS, &block(&producer, seq_base, ts_base))
            .await
            .unwrap();
    }
}

/// Commit every producer's block CONCURRENTLY — `PRODUCERS` tasks racing on the same `series` and
/// `series_latest` rows, with nothing serializing them. Panics with the verbatim conflict string if
/// any commit surfaces one.
async fn commit_storm(store: &Store, seq_base: u64, ts_base: u64) {
    let mut handles = Vec::new();
    for p in 0..PRODUCERS {
        let s = store.clone();
        let producer = format!("p{p}");
        handles.push(tokio::spawn(async move {
            commit_direct(&s, WS, &block(&producer, seq_base, ts_base))
                .await
                .map_err(|e| e.to_string())
        }));
    }
    for h in handles {
        h.await
            .expect("commit task panicked")
            .expect("a commit surfaced a retryable conflict — the bounded retry regressed");
    }
}

/// **Commit-vs-commit.** N producers commit their own block at once, with only the bounded retry
/// underneath. Asserts no conflict error AND the committed series is exactly-once: every producer's
/// full seq range, no gap (a dropped batch), no duplicate (a double-commit).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_commits_are_exactly_once() {
    let store = Store::memory().await.unwrap();

    commit_storm(&store, 0, 0).await;

    let rows = read(&store, WS, SERIES, None, None).await.unwrap();
    let expected = (PRODUCERS * PER_PRODUCER) as usize;
    assert_eq!(
        rows.len(),
        expected,
        "no gap and no duplicate in the series"
    );
    for p in 0..PRODUCERS {
        let producer = format!("p{p}");
        let mut seqs: Vec<u64> = rows
            .iter()
            .filter(|r| r.producer == producer)
            .map(|r| r.seq)
            .collect();
        seqs.sort_unstable();
        assert_eq!(
            seqs,
            (1..=PER_PRODUCER).collect::<Vec<_>>(),
            "producer {producer} committed every seq exactly once"
        );
    }
    let last = latest(&store, WS, SERIES).await.unwrap().unwrap();
    assert_eq!(last.seq, PER_PRODUCER, "latest pointer at the newest seq");
}

/// **Commit-vs-GC.** A settled historical block (A) is evicted-and-rolled-up by a GC looper while N
/// producers concurrently commit a NEW block (B) to the same `series` table. The two blocks
/// occupy disjoint stretches of the time axis, so GC only ever evicts settled data (as in
/// production — GC trims the old tail, writes hit the head); the contention is purely on the shared
/// `series` table under MVCC. Asserts no conflict error from EITHER side, and that no sample is lost:
/// B survives as raw exactly-once, and all of A is preserved in the rollup tier.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_commits_vs_gc_lose_no_samples() {
    let store = Store::memory().await.unwrap();

    // Keep the newest 100s of raw; fold the rest into 10s buckets kept forever.
    set_policy(
        &store,
        WS,
        &Policy {
            prefix: SERIES.into(),
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

    // Block A: ts 1_000..=350_000, fully committed BEFORE any GC runs — a settled tail.
    commit_block(&store, 0, 0).await;

    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    // GC looper: real rollup-then-evict passes. `now = 600_000` ⇒ cutoff 500_000, between A
    // (≤350_000) and B (≥1_001_000): every pass rolls ALL of A into 10s buckets and evicts A's raw,
    // never touching B. Realistic 1ms spacing keeps it a periodic pass, not an unbounded spin.
    let gc = {
        let s = store.clone();
        let stop = stop.clone();
        tokio::spawn(async move {
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                run_gc(&s, WS, 600_000).await.map_err(|e| e.to_string())?;
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            }
            Ok::<(), String>(())
        })
    };

    // Block B: ts 1_001_000..=1_350_000 (seq 351..=700) — the live head, disjoint from A, committed
    // concurrently and racing GC's eviction of A on `series`.
    commit_storm(&store, PER_PRODUCER, 1_000_000).await;

    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    gc.await
        .expect("gc task panicked")
        .expect("gc surfaced a retryable conflict under concurrent commits");

    // Settle: one last GC pass rolls up and evicts whatever A raw the race timing left.
    run_gc(&store, WS, 600_000).await.unwrap();

    // Block B survives as raw, exactly-once — no gap opened by a dropped conflict.
    let b_rows = read(&store, WS, SERIES, Some(PER_PRODUCER + 1), None)
        .await
        .unwrap();
    assert_eq!(
        b_rows.len(),
        (PRODUCERS * PER_PRODUCER) as usize,
        "every block-B sample survives the commit-vs-GC race"
    );

    // Block A is fully preserved in the rollup tier — the bucketed read over A's window sums to
    // every A sample (rollup count travels with the aggregate; series-retention scope).
    let q = BucketQuery {
        from_ts: 0,
        to_ts: PER_PRODUCER * TS_STEP + 1,
        width_ms: Some(10_000),
        budget: None,
        ..Default::default()
    };
    let a_buckets = read_buckets(&store, WS, SERIES, &q, 10_000).await.unwrap();
    let a_total: u64 = a_buckets.iter().map(|b| b.count).sum();
    assert_eq!(
        a_total,
        PRODUCERS * PER_PRODUCER,
        "all of block A survives eviction in the rollup tier — no sample lost to a conflict"
    );
}
