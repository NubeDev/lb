//! `write_if_rev`: a write lands only while the record is still at the revision the caller read.
//! Real `mem://` store, no mocks.

use lb_store::{read, read_versioned, write, write_if_rev, RevWrite, Store};
use serde_json::json;

#[tokio::test]
async fn rev_zero_creates_once() {
    let store = Store::memory().await.unwrap();
    let ws = "wir-create";
    let first = write_if_rev(&store, ws, "kv", "x", &json!({"n": 1}), 0)
        .await
        .unwrap();
    assert_eq!(first, RevWrite::Written { rev: 1 });
    let again = write_if_rev(&store, ws, "kv", "x", &json!({"n": 2}), 0)
        .await
        .unwrap();
    assert_eq!(
        again,
        RevWrite::Stale,
        "rev 0 means the record must not exist yet"
    );
    assert_eq!(
        read(&store, ws, "kv", "x").await.unwrap(),
        Some(json!({"n": 1}))
    );
}

#[tokio::test]
async fn the_current_rev_writes_and_bumps() {
    let store = Store::memory().await.unwrap();
    let ws = "wir-bump";
    write(&store, ws, "kv", "x", &json!({"n": 1}))
        .await
        .unwrap();
    write(&store, ws, "kv", "x", &json!({"n": 2}))
        .await
        .unwrap();
    let out = write_if_rev(&store, ws, "kv", "x", &json!({"n": 3}), 2)
        .await
        .unwrap();
    assert_eq!(out, RevWrite::Written { rev: 3 });
    let live = read_versioned(&store, ws, "kv", "x").await.unwrap();
    assert_eq!(live.rev, 3);
    assert_eq!(live.value, Some(json!({"n": 3})));
}

#[tokio::test]
async fn a_stale_rev_changes_nothing() {
    let store = Store::memory().await.unwrap();
    let ws = "wir-stale";
    write(&store, ws, "kv", "x", &json!({"n": 1}))
        .await
        .unwrap();
    assert_eq!(
        write_if_rev(&store, ws, "kv", "x", &json!({"n": 9}), 5)
            .await
            .unwrap(),
        RevWrite::Stale
    );
    assert_eq!(
        write_if_rev(&store, ws, "kv", "x", &json!({"n": 9}), 0)
            .await
            .unwrap(),
        RevWrite::Stale
    );
    let live = read_versioned(&store, ws, "kv", "x").await.unwrap();
    assert_eq!(live.rev, 1);
    assert_eq!(live.value, Some(json!({"n": 1})));
}

/// A row written before revisions existed has no `rev`; the guard counts it as
/// rev 1 (what `Record`'s serde default reads it as).
#[tokio::test]
async fn a_row_written_before_revisions_counts_as_rev_1() {
    let store = Store::memory().await.unwrap();
    let ws = "wir-legacy";
    store
        .query_ws(
            ws,
            "UPSERT type::record('kv', 'x') CONTENT { data: { n: 1 } } RETURN NONE",
            vec![],
        )
        .await
        .unwrap();
    assert_eq!(
        write_if_rev(&store, ws, "kv", "x", &json!({"n": 0}), 0)
            .await
            .unwrap(),
        RevWrite::Stale
    );
    assert_eq!(
        write_if_rev(&store, ws, "kv", "x", &json!({"n": 2}), 1)
            .await
            .unwrap(),
        RevWrite::Written { rev: 2 }
    );
}

/// The workspace wall: the same `table:id` in another workspace has its own revision.
#[tokio::test]
async fn revisions_are_per_workspace() {
    let store = Store::memory().await.unwrap();
    let a = write_if_rev(&store, "wir-a", "kv", "x", &json!({"n": 1}), 0)
        .await
        .unwrap();
    let b = write_if_rev(&store, "wir-b", "kv", "x", &json!({"n": 1}), 0)
        .await
        .unwrap();
    assert_eq!(a, RevWrite::Written { rev: 1 });
    assert_eq!(b, RevWrite::Written { rev: 1 });
}

/// Two processes holding the same `rev` can never both land (design §5).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn writers_racing_on_one_rev_land_exactly_once() {
    let store = Store::memory().await.unwrap();
    let ws = "wir-race";
    write(&store, ws, "kv", "x", &json!({"n": 0}))
        .await
        .unwrap();
    let mut tasks = Vec::new();
    for n in 1..=8 {
        let s = store.clone();
        tasks.push(tokio::spawn(async move {
            write_if_rev(&s, "wir-race", "kv", "x", &json!({ "n": n }), 1)
                .await
                .unwrap()
        }));
    }
    let mut written = 0;
    for t in tasks {
        if let RevWrite::Written { rev } = t.await.unwrap() {
            assert_eq!(rev, 2);
            written += 1;
        }
    }
    assert_eq!(written, 1, "exactly one writer at rev 1 may land");
    assert_eq!(read_versioned(&store, ws, "kv", "x").await.unwrap().rev, 2);
}
