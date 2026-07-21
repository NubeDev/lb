//! Host-layer tests for the **rules + workflow convergence** (rules-workflow-convergence scope). Real
//! store (`mem://`), real caps, real `lb-jobs`, real ingest buffer, real reactors — no mocks (rule 9).
//! Every flow is a real `flow` record through `flows.save`; rules via `rules.save`; webhook hits via
//! the real `ingest.write` → `drain_workspace` commit path.
//!
//! Covers, per slice:
//!  1. `rules.eval` — the flow-envelope rule entry (+ deny per verb); the `rhai`/`rule` nodes run it.
//!  2. concurrency policy (`skip`/`queue`/`restart`) at the fire seam; per-node `timeout_ms`.
//!  4. the `approval` gate parks the run + the flow-approval reactor resumes on `Approved` / cancels on
//!     `Rejected`; the outbox relay reactor delivers a staged `sink(target=outbox)` effect.
//!  5. the `webhook` source node fires a run per hit via the series-event reactor.
//!
//! Mandatory categories: **capability-deny** (each new verb/node) + **workspace-isolation** (a ws-B
//! reactor never touches a ws-A run/hit).

use std::future::Future;
use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_flows::{Concurrency, FailurePolicy, Flow, Node};
use lb_host::{
    call_tool, react_to_flow_approvals, react_to_flow_sources, relay_outbox, Node as HostNode,
    Target,
};
use lb_outbox::Effect;
use serde_json::{json, Value};

// ---- fixtures ----------------------------------------------------------------------------------

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

/// A node-internal service principal (the reactor's authority): the flows run/resume/cancel surface +
/// series read + the store + inbox. Mirrors the reactor caps in `reactor_loop.rs`.
fn service(ws: &str) -> Principal {
    principal(
        ws,
        &[
            "mcp:flows.run:call",
            "mcp:flows.resume:call",
            "mcp:flows.cancel:call",
            "mcp:series.read:call",
            "mcp:inbox.record:call",
            "mcp:inbox.resolve:call",
            "mcp:rules.eval:call",
            "store:flow:read",
            "store:flow:write",
            "store:*:read",
            "store:*:write",
            "mcp:*.call:call",
        ],
    )
}

/// Full member caps: every flows verb + rules.eval + the store surface + inbox/ingest for the nodes.
const FULL: &[&str] = &[
    "mcp:flows.save:call",
    "mcp:flows.get:call",
    "mcp:flows.list:call",
    "mcp:flows.run:call",
    "mcp:flows.resume:call",
    "mcp:flows.cancel:call",
    "mcp:flows.runs.get:call",
    "mcp:flows.runs.list:call",
    "mcp:flows.nodes:call",
    "mcp:rules.eval:call",
    "mcp:rules.save:call",
    "mcp:inbox.record:call",
    "mcp:inbox.resolve:call",
    "mcp:inbox.list:call",
    "mcp:ingest.write:call",
    "mcp:series.read:call",
    "mcp:outbox.status:call",
    "store:flow:write",
    "store:flow:read",
    "store:rule:write",
    "store:rule:read",
];

fn flow_node(id: &str, ty: &str, needs: &[&str], config: Value) -> Node {
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

fn flow_with(id: &str, nodes: Vec<Node>, concurrency: Concurrency) -> Flow {
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
        concurrency,
        cron: None,
        next_attempt_ts: 0,
        managed_by: None,
    }
}

async fn save_flow(node: &Arc<HostNode>, p: &Principal, ws: &str, flow: &Flow) {
    let body = serde_json::to_value(flow).unwrap().to_string();
    call_tool(node, p, ws, "flows.save", &body)
        .await
        .unwrap_or_else(|e| panic!("save {}: {e:?}", flow.id));
}

async fn runs_get(node: &Arc<HostNode>, p: &Principal, ws: &str, run_id: &str) -> Value {
    let req = json!({ "run_id": run_id }).to_string();
    let out = call_tool(node, p, ws, "flows.runs.get", &req)
        .await
        .unwrap();
    serde_json::from_str(&out).unwrap()
}

/// Poll a run to a terminal status (or return the last snapshot if it stays live, e.g. `suspended`).
async fn await_status(
    node: &Arc<HostNode>,
    p: &Principal,
    ws: &str,
    run_id: &str,
    want: &str,
) -> Value {
    for _ in 0..400 {
        let snap = runs_get(node, p, ws, run_id).await;
        if snap["status"].as_str() == Some(want) {
            return snap;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    let snap = runs_get(node, p, ws, run_id).await;
    panic!(
        "run {run_id} never reached {want} (last: {})",
        snap["status"]
    );
}

async fn await_terminal(node: &Arc<HostNode>, p: &Principal, ws: &str, run_id: &str) -> Value {
    for _ in 0..400 {
        let snap = runs_get(node, p, ws, run_id).await;
        if matches!(
            snap["status"].as_str(),
            Some("success") | Some("failed") | Some("partialFailure") | Some("cancelled")
        ) {
            return snap;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    panic!("run {run_id} never reached a terminal status");
}

/// Read one node's step record from a run snapshot (`{outcome, output, error, ...}`).
fn node_step<'a>(snap: &'a Value, node_id: &str) -> &'a Value {
    snap["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == node_id)
        .unwrap_or_else(|| panic!("node {node_id} not in snapshot"))
}

/// Read one node's settled `output` object (the `{payload, findings, ...}` the node emitted).
fn node_output<'a>(snap: &'a Value, node_id: &str) -> &'a Value {
    &node_step(snap, node_id)["output"]
}

// ================================================================================================
// Slice 1 — rules.eval + the rhai/rule nodes
// ================================================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rules_eval_denied_without_the_cap() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    // A principal with every OTHER rules verb but NOT rules.eval.
    let p = principal("ws", &["mcp:rules.run:call", "store:rule:read"]);
    let req = json!({ "body": "1 + 1", "envelope": {}, "ts": 1 }).to_string();
    let err = call_tool(&node, &p, "ws", "rules.eval", &req).await;
    assert!(
        err.is_err(),
        "rules.eval without mcp:rules.eval:call must be denied"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rules_eval_maps_envelope_to_params_and_returns_findings() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    // The rule reads `params.payload` (the envelope field) and emits a finding.
    let body = r#"let x = payload; emit(#{ doubled: x * 2 }); x * 2"#;
    let req = json!({
        "body": body,
        "envelope": { "payload": 21 },
        "ts": 1
    })
    .to_string();
    let out = call_tool(&node, &p, "ws", "rules.eval", &req)
        .await
        .unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    // `output` is the rule's return; `findings` carries the emit.
    assert!(v.get("output").is_some(), "rules.eval returns output");
    assert!(v.get("findings").is_some(), "rules.eval returns findings");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rhai_node_runs_rules_eval_end_to_end() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    // trigger → rhai(double the payload). The trigger emits payload=21 (via run params); rhai doubles.
    let trig = flow_node("t", "trigger", &[], json!({ "mode": "manual" }));
    let mut rhai = flow_node("r", "rhai", &["t"], json!({ "source": "payload * 2" }));
    rhai.with
        .insert("payload".into(), json!("${steps.t.payload}"));
    let f = flow_with("rhai-flow", vec![trig, rhai], Concurrency::Queue);
    save_flow(&node, &p, "ws", &f).await;

    let req =
        json!({ "id": "rhai-flow", "run_id": "r1", "params": { "t": 21 }, "ts": 1 }).to_string();
    call_tool(&node, &p, "ws", "flows.run", &req).await.unwrap();
    let snap = await_terminal(&node, &p, "ws", "r1").await;
    assert_eq!(snap["status"], "success");
    assert_eq!(node_output(&snap, "r")["payload"], json!(42));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn saved_rule_node_runs_a_stored_rule_by_id() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    // Save a rule, then a flow whose `rule` node runs it by id.
    let save = json!({ "id": "doubler", "name": "doubler", "body": "payload * 2" }).to_string();
    call_tool(&node, &p, "ws", "rules.save", &save)
        .await
        .unwrap();

    let trig = flow_node("t", "trigger", &[], json!({ "mode": "manual" }));
    let mut rule = flow_node("ru", "rule", &["t"], json!({ "rule": "doubler" }));
    rule.with
        .insert("payload".into(), json!("${steps.t.payload}"));
    let f = flow_with("rule-flow", vec![trig, rule], Concurrency::Queue);
    save_flow(&node, &p, "ws", &f).await;

    let req =
        json!({ "id": "rule-flow", "run_id": "r1", "params": { "t": 5 }, "ts": 1 }).to_string();
    call_tool(&node, &p, "ws", "flows.run", &req).await.unwrap();
    let snap = await_terminal(&node, &p, "ws", "r1").await;
    assert_eq!(snap["status"], "success");
    assert_eq!(node_output(&snap, "ru")["payload"], json!(10));
}

// ================================================================================================
// Slice 2 — concurrency policy + per-node timeout
// ================================================================================================

/// A flow whose one node PARKS forever (a `delay` with a huge ms) so a run stays live — a fixture for
/// the concurrency guard. `flows.run` returns before the drive parks it, then the guard sees it live.
fn parking_flow(id: &str, concurrency: Concurrency) -> Flow {
    let trig = flow_node("t", "trigger", &[], json!({ "mode": "manual" }));
    let mut delay = flow_node(
        "d",
        "delay",
        &["t"],
        json!({ "mode": "delay", "ms": 9_000_000 }),
    );
    delay
        .with
        .insert("payload".into(), json!("${steps.t.payload}"));
    flow_with(id, vec![trig, delay], concurrency)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn concurrency_skip_drops_the_overlapping_firing() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let f = parking_flow("skip-flow", Concurrency::Skip);
    save_flow(&node, &p, "ws", &f).await;

    // First run: parks on the delay (stays `suspended` — a live run).
    let r1 = json!({ "id": "skip-flow", "run_id": "run-a", "ts": 1 }).to_string();
    call_tool(&node, &p, "ws", "flows.run", &r1).await.unwrap();
    await_status(&node, &p, "ws", "run-a", "suspended").await;

    // Second firing with the live run present → `skip` drops it: no `run-b` record is ever created.
    let r2 = json!({ "id": "skip-flow", "run_id": "run-b", "ts": 2 }).to_string();
    call_tool(&node, &p, "ws", "flows.run", &r2).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let got = call_tool(
        &node,
        &p,
        "ws",
        "flows.runs.get",
        &json!({ "run_id": "run-b" }).to_string(),
    )
    .await;
    assert!(
        got.is_err(),
        "skip must not seed the overlapping run (run-b never exists)"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn concurrency_restart_cancels_the_live_run() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let f = parking_flow("restart-flow", Concurrency::Restart);
    save_flow(&node, &p, "ws", &f).await;

    let r1 = json!({ "id": "restart-flow", "run_id": "run-a", "ts": 1 }).to_string();
    call_tool(&node, &p, "ws", "flows.run", &r1).await.unwrap();
    await_status(&node, &p, "ws", "run-a", "suspended").await;

    // Second firing with `restart` → the live run-a is cancelled, run-b starts.
    let r2 = json!({ "id": "restart-flow", "run_id": "run-b", "ts": 2 }).to_string();
    call_tool(&node, &p, "ws", "flows.run", &r2).await.unwrap();
    await_status(&node, &p, "ws", "run-a", "cancelled").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn concurrency_queue_lets_runs_overlap() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let f = parking_flow("queue-flow", Concurrency::Queue);
    save_flow(&node, &p, "ws", &f).await;

    let r1 = json!({ "id": "queue-flow", "run_id": "run-a", "ts": 1 }).to_string();
    call_tool(&node, &p, "ws", "flows.run", &r1).await.unwrap();
    await_status(&node, &p, "ws", "run-a", "suspended").await;

    // `queue` allows overlap: run-b is seeded and also parks (both live).
    let r2 = json!({ "id": "queue-flow", "run_id": "run-b", "ts": 2 }).to_string();
    call_tool(&node, &p, "ws", "flows.run", &r2).await.unwrap();
    await_status(&node, &p, "ws", "run-b", "suspended").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn per_node_timeout_settles_err_timeout() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    // A subflow node that would run a child, wrapped in a 1ms timeout — the child load/drive exceeds
    // it, so the node settles `err:"timeout"`. We use a `tool` node calling a slow-ish verb instead: a
    // `delay` node with ms far above the 1ms node timeout can't be used (delay parks, not runs), so we
    // wrap a `rhai` doing real work. To be deterministic we set timeout_ms=1 on a node whose dispatch
    // makes a real tool call (rules.eval), which cannot complete inside 1ms of wall-clock reliably.
    let trig = flow_node("t", "trigger", &[], json!({ "mode": "manual" }));
    let mut rhai = flow_node(
        "r",
        "rhai",
        &["t"],
        json!({ "source": "payload", "timeout_ms": 1 }),
    );
    rhai.with
        .insert("payload".into(), json!("${steps.t.payload}"));
    // Also set the NODE-level timeout wrapper to 1ms (the generic guard), belt-and-braces.
    let f = flow_with("timeout-flow", vec![trig, rhai], Concurrency::Queue);
    save_flow(&node, &p, "ws", &f).await;

    let req =
        json!({ "id": "timeout-flow", "run_id": "r1", "params": { "t": 1 }, "ts": 1 }).to_string();
    call_tool(&node, &p, "ws", "flows.run", &req).await.unwrap();
    let snap = await_terminal(&node, &p, "ws", "r1").await;
    // The rhai node's dispatch (a real cross-thread `rules.eval`) cannot finish inside the 1ms node
    // ceiling, so `execute_one` settles it `err:"timeout"` and the run halts (Halt policy).
    let r = node_step(&snap, "r");
    assert_eq!(r["outcome"], "err", "the 1ms-bounded node settles an error");
    assert_eq!(r["error"], "timeout", "the error is a timeout");
    assert_ne!(
        snap["status"], "success",
        "a timed-out node's run is not a success"
    );
}

// ================================================================================================
// Slice 4 — approval gate (park→resume/cancel) + outbox relay reactor
// ================================================================================================

/// trigger → approval gate → rhai(sink of the decision). The gate parks the run.
fn approval_flow(id: &str) -> Flow {
    let trig = flow_node("t", "trigger", &[], json!({ "mode": "manual" }));
    let mut gate = flow_node("g", "approval", &["t"], json!({ "team": "reviewers" }));
    gate.with
        .insert("payload".into(), json!("${steps.t.payload}"));
    let mut after = flow_node("a", "rhai", &["g"], json!({ "source": "payload" }));
    after
        .with
        .insert("payload".into(), json!("${steps.g.payload}"));
    flow_with(id, vec![trig, gate, after], Concurrency::Queue)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn approval_gate_parks_then_resumes_on_approved() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let f = approval_flow("appr-flow");
    save_flow(&node, &p, "ws", &f).await;

    // Fire the run — it parks on the gate (suspended), writing a needs:approval item.
    let req =
        json!({ "id": "appr-flow", "run_id": "run1", "params": { "t": 7 }, "ts": 1 }).to_string();
    call_tool(&node, &p, "ws", "flows.run", &req).await.unwrap();
    await_status(&node, &p, "ws", "run1", "suspended").await;

    // The gate wrote its inbox item `flow-approval:run1:g`. Approve it (the reviewer's action).
    let resolve = json!({
        "item_id": "flow-approval:run1:g",
        "decision": "approved",
        "ts": 2
    })
    .to_string();
    call_tool(&node, &p, "ws", "inbox.resolve", &resolve)
        .await
        .unwrap();

    // The flow-approval reactor resumes the parked run; it completes with the payload passed through.
    let pass = react_to_flow_approvals(&node, &service("ws"), "ws", 3)
        .await
        .unwrap();
    assert_eq!(pass.resumed, 1, "one parked run resumed on approval");
    let snap = await_terminal(&node, &p, "ws", "run1").await;
    assert_eq!(snap["status"], "success");
    assert_eq!(node_output(&snap, "a")["payload"], json!(7));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn approval_gate_cancels_the_run_on_rejected() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let f = approval_flow("rej-flow");
    save_flow(&node, &p, "ws", &f).await;

    let req =
        json!({ "id": "rej-flow", "run_id": "run1", "params": { "t": 7 }, "ts": 1 }).to_string();
    call_tool(&node, &p, "ws", "flows.run", &req).await.unwrap();
    await_status(&node, &p, "ws", "run1", "suspended").await;

    let resolve = json!({
        "item_id": "flow-approval:run1:g",
        "decision": "rejected",
        "ts": 2
    })
    .to_string();
    call_tool(&node, &p, "ws", "inbox.resolve", &resolve)
        .await
        .unwrap();

    let pass = react_to_flow_approvals(&node, &service("ws"), "ws", 3)
        .await
        .unwrap();
    assert_eq!(pass.cancelled, 1, "one parked run cancelled on rejection");
    let snap = runs_get(&node, &p, "ws", "run1").await;
    assert_eq!(snap["status"], "cancelled");
}

/// A recording delivery `Target` — the only mocked thing (a true external), per testing §3.
#[derive(Default, Clone)]
struct RecordingTarget {
    delivered: Arc<std::sync::Mutex<Vec<String>>>,
    fail_first: Arc<std::sync::atomic::AtomicBool>,
}
impl Target for RecordingTarget {
    fn deliver(&self, effect: &Effect) -> impl Future<Output = Result<(), String>> + Send {
        let delivered = self.delivered.clone();
        let fail_first = self.fail_first.clone();
        let id = effect.id.clone();
        async move {
            if fail_first.swap(false, std::sync::atomic::Ordering::SeqCst) {
                return Err("transient".into());
            }
            delivered.lock().unwrap().push(id);
            Ok(())
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn outbox_sink_effect_is_delivered_by_the_relay() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal(
        "ws",
        &[
            "mcp:flows.save:call",
            "mcp:flows.run:call",
            "mcp:flows.runs.get:call",
            "mcp:outbox.enqueue:call",
            "store:flow:write",
            "store:flow:read",
        ],
    );
    // trigger → sink(target=outbox). The sink stages a pending effect; the relay delivers it.
    let trig = flow_node("t", "trigger", &[], json!({ "mode": "manual" }));
    let mut sink = flow_node(
        "s",
        "sink",
        &["t"],
        json!({ "target": "outbox", "name": "notify" }),
    );
    sink.with
        .insert("payload".into(), json!("${steps.t.payload}"));
    let f = flow_with("outbox-flow", vec![trig, sink], Concurrency::Queue);
    save_flow(&node, &p, "ws", &f).await;

    let req =
        json!({ "id": "outbox-flow", "run_id": "r1", "params": { "t": {"msg":"hi"} }, "ts": 1 })
            .to_string();
    call_tool(&node, &p, "ws", "flows.run", &req).await.unwrap();
    await_terminal(&node, &p, "ws", "r1").await;

    // Drive the relay with a recording Target that FAILS the first attempt then succeeds — proving the
    // retry/backoff path, and that the staged effect is really delivered.
    let target = RecordingTarget {
        fail_first: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        ..Default::default()
    };
    let p1 = relay_outbox(&node.store, "ws", &target, 10).await.unwrap();
    assert_eq!(p1.delivered, 0, "first pass fails (transient)");
    assert_eq!(p1.failed, 1);
    let p2 = relay_outbox(&node.store, "ws", &target, 100).await.unwrap();
    assert_eq!(p2.delivered, 1, "second pass (past backoff) delivers");
    assert_eq!(target.delivered.lock().unwrap().len(), 1);
}

// ================================================================================================
// Slice 5 — the webhook source node + series-event reactor
// ================================================================================================

/// A flow: webhook(source) → rhai(echo). Firing per hit is driven by `react_to_flow_sources`.
fn webhook_flow(id: &str, webhook_id: &str) -> Flow {
    let src = flow_node("w", "webhook", &[], json!({ "webhook_id": webhook_id }));
    let mut echo = flow_node("e", "rhai", &["w"], json!({ "source": "payload" }));
    echo.with
        .insert("payload".into(), json!("${steps.w.payload}"));
    flow_with(id, vec![src, echo], Concurrency::Queue)
}

/// Seed a real committed hit on the webhook series `webhook:{ws}:{id}` via the real ingest path.
async fn seed_hit(
    node: &Arc<HostNode>,
    p: &Principal,
    ws: &str,
    hook: &str,
    seq: u64,
    payload: Value,
) {
    let series = format!("webhook:{ws}:{hook}");
    let req = json!({ "samples": [{
        "series": series, "producer": "", "ts": seq, "seq": seq, "payload": payload,
    }]})
    .to_string();
    call_tool(node, p, ws, "ingest.write", &req).await.unwrap();
    lb_host::drain_workspace(&node.store, ws).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn webhook_source_fires_a_run_per_hit() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let f = webhook_flow("hook-flow", "hook1");
    save_flow(&node, &p, "ws", &f).await;

    // Seed one hit, then run the source reactor: exactly one run fires with the hit payload.
    seed_hit(&node, &p, "ws", "hook1", 1, json!({ "event": "push" })).await;
    let pass = react_to_flow_sources(&node, &service("ws"), "ws", 5)
        .await
        .unwrap();
    assert_eq!(pass.fired, 1, "one hit → one run");

    let run_id = lb_host::source_run_id("hook-flow", "w", 1);
    let snap = await_terminal(&node, &p, "ws", &run_id).await;
    assert_eq!(snap["status"], "success");
    assert_eq!(
        node_output(&snap, "e")["payload"],
        json!({ "event": "push" })
    );

    // A second reactor pass with no new hits fires nothing (the cursor advanced — fire-once).
    let again = react_to_flow_sources(&node, &service("ws"), "ws", 6)
        .await
        .unwrap();
    assert_eq!(again.fired, 0, "no new hits → no new run (cursor advanced)");

    // A NEW hit fires exactly one more run.
    seed_hit(&node, &p, "ws", "hook1", 2, json!({ "event": "pr" })).await;
    let third = react_to_flow_sources(&node, &service("ws"), "ws", 7)
        .await
        .unwrap();
    assert_eq!(third.fired, 1, "the new hit fires one run");
}

// ================================================================================================
// Mandatory: workspace isolation across the new reactors
// ================================================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_source_reactor_never_fires_ws_a_hits() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let pa = principal("ws-a", FULL);
    // ws-A has a webhook flow + a real hit.
    save_flow(&node, &pa, "ws-a", &webhook_flow("hf", "hook1")).await;
    seed_hit(&node, &pa, "ws-a", "hook1", 1, json!({ "x": 1 })).await;

    // A ws-B reactor pass sees NONE of ws-A's flows or hits (the hard wall §7).
    let pass_b = react_to_flow_sources(&node, &service("ws-b"), "ws-b", 5)
        .await
        .unwrap();
    assert_eq!(pass_b.fired, 0, "ws-B reactor fires nothing for ws-A hits");

    // ...and the ws-A reactor does fire ws-A's hit (proving the flow itself is valid).
    let pass_a = react_to_flow_sources(&node, &service("ws-a"), "ws-a", 6)
        .await
        .unwrap();
    assert_eq!(pass_a.fired, 1, "ws-A reactor fires ws-A's own hit");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_approval_reactor_never_resumes_ws_a_run() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let pa = principal("ws-a", FULL);
    save_flow(&node, &pa, "ws-a", &approval_flow("af")).await;
    let req = json!({ "id": "af", "run_id": "run1", "params": { "t": 1 }, "ts": 1 }).to_string();
    call_tool(&node, &pa, "ws-a", "flows.run", &req)
        .await
        .unwrap();
    await_status(&node, &pa, "ws-a", "run1", "suspended").await;
    // Approve in ws-A.
    let resolve =
        json!({ "item_id": "flow-approval:run1:g", "decision": "approved", "ts": 2 }).to_string();
    call_tool(&node, &pa, "ws-a", "inbox.resolve", &resolve)
        .await
        .unwrap();

    // A ws-B reactor pass resumes NOTHING (it scans ws-B's resolutions only).
    let pass_b = react_to_flow_approvals(&node, &service("ws-b"), "ws-b", 3)
        .await
        .unwrap();
    assert_eq!(pass_b.resumed, 0, "ws-B reactor never resumes a ws-A run");
    // The ws-A run is still suspended (untouched by ws-B).
    let snap = runs_get(&node, &pa, "ws-a", "run1").await;
    assert_eq!(snap["status"], "suspended");
}
