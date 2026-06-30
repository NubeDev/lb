//! `write_locked` — the conflict-safe rev-bumping write (flow-plc-reliability scope). Proves the
//! same-record race that broke the flows run-store CANNOT escape as an error and leaves a consistent
//! monotonic `rev`. Real embedded `mem://` store, no mocks (CLAUDE §9). Mirrors
//! `capped_test::concurrent_inserts_past_cap_leave_exactly_cap` in shape — the precedent this fix
//! ports (`debugging/observability/capped-insert-overgrows-cap-under-concurrency.md`).

use lb_store::{read, write_locked, Store};
use serde_json::json;

/// Read the stored `rev` for `table:id` (the monotonic optimistic-concurrency token). Read through
/// the real query surface, never a mirror.
async fn rev(store: &Store, ws: &str, table: &str, id: &str) -> Option<i64> {
    let mut resp = store
        .query_ws(
            ws,
            "SELECT VALUE rev FROM ONLY type::thing($tb, $id)",
            vec![("tb".into(), json!(table)), ("id".into(), json!(id))],
        )
        .await
        .expect("rev query");
    let v: Option<i64> = resp.take(0).expect("take rev");
    v
}

/// N concurrent writers to the SAME record must all succeed (no `Invalid revision` / transaction
/// conflict escapes) and leave the record present with a coherent `rev`. On today's bare `write`
/// this races into a `read or write conflict` under the durable engine; `write_locked` serializes +
/// retries so every writer lands.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_same_record_writes_never_conflict() {
    let store = Store::memory().await.expect("open store");
    let ws = "acme";
    let n = 16;

    let mut handles = Vec::new();
    for i in 0..n {
        let s = store.clone();
        handles.push(tokio::spawn(async move {
            write_locked(&s, ws, "flow_run", "chain4-run-X", &json!({ "writer": i }))
                .await
                .map_err(|e| e.to_string())
        }));
    }
    for h in handles {
        // The whole point: not one writer returns an error.
        h.await
            .expect("task joined")
            .expect("write_locked must not error under same-record race");
    }

    // The record exists exactly once and its rev advanced once per write (serialized) — a coherent,
    // non-corrupt token, readable without an `Invalid revision`.
    let got = read(&store, ws, "flow_run", "chain4-run-X")
        .await
        .expect("read after race")
        .expect("record present");
    assert!(got.get("data").is_some() || got.get("writer").is_some());
    assert_eq!(
        rev(&store, ws, "flow_run", "chain4-run-X").await,
        Some(n as i64)
    );
}
