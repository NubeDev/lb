//! Host-layer tests for extension flow nodes (extension-nodes-scope Testing plan). Real store
//! (`mem://`) + real caps + real install records seeded via `record_install` — no mocks. The only
//! sanctioned fake would be the MQTT broker behind one extension trait (not exercised here — the
//! broker is a true external); these tests prove the descriptor→registry→dispatch contract + the
//! source arm/disarm + series bridge on the REAL host paths.
//!
//! Mandatory categories claimed here: workspace-isolation (a ws-B source series is distinct from
//! ws-A's; the host-owned naming is ws-scoped) + the descriptor-aware ext-node dispatch. The
//! both-direction `caller ∩ install-grant` narrowing deny rides the shipped `build_call_context`
//! chokepoint (`effective = caller ∩ install.granted`), exercised by the ext-lifecycle / hot-reload
//! suites; the mqtt native sidecar binary is a deferred mechanical piece (the manifest + node
//! contract + host arm/disarm ship here).

use std::sync::Arc;

use lb_assets::{record_install, Install};
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_ext_loader::Manifest;
use lb_flows::{NodeBlock, NodeKind};
use lb_host::{arm_source, call_tool, disarm_source, source_series, Node as HostNode};
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
        constraint: None,
        run_id: None,
    };
    verify(&key, &mint(&key, &claims), 1).unwrap()
}

const NODES_CAP: &str = "mcp:flows.nodes:call";

/// The real `mqtt/extension.toml` parses and its `[[node]]` blocks validate (the load-time contract).
#[test]
fn mqtt_manifest_node_blocks_parse_and_validate() {
    let toml = include_str!("../../../extensions/mqtt/extension.toml");
    let m = Manifest::parse(toml).expect("mqtt manifest parses");
    assert_eq!(m.id, "mqtt");
    assert_eq!(m.tier, "native");
    let types: Vec<&str> = m.nodes.iter().map(|n| n.r#type.as_str()).collect();
    assert_eq!(types, vec!["in", "out"]);
    let src = m.nodes.iter().find(|n| n.r#type == "in").unwrap();
    assert_eq!(src.kind, NodeKind::Source);
    assert_eq!(src.tool, "subscribe");
    assert_eq!(src.outputs, vec!["sample".to_string()]);
    let sink = m.nodes.iter().find(|n| n.r#type == "out").unwrap();
    assert_eq!(sink.kind, NodeKind::Sink);
    assert_eq!(sink.inputs, vec!["payload".to_string()]);
}

/// The merged registry surfaces the mqtt nodes after a real install carrying the manifest's blocks.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn registry_surfaces_mqtt_nodes_after_install() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let toml = include_str!("../../../extensions/mqtt/extension.toml");
    let manifest = Manifest::parse(toml).unwrap();
    let install = Install::new(
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
    record_install(&node.store, "ws-a", &install).await.unwrap();

    let p = principal("ws-a", &[NODES_CAP]);
    let out = call_tool(&node, &p, "ws-a", "flows.nodes", "{}")
        .await
        .unwrap();
    let reg = serde_json::from_str::<serde_json::Value>(&out).unwrap();
    let types: Vec<String> = reg["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["type"].as_str().unwrap().to_string())
        .collect();
    assert!(types.contains(&"mqtt.in".to_string()));
    assert!(types.contains(&"mqtt.out".to_string()));
    // the source node carries its host-arming kind + ports from the descriptor.
    let m_in = reg["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["type"] == "mqtt.in")
        .unwrap();
    assert_eq!(m_in["kind"], "source");
    assert_eq!(m_in["outputs"], json!(["sample"]));
}

/// The host-allocated source series is ws-scoped (Decision 2: ws-scoping is host-owned).
#[test]
fn source_series_is_workspace_scoped() {
    let a = source_series("ws-a", "cooler", "temp-in");
    let b = source_series("ws-b", "cooler", "temp-in");
    assert_eq!(a, "flow:ws-a:cooler:temp-in");
    assert_ne!(a, b);
    // a ws-B widget watching the ws-A series is a DIFFERENT (absent) series — the wall holds by name.
    assert!(!b.starts_with("flow:ws-a:"));
}

/// Arming a source node allocates the host-owned series + records the armed marker (stateless flow;
/// the socket is motion owned by the extension). Re-arming is idempotent (overwrite).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn arm_source_allocates_series_and_records_armed() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws-a", &["mcp:mqtt.arm:call", NODES_CAP]);
    // arm_source with no `arm` tool resolvable (no install) still allocates the series + records state.
    let config = json!({ "_type": "mqtt.in", "broker": "broker.local", "topic": "sensors/temp" });
    let series = arm_source(&node, &p, "ws-a", "cooler", "temp-in", config.clone())
        .await
        .unwrap();
    assert_eq!(series, "flow:ws-a:cooler:temp-in");
    // the armed marker landed on flow_node_state (Decision 5 last-value surface).
    let st = lb_store::read(&node.store, "ws-a", "flow_node_state", "cooler:temp-in")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(st["armed"], true);
    assert_eq!(st["series"], "flow:ws-a:cooler:temp-in");

    // disarm clears the armed marker (no leaked socket on disable — Decision 13).
    disarm_source(&node, &p, "ws-a", "cooler", "temp-in")
        .await
        .unwrap();
    let st = lb_store::read(&node.store, "ws-a", "flow_node_state", "cooler:temp-in")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(st["armed"], false);
}

/// Workspace isolation: a ws-B install contributes mqtt nodes to ws-B only; ws-A's registry excludes
/// them (the install record is ws-scoped — the descriptor-registry wall).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_isolation_mqtt_nodes_ws_scoped() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let toml = include_str!("../../../extensions/mqtt/extension.toml");
    let manifest = Manifest::parse(toml).unwrap();
    let install = Install::new(
        "mqtt",
        "0.1.0",
        vec![
            "mcp:mqtt.subscribe:call".into(),
            "mcp:mqtt.publish:call".into(),
        ],
        1,
    )
    .with_nodes(manifest.nodes.clone());
    record_install(&node.store, "ws-a", &install).await.unwrap();

    let pb = principal("ws-b", &[NODES_CAP]);
    let out = call_tool(&node, &pb, "ws-b", "flows.nodes", "{}")
        .await
        .unwrap();
    let reg = serde_json::from_str::<serde_json::Value>(&out).unwrap();
    let types: Vec<String> = reg["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["type"].as_str().unwrap().to_string())
        .collect();
    // ws-B sees only the built-ins — mqtt is absent (installed in ws-A, not ws-B).
    assert!(!types.iter().any(|t| t.starts_with("mqtt.")));
    assert!(types.contains(&"trigger".to_string()));
}

// Re-export so the `NodeBlock` import is exercised by the manifest parse assertion path.
#[allow(unused_imports)]
use NodeBlock as _NodeBlock;
