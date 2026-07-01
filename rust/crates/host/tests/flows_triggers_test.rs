//! Host-layer tests for the triggers + lifecycle slice (triggers-lifecycle-scope Testing plan).
//! Real store (`mem://`) + real `lb-jobs` + real caps — no mocks. Injected clock via the logical
//! `ts` (never wall-clock). Mandatory: capability-deny, workspace-isolation, the cron reactor
//! (fire-once-then-skip + idempotent re-scan), the `inject` retain-vs-fire split (Decision 9),
//! `flows.enable`, and the reconciler's owner election (placement matched as data — no `if cloud`).

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_flows::{Flow, Node, Placement};
use lb_host::{
    call_tool, placement_matches, react_to_flows_cron, reconcile_flows, Node as HostNode,
};
use lb_store::read as store_read;
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

const FULL: &[&str] = &[
    "mcp:flows.save:call",
    "mcp:flows.get:call",
    "mcp:flows.run:call",
    "mcp:flows.enable:call",
    "mcp:flows.inject:call",
    "mcp:flows.node_state:call",
    "mcp:flows.runs.get:call",
    "mcp:flows.runs.list:call",
    "mcp:rules.run:call",
    "store:flow:write",
    "store:flow:read",
];

fn rhai_flow(id: &str) -> Flow {
    let n = Node {
        id: "a".into(),
        node_type: "rhai".into(),
        needs: vec![],
        with: Default::default(),
        config: json!({"source":"1"}),
    };
    Flow {
        workspace: "ws".into(),
        id: id.into(),
        name: id.into(),
        version: 0,
        params: Default::default(),
        nodes: vec![n],
        failure_policy: Default::default(),
        deleted: false,
        enabled: true,
        start_on_boot: false,
        placement: Placement::Either,
        cron: None,
        next_attempt_ts: 0,
    }
}

async fn save(node: &Arc<HostNode>, p: &Principal, ws: &str, f: &Flow) {
    let body = serde_json::to_value(f).unwrap().to_string();
    call_tool(node, p, ws, "flows.save", &body).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn enable_flips_the_durable_flags() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    save(&node, &p, "ws", &rhai_flow("en")).await;
    let req = json!({ "id": "en", "enabled": false, "start_on_boot": true }).to_string();
    call_tool(&node, &p, "ws", "flows.enable", &req)
        .await
        .unwrap();
    let got = store_read(&node.store, "ws", "flow", "en")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got["enabled"], false);
    assert_eq!(got["startOnBoot"], true);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn inject_retain_updates_state_and_starts_no_run() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    // A trigger node with mode=inject, inject_mode=retain — the control-loop retained input.
    let n = Node {
        id: "setpoint".into(),
        node_type: "trigger".into(),
        needs: vec![],
        with: Default::default(),
        config: json!({"mode":"inject","inject_mode":"retain"}),
    };
    let mut f = rhai_flow("cool");
    f.nodes = vec![n];
    save(&node, &p, "ws", &f).await;
    let req = json!({ "id": "cool", "node": "setpoint", "value": 4, "ts": 1 }).to_string();
    let out = call_tool(&node, &p, "ws", "flows.inject", &req)
        .await
        .unwrap();
    let r = serde_json::from_str::<Value>(&out).unwrap();
    assert_eq!(r["fired_run"], false); // retain → no run (Decision 9)
                                       // the retained value landed on flow_input.
    let st = store_read(&node.store, "ws", "flow_input", "cool:setpoint")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(st["value"], 4);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn inject_fire_starts_one_run() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let n = Node {
        id: "go".into(),
        node_type: "trigger".into(),
        needs: vec![],
        with: Default::default(),
        config: json!({"mode":"inject","inject_mode":"fire"}),
    };
    let mut f = rhai_flow("fire");
    f.nodes = vec![n];
    save(&node, &p, "ws", &f).await;
    let req = json!({ "id": "fire", "node": "go", "value": 99, "ts": 1 }).to_string();
    let out = call_tool(&node, &p, "ws", "flows.inject", &req)
        .await
        .unwrap();
    let r = serde_json::from_str::<Value>(&out).unwrap();
    assert_eq!(r["fired_run"], true); // fire → one run started
}

/// A flow authored as the CANVAS authors it: a `mode:"cron"` trigger node carrying the schedule in
/// `config.cron`, feeding `a`. The reactor scans the trigger NODE (per-node cursor), not a flow-level
/// `flow.cron`.
fn cron_node_flow(id: &str, node_id: &str, spec: &str) -> Flow {
    let trig = Node {
        id: node_id.into(),
        node_type: "trigger".into(),
        needs: vec![],
        with: Default::default(),
        config: json!({ "mode": "cron", "cron": spec }),
    };
    let a = Node {
        id: "a".into(),
        node_type: "rhai".into(),
        needs: vec![node_id.to_string()],
        with: Default::default(),
        config: json!({"source":"1"}),
    };
    let mut f = rhai_flow(id);
    f.nodes = vec![trig, a];
    f
}

/// The per-node cursor `next_attempt_ts` (the reactor's durable per-trigger state), read from the
/// store (`flow_trigger_state:{flow}:{node}`).
async fn cursor_next(node: &Arc<HostNode>, ws: &str, flow: &str, node_id: &str) -> u64 {
    store_read(
        &node.store,
        ws,
        "flow_trigger_state",
        &format!("{flow}:{node_id}"),
    )
    .await
    .unwrap()
    .and_then(|v| v["next_attempt_ts"].as_u64())
    .unwrap_or(0)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn cron_reactor_fires_once_then_skips() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    save(
        &node,
        &p,
        "ws",
        &cron_node_flow("cron1", "t", "*/1 * * * *"),
    )
    .await;
    // First pass primes the per-node cursor (no fire on init).
    react_to_flows_cron(&node, &p, "ws", 100).await.unwrap();
    let next = cursor_next(&node, "ws", "cron1", "t").await;
    assert!(next > 0, "reactor primed the trigger node's cursor");
    // Due pass → fires once, advances the cursor.
    let pass = react_to_flows_cron(&node, &p, "ws", next + 1)
        .await
        .unwrap();
    assert_eq!(pass.fired, 1);
    // A re-scan at the same now → idempotent no-op (the job exists).
    let pass2 = react_to_flows_cron(&node, &p, "ws", next + 1)
        .await
        .unwrap();
    assert_eq!(pass2.fired, 0);
}

/// END-TO-END: a canvas-authored cron flow FIRES through the reactor and settles a run with real
/// values, firing ONLY the trigger's subgraph (entry = the trigger node). This is the path broken
/// live ("added a cron trigger, count never goes up").
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn cron_trigger_node_fires_its_subgraph() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);

    let trig = Node {
        id: "trigger-5".into(),
        node_type: "trigger".into(),
        needs: vec![],
        with: Default::default(),
        config: json!({ "mode": "cron", "cron": "* * * * *" }),
    };
    let counter = Node {
        id: "a".into(),
        node_type: "count".into(),
        needs: vec!["trigger-5".into()],
        with: serde_json::Map::from_iter([("payload".into(), json!([1, 2, 3, 4]))]),
        config: json!({}),
    };
    let mut f = rhai_flow("chain4");
    f.nodes = vec![trig, counter];
    save(&node, &p, "ws", &f).await;

    // The reactor primes the trigger node's cursor on first sight, then fires on a due pass.
    react_to_flows_cron(&node, &p, "ws", 100).await.unwrap();
    let next = cursor_next(&node, "ws", "chain4", "trigger-5").await;
    assert!(next > 0, "reactor primed the per-node cursor");

    let pass = react_to_flows_cron(&node, &p, "ws", next + 1)
        .await
        .unwrap();
    assert_eq!(pass.fired, 1, "the cron trigger fired");

    // The fired run is real + settled with values, with the run id keyed per (flow, NODE, instant).
    let run_id = lb_host::cron_run_id("chain4", "trigger-5", next);
    let snap = await_terminal(&node, &p, "ws", &run_id).await;
    assert_eq!(snap["status"], "success");
    let step_a = snap["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == "a")
        .expect("count step present");
    assert_eq!(
        step_a["output"]["payload"], 4,
        "count node produced a real value"
    );
}

/// Poll the durable run until terminal (the cron-fired run is driven synchronously by
/// `react_to_flows_cron` → `flows_run`, but poll defensively so the assert is on a settled snapshot).
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
    panic!("cron run {run_id} never settled");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn cron_reactor_workspace_isolation() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let pa = principal("ws-a", FULL);
    let mut f = rhai_flow("cron-iso");
    f.cron = Some("*/1 * * * *".into());
    f.next_attempt_ts = 100;
    save(&node, &pa, "ws-a", &f).await;
    // a ws-B reactor pass never sees/fires ws-A's flow (the directory is ws-scoped).
    let pb = principal("ws-b", FULL);
    let pass = react_to_flows_cron(&node, &pb, "ws-b", 200).await.unwrap();
    assert_eq!(pass.fired, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn capability_deny_enable_without_cap() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let saver = principal("ws", FULL);
    let caps: Vec<&str> = FULL
        .iter()
        .filter(|c| **c != "mcp:flows.enable:call")
        .cloned()
        .collect();
    let enabler = principal("ws", &caps);
    save(&node, &saver, "ws", &rhai_flow("den")).await;
    let req = json!({ "id": "den", "enabled": false }).to_string();
    let err = call_tool(&node, &enabler, "ws", "flows.enable", &req)
        .await
        .unwrap_err();
    assert!(matches!(err, lb_mcp::ToolError::Denied));
}

// ───────────────────────── flow⇄dashboard binding UX (port-aware inject) ─────────────────────────

/// Run a flow synchronously-then-poll and return its terminal snapshot.
async fn run_to_terminal(
    node: &Arc<HostNode>,
    p: &Principal,
    ws: &str,
    id: &str,
    run_id: &str,
) -> Value {
    let req = json!({ "id": id, "run_id": run_id, "ts": 1 }).to_string();
    call_tool(node, p, ws, "flows.run", &req).await.unwrap();
    await_terminal(node, p, ws, run_id).await
}

fn step_payload<'a>(snap: &'a Value, id: &str) -> &'a Value {
    snap["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == id)
        .map(|s| &s["output"]["payload"])
        .unwrap_or(&Value::Null)
}

/// A single-node `ctl` flow: a rhai node that echoes its `payload` (so the run's resolved input is
/// observable as the recorded output). `with.payload` is the static baseline (lowest precedence).
fn ctl_flow(id: &str, static_payload: Value) -> Flow {
    let ctl = Node {
        id: "ctl".into(),
        node_type: "rhai".into(),
        needs: vec![],
        with: serde_json::Map::from_iter([("payload".into(), static_payload)]),
        config: json!({ "source": "payload" }),
    };
    let mut f = rhai_flow(id);
    f.nodes = vec![ctl];
    f
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn inject_with_port_upserts_the_per_port_record() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    save(&node, &p, "ws", &ctl_flow("pp", json!(1))).await;
    // inject WITH a port → the per-port record `flow_input:{flow}:{node}:{port}`.
    let req =
        json!({ "id": "pp", "node": "ctl", "port": "payload", "value": 7, "ts": 1 }).to_string();
    call_tool(&node, &p, "ws", "flows.inject", &req)
        .await
        .unwrap();
    let st = store_read(&node.store, "ws", "flow_input", "pp:ctl:payload")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(st["value"], 7);
    assert_eq!(st["port"], "payload");
    // the node-level record is untouched (the per-port write does not derive it).
    assert!(store_read(&node.store, "ws", "flow_input", "pp:ctl")
        .await
        .unwrap()
        .is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn inject_without_port_unchanged_node_level_record() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    save(&node, &p, "ws", &ctl_flow("nl", json!(1))).await;
    // inject WITHOUT a port → node-level record, exactly as before (back-compat).
    let req = json!({ "id": "nl", "node": "ctl", "value": 3, "ts": 1 }).to_string();
    call_tool(&node, &p, "ws", "flows.inject", &req)
        .await
        .unwrap();
    let st = store_read(&node.store, "ws", "flow_input", "nl:ctl")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(st["value"], 3);
    assert!(st.get("port").is_none() || st["port"].is_null());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn binding_precedence_per_port_over_node_level_over_with() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    save(&node, &p, "ws", &ctl_flow("prec", json!(1))).await;

    // 1) only the static `with` → payload 1.
    let snap = run_to_terminal(&node, &p, "ws", "prec", "prec-1").await;
    assert_eq!(step_payload(&snap, "ctl"), &json!(1));

    // 2) node-level retained beats `with` → payload 5.
    let req = json!({ "id": "prec", "node": "ctl", "value": 5, "ts": 1 }).to_string();
    call_tool(&node, &p, "ws", "flows.inject", &req)
        .await
        .unwrap();
    let snap = run_to_terminal(&node, &p, "ws", "prec", "prec-2").await;
    assert_eq!(step_payload(&snap, "ctl"), &json!(5));

    // 3) per-port retained beats node-level → payload 9 (the precedence headline).
    let req =
        json!({ "id": "prec", "node": "ctl", "port": "payload", "value": 9, "ts": 1 }).to_string();
    call_tool(&node, &p, "ws", "flows.inject", &req)
        .await
        .unwrap();
    let snap = run_to_terminal(&node, &p, "ws", "prec", "prec-3").await;
    assert_eq!(step_payload(&snap, "ctl"), &json!(9));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn object_payload_round_trips_inject_to_run() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    save(&node, &p, "ws", &ctl_flow("obj", json!(null))).await;
    let obj = json!({ "mode": "eco", "band": [3.5, 4.5] });
    let req = json!({ "id": "obj", "node": "ctl", "value": obj, "ts": 1 }).to_string();
    call_tool(&node, &p, "ws", "flows.inject", &req)
        .await
        .unwrap();
    // it persisted on flow_input...
    let st = store_read(&node.store, "ws", "flow_input", "obj:ctl")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(st["value"], obj);
    // ...and the run reads the structured value as the node's payload.
    let snap = run_to_terminal(&node, &p, "ws", "obj", "obj-1").await;
    assert_eq!(step_payload(&snap, "ctl"), &obj);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn node_state_reads_back_retained_inputs() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    save(&node, &p, "ws", &ctl_flow("rb", json!(1))).await;
    // a node-level retained value AND a per-port value.
    call_tool(
        &node,
        &p,
        "ws",
        "flows.inject",
        &json!({ "id": "rb", "node": "ctl", "value": 4, "ts": 1 }).to_string(),
    )
    .await
    .unwrap();
    call_tool(
        &node,
        &p,
        "ws",
        "flows.inject",
        &json!({ "id": "rb", "node": "ctl", "port": "payload", "value": 6, "ts": 1 }).to_string(),
    )
    .await
    .unwrap();
    let out = call_tool(
        &node,
        &p,
        "ws",
        "flows.node_state",
        &json!({ "id": "rb" }).to_string(),
    )
    .await
    .unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    let entry = v["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["node"] == "ctl")
        .unwrap();
    // a control seeds its current state from its OWN input (node-level + per-port), not its output.
    assert_eq!(entry["input"], 4);
    assert_eq!(entry["inputs"]["payload"], 6);
}

/// AGNOSTIC to the port NAME: inject + precedence + read-back all key on the `{port}` string, never a
/// known `payload`. A developer's new node type with an arbitrary input port (`setpoint`) drives and
/// reads back with zero engine changes. (Here the `ctl` rhai node still echoes `payload`, but we prove
/// the per-port record + node_state read-back work for a non-`payload` slot name.)
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn port_aware_inject_is_agnostic_to_the_port_name() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    save(&node, &p, "ws", &ctl_flow("agno", json!(1))).await;
    // inject a value into an arbitrarily-named port the engine has never heard of.
    call_tool(
        &node,
        &p,
        "ws",
        "flows.inject",
        &json!({ "id": "agno", "node": "ctl", "port": "setpoint", "value": 21.5, "ts": 1 })
            .to_string(),
    )
    .await
    .unwrap();
    // the per-port record is keyed by the real port name...
    let st = store_read(&node.store, "ws", "flow_input", "agno:ctl:setpoint")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(st["value"], 21.5);
    assert_eq!(st["port"], "setpoint");
    // ...and node_state reads it back under that name (not flattened into `payload`).
    let out = call_tool(
        &node,
        &p,
        "ws",
        "flows.node_state",
        &json!({ "id": "agno" }).to_string(),
    )
    .await
    .unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    let entry = v["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["node"] == "ctl")
        .unwrap();
    assert_eq!(entry["inputs"]["setpoint"], 21.5);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn capability_deny_inject_does_not_upsert_node_or_port_record() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let saver = principal("ws", FULL);
    save(&node, &saver, "ws", &ctl_flow("deny", json!(1))).await;
    // a caller WITHOUT mcp:flows.inject:call — denied server-side at the bridge.
    let caps: Vec<&str> = FULL
        .iter()
        .filter(|c| **c != "mcp:flows.inject:call")
        .cloned()
        .collect();
    let viewer = principal("ws", &caps);
    // node-level inject denied...
    let err = call_tool(
        &node,
        &viewer,
        "ws",
        "flows.inject",
        &json!({ "id": "deny", "node": "ctl", "value": 5, "ts": 1 }).to_string(),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, lb_mcp::ToolError::Denied));
    // ...and port-keyed inject denied — neither record is upserted.
    let err = call_tool(
        &node,
        &viewer,
        "ws",
        "flows.inject",
        &json!({ "id": "deny", "node": "ctl", "port": "payload", "value": 5, "ts": 1 }).to_string(),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, lb_mcp::ToolError::Denied));
    assert!(store_read(&node.store, "ws", "flow_input", "deny:ctl")
        .await
        .unwrap()
        .is_none());
    assert!(
        store_read(&node.store, "ws", "flow_input", "deny:ctl:payload")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_isolation_ws_b_cannot_inject_into_ws_a_flow() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let pa = principal("ws-a", FULL);
    save(&node, &pa, "ws-a", &ctl_flow("iso", json!(1))).await;
    // ws-B injects into "iso" — the flow does not exist in ws-B → denied (NotFound→Denied).
    let pb = principal("ws-b", FULL);
    let err = call_tool(
        &node,
        &pb,
        "ws-b",
        "flows.inject",
        &json!({ "id": "iso", "node": "ctl", "value": 9, "ts": 1 }).to_string(),
    )
    .await
    .unwrap_err();
    assert!(matches!(
        err,
        lb_mcp::ToolError::Denied | lb_mcp::ToolError::NotFound
    ));
    // ws-A's record is untouched; no ws-B record was written.
    assert!(store_read(&node.store, "ws-a", "flow_input", "iso:ctl")
        .await
        .unwrap()
        .is_none());
    assert!(store_read(&node.store, "ws-b", "flow_input", "iso:ctl")
        .await
        .unwrap()
        .is_none());
}

#[test]
fn placement_matched_as_data_no_role_branch() {
    // EITHER matches every role; CLOUD-ONLY only the shared-authority roles; LOCAL-ONLY the edge.
    assert!(placement_matches(Placement::Either, lb_host::Role::Solo));
    assert!(placement_matches(Placement::Either, lb_host::Role::Edge));
    assert!(placement_matches(Placement::CloudOnly, lb_host::Role::Hub));
    assert!(!placement_matches(
        Placement::CloudOnly,
        lb_host::Role::Edge
    ));
    assert!(placement_matches(Placement::LocalOnly, lb_host::Role::Edge));
    assert!(!placement_matches(Placement::LocalOnly, lb_host::Role::Hub));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn reconciler_disarms_a_disabled_flow() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    // Seed a real mqtt install so the `mqtt.in` source node type validates at save.
    let toml = include_str!("../../../extensions/mqtt/extension.toml");
    let manifest = lb_ext_loader::Manifest::parse(toml).unwrap();
    let install = lb_assets::Install::new(
        "mqtt",
        "0.1.0",
        vec![
            "mcp:mqtt.subscribe:call".into(),
            "mcp:mqtt.publish:call".into(),
            "mcp:mqtt.arm:call".into(),
            "mcp:mqtt.disarm:call".into(),
        ],
        1,
    )
    .with_nodes(manifest.nodes.clone());
    lb_assets::record_install(&node.store, "ws", &install)
        .await
        .unwrap();
    // A flow with one ext (source) node, disabled → the reconciler disarms (no leaked socket).
    let src = Node {
        id: "in".into(),
        node_type: "mqtt.in".into(),
        needs: vec![],
        with: Default::default(),
        config: json!({"broker":"b","topic":"t"}),
    };
    let mut f = rhai_flow("rc");
    f.nodes = vec![src];
    f.enabled = false;
    save(&node, &p, "ws", &f).await;
    let pass = reconcile_flows(&node, &p, "ws", lb_host::Role::Solo, 1)
        .await
        .unwrap();
    assert_eq!(pass.disarmed, 1);
    assert_eq!(pass.armed, 0);
    // the source marker is disarmed.
    let st = store_read(&node.store, "ws", "flow_node_state", "rc:in")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(st["armed"], false);
}
