//! The day-one capability spike — a PERMANENT, HERMETIC CI test that defines and exercises every
//! SurrealDB feature the S9 scopes (ingest, tags) assume on the **persistent** engine
//! (`Store::open` → SurrealKV), and records, per feature, available-or-not. The suite's output IS
//! the recorded capability matrix (docs/scope/store/persistent-backend-scope.md). It runs on a real
//! on-disk store in a fresh temp dir with idempotent cleanup, so a future SurrealDB upgrade that
//! drops a feature is caught here, not mid-build of a dependent slice.
//!
//! Classification (the scope's matrix):
//!   - **LOAD-BEARING** — a ✗ is NO-GO for all of S9; the test FAILS so it can never be ignored.
//!     (durability across restart, composite/array record IDs, RELATE edges w/ props,
//!      namespace isolation on disk, multi-statement transactions)
//!   - **DEGRADABLE** — a ✗ defers one capability to a follow-up; recorded as a documented `false`,
//!     never a hard failure. (DEFINE BUCKET, SEARCH/BM25, HNSW vector, materialized views, LIVE)

use lb_store::Store;

/// A fresh on-disk path under the OS temp dir, unique per (test, call) — hermetic, no collisions.
fn temp_path(tag: &str) -> String {
    let base = std::env::temp_dir();
    let pid = std::process::id();
    base.join(format!("lb-spike-{tag}-{pid}"))
        .to_string_lossy()
        .into_owned()
}

fn cleanup(path: &str) {
    let _ = std::fs::remove_dir_all(path);
}

/// Probe one feature: run `sql` against a hermetic namespace; `Ok(true)` if it ran clean.
/// A feature that errors is captured (caller decides LOAD-BEARING-fail vs DEGRADABLE-record).
async fn probe(store: &Store, ws: &str, sql: &str) -> Result<(), String> {
    store
        .query_ws(ws, sql, vec![])
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

// ── LOAD-BEARING features: a ✗ here FAILS the test (NO-GO for all of S9) ──────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn load_bearing_durability_across_restart() {
    let path = temp_path("durability");
    cleanup(&path);
    {
        let store = Store::open(&path).await.expect("open persistent store");
        lb_store::write(
            &store,
            "spike",
            "kept",
            "row1",
            &serde_json::json!({"v": 42}),
        )
        .await
        .expect("write");
        // handle drops here — simulates a clean process exit
    }
    {
        let store = Store::open(&path).await.expect("reopen same path");
        let got = lb_store::read(&store, "spike", "kept", "row1")
            .await
            .expect("read after reopen");
        assert_eq!(
            got,
            Some(serde_json::json!({"v": 42})),
            "LOAD-BEARING durability: write must survive drop+reopen"
        );
    }
    cleanup(&path);
    println!("SPIKE durability-across-restart = AVAILABLE (LOAD-BEARING)");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn load_bearing_composite_array_record_ids() {
    let path = temp_path("composite");
    cleanup(&path);
    let store = Store::open(&path).await.expect("open");
    // The ingest dedup id [series, producer, seq] and the tag node id [key, value] both rely on
    // array/composite record IDs being addressable and range-scannable.
    probe(
        &store,
        "spike",
        "CREATE sample:['cpu', 'pi-7', 5] SET v = 61.4;",
    )
    .await
    .expect("LOAD-BEARING composite id: create with array id must work");
    let mut resp = store
        .query_ws("spike", "SELECT v FROM sample:['cpu', 'pi-7', 5];", vec![])
        .await
        .expect("select by composite id");
    let v: Option<f64> = resp.take("v").expect("take v");
    assert_eq!(v, Some(61.4), "composite-id point read must round-trip");
    cleanup(&path);
    println!("SPIKE composite-array-record-ids = AVAILABLE (LOAD-BEARING)");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn load_bearing_relate_edges_with_properties() {
    let path = temp_path("relate");
    cleanup(&path);
    let store = Store::open(&path).await.expect("open");
    // The tag/provenance/lineage graph is RELATE edges carrying properties (at/by/source/...).
    probe(
        &store,
        "spike",
        "CREATE series:s1; CREATE tag:['region','eu'];
         RELATE series:s1 -> tagged -> tag:['region','eu']
            SET at = 1, by = 'user:ada', source = 'human', confidence = 1.0;",
    )
    .await
    .expect("LOAD-BEARING RELATE w/ props: relate must store edge properties");
    let mut resp = store
        .query_ws(
            "spike",
            "SELECT source FROM series:s1 -> tagged;",
            vec![],
        )
        .await
        .expect("traverse edge");
    let src: Option<String> = resp.take("source").expect("take source");
    assert_eq!(
        src.as_deref(),
        Some("human"),
        "edge property must be queryable via traversal"
    );
    cleanup(&path);
    println!("SPIKE relate-edges-with-properties = AVAILABLE (LOAD-BEARING)");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn load_bearing_namespace_isolation_on_disk() {
    let path = temp_path("nsiso");
    cleanup(&path);
    let store = Store::open(&path).await.expect("open");
    // Same table:id in two namespaces (workspaces) must not bleed — the hard wall on disk.
    lb_store::write(&store, "ws-a", "secret", "x", &serde_json::json!({"who": "a"}))
        .await
        .expect("write ws-a");
    lb_store::write(&store, "ws-b", "secret", "x", &serde_json::json!({"who": "b"}))
        .await
        .expect("write ws-b");
    let a = lb_store::read(&store, "ws-a", "secret", "x").await.unwrap();
    let b = lb_store::read(&store, "ws-b", "secret", "x").await.unwrap();
    assert_eq!(a, Some(serde_json::json!({"who": "a"})));
    assert_eq!(b, Some(serde_json::json!({"who": "b"})));
    assert_ne!(a, b, "LOAD-BEARING ns isolation: namespaces must not share rows on disk");
    cleanup(&path);
    println!("SPIKE namespace-isolation-on-disk = AVAILABLE (LOAD-BEARING)");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn load_bearing_multi_statement_transactions() {
    let path = temp_path("tx");
    cleanup(&path);
    let store = Store::open(&path).await.expect("open");
    // Batch-commit atomicity (one batch = one tx) depends on all-or-nothing multi-statement tx.
    probe(
        &store,
        "spike",
        "BEGIN TRANSACTION;
         CREATE row:a SET v = 1;
         CREATE row:b SET v = 2;
         COMMIT TRANSACTION;",
    )
    .await
    .expect("LOAD-BEARING tx: a committed transaction must apply");
    let mut resp = store
        .query_ws("spike", "SELECT count() FROM row GROUP ALL;", vec![])
        .await
        .expect("count");
    let n: Option<i64> = resp.take("count").expect("take count");
    assert_eq!(n, Some(2), "both rows of the committed tx must be present");

    // And a CANCELled transaction must leave nothing — the rollback half atomicity relies on.
    let _ = store
        .query_ws(
            "spike",
            "BEGIN TRANSACTION; CREATE row:c SET v = 3; CANCEL TRANSACTION;",
            vec![],
        )
        .await;
    let mut resp = store
        .query_ws("spike", "SELECT count() FROM row GROUP ALL;", vec![])
        .await
        .expect("recount");
    let n: Option<i64> = resp.take("count").expect("take count");
    assert_eq!(n, Some(2), "a cancelled tx must roll back (no row:c)");
    cleanup(&path);
    println!("SPIKE multi-statement-transactions = AVAILABLE (LOAD-BEARING)");
}

// ── DEGRADABLE features: a ✗ is RECORDED (printed false), never a hard failure ────────────────

/// Record a degradable probe: print AVAILABLE/UNAVAILABLE so the suite output is the matrix.
/// Returns the boolean for any test that wants to assert the *recorded* state for its own slice.
async fn record_degradable(store: &Store, ws: &str, name: &str, sql: &str) -> bool {
    match probe(store, ws, sql).await {
        Ok(()) => {
            println!("SPIKE {name} = AVAILABLE (DEGRADABLE)");
            true
        }
        Err(e) => {
            println!("SPIKE {name} = UNAVAILABLE (DEGRADABLE) — fallback applies: {e}");
            false
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn degradable_feature_matrix() {
    let path = temp_path("degradable");
    cleanup(&path);
    let store = Store::open(&path).await.expect("open");

    // DEFINE BUCKET / file storage → ingest binary payloads fall back to record-as-content.
    let bucket = record_degradable(
        &store,
        "spike",
        "define-bucket",
        "DEFINE BUCKET assets BACKEND \"memory\";",
    )
    .await;

    // DEFINE ANALYZER + SEARCH/BM25 full-text → tags value full-text → follow-up if absent.
    let search = record_degradable(
        &store,
        "spike",
        "search-bm25",
        "DEFINE ANALYZER simple TOKENIZERS blank FILTERS lowercase;
         DEFINE INDEX tag_text ON TABLE tag FIELDS value SEARCH ANALYZER simple BM25;",
    )
    .await;

    // HNSW vector index → tags semantic/similar-to → follow-up if absent.
    let hnsw = record_degradable(
        &store,
        "spike",
        "hnsw-vector",
        "DEFINE INDEX tag_vec ON TABLE tag FIELDS embedding HNSW DIMENSION 4 DIST COSINE;",
    )
    .await;

    // Materialized view (per-dimension tag_counts) → computed per-query if absent.
    let views = record_degradable(
        &store,
        "spike",
        "materialized-view",
        "DEFINE TABLE tag_counts AS
            SELECT count() AS n, key FROM tag GROUP BY key;",
    )
    .await;

    // LIVE SELECT → a convenience only (motion rides Zenoh); no impact either way.
    let live = record_degradable(
        &store,
        "spike",
        "live-select",
        "LIVE SELECT * FROM tag;",
    )
    .await;

    cleanup(&path);

    // The matrix as a single line — grep-able, and the value consumed by the dependent scopes.
    println!(
        "SPIKE-MATRIX degradable: bucket={bucket} search={search} hnsw={hnsw} views={views} live={live}"
    );
    // No assert: degradable features are recorded, not gated. The dependent slices read this output.
}
