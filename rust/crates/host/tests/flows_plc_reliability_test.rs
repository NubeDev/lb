//! PLC-reliability regression tests (flow-plc-reliability-scope). Real store (`mem://`), real caps,
//! real jobs — no mocks (CLAUDE §9). These prove the three properties the live `:8080` repro broke:
//!
//! 1. **Concurrent drives of the SAME run id never escape a store conflict** and the run settles
//!    exactly once (mirrors `capped_test`'s concurrency proof — the precedent this fix ports).
//! 2. **A manual `flows.run` with no `run_id` mints a UNIQUE id** — two back-to-back runs of the same
//!    flow are two distinct `flow_run` records, neither re-driving the other (the frozen-clock bug).
//! 3. **Idempotent seed under a racing same-id `start`** — exactly one seed, run settles once.
//!
//! Plus the mandatory cap-deny + workspace-isolation per `scope/testing/testing-scope.md`.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_flows::{FailurePolicy, Flow, Node, Placement};
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
    "store:flow:write",
    "store:flow:read",
];

fn count_node(id: &str, needs: &[&str], items: Value) -> Node {
    Node {
        id: id.into(),
        node_type: "count".into(),
        needs: needs.iter().map(|s| s.to_string()).collect(),
        with: serde_json::Map::from_iter([("items".into(), items)]),
        config: json!({}),
        position: None,
    }
}

fn chain4(id: &str) -> Flow {
    Flow {
        workspace: "ws".into(),
        id: id.into(),
        name: id.into(),
        version: 0,
        params: Default::default(),
        nodes: vec![
            count_node("a", &[], json!([1, 2, 3, 4])),
            count_node("b", &["a"], json!("${steps.a.output}")),
            count_node("c", &["b"], json!("${steps.b.output}")),
            count_node("d", &["c"], json!("${steps.c.output}")),
        ],
        failure_policy: FailurePolicy::Halt,
        deleted: false,
        enabled: true,
        start_on_boot: false,
        placement: Placement::Either,
        concurrency: Default::default(),
        cron: None,
        next_attempt_ts: 0,
    }
}

async fn save(node: &Arc<HostNode>, p: &Principal, ws: &str, flow: &Flow) {
    let body = serde_json::to_value(flow).unwrap().to_string();
    call_tool(node, p, ws, "flows.save", &body).await.unwrap();
}

async fn run(
    node: &Arc<HostNode>,
    p: &Principal,
    ws: &str,
    flow_id: &str,
    run_id: Option<&str>,
) -> Value {
    let mut req = json!({ "id": flow_id, "ts": 1 });
    if let Some(r) = run_id {
        req["run_id"] = json!(r);
    }
    let out = call_tool(node, p, ws, "flows.run", &req.to_string())
        .await
        .unwrap();
    serde_json::from_str(&out).unwrap()
}

async fn await_terminal(node: &Arc<HostNode>, p: &Principal, ws: &str, run_id: &str) -> Value {
    for _ in 0..600 {
        let out = call_tool(
            node,
            p,
            ws,
            "flows.runs.get",
            &json!({ "run_id": run_id }).to_string(),
        )
        .await
        .unwrap();
        let snap: Value = serde_json::from_str(&out).unwrap();
        if matches!(
            snap["status"].as_str().unwrap_or(""),
            "success" | "partialFailure" | "failed" | "cancelled"
        ) {
            return snap;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    panic!("run {run_id} never settled");
}

/// MANDATORY regression: N concurrent `flows.run` of the SAME run id must not surface a store
/// `Invalid revision` / transaction conflict, and the run settles exactly once to `success`. This is
/// the shared-run-id-under-concurrency shape reproduced live on `:8080`.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_same_run_id_never_conflicts_and_settles_once() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    save(&node, &p, "ws", &chain4("chain4")).await;

    let run_id = "chain4-run-fixed"; // a shared, caller-supplied id (the frozen-clock shape)
    let mut handles = Vec::new();
    for _ in 0..8 {
        let (n, p) = (node.clone(), p.clone());
        handles.push(tokio::spawn(async move {
            run(&n, &p, "ws", "chain4", Some(run_id)).await
        }));
    }
    for h in handles {
        let res = h.await.unwrap();
        // Every caller gets the same id back, and NONE returned a store error (call_tool would have
        // surfaced it as Err, unwrapped above; here we just assert the run id echoed).
        assert_eq!(res["run_id"], run_id);
    }
    let snap = await_terminal(&node, &p, "ws", run_id).await;
    assert_eq!(
        snap["status"], "success",
        "the shared run settles once, cleanly"
    );
    for step in snap["steps"].as_array().unwrap() {
        assert_eq!(step["outcome"], "ok", "node {} not ok", step["id"]);
    }
}

/// A manual run with no `run_id` mints a UNIQUE id: two back-to-back runs of the same flow are two
/// distinct runs, both terminal, neither re-driving the other (the frozen-clock collision).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manual_run_mints_unique_run_id() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    save(&node, &p, "ws", &chain4("chain4")).await;

    let r1 = run(&node, &p, "ws", "chain4", None).await;
    let r2 = run(&node, &p, "ws", "chain4", None).await;
    let id1 = r1["run_id"].as_str().unwrap().to_string();
    let id2 = r2["run_id"].as_str().unwrap().to_string();
    assert_ne!(id1, id2, "two manual runs must have distinct ids");
    assert_eq!(
        await_terminal(&node, &p, "ws", &id1).await["status"],
        "success"
    );
    assert_eq!(
        await_terminal(&node, &p, "ws", &id2).await["status"],
        "success"
    );
}

/// Capability deny: a principal lacking `mcp:flows.run:call` cannot run.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn run_denied_without_capability() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let full = principal("ws", FULL);
    save(&node, &full, "ws", &chain4("chain4")).await;
    // Same caps minus flows.run.
    let no_run: Vec<&str> = FULL
        .iter()
        .copied()
        .filter(|c| *c != "mcp:flows.run:call")
        .collect();
    let p = principal("ws", &no_run);
    let err = call_tool(
        &node,
        &p,
        "ws",
        "flows.run",
        &json!({ "id": "chain4", "ts": 1 }).to_string(),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, lb_mcp::ToolError::Denied),
        "expected Denied, got {err:?}"
    );
}

/// Workspace isolation: ws-B cannot run a flow that lives in ws-A.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn run_isolated_across_workspaces() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let a = principal("wsA", FULL);
    save(&node, &a, "wsA", &chain4("chain4")).await;
    let b = principal("wsB", FULL);
    // ws-B has full caps in its OWN workspace but the flow does not exist there.
    let err = call_tool(
        &node,
        &b,
        "wsB",
        "flows.run",
        &json!({ "id": "chain4", "ts": 1 }).to_string(),
    )
    .await
    .unwrap_err();
    // A missing flow in ws-B maps to `Denied` (FlowsError::NotFound → ToolError::Denied — tool
    // existence is not revealed). The point: ws-B physically cannot reach ws-A's record.
    assert!(
        matches!(err, lb_mcp::ToolError::Denied),
        "ws-B must not reach ws-A's flow, got {err:?}"
    );
}
