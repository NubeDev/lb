//! S4 — the appliance registry drives a two-node routed `control-engine.tree`, plus offline fail-loud
//! (control-engine slice-4 exit gate). Two REAL in-process `Node`s on one real Zenoh bus linked
//! point-to-point over loopback TCP (the `cross_node_routing_test.rs` deterministic pattern); node B
//! runs the REAL `control-engine` native sidecar (built `--features ce-fake`, armed by `LB_CE_FAKE=1`),
//! which lands in B's `lb_mcp::Registry` via the Tier-agnostic `SidecarDispatch` adapter and is served
//! to remote callers by B's `serve_ext`. Node A holds a `ce_appliance` record naming node B (the S4
//! artifact a discovery layer reads to populate the remote-routing entry — here `register_remote_extension`
//! stands in for that discovery, per the slice's kickoff note) and routes `control-engine.tree` to B.
//!
//! Proves:
//!   - **two-node routed `control-engine.tree`** — a ws-A caller on node A runs `control-engine.tree`;
//!     the appliance record points at node B; the call routes over Zenoh and B's native sidecar returns
//!     its seeded graph (the exit gate).
//!   - **offline fail-loud** — with node B's server dropped, the same routed call errors PROMPTLY and
//!     loudly (a `ToolError::Extension` "no node answered"), and NOTHING is queued: interactive graph
//!     commands are online request/response by decision — assert the workspace outbox stays empty.
//!
//! Naming the ext in a TEST is allowed (the ban is on CE strings in `src`).

use std::process::Command;
use std::time::Duration;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_bus::Bus;
use lb_host::{
    install_native, register_remote_extension, serve_ext, Node, Role as NodeRole, ToolServer,
};
use lb_mcp::{call, ToolError};
use lb_supervisor::OsLauncher;
use serde_json::{json, Value};

const MANIFEST: &str = include_str!("../../../extensions/control-engine/extension.toml");

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

fn admin(ws: &str) -> Principal {
    principal(
        ws,
        &[
            "mcp:native.install:call",
            "mcp:control-engine.tree:call",
            "store:ce_appliance:write",
        ],
    )
}

fn control_engine_dir() -> Option<String> {
    if let Ok(p) = std::env::var("CONTROL_ENGINE_BIN") {
        let dir = std::path::PathBuf::from(&p);
        return Some(dir.parent().unwrap().to_string_lossy().into_owned());
    }
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target = manifest_dir.join("../../target/debug");
    let bin = target.join("control-engine");
    let status = Command::new("cargo")
        .args(["build", "-p", "control-engine", "--features", "ce-fake"])
        .current_dir(manifest_dir.join("../.."))
        .status();
    match status {
        Ok(s) if s.success() && bin.exists() => Some(target.to_string_lossy().into_owned()),
        _ => None,
    }
}

async fn node_on_bus(bus: Bus, role: NodeRole) -> Node {
    Node::boot_on_bus(bus, role).await.expect("node boots")
}

/// Seed the S4 `ce_appliance` record on `node` (in `ws`) naming `owner` at `base` — the registry
/// artifact a discovery layer reads. Written straight through the real store (the same record the
/// `appliance.add` verb persists via `store.write`).
async fn seed_appliance(node: &Node, ws: &str, id: &str, owner: &str, base: &str) {
    let rec = json!({
        "id": id, "name": id, "mode": "appliance", "node": owner, "base": base, "ts": 1
    });
    lb_store::write(&node.store, ws, "ce_appliance", id, &rec)
        .await
        .expect("seed ce_appliance record");
}

async fn route_tree(
    edge: &Node,
    p: &Principal,
    ws: &str,
    appliance: &str,
) -> Result<Value, ToolError> {
    let input = json!({ "appliance": appliance }).to_string();
    call(
        &edge.registry,
        &edge.bus,
        p,
        ws,
        "control-engine.tree",
        &input,
    )
    .await
    .map(|out| serde_json::from_str(&out).unwrap())
}

/// Poll the routed call until node B's queryable is reachable (the readiness barrier), then return the
/// tree — the deterministic-link pattern from `cross_node_routing_test.rs`.
async fn route_until_reachable(edge: &Node, p: &Principal, ws: &str, appliance: &str) -> Value {
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    let mut last = None;
    while std::time::Instant::now() < deadline {
        match tokio::time::timeout(
            Duration::from_millis(500),
            route_tree(edge, p, ws, appliance),
        )
        .await
        {
            Ok(Ok(v)) => return v,
            Ok(Err(e)) => last = Some(format!("{e:?}")),
            Err(_) => last = Some("timed out (queryable not yet reachable)".into()),
        }
    }
    panic!("routed ce.tree never became reachable; last: {last:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn appliance_record_routes_ce_tree_to_node_b_and_offline_fails_loud() {
    std::env::set_var("LB_CE_FAKE", "1");
    let Some(dir) = control_engine_dir() else {
        eprintln!("SKIP appliance routing: could not build control-engine --features ce-fake");
        return;
    };

    let ws = "ce-route";
    let port = {
        let probe = std::net::TcpListener::bind("127.0.0.1:0").expect("free loopback port");
        probe.local_addr().unwrap().port()
    };
    let endpoint = format!("tcp/127.0.0.1:{port}");

    // Node B (hub): install the native CE sidecar (now ALSO registered into B's MCP registry via the
    // adapter) and serve it to remote callers.
    let hub_bus = Bus::peer_with(&[endpoint.clone()], &[])
        .await
        .expect("hub bus");
    let hub = node_on_bus(hub_bus, NodeRole::Hub).await;
    let admin = admin(ws);
    let approved = vec!["net:tcp:127.0.0.1:7979:connect".to_string()];
    install_native(&hub, &OsLauncher, &admin, ws, MANIFEST, &dir, &approved, 1)
        .await
        .expect("node B installs + spawns the native sidecar");
    let server: ToolServer = serve_ext(&hub.bus, hub.registry.clone(), "control-engine")
        .await
        .expect("node B serves the native ext");

    // Node A (edge): holds the appliance record (plant-1 → node B) and a remote routing entry for
    // control-engine (the discovery layer's job, stood in for here).
    let edge_bus = Bus::peer_with(&[], &[endpoint]).await.expect("edge bus");
    let edge = node_on_bus(edge_bus, NodeRole::Edge).await;
    seed_appliance(&edge, ws, "plant-1", "node-b", "http://127.0.0.1:7979").await;
    register_remote_extension(&edge, "control-engine", &["tree".to_string()]);

    // --- ROUTED: control-engine.tree naming plant-1 routes to node B and returns its seeded graph. ---
    let tree = route_until_reachable(&edge, &admin, ws, "plant-1").await;
    let nodes = tree["nodes"].as_array().expect("nodes array");
    assert_eq!(nodes.len(), 1, "node B's seeded native graph: {tree}");
    assert_eq!(nodes[0]["uid"], 1);
    assert_eq!(nodes[0]["type"], "test-math::add");

    // --- OFFLINE FAIL-LOUD: drop node B's server; the same routed call errors promptly + loudly, and
    //     nothing is queued (interactive request/response — no outbox rows). ---
    drop(server);
    drop(hub); // node B goes away entirely.

    let err = tokio::time::timeout(
        Duration::from_secs(15),
        route_tree(&edge, &admin, ws, "plant-1"),
    )
    .await
    .expect("offline routed call returns promptly (does not hang)")
    .expect_err("a routed call to an offline node fails loud");
    assert!(
        matches!(err, ToolError::Extension(_)),
        "loud transport error, not a silent success: {err:?}"
    );

    // Nothing queued: the interactive graph command is online-only (control-engine scope decision). The
    // workspace outbox has no rows — the call did not enqueue a retry.
    let pending = lb_store::scan(&edge.store, ws, "outbox", 50, None)
        .await
        .expect("scan outbox");
    assert!(
        pending.rows.is_empty(),
        "no outbox rows queued for an interactive routed call: {:?}",
        pending.rows
    );
}
