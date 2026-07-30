//! The dead-letter horizon (disk-budget scope, decision 7): `ingest_dead_letter` was the one ingest
//! table nothing ever pruned. It now ages out on its OWN 30-day horizon inside the existing
//! `run_gc` pass — separate from `raw_for_ms` on purpose, so tightening series retention never
//! destroys the evidence of why records were dead-lettered.
//!
//! Rows are produced through the REAL overflow path (`write` with a staging bound + `MustDeliver`),
//! never hand-inserted: the thing under test is what the shipped divert writes, `dead_at` included.

use lb_ingest::{
    prune_dead_letters, run_gc, write, Qos, Sample, DEAD_LETTER_KEEP_MS, DEAD_LETTER_TABLE,
};
use lb_store::Store;
use serde_json::{json, Value};

const DAY_MS: u64 = 24 * 60 * 60 * 1000;

fn must_deliver(series: &str, seq: u64) -> Sample {
    Sample {
        series: series.into(),
        producer: "p".into(),
        ts: seq * 1_000,
        seq,
        payload: json!(seq),
        labels: json!({}),
        qos: Qos::MustDeliver,
    }
}

/// Overflow exactly `n` must-deliver samples into the dead-letter table through the real
/// `enforce_bound` path: staging is bound at 1 and primed full by a best-effort sample first (which
/// drop-oldests rather than dead-letters, so priming twice in one workspace stays at one row), so
/// every must-deliver write after it is diverted.
async fn dead_letter_n(store: &Store, ws: &str, series: &str, n: u64) {
    let mut primer = must_deliver("primer", 1);
    primer.qos = Qos::BestEffort;
    write(store, ws, &[primer], 1).await.unwrap();
    for seq in 1..=n {
        write(store, ws, &[must_deliver(series, seq)], 1)
            .await
            .unwrap();
    }
}

async fn dead_letter_count(store: &Store, ws: &str) -> i64 {
    let mut resp = store
        .query_ws(
            ws,
            &format!("SELECT count() FROM {DEAD_LETTER_TABLE} GROUP ALL"),
            vec![],
        )
        .await
        .unwrap();
    let n: Option<i64> = resp.take("count").unwrap();
    n.unwrap_or(0)
}

/// Backdate one dead letter's `dead_at` — a real UPDATE of a real row, which is the only way to make
/// a row genuinely older than the horizon without sleeping for 30 days.
async fn backdate(store: &Store, ws: &str, series: &str, dead_at: u64) {
    store
        .query_ws(
            ws,
            &format!("UPDATE {DEAD_LETTER_TABLE} SET dead_at = $t WHERE sample.series = $series"),
            vec![
                ("t".into(), Value::Number(dead_at.into())),
                ("series".into(), Value::String(series.to_string())),
            ],
        )
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn dead_letters_past_the_horizon_are_pruned_and_fresh_ones_survive() {
    let store = Store::memory().await.unwrap();
    let now = 400 * DAY_MS; // a logical clock well past the horizon, so `now - keep` is meaningful

    dead_letter_n(&store, "acme", "old", 2).await;
    dead_letter_n(&store, "acme", "fresh", 3).await;
    assert_eq!(dead_letter_count(&store, "acme").await, 5, "seeded");

    // "old" is 40 days back; "fresh" is 1 day back — one side of the 30-day horizon each.
    backdate(&store, "acme", "old", now - 40 * DAY_MS).await;
    backdate(&store, "acme", "fresh", now - DAY_MS).await;

    let evicted = prune_dead_letters(&store, "acme", now, DEAD_LETTER_KEEP_MS)
        .await
        .unwrap();
    assert_eq!(evicted, 2, "the two rows past the horizon went");
    assert_eq!(
        dead_letter_count(&store, "acme").await,
        3,
        "entries INSIDE the horizon survive — the evidence outlives the data that produced it"
    );

    // Idempotent: nothing left to prune.
    assert_eq!(
        prune_dead_letters(&store, "acme", now, DEAD_LETTER_KEEP_MS)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_gc_pass_prunes_dead_letters_and_reports_it() {
    // The horizon runs inside the SHIPPED pass, not only when called directly — that is what makes
    // it retention rather than a function nobody invokes.
    let store = Store::memory().await.unwrap();
    let now = 400 * DAY_MS;
    dead_letter_n(&store, "acme", "stale", 4).await;
    backdate(&store, "acme", "stale", now - 31 * DAY_MS).await;

    let pass = run_gc(&store, "acme", now).await.unwrap();
    assert_eq!(
        pass.evicted_dead_letters, 4,
        "the pass reports what it took"
    );
    assert_eq!(dead_letter_count(&store, "acme").await, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_row_with_no_dead_at_falls_back_to_the_samples_own_ts() {
    // Rows written before `dead_at` existed. The upgrade must not pin the whole table to "never
    // expires" — an unbounded table is exactly what this horizon is for.
    let store = Store::memory().await.unwrap();
    let now = 400 * DAY_MS;
    dead_letter_n(&store, "acme", "legacy", 1).await;
    store
        .query_ws(
            "acme",
            &format!("UPDATE {DEAD_LETTER_TABLE} SET dead_at = NONE"),
            vec![],
        )
        .await
        .unwrap();
    // The sample's own ts is 1_000 ms — ancient against a 400-day clock.
    let evicted = prune_dead_letters(&store, "acme", now, DEAD_LETTER_KEEP_MS)
        .await
        .unwrap();
    assert_eq!(evicted, 1, "the fallback age applies");
    assert_eq!(dead_letter_count(&store, "acme").await, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_dead_letter_horizon_never_crosses_the_workspace_wall() {
    let store = Store::memory().await.unwrap();
    let now = 400 * DAY_MS;
    for ws in ["acme", "globex"] {
        dead_letter_n(&store, ws, "shared.name", 2).await;
        backdate(&store, ws, "shared.name", now - 40 * DAY_MS).await;
    }

    let pass = run_gc(&store, "acme", now).await.unwrap();
    assert_eq!(pass.evicted_dead_letters, 2);
    assert_eq!(dead_letter_count(&store, "acme").await, 0);
    assert_eq!(
        dead_letter_count(&store, "globex").await,
        2,
        "ws-B's identically-aged rows are untouched by ws-A's GC (the hard wall)"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn keep_forever_is_zero_and_an_unelapsed_horizon_evicts_nothing() {
    let store = Store::memory().await.unwrap();
    dead_letter_n(&store, "acme", "s", 2).await;
    let old_clock = 400 * DAY_MS;
    assert_eq!(
        prune_dead_letters(&store, "acme", old_clock, 0)
            .await
            .unwrap(),
        0,
        "0 = unbounded, the same grammar every other horizon in this crate uses"
    );
    assert_eq!(
        prune_dead_letters(&store, "acme", DAY_MS, DEAD_LETTER_KEEP_MS)
            .await
            .unwrap(),
        0,
        "a clock younger than the horizon evicts nothing rather than underflowing to 'everything'"
    );
    assert_eq!(dead_letter_count(&store, "acme").await, 2);
}
