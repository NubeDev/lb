//! Host-layer tests for the orphan-source sweep (flow-deploy-ux-scope Testing plan). The per-flow
//! reconcile pass only converges sources of a flow STILL in the list; a deleted (tombstoned) flow, or
//! a source node removed by an edit, leaked its live socket. The sweep is the missing convergence.
//! Real store (`mem://`) + real caps — no mocks. Mandatory: the delete-orphan regression, the
//! node-removal orphan, idempotency, and workspace isolation (a ws-B armed source survives a ws-A
//! reconcile).

use std::sync::Arc;

use lb_assets::{record_install, Install};
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_ext_loader::Manifest;
use lb_flows::{Flow, Node, Placement};
use lb_host::{arm_source, call_tool, reconcile_flows, Node as HostNode, Role as NodeRole};
use serde_json::json;

/// Seed the real `mqtt` install (its `[[node]]` descriptors) so `flows.save` validates `mqtt.in`
/// configs against the installed schema — the same path the live registry uses.
async fn install_mqtt(node: &Arc<HostNode>, ws: &str) {
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
    record_install(&node.store, ws, &install).await.unwrap();
}

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

const CAPS: &[&str] = &[
    "mcp:flows.save:call",
    "mcp:flows.delete:call",
    "mcp:flows.nodes:call",
    "store:flow:write",
    "store:flow:read",
    "store:flow_node_state:write",
    "store:flow_node_state:read",
    "mcp:*.call:call",
];

/// A flow with one ext source node (`mqtt.in`) — the reconciler treats a non-builtin node as a
/// potential source and arms it.
fn source_flow(id: &str, node_id: &str) -> Flow {
    let n = Node {
        id: node_id.into(),
        node_type: "mqtt.in".into(),
        needs: vec![],
        with: Default::default(),
        config: json!({ "broker": "broker.local", "topic": "sensors/temp" }),
        inputs: Vec::new(),
        position: None,
    };
    Flow {
        workspace: "ws-a".into(),
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
        concurrency: Default::default(),
        cron: None,
        next_attempt_ts: 0,
        managed_by: None,
    }
}

async fn armed(node: &Arc<HostNode>, ws: &str, flow: &str, node_id: &str) -> bool {
    lb_store::read(
        &node.store,
        ws,
        "flow_node_state",
        &format!("{flow}:{node_id}"),
    )
    .await
    .unwrap()
    .and_then(|v| v.get("armed").and_then(|a| a.as_bool()))
    .unwrap_or(false)
}

/// Regression: a DELETED (tombstoned) flow's armed source is disarmed by the sweep — the leaked socket
/// the per-flow pass never reaches (a tombstone is skipped by `flows_list_internal`).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn delete_orphans_the_source_and_the_sweep_disarms_it() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws-a", CAPS);

    // Arm the source directly (the reconciler would too; this pins the marker with its `_type`).
    let cfg = json!({ "_type": "mqtt.in", "broker": "broker.local", "topic": "sensors/temp" });
    arm_source(&node, &p, "ws-a", "cooler", "temp-in", cfg)
        .await
        .unwrap();
    assert!(
        armed(&node, "ws-a", "cooler", "temp-in").await,
        "armed after arm"
    );

    // The flow is not in the store (deleted/never-saved) → the marker is an orphan. One reconcile pass
    // sweeps it.
    let pass = reconcile_flows(&node, &p, "ws-a", NodeRole::Solo, 0)
        .await
        .unwrap();
    assert_eq!(pass.orphans_disarmed, 1, "the orphan source was disarmed");
    assert!(
        !armed(&node, "ws-a", "cooler", "temp-in").await,
        "disarmed after sweep"
    );

    // Idempotent: a second pass finds nothing to sweep.
    let pass2 = reconcile_flows(&node, &p, "ws-a", NodeRole::Solo, 0)
        .await
        .unwrap();
    assert_eq!(pass2.orphans_disarmed, 0, "second pass is a no-op");
}

/// Removing a source node via an edit (a save that drops the node) orphans its marker; the sweep
/// disarms it while the flow's REMAINING sources stay armed (the sweep only targets orphans).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn source_node_removal_orphans_only_the_removed_node() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws-a", CAPS);
    install_mqtt(&node, "ws-a").await; // so `flows.save` validates the `mqtt.in` configs

    // Save a flow with TWO source nodes and let a reconcile arm both.
    let mut flow = source_flow("plant", "src-keep");
    flow.nodes.push(Node {
        id: "src-drop".into(),
        node_type: "mqtt.in".into(),
        needs: vec![],
        with: Default::default(),
        config: json!({ "broker": "b", "topic": "t2" }),
        inputs: Vec::new(),
        position: None,
    });
    let body = serde_json::to_value(&flow).unwrap().to_string();
    call_tool(&node, &p, "ws-a", "flows.save", &body)
        .await
        .unwrap();
    reconcile_flows(&node, &p, "ws-a", NodeRole::Solo, 0)
        .await
        .unwrap();
    assert!(
        armed(&node, "ws-a", "plant", "src-keep").await,
        "src-keep armed"
    );
    assert!(
        armed(&node, "ws-a", "plant", "src-drop").await,
        "src-drop armed"
    );

    // Edit: save the flow with `src-drop` removed (topology edit — a new version).
    flow.nodes.retain(|n| n.id != "src-drop");
    let body = serde_json::to_value(&flow).unwrap().to_string();
    call_tool(&node, &p, "ws-a", "flows.save", &body)
        .await
        .unwrap();

    let pass = reconcile_flows(&node, &p, "ws-a", NodeRole::Solo, 0)
        .await
        .unwrap();
    assert_eq!(pass.orphans_disarmed, 1, "only the removed node is swept");
    assert!(
        !armed(&node, "ws-a", "plant", "src-drop").await,
        "removed node disarmed"
    );
    assert!(
        armed(&node, "ws-a", "plant", "src-keep").await,
        "kept node stays armed"
    );
}

/// Workspace isolation: a ws-A reconcile pass never touches a ws-B armed source (the scan is
/// ws-walled). A ws-B orphan survives a ws-A sweep.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_a_sweep_leaves_ws_b_armed_source_untouched() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let pa = principal("ws-a", CAPS);
    let pb = principal("ws-b", CAPS);

    let cfg = json!({ "_type": "mqtt.in", "broker": "b", "topic": "t" });
    arm_source(&node, &pb, "ws-b", "cooler", "temp-in", cfg)
        .await
        .unwrap();

    // A ws-A reconcile (no ws-A flows/markers) sweeps nothing in ws-B.
    let pass = reconcile_flows(&node, &pa, "ws-a", NodeRole::Solo, 0)
        .await
        .unwrap();
    assert_eq!(pass.orphans_disarmed, 0, "ws-A pass sees no ws-B markers");
    assert!(
        armed(&node, "ws-b", "cooler", "temp-in").await,
        "ws-B source survives"
    );
}
