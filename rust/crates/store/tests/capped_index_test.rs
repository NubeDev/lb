//! Tests for the capped ring's trim **index** (`capped_index.rs`): the trim reads one key's rows
//! through `(cap_key, seq)` instead of scanning the ring, the index is built per workspace, an old
//! unindexed ring picks it up on first write, concurrent first writers all land, and a failed index
//! build never costs the row. The trim semantics themselves are in `capped_test.rs`.
//! Real embedded `mem://` store throughout (testing §0, CLAUDE §9 — no mocks).
//! (`docs/debugging/store/capped-insert-trim-scans-whole-ring.md`)

use lb_store::{capped_insert, new_ulid, Store};
use serde_json::json;

/// Count the rows in `table` (in `ws`) whose `cap_key == key`. The trim invariant is read back
/// through the real store, never a mirror.
async fn count_key(store: &Store, ws: &str, table: &str, key: &str) -> usize {
    let mut resp = store
        .query_ws(
            ws,
            "SELECT count() AS c FROM type::table($tb) WHERE cap_key = $key GROUP ALL",
            vec![("tb".into(), json!(table)), ("key".into(), json!(key))],
        )
        .await
        .expect("count query");
    let rows: Vec<serde_json::Value> = resp.take(0).expect("take count rows");
    rows.first()
        .and_then(|r| r.get("c"))
        .and_then(|c| c.as_u64())
        .unwrap_or(0) as usize
}

/// The trim must read ONE key's rows through the `(cap_key, seq)` index, not scan the ring. A ring
/// holds `cap` rows for every key, so without the index each insert scanned the whole table: a busy
/// node's telemetry ring (~20k rows) cost ~0.6 s per event in a debug build and pegged several cores
/// (`docs/debugging/store/capped-insert-trim-scans-whole-ring.md`). Deterministic: it checks the
/// plan the engine chose, not a wall-clock time.
#[tokio::test]
async fn trim_reads_one_key_through_the_index() {
    let store = Store::memory().await.expect("open store");
    for k in ["a", "b", "c"] {
        for _ in 0..5 {
            capped_insert(&store, "ws", "ring", &new_ulid(), k, 3, &json!({}))
                .await
                .unwrap();
        }
    }

    let mut info = store
        .query_ws("ws", "INFO FOR TABLE ring", vec![])
        .await
        .expect("info");
    let info: Vec<serde_json::Value> = info.take(0).expect("take info");
    assert!(
        info[0]["indexes"].get("capped_key_seq").is_some(),
        "capped_insert defines its trim index: {info:?}"
    );

    // The exact read the trim runs.
    let mut plan = store
        .query_ws(
            "ws",
            "SELECT VALUE seq FROM type::table($tb) WHERE cap_key = $key \
             ORDER BY seq DESC LIMIT 1 START 3 EXPLAIN",
            vec![("tb".into(), json!("ring")), ("key".into(), json!("b"))],
        )
        .await
        .expect("explain");
    let plan: Vec<serde_json::Value> = plan.take(0).expect("take plan");
    let plan = serde_json::to_string(&plan).unwrap();
    assert!(
        plan.contains("IndexScan") && plan.contains("capped_key_seq"),
        "trim read uses the index: {plan}"
    );
    assert!(
        !plan.contains("TableScan"),
        "trim read must not scan the ring: {plan}"
    );

    for k in ["a", "b", "c"] {
        assert_eq!(
            count_key(&store, "ws", "ring", k).await,
            3,
            "key {k} trimmed to cap"
        );
    }
}

/// The table name is interpolated into the index DDL, so anything but a plain identifier is refused
/// before it reaches SurrealDB.
#[tokio::test]
async fn non_identifier_table_is_refused() {
    let store = Store::memory().await.expect("open store");
    let err = capped_insert(
        &store,
        "ws",
        "ring; REMOVE TABLE x",
        "id",
        "k",
        3,
        &json!({}),
    )
    .await
    .expect_err("refused");
    assert!(err.to_string().contains("not a plain identifier"), "{err}");
}

/// `true` when `table` in `ws` carries the trim index.
async fn has_trim_index(store: &Store, ws: &str, table: &str) -> bool {
    let mut info = store
        .query_ws(ws, &format!("INFO FOR TABLE {table}"), vec![])
        .await
        .expect("info");
    let info: Vec<serde_json::Value> = info.take(0).expect("take info");
    info[0]["indexes"].get("capped_key_seq").is_some()
}

/// Write `n` ring rows for `key` straight into `table`, bypassing `capped_insert` — the shape a ring
/// written before the trim index existed has on disk.
async fn seed_unindexed(store: &Store, ws: &str, table: &str, key: &str, n: usize) {
    let rows: Vec<_> = (0..n)
        .map(|_| {
            let id = new_ulid();
            json!({ "id": id.clone(), "cap_key": key, "seq": id })
        })
        .collect();
    store
        .query_ws(
            ws,
            &format!("INSERT INTO {table} $rows"),
            vec![("rows".into(), json!(rows))],
        )
        .await
        .expect("seed rows");
}

/// The index is ensured per WORKSPACE: each workspace is its own namespace, so one store must build
/// it in both. Narrowing the remembered key to the table alone would leave the second workspace
/// scanning, and this is the test that would catch it.
#[tokio::test]
async fn trim_index_is_ensured_in_every_workspace() {
    let store = Store::memory().await.expect("open store");
    for ws in ["ws-a", "ws-b"] {
        capped_insert(&store, ws, "ring", &new_ulid(), "k", 3, &json!({}))
            .await
            .unwrap();
    }
    assert!(has_trim_index(&store, "ws-a", "ring").await);
    assert!(has_trim_index(&store, "ws-b", "ring").await);
    assert_eq!(count_key(&store, "ws-a", "ring", "k").await, 1);
    assert_eq!(
        count_key(&store, "ws-b", "ring", "k").await,
        1,
        "ws-b sees only its own row"
    );
}

/// A ring written before the index existed (the upgrade case) gets the index on its first
/// `capped_insert` and is trimmed back to its cap in that same call.
#[tokio::test]
async fn existing_unindexed_ring_is_indexed_and_trimmed_on_first_insert() {
    let store = Store::memory().await.expect("open store");
    seed_unindexed(&store, "ws", "ring", "k", 50).await;
    assert!(
        !has_trim_index(&store, "ws", "ring").await,
        "old ring has no index"
    );

    capped_insert(&store, "ws", "ring", &new_ulid(), "k", 10, &json!({}))
        .await
        .unwrap();
    assert!(has_trim_index(&store, "ws", "ring").await);
    assert_eq!(count_key(&store, "ws", "ring", "k").await, 10);
}

/// Many writers touching an unindexed ring at once (a busy node's first events after upgrade) all
/// land, the index exists afterwards, and every key is trimmed to its cap. (That the index is BUILT
/// once is the `OnceCell` in `capped_index.rs`; this test does not count builds.)
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_first_inserts_on_an_unindexed_ring_all_land() {
    let store = Store::memory().await.expect("open store");
    for k in 0..5 {
        seed_unindexed(&store, "ws", "ring", &format!("k{k}"), 400).await;
    }
    let tasks: Vec<_> = (0..50)
        .map(|i| {
            let s = store.clone();
            tokio::spawn(async move {
                capped_insert(
                    &s,
                    "ws",
                    "ring",
                    &new_ulid(),
                    &format!("k{}", i % 5),
                    100,
                    &json!({}),
                )
                .await
            })
        })
        .collect();
    for t in tasks {
        t.await.expect("join").expect("every insert lands");
    }
    assert!(has_trim_index(&store, "ws", "ring").await);
    for k in 0..5 {
        assert_eq!(count_key(&store, "ws", "ring", &format!("k{k}")).await, 100);
    }
}

/// The index only makes the trim cheap; a failed index build must not cost the row. SurrealDB
/// refuses to index a COMPUTED field, so a table whose `cap_key` is computed makes the DDL fail
/// while inserts still work.
#[tokio::test]
async fn insert_lands_and_trims_when_the_index_cannot_be_built() {
    let store = Store::memory().await.expect("open store");
    store
        .query_ws(
            "ws",
            "DEFINE FIELD cap_key ON TABLE ring COMPUTED 'k'",
            vec![],
        )
        .await
        .expect("computed field");
    for _ in 0..6 {
        capped_insert(&store, "ws", "ring", &new_ulid(), "k", 4, &json!({}))
            .await
            .expect("insert lands without the index");
    }
    assert!(!has_trim_index(&store, "ws", "ring").await);
    assert_eq!(
        count_key(&store, "ws", "ring", "k").await,
        4,
        "still trimmed, by scan"
    );
}
