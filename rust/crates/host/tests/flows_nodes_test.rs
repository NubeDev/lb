//! Host-layer tests for the `flows.nodes` merged registry (node-descriptor-scope Testing plan).
//! Real store (`mem://`), real caps, real install records seeded through the real `record_install`
//! write path — no mocks. The descriptor declares no caps; the deny lives at the bridge gate.
//!
//! Mandatory: capability-deny (`flows.nodes` refused without the cap), workspace-isolation (an ext
//! installed in ws-A is absent from ws-B's registry), the merged registry reflects install/uninstall,
//! and a node whose bound tool the install grant omits is dropped (it could not run anyway).

use std::sync::Arc;

use lb_assets::{record_install, Install};
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_flows::{NodeBlock, NodeKind};
use lb_host::{call_tool, Node};
use serde_json::json;

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

const NODES_CAP: &str = "mcp:flows.nodes:call";

fn node_block(r#type: &str, kind: NodeKind, tool: &str) -> NodeBlock {
    NodeBlock {
        r#type: r#type.into(),
        kind,
        tool: tool.into(),
        title: None,
        category: Some("Messaging".into()),
        inputs: if matches!(kind, NodeKind::Sink) {
            vec!["payload".into()]
        } else {
            vec![]
        },
        outputs: if matches!(kind, NodeKind::Sink) {
            vec![]
        } else {
            vec!["sample".into()]
        },
        config_version: 1,
        config: json!({"type":"object","properties":{"topic":{"type":"string"}}}),
    }
}

/// Seed a real install record (the real write path) carrying validated node blocks + the grants that
/// make those nodes' bound tools runnable.
async fn seed_ext(
    node: &Arc<Node>,
    ws: &str,
    ext_id: &str,
    nodes: Vec<NodeBlock>,
    granted: Vec<String>,
) {
    let install = Install::new(ext_id, "0.1.0", granted, 1).with_nodes(nodes);
    record_install(&node.store, ws, &install).await.unwrap();
}

fn types(out: &serde_json::Value) -> Vec<String> {
    out["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["type"].as_str().unwrap().to_string())
        .collect()
}

async fn call_nodes(node: &Arc<Node>, p: &Principal, ws: &str) -> serde_json::Value {
    let out = call_tool(node, p, ws, "flows.nodes", "{}").await.unwrap();
    serde_json::from_str(&out).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn registry_has_all_builtins_with_no_installs() {
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("ws-a", &[NODES_CAP]);
    let out = call_nodes(&node, &p, "ws-a").await;
    let types = types(&out);
    assert_eq!(
        types,
        vec!["trigger", "tool", "rhai", "count", "subflow", "sink"]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn registry_reflects_an_installed_extension() {
    let node = Arc::new(Node::boot().await.unwrap());
    seed_ext(
        &node,
        "ws-a",
        "mqtt",
        vec![
            node_block("in", NodeKind::Source, "subscribe"),
            node_block("out", NodeKind::Sink, "publish"),
        ],
        vec![
            "mcp:mqtt.subscribe:call".into(),
            "mcp:mqtt.publish:call".into(),
        ],
    )
    .await;
    let p = principal("ws-a", &[NODES_CAP]);
    let out = call_nodes(&node, &p, "ws-a").await;
    let types = types(&out);
    // built-ins first, then the ext nodes (global type <ext>.<type>), sorted.
    assert_eq!(
        types,
        vec!["trigger", "tool", "rhai", "count", "subflow", "sink", "mqtt.in", "mqtt.out"]
    );
    // the ext descriptor carries its ports + category from the block.
    let mqtt_in = out["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["type"] == "mqtt.in")
        .unwrap();
    assert_eq!(mqtt_in["kind"], "source");
    assert_eq!(mqtt_in["category"], "Messaging");
    assert_eq!(mqtt_in["outputs"], json!(["sample"]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_isolation_ext_in_ws_a_absent_in_ws_b() {
    let node = Arc::new(Node::boot().await.unwrap());
    seed_ext(
        &node,
        "ws-a",
        "mqtt",
        vec![node_block("in", NodeKind::Source, "subscribe")],
        vec!["mcp:mqtt.subscribe:call".into()],
    )
    .await;
    // ws-A sees the ext node.
    let pa = principal("ws-a", &[NODES_CAP]);
    let out_a = call_nodes(&node, &pa, "ws-a").await;
    assert!(types(&out_a).contains(&"mqtt.in".to_string()));
    // ws-B (no install) sees only built-ins — the wall holds at the registry.
    let pb = principal("ws-b", &[NODES_CAP]);
    let out_b = call_nodes(&node, &pb, "ws-b").await;
    assert_eq!(
        types(&out_b),
        vec!["trigger", "tool", "rhai", "count", "subflow", "sink"]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn capability_deny_without_flows_nodes_cap() {
    let node = Arc::new(Node::boot().await.unwrap());
    // a principal with NO caps — the bridge gate refuses `mcp:flows.nodes:call`.
    let p = principal("ws-a", &[]);
    let err = call_tool(&node, &p, "ws-a", "flows.nodes", "{}")
        .await
        .unwrap_err();
    assert!(matches!(err, lb_mcp::ToolError::Denied));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn node_whose_tool_grant_omitted_is_dropped() {
    let node = Arc::new(Node::boot().await.unwrap());
    // `in` binds `subscribe` (granted) → kept; `out` binds `publish` (NOT granted) → dropped: the
    // node could not run anyway (no install grant), so it never reaches the palette.
    seed_ext(
        &node,
        "ws-a",
        "mqtt",
        vec![
            node_block("in", NodeKind::Source, "subscribe"),
            node_block("out", NodeKind::Sink, "publish"),
        ],
        vec!["mcp:mqtt.subscribe:call".into()], // publish grant omitted
    )
    .await;
    let p = principal("ws-a", &[NODES_CAP]);
    let out = call_nodes(&node, &p, "ws-a").await;
    let types = types(&out);
    assert!(types.contains(&"mqtt.in".to_string()));
    assert!(!types.contains(&"mqtt.out".to_string()));
}
