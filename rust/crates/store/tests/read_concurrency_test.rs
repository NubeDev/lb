//! REGRESSION (store-concurrency scope) — a foreground point-read must NOT stall behind a continuous
//! slow background scan. Before the fix, every store op serialized through one session mutex held
//! across the whole query, so a reactor's long `SELECT` blocked all reads node-wide (foreground reads
//! measured ~400-500ms behind continuous scans; the dashboard-12s report). The per-query-`USE` design
//! (no session mutex; the engine runs queries in parallel) removes that coupling.
//!
//! This pins it: while a background task scans a large table back-to-back, foreground `read`s stay
//! fast. It fails loudly if the serializing mutex is ever reintroduced.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use lb_store::{read, write, Store};

async fn seed_big(store: &Store, ws: &str, rows: usize) {
    let blob = serde_json::json!({ "b": "y".repeat(1200) });
    for k in 0..rows {
        write(store, ws, "big", &format!("b{k}"), &blob)
            .await
            .unwrap();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn foreground_reads_do_not_stall_behind_a_continuous_scan() {
    let store = Store::memory().await.unwrap();
    seed_big(&store, "nube", 8000).await;
    write(
        &store,
        "nube",
        "kv",
        "target",
        &serde_json::json!({ "v": 1 }),
    )
    .await
    .unwrap();

    // Background: scan the big table over and over (each scan is a long-held query on the old design).
    let stop = Arc::new(AtomicBool::new(false));
    let bg = {
        let store = store.clone();
        let stop = Arc::clone(&stop);
        tokio::spawn(async move {
            while !stop.load(Ordering::Relaxed) {
                // A raw unbounded scan — the shape a reactor's `SELECT` takes; on the old design this
                // held the global session mutex for its whole duration.
                let _ = store.query_ws("nube", "SELECT data FROM big", vec![]).await;
            }
        })
    };
    // Let the scanner get going.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Foreground: point reads, spaced so each is a fresh acquisition (the real usage pattern).
    let mut worst = 0u128;
    for _ in 0..10 {
        let t = Instant::now();
        let got = read(&store, "nube", "kv", "target").await.unwrap();
        assert!(
            got.is_some(),
            "read must see the record while scans run concurrently"
        );
        worst = worst.max(t.elapsed().as_millis());
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    stop.store(true, Ordering::Relaxed);
    let _ = bg.await;

    // Generous ceiling: a point read is sub-millisecond warm; the OLD mutex design parked it behind a
    // full scan (hundreds of ms). 100ms leaves huge headroom for CI noise while still catching a
    // reintroduced global serialization (which pushed this to ~400-500ms).
    assert!(
        worst < 100,
        "foreground read stalled {worst}ms behind background scans — the store is serializing reads \
         again (regression: the per-query-USE concurrency was lost)"
    );
}
