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
                write(
                    &store,
                    "compact",
                    "kv",
                    &format!("k{k}"),
                    &serde_json::json!({"round": round, "k": k}),
                )
                .await
                .unwrap();
            }
        }
    } // drop → release the handle so reopen (with compaction) can take it

    // Reopen: this runs compact_log over the log we just built.
    let store = Store::open(&path)
        .await
        .expect("reopen compacts, then opens");

    // Every key's LATEST value (round 3) survived compaction — superseded versions dropped, live kept.
    for k in 0..N {
        assert_eq!(
            read(&store, "compact", "kv", &format!("k{k}"))
                .await
                .unwrap(),
            Some(serde_json::json!({"round": 3, "k": k})),
            "key k{k} must keep its newest value across a compacting reopen"
        );
    }
    cleanup(&path);
}

/// REGRESSION — debugging/store/compaction-merge-eats-next-sessions-writes.md (P0).
/// surrealkv 0.9.3 applies a pending compaction merge with the append-log already open, so any
/// session that applies a merge appends into unlinked inodes and its writes vanish at close.
/// On the shipped boot path that meant: every boot from the THIRD onward destroyed all writes
/// made since the previous boot. `compact_log` now completes the merge with a throwaway,
/// non-writing open before surrealdb ever opens the dir. This test is 8 open→write→close
/// cycles (each `Store::open` compacts); every cycle's sentinel must survive all later cycles.
/// FAILS before the fix (loses every post-first-compaction sentinel), passes after.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn repeated_compaction_cycles_keep_every_sessions_writes() {
    let path = temp_path("cycles");
    cleanup(&path);
    for round in 0..8u64 {
        let store = Store::open(&path).await.unwrap();
        for prev in 0..round {
            assert_eq!(
                read(&store, "compact", "kv", &format!("s{prev}"))
                    .await
                    .unwrap(),
                Some(serde_json::json!({"r": prev})),
                "cycle {round}: sentinel s{prev} written {} compacting reopens ago must survive",
                round - prev
            );
        }
        write(
            &store,
            "compact",
            "kv",
            &format!("s{round}"),
            &serde_json::json!({"r": round}),
        )
        .await
        .unwrap();
        drop(store);
        // The engine releases its files asynchronously after the drop (spike Q2: 74–240 ms);
        // wait for release before the next open, exactly as a real reboot would.
        wait_release(&path).await;
    }
    cleanup(&path);
}

/// Wait until this process holds no fd under `dir` — the engine's async shutdown after drop.
async fn wait_release(dir: &str) {
    let t0 = std::time::Instant::now();
    loop {
        let open = std::fs::read_dir("/proc/self/fd")
            .map(|rd| {
                rd.flatten()
                    .filter_map(|e| std::fs::read_link(e.path()).ok())
                    .any(|t| t.starts_with(dir))
            })
            .unwrap_or(false);
        if !open || t0.elapsed().as_secs() > 10 {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn compaction_keeps_a_deleted_key_deleted() {
    // The tombstone case: a key written then deleted must NOT reappear after a compacting reopen,
    // while a sibling key that was never deleted must still be present.
    let path = temp_path("tombstone");
    cleanup(&path);
    {
        let store = Store::open(&path).await.unwrap();
        write(
            &store,
            "compact",
            "kv",
            "gone",
            &serde_json::json!({"v": 1}),
        )
        .await
        .unwrap();
        write(
            &store,
            "compact",
            "kv",
            "kept",
            &serde_json::json!({"v": 2}),
        )
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
