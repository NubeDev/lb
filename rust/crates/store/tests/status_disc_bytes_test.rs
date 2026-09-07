//! `lb_store::status` must report what the store ACTUALLY occupies on disc.
//!
//! **The regression this pins.** `log_bytes` was measured by summing `clog/*.clog`, the append-only
//! commit log surrealkv 0.9 wrote. surrealkv 0.21 is an LSM tree: it writes `sstables/`, `wal/`,
//! `vlog/`, `versioned_index/` and `manifest`, and creates no `clog` directory at all. So after the
//! upgrade the measurement found nothing and `log_bytes` collapsed to the size of `manifest` alone —
//! a few kilobytes for a store holding gigabytes.
//!
//! That is not cosmetic. `store_admin`'s budget driver decides purely on this number and returns
//! `Idle` below the soft mark, so a `log_bytes` pinned near zero means the marks are never crossed
//! and the disc budget never fires — the one mechanism between a node and a full disc, inert.
//!
//! The predecessor of this file asserted `log_bytes == clog + manifest` and passed green over
//! exactly that state, because it compared the reported number against the same dead directory the
//! implementation read. So these tests never name a directory: they write real records and require
//! the reported number to MOVE with them. A measurement that reads the wrong place cannot pass.
//!
//! Real SurrealKV engine on a real temp path, real records through the real write path (rule 9).

use lb_store::{status, write, Store};

fn temp_path(tag: &str) -> String {
    std::env::temp_dir()
        .join(format!("lb-status-disc-{tag}-{}", std::process::id()))
        .to_string_lossy()
        .into_owned()
}

/// Sum every byte under `path`, whatever the engine chose to call its directories. This is the
/// ground truth `log_bytes` is checked against — deliberately layout-agnostic, so it stays correct
/// across the next engine change too.
fn every_byte_under(path: &str) -> u64 {
    fn walk(dir: &std::path::Path) -> u64 {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return 0;
        };
        rd.flatten()
            .map(|e| match e.metadata() {
                Ok(m) if m.is_dir() => walk(&e.path()),
                Ok(m) if m.is_file() => m.len(),
                _ => 0,
            })
            .sum()
    }
    walk(std::path::Path::new(path))
}

async fn seed(store: &Store, ws: &str, from: usize, to: usize) {
    for k in from..to {
        write(
            store,
            ws,
            "kv",
            &format!("k{k}"),
            &serde_json::json!({ "k": k, "pad": "x".repeat(512) }),
        )
        .await
        .unwrap();
    }
}

/// `log_bytes` accounts for essentially the whole store directory — not one directory of it, and
/// certainly not the manifest alone.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn log_bytes_accounts_for_what_is_really_on_disc() {
    let path = temp_path("whole");
    let _ = std::fs::remove_dir_all(&path);
    let store = Store::open(&path).await.unwrap();
    seed(&store, "ws-a", 0, 200).await;

    let reported = status(&store).log_bytes;
    let actual = every_byte_under(&path);

    assert!(
        actual > 0,
        "a real store directory holds real bytes — got none at {path}"
    );
    // At least 90% of the directory. Not exact equality: the engine is free to drop a lock file or
    // a scratch entry beside the directories it documents, and a test that breaks on one stray byte
    // teaches the next reader to loosen it rather than to look.
    assert!(
        reported * 10 >= actual * 9,
        "log_bytes must account for the store directory: reported {reported}, on disc {actual}"
    );

    drop(store);
    let _ = std::fs::remove_dir_all(&path);
}

/// The property the budget driver actually depends on: write more, and the number GOES UP. This is
/// what a measurement pointed at a dead directory can never do.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn log_bytes_grows_as_records_are_written() {
    let path = temp_path("grows");
    let _ = std::fs::remove_dir_all(&path);
    let store = Store::open(&path).await.unwrap();

    seed(&store, "ws-a", 0, 50).await;
    let small = status(&store).log_bytes;

    seed(&store, "ws-a", 50, 1_000).await;
    let large = status(&store).log_bytes;

    assert!(small > 0, "50 real records ⇒ measurable bytes, got {small}");
    assert!(
        large > small,
        "950 more records must raise log_bytes: {small} → {large}"
    );

    drop(store);
    let _ = std::fs::remove_dir_all(&path);
}

/// A memory store has no directory, so every byte field is zero — and `status` must not panic or
/// invent a number for it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_memory_store_reports_zero_and_not_persistent() {
    let store = Store::memory().await.unwrap();
    seed(&store, "ws-a", 0, 20).await;

    let snap = status(&store);
    assert!(!snap.persistent);
    assert_eq!(snap.log_bytes, 0);
    assert_eq!(snap.segment_count, 0);
}
