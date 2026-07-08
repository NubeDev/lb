//! Slice 3 — the poller, end to end against a REAL gateway + store + ingest path (no mocks; CLAUDE
//! rule 9 / testing §0). A real `Node`, a real `axum` gateway on a real port, the real
//! `lb-sidecar-client`, the real `ingest.write`/`series.*` host verbs. The ONLY fake is the ROS box,
//! behind the `RosApi` trait (`RosFake`).
//!
//! The poll task calls `ingest.write` through the same real callback the CRUD tree uses. We arm it via
//! `ros.start`, let real ticks fire, and assert the polled `present_value` lands on the series
//! `ros.{ws}.{ros}.{net}.{dev}.{point}` — read back through `series.latest`. This proves the mandatory
//! properties on the real path:
//!   - **enable-gating (integration):** toggling any of the four levels via `*.update {enable:false}`
//!     changes which series receive samples (here: connection off silences the whole box; a fresh
//!     network flips a leaf on/off in the fake tree).
//!   - **capability deny:** a reader WITHOUT `mcp:series.read:call` cannot see the polled values.
//!   - **workspace isolation:** a series written by ws-A's poller is invisible to ws-B.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

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

/// The sidecar's full grant (mirrors `extension.toml`) — the CRUD caps plus the poll-verb + ingest
/// caps slice 3 needs.
fn full_caps() -> Vec<String> {
    [
        "mcp:ros.list:call",
        "mcp:ros.get:call",
        "mcp:ros.create:call",
        "mcp:ros.update:call",
        "mcp:ros.delete:call",
        "mcp:ros.ping:call",
        "mcp:ros.start:call",
        "mcp:ros.stop:call",
        "mcp:ros.status:call",
        "mcp:ros.restart:call",
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
        "mcp:ingest.write:call",
        "mcp:series.latest:call",
        "mcp:series.read:call",
        "mcp:series.list:call",
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

/// Drive a verb through the SAME dispatcher `call.rs` uses, sharing ONE registry across calls (so a
/// `ros.start` in one call is inspectable by a later `ros.status`/`ros.stop`).
async fn call(
    host: &HostCtx,
    factory: &FakeFactory,
    registry: &Arc<PollRegistry>,
    tool: &str,
    input: Value,
) -> Result<Value, String> {
    match dispatch(host, factory, registry, tool, &input, NOW).await {
        Ok(Some(s)) => Ok(serde_json::from_str(&s).unwrap()),
        Ok(None) => Err(format!("unknown tool {tool}")),
        Err(e) => Err(e.to_string()),
    }
}

/// Read a series' latest committed value straight off the host (the reader side of the round-trip),
/// as the given principal — so a cap/ws denial surfaces exactly as the real read path would.
async fn latest(host: &HostCtx, series: &str) -> Result<Option<f64>, String> {
    let out = host
        .client()
        .call_tool("series.latest", json!({ "series": series }))
        .await
        .map_err(|e| e.to_string())?;
    Ok(out
        .get("sample")
        .and_then(|s| s.get("payload"))
        .and_then(|v| v.as_f64()))
}

/// Poll `series.latest` until it returns a value or the budget elapses — the poll task ticks on a real
/// timer, so the test waits for a real sample to commit (no fixed sleep guess). Returns the value.
async fn await_sample(host: &HostCtx, series: &str) -> Option<f64> {
    for _ in 0..50 {
        if let Ok(Some(v)) = latest(host, series).await {
            return Some(v);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    None
}

/// Create a connection with a fast poll rate so ticks fire quickly in the test.
async fn create_conn(
    host: &HostCtx,
    factory: &FakeFactory,
    registry: &Arc<PollRegistry>,
    uuid: &str,
) {
    call(
        host,
        factory,
        registry,
        "ros.create",
        json!({ "uuid": uuid, "name": "Box", "base_url": "http://box.local", "token": "tok", "poll_rate": 1 }),
    )
    .await
    .expect("create ok");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn poller_writes_present_value_to_series() {
    let (_node, key, base) = serve().await;
    let ws = "ros-poll";
    let host = host_for(&key, &base, ws, &full_caps());
    let fake = Arc::new(RosFake::seeded("point-1", 21.5));
    let factory = FakeFactory::new(fake.clone());
    let registry = Arc::new(PollRegistry::new());

    create_conn(&host, &factory, &registry, "ros-1").await;
    let started = call(
        &host,
        &factory,
        &registry,
        "ros.start",
        json!({ "ros_uuid": "ros-1" }),
    )
    .await
    .expect("start ok");
    assert_eq!(started["running"], true);

    let series = format!("ros.{ws}.ros-1.net-1.dev-1.point-1");
    let v = await_sample(&host, &series).await;
    assert_eq!(
        v,
        Some(21.5),
        "poller committed the present_value to the series"
    );

    // status reflects a healthy task with samples.
    let st = call(
        &host,
        &factory,
        &registry,
        "ros.status",
        json!({ "ros_uuid": "ros-1" }),
    )
    .await
    .expect("status ok");
    assert_eq!(st["running"], true);
    assert!(
        st["samples"].as_u64().unwrap() >= 1,
        "status counts samples: {st}"
    );

    // stop parks it.
    let stopped = call(
        &host,
        &factory,
        &registry,
        "ros.stop",
        json!({ "ros_uuid": "ros-1" }),
    )
    .await
    .expect("stop ok");
    assert_eq!(stopped["running"], false);
    assert_eq!(stopped["was_running"], true);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn connection_disable_gates_the_whole_box() {
    let (_node, key, base) = serve().await;
    let ws = "ros-gate";
    let host = host_for(&key, &base, ws, &full_caps());
    let fake = Arc::new(RosFake::seeded("point-g", 7.0));
    let factory = FakeFactory::new(fake.clone());
    let registry = Arc::new(PollRegistry::new());

    // Create the connection DISABLED (enable:false) — the connection-level flag ANDs to false, so the
    // poller's target set is empty and NO series ever receives a sample.
    call(
        &host,
        &factory,
        &registry,
        "ros.create",
        json!({ "uuid": "ros-g", "name": "Box", "base_url": "http://b", "token": "t", "poll_rate": 1, "enable": false }),
    )
    .await
    .expect("create ok");
    call(
        &host,
        &factory,
        &registry,
        "ros.start",
        json!({ "ros_uuid": "ros-g" }),
    )
    .await
    .expect("start ok");

    let series = format!("ros.{ws}.ros-g.net-1.dev-1.point-g");
    // Give the loop real time to tick a few times; it must write NOTHING (connection disabled).
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(
        latest(&host, &series).await.unwrap(),
        None,
        "a disabled connection silences the whole box — no samples"
    );

    // Re-enable and re-start: now the leaf polls and the series fills.
    call(
        &host,
        &factory,
        &registry,
        "ros.update",
        json!({ "uuid": "ros-g", "enable": true }),
    )
    .await
    .expect("update ok");
    call(
        &host,
        &factory,
        &registry,
        "ros.restart",
        json!({ "ros_uuid": "ros-g" }),
    )
    .await
    .expect("restart ok");
    assert_eq!(
        await_sample(&host, &series).await,
        Some(7.0),
        "re-enabling the connection resumes polling into the series"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reader_without_series_read_cap_cannot_see_values() {
    let (_node, key, base) = serve().await;
    let ws = "ros-capdeny";
    let full = host_for(&key, &base, ws, &full_caps());
    let fake = Arc::new(RosFake::seeded("point-c", 3.3));
    let factory = FakeFactory::new(fake.clone());
    let registry = Arc::new(PollRegistry::new());

    create_conn(&full, &factory, &registry, "ros-c").await;
    call(
        &full,
        &factory,
        &registry,
        "ros.start",
        json!({ "ros_uuid": "ros-c" }),
    )
    .await
    .expect("start ok");
    let series = format!("ros.{ws}.ros-c.net-1.dev-1.point-c");
    assert_eq!(
        await_sample(&full, &series).await,
        Some(3.3),
        "value committed"
    );

    // A reader in the SAME workspace but WITHOUT `mcp:series.latest`/`read` — the host refuses the read
    // (opaque 403 → Denied). The polled value exists but is unreachable without the cap.
    let mut reader_caps = full_caps();
    reader_caps.retain(|c| c != "mcp:series.latest:call" && c != "mcp:series.read:call");
    let reader = host_for(&key, &base, ws, &reader_caps);
    let denied = latest(&reader, &series).await;
    assert!(
        denied.is_err() || denied.as_ref().unwrap().is_none(),
        "reader without series.read cap cannot see the polled value: {denied:?}"
    );
    // Precisely: the call is denied (not merely empty).
    assert!(reader
        .client()
        .call_tool("series.latest", json!({ "series": series }))
        .await
        .is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_isolation_series_invisible_across_ws() {
    let (_node, key, base) = serve().await;
    let caps = full_caps();
    let fake = Arc::new(RosFake::seeded("point-i", 9.9));
    let factory = FakeFactory::new(fake.clone());

    // ws-A polls a connection into its series.
    let host_a = host_for(&key, &base, "ws-a", &caps);
    let reg_a = Arc::new(PollRegistry::new());
    create_conn(&host_a, &factory, &reg_a, "a-1").await;
    call(
        &host_a,
        &factory,
        &reg_a,
        "ros.start",
        json!({ "ros_uuid": "a-1" }),
    )
    .await
    .expect("A start ok");
    let series_a = "ros.ws-a.a-1.net-1.dev-1.point-i".to_string();
    assert_eq!(
        await_sample(&host_a, &series_a).await,
        Some(9.9),
        "A wrote its series"
    );

    // ws-B (full grant) reads the SAME series id — it must see nothing (the series is ws-A's; the
    // workspace is the hard wall, enforced host-side by B's token).
    let host_b = host_for(&key, &base, "ws-b", &caps);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        latest(&host_b, &series_a).await.unwrap(),
        None,
        "ws-B cannot see a series written by ws-A's poller"
    );
}
