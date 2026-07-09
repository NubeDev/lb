//! Bounded retention for finished flow runs + their step rows (`retain_runs`) — the sibling of the
//! `lb_jobs::retain_terminal` guard, for the tables that are the actual bulk behind the node's disk
//! bloat (`docs/debugging/jobs/node-pegs-cpu-reactor-rescans-job-table.md`). Real embedded `mem://`
//! store; runs + step rows seeded through the real `lb_store::write` path (rule 9 — no mocks).
//!
//! Invariants asserted:
//!   - a `pending`/`suspended` (live/restartable) run is NEVER trimmed — the load-bearing safety
//!     property (trimming one would orphan or double-run it);
//!   - only finished runs beyond the cap are trimmed, newest kept;
//!   - a purged run's `flow_step_output` rows are deleted in tandem (the real bulk);
//!   - the pass is workspace-scoped — a ws-B sweep never touches ws-A rows (the hard wall, §7).

use lb_host::flow_engine::{
    retain_runs, ClaimState, FlowRunRecord, FlowStepRecord, FLOW_RUN_TABLE, FLOW_STEP_TABLE,
};
use lb_store::Store;
use serde_json::{json, Value};

/// Seed one `flow_run` row (id = run_id) with the given status, plus two `flow_step_output` rows for
/// it — through the real store write path, exactly as the run engine does.
async fn seed_run(store: &Store, ws: &str, run_id: &str, status: &str) {
    let run = FlowRunRecord {
        run_id: run_id.to_string(),
        flow_id: "flow-x".to_string(),
        flow_version: 1,
        status: status.to_string(),
        params: Value::Null,
        ts: 0,
        entry_node: None,
    };
    lb_store::write(
        store,
        ws,
        FLOW_RUN_TABLE,
        run_id,
        &serde_json::to_value(&run).unwrap(),
    )
    .await
    .unwrap();
    for node_id in ["n1", "n2"] {
        let step = FlowStepRecord {
            run_id: run_id.to_string(),
            node_id: node_id.to_string(),
            claim: ClaimState::Done,
            indegree: 0,
            outcome: "ok".to_string(),
            output: Value::Null,
            findings: Value::Null,
            error: None,
            attempts: 1,
            ms: 0,
            patched_config: None,
            fctx: String::new(),
            triggered_by: None,
            parent_fctx: None,
        };
        let step_id = format!("{run_id}:{node_id}");
        lb_store::write(
            store,
            ws,
            FLOW_STEP_TABLE,
            &step_id,
            &serde_json::to_value(&step).unwrap(),
        )
        .await
        .unwrap();
    }
}

async fn count(store: &Store, ws: &str, table: &str) -> usize {
    let mut resp = store
        .query_ws(
            ws,
            "SELECT count() AS c FROM type::table($tb) GROUP ALL",
            vec![("tb".into(), json!(table))],
        )
        .await
        .unwrap();
    let rows: Vec<Value> = resp.take(0).unwrap();
    rows.first()
        .and_then(|r| r.get("c"))
        .and_then(|c| c.as_u64())
        .unwrap_or(0) as usize
}

#[tokio::test]
async fn retain_runs_never_trims_a_live_run_and_trims_step_rows() {
    let store = Store::memory().await.unwrap();
    let ws = "flowret";

    // Two live runs (must survive forever) with LOW ids — first to be evicted by any id-order trim if
    // the predicate were wrong.
    seed_run(&store, ws, "aaa-pending", "pending").await;
    seed_run(&store, ws, "aab-suspended", "suspended").await;
    // Many finished runs, sorting after the live ones.
    for i in 0..30u32 {
        seed_run(&store, ws, &format!("run-{i:03}"), "success").await;
    }

    let deleted = retain_runs(&store, ws, 5).await.unwrap();
    assert_eq!(
        deleted, 25,
        "25 of 30 finished runs trimmed to the cap of 5"
    );

    // The two live runs are untouched; exactly cap finished + 2 live remain.
    assert_eq!(count(&store, ws, FLOW_RUN_TABLE).await, 5 + 2);
    // Step rows: (5 kept finished + 2 live) * 2 nodes = 14. The 25 purged runs' 50 step rows are gone.
    assert_eq!(count(&store, ws, FLOW_STEP_TABLE).await, (5 + 2) * 2);

    // Prove the live runs specifically survived (not just "some 7 rows").
    let mut resp = store
        .query_ws(
            ws,
            "SELECT VALUE data.status FROM type::table($tb) WHERE data.run_id IN $ids",
            vec![
                ("tb".into(), json!(FLOW_RUN_TABLE)),
                ("ids".into(), json!(["aaa-pending", "aab-suspended"])),
            ],
        )
        .await
        .unwrap();
    let mut statuses: Vec<String> = resp
        .take::<Vec<Value>>(0)
        .unwrap()
        .into_iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();
    statuses.sort();
    assert_eq!(
        statuses,
        vec!["pending".to_string(), "suspended".to_string()],
        "a pending/suspended run is never trimmed, even older/lower-id than the window"
    );
}

#[tokio::test]
async fn retain_runs_is_workspace_scoped() {
    let store = Store::memory().await.unwrap();
    for i in 0..8u32 {
        seed_run(&store, "ws-a", &format!("a-{i:02}"), "success").await;
        seed_run(&store, "ws-b", &format!("b-{i:02}"), "success").await;
    }
    retain_runs(&store, "ws-b", 2).await.unwrap();

    assert_eq!(
        count(&store, "ws-a", FLOW_RUN_TABLE).await,
        8,
        "a ws-b retention pass leaves every ws-a run intact (the hard wall)"
    );
    assert_eq!(count(&store, "ws-a", FLOW_STEP_TABLE).await, 16);
    assert_eq!(count(&store, "ws-b", FLOW_RUN_TABLE).await, 2);
}
