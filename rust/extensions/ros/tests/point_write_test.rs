//! Slice 4 — `point.write` (must-deliver setpoint → outbox) + the sidecar relay, end to end against a
//! REAL gateway + store + outbox (no mocks; CLAUDE rule 9 / testing §0). A real `Node`, a real `axum`
//! gateway on a real port, the real `lb-sidecar-client`, the real `outbox.enqueue`/`due`/`mark_*` host
//! verbs. The ONLY fake is the ROS box, behind `RosApi` (`RosFake`) — and its `writes()` recorder is
//! what proves a setpoint actually reached "the box".
//!
//! Proves the must-deliver contract on the real path:
//!   - **staged, not inline:** `point.write` enqueues an effect (visible via `outbox.status`) and NO
//!     REST write leaves the node until the relay runs.
//!   - **relay delivers:** `relay_pass` pulls the due `ros` effect, delivers it through `RosTarget`,
//!     the fake box records the PATCH, and the effect is marked delivered (not due again).
//!   - **retry, not drop:** a box-unreachable delivery leaves the effect schedulable (retried), and a
//!     later pass against a recovered box delivers it — the setpoint is never lost.
//!   - **capability deny:** without `mcp:point.write:call`, the write is refused before any enqueue.
//!   - **workspace isolation:** ws-A's setpoint effect is invisible to ws-B's relay.

use std::net::SocketAddr;
use std::sync::Arc;

use lb_auth::{mint, Claims, Role, SigningKey};
use lb_host::{Node, Role as NodeRole};
use lb_role_gateway::{router, Gateway};
use lb_sidecar_client::{Config, SidecarClient};
use ros_sidecar::handlers::dispatch;
use ros_sidecar::host::HostCtx;
use ros_sidecar::poller::relay::relay_pass;
use ros_sidecar::poller::run::PollRegistry;
use ros_sidecar::ros_fake::{FakeFactory, RosFake};
use serde_json::{json, Value};

const NOW: u64 = 1000;

fn full_caps() -> Vec<String> {
    [
        "mcp:ros.create:call",
        "mcp:ros.get:call",
        "mcp:point.write:call",
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
        "mcp:outbox.enqueue:call",
        "mcp:outbox.status:call",
        "mcp:outbox.due:call",
        "mcp:outbox.mark_delivered:call",
        "mcp:outbox.mark_failed:call",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

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

fn host_for(key: &SigningKey, base: &str, ws: &str, caps: &[String]) -> HostCtx {
    let token = child_token(key, ws, caps);
    HostCtx::with_parts(
        SidecarClient::with_config(Config::new(base, token, ws, "ros")),
        caps.to_vec(),
        ws,
    )
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

/// How many effects the workspace has staged pending (via the real `outbox.status`).
async fn pending_count(host: &HostCtx) -> usize {
    let out = host
        .client()
        .call_tool("outbox.status", json!({}))
        .await
        .expect("status ok");
    out.get("pending")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0)
}

async fn create_conn(host: &HostCtx, factory: &FakeFactory, uuid: &str) {
    call(
        host,
        factory,
        "ros.create",
        json!({ "uuid": uuid, "name": "Box", "base_url": "http://box.local", "token": "tok" }),
    )
    .await
    .expect("create ok");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn point_write_stages_then_relay_delivers_to_box() {
    let (_node, key, base) = serve().await;
    let ws = "ros-write";
    let host = host_for(&key, &base, ws, &full_caps());
    let fake = Arc::new(RosFake::seeded("point-1", 20.0));
    let factory = FakeFactory::new(fake.clone());

    create_conn(&host, &factory, "ros-1").await;

    // Stage the setpoint. NO write reaches the box yet (must-deliver → outbox, not inline).
    let staged = call(
        &host,
        &factory,
        "point.write",
        json!({ "ros_uuid": "ros-1", "point_uuid": "point-1", "slot": 8, "value": 21.5 }),
    )
    .await
    .expect("write ok");
    assert_eq!(staged["status"], "pending");
    assert!(
        fake.writes().is_empty(),
        "no REST write leaves the node at enqueue time"
    );
    assert_eq!(pending_count(&host).await, 1, "one effect staged pending");

    // Run the relay: it pulls the due `ros` effect, delivers it, and marks it delivered.
    let pass = relay_pass(&host, &factory, NOW + 10)
        .await
        .expect("relay ok");
    assert_eq!(pass.delivered, 1, "relay delivered the setpoint: {pass:?}");

    let writes = fake.writes();
    assert_eq!(writes.len(), 1, "exactly one PATCH reached the box");
    assert_eq!(writes[0].point_uuid, "point-1");
    assert_eq!(writes[0].slot, 8);
    assert_eq!(writes[0].value, Some(21.5));

    // The effect is terminal — a second relay pass delivers nothing (never double-sent).
    let pass2 = relay_pass(&host, &factory, NOW + 20)
        .await
        .expect("relay2 ok");
    assert_eq!(
        pass2,
        Default::default(),
        "delivered effect not re-sent: {pass2:?}"
    );
    assert_eq!(fake.writes().len(), 1, "box not written twice");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unreachable_box_retries_then_delivers() {
    let (_node, key, base) = serve().await;
    let ws = "ros-retry";
    let host = host_for(&key, &base, ws, &full_caps());
    let fake = Arc::new(RosFake::seeded("point-r", 0.0));
    let factory = FakeFactory::new(fake.clone());
    create_conn(&host, &factory, "ros-r").await;

    call(
        &host,
        &factory,
        "point.write",
        json!({ "ros_uuid": "ros-r", "point_uuid": "point-r", "slot": 5, "value": 42.0 }),
    )
    .await
    .expect("write ok");

    // Box is down: the relay pass retries (schedulable), does NOT deliver, and the box records nothing.
    fake.set_unreachable(true);
    let down = relay_pass(&host, &factory, NOW + 10)
        .await
        .expect("relay ok");
    assert_eq!(down.retried, 1, "down box → retried, not dropped: {down:?}");
    assert!(
        fake.writes().is_empty(),
        "nothing written while the box is down"
    );

    // Box recovers; a later pass (past the backoff) delivers the same effect — never lost.
    fake.set_unreachable(false);
    let up = relay_pass(&host, &factory, NOW + 1_000)
        .await
        .expect("relay ok");
    assert_eq!(
        up.delivered, 1,
        "recovered box → the setpoint lands: {up:?}"
    );
    let writes = fake.writes();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].value, Some(42.0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn write_without_cap_is_denied_before_enqueue() {
    let (_node, key, base) = serve().await;
    let ws = "ros-wdeny";
    let full = host_for(&key, &base, ws, &full_caps());
    let fake = Arc::new(RosFake::seeded("point-1", 1.0));
    let factory = FakeFactory::new(fake.clone());
    create_conn(&full, &factory, "ros-1").await;

    // A grant WITHOUT mcp:point.write:call — the write is refused before any enqueue.
    let mut caps = full_caps();
    caps.retain(|c| c != "mcp:point.write:call");
    let denied_host = host_for(&key, &base, ws, &caps);
    let denied = call(
        &denied_host,
        &factory,
        "point.write",
        json!({ "ros_uuid": "ros-1", "point_uuid": "point-1", "slot": 3, "value": 9.0 }),
    )
    .await;
    assert_eq!(denied.unwrap_err(), "denied", "write without cap is denied");

    // No effect staged (a full-grant status shows nothing pending), and no box write.
    assert_eq!(
        pending_count(&full).await,
        0,
        "denied write staged no effect"
    );
    assert!(
        fake.writes().is_empty(),
        "no REST write on a denied setpoint"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_isolation_setpoint_invisible_to_b() {
    let (_node, key, base) = serve().await;
    let caps = full_caps();
    let fake = Arc::new(RosFake::seeded("point-i", 5.0));
    let factory = FakeFactory::new(fake.clone());

    // ws-A stages a setpoint.
    let host_a = host_for(&key, &base, "ws-a", &caps);
    create_conn(&host_a, &factory, "a-1").await;
    call(
        &host_a,
        &factory,
        "point.write",
        json!({ "ros_uuid": "a-1", "point_uuid": "point-i", "slot": 1, "value": 5.0 }),
    )
    .await
    .expect("A write ok");

    // ws-B's relay (full grant) sees NO due effects — the outbox is walled by workspace (§7).
    let host_b = host_for(&key, &base, "ws-b", &caps);
    let pass_b = relay_pass(&host_b, &factory, NOW + 10)
        .await
        .expect("B relay ok");
    assert_eq!(
        pass_b,
        Default::default(),
        "ws-B relay sees none of ws-A's effects: {pass_b:?}"
    );
    assert!(
        fake.writes().is_empty(),
        "ws-B relay delivered nothing to the box"
    );
}
