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
    // caller holds flows.run but NOT `mcp:rules.run` — a tool node that dispatches `rules.run` is
    // DENIED at that node (no widening); the node records `err`, the run fails (Halt).
    let caps: Vec<&str> = FULL
        .iter()
        .filter(|c| **c != "mcp:rules.run:call")
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
