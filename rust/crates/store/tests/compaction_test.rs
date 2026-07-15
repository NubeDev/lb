//! Log compaction preserves live data. `Store::open` compacts the append-only commit log before
//! SurrealDB opens it (see `open::compact_log`) so a long-running node does not replay its whole
//! write history on every boot. Compaction rewrites only the latest live versions and drops
//! superseded/tombstoned records — so the load-bearing invariant is that it is **lossless for the
//! live set**: every key's newest value survives, and a deleted key stays deleted.
//!
//! These are in-process write→drop→reopen cycles (the reopen is what runs compaction). No mocks —
//! the real SurrealKV engine on a real temp path, same as the crash suite (store scope §4).

use lb_store::{read, write, Store};

/// A fresh on-disk path, unique per case + run.
fn temp_path(tag: &str) -> String {
    std::env::temp_dir()
        .join(format!("lb-compact-{tag}-{}", std::process::id()))
        .to_string_lossy()
        .into_owned()
}

fn cleanup(path: &str) {
    let _ = std::fs::remove_dir_all(path);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn compaction_at_open_preserves_the_live_set() {
    // Build a log with real garbage to compact: write N keys, then OVERWRITE each one several times
    // so the log holds many superseded versions the compactor should drop — while the newest value
    // of every key must survive.
    let path = temp_path("live-set");
    cleanup(&path);

    const N: usize = 32;
    {
        let store = Store::open(&path).await.unwrap();
        for round in 0..4u64 {
            for k in 0..N {
                write(&store, "compact", "kv", &format!("k{k}"), &serde_json::json!({"round": round, "k": k}))
                    .await
                    .unwrap();
            }
        }
    } // drop → release the handle so reopen (with compaction) can take it

    // Reopen: this runs compact_log over the log we just built.
    let store = Store::open(&path).await.expect("reopen compacts, then opens");

    // Every key's LATEST value (round 3) survived compaction — superseded versions dropped, live kept.
    for k in 0..N {
        assert_eq!(
            read(&store, "compact", "kv", &format!("k{k}")).await.unwrap(),
            Some(serde_json::json!({"round": 3, "k": k})),
            "key k{k} must keep its newest value across a compacting reopen"
        );
    }
    cleanup(&path);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn compaction_keeps_a_deleted_key_deleted() {
    // The tombstone case: a key written then deleted must NOT reappear after a compacting reopen,
    // while a sibling key that was never deleted must still be present.
    let path = temp_path("tombstone");
    cleanup(&path);
    {
        let store = Store::open(&path).await.unwrap();
        write(&store, "compact", "kv", "gone", &serde_json::json!({"v": 1}))
            .await
            .unwrap();
        write(&store, "compact", "kv", "kept", &serde_json::json!({"v": 2}))
            .await
            .unwrap();
        lb_store::delete(&store, "compact", "kv", "gone")
            .await
            .unwrap();
    }

    let store = Store::open(&path).await.expect("reopen after delete");
    assert_eq!(
        read(&store, "compact", "kv", "gone").await.unwrap(),
        None,
        "a deleted key stays deleted across a compacting reopen — the tombstone is honored"
    );
    assert_eq!(
        read(&store, "compact", "kv", "kept").await.unwrap(),
        Some(serde_json::json!({"v": 2})),
        "an un-deleted sibling survives compaction"
    );
    cleanup(&path);
}
