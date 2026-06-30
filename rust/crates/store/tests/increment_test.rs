//! `increment` — the atomic server-side numeric accumulate behind a stateful counter node
//! (flow-multi-trigger-reactive scope). Proves the running total goes up correctly even when N
//! firings race the SAME record — the lost-update a host-side read-then-write would suffer is
//! impossible because the add happens inside the UPSERT. Real embedded `mem://`, no mocks (CLAUDE §9).

use lb_store::{increment, read, Store};

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn increment_accumulates_and_resets() {
    let store = Store::memory().await.expect("open store");
    let ws = "acme";
    let (tb, id) = ("flow_node_memory", "flow1:counter1");

    assert_eq!(
        increment(&store, ws, tb, id, 1, false, 10).await.unwrap(),
        1
    );
    assert_eq!(
        increment(&store, ws, tb, id, 1, false, 11).await.unwrap(),
        2
    );
    assert_eq!(
        increment(&store, ws, tb, id, 5, false, 12).await.unwrap(),
        7
    );
    // reset zeroes the prior total, then applies this firing's `by`.
    assert_eq!(increment(&store, ws, tb, id, 3, true, 13).await.unwrap(), 3);
    // the durable value is readable as the node's last-value (and `ts` stamped).
    let v = read(&store, ws, tb, id).await.unwrap().unwrap();
    assert_eq!(v["count"], 3);
    assert_eq!(v["ts"], 13);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_increments_never_lose_a_count() {
    let store = Store::memory().await.expect("open store");
    let ws = "acme";
    let (tb, id) = ("flow_node_memory", "flow1:c");
    let n = 64;

    let mut handles = Vec::new();
    for i in 0..n {
        let s = store.clone();
        handles.push(tokio::spawn(async move {
            increment(&s, ws, tb, id, 1, false, 100 + i as u64)
                .await
                .expect("increment")
        }));
    }
    let mut totals = Vec::new();
    for h in handles {
        totals.push(h.await.expect("join"));
    }
    // Every firing observed a distinct running total (no two saw the same value) and the final total
    // is exactly N — a host-side read-modify-write would lose updates here; the atomic add cannot.
    totals.sort_unstable();
    assert_eq!(
        totals,
        (1..=n).collect::<Vec<i64>>(),
        "each firing got a unique total 1..=N"
    );
    let v = read(&store, ws, tb, id).await.unwrap().unwrap();
    assert_eq!(v["count"], n, "final running total == number of firings");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn counter_memory_is_workspace_walled() {
    let store = Store::memory().await.expect("open store");
    let (tb, id) = ("flow_node_memory", "flow1:c");
    increment(&store, "ws-a", tb, id, 10, false, 1)
        .await
        .unwrap();
    // The SAME table:id in another workspace starts from zero (the hard wall, README §7).
    assert_eq!(
        increment(&store, "ws-b", tb, id, 1, false, 1)
            .await
            .unwrap(),
        1
    );
    // ws-a is untouched.
    assert_eq!(
        read(&store, "ws-a", tb, id).await.unwrap().unwrap()["count"],
        10
    );
}
