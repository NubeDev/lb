//! Crash-test helper for the durable, exactly-once re-drain (ingest scope). Opens a PERSISTENT
//! store at `argv[1]`, performs the phase named by `argv[2]`, then **hard-aborts** (SIGABRT — no
//! graceful flush). The parent test reopens and asserts exactly-once recovery. Phases:
//!   - `stage-then-kill`  : durable-append a batch to staging, abort BEFORE any commit → the parent
//!                          must drain it and commit each sample EXACTLY once (restart re-drain).
//!   - `commit-then-kill` : append, commit one batch, abort AFTER the commit returned → the parent
//!                          must see the batch committed once and staging empty (no double-commit).

use lb_ingest::{commit_batch, write, Qos, Sample};
use lb_store::Store;

fn sample(series: &str, producer: &str, seq: u64) -> Sample {
    Sample {
        series: series.into(),
        producer: producer.into(),
        ts: seq,
        seq,
        payload: serde_json::json!(seq),
        labels: serde_json::json!({}),
        qos: Qos::MustDeliver,
    }
}

#[tokio::main(flavor = "multi_thread", worker_threads = 1)]
async fn main() {
    let path = std::env::args().nth(1).expect("path");
    let phase = std::env::args().nth(2).expect("phase");
    let store = Store::open(&path).await.expect("open");

    let batch: Vec<Sample> = (1..=5).map(|i| sample("m", "pi-7", i)).collect();
    write(&store, "acme", &batch, 0).await.expect("stage");

    match phase.as_str() {
        "stage-then-kill" => {
            // Samples are durably staged; die before the worker drains them.
            std::process::abort();
        }
        "commit-then-kill" => {
            let pass = commit_batch(&store, "acme", 100).await.expect("commit");
            assert_eq!(pass.committed, 5);
            // The commit returned (durable); die before any graceful shutdown.
            std::process::abort();
        }
        other => panic!("unknown phase: {other}"),
    }
}
