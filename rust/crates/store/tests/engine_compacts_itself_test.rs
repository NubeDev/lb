//! Compaction under surrealkv 0.21 — the engine's job, not ours.
//!
//! This replaces `online_compaction_test.rs`, which tested machinery that no longer exists. That
//! suite asserted on `.clog` commit-log segments, a `.merge` crash artifact and a stop-the-world
//! pass that swapped the live handle — all bitcask-era concepts. surrealkv 0.21 is an LSM tree:
//! flush and level-compaction run continuously in its `TaskManager`, so there is no pass to invoke
//! and no artifact to repair.
//!
//! What still needs holding down is the contract that replaced it:
//!   * `compact()` is a no-op that SAYS SO — silence would read as "it ran";
//!   * it no longer stalls writers (the old pass held the handle write guard for ~94 s on RC-6);
//!   * and the engine really does reclaim space on its own, which is the whole reason for the
//!     upgrade and was never true before.
//!
//! Real engine, real temp directories, real bytes — no mocks.

use lb_store::{compact, delete, status, write, Store};

fn temp_path(tag: &str) -> String {
    std::env::temp_dir()
        .join(format!("lb-engine-compact-{tag}-{}", std::process::id()))
        .to_string_lossy()
        .into_owned()
}

fn cleanup(path: &str) {
    let _ = std::fs::remove_dir_all(path);
}

/// Bytes on disc under `path`, following subdirectories (an LSM store spreads over several).
fn dir_bytes(path: &str) -> u64 {
    fn walk(p: &std::path::Path) -> u64 {
        let mut n = 0;
        if let Ok(rd) = std::fs::read_dir(p) {
            for e in rd.flatten() {
                let path = e.path();
                n += if path.is_dir() {
                    walk(&path)
                } else {
                    e.metadata().map(|m| m.len()).unwrap_or(0)
                };
            }
        }
        n
    }
    walk(std::path::Path::new(path))
}

#[tokio::test]
async fn compact_reports_a_no_op_rather_than_pretending_to_run() {
    let path = temp_path("noop");
    cleanup(&path);
    let store = Store::open(&path).await.unwrap();
    write(&store, "ws-a", "kv", "k", &serde_json::json!({"v": 1}))
        .await
        .unwrap();

    let rec = compact(&store).await.expect("compact must not error");
    assert!(rec.ok, "a no-op is still a success");
    let why = rec
        .skipped
        .as_deref()
        .expect("a skipped pass MUST say why — an empty record reads as 'it ran'");
    assert!(
        why.contains("surrealkv"),
        "the reason must name the engine that took the job over, got: {why}"
    );
    assert_eq!(rec.duration_ms, 0, "nothing ran, so nothing took time");

    drop(store);
    cleanup(&path);
}

/// Deliberate contract change, recorded here so it is not mistaken for a regression: the old
/// implementation returned a typed "no commit log" ERROR for an in-memory store. There is no commit
/// log on disc either now, so refusing only for `mem://` would be arbitrary; both report a no-op.
#[tokio::test]
async fn a_memory_store_reports_the_same_no_op() {
    let store = Store::memory().await.unwrap();
    let rec = compact(&store).await.expect("no longer an error");
    assert!(rec.ok && rec.skipped.is_some());
    assert!(!status(&store).persistent);
}

/// The old pass quiesced writes and held the handle's write guard for the length of whole-log I/O.
/// Nothing may block behind `compact()` now.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn compact_does_not_stall_a_concurrent_writer() {
    let path = temp_path("nostall");
    cleanup(&path);
    let store = Store::open(&path).await.unwrap();

    let t0 = std::time::Instant::now();
    let (c, w) = tokio::join!(compact(&store), async {
        write(&store, "ws-a", "kv", "during", &serde_json::json!({"v": 1})).await
    });
    c.expect("compact");
    w.expect("a write issued alongside compact must land, not queue behind it");
    assert!(
        t0.elapsed() < std::time::Duration::from_secs(5),
        "compact + a concurrent write took {:?}; the pass is supposed to be free now",
        t0.elapsed()
    );

    drop(store);
    cleanup(&path);
}

/// The point of the upgrade. Write a lot, delete it all, and the store must give the space back
/// **without anyone asking it to**. Under surrealkv 0.9 this number only ever went up: deletes
/// appended tombstones to an append-only log that nothing but our manual pass ever reclaimed.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_engine_reclaims_deleted_space_on_its_own() {
    let path = temp_path("reclaim");
    cleanup(&path);
    let rows = 4_000;
    let filler = "x".repeat(400);

    let peak = {
        let store = Store::open(&path).await.unwrap();
        for i in 0..rows {
            write(
                &store,
                "ws-a",
                "bulk",
                &format!("k{i}"),
                &serde_json::json!({"i": i, "pad": filler}),
            )
            .await
            .unwrap();
        }
        let peak = dir_bytes(&path);
        for i in 0..rows {
            delete(&store, "ws-a", "bulk", &format!("k{i}"))
                .await
                .unwrap();
        }
        peak
    };

    // Give the background TaskManager room to flush and compact, then reopen so the manifest is
    // re-read. Polling rather than one fixed sleep keeps this from being a timing coin-flip.
    let mut after = dir_bytes(&path);
    let t0 = std::time::Instant::now();
    while t0.elapsed() < std::time::Duration::from_secs(60) {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let store = Store::open(&path).await.unwrap();
        drop(store);
        after = dir_bytes(&path);
        if after < peak / 2 {
            break;
        }
    }

    println!(
        "peak={peak} after={after} ratio={:.3}",
        after as f64 / peak as f64
    );
    // Measured on this suite's first run: peak 2 235 365 B -> 98 101 B, i.e. 4.4% of peak. The
    // bar is set at half, far below what was observed, so ordinary variation in when the background
    // TaskManager runs cannot turn this red — only a real loss of reclamation can.
    assert!(
        after < peak / 2,
        "after deleting every row the store still holds {after} B against a peak of {peak} B — \
         the engine is not reclaiming, which is the bug this upgrade was for"
    );

    cleanup(&path);
}
