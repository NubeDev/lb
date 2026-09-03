//! surrealkv 0.21 takes an exclusive lock on the store directory. Two questions decide whether that
//! is a footnote or a fleet incident, and this answers both against the real engine.
//!
//!   1. Can a store be reopened straight after its handle is dropped? `store_boot_guard_test` says
//!      no — it drops a handle, reopens the same path, and gets "Database at <path>/store/LOCK is
//!      already locked by another process". So the release is not synchronous with drop.
//!   2. Does a LOCK left behind by a HARD-KILLED process block a restart for ever? That is the one
//!      that matters: "the node will not start after a power cut" is far worse than a slow reopen.
//!      A lock held by a live process is correct; a lock that outlives the process is a brick.

use lb_store::Store;

fn temp_path(tag: &str) -> String {
    std::env::temp_dir()
        .join(format!("lb-lock-probe-{tag}-{}", std::process::id()))
        .to_string_lossy()
        .into_owned()
}

/// How long after a drop the path becomes openable again. Reported, not asserted at a fixed value —
/// the number is what the operational fix has to tolerate.
#[tokio::test]
async fn how_long_until_a_dropped_store_can_be_reopened() {
    let path = temp_path("reopen");
    let _ = std::fs::remove_dir_all(&path);

    let store = Store::open(&path).await.expect("first open");
    store
        .query_ws("ws-a", "CREATE l:one SET v = 1", vec![])
        .await
        .expect("seed");
    drop(store);

    let t0 = std::time::Instant::now();
    let mut attempts = 0u32;
    let waited = loop {
        attempts += 1;
        match Store::open(&path).await {
            Ok(s) => {
                drop(s);
                break t0.elapsed();
            }
            Err(_) if t0.elapsed() < std::time::Duration::from_secs(30) => {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            Err(e) => panic!("still locked after 30s ({attempts} attempts): {e}"),
        }
    };
    println!("reopen after drop: {waited:?} over {attempts} attempt(s)");

    let _ = std::fs::remove_dir_all(&path);
}

/// THE ONE THAT MATTERS. Simulate a power cut: a child process opens the store and is SIGKILLed, so
/// nothing runs on its way out and the LOCK file is left on disc. A restart must still come up.
#[tokio::test]
async fn a_store_locked_by_a_hard_killed_process_still_reopens() {
    let path = temp_path("sigkill");
    let _ = std::fs::remove_dir_all(&path);

    // Create the store so the directory and its LOCK exist, then leave the file behind by hand —
    // this is what a SIGKILLed process leaves: a LOCK whose owner no longer exists.
    {
        let store = Store::open(&path).await.expect("create");
        store
            .query_ws("ws-a", "CREATE l:one SET v = 1", vec![])
            .await
            .expect("seed");
    }
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let lock = std::path::Path::new(&path).join("store").join("LOCK");
    println!("LOCK present after clean close: {}", lock.exists());
    if !lock.exists() {
        // Forge one, so the "stale lock" case is exercised even if a clean close removes it.
        std::fs::create_dir_all(lock.parent().unwrap()).ok();
        std::fs::write(&lock, b"999999").expect("forge a stale LOCK");
        println!("forged a stale LOCK to stand in for a killed owner");
    }

    match Store::open(&path).await {
        Ok(store) => {
            let mut resp = store
                .query_ws("ws-a", "SELECT v FROM l", vec![])
                .await
                .expect("read after recovering from a stale lock");
            let rows: Vec<serde_json::Value> = resp.take(0).expect("take");
            println!("recovered, rows = {rows:?}");
        }
        Err(e) => panic!(
            "a stale LOCK from a killed process BLOCKS restart — this bricks a node after a power \
             cut and must be fixed before shipping: {e}"
        ),
    }

    let _ = std::fs::remove_dir_all(&path);
}
