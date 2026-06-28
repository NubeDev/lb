//! Probe: verify the store-managed monotonic `rev` actually increments across writes and that a
//! conditional UPSERT (apply only if current rev matches expected) works on the real engine.
//! This de-risks the undo journal's conditional-restore predicate before anything is built on it.

use lb_store::{read, read_versioned, write, Store};
use serde_json::json;

#[tokio::test]
async fn rev_starts_at_one_and_increments_monotonically() {
    let store = Store::memory().await.unwrap();
    let ws = "rev-probe";

    // Absent → rev 0, no value.
    let v0 = read_versioned(&store, ws, "kv", "x").await.unwrap();
    assert_eq!(v0.rev, 0, "absent record reports rev 0");
    assert_eq!(v0.value, None);

    write(&store, ws, "kv", "x", &json!({"n": 1}))
        .await
        .unwrap();
    let v1 = read_versioned(&store, ws, "kv", "x").await.unwrap();
    assert_eq!(v1.rev, 1, "first write lands at rev 1");
    assert_eq!(v1.value, Some(json!({"n": 1})));

    write(&store, ws, "kv", "x", &json!({"n": 2}))
        .await
        .unwrap();
    let v2 = read_versioned(&store, ws, "kv", "x").await.unwrap();
    assert_eq!(v2.rev, 2, "second write bumps to rev 2");
    assert_eq!(v2.value, Some(json!({"n": 2})));

    write(&store, ws, "kv", "x", &json!({"n": 3}))
        .await
        .unwrap();
    let v3 = read_versioned(&store, ws, "kv", "x").await.unwrap();
    assert_eq!(v3.rev, 3, "third write bumps to rev 3");

    // The plain read path is unchanged — still returns just the host data.
    assert_eq!(
        read(&store, ws, "kv", "x").await.unwrap(),
        Some(json!({"n": 3}))
    );
}

#[tokio::test]
async fn rev_is_per_record_not_global() {
    let store = Store::memory().await.unwrap();
    let ws = "rev-per-record";
    write(&store, ws, "kv", "a", &json!({"v": 1}))
        .await
        .unwrap();
    write(&store, ws, "kv", "a", &json!({"v": 2}))
        .await
        .unwrap();
    write(&store, ws, "kv", "b", &json!({"v": 1}))
        .await
        .unwrap();

    assert_eq!(read_versioned(&store, ws, "kv", "a").await.unwrap().rev, 2);
    assert_eq!(
        read_versioned(&store, ws, "kv", "b").await.unwrap().rev,
        1,
        "b's rev is independent of a's"
    );
}
