//! Crash-consistency: the durability foundation needs more than one happy-path reopen, because
//! ingest's entire "never lost until on disk" guarantee rests here (store scope). Each case spawns
//! the `crash_writer` example as a SEPARATE PROCESS and SIGABRTs it at a chosen point — an
//! in-process `drop` runs destructors (a graceful close) and would prove nothing about a power-cut.
//! The parent then reopens the same on-disk path and asserts the engine recovered, never corrupt.
//!
//! Covers the scope's full set: write→drop→reopen present; kill mid-tx → rolled back; kill during a
//! flush/compaction burst → last committed survives; reopen after an unclean kill → recovers.

use std::process::Command;

use lb_store::{read, Store};

/// A fresh on-disk path, unique per case + run.
fn temp_path(tag: &str) -> String {
    std::env::temp_dir()
        .join(format!("lb-crash-{tag}-{}", std::process::id()))
        .to_string_lossy()
        .into_owned()
}

fn cleanup(path: &str) {
    let _ = std::fs::remove_dir_all(path);
}

/// Run the crash_writer example for `phase` at `path` and assert it died by signal (SIGABRT),
/// i.e. it did NOT exit cleanly — the precondition for an honest crash test.
fn crash_at(path: &str, phase: &str) {
    let status = Command::new(env!("CARGO"))
        .args(["run", "--quiet", "--example", "crash_writer", "--", path, phase])
        .status()
        .expect("spawn crash_writer");
    // SIGABRT → no clean exit code; on Unix the process is terminated by signal.
    assert!(
        !status.success(),
        "crash_writer for phase {phase} must die uncleanly, not exit 0"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn write_drop_reopen_is_present() {
    // The baseline: an in-process write, handle dropped (clean), reopened — present.
    let path = temp_path("baseline");
    cleanup(&path);
    {
        let store = Store::open(&path).await.unwrap();
        lb_store::write(&store, "crash", "kv", "x", &serde_json::json!({"v": 7}))
            .await
            .unwrap();
    }
    let store = Store::open(&path).await.unwrap();
    assert_eq!(
        read(&store, "crash", "kv", "x").await.unwrap(),
        Some(serde_json::json!({"v": 7}))
    );
    cleanup(&path);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn commit_then_hard_kill_survives() {
    // A committed write followed by SIGABRT must survive recovery — the core durability promise.
    let path = temp_path("commit-kill");
    cleanup(&path);
    crash_at(&path, "commit-then-kill");

    let store = Store::open(&path).await.expect("reopen after SIGABRT");
    assert_eq!(
        read(&store, "crash", "kv", "committed").await.unwrap(),
        Some(serde_json::json!({"v": 1})),
        "a write that returned (committed) must survive a hard kill"
    );
    cleanup(&path);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn kill_mid_transaction_rolls_back() {
    // A transaction killed BEFORE COMMIT must leave nothing — atomicity is what batch-commit needs.
    let path = temp_path("mid-tx");
    cleanup(&path);
    crash_at(&path, "kill-mid-tx");

    let store = Store::open(&path).await.expect("reopen after mid-tx kill");
    assert_eq!(
        read(&store, "crash", "kv", "inflight").await.unwrap(),
        None,
        "an uncommitted transaction must roll back on recovery, not half-apply"
    );
    cleanup(&path);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn kill_during_flush_burst_keeps_last_commit() {
    // A burst (exercises flush/compaction) then SIGABRT: the last committed write must survive and
    // the store must reopen without corruption (the half-WAL / torn-tail recovery case).
    let path = temp_path("flush-burst");
    cleanup(&path);
    crash_at(&path, "kill-after-many");

    let store = Store::open(&path).await.expect("reopen after flush-burst kill");
    // The high-water mark write returned before the abort, so it must be durable.
    assert_eq!(
        read(&store, "crash", "kv", "hwm").await.unwrap(),
        Some(serde_json::json!({"v": 199})),
        "last committed write must survive a kill during a flush burst"
    );
    // And the store is readable (not corrupt) — a sample of the burst round-trips.
    let row0 = read(&store, "crash", "kv", "row0").await.unwrap();
    assert_eq!(row0, Some(serde_json::json!({"v": 0})));
    cleanup(&path);
}
