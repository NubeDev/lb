//! REGRESSION — the online pass must work on a store that ever ran a `DEFINE INDEX`.
//!
//! At surrealdb-core 2.6.5, `DEFINE INDEX` spawns an index-builder task that holds the engine
//! (transaction factory) FOREVER — the store's files never reach fd-zero after the last handle
//! drops (measured: still held 120 s later). Every real node defines the jobs
//! `(kind, status)` index, so a pass gated on full fd release alone can never run in
//! production — the exact failure the host job-flow test caught. `wait_for_quiesce` therefore
//! accepts proven stability (unchanged size+mtime across the window) from an inert leaked
//! engine. This test pins that: define an index, run the online pass on the live handle,
//! and require success + intact data.

use lb_store::{compact, read, write, Store};

fn temp_path(tag: &str) -> String {
    std::env::temp_dir()
        .join(format!("lb-idx-quiesce-{tag}-{}", std::process::id()))
        .to_string_lossy()
        .into_owned()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn online_pass_succeeds_after_a_define_index() {
    let path = temp_path("main");
    let _ = std::fs::remove_dir_all(&path);
    let store = Store::open(&path).await.unwrap();

    // Real churn + the exact index the jobs crate defines on every workspace.
    for k in 0..20 {
        for round in 0..5u64 {
            write(
                &store,
                "probe",
                "kv",
                &format!("k{k}"),
                &serde_json::json!({"round": round}),
            )
            .await
            .unwrap();
        }
    }
    store
        .query_ws(
            "probe",
            "DEFINE INDEX IF NOT EXISTS job_kind_status ON TABLE job COLUMNS data.kind, data.status",
            vec![],
        )
        .await
        .unwrap();
    write(
        &store,
        "probe",
        "job",
        "j1",
        &serde_json::json!({"kind": "store-compact", "status": "running"}),
    )
    .await
    .unwrap();

    let rec = compact(&store)
        .await
        .expect("the pass must succeed despite the index-builder engine leak (quiesce fallback)");
    assert!(rec.ok);
    assert!(rec.after_bytes < rec.before_bytes, "churn compacts away");

    // The live handle serves intact data after the swap.
    for k in 0..20 {
        let got = read(&store, "probe", "kv", &format!("k{k}")).await.unwrap();
        assert_eq!(
            got.as_ref()
                .and_then(|v| v.get("round"))
                .and_then(|r| r.as_u64()),
            Some(4),
            "k{k} newest value survives"
        );
    }
    assert!(
        read(&store, "probe", "job", "j1").await.unwrap().is_some(),
        "the indexed row survives"
    );
    let _ = std::fs::remove_dir_all(&path);
}
