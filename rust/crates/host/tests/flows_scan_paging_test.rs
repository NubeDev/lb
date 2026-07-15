//! Regression tests for the **single-scan-page** read-back bug
//! (debugging/flows/single-scan-page-drops-rows-past-200.md): every flows verb that reads a shared
//! ws table (step slots, node last-values, runs) used ONE `lb_store::scan` page (hard-capped at 200
//! rows) and filtered in code — so once a deployed workspace outgrew a page, `flows.node_state`
//! silently dropped node values and runs stopped finalising. These tests seed 200+ REAL rows (rule 9:
//! real records into the real store, no fakes) that sort BEFORE the flow under test, so the old
//! one-page read provably misses it, and assert the paginated path (`scan_all`) still returns
//! everything. Plus the failed-firing read-back: an Err stamps `lastError` onto `flow_node_state`
//! instead of leaving a stale last-good value.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_flows::{table, FailurePolicy, Flow, Node};
use lb_host::{call_tool, Node as HostNode};
use serde_json::{json, Value};

fn principal(ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:test".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    verify(&key, &mint(&key, &claims), 1).unwrap()
}

const FULL: &[&str] = &[
    "mcp:flows.save:call",
    "mcp:flows.get:call",
    "mcp:flows.run:call",
    "mcp:flows.runs.get:call",
    "mcp:flows.runs.list:call",
    "mcp:flows.node.update:call",
    "mcp:flows.node_state:call",
    "store:flow:write",
    "store:flow:read",
];

fn flow(ws: &str, id: &str, nodes: Vec<Node>) -> Flow {
    Flow {
        workspace: ws.into(),
        id: id.into(),
        name: id.into(),
        version: 0,
        params: Default::default(),
        nodes,
        failure_policy: FailurePolicy::Halt,
        deleted: false,
        enabled: true,
        start_on_boot: false,
        placement: lb_flows::Placement::Either,
        concurrency: Default::default(),
        cron: None,
        next_attempt_ts: 0,
    }
}

fn count_with(id: &str, items: Value) -> Node {
    Node {
        id: id.into(),
        node_type: "count".into(),
        needs: Vec::new(),
        with: serde_json::Map::from_iter([("payload".into(), items)]),
        config: json!({}),
        inputs: Vec::new(),
        position: None,
    }
}

async fn save_flow(node: &Arc<HostNode>, p: &Principal, ws: &str, flow: &Flow) -> Value {
    let body = serde_json::to_value(flow).unwrap();
    let out = call_tool(node, p, ws, "flows.save", &body.to_string())
        .await
        .unwrap();
    serde_json::from_str(&out).unwrap()
}

async fn call(node: &Arc<HostNode>, p: &Principal, ws: &str, verb: &str, input: Value) -> Value {
    let out = call_tool(node, p, ws, verb, &input.to_string())
        .await
        .unwrap();
    serde_json::from_str(&out).unwrap()
}

async fn await_terminal(node: &Arc<HostNode>, p: &Principal, ws: &str, run_id: &str) -> Value {
    for _ in 0..600 {
        let snap = call(node, p, ws, "flows.runs.get", json!({ "run_id": run_id })).await;
        let status = snap["status"].as_str().unwrap_or("");
        if matches!(
            status,
            "success" | "partialFailure" | "failed" | "cancelled"
        ) {
            return snap;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    panic!("run {run_id} did not settle");
}

fn node_entry<'a>(state: &'a Value, id: &str) -> &'a Value {
    state["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["node"] == id)
        .unwrap()
}

/// More `flow_node_state` rows than one scan page (200), all sorting BEFORE the flow under test:
/// `flows.node_state` must still return the flow's real value (the one-page read returned only the
/// filler and painted this flow null).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn node_state_reads_values_past_one_scan_page() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);

    // 240 real last-value rows for OTHER flows, ids `aaa-fill-XXXX:n` — they sort before `zzz-page:*`.
    for i in 0..240 {
        lb_store::write(
            &node.store,
            "ws",
            table::FLOW_NODE_STATE,
            &format!("aaa-fill-{i:04}:n"),
            &json!({ "payload": i }),
        )
        .await
        .unwrap();
    }

    let f = flow("ws", "zzz-page", vec![count_with("a", json!([1, 2, 3]))]);
    save_flow(&node, &p, "ws", &f).await;
    call(
        &node,
        &p,
        "ws",
        "flows.run",
        json!({ "id": "zzz-page", "run_id": "zzz-run-1", "ts": 1 }),
    )
    .await;
    await_terminal(&node, &p, "ws", "zzz-run-1").await;

    let state = call(
        &node,
        &p,
        "ws",
        "flows.node_state",
        json!({ "id": "zzz-page" }),
    )
    .await;
    assert_eq!(
        node_entry(&state, "a")["value"]["payload"],
        3,
        "the value must survive a workspace larger than one scan page"
    );
}

/// More `flow_step_output` rows than one scan page, all sorting BEFORE the run under test: the run
/// must still drive to `success` and `flows.runs.get` must show its steps (the one-page read made
/// `scan_run_slots` miss the run's own slots, so it never finalised / showed no steps).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn run_finalizes_and_reports_steps_past_one_scan_page() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);

    // 240 real terminal step rows of an OLD run, ids `aaa-old-run:nXXXX` — they sort before `zzz-*`.
    for i in 0..240 {
        lb_store::write(
            &node.store,
            "ws",
            table::FLOW_STEP,
            &format!("aaa-old-run:n{i:04}"),
            &json!({
                "run_id": "aaa-old-run",
                "node_id": format!("n{i:04}"),
                "claim": "done",
                "indegree": 0,
                "outcome": "ok",
                "output": { "payload": i },
                "findings": null,
                "attempts": 1,
                "ms": 0,
                "fctx": "",
            }),
        )
        .await
        .unwrap();
    }

    let f = flow("ws", "zzz-steps", vec![count_with("a", json!([1, 2]))]);
    save_flow(&node, &p, "ws", &f).await;
    call(
        &node,
        &p,
        "ws",
        "flows.run",
        json!({ "id": "zzz-steps", "run_id": "zzz-run-2", "ts": 1 }),
    )
    .await;

    let snap = await_terminal(&node, &p, "ws", "zzz-run-2").await;
    assert_eq!(snap["status"], "success");
    let steps = snap["steps"].as_array().unwrap();
    assert!(
        steps.iter().any(|s| s["id"] == "a" && s["outcome"] == "ok"),
        "runs.get must show the run's own steps past one scan page: {steps:?}"
    );
}

/// A `json` node primed to flip between success and failure: `stringify` succeeds on an object
/// payload; `parse` FAILS on the same (non-string) payload — a real runtime error, no fakes.
fn json_flip_node(mode: &str) -> Node {
    Node {
        id: "j".into(),
        node_type: "json".into(),
        needs: Vec::new(),
        with: serde_json::Map::from_iter([("payload".into(), json!({ "x": 1 }))]),
        config: json!({ "mode": mode }),
        inputs: Vec::new(),
        position: None,
    }
}

// The error read-back contract is covered as TWO tests (ok→err and err→ok) rather than one
// ok→err→ok sequence: the debug-profile poll depth of the three-run sequence overflows the default
// 2 MiB test-thread stack (each half is comfortably under; the halves cover the same assertions).

/// A failed firing stamps `lastError` onto the node's last-value record — merged, so the last GOOD
/// value stays readable next to the error — and `flows.node_state` lifts it to the entry.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_firing_marks_last_error_and_keeps_the_good_value() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let f = flow("ws", "ef", vec![json_flip_node("stringify")]);
    save_flow(&node, &p, "ws", &f).await;

    call(
        &node,
        &p,
        "ws",
        "flows.run",
        json!({ "id": "ef", "run_id": "ef-1", "ts": 1 }),
    )
    .await;
    await_terminal(&node, &p, "ws", "ef-1").await;
    let ok_state = call(&node, &p, "ws", "flows.node_state", json!({ "id": "ef" })).await;
    let good_payload = node_entry(&ok_state, "j")["value"]["payload"].clone();
    assert!(
        good_payload.is_string(),
        "stringify emitted a string payload"
    );
    assert!(
        node_entry(&ok_state, "j")["error"].is_null(),
        "no error yet"
    );

    // Flip the node to `parse` (schema-valid; fails at runtime on the object payload) and re-run.
    call(
        &node,
        &p,
        "ws",
        "flows.node.update",
        json!({ "id": "ef", "node": "j", "config": { "mode": "parse" } }),
    )
    .await;
    call(
        &node,
        &p,
        "ws",
        "flows.run",
        json!({ "id": "ef", "run_id": "ef-2", "ts": 2 }),
    )
    .await;
    let snap = await_terminal(&node, &p, "ws", "ef-2").await;
    assert_eq!(snap["status"], "failed");

    let err_state = call(&node, &p, "ws", "flows.node_state", json!({ "id": "ef" })).await;
    let entry = node_entry(&err_state, "j");
    assert!(
        entry["error"].as_str().unwrap_or("").contains("string"),
        "the failed firing surfaces its error on the entry: {entry}"
    );
    assert_eq!(
        entry["value"]["payload"], good_payload,
        "the last GOOD value stays readable next to the error (merge, not clobber)"
    );
}

/// The next Ok firing clears `lastError` (whole-record overwrite): a flow whose first run FAILS,
/// fixed and re-run, reads back clean.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn next_ok_firing_clears_last_error() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let f = flow("ws", "ec", vec![json_flip_node("parse")]);
    save_flow(&node, &p, "ws", &f).await;

    call(
        &node,
        &p,
        "ws",
        "flows.run",
        json!({ "id": "ec", "run_id": "ec-1", "ts": 1 }),
    )
    .await;
    let snap = await_terminal(&node, &p, "ws", "ec-1").await;
    assert_eq!(snap["status"], "failed");
    let err_state = call(&node, &p, "ws", "flows.node_state", json!({ "id": "ec" })).await;
    assert!(
        node_entry(&err_state, "j")["error"]
            .as_str()
            .unwrap_or("")
            .contains("string"),
        "the failed first firing surfaces its error"
    );

    // Fix the node (`stringify` succeeds on the object payload) — the Ok overwrite clears the error.
    call(
        &node,
        &p,
        "ws",
        "flows.node.update",
        json!({ "id": "ec", "node": "j", "config": { "mode": "stringify" } }),
    )
    .await;
    call(
        &node,
        &p,
        "ws",
        "flows.run",
        json!({ "id": "ec", "run_id": "ec-2", "ts": 2 }),
    )
    .await;
    await_terminal(&node, &p, "ws", "ec-2").await;
    let clear_state = call(&node, &p, "ws", "flows.node_state", json!({ "id": "ec" })).await;
    assert!(
        node_entry(&clear_state, "j")["error"].is_null(),
        "the next Ok firing clears the error"
    );
    assert!(
        node_entry(&clear_state, "j")["value"]["payload"].is_string(),
        "the good value is the current read-back again"
    );
}
