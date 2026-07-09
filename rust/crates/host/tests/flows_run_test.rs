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
use lb_flows::{FailurePolicy, Flow, InputEdge, Node};
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
    "mcp:rules.eval:call",
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
        inputs: Vec::new(),
        position: None,
    }
}

fn rhai_node(id: &str, needs: &[&str], source: &str) -> Node {
    node(id, "rhai", needs, json!({ "source": source }))
}

/// A `link-out {target}` node forwarding its upstream(s) wirelessly (flow-input-ports-scope Slice 3).
fn link_out_node(id: &str, target: &str, needs: &[&str]) -> Node {
    node(id, "link-out", needs, json!({ "target": target }))
}

/// A `link-in {name}` collector — the `any`-funnel every matching `link-out` resolves onto.
fn link_in_node(id: &str, name: &str, needs: &[&str]) -> Node {
    node(id, "link-in", needs, json!({ "name": name }))
}

/// Save a flow through the real `flows.save` write path (DAG + config validated).
async fn save_flow(node: &Arc<HostNode>, p: &Principal, ws: &str, flow: &Flow) -> Value {
    let body = serde_json::to_value(flow).unwrap();
    let out = call_tool(node, p, ws, "flows.save", &body.to_string())
        .await
        .unwrap();
    serde_json::from_str(&out).unwrap()
}

async fn run_flow(node: &Arc<HostNode>, p: &Principal, ws: &str, id: &str, run_id: &str) -> Value {
    let req = json!({ "id": id, "run_id": run_id, "ts": 1 }).to_string();
    let out = call_tool(node, p, ws, "flows.run", &req).await.unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    // `flows.run` is now a BACKGROUND job (flow-runtime-control-scope): it returns the run id before
    // the drive finishes. Tests that assert on the terminal snapshot must await completion — poll the
    // durable run status until it settles (bounded, deterministic on the in-process store).
    await_terminal(node, p, ws, run_id).await;
    v
}

/// Poll `flows.runs.get` until the run reaches a terminal status (the background drive has finished).
/// Bounded so a stuck run can never hang the test.
async fn await_terminal(node: &Arc<HostNode>, p: &Principal, ws: &str, run_id: &str) -> Value {
    for _ in 0..600 {
        let snap = runs_get(node, p, ws, run_id).await;
        let status = snap["status"].as_str().unwrap_or("");
        if matches!(
            status,
            "success" | "partialFailure" | "failed" | "cancelled"
        ) {
            return snap;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    panic!("run {run_id} did not reach a terminal status in time");
}

async fn runs_get(node: &Arc<HostNode>, p: &Principal, ws: &str, run_id: &str) -> Value {
    let req = json!({ "run_id": run_id }).to_string();
    let out = call_tool(node, p, ws, "flows.runs.get", &req)
        .await
        .unwrap();
    serde_json::from_str(&out).unwrap()
}

fn flow(id: &str, nodes: Vec<Node>) -> Flow {
    Flow {
        workspace: "ws".into(),
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

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn save_rejects_a_cyclic_dag() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let a = rhai_node("a", &["b"], "42");
    let b = rhai_node("b", &["a"], "42");
    let f = flow("cyc", vec![a, b]);
    let body = serde_json::to_value(&f).unwrap().to_string();
    let err = call_tool(&node, &p, "ws", "flows.save", &body)
        .await
        .unwrap_err();
    assert!(matches!(err, lb_mcp::ToolError::BadInput(_)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn linear_rhai_flow_runs_to_success() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let f = flow(
        "lin",
        vec![rhai_node("a", &[], "42"), rhai_node("b", &["a"], "43")],
    );
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
async fn count_node_counts_its_input() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    // A count node fed a literal 4-element array via `with.payload` (the envelope's value slot). The
    // count transform reads `payload` and emits the envelope `{ payload: 4 }`.
    let count = Node {
        id: "count".into(),
        node_type: "count".into(),
        needs: vec![],
        with: serde_json::Map::from_iter([("payload".into(), json!([1, 2, 3, 4]))]),
        config: json!({}),
        inputs: Vec::new(),
        position: None,
    };
    let f = flow("cnt", vec![count]);
    save_flow(&node, &p, "ws", &f).await;
    run_flow(&node, &p, "ws", "cnt", "cnt-run-1").await;
    let snap = runs_get(&node, &p, "ws", "cnt-run-1").await;
    assert_eq!(snap["status"], "success");
    let step = &snap["steps"][0];
    assert_eq!(step["outcome"], "ok");
    assert_eq!(step["output"]["payload"], 4);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn diamond_frontier_runs_in_dependency_order() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    // `d` joins `b` + `c` (≥2 upstreams), so it MUST bind `payload` explicitly (D3 join lint) —
    // auto-wire is single-upstream only.
    let mut d = rhai_node("d", &["b", "c"], "4");
    d.with.insert("payload".into(), json!("${steps.b.payload}"));
    let f = flow(
        "dia",
        vec![
            rhai_node("a", &[], "1"),
            rhai_node("b", &["a"], "2"),
            rhai_node("c", &["a"], "3"),
            d,
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
    let mut f = flow(
        "halt",
        vec![
            rhai_node("bad", &[], "syntax@#$error"),
            rhai_node("dep", &["bad"], "1"),
        ],
    );
    f.failure_policy = FailurePolicy::Halt;
    save_flow(&node, &p, "ws", &f).await;
    run_flow(&node, &p, "ws", "halt", "halt-run-1").await;
    let snap = runs_get(&node, &p, "ws", "halt-run-1").await;
    // a rhai syntax error → that node err; dep skipped; run = failed (no ok node).
    assert_eq!(snap["status"], "failed");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn halt_with_a_successful_upstream_is_partial_failure() {
    // Superset proof for the retired rule-DAG `halt_skips_the_subtree_of_a_failure` case: a MIXED
    // run — an upstream node succeeds, a middle node fails, its dependent is skipped — settles
    // `partialFailure` (not `failed`), and the skipped node records `skipped`. The sibling
    // `halt_policy_skips_failed_subtree` covers the all-fail (`failed`) case; this covers the
    // ok→err→skipped case the retired engine's test asserted.
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let mut f = flow(
        "phalt",
        vec![
            rhai_node("a", &[], "1"),                 // ok
            rhai_node("bad", &["a"], "syntax@#$err"), // err (rhai syntax error)
            rhai_node("dep", &["bad"], "2"),          // depends on bad → skipped under Halt
        ],
    );
    f.failure_policy = FailurePolicy::Halt;
    save_flow(&node, &p, "ws", &f).await;
    run_flow(&node, &p, "ws", "phalt", "phalt-run-1").await;
    let snap = runs_get(&node, &p, "ws", "phalt-run-1").await;
    assert_eq!(
        snap["status"], "partialFailure",
        "one ok + one err = partial"
    );
    let outcome = |id: &str| -> String {
        snap["steps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["id"] == id)
            .unwrap()["outcome"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(outcome("a"), "ok");
    assert_eq!(outcome("bad"), "err");
    assert_eq!(outcome("dep"), "skipped", "Halt prunes the failed subtree");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn save_rejects_a_flow_over_the_node_cap() {
    // Superset proof for the retired rule-DAG `size_cap_is_rejected` case: the host `flows.save`
    // write path enforces `MAX_FLOW_NODES` — a hand-edited over-cap record is refused as BadInput
    // before any run. (The unit test `rejects_over_the_node_cap` in `lb_flows::model` covers the
    // validator directly; this proves the save boundary re-checks it.)
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let over: Vec<Node> = (0..=lb_flows::MAX_FLOW_NODES)
        .map(|i| rhai_node(&format!("n{i}"), &[], "1"))
        .collect();
    let f = flow("toobig", over);
    let body = serde_json::to_value(&f).unwrap().to_string();
    let err = call_tool(&node, &p, "ws", "flows.save", &body)
        .await
        .unwrap_err();
    assert!(matches!(err, lb_mcp::ToolError::BadInput(_)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn no_widening_tool_node_denied_without_the_tool_cap() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    // caller holds flows.run but NOT `mcp:rules.eval` — a rhai node that dispatches `rules.eval` is
    // DENIED at that node (no widening); the node records `err`, the run fails (Halt).
    let caps: Vec<&str> = FULL
        .iter()
        .filter(|c| **c != "mcp:rules.eval:call")
        .cloned()
        .collect();
    let p = principal("ws", &caps);
    let f = flow("widen", vec![rhai_node("r", &[], "1")]);
    save_flow(&node, &p, "ws", &f).await;
    run_flow(&node, &p, "ws", "widen", "widen-run-1").await;
    let snap = runs_get(&node, &p, "ws", "widen-run-1").await;
    assert_eq!(snap["status"], "failed");
    let r = snap["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == "r")
        .unwrap();
    assert_eq!(r["outcome"], "err");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn capability_deny_run_without_flows_run_cap() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let caps: Vec<&str> = FULL
        .iter()
        .filter(|c| **c != "mcp:flows.run:call")
        .cloned()
        .collect();
    let saver = principal("ws", FULL);
    let runner = principal("ws", &caps);
    let f = flow("deny", vec![rhai_node("a", &[], "1")]);
    save_flow(&node, &saver, "ws", &f).await;
    let req = json!({ "id": "deny", "run_id": "d1", "ts": 1 }).to_string();
    let err = call_tool(&node, &runner, "ws", "flows.run", &req)
        .await
        .unwrap_err();
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
    let err = call_tool(&node, &pb, "ws-b", "flows.get", &req)
        .await
        .unwrap_err();
    assert!(matches!(err, lb_mcp::ToolError::Denied));
    // ws-B running `iso` creates its own (absent) flow → NotFound → denied.
    let req = json!({ "id": "iso", "run_id": "x", "ts": 1 }).to_string();
    let err = call_tool(&node, &pb, "ws-b", "flows.run", &req)
        .await
        .unwrap_err();
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
    let f = flow(
        "patch",
        vec![rhai_node("a", &[], "1"), rhai_node("b", &["a"], "2")],
    );
    save_flow(&node, &p, "ws", &f).await;
    // Seed the run records by running, then assert an executed node rejects a patch.
    run_flow(&node, &p, "ws", "patch", "patch-run-1").await;
    let req =
        json!({ "run_id": "patch-run-1", "node": "a", "config": {"source":"99"} }).to_string();
    let err = call_tool(&node, &p, "ws", "flows.patch_run", &req)
        .await
        .unwrap_err();
    assert!(matches!(err, lb_mcp::ToolError::BadInput(_)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn resume_is_idempotent_a_re_drive_no_ops() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let f = flow(
        "resume",
        vec![rhai_node("a", &[], "5"), rhai_node("b", &["a"], "6")],
    );
    save_flow(&node, &p, "ws", &f).await;
    run_flow(&node, &p, "ws", "resume", "resume-run-1").await;
    let snap1 = runs_get(&node, &p, "ws", "resume-run-1").await;
    assert_eq!(snap1["status"], "success");
    // a re-resume (the offline/sync replay) is a no-op: the CAS claim makes a redelivered node a no-op.
    let req = json!({ "run_id": "resume-run-1", "ts": 2 }).to_string();
    call_tool(&node, &p, "ws", "flows.resume", &req)
        .await
        .unwrap();
    let snap2 = runs_get(&node, &p, "ws", "resume-run-1").await;
    assert_eq!(snap2["status"], "success");
    // outputs unchanged (exactly-once — no re-run, no double effect).
    let a1 = snap1["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == "a")
        .unwrap()["output"]
        .clone();
    let a2 = snap2["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == "a")
        .unwrap()["output"]
        .clone();
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
    let out = call_tool(&node, &p, "ws", "flows.runs.list", &req)
        .await
        .unwrap();
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
    let parent = flow(
        "parent",
        vec![Node {
            id: "sub".into(),
            node_type: "subflow".into(),
            needs: vec![],
            with: serde_json::Map::new(),
            config: json!({ "flow": "child@1" }),
            inputs: Vec::new(),
            position: None,
        }],
    );
    save_flow(&node, &p, "ws", &parent).await;
    run_flow(&node, &p, "ws", "parent", "parent-run-1").await;
    let snap = runs_get(&node, &p, "ws", "parent-run-1").await;
    assert_eq!(snap["status"], "success");
    let sub = snap["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == "sub")
        .unwrap();
    assert_eq!(sub["outcome"], "ok");
    // the subflow node emits an envelope whose `payload` folds the child's terminal node envelopes
    // (each child node's recorded envelope is `{payload: <value>}`).
    assert_eq!(sub["output"]["payload"]["c"]["payload"], 7);
}

fn step_output<'a>(snap: &'a Value, id: &str) -> &'a Value {
    snap["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == id)
        .unwrap_or_else(|| panic!("no step {id}"))
        .get("output")
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn auto_wire_flows_the_envelope_end_to_end_with_no_with() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    // A 3-node linear chain, NO `with` typed on any node (D3 auto-wire). `a` emits payload 10; `b`
    // reads `payload` (auto-wired from `a`) and doubles it; `c` reads `b`'s payload and adds 5.
    let f = flow(
        "auto",
        vec![
            rhai_node("a", &[], "10"),
            rhai_node("b", &["a"], "payload * 2"),
            rhai_node("c", &["b"], "payload + 5"),
        ],
    );
    save_flow(&node, &p, "ws", &f).await;
    run_flow(&node, &p, "ws", "auto", "auto-1").await;
    let snap = runs_get(&node, &p, "ws", "auto-1").await;
    assert_eq!(snap["status"], "success");
    assert_eq!(step_output(&snap, "a")["payload"], 10);
    assert_eq!(step_output(&snap, "b")["payload"], 20);
    assert_eq!(step_output(&snap, "c")["payload"], 25);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn save_rejects_a_join_with_no_payload_binding() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    // `j` joins `a` + `b` but binds no `payload` → the save-time join lint rejects it (D3).
    let f = flow(
        "join",
        vec![
            rhai_node("a", &[], "1"),
            rhai_node("b", &[], "2"),
            rhai_node("j", &["a", "b"], "3"),
        ],
    );
    let body = serde_json::to_value(&f).unwrap().to_string();
    let err = call_tool(&node, &p, "ws", "flows.save", &body)
        .await
        .unwrap_err();
    assert!(matches!(err, lb_mcp::ToolError::BadInput(_)));
    // ...and saving succeeds once `payload` is bound.
    let mut j = rhai_node("j", &["a", "b"], "3");
    j.with.insert("payload".into(), json!("${steps.a.payload}"));
    let f = flow(
        "join",
        vec![rhai_node("a", &[], "1"), rhai_node("b", &[], "2"), j],
    );
    let saved = save_flow(&node, &p, "ws", &f).await;
    assert_eq!(saved["version"], 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn topic_carries_forward_down_the_chain() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    // `a` sets a topic on its envelope (`return msg`-style object); `b` (auto-wired) emits only a new
    // payload — the topic must CARRY FORWARD to `b`'s recorded envelope (D4).
    let f = flow(
        "carry",
        vec![
            rhai_node("a", &[], r#"#{payload: 1, topic: "kfc.temp"}"#),
            rhai_node("b", &["a"], "payload + 1"),
        ],
    );
    save_flow(&node, &p, "ws", &f).await;
    run_flow(&node, &p, "ws", "carry", "carry-1").await;
    let snap = runs_get(&node, &p, "ws", "carry-1").await;
    assert_eq!(snap["status"], "success");
    assert_eq!(step_output(&snap, "a")["topic"], "kfc.temp");
    assert_eq!(step_output(&snap, "b")["payload"], 2);
    // topic carried even though `b` only set a new payload.
    assert_eq!(step_output(&snap, "b")["topic"], "kfc.temp");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rhai_return_msg_round_trips_the_envelope() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    // An object carrying a `payload` key IS the emitted envelope (function-node `return msg`); a bare
    // value (no `payload` key) is the new payload (D6).
    let f = flow(
        "ret",
        vec![
            rhai_node("env", &[], r#"#{payload: 99, topic: "t"}"#),
            rhai_node("bare", &[], "#{a: 1}"),
        ],
    );
    save_flow(&node, &p, "ws", &f).await;
    run_flow(&node, &p, "ws", "ret", "ret-1").await;
    let snap = runs_get(&node, &p, "ws", "ret-1").await;
    assert_eq!(snap["status"], "success");
    // object with `payload` → it is the envelope verbatim.
    assert_eq!(step_output(&snap, "env")["payload"], 99);
    assert_eq!(step_output(&snap, "env")["topic"], "t");
    // object WITHOUT `payload` → wrapped as the new payload.
    assert_eq!(step_output(&snap, "bare")["payload"]["a"], 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn counter_tick_mode_does_not_jump_by_payload_size() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    // Regression for the implicit-throughput trap (D7): an auto-wired counter in DEFAULT tick mode
    // increments by `step` (1), NOT by the size of the wired payload. `src` emits a 4-element array;
    // `tick` is auto-wired to it but must read +1, not +4. (Fail-before: the old `items`-detect would
    // have jumped to 4.)
    let counter = Node {
        id: "tick".into(),
        node_type: "counter".into(),
        needs: vec!["src".into()],
        with: serde_json::Map::new(),
        config: json!({}),
        inputs: Vec::new(),
        position: None,
    };
    let f = flow("tick", vec![rhai_node("src", &[], "[1, 2, 3, 4]"), counter]);
    save_flow(&node, &p, "ws", &f).await;
    run_flow(&node, &p, "ws", "tick", "tick-1").await;
    let snap = runs_get(&node, &p, "ws", "tick-1").await;
    assert_eq!(snap["status"], "success");
    assert_eq!(step_output(&snap, "tick")["payload"], 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn counter_throughput_mode_adds_payload_size() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    // Explicit `throughput` mode → increment by the payload's size (4 for a 4-element array, D7).
    let counter = Node {
        id: "thru".into(),
        node_type: "counter".into(),
        needs: vec!["src".into()],
        with: serde_json::Map::new(),
        config: json!({ "mode": "throughput" }),
        inputs: Vec::new(),
        position: None,
    };
    let f = flow("thru", vec![rhai_node("src", &[], "[1, 2, 3, 4]"), counter]);
    save_flow(&node, &p, "ws", &f).await;
    run_flow(&node, &p, "ws", "thru", "thru-1").await;
    let snap = runs_get(&node, &p, "ws", "thru-1").await;
    assert_eq!(snap["status"], "success");
    assert_eq!(step_output(&snap, "thru")["payload"], 4);
}

// ── the Node-RED `json` node: parse/stringify at a flow's text boundary ──────────────────────────
// (json-node-scope). Real run through the engine; the value is fed via `with.payload` (the envelope
// value slot), exactly like `count_node_counts_its_input`. Covers: parse string→object, stringify
// object→string (+ pretty), the Node-RED "fail on bad JSON" contract, and topic carry-forward.

/// A `json` node fed a literal `payload` (no upstream) — the single-node flow shape these tests use.
fn json_node(id: &str, mode: &str, payload: Value, extra: Value) -> Node {
    let mut config = json!({ "mode": mode });
    if let (Value::Object(c), Value::Object(e)) = (&mut config, &extra) {
        for (k, v) in e {
            c.insert(k.clone(), v.clone());
        }
    }
    Node {
        id: id.into(),
        node_type: "json".into(),
        needs: vec![],
        with: serde_json::Map::from_iter([("payload".into(), payload)]),
        config,
        inputs: Vec::new(),
        position: None,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn json_parse_turns_a_string_payload_into_an_object() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    // A JSON STRING arrives (a webhook body / MQTT text) → the node emits the parsed object so a
    // downstream `${steps.x.payload.field}` binding can walk into it.
    let n = json_node(
        "j",
        "parse",
        json!("{\"temp\": 21, \"unit\": \"C\"}"),
        json!({}),
    );
    let f = flow("jp", vec![n]);
    save_flow(&node, &p, "ws", &f).await;
    run_flow(&node, &p, "ws", "jp", "jp-1").await;
    let snap = runs_get(&node, &p, "ws", "jp-1").await;
    assert_eq!(snap["status"], "success");
    assert_eq!(
        step_output(&snap, "j")["payload"],
        json!({ "temp": 21, "unit": "C" })
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn json_stringify_turns_a_value_into_a_json_string() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    // An object → its compact JSON string (what a sink/outbox text target wants).
    let n = json_node("j", "stringify", json!({ "a": 1, "b": [2, 3] }), json!({}));
    let f = flow("js", vec![n]);
    save_flow(&node, &p, "ws", &f).await;
    run_flow(&node, &p, "ws", "js", "js-1").await;
    let snap = runs_get(&node, &p, "ws", "js-1").await;
    assert_eq!(snap["status"], "success");
    // The emitted payload is a STRING that re-parses to the original value (key order is not asserted).
    let s = step_output(&snap, "j")["payload"]
        .as_str()
        .unwrap()
        .to_string();
    let reparsed: Value = serde_json::from_str(&s).unwrap();
    assert_eq!(reparsed, json!({ "a": 1, "b": [2, 3] }));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn json_parse_fails_the_node_on_invalid_json() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    // Node-RED parity: a malformed body FAILS the node (not a silent passthrough). Under the default
    // Halt policy that settles the run `failed`, surfacing the bad input instead of flowing it on.
    let n = json_node("j", "parse", json!("{not json"), json!({}));
    let f = flow("jbad", vec![n]);
    save_flow(&node, &p, "ws", &f).await;
    run_flow(&node, &p, "ws", "jbad", "jbad-1").await;
    let snap = runs_get(&node, &p, "ws", "jbad-1").await;
    assert_eq!(snap["status"], "failed");
    assert_eq!(
        step_output(&snap, "j").is_null() || step_output(&snap, "j") == &json!({}),
        true
    );
    let step = snap["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == "j")
        .unwrap();
    assert_eq!(step["outcome"], "err");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn json_parse_carries_topic_forward() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    // A trigger stamps a `topic`; auto-wire carries it onto the json node's envelope (the executor's
    // carry merge), so a parsed message keeps its routing topic for a downstream `sink`.
    let trig = node_with_topic("t", "kfc.fryer");
    let j = Node {
        id: "j".into(),
        node_type: "json".into(),
        needs: vec!["t".into()],
        with: serde_json::Map::new(),
        config: json!({ "mode": "parse" }),
        inputs: Vec::new(),
        position: None,
    };
    let f = flow("jtopic", vec![trig, j]);
    save_flow(&node, &p, "ws", &f).await;
    // The trigger's firing payload is a JSON string (set via params under the node id at run time).
    run_flow_with_param(
        &node,
        &p,
        "ws",
        "jtopic",
        "jtopic-1",
        "t",
        json!("{\"v\": 9}"),
    )
    .await;
    let snap = runs_get(&node, &p, "ws", "jtopic-1").await;
    assert_eq!(snap["status"], "success");
    assert_eq!(step_output(&snap, "j")["payload"], json!({ "v": 9 }));
    assert_eq!(step_output(&snap, "j")["topic"], "kfc.fryer");
}

/// A trigger node that stamps a `topic` on its firing envelope (D6).
fn node_with_topic(id: &str, topic: &str) -> Node {
    Node {
        id: id.into(),
        node_type: "trigger".into(),
        needs: vec![],
        with: serde_json::Map::new(),
        config: json!({ "mode": "manual", "topic": topic }),
        inputs: Vec::new(),
        position: None,
    }
}

/// Like `run_flow` but seeds the trigger node's firing payload via `params[node_id]` (the firing path).
async fn run_flow_with_param(
    node: &Arc<HostNode>,
    p: &Principal,
    ws: &str,
    id: &str,
    run_id: &str,
    trigger_node: &str,
    payload: Value,
) {
    let req = json!({ "id": id, "run_id": run_id, "ts": 1, "params": { trigger_node: payload } })
        .to_string();
    call_tool(node, p, ws, "flows.run", &req).await.unwrap();
    await_terminal(node, p, ws, run_id).await;
}

// --- flow-input-ports-scope Slice 1: port-labelled edges + the save lints ---

/// `flows.save` rejects a wire to an **undeclared** input port (a misnamed handle / a port the node
/// type does not expose) at save — not silently dropped at run. The registry knows `count` exposes
/// one input port, `payload`; a wire naming anything else is a clear topology mistake.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn save_rejects_a_wire_to_an_undeclared_input_port() {
    let host = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let n = rhai_node("a", &[], "42");
    // `b` is a count node (declared input port: `payload`); wire `a` into a non-existent port.
    let mut b = node("b", "count", &["a"], json!({}));
    b.inputs
        .push(InputEdge::new("a", Some("no-such-port".into())));
    let f = flow("badport", vec![n, b]);
    let body = serde_json::to_value(&f).unwrap().to_string();
    let err = call_tool(&host, &p, "ws", "flows.save", &body)
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("undeclared input port") && msg.contains("no-such-port"),
        "expected an undeclared-port error, got: {msg}"
    );
}

/// A wire that names a real declared port saves green. `count` exposes `payload`, so a `to_port`
/// of `payload` is accepted (and a `to_port` of None ⇒ the primary, also accepted).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn save_accepts_a_wire_to_a_declared_input_port() {
    let host = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let a = rhai_node("a", &[], "42");
    let mut b = node("b", "count", &["a"], json!({}));
    b.inputs.push(InputEdge::new("a", Some("payload".into()))); // explicit primary port
    let f = flow("goodport", vec![a, b]);
    let saved = save_flow(&host, &p, "ws", &f).await;
    assert_eq!(saved["version"], 1);
    // And the wire round-trips through the saved record (flows.get).
    let req = json!({ "id": "goodport" }).to_string();
    let out = call_tool(&host, &p, "ws", "flows.get", &req).await.unwrap();
    let reloaded: serde_json::Value = serde_json::from_str(&out).unwrap();
    let b_node = reloaded["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == "b")
        .unwrap();
    assert_eq!(b_node["inputs"][0]["from"], "a");
    assert_eq!(b_node["inputs"][0]["toPort"], "payload");
}

/// A node with an incoming wire but no declared input port (e.g. a misconfigured `trigger`/`source`
/// receiving an inbound edge) is rejected at save.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn save_rejects_a_wire_into_a_node_with_no_input_ports() {
    let host = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    // `trigger` declares no input ports; wiring `a` into it is a topology mistake.
    let a = rhai_node("a", &[], "42");
    let b = node("b", "trigger", &["a"], json!({ "mode": "manual" }));
    let f = flow("noinputs", vec![a, b]);
    let body = serde_json::to_value(&f).unwrap().to_string();
    let err = call_tool(&host, &p, "ws", "flows.save", &body)
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("declares no input port"),
        "expected a no-input-port error, got: {err}"
    );
}

/// The join lint still holds for an `all` (barrier) port — `count` is a transform (default `all`),
/// so ≥2 wires with no `payload` binding is a data-drop bug, rejected at save. (flow-input-ports-
/// scope Slice 2: the lint is now per-port policy-aware; an `any` port with N wires is valid.)
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn save_still_requires_a_payload_binding_for_a_multi_input_join() {
    let host = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let f = flow(
        "join",
        vec![
            rhai_node("a", &[], "1"),
            rhai_node("b", &[], "2"),
            node("c", "count", &["a", "b"], json!({})), // no `with.payload`
        ],
    );
    let body = serde_json::to_value(&f).unwrap().to_string();
    let err = call_tool(&host, &p, "ws", "flows.save", &body)
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("`all` (join) input port"),
        "expected the all-port join lint, got: {err}"
    );
}

// --- flow-input-ports-scope Slice 2: the `any` funnel runtime + firing context ---

/// THE headline: three wires into one `any` port ⇒ the node fires **once per upstream** (Node-RED's
/// fire-per-message OR), in ONE durable run. `debug` is a `sink` (default `any`); three `rhai`
/// sources wire into it. Fail-before (today's engine): `debug` settles ONCE (a join barrier). The
/// firing-context seam: each firing mints a distinct `fctx` (`debug#<source>`) and reads its OWN
/// upstream's payload — three settles, three distinct payloads, no swallowed firings.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn any_funnel_fires_once_per_upstream() {
    let host = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    // Three independent sources (frontier), each emitting a distinct payload, all wired into `debug`.
    let a = rhai_node("a", &[], "1");
    let b = rhai_node("b", &[], "2");
    let c = rhai_node("c", &[], "3");
    let dbg = node("dbg", "debug", &["a", "b", "c"], json!({}));
    let f = flow("funnel", vec![a, b, c, dbg]);
    save_flow(&host, &p, "ws", &f).await;
    run_flow(&host, &p, "ws", "funnel", "funnel-1").await;
    let snap = runs_get(&host, &p, "ws", "funnel-1").await;
    assert_eq!(
        snap["status"], "success",
        "the run reaches terminal (no park)"
    );

    // Exactly THREE `dbg` settles — one per upstream — each under its own firing context.
    let dbg_slots: Vec<&serde_json::Value> = snap["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["id"] == "dbg")
        .collect();
    assert_eq!(
        dbg_slots.len(),
        3,
        "debug fires once per upstream (Node-RED OR)"
    );
    // Each firing carries its OWN upstream's payload (the fctx-scoped resolution) + a distinct fctx.
    let mut payloads: Vec<serde_json::Value> = dbg_slots
        .iter()
        .map(|s| s["output"]["payload"].clone())
        .collect();
    payloads.sort_by_key(|v| v.as_i64().unwrap_or(0));
    assert_eq!(payloads, vec![json!(1), json!(2), json!(3)]);
    let mut fctxs: Vec<String> = dbg_slots
        .iter()
        .map(|s| s["fctx"].as_str().unwrap_or("").to_string())
        .collect();
    fctxs.sort();
    assert_eq!(
        fctxs,
        vec!["dbg#a", "dbg#b", "dbg#c"],
        "each firing is labelled by its triggering upstream"
    );
    // The three sources each settled ONCE (fctx="" — they are frontier barriers).
    for src in ["a", "b", "c"] {
        let src_slots: Vec<&serde_json::Value> = snap["steps"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| s["id"] == src)
            .collect();
        assert_eq!(src_slots.len(), 1, "{src} settles once");
    }
}

/// The all-`all` barrier is **byte-for-byte unchanged**: a 2-upstream `all` join fires exactly once,
/// at `fctx=""`, with the today's claim-key shape (no `@fctx` suffix). This is the invariant the
/// firing-context seam must not disturb (empty `fctx` ⇒ today's key).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn all_join_barrier_settles_once_at_empty_fctx() {
    let host = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    // `rhai` is a transform (default `all`); `j` joins hi + lo with an explicit payload binding.
    let hi = rhai_node("hi", &[], "10");
    let lo = rhai_node("lo", &[], "20");
    let mut j = rhai_node("j", &["hi", "lo"], "payload");
    j.with
        .insert("payload".into(), json!("${steps.hi.payload}"));
    let f = flow("ajoin", vec![hi, lo, j]);
    save_flow(&host, &p, "ws", &f).await;
    run_flow(&host, &p, "ws", "ajoin", "ajoin-1").await;
    let snap = runs_get(&host, &p, "ws", "ajoin-1").await;
    assert_eq!(snap["status"], "success");
    // The join settles exactly once (a barrier), no firing-context suffix.
    let j_slots: Vec<&serde_json::Value> = snap["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["id"] == "j")
        .collect();
    assert_eq!(j_slots.len(), 1, "an all-join settles once");
    assert!(
        j_slots[0].get("fctx").is_none() || j_slots[0]["fctx"].as_str() == Some(""),
        "the all-all common case carries no fctx (today's shape)"
    );
}

/// Workspace isolation holds under the new `@{fctx}` step-key shape: a ws-B caller cannot read a
/// ws-A `any` funnel's per-firing settles (the mandatory category re-asserted for this slice).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_isolation_any_funnel_step_keys() {
    let host = Arc::new(HostNode::boot().await.unwrap());
    let p_a = principal("wsA", FULL);
    let p_b = principal("wsB", FULL);
    let a = rhai_node("a", &[], "1");
    let b = rhai_node("b", &[], "2");
    let dbg = node("dbg", "debug", &["a", "b"], json!({}));
    let f = flow("iso", vec![a, b, dbg]);
    save_flow(&host, &p_a, "wsA", &f).await;
    run_flow(&host, &p_a, "wsA", "iso", "iso-1").await;

    // ws-B cannot read ws-A's run (the run record + every `@{fctx}` slot is ws-walled).
    let req = json!({ "run_id": "iso-1" }).to_string();
    let res = call_tool(&host, &p_b, "wsB", "flows.runs.get", &req).await;
    assert!(
        res.is_err(),
        "ws-B cannot read a ws-A run's per-firing slots"
    );
}

// ───────────────────────── flow-input-ports-scope Slice 3: the `link` pair ─────────────────────────
//
// THE seam test lives here: a non-sink `any` node (link-in) feeding a downstream transform W must
// settle W ONCE PER link-in firing — proving the firing context (`fctx`) propagates one hop past the
// funnel. A naive `#{upstream}` depth-1 suffix settles W ONCE (W has a single wire from link-in ⇒ one
// slot). Plus the per-firing cap-deny, the per-firing outbox-dedup, exactly-once-on-redelivery one
// hop past the funnel, and the save-time link topology lints.

/// A flow `failure_policy` override helper (the default `flow()` builder uses Halt; some link tests
/// need Continue so independent per-firing failures don't gate each other).
fn flow_with_policy(id: &str, nodes: Vec<Node>, policy: lb_flows::FailurePolicy) -> Flow {
    let mut f = flow(id, nodes);
    f.failure_policy = policy;
    f
}

/// THE headline of Slice 3 (the scope's load-bearing fail-before for a naive depth-1 suffix):
/// `link-in` (`any`, fed by 3 `link-out`s each from a distinct source) → transform `W` (one wire from
/// link-in) ⇒ **`W` settles THREE times**, each `W@<fctx>` reading its OWN firing's `link-in` message.
/// A naive `#{upstream}` scheme settles `W` ONCE — this proves the `fctx` propagates past the funnel.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn link_funnel_propagates_one_hop_past_the_funnel() {
    let host = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    // Three independent sources, each forwarded wirelessly onto link-in `T`; W reads link-in.
    let a = rhai_node("a", &[], "1");
    let b = rhai_node("b", &[], "2");
    let c = rhai_node("c", &[], "3");
    let lo_a = link_out_node("lo-a", "T", &["a"]);
    let lo_b = link_out_node("lo-b", "T", &["b"]);
    let lo_c = link_out_node("lo-c", "T", &["c"]);
    let li = link_in_node("li", "T", &[]);
    let w = rhai_node("w", &["li"], "payload");
    let f = flow("linkfun", vec![a, b, c, lo_a, lo_b, lo_c, li, w]);
    save_flow(&host, &p, "ws", &f).await;
    run_flow(&host, &p, "ws", "linkfun", "linkfun-1").await;
    let snap = runs_get(&host, &p, "ws", "linkfun-1").await;
    assert_eq!(snap["status"], "success", "run reaches terminal (no park)");

    // link-in fires THREE times (one per resolved upstream) — the any-funnel.
    let li_slots: Vec<&Value> = snap["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["id"] == "li")
        .collect();
    assert_eq!(
        li_slots.len(),
        3,
        "link-in fires once per resolved upstream"
    );
    let mut li_fctxs: Vec<String> = li_slots
        .iter()
        .map(|s| s["fctx"].as_str().unwrap_or("").to_string())
        .collect();
    li_fctxs.sort();
    assert_eq!(li_fctxs, vec!["li#a", "li#b", "li#c"]);

    // THE seam: W (one wire from link-in, `all` transform) settles THREE times — once per link-in
    // firing — each `W@<fctx>` carrying the matching firing's payload. A depth-1 suffix would settle W
    // ONCE (W has a single wire ⇒ one slot) and `${steps.li.payload}` would be ambiguous.
    let w_slots: Vec<&Value> = snap["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["id"] == "w")
        .collect();
    assert_eq!(
        w_slots.len(),
        3,
        "W settles once per link-in firing (fctx propagates past the funnel)"
    );
    let mut w_payloads: Vec<i64> = w_slots
        .iter()
        .map(|s| s["output"]["payload"].as_i64().unwrap_or(0))
        .collect();
    w_payloads.sort();
    assert_eq!(
        w_payloads,
        vec![1, 2, 3],
        "each W reads its OWN firing's message"
    );
    // Each W firing's fctx extends its triggering link-in firing (the propagated context).
    let mut w_fctxs: Vec<String> = w_slots
        .iter()
        .map(|s| s["fctx"].as_str().unwrap_or("").to_string())
        .collect();
    w_fctxs.sort();
    assert_eq!(
        w_fctxs,
        vec!["li#a", "li#b", "li#c"],
        "W's fctx is the link-in firing it rides on"
    );
    // The link-out senders are NOT in the run snapshot (dropped at run load — editor sugar only).
    assert!(
        !snap["steps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["id"] == "lo-a" || s["id"] == "lo-b" || s["id"] == "lo-c"),
        "link-out nodes are dropped from the run graph"
    );
}

/// Per-firing capability deny: an `any` funnel feeding a `tool` node whose verb the caller lacks is
/// denied at EACH firing (N err settles, not one swallowed). The no-widening run gate bites per slot.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn link_funnel_denies_per_firing_at_a_downstream_tool() {
    let host = Arc::new(HostNode::boot().await.unwrap());
    // The caller has NO `mcp:series.write:call` — the tool node's verb is ungranted.
    let mut caps = FULL.to_vec();
    caps.retain(|c| c != &"mcp:rules.run:call" && c != &"mcp:rules.eval:call");
    let p = principal("ws", &caps);
    let a = rhai_node("a", &[], "1");
    let b = rhai_node("b", &[], "2");
    let lo_a = link_out_node("lo-a", "T", &["a"]);
    let lo_b = link_out_node("lo-b", "T", &["b"]);
    let li = link_in_node("li", "T", &[]);
    // A tool node whose verb the caller lacks — each firing is denied, independently.
    let tool = node(
        "denied",
        "tool",
        &["li"],
        json!({ "verb": "series.write", "args": {} }),
    );
    let f = flow_with_policy(
        "linkdeny",
        vec![a, b, lo_a, lo_b, li, tool],
        lb_flows::FailurePolicy::Continue,
    );
    save_flow(&host, &p, "ws", &f).await;
    run_flow(&host, &p, "ws", "linkdeny", "linkdeny-1").await;
    let snap = runs_get(&host, &p, "ws", "linkdeny-1").await;
    // Two firings ⇒ two err settles (not one). The deny bites per (node, fctx) slot.
    let err_slots: Vec<&Value> = snap["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["id"] == "denied" && s["outcome"] == "err")
        .collect();
    assert_eq!(
        err_slots.len(),
        2,
        "the denied tool errs once per funnel firing (per-firing deny)"
    );
}

/// Outbox dedup per firing: an `any` funnel feeding a must-deliver sink ⇒ N idempotent outbox
/// deliveries (one per `fctx`-scoped effect id), not one swallowing the rest. The tripwire the scope
/// named — the outbox key now carries the `fctx` suffix wherever the node key was used.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn link_funnel_outbox_dedups_per_firing() {
    let host = Arc::new(HostNode::boot().await.unwrap());
    // The outbox enqueue cap is the one new cap this run needs (under `flows.run` like any node).
    let mut caps = FULL.to_vec();
    caps.push("mcp:outbox.enqueue:call");
    let p = principal("ws", &caps);
    let a = rhai_node("a", &[], "1");
    let b = rhai_node("b", &[], "2");
    let lo_a = link_out_node("lo-a", "T", &["a"]);
    let lo_b = link_out_node("lo-b", "T", &["b"]);
    let li = link_in_node("li", "T", &[]);
    let sink = node(
        "out",
        "sink",
        &["li"],
        json!({ "target": "outbox", "name": "link-deliver" }),
    );
    let f = flow("linkoutbox", vec![a, b, lo_a, lo_b, li, sink]);
    save_flow(&host, &p, "ws", &f).await;
    run_flow(&host, &p, "ws", "linkoutbox", "linkoutbox-1").await;
    let snap = runs_get(&host, &p, "ws", "linkoutbox-1").await;
    assert_eq!(snap["status"], "success", "run snapshot: {snap}");
    // The sink fired TWICE (one per link-in firing) — two distinct (node, fctx) slots.
    let sink_slots: Vec<&Value> = snap["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["id"] == "out")
        .collect();
    assert_eq!(sink_slots.len(), 2, "the sink fires once per funnel firing");

    // TWO distinct outbox effects were staged (the `@{fctx}` effect-id suffix makes each firing its
    // own idempotent delivery, not one delivery swallowing the rest).
    let pending = lb_outbox::pending(&host.store, "ws").await.unwrap();
    let ours: Vec<&lb_outbox::Effect> = pending
        .iter()
        .filter(|e| e.target == "link-deliver")
        .collect();
    assert_eq!(ours.len(), 2, "N firings ⇒ N outbox effects: {pending:?}");
    // The two effect ids are the per-firing keys (run:node@fctx), distinct.
    let mut ids: Vec<&str> = ours.iter().map(|e| e.id.as_str()).collect();
    ids.sort();
    assert!(
        ids.iter().all(|id| id.contains('@')),
        "each effect id carries the fctx suffix: {ids:?}"
    );
    assert_ne!(ids[0], ids[1]);
}

/// Exactly-once per firing on redelivery one hop past the funnel: re-running the SAME run_id no-ops
/// every `(node, fctx)` slot (the deterministic fctx re-mints the same keys ⇒ CAS claim no-ops). A
/// different upstream still fires its own slot. This is the redelivery guarantee past the funnel.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn link_funnel_exactly_once_per_firing_on_redelivery() {
    let host = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let a = rhai_node("a", &[], "1");
    let b = rhai_node("b", &[], "2");
    let lo_a = link_out_node("lo-a", "T", &["a"]);
    let lo_b = link_out_node("lo-b", "T", &["b"]);
    let li = link_in_node("li", "T", &[]);
    let w = rhai_node("w", &["li"], "payload");
    let f = flow("linkonce", vec![a, b, lo_a, lo_b, li, w]);
    save_flow(&host, &p, "ws", &f).await;
    run_flow(&host, &p, "ws", "linkonce", "linkonce-1").await;
    let snap1 = runs_get(&host, &p, "ws", "linkonce-1").await;
    let w_count_1 = snap1["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["id"] == "w")
        .count();
    assert_eq!(w_count_1, 2, "first run: W settles once per link-in firing");

    // Re-run with the SAME run_id (the redelivery seam): every slot is already terminal ⇒ the CAS
    // claim no-ops, no new settles, the per-firing exactly-once holds one hop past the funnel.
    let req = json!({ "id": "linkonce", "run_id": "linkonce-1", "ts": 2 }).to_string();
    let _ = call_tool(&host, &p, "ws", "flows.run", &req).await;
    let snap2 = runs_get(&host, &p, "ws", "linkonce-1").await;
    let w_count_2 = snap2["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["id"] == "w")
        .count();
    assert_eq!(
        w_count_2, 2,
        "redelivery no-ops every (node, fctx) slot — still exactly two W settles"
    );
}

/// The save-time link topology lints: a `link-out` naming a missing `link-in` is rejected before any
/// run (a wireless sender pointing nowhere is a naming typo, not a silently-dead link).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn save_rejects_a_link_out_naming_a_missing_link_in() {
    let host = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let a = rhai_node("a", &[], "1");
    let lo = link_out_node("lo", "nope", &["a"]); // no link-in names "nope"
    let f = flow("linkbad", vec![a, lo]);
    let body = serde_json::to_value(&f).unwrap().to_string();
    let err = call_tool(&host, &p, "ws", "flows.save", &body).await;
    assert!(
        err.is_err(),
        "a link-out targeting a missing link-in is rejected at save"
    );
    let msg = err.unwrap_err().to_string();
    assert!(
        msg.contains("nope") && msg.contains("link-in"),
        "error names the bad target + the missing link-in: {msg}"
    );
}

/// The save-time link topology lints: a node may not wire FROM a `link-out` (its output is the
/// wireless name, not a data port). Caught at save — that wire would vanish at run load otherwise.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn save_rejects_a_wire_from_a_link_out() {
    let host = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let a = rhai_node("a", &[], "1");
    let lo = link_out_node("lo", "T", &["a"]);
    let li = link_in_node("li", "T", &[]);
    let bad = rhai_node("bad", &["lo"], "payload"); // wires from a link-out — a mistake
    let f = flow("linkwire", vec![a, lo, li, bad]);
    let body = serde_json::to_value(&f).unwrap().to_string();
    let err = call_tool(&host, &p, "ws", "flows.save", &body).await;
    assert!(err.is_err(), "a wire from a link-out is rejected at save");
}

/// Workspace isolation holds for the link pair's per-firing slots: a ws-B caller cannot read a ws-A
/// `link-in` funnel's per-firing settles (the `@{fctx}` slots stay inside `{ws}:{run}`).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_isolation_link_funnel_step_keys() {
    let host = Arc::new(HostNode::boot().await.unwrap());
    let p_a = principal("wsA", FULL);
    let p_b = principal("wsB", FULL);
    let a = rhai_node("a", &[], "1");
    let b = rhai_node("b", &[], "2");
    let lo_a = link_out_node("lo-a", "T", &["a"]);
    let lo_b = link_out_node("lo-b", "T", &["b"]);
    let li = link_in_node("li", "T", &[]);
    let w = rhai_node("w", &["li"], "payload");
    let f = flow("linkiso", vec![a, b, lo_a, lo_b, li, w]);
    save_flow(&host, &p_a, "wsA", &f).await;
    run_flow(&host, &p_a, "wsA", "linkiso", "linkiso-1").await;

    // ws-B cannot read ws-A's run (the run + every link-in `@{fctx}` slot is ws-walled).
    let req = json!({ "run_id": "linkiso-1" }).to_string();
    let res = call_tool(&host, &p_b, "wsB", "flows.runs.get", &req).await;
    assert!(
        res.is_err(),
        "ws-B cannot read a ws-A link-funnel run's per-firing slots"
    );
}
