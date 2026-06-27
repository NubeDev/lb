//! The dashboard routes over the real gateway (dashboard scope, build step 3) — the `dashboard.*`
//! CRUD + the live **series** SSE feed, end to end. Mirrors the host tests at the transport boundary:
//! the CRUD round-trip, capability-deny per verb, two-session workspace isolation, the gate-3
//! visibility path, and the series stream (`401` without a token, a live `sample` over a real socket).
//! The gateway re-checks every gate server-side — the workspace + owner come from the token (§7).

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::*;
use lb_auth::SigningKey;
use lb_host::{Node, Qos, Role as NodeRole, Sample};
use lb_role_gateway::router;
use serde_json::{json, Value};
use tower::ServiceExt; // for `oneshot`

const CAPS: &[&str] = &[
    "mcp:dashboard.get:call",
    "mcp:dashboard.list:call",
    "mcp:dashboard.save:call",
    "mcp:dashboard.delete:call",
    "mcp:dashboard.share:call",
];

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn dashboard_crud_round_trip_over_the_gateway() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:ada", "acme", CAPS);

    // create
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/dashboards",
                json!({ "id": "ops", "title": "Ops", "cells": [] }),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // get
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/dashboards/ops"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let d: Value = json_body(resp).await;
    assert_eq!(d["title"], "Ops");
    assert_eq!(d["owner"], "user:ada");

    // roster includes it
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/dashboards"), &tok))
        .await
        .unwrap();
    let body: Value = json_body(resp).await;
    let ids: Vec<&str> = body["dashboards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"ops"));

    // delete → 204, then get is 404
    let resp = router(gw.clone())
        .oneshot(bearer(delete_req("/dashboards/ops"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/dashboards/ops"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn save_without_the_cap_is_denied_server_side() {
    let (gw, key) = gateway().await;
    // A token holding every dashboard cap EXCEPT save.
    let tok = token(
        &key,
        "user:ada",
        "acme",
        &[
            "mcp:dashboard.get:call",
            "mcp:dashboard.list:call",
            "mcp:dashboard.delete:call",
            "mcp:dashboard.share:call",
        ],
    );
    let resp = router(gw)
        .oneshot(bearer(
            json_post(
                "/dashboards",
                json!({ "id": "x", "title": "X", "cells": [] }),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN, "no save cap → 403");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn two_sessions_are_workspace_isolated() {
    // One node, two sessions in different workspaces — ws-B sees none of ws-A's dashboards.
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let ada = token(&key, "user:ada", "ws-a", CAPS);
    let ben = token(&key, "user:ben", "ws-b", CAPS);

    router(gateway_on(node.clone(), &key))
        .oneshot(bearer(
            json_post(
                "/dashboards",
                json!({ "id": "ops", "title": "Ops A", "cells": [] }),
            ),
            &ada,
        ))
        .await
        .unwrap();

    // Ben (ws-B) gets a 404 for ws-A's dashboard and an empty roster.
    let resp = router(gateway_on(node.clone(), &key))
        .oneshot(bearer(get_req("/dashboards/ops"), &ben))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let resp = router(gateway_on(node.clone(), &key))
        .oneshot(bearer(get_req("/dashboards"), &ben))
        .await
        .unwrap();
    let body: Value = json_body(resp).await;
    assert!(body["dashboards"].as_array().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn shared_workspace_visible_is_read_by_another_member() {
    // One node, two members of the SAME workspace. Ada shares her dashboard `workspace`-wide; Ben (a
    // different principal) can then read it — the gate-3 workspace tier.
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let ada = token(&key, "user:ada", "acme", CAPS);
    let ben = token(&key, "user:ben", "acme", CAPS);

    router(gateway_on(node.clone(), &key))
        .oneshot(bearer(
            json_post(
                "/dashboards",
                json!({ "id": "ops", "title": "Ops", "cells": [] }),
            ),
            &ada,
        ))
        .await
        .unwrap();

    // Private → Ben (not the owner) is denied.
    let resp = router(gateway_on(node.clone(), &key))
        .oneshot(bearer(get_req("/dashboards/ops"), &ben))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // Ada shares it workspace-wide.
    let resp = router(gateway_on(node.clone(), &key))
        .oneshot(bearer(
            json_post(
                "/dashboards/ops/share",
                json!({ "visibility": "workspace" }),
            ),
            &ada,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Now Ben reads it.
    let resp = router(gateway_on(node.clone(), &key))
        .oneshot(bearer(get_req("/dashboards/ops"), &ben))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_series_stream_without_a_token_is_401() {
    let (gw, _key) = gateway().await;
    let resp = router(gw)
        .oneshot(get_req("/series/cooler.temp/stream"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "no ?token= → 401");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_series_stream_pushes_a_live_sample() {
    use std::time::Duration;

    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let ws = "gw-series-sse";
    let tok = token(&key, "user:ada", ws, &["mcp:series.read:call"]);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = router(gateway_on(node.clone(), &key));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Open the SSE stream first (Zenoh pub/sub is not durable — the subscriber must exist before the
    // publish to receive it).
    let client = reqwest::Client::new();
    let mut resp = client
        .get(format!(
            "http://{addr}/series/cooler.temp/stream?token={tok}"
        ))
        .send()
        .await
        .expect("sse stream opens");
    assert_eq!(resp.status(), 200);

    // Give the subscriber a moment to declare interest, then publish a live sample onto the series
    // motion subject on the shared node.
    tokio::time::sleep(Duration::from_millis(200)).await;
    let sample = Sample {
        series: "cooler.temp".into(),
        producer: "user:ada".into(),
        ts: 1,
        seq: 1,
        payload: json!(3.4),
        labels: json!({}),
        qos: Qos::BestEffort,
    };
    lb_host::publish_sample(&node.bus, ws, &sample)
        .await
        .expect("publish motion");

    let body = tokio::time::timeout(Duration::from_secs(5), async {
        let mut acc = String::new();
        while let Some(chunk) = resp.chunk().await.expect("read chunk") {
            acc.push_str(&String::from_utf8_lossy(&chunk));
            if acc.contains("\"seq\":1") {
                return acc;
            }
        }
        acc
    })
    .await
    .expect("a live sample arrives within 5s");
    assert!(body.contains("event:sample") || body.contains("event: sample"));
}
