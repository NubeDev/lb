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
