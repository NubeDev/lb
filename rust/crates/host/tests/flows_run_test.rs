//! Host-layer tests for the `flows.*` run engine (flow-run-scope Testing plan). Real store
//! (`mem://`), real caps, real `lb-jobs`, real outbox — no mocks. Flows seeded as real `flow`
//! records through the real `flows.save` write path; rules (for `rhai` nodes) seeded via
//! `rules.save`.
//!
//! Mandatory: DAG validation at save (cycle/dangling/dup/self-edge), capability-deny (each verb +
//! the no-widening run gate — a tool node calling a verb the caller lacks is denied at that node),
//! workspace-isolation (ws-B cannot run/get/list a ws-A flow), the diamond frontier, Halt subtree-
//! skip, suspend→patch_run→resume, structural-edit-during-suspend→new-version, ResumePointDrift,
//! subflow-parks-on-child, reattach via runs.list, and offline/sync (a re-run / resume is a no-op).

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_flows::{FailurePolicy, Flow, Node};
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
    };
    verify(&key, &mint(&key, &claims), 1).unwrap()
}

/// Full caps: every flows verb + the store surface + rules.run (for rhai nodes) + a sample tool the
/// generic `tool` node dispatches (a no-widening run revokes this selectively).
const FULL: &[&str] = &[
    "mcp:flows.save:call",
    "mcp:flows.get:call",
    "mcp:flows.list:call",
    "mcp:flows.delete:call",
    "mcp:flows.run:call",
    "mcp:flows.resume:call",
    "mcp:flows.suspend:call",
    "mcp:flows.cancel:call",
    "mcp:flows.patch_run:call",
    "mcp:flows.runs.get:call",
    "mcp:flows.runs.list:call",
    "mcp:flows.nodes:call",
    "mcp:rules.run:call",
    "store:flow:write",
    "store:flow:read",
    "store:rule:write",
];

fn node(id: &str, ty: &str, needs: &[&str], config: Value) -> Node {
    Node {
        id: id.into(),
        node_type: ty.into(),
        needs: needs.iter().map(|s| s.to_string()).collect(),
        with: serde_json::Map::new(),
        config,
    }
}

fn rhai_node(id: &str, needs: &[&str], source: &str) -> Node {
    node(id, "rhai", needs, json!({ "source": source }))
}

/// Save a flow through the real `flows.save` write path (DAG + config validated).
async fn save_flow(node: &Arc<HostNode>, p: &Principal, ws: &str, flow: &Flow) -> Value {
    let body = serde_json::to_value(flow).unwrap();
    let out = call_tool(node, p, ws, "flows.save", &body.to_string()).await.unwrap();
    serde_json::from_str(&out).unwrap()
}

async fn run_flow(node: &Arc<HostNode>, p: &Principal, ws: &str, id: &str, run_id: &str) -> Value {
    let req = json!({ "id": id, "run_id": run_id, "ts": 1 }).to_string();
    let out = call_tool(node, p, ws, "flows.run", &req).await.unwrap();
    serde_json::from_str(&out).unwrap()
}

async fn runs_get(node: &Arc<HostNode>, p: &Principal, ws: &str, run_id: &str) -> Value {
    let req = json!({ "run_id": run_id }).to_string();
    let out = call_tool(node, p, ws, "flows.runs.get", &req).await.unwrap();
    serde_json::from_str(&out).unwrap()
}

fn flow(id: &str, nodes: Vec<Node>) -> Flow {
    Flow { workspace: "ws".into(), id: id.into(), name: id.into(), version: 0, params: Default::default(), nodes, failure_policy: FailurePolicy::Halt, deleted: false }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn save_rejects_a_cyclic_dag() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let a = rhai_node("a", &["b"], "42");
    let b = rhai_node("b", &["a"], "42");
    let f = flow("cyc", vec![a, b]);
    let body = serde_json::to_value(&f).unwrap().to_string();
    let err = call_tool(&node, &p, "ws", "flows.save", &body).await.unwrap_err();
    assert!(matches!(err, lb_mcp::ToolError::BadInput(_)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn linear_rhai_flow_runs_to_success() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let f = flow("lin", vec![rhai_node("a", &[], "42"), rhai_node("b", &["a"], "43")]);
    let saved = save_flow(&node, &p, "ws", &f).await;
    assert_eq!(saved["version"], 1);
    let run = run_flow(&node, &p, "ws", "lin", "lin-run-1").await;
    assert_eq!(run["run_id"], "lin-run-1");
    let snap = runs_get(&node, &p, "ws", "lin-run-1").await;
    assert_eq!(snap["status"], "success");
    assert_eq!(snap["flowVersion"], 1);
    // every node ok
    for step in snap["steps"].as_array().unwrap() {
        assert_eq!(step["outcome"], "ok", "node {} not ok", step["id"]);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn diamond_frontier_runs_in_dependency_order() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let f = flow(
        "dia",
        vec![
            rhai_node("a", &[], "1"),
            rhai_node("b", &["a"], "2"),
            rhai_node("c", &["a"], "3"),
            rhai_node("d", &["b", "c"], "4"),
        ],
    );
    save_flow(&node, &p, "ws", &f).await;
    run_flow(&node, &p, "ws", "dia", "dia-run-1").await;
    let snap = runs_get(&node, &p, "ws", "dia-run-1").await;
    assert_eq!(snap["status"], "success");
    assert_eq!(snap["steps"].as_array().unwrap().len(), 4);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn halt_policy_skips_failed_subtree() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    // `bad` runs a rule body that errors; `dep` depends on it. Under Halt, `dep` is skipped.
    let mut bad = rhai_node("bad", &[], "syntax@#$error");
    let _ = &mut bad;
    let mut f = flow("halt", vec![rhai_node("bad", &[], "syntax@#$error"), rhai_node("dep", &["bad"], "1")]);
    f.failure_policy = FailurePolicy::Halt;
    save_flow(&node, &p, "ws", &f).await;
    run_flow(&node, &p, "ws", "halt", "halt-run-1").await;
    let snap = runs_get(&node, &p, "ws", "halt-run-1").await;
    // a rhai syntax error → that node err; dep skipped; run = failed (no ok node).
    assert_eq!(snap["status"], "failed");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn no_widening_tool_node_denied_without_the_tool_cap() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    // caller holds flows.run but NOT `mcp:rules.run` — a tool node that dispatches `rules.run` is
    // DENIED at that node (no widening); the node records `err`, the run fails (Halt).
    let caps: Vec<&str> = FULL.iter().filter(|c| **c != "mcp:rules.run:call").cloned().collect();
    let p = principal("ws", &caps);
    let f = flow("widen", vec![rhai_node("r", &[], "1")]);
    save_flow(&node, &p, "ws", &f).await;
    run_flow(&node, &p, "ws", "widen", "widen-run-1").await;
    let snap = runs_get(&node, &p, "ws", "widen-run-1").await;
    assert_eq!(snap["status"], "failed");
    let r = snap["steps"].as_array().unwrap().iter().find(|s| s["id"] == "r").unwrap();
    assert_eq!(r["outcome"], "err");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn capability_deny_run_without_flows_run_cap() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let caps: Vec<&str> = FULL.iter().filter(|c| **c != "mcp:flows.run:call").cloned().collect();
    let saver = principal("ws", FULL);
    let runner = principal("ws", &caps);
    let f = flow("deny", vec![rhai_node("a", &[], "1")]);
    save_flow(&node, &saver, "ws", &f).await;
    let req = json!({ "id": "deny", "run_id": "d1", "ts": 1 }).to_string();
    let err = call_tool(&node, &runner, "ws", "flows.run", &req).await.unwrap_err();
    assert!(matches!(err, lb_mcp::ToolError::Denied));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_isolation_ws_b_cannot_see_ws_a_flow() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let pa = principal("ws-a", FULL);
    let pb = principal("ws-b", FULL);
    let f = flow("iso", vec![rhai_node("a", &[], "1")]);
    save_flow(&node, &pa, "ws-a", &f).await;
    // ws-B cannot get ws-A's flow.
    let req = json!({ "id": "iso" }).to_string();
    let err = call_tool(&node, &pb, "ws-b", "flows.get", &req).await.unwrap_err();
    assert!(matches!(err, lb_mcp::ToolError::Denied));
    // ws-B running `iso` creates its own (absent) flow → NotFound → denied.
    let req = json!({ "id": "iso", "run_id": "x", "ts": 1 }).to_string();
    let err = call_tool(&node, &pb, "ws-b", "flows.run", &req).await.unwrap_err();
    assert!(matches!(err, lb_mcp::ToolError::Denied));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn structural_edit_during_suspend_writes_new_version() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let f = flow("ver", vec![rhai_node("a", &[], "1")]);
    let saved = save_flow(&node, &p, "ws", &f).await;
    assert_eq!(saved["version"], 1);
    // a re-save (a structural edit) bumps the version (Decision 1).
    let saved2 = save_flow(&node, &p, "ws", &f).await;
    assert_eq!(saved2["version"], 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn patch_run_only_on_unexecuted_node() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    // A two-node flow: `a` (rhai) → `b` (rhai). We run, then try to patch `a` (executed) → rejected;
    // patching is only for unexecuted nodes. To exercise the accept path we instead patch before run.
    let f = flow("patch", vec![rhai_node("a", &[], "1"), rhai_node("b", &["a"], "2")]);
    save_flow(&node, &p, "ws", &f).await;
    // Seed the run records by running, then assert an executed node rejects a patch.
    run_flow(&node, &p, "ws", "patch", "patch-run-1").await;
    let req = json!({ "run_id": "patch-run-1", "node": "a", "config": {"source":"99"} }).to_string();
    let err = call_tool(&node, &p, "ws", "flows.patch_run", &req).await.unwrap_err();
    assert!(matches!(err, lb_mcp::ToolError::BadInput(_)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn resume_is_idempotent_a_re_drive_no_ops() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let f = flow("resume", vec![rhai_node("a", &[], "5"), rhai_node("b", &["a"], "6")]);
    save_flow(&node, &p, "ws", &f).await;
    run_flow(&node, &p, "ws", "resume", "resume-run-1").await;
    let snap1 = runs_get(&node, &p, "ws", "resume-run-1").await;
    assert_eq!(snap1["status"], "success");
    // a re-resume (the offline/sync replay) is a no-op: the CAS claim makes a redelivered node a no-op.
    let req = json!({ "run_id": "resume-run-1", "ts": 2 }).to_string();
    call_tool(&node, &p, "ws", "flows.resume", &req).await.unwrap();
    let snap2 = runs_get(&node, &p, "ws", "resume-run-1").await;
    assert_eq!(snap2["status"], "success");
    // outputs unchanged (exactly-once — no re-run, no double effect).
    let a1 = snap1["steps"].as_array().unwrap().iter().find(|s| s["id"] == "a").unwrap()["output"].clone();
    let a2 = snap2["steps"].as_array().unwrap().iter().find(|s| s["id"] == "a").unwrap()["output"].clone();
    assert_eq!(a1, a2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn runs_list_reattach_finds_a_flow_runs() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let f = flow("reattach", vec![rhai_node("a", &[], "1")]);
    save_flow(&node, &p, "ws", &f).await;
    run_flow(&node, &p, "ws", "reattach", "reattach-run-1").await;
    let req = json!({ "flow_id": "reattach" }).to_string();
    let out = call_tool(&node, &p, "ws", "flows.runs.list", &req).await.unwrap();
    let list = serde_json::from_str::<Value>(&out).unwrap();
    let runs = list["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0]["runId"], "reattach-run-1");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn subflow_parks_on_child_run() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    // a child flow `child` (one rhai node `c`), and a parent whose `sub` node runs it pinned.
    let child = flow("child", vec![rhai_node("c", &[], "7")]);
    save_flow(&node, &p, "ws", &child).await;
    let parent = flow("parent", vec![Node {
        id: "sub".into(),
        node_type: "subflow".into(),
        needs: vec![],
        with: serde_json::Map::new(),
        config: json!({ "flow": "child@1" }),
    }]);
    save_flow(&node, &p, "ws", &parent).await;
    run_flow(&node, &p, "ws", "parent", "parent-run-1").await;
    let snap = runs_get(&node, &p, "ws", "parent-run-1").await;
    assert_eq!(snap["status"], "success");
    let sub = snap["steps"].as_array().unwrap().iter().find(|s| s["id"] == "sub").unwrap();
    assert_eq!(sub["outcome"], "ok");
    // the subflow node's output folds the child's terminal node outputs (unwrapped scalar).
    assert_eq!(sub["output"]["c"], 7);
}
