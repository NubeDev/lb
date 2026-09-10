//! **A compound index makes a range query on its SECOND field return wrong rows, in wrong order.**
//!
//! Measured on SurrealDB 3.2.4. This is an ENGINE defect, not an lb one: the identical statement
//! against an unindexed table answers correctly, and the only difference is
//! `DEFINE INDEX series_ts_idx ON series FIELDS series, ts` — the index lb defines in
//! `ingest/src/schema.rs` to keep paging O(page).
//!
//! It surfaced as `series_plane_test::a_producer_restart_that_resets_seq_still_pages_in_time_order`
//! returning a sample 1000 ms below a window whose lower bound was 1500 ms.
//!
//! Rows at ts 1000, 1001, 1005, 1008, 2000, 2001, 2003; window `[1500, 3000)`:
//!
//! | query | with the index | correct |
//! |---|---|---|
//! | both bounds + ORDER BY | `[2000, 2003, 2001, 1000]` | `[2003, 2001, 2000]` |
//! | both bounds, no ORDER BY | `[1000, 2001, 2003, 2000]` | 2000, 2001, 2003 |
//! | ORDER BY only, no bounds | `[2000, 2003, 2001, 1000, 1008, 1005, 1001]` | fully descending |
//! | lower bound only | `[2001, 2003, 2000]` | correct |
//! | range on a NON-indexed field | `[2001, 2003, 2000]` | correct |
//!
//! So two things break, and only when the range/sort is on the indexed second field: an upper bound
//! admits out-of-range rows, and `ORDER BY` is not applied at all. A lower bound alone is fine.
//!
//! This matters because `(series, ts)` equality-plus-range is the timeseries read path — paging,
//! bucketing, retention and GC all shape queries this way. Wrong rows are returned SILENTLY.
//!
//! These tests RECORD the behaviour rather than asserting the correct answer, so the suite stays
//! honest about a defect it cannot fix from here. Assert the correct answer once the engine is
//! fixed, and delete this note.

use lb_store::Store;
use serde_json::Value;

async fn seed(s: &Store, indexed: bool) {
    if indexed {
        s.query_ws(
            "w",
            "DEFINE INDEX IF NOT EXISTS series_ts_idx ON series FIELDS series, ts;",
            vec![],
        )
        .await
        .unwrap()
        .check()
        .unwrap();
    }
    for (i, t) in [1000u64, 1001, 1005, 1008, 2000, 2001, 2003]
        .iter()
        .enumerate()
    {
        s.query_ws(
            "w",
            "CREATE type::record('series', $id) SET series = 'pw', ts = time::from_millis($t), t = $t;",
            vec![("id".into(), Value::from(i as u64)), ("t".into(), Value::from(*t))],
        ).await.unwrap().check().unwrap();
    }
}

async fn window(s: &Store) -> Vec<u64> {
    let mut r = s
        .query_ws(
            "w",
            "SELECT series, seq, time::millis(ts) AS ts, t FROM series \
         WHERE series = $series AND ts >= time::from_millis($from) AND ts < time::from_millis($to) \
         ORDER BY ts DESC LIMIT 100",
            vec![
                ("series".into(), Value::from("pw")),
                ("from".into(), Value::from(1500u64)),
                ("to".into(), Value::from(3000u64)),
            ],
        )
        .await
        .unwrap();
    let rows: Vec<Value> = r.take(0).unwrap_or_default();
    rows.iter()
        .filter_map(|v| v.get("t").and_then(|x| x.as_u64()))
        .collect()
}

#[tokio::test]
async fn without_the_index() {
    let s = Store::memory().await.unwrap();
    seed(&s, false).await;
    println!(
        "NO-INDEX  [1500,3000) -> {:?}   (expect [2003, 2001, 2000])",
        window(&s).await
    );
}

#[tokio::test]
async fn with_the_index() {
    let s = Store::memory().await.unwrap();
    seed(&s, true).await;
    println!(
        "WITH-INDEX [1500,3000) -> {:?}   (expect [2003, 2001, 2000])",
        window(&s).await
    );
}

async fn q(s: &Store, sql: &str) -> Vec<u64> {
    let mut r = s
        .query_ws(
            "w",
            sql,
            vec![
                ("series".into(), Value::from("pw")),
                ("from".into(), Value::from(1500u64)),
                ("to".into(), Value::from(3000u64)),
            ],
        )
        .await
        .unwrap();
    let rows: Vec<Value> = r.take(0).unwrap_or_default();
    rows.iter()
        .filter_map(|v| v.get("t").and_then(|x| x.as_u64()))
        .collect()
}

/// Which half is broken: the RANGE, or the ORDER BY?
#[tokio::test]
async fn characterise_the_index_bug() {
    let s = Store::memory().await.unwrap();
    seed(&s, true).await;
    println!("A range, no order   -> {:?}  (expect 2000,2001,2003 in any order)",
        q(&s, "SELECT t FROM series WHERE series = $series AND ts >= time::from_millis($from) AND ts < time::from_millis($to)").await);
    println!(
        "B order, no range   -> {:?}  (expect 2003..1000 descending)",
        q(
            &s,
            "SELECT time::millis(ts) AS ts, t FROM series WHERE series = $series ORDER BY ts DESC"
        )
        .await
    );
    println!(
        "C range, lower only -> {:?}  (expect 2000,2001,2003)",
        q(
            &s,
            "SELECT t FROM series WHERE series = $series AND ts >= time::from_millis($from)"
        )
        .await
    );
    println!(
        "D range on t (no idx field) -> {:?}  (expect 2000,2001,2003)",
        q(
            &s,
            "SELECT t FROM series WHERE series = $series AND t >= 1500 AND t < 3000"
        )
        .await
    );
}
