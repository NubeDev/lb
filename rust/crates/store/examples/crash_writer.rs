//! A crash-test helper: open a persistent store at `argv[1]`, perform the write phase named by
//! `argv[2]`, then **hard-abort** (`std::process::abort()` — SIGABRT, no destructors, no flush)
//! at the phase's kill point. The parent crash test (`tests/crash_test.rs`) spawns this, lets it
//! die uncleanly, then reopens the same path and asserts the store recovered — never corrupt.
//!
//! This is the only honest way to test crash-consistency: an in-process drop runs destructors and
//! is a *graceful* close, which proves nothing about a power-cut. A separate process we SIGABRT
//! does. Phases:
//!   - `commit-then-kill` : commit a tx, THEN abort → the committed write must survive.
//!   - `kill-mid-tx`      : begin a tx, write, abort BEFORE commit → must roll back, not half-apply.
//!   - `kill-after-many`  : write a burst (exercises flush/compaction), abort → last commit survives.

use lb_store::{write, Store};

#[tokio::main(flavor = "multi_thread", worker_threads = 1)]
async fn main() {
    let path = std::env::args().nth(1).expect("path arg");
    let phase = std::env::args().nth(2).expect("phase arg");
    let store = Store::open(&path).await.expect("open");

    match phase.as_str() {
        "commit-then-kill" => {
            write(
                &store,
                "crash",
                "kv",
                "committed",
                &serde_json::json!({"v": 1}),
            )
            .await
            .expect("write committed");
            // The write returned (committed). Now die hard before any graceful shutdown.
            std::process::abort();
        }
        "kill-mid-tx" => {
            // Open a transaction, write inside it, and abort BEFORE COMMIT. The partial tx must
            // not be visible after recovery.
            let _ = store
                .query_ws(
                    "crash",
                    "BEGIN TRANSACTION; CREATE kv:inflight SET v = 1;",
                    vec![],
                )
                .await;
            std::process::abort();
        }
        "kill-after-many" => {
            for i in 0..200u64 {
                write(
                    &store,
                    "crash",
                    "kv",
                    &format!("row{i}"),
                    &serde_json::json!({ "v": i }),
                )
                .await
                .expect("burst write");
            }
            // Record the high-water mark as the last committed write, then die mid-flux.
            write(&store, "crash", "kv", "hwm", &serde_json::json!({"v": 199}))
                .await
                .expect("write hwm");
            std::process::abort();
        }
        other => panic!("unknown phase: {other}"),
    }
}
