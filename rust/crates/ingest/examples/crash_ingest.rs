//! Crash-test helper for the durable, exactly-once re-drain (ingest scope). Opens a PERSISTENT
//! store at `argv[1]`, performs the phase named by `argv[2]`, then **hard-aborts** (SIGABRT — no
//! graceful flush). The parent test reopens and asserts exactly-once recovery. Phases:
//!   - `stage-then-kill`  : durable-append a batch to staging, abort BEFORE any commit → the parent
//!     must drain it and commit each sample EXACTLY once (restart re-drain).
//!   - `commit-then-kill` : append, commit ONE batch, abort AFTER the commit returned → the parent
//!     must see that batch once, the remainder still staged, and the re-drain
//!     finish the job without a double-commit.
//!
//! **[`STAGED`] is deliberately multi-batch.** A backlog under one `COMMIT_BATCH` (256) lets the
//! parent's drain LOOP terminate on its first pass, so every loop-termination bug is invisible —
//! the blind spot catalogued in `docs/scope/testing/testing-scope.md` §3.2 and lived through in
//! `docs/debugging/ingest/filtered-batch-stops-the-drain-loop.md`.

use lb_ingest::{commit_batch, write, Qos, Sample};
use lb_store::Store;

/// Samples staged before the kill — three `COMMIT_BATCH`es, so recovery must iterate.
pub const STAGED: u64 = 700;
/// The batch the `commit-then-kill` phase commits before dying: exactly one, so the kill lands
/// mid-backlog with a real remainder behind it.
pub const ONE_BATCH: usize = 256;

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

    let batch: Vec<Sample> = (1..=STAGED).map(|i| sample("m", "pi-7", i)).collect();
    write(&store, "acme", &batch, 0).await.expect("stage");

    match phase.as_str() {
        "stage-then-kill" => {
            // Samples are durably staged; die before the worker drains them.
            std::process::abort();
        }
        "commit-then-kill" => {
            let pass = commit_batch(&store, "acme", ONE_BATCH)
                .await
                .expect("commit");
            assert_eq!(pass.committed, ONE_BATCH);
            // The commit returned (durable); die before any graceful shutdown.
            std::process::abort();
        }
        other => panic!("unknown phase: {other}"),
    }
}
