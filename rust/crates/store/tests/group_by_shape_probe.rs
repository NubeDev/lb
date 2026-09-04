//! What shape does `GROUP BY` give each selected column in SurrealDB 3?
//!
//! `lb_ingest`'s bucketed read is two `GROUP BY b` aggregates whose rows decode into structs with
//! scalar fields (`num_count: u64`, `count: u64`, `last_ts: u64`). Under SurrealDB 3 that decode
//! fails with "invalid type: sequence, expected u64" — some column now arrives as an array. This
//! prints the raw JSON so the fix targets the column that actually changed.

use lb_store::Store;
use serde_json::Value;

#[tokio::test]
async fn what_group_by_returns_per_column() {
    let store = Store::memory().await.expect("open");
    for (i, v) in [(0u64, 1.0), (1, 2.0), (2, 3.0), (3, 4.0)] {
        store
            .query_ws(
                "ws-a",
                &format!(
                    "CREATE s:k{i} SET b = {}, payload = {v}, t = {}",
                    i / 2,
                    100 + i
                ),
                vec![],
            )
            .await
            .expect("seed");
    }

    let mut resp = store
        .query_ws(
            "ws-a",
            "SELECT b, count() AS num_count, math::min(payload) AS min, \
             math::max(payload) AS max, math::sum(payload) AS sum FROM s GROUP BY b",
            vec![],
        )
        .await
        .expect("aggregate query");
    let rows: Vec<Value> = resp.take(0).expect("take");
    println!(
        "aggregate rows: {}",
        serde_json::to_string_pretty(&rows).unwrap()
    );
    // True aggregates are unchanged: one scalar per group. This half of the bucketed read is fine.
    assert_eq!(rows.len(), 2, "one row per bucket");
    for r in &rows {
        assert!(r["num_count"].is_u64(), "count() is still a scalar: {r}");
        assert!(r["sum"].is_number(), "math::sum is still a scalar: {r}");
    }

    let mut resp = store
        .query_ws(
            "ws-a",
            "SELECT b, count() AS count, array::last(p) AS last, array::last(t) AS last_ts \
             FROM (SELECT b, payload AS p, t FROM s ORDER BY t ASC) GROUP BY b",
            vec![],
        )
        .await
        .expect("ordered-subquery aggregate");
    let rows2: Vec<Value> = resp.take(0).expect("take");
    println!(
        "subquery rows: {}",
        serde_json::to_string_pretty(&rows2).unwrap()
    );

    // THE BREAKING CHANGE. `lb_ingest::bucket` gets the first/last payload of a bucket by ordering
    // in a subquery and taking `array::last(p)` over the group. That worked because a
    // non-aggregated column inside `GROUP BY` collected into an array before the function ran.
    //
    // SurrealDB 3 reverses the order: only functions it knows as AGGREGATES see the whole group.
    // Everything else is evaluated PER ROW and the per-row results are collected. So `array::last(p)`
    // now asks for the last element of a scalar — NONE — once per row, and the column comes back as
    // `[null, null]`, which fails the decode with "invalid type: sequence, expected u64".
    // `group_collect_probe.rs` shows the contrast: `array::group(p)` IS an aggregate and returns the
    // group's real values.
    //
    // This is asserted as the CURRENT behaviour, not the desired one. `bucket.rs` still needs a real
    // aggregate for first/last-in-group; when that lands, this test should be rewritten to pin the
    // new query rather than deleted, so the idiom cannot quietly come back.
    for r in &rows2 {
        assert!(r["count"].is_u64(), "count() survives grouping: {r}");
        assert!(
            r["last"].as_array().is_some_and(|a| a.iter().all(|v| v.is_null())),
            "a non-aggregated column inside GROUP BY no longer collects; expected all-null, got {r}"
        );
    }
}
