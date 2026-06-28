//! The generic bus routes over the real gateway (widget-config-vars scope, "Platform fix") — the
//! `POST /bus/publish` sink + the `GET /bus/{subject}/stream?token=` SSE feed, end to end. Mirrors the
//! series-stream test at the transport boundary: `401` without a token, `403` without the cap, a
//! reserved subject refused, and a real publish→watch round-trip over a live socket. No mock (CLAUDE §9).

mod common;

use std::sync::Arc;
use std::time::Duration;

use axum::http::StatusCode;
use common::*;
use lb_auth::SigningKey;
use lb_host::{Node, Role as NodeRole};
use lb_role_gateway::router;
use serde_json::json;
use tower::ServiceExt; // for `oneshot`

const BUS: &[&str] = &["mcp:bus.publish:call", "mcp:bus.watch:call"];

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_bus_stream_without_a_token_is_401() {
    let (gw, _key) = gateway().await;
    let resp = router(gw)
        .oneshot(get_req("/bus/stream?subject=cooler/alerts"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "no ?token= → 401");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn publish_without_the_cap_is_denied() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:nobody", "gw-bus-deny", &[]); // no bus caps
    let resp = router(gw)
        .oneshot(bearer(
            json_post("/bus/publish", json!({ "subject": "x", "payload": {} })),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN, "no cap → 403");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_reserved_subject_is_refused() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:ada", "gw-bus-wall", BUS);
    let resp = router(gw)
        .oneshot(bearer(
            json_post(
                "/bus/publish",
                json!({ "subject": "series/cpu", "payload": {} }),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "reserved subject → 400"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn publish_over_post_arrives_on_the_watch_sse() {
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let ws = "gw-bus-sse";
    let tok = token(&key, "user:ada", ws, BUS);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = router(gateway_on(node.clone(), &key));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Open the SSE stream first (Zenoh is not durable — the subscriber must exist before the publish).
    let client = reqwest::Client::new();
    let mut resp = client
        .get(format!(
            "http://{addr}/bus/stream?subject=cooler/alerts&token={tok}"
        ))
        .send()
        .await
        .expect("sse stream opens");
    assert_eq!(resp.status(), 200);

    // Give the subscriber a moment to declare interest, then publish over the real POST route.
    tokio::time::sleep(Duration::from_millis(200)).await;
    let pub_resp = client
        .post(format!("http://{addr}/bus/publish"))
        .bearer_auth(&tok)
        .json(&json!({ "subject": "cooler/alerts", "payload": { "msg": "defrost" } }))
        .send()
        .await
        .expect("publish posts");
    assert_eq!(pub_resp.status(), 200);

    let body = tokio::time::timeout(Duration::from_secs(5), async {
        let mut acc = String::new();
        while let Some(chunk) = resp.chunk().await.expect("read chunk") {
            acc.push_str(&String::from_utf8_lossy(&chunk));
            if acc.contains("defrost") {
                return acc;
            }
        }
        acc
    })
    .await
    .expect("the published message arrives within 5s");
    assert!(body.contains("event:message") || body.contains("event: message"));
    assert!(body.contains("defrost"));
}
