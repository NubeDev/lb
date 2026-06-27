//! Engine parity + workspace isolation on the PERSISTENT engine. The scope requires the existing
//! store isolation/verb behavior to pass *identically* on `Store::open()` as on `Store::memory()` —
//! proving the engine swap is transparent above the open seam and the hard wall holds on disk, not
//! just in memory.

use lb_store::{list, read, write, write_tx, Store, Upsert};

fn temp_path(tag: &str) -> String {
    std::env::temp_dir()
        .join(format!("lb-parity-{tag}-{}", std::process::id()))
        .to_string_lossy()
        .into_owned()
}

fn cleanup(path: &str) {
    let _ = std::fs::remove_dir_all(path);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn write_read_round_trips_on_disk() {
    let path = temp_path("rw");
    cleanup(&path);
    let store = Store::open(&path).await.unwrap();
    write(&store, "acme", "thing", "1", &serde_json::json!({"a": 1}))
        .await
        .unwrap();
    assert_eq!(
        read(&store, "acme", "thing", "1").await.unwrap(),
        Some(serde_json::json!({"a": 1}))
    );
    assert_eq!(
        read(&store, "acme", "thing", "missing").await.unwrap(),
        None
    );
    cleanup(&path);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_cannot_read_ws_a_record_on_disk() {
    // The hard wall, on disk: the SAME table:id in two namespaces does not bleed.
    let path = temp_path("iso");
    cleanup(&path);
    let store = Store::open(&path).await.unwrap();
    write(
        &store,
        "ws-a",
        "secret",
        "x",
        &serde_json::json!({"who": "a"}),
    )
    .await
    .unwrap();
    // ws-B reading the identical key sees nothing of A's.
    assert_eq!(read(&store, "ws-b", "secret", "x").await.unwrap(), None);
    // And ws-B writing its own does not touch A's.
    write(
        &store,
        "ws-b",
        "secret",
        "x",
        &serde_json::json!({"who": "b"}),
    )
    .await
    .unwrap();
    assert_eq!(
        read(&store, "ws-a", "secret", "x").await.unwrap(),
        Some(serde_json::json!({"who": "a"}))
    );
    cleanup(&path);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn list_filters_by_field_on_disk() {
    let path = temp_path("list");
    cleanup(&path);
    let store = Store::open(&path).await.unwrap();
    write(
        &store,
        "acme",
        "inbox",
        "a",
        &serde_json::json!({"channel": "c1", "ts": 1}),
    )
    .await
    .unwrap();
    write(
        &store,
        "acme",
        "inbox",
        "b",
        &serde_json::json!({"channel": "c2", "ts": 2}),
    )
    .await
    .unwrap();
    let c1 = list(&store, "acme", "inbox", "channel", "c1")
        .await
        .unwrap();
    assert_eq!(c1.len(), 1);
    assert_eq!(c1[0]["channel"], "c1");
    cleanup(&path);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn write_tx_is_atomic_on_disk() {
    let path = temp_path("tx");
    cleanup(&path);
    let store = Store::open(&path).await.unwrap();
    let cv = serde_json::json!({"kind": "change"});
    let ev = serde_json::json!({"kind": "effect"});
    write_tx(
        &store,
        "acme",
        &Upsert {
            table: "domain",
            id: "d1",
            value: &cv,
        },
        &Upsert {
            table: "outbox",
            id: "e1",
            value: &ev,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        read(&store, "acme", "domain", "d1").await.unwrap(),
        Some(cv)
    );
    assert_eq!(
        read(&store, "acme", "outbox", "e1").await.unwrap(),
        Some(ev)
    );
    cleanup(&path);
}
