//! The compaction **pause measurement** at budget scale — the slice-2 gate of the disk-budget
//! scope (issue #122), reversing `online-compaction-scope.md` OQ5's deferral. Not a pass/fail
//! assertion: it builds a multi-GB real commit log through the real write path and reports the
//! `duration_ms` of one real pass, which is the number the auto-trigger decision is made on.
//!
//! `#[ignore]` — it writes gigabytes and takes minutes. Run it deliberately:
//! `cargo test --release -p lb-store --test compaction_pause_measure_test -- --ignored --nocapture`
//! Override the target with `LB_PAUSE_MEASURE_BYTES` (default 2 GiB).

use lb_store::{compact, status, write, Store};

/// Target log size. The scope asks for "GB, not the 58 KB of the existing session log".
const DEFAULT_TARGET_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Distinct live keys. Small relative to the log so the pass has real dead bytes to reclaim —
/// the 26–65x bloat shape the scope measured in the field, not a live-set-is-the-budget store.
const LIVE_KEYS: usize = 4_000;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "writes gigabytes; run deliberately for the pause measurement"]
async fn measure_compaction_pause_at_budget_scale() {
    let target: u64 = std::env::var("LB_PAUSE_MEASURE_BYTES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_TARGET_BYTES);

    let path = std::env::temp_dir()
        .join(format!("lb-pause-measure-{}", std::process::id()))
        .to_string_lossy()
        .into_owned();
    let _ = std::fs::remove_dir_all(&path);
    let store = Store::open(&path).await.unwrap();

    // Overwrite the same key set round after round: every superseded version stays in the log.
    let pad = "x".repeat(4096);
    let seed_started = std::time::Instant::now();
    let mut round = 0u64;
    loop {
        for k in 0..LIVE_KEYS {
            write(
                &store,
                "ws-measure",
                "kv",
                &format!("k{k}"),
                &serde_json::json!({ "round": round, "pad": pad }),
            )
            .await
            .unwrap();
        }
        round += 1;
        let snap = status(&store);
        println!(
            "seeded round {round}: log_bytes={} ({} MiB) after {}s",
            snap.log_bytes,
            snap.log_bytes / (1024 * 1024),
            seed_started.elapsed().as_secs()
        );
        if snap.log_bytes >= target {
            break;
        }
    }

    let before = status(&store);
    let wall = std::time::Instant::now();
    let rec = compact(&store).await.unwrap();
    let wall_ms = wall.elapsed().as_millis() as u64;

    println!(
        "PAUSE MEASUREMENT: before_bytes={} ({} MiB) after_bytes={} ({} MiB) \
         duration_ms={} wall_ms={} segments_before={} rounds={}",
        rec.before_bytes,
        rec.before_bytes / (1024 * 1024),
        rec.after_bytes,
        rec.after_bytes / (1024 * 1024),
        rec.duration_ms,
        wall_ms,
        before.segment_count,
        round,
    );
    assert!(rec.ok, "the measured pass must succeed: {:?}", rec.error);

    let _ = std::fs::remove_dir_all(&path);
}
