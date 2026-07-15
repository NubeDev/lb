//! Online commit-log compaction (`lb_store::compact`) — the runtime pass the online-compaction
//! scope ships (issue #67). Real SurrealKV engine on real temp paths, real bytes — no mocks
//! (rule 9). Covers the scope's key cases: shrink with survivors intact across workspaces,
//! concurrent writers blocking on the session mutex and landing after the swap, crash-window
//! artifacts (`.merge` / `.tmp.merge` left behind), the boot dividend, and the memory-store
//! refusal.

use lb_store::{compact, delete, read, status, write, Store};

fn temp_path(tag: &str) -> String {
    std::env::temp_dir()
        .join(format!("lb-online-compact-{tag}-{}", std::process::id()))
        .to_string_lossy()
        .into_owned()
}

fn cleanup(path: &str) {
    let _ = std::fs::remove_dir_all(path);
}

/// Shrink: build a log that is mostly dead bytes (overwrites + deletes through the REAL write
/// path), run the online pass on the LIVE handle, and assert the log shrank while every
/// surviving record in BOTH workspaces reads back intact.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn online_pass_shrinks_log_and_keeps_live_set() {
    let path = temp_path("shrink");
    cleanup(&path);
    let store = Store::open(&path).await.unwrap();

    // Two workspaces — compaction operates below the namespace wall; both must survive.
    for ws in ["ws-a", "ws-b"] {
        for k in 0..40 {
            // 6 superseded versions per key + the live one.
            for round in 0..7u64 {
                write(
                    &store,
                    ws,
                    "kv",
                    &format!("k{k}"),
                    &serde_json::json!({"round": round, "k": k, "pad": "x".repeat(256)}),
                )
                .await
                .unwrap();
            }
        }
        // Evict half of them through the real delete path — tombstones, the retention shape.
        for k in 0..20 {
            delete(&store, ws, "kv", &format!("k{k}")).await.unwrap();
        }
    }

    let before = status(&store);
    assert!(before.log_bytes > 0, "grown log must be measurable");

    let rec = compact(&store).await.expect("online pass succeeds");
    assert!(rec.ok);
    assert!(
        rec.after_bytes < rec.before_bytes / 2,
        "log must shrink to well under half (mostly dead bytes): before {} after {}",
        rec.before_bytes,
        rec.after_bytes
    );

    // The LIVE handle (same Store value, post-swap) serves the surviving set intact.
    for ws in ["ws-a", "ws-b"] {
        for k in 0..20 {
            assert_eq!(
                read(&store, ws, "kv", &format!("k{k}")).await.unwrap(),
                None,
                "{ws}: deleted k{k} stays deleted after the pass"
            );
        }
        for k in 20..40 {
            let got = read(&store, ws, "kv", &format!("k{k}")).await.unwrap();
            assert_eq!(
                got.as_ref()
                    .and_then(|v| v.get("round"))
                    .and_then(|r| r.as_u64()),
                Some(6),
                "{ws}: k{k} must keep its newest value across the online pass"
            );
        }
    }

    // status reflects the pass.
    let after = status(&store);
    assert!(after.persistent);
    // The reopened engine appends its own bookkeeping after the pass — status tracks the
    // compacted size closely, not exactly.
    assert!(
        after.log_bytes >= rec.after_bytes && after.log_bytes < rec.before_bytes / 2,
        "status log_bytes ({}) must reflect the compacted log ({} .. {})",
        after.log_bytes,
        rec.after_bytes,
        rec.before_bytes / 2
    );
    let last = after.last_compaction.expect("last pass recorded");
    assert!(last.ok);
    cleanup(&path);
}

/// Writers racing the pass: they block on the session mutex, land after the swap — none lost,
/// none duplicated, and every one durable across a reopen.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_writers_during_pass_all_land() {
    let path = temp_path("racers");
    cleanup(&path);
    let store = Store::open(&path).await.unwrap();

    // Seed garbage so the pass has real work to do.
    for k in 0..50 {
        for round in 0..5u64 {
            write(
                &store,
                "race",
                "kv",
                &format!("seed{k}"),
                &serde_json::json!({"round": round}),
            )
            .await
            .unwrap();
        }
    }

    // Launch 16 writers concurrently WITH the pass. Every write must land exactly once.
    let mut handles = Vec::new();
    for w in 0..16u64 {
        let s = store.clone();
        handles.push(tokio::spawn(async move {
            write(
                &s,
                "race",
                "kv",
                &format!("racer{w}"),
                &serde_json::json!({"w": w}),
            )
            .await
            .unwrap();
        }));
    }
    let pass = {
        let s = store.clone();
        tokio::spawn(async move { compact(&s).await })
    };
    for h in handles {
        h.await.unwrap();
    }
    pass.await.unwrap().expect("pass succeeds with racers");

    for w in 0..16u64 {
        assert_eq!(
            read(&store, "race", "kv", &format!("racer{w}"))
                .await
                .unwrap(),
            Some(serde_json::json!({"w": w})),
            "racer{w} must land exactly once, before or after the swap"
        );
    }

    // And durably: reopen the store and re-check.
    drop(store);
    let store = Store::open(&path).await.unwrap();
    for w in 0..16u64 {
        assert_eq!(
            read(&store, "race", "kv", &format!("racer{w}"))
                .await
                .unwrap(),
            Some(serde_json::json!({"w": w})),
            "racer{w} must survive a reopen after the pass"
        );
    }
    cleanup(&path);
}

/// Crash windows: a pass interrupted after `compact()` leaves `.merge/` pending; one
/// interrupted mid-rewrite leaves `.tmp.merge/`. Reopen must succeed on either the old or the
/// new log — never a corrupt one, and never (the P0) a store that eats the next session's
/// writes. This simulates the exact artifacts a kill leaves, deterministically.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn crash_artifacts_reopen_clean_and_keep_writing() {
    // Window 1: pending .merge (killed between compact() and merge completion).
    let path = temp_path("crash-merge");
    cleanup(&path);
    {
        let store = Store::open(&path).await.unwrap();
        for k in 0..10 {
            write(
                &store,
                "crash",
                "kv",
                &format!("k{k}"),
                &serde_json::json!({"k": k}),
            )
            .await
            .unwrap();
        }
    }
    wait_release(&path).await;
    // Produce the pending-merge artifact with a raw engine handle, exactly as a killed pass
    // would: compact() writes .merge, then "die" before any completion open.
    {
        let mut opts = surrealkv::Options::new();
        opts.dir = std::path::Path::new(&path).to_path_buf();
        opts.disk_persistence = true;
        opts.enable_versions = false;
        opts.max_segment_size = 1 << 29;
        opts.max_value_threshold = 64;
        let s = surrealkv::Store::new(opts).unwrap();
        s.compact().unwrap();
        s.close().unwrap();
        assert!(std::path::Path::new(&path).join(".merge").exists());
    }
    let store = Store::open(&path)
        .await
        .expect("reopen with pending .merge");
    for k in 0..10 {
        assert_eq!(
            read(&store, "crash", "kv", &format!("k{k}")).await.unwrap(),
            Some(serde_json::json!({"k": k})),
            "k{k} survives a reopen that found a pending merge"
        );
    }
    // The P0 shape: a write made NOW must survive the next reopen.
    write(&store, "crash", "kv", "after", &serde_json::json!({"v": 1}))
        .await
        .unwrap();
    drop(store);
    wait_release(&path).await;
    let store = Store::open(&path).await.unwrap();
    assert_eq!(
        read(&store, "crash", "kv", "after").await.unwrap(),
        Some(serde_json::json!({"v": 1})),
        "a write into the recovered store survives the next compacting reopen"
    );
    drop(store);
    cleanup(&path);

    // Window 2: leftover .tmp.merge (killed mid-rewrite) — must be discarded, store intact.
    let path = temp_path("crash-tmp");
    cleanup(&path);
    {
        let store = Store::open(&path).await.unwrap();
        write(&store, "crash", "kv", "x", &serde_json::json!({"v": 7}))
            .await
            .unwrap();
    }
    wait_release(&path).await;
    std::fs::create_dir_all(std::path::Path::new(&path).join(".tmp.merge/clog")).unwrap();
    std::fs::write(
        std::path::Path::new(&path).join(".tmp.merge/garbage"),
        b"half-written",
    )
    .unwrap();
    let store = Store::open(&path)
        .await
        .expect("reopen with leftover .tmp.merge");
    assert_eq!(
        read(&store, "crash", "kv", "x").await.unwrap(),
        Some(serde_json::json!({"v": 7})),
        "data intact after discarding a half-written compaction"
    );
    assert!(
        !std::path::Path::new(&path).join(".tmp.merge").exists(),
        "the half-written artifact is cleaned up"
    );
    drop(store);
    cleanup(&path);
}

/// Boot dividend: opening a compacted copy of a grown store is bounded by the live set, not
/// the write history. Asserted on bytes (deterministic) + open-time (lenient, real clocks).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn boot_dividend_compacted_copy_opens_leaner() {
    let path = temp_path("dividend");
    cleanup(&path);
    {
        let store = Store::open(&path).await.unwrap();
        for k in 0..60 {
            for round in 0..20u64 {
                write(
                    &store,
                    "boot",
                    "kv",
                    &format!("k{k}"),
                    &serde_json::json!({"round": round, "pad": "y".repeat(512)}),
                )
                .await
                .unwrap();
            }
        }
    }
    wait_release(&path).await;

    // Uncompacted copy (cp before any further compacting open).
    let raw_copy = temp_path("dividend-raw");
    cleanup(&raw_copy);
    copy_dir(&path, &raw_copy);
    let (raw_bytes, _) = clog_stats(&raw_copy);

    // Compacted copy: open+close (Store::open compacts), then copy.
    {
        let _ = Store::open(&path).await.unwrap();
    }
    wait_release(&path).await;
    let compacted_copy = temp_path("dividend-compacted");
    cleanup(&compacted_copy);
    copy_dir(&path, &compacted_copy);
    let (compact_bytes, _) = clog_stats(&compacted_copy);

    assert!(
        compact_bytes < raw_bytes / 5,
        "compacted log must be a small fraction of the raw log: raw {raw_bytes} vs compacted {compact_bytes}"
    );

    // Time-to-open: generous bound (CI clocks) — the compacted copy must not open slower.
    let t0 = std::time::Instant::now();
    let s1 = Store::open(&raw_copy).await.unwrap();
    let raw_open = t0.elapsed();
    drop(s1);
    wait_release(&raw_copy).await;
    let t0 = std::time::Instant::now();
    let s2 = Store::open(&compacted_copy).await.unwrap();
    let compacted_open = t0.elapsed();
    drop(s2);
    eprintln!(
        "boot dividend: raw open {raw_open:?} ({raw_bytes} B) vs compacted open {compacted_open:?} ({compact_bytes} B)"
    );
    assert!(
        compacted_open <= raw_open * 2,
        "a compacted copy must not open meaningfully slower than the raw one (raw {raw_open:?}, compacted {compacted_open:?})"
    );
    cleanup(&path);
    cleanup(&raw_copy);
    cleanup(&compacted_copy);
}

/// A memory store has no commit log: the pass refuses, loudly and typed.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn memory_store_refuses_compaction() {
    let store = Store::memory().await.unwrap();
    let err = compact(&store).await.unwrap_err();
    assert!(
        err.to_string().contains("no commit log"),
        "memory store must refuse with the typed message, got: {err}"
    );
    let st = status(&store);
    assert!(!st.persistent);
    assert_eq!(st.log_bytes, 0);
}

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

fn copy_dir(from: &str, to: &str) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let src = e.path();
        let dst = std::path::Path::new(to).join(e.file_name());
        if src.is_dir() {
            copy_dir(src.to_str().unwrap(), dst.to_str().unwrap());
        } else {
            std::fs::copy(&src, &dst).unwrap();
        }
    }
}

fn clog_stats(path: &str) -> (u64, u32) {
    let clog = std::path::Path::new(path).join("clog");
    let mut bytes = 0u64;
    let mut count = 0u32;
    if let Ok(rd) = std::fs::read_dir(clog) {
        for e in rd.flatten() {
            if e.path().extension().and_then(|x| x.to_str()) == Some("clog") {
                bytes += e.metadata().map(|m| m.len()).unwrap_or(0);
                count += 1;
            }
        }
    }
    (bytes, count)
}
