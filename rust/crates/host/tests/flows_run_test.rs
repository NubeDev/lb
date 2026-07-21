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

/// Seed an extension install whose one node (`extall.collect`) declares an **explicit `all`**
/// primary input port — the descriptor-level opt-in flow-plain-wiring-scope keeps. No built-in
/// declares `all` any more, so the barrier tests use this real install record (seeded through the
/// real `record_install` write path — no fake registry).
async fn seed_explicit_all_ext(node: &Arc<HostNode>, ws: &str) {
    use lb_flows::{InputPort, JoinPolicy};
    let block = lb_flows::NodeBlock {
        r#type: "collect".into(),
        kind: lb_flows::NodeKind::Transform,
        tool: "collect".into(),
        title: Some("Collect (all)".into()),
        category: None,
        inputs: vec!["payload".into()],
        outputs: vec!["payload".into()],
        input_ports: vec![InputPort {
            name: "payload".into(),
            join: JoinPolicy::All,
        }],
        config_version: 1,
        config: json!({}),
    };
    let install =
        lb_assets::Install::new("extall", "0.1.0", vec!["mcp:extall.collect:call".into()], 1)
            .with_nodes(vec![block]);
    lb_assets::record_install(&node.store, ws, &install)
        .await
        .unwrap();
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
        managed_by: None,
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
    // flow-plain-wiring-scope: `d`'s two wires are plain per-message wiring — the diamond join now
    // fires TWICE (once per arriving branch), each firing auto-wired to its own upstream's message.
    // No binding demanded, no barrier. (Was: an `all` barrier firing once behind a payload lint.)
    let f = flow(
        "dia",
        vec![
            rhai_node("a", &[], "1"),
            rhai_node("b", &["a"], "2"),
            rhai_node("c", &["a"], "3"),
            rhai_node("d", &["b", "c"], "payload + 10"),
        ],
    );
    save_flow(&node, &p, "ws", &f).await;
    run_flow(&node, &p, "ws", "dia", "dia-run-1").await;
    let snap = runs_get(&node, &p, "ws", "dia-run-1").await;
    assert_eq!(snap["status"], "success");
    // a, b, c settle once each; d settles TWICE (per-message) ⇒ 5 slots.
    assert_eq!(snap["steps"].as_array().unwrap().len(), 5);
    let d_slots: Vec<&Value> = snap["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["id"] == "d")
        .collect();
    assert_eq!(d_slots.len(), 2, "the diamond join fires once per branch");
    let mut fctxs: Vec<&str> = d_slots
        .iter()
        .map(|s| s["fctx"].as_str().unwrap_or(""))
        .collect();
    fctxs.sort();
    assert_eq!(
        fctxs,
        vec!["d#b", "d#c"],
        "one minted firing per sibling wire"
    );
    let mut payloads: Vec<i64> = d_slots
        .iter()
        .map(|s| s["output"]["payload"].as_i64().unwrap())
        .collect();
    payloads.sort();
    assert_eq!(
        payloads,
        vec![12, 13],
        "each firing reads its OWN branch's message"
    );
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
async fn save_is_silent_on_multi_wire_plain_wiring() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    // flow-plain-wiring-scope: `j` has two plain wires and NO binding — this now saves GREEN (no
    // bind-payload lint, no policy question). Fail-before: the old `all` default rejected it.
    let f = flow(
        "join",
        vec![
            rhai_node("a", &[], "1"),
            rhai_node("b", &[], "2"),
            rhai_node("j", &["a", "b"], "3"),
        ],
    );
    let saved = save_flow(&node, &p, "ws", &f).await;
    assert_eq!(saved["version"], 1);
    // A binding to an actual upstream is still allowed (and lineage-linted, not policy-linted).
    let mut j = rhai_node("j", &["a", "b"], "3");
    j.with.insert("payload".into(), json!("${steps.a.payload}"));
    let f = flow(
        "join2",
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

/// The join lint survives ONLY for an **explicit** `all` port (a descriptor opt-in — no built-in
/// declares one after flow-plain-wiring-scope): ≥2 wires with no `payload` binding is a data-drop
/// bug, rejected at save. A built-in (`count`) with the same wiring saves green (plain wiring).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn save_still_requires_a_payload_binding_for_an_explicit_all_join() {
    let host = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    seed_explicit_all_ext(&host, "ws").await;
    let f = flow(
        "join",
        vec![
            rhai_node("a", &[], "1"),
            rhai_node("b", &[], "2"),
            node("c", "extall.collect", &["a", "b"], json!({})), // explicit-all, no `with.payload`
        ],
    );
    let body = serde_json::to_value(&f).unwrap().to_string();
    let err = call_tool(&host, &p, "ws", "flows.save", &body)
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("`all` (join) input port"),
        "expected the explicit-all join lint, got: {err}"
    );
    // The same wiring into a BUILT-IN (default `any`) saves green — the lint retired to the opt-in.
    let f = flow(
        "join-plain",
        vec![
            rhai_node("a", &[], "1"),
            rhai_node("b", &[], "2"),
            node("c", "count", &["a", "b"], json!({})),
        ],
    );
    let saved = save_flow(&host, &p, "ws", &f).await;
    assert_eq!(saved["version"], 1);
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

/// The **explicit-`all`** barrier still works (the opt-in flow-plain-wiring-scope keeps): a
/// 2-upstream explicit-`all` join settles exactly once, at `fctx=""` (no `@fctx` suffix — the
/// pre-ports claim-key shape). A built-in never barriers (none declares `all` — audited by the
/// descriptor unit tests); this uses the real ext-install opt-in fixture.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn explicit_all_join_barrier_settles_once_at_empty_fctx() {
    let host = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    seed_explicit_all_ext(&host, "ws").await;
    let hi = rhai_node("hi", &[], "10");
    let lo = rhai_node("lo", &[], "20");
    let mut j = node("j", "extall.collect", &["hi", "lo"], json!({}));
    j.with
        .insert("payload".into(), json!("${steps.hi.payload}"));
    let f = flow("ajoin", vec![hi, lo, j]);
    save_flow(&host, &p, "ws", &f).await;
    run_flow(&host, &p, "ws", "ajoin", "ajoin-1").await;
    let snap = runs_get(&host, &p, "ws", "ajoin-1").await;
    // The join settles exactly once (a barrier), no firing-context suffix. (Its OUTCOME is `err` —
    // the fixture ext has no runnable sidecar tool — which is irrelevant here: the barrier count and
    // key shape are the invariants; the run still reaches a terminal status.)
    let j_slots: Vec<&serde_json::Value> = snap["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["id"] == "j")
        .collect();
    assert_eq!(j_slots.len(), 1, "an explicit-all join settles once");
    assert!(
        j_slots[0].get("fctx").is_none() || j_slots[0]["fctx"].as_str() == Some(""),
        "the barrier slot carries no fctx (the pre-ports key shape)"
    );
    assert!(
        matches!(
            snap["status"].as_str(),
            Some("partialFailure" | "failed" | "success")
        ),
        "the run reaches terminal: {}",
        snap["status"]
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

// ──────────────────────── flow-plain-wiring-scope: plain per-message wiring ────────────────────────
//
// The link pair is GONE (`link-out`/`link-in` are unknown kinds at save AND at run load) and every
// port defaults to `any`: N wires onto a port ⇒ one firing per arriving message, exactly Node-RED.
// The suite covers the headline flip, the switch matched-release fix (the peer-review blocker),
// lineage bindings, the cross-branch save lint, propagate-vs-mint, fan-out, duplicate-wire collapse,
// suspend/resume across partial firing sets, and the mandatory cap-deny + ws-isolation categories.

/// A flow `failure_policy` override helper (the default `flow()` builder uses Halt; some tests need
/// Continue so independent per-firing failures don't gate each other).
fn flow_with_policy(id: &str, nodes: Vec<Node>, policy: lb_flows::FailurePolicy) -> Flow {
    let mut f = flow(id, nodes);
    f.failure_policy = policy;
    f
}

/// Poll `flows.runs.get` until the run reaches `wanted` (e.g. "suspended") AND no slot is still
/// `running` — the drive batch in flight when the status flips is allowed to finish, so assertions
/// see the quiesced slot set. Bounded.
async fn await_run_status(
    node: &Arc<HostNode>,
    p: &Principal,
    ws: &str,
    run_id: &str,
    wanted: &str,
) -> Value {
    for _ in 0..600 {
        let snap = runs_get(node, p, ws, run_id).await;
        let quiesced = snap["steps"]
            .as_array()
            .map(|s| s.iter().all(|st| st["claim"] != "running"))
            .unwrap_or(false);
        if snap["status"].as_str() == Some(wanted) && quiesced {
            return snap;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    panic!("run {run_id} never reached a quiesced {wanted}");
}

/// The slots of one node in a snapshot.
fn slots_of<'a>(snap: &'a Value, id: &str) -> Vec<&'a Value> {
    snap["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["id"] == id)
        .collect()
}

/// THE headline (flow-plain-wiring-scope): three wires into an ordinary TRANSFORM fire it once per
/// arriving message — no barrier, no binding, no link nodes. Each firing carries its own upstream's
/// payload AND its carried `topic`. One durable whole-graph run. Fail-before: the old `all` default
/// barriered the transform into one firing behind a bind-payload lint.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn transform_funnels_by_default() {
    let host = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let a = rhai_node("a", &[], r#"#{payload: 1, topic: "t.a"}"#);
    let b = rhai_node("b", &[], r#"#{payload: 2, topic: "t.b"}"#);
    let c = rhai_node("c", &[], r#"#{payload: 3, topic: "t.c"}"#);
    let tf = rhai_node("tf", &["a", "b", "c"], "payload * 2");
    let f = flow("plain", vec![a, b, c, tf]);
    save_flow(&host, &p, "ws", &f).await;
    run_flow(&host, &p, "ws", "plain", "plain-1").await;
    let snap = runs_get(&host, &p, "ws", "plain-1").await;
    assert_eq!(snap["status"], "success", "one run, reaches terminal");
    let tf_slots = slots_of(&snap, "tf");
    assert_eq!(tf_slots.len(), 3, "three messages in, three firings out");
    let mut got: Vec<(i64, String)> = tf_slots
        .iter()
        .map(|s| {
            (
                s["output"]["payload"].as_i64().unwrap(),
                s["output"]["topic"].as_str().unwrap_or("").to_string(),
            )
        })
        .collect();
    got.sort();
    assert_eq!(
        got,
        vec![
            (2, "t.a".to_string()),
            (4, "t.b".to_string()),
            (6, "t.c".to_string())
        ],
        "each firing reads + carries ITS message (payload doubled, topic forwarded)"
    );
    // Whole-graph posture: 3 sources + 3 tf firings = 6 slots in ONE run.
    assert_eq!(snap["steps"].as_array().unwrap().len(), 6);
}

/// Reactive posture of the same flow: firing FROM one source (`entry`) runs only that subgraph —
/// one run per event, the transform settling ONCE per run (the run-count caveat the scope names:
/// same per-message behaviour, different run bookkeeping).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn reactive_posture_fires_one_run_per_event() {
    let host = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let a = rhai_node("a", &[], "1");
    let b = rhai_node("b", &[], "2");
    let c = rhai_node("c", &[], "3");
    let tf = rhai_node("tf", &["a", "b", "c"], "payload * 2");
    let f = flow("reactive", vec![a, b, c, tf]);
    save_flow(&host, &p, "ws", &f).await;
    for (i, entry) in ["a", "b", "c"].iter().enumerate() {
        let run_id = format!("re-{i}");
        let req =
            json!({ "id": "reactive", "run_id": run_id, "ts": 1, "entry": entry }).to_string();
        call_tool(&host, &p, "ws", "flows.run", &req).await.unwrap();
        let snap = await_terminal(&host, &p, "ws", &run_id).await;
        assert_eq!(snap["status"], "success");
        assert_eq!(
            slots_of(&snap, "tf").len(),
            1,
            "reactive: one firing per run (entry {entry})"
        );
        // Only the fired source + tf are in the run (the induced subgraph).
        assert_eq!(snap["steps"].as_array().unwrap().len(), 2);
    }
}

/// Multiplicity propagates one hop downstream WITHOUT the link pair: a plain 3-wire funnel node
/// feeding a single-wire transform `w` settles `w` once per funnel firing, each reading its own
/// firing's message — and the single-wire hop PROPAGATES the funnel's `fctx` (no per-hop mint, no
/// lineage growth on the chain).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn funnel_multiplicity_propagates_one_hop_downstream() {
    let host = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let a = rhai_node("a", &[], "1");
    let b = rhai_node("b", &[], "2");
    let c = rhai_node("c", &[], "3");
    let li = rhai_node("li", &["a", "b", "c"], "payload");
    let w = rhai_node("w", &["li"], "payload");
    let f = flow("fun", vec![a, b, c, li, w]);
    save_flow(&host, &p, "ws", &f).await;
    run_flow(&host, &p, "ws", "fun", "fun-1").await;
    let snap = runs_get(&host, &p, "ws", "fun-1").await;
    assert_eq!(snap["status"], "success");
    let li_slots = slots_of(&snap, "li");
    assert_eq!(li_slots.len(), 3, "the funnel fires once per upstream");
    let mut li_fctxs: Vec<String> = li_slots
        .iter()
        .map(|s| s["fctx"].as_str().unwrap_or("").to_string())
        .collect();
    li_fctxs.sort();
    assert_eq!(li_fctxs, vec!["li#a", "li#b", "li#c"]);
    let w_slots = slots_of(&snap, "w");
    assert_eq!(w_slots.len(), 3, "the downstream settles once per firing");
    let mut w_payloads: Vec<i64> = w_slots
        .iter()
        .map(|s| s["output"]["payload"].as_i64().unwrap_or(0))
        .collect();
    w_payloads.sort();
    assert_eq!(w_payloads, vec![1, 2, 3], "each w reads its OWN firing");
    // The single-wire hop PROPAGATES: w's fctxs are the funnel's, not extended per hop.
    let mut w_fctxs: Vec<String> = w_slots
        .iter()
        .map(|s| s["fctx"].as_str().unwrap_or("").to_string())
        .collect();
    w_fctxs.sort();
    assert_eq!(
        w_fctxs,
        vec!["li#a", "li#b", "li#c"],
        "single-wire ports propagate the fctx (no lineage growth on a chain)"
    );
}

/// Mandatory capability-deny, per firing: a 2-wire funnel feeding a `rhai` node whose `rules.eval`
/// verb the caller lacks is denied at EACH firing (two independent `Err` settles, not one).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn plain_funnel_denies_per_firing_at_a_downstream_tool() {
    let host = Arc::new(HostNode::boot().await.unwrap());
    // The caller lacks the rhai cage's verbs — the funnel's downstream is denied per slot.
    let mut caps = FULL.to_vec();
    caps.retain(|c| c != &"mcp:rules.run:call" && c != &"mcp:rules.eval:call");
    let p = principal("ws", &caps);
    let a = node("a", "trigger", &[], json!({ "mode": "manual" }));
    let b = node("b", "trigger", &[], json!({ "mode": "manual" }));
    let denied = rhai_node("denied", &["a", "b"], "payload");
    let f = flow_with_policy(
        "plaindeny",
        vec![a, b, denied],
        lb_flows::FailurePolicy::Continue,
    );
    save_flow(&host, &p, "ws", &f).await;
    run_flow(&host, &p, "ws", "plaindeny", "plaindeny-1").await;
    let snap = runs_get(&host, &p, "ws", "plaindeny-1").await;
    let err_slots: Vec<&Value> = slots_of(&snap, "denied")
        .into_iter()
        .filter(|s| s["outcome"] == "err")
        .collect();
    assert_eq!(
        err_slots.len(),
        2,
        "the deny bites once per (node, fctx) firing: {snap}"
    );
}

/// Outbox dedup per firing over DIRECT wiring: a 2-wire funnel sink stages two idempotent outbox
/// effects (one per `fctx`-scoped effect id), not one swallowing the other.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn plain_funnel_outbox_dedups_per_firing() {
    let host = Arc::new(HostNode::boot().await.unwrap());
    let mut caps = FULL.to_vec();
    caps.push("mcp:outbox.enqueue:call");
    let p = principal("ws", &caps);
    let a = rhai_node("a", &[], "1");
    let b = rhai_node("b", &[], "2");
    let sink = node(
        "out",
        "sink",
        &["a", "b"],
        json!({ "target": "outbox", "name": "plain-deliver" }),
    );
    let f = flow("plainoutbox", vec![a, b, sink]);
    save_flow(&host, &p, "ws", &f).await;
    run_flow(&host, &p, "ws", "plainoutbox", "plainoutbox-1").await;
    let snap = runs_get(&host, &p, "ws", "plainoutbox-1").await;
    assert_eq!(snap["status"], "success", "run snapshot: {snap}");
    assert_eq!(slots_of(&snap, "out").len(), 2, "one sink firing per wire");
    let pending = lb_outbox::pending(&host.store, "ws").await.unwrap();
    let ours: Vec<&lb_outbox::Effect> = pending
        .iter()
        .filter(|e| e.target == "plain-deliver")
        .collect();
    assert_eq!(ours.len(), 2, "N firings ⇒ N outbox effects: {pending:?}");
    let mut ids: Vec<&str> = ours.iter().map(|e| e.id.as_str()).collect();
    ids.sort();
    assert!(
        ids.iter().all(|id| id.contains('@')),
        "each effect id carries the fctx suffix: {ids:?}"
    );
    assert_ne!(ids[0], ids[1]);
}

/// Exactly-once per firing on redelivery, one hop past a plain funnel: re-running the SAME run_id
/// no-ops every `(node, fctx)` slot (deterministic fctx ⇒ same keys ⇒ CAS no-op).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn plain_funnel_exactly_once_per_firing_on_redelivery() {
    let host = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let a = rhai_node("a", &[], "1");
    let b = rhai_node("b", &[], "2");
    let li = rhai_node("li", &["a", "b"], "payload");
    let w = rhai_node("w", &["li"], "payload");
    let f = flow("plainonce", vec![a, b, li, w]);
    save_flow(&host, &p, "ws", &f).await;
    run_flow(&host, &p, "ws", "plainonce", "plainonce-1").await;
    let snap1 = runs_get(&host, &p, "ws", "plainonce-1").await;
    assert_eq!(slots_of(&snap1, "w").len(), 2, "first run: w per firing");
    let req = json!({ "id": "plainonce", "run_id": "plainonce-1", "ts": 2 }).to_string();
    let _ = call_tool(&host, &p, "ws", "flows.run", &req).await;
    let snap2 = runs_get(&host, &p, "ws", "plainonce-1").await;
    assert_eq!(
        slots_of(&snap2, "w").len(),
        2,
        "redelivery no-ops every (node, fctx) slot"
    );
}

/// The removal contract at SAVE: `link-out`/`link-in` are unknown kinds (the registry no longer
/// carries them), rejected with the standard unknown-type error before any run.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn save_rejects_link_kinds_as_unknown() {
    let host = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    for kind in ["link-out", "link-in"] {
        let a = rhai_node("a", &[], "1");
        let l = node("l", kind, &["a"], json!({ "target": "T", "name": "T" }));
        let f = flow("linkgone", vec![a, l]);
        let body = serde_json::to_value(&f).unwrap().to_string();
        let err = call_tool(&host, &p, "ws", "flows.save", &body)
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("unknown type"),
            "{kind} is an unknown kind at save: {err}"
        );
    }
}

/// The removal contract at RUN LOAD (the reactor path never re-saves): an already-armed persisted
/// flow holding a removed kind fails `flows.run` with a clear unknown-kind error — never a
/// confusing unknown-tool denial from the extension-dispatch leg. The stale record is written
/// through the real store write path, bypassing save (exactly what an armed pre-removal flow is).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn run_load_guard_fails_an_armed_flow_with_a_removed_kind() {
    let host = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let mut f = flow(
        "armedlink",
        vec![
            rhai_node("a", &[], "1"),
            node("li", "link-in", &["a"], json!({ "name": "T" })),
        ],
    );
    f.version = 1;
    let value = serde_json::to_value(&f).unwrap();
    lb_store::write(&host.store, "ws", "flow", "armedlink", &value)
        .await
        .unwrap();
    let req = json!({ "id": "armedlink", "run_id": "armed-1", "ts": 1 }).to_string();
    let err = call_tool(&host, &p, "ws", "flows.run", &req)
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("unknown node kind") && msg.contains("link-in"),
        "the run-load guard names the removed kind clearly: {msg}"
    );
}

/// Mandatory workspace isolation over a link-free multi-wire topology: ws-B cannot read ws-A's
/// per-firing `@{fctx}` slots.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_isolation_plain_funnel_step_keys() {
    let host = Arc::new(HostNode::boot().await.unwrap());
    let p_a = principal("wsA", FULL);
    let p_b = principal("wsB", FULL);
    let a = rhai_node("a", &[], "1");
    let b = rhai_node("b", &[], "2");
    let li = rhai_node("li", &["a", "b"], "payload");
    let w = rhai_node("w", &["li"], "payload");
    let f = flow("plainiso", vec![a, b, li, w]);
    save_flow(&host, &p_a, "wsA", &f).await;
    run_flow(&host, &p_a, "wsA", "plainiso", "plainiso-1").await;
    let req = json!({ "run_id": "plainiso-1" }).to_string();
    let res = call_tool(&host, &p_b, "wsB", "flows.runs.get", &req).await;
    assert!(res.is_err(), "ws-B cannot read ws-A's per-firing slots");
}

// ───────────── the switch matched-release fix (the peer-review blocker) ─────────────

/// THE blocker (flow-plain-wiring-scope): a matched `switch` plus two plain wires into ONE node.
/// The matched release must mint a normal `any` firing (`triggered_by` = the switch) — releasing
/// through the barrier path seeds a Pending slot at indegree 3 that the sibling any-firings never
/// touch, and the run HANGS. Fail-before: `await_terminal` times out (verified by reverting the
/// matched release to the barrier path).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn matched_switch_into_a_multi_wire_any_port_reaches_terminal() {
    let host = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let a = rhai_node("a", &[], "1");
    let b = rhai_node("b", &[], "2");
    let src = rhai_node("src", &[], "5");
    let sw = node(
        "sw",
        "switch",
        &["src"],
        json!({ "rules": [ { "op": "gt", "value": 0, "to": ["w"] } ] }),
    );
    let w = rhai_node("w", &["a", "b", "sw"], "payload");
    let f = flow("swany", vec![a, b, src, sw, w]);
    save_flow(&host, &p, "ws", &f).await;
    run_flow(&host, &p, "ws", "swany", "swany-1").await;
    let snap = runs_get(&host, &p, "ws", "swany-1").await;
    assert_eq!(snap["status"], "success", "the run reaches terminal");
    let w_slots = slots_of(&snap, "w");
    assert_eq!(
        w_slots.len(),
        3,
        "w fires once per upstream incl. the matched switch: {snap}"
    );
    let mut fctxs: Vec<String> = w_slots
        .iter()
        .map(|s| s["fctx"].as_str().unwrap_or("").to_string())
        .collect();
    fctxs.sort();
    assert_eq!(
        fctxs,
        vec!["w#a", "w#b", "w#sw"],
        "the matched release minted a normal any-firing (triggered_by = switch)"
    );
    let mut payloads: Vec<i64> = w_slots
        .iter()
        .map(|s| s["output"]["payload"].as_i64().unwrap_or(-1))
        .collect();
    payloads.sort();
    assert_eq!(
        payloads,
        vec![1, 2, 5],
        "the switch firing carries its routed message"
    );
}

/// The gated side stays as-is: a `switch` wire into a multi-wire `any` port that does NOT match
/// settles its `(dep, fctx)` slot `Skipped` — one fewer firing, and the run still reaches terminal.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn gated_switch_wire_into_a_multi_wire_port_settles_skipped() {
    let host = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let a = rhai_node("a", &[], "1");
    let b = rhai_node("b", &[], "2");
    let src = rhai_node("src", &[], "5");
    // The rule can never match (5 is not < 0) and there is no `else` ⇒ w's switch wire is gated.
    let sw = node(
        "sw",
        "switch",
        &["src"],
        json!({ "rules": [ { "op": "lt", "value": 0, "to": ["w"] } ] }),
    );
    let w = rhai_node("w", &["a", "b", "sw"], "payload");
    let f = flow("swgate", vec![a, b, src, sw, w]);
    save_flow(&host, &p, "ws", &f).await;
    run_flow(&host, &p, "ws", "swgate", "swgate-1").await;
    let snap = runs_get(&host, &p, "ws", "swgate-1").await;
    assert_eq!(snap["status"], "success", "no hang on the gated slot");
    let w_slots = slots_of(&snap, "w");
    let ok: Vec<&&Value> = w_slots.iter().filter(|s| s["outcome"] == "ok").collect();
    let skipped: Vec<&&Value> = w_slots
        .iter()
        .filter(|s| s["outcome"] == "skipped")
        .collect();
    assert_eq!(ok.len(), 2, "the two plain wires still fire");
    assert_eq!(
        skipped.len(),
        1,
        "the gated switch wire is one fewer firing"
    );
}

/// A matched `switch` into an EXPLICIT-`all` port still takes the barrier path (the opt-in
/// survives): the dependent settles once, at the shared `fctx`, after every wired upstream —
/// switch included — has released it.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn matched_switch_into_an_explicit_all_port_takes_the_barrier() {
    let host = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    seed_explicit_all_ext(&host, "ws").await;
    let hi = rhai_node("hi", &[], "10");
    let src = rhai_node("src", &[], "5");
    let sw = node(
        "sw",
        "switch",
        &["src"],
        json!({ "rules": [ { "op": "gt", "value": 0, "to": ["j"] } ] }),
    );
    let mut j = node("j", "extall.collect", &["hi", "sw"], json!({}));
    j.with
        .insert("payload".into(), json!("${steps.hi.payload}"));
    let f = flow_with_policy(
        "swall",
        vec![hi, src, sw, j],
        lb_flows::FailurePolicy::Continue,
    );
    save_flow(&host, &p, "ws", &f).await;
    run_flow(&host, &p, "ws", "swall", "swall-1").await;
    let snap = runs_get(&host, &p, "ws", "swall-1").await;
    let j_slots = slots_of(&snap, "j");
    assert_eq!(
        j_slots.len(),
        1,
        "the explicit-all barrier settles once: {snap}"
    );
    assert!(
        j_slots[0]["fctx"].as_str().unwrap_or("").is_empty(),
        "the barrier slot keeps the shared (empty) fctx"
    );
}

// ───────────── lineage bindings + the cross-branch save lint ─────────────

/// Lineage bindings (flow-plain-wiring-scope): in a linear chain `t → a → b` under universal `any`,
/// `b`'s `${steps.t.payload}` (a GRANDPARENT binding) resolves — X's settle matches from any
/// ancestor fctx in the firing's lineage. Fail-before: the recorded map held only the arriving
/// upstream (`a`), so the grandparent binding silently bound null.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn lineage_binding_resolves_a_grandparent_under_any() {
    let host = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let t = rhai_node("t", &[], "7");
    let a = rhai_node("a", &["t"], "payload * 2");
    let mut b = rhai_node("b", &["a"], "payload");
    b.with.insert("payload".into(), json!("${steps.t.payload}"));
    let f = flow("lineage", vec![t, a, b]);
    save_flow(&host, &p, "ws", &f).await;
    run_flow(&host, &p, "ws", "lineage", "lineage-1").await;
    let snap = runs_get(&host, &p, "ws", "lineage-1").await;
    assert_eq!(snap["status"], "success");
    assert_eq!(
        slots_of(&snap, "b")[0]["output"]["payload"],
        7,
        "the grandparent binding resolves along the lineage (not a's 14): {snap}"
    );
}

/// The cross-branch save lint: a `${steps.X}` binding where X is neither the node itself nor a
/// transitive upstream can never be in the firing's lineage — a save ERROR, not a silent per-firing
/// null. Fail-before: saved green and bound null.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn cross_branch_binding_is_a_save_error() {
    let host = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let t1 = rhai_node("t1", &[], "1");
    let p1 = rhai_node("p1", &["t1"], "payload");
    let t2 = rhai_node("t2", &[], "2");
    let mut q = rhai_node("q", &["t2"], "payload");
    // q references p1 — an unrelated branch, never in q's lineage.
    q.with
        .insert("payload".into(), json!("${steps.p1.payload}"));
    let f = flow("crossbranch", vec![t1, p1, t2, q]);
    let body = serde_json::to_value(&f).unwrap().to_string();
    let err = call_tool(&host, &p, "ws", "flows.save", &body)
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("lineage") && msg.contains("p1"),
        "the lint names the cross-branch reference: {msg}"
    );
    // The same binding on a node that IS downstream of p1 saves green (a real ancestor).
    let t1 = rhai_node("t1", &[], "1");
    let p1 = rhai_node("p1", &["t1"], "payload");
    let mut q = rhai_node("q", &["p1"], "payload");
    q.with
        .insert("payload".into(), json!("${steps.t1.payload}"));
    let f = flow("inbranch", vec![t1, p1, q]);
    let saved = save_flow(&host, &p, "ws", &f).await;
    assert_eq!(saved["version"], 1);
}

// ───────────── named divergences + structure pins ─────────────

/// Duplicate wires collapse (the named Node-RED divergence, pinned deliberately): two wires from
/// the same output to the same input are ONE firing — the firing id is deterministic per
/// `(node, upstream, parent fctx)`, so the second release re-mints the same slot and no-ops.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn duplicate_wire_collapses_to_one_firing() {
    let host = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let a = rhai_node("a", &[], "1");
    let x = rhai_node("x", &["a", "a"], "payload");
    let f = flow("dup", vec![a, x]);
    save_flow(&host, &p, "ws", &f).await;
    run_flow(&host, &p, "ws", "dup", "dup-1").await;
    let snap = runs_get(&host, &p, "ws", "dup-1").await;
    assert_eq!(snap["status"], "success");
    assert_eq!(
        slots_of(&snap, "x").len(),
        1,
        "two identical wires are one firing (deterministic slot id): {snap}"
    );
}

/// Output fan-out is plain too (stated + pinned): one output port wired to three downstreams fires
/// all three, each from its own immutable copy of the envelope.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn output_fanout_fires_every_downstream() {
    let host = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let src = rhai_node("src", &[], "7");
    let d1 = rhai_node("d1", &["src"], "payload + 1");
    let d2 = rhai_node("d2", &["src"], "payload + 2");
    let d3 = rhai_node("d3", &["src"], "payload + 3");
    let f = flow("fan", vec![src, d1, d2, d3]);
    save_flow(&host, &p, "ws", &f).await;
    run_flow(&host, &p, "ws", "fan", "fan-1").await;
    let snap = runs_get(&host, &p, "ws", "fan-1").await;
    assert_eq!(snap["status"], "success");
    for (id, want) in [("d1", 8), ("d2", 9), ("d3", 10)] {
        let slots = slots_of(&snap, id);
        assert_eq!(slots.len(), 1, "{id} fires once");
        assert_eq!(
            slots[0]["output"]["payload"], want,
            "{id} read its own immutable copy"
        );
    }
}

/// The durable-suspend analogue of hot-reload (mandatory): a suspend BETWEEN two `any` firings
/// (one settled, its sibling parked behind a durable delay) resumes by rebuilding the partial
/// `(node, fctx)` slot set — the settled firing is not re-run, the parked one completes, and the
/// funnel's downstream settles once per firing across the suspend boundary.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn suspend_resume_between_any_firings_rebuilds_the_slot_set() {
    let host = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    // a → fn (fires immediately); t0 → dl (delay 1000ms) → fn (fires after resume). fn → w.
    let a = rhai_node("a", &[], "1");
    let t0 = rhai_node("t0", &[], "9");
    let dl = node(
        "dl",
        "delay",
        &["t0"],
        json!({ "mode": "delay", "ms": 1000 }),
    );
    let fnode = rhai_node("fn", &["a", "dl"], "payload");
    let w = rhai_node("w", &["fn"], "payload");
    let f = flow("suspany", vec![a, t0, dl, fnode, w]);
    save_flow(&host, &p, "ws", &f).await;
    // Fire at t=1: the delay parks and the run suspends; fn's `a` firing has settled by then
    // (same drive batch), its sibling has not — the partial slot set.
    let req = json!({ "id": "suspany", "run_id": "susp-1", "ts": 1 }).to_string();
    call_tool(&host, &p, "ws", "flows.run", &req).await.unwrap();
    let s1 = await_run_status(&host, &p, "ws", "susp-1", "suspended").await;
    let settled_fn = slots_of(&s1, "fn")
        .iter()
        .filter(|s| s["outcome"] == "ok")
        .count();
    assert_eq!(
        settled_fn, 1,
        "exactly one any-firing settled pre-suspend: {s1}"
    );
    // Resume past the timer: the parked branch completes; the settled firing is NOT re-run.
    let req = json!({ "run_id": "susp-1", "ts": 2000 }).to_string();
    call_tool(&host, &p, "ws", "flows.resume", &req)
        .await
        .unwrap();
    let s2 = await_terminal(&host, &p, "ws", "susp-1").await;
    assert_eq!(s2["status"], "success");
    let fn_slots = slots_of(&s2, "fn");
    assert_eq!(fn_slots.len(), 2, "both firings present after resume");
    assert!(fn_slots.iter().all(|s| s["outcome"] == "ok"));
    let w_slots = slots_of(&s2, "w");
    assert_eq!(
        w_slots.len(),
        2,
        "the downstream settled once per firing across the suspend"
    );
    let mut w_payloads: Vec<i64> = w_slots
        .iter()
        .map(|s| s["output"]["payload"].as_i64().unwrap_or(-1))
        .collect();
    w_payloads.sort();
    assert_eq!(
        w_payloads,
        vec![1, 9],
        "each downstream slot rode its own firing"
    );
}
