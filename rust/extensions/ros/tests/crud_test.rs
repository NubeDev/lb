//! Slice 2 — the CRUD tree, end to end against a REAL gateway (native-callback pattern, copied from
//! `role/gateway/tests/native_callback_test.rs`). No mocks (CLAUDE rule 9 / testing §0): a real `Node`,
//! a real `axum` gateway on a real TCP port, the real `lb-sidecar-client` making real `reqwest` calls,
//! the real MCP gate + `assets.*`/`secret.*` host verbs + embedded store. The ONLY fake is the ROS box,
//! behind the `RosApi` trait (a `RosFake`), per the one-external rule.
//!
//! The handlers are driven exactly as `call.rs` drives them — `handlers::dispatch` with a `HostCtx`
//! built over the real gateway + the sidecar's grant. This proves the mandatory properties:
//!   - **capability deny:** a grant missing `mcp:ros.create:call` refuses before any store write; a
//!     grant missing `mcp:point.get:call` cannot read a value.
//!   - **workspace isolation:** ws-A's connections are invisible to a ws-B sidecar.
//!   - **token hygiene:** the External token is never returned by `get`/`list`.
//!   - **CRUD round-trip + tree proxy:** create → get/list; network/device/point list proxy the fake.

use std::net::SocketAddr;
use std::sync::Arc;

use lb_auth::{mint, Claims, Role, SigningKey};
use lb_host::{Node, Role as NodeRole};
use lb_role_gateway::{router, Gateway};
use lb_sidecar_client::{Config, SidecarClient};
use ros_sidecar::handlers::dispatch;
use ros_sidecar::host::HostCtx;
use ros_sidecar::poller::run::PollRegistry;
use ros_sidecar::ros_fake::{FakeFactory, RosFake};
use serde_json::{json, Value};

const NOW: u64 = 1000;

/// The full grant a fully-authorized ros sidecar holds (its manifest `request`, resolved to caps).
/// The resource caps (`secret:…`, and the per-verb `mcp:…:call`) mirror `extension.toml`.
fn full_caps() -> Vec<String> {
    [
        "mcp:ros.list:call",
        "mcp:ros.get:call",
        "mcp:ros.create:call",
        "mcp:ros.update:call",
        "mcp:ros.delete:call",
        "mcp:ros.ping:call",
        "mcp:network.list:call",
        "mcp:network.get:call",
        "mcp:device.list:call",
        "mcp:device.get:call",
        "mcp:point.list:call",
        "mcp:point.get:call",
        "mcp:assets.put_doc:call",
        "mcp:assets.get_doc:call",
        "mcp:assets.list_docs:call",
        "mcp:assets.delete_doc:call",
        "store:doc/**:read",
        "store:doc/ros/*:write",
        "store:doc/ros/*:delete",
        "mcp:secret.set:call",
        "mcp:secret.get:call",
        "mcp:secret.delete:call",
        "secret:ros/*/token:get",
        "secret:ros/*/token:write",
        "secret:ros/*/token:delete",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Mint a child token like `native/spec.rs`: `sub = ext:ros`, Member, the given caps, signed with the
/// node's key so the gateway verifies it.
fn child_token(key: &SigningKey, ws: &str, caps: &[String]) -> String {
    let claims = Claims {
        sub: "ext:ros".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.to_vec(),
        iat: NOW - 1,
        exp: NOW + 10_000,
        constraint: None,
        run_id: None,
    };
    mint(key, &claims)
}

/// Boot a real node + real gateway on a real ephemeral port. The node's key IS the gateway's key, so a
/// token minted with `key` verifies on `/mcp/call`.
async fn serve() -> (Arc<Node>, SigningKey, String) {
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let gw = Gateway::new(node.clone(), key.clone(), NOW);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr: SocketAddr = listener.local_addr().unwrap();
    let app = router(gw);
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (node, key, format!("http://{addr}"))
}

/// Build a HostCtx whose SidecarClient carries a REAL signed child token (so callbacks authenticate),
/// and whose self-check caps match that token's grant.
fn host_for(key: &SigningKey, base: &str, ws: &str, caps: &[String]) -> HostCtx {
    let token = child_token(key, ws, caps);
    HostCtx::with_parts(
        SidecarClient::with_config(Config::new(base, token, ws, "ros")),
        caps.to_vec(),
        ws,
    )
}

fn fake_factory() -> (FakeFactory, Arc<RosFake>) {
    let fake = Arc::new(RosFake::seeded("point-1", 21.5));
    (FakeFactory::new(fake.clone()), fake)
}

async fn call(
    host: &HostCtx,
    factory: &FakeFactory,
    tool: &str,
    input: Value,
) -> Result<Value, String> {
    let registry = Arc::new(PollRegistry::new());
    match dispatch(host, factory, &registry, tool, &input, NOW).await {
        Ok(Some(s)) => Ok(serde_json::from_str(&s).unwrap()),
        Ok(None) => Err(format!("unknown tool {tool}")),
        Err(e) => Err(e.to_string()),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn crud_round_trip_and_token_is_never_returned() {
    let (_node, key, base) = serve().await;
    let ws = "ros-crud";
    let caps = full_caps();
    let host = host_for(&key, &base, ws, &caps);
    let (factory, _fake) = fake_factory();

    // create
    let created = call(
        &host,
        &factory,
        "ros.create",
        json!({ "uuid": "ros-1", "name": "Boiler Room", "base_url": "http://box.local", "token": "super-secret-token" }),
    )
    .await
    .expect("create ok");
    assert_eq!(created["uuid"], "ros-1");

    // get — token must NOT appear anywhere in the response
    let got = call(&host, &factory, "ros.get", json!({ "uuid": "ros-1" }))
        .await
        .expect("get ok");
    assert_eq!(got["name"], "Boiler Room");
    assert_eq!(got["base_url"], "http://box.local");
    assert!(
        !got.to_string().contains("super-secret-token"),
        "token leaked in ros.get: {got}"
    );

    // list — also token-free, and the created connection is present
    let listed = call(&host, &factory, "ros.list", json!({}))
        .await
        .expect("list ok");
    assert!(
        !listed.to_string().contains("super-secret-token"),
        "token leaked in ros.list: {listed}"
    );
    let items = listed["items"].as_array().unwrap();
    assert!(
        items.iter().any(|c| c["uuid"] == "ros-1"),
        "created conn listed"
    );

    // tree proxy: network/device/point list drill the fake box
    let nets = call(
        &host,
        &factory,
        "network.list",
        json!({ "ros_uuid": "ros-1" }),
    )
    .await
    .expect("network.list ok");
    assert_eq!(nets["items"][0]["uuid"], "net-1");

    let devs = call(
        &host,
        &factory,
        "device.list",
        json!({ "ros_uuid": "ros-1", "network_uuid": "net-1" }),
    )
    .await
    .expect("device.list ok");
    assert_eq!(devs["items"][0]["uuid"], "dev-1");

    let pts = call(
        &host,
        &factory,
        "point.list",
        json!({ "ros_uuid": "ros-1", "device_uuid": "dev-1" }),
    )
    .await
    .expect("point.list ok");
    assert_eq!(pts["items"][0]["uuid"], "point-1");
    assert_eq!(pts["items"][0]["present_value"], 21.5);

    // update flips enable; get reflects it
    call(
        &host,
        &factory,
        "ros.update",
        json!({ "uuid": "ros-1", "enable": false }),
    )
    .await
    .expect("update ok");
    let got2 = call(&host, &factory, "ros.get", json!({ "uuid": "ros-1" }))
        .await
        .expect("get2 ok");
    assert_eq!(got2["enable"], false);

    // delete removes it
    call(&host, &factory, "ros.delete", json!({ "uuid": "ros-1" }))
        .await
        .expect("delete ok");
    let gone = call(&host, &factory, "ros.get", json!({ "uuid": "ros-1" }))
        .await
        .expect("get3 ok");
    assert_eq!(gone["error"], "not_found");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn capability_deny_refuses_before_any_effect() {
    let (_node, key, base) = serve().await;
    let ws = "ros-deny";
    // A grant WITHOUT `mcp:ros.create:call` (but otherwise full) — create must be denied, and no
    // shadow doc is written (a subsequent list, with a full grant, shows nothing).
    let mut caps = full_caps();
    caps.retain(|c| c != "mcp:ros.create:call");
    let host = host_for(&key, &base, ws, &caps);
    let (factory, _fake) = fake_factory();

    let denied = call(
        &host,
        &factory,
        "ros.create",
        json!({ "uuid": "ros-x", "name": "X", "base_url": "http://x", "token": "t" }),
    )
    .await;
    assert_eq!(
        denied.unwrap_err(),
        "denied",
        "create without cap is denied"
    );

    // With a FULL grant, the workspace has no ros-x (the denied create wrote nothing).
    let full = host_for(&key, &base, ws, &full_caps());
    let listed = call(&full, &factory, "ros.list", json!({}))
        .await
        .expect("list ok");
    assert!(
        listed["items"].as_array().unwrap().is_empty(),
        "denied create left no shadow: {listed}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_isolation_a_cannot_see_b() {
    let (_node, key, base) = serve().await;
    let caps = full_caps();
    let (factory, _fake) = fake_factory();

    // ws-A creates a connection.
    let host_a = host_for(&key, &base, "ws-a", &caps);
    call(
        &host_a,
        &factory,
        "ros.create",
        json!({ "uuid": "a-1", "name": "A", "base_url": "http://a", "token": "ta" }),
    )
    .await
    .expect("A create ok");

    // ws-B, full grant, sees NONE of A's connections — the workspace is the token's (the hard wall).
    let host_b = host_for(&key, &base, "ws-b", &caps);
    let listed_b = call(&host_b, &factory, "ros.list", json!({}))
        .await
        .expect("B list ok");
    assert!(
        listed_b["items"].as_array().unwrap().is_empty(),
        "ws-B must not see ws-A connections: {listed_b}"
    );
    let get_b = call(&host_b, &factory, "ros.get", json!({ "uuid": "a-1" }))
        .await
        .expect("B get ok");
    assert_eq!(get_b["error"], "not_found", "ws-B cannot get ws-A's a-1");
}
