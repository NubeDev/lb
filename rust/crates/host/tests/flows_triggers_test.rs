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

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn cron_reactor_fires_once_then_skips() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let mut f = rhai_flow("cron1");
    f.cron = Some("*/1 * * * *".into());
    f.next_attempt_ts = 100;
    save(&node, &p, "ws", &f).await;
    // due at now=200 (next_attempt_ts=100 ≤ 200) → fires once, advances next_attempt_ts past 200.
    let pass = react_to_flows_cron(&node, &p, "ws", 200).await.unwrap();
    assert_eq!(pass.fired, 1);
    // a re-scan at the same now → idempotent no-op (the job exists) + the firing advanced.
    let pass2 = react_to_flows_cron(&node, &p, "ws", 200).await.unwrap();
    assert_eq!(pass2.fired, 0);
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
