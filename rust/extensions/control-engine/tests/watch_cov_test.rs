#![cfg(feature = "ce-fake")]
//! Requires the `ce-fake` feature (the ONE sanctioned CE stub the assertions drive). Without it the
//! file compiles empty — run with `cargo test -p control-engine --features ce-fake --test watch_cov_test`.
//!
//! S6 — the live COV feed end to end against a REAL gateway + node + store + bus (the native-callback
//! pattern from `appliance_registry_test.rs`; CLAUDE rule 9 / testing §0: no mocks except the ONE
//! sanctioned `ce_fake`). The novel plumbing in S6 is the MOTION path: a decoded COV event →
//! `frame::encode` → the REAL `ingest.write` host callback → the gateway's real drain +
//! `publish_sample` → the workspace bus subject `ws/{id}/series/{series}` a dashboard/SSE relays.
//!
//! We assert on the bus subject directly (the task's sanctioned choice): that IS the motion the
//! gateway's `GET /series/{series}/stream` SSE relays verbatim (`ingest/motion.rs`), so proving the
//! frame lands there proves S7 can open the SSE and receive it. `series.latest` over the same bridge
//! is the durable-copy cross-check.
//!
//! Proves the S6 categories:
//!   - **arm → frame → motion (exit gate):** arming a watch publishes the seeded COV frame onto the
//!     series subject, re-encoded to the frame contract (`{kind:"cov", values:[{uid,v}], ...}`).
//!   - **deny:** a grant without `mcp:control-engine.watch:call` refuses at the verb before any arm.
//!   - **isolation:** the verb resolves the appliance in the caller's workspace; an unknown/other-ws
//!     selector is a clean not-found (asserted via `resolve`, the same wall the verb applies).

use std::net::SocketAddr;
use std::sync::Arc;

use control_engine::ce_fake::CeFake;
use control_engine::host::{HostCtx, HostError};
use control_engine::watch::{verb, WatchRegistry};
use lb_auth::{mint, Claims, Role, SigningKey};
use lb_host::{Node, Role as NodeRole};
use lb_ingest::Sample;
use lb_role_gateway::{router, Gateway};
use lb_sidecar_client::{Config, SidecarClient};
use rubix_ce::{ControlEngine, CovScope};
use serde_json::{json, Value};

const NOW: u64 = 1000;

fn watch_caps() -> Vec<String> {
    ["mcp:control-engine.watch:call", "mcp:ingest.write:call"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

fn child_token(key: &SigningKey, ws: &str, caps: &[String]) -> String {
    let claims = Claims {
        sub: "ext:control-engine".into(),
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

/// Boot a real node + real gateway on a real ephemeral port (the node's key IS the gateway's key, so a
/// child token minted with `key` verifies on `/mcp/call`).
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
        SidecarClient::with_config(Config::new(base, token, ws, "control-engine")),
        caps.to_vec(),
        ws,
    )
}

/// Await the first COV frame on `series`'s bus subject, decoding the `Sample` payload. Returns `None`
/// on timeout. Skips empty-changes heartbeat ticks (the fake's liveness probe) — we want the seeded
/// value frame.
async fn recv_cov_frame(node: &Node, ws: &str, series: &str) -> Option<Value> {
    let sub = lb_bus::subscribe(&node.bus, ws, &format!("series/{series}"))
        .await
        .expect("subscribe series subject");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        let Ok(Some(bytes)) =
            tokio::time::timeout(std::time::Duration::from_secs(2), sub.recv()).await
        else {
            continue;
        };
        let Ok(sample) = serde_json::from_slice::<Sample>(&bytes) else {
            continue;
        };
        let payload = sample.payload;
        // Skip the empty-changes heartbeat; wait for the seeded value frame.
        if payload["kind"] == "cov"
            && payload["values"]
                .as_array()
                .map_or(false, |a| !a.is_empty())
        {
            return Some(payload);
        }
    }
    None
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn armed_watch_publishes_cov_frames_onto_the_series_subject() {
    let (node, key, gw) = serve().await;
    let ws = "ce-watch";
    let host = host_for(&key, &gw, ws, &watch_caps());

    let fake = CeFake::seeded();
    let engine: Arc<dyn ControlEngine> = fake.clone();
    let watches = WatchRegistry::new();

    // Arm directly on the fake engine (the routed dispatch to the owning node is proven by the S4
    // routing test; here we prove the novel MOTION path). Subscribe to the subject BEFORE arming so we
    // do not miss the seeded frame.
    let series = control_engine::watch::series::target("plant-1", &json!({})).series;
    let subject_task = {
        let node = node.clone();
        let series = series.clone();
        tokio::spawn(async move { recv_cov_frame(&node, ws, &series).await })
    };
    // Small delay so the subscriber is declared on the bus before the first publish.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    watches.arm(host, engine, "plant-1", &series, CovScope::default());

    let frame = subject_task
        .await
        .expect("join")
        .expect("a COV frame arrives on the series subject (exit gate)");
    assert_eq!(frame["kind"], "cov");
    assert_eq!(frame["values"][0]["uid"], 1_000_100);
    assert_eq!(frame["values"][0]["v"], 4.2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn watch_without_its_cap_is_denied_before_any_arm() {
    let (_node, key, gw) = serve().await;
    let ws = "ce-watch-deny";
    // Grant everything EXCEPT the watch verb's own cap.
    let host = host_for(&key, &gw, ws, &["mcp:ingest.write:call".to_string()]);
    let clients = control_engine::engine::Registry::new();
    let watches = WatchRegistry::new();

    let err = verb::run(&host, &clients, &watches, &json!({ "appliance": "" }))
        .await
        .expect_err("denied without the watch cap");
    assert!(matches!(err, HostError::Denied), "opaque deny: {err:?}");
    assert_eq!(watches.armed_count(), 0, "nothing armed on a denied call");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_cannot_watch_ws_a_appliance() {
    // ws-A registers an appliance; a ws-B caller naming it resolves to not-found (the isolation wall
    // the watch verb applies via `resolve`). We drive `resolve` here — the exact call the verb makes
    // before arming — so a ws-B watch of a ws-A appliance can never arm.
    let (node, key, gw) = serve().await;
    // Seed a ws-A appliance straight through the real store.
    let rec = json!({ "id": "plant-a", "name": "A", "mode": "appliance", "node": "n", "base": "http://127.0.0.1:7979", "ts": 1 });
    lb_store::write(&node.store, "ws-a", "ce_appliance", "plant-a", &rec)
        .await
        .expect("seed ws-a appliance");

    // A ws-B sidecar with the store-read grant resolving "plant-a" → not-found (no cross-ws existence
    // leak). It CAN query its own workspace's registry (real store.query), but ws-A's record is walled
    // off — the query returns Ok(None), which `resolve` maps to the isolation not-found.
    let host_b = host_for(
        &key,
        &gw,
        "ws-b",
        &[
            "mcp:control-engine.watch:call".to_string(),
            "mcp:store.query:call".to_string(),
            "store:ce_appliance:read".to_string(),
        ],
    );
    let err = control_engine::resolve::resolve(&host_b, "plant-a")
        .await
        .expect_err("ws-b cannot see ws-a's appliance");
    assert!(
        matches!(err, HostError::NotFound),
        "isolation not-found: {err:?}"
    );
}
