//! How do you collect a group's values in SurrealDB 3?
//!
//! `lb_ingest::bucket` needs the chronologically FIRST and LAST payload in each bucket. It got them
//! by ordering rows in a subquery and taking `array::last(p)` over the group, relying on a
//! non-aggregated column collecting into an array before the function ran.
//!
//! Measured here, SurrealDB 3 does two things differently, and BOTH break that query:
//!
//!   1. Only functions it recognises as AGGREGATES see the whole group. `array::group(p)` does and
//!      returns the group's values; `array::last(p)` does not, so it runs per row on a scalar and
//!      the column comes back as an array of NONEs.
//!   2. The inner `ORDER BY t` is rejected outright unless `t` is in that subquery's selection —
//!      the same "order idiom must be selected" rule that bit `store_crud`.
//!
//! Note what this does NOT give us: `array::group` collects in storage order, not time order, so it
//! is not a drop-in for "last by `(ts, seq)`". Choosing the replacement is a design decision for
//! `bucket.rs`, which is why this file records the behaviour rather than pretending to fix it.

use lb_store::Store;
use serde_json::Value;

async fn seeded() -> Store {
    let store = Store::memory().await.expect("open");
    // Two buckets, deliberately inserted out of time order so "last" cannot be "last inserted".
    for (id, b, t, p) in [
        ("a", 0, 102, 3),
        ("b", 0, 100, 1),
        ("c", 0, 101, 2),
        ("d", 1, 201, 9),
        ("e", 1, 200, 8),
    ] {
        store
            .query_ws(
                "ws-a",
                &format!("CREATE s:{id} SET b = {b}, t = {t}, p = {p}"),
                vec![],
            )
            .await
            .expect("seed");
    }
    store
}

#[tokio::test]
async fn which_form_collects_a_groups_values() {
    let store = seeded().await;
    let candidates: &[(&str, &str)] = &[
        (
            "array::group(p)",
            "SELECT b, array::group(p) AS v FROM s GROUP BY b",
        ),
        (
            "array::distinct(p)",
            "SELECT b, array::distinct(p) AS v FROM s GROUP BY b",
        ),
        (
            "math::max(t)",
            "SELECT b, math::max(t) AS v FROM s GROUP BY b",
        ),
        (
            "ordered subquery + array::last",
            "SELECT b, array::last(p) AS v FROM (SELECT b, p FROM s ORDER BY t ASC) GROUP BY b",
        ),
    ];
    for (name, sql) in candidates {
        match store.query_ws("ws-a", sql, vec![]).await {
            Ok(mut r) => {
                let rows: Vec<Value> = r.take(0).unwrap_or_default();
                println!("{name:32} -> {}", serde_json::to_string(&rows).unwrap());
            }
            Err(e) => println!("{name:32} -> ERROR {e}"),
        }
    }
}

/// The candidate repair for `bucket.rs`: collect `[t, seq, payload]` triples with the `array::group`
/// AGGREGATE, sort them server-side, and take both ends. Arrays sort lexicographically, so ordering
/// by the triple is ordering by `(ts, seq)` — the exact key the fold uses — without an inner
/// `ORDER BY` and without shipping every row to the client.
#[tokio::test]
async fn sorting_grouped_triples_gives_first_and_last_by_ts_seq() {
    let store = seeded().await;
    let mut resp = store
        .query_ws(
            "ws-a",
            "SELECT b,                array::first(array::sort(array::group([t, 0, p]))) AS first,                array::last(array::sort(array::group([t, 0, p]))) AS last              FROM s GROUP BY b",
            vec![],
        )
        .await
        .expect("the repair query must run");
    let rows: Vec<Value> = resp.take(0).expect("take");
    println!("{}", serde_json::to_string(&rows).unwrap());

    let by_b: std::collections::BTreeMap<i64, &Value> = rows
        .iter()
        .map(|r| (r["b"].as_i64().expect("b"), r))
        .collect();

    // Bucket 0 holds t=100..102 inserted out of order; bucket 1 holds t=200,201 likewise.
    let b0 = by_b[&0];
    assert_eq!(
        b0["first"][0].as_i64(),
        Some(100),
        "earliest ts in bucket 0"
    );
    assert_eq!(b0["first"][2].as_i64(), Some(1), "its payload");
    assert_eq!(b0["last"][0].as_i64(), Some(102), "latest ts in bucket 0");
    assert_eq!(b0["last"][2].as_i64(), Some(3), "its payload");

    let b1 = by_b[&1];
    assert_eq!(b1["first"][0].as_i64(), Some(200));
    assert_eq!(b1["first"][2].as_i64(), Some(8));
    assert_eq!(b1["last"][0].as_i64(), Some(201));
    assert_eq!(b1["last"][2].as_i64(), Some(9));
}
