//! An upgraded node must not start against a SurrealDB 2 store, and must SAY why.
//!
//! surrealkv 0.9 wrote a bitcask log into `clog/`; 0.21 is an LSM tree and cannot read it. Opening
//! fails — which is right, because booting green onto an empty workspace would lose an edge node's
//! buffered samples silently. What matters here is that the refusal names the cause: on its own the
//! engine says `IO error: File exists`, which sends an operator nowhere.

use lb_store::Store;

#[tokio::test]
async fn an_old_engine_store_is_refused_by_name() {
    let dir = std::env::temp_dir().join(format!("lb-oldfmt-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("clog")).expect("clog dir");
    std::fs::write(dir.join("clog/00000000000000000000.clog"), vec![0u8; 4096]).expect("segment");
    std::fs::write(dir.join("manifest"), b"old").expect("manifest");

    // `Store` is not `Debug`, so match rather than `expect_err`.
    let msg = match Store::open(&dir.to_string_lossy()).await {
        Ok(_) => panic!("a 0.9-format directory must not open"),
        Err(e) => e.to_string(),
    };
    assert!(
        msg.contains("surrealkv 0.9") && msg.contains("clog"),
        "the refusal must name the cause, got: {msg}"
    );
    assert!(
        msg.contains("will NOT start"),
        "the refusal must say the node does not start, got: {msg}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A FRESH directory is untouched by the check — the message is for upgrades, not new nodes.
#[tokio::test]
async fn a_fresh_directory_still_opens() {
    let dir = std::env::temp_dir().join(format!("lb-freshfmt-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        Store::open(&dir.to_string_lossy()).await.is_ok(),
        "a new directory opens normally"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
