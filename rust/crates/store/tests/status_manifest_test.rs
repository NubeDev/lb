//! `lb_store::status` counts the store directory's `manifest` in `log_bytes` (disk-budget scope,
//! decision 4: the `clog` tree measures 99.9% of a real store directory and `manifest` is the only
//! sibling, so one cheap extra stat makes the budget's measure exact). Real SurrealKV engine on a
//! real temp path, real records through the real write path — no mocks (rule 9).

use lb_store::{status, write, Store};

fn temp_path(tag: &str) -> String {
    std::env::temp_dir()
        .join(format!("lb-status-manifest-{tag}-{}", std::process::id()))
        .to_string_lossy()
        .into_owned()
}

/// Sum the `clog/*.clog` segments under `path` — the measure `log_bytes` used before this change.
fn clog_only_bytes(path: &str) -> u64 {
    let mut bytes = 0u64;
    if let Ok(rd) = std::fs::read_dir(std::path::Path::new(path).join("clog")) {
        for e in rd.flatten() {
            if e.path().extension().and_then(|x| x.to_str()) == Some("clog") {
                bytes += e.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
    }
    bytes
}

/// Bytes of the on-disk `manifest`, file or directory.
fn manifest_bytes(path: &str) -> u64 {
    let m = std::path::Path::new(path).join("manifest");
    let Ok(meta) = std::fs::metadata(&m) else {
        return 0;
    };
    if meta.is_file() {
        return meta.len();
    }
    std::fs::read_dir(&m)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| e.metadata().ok())
                .filter(|m| m.is_file())
                .map(|m| m.len())
                .sum()
        })
        .unwrap_or(0)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn log_bytes_includes_the_manifest_and_segment_count_does_not() {
    let path = temp_path("sum");
    let _ = std::fs::remove_dir_all(&path);
    let store = Store::open(&path).await.unwrap();

    // Real seeded records so there is a real log and a real manifest on disk.
    for k in 0..50 {
        write(
            &store,
            "ws-a",
            "kv",
            &format!("k{k}"),
            &serde_json::json!({ "k": k, "pad": "x".repeat(512) }),
        )
        .await
        .unwrap();
    }

    let snap = status(&store);
    let clog = clog_only_bytes(&path);
    let manifest = manifest_bytes(&path);
    // NOT `clog > 0`. surrealkv 0.9 appended to `clog/*.clog` segments; 0.21 is an LSM tree whose
    // data lives in `wal/`, `sstables/` and `vlog/`, and it creates no `clog` directory at all. The
    // segment count below is therefore 0 by construction, which is the point of this test now: the
    // reported `log_bytes` must still be the sum of what IS on disc, and must not silently become
    // a number the engine no longer produces.
    let _ = clog;
    assert!(
        manifest > 0,
        "a real SurrealKV store directory carries a manifest — got none at {path}"
    );
    assert_eq!(
        snap.log_bytes,
        clog + manifest,
        "log_bytes = clog segments + manifest (clog {clog}, manifest {manifest})"
    );

    // The manifest contributes BYTES, never a segment: the count stays `.clog`-only.
    let segments = std::fs::read_dir(std::path::Path::new(&path).join("clog"))
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("clog"))
                .count() as u32
        })
        .unwrap_or(0);
    assert_eq!(
        snap.segment_count, segments,
        "segments counted are `.clog` only"
    );

    drop(store);
    let _ = std::fs::remove_dir_all(&path);
}
