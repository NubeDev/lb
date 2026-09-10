//! `ensure_series_schema` must survive a RESTART against a store that already holds samples.
//!
//! The legacy-`ts` migration is the first statement the schema pass runs. Written as
//! `UPDATE series SET ts = time::from_millis(ts) WHERE type::is_number(ts)` it passed CI and
//! failed everywhere else: SurrealDB 3 evaluates the SET expression before the WHERE narrows the
//! rows, so `time::from_millis` is handed the `datetime` it was written to skip and the whole pass
//! errors. Every test that ran it did so against an EMPTY table, where no row exists to hand it.
//!
//! The consequence was total, not cosmetic: a node with any committed sample failed to serve the
//! workspace on its next boot. So this test does the one thing the suite never did — calls the
//! pass a second time, in a second process-equivalent, with rows on disc.

use lb_ingest::{commit_direct, ensure_series_schema, Qos, Sample};
use lb_store::Store;

fn temp_path(tag: &str) -> String {
    std::env::temp_dir()
        .join(format!("lb-schema-restart-{tag}-{}", std::process::id()))
        .to_string_lossy()
        .into_owned()
}

fn sample(series: &str, seq: u64) -> Sample {
    Sample {
        series: series.into(),
        producer: "user:probe".into(),
        seq,
        ts: 1_756_000_000_000 + seq * 60_000,
        payload: serde_json::json!(20.5),
        labels: serde_json::json!({}),
        qos: Qos::BestEffort,
    }
}

#[tokio::test]
async fn the_schema_pass_runs_again_over_a_store_that_already_holds_samples() {
    let path = temp_path("committed");
    let _ = std::fs::remove_dir_all(&path);

    let store = Store::open(&path).await.expect("open");
    ensure_series_schema(&store, "ops")
        .await
        .expect("first pass");
    commit_direct(&store, "ops", &[sample("ops.a", 0), sample("ops.a", 1)])
        .await
        .expect("commit");

    // The restart. `ensure_series_schema` keeps a process-local "already ensured" set, so reach
    // past it the way a new process does: a fresh handle on the same directory.
    drop(store);
    let reopened = Store::open(&path).await.expect("reopen");
    lb_ingest::reset_schema_guard_for_test();
    ensure_series_schema(&reopened, "ops")
        .await
        .expect("the schema pass must survive a restart with rows on disc");

    let mut r = reopened
        .query_ws("ops", "SELECT count() FROM series GROUP ALL", vec![])
        .await
        .expect("count");
    let rows: Vec<Count> = r.take(0).expect("decode");
    assert_eq!(
        rows.first().map(|c| c.count),
        Some(2),
        "both samples survive"
    );

    let _ = std::fs::remove_dir_all(&path);
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Count {
    count: i64,
}
lb_store::surreal_value_via_serde!(Count);
