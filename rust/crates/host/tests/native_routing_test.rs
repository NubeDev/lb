//! CORE proof: a **native (Tier-2) sidecar** is reachable over the routed cross-node MCP hop —
//! the same seam `cross_node_routing_test.rs` proves for a wasm ext, now Tier-agnostic. A native
//! sidecar installed on node B lands in B's `lb_mcp::Registry` (via the `SidecarDispatch` adapter),
//! so B's `serve_ext`/`serve_call` answer a routed call from node A against its own native child —
//! with ZERO Tier knowledge in the call path (§3.1).
//!
//! This is generic platform infra; it merely REUSES the control-engine sidecar (built with
//! `--features ce-fake`, armed by `LB_CE_FAKE=1`) as a real native child to exercise the path — the
//! one sanctioned true-external, the same fixture `control_engine_test.rs` uses. Naming the ext in a
//! TEST is allowed (the ban is on CE strings in `src`). No mocks: real embedded SurrealDB, two real
//! in-proc Zenoh peers linked point-to-point over loopback TCP (the deterministic-discovery pattern
//! from `cross_node_routing_test.rs`), the real supervisor spawning the real sidecar binary.

use std::process::Command;
use std::time::Duration;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_bus::Bus;
use lb_host::{
    install_native, register_remote_extension, serve_ext, Node, Role as NodeRole, ToolServer,
};
use lb_mcp::call;
use lb_supervisor::OsLauncher;

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
        &["mcp:native.install:call", "mcp:control-engine.tree:call"],
    )
}

/// Build the sidecar with `--features ce-fake` and return the dir holding the binary (or `None` if
/// the build fails — then the test SKIPs, exactly like `control_engine_test.rs`).
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

/// Poll the routed call until node B's queryable is reachable, then return the parsed output — the
/// deterministic-link + readiness-barrier pattern from `cross_node_routing_test.rs`.
async fn route_until_reachable(
    edge: &Node,
    p: &Principal,
    ws: &str,
    tool: &str,
    input_json: &str,
) -> String {
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    let mut last_err = None;
    while std::time::Instant::now() < deadline {
        match tokio::time::timeout(
            Duration::from_millis(500),
            call(&edge.registry, &edge.bus, p, ws, tool, input_json),
        )
        .await
        {
            Ok(Ok(out)) => return out,
            Ok(Err(e)) => last_err = Some(format!("{e:?}")),
            Err(_) => last_err = Some("attempt timed out (queryable not yet reachable)".into()),
        }
    }
    panic!("routed native call never became reachable; last: {last_err:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_call_on_node_a_routes_to_a_native_sidecar_on_node_b() {
    std::env::set_var("LB_CE_FAKE", "1");
    let Some(dir) = control_engine_dir() else {
        eprintln!("SKIP a_call_on_node_a_routes_to_a_native_sidecar_on_node_b: could not build control-engine --features ce-fake");
        return;
    };

    let ws = "native-xnode";

    // Deterministic loopback link (see cross_node_routing_test.rs module doc).
    let port = {
        let probe = std::net::TcpListener::bind("127.0.0.1:0").expect("free loopback port");
        probe.local_addr().unwrap().port()
    };
    let endpoint = format!("tcp/127.0.0.1:{port}");

    // Node B (hub): install the native sidecar — install now ALSO registers it into B's MCP
    // registry via the SidecarDispatch adapter — then serve it to remote callers.
    let hub_bus = Bus::peer_with(&[endpoint.clone()], &[])
        .await
        .expect("hub bus");
    let hub = node_on_bus(hub_bus, NodeRole::Hub).await;
    let admin = admin(ws);
    let approved = vec!["net:tcp:127.0.0.1:7979:connect".to_string()];
    install_native(&hub, &OsLauncher, &admin, ws, MANIFEST, &dir, &approved, 1)
        .await
        .expect("node B installs + spawns the native sidecar");
    let _server: ToolServer = serve_ext(&hub.bus, hub.registry.clone(), "control-engine")
        .await
        .expect("node B serves the native ext");

    // Node A (edge): knows control-engine lives elsewhere — a remote routing entry, no local child.
    let edge_bus = Bus::peer_with(&[], &[endpoint]).await.expect("edge bus");
    let edge = node_on_bus(edge_bus, NodeRole::Edge).await;
    register_remote_extension(&edge, "control-engine", &["tree".to_string()]);

    // A routed control-engine.tree from node A returns node B's native sidecar's seeded graph — the
    // call site is IDENTICAL to a local call; dispatch routes over the bus to B, whose serve_call
    // reaches its native child through the Tier-agnostic adapter.
    let out = route_until_reachable(
        &edge,
        &admin,
        ws,
        "control-engine.tree",
        r#"{"appliance":"127.0.0.1:7979"}"#,
    )
    .await;

    let tree: serde_json::Value = serde_json::from_str(&out).expect("tree json");
    let nodes = tree["nodes"].as_array().expect("nodes array");
    assert_eq!(nodes.len(), 1, "node B's seeded native graph: {tree}");
    assert_eq!(nodes[0]["uid"], 1);
    assert_eq!(nodes[0]["type"], "test-math::add");
}
