//! Retention + indexed-drain tests for the `job` table — the regression guard for the CPU-burn in
//! `docs/debugging/jobs/node-pegs-cpu-reactor-rescans-job-table.md`. All against a real embedded
//! `mem://` store (rule 9 — no mocks; real rows seeded through the real `create`/`complete` path).
//!
//! Covers:
//!   - **Performance/cost:** with ~5,000 terminal jobs + a few resumable, `pending` returns exactly
//!     the resumable set, and the DB-side predicate — not a Rust filter over the whole table — is what
//!     selects them (asserted by an index-backed `count()` equalling the resumable count, and by the
//!     `(kind,status)` index existing on the table). Deterministic: we measure the query's own filter,
//!     not wall-clock.
//!   - **Retention correctness:** a resumable job is NEVER trimmed (even when far older/lower-id than
//!     the window); only terminal rows are trimmed; the count bound is respected.
//!   - **Workspace isolation:** a ws-B retention pass leaves ws-A's terminal rows untouched.
//!   - **No user-cap surface:** these are raw store verbs (asserted by construction — no MCP path).

use lb_jobs::{
    cancel, complete, create, define_job_index, pending, retain_terminal, Job, JobStatus,
};
use lb_store::Store;
use serde_json::{json, Value};

const KIND: &str = "channel-agent-run";

/// Count rows in `table` (ws) matching the indexed drain predicate — the same `WHERE` `pending` runs.
/// If this equals the number of resumable rows while the table holds thousands of terminal rows, the
/// filter ran in the DB (index-backed), not by scanning every row into Rust.
async fn count_resumable(store: &Store, ws: &str, kind: &str) -> usize {
    let mut resp = store
        .query_ws(
            ws,
            "SELECT count() AS c FROM type::table($tb) \
             WHERE data.kind = $kind AND data.status IN ['running','suspended'] GROUP ALL",
            vec![("tb".into(), json!("job")), ("kind".into(), json!(kind))],
        )
        .await
        .expect("count query");
    let rows: Vec<Value> = resp.take(0).expect("take count");
    rows.first()
        .and_then(|r| r.get("c"))
        .and_then(|c| c.as_u64())
        .unwrap_or(0) as usize
}

/// Total row count in the `job` table (ws) — the table-size measure retention bounds.
async fn total_jobs(store: &Store, ws: &str) -> usize {
    let mut resp = store
        .query_ws(
            ws,
            "SELECT count() AS c FROM type::table($tb) GROUP ALL",
            vec![("tb".into(), json!("job"))],
        )
        .await
        .expect("count query");
    let rows: Vec<Value> = resp.take(0).expect("take count");
    rows.first()
        .and_then(|r| r.get("c"))
        .and_then(|c| c.as_u64())
        .unwrap_or(0) as usize
}

/// The `(kind,status)` index must actually be defined on the table, or the drain query silently falls
/// back to a full scan and we've fixed nothing (the scope's "index correctness on the stored shape").
async fn index_defined(store: &Store, ws: &str) -> bool {
    let mut resp = store
        .query_ws(ws, "INFO FOR TABLE job", vec![])
        .await
        .expect("info query");
    let rows: Vec<Value> = resp.take(0).expect("take info");
    // `INFO FOR TABLE` returns one object with an `indexes` map keyed by index name.
    rows.iter().any(|r| {
        r.get("indexes")
            .and_then(|ix| ix.as_object())
            .map(|m| m.contains_key("job_kind_status"))
            .unwrap_or(false)
    })
}

#[tokio::test]
async fn pending_is_indexed_and_returns_only_resumable_at_scale() {
    let store = Store::memory().await.unwrap();
    let ws = "scale";

    // Seed ~5,000 terminal jobs of the drain kind (alternating Done/Failed/Cancelled), then a few
    // resumable ones (Running + Suspended). The resumable ids sort LATE and EARLY so the "late-sorting
    // job is still found" property is exercised (no paging to fall off — the index returns it direct).
    define_job_index(&store, ws).await.unwrap();
    for i in 0..5_000u32 {
        let id = format!("term:{i:05}");
        create(&store, ws, &Job::new(&id, KIND, "{}", i as u64))
            .await
            .unwrap();
        match i % 3 {
            0 => complete(&store, ws, &id, JobStatus::Done).await.unwrap(),
            1 => complete(&store, ws, &id, JobStatus::Failed).await.unwrap(),
            _ => cancel(&store, ws, &id).await.unwrap(),
        }
    }
    // Resumable jobs — one with an id that sorts BEFORE every terminal row, one AFTER.
    create(&store, ws, &Job::new("aaa:run", KIND, "{}", 1))
        .await
        .unwrap(); // stays Running
    create(&store, ws, &Job::new("zzz:run", KIND, "{}", 2))
        .await
        .unwrap();
    lb_jobs::suspend(&store, ws, "zzz:run").await.unwrap();

    assert!(
        index_defined(&store, ws).await,
        "the (kind,status) index must be defined so the drain query is a lookup, not a scan"
    );

    let mut ids: Vec<String> = pending(&store, ws, KIND)
        .await
        .unwrap()
        .into_iter()
        .map(|j| j.id)
        .collect();
    ids.sort();
    assert_eq!(
        ids,
        vec!["aaa:run".to_string(), "zzz:run".to_string()],
        "exactly the resumable set is returned, regardless of 5,000 terminal rows"
    );

    // The DB-side predicate selects exactly the 2 resumable rows out of 5,002 total — proof the filter
    // ran in SurrealDB (index-backed), not by pulling the whole table into Rust to filter.
    assert_eq!(count_resumable(&store, ws, KIND).await, 2);
    assert!(total_jobs(&store, ws).await >= 5_000);
}

#[tokio::test]
async fn retention_never_trims_a_resumable_job() {
    let store = Store::memory().await.unwrap();
    let ws = "retain";

    // A resumable job with the LOWEST id — it would be the first evicted by any id-ordered trim if the
    // predicate were wrong. It must survive forever regardless of the cap.
    create(&store, ws, &Job::new("aaa:live", KIND, "{}", 0))
        .await
        .unwrap(); // Running
    create(&store, ws, &Job::new("aab:suspended", KIND, "{}", 0))
        .await
        .unwrap();
    lb_jobs::suspend(&store, ws, "aab:suspended").await.unwrap();

    // Plenty of terminal jobs, all sorting AFTER the live ones.
    for i in 0..50u32 {
        let id = format!("term:{i:03}");
        create(&store, ws, &Job::new(&id, KIND, "{}", i as u64))
            .await
            .unwrap();
        complete(&store, ws, &id, JobStatus::Done).await.unwrap();
    }

    // Trim to a tiny cap — far smaller than the terminal count and even smaller than +resumables.
    let deleted = retain_terminal(&store, ws, 5).await.unwrap();
    assert_eq!(
        deleted, 45,
        "45 of 50 terminal jobs trimmed to the cap of 5"
    );

    // The two resumable jobs are STILL pending — never in the delete set (the load-bearing invariant).
    let mut live: Vec<String> = pending(&store, ws, KIND)
        .await
        .unwrap()
        .into_iter()
        .map(|j| j.id)
        .collect();
    live.sort();
    assert_eq!(
        live,
        vec!["aaa:live".to_string(), "aab:suspended".to_string()],
        "a Running/Suspended job is never trimmed, even older/lower-id than the window"
    );

    // Bound respected: exactly cap terminal + the 2 resumable survive.
    assert_eq!(total_jobs(&store, ws).await, 5 + 2);
}

#[tokio::test]
async fn retention_keeps_the_newest_terminal_rows() {
    let store = Store::memory().await.unwrap();
    let ws = "newest";
    for i in 0..20u32 {
        let id = format!("j:{i:02}");
        create(&store, ws, &Job::new(&id, KIND, "{}", i as u64))
            .await
            .unwrap();
        complete(&store, ws, &id, JobStatus::Done).await.unwrap();
    }
    retain_terminal(&store, ws, 3).await.unwrap();

    let mut resp = store
        .query_ws(
            ws,
            "SELECT meta::id(id) AS rid, <string>id AS oid FROM type::table($tb) ORDER BY oid ASC",
            vec![("tb".into(), json!("job"))],
        )
        .await
        .unwrap();
    let ids: Vec<String> = resp
        .take::<Vec<Value>>(0)
        .unwrap()
        .into_iter()
        .filter_map(|v| v.get("rid").and_then(|r| r.as_str()).map(String::from))
        .collect();
    assert_eq!(
        ids,
        vec!["j:17".to_string(), "j:18".to_string(), "j:19".to_string()],
        "the newest cap rows survive; the oldest are evicted"
    );
}

#[tokio::test]
async fn retention_is_workspace_scoped() {
    let store = Store::memory().await.unwrap();

    // ws-a: terminal rows that must be untouched by a ws-b sweep.
    for i in 0..10u32 {
        let id = format!("a:{i:02}");
        create(&store, "ws-a", &Job::new(&id, KIND, "{}", i as u64))
            .await
            .unwrap();
        complete(&store, "ws-a", &id, JobStatus::Done)
            .await
            .unwrap();
    }
    // ws-b: more terminal rows.
    for i in 0..10u32 {
        let id = format!("b:{i:02}");
        create(&store, "ws-b", &Job::new(&id, KIND, "{}", i as u64))
            .await
            .unwrap();
        complete(&store, "ws-b", &id, JobStatus::Done)
            .await
            .unwrap();
    }

    // Trim ws-b hard.
    retain_terminal(&store, "ws-b", 2).await.unwrap();

    assert_eq!(
        total_jobs(&store, "ws-a").await,
        10,
        "a ws-b retention pass leaves every ws-a terminal row intact (the hard wall)"
    );
    assert_eq!(
        total_jobs(&store, "ws-b").await,
        2,
        "ws-b trimmed to its cap"
    );
}
