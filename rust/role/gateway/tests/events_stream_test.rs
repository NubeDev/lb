//! The **unified event stream** end to end over the real gateway (unified-event-stream scope). One
//! multiplexed SSE connection (`GET /events/stream`) + the `POST /events/{sid}/{subscribe,unsubscribe}`
//! control verbs, exercised against a live socket + a real node + real bus (no mock, CLAUDE §9). Covers
//! the mandatory categories: per-subject **capability deny** and **workspace isolation** (both an opaque
//! `error` mux frame, the connection staying up), plus **mux interleave**, **parity** with the dedicated
//! `/bus/stream` route, and **unsubscribe** releasing the subject (frames stop + the hub task is gone).
//!
//! Subjects are driven via `bus:{subject}` (`POST /bus/publish`) — the one feed a test can produce on
//! demand over HTTP with deterministic timing (Zenoh is fire-and-forget, so the subscribe must land
//! before the publish, exactly as the dedicated `bus_routes_test` does).

mod common;

use std::sync::Arc;
use std::time::Duration;

use axum::http::StatusCode;
use common::*;
use lb_auth::SigningKey;
use lb_host::{Node, Role as NodeRole};
use lb_role_gateway::{router, Gateway};
use serde_json::{json, Value};
use tower::ServiceExt; // for `oneshot`

const BUS: &[&str] = &["mcp:bus.publish:call", "mcp:bus.watch:call"];

/// Boot a real node + a gateway on a bound socket; return the base URL, the gateway (for hub asserts),
/// the signing key, and a spawned server. One helper for every live-socket case here.
async fn live_gateway() -> (String, Gateway, SigningKey) {
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let gw = gateway_on(node, &key);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = router(gw.clone());
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}"), gw, key)
}

/// Open the mux stream and read the `hello` frame, returning `(response, sid)`. The sid is what the
/// control POSTs address.
async fn open_stream(
    client: &reqwest::Client,
    base: &str,
    tok: &str,
) -> (reqwest::Response, String) {
    let mut resp = client
        .get(format!("{base}/events/stream?token={tok}"))
        .send()
        .await
        .expect("stream opens");
    assert_eq!(resp.status(), 200, "the mux stream opens 200");
    // The first frame is `event: hello` carrying `{sid}`.
    let hello = tokio::time::timeout(Duration::from_secs(5), async {
        let mut acc = String::new();
        while let Some(chunk) = resp.chunk().await.expect("read chunk") {
            acc.push_str(&String::from_utf8_lossy(&chunk));
            if acc.contains("event:hello") || acc.contains("event: hello") {
                return acc;
            }
        }
        acc
    })
    .await
    .expect("hello arrives within 5s");
    let sid = extract_sid(&hello);
    (resp, sid)
}

/// Pull the `sid` out of the `hello` frame's `data:` line (`data:{"sid":"…"}`).
fn extract_sid(frame: &str) -> String {
    let line = frame
        .lines()
        .find(|l| l.trim_start().starts_with("data:"))
        .expect("hello has a data line");
    let json = line.trim_start().trim_start_matches("data:").trim();
    let v: Value = serde_json::from_str(json).expect("hello data is json");
    v["sid"].as_str().expect("sid string").to_string()
}

/// POST a subscribe control verb for `subject` on `sid`.
async fn subscribe(
    client: &reqwest::Client,
    base: &str,
    sid: &str,
    tok: &str,
    subject: &str,
) -> reqwest::Response {
    client
        .post(format!("{base}/events/{sid}/subscribe"))
        .bearer_auth(tok)
        .json(&json!({ "subject": subject }))
        .send()
        .await
        .expect("subscribe posts")
}

/// Read from `resp` until `needle` appears (or 5s), returning everything read so far.
async fn read_until(resp: &mut reqwest::Response, needle: &str) -> String {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut acc = String::new();
        while let Some(chunk) = resp.chunk().await.expect("read chunk") {
            acc.push_str(&String::from_utf8_lossy(&chunk));
            if acc.contains(needle) {
                return acc;
            }
        }
        acc
    })
    .await
    .unwrap_or_default()
}

/// Publish `payload` onto `bus:{subject}`.
async fn publish(client: &reqwest::Client, base: &str, tok: &str, subject: &str, payload: Value) {
    let resp = client
        .post(format!("{base}/bus/publish"))
        .bearer_auth(tok)
        .json(&json!({ "subject": subject, "payload": payload }))
        .send()
        .await
        .expect("publish posts");
    assert_eq!(resp.status(), 200, "publish 200");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_stream_without_a_token_is_401() {
    // Auth-first: a bad/absent token is 401 before any connection registers (oneshot, no socket needed).
    let (gw, _key) = gateway().await;
    let resp = router(gw).oneshot(get_req("/events/stream")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "no ?token= → 401");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hello_frame_mints_a_sid_and_registers_the_connection() {
    let (base, gw, key) = live_gateway().await;
    let tok = token(&key, "user:ada", "gw-ev-hello", BUS);
    let client = reqwest::Client::new();
    let (_resp, sid) = open_stream(&client, &base, &tok).await;
    assert!(!sid.is_empty(), "hello carries a non-empty sid");
    // The connection is live in the hub (0 subjects until we subscribe one).
    assert_eq!(gw.events.subject_count(&sid).await, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_denied_subject_is_an_opaque_error_frame_the_connection_survives() {
    // MANDATORY capability-deny: a session WITHOUT `mcp:agent.watch:call` subscribes `run:{job}` → an
    // opaque per-subject error frame; the connection stays open and a PERMITTED subject on the SAME
    // connection still streams (scope: deny is a per-subscription error frame, never a connection kill).
    let (base, _gw, key) = live_gateway().await;
    // Has bus caps (so a permitted subject works) but NOT agent.watch (so `run:` denies).
    let tok = token(&key, "user:ada", "gw-ev-deny", BUS);
    let client = reqwest::Client::new();
    let (mut resp, sid) = open_stream(&client, &base, &tok).await;

    // Subscribe the DENIED run subject → 200 at the control layer (no oracle), an `error` frame on stream.
    let r = subscribe(&client, &base, &sid, &tok, "run:some-job").await;
    assert_eq!(
        r.status(),
        200,
        "subscribe control verb is always 200 when the conn exists"
    );
    let body = read_until(&mut resp, "\"event\":\"error\"").await;
    assert!(
        body.contains("\"sub\":\"run:some-job\"") && body.contains("\"event\":\"error\""),
        "denied subject → opaque error mux frame, got: {body}"
    );

    // The connection is ALIVE: a permitted `bus:` subject on the same connection still streams.
    let r = subscribe(&client, &base, &sid, &tok, "bus:cooler/alerts").await;
    assert_eq!(r.status(), 200);
    tokio::time::sleep(Duration::from_millis(200)).await;
    publish(
        &client,
        &base,
        &tok,
        "cooler/alerts",
        json!({ "msg": "defrost" }),
    )
    .await;
    let body = read_until(&mut resp, "defrost").await;
    assert!(
        body.contains("defrost") && body.contains("\"sub\":\"bus:cooler/alerts\""),
        "the permitted subject still streams on the same connection, got: {body}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_cross_workspace_subject_is_the_same_opaque_deny() {
    // MANDATORY workspace-isolation: a ws-B session subscribes a `bus:` subject and publishes on ws-A —
    // no frame ever crosses. The subscribe itself is walled by the token's workspace (the subject is
    // resolved inside ws-B only), so a ws-A publish is invisible. We prove the negative: after a ws-A
    // publish, the ws-B stream sees NOTHING for that subject (only its own hello).
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base = format!("http://{addr}");
    let app = router(gateway_on(node.clone(), &key));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let client = reqwest::Client::new();

    let tok_a = token(&key, "user:ada", "ws-A", BUS);
    let tok_b = token(&key, "user:bob", "ws-B", BUS);

    // ws-B opens the mux and subscribes `bus:cooler/alerts` (resolved inside ws-B).
    let (mut resp_b, sid_b) = open_stream(&client, &base, &tok_b).await;
    subscribe(&client, &base, &sid_b, &tok_b, "bus:cooler/alerts").await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    // ws-A publishes the SAME subject name in ITS workspace.
    publish(
        &client,
        &base,
        &tok_a,
        "cooler/alerts",
        json!({ "secret": "ws-A-only" }),
    )
    .await;

    // ws-B must NOT see ws-A's payload. Read for a bounded window; assert the secret never arrives.
    let body = read_until(&mut resp_b, "ws-A-only").await;
    assert!(
        !body.contains("ws-A-only"),
        "ws-B saw ws-A's payload — workspace wall breached: {body}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_subjects_interleave_on_one_connection_with_parity() {
    // Mux correctness + parity: subscribe TWO subjects on one connection, drive both, assert both feeds
    // interleave and each mux frame's `data` is byte-identical to what the dedicated `/bus/stream` route
    // emits for the same publish (the payload rides verbatim inside the envelope).
    let (base, _gw, key) = live_gateway().await;
    let tok = token(&key, "user:ada", "gw-ev-mux", BUS);
    let client = reqwest::Client::new();

    // The dedicated route's frame for the SAME publish, as the parity oracle.
    let mut dedicated = client
        .get(format!("{base}/bus/stream?subject=alpha&token={tok}"))
        .send()
        .await
        .expect("dedicated stream opens");
    assert_eq!(dedicated.status(), 200);

    let (mut resp, sid) = open_stream(&client, &base, &tok).await;
    subscribe(&client, &base, &sid, &tok, "bus:alpha").await;
    subscribe(&client, &base, &sid, &tok, "bus:beta").await;
    tokio::time::sleep(Duration::from_millis(250)).await;

    publish(&client, &base, &tok, "alpha", json!({ "v": 1 })).await;
    publish(&client, &base, &tok, "beta", json!({ "v": 2 })).await;

    // Both subjects arrive on the ONE mux connection.
    let mux_body = read_until(&mut resp, "\"sub\":\"bus:beta\"").await;
    assert!(
        mux_body.contains("\"sub\":\"bus:alpha\"") && mux_body.contains("\"sub\":\"bus:beta\""),
        "both subjects interleave on one connection, got: {mux_body}"
    );
    // The mux envelope embeds the payload verbatim — the same `{"v":1}` the dedicated route sends.
    let dedicated_body = read_until(&mut dedicated, "\"v\":1").await;
    assert!(
        dedicated_body.contains("\"v\":1"),
        "dedicated route sent the payload"
    );
    assert!(
        mux_body.contains("\"data\":{\"v\":1}"),
        "mux `data` is the dedicated payload byte-for-byte, got: {mux_body}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unsubscribe_stops_frames_and_releases_the_subject() {
    // After unsubscribe, further server-side events for that subject emit nothing on the stream AND the
    // hub's subject task (its bus subscription) is released (scope "Unsubscribe").
    let (base, gw, key) = live_gateway().await;
    let tok = token(&key, "user:ada", "gw-ev-unsub", BUS);
    let client = reqwest::Client::new();
    let (mut resp, sid) = open_stream(&client, &base, &tok).await;

    subscribe(&client, &base, &sid, &tok, "bus:room").await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        gw.events.subject_count(&sid).await,
        1,
        "one subject task after subscribe"
    );

    // Prove the subscription is live.
    publish(&client, &base, &tok, "room", json!({ "n": "first" })).await;
    let body = read_until(&mut resp, "first").await;
    assert!(
        body.contains("first"),
        "the subject streams while subscribed"
    );

    // Unsubscribe → the task is released.
    let r = client
        .post(format!("{base}/events/{sid}/unsubscribe"))
        .bearer_auth(&tok)
        .json(&json!({ "subject": "bus:room" }))
        .send()
        .await
        .expect("unsubscribe posts");
    assert_eq!(r.status(), 200);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        gw.events.subject_count(&sid).await,
        0,
        "the subject task is released after unsubscribe"
    );

    // A further publish emits nothing new for that subject.
    publish(&client, &base, &tok, "room", json!({ "n": "second" })).await;
    let body = read_until(&mut resp, "second").await;
    assert!(
        !body.contains("second"),
        "no frames after unsubscribe, got: {body}"
    );
}
